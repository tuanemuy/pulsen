//! コマンドライン引数の定義。

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

/// AIエージェントタスクの汎用スケジューラー。
#[derive(Debug, Parser)]
#[command(name = "pulsen", version, about, long_about = None)]
pub struct Cli {
    /// グローバルホームのディレクトリ(既定: 環境変数 PULSEN_HOME、なければ ~/.pulsen)
    #[arg(long, global = true, value_name = "DIR")]
    pub home: Option<PathBuf>,

    /// 実行するサブコマンド。
    #[command(subcommand)]
    pub command: Command,
}

/// サブコマンド。
#[derive(Debug, Subcommand)]
pub enum Command {
    /// タスクを登録する(実行はしない)
    Add(AddArgs),
}

/// `add` の引数。
#[derive(Debug, Args)]
pub struct AddArgs {
    /// ワークフロー名、またはワークフロー定義YAMLのパス
    #[arg(long, value_name = "NAME|PATH")]
    pub workflow: String,

    /// 対象リポジトリのパス
    #[arg(long, value_name = "PATH")]
    pub repo: PathBuf,

    /// ベースブランチ(省略時はリポジトリの HEAD が指すブランチ)
    // doc コメントは clap がヘルプ本文にするため、実装の理由はここに置く。
    // `-` で始まる値も値として受け取る — 「先頭が `-` のブランチ名」はドメインが
    // 拒否する対象であり、引数の使い方の誤りではない(spec 境界値)。
    #[arg(long, value_name = "BRANCH", allow_hyphen_values = true)]
    pub base: Option<String>,
}
