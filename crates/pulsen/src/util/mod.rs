//! アダプターが共有する低レベル操作。
//!
//! アトミック性が要る操作は個別に再実装せず、ここに集約する(CLAUDE.md 技術方針)。

pub mod atomic;
pub mod fsdir;
