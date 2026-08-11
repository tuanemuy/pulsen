//! ポート適合テストの枠組み。
//!
//! `spec/testcases/ports/*.md` の表の1行を1つのケース関数として書き、アダプター側の
//! テストファイルはマクロ呼び出し1行でスイート全体を適用する。ケース関数はポートの
//! トレイトと `Harness` のフックだけを使い、永続化技術には依存しない。後続スライスの
//! in-memory 実装や別プラットフォーム実装も、フックを実装してマクロを呼ぶだけで
//! 同じスイートを通せる。
//!
//! # ハーネス
//!
//! ポートごとに `Harness` トレイトがある。検証の対象は関連型と共有参照の組
//! (`type Repo` + `fn repo(&self) -> &Self::Repo`)で渡し、それ以外のメソッドは
//! **前提条件の意味だけを受け取るフック**にする。生の JSON テキストや権限操作の手順を
//! スイート側に持ち込まないことで、ケースが特定のアダプター専用にならない。
//!
//! 対象は共有参照でしか渡らないため、「構築済みの対象を壊す」フックは置けない。壊れた
//! 状況が要るケースは、別ハンドルを返すフック(`concurrent_repo` / `failing_manager` /
//! `unusable_lock` / `separate_home` / `another_generator`)で表す。
//!
//! # スキップ
//!
//! フックの既定実装はすべて `None` を返し、そのケースはスキップされる。`spec` が
//! 「再現できるアダプター環境に限る」と明示する行も同じ仕組みで落ちる。
//! スキップの理由(どのフックが提供されなかったか)は標準出力に書き出すため、
//! `cargo test -- --nocapture` で確認できる。
//!
//! 権限操作を伴うフック(`make_unreadable` / `make_unwritable`)は、**制限が実際に効いた
//! ことを確認してから `Some` を返す**規則にする。`chmod 000` は root では効かないため、
//! 確認せずに `Some` を返すと `Err(Io)` を期待するケースがスキップに落ちずに失敗する。
//!
//! # スイートの適用
//!
//! ケース関数はポートごとのモジュールに置き、そのモジュールがスイート適用のマクロを
//! 1つ公開する。マクロはケース名を並べるだけで、`#[test]` の生成・ハーネスの構築・
//! スキップの報告は [`conformance_cases!`] が行う。
//!
//! ```text
//! // crates/pulsen-conformance/src/config_store.rs
//! pub fn tc_port_config_store_001_全キーが反映される(h: &impl ConfigStoreHarness) -> CaseOutcome {
//!     require!(h.put_config(TEXT));
//!     ...
//!     CaseOutcome::Ran
//! }
//!
//! #[macro_export]
//! macro_rules! config_store_conformance {
//!     ($setup:expr) => {
//!         use $crate::config_store as __pulsen_conformance_config_store;
//!         $crate::conformance_cases!(
//!             __pulsen_conformance_config_store,
//!             $setup,
//!             [tc_port_config_store_001_全キーが反映される]
//!         );
//!     };
//! }
//!
//! // crates/pulsen/tests/conformance_config_store.rs
//! pulsen_conformance::config_store_conformance!(FsConfigStoreHarness::new());
//! ```
//!
//! # 対応表
//!
//! どの行をどのフックで組むかは `HOOKS.md` にある。フックを足すときは対応表も更新する。
//!
//! # テストダブル
//!
//! ユースケースの分岐網羅に使うスクリプト式のポート実装は [`doubles`] にある(ADR-028)。
//! 適合スイートとは目的が違う(契約への適合 vs 分岐の網羅)ため、フックとは別の口にする。

pub mod clock;
pub mod config_store;
pub mod doubles;
pub mod exclusive_lock;
pub mod task_id_generator;
pub mod task_repository;
pub mod workflow_store;
pub mod worktree_manager;

use std::path::PathBuf;

use pulsen_domain::definition::{ConfigStore, WorkflowStore};
use pulsen_domain::execution::{ExclusiveLock, WorktreeManager};
use pulsen_domain::task::{
    BranchName, Clock, RepoPath, TaskId, TaskIdGenerator, TaskRepository, Timestamp,
};

/// タスクファイルの置き場。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Area {
    /// 現役(`state/tasks/`)。
    Active,
    /// アーカイブ済み(`state/archive/`)。
    Archived,
}

/// フックが掛けた制限を元に戻すハンドル。
///
/// ドロップで復元する。ケースは観測が終わるまで保持するだけでよく、復元の手順は
/// ハーネスの内側に閉じる。
pub struct Restore {
    undo: Option<Box<dyn FnOnce()>>,
}

