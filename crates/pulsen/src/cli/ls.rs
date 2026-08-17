//! `ls` サブコマンドの実行。

use std::path::PathBuf;

use crate::application::list_tasks::{ListTasks, ListTasksError, ListTasksInput, TaskList};

use super::args::LsArgs;
use super::wire::{self, WireError};

/// `ls` の失敗。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LsError {
    /// 起動時の結線・グローバル設定の読み込みで失敗した。
    Wire(WireError),
    /// 一覧そのものに失敗した。
    List(ListTasksError),
}

/// グローバルホームを解決してアダプターを結線し、タスクを一覧する。
///
/// **`runtime.lock()` を渡さない。** 読み取り専用であることを、ユースケースが
/// 排他ロックを受け取れないことで担保する。
pub fn execute(home: Option<PathBuf>, args: LsArgs) -> Result<TaskList, LsError> {
    let runtime = wire::compose(home).map_err(LsError::Wire)?;

    ListTasks::new(runtime.tasks())
        .execute(ListTasksInput {
            status: args.status,
            state: args.state,
            all: args.all,
        })
        .map_err(LsError::List)
}
