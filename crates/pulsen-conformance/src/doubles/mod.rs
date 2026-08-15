//! ポートのテストダブル。
//!
//! ユースケースの分岐を網羅するための**スクリプト式**の実装を置く。実
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

use std::sync::atomic::{AtomicU64, Ordering};

mod clock;
mod command_runner;
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
pub use command_runner::{CommandRunnerCall, ScriptedCommandRunner};
pub use lock::{LockOutcome, ScriptedExclusiveLock};
pub use process::{ProcessControllerCall, ScriptedProcessController};
pub use run_store::{RunStoreCall, ScriptedRunStore};
pub use stores::ScriptedWorkflowStore;
pub use task_id::ScriptedTaskIdGenerator;
pub use task_repository::ScriptedTaskRepository;
pub use worktree::{ScriptedWorktreeManager, WorktreeManagerCall};

/// 記録された1件の順序。
///
/// ダブルはそれぞれ独立した列に記録するため、列を別々に見るだけでは「保存が先か通知が
/// 先か」を突き合わせられない。ポートをまたいで順序を規定する契約(凍結を書く → 通知を
/// 実行する)は、この採番で1本の列に並べ直してはじめて主張できる。
///
/// 採番元は1つで、番号は単調に増える。並行して走るテストの記録が番号を飛ばすことは
/// あっても、1つのハーネスに属する記録どうしの前後関係は保たれる。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RecordSeq(u64);

impl RecordSeq {
    /// 次の番号を採る。
    fn next() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        Self(COUNTER.fetch_add(1, Ordering::Relaxed))
    }
}
