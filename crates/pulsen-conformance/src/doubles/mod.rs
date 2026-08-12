//! ポートのテストダブル。
//!
//! ユースケースの分岐を網羅するための**スクリプト式**の実装を置く(ADR-028)。実
//! アダプター(乱数ID・ファイルロック・git CLI・実ファイルシステム・実プロセス)では
//! 外から作れない状況 — ID衝突・`LockError::Failed`・`TargetError::Failed`・入出力
//! エラー・同定情報の取得失敗・spawn の同期エラー・猶予時間の境界 — を、ポートを
//! 差し替えることで表す。
//!
//! 各ダブルは「あらかじめ与えた結果列を順に返す」ことと「受け取った呼び出しを記録して
//! 検査できる」ことだけを持つ。汎用の in-memory ストア(適合スイート全件を通す実装)は
//! 置かない — 用途が違う(分岐網羅 vs 契約適合)。
//!
//! スクリプトを使い切った呼び出しと、ダブルが扱わない操作はパニックさせる。どちらも
//! テスト側の不変条件違反であり、値として返すとテストが誤った前提のまま緑になる。

mod clock;
mod lock;
mod process;
mod run_store;
mod stores;
mod task_id;
mod task_repository;
mod worktree;

#[cfg(test)]
mod tests;

pub use clock::{FixedClock, SettableClock};
pub use lock::{LockOutcome, ScriptedExclusiveLock};
pub use process::{ProcessControllerCall, ScriptedProcessController};
pub use run_store::{RunStoreCall, ScriptedRunStore};
pub use stores::ScriptedWorkflowStore;
pub use task_id::ScriptedTaskIdGenerator;
pub use task_repository::ScriptedTaskRepository;
pub use worktree::{ScriptedWorktreeManager, WorktreeManagerCall};
