//! ExclusiveLock の適合スイートを `FileExclusiveLock` に適用する。

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Child;

use common::lock::{release, spawn_holder};
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

/// 別プロセスにロックを保持させられない環境でのみスキップされるケース。
const LOCK_HOLDER_CASES: [&str; 4] = [
    "tc_port_exclusive_lock_002",
    "tc_port_exclusive_lock_003",
    "tc_port_exclusive_lock_004",
    "tc_port_exclusive_lock_005",
];

/// この環境でスキップを許容するケース。
///
/// ロック機構の異常は別ハンドル(`unusable_lock`)で組めるため、許容するのは保持プロセスの
/// 実行ファイルが無い場合の上記4件だけ。宣言をプラットフォームではなく実行時の述語で
/// 決めることで、同じフィクスチャを使う CLI 側の受け入れテスト
/// (TC-task-register-task-017)と扱いが揃う(ADR-055)。
fn allowed_skips() -> Vec<&'static str> {
    if common::lock::holder_program().is_some() {
        Vec::new()
    } else {
        LOCK_HOLDER_CASES.to_vec()
    }
}

pulsen_conformance::exclusive_lock_conformance!(FileExclusiveLockHarness::new(), allowed_skips());
