//! 実行の観測と判断を担うドメイン。
//!
//! run ディレクトリの語彙と launching / running の分類、判定・通知の構成、対象の検証・
//! 排他ロック・run ディレクトリの読み書き・プロセスの起動と観測・コマンド実行のポートを
//! 定義する。gc のドメインサービスは、それを使うスライスで足す。

mod judgement;
mod launching;
mod notification;
mod port;
mod running;
mod value;

pub use judgement::JudgementService;
pub use launching::{
    InconsistentRunFiles, LaunchingClassifier, LaunchingDecision, LaunchingRecheck,
};
pub use notification::NotificationService;
pub use port::{
    CommandRunner, ExclusiveLock, Io, KillError, LockError, LockGuard, ProcessController,
    RemnantOutcome, RunFileError, RunStore, SpawnError, TargetError, WorktreeError,
    WorktreeManager, WrapperIdentity, WrapperLaunchSpec,
};
pub use running::{Aliveness, IdentityCheck, RunningClassifier, RunningDecision};
pub use value::{CommandCompletion, ExitCode, JudgeConclusion, JudgeOutcome, PidFileContent};
