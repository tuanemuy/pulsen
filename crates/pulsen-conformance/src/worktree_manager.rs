//! WorktreeManager の適合ケース(`spec/testcases/ports/worktree-manager.md` のうち、
//! 対象の検証を行う3メソッドと `create` の16行)と、ADR-077 由来の追加1件。
//!
//! `remove` の5行は、それをポートに足すスライスで扱う。

use pulsen_domain::execution::{TargetError, WorktreeError, WorktreeManager};

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

pub fn tc_port_worktree_manager_010_未使用のパスとブランチにはbaseから新しいworktreeが作られる(
    harness: &impl WorktreeManagerHarness,
) -> CaseOutcome {
    let repo = require!(harness.repo_with_commit());
    let base = require!(harness.head_branch_name());
    let workspace = require!(harness.unused_workspace());
    let base_tip = require!(harness.branch_tip(&base));

    assert_eq!(harness.manager().create(&repo, &base, &workspace), Ok(()));

    assert!(require!(harness.worktree_present(&workspace)));
    assert_eq!(
        harness.manager().branch_exists(&repo, workspace.branch()),
        Ok(true)
    );
    assert_eq!(
        require!(harness.branch_tip(workspace.branch())),
        base_tip,
        "新しいブランチは base の先端から作られる"
    );
    CaseOutcome::Ran
}

pub fn tc_port_worktree_manager_011_worktreeの置き場が未作成でも用意される(
    harness: &impl WorktreeManagerHarness,
) -> CaseOutcome {
    let repo = require!(harness.repo_with_commit());
    let base = require!(harness.head_branch_name());
    let workspace = require!(harness.workspace_under_missing_root());

    assert_eq!(harness.manager().create(&repo, &base, &workspace), Ok(()));

    assert!(require!(harness.worktree_present(&workspace)));
    CaseOutcome::Ran
}

pub fn tc_port_worktree_manager_012_自タスクの残骸への再作成は内容に触れず成功する(
    harness: &impl WorktreeManagerHarness,
) -> CaseOutcome {
    let repo = require!(harness.repo_with_commit());
    let base = require!(harness.head_branch_name());
    let workspace = require!(harness.unused_workspace());
    assert_eq!(harness.manager().create(&repo, &base, &workspace), Ok(()));
    require!(harness.put_worktree_marker(&workspace, "作業中の変更"));

    assert_eq!(harness.manager().create(&repo, &base, &workspace), Ok(()));

    assert_eq!(
        require!(harness.worktree_marker(&workspace)),
        "作業中の変更",
        "既存の変更に触れない"
    );
    CaseOutcome::Ran
}

pub fn tc_port_worktree_manager_013_ブランチのみ残存していれば先端を変えずに張り直される(
    harness: &impl WorktreeManagerHarness,
) -> CaseOutcome {
    let repo = require!(harness.repo_with_commit());
    let base = require!(harness.head_branch_name());
    let (workspace, committed) = require!(harness.workspace_with_orphan_branch());
    let tip = require!(harness.branch_tip(workspace.branch()));

    assert_eq!(harness.manager().create(&repo, &base, &workspace), Ok(()));

    assert_eq!(
        require!(harness.branch_tip(workspace.branch())),
        tip,
        "ブランチの先端は変わらない"
    );
    assert_eq!(
        require!(harness.worktree_marker(&workspace)),
        committed,
        "積まれたコミットの成果物が worktree に現れる"
    );
    CaseOutcome::Ran
}

pub fn tc_port_worktree_manager_014_worktreeでない通常のディレクトリは失敗し内容に触れない(
    harness: &impl WorktreeManagerHarness,
) -> CaseOutcome {
    let repo = require!(harness.repo_with_commit());
    let base = require!(harness.head_branch_name());
    let (workspace, existing) = require!(harness.workspace_over_plain_dir());

    assert_create_failed(harness.manager().create(&repo, &base, &workspace));

    assert_eq!(
        require!(harness.worktree_marker(&workspace)),
        existing,
        "既存ディレクトリの内容に触れない"
    );
    assert_eq!(
        harness.manager().branch_exists(&repo, workspace.branch()),
        Ok(false),
        "自動修復しない"
    );
    CaseOutcome::Ran
}

