//! 実行の観測と判断を担うドメイン。
//!
//! 本スライスでは、run ディレクトリの語彙と launching の分類、対象の検証と排他ロックの
//! ポートを定義する。判定・gc のドメインサービスは、それらを使うスライスで足す。

mod launching;
mod port;
mod value;

pub use launching::{
    InconsistentRunFiles, LaunchingClassifier, LaunchingDecision, LaunchingRecheck,
};
pub use port::{ExclusiveLock, LockError, LockGuard, TargetError, WorktreeManager};
pub use value::{ExitCode, PidFileContent};
