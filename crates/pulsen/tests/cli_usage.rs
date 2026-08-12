//! 引数の解釈と、提供しないことの確認(pages 共通事項 / PAGE-common-007 / PAGE-common-011)。
//!
//! 終了コードの規約(引数の使い方の誤りは 2、`--help` は 0)と、提供するサブコマンドの
//! 集合を固定する。「機械可読な出力形式を提供しない」「設定・ワークフロー定義の作成
//! コマンドを提供しない」は不在によって満たされる主張なので、不在そのものを観測する。

mod common;

use common::run_cli;

/// 引数の使い方の誤りの終了コード。
const USAGE: i32 = 2;

/// clap が自動で足すサブコマンド。
const BUILTIN_SUBCOMMAND: &str = "help";

#[test]
fn 必須の引数が欠けていれば使い方の誤りとして終わる() {
    let run = run_cli(&["add", "--repo", "."]);

    assert_eq!(run.code, Some(USAGE), "{}", run.stderr);
    assert!(
        run.stderr.contains("--workflow"),
        "欠けている引数を示す: {}",
        run.stderr
    );
    assert!(run.stdout.is_empty(), "誤りの案内は標準出力に出さない");
}

#[test]
fn 未知のフラグは使い方の誤りとして終わる() {
    let run = run_cli(&["add", "--json", "--workflow", "implement", "--repo", "."]);

    assert_eq!(run.code, Some(USAGE), "{}", run.stderr);
    assert!(
        run.stderr.contains("--json"),
        "受け取れないフラグを示す: {}",
        run.stderr
    );
    assert!(run.stdout.is_empty(), "誤りの案内は標準出力に出さない");
}

#[test]
fn helpの表示は0で終わり標準出力に出る() {
    let run = run_cli(&["--help"]);

    assert!(run.succeeded(), "0 で終了する(code={:?})", run.code);
    assert!(!run.stdout.is_empty(), "案内は標準出力に出る");
    assert!(run.stderr.is_empty(), "標準エラー出力には出さない");
}

#[test]
fn 提供するサブコマンドはタスク登録だけである() {
    let run = run_cli(&["--help"]);
    run.assert_succeeded();

    assert_eq!(subcommands(&run.stdout), vec!["add", BUILTIN_SUBCOMMAND]);
}

#[test]
fn 機械可読な出力形式のフラグは提供されない() {
    let run = run_cli(&["add", "--help"]);
    run.assert_succeeded();

    for flag in ["--json", "--format", "--output"] {
        assert!(
            !run.stdout.contains(flag),
            "{flag} は提供しない: {}",
            run.stdout
        );
    }
}

/// `--help` の Commands 節に並ぶサブコマンド名。
fn subcommands(help: &str) -> Vec<String> {
    help.lines()
        .skip_while(|line| !line.starts_with("Commands:"))
        .skip(1)
        .take_while(|line| !line.trim().is_empty())
        .filter_map(|line| line.split_whitespace().next().map(str::to_owned))
        .collect()
}
