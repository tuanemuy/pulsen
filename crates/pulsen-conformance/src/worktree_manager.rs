//! WorktreeManager の適合ケース(`spec/testcases/ports/worktree-manager.md` のうち、
//! 対象の検証を行う3メソッド分の9行)。
//!
//! `create` / `remove` の12行は、それらをポートに足すスライスで扱う。

use pulsen_domain::execution::{TargetError, WorktreeManager};

use crate::{CaseOutcome, WorktreeManagerHarness, require};

pub fn tc_port_worktree_manager_001_コミットのあるリポジトリは検証を通る(
    harness: &impl WorktreeManagerHarness,
) -> CaseOutcome {
    let repo = require!(harness.repo_with_commit());

    assert_eq!(harness.manager().validate_repo(&repo), Ok(()));
    CaseOutcome::Ran
}

pub fn tc_port_worktree_manager_002_存在しないパスは見つからない(
    harness: &impl WorktreeManagerHarness,
) -> CaseOutcome {
    let repo = require!(harness.missing_path());

    assert_eq!(
        harness.manager().validate_repo(&repo),
        Err(TargetError::NotFound)
    );
    CaseOutcome::Ran
}

pub fn tc_port_worktree_manager_003_リポジトリでないディレクトリは拒否される(
    harness: &impl WorktreeManagerHarness,
) -> CaseOutcome {
    let repo = require!(harness.non_repo_dir());

    assert_eq!(
        harness.manager().validate_repo(&repo),
        Err(TargetError::NotARepository)
    );
    CaseOutcome::Ran
}

pub fn tc_port_worktree_manager_004_headがブランチを指せばそのブランチ名が返る(
    harness: &impl WorktreeManagerHarness,
) -> CaseOutcome {
    let repo = require!(harness.repo_with_commit());
    let expected = require!(harness.head_branch_name());

    assert_eq!(harness.manager().head_branch(&repo), Ok(expected));
    CaseOutcome::Ran
}

pub fn tc_port_worktree_manager_005_detached_headはブランチ名を持たない(
    harness: &impl WorktreeManagerHarness,
) -> CaseOutcome {
    let repo = require!(harness.detached_repo());

    assert_eq!(
        harness.manager().head_branch(&repo),
        Err(TargetError::DetachedHead)
    );
    CaseOutcome::Ran
}

pub fn tc_port_worktree_manager_006_コミットのない空リポジトリは空として返る(
    harness: &impl WorktreeManagerHarness,
) -> CaseOutcome {
    let repo = require!(harness.repo_without_commit());

    assert_eq!(
        harness.manager().head_branch(&repo),
        Err(TargetError::EmptyRepository)
    );
    CaseOutcome::Ran
}

pub fn tc_port_worktree_manager_007_存在するブランチは真になる(
    harness: &impl WorktreeManagerHarness,
) -> CaseOutcome {
    let repo = require!(harness.repo_with_commit());
    let branch = require!(harness.head_branch_name());

    assert_eq!(harness.manager().branch_exists(&repo, &branch), Ok(true));
    CaseOutcome::Ran
}

pub fn tc_port_worktree_manager_008_存在しないブランチは偽になる(
    harness: &impl WorktreeManagerHarness,
) -> CaseOutcome {
    let repo = require!(harness.repo_with_commit());
    let branch = require!(harness.absent_branch_name());

    assert_eq!(harness.manager().branch_exists(&repo, &branch), Ok(false));
    CaseOutcome::Ran
}

pub fn tc_port_worktree_manager_009_操作自体の失敗は分類と区別して返る(
    harness: &impl WorktreeManagerHarness,
) -> CaseOutcome {
    let failing = require!(harness.failing_manager());
    let repo = require!(harness.repo_with_commit());
    let branch = require!(harness.head_branch_name());

    assert_failed(failing.validate_repo(&repo).err(), "validate_repo");
    assert_failed(failing.head_branch(&repo).err(), "head_branch");
    assert_failed(failing.branch_exists(&repo, &branch).err(), "branch_exists");
    CaseOutcome::Ran
}

/// 実行環境のエラーであり、対象の分類のいずれでもないことを確かめる。
fn assert_failed(error: Option<TargetError>, method: &str) {
    let Some(error) = error else {
        panic!("{method} が失敗しなかった");
    };
    match error {
        TargetError::Failed { message } => {
            assert!(!message.is_empty(), "{method}: 原因が説明される");
        }
        TargetError::NotFound
        | TargetError::NotARepository
        | TargetError::DetachedHead
        | TargetError::EmptyRepository => {
            panic!("{method} が実行環境のエラーではなく対象の分類を返した")
        }
    }
}

/// WorktreeManager の適合スイート(対象の検証を行う3メソッド分)をアダプターに適用する。
///
/// `$setup` はケースごとに評価され、ハーネスは共有されない。`$allowed_skips` は
/// この環境でスキップを許容するケース(TC ID)の集合で、集合の外のスキップはその
/// ケースの失敗になる。
#[macro_export]
macro_rules! worktree_manager_conformance {
    ($setup:expr, $allowed_skips:expr) => {
        use $crate::worktree_manager as __pulsen_conformance_worktree_manager;

        $crate::conformance_cases!(
            __pulsen_conformance_worktree_manager,
            $setup,
            __PULSEN_CONFORMANCE_WORKTREE_MANAGER_SKIPS = $allowed_skips,
            [
                tc_port_worktree_manager_001_コミットのあるリポジトリは検証を通る,
                tc_port_worktree_manager_002_存在しないパスは見つからない,
                tc_port_worktree_manager_003_リポジトリでないディレクトリは拒否される,
                tc_port_worktree_manager_004_headがブランチを指せばそのブランチ名が返る,
                tc_port_worktree_manager_005_detached_headはブランチ名を持たない,
                tc_port_worktree_manager_006_コミットのない空リポジトリは空として返る,
                tc_port_worktree_manager_007_存在するブランチは真になる,
                tc_port_worktree_manager_008_存在しないブランチは偽になる,
                tc_port_worktree_manager_009_操作自体の失敗は分類と区別して返る,
            ]
        );
    };
}
