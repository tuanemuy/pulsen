//! RunStore の適合スイートを `FsRunStore` に適用する。
//!
//! 破損のフィクスチャは「JSON として読めない内容を対象ファイルの位置に置く」形で作る
//! (ADR-080 — 各ファイルは JSON なので、構文として壊せば分類が構文で決まる)。

use std::fs;
use std::path::Path;

use pulsen::adapter::run_store::FsRunStore;
use pulsen::util::fsdir::ensure_dir;
use pulsen_conformance::{Restore, RunFileKind, RunStoreHarness};
use pulsen_domain::execution::RunStore;
use pulsen_domain::task::{AttemptNumber, RunDirPath, StateRoot, TaskId};
use tempfile::TempDir;

/// 一時ディレクトリを状態ディレクトリに見立てたハーネス。
struct FsRunStoreHarness {
    _home: TempDir,
    state_root: StateRoot,
    store: FsRunStore,
}

impl FsRunStoreHarness {
    fn new() -> Self {
        let home = tempfile::tempdir().expect("一時ホームを作れる");
        let state_root = StateRoot::parse(home.path().join("state")).expect("絶対パスになる");
        let store = FsRunStore::new(state_root.clone());
        Self {
            _home: home,
            state_root,
            store,
        }
    }
}

/// 種別に対応するファイルの位置(契約の語彙で決まる)。
fn file_of(run_dir: &RunDirPath, kind: RunFileKind) -> std::path::PathBuf {
    match kind {
        RunFileKind::Pid => run_dir.pid_file(),
        RunFileKind::StartTime => run_dir.starttime_file(),
        RunFileKind::Exit => run_dir.exit_file(),
    }
}

impl RunStoreHarness for FsRunStoreHarness {
    type Store = FsRunStore;

    fn store(&self) -> &Self::Store {
        &self.store
    }

    fn expected_run_dir(&self, id: &TaskId, number: AttemptNumber) -> Option<RunDirPath> {
        Some(RunDirPath::derive(&self.state_root, id, number))
    }

    fn attempt_dir_present(&self, run_dir: &RunDirPath) -> Option<bool> {
        Some(run_dir.as_path().is_dir())
    }

    fn put_unreadable_content(&self, run_dir: &RunDirPath, kind: RunFileKind) -> Option<()> {
        ensure_dir(run_dir.as_path()).ok()?;
        fs::write(file_of(run_dir, kind), b"{ not json").ok()
    }

    fn make_unreadable(&self, run_dir: &RunDirPath, kind: RunFileKind) -> Option<Restore> {
        deny_file_read(&file_of(run_dir, kind))
    }

    fn make_attempt_unwritable(&self, run_dir: &RunDirPath) -> Option<Restore> {
        deny_dir_write(run_dir.as_path())
    }

    fn concurrent_store(&self) -> Option<&(dyn RunStore + Sync)> {
        Some(&self.store)
    }
}

/// ファイルを読み取れない状態にする。
///
/// 制限が実際に効いたことを確認してから `Some` を返す(ADR-027)。
#[cfg(unix)]
fn deny_file_read(path: &Path) -> Option<Restore> {
    let restore = set_mode(path, 0o000)?;
    if fs::read(path).is_ok() {
        return None;
    }
    Some(restore)
}

/// ディレクトリへ書き込めない状態にする。読み取りは残す。
#[cfg(unix)]
fn deny_dir_write(dir: &Path) -> Option<Restore> {
    let restore = set_mode(dir, 0o555)?;
    let probe = dir.join("probe");
    if fs::write(&probe, b"probe").is_ok() {
        let _ = fs::remove_file(&probe);
        return None;
    }
    Some(restore)
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) -> Option<Restore> {
    use std::os::unix::fs::PermissionsExt;

    let original = fs::metadata(path).ok()?.permissions();
    let mut restricted = original.clone();
    restricted.set_mode(mode);
    fs::set_permissions(path, restricted).ok()?;

    let target = path.to_path_buf();
    Some(Restore::new(move || {
        let _ = fs::set_permissions(&target, original);
    }))
}

#[cfg(not(unix))]
fn deny_file_read(_path: &Path) -> Option<Restore> {
    None
}

#[cfg(not(unix))]
fn deny_dir_write(_dir: &Path) -> Option<Restore> {
    None
}

/// 権限制限が効かない環境でのみスキップされるケース(HOOKS.md の区分 C)。
///
/// 読み取れないファイル・書き込めない attempt ディレクトリを作れない環境 — 権限操作を
/// 持たないプラットフォーム・root 実行・権限を持たないファイルシステム — では走らない。
const PERMISSION_CASES: [&str; 2] = ["tc_port_run_store_007", "tc_port_run_store_017"];

/// この環境でスキップを許容するケース。
fn allowed_skips() -> Vec<&'static str> {
    if pulsen_conformance::permission_restrictions_effective() {
        Vec::new()
    } else {
        PERMISSION_CASES.to_vec()
    }
}

pulsen_conformance::run_store_conformance!(FsRunStoreHarness::new(), allowed_skips());
