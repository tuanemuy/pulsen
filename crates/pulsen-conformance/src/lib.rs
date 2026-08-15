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
//! この性質が成り立つのは TaskRepository / Clock / TaskIdGenerator / ExclusiveLock /
//! WorktreeManager / RunStore / ProcessController / CommandRunner の8ポート。
//! ConfigStore / WorkflowStore の入力系フックは **YAML
//! ソースを受け取り**、この2ポートのスイートは YAML 表現に結合している — 「YAML 構文
//! エラー」「重複キー」を前提とする行は、表現そのものを渡す口が無ければ組み立てられない。
//!
//! 対象は共有参照でしか渡らないため、「構築済みの対象を壊す」フックは置けない。壊れた
//! 状況が要るケースは、別ハンドルを返すフック(`concurrent_repo` / `concurrent_store` /
//! `failing_manager` / `unusable_lock` / `separate_home` / `another_generator`)で表す。
//!
//! # スキップ
//!
//! フックの既定実装はすべて `None` を返し、そのケースはスキップされる。`spec` が
//! 「再現できるアダプター環境に限る」と明示する行も同じ仕組みで落ちる。
//! スキップの理由(どのフックが提供されなかったか)は標準出力に書き出すため、
//! `cargo test -- --nocapture` で確認できる。
//!
//! 標準出力は libtest が握り潰すため、報告だけではスキップと成功を区別できない。
//! スイートの適用側は[スキップを許容するケースの集合][SkipBudget]を宣言し、集合の外の
//! スキップはそのケースの失敗として現れる。「N件 PASS」の意味が環境で変わったことが
//! 出力に出る。
//!
//! 権限操作を伴うフック(`make_unreadable` / `make_unwritable`)は、**制限が実際に効いた
//! ことを確認してから `Some` を返す**規則にする。`chmod 000` は root では効かないため、
//! 確認せずに `Some` を返すと `Err(Io)` を期待するケースがスキップに落ちずに失敗する。
//! 許容する集合も同じ述語([`permission_restrictions_effective`])で決めることで、宣言が
//! プラットフォームではなく**環境の能力**に対応する。
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
//!     ($setup:expr, $allowed_skips:expr) => {
//!         use $crate::config_store as __pulsen_conformance_config_store;
//!         $crate::conformance_cases!(
//!             __pulsen_conformance_config_store,
//!             $setup,
//!             __PULSEN_CONFORMANCE_CONFIG_STORE_SKIPS = $allowed_skips,
//!             [tc_port_config_store_001_全キーが反映される]
//!         );
//!     };
//! }
//!
//! // crates/pulsen/tests/conformance_config_store.rs
//! pulsen_conformance::config_store_conformance!(FsConfigStoreHarness::new(), Vec::new());
//! ```
//!
//! # 対応表
//!
//! どの行をどのフックで組むかは `HOOKS.md` にある。フックを足すときは対応表も更新する。
//!
//! # テストダブル
//!
//! ユースケースの分岐網羅に使うスクリプト式のポート実装は [`doubles`] にある。
//! 適合スイートとは目的が違う(契約への適合 vs 分岐の網羅)ため、フックとは別の口にする。

pub mod clock;
pub mod command_runner;
pub mod config_store;
pub mod doubles;
pub mod exclusive_lock;
pub mod process_controller;
pub mod run_store;
pub mod task_id_generator;
pub mod task_repository;
pub mod workflow_store;
pub mod worktree_manager;

use std::path::PathBuf;
use std::time::Duration;

use pulsen_domain::definition::{CommandLine, ConfigStore, PlainCommand, WorkflowStore};
use pulsen_domain::execution::{
    CommandRunner, ExclusiveLock, ProcessController, RunStore, WorktreeManager, WrapperLaunchSpec,
};
use pulsen_domain::task::{
    AttemptNumber, BranchName, Clock, KillIdent, Pid, RepoPath, RunDirPath, TaskId,
    TaskIdGenerator, TaskRepository, Timestamp, Workspace, WorktreePath,
};

/// タスクファイルの置き場。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Area {
    /// 現役(`state/tasks/`)。
    Active,
    /// アーカイブ済み(`state/archive/`)。
    Archived,
}

