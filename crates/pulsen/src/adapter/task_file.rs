//! タスクファイルの直列化形式(人間可読な JSON)と符号化・復号。
//!
//! ドメイン型は serde を持たない(ADR-020)ため、永続化の表現はここの DTO が担い、
//! 復元は必ずドメインの `parse` / `rehydrate` を通す。スナップショットはタスクファイルの
//! 1フィールドとして埋め込む(ADR-015)。
//!
//! 復号の分類は ADR-025 の表に従う。
//!
//! | 状態 | 分類 |
//! |---|---|
//! | ファイル全体が JSON として不正 | `Corrupt`(このモジュールでは `Err(message)`) |
//! | 有効な JSON だが、タスク側フィールドの型・値制約・未知キーが破れている | `Corrupt` |
//! | タスク側フィールドは読めるが、`snapshot` が解釈できない | `SnapshotUnreadable` |
//! | 状態間整合の不変条件2〜4の破れ | 検証しない(`Intact`) |
//!
//! 不変条件2〜4を復号で検証すると、遷移関数の前提検査(`InvariantViolated`)が受け持つ
//! 「手動修復で壊れたタスクを人間に見せる」経路が塞がる。

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;

use pulsen_domain::definition::{
    AgentInput, AgentName, ModelName, PlainCommand, Prompt, SkillName, StatusDefinition,
    StatusName, TimeoutSpec, WorkflowDefinition, WorkflowName, WorkflowSnapshot,
};
use pulsen_domain::task::{
    AttemptNumber, AttemptRef, BranchName, DegradedTask, DegradedTaskFields, ExecutionState,
    FailureKind, FailureNote, KillIdent, Pid, ProcessIdent, ProcessStartTime, RehydrateError,
    RepoPath, RetryCounters, RunDirPath, StartTimeRecord, StopReason, Target, Task, TaskFields,
    TaskId, TaskRecord, Timestamp, Workspace, WorktreePath,
};

