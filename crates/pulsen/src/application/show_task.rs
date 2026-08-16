//! タスク詳細のユースケース(UC-task-005)。
//!
//! ポート越しに読み(`TaskRepository::find` / `RunStore`)、縮退の場合分けをここで
//! 直和型に畳む。文言の組み立ては行わず、原因を値として返す。
//!
//! **`ExclusiveLock` を持たない。** 読み取りがロックを取らないこと(pages 共通事項・
//! 縮退表の「—(非取得)」)を、受け取れる型が無いことで示す。
//!
//! **`workspace` のパスの存在検証は行わない**(pages ※9)。アーカイブ済みの
//! 「worktree は削除済み」注記も、アーカイブ済みであるという事実から導く。

use std::path::PathBuf;

use pulsen_domain::definition::{GlobalConfig, StatusName, WorkflowName};
use pulsen_domain::execution::{ExitCode, Io, RunFileError, RunStore};
use pulsen_domain::task::{
    AttemptNumber, AttemptRef, DegradedTask, ExecutionState, ExecutionStateKind, FailureNote,
    ProcessIdent, ReadError, RetryCounters, RunDirPath, StateRoot, StopReason, Target, Task,
    TaskFilePath, TaskId, TaskIdError, TaskLookup, TaskRecord, TaskRepository, Timestamp,
    Workspace,
};

/// 詳細の入力。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShowTaskInput {
    /// `<task-id>` の値。
    pub task_id: String,
}

/// 現ステータスに適用されるリトライ上限。
///
/// 「適用対象がない」(Wait)と「導出できない」(スナップショット破損)を
/// `Option<u32>` に潰さない — 前者は併記なし、後者は導出不能である旨の注記になる。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetryLimitInfo {
    /// 適用される上限。
    Applicable(u32),
    /// 適用対象がない(attempt_count を消費する操作を持たないステータス)。
    NotApplicable,
    /// スナップショットが読めず導出できない。
    Unknown,
}

/// 各カウンタに併記する上限。
///
/// judge / spawn はスナップショットに依存しないため、縮退時も config の値で常に埋まる。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// リトライ上限。
    pub retry: RetryLimitInfo,
    /// 判定失敗の上限。
    pub judge: u32,
    /// spawn 失敗の上限。
    pub spawn: u32,
}

/// attempt の run ディレクトリの有無。
///
/// 「不在」と「確認できなかった」を同じ値に潰さない — 前者は gc 済み・未作成という
/// 正常な状態、後者は観測の失敗であり、次に取る行動が違う。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunDirPresence {
    /// 存在する。
    Present,
    /// 存在しない(未作成・gc 済み)。
    Absent,
    /// 存在確認自体が失敗した。
    Unknown {
        /// 原因の説明。
        message: String,
    },
}

/// exit ファイルの読み取り結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExitInfo {
    /// 記録された終了コード。
    Recorded(ExitCode),
    /// 記録がない。
    Absent,
    /// 読み取りが失敗した。
    Unreadable {
        /// 原因の説明。
        message: String,
    },
    /// run ディレクトリの有無を確かめられず、読みに行っていない。
    Unread,
}

/// 現在 attempt の実行メタデータ。
///
/// 値そのものが `None` になりうる `process` と、attempt が「なし」であることを
/// 表す `Option<AttemptSummary>` を分けることで、「attempt なし」と「attempt あり・
/// 同定情報未取得」を区別する。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttemptSummary {
    /// attempt 番号。
    pub number: AttemptNumber,
    /// run ディレクトリ。
    pub run_dir: RunDirPath,
    /// プロセス同定情報。起動確認前は `None`(「未取得」)。
    pub process: Option<ProcessIdent>,
    /// exit の読み取り結果。
    pub exit: ExitInfo,
    /// 標準出力ログのパス。
    pub stdout_log: PathBuf,
    /// 標準エラーログのパス。
    pub stderr_log: PathBuf,
    /// run ディレクトリ自体の有無。
    pub run_dir_exists: RunDirPresence,
}

/// 凍結の要因と通知。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StopInfo {
    /// 凍結に至った経路。
    pub reason: StopReason,
    /// 通知済みの時刻。未通知は `None`。
    pub notified_at: Option<Timestamp>,
}

