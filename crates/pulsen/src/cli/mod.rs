//! CLI — pulsen の「画面」。
//!
//! 引数の解釈・合成ルート・出力文言・終了コードを持つ。アダプターへの依存は `wire` の
//! 1箇所に閉じる。

pub mod add;
pub mod args;
pub mod exit;
pub mod render;
pub mod wire;

use std::process::ExitCode;

use clap::Parser;

use args::{Cli, Command};

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
    }
}
