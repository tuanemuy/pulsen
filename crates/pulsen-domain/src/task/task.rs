//! タスク集約ルート。

use crate::definition::{StatusDefinition, StatusName, WorkflowName, WorkflowSnapshot};

use super::attempt::{AttemptNumber, AttemptRef};
use super::branch::{Target, Workspace};
use super::counters::RetryCounters;
use super::failure::{FailureKind, FailureNote, ToolFailureKind};
use super::id::TaskId;
use super::path::{RunDirPath, StateRoot};
use super::process::ProcessIdent;
use super::state::{ExecutionState, ExecutionStateKind, StopReason};
use super::time::Timestamp;
use super::transition::TransitionError;

/// 永続化からの再構築の失敗。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RehydrateError {
    /// 不変条件1の破れ — タスクステータスがスナップショットに存在しない。
    ///
    /// TaskRepository のアダプターはこれをスナップショット読み取り不能へ写像する。
    StatusNotInSnapshot {
        /// 記録されていたタスクステータス。
        status: StatusName,
        /// スナップショットに定義されているステータス。
        defined: Vec<StatusName>,
    },
}

/// 永続化されたタスクの全フィールド。
///
/// 再構築の入力を1つの値にまとめ、フィールドの追加が呼び出し側の引数順に依存しないようにする。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskFields {
    /// タスクID。
    pub id: TaskId,
    /// 表示名。
    pub workflow_name: WorkflowName,
    /// 対象のリポジトリとベースブランチ。
    pub target: Target,
    /// 埋め込まれたワークフロー定義。
    pub snapshot: WorkflowSnapshot,
    /// タスクステータス。
    pub task_status: StatusName,
    /// 実行状態。
    pub execution: ExecutionState,
    /// 確定済みのワークスペース。
    pub workspace: Option<Workspace>,
    /// 現在 attempt への参照。
    pub current_attempt: Option<AttemptRef>,
    /// カウンタ。
    pub counters: RetryCounters,
    /// 直近の失敗要因。
    pub last_failure: Option<FailureNote>,
    /// 最終更新時刻。
    pub updated_at: Timestamp,
}

/// タスク — スケジューラーの帳簿の集約ルート。
///
/// 生成経路は新規登録(`register`)と永続化からの再構築(`rehydrate`)の2つだけ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Task {
    id: TaskId,
    workflow_name: WorkflowName,
    target: Target,
    snapshot: WorkflowSnapshot,
    task_status: StatusName,
    execution: ExecutionState,
    workspace: Option<Workspace>,
    current_attempt: Option<AttemptRef>,
    counters: RetryCounters,
    last_failure: Option<FailureNote>,
    updated_at: Timestamp,
}

impl Task {
    /// 新規登録。タスクステータスは初期ステータス、実行状態は起動待ちになる。
    pub fn register(
        id: TaskId,
        workflow_name: WorkflowName,
        target: Target,
        snapshot: WorkflowSnapshot,
        now: Timestamp,
    ) -> Self {
        let task_status = snapshot.initial().clone();
        Self {
            id,
            workflow_name,
            target,
            snapshot,
            task_status,
            execution: ExecutionState::Pending,
            workspace: None,
            current_attempt: None,
            counters: RetryCounters::initial(),
            last_failure: None,
            updated_at: now,
        }
    }

    /// 永続化からの再構築。
    ///
    /// 不変条件1(`task_status ∈ snapshot.statuses`)だけを検証する。状態間整合の
    /// 不変条件2〜4は手動修復で破られたまま再構築されうるため、遷移関数の前提検査に委ねる。
    pub fn rehydrate(fields: TaskFields) -> Result<Self, RehydrateError> {
        if fields.snapshot.status(&fields.task_status).is_none() {
            return Err(RehydrateError::StatusNotInSnapshot {
                status: fields.task_status,
                defined: fields.snapshot.statuses().keys().cloned().collect(),
            });
        }

        Ok(Self {
            id: fields.id,
            workflow_name: fields.workflow_name,
            target: fields.target,
            snapshot: fields.snapshot,
            task_status: fields.task_status,
            execution: fields.execution,
            workspace: fields.workspace,
            current_attempt: fields.current_attempt,
            counters: fields.counters,
            last_failure: fields.last_failure,
            updated_at: fields.updated_at,
        })
    }

    /// タスクID。
    pub fn id(&self) -> &TaskId {
        &self.id
    }

    /// ワークフローの表示名。
    pub fn workflow_name(&self) -> &WorkflowName {
        &self.workflow_name
    }

    /// 対象のリポジトリとベースブランチ。
    pub fn target(&self) -> &Target {
        &self.target
    }

    /// 埋め込まれたワークフロー定義。
    pub fn snapshot(&self) -> &WorkflowSnapshot {
        &self.snapshot
    }

    /// タスクステータス。
    pub fn task_status(&self) -> &StatusName {
        &self.task_status
    }

    /// 実行状態。
    pub fn execution(&self) -> &ExecutionState {
        &self.execution
    }

    /// 確定済みのワークスペース。未確定は `None`。
    pub fn workspace(&self) -> Option<&Workspace> {
        self.workspace.as_ref()
    }

    /// 現在 attempt への参照。
    pub fn current_attempt(&self) -> Option<&AttemptRef> {
        self.current_attempt.as_ref()
    }

    /// カウンタ。
    pub fn counters(&self) -> RetryCounters {
        self.counters
    }

