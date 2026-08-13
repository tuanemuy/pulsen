//! ExclusiveLock の適合スイートを `FileExclusiveLock` に適用する。

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Child;

use common::lock::{HolderCapability, release, spawn_holder};
use pulsen::adapter::lock::FileExclusiveLock;
use pulsen_conformance::ExclusiveLockHarness;
use tempfile::TempDir;

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
        common::lock::hold(&self.lock_path())
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

/// 保持プロセスの合図が期限内に返らない環境でのみスキップされるケース。
const LOCK_HOLDER_CASES: [&str; 4] = [
    "tc_port_exclusive_lock_002",
    "tc_port_exclusive_lock_003",
    "tc_port_exclusive_lock_004",
    "tc_port_exclusive_lock_005",
];

/// この環境でスキップを許容するケース。
///
/// 許容するのは保持プロセスの合図が期限内に返らない環境だけ。実行ファイルの不在や
/// 起動の失敗は環境の能力ではないので、緑にせずケースの失敗にする
/// (HOOKS.md / ADR-068)。同じ判定を CLI 側の受け入れテスト
/// (TC-task-register-task-017)も使うため、両者で扱いが揃う(ADR-055)。
fn allowed_skips() -> Vec<&'static str> {
    match common::lock::holder_capability() {
        HolderCapability::SignalTimedOut => LOCK_HOLDER_CASES.to_vec(),
        HolderCapability::Available(_)
        | HolderCapability::ProgramMissing
        | HolderCapability::ProgramUnusable(_) => Vec::new(),
    }
}

pulsen_conformance::exclusive_lock_conformance!(FileExclusiveLockHarness::new(), allowed_skips());