pub fn tc_port_worktree_manager_015_別ブランチのworktreeがあれば失敗し既存に触れない(
    harness: &impl WorktreeManagerHarness,
) -> CaseOutcome {
    let repo = require!(harness.repo_with_commit());
    let base = require!(harness.head_branch_name());
    let (workspace, existing) = require!(harness.workspace_over_other_branch());

    assert_create_failed(harness.manager().create(&repo, &base, &workspace));

    assert_eq!(
        require!(harness.worktree_marker(&workspace)),
        existing,
        "既存 worktree の内容に触れない"
    );
    assert_eq!(
        harness.manager().branch_exists(&repo, workspace.branch()),
        Ok(false),
        "パスの存在だけでは達成済みとみなさず、ブランチも作らない"
    );
    CaseOutcome::Ran
}

pub fn tc_port_worktree_manager_016_baseが存在しなければブランチもworktreeも作られない(
    harness: &impl WorktreeManagerHarness,
) -> CaseOutcome {
    let repo = require!(harness.repo_with_commit());
    let base = require!(harness.absent_branch_name());
    let workspace = require!(harness.unused_workspace());

    assert_create_failed(harness.manager().create(&repo, &base, &workspace));

    assert!(!require!(harness.worktree_present(&workspace)));
    assert_eq!(
        harness.manager().branch_exists(&repo, workspace.branch()),
        Ok(false)
    );
    CaseOutcome::Ran
}

/// 台帳行に対応しない追加ケース(ADR-077)。
///
/// 実体の消えた登録からの復旧は、登録ごと消えてブランチだけが残った状態
/// (TC-013)とは別の分岐で処理される。両方を実行するケースが無いと、片方の分岐が
/// 落ちても適合スイートが緑のままになる。
pub fn create_prunable_実体の消えた登録は先端を変えずに張り直される(
    harness: &impl WorktreeManagerHarness,
) -> CaseOutcome {
    let repo = require!(harness.repo_with_commit());
    let base = require!(harness.head_branch_name());
    let (workspace, committed) = require!(harness.workspace_with_prunable_registration());
    let tip = require!(harness.branch_tip(workspace.branch()));

    assert_eq!(harness.manager().create(&repo, &base, &workspace), Ok(()));

    assert_eq!(
        require!(harness.branch_tip(workspace.branch())),
        tip,
        "ブランチの先端は変わらない"
    );
    assert_eq!(
        require!(harness.worktree_marker(&workspace)),
        committed,
        "積まれたコミットの成果物が worktree に戻る"
    );
    CaseOutcome::Ran
}

/// 自動修復せず、原因の説明を伴って失敗することを確かめる。
fn assert_create_failed(result: Result<(), WorktreeError>) {
    match result {
        Err(WorktreeError::Failed { message }) => {
            assert!(!message.is_empty(), "原因が説明される");
        }
        Ok(()) => panic!("既存の実体には触れず失敗する"),
    }
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
                tc_port_worktree_manager_010_未使用のパスとブランチにはbaseから新しいworktreeが作られる,
                tc_port_worktree_manager_011_worktreeの置き場が未作成でも用意される,
                tc_port_worktree_manager_012_自タスクの残骸への再作成は内容に触れず成功する,
                tc_port_worktree_manager_013_ブランチのみ残存していれば先端を変えずに張り直される,
                tc_port_worktree_manager_014_worktreeでない通常のディレクトリは失敗し内容に触れない,
                tc_port_worktree_manager_015_別ブランチのworktreeがあれば失敗し既存に触れない,
                tc_port_worktree_manager_016_baseが存在しなければブランチもworktreeも作られない,
                create_prunable_実体の消えた登録は先端を変えずに張り直される,
            ]
        );
    };
}