/// タスク1件の詳細。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskDetail {
    /// タスクID。
    pub task_id: TaskId,
    /// ワークフローの表示名。
    pub workflow_name: WorkflowName,
    /// 対象のリポジトリとベースブランチ。
    pub target: Target,
    /// タスクステータス。
    pub task_status: StatusName,
    /// 実行状態。
    pub execution_state: ExecutionStateKind,
    /// 確定済みワークスペース。`None` は「未作成」。
    pub workspace: Option<Workspace>,
    /// 3つのカウンタ。
    pub counters: RetryCounters,
    /// カウンタに併記する上限。
    pub limits: Limits,
    /// 現在 attempt。`None` は「なし」。
    pub attempt: Option<AttemptSummary>,
    /// 直近のツール操作・判定の失敗。
    pub last_failure: Option<FailureNote>,
    /// 凍結の要因と通知。stopped のときだけ `Some`。
    pub stop_info: Option<StopInfo>,
    /// スナップショットの定義済みステータス一覧。破損時は `None`。
    pub defined_statuses: Option<Vec<StatusName>>,
    /// スナップショットが読めない理由。破損時だけ `Some`。
    pub snapshot_error: Option<String>,
    /// スナップショットの保存先(タスクファイル自身。ADR-015)。
    pub task_file_path: PathBuf,
    /// アーカイブ側で見つかったか。
    pub archived: bool,
    /// 最終更新時刻。
    pub updated_at: Timestamp,
}

/// 詳細の失敗。
///
/// いずれの場合も書き込みを行わない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShowTaskError {
    /// タスクIDとして解釈できない。
    InvalidTaskId(TaskIdError),
    /// 現役にもアーカイブにも存在しない。
    NotFound {
        /// 指定されたタスクID。
        task_id: TaskId,
    },
    /// タスクファイル全体が読めない。
    Corrupt {
        /// 対象のファイル。
        path: PathBuf,
        /// 読めない理由。
        message: String,
    },
    /// 走査自体ができない。
    Read(ReadError),
}

/// タスク1件を解決し、実行メタデータを補って詳細に組み立てる。
///
/// ポートはジェネリック引数で受け取り、実アダプターとテストダブルのどちらにも同じ
/// 制御フローが乗ることを型で示す。
pub struct ShowTask<'a, R, S> {
    config: &'a GlobalConfig,
    state_root: &'a StateRoot,
    tasks: &'a R,
    runs: &'a S,
}

