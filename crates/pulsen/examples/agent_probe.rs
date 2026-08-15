//! 適合テストがエージェントとして起動するテスト用プログラム(ADR-082)。
//!
//! シェル(`sh -c`)に頼るとクロスプラットフォームで破綻し、「シェルを介さない直接起動」を
//! 検証するケースと矛盾する。利用者に見えるサブコマンドを増やさないため、bin ではなく
//! example として置く。
//!
//! | モード | 振る舞い |
//! |---|---|
//! | `exit <n>` | `n` で終了する |
//! | `exit-from <パス>` | そのファイルの内容を終了コードとして終了する(不在なら 0) |
//! | `print <標準出力> <標準エラー>` | それぞれへ書いて 0 で終了する |
//! | `check-cwd <期待パス>` | 作業ディレクトリが一致すれば 0、しなければ 1 |
//! | `echo-args <トークン...>` | 受け取ったトークンを1行ずつ標準出力へ書いて 0 で終了する |
//! | `sleep <ミリ秒>` | その時間だけ実行を続けてから 0 で終了する |
//! | `wait-for <パス> <上限ミリ秒>` | 滞留の合図を標準出力へ書き、そのパスにファイルが現れるまで実行を続け、解放の合図を書いて 0 で終了する |
//! | `spawn-child <PID記録先> <解放パス> <上限ミリ秒>` | 自分と同じプログラムを子として起こし、自身と子のPIDを記録先へ書いてから `wait-for` と同じ滞留に入る |
//! | `abort` | exit code を持たない終了(シグナル死)をする |

use std::io::Write;
use std::path::Path;
use std::process::exit;
use std::time::{Duration, Instant};

/// 使い方が満たされないときの終了コード。エージェントの符号化値(126 / 127)と紛れない値。
const MISUSE: i32 = 64;

/// 解放されないまま上限に達したときの終了コード。使い方の誤りとも符号化値とも紛れない値。
const NOT_RELEASED: i32 = 65;

/// 解放を待つポーリングの間隔。
const POLL_INTERVAL: Duration = Duration::from_millis(20);

/// 滞留に入ったことを告げる合図。
///
/// 呼び出し側は標準出力(ラッパー経由ならログ)への出現をもって「エージェントが実行中で
/// ある」ことを観測できる。ラッパーが作るログの存在だけでは、エージェントの起動が済んだ
/// かどうかまでは決まらない。
const WAITING: &str = "waiting";

/// 解放を観測して終わることを告げる合図。
///
/// ラッパーが先に死んだ場合、このプロセスの終わりを告げるものは他に無い。呼び出し側は
/// この合図で、一時ディレクトリを消す前に滞留が解けたことを確かめられる。
const RELEASED: &str = "released";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some((mode, rest)) = args.split_first() else {
        misuse("モードが指定されていない");
    };

    match mode.as_str() {
        "exit" => exit(number(rest.first())),
        "exit-from" => exit(exit_from(rest.first())),
        "print" => print(rest),
        "check-cwd" => exit(check_cwd(rest.first())),
        "echo-args" => echo_args(rest),
        "sleep" => sleep(rest.first()),
        "wait-for" => wait_for(rest),
        "spawn-child" => spawn_child(rest),
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
    std::thread::sleep(Duration::from_millis(millis));
    exit(0)
}

/// 指定パスにファイルが現れるまで実行を続ける。
///
/// 滞留の終わりを呼び出し側が決めるので、「このプロセスが生きている間に別の処理が走る」
/// という前提が実行環境の速さに左右されない。上限は、解放が来ないときにこのプロセスが
/// 残り続けないための歯止めであって、前提の根拠ではない。
fn wait_for(rest: &[String]) -> ! {
    let [path, limit, ..] = rest else {
        misuse("wait-for は待つパスと上限ミリ秒の2つを取る");
    };
    let Some(limit) = limit.parse::<u64>().ok().map(Duration::from_millis) else {
        misuse("wait-for の上限を解釈できない");
    };
    let mut stdout = std::io::stdout();
    signal(&mut stdout, WAITING);
    let path = Path::new(path);
    let deadline = Instant::now() + limit;
    while !path.exists() {
        if Instant::now() >= deadline {
            eprintln!(
                "agent_probe: {limit:?} 以内に {} が現れなかった",
                path.display()
            );
            exit(NOT_RELEASED);
        }
        std::thread::sleep(POLL_INTERVAL);
    }
    signal(&mut stdout, RELEASED);
    exit(0)
}

/// 子プロセスを起こしてから滞留に入る。
///
/// 実行単位(プロセスグループ相当)は起動の際に決まり、子はそれを引き継ぐ。「実行単位に
/// 属する全プロセスが終了する」ことを確かめるには、ラッパー・エージェント・その子という
/// 3階層が要る。PIDを記録先へ書くのは、呼び出し側がプロセスの同定手段を持たないため。
fn spawn_child(rest: &[String]) -> ! {
    let [pid_file, release, limit, ..] = rest else {
        misuse("spawn-child は PID記録先・解放パス・上限ミリ秒の3つを取る");
    };
    let Ok(program) = std::env::current_exe() else {
        misuse("自身の実行ファイルを特定できない");
    };
    let child = std::process::Command::new(program)
        .args(["wait-for", release, limit])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
    let Ok(child) = child else {
        misuse("子プロセスを起こせない");
    };

    // 滞留に入る前に書く。呼び出し側はこのファイルの出現をもって「実行単位のメンバーが
    // 揃った」ことを観測する。
    if std::fs::write(
        pid_file,
        format!("{}\n{}\n", std::process::id(), child.id()),
    )
    .is_err()
    {
        misuse("PIDを記録できない");
    }
    wait_for(&[release.clone(), limit.clone()])
}

/// 合図を1行書いて送り出す。
fn signal(stdout: &mut std::io::Stdout, text: &str) {
    if writeln!(stdout, "{text}")
        .and_then(|()| stdout.flush())
        .is_err()
    {
        exit(MISUSE);
    }
}

/// 制御ファイルの内容を終了コードとして解釈する。
///
/// 同じコマンドのまま attempt ごとに結末を変えるための口。呼び出し側は制御ファイルを
/// 書き換えるだけでよく、ワークフロー定義を触らずに一過性の失敗と回復を作れる。
fn exit_from(path: Option<&String>) -> i32 {
    let Some(path) = path else {
        misuse("exit-from は制御ファイルのパスを取る");
    };
    match std::fs::read_to_string(path) {
        Ok(text) => text.trim().parse().unwrap_or(MISUSE),
        Err(_) => 0,
    }
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
