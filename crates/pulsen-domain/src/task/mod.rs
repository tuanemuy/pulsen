//! タスク集約 — スケジューラーの帳簿。
//!
//! タスクステータス(ユーザー定義)・実行状態(固定6値)・カウンタ・スナップショット・
//! 実行メタデータの参照を持ち、生成と状態遷移を純粋関数として定義する。

mod attempt;
mod branch;
mod counters;
mod degraded;
mod failure;
mod id;
mod path;
mod planner;
mod port;
mod process;
mod state;
// 集約ルートの定義はドメイン名と同じ語で呼ぶのが仕様の語彙に忠実であり、
// ファイル名を言い換えると spec の対応が読み取りにくくなる。
#[allow(clippy::module_inception)]
mod task;
mod time;
mod transition;

pub use attempt::{AttemptNumber, AttemptNumberError, AttemptRef};
pub use branch::{BranchName, BranchNameError, Target, Workspace};
pub use counters::RetryCounters;
pub use degraded::{DegradedTask, DegradedTaskFields};
pub use failure::{FailureKind, FailureNote, FailureNoteError};
pub use id::{TaskId, TaskIdError};
pub use path::{
    AbsolutePathError, RepoPath, RunDirPath, StateRoot, TaskFilePath, WorktreePath, WorktreeRoot,
};
pub use planner::WorkspacePlanner;
pub use port::{
    ArchiveError, Clock, CreateError, ReadError, SaveError, TaskEntry, TaskIdGenerator, TaskLookup,
    TaskRecord, TaskRepository,
};
pub use process::{
    KillIdent, Pid, ProcessIdent, ProcessStartTime, ProcessValueError, StartTimeRecord,
};
pub use state::{ExecutionState, ExecutionStateKind, StateKindError, StopReason};
pub use task::{RehydrateError, Task, TaskFields};
pub use time::{Timestamp, TimestampError};
pub use transition::TransitionError;