    /// 直近の失敗要因。
    pub fn last_failure(&self) -> Option<&FailureNote> {
        self.last_failure.as_ref()
    }

    /// 最終更新時刻。
    pub fn updated_at(&self) -> Timestamp {
        self.updated_at
    }

    /// 実行状態の判別子。
    ///
    /// 実行状態に付随する値ではなく判別だけが要る経路(凍結したかの判定など)の読み取り口。
    /// 判別と同時に付随する値が要る経路は `execution()` の網羅 `match` を通る。
    pub fn execution_kind(&self) -> ExecutionStateKind {
        self.execution.kind()
    }

    /// 現ステータスの定義。
    ///
    /// 不変条件1(`task_status ∈ snapshot.statuses`)は再構築・登録の両方の生成経路で
    /// 保証されるため全域関数になる。
    pub fn current_status_def(&self) -> &StatusDefinition {
        self.snapshot
            .status(&self.task_status)
            .expect("不変条件1: タスクステータスはスナップショットに定義されている")
    }

    /// 次に採番される attempt 番号。まだ attempt が無ければ最初の番号。
    pub fn next_attempt_number(&self) -> AttemptNumber {
        self.current_attempt
            .as_ref()
            .map_or(AttemptNumber::FIRST, |attempt| attempt.number().next())
    }

    /// 現ステータスがエージェント実行か。
    ///
    /// 動作種別の問い合わせ3種は spec が一組で定める読み取り口。真偽だけが要る経路
    /// (`record_launching` の前提検査など)がこれらを使い、判別と同時に定義の中身が要る
    /// 経路は `current_status_def` の網羅 `match` を通る — tick の分岐がこちらで、
    /// 動作種別ごとに違う値を取り出すため真偽では足りない(ADR-091)。
    pub fn is_agent_run(&self) -> bool {
        match self.current_status_def() {
            StatusDefinition::AgentRun { .. } => true,
            StatusDefinition::Wait | StatusDefinition::Cleanup => false,
        }
    }

    /// 現ステータスが待機か。
    ///
    /// 3種を一組で置く理由は [`Task::is_agent_run`] を参照。
    pub fn is_wait(&self) -> bool {
        match self.current_status_def() {
            StatusDefinition::Wait => true,
            StatusDefinition::AgentRun { .. } | StatusDefinition::Cleanup => false,
        }
    }

    /// 現ステータスがクリーンアップか。
    ///
    /// 3種を一組で置く理由は [`Task::is_agent_run`] を参照。
    pub fn is_cleanup(&self) -> bool {
        match self.current_status_def() {
            StatusDefinition::Cleanup => true,
            StatusDefinition::AgentRun { .. } | StatusDefinition::Wait => false,
        }
    }

    /// 現ステータスに適用されるリトライ上限。
    ///
    /// 待機は attempt_count を消費する操作を持たないため適用対象がない。クリーンアップの
    /// 上限は組み込みデフォルトであり、その値は実効リトライ上限の解決から返る。
    pub fn applicable_retry_limit(&self) -> Option<u32> {
        match self.current_status_def() {
            StatusDefinition::AgentRun { .. } | StatusDefinition::Cleanup => {
                Some(self.snapshot.effective_retry_limit(&self.task_status))
            }
            StatusDefinition::Wait => None,
        }
    }

    /// ワークスペースを確定する。
    ///
    /// 前提: 未確定。確定済みのワークスペースは変更されない(不変条件6)。
    pub fn confirm_workspace(
        self,
        workspace: Workspace,
        now: Timestamp,
    ) -> Result<Self, TransitionError> {
        if self.workspace.is_some() {
            return Err(TransitionError::WorkspaceAlreadySet);
        }

        Ok(Self {
            workspace: Some(workspace),
            updated_at: now,
            ..self
        })
    }

    /// 起動を記録する。次の attempt を採番し、導出した run ディレクトリを返す。
    ///
    /// 前提: 起動待ちまたは失敗確定・エージェント実行ステータス・ワークスペース確定済み。
    /// 返す run ディレクトリで呼び出し側が attempt の準備と spawn を行う。
    pub fn record_launching(
        self,
        state_root: &StateRoot,
        now: Timestamp,
    ) -> Result<(Self, RunDirPath), TransitionError> {
        self.ensure_restartable()?;
        if !self.is_agent_run() {
            return Err(TransitionError::NotAgentRunStatus {
                status: self.task_status.clone(),
            });
        }
        if self.workspace.is_none() {
            return Err(TransitionError::WorkspaceNotSet);
        }

        let number = self.next_attempt_number();
        let run_dir = RunDirPath::derive(state_root, &self.id, number);
        let recorded = Self {
            execution: ExecutionState::Launching { recorded_at: now },
            current_attempt: Some(AttemptRef::launching(number, run_dir.clone())),
            updated_at: now,
            ..self
        };
        Ok((recorded, run_dir))
    }

    /// 起動を確認し、同定情報を取り込む。
    ///
    /// 前提: 起動記録済み。リセットするのは spawn_fail_count だけで、実行の失敗を数える
    /// attempt_count・judge_attempt_count は保持する(リセットは判定の確定と人間の操作のみ)。
    pub fn confirm_running(
        mut self,
        process: ProcessIdent,
        now: Timestamp,
    ) -> Result<Self, TransitionError> {
        self.ensure_launching()?;
        let attempt = self
            .current_attempt
            .take()
            .ok_or(TransitionError::MissingCurrentAttempt)?;

        Ok(Self {
            execution: ExecutionState::Running,
            current_attempt: Some(attempt.with_process(process)),
            counters: self.counters.reset_spawn_fail(),
            updated_at: now,
            ..self
        })
    }

