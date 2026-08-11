//! スナップショットが読めないタスク。
//!
//! タスクファイル自体は読めるが、埋め込まれたスナップショットが欠落・不正なタスク。
//! スナップショットに依存する操作(`set_status` / `advance` / 起動記録 / ステータス定義の
//! 参照)を**持たない型**として表現し、縮退時に許される操作を型で制限する。

use crate::definition::{StatusName, WorkflowName};

use super::attempt::AttemptRef;
use super::branch::{Target, Workspace};
use super::counters::RetryCounters;
use super::failure::FailureNote;
use super::id::TaskId;
use super::state::{ExecutionState, ExecutionStateKind};
use super::time::Timestamp;

/// 永続化されたスナップショット破損タスクの全フィールド。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DegradedTaskFields {
    /// タスクID。
    pub id: TaskId,
    /// 表示名。
    pub workflow_name: WorkflowName,
    /// 対象のリポジトリとベースブランチ。
    pub target: Target,
    /// タスクステータス。スナップショットと照合できないため検証しない。
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
    /// スナップショットが読めない理由。
    pub snapshot_error: String,
}

/// スナップショット破損タスク。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DegradedTask {
    id: TaskId,
    workflow_name: WorkflowName,
    target: Target,
    task_status: StatusName,
    execution: ExecutionState,
    workspace: Option<Workspace>,
    current_attempt: Option<AttemptRef>,
    counters: RetryCounters,
    last_failure: Option<FailureNote>,
    updated_at: Timestamp,
    snapshot_error: String,
}

impl DegradedTask {
    /// 永続化からの再構築。
    ///
    /// 不変条件1(タスクステータスのスナップショット所属)と4(エージェント実行での
    /// ワークスペース確定)は動作種別の判定にスナップショットが要るため課さない。
    pub fn rehydrate(fields: DegradedTaskFields) -> Self {
        Self {
            id: fields.id,
            workflow_name: fields.workflow_name,
            target: fields.target,
            task_status: fields.task_status,
            execution: fields.execution,
            workspace: fields.workspace,
            current_attempt: fields.current_attempt,
            counters: fields.counters,
            last_failure: fields.last_failure,
            updated_at: fields.updated_at,
            snapshot_error: fields.snapshot_error,
        }
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

    /// タスクステータス。
    pub fn task_status(&self) -> &StatusName {
        &self.task_status
    }

    /// 実行状態。
    pub fn execution(&self) -> &ExecutionState {
        &self.execution
    }

    /// 実行状態の判別子。
    pub fn execution_kind(&self) -> ExecutionStateKind {
        self.execution.kind()
    }

    /// 確定済みのワークスペース。
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

    /// スナップショットが読めない理由。
    pub fn snapshot_error(&self) -> &str {
        &self.snapshot_error
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::task::branch::BranchName;
    use crate::task::path::RepoPath;
    use crate::task::state::StopReason;

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

    fn now() -> Timestamp {
        Timestamp::parse_rfc3339("2026-08-11T09:15:30Z").expect("受理される")
    }

    fn fields() -> DegradedTaskFields {
        DegradedTaskFields {
            id: TaskId::parse("20260811t091530-k3f9qa1b".to_owned()).expect("受理される"),
            workflow_name: WorkflowName::parse("implement".to_owned()).expect("受理される"),
            target: Target::new(
                RepoPath::parse(absolute(&["repos", "pulsen"])).expect("受理される"),
                BranchName::parse("main".to_owned()).expect("受理される"),
            ),
            task_status: StatusName::parse("queued".to_owned()).expect("受理される"),
            execution: ExecutionState::Pending,
            workspace: None,
            current_attempt: None,
            counters: RetryCounters::initial(),
            last_failure: None,
            updated_at: now(),
            snapshot_error: "snapshot が読めない".to_owned(),
        }
    }

    #[test]
    fn 再構築したスナップショット破損タスクは読めない理由を保持する() {
        let task = DegradedTask::rehydrate(fields());

        assert_eq!(task.snapshot_error(), "snapshot が読めない");
        assert_eq!(task.execution_kind(), ExecutionStateKind::Pending);
        assert_eq!(task.counters(), RetryCounters::initial());
        assert_eq!(task.updated_at(), now());
    }

    #[test]
    fn スナップショットに照合できないタスクステータスも再構築できる() {
        let task = DegradedTask::rehydrate(DegradedTaskFields {
            task_status: StatusName::parse("どこにも定義がない".to_owned()).expect("受理される"),
            ..fields()
        });

        assert_eq!(task.task_status().as_str(), "どこにも定義がない");
    }

    #[test]
    fn 凍結状態のスナップショット破損タスクも再構築できる() {
        let task = DegradedTask::rehydrate(DegradedTaskFields {
            execution: ExecutionState::Stopped {
                reason: StopReason::Aborted,
                notified_at: None,
            },
            ..fields()
        });

        assert_eq!(task.execution_kind(), ExecutionStateKind::Stopped);
        assert_eq!(
            task.execution(),
            &ExecutionState::Stopped {
                reason: StopReason::Aborted,
                notified_at: None,
            }
        );
    }
}
