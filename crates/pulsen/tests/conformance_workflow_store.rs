//! WorkflowStore の適合スイートを `FsWorkflowStore` に適用する。
//!
//! 相対パス解決のケースは、カレントディレクトリを書き換えずに基準ディレクトリの注入で
//! 組み立てる(ADR-030)。テストが並列に走ってもプロセス全体の状態を壊さない。

mod common;

use std::fs;
use std::path::{Path, PathBuf};

use pulsen::adapter::workflow_store::FsWorkflowStore;
use pulsen::util::fsdir::ensure_dir;
use pulsen_conformance::{Restore, WorkflowStoreHarness};
use tempfile::TempDir;

/// 一時ディレクトリをグローバルホームと基準ディレクトリに見立てたハーネス。
struct FsWorkflowStoreHarness {
    home: TempDir,
    base: TempDir,
    store: FsWorkflowStore,
}

impl FsWorkflowStoreHarness {
    fn new() -> Self {
        let home = tempfile::tempdir().expect("一時ホームを作れる");
        let base = tempfile::tempdir().expect("一時の基準ディレクトリを作れる");
        let store = FsWorkflowStore::new(home.path().join("workflows"), base.path().to_path_buf());
        Self { home, base, store }
    }

    fn workflows_dir(&self) -> PathBuf {
        self.home.path().join("workflows")
    }
}

fn write(path: &Path, text: &str) -> Option<()> {
    ensure_dir(path.parent()?).ok()?;
    fs::write(path, text).ok()?;
    Some(())
}

impl WorkflowStoreHarness for FsWorkflowStoreHarness {
    type Store = FsWorkflowStore;

    fn store(&self) -> &Self::Store {
        &self.store
    }

    fn put_named(&self, name: &str, text: &str) -> Option<()> {
        write(&self.workflows_dir().join(format!("{name}.yaml")), text)
    }

    fn put_named_with_ext(&self, name: &str, extension: &str, text: &str) -> Option<()> {
        write(
            &self.workflows_dir().join(format!("{name}.{extension}")),
            text,
        )
    }

    fn expected_path_for_name(&self, name: &str) -> Option<PathBuf> {
        Some(self.workflows_dir().join(format!("{name}.yaml")))
    }

    fn put_at_absolute(&self, text: &str) -> Option<PathBuf> {
        let path = self.home.path().join("elsewhere").join("definition.yaml");
        write(&path, text)?;
        Some(path)
    }

    fn put_at_relative(&self, text: &str) -> Option<(PathBuf, PathBuf)> {
        let relative = PathBuf::from("nested").join("definition.yaml");
        let absolute = self.base.path().join(&relative);
        write(&absolute, text)?;
        Some((relative, absolute))
    }

    fn missing_absolute_path(&self) -> Option<PathBuf> {
        Some(self.home.path().join("elsewhere").join("absent.yaml"))
    }

    fn make_unreadable(&self, name: &str) -> Option<Restore> {
        common::deny_read(&self.workflows_dir().join(format!("{name}.yaml")))
    }
}

pulsen_conformance::workflow_store_conformance!(FsWorkflowStoreHarness::new());