    /// 猶予時間を超えた spawn 失敗を記録する。
    ///
    /// 前提: 起動記録済み。上限を超えなければ起動待ちへ戻し、次の tick が再起動する。
    pub fn record_spawn_failure(
        self,
        message: String,
        spawn_fail_limit: u32,
        now: Timestamp,
    ) -> Result<Self, TransitionError> {
        self.ensure_launching()?;

        let counters = self.counters.increment_spawn_fail();
        let execution = if limit_exceeded(counters.spawn_fail_count(), spawn_fail_limit) {
            stopped(StopReason::SpawnFailLimitExceeded)
        } else {
            ExecutionState::Pending
        };

        Ok(Self {
            execution,
            counters,
            last_failure: Some(FailureNote::record(FailureKind::SpawnFail, message, now)),
            updated_at: now,
            ..self
        })
    }

    /// テンプレート展開の失敗による同期的な spawn 失敗を記録する。
    ///
    /// 前提: 起動待ちまたは失敗確定。attempt を採番せず、上限を超えなければ実行状態も
    /// 変えない(起動を試みる前の失敗であり、attempt は消費されていない)。
    pub fn record_spawn_failure_in_place(
        self,
        message: String,
        spawn_fail_limit: u32,
        now: Timestamp,
    ) -> Result<Self, TransitionError> {
        self.ensure_restartable()?;

        let counters = self.counters.increment_spawn_fail();
        let execution = if limit_exceeded(counters.spawn_fail_count(), spawn_fail_limit) {
            stopped(StopReason::SpawnFailLimitExceeded)
        } else {
            self.execution.clone()
        };

        Ok(Self {
            execution,
            counters,
            last_failure: Some(FailureNote::record(FailureKind::SpawnFail, message, now)),
            updated_at: now,
            ..self
        })
    }

    /// ツール操作(worktree の作成・削除、アーカイブ移動)の失敗を記録する。
    ///
    /// 前提: 起動待ちまたは失敗確定。
    pub fn record_tool_failure(
        self,
        kind: ToolFailureKind,
        message: String,
        retry_limit: u32,
        now: Timestamp,
    ) -> Result<Self, TransitionError> {
        self.ensure_restartable()?;

        let counters = self.counters.increment_attempt();
        let execution = if limit_exceeded(counters.attempt_count(), retry_limit) {
            stopped(StopReason::RetryLimitExceeded)
        } else {
            ExecutionState::Failed
        };

        Ok(Self {
            execution,
            counters,
            last_failure: Some(FailureNote::record(kind.recorded(), message, now)),
            updated_at: now,
            ..self
        })
    }

    /// 再起動できる状態(起動待ち・失敗確定)であることを検査する。
    fn ensure_restartable(&self) -> Result<(), TransitionError> {
        match self.execution {
            ExecutionState::Pending | ExecutionState::Failed => Ok(()),
            ExecutionState::Launching { .. }
            | ExecutionState::Running
            | ExecutionState::Completed
            | ExecutionState::Stopped { .. } => Err(TransitionError::InvalidState {
                expected: RESTARTABLE,
                actual: self.execution.kind(),
            }),
        }
    }

    /// 起動記録済みであることを検査する。
    fn ensure_launching(&self) -> Result<(), TransitionError> {
        match self.execution {
            ExecutionState::Launching { .. } => Ok(()),
            ExecutionState::Pending
            | ExecutionState::Running
            | ExecutionState::Completed
            | ExecutionState::Failed
            | ExecutionState::Stopped { .. } => Err(TransitionError::InvalidState {
                expected: LAUNCHING,
                actual: self.execution.kind(),
            }),
        }
    }
}

/// 再起動できる状態(前提の不一致の報告に使う)。
const RESTARTABLE: &[ExecutionStateKind] =
    &[ExecutionStateKind::Pending, ExecutionStateKind::Failed];
/// 起動記録済みの状態。
const LAUNCHING: &[ExecutionStateKind] = &[ExecutionStateKind::Launching];

/// 上限の超過は加算後の値が上限を**上回った**ときにのみ成立する(等号では凍結しない)。
///
/// 3つの遷移が同じ規則を使うため1箇所に集約する。
fn limit_exceeded(count: u32, limit: u32) -> bool {
    count > limit
}

