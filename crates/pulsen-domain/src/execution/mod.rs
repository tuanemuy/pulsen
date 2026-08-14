//! 実行の観測と判断を担うドメイン。
//!
//! run ディレクトリの語彙と launching の分類、対象の検証・排他ロック・run ディレクトリの
//! 読み書き・プロセスの起動のポートを定義する。判定・gc のドメインサービスは、それらを
//! 使うスライスで足す。

mod launching;
mod port;
mod value;

pub use launching::{
    InconsistentRunFiles, LaunchingClassifier, LaunchingDecision, LaunchingRecheck,
};
pub use port::{
    ExclusiveLock, Io, LockError, LockGuard, ProcessController, RunFileError, RunStore, SpawnError,
    TargetError, WorktreeError, WorktreeManager, WrapperIdentity, WrapperLaunchSpec,
};
pub use value::{ExitCode, PidFileContent};
