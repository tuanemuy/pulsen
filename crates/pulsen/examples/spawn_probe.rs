//! デタッチ性を検証するためのテスト用プログラム(ADR-010)。
//!
//! `spawn_wrapper` を呼んで**即座に終了する**。デタッチ性は「呼び出し側プロセスの終了後も
//! ラッパーが完走する」ことなので、同一プロセス内のテストでは表現できない。呼び出し側は
//! このプログラムの終了を待ってから run ディレクトリを観測する。
//!
//! 使い方: `spawn_probe <pulsenのパス> <run_dir> <workspace> <エージェントのコマンド...>`

use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use pulsen::adapter::clock::SystemClock;
use pulsen::adapter::process::{IdentitySource, SystemProcessController};
use pulsen_domain::definition::CommandLine;
use pulsen_domain::execution::{ProcessController, SpawnError, WrapperLaunchSpec};
use pulsen_domain::task::{RunDirPath, WorktreePath};

fn main() -> ExitCode {
    let arguments: Vec<String> = env::args().skip(1).collect();
    let [self_exe, run_dir, workspace, agent_cmd @ ..] = arguments.as_slice() else {
        eprintln!("使い方: spawn_probe <pulsenのパス> <run_dir> <workspace> <コマンド...>");
        return ExitCode::FAILURE;
    };

    let (Ok(run_dir), Ok(workspace), Ok(agent_cmd)) = (
        RunDirPath::parse(PathBuf::from(run_dir)),
        WorktreePath::parse(PathBuf::from(workspace)),
        CommandLine::rehydrate(agent_cmd.to_vec()),
    ) else {
        eprintln!("spawn_probe: 起動引数を値にできない");
        return ExitCode::FAILURE;
    };

    let controller = SystemProcessController::new(
        PathBuf::from(self_exe),
        IdentitySource::platform_default(),
        SystemClock::new(),
    );
    match controller.spawn_wrapper(&WrapperLaunchSpec::new(run_dir, agent_cmd, workspace)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(SpawnError::Failed { message }) => {
            eprintln!("spawn_probe: ラッパーを起動できない: {message}");
            ExitCode::FAILURE
        }
    }
}
