//! ロックを別プロセスに保持させるフィクスチャ(ADR-032)。

use std::env;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

/// ロックを取得した保持プロセスが書き出す合図。
const LOCKED: &str = "locked";

/// 合図を待つ期限。保持プロセスが合図も終了も返さないとき、待ち続けずに前提を作れなかった
/// ものとして扱う(ADR-060 と同じ理由 — フィクスチャのハングはテストの失敗より診断が難しい)。
const SIGNAL_DEADLINE: Duration = Duration::from_secs(10);

/// ロックを保持し続けるフィクスチャの実行ファイル。
///
/// パッケージ全体を対象にした `cargo test` は example もビルドするため、バイナリと同じ
/// 出力ディレクトリの `examples/` に置かれる。単一のテストターゲットを指定した実行では
/// ビルドされないため、不在は「前提を作れない環境」として扱う。
pub fn holder_program() -> Option<PathBuf> {
    let binary = Path::new(env!("CARGO_BIN_EXE_pulsen"));
    let program = binary
        .parent()?
        .join("examples")
        .join(format!("lock_holder{}", env::consts::EXE_SUFFIX));
    program.is_file().then_some(program)
}

/// 保持プロセスを起動し、ロックを取得できたかを添えて返す。
pub fn spawn_holder(lock_path: &Path) -> Option<(Child, bool)> {
    let mut holder = Command::new(holder_program()?)
        .arg(lock_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let stdout = holder.stdout.take()?;
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut signal = String::new();
        let read = BufReader::new(stdout).read_line(&mut signal).ok();
        let _ = sender.send(read.map(|_| signal));
    });
    match receiver.recv_timeout(SIGNAL_DEADLINE) {
        Ok(Some(signal)) => Some((holder, signal.trim() == LOCKED)),
        Ok(None) | Err(_) => {
            let _ = holder.kill();
            let _ = holder.wait();
            None
        }
    }
}

/// ロックを保持している別プロセスを用意する。取得できなければ `None`。
pub fn hold(lock_path: &Path) -> Option<Child> {
    let (holder, locked) = spawn_holder(lock_path)?;
    if !locked {
        let _ = release(holder);
        return None;
    }
    Some(holder)
}

/// 保持プロセスの標準入力を閉じて終了を待つ。
pub fn release(mut holder: Child) -> Option<()> {
    drop(holder.stdin.take());
    holder.wait().ok()?;
    Some(())
}
