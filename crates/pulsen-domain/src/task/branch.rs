//! ブランチ名と、パス・ブランチの組(ワークスペース / 対象)。

use super::path::{RepoPath, WorktreePath};

/// ブランチ名の生成失敗。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BranchNameError {
    /// 空文字列。
    Empty,
    /// 空白または制御文字を含む。
    ContainsWhitespaceOrControl {
        /// 見つかった文字。
        char: char,
        /// 0 始まりの位置。
        position: usize,
    },
    /// 先頭が `-`(オプションと紛れる)。
    LeadingHyphen,
    /// `..` を含む。
    ContainsDotDot,
    /// `/` で始まる。
    LeadingSlash,
    /// `/` で終わる。
    TrailingSlash,
    /// `.lock` で終わる。
    LockSuffix,
}

/// git 参照名として有効な実用サブセットのブランチ名。
///
/// git が許す全体ではなく、ツールが安全に扱える範囲に狭める。git 側で有効でも
/// ここで弾かれる名前は、対象の分類ではなく実行環境の制約として扱う。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BranchName(String);

impl BranchName {
    /// `.lock` 終端の禁止に使う接尾辞。
    const LOCK_SUFFIX: &'static str = ".lock";

    /// 実用サブセットの制約を満たす文字列としてのみ生成する。
    pub fn parse(s: String) -> Result<Self, BranchNameError> {
        if s.is_empty() {
            return Err(BranchNameError::Empty);
        }
        for (position, found) in s.chars().enumerate() {
            if found.is_whitespace() || found.is_control() {
                return Err(BranchNameError::ContainsWhitespaceOrControl {
                    char: found,
                    position,
                });
            }
        }
        if s.starts_with('-') {
            return Err(BranchNameError::LeadingHyphen);
        }
        if s.contains("..") {
            return Err(BranchNameError::ContainsDotDot);
        }
        if s.starts_with('/') {
            return Err(BranchNameError::LeadingSlash);
        }
        if s.ends_with('/') {
            return Err(BranchNameError::TrailingSlash);
        }
        if s.ends_with(Self::LOCK_SUFFIX) {
            return Err(BranchNameError::LockSuffix);
        }
        Ok(Self(s))
    }

    /// 保持する文字列を借用する。
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// 保持する文字列を取り出す。
    pub fn into_string(self) -> String {
        self.0
    }
}

/// タスクに割り当てられた worktree のパスとブランチ。
///
/// 最初の実行時に確定し、以降不変。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Workspace {
    path: WorktreePath,
    branch: BranchName,
}

impl Workspace {
    /// パスとブランチの組を作る。
    pub fn new(path: WorktreePath, branch: BranchName) -> Self {
        Self { path, branch }
    }

    /// worktree のパス。
    pub fn path(&self) -> &WorktreePath {
        &self.path
    }

    /// worktree のブランチ。
    pub fn branch(&self) -> &BranchName {
        &self.branch
    }
}

/// タスクが作用するリポジトリとベースブランチ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    repo: RepoPath,
    base_branch: BranchName,
}

impl Target {
    /// リポジトリとベースブランチの組を作る。
    pub fn new(repo: RepoPath, base_branch: BranchName) -> Self {
        Self { repo, base_branch }
    }

    /// リポジトリのパス。
    pub fn repo(&self) -> &RepoPath {
        &self.repo
    }

    /// ベースブランチ。
    pub fn base_branch(&self) -> &BranchName {
        &self.base_branch
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::task::id::TaskId;

    fn branch(name: &str) -> BranchName {
        BranchName::parse(name.to_owned()).expect("受理される")
    }

    #[test]
    fn 実用サブセットのブランチ名は受理される() {
        for name in [
            "main",
            "feature/x",
            "pulsen/20260811t091530-k3f9qa1b",
            "v1.0",
        ] {
            assert!(BranchName::parse(name.to_owned()).is_ok(), "{name}");
        }
    }

    #[test]
    fn 空のブランチ名は受理されない() {
        assert_eq!(
            BranchName::parse(String::new()),
            Err(BranchNameError::Empty)
        );
    }

    #[test]
    fn 空白と制御文字を含むブランチ名は受理されない() {
        assert_eq!(
            BranchName::parse("feature x".to_owned()),
            Err(BranchNameError::ContainsWhitespaceOrControl {
                char: ' ',
                position: 7
            })
        );
        assert_eq!(
            BranchName::parse("feature\tx".to_owned()),
            Err(BranchNameError::ContainsWhitespaceOrControl {
                char: '\t',
                position: 7
            })
        );
        assert_eq!(
            BranchName::parse("feature\nx".to_owned()),
            Err(BranchNameError::ContainsWhitespaceOrControl {
                char: '\n',
                position: 7
            })
        );
    }

    #[test]
    fn git参照名として危険な形のブランチ名は受理されない() {
        assert_eq!(
            BranchName::parse("-main".to_owned()),
            Err(BranchNameError::LeadingHyphen)
        );
        assert_eq!(
            BranchName::parse("a..b".to_owned()),
            Err(BranchNameError::ContainsDotDot)
        );
        assert_eq!(
            BranchName::parse("/main".to_owned()),
            Err(BranchNameError::LeadingSlash)
        );
        assert_eq!(
            BranchName::parse("main/".to_owned()),
            Err(BranchNameError::TrailingSlash)
        );
        assert_eq!(
            BranchName::parse("main.lock".to_owned()),
            Err(BranchNameError::LockSuffix)
        );
    }

    #[test]
    fn タスクidから導出したブランチ名は常に受理される() {
        for raw in [
            "a",
            "20260811t091530-k3f9qa1b",
            "a--b",
            "abc-",
            "0123456789",
            &"a".repeat(TaskId::MAX_LENGTH),
        ] {
            let id = TaskId::parse(raw.to_owned()).expect("受理される");
            let derived = format!("pulsen/{}", id.as_str());
            assert!(BranchName::parse(derived.clone()).is_ok(), "{derived}");
        }
    }

    #[test]
    fn ワークスペースは両フィールドの一致で等価になる() {
        let path = WorktreePath::parse(PathBuf::from(if std::path::MAIN_SEPARATOR == '\\' {
            "C:\\worktrees\\t1"
        } else {
            "/worktrees/t1"
        }))
        .expect("受理される");
        let workspace = Workspace::new(path.clone(), branch("pulsen/t1"));
        assert_eq!(workspace, Workspace::new(path.clone(), branch("pulsen/t1")));
        assert_ne!(workspace, Workspace::new(path, branch("pulsen/t2")));
        assert_eq!(workspace.branch(), &branch("pulsen/t1"));
    }

    #[test]
    fn 対象はリポジトリとベースブランチを保持する() {
        let repo = RepoPath::parse(PathBuf::from(if std::path::MAIN_SEPARATOR == '\\' {
            "C:\\repos\\pulsen"
        } else {
            "/repos/pulsen"
        }))
        .expect("受理される");
        let target = Target::new(repo.clone(), branch("main"));
        assert_eq!(target.repo(), &repo);
        assert_eq!(target.base_branch(), &branch("main"));
    }
}
