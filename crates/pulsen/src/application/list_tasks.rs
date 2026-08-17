//! タスク一覧のユースケース(UC-task-004)。
//!
//! ポート越しに走査し(`TaskRepository`)、絞り込みと破損の仕分けをここで済ませる。
//! 文言の組み立ては行わず、原因を値として返す。
//!
//! **`ExclusiveLock` を持たない。** 読み取りがロックを取らないこと(pages 共通事項・
//! 縮退表の「—(非取得)」)を、受け取れる型が無いことで示す。

use std::path::PathBuf;

use pulsen_domain::definition::{StatusName, WorkflowName};
use pulsen_domain::task::{
    BranchName, ExecutionStateKind, ReadError, RepoPath, StateKindError, TaskEntry, TaskId,
    TaskRecord, TaskRepository, Timestamp,
};

/// 一覧の入力。
///
/// `status` / `state` は CLI が受け取った文字列のまま入る。ドメイン型への写像は
/// このユースケースの入力境界で一度だけ行う。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ListTasksInput {
    /// `--status` の値。ユーザー定義語彙のため検証しない。
    pub status: Option<String>,
    /// `--state` の値。固定6値のみを受理する。
    pub state: Option<String>,
    /// `--all`。アーカイブ済みも対象集合に含める。
    pub all: bool,
}

/// 一覧に並ぶタスク1件。
///
/// `snapshot_unreadable` が立っていても行として出る — 実行状態とタスクステータスは
/// 読めており、絞り込みの対象になる。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskRow {
    /// タスクID。
    pub task_id: TaskId,
    /// ワークフローの表示名。
    pub workflow_name: WorkflowName,
    /// 対象のリポジトリ。
    pub repo: RepoPath,
    /// 確定済みワークスペースのブランチ。未確定なら `None`。
    pub branch: Option<BranchName>,
    /// タスクステータス。
    pub task_status: StatusName,
    /// 実行状態。
    pub execution_state: ExecutionStateKind,
    /// 連続した実行失敗の数。
    pub attempt_count: u32,
    /// 最終更新時刻。
    pub updated_at: Timestamp,
    /// アーカイブ側で見つかったか。
    pub archived: bool,
    /// スナップショットだけが読めないか。
    pub snapshot_unreadable: bool,
}

/// 読み取れなかったタスクファイル1件。
///
/// ここに来るのは `Corrupt` だけ — タスクIDもステータスも読めておらず、行の列を
/// 埋められない。パスを添えることが修復の入口になる(pages ※5)。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnreadableRow {
    /// 対象のファイル。
    pub path: PathBuf,
    /// 読めない理由。
    pub message: String,
}

/// 一覧の結果。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TaskList {
    /// 絞り込み後の行。
    pub rows: Vec<TaskRow>,
    /// 読み取れなかったファイル。絞り込みの影響を受けない。
    pub unreadable: Vec<UnreadableRow>,
}

/// 一覧の失敗。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListTasksError {
    /// `--state` が固定6値のいずれでもない。
    InvalidState(StateKindError),
    /// 走査自体ができない。
    Scan(ReadError),
}

/// 現役(と、要求されればアーカイブ)のタスクを走査して絞り込む。
///
/// ポートはジェネリック引数で受け取り、実アダプターとテストダブルのどちらにも同じ
/// 制御フローが乗ることを型で示す。
pub struct ListTasks<'a, R> {
    tasks: &'a R,
}

impl<'a, R> ListTasks<'a, R>
where
    R: TaskRepository,
{
    /// タスクの永続化だけを結線する。
    ///
    /// 排他ロックもグローバル設定も取らない — 一覧は状態を変更せず、config の項目を
    /// 1つも参照しない(起動時の読み込み自体は全コマンド共通の処理。pages ※1)。
    pub fn new(tasks: &'a R) -> Self {
        Self { tasks }
    }

    /// spec の処理フローの順に実行する。
    pub fn execute(&self, input: ListTasksInput) -> Result<TaskList, ListTasksError> {
        // ロックを取らない読み取りで、`state` の可否は走査結果に依存しない。入力の誤りを
        // 走査の成否に従属させないため入力境界で先に弾く。
        let state = match input.state {
            Some(given) => {
                Some(ExecutionStateKind::parse(&given).map_err(ListTasksError::InvalidState)?)
            }
            None => None,
        };

        let mut list = TaskList::default();
        self.collect(
            self.tasks.list_active().map_err(ListTasksError::Scan)?,
            false,
            &mut list,
        );
        if input.all {
            self.collect(
                self.tasks.list_archived().map_err(ListTasksError::Scan)?,
                true,
                &mut list,
            );
        }

        // 絞り込みは対象集合の拡張(`all`)を済ませた後の集合に適用する。
        list.rows.retain(|row| {
            matches(&input.status, &row.task_status)
                && state.is_none_or(|s| s == row.execution_state)
        });
        Ok(list)
    }

    /// 走査結果を行と読み取り不能に仕分ける。
    fn collect(&self, entries: Vec<TaskEntry>, archived: bool, list: &mut TaskList) {
        for entry in entries {
            match entry {
                TaskEntry::Record(TaskRecord::Intact(task)) => list.rows.push(TaskRow {
                    task_id: task.id().clone(),
                    workflow_name: task.workflow_name().clone(),
                    repo: task.target().repo().clone(),
                    branch: task.workspace().map(|ws| ws.branch().clone()),
                    task_status: task.task_status().clone(),
                    execution_state: task.execution_kind(),
                    attempt_count: task.counters().attempt_count(),
                    updated_at: task.updated_at(),
                    archived,
                    snapshot_unreadable: false,
                }),
                TaskEntry::Record(TaskRecord::SnapshotUnreadable(task)) => {
                    list.rows.push(TaskRow {
                        task_id: task.id().clone(),
                        workflow_name: task.workflow_name().clone(),
                        repo: task.target().repo().clone(),
                        branch: task.workspace().map(|ws| ws.branch().clone()),
                        task_status: task.task_status().clone(),
                        execution_state: task.execution_kind(),
                        attempt_count: task.counters().attempt_count(),
                        updated_at: task.updated_at(),
                        archived,
                        snapshot_unreadable: true,
                    })
                }
                TaskEntry::Corrupt { path, message } => {
                    list.unreadable.push(UnreadableRow { path, message });
                }
            }
        }
    }
}

/// タスクステータスの絞り込み。未知の値は一致しない(該当0件)。
fn matches(status: &Option<String>, actual: &StatusName) -> bool {
    status
        .as_deref()
        .is_none_or(|expected| expected == actual.as_str())
}
