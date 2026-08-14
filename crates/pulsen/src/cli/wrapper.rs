//! `wrapper` サブコマンドの実行。
//!
//! 起動引数を `RunDirPath` / `WorktreePath` / `CommandLine` に通すのがこのコマンドの
//! parse 境界になる。直列化が破れていれば run ディレクトリに何も書かずに終える —
//! pid が現れないことで、tick の猶予時間経路が spawn 失敗として分類する。

use std::path::PathBuf;

use pulsen_domain::definition::{CommandError, CommandLine};
use pulsen_domain::execution::WrapperLaunchSpec;
use pulsen_domain::task::{AbsolutePathError, RunDirPath, WorktreePath};

use crate::application::run_wrapper::{RunWrapper, WrapperOutcome};

use super::args::WrapperArgs;
use super::wire::{self, WireError};

/// `wrapper` の失敗。
///
/// いずれの場合も run ディレクトリには何も書かれない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WrapperError {
    /// `--run-dir` が絶対パスではない。
    InvalidRunDir(AbsolutePathError),
    /// `--workspace` が絶対パスではない。
    InvalidWorkspace(AbsolutePathError),
    /// エージェントのコマンドを復元できない。
    InvalidAgentCommand(CommandError),
    /// アダプターの結線で失敗した。
    Wire(WireError),
    /// 同定情報を残せず、何も書かずに終えた。
    NothingRecorded {
        /// 対象の run ディレクトリ。
        run_dir: PathBuf,
    },
}

/// 起動引数を検証してアダプターを結線し、ラッパーを実行する。
pub fn execute(args: WrapperArgs) -> Result<WrapperOutcome, WrapperError> {
    let run_dir = RunDirPath::parse(args.run_dir).map_err(WrapperError::InvalidRunDir)?;
    let workspace = WorktreePath::parse(args.workspace).map_err(WrapperError::InvalidWorkspace)?;
    let agent_cmd =
        CommandLine::rehydrate(args.agent_cmd).map_err(WrapperError::InvalidAgentCommand)?;

    let runtime = wire::compose_wrapper(&run_dir).map_err(WrapperError::Wire)?;
    let spec = WrapperLaunchSpec::new(run_dir, agent_cmd, workspace);

    match RunWrapper::new(runtime.runs(), runtime.processes()).execute(&spec) {
        outcome @ (WrapperOutcome::Ran(_) | WrapperOutcome::Suppressed) => Ok(outcome),
        WrapperOutcome::Silent => Err(WrapperError::NothingRecorded {
            run_dir: spec.run_dir().as_path().to_path_buf(),
        }),
    }
}
