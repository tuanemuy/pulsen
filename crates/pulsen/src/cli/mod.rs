//! CLI — pulsen の「画面」。
//!
//! 引数の解釈・合成ルート・出力文言・終了コードを持つ。アダプターへの依存は `wire` の
//! 1箇所に閉じる。

pub mod add;
pub mod args;
pub mod exit;
pub mod render;
pub mod tick;
pub mod wire;
pub mod wrapper;

use std::process::ExitCode;

use clap::Parser;

use args::{Cli, Command};

use crate::application::tick::TickOutcome;

/// 引数を解釈してサブコマンドを実行し、規約どおりの終了コードを返す。
///
/// 成功は標準出力、エラーは標準エラー出力に人間可読なテキストで書く。
pub fn run() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => return exit::from_clap(&error),
    };

    match cli.command {
        Command::Add(args) => match add::execute(cli.home, args) {
            Ok(registered) => {
                println!("{}", render::registered(&registered));
                exit::SUCCESS
            }
            Err(error) => {
                eprintln!("{}", render::add_error(&error));
                exit::FAILURE
            }
        },
        // ロック競合は失敗ではなくスキップ。cron 運用でアラートにしないため 0 で終える。
        Command::Tick => match tick::execute(cli.home) {
            Ok(TickOutcome::Skipped) => {
                println!("{}", render::tick_skipped());
                exit::SUCCESS
            }
            Ok(TickOutcome::Completed(summary)) => {
                println!("{}", render::tick_summary(&summary));
                exit::SUCCESS
            }
            Err(error) => {
                eprintln!("{}", render::tick_error(&error));
                exit::FAILURE
            }
        },
        // 結果は run ディレクトリのファイルとして現れる。標準出力には何も出さない。
        Command::Wrapper(args) => match wrapper::execute(args) {
            Ok(_) => exit::SUCCESS,
            Err(error) => {
                eprintln!("{}", render::wrapper_error(&error));
                exit::FAILURE
            }
        },
    }
}
