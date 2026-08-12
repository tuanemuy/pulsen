//! WorktreeManager の適合スイート(対象の検証を行う3メソッドと `create`)を
//! `GitCliWorktreeManager` に適用する。

mod common;

use std::fs;
use std::path::{Path, PathBuf};

use pulsen::adapter::worktree::GitCliWorktreeManager;
use pulsen_conformance::WorktreeManagerHarness;
use pulsen_domain::task::{BranchName, RepoPath, Workspace, WorktreePath};
use tempfile::TempDir;

/// フィクスチャの既定ブランチ名(`git init -b main`)。
const HEAD_BRANCH: &str = "main";
/// どのフィクスチャにも作らないブランチ名。
const ABSENT_BRANCH: &str = "no-such-branch";
/// worktree の内容を観測するためのファイル名。
const MARKER_FILE: &str = "marker.txt";

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

    /// コミットのあるリポジトリを1つだけ用意する。
    ///
    /// 呼ぶたびにコミットを積むと、`create` のケースが比べるブランチ先端が動く。
    fn repo_dir(&self) -> Option<PathBuf> {
        let dir = self.dir("with-commit");
        if !dir.join(".git").exists() {
            common::git::init_repo(&dir)?;
            common::git::commit(&dir, "README.md")?;
        }
        Some(dir)
    }

    /// worktree の置き場。**シンボリックリンク経由**のパスとして組む。
    ///
    /// 同定の鍵は物理パスなので(ADR-013)、置き場が実体そのものだと正規化の分岐が
    /// どのケースからも実行されない。リンクを作れない環境では実体へ落とす。
    fn worktree_root(&self) -> PathBuf {
        let real = self.dir("worktrees-real");
        let link = self.dir("worktrees");
        if fs::create_dir_all(&real).is_err() {
            return real;
        }
        if !link.exists() {
            symlink_dir(&real, &link);
        }
        if link.is_dir() { link } else { real }
    }

    /// 置き場の下にタスク1件分のワークスペースを組む。
    fn workspace_in(&self, root: &Path, name: &str) -> Option<Workspace> {
        let path = WorktreePath::parse(root.join(name)).ok()?;
        let branch = BranchName::parse(format!("pulsen/{name}")).ok()?;
        Some(Workspace::new(path, branch))
    }

    fn workspace(&self, name: &str) -> Option<Workspace> {
        self.workspace_in(&self.worktree_root(), name)
    }

    fn marker_path(&self, workspace: &Workspace) -> PathBuf {
        workspace.path().as_path().join(MARKER_FILE)
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

#[cfg(unix)]
fn symlink_dir(original: &Path, link: &Path) {
    let _ = std::os::unix::fs::symlink(original, link);
}

#[cfg(windows)]
fn symlink_dir(original: &Path, link: &Path) {
    let _ = std::os::windows::fs::symlink_dir(original, link);
}

#[cfg(not(any(unix, windows)))]
fn symlink_dir(_original: &Path, _link: &Path) {}

impl WorktreeManagerHarness for GitCliWorktreeManagerHarness {
    type Manager = GitCliWorktreeManager;

    fn manager(&self) -> &Self::Manager {
        &self.manager
    }

    fn repo_with_commit(&self) -> Option<RepoPath> {
        repo_path(&self.repo_dir()?)
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

    fn unused_workspace(&self) -> Option<Workspace> {
        self.workspace("t-new")
    }

    fn workspace_under_missing_root(&self) -> Option<Workspace> {
        self.workspace_in(&self.dir("worktrees-missing"), "t-first")
    }

    fn workspace_with_orphan_branch(&self) -> Option<(Workspace, String)> {
        let repo = self.repo_dir()?;
        let workspace = self.workspace("t-orphan")?;
        // 別の場所で作業してコミットを積み、登録だけを片付ける。ブランチと先端は残る。
        let scratch = self.dir("scratch-orphan");
        common::git::add_worktree_with_branch(
            &repo,
            &scratch,
            workspace.branch().as_str(),
            HEAD_BRANCH,
        )?;
        common::git::commit_file(&scratch, MARKER_FILE, "積まれた成果物")?;
        common::git::remove_worktree(&repo, &scratch)?;
        (common::git::worktree_registration(&repo, workspace.path().as_path()).is_none())
            .then_some(())?;
        Some((workspace, "積まれた成果物".to_owned()))
    }

    fn workspace_with_prunable_registration(&self) -> Option<(Workspace, String)> {
        let repo = self.repo_dir()?;
        let workspace = self.workspace("t-prunable")?;
        common::git::add_worktree_with_branch(
            &repo,
            workspace.path().as_path(),
            workspace.branch().as_str(),
            HEAD_BRANCH,
        )?;
        common::git::commit_file(workspace.path().as_path(), MARKER_FILE, "消える前の成果物")?;
        // 実体だけを消すと登録が残り、git は `prunable` として列挙する。
        fs::remove_dir_all(workspace.path().as_path()).ok()?;
        let registration = common::git::worktree_registration(&repo, workspace.path().as_path())?;
        registration.prunable.then_some(())?;
        Some((workspace, "消える前の成果物".to_owned()))
    }

    fn workspace_over_plain_dir(&self) -> Option<(Workspace, String)> {
        let workspace = self.workspace("t-plain")?;
        fs::create_dir_all(workspace.path().as_path()).ok()?;
        fs::write(self.marker_path(&workspace), "利用者が置いたもの").ok()?;
        Some((workspace, "利用者が置いたもの".to_owned()))
    }

    fn workspace_over_other_branch(&self) -> Option<(Workspace, String)> {
        let repo = self.repo_dir()?;
        let workspace = self.workspace("t-other")?;
        common::git::add_worktree_with_branch(
            &repo,
            workspace.path().as_path(),
            "occupant",
            HEAD_BRANCH,
        )?;
        common::git::commit_file(workspace.path().as_path(), MARKER_FILE, "別タスクの成果物")?;
        Some((workspace, "別タスクの成果物".to_owned()))
    }

    fn put_worktree_marker(&self, workspace: &Workspace, text: &str) -> Option<()> {
        fs::write(self.marker_path(workspace), text).ok()
    }

    fn worktree_marker(&self, workspace: &Workspace) -> Option<String> {
        Some(fs::read_to_string(self.marker_path(workspace)).unwrap_or_default())
    }

    fn worktree_present(&self, workspace: &Workspace) -> Option<bool> {
        Some(workspace.path().as_path().is_dir())
    }

    fn branch_tip(&self, name: &BranchName) -> Option<String> {
        common::git::branch_tip(&self.repo_dir()?, name.as_str())
    }
}

/// 一時ディレクトリ自体が git リポジトリ配下にある環境でのみスキップされるケース。
///
/// `non_repo_dir` は「git リポジトリでない実在のディレクトリ」を前提にするため、TMPDIR が
/// リポジトリ配下だと上位へ遡って成功し、前提が成立しない(ADR-033)。
const OUTSIDE_REPOSITORY_CASES: [&str; 1] = ["tc_port_worktree_manager_003"];

/// この環境でスキップを許容するケース。
///
/// git 操作の失敗は別ハンドル(`failing_manager`)で組めるため、許容するのは環境が前提を
/// 作れない上記1件だけ。宣言をプラットフォームではなく実行時の述語で決めることで、同じ
/// 前提を使う CLI 側の受け入れテスト(TC-task-register-task-036)と扱いが揃う(ADR-055)。
fn allowed_skips() -> Vec<&'static str> {
    if common::git::tmpdir_outside_repository() {
        Vec::new()
    } else {
        OUTSIDE_REPOSITORY_CASES.to_vec()
    }
}

pulsen_conformance::worktree_manager_conformance!(
    GitCliWorktreeManagerHarness::new(),
    allowed_skips()
);
