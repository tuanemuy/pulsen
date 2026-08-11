//! TaskRepository の適合スイートを `FsTaskRepository` に適用する。
//!
//! 破損のフィクスチャは「有効な JSON でありながらスナップショットとして解釈できない」
//! 形で作る(ADR-025)。スナップショットの中身だけを JSON 構文として壊すことはできない —
//! ファイル全体を1回のパースで読む以上、それはファイル全体の破損になる。

use std::fs;
use std::path::{Path, PathBuf};

use pulsen::adapter::task_repository::FsTaskRepository;
use pulsen::util::fsdir::ensure_dir;
use pulsen_conformance::{Area, Restore, TaskRepositoryHarness};
use pulsen_domain::task::{StateRoot, TaskFilePath, TaskId, TaskRepository};
use serde_json::{Value, json};
use tempfile::TempDir;

/// 一時ディレクトリを状態ディレクトリに見立てたハーネス。
struct FsTaskRepositoryHarness {
    _home: TempDir,
    state_root: StateRoot,
    repo: FsTaskRepository,
}

impl FsTaskRepositoryHarness {
    fn new() -> Self {
        let home = tempfile::tempdir().expect("一時ホームを作れる");
        let state_root = StateRoot::parse(home.path().join("state")).expect("絶対パスになる");
        let repo = FsTaskRepository::new(state_root.clone());
        Self {
            _home: home,
            state_root,
            repo,
        }
    }

    fn path(&self, area: Area, id: &TaskId) -> PathBuf {
        match area {
            Area::Active => TaskFilePath::active(&self.state_root, id),
            Area::Archived => TaskFilePath::archived(&self.state_root, id),
        }
    }

    fn dir(&self, area: Area) -> PathBuf {
        match area {
            Area::Active => TaskFilePath::active_dir(&self.state_root),
            Area::Archived => TaskFilePath::archived_dir(&self.state_root),
        }
    }

    /// タスクファイルを有効な JSON のまま書き換える。
    fn edit(
        &self,
        area: Area,
        id: &TaskId,
        edit: impl FnOnce(&mut serde_json::Map<String, Value>),
    ) -> Option<()> {
        let path = self.path(area, id);
        let mut document: Value = serde_json::from_slice(&fs::read(&path).ok()?).ok()?;
        edit(document.as_object_mut()?);
        fs::write(&path, serde_json::to_vec_pretty(&document).ok()?).ok()
    }
}

impl TaskRepositoryHarness for FsTaskRepositoryHarness {
    type Repo = FsTaskRepository;

    fn repo(&self) -> &Self::Repo {
        &self.repo
    }

    fn corrupt_whole_record(&self, area: Area, id: &TaskId) -> Option<()> {
        let path = self.path(area, id);
        path.is_file().then_some(())?;
        fs::write(&path, b"{ \xe5\xa3\x8a\xe3\x82\x8c\xe3\x81\x9f JSON").ok()
    }

    fn break_task_field(&self, area: Area, id: &TaskId) -> Option<()> {
        self.edit(area, id, |document| {
            document.insert("task_id".to_owned(), json!("文字集合の外の値 (UPPER CASE)"));
        })
    }

    fn corrupt_snapshot(&self, area: Area, id: &TaskId) -> Option<()> {
        self.edit(area, id, |document| {
            document.insert(
                "snapshot".to_owned(),
                json!("スナップショットとして解釈できない値"),
            );
        })
    }

    fn drop_snapshot_field(&self, area: Area, id: &TaskId) -> Option<()> {
        self.edit(area, id, |document| {
            document.remove("snapshot");
        })
    }

    fn set_task_status_outside_snapshot(
        &self,
        area: Area,
        id: &TaskId,
        status: &str,
    ) -> Option<()> {
        let status = status.to_owned();
        self.edit(area, id, |document| {
            document.insert("task_status".to_owned(), json!(status));
        })
    }

    fn break_snapshot_invariant(&self, area: Area, id: &TaskId) -> Option<()> {
        self.edit(area, id, |document| {
            if let Some(snapshot) = document.get_mut("snapshot").and_then(Value::as_object_mut) {
                snapshot.insert("initial".to_owned(), json!("statuses に無いステータス"));
            }
        })
    }

    fn place_in_both_areas(&self, id: &TaskId) -> Option<()> {
        let archived = self.path(Area::Archived, id);
        ensure_dir(archived.parent()?).ok()?;
        fs::copy(self.path(Area::Active, id), archived).ok()?;
        Some(())
    }

    fn put_unnamed_entry(&self, area: Area) -> Option<()> {
        let dir = self.dir(area);
        ensure_dir(&dir).ok()?;
        for name in [".tmpA1b2C3", "notes.txt", "task-a.json.tmp", "task-a"] {
            fs::write(dir.join(name), b"{}").ok()?;
        }
        Some(())
    }

    fn record_bytes(&self, area: Area, id: &TaskId) -> Option<Vec<u8>> {
        fs::read(self.path(area, id)).ok()
    }

    fn snapshot_bytes(&self, area: Area, id: &TaskId) -> Option<Vec<u8>> {
        let document: Value = serde_json::from_slice(&fs::read(self.path(area, id)).ok()?).ok()?;
        serde_json::to_vec(document.get("snapshot")?).ok()
    }

    fn make_unreadable(&self, area: Area) -> Option<Restore> {
        deny_dir_read(&self.dir(area))
    }

    fn make_unwritable(&self, area: Area) -> Option<Restore> {
        deny_dir_write(&self.dir(area))
    }

    fn concurrent_repo(&self) -> Option<&(dyn TaskRepository + Sync)> {
        Some(&self.repo)
    }
}

/// ディレクトリを読み取れない状態にする。
///
/// 制限が実際に効いたことを確認してから `Some` を返す(ADR-027)。root 実行や権限を
/// 持たないファイルシステムでは `chmod` が効かず、確認を省くと `Err(Io)` を期待する
/// ケースがスキップに落ちずに失敗する。
#[cfg(unix)]
fn deny_dir_read(dir: &Path) -> Option<Restore> {
    let restore = set_mode(dir, 0o000)?;
    if fs::read_dir(dir).is_ok() {
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
fn set_mode(dir: &Path, mode: u32) -> Option<Restore> {
    use std::os::unix::fs::PermissionsExt;

    ensure_dir(dir).ok()?;
    let original = fs::metadata(dir).ok()?.permissions();
    let mut restricted = original.clone();
    restricted.set_mode(mode);
    fs::set_permissions(dir, restricted).ok()?;

    let target = dir.to_path_buf();
    Some(Restore::new(move || {
        let _ = fs::set_permissions(&target, original);
    }))
}

#[cfg(not(unix))]
fn deny_dir_read(_dir: &Path) -> Option<Restore> {
    None
}

#[cfg(not(unix))]
fn deny_dir_write(_dir: &Path) -> Option<Restore> {
    None
}

/// この環境で許容するスキップ件数。
///
/// 権限操作を持たないプラットフォームでは、読み取り不能・書き込み不能を前提とする
/// TC-port-task-repository-005/011/012/019/035/041 が走らない。root 実行のように
/// `chmod` が効かない環境も同じ状況になるが、そこは宣言と食い違うことを失敗として見せる。
#[cfg(unix)]
const ALLOWED_SKIPS: usize = 0;
#[cfg(not(unix))]
const ALLOWED_SKIPS: usize = 6;

pulsen_conformance::task_repository_conformance!(FsTaskRepositoryHarness::new(), ALLOWED_SKIPS);
