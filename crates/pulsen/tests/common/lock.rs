//! ロックを別プロセスに保持させるフィクスチャ(ADR-032)。

use std::env;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

/// ロックを取得した保持プロセスが書き出す合図。
const LOCKED: &str = "locked";

/// ロックを保持し続けるフィクスチャの実行ファイル。
///
/// `cargo test` は example もビルドするため、バイナリと同じ出力ディレクトリの
/// `examples/` に必ず置かれる。
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
    let mut signal = String::new();
    BufReader::new(stdout).read_line(&mut signal).ok()?;
    Some((holder, signal.trim() == LOCKED))
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
