//! 適合テスト・受け入れテストが判定コマンド / 通知コマンドとして起動するテスト用
//! プログラム(ADR-082)。
//!
//! `CommandRunner` は標準出力・標準エラーを捕捉しない契約なので、観測結果は exit code か
//! このプログラム自身が書き出すファイルで表す。シェル(`sh -c`)に頼るとクロスプラット
//! フォームで破綻し、「シェルを介さない直接起動」を検証するケースと矛盾する。
//!
//! | モード | 振る舞い |
//! |---|---|
//! | `exit <n>` | `n` で終了する |
//! | `abort` | exit code を持たない終了(シグナル死)をする |
//! | `check-args <期待ファイル> <トークン...>` | 残りの引数が期待ファイルの各行とリテラル一致すれば 0、しなければ 1 |
//! | `check-env <名前> <期待値>` | その環境変数が期待値なら 0、しなければ 1 |
//! | `check-cwd <期待パス>` | 作業ディレクトリが一致すれば 0、しなければ 1 |
//! | `print <標準出力> <標準エラー>` | それぞれへ書いて 0 で終了する |
//! | `sleep <ミリ秒>` | その時間だけ実行を続けてから 0 で終了する |
//! | `record <ミリ秒> <証跡パス>` | その時間だけ実行を続け、終了直前に証跡を書いて 0 で終了する |
//! | `log <記録先> <環境変数名...>` | 受け取った環境変数を1行1件で追記し、`<記録先>.exit` があればその内容を終了コードにする |
//!
//! `log` は受け入れテスト用。判定・通知コマンドが受け取った文脈を証跡として残しつつ、
//! 終了コードを制御ファイルで外から差し替えられるようにする — コマンド自身は制御ファイルを
//! 読むだけなので、ツール側の判定の冪等性だけを主張できる。

use std::io::Write;
use std::path::Path;
use std::process::exit;
use std::time::Duration;

/// 使い方が満たされないときの終了コード。判定プロトコルの4値と紛れない値。
const MISUSE: i32 = 64;

/// 証跡を書けなかったときの終了コード。
const NOT_RECORDED: i32 = 65;

/// 終了コードを差し替える制御ファイルの拡張子。
const EXIT_CONTROL_SUFFIX: &str = ".exit";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some((mode, rest)) = args.split_first() else {
        misuse("モードが指定されていない");
    };

    match mode.as_str() {
        "exit" => exit(number(rest.first())),
        "abort" => std::process::abort(),
        "check-args" => exit(check_args(rest)),
        "check-env" => exit(check_env(rest)),
        "check-cwd" => exit(check_cwd(rest.first())),
        "print" => print(rest),
        "sleep" => sleep(rest.first()),
        "record" => record(rest),
        "log" => log(rest),
        other => misuse(&format!("知らないモード: {other}")),
    }
}

/// 受け取った引数が期待どおりかを終了コードで返す。
///
/// 自分が受け取った値を自分で照合するのは、`CommandRunner` の結果に出力が現れない
/// (捕捉しない契約)ため。渡した側は exit code でしか観測できない。期待は引数ではなく
/// ファイルで受け取る — 引数で渡すと、シェルが解釈した場合に期待側も同じように歪んで
/// 照合が通ってしまう。
fn check_args(rest: &[String]) -> i32 {
    let Some((expected_file, actual)) = rest.split_first() else {
        misuse("check-args は期待ファイルを取る");
    };
    let Ok(text) = std::fs::read_to_string(expected_file) else {
        return 1;
    };
    let expected: Vec<&str> = text.lines().collect();
    i32::from(expected != actual.iter().map(String::as_str).collect::<Vec<_>>())
}

/// 環境変数が期待どおりかを終了コードで返す。
fn check_env(rest: &[String]) -> i32 {
    let [name, expected, ..] = rest else {
        misuse("check-env は変数名と期待値の2つを取る");
    };
    match std::env::var(name) {
        Ok(actual) => i32::from(&actual != expected),
        Err(_) => 1,
    }
}

/// 作業ディレクトリが期待どおりかを終了コードで返す。
fn check_cwd(expected: Option<&String>) -> i32 {
    let Some(expected) = expected else {
        misuse("check-cwd は期待するパスを取る");
    };
    let (Ok(actual), Ok(expected)) = (
        std::env::current_dir().and_then(std::fs::canonicalize),
        std::fs::canonicalize(Path::new(expected)),
    ) else {
        return 1;
    };
    i32::from(actual != expected)
}

/// 標準出力・標準エラーへそれぞれ書く。
fn print(rest: &[String]) -> ! {
    let [out, err, ..] = rest else {
        misuse("print は標準出力と標準エラーの2つを取る");
    };
    let mut stdout = std::io::stdout();
    let mut stderr = std::io::stderr();
    if write!(stdout, "{out}")
        .and_then(|()| stdout.flush())
        .is_err()
    {
        exit(MISUSE);
    }
    if write!(stderr, "{err}")
        .and_then(|()| stderr.flush())
        .is_err()
    {
        exit(MISUSE);
    }
    exit(0)
}

/// 指定のミリ秒だけ実行を続ける。
fn sleep(millis: Option<&String>) -> ! {
    std::thread::sleep(Duration::from_millis(millis_of(millis)));
    exit(0)
}

/// 指定のミリ秒だけ実行を続け、終了直前に証跡を書く。
fn record(rest: &[String]) -> ! {
    let [millis, evidence, ..] = rest else {
        misuse("record は実行時間と証跡パスの2つを取る");
    };
    std::thread::sleep(Duration::from_millis(millis_of(Some(millis))));
    if std::fs::write(evidence, b"done").is_err() {
        exit(NOT_RECORDED);
    }
    exit(0)
}

/// 受け取った環境変数を記録先へ1行1件で追記し、制御ファイルの終了コードで終わる。
fn log(rest: &[String]) -> ! {
    let Some((path, names)) = rest.split_first() else {
        misuse("log は記録先と環境変数名を取る");
    };
    let mut line = String::new();
    for name in names {
        let value = std::env::var(name).unwrap_or_default();
        line.push_str(&format!("{name}={value}\t"));
    }
    line.push('\n');

    let appended = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .and_then(|mut file| file.write_all(line.as_bytes()));
    if appended.is_err() {
        exit(NOT_RECORDED);
    }

    let control = format!("{path}{EXIT_CONTROL_SUFFIX}");
    match std::fs::read_to_string(&control) {
        Ok(text) => exit(text.trim().parse().unwrap_or(MISUSE)),
        Err(_) => exit(0),
    }
}

/// 終了コードとして解釈する。
fn number(value: Option<&String>) -> i32 {
    let Some(parsed) = value.and_then(|value| value.parse::<i32>().ok()) else {
        misuse("終了コードを解釈できない");
    };
    parsed
}

/// ミリ秒として解釈する。
fn millis_of(value: Option<&String>) -> u64 {
    let Some(parsed) = value.and_then(|value| value.parse::<u64>().ok()) else {
        misuse("ミリ秒を解釈できない");
    };
    parsed
}

fn misuse(reason: &str) -> ! {
    eprintln!("judge_probe: {reason}");
    exit(MISUSE)
}