impl Restore {
    /// 復元処理を受け取る。
    pub fn new(undo: impl FnOnce() + 'static) -> Self {
        Self {
            undo: Some(Box::new(undo)),
        }
    }
}

impl Drop for Restore {
    fn drop(&mut self) {
        if let Some(undo) = self.undo.take() {
            undo();
        }
    }
}

impl std::fmt::Debug for Restore {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Restore")
    }
}

/// ケース1件の結果。
///
/// 検証の成否は `assert!` 系のパニックで表すため、ここが持つのは「走ったか、前提条件を
/// 用意できずスキップしたか」だけになる。
#[derive(Debug, PartialEq, Eq)]
#[must_use]
pub enum CaseOutcome {
    /// 前提条件を用意でき、検証まで到達した。
    Ran,
    /// 前提条件を用意できなかった。
    Skipped {
        /// `None` を返したフックの名前。
        hook: &'static str,
    },
}

impl CaseOutcome {
    /// 指定のフックが提供されなかったことによるスキップ。
    pub fn skipped(hook: &'static str) -> Self {
        Self::Skipped { hook }
    }

    /// 結果を報告する。マクロが生成する `#[test]` から呼ばれる。
    pub fn report(self, case: &str) {
        match self {
            Self::Ran => {}
            Self::Skipped { hook } => {
                println!(
                    "SKIP {case}: ハーネスが {hook} を提供しないため、この環境では前提条件を用意できない"
                );
            }
        }
    }
}

/// フックの値を取り出し、提供されていなければケースをスキップして戻る。
///
/// ```text
/// let home = require!(harness.home_path());
/// ```
#[macro_export]
macro_rules! require {
    ($harness:ident . $hook:ident ( $($argument:expr),* $(,)? )) => {
        match $harness.$hook($($argument),*) {
            ::std::option::Option::Some(value) => value,
            ::std::option::Option::None => {
                return $crate::CaseOutcome::skipped(stringify!($hook));
            }
        }
    };
}

/// ケース関数の並びから `#[test]` 関数を生成する。
///
/// `$module` にはケース関数を持つモジュールの別名を渡す(ポートごとのマクロが
/// `use` で用意する)。セットアップ式はケースごとに評価され、ハーネスは共有されない。
#[macro_export]
macro_rules! conformance_cases {
    ($module:ident, $setup:expr, [ $($case:ident),* $(,)? ]) => {
        $(
            #[test]
            fn $case() {
                let harness = $setup;
                $crate::CaseOutcome::report($module::$case(&harness), stringify!($case));
            }
        )*
    };
}

/// ConfigStore の適合スイートが要求する環境。
pub trait ConfigStoreHarness {
    /// 検証対象。
    type Store: ConfigStore;

    /// 検証対象を返す。
    fn store(&self) -> &Self::Store;

    /// グローバルホームの config.yaml を指定の内容にする(既存があれば置き換える)。
    fn put_config(&self, _text: &str) -> Option<()> {
        None
    }

    /// config.yaml を取り除く(TC-port-config-store-013)。
    fn remove_config(&self) -> Option<()> {
        None
    }

    /// `ConfigLoadError::NotFound` が含むべき解決後のホームパス
    /// (TC-port-config-store-013)。
    fn home_path(&self) -> Option<PathBuf> {
        None
    }

    /// config.yaml を読み取れない状態にする(TC-port-config-store-023)。
    ///
    /// 制限が実際に効いたことを確認してから `Some` を返す。効かなければ復元して `None`。
    fn make_unreadable(&self) -> Option<Restore> {
        None
    }
}

/// WorkflowStore の適合スイートが要求する環境。
pub trait WorkflowStoreHarness {
    /// 検証対象。
    type Store: WorkflowStore;

    /// 検証対象を返す。
    fn store(&self) -> &Self::Store;

    /// 名前で解決される位置に定義を置く(既存があれば置き換える。
    /// TC-port-workflow-store-031 の上書きも兼ねる)。
    fn put_named(&self, _name: &str, _text: &str) -> Option<()> {
        None
    }

    /// 名前解決の対象にならない拡張子で定義を置く(TC-port-workflow-store-002)。
    fn put_named_with_ext(&self, _name: &str, _extension: &str, _text: &str) -> Option<()> {
        None
    }

    /// 名前が解決されるべきパス。`NotFound` の `attempted` と `resolved_from` の期待値
    /// (TC-port-workflow-store-001/002/003)。
    fn expected_path_for_name(&self, _name: &str) -> Option<PathBuf> {
        None
    }