/// run ディレクトリに置かれるファイルの種別。
///
/// 破損・読み取り不能の前提を作るフックが、どのファイルを対象にするかだけを受け取る。
/// 置き場そのもの(パス)は契約の語彙(`RunDirPath` の導出関数)で決まる。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunFileKind {
    /// pid ファイル。
    Pid,
    /// starttime ファイル。
    StartTime,
    /// exit ファイル。
    Exit,
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
    pub fn report(self, case: &str, budget: &SkipBudget) {
        match self {
            Self::Ran => {}
            Self::Skipped { hook } => budget.record(case, hook),
        }
    }
}

/// スイート1適用あたりでスキップを許容するケースの集合。
///
/// libtest は成功したテストの標準出力を握り潰すため、スキップの報告だけでは
/// 「何件が実際に走ったか」が環境によって静かに変わる。適用側が「この環境ではどのケースが
/// スキップされうるか」を宣言し、集合の外のスキップをそのケースの失敗にすることで、
/// 差が出力に現れる。
///
/// 件数ではなく集合で宣言するのは、想定した行が走った代わりに別の行がフックを得られず
/// スキップしても、合計が合えば緑のまま通ってしまうため。集合は実行時に組んでよく、
/// 環境の能力([`permission_restrictions_effective`])から導くと、宣言が
/// プラットフォームではなく実際に前提を作れるかどうかに対応する。
pub struct SkipBudget {
    allowed: Vec<&'static str>,
}

impl SkipBudget {
    /// スキップを許容するケースを宣言する。
    ///
    /// 要素はケース名の接頭辞となる TC ID(`tc_port_config_store_023`)。ケース関数は
    /// `<TC ID>_<仕様の言葉>` で名付けるため、説明部分を言い換えても宣言は腐らない。
    pub fn new(allowed: Vec<&'static str>) -> Self {
        Self { allowed }
    }

    /// スキップを1件記録する。宣言していないケースのスキップはそのケースを失敗させる。
    ///
    /// 集合内のスキップも `SKIP` 行として必ず出力する — 走らなかった行は
    /// `cargo test -- --nocapture` で理由まで辿れる。
    pub fn record(&self, case: &str, hook: &'static str) {
        println!(
            "SKIP {case}: ハーネスが {hook} を提供しないため、この環境では前提条件を用意できない"
        );
        assert!(
            self.allows(case),
            "{case} がスキップされた({hook} が提供されない)。このスイートがスキップを\
             許容するのは {:?} だけ。この環境で前提条件を用意できるようにするか、\
             宣言を実態に合わせる",
            self.allowed
        );
    }

    /// 宣言した TC ID のいずれかがケース名の先頭に一致するか。
    fn allows(&self, case: &str) -> bool {
        self.allowed.iter().any(|id| {
            case.strip_prefix(id)
                .is_some_and(|rest| rest.is_empty() || rest.starts_with('_'))
        })
    }
}

/// この環境で「読み取れないファイル」を作れるか(1度だけ調べて使い回す)。
///
/// 権限操作のフックは「制限が実際に効いたことを確認してから `Some` を返す」規則なので、
/// root 実行や権限を持たないファイルシステムでは必ず `None` を返し、
/// 権限を前提とするケースはスキップされる。[`SkipBudget`] の宣言をこの述語で決めると、
/// 「環境が前提を作れないからスキップした」と「フックの実装漏れでスキップした」を
/// 取り違えずに済む。
#[must_use]
pub fn permission_restrictions_effective() -> bool {
    static EFFECTIVE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();

    *EFFECTIVE.get_or_init(probe_permission_restrictions)
}

/// 一時ファイルを `chmod 000` して読めるかを試す。フックが掛ける制限と同じ手順で
/// 判定するため、判定と実際のスキップが食い違わない。
#[cfg(unix)]
fn probe_permission_restrictions() -> bool {
    use std::os::unix::fs::PermissionsExt;
    use std::time::{SystemTime, UNIX_EPOCH};

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    let path = std::env::temp_dir().join(format!(
        "pulsen-conformance-probe-{}-{nanos}",
        std::process::id()
    ));
    if std::fs::write(&path, b"probe").is_err() {
        return false;
    }

    let denied = match std::fs::metadata(&path) {
        Ok(metadata) => {
            let mut permissions = metadata.permissions();
            permissions.set_mode(0o000);
            std::fs::set_permissions(&path, permissions).is_ok() && std::fs::read(&path).is_err()
        }
        Err(_) => false,
    };
    let _ = std::fs::remove_file(&path);
    denied
}

