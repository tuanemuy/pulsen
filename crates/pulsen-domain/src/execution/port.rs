//! Execution ドメインが外界に要求する操作。
//!
//! 本スライスでは対象(リポジトリ・ブランチ)の検証と排他ロックだけを宣言する。
//! worktree の作成・削除、run ディレクトリ・プロセス・コマンド実行のポートは、それらを
//! 使うスライスでメソッドを足す(未実装メソッドの宣言とスタブは置かない)。

use crate::task::{BranchName, RepoPath};

/// 対象の検証の失敗。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetError {
    /// パスが存在しない。
    NotFound,
    /// git リポジトリではない。
    NotARepository,
    /// HEAD がブランチを指していない。
    DetachedHead,
    /// コミットのない空リポジトリ。
    EmptyRepository,
    /// 実行環境のエラー(git 操作自体の失敗)。
    Failed {
        /// 原因の説明。
        message: String,
    },
}

/// 対象(リポジトリ・ブランチ)の検証。
///
/// 契約: 検証は読み取りのみで、リポジトリの内容を変更しない。分類(`NotFound` /
/// `NotARepository` / `DetachedHead` / `EmptyRepository`)と実行環境のエラー(`Failed`)を
/// 区別して値で返す。
pub trait WorktreeManager {
    /// 対象が git リポジトリであることを検証する。
    fn validate_repo(&self, repo: &RepoPath) -> Result<(), TargetError>;

    /// HEAD が指すブランチ名を取得する。
    fn head_branch(&self, repo: &RepoPath) -> Result<BranchName, TargetError>;

    /// 指定ブランチの存在有無を返す。
    fn branch_exists(&self, repo: &RepoPath, branch: &BranchName) -> Result<bool, TargetError>;
}

/// ロック機構自体の異常。
///
/// 「取得できなかった」は `Ok(None)` でありエラーではない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LockError {
    /// ロック機構の異常。
    Failed {
        /// 原因の説明。
        message: String,
    },
}

/// 排他ロックの保持を表すガード。
///
/// ドロップで解放されること**だけ**を契約とするマーカートレイト。ハンドルの実体
/// (ファイル記述子等)はアダプターが持ち、ドメインはロックの寿命だけを知る。
pub trait LockGuard {}

/// グローバルホーム単位の単一の排他ロック。
///
/// 契約:
/// - 単一のグローバルロック(グローバルホームに1つ)。すべての状態変更操作が同じロックを取る
/// - ブロックしない: 取得できなければ即座に `Ok(None)`
/// - ガードのドロップで解放される。保持プロセスの異常終了でも OS により解放される
/// - 排他の単位はプロセス間である。同一プロセス内での再取得の挙動は規定しない
/// - 読み取り専用操作は取得しない
pub trait ExclusiveLock {
    /// ロックの取得を試みる。
    fn try_acquire(&self) -> Result<Option<Box<dyn LockGuard>>, LockError>;
}
