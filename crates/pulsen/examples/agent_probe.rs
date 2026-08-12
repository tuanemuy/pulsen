//! 適合テストがエージェントとして起動するテスト用プログラム(ADR-010)。
//!
//! シェル(`sh -c`)に頼るとクロスプラットフォームで破綻し、「シェルを介さない直接起動」を
//! 検証するケースと矛盾する。利用者に見えるサブコマンドを増やさないため、bin ではなく
//! example として置く。
//!
//! | モード | 振る舞い |
//! |---|---|
//! | `exit <n>` | `n` で終了する |
//! | `print <標準出力> <標準エラー>` | それぞれへ書いて 0 で終了する |
//! | `check-cwd <期待パス>` | 作業ディレクトリが一致すれば 0、しなければ 1 |
//! | `echo-args <トークン...>` | 受け取ったトークンを1行ずつ標準出力へ書いて 0 で終了する |
//! | `sleep <ミリ秒>` | その時間だけ実行を続けてから 0 で終了する |
//! | `abort` | exit code を持たない終了(シグナル死)をする |

use std::io::Write;
use std::path::Path;
use std::process::exit;

/// 使い方が満たされないときの終了コード。エージェントの符号化値(126 / 127)と紛れない値。
const MISUSE: i32 = 64;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some((mode, rest)) = args.split_first() else {
        misuse("モードが指定されていない");
    };

    match mode.as_str() {
        "exit" => exit(number(rest.first())),
        "print" => print(rest),
        "check-cwd" => exit(check_cwd(rest.first())),
        "echo-args" => echo_args(rest),
        "sleep" => sleep(rest.first()),
        "abort" => std::process::abort(),
        other => misuse(&format!("知らないモード: {other}")),
    }
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

/// 作業ディレクトリが期待どおりかを終了コードで返す。
///
/// シンボリックリンク経由のパスでも一致するよう、両側を正規化してから比べる。
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

/// 受け取ったトークンを1行ずつ書き出す。
///
/// 自分が受け取った値を自分で照合しても、シェルを経由した場合の食い違いは検出できない。
/// 期待との照合は、渡した側(呼び出し元)が標準出力の内容に対して行う。
fn echo_args(rest: &[String]) -> ! {
    let mut stdout = std::io::stdout();
    for token in rest {
        if writeln!(stdout, "{token}").is_err() {
            exit(MISUSE);
        }
    }
    if stdout.flush().is_err() {
        exit(MISUSE);
    }
    exit(0)
}

/// 指定のミリ秒だけ実行を続ける。
fn sleep(millis: Option<&String>) -> ! {
    let Some(millis) = millis.and_then(|value| value.parse::<u64>().ok()) else {
        misuse("sleep はミリ秒を取る");
    };
    std::thread::sleep(std::time::Duration::from_millis(millis));
    exit(0)
}

/// 終了コードとして解釈する。
fn number(value: Option<&String>) -> i32 {
    let Some(parsed) = value.and_then(|value| value.parse::<i32>().ok()) else {
        misuse("終了コードを解釈できない");
    };
    parsed
}

fn misuse(reason: &str) -> ! {
    eprintln!("agent_probe: {reason}");
    exit(MISUSE)
}
