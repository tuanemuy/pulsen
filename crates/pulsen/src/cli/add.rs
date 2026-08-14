//! `add` サブコマンドの実行。

use std::path::PathBuf;

use crate::application::register_task::{
    RegisterTask, RegisterTaskError, RegisterTaskInput, RegisteredTask,
};

use super::args::AddArgs;
use super::wire::{self, WireError};

/// `add` の失敗。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AddError {
    /// 起動時の結線・グローバル設定の読み込みで失敗した。
    Wire(WireError),
    /// 登録そのものに失敗した。
    Register(RegisterTaskError),
}

/// グローバルホームを解決してアダプターを結線し、タスクを登録する。
pub fn execute(home: Option<PathBuf>, args: AddArgs) -> Result<RegisteredTask, AddError> {
    let runtime = wire::compose(home).map_err(AddError::Wire)?;
    let repo = runtime.absolute_repo(args.repo).map_err(AddError::Wire)?;
    // ワークフローの解決基準になるカレントディレクトリとID発行の乱数は、登録の経路
    // だけが要る資源なので `compose` ではなくここで解決する(ADR-099)。
    let workflows = runtime.workflow_store().map_err(AddError::Wire)?;
    let ids = wire::id_generator().map_err(AddError::Wire)?;

    RegisterTask::new(
        runtime.config(),
        &workflows,
        runtime.worktrees(),
        &ids,
        runtime.clock(),
        runtime.tasks(),
        runtime.lock(),
    )
    .execute(RegisterTaskInput {
        workflow: args.workflow,
        repo,
        base: args.base,
    })
    .map_err(AddError::Register)
}
