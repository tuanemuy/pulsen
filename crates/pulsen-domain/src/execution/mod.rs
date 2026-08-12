//! 実行の観測と判断を担うドメイン。
//!
//! 本スライスでは、対象の検証と排他ロックのポートだけを定義する。分類・判定・gc の
//! ドメインサービスは、それらを使うスライスで足す。

mod port;

pub use port::{ExclusiveLock, LockError, LockGuard, TargetError, WorktreeManager};
