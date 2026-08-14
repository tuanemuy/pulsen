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

    /// 1回のtickパスを実行する
    Tick,

    /// デタッチ起動されるラッパーモード(内部用)
    // 利用者向けのインターフェースではないため、ヘルプの一覧には出さない(ADR-077)。
    // 隠すだけで到達はできる — spec が求めるのは「一覧に表示しない」こと。
    #[command(hide = true)]
    Wrapper(WrapperArgs),
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

/// `wrapper` の引数(`WrapperLaunchSpec` の直列化)。
#[derive(Debug, Args)]
pub struct WrapperArgs {
    /// attempt の run ディレクトリ
    #[arg(long, value_name = "DIR")]
    pub run_dir: PathBuf,

    /// エージェントの作業ディレクトリになる worktree
    #[arg(long, value_name = "DIR")]
    pub workspace: PathBuf,

    /// 起動するエージェントのコマンド
    // エージェントのコマンドは `--model` のような `-` 始まりのトークンや、config の配列
    // 形式が許す空文字列トークンを含みうる。明示しないと `spawn_wrapper` が組んだ argv を
    // 受理できず、起動経路だけが壊れて run ディレクトリに何も残らない。
    #[arg(
        trailing_var_arg = true,
        allow_hyphen_values = true,
        value_name = "COMMAND"
    )]
    pub agent_cmd: Vec<String>,
}
