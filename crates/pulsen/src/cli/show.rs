//! `show` サブコマンドの実行。

use std::path::PathBuf;

use crate::application::show_task::{ShowTask, ShowTaskError, ShowTaskInput, TaskDetail};

use super::args::ShowArgs;
use super::wire::{self, WireError};

/// `show` の失敗。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShowError {
    /// 起動時の結線・グローバル設定の読み込みで失敗した。
    Wire(WireError),
    /// 詳細の取得そのものに失敗した。
    Show(ShowTaskError),
}

/// グローバルホームを解決してアダプターを結線し、タスクの詳細を取得する。
///
/// **`runtime.lock()` を渡さない。** 読み取り専用であることを、ユースケースが
/// 排他ロックを受け取れないことで担保する。
pub fn execute(home: Option<PathBuf>, args: ShowArgs) -> Result<TaskDetail, ShowError> {
    let runtime = wire::compose(home).map_err(ShowError::Wire)?;

    ShowTask::new(
        runtime.config(),
        runtime.state_root(),
        runtime.tasks(),
        runtime.runs(),
    )
    .execute(ShowTaskInput {
        task_id: args.task_id,
    })
    .map_err(ShowError::Show)
}