/// 凍結の記録。過去の通知記録は引き継がない(常に未通知として記録する)。
fn stopped(reason: StopReason) -> ExecutionState {
    ExecutionState::Stopped {
        reason,
        notified_at: None,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use super::*;
    use crate::definition::{
        AgentInput, Prompt, StatusDefinition, WorkflowDefinition, WorkflowSnapshot,
    };
    use crate::task::attempt::AttemptNumber;
    use crate::task::branch::BranchName;
    use crate::task::failure::{FailureKind, ToolFailureKind};
    use crate::task::path::{RepoPath, RunDirPath, StateRoot, WorktreePath};
    use crate::task::process::{KillIdent, Pid, ProcessIdent, ProcessStartTime, StartTimeRecord};
    use crate::task::state::{ExecutionStateKind, StopReason};

    fn absolute(segments: &[&str]) -> PathBuf {
        let mut path = if std::path::MAIN_SEPARATOR == '\\' {
            PathBuf::from("C:\\")
        } else {
            PathBuf::from("/")
        };
        for segment in segments {
            path.push(segment);
        }
        path
    }

    fn status(name: &str) -> StatusName {
        StatusName::parse(name.to_owned()).expect("受理される")
    }

    fn branch(name: &str) -> BranchName {
        BranchName::parse(name.to_owned()).expect("受理される")
    }

    fn task_id() -> TaskId {
        TaskId::parse("20260811t091530-k3f9qa1b".to_owned()).expect("受理される")
    }

    fn workflow_name() -> WorkflowName {
        WorkflowName::parse("implement".to_owned()).expect("受理される")
    }

    fn target() -> Target {
        Target::new(
            RepoPath::parse(absolute(&["repos", "pulsen"])).expect("受理される"),
            branch("main"),
        )
    }

    fn agent_run(next: &str, retries: Option<u32>) -> StatusDefinition {
        StatusDefinition::AgentRun {
            input: AgentInput::Prompt(Prompt::parse("実装して".to_owned()).expect("受理される")),
            agent: None,
            model: None,
            timeout: None,
            retries,
            judge: None,
            next: status(next),
        }
    }

    /// 先頭のステータスを初期ステータスとするスナップショット。
    fn snapshot_of(statuses: Vec<(&str, StatusDefinition)>) -> WorkflowSnapshot {
        let initial = status(statuses.first().expect("1件以上").0);
        let definition = WorkflowDefinition::new(
            None,
            None,
            initial,
            statuses
                .into_iter()
                .map(|(name, definition)| (status(name), definition))
                .collect::<BTreeMap<_, _>>(),
        )
        .expect("不変条件を満たす");
        WorkflowSnapshot::rehydrate(definition)
    }

    fn snapshot() -> WorkflowSnapshot {
        snapshot_of(vec![
            ("queued", agent_run("done", None)),
            ("done", StatusDefinition::Cleanup),
        ])
    }

    fn now() -> Timestamp {
        Timestamp::parse_rfc3339("2026-08-11T09:15:30Z").expect("受理される")
    }

    fn later() -> Timestamp {
        Timestamp::parse_rfc3339("2026-08-11T10:00:00Z").expect("受理される")
    }

    fn state_root() -> StateRoot {
        StateRoot::parse(absolute(&["home", "u", ".pulsen", "state"])).expect("受理される")
    }

    fn workspace() -> Workspace {
        Workspace::new(
            WorktreePath::parse(absolute(&["worktrees", "20260811t091530-k3f9qa1b"]))
                .expect("受理される"),
            branch("pulsen/20260811t091530-k3f9qa1b"),
        )
    }

    fn attempt(number: u32) -> AttemptRef {
        let number = AttemptNumber::parse(number).expect("受理される");
        AttemptRef::rehydrate(
            number,
            RunDirPath::derive(&state_root(), &task_id(), number),
            None,
        )
    }

    fn process() -> ProcessIdent {
        ProcessIdent::new(
            Pid::new(4242),
            KillIdent::parse("-4242".to_owned()).expect("受理される"),
            StartTimeRecord::new(
                ProcessStartTime::parse("871234".to_owned()).expect("受理される"),
                now(),
            ),
        )
    }

    fn fields() -> TaskFields {
        TaskFields {
            id: task_id(),
            workflow_name: workflow_name(),
            target: target(),
            snapshot: snapshot(),
            task_status: status("queued"),
            execution: ExecutionState::Pending,
            workspace: None,
            current_attempt: None,
            counters: RetryCounters::initial(),
            last_failure: None,
            updated_at: now(),
        }
    }

    fn task_of(fields: TaskFields) -> Task {
        Task::rehydrate(fields).expect("不変条件1を満たす")
    }

    /// ワークスペースと attempt が揃った、実行状態だけが異なるタスク。
    fn ready_task(execution: ExecutionState) -> Task {
        task_of(TaskFields {
            execution,
            workspace: Some(workspace()),
            current_attempt: Some(attempt(1)),
            ..fields()
        })
    }

    /// 実行状態の6値。前提状態の検査を状態ごとに網羅するために使う。
    fn every_execution_state() -> [ExecutionState; ExecutionStateKind::COUNT] {
        [
            ExecutionState::Pending,
            ExecutionState::Launching { recorded_at: now() },
            ExecutionState::Running,
            ExecutionState::Completed,
            ExecutionState::Failed,
            ExecutionState::Stopped {
                reason: StopReason::Aborted,
                notified_at: None,
            },
        ]
    }

    #[test]
    fn 登録直後のタスクは初期ステータスの起動待ちになる() {
        let task = Task::register(task_id(), workflow_name(), target(), snapshot(), now());

        assert_eq!(task.task_status(), snapshot().initial());
        assert_eq!(task.execution(), &ExecutionState::Pending);
        assert_eq!(task.workspace(), None);
        assert_eq!(task.current_attempt(), None);
        assert_eq!(task.last_failure(), None);
        assert_eq!(task.counters(), RetryCounters::initial());
        assert_eq!(task.updated_at(), now());
        assert_eq!(task.id(), &task_id());
        assert_eq!(task.workflow_name(), &workflow_name());
        assert_eq!(task.target(), &target());
        assert_eq!(task.snapshot(), &snapshot());
    }

    #[test]
    fn 再構築は与えたフィールドをそのまま復元する() {
        let task = Task::rehydrate(fields()).expect("不変条件1を満たす");

        assert_eq!(task.id(), &task_id());
        assert_eq!(task.task_status(), &status("queued"));
        assert_eq!(task.execution(), &ExecutionState::Pending);
        assert_eq!(task.updated_at(), now());
    }

    #[test]
    fn スナップショットにないタスクステータスは再構築されない() {
        let fields = TaskFields {
            task_status: status("unknown"),
            ..fields()
        };

        assert_eq!(
            Task::rehydrate(fields),
            Err(RehydrateError::StatusNotInSnapshot {
                status: status("unknown"),
                defined: vec![status("done"), status("queued")],
            })
        );
    }

    #[test]
    fn 実行状態の6値すべてを再構築できる() {
        let states = [
            ExecutionState::Pending,
            ExecutionState::Launching { recorded_at: now() },
            ExecutionState::Running,
            ExecutionState::Completed,
            ExecutionState::Failed,
            ExecutionState::Stopped {
                reason: StopReason::Aborted,
                notified_at: Some(now()),
            },
        ];

        for state in states {
            let expected = state.kind();
            let task = Task::rehydrate(TaskFields {
                execution: state,
                ..fields()
            })
            .expect("不変条件1を満たす");
            assert_eq!(task.execution().kind(), expected);
        }
    }

    #[test]
    fn 任意フィールドが埋まったタスクも再構築できる() {
        let number = AttemptNumber::parse(2).expect("受理される");
        let state_root = StateRoot::parse(absolute(&["state"])).expect("受理される");
        let attempt = AttemptRef::rehydrate(
            number,
            RunDirPath::derive(&state_root, &task_id(), number),
            Some(ProcessIdent::new(
                Pid::new(4242),
                KillIdent::parse("-4242".to_owned()).expect("受理される"),
                StartTimeRecord::new(
                    ProcessStartTime::parse("871234".to_owned()).expect("受理される"),
                    now(),
                ),
            )),
        );
        let workspace = Workspace::new(
            WorktreePath::parse(absolute(&["worktrees", "t1"])).expect("受理される"),
            branch("pulsen/t1"),
        );
        let failure = FailureNote::parse(FailureKind::SpawnFail, "起動できない".to_owned(), now())
            .expect("受理される");

        let task = Task::rehydrate(TaskFields {
            task_status: status("done"),
            execution: ExecutionState::Running,
            workspace: Some(workspace.clone()),
            current_attempt: Some(attempt.clone()),
            counters: RetryCounters::rehydrate(1, 2, 3),
            last_failure: Some(failure.clone()),
            ..fields()
        })
        .expect("不変条件1を満たす");

        assert_eq!(task.task_status(), &status("done"));
        assert_eq!(task.execution().kind(), ExecutionStateKind::Running);
        assert_eq!(task.workspace(), Some(&workspace));
        assert_eq!(task.current_attempt(), Some(&attempt));
        assert_eq!(task.counters(), RetryCounters::rehydrate(1, 2, 3));
        assert_eq!(task.last_failure(), Some(&failure));
    }

    #[test]
    fn 実行状態の判別子を問い合わせられる() {
        for execution in every_execution_state() {
            let expected = execution.kind();
            assert_eq!(ready_task(execution).execution_kind(), expected);
        }
    }

    #[test]
    fn 現ステータスの定義を引ける() {
        let task = task_of(fields());

        assert_eq!(task.current_status_def(), &agent_run("done", None));
    }

    #[test]
    fn 動作種別は現ステータスの定義から決まる() {
        let snapshot = snapshot_of(vec![
            ("queued", agent_run("waiting", None)),
            ("waiting", StatusDefinition::Wait),
            ("done", StatusDefinition::Cleanup),
        ]);
        let task_at = |name: &str| {
            task_of(TaskFields {
                snapshot: snapshot.clone(),
                task_status: status(name),
                ..fields()
            })
        };

        let running = task_at("queued");
        assert!(running.is_agent_run());
        assert!(!running.is_wait());
        assert!(!running.is_cleanup());

        let waiting = task_at("waiting");
        assert!(waiting.is_wait());
        assert!(!waiting.is_agent_run());
        assert!(!waiting.is_cleanup());

        let cleanup = task_at("done");
        assert!(cleanup.is_cleanup());
        assert!(!cleanup.is_agent_run());
        assert!(!cleanup.is_wait());
    }

    #[test]
    fn 適用されるリトライ上限は動作種別ごとに決まる() {
        let snapshot = snapshot_of(vec![
            ("queued", agent_run("tuned", None)),
            ("tuned", agent_run("waiting", Some(5))),
            ("waiting", StatusDefinition::Wait),
            ("done", StatusDefinition::Cleanup),
        ]);
        let limit_at = |name: &str| {
            task_of(TaskFields {
                snapshot: snapshot.clone(),
                task_status: status(name),
                ..fields()
            })
            .applicable_retry_limit()
        };

        assert_eq!(limit_at("tuned"), Some(5));
        assert_eq!(
            limit_at("queued"),
            Some(WorkflowDefinition::DEFAULT_RETRY_LIMIT)
        );
        assert_eq!(
            limit_at("done"),
            Some(WorkflowDefinition::DEFAULT_RETRY_LIMIT)
        );
        assert_eq!(limit_at("waiting"), None);
    }

    #[test]
    fn 次のattempt番号は現在attemptがなければ最初の番号になる() {
        assert_eq!(
            task_of(fields()).next_attempt_number(),
            AttemptNumber::FIRST
        );
    }

    #[test]
    fn 次のattempt番号は現在attemptの1つ後になる() {
        let task = task_of(TaskFields {
            current_attempt: Some(attempt(3)),
            ..fields()
        });

        assert_eq!(task.next_attempt_number().get(), 4);
    }

    #[test]
    fn ワークスペースは未確定のときだけ確定できる() {
        let task = task_of(fields())
            .confirm_workspace(workspace(), later())
            .expect("未確定である");

        assert_eq!(task.workspace(), Some(&workspace()));
        assert_eq!(task.updated_at(), later());
    }

    #[test]
    fn ワークスペースの確定はリトライのカウンタをリセットしない() {
        let counters = RetryCounters::rehydrate(2, 1, 3);
        let task = task_of(TaskFields {
            execution: ExecutionState::Failed,
            counters,
            ..fields()
        });

        let confirmed = task
            .confirm_workspace(workspace(), later())
            .expect("未確定である");

        assert_eq!(confirmed.counters(), counters);
    }

    #[test]
    fn 確定済みのワークスペースは再確定されない() {
        let task = task_of(TaskFields {
            workspace: Some(workspace()),
            ..fields()
        });

        assert_eq!(
            task.confirm_workspace(workspace(), later()),
            Err(TransitionError::WorkspaceAlreadySet)
        );
    }

    #[test]
    fn 起動記録は次の番号を採番し番号どおりのrunディレクトリを導出する() {
        for (execution, current, expected) in [
            (ExecutionState::Pending, None, attempt(1)),
            (ExecutionState::Failed, Some(attempt(1)), attempt(2)),
        ] {
            let number = expected.number().get();
            let task = task_of(TaskFields {
                execution,
                workspace: Some(workspace()),
                current_attempt: current,
                ..fields()
            });

            let (recorded, run_dir) = task
                .record_launching(&state_root(), later())
                .expect("前提を満たす");

            assert_eq!(run_dir, *expected.run_dir(), "attempt-{number}");
            assert_eq!(
                recorded.execution(),
                &ExecutionState::Launching {
                    recorded_at: later()
                },
                "attempt-{number}"
            );
            assert_eq!(
                recorded.current_attempt(),
                Some(&expected),
                "attempt-{number}"
            );
            assert_eq!(recorded.updated_at(), later(), "attempt-{number}");
        }
    }

    #[test]
    fn 起動記録はリトライのカウンタをリセットしない() {
        let counters = RetryCounters::rehydrate(1, 2, 3);
        for (execution, current) in [
            (ExecutionState::Pending, None),
            (ExecutionState::Failed, Some(attempt(1))),
        ] {
            let kind = execution.kind();
            let task = task_of(TaskFields {
                execution,
                workspace: Some(workspace()),
                current_attempt: current,
                counters,
                ..fields()
            });

            let (recorded, _) = task
                .record_launching(&state_root(), later())
                .expect("前提を満たす");

            assert_eq!(recorded.counters(), counters, "{kind:?}");
        }
    }

    #[test]
    fn 起動記録は起動待ちと失敗確定からのみ行える() {
        for execution in every_execution_state() {
            let kind = execution.kind();
            let result = ready_task(execution).record_launching(&state_root(), later());

            match kind {
                ExecutionStateKind::Pending | ExecutionStateKind::Failed => {
                    assert!(result.is_ok(), "{kind:?}");
                }
                ExecutionStateKind::Launching
                | ExecutionStateKind::Running
                | ExecutionStateKind::Completed
                | ExecutionStateKind::Stopped => assert_eq!(
                    result,
                    Err(TransitionError::InvalidState {
                        expected: RESTARTABLE,
                        actual: kind,
                    }),
                    "{kind:?}"
                ),
            }
        }
    }

    #[test]
    fn ワークスペース未確定のタスクは起動記録できない() {
        let task = task_of(fields());

        assert_eq!(
            task.record_launching(&state_root(), later()),
            Err(TransitionError::WorkspaceNotSet)
        );
    }

    #[test]
    fn エージェント実行以外のステータスは起動記録できない() {
        let snapshot = snapshot_of(vec![
            ("waiting", StatusDefinition::Wait),
            ("done", StatusDefinition::Cleanup),
        ]);
        for name in ["waiting", "done"] {
            let task = task_of(TaskFields {
                snapshot: snapshot.clone(),
                task_status: status(name),
                workspace: Some(workspace()),
                ..fields()
            });

            assert_eq!(
                task.record_launching(&state_root(), later()),
                Err(TransitionError::NotAgentRunStatus {
                    status: status(name)
                }),
                "{name}"
            );
        }
    }

    #[test]
    fn 起動確認は同定情報を取り込みspawn失敗のカウンタだけをリセットする() {
        let task = task_of(TaskFields {
            execution: ExecutionState::Launching { recorded_at: now() },
            workspace: Some(workspace()),
            current_attempt: Some(attempt(2)),
            counters: RetryCounters::rehydrate(1, 2, 3),
            ..fields()
        });

        let running = task
            .confirm_running(process(), later())
            .expect("起動記録済み");

        assert_eq!(running.execution(), &ExecutionState::Running);
        assert_eq!(
            running.current_attempt().and_then(AttemptRef::process),
            Some(&process())
        );
        assert_eq!(
            running.current_attempt().map(AttemptRef::number),
            Some(AttemptNumber::parse(2).expect("受理される"))
        );
        assert_eq!(running.counters(), RetryCounters::rehydrate(1, 2, 0));
        assert_eq!(running.updated_at(), later());
    }

    #[test]
    fn 起動確認は起動記録済みからのみ行える() {
        for execution in every_execution_state() {
            let kind = execution.kind();
            let result = ready_task(execution).confirm_running(process(), later());

            match kind {
                ExecutionStateKind::Launching => assert!(result.is_ok(), "{kind:?}"),
                ExecutionStateKind::Pending
                | ExecutionStateKind::Running
                | ExecutionStateKind::Completed
                | ExecutionStateKind::Failed
                | ExecutionStateKind::Stopped => assert_eq!(
                    result,
                    Err(TransitionError::InvalidState {
                        expected: LAUNCHING,
                        actual: kind,
                    }),
                    "{kind:?}"
                ),
            }
        }
    }

    #[test]
    fn 現在attemptのない起動記録済みタスクの起動確認は不変条件の破れになる() {
        let task = task_of(TaskFields {
            execution: ExecutionState::Launching { recorded_at: now() },
            workspace: Some(workspace()),
            ..fields()
        });

        assert_eq!(
            task.confirm_running(process(), later()),
            Err(TransitionError::MissingCurrentAttempt)
        );
    }

    #[test]
    fn 猶予超過のspawn失敗は起動待ちへ戻し失敗要因を残す() {
        let task = task_of(TaskFields {
            execution: ExecutionState::Launching { recorded_at: now() },
            workspace: Some(workspace()),
            current_attempt: Some(attempt(1)),
            counters: RetryCounters::rehydrate(1, 2, 3),
            ..fields()
        });

        let failed = task
            .record_spawn_failure("起動を確認できない".to_owned(), 9, later())
            .expect("起動記録済み");

        assert_eq!(failed.execution(), &ExecutionState::Pending);
        assert_eq!(
            failed.counters(),
            RetryCounters::rehydrate(1, 2, 4),
            "進むのは spawn_fail_count だけで、実行と判定のカウンタは保持される"
        );
        assert_eq!(
            failed.last_failure().map(FailureNote::kind),
            Some(FailureKind::SpawnFail)
        );
        assert_eq!(failed.last_failure().map(FailureNote::at), Some(later()));
        assert_eq!(failed.current_attempt(), Some(&attempt(1)));
        assert_eq!(failed.updated_at(), later());
    }

    #[test]
    fn 猶予超過のspawn失敗は起動記録済みからのみ記録できる() {
        for execution in every_execution_state() {
            let kind = execution.kind();
            let result = ready_task(execution).record_spawn_failure(
                "起動を確認できない".to_owned(),
                3,
                later(),
            );

            match kind {
                ExecutionStateKind::Launching => assert!(result.is_ok(), "{kind:?}"),
                ExecutionStateKind::Pending
                | ExecutionStateKind::Running
                | ExecutionStateKind::Completed
                | ExecutionStateKind::Failed
                | ExecutionStateKind::Stopped => assert_eq!(
                    result,
                    Err(TransitionError::InvalidState {
                        expected: LAUNCHING,
                        actual: kind,
                    }),
                    "{kind:?}"
                ),
            }
        }
    }

    #[test]
    fn 猶予超過のspawn失敗は上限と等しい回数では凍結しない() {
        let task = task_of(TaskFields {
            execution: ExecutionState::Launching { recorded_at: now() },
            workspace: Some(workspace()),
            current_attempt: Some(attempt(1)),
            counters: RetryCounters::rehydrate(0, 0, 2),
            ..fields()
        });

        let failed = task
            .record_spawn_failure("起動を確認できない".to_owned(), 3, later())
            .expect("起動記録済み");

        assert_eq!(failed.execution(), &ExecutionState::Pending);
        assert_eq!(failed.counters().spawn_fail_count(), 3);
    }

    #[test]
    fn 猶予超過のspawn失敗は上限を超えると凍結する() {
        let task = task_of(TaskFields {
            execution: ExecutionState::Launching { recorded_at: now() },
            workspace: Some(workspace()),
            current_attempt: Some(attempt(1)),
            counters: RetryCounters::rehydrate(0, 0, 3),
            ..fields()
        });

        let frozen = task
            .record_spawn_failure("起動を確認できない".to_owned(), 3, later())
            .expect("起動記録済み");

        assert_eq!(
            frozen.execution(),
            &ExecutionState::Stopped {
                reason: StopReason::SpawnFailLimitExceeded,
                notified_at: None,
            }
        );
        assert_eq!(frozen.counters().spawn_fail_count(), 4);
    }

    #[test]
    fn 同期的なspawn失敗は実行状態もattemptも変えない() {
        for execution in [ExecutionState::Pending, ExecutionState::Failed] {
            let expected = execution.clone();
            let task = task_of(TaskFields {
                execution,
                workspace: Some(workspace()),
                current_attempt: Some(attempt(1)),
                ..fields()
            });

            let recorded = task
                .record_spawn_failure_in_place("展開できない".to_owned(), 3, later())
                .expect("前提を満たす");

            assert_eq!(recorded.execution(), &expected);
            assert_eq!(recorded.current_attempt(), Some(&attempt(1)));
            assert_eq!(recorded.next_attempt_number().get(), 2);
            assert_eq!(recorded.counters().spawn_fail_count(), 1);
            assert_eq!(
                recorded.last_failure().map(FailureNote::kind),
                Some(FailureKind::SpawnFail)
            );
            assert_eq!(recorded.updated_at(), later());
        }
    }

    #[test]
    fn 同期的なspawn失敗は上限を超えると凍結する() {
        let at_limit = task_of(TaskFields {
            counters: RetryCounters::rehydrate(0, 0, 2),
            ..fields()
        })
        .record_spawn_failure_in_place("展開できない".to_owned(), 3, later())
        .expect("前提を満たす");
        assert_eq!(at_limit.execution(), &ExecutionState::Pending);

        let over_limit = task_of(TaskFields {
            counters: RetryCounters::rehydrate(0, 0, 3),
            ..fields()
        })
        .record_spawn_failure_in_place("展開できない".to_owned(), 3, later())
        .expect("前提を満たす");
        assert_eq!(
            over_limit.execution(),
            &ExecutionState::Stopped {
                reason: StopReason::SpawnFailLimitExceeded,
                notified_at: None,
            }
        );
    }

    #[test]
    fn 同期的なspawn失敗は起動待ちと失敗確定からのみ行える() {
        for execution in every_execution_state() {
            let kind = execution.kind();
            let result = ready_task(execution).record_spawn_failure_in_place(
                "展開できない".to_owned(),
                3,
                later(),
            );

            match kind {
                ExecutionStateKind::Pending | ExecutionStateKind::Failed => {
                    assert!(result.is_ok(), "{kind:?}");
                }
                ExecutionStateKind::Launching
                | ExecutionStateKind::Running
                | ExecutionStateKind::Completed
                | ExecutionStateKind::Stopped => assert_eq!(
                    result,
                    Err(TransitionError::InvalidState {
                        expected: RESTARTABLE,
                        actual: kind,
                    }),
                    "{kind:?}"
                ),
            }
        }
    }

    #[test]
    fn ツール操作の失敗は失敗確定にして種別と説明を残す() {
        let task = task_of(TaskFields {
            counters: RetryCounters::rehydrate(0, 1, 2),
            ..fields()
        });

        let failed = task
            .record_tool_failure(
                ToolFailureKind::WorktreeCreate,
                "git worktree add に失敗".to_owned(),
                2,
                later(),
            )
            .expect("前提を満たす");

        assert_eq!(failed.execution(), &ExecutionState::Failed);
        assert_eq!(failed.counters(), RetryCounters::rehydrate(1, 1, 2));
        assert_eq!(
            failed.last_failure().map(FailureNote::kind),
            Some(FailureKind::WorktreeCreate)
        );
        assert_eq!(
            failed.last_failure().map(FailureNote::message),
            Some("git worktree add に失敗")
        );
        assert_eq!(failed.updated_at(), later());
    }

    #[test]
    fn ツール操作の失敗は上限を超えると凍結する() {
        let at_limit = task_of(TaskFields {
            counters: RetryCounters::rehydrate(1, 0, 0),
            ..fields()
        })
        .record_tool_failure(
            ToolFailureKind::WorktreeCreate,
            "失敗".to_owned(),
            2,
            later(),
        )
        .expect("前提を満たす");
        assert_eq!(at_limit.execution(), &ExecutionState::Failed);
        assert_eq!(at_limit.counters().attempt_count(), 2);

        let over_limit = task_of(TaskFields {
            counters: RetryCounters::rehydrate(2, 0, 0),
            ..fields()
        })
        .record_tool_failure(
            ToolFailureKind::WorktreeCreate,
            "失敗".to_owned(),
            2,
            later(),
        )
        .expect("前提を満たす");
        assert_eq!(
            over_limit.execution(),
            &ExecutionState::Stopped {
                reason: StopReason::RetryLimitExceeded,
                notified_at: None,
            }
        );
        assert_eq!(over_limit.counters().attempt_count(), 3);
    }

    #[test]
    fn リトライ上限が0のステータスは最初のツール操作失敗で凍結する() {
        let frozen = task_of(fields())
            .record_tool_failure(
                ToolFailureKind::WorktreeCreate,
                "失敗".to_owned(),
                0,
                later(),
            )
            .expect("前提を満たす");

        assert_eq!(
            frozen.execution(),
            &ExecutionState::Stopped {
                reason: StopReason::RetryLimitExceeded,
                notified_at: None,
            }
        );
        assert_eq!(frozen.counters().attempt_count(), 1);
    }

    #[test]
    fn ツール操作の失敗は起動待ちと失敗確定からのみ記録できる() {
        for execution in every_execution_state() {
            let kind = execution.kind();
            let result = ready_task(execution).record_tool_failure(
                ToolFailureKind::WorktreeCreate,
                "失敗".to_owned(),
                2,
                later(),
            );

            match kind {
                ExecutionStateKind::Pending | ExecutionStateKind::Failed => {
                    assert!(result.is_ok(), "{kind:?}");
                }
                ExecutionStateKind::Launching
                | ExecutionStateKind::Running
                | ExecutionStateKind::Completed
                | ExecutionStateKind::Stopped => assert_eq!(
                    result,
                    Err(TransitionError::InvalidState {
                        expected: RESTARTABLE,
                        actual: kind,
                    }),
                    "{kind:?}"
                ),
            }
        }
    }

    #[test]
    fn 説明のない失敗も既定の文言で記録される() {
        let recorded = task_of(fields())
            .record_spawn_failure_in_place(String::new(), 3, later())
            .expect("前提を満たす");

        let note = recorded.last_failure().expect("記録される");
        assert!(!note.message().is_empty());
        assert_eq!(note.kind(), FailureKind::SpawnFail);
    }
}