impl<'a, R, S> ShowTask<'a, R, S>
where
    R: TaskRepository,
    S: RunStore,
{
    /// 読み込み済みのグローバル設定と各ポートを結線する。
    ///
    /// 排他ロックは取らない。グローバル設定は judge / spawn の上限の出どころとして要る。
    pub fn new(
        config: &'a GlobalConfig,
        state_root: &'a StateRoot,
        tasks: &'a R,
        runs: &'a S,
    ) -> Self {
        Self {
            config,
            state_root,
            tasks,
            runs,
        }
    }

    /// spec の処理フローの順に実行する。
    pub fn execute(&self, input: ShowTaskInput) -> Result<TaskDetail, ShowTaskError> {
        let id = TaskId::parse(input.task_id).map_err(ShowTaskError::InvalidTaskId)?;

        // 解決順(現役 → アーカイブ)は `find` の契約に委ねる。
        match self.tasks.find(&id).map_err(ShowTaskError::Read)? {
            TaskLookup::Active(record) => Ok(self.detail(record, false)),
            TaskLookup::Archived(record) => Ok(self.detail(record, true)),
            TaskLookup::NotFound => Err(ShowTaskError::NotFound { task_id: id }),
            TaskLookup::Corrupt { path, message } => Err(ShowTaskError::Corrupt { path, message }),
        }
    }

    /// 読めたタスクを詳細に組み立てる。
    fn detail(&self, record: TaskRecord, archived: bool) -> TaskDetail {
        match record {
            TaskRecord::Intact(task) => self.intact(task, archived),
            TaskRecord::SnapshotUnreadable(task) => self.degraded(task, archived),
        }
    }

    /// スナップショットまで読めたタスク。
    fn intact(&self, task: Task, archived: bool) -> TaskDetail {
        let retry = match task.applicable_retry_limit() {
            Some(limit) => RetryLimitInfo::Applicable(limit),
            None => RetryLimitInfo::NotApplicable,
        };
        let defined_statuses = task.snapshot().statuses().keys().cloned().collect();

        TaskDetail {
            task_id: task.id().clone(),
            workflow_name: task.workflow_name().clone(),
            target: task.target().clone(),
            task_status: task.task_status().clone(),
            execution_state: task.execution_kind(),
            workspace: task.workspace().cloned(),
            counters: task.counters(),
            limits: self.limits(retry),
            attempt: self.attempt(task.current_attempt()),
            last_failure: task.last_failure().cloned(),
            stop_info: stop_info(task.execution()),
            defined_statuses: Some(defined_statuses),
            snapshot_error: None,
            task_file_path: self.task_file_path(task.id(), archived),
            archived,
            updated_at: task.updated_at(),
        }
    }

    /// スナップショットだけが読めないタスク。
    ///
    /// 読める項目はすべて載せ、スナップショット由来の項目(定義済みステータス一覧・
    /// リトライ上限)だけを落として理由を添える(pages ※6)。
    fn degraded(&self, task: DegradedTask, archived: bool) -> TaskDetail {
        TaskDetail {
            task_id: task.id().clone(),
            workflow_name: task.workflow_name().clone(),
            target: task.target().clone(),
            task_status: task.task_status().clone(),
            execution_state: task.execution_kind(),
            workspace: task.workspace().cloned(),
            counters: task.counters(),
            limits: self.limits(RetryLimitInfo::Unknown),
            attempt: self.attempt(task.current_attempt()),
            last_failure: task.last_failure().cloned(),
            stop_info: stop_info(task.execution()),
            defined_statuses: None,
            snapshot_error: Some(task.snapshot_error().to_owned()),
            task_file_path: self.task_file_path(task.id(), archived),
            archived,
            updated_at: task.updated_at(),
        }
    }

    /// judge / spawn の上限は config から常に埋める(スナップショット非依存)。
    fn limits(&self, retry: RetryLimitInfo) -> Limits {
        Limits {
            retry,
            judge: self.config.judge_attempt_limit(),
            spawn: self.config.spawn_fail_limit(),
        }
    }

    /// スナップショットの保存先はタスクファイル自身(ADR-015)。
    fn task_file_path(&self, id: &TaskId, archived: bool) -> PathBuf {
        if archived {
            TaskFilePath::archived(self.state_root, id)
        } else {
            TaskFilePath::active(self.state_root, id)
        }
    }

    /// 現在 attempt に実行メタデータを補う。
    ///
    /// 存在確認と exit の読み取りの失敗は**エラーに昇格させない** — 当該項目に
    /// 読めない旨を載せて表示を続ける(pages 縮退表 show 行)。
    fn attempt(&self, attempt: Option<&AttemptRef>) -> Option<AttemptSummary> {
        let attempt = attempt?;
        let run_dir = attempt.run_dir();

        let presence = match self.runs.attempt_exists(run_dir) {
            Ok(true) => RunDirPresence::Present,
            Ok(false) => RunDirPresence::Absent,
            Err(Io::Failed { message }) => RunDirPresence::Unknown { message },
        };
        let exit = match presence {
            RunDirPresence::Present => match self.runs.read_exit(run_dir) {
                Ok(Some(code)) => ExitInfo::Recorded(code),
                Ok(None) => ExitInfo::Absent,
                Err(RunFileError::Corrupt { path, message }) => ExitInfo::Unreadable {
                    message: format!("{}: {message}", path.display()),
                },
                Err(RunFileError::Io { message }) => ExitInfo::Unreadable { message },
            },
            // ディレクトリごと無いなら exit ファイルも無い。
            RunDirPresence::Absent => ExitInfo::Absent,
            // 有無を確かめられていない以上、記録の不在も主張できない。
            RunDirPresence::Unknown { .. } => ExitInfo::Unread,
        };

        Some(AttemptSummary {
            number: attempt.number(),
            run_dir: run_dir.clone(),
            process: attempt.process().cloned(),
            exit,
            stdout_log: run_dir.stdout_log(),
            stderr_log: run_dir.stderr_log(),
            run_dir_exists: presence,
        })
    }
}

/// 凍結の要因と通知。凍結していないタスクは持たない。
fn stop_info(execution: &ExecutionState) -> Option<StopInfo> {
    match execution {
        ExecutionState::Stopped {
            reason,
            notified_at,
        } => Some(StopInfo {
            reason: *reason,
            notified_at: *notified_at,
        }),
        ExecutionState::Pending
        | ExecutionState::Launching { .. }
        | ExecutionState::Running
        | ExecutionState::Completed
        | ExecutionState::Failed => None,
    }
}