/// 権限で読み取りを止める仕組みを持たないプラットフォームでは、権限操作のフックも
/// 一律 `None` を返す。
#[cfg(not(unix))]
fn probe_permission_restrictions() -> bool {
    false
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
/// `$budget` はスキップ宣言の置き場で、1つのテストファイルに複数のスイートを適用
/// できるようポートごとに別の名前を渡す。
///
/// `$allowed_skips` は最初のケースが走るときに1度だけ評価される。環境を調べて集合を
/// 決める式を書けるよう、定数ではなく遅延初期化にする。
#[macro_export]
macro_rules! conformance_cases {
    ($module:ident, $setup:expr, $budget:ident = $allowed_skips:expr, [ $($case:ident),* $(,)? ]) => {
        /// このスイートでスキップを許容するケース。
        static $budget: ::std::sync::LazyLock<$crate::SkipBudget> =
            ::std::sync::LazyLock::new(|| $crate::SkipBudget::new($allowed_skips));

        $(
            #[test]
            fn $case() {
                let harness = $setup;
                $crate::CaseOutcome::report(
                    $module::$case(&harness),
                    stringify!($case),
                    &$budget,
                );
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

/// テスト用エージェントに求める振る舞い。
///
/// 適合ケースは「exit code を制御できる」「引数どおりに出力する」といった**意味**だけを
/// 渡し、それを満たすコマンドの組み立てはハーネスに委ねる。プラットフォーム固有の
/// コマンド名やシェルをケースに持ち込まないための口。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentBehavior {
    /// 指定の exit code で終了する。
    Exit(i32),
    /// 標準出力・標準エラーへそれぞれ既知の文字列を書いて成功する。
    Print {
        /// 標準出力へ書く内容。
        stdout: String,
        /// 標準エラーへ書く内容。
        stderr: String,
    },
    /// 作業ディレクトリが指定の worktree なら成功する。
    CheckCwd(WorktreePath),
    /// 受け取った引数を1行ずつ標準出力へ書いて成功する。
    EchoArgs(Vec<String>),
    /// 指定の時間だけ実行を続けてから成功する。
    Sleep(Duration),
    /// exit code を持たない終了(シグナル死等)をする。
    Abort,
}

/// ProcessController の適合スイートが要求する環境。
///
/// スイートは `own_identity` / `run_agent` の [`process_controller::identity_and_agent`] と
/// `spawn_wrapper` の [`process_controller::spawn`] に分かれる。前者はアダプター
/// 単体で閉じ、後者はラッパーモードの実装を要するため適用の時期が違う。
pub trait ProcessControllerHarness {
    /// 検証対象。
    type Controller: ProcessController;

    /// 検証対象を返す。
    fn controller(&self) -> &Self::Controller;

    /// 外部から観測した現在の実時刻(TC-port-process-controller-004)。
    fn observe_wall_clock(&self) -> Option<Timestamp> {
        None
    }

    /// 同定情報の取得機構自体が失敗する状態の実装
    /// (TC-port-process-controller-005)。
    ///
    /// 存在しない取得元を注入した2つ目のコントローラを返す形にすると、権限操作にも
    /// root 実行の可否にも依存せず確定的に走る。
    fn failing_identity_controller(&self) -> Option<&Self::Controller> {
        None
    }

    /// 実在する worktree(`run_agent` の cwd)。
    fn worktree(&self) -> Option<WorktreePath> {
        None
    }

    /// 存在しない worktree(TC-port-process-controller-026)。
    fn missing_worktree(&self) -> Option<WorktreePath> {
        None
    }

    /// 書き込み可能な (標準出力, 標準エラー) のログパス。
    fn log_paths(&self) -> Option<(PathBuf, PathBuf)> {
        None
    }

    /// 開けないログパスと、その制限を戻すハンドル(TC-port-process-controller-025)。
    ///
    /// 制限が実際に効いたことを確認してから `Some` を返す。効かなければ復元して `None`。
    fn unwritable_log_path(&self) -> Option<(PathBuf, Restore)> {
        None
    }

    /// 指定の振る舞いをするテスト用エージェントのコマンド。
    fn agent_command(&self, _behavior: AgentBehavior) -> Option<CommandLine> {
        None
    }

    /// 存在しないコマンド名(TC-port-process-controller-022)。
    fn missing_command(&self) -> Option<CommandLine> {
        None
    }

    /// 実行できない実体を指すコマンド(TC-port-process-controller-023)。
    ///
    /// 制限が実際に効いたことを確認してから `Some` を返す。
    fn non_executable_command(&self) -> Option<CommandLine> {
        None
    }

    /// 指定の振る舞いのエージェントを起動するラッパーの起動仕様
    /// (TC-port-process-controller-001/002/003)。run ディレクトリは用意済み。
    fn launch_spec(&self, _behavior: AgentBehavior) -> Option<WrapperLaunchSpec> {
        None
    }

    /// run ディレクトリに starttime・pid・exit が揃うまで期限つきで待ち、揃ったかを返す
    /// (TC-port-process-controller-001/002)。
    fn wait_for_run_files(&self, _spec: &WrapperLaunchSpec) -> Option<bool> {
        None
    }

    /// 別プロセスから `spawn_wrapper` を呼び、そのプロセスの終了まで待つ
    /// (TC-port-process-controller-002)。
    ///
    /// デタッチ性は「呼び出し側プロセスの終了後もラッパーが完走する」ことなので、
    /// 同一プロセス内では表現できない。
    fn spawn_from_other_process(&self, _spec: &WrapperLaunchSpec) -> Option<()> {
        None
    }

    /// ラッパーの起動自体が不可能な状態の実装(TC-port-process-controller-003)。
    ///
    /// 存在しないパスを自バイナリとして注入した2つ目のコントローラを返す。
    fn failing_controller(&self) -> Option<&Self::Controller> {
        None
    }

    /// run ディレクトリに何も書かれていないか(TC-port-process-controller-003)。
    fn run_dir_is_empty(&self, _spec: &WrapperLaunchSpec) -> Option<bool> {
        None
    }

    /// 終了を確認済みのプロセスのPID(TC-port-process-controller-007)。
    fn terminated_pid(&self) -> Option<Pid> {
        None
    }

    /// 実行単位に属する全プロセスが生存している実行単位
    /// (TC-port-process-controller-011/015)。
    fn live_execution_unit(&self) -> Option<ExecutionUnit> {
        None
    }

    /// spawn 元プロセスが終了済みの実行単位と、新規に構成したコントローラ
    /// (TC-port-process-controller-012)。
    ///
    /// 「プロセス内に保持したハンドルに依存しない」ことは、起動した側とも起動時の
    /// インスタンスとも縁の切れた入力だけで kill できることでしか示せない。
    fn detached_execution_unit(&self) -> Option<(ExecutionUnit, &Self::Controller)> {
        None
    }

    /// ラッパーのみ死亡し、残りのメンバーが実行単位に属したまま生存している実行単位
    /// (TC-port-process-controller-014/016)。
    fn orphaned_execution_unit(&self) -> Option<ExecutionUnit> {
        None
    }

    /// 実行単位への終了操作自体が失敗する状態の実装
    /// (TC-port-process-controller-013/016)。
    ///
    /// 存在しない終了操作の実体を注入した2つ目のコントローラを返す形にすると、権限にも
    /// プラットフォームにも依存せず確定的に走る(取得元の注入と同じ手)。
    fn failing_terminator_controller(&self) -> Option<&Self::Controller> {
        None
    }
}

/// 実行単位1つ分の観測対象。
///
/// 期待結果は契約の語彙(「実行単位に属する全プロセスが終了する」)で書くため、ケースは
/// 同定子とメンバーのPIDだけを受け取り、プラットフォーム固有の機構名に踏み込まない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionUnit {
    /// 永続化された kill 同定子。
    pub kill_ident: KillIdent,
    /// 実行単位に属し、生存が確認されているプロセス。
    pub members: Vec<Pid>,
}

/// テスト用コマンドに求める振る舞い。
///
/// `AgentBehavior` と同じく、ケースは**意味**だけを渡す。標準出力・標準エラーは捕捉されない
/// 契約なので、観測結果は exit code かコマンド自身が書き出すファイルで表す。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandBehavior {
    /// 指定の exit code で終了する。
    Exit(i32),
    /// exit code を持たない終了(シグナル死等)をする。
    Abort,
    /// 受け取った引数が指定のトークン列とリテラル一致するなら 0 で終了する。
    CheckArgs(Vec<String>),
    /// 指定の環境変数が指定の値なら 0 で終了する。
    CheckEnv {
        /// 変数名。
        name: String,
        /// 期待する値。
        value: String,
    },
    /// 作業ディレクトリが指定のパスなら 0 で終了する。
    CheckCwd(PathBuf),
    /// 標準出力・標準エラーへそれぞれ既知の文字列を書いて 0 で終了する。
    Print {
        /// 標準出力へ書く内容。
        stdout: String,
        /// 標準エラーへ書く内容。
        stderr: String,
    },
    /// 指定の時間だけ実行を続けてから 0 で終了する。
    Sleep(Duration),
    /// 指定の時間だけ実行を続け、終了直前に証跡を残して 0 で終了する。
    Record {
        /// 証跡を残すまでの実行時間。
        after: Duration,
        /// 証跡の置き場。
        evidence: PathBuf,
    },
}

/// CommandRunner の適合スイートが要求する環境。
pub trait CommandRunnerHarness {
    /// 検証対象。
    type Runner: CommandRunner;

    /// 検証対象を返す。
    fn runner(&self) -> &Self::Runner;

    /// 指定の振る舞いをするテスト用コマンド。
    fn command(&self, _behavior: CommandBehavior) -> Option<PlainCommand> {
        None
    }

    /// 存在しないコマンド名(TC-port-command-runner-003)。
    fn missing_command(&self) -> Option<PlainCommand> {
        None
    }

    /// 実行できない実体を指すコマンド(TC-port-command-runner-004)。
    ///
    /// 制限が実際に効いたことを確認してから `Some` を返す。
    fn non_executable_command(&self) -> Option<PlainCommand> {
        None
    }

    /// 呼び出しプロセスに既に設定されている環境変数の (名前, 値)
    /// (TC-port-command-runner-008/010)。
    ///
    /// 「設定する」ではなく「既にあるものを教える」形にするのは、実行中プロセスの環境の
    /// 書き換えが安全に行えないため。継承の検証にはどちらでも足りる。
    fn caller_env(&self) -> Option<(String, String)> {
        None
    }

    /// 呼び出しプロセスに設定されていない変数名(TC-port-command-runner-009)。
    fn absent_env_name(&self) -> Option<String> {
        None
    }

    /// 呼び出しプロセスの作業ディレクトリ(TC-port-command-runner-011)。
    fn caller_current_dir(&self) -> Option<PathBuf> {
        None
    }

    /// まだ存在しない証跡の置き場(TC-port-command-runner-012/015)。
    ///
    /// 呼び出しごとに別のパスを返す契約。同じパスを返すと、前のケースの証跡が残って
    /// 「終了させられている」ことの観測が壊れる。
    fn evidence_path(&self) -> Option<PathBuf> {
        None
    }
}

/// RunStore の適合スイートが要求する環境。
pub trait RunStoreHarness {
    /// 検証対象。
    type Store: RunStore;

    /// 検証対象を返す。
    fn store(&self) -> &Self::Store;

    /// `prepare_attempt(id, number)` が返すべき run ディレクトリ
    /// (TC-port-run-store-001)。パスの決定は `RunDirPath::derive` に従う。
    fn expected_run_dir(&self, _id: &TaskId, _number: AttemptNumber) -> Option<RunDirPath> {
        None
    }

    /// attempt ディレクトリ自体が存在するか(TC-port-run-store-001)。
    ///
    /// read 系の `Ok(None)` では「空のディレクトリ」と「ディレクトリごと不在」を区別
    /// できないため、観測を環境に問う。ケースは `prepare_attempt` の**前後で観測が反転
    /// すること**まで主張する — 定数を返す実装はどちらかの側で落ちる。
    fn attempt_dir_present(&self, _run_dir: &RunDirPath) -> Option<bool> {
        None
    }

    /// 指定種別のファイルの位置に解釈不能な内容を直接置く
    /// (TC-port-run-store-006/011/015)。
    fn put_unreadable_content(&self, _run_dir: &RunDirPath, _kind: RunFileKind) -> Option<()> {
        None
    }

    /// 指定種別のファイルは存在するが読み取り自体が失敗する状態にする
    /// (TC-port-run-store-007)。
    ///
    /// 制限が実際に効いたことを確認してから `Some` を返す。効かなければ復元して `None`。
    fn make_unreadable(&self, _run_dir: &RunDirPath, _kind: RunFileKind) -> Option<Restore> {
        None
    }

    /// attempt への書き込みが途中で失敗する状態にする(TC-port-run-store-017)。
    /// 読み取りは残す — 失敗後に従前の値が読めることが行の主張だから。
    ///
    /// 制限が実際に効いたことを確認してから `Some` を返す。効かなければ復元して `None`。
    fn make_attempt_unwritable(&self, _run_dir: &RunDirPath) -> Option<Restore> {
        None
    }

    /// 並行して読み書きできる対象(TC-port-run-store-016)。
    ///
    /// 別スレッドから読み続ける前提を持つケースだけがこのハンドルを使う。
    fn concurrent_store(&self) -> Option<&(dyn RunStore + Sync)> {
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

    /// パスもブランチも未使用のワークスペース(TC-port-worktree-manager-010/016)。
    ///
    /// `ws.path` の親(worktree_root)は存在する。正規化の分岐を必ず通すため、置き場は
    /// シンボリックリンクを経由するパスとして組む。
    fn unused_workspace(&self) -> Option<Workspace> {
        None
    }

    /// worktree_root 自体がまだ存在しないワークスペース
    /// (TC-port-worktree-manager-011)。
    fn workspace_under_missing_root(&self) -> Option<Workspace> {
        None
    }

    /// 登録が無く、コミットの積まれた `ws.branch` だけが存在するワークスペースと、
    /// そのコミットが worktree に置くマーカーの内容(TC-port-worktree-manager-013)。
    fn workspace_with_orphan_branch(&self) -> Option<(Workspace, String)> {
        None
    }

    /// 自タスクのパスとブランチの登録は残るが実体が消えた(`prunable`)ワークスペースと、
    /// 消える前にコミットしたマーカーの内容(台帳行に対応しない追加ケース)。
    fn workspace_with_prunable_registration(&self) -> Option<(Workspace, String)> {
        None
    }

    /// `ws.path` に worktree でない通常のディレクトリがあるワークスペースと、そこに
    /// 置かれている内容(TC-port-worktree-manager-014)。
    fn workspace_over_plain_dir(&self) -> Option<(Workspace, String)> {
        None
    }

    /// `ws.path` に `ws.branch` 以外のブランチの worktree があるワークスペースと、その
    /// worktree に置かれている内容(TC-port-worktree-manager-015)。
    fn workspace_over_other_branch(&self) -> Option<(Workspace, String)> {
        None
    }

    /// worktree の中にマーカーを置く(TC-port-worktree-manager-012)。
    fn put_worktree_marker(&self, _ws: &Workspace, _text: &str) -> Option<()> {
        None
    }

    /// `ws.path` にあるマーカーの内容。**不在は空文字列**として返す — 「観測できない」
    /// (`None`)と区別しないと、消えたことがスキップとして通ってしまう。
    fn worktree_marker(&self, _ws: &Workspace) -> Option<String> {
        None
    }

    /// `ws.path` が実体として存在するか(TC-port-worktree-manager-011/016)。
    fn worktree_present(&self, _ws: &Workspace) -> Option<bool> {
        None
    }

    /// ブランチ先端の同定子(TC-port-worktree-manager-010/013 と追加ケース)。
    fn branch_tip(&self, _branch: &BranchName) -> Option<String> {
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

    conformance_cases!(
        cases,
        not_sync_harness(),
        __PULSEN_CONFORMANCE_FRAMEWORK_SKIPS = Vec::new(),
        [走査するケース]
    );

    #[test]
    fn 宣言した集合に含まれるケースのスキップは報告だけで済む() {
        let budget = SkipBudget::new(vec!["tc_port_clock_005"]);

        CaseOutcome::skipped("rewind").report("tc_port_clock_005_時刻の巻き戻し", &budget);
    }

    #[test]
    #[should_panic(expected = "tc_port_clock_004_時刻の前進")]
    fn 宣言していないケースのスキップはそのケースの失敗になる() {
        let budget = SkipBudget::new(vec!["tc_port_clock_005"]);

        CaseOutcome::skipped("advance").report("tc_port_clock_004_時刻の前進", &budget);
    }

    #[test]
    #[should_panic(expected = "tc_port_clock_0051_")]
    fn 宣言した番号は別の番号のケースには一致しない() {
        let budget = SkipBudget::new(vec!["tc_port_clock_005"]);

        CaseOutcome::skipped("rewind").report("tc_port_clock_0051_別のケース", &budget);
    }

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
