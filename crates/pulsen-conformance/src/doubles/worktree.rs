//! 検証結果を与える `WorktreeManager` のダブル。

use std::cell::RefCell;
use std::collections::VecDeque;

use pulsen_domain::execution::{TargetError, WorktreeManager};
use pulsen_domain::task::{BranchName, RepoPath};

/// 記録された呼び出し。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorktreeManagerCall {
    /// `validate_repo`。
    ValidateRepo {
        /// 検証したリポジトリ。
        repo: RepoPath,
    },
    /// `head_branch`。
    HeadBranch {
        /// 問い合わせたリポジトリ。
        repo: RepoPath,
    },
    /// `branch_exists`。
    BranchExists {
        /// 問い合わせたリポジトリ。
        repo: RepoPath,
        /// 問い合わせたブランチ。
        branch: BranchName,
    },
}

/// メソッドごとにあらかじめ与えた結果を順に返すマネージャー。
///
/// `TargetError` の5分岐すべてを外から与えられる。実アダプター(git CLI)では
/// `Failed` の再現に git を起動できない状況が要る(ADR-024)ため、ユースケースの
/// 分岐網羅はここで行う。
#[derive(Debug, Default)]
pub struct ScriptedWorktreeManager {
    validate_repo: RefCell<VecDeque<Result<(), TargetError>>>,
    head_branch: RefCell<VecDeque<Result<BranchName, TargetError>>>,
    branch_exists: RefCell<VecDeque<Result<bool, TargetError>>>,
    calls: RefCell<Vec<WorktreeManagerCall>>,
}

impl ScriptedWorktreeManager {
    /// どのメソッドの台本も持たないマネージャーを作る。
    pub fn new() -> Self {
        Self::default()
    }

    /// `validate_repo` が返す結果の列を与える。
    pub fn with_validate_repo(
        self,
        results: impl IntoIterator<Item = Result<(), TargetError>>,
    ) -> Self {
        *self.validate_repo.borrow_mut() = results.into_iter().collect();
        self
    }

    /// `head_branch` が返す結果の列を与える。
    pub fn with_head_branch(
        self,
        results: impl IntoIterator<Item = Result<BranchName, TargetError>>,
    ) -> Self {
        *self.head_branch.borrow_mut() = results.into_iter().collect();
        self
    }

    /// `branch_exists` が返す結果の列を与える。
    pub fn with_branch_exists(
        self,
        results: impl IntoIterator<Item = Result<bool, TargetError>>,
    ) -> Self {
        *self.branch_exists.borrow_mut() = results.into_iter().collect();
        self
    }

    /// これまでに受け取った呼び出し。
    pub fn calls(&self) -> Vec<WorktreeManagerCall> {
        self.calls.borrow().clone()
    }
}

impl WorktreeManager for ScriptedWorktreeManager {
    fn validate_repo(&self, repo: &RepoPath) -> Result<(), TargetError> {
        self.calls
            .borrow_mut()
            .push(WorktreeManagerCall::ValidateRepo { repo: repo.clone() });
        let Some(result) = self.validate_repo.borrow_mut().pop_front() else {
            panic!("validate_repo の結果を使い切った")
        };
        result
    }

    fn head_branch(&self, repo: &RepoPath) -> Result<BranchName, TargetError> {
        self.calls
            .borrow_mut()
            .push(WorktreeManagerCall::HeadBranch { repo: repo.clone() });
        let Some(result) = self.head_branch.borrow_mut().pop_front() else {
            panic!("head_branch の結果を使い切った")
        };
        result
    }

    fn branch_exists(&self, repo: &RepoPath, branch: &BranchName) -> Result<bool, TargetError> {
        self.calls
            .borrow_mut()
            .push(WorktreeManagerCall::BranchExists {
                repo: repo.clone(),
                branch: branch.clone(),
            });
        let Some(result) = self.branch_exists.borrow_mut().pop_front() else {
            panic!("branch_exists の結果を使い切った")
        };
        result
    }
}
