//! `tick` サブコマンドの実行。
//!
//! 定期実行は外部スケジューラーが担うため、tick は任意のカレントディレクトリから
//! 起動されうる。帳簿に載るパスがすべて絶対パスであることでこれが成立する。

use std::path::PathBuf;

use crate::application::tick::{Tick, TickError, TickOutcome};

use super::wire::{self, WireError};

/// `tick` の失敗。
///
/// いずれもタスクの状態を変更しない。ロック競合は失敗ではなくスキップとして 0 で終わる。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TickCommandError {
    /// 起動時の結線・グローバル設定の読み込みで失敗した。
    Wire(WireError),
    /// tick パス自体を実行できなかった。
    Tick(TickError),
}

/// グローバルホームを解決してアダプターを結線し、1回の tick パスを実行する。
pub fn execute(home: Option<PathBuf>) -> Result<TickOutcome, TickCommandError> {
    let runtime = wire::compose(home).map_err(TickCommandError::Wire)?;
    // 自身の実行ファイルのパスを要するのはプロセスを起動する経路だけなので、
    // `compose` ではなくここで解決する(ADR-076)。
    let processes = wire::process_controller().map_err(TickCommandError::Wire)?;
    let commands = wire::command_runner();

    Tick::new(
        runtime.config(),
        runtime.state_root(),
        runtime.worktree_root(),
        runtime.tasks(),
        runtime.lock(),
        runtime.clock(),
        runtime.worktrees(),
        runtime.runs(),
        &processes,
        &commands,
    )
    .execute()
    .map_err(TickCommandError::Tick)
}
