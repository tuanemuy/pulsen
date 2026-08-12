//! ConfigStore の適合スイートを `FsConfigStore` に適用する。

mod common;

use std::fs;
use std::io;
use std::path::PathBuf;

use pulsen::adapter::config_store::FsConfigStore;
use pulsen::util::fsdir::ensure_dir;
use pulsen_conformance::{ConfigStoreHarness, Restore};
use tempfile::TempDir;

/// 一時ディレクトリをグローバルホームに見立てたハーネス。
struct FsConfigStoreHarness {
    home: TempDir,
    store: FsConfigStore,
}

impl FsConfigStoreHarness {
    fn new() -> Self {
        let home = tempfile::tempdir().expect("一時ホームを作れる");
        let store = FsConfigStore::new(home.path().join("config.yaml"), home.path().to_path_buf());
        Self { home, store }
    }

    fn config_path(&self) -> PathBuf {
        self.home.path().join("config.yaml")
    }
}

impl ConfigStoreHarness for FsConfigStoreHarness {
    type Store = FsConfigStore;

    fn store(&self) -> &Self::Store {
        &self.store
    }

    fn put_config(&self, text: &str) -> Option<()> {
        ensure_dir(self.home.path()).ok()?;
        fs::write(self.config_path(), text).ok()?;
        Some(())
    }

    fn remove_config(&self) -> Option<()> {
        match fs::remove_file(self.config_path()) {
            Ok(()) => Some(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Some(()),
            Err(_) => None,
        }
    }

    fn home_path(&self) -> Option<PathBuf> {
        Some(self.home.path().to_path_buf())
    }

    fn make_unreadable(&self) -> Option<Restore> {
        common::deny_read(&self.config_path())
    }
}

/// 権限制限が効かない環境でのみスキップされるケース(HOOKS.md の区分 C)。
///
/// 「読み取れない config.yaml」を作れない環境 — 権限操作を持たないプラットフォーム・
/// root 実行・権限を持たないファイルシステム — では TC-port-config-store-023 が走らない。
const PERMISSION_CASES: [&str; 1] = ["tc_port_config_store_023"];

/// この環境でスキップを許容するケース。
fn allowed_skips() -> Vec<&'static str> {
    if pulsen_conformance::permission_restrictions_effective() {
        Vec::new()
    } else {
        PERMISSION_CASES.to_vec()
    }
}

pulsen_conformance::config_store_conformance!(FsConfigStoreHarness::new(), allowed_skips());
