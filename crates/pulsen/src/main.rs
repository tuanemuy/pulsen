//! `pulsen` の entrypoint。処理は `cli::run` に委ね、ここは終了コードを返すだけにする。

use std::process::ExitCode;

fn main() -> ExitCode {
    pulsen::cli::run()
}