    /// 名前解決の対象外の場所に定義を置き、その絶対パスを返す
    /// (TC-port-workflow-store-004)。
    fn put_at_absolute(&self, _text: &str) -> Option<PathBuf> {
        None
    }

    /// 相対パスで参照できる場所に定義を置き、(相対パス, 解決されるべき絶対パス)を返す
    /// (TC-port-workflow-store-005)。相対の基準がハーネス側にあることを型で示す。
    fn put_at_relative(&self, _text: &str) -> Option<(PathBuf, PathBuf)> {
        None
    }

    /// 定義が存在しない絶対パス(TC-port-workflow-store-006)。
    fn missing_absolute_path(&self) -> Option<PathBuf> {
        None
    }

    /// 名前で解決される定義を読み取れない状態にする(TC-port-workflow-store-030)。
    ///
    /// 制限が実際に効いたことを確認してから `Some` を返す。効かなければ復元して `None`。
    fn make_unreadable(&self, _name: &str) -> Option<Restore> {
        None
    }
}

/// TaskRepository の適合スイートが要求する環境。
///
/// 破損系のフックは「どう壊すか」ではなく「何が壊れているか」を受け取る。対象の ID は
/// 呼び出し前に `create` 済みであることを前提にしてよい。
pub trait TaskRepositoryHarness {
    /// 検証対象。
    type Repo: TaskRepository;

    /// 検証対象を返す。
    fn repo(&self) -> &Self::Repo;

    /// レコード全体を読めない状態にする
    /// (TC-port-task-repository-004/020/027/029/039/040)。
    fn corrupt_whole_record(&self, _area: Area, _id: &TaskId) -> Option<()> {
        None
    }

    /// レコードは読めるまま、タスク側フィールドの値制約を破る
    /// (TC-port-task-repository-021)。
    fn break_task_field(&self, _area: Area, _id: &TaskId) -> Option<()> {
        None
    }

    /// タスク側フィールドは読めるまま、スナップショットを解釈できない内容にする
    /// (TC-port-task-repository-009/012/022/028/029/039/040)。
    fn corrupt_snapshot(&self, _area: Area, _id: &TaskId) -> Option<()> {
        None
    }

    /// スナップショットそのものを取り除く(TC-port-task-repository-023)。
    fn drop_snapshot_field(&self, _area: Area, _id: &TaskId) -> Option<()> {
        None
    }

    /// タスクステータスをスナップショットに無い名前にする(不変条件1の破れ。
    /// TC-port-task-repository-024)。
    fn set_task_status_outside_snapshot(
        &self,
        _area: Area,
        _id: &TaskId,
        _status: &str,
    ) -> Option<()> {
        None
    }

    /// スナップショットの構造不変条件(`initial ∉ statuses`・`next ∉ statuses`)を破る
    /// (TC-port-task-repository-025)。
    fn break_snapshot_invariant(&self, _area: Area, _id: &TaskId) -> Option<()> {
        None
    }

    /// 同一 ID を現役とアーカイブの双方に置く(TC-port-task-repository-018)。
    fn place_in_both_areas(&self, _id: &TaskId) -> Option<()> {
        None
    }

    /// タスクファイルの命名形式に合致しないエントリを置く
    /// (TC-port-task-repository-030)。
    fn put_unnamed_entry(&self, _area: Area) -> Option<()> {
        None
    }

    /// レコード全体の現在の内容。書き込みを拒否したときの不変を観測する
    /// (TC-port-task-repository-004)。
    fn record_bytes(&self, _area: Area, _id: &TaskId) -> Option<Vec<u8>> {
        None
    }

    /// スナップショットの現在の内容。破損したスナップショットの温存を観測する
    /// (TC-port-task-repository-009)。
    fn snapshot_bytes(&self, _area: Area, _id: &TaskId) -> Option<Vec<u8>> {
        None
    }

    /// 走査対象を読み取れない状態にする(TC-port-task-repository-019/041)。
    ///
    /// 制限が実際に効いたことを確認してから `Some` を返す。効かなければ復元して `None`。
    fn make_unreadable(&self, _area: Area) -> Option<Restore> {
        None
    }

    /// 書き込み先を用意できない状態にする(TC-port-task-repository-005/011/012/035)。
    ///
    /// 制限が実際に効いたことを確認してから `Some` を返す。効かなければ復元して `None`。
    fn make_unwritable(&self, _area: Area) -> Option<Restore> {
        None
    }

    /// 並行して読み書きできる対象(TC-port-task-repository-042/044)。
    ///
    /// 別スレッドから読み続ける前提を持つケースだけがこのハンドルを使う。`Sync` を
    /// ここに閉じ込めることで、`RefCell` ベースの実装も残りのケースを通せる。
    fn concurrent_repo(&self) -> Option<&(dyn TaskRepository + Sync)> {
        None
    }
}

