//! WorktreeManager の適合スイート(対象の検証を行う3メソッド分)を
//! `GitCliWorktreeManager` に適用する。

mod common;

use std::fs;
use std::path::{Path, PathBuf};

use pulsen::adapter::worktree::GitCliWorktreeManager;
use pulsen_conformance::WorktreeManagerHarness;
use pulsen_domain::task::{BranchName, RepoPath};
use tempfile::TempDir;

/// フィクスチャの既定ブランチ名(`git init -b main`)。
const HEAD_BRANCH: &str = "main";
/// どのフィクスチャにも作らないブランチ名。
const ABSENT_BRANCH: &str = "no-such-branch";

/// 一時ディレクトリに git リポジトリを用意するハーネス。
struct GitCliWorktreeManagerHarness {
    root: TempDir,
    manager: GitCliWorktreeManager,
    failing: GitCliWorktreeManager,
}

impl GitCliWorktreeManagerHarness {
    fn new() -> Self {
        let root = tempfile::tempdir().expect("一時ディレクトリを作れる");
        // 存在しないパスを git 実行ファイルとして渡すと、3メソッドとも起動失敗で
        // `Failed` に落ちる(ADR-024)。本番のインスタンスは触らない。
        let failing = GitCliWorktreeManager::new(root.path().join("no-such-git"));
        Self {
            root,
            manager: GitCliWorktreeManager::new(PathBuf::from("git")),
            failing,
        }
    }

    fn dir(&self, name: &str) -> PathBuf {
        self.root.path().join(name)
    }
}

/// 絶対パスとして受理させる。
fn repo_path(dir: &Path) -> Option<RepoPath> {
    RepoPath::parse(dir.to_path_buf()).ok()
}

/// ブランチ名として受理させる。
fn branch(name: &str) -> Option<BranchName> {
    BranchName::parse(name.to_owned()).ok()
}

impl WorktreeManagerHarness for GitCliWorktreeManagerHarness {
    type Manager = GitCliWorktreeManager;

    fn manager(&self) -> &Self::Manager {
        &self.manager
    }

    fn repo_with_commit(&self) -> Option<RepoPath> {
        let dir = self.dir("with-commit");
        common::git::init_repo(&dir)?;
        common::git::commit(&dir, "README.md")?;
        repo_path(&dir)
    }

    fn repo_without_commit(&self) -> Option<RepoPath> {
        let dir = self.dir("empty");
        common::git::init_repo(&dir)?;
        repo_path(&dir)
    }

    fn detached_repo(&self) -> Option<RepoPath> {
        let dir = self.dir("detached");
        common::git::init_repo(&dir)?;
        common::git::commit(&dir, "README.md")?;
        common::git::detach_head(&dir)?;
        repo_path(&dir)
    }

    fn non_repo_dir(&self) -> Option<RepoPath> {
        let dir = self.dir("plain");
        fs::create_dir_all(&dir).ok()?;
        // TMPDIR 自体がリポジトリ配下だと上位へ遡って成功するため、前提が成立する
        // ことを確かめてから使う(ADR-033)。
        common::git::is_outside_repository(&dir).then_some(())?;
        repo_path(&dir)
    }

    fn missing_path(&self) -> Option<RepoPath> {
        repo_path(&self.dir("missing"))
    }

    fn head_branch_name(&self) -> Option<BranchName> {
        branch(HEAD_BRANCH)
    }

    fn absent_branch_name(&self) -> Option<BranchName> {
        branch(ABSENT_BRANCH)
    }

    fn failing_manager(&self) -> Option<&Self::Manager> {
        Some(&self.failing)
    }
}

// git 操作の失敗も別ハンドルで組めるため、スキップは1件も許容しない。一時ディレクトリ
// がリポジトリ配下にある環境では TC-port-worktree-manager-003 の前提が成立せず
// (ADR-033)、その差はここで失敗として現れる。
pulsen_conformance::worktree_manager_conformance!(GitCliWorktreeManagerHarness::new(), 0);
