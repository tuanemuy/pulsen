//! タスク集約ルート。

use crate::definition::{StatusName, WorkflowName, WorkflowSnapshot};

use super::attempt::AttemptRef;
use super::branch::{Target, Workspace};
use super::counters::RetryCounters;
use super::failure::FailureNote;
use super::id::TaskId;
use super::state::ExecutionState;
use super::time::Timestamp;

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
    use crate::task::failure::FailureKind;
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

    fn snapshot() -> WorkflowSnapshot {
        let definition = WorkflowDefinition::new(
            None,
            None,
            status("queued"),
            BTreeMap::from([
                (
                    status("queued"),
                    StatusDefinition::AgentRun {
                        input: AgentInput::Prompt(
                            Prompt::parse("実装して".to_owned()).expect("受理される"),
                        ),
                        agent: None,
                        model: None,
                        timeout: None,
                        retries: None,
                        judge: None,
                        next: status("done"),
                    },
                ),
                (status("done"), StatusDefinition::Cleanup),
            ]),
        )
        .expect("不変条件を満たす");
        WorkflowSnapshot::rehydrate(definition)
    }

    fn now() -> Timestamp {
        Timestamp::parse_rfc3339("2026-08-11T09:15:30Z").expect("受理される")
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
}