/// Clock の適合スイートが要求する環境。
pub trait ClockHarness {
    /// 検証対象。
    type Clock: Clock;

    /// 検証対象を返す。
    fn clock(&self) -> &Self::Clock;

    /// 外部から観測した現在の実時刻(TC-port-clock-003)。
    fn observe_wall_clock(&self) -> Option<Timestamp> {
        None
    }

    /// 時刻が確実に前進した状態にする(TC-port-clock-004)。実時間を待つ実装を含む。
    fn advance(&self) -> Option<()> {
        None
    }

    /// 時刻を過去に巻き戻した状態にする(TC-port-clock-005)。
    fn rewind(&self) -> Option<()> {
        None
    }
}

/// TaskIdGenerator の適合スイートが要求する環境。
pub trait TaskIdGeneratorHarness {
    /// 検証対象。
    type Generator: TaskIdGenerator;

    /// 検証対象を返す。
    fn generator(&self) -> &Self::Generator;

    /// 同じ構成で用意した別のジェネレーター(TC-port-task-id-generator-004)。
    fn another_generator(&self) -> Option<&Self::Generator> {
        None
    }
}

/// ExclusiveLock の適合スイートが要求する環境。
pub trait ExclusiveLockHarness {
    /// 検証対象。
    type Lock: ExclusiveLock;

    /// ロックを保持している別プロセスのハンドル。別プロセスを扱えない実装は
    /// `type Holder = ();` を置き、`hold_from_other_process` の既定実装をそのまま使う。
    type Holder;

    /// 検証対象を返す。
    fn lock(&self) -> &Self::Lock;

    /// 同一グローバルホームのロックを別プロセスに保持させる
    /// (TC-port-exclusive-lock-002/003/005)。
    fn hold_from_other_process(&self) -> Option<Self::Holder> {
        None
    }

    /// 保持プロセスを強制終了する(解放処理を実行させない。
    /// TC-port-exclusive-lock-005)。
    fn kill_holder(&self, _holder: Self::Holder) -> Option<()> {
        None
    }

    /// 保持プロセスを正常に終了させ、ロックを解放する。
    fn release_holder(&self, _holder: Self::Holder) -> Option<()> {
        None
    }

    /// 別プロセスから取得を試み、取得できたかを返す(TC-port-exclusive-lock-004)。
    ///
    /// `None` は「別プロセスを扱えない実装」で、`Some(false)` は「競合して取得できな
    /// かった」。両者を混同すると、スキップすべきケースが失敗として現れる。
    fn try_acquire_from_other_process(&self) -> Option<bool> {
        None
    }

    /// 異なるグローバルホームのロック(TC-port-exclusive-lock-006)。
    fn separate_home(&self) -> Option<&Self::Lock> {
        None
    }

    /// ロック機構自体が利用不能な状態のロック(TC-port-exclusive-lock-007)。
    ///
    /// `try_acquire` が `Err(LockError::Failed)` を返す実装を返す契約。
    fn unusable_lock(&self) -> Option<&Self::Lock> {
        None
    }
}

/// WorktreeManager の適合スイートが要求する環境(対象の検証を行う3メソッド分)。
pub trait WorktreeManagerHarness {
    /// 検証対象。
    type Manager: WorktreeManager;

    /// 検証対象を返す。
    fn manager(&self) -> &Self::Manager;

    /// コミットのあるリポジトリ(TC-port-worktree-manager-001/004/007/008)。
    fn repo_with_commit(&self) -> Option<RepoPath> {
        None
    }

    /// コミットのない空リポジトリ(TC-port-worktree-manager-006)。
    fn repo_without_commit(&self) -> Option<RepoPath> {
        None
    }

    /// detached HEAD のリポジトリ(TC-port-worktree-manager-005)。
    fn detached_repo(&self) -> Option<RepoPath> {
        None
    }

    /// リポジトリでない実在のディレクトリ(TC-port-worktree-manager-003)。
    fn non_repo_dir(&self) -> Option<RepoPath> {
        None
    }

    /// 存在しないパス(TC-port-worktree-manager-002)。スイートが捏造すると、パスを
    /// 持たない実装で意味を持たなくなる。
    fn missing_path(&self) -> Option<RepoPath> {
        None
    }

    /// `repo_with_commit` の HEAD が指すブランチ名
    /// (TC-port-worktree-manager-004/007)。
    fn head_branch_name(&self) -> Option<BranchName> {
        None
    }

