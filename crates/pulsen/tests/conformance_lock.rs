//! ExclusiveLock の適合スイートを `FileExclusiveLock` に適用する。

use std::env;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use pulsen::adapter::lock::FileExclusiveLock;
use pulsen_conformance::ExclusiveLockHarness;
use tempfile::TempDir;

/// ロックを保持した保持プロセスが書き出す合図。
const LOCKED: &str = "locked";

/// ロックを保持し続けるフィクスチャの実行ファイル(ADR-032)。
///
/// `cargo test` は example もビルドするため、バイナリと同じ出力ディレクトリの
/// `examples/` に必ず置かれる。
fn lock_holder_program() -> Option<PathBuf> {
    let binary = Path::new(env!("CARGO_BIN_EXE_pulsen"));
    let program = binary
        .parent()?
        .join("examples")
        .join(format!("lock_holder{}", env::consts::EXE_SUFFIX));
    program.is_file().then_some(program)
}

/// 保持プロセスを起動し、ロックを取得できたかを返す。
fn spawn_holder(lock_path: &Path) -> Option<(Child, bool)> {
    let mut holder = Command::new(lock_holder_program()?)
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

/// 保持プロセスの標準入力を閉じて終了を待つ。
fn release(mut holder: Child) -> Option<()> {
    drop(holder.stdin.take());
    holder.wait().ok()?;
    Some(())
}

/// 一時ディレクトリをグローバルホームに見立てたハーネス。
struct FileExclusiveLockHarness {
    home: TempDir,
    /// 別ホームの寿命を握るだけのフィールド。
    _separate: TempDir,
    lock: FileExclusiveLock,
    separate_lock: FileExclusiveLock,
    unusable: FileExclusiveLock,
}

impl FileExclusiveLockHarness {
    fn new() -> Self {
        let home = tempfile::tempdir().expect("一時ホームを作れる");
        let separate = tempfile::tempdir().expect("別の一時ホームを作れる");
        // ロックの置き場としてディレクトリがあると開けない。権限操作と違い root でも
        // Windows でも成立するため、機構の異常を環境非依存に再現できる(ADR-032)。
        let occupied = home.path().join("unusable-lock");
        fs::create_dir(&occupied).expect("ディレクトリを作れる");

        let lock = FileExclusiveLock::new(lock_path(home.path()));
        let separate_lock = FileExclusiveLock::new(lock_path(separate.path()));
        Self {
            home,
            _separate: separate,
            lock,
            separate_lock,
            unusable: FileExclusiveLock::new(occupied),
        }
    }

    fn lock_path(&self) -> PathBuf {
        lock_path(self.home.path())
    }
}

/// グローバルホーム配下のロックファイル。
fn lock_path(home: &Path) -> PathBuf {
    home.join("state").join("lock")
}

impl ExclusiveLockHarness for FileExclusiveLockHarness {
    type Lock = FileExclusiveLock;
    type Holder = Child;

    fn lock(&self) -> &Self::Lock {
        &self.lock
    }

    fn hold_from_other_process(&self) -> Option<Self::Holder> {
        let (holder, locked) = spawn_holder(&self.lock_path())?;
        if !locked {
            let _ = release(holder);
            return None;
        }
        Some(holder)
    }

    fn kill_holder(&self, mut holder: Self::Holder) -> Option<()> {
        holder.kill().ok()?;
        // 待たないと解放が観測できる保証がない(終了処理の完了を待つ)。
        holder.wait().ok()?;
        Some(())
    }

    fn release_holder(&self, holder: Self::Holder) -> Option<()> {
        release(holder)
    }

    fn try_acquire_from_other_process(&self) -> Option<bool> {
        let (holder, locked) = spawn_holder(&self.lock_path())?;
        let _ = release(holder);
        Some(locked)
    }

    fn separate_home(&self) -> Option<&Self::Lock> {
        Some(&self.separate_lock)
    }

    fn unusable_lock(&self) -> Option<&Self::Lock> {
        Some(&self.unusable)
    }
}

pulsen_conformance::exclusive_lock_conformance!(FileExclusiveLockHarness::new());