/// タスクファイル1件の内容。
///
/// スナップショットの表現を型引数にすることで、`Task` の保存では構造化した値を整形して
/// 書き、`DegradedTask` の保存では読み取った生バイト列をそのまま書き戻せる。
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TaskFileDto<Snapshot> {
    task_id: String,
    workflow_name: String,
    target: TargetDto,
    task_status: String,
    execution: ExecutionDto,
    workspace: Option<WorkspaceDto>,
    current_attempt: Option<AttemptDto>,
    counters: CountersDto,
    last_failure: Option<FailureDto>,
    updated_at: String,
    /// 不在は不在のまま書き戻す。既定の直列化ではキーの削除が `null` に化け、
    /// 「元の内容のまま温存する」契約から外れる。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    snapshot: Option<Snapshot>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TargetDto {
    repo: PathBuf,
    base_branch: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
enum ExecutionDto {
    Pending,
    Launching {
        recorded_at: String,
    },
    Running,
    Completed,
    Failed,
    Stopped {
        reason: StopReasonDto,
        notified_at: Option<String>,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum StopReasonDto {
    RetryLimitExceeded,
    JudgeLimitExceeded,
    SpawnFailLimitExceeded,
    Aborted,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkspaceDto {
    path: PathBuf,
    branch: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AttemptDto {
    number: u32,
    run_dir: PathBuf,
    process: Option<ProcessDto>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProcessDto {
    pid: u32,
    kill_ident: String,
    starttime: StartTimeDto,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StartTimeDto {
    ident: String,
    wall: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CountersDto {
    attempt_count: u32,
    judge_attempt_count: u32,
    spawn_fail_count: u32,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FailureDto {
    kind: FailureKindDto,
    message: String,
    at: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum FailureKindDto {
    WorktreeCreate,
    WorktreeRemove,
    ArchiveMove,
    SpawnFail,
    JudgeFail,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SnapshotDto {
    default_agent: Option<String>,
    default_model: Option<String>,
    initial: String,
    statuses: BTreeMap<String, StatusDto>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
enum StatusDto {
    AgentRun {
        input: InputDto,
        agent: Option<String>,
        model: Option<String>,
        timeout: Option<String>,
        retries: Option<u32>,
        judge: Option<Vec<String>>,
        next: String,
    },
    Wait,
    Cleanup,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
enum InputDto {
    Prompt(String),
    Skill(String),
}

/// タスクを人間可読な JSON に符号化する。
pub fn encode_task(task: &Task) -> Result<Vec<u8>, String> {
    let dto = TaskFileDto {
        task_id: task.id().as_str().to_owned(),
        workflow_name: task.workflow_name().as_str().to_owned(),
        target: TargetDto {
            repo: task.target().repo().as_path().to_path_buf(),
            base_branch: task.target().base_branch().as_str().to_owned(),
        },
        task_status: task.task_status().as_str().to_owned(),
        execution: execution_dto(task.execution()),
        workspace: task.workspace().map(workspace_dto),
        current_attempt: task.current_attempt().map(attempt_dto),
        counters: counters_dto(task.counters()),
        last_failure: task.last_failure().map(failure_dto),
        updated_at: task.updated_at().to_rfc3339(),
        snapshot: Some(snapshot_dto(task.snapshot())),
    };
    to_bytes(&dto)
}

/// スナップショット破損タスクを符号化する。
///
/// スナップショットは引き継いだ生バイト列をそのまま書き戻す — `DegradedTask` は
/// スナップショットを持たず、読めなかった内容が唯一の修復材料になる。
pub fn encode_degraded(
    task: &DegradedTask,
    snapshot: Option<Box<RawValue>>,
) -> Result<Vec<u8>, String> {
    let dto = TaskFileDto {
        task_id: task.id().as_str().to_owned(),
        workflow_name: task.workflow_name().as_str().to_owned(),
        target: TargetDto {
            repo: task.target().repo().as_path().to_path_buf(),
            base_branch: task.target().base_branch().as_str().to_owned(),
        },
        task_status: task.task_status().as_str().to_owned(),
        execution: execution_dto(task.execution()),
        workspace: task.workspace().map(workspace_dto),
        current_attempt: task.current_attempt().map(attempt_dto),
        counters: counters_dto(task.counters()),
        last_failure: task.last_failure().map(failure_dto),
        updated_at: task.updated_at().to_rfc3339(),
        snapshot,
    };
    to_bytes(&dto)
}

/// 既存の内容からスナップショットの生バイト列を取り出す。
///
/// タスク側フィールドの妥当性は問わない — 引き継ぎたいのは読めない部分だけであり、
/// 復号できない内容を理由に修復材料を落とさない。
pub fn carried_snapshot(bytes: &[u8]) -> Result<Option<Box<RawValue>>, String> {
    #[derive(Deserialize)]
    struct SnapshotCarrier {
        #[serde(default)]
        snapshot: Option<Box<RawValue>>,
    }

    serde_json::from_slice::<SnapshotCarrier>(bytes)
        .map(|carrier| carrier.snapshot)
        .map_err(|error| error.to_string())
}

/// タスクファイルの内容を復号する。
///
/// `Err(message)` はファイル全体の破損(`Corrupt`)を表す。
pub fn decode(bytes: &[u8]) -> Result<TaskRecord, String> {
    let dto: TaskFileDto<Box<RawValue>> =
        serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
    let parts = TaskParts::from_dto(&dto)?;

    let snapshot = match &dto.snapshot {
        None => {
            return Ok(TaskRecord::SnapshotUnreadable(DegradedTask::rehydrate(
                parts.into_degraded_fields("snapshot フィールドがありません".to_owned()),
            )));
        }
        Some(raw) => match decode_snapshot(raw) {
            Ok(snapshot) => snapshot,
            Err(message) => {
                return Ok(TaskRecord::SnapshotUnreadable(DegradedTask::rehydrate(
                    parts.into_degraded_fields(message),
                )));
            }
        },
    };

    match Task::rehydrate(parts.clone().into_task_fields(snapshot)) {
        Ok(task) => Ok(TaskRecord::Intact(task)),
        Err(RehydrateError::StatusNotInSnapshot { status, defined }) => {
            let defined: Vec<&str> = defined.iter().map(StatusName::as_str).collect();
            let message = format!(
                "タスクステータス `{}` がスナップショットにありません(定義済み: {})",
                status.as_str(),
                defined.join(", ")
            );
            Ok(TaskRecord::SnapshotUnreadable(DegradedTask::rehydrate(
                parts.into_degraded_fields(message),
            )))
        }
    }
}

/// スナップショット以外の全フィールド。
///
/// タスクとスナップショット破損タスクのどちらにも復元できる形にして、分類が決まる前に
/// タスク側フィールドの検証を1度だけ行えるようにする。
#[derive(Clone)]
struct TaskParts {
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
}

impl TaskParts {
    fn from_dto<Snapshot>(dto: &TaskFileDto<Snapshot>) -> Result<Self, String> {
        Ok(Self {
            id: TaskId::parse(dto.task_id.clone())
                .map_err(|error| field_error("task_id", error.describe()))?,
            workflow_name: WorkflowName::parse(dto.workflow_name.clone())
                .map_err(|error| field_error("workflow_name", error.describe()))?,
            target: Target::new(
                RepoPath::parse(dto.target.repo.clone())
                    .map_err(|error| field_error("target.repo", error.describe()))?,
                BranchName::parse(dto.target.base_branch.clone())
                    .map_err(|error| field_error("target.base_branch", error.describe()))?,
            ),
            task_status: StatusName::parse(dto.task_status.clone())
                .map_err(|error| field_error("task_status", error.describe()))?,
            execution: execution(&dto.execution)?,
            workspace: dto.workspace.as_ref().map(workspace).transpose()?,
            current_attempt: dto.current_attempt.as_ref().map(attempt).transpose()?,
            counters: RetryCounters::rehydrate(
                dto.counters.attempt_count,
                dto.counters.judge_attempt_count,
                dto.counters.spawn_fail_count,
            ),
            last_failure: dto.last_failure.as_ref().map(failure).transpose()?,
            updated_at: timestamp(&dto.updated_at, "updated_at")?,
        })
    }

    fn into_task_fields(self, snapshot: WorkflowSnapshot) -> TaskFields {
        TaskFields {
            id: self.id,
            workflow_name: self.workflow_name,
            target: self.target,
            snapshot,
            task_status: self.task_status,
            execution: self.execution,
            workspace: self.workspace,
            current_attempt: self.current_attempt,
            counters: self.counters,
            last_failure: self.last_failure,
            updated_at: self.updated_at,
        }
    }

    fn into_degraded_fields(self, snapshot_error: String) -> DegradedTaskFields {
        DegradedTaskFields {
            id: self.id,
            workflow_name: self.workflow_name,
            target: self.target,
            task_status: self.task_status,
            execution: self.execution,
            workspace: self.workspace,
            current_attempt: self.current_attempt,
            counters: self.counters,
            last_failure: self.last_failure,
            updated_at: self.updated_at,
            snapshot_error,
        }
    }
}

fn to_bytes<Snapshot: Serialize>(dto: &TaskFileDto<Snapshot>) -> Result<Vec<u8>, String> {
    let mut bytes = serde_json::to_vec_pretty(dto).map_err(|error| error.to_string())?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn field_error(location: &str, message: impl AsRef<str>) -> String {
    format!("{location}: {}", message.as_ref())
}

fn execution_dto(state: &ExecutionState) -> ExecutionDto {
    match state {
        ExecutionState::Pending => ExecutionDto::Pending,
        ExecutionState::Launching { recorded_at } => ExecutionDto::Launching {
            recorded_at: recorded_at.to_rfc3339(),
        },
        ExecutionState::Running => ExecutionDto::Running,
        ExecutionState::Completed => ExecutionDto::Completed,
        ExecutionState::Failed => ExecutionDto::Failed,
        ExecutionState::Stopped {
            reason,
            notified_at,
        } => ExecutionDto::Stopped {
            reason: match reason {
                StopReason::RetryLimitExceeded => StopReasonDto::RetryLimitExceeded,
                StopReason::JudgeLimitExceeded => StopReasonDto::JudgeLimitExceeded,
                StopReason::SpawnFailLimitExceeded => StopReasonDto::SpawnFailLimitExceeded,
                StopReason::Aborted => StopReasonDto::Aborted,
            },
            notified_at: notified_at.map(|at| at.to_rfc3339()),
        },
    }
}

fn workspace_dto(workspace: &Workspace) -> WorkspaceDto {
    WorkspaceDto {
        path: workspace.path().as_path().to_path_buf(),
        branch: workspace.branch().as_str().to_owned(),
    }
}

fn attempt_dto(attempt: &AttemptRef) -> AttemptDto {
    AttemptDto {
        number: attempt.number().get(),
        run_dir: attempt.run_dir().as_path().to_path_buf(),
        process: attempt.process().map(|process| ProcessDto {
            pid: process.pid().get(),
            kill_ident: process.kill_ident().as_str().to_owned(),
            starttime: StartTimeDto {
                ident: process.starttime().ident().as_str().to_owned(),
                wall: process.starttime().wall().to_rfc3339(),
            },
        }),
    }
}

fn counters_dto(counters: RetryCounters) -> CountersDto {
    CountersDto {
        attempt_count: counters.attempt_count(),
        judge_attempt_count: counters.judge_attempt_count(),
        spawn_fail_count: counters.spawn_fail_count(),
    }
}

fn failure_dto(failure: &FailureNote) -> FailureDto {
    FailureDto {
        kind: match failure.kind() {
            FailureKind::WorktreeCreate => FailureKindDto::WorktreeCreate,
            FailureKind::WorktreeRemove => FailureKindDto::WorktreeRemove,
            FailureKind::ArchiveMove => FailureKindDto::ArchiveMove,
            FailureKind::SpawnFail => FailureKindDto::SpawnFail,
            FailureKind::JudgeFail => FailureKindDto::JudgeFail,
        },
        message: failure.message().to_owned(),
        at: failure.at().to_rfc3339(),
    }
}

fn snapshot_dto(snapshot: &WorkflowSnapshot) -> SnapshotDto {
    SnapshotDto {
        default_agent: snapshot
            .default_agent()
            .map(|name| name.as_str().to_owned()),
        default_model: snapshot
            .default_model()
            .map(|name| name.as_str().to_owned()),
        initial: snapshot.initial().as_str().to_owned(),
        statuses: snapshot
            .statuses()
            .iter()
            .map(|(name, definition)| (name.as_str().to_owned(), status_dto(definition)))
            .collect(),
    }
}

fn status_dto(definition: &StatusDefinition) -> StatusDto {
    match definition {
        StatusDefinition::AgentRun {
            input,
            agent,
            model,
            timeout,
            retries,
            judge,
            next,
        } => StatusDto::AgentRun {
            input: match input {
                AgentInput::Prompt(prompt) => InputDto::Prompt(prompt.as_str().to_owned()),
                AgentInput::Skill(skill) => InputDto::Skill(skill.as_str().to_owned()),
            },
            agent: agent.as_ref().map(|name| name.as_str().to_owned()),
            model: model.as_ref().map(|name| name.as_str().to_owned()),
            timeout: timeout.map(timeout_text),
            retries: *retries,
            judge: judge.as_ref().map(|command| command.tokens().to_vec()),
            next: next.as_str().to_owned(),
        },
        StatusDefinition::Wait => StatusDto::Wait,
        StatusDefinition::Cleanup => StatusDto::Cleanup,
    }
}

/// `TimeoutSpec` の往復表現。無制限は定義の記法と同じ `none`。
fn timeout_text(timeout: TimeoutSpec) -> String {
    match timeout {
        TimeoutSpec::Limited(duration) => format!("{}s", duration.seconds()),
        TimeoutSpec::Unlimited => TimeoutSpec::UNLIMITED.to_owned(),
    }
}

fn execution(dto: &ExecutionDto) -> Result<ExecutionState, String> {
    Ok(match dto {
        ExecutionDto::Pending => ExecutionState::Pending,
        ExecutionDto::Launching { recorded_at } => ExecutionState::Launching {
            recorded_at: timestamp(recorded_at, "execution.recorded_at")?,
        },
        ExecutionDto::Running => ExecutionState::Running,
        ExecutionDto::Completed => ExecutionState::Completed,
        ExecutionDto::Failed => ExecutionState::Failed,
        ExecutionDto::Stopped {
            reason,
            notified_at,
        } => ExecutionState::Stopped {
            reason: match reason {
                StopReasonDto::RetryLimitExceeded => StopReason::RetryLimitExceeded,
                StopReasonDto::JudgeLimitExceeded => StopReason::JudgeLimitExceeded,
                StopReasonDto::SpawnFailLimitExceeded => StopReason::SpawnFailLimitExceeded,
                StopReasonDto::Aborted => StopReason::Aborted,
            },
            notified_at: match notified_at {
                Some(at) => Some(timestamp(at, "execution.notified_at")?),
                None => None,
            },
        },
    })
}

fn workspace(dto: &WorkspaceDto) -> Result<Workspace, String> {
    Ok(Workspace::new(
        WorktreePath::parse(dto.path.clone())
            .map_err(|error| field_error("workspace.path", error.describe()))?,
        BranchName::parse(dto.branch.clone())
            .map_err(|error| field_error("workspace.branch", error.describe()))?,
    ))
}

fn attempt(dto: &AttemptDto) -> Result<AttemptRef, String> {
    let process = match &dto.process {
        Some(process) => Some(ProcessIdent::new(
            Pid::new(process.pid),
            KillIdent::parse(process.kill_ident.clone()).map_err(|error| {
                field_error("current_attempt.process.kill_ident", error.describe())
            })?,
            StartTimeRecord::new(
                ProcessStartTime::parse(process.starttime.ident.clone()).map_err(|error| {
                    field_error("current_attempt.process.starttime.ident", error.describe())
                })?,
                timestamp(
                    &process.starttime.wall,
                    "current_attempt.process.starttime.wall",
                )?,
            ),
        )),
        None => None,
    };

    Ok(AttemptRef::rehydrate(
        AttemptNumber::parse(dto.number)
            .map_err(|error| field_error("current_attempt.number", error.describe()))?,
        RunDirPath::parse(dto.run_dir.clone())
            .map_err(|error| field_error("current_attempt.run_dir", error.describe()))?,
        process,
    ))
}

fn failure(dto: &FailureDto) -> Result<FailureNote, String> {
    FailureNote::parse(
        match dto.kind {
            FailureKindDto::WorktreeCreate => FailureKind::WorktreeCreate,
            FailureKindDto::WorktreeRemove => FailureKind::WorktreeRemove,
            FailureKindDto::ArchiveMove => FailureKind::ArchiveMove,
            FailureKindDto::SpawnFail => FailureKind::SpawnFail,
            FailureKindDto::JudgeFail => FailureKind::JudgeFail,
        },
        dto.message.clone(),
        timestamp(&dto.at, "last_failure.at")?,
    )
    .map_err(|error| field_error("last_failure", error.describe()))
}

fn timestamp(text: &str, location: &str) -> Result<Timestamp, String> {
    Timestamp::parse_rfc3339(text).map_err(|error| field_error(location, error.describe()))
}

fn decode_snapshot(raw: &RawValue) -> Result<WorkflowSnapshot, String> {
    let dto: SnapshotDto = serde_json::from_str(raw.get()).map_err(|error| error.to_string())?;

    let mut statuses = BTreeMap::new();
    for (name, status) in &dto.statuses {
        statuses.insert(
            StatusName::parse(name.clone())
                .map_err(|error| field_error(&format!("statuses.{name}"), error.describe()))?,
            status_definition(name, status)?,
        );
    }

    let definition = WorkflowDefinition::new(
        dto.default_agent
            .clone()
            .map(AgentName::parse)
            .transpose()
            .map_err(|error| field_error("default_agent", error.describe()))?,
        dto.default_model
            .clone()
            .map(ModelName::parse)
            .transpose()
            .map_err(|error| field_error("default_model", error.describe()))?,
        StatusName::parse(dto.initial.clone())
            .map_err(|error| field_error("initial", error.describe()))?,
        statuses,
    )
    .map_err(|error| error.describe())?;

    Ok(WorkflowSnapshot::rehydrate(definition))
}

fn status_definition(name: &str, dto: &StatusDto) -> Result<StatusDefinition, String> {
    let location = |field: &str| format!("statuses.{name}.{field}");

    match dto {
        StatusDto::AgentRun {
            input,
            agent,
            model,
            timeout,
            retries,
            judge,
            next,
        } => Ok(StatusDefinition::AgentRun {
            input: match input {
                InputDto::Prompt(prompt) => AgentInput::Prompt(
                    Prompt::parse(prompt.clone())
                        .map_err(|error| field_error(&location("input"), error.describe()))?,
                ),
                InputDto::Skill(skill) => AgentInput::Skill(
                    SkillName::parse(skill.clone())
                        .map_err(|error| field_error(&location("input"), error.describe()))?,
                ),
            },
            agent: agent
                .clone()
                .map(AgentName::parse)
                .transpose()
                .map_err(|error| field_error(&location("agent"), error.describe()))?,
            model: model
                .clone()
                .map(ModelName::parse)
                .transpose()
                .map_err(|error| field_error(&location("model"), error.describe()))?,
            timeout: timeout
                .as_deref()
                .map(TimeoutSpec::parse)
                .transpose()
                .map_err(|error| field_error(&location("timeout"), error.describe()))?,
            retries: *retries,
            judge: judge
                .clone()
                .map(PlainCommand::parse_tokens)
                .transpose()
                .map_err(|error| field_error(&location("judge"), error.describe()))?,
            next: StatusName::parse(next.clone())
                .map_err(|error| field_error(&location("next"), error.describe()))?,
        }),
        StatusDto::Wait => Ok(StatusDefinition::Wait),
        StatusDto::Cleanup => Ok(StatusDefinition::Cleanup),
    }
}

#[cfg(test)]
mod tests {
    use pulsen_domain::definition::{NameError, WorkflowDefinition};
    use pulsen_domain::task::BranchNameError;

    use super::*;

    fn status(name: &str) -> StatusName {
        StatusName::parse(name.to_owned()).expect("受理される")
    }

    fn snapshot() -> WorkflowSnapshot {
        let definition = WorkflowDefinition::new(
            Some(AgentName::parse("shell".to_owned()).expect("受理される")),
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
        .expect("構造不変条件を満たす");
        WorkflowSnapshot::rehydrate(definition)
    }

    fn registered() -> Task {
        Task::register(
            TaskId::parse("20260811t091530-k3f9qa1b".to_owned()).expect("受理される"),
            WorkflowName::parse("implement".to_owned()).expect("受理される"),
            Target::new(
                RepoPath::parse(PathBuf::from("/abs/path")).expect("受理される"),
                BranchName::parse("main".to_owned()).expect("受理される"),
            ),
            snapshot(),
            Timestamp::parse_rfc3339("2026-08-11T09:15:30Z").expect("受理される"),
        )
    }

    fn encoded(task: &Task) -> String {
        String::from_utf8(encode_task(task).expect("符号化できる")).expect("UTF-8")
    }

    #[test]
    fn 登録直後のタスクは整形されたjsonとして書かれる() {
        assert_eq!(
            encoded(&registered()),
            r#"{
  "task_id": "20260811t091530-k3f9qa1b",
  "workflow_name": "implement",
  "target": {
    "repo": "/abs/path",
    "base_branch": "main"
  },
  "task_status": "queued",
  "execution": {
    "state": "pending"
  },
  "workspace": null,
  "current_attempt": null,
  "counters": {
    "attempt_count": 0,
    "judge_attempt_count": 0,
    "spawn_fail_count": 0
  },
  "last_failure": null,
  "updated_at": "2026-08-11T09:15:30Z",
  "snapshot": {
    "default_agent": "shell",
    "default_model": null,
    "initial": "queued",
    "statuses": {
      "done": {
        "action": "cleanup"
      },
      "queued": {
        "action": "agent_run",
        "input": {
          "prompt": "実装して"
        },
        "agent": null,
        "model": null,
        "timeout": null,
        "retries": null,
        "judge": null,
        "next": "done"
      }
    }
  }
}
"#
        );
    }

    #[test]
    fn タスク側の未知キーは全体の破損として扱われる() {
        let text =
            encoded(&registered()).replace("\"workspace\":", "\"typo\": 1,\n  \"workspace\":");

        let message = decode(text.as_bytes()).expect_err("破損として扱われる");
        assert!(message.contains("typo"), "{message}");
    }

    #[test]
    fn 値の制約の破れは項目名と制約の説明つきで返る() {
        let text = encoded(&registered())
            .replace("\"base_branch\": \"main\"", "\"base_branch\": \"-broken\"");

        let message = decode(text.as_bytes()).expect_err("破損として扱われる");
        assert_eq!(
            message,
            format!(
                "target.base_branch: {}",
                BranchNameError::LeadingHyphen.describe()
            )
        );
    }

    #[test]
    fn スナップショットの値の制約の破れは縮退として扱われる() {
        let text = encoded(&registered()).replace("\"initial\": \"queued\"", "\"initial\": \"\"");

        let degraded = match decode(text.as_bytes()) {
            Err(message) => panic!("タスク側は読める: {message}"),
            Ok(TaskRecord::Intact(_)) => panic!("スナップショットが読めなければ縮退になる"),
            Ok(TaskRecord::SnapshotUnreadable(degraded)) => degraded,
        };
        assert_eq!(
            degraded.snapshot_error(),
            format!("initial: {}", NameError::Empty.describe())
        );
    }

    #[test]
    fn スナップショットの不在は不在のまま書き戻される() {
        let mut document: serde_json::Value =
            serde_json::from_str(&encoded(&registered())).expect("有効な JSON");
        document
            .as_object_mut()
            .expect("マッピング")
            .remove("snapshot");
        let without = serde_json::to_string(&document).expect("直列化できる");
        let carried = carried_snapshot(without.as_bytes()).expect("読み取れる");

        let degraded = match decode(without.as_bytes()) {
            Err(message) => panic!("タスク側は読める: {message}"),
            Ok(TaskRecord::Intact(_)) => panic!("スナップショットが無ければ縮退になる"),
            Ok(TaskRecord::SnapshotUnreadable(degraded)) => degraded,
        };
        let written = encode_degraded(&degraded, carried).expect("符号化できる");

        assert!(
            !String::from_utf8(written)
                .expect("UTF-8")
                .contains("snapshot")
        );
    }
}