    /// `repo_with_commit` に存在しないブランチ名(TC-port-worktree-manager-008)。
    fn absent_branch_name(&self) -> Option<BranchName> {
        None
    }

    /// git 操作自体が失敗する状態の実装(TC-port-worktree-manager-009)。
    ///
    /// 3メソッドとも `Err(TargetError::Failed)` を返す契約。対象を壊すのではなく別の
    /// ハンドルを返すため、本番アダプターはイミュータブルなままでよい。
    fn failing_manager(&self) -> Option<&Self::Manager> {
        None
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::thread;

    use pulsen_domain::task::{
        ArchiveError, CreateError, DegradedTask, ReadError, SaveError, Task, TaskEntry, TaskLookup,
    };

    use super::*;

    /// 枠組みの検証だけに使う対象。内部の型引数を変えるだけで `Sync` の有無を作り分けられる。
    struct ToyRepo<Cell>(Cell);

    impl<Cell> TaskRepository for ToyRepo<Cell> {
        fn create(&self, _task: &Task) -> Result<(), CreateError> {
            Ok(())
        }

        fn save(&self, _task: &Task) -> Result<(), SaveError> {
            Err(SaveError::NotFound)
        }

        fn save_degraded(&self, _task: &DegradedTask) -> Result<(), SaveError> {
            Err(SaveError::NotFound)
        }

        fn find(&self, _id: &TaskId) -> Result<TaskLookup, ReadError> {
            Ok(TaskLookup::NotFound)
        }

        fn list_active(&self) -> Result<Vec<TaskEntry>, ReadError> {
            Ok(Vec::new())
        }

        fn list_archived(&self) -> Result<Vec<TaskEntry>, ReadError> {
            Ok(Vec::new())
        }

        fn archive(&self, _id: &TaskId) -> Result<(), ArchiveError> {
            Err(ArchiveError::NotFound)
        }
    }

    /// 共有可変状態を内部可変性で持つ実装。`Sync` を満たさない。
    struct NotSyncHarness {
        repo: ToyRepo<RefCell<u32>>,
    }

    impl TaskRepositoryHarness for NotSyncHarness {
        type Repo = ToyRepo<RefCell<u32>>;

        fn repo(&self) -> &Self::Repo {
            &self.repo
        }
    }

    /// 並行して読み書きできる実装。
    struct SyncHarness {
        repo: ToyRepo<()>,
    }

    impl TaskRepositoryHarness for SyncHarness {
        type Repo = ToyRepo<()>;

        fn repo(&self) -> &Self::Repo {
            &self.repo
        }

        fn concurrent_repo(&self) -> Option<&(dyn TaskRepository + Sync)> {
            Some(&self.repo)
        }
    }

    /// 大多数のケースの形。`Sync` を要求しないため `RefCell` ベースの実装にも適用できる。
    mod cases {
        use super::*;

        pub fn 走査するケース(harness: &impl TaskRepositoryHarness) -> CaseOutcome {
            let entries = harness.repo().list_active().expect("走査できる");

            assert!(entries.is_empty());
            CaseOutcome::Ran
        }

        pub fn 並行して読むケース(harness: &impl TaskRepositoryHarness) -> CaseOutcome {
            let repo = require!(harness.concurrent_repo());

            thread::scope(|scope| {
                scope.spawn(|| {
                    let observed = repo.list_active().expect("走査できる");
                    assert!(observed.is_empty());
                });
            });
            CaseOutcome::Ran
        }
    }

    fn not_sync_harness() -> NotSyncHarness {
        NotSyncHarness {
            repo: ToyRepo(RefCell::new(0)),
        }
    }

    fn sync_harness() -> SyncHarness {
        SyncHarness { repo: ToyRepo(()) }
    }

    conformance_cases!(cases, not_sync_harness(), [走査するケース]);

    #[test]
    fn 並行フックを持たないハーネスではケースがスキップされる() {
        let outcome = cases::並行して読むケース(&not_sync_harness());

        assert_eq!(outcome, CaseOutcome::skipped("concurrent_repo"));
    }

    #[test]
    fn 並行フックを持つハーネスでは別スレッドから読める() {
        let outcome = cases::並行して読むケース(&sync_harness());

        assert_eq!(outcome, CaseOutcome::Ran);
    }

    #[test]
    fn 復元ハンドルはドロップで元に戻す() {
        let restored = std::rc::Rc::new(std::cell::Cell::new(false));
        let flag = std::rc::Rc::clone(&restored);

        drop(Restore::new(move || flag.set(true)));

        assert!(restored.get());
    }
}
