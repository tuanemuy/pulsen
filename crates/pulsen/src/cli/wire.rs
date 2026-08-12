//! 合成ルート。
//!
//! ホームの解決・カレントディレクトリの取得・アダプターの構築と結線をここだけで行う
//! (ADR-030 / ADR-031)。アプリケーション層はここから渡されたポートしか知らない。

use std::env;
use std::io;
use std::path::{Path, PathBuf};

use pulsen_domain::definition::{ConfigLoadError, ConfigStore, GlobalConfig};
use pulsen_domain::task::{AbsolutePathError, RunDirPath, StateRoot, WorktreeRoot};

use crate::adapter::clock::SystemClock;
use crate::adapter::config_store::FsConfigStore;
use crate::adapter::lock::FileExclusiveLock;
use crate::adapter::process::{IdentitySource, SystemProcessController};
use crate::adapter::run_store::FsRunStore;
use crate::adapter::task_id::{DefaultTaskIdGenerator, IdGeneratorInitError};
use crate::adapter::task_repository::FsTaskRepository;
use crate::adapter::workflow_store::FsWorkflowStore;
use crate::adapter::worktree::GitCliWorktreeManager;
use crate::application::home::PulsenHome;

/// グローバルホームを指す環境変数。
const HOME_ENV: &str = "PULSEN_HOME";
/// 既定のグローバルホームのディレクトリ名(ユーザーのホーム直下)。
const DEFAULT_HOME_DIR: &str = ".pulsen";
/// 既定の git 実行ファイル。PATH から解決される。
const DEFAULT_GIT_PROGRAM: &str = "git";

/// 結線そのものの失敗。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WireError {
    /// ホームを決められない(フラグ・環境変数の指定がなく、ユーザーのホームも不明)。
    HomeUnresolvable,
    /// 指定されたホームを絶対パスにできない。
    HomeUnusable {
        /// 与えられたパス。
        given: PathBuf,
        /// 原因の説明。
        message: String,
    },
    /// カレントディレクトリを取得できない。
    CurrentDirUnavailable {
        /// 原因の説明。
        message: String,
    },
    /// `--repo` を絶対パスにできない。
    RepoPathUnusable {
        /// 与えられたパス。
        given: PathBuf,
        /// 原因の説明。
        message: String,
    },
    /// グローバル設定を読み込めない。
    Config {
        /// 読もうとしたパス(案内に使う)。
        config_path: PathBuf,
        /// 読み込みの失敗。
        error: ConfigLoadError,
    },
    /// タスクIDの発行を初期化できない。
    IdGenerator {
        /// 原因の説明。
        message: String,
    },
    /// 自身の実行ファイルのパスを取得できない。
    SelfExeUnavailable {
        /// 原因の説明。
        message: String,
    },
    /// run ディレクトリから状態のルートを復元できない。
    RunDirUnusable {
        /// 与えられた run ディレクトリ。
        given: PathBuf,
    },
}

/// 結線済みのアダプターと、起動時に読み込んだグローバル設定。
pub struct Runtime {
    config: GlobalConfig,
    state_root: StateRoot,
    worktree_root: WorktreeRoot,
    workflows: FsWorkflowStore,
    worktrees: GitCliWorktreeManager,
    ids: DefaultTaskIdGenerator<SystemClock>,
    clock: SystemClock,
    tasks: FsTaskRepository,
    runs: FsRunStore,
    lock: FileExclusiveLock,
}

impl Runtime {
    /// 読み込み済みのグローバル設定。
    pub fn config(&self) -> &GlobalConfig {
        &self.config
    }

    /// 状態のルート。
    ///
    /// tick は run ディレクトリのパスをここから決定的に導出する。導出はドメイン
    /// (`RunDirPath::derive`)が行うため、合成ルートはルートの値だけを渡す。
    pub fn state_root(&self) -> &StateRoot {
        &self.state_root
    }

    /// worktree のルート。
    ///
    /// tick はタスクIDからワークスペースを導出する(`WorkspacePlanner::derive`)。
    pub fn worktree_root(&self) -> &WorktreeRoot {
        &self.worktree_root
    }

    /// ワークフロー定義のストア。
    pub fn workflows(&self) -> &FsWorkflowStore {
        &self.workflows
    }

    /// 対象の検証。
    pub fn worktrees(&self) -> &GitCliWorktreeManager {
        &self.worktrees
    }

    /// タスクIDの発行。
    pub fn ids(&self) -> &DefaultTaskIdGenerator<SystemClock> {
        &self.ids
    }

    /// 時刻。
    pub fn clock(&self) -> &SystemClock {
        &self.clock
    }

    /// タスクの永続化。
    pub fn tasks(&self) -> &FsTaskRepository {
        &self.tasks
    }

    /// run ディレクトリの読み書き。
    pub fn runs(&self) -> &FsRunStore {
        &self.runs
    }

    /// 排他ロック。
    pub fn lock(&self) -> &FileExclusiveLock {
        &self.lock
    }

    /// 相対パスをカレントディレクトリ基準で絶対化する。
    ///
    /// ユースケースには絶対パスだけが入る(ADR-030)。存在の検証はポートが行う。
    pub fn absolute_repo(&self, repo: PathBuf) -> Result<PathBuf, WireError> {
        absolute(&repo).map_err(|error| WireError::RepoPathUnusable {
            given: repo,
            message: error.to_string(),
        })
    }
}

/// グローバルホームを解決し、アダプターを構築してグローバル設定を読み込む。
///
/// 起動時のグローバル設定の読み込みは全コマンド共通(pages ※1)なので、ここで行う。
pub fn compose(home_flag: Option<PathBuf>) -> Result<Runtime, WireError> {
    let home = resolve_home(home_flag)?;
    let base_dir = env::current_dir().map_err(|error| WireError::CurrentDirUnavailable {
        message: error.to_string(),
    })?;

    let config_path = home.config_path();
    let config = FsConfigStore::new(config_path.clone(), home.path().to_path_buf())
        .load()
        .map_err(|error| WireError::Config { config_path, error })?;

    let ids = DefaultTaskIdGenerator::new(SystemClock::new()).map_err(|error| {
        WireError::IdGenerator {
            message: id_generator_cause(&error),
        }
    })?;

    Ok(Runtime {
        workflows: FsWorkflowStore::new(home.workflows_dir(), base_dir),
        worktrees: GitCliWorktreeManager::new(PathBuf::from(DEFAULT_GIT_PROGRAM)),
        ids,
        clock: SystemClock::new(),
        tasks: FsTaskRepository::new(home.state_root().clone()),
        runs: FsRunStore::new(home.state_root().clone()),
        lock: FileExclusiveLock::new(home.lock_path()),
        state_root: home.state_root().clone(),
        worktree_root: home.worktree_root().clone(),
        config,
    })
}

/// ラッパーモードだけが使うアダプター。
///
/// ホームも config も読まない — ラッパーが必要とする情報はすべて起動引数で受け取る
/// (ADR-070)。`Runtime` とは別の型にすることで、ラッパーの経路にホーム解決や設定の
/// 読み込みが後から紛れ込まないようにする。
pub struct WrapperRuntime {
    runs: FsRunStore,
    processes: SystemProcessController,
}

impl WrapperRuntime {
    /// run ディレクトリの読み書き。
    pub fn runs(&self) -> &FsRunStore {
        &self.runs
    }

    /// プロセスの起動と観測。
    pub fn processes(&self) -> &SystemProcessController {
        &self.processes
    }
}

/// run ディレクトリだけからラッパー用のアダプターを組む。
///
/// `--home` は `global = true` なので `wrapper` にも付くが、ラッパーはこれを使わない。
/// tick が spawn したラッパーは `--home` を受け取らないため、ホームを解決すると既定の
/// `~/.pulsen` へ落ちる。値が使われないことに依存した配線は次の変更で壊れる。
///
/// `current_exe()` も読まない — ラッパーが呼ぶのは自身の同定情報の取得とエージェントの
/// 起動だけで、自バイナリのパスを要するのはラッパーを spawn する側だけ(ADR-068)。
pub fn compose_wrapper(run_dir: &RunDirPath) -> Result<WrapperRuntime, WireError> {
    let state_root = run_dir
        .state_root()
        .ok_or_else(|| WireError::RunDirUnusable {
            given: run_dir.as_path().to_path_buf(),
        })?;

    Ok(WrapperRuntime {
        runs: FsRunStore::new(state_root),
        processes: SystemProcessController::without_self_exe(
            IdentitySource::platform_default(),
            SystemClock::new(),
        ),
    })
}

/// 自バイナリのパスと同定情報の取得元を解決してプロセス操作を組む。
///
/// `compose` には載せない — `ProcessController` を要するのはプロセスを起動する経路
/// だけであり、`current_exe()` の失敗で `add` が落ちるのは筋が通らない(ADR-068)。
pub fn process_controller() -> Result<SystemProcessController, WireError> {
    let self_exe = env::current_exe().map_err(|error| WireError::SelfExeUnavailable {
        message: error.to_string(),
    })?;
    Ok(SystemProcessController::new(
        self_exe,
        IdentitySource::platform_default(),
        SystemClock::new(),
    ))
}

/// タスクID発行の初期化に失敗した原因。
///
/// 乱数を取れないというアダプター固有の事情はここで説明に畳む。アダプターへの依存を
/// 合成ルートの1箇所に閉じるため、文言層にはアダプターの型を渡さない。
fn id_generator_cause(error: &IdGeneratorInitError) -> String {
    match error {
        IdGeneratorInitError::Entropy { message } => format!("乱数を取得できない: {message}"),
        IdGeneratorInitError::InvalidFormat(_) => "IDの組み立て規則が制約を満たさない".to_owned(),
    }
}

/// `--home` > `PULSEN_HOME` > `~/.pulsen/` の順で解決する(pages 共通事項)。
fn resolve_home(flag: Option<PathBuf>) -> Result<PulsenHome, WireError> {
    if let Some(path) = flag {
        return home_from(path);
    }
    // 空文字は未設定と同義に扱う。空パスはホームとして解決できず、既定へ落ちるほうが
    // 「変数を消し忘れた」状況に対する挙動として素直なため。
    if let Some(value) = env::var_os(HOME_ENV)
        && !value.is_empty()
    {
        return home_from(PathBuf::from(value));
    }
    let default = env::home_dir()
        .ok_or(WireError::HomeUnresolvable)?
        .join(DEFAULT_HOME_DIR);
    home_from(default)
}

/// 与えられたホームを絶対化して配置を導出する。
fn home_from(given: PathBuf) -> Result<PulsenHome, WireError> {
    let absolute = absolute(&given).map_err(|error| WireError::HomeUnusable {
        given: given.clone(),
        message: error.to_string(),
    })?;
    PulsenHome::new(absolute).map_err(|AbsolutePathError::NotAbsolute { given }| {
        WireError::HomeUnusable {
            given,
            message: "絶対パスにならない".to_owned(),
        }
    })
}

/// 相対パスをカレントディレクトリ基準の絶対パスにする(ファイルシステムは参照しない)。
fn absolute(path: &Path) -> io::Result<PathBuf> {
    std::path::absolute(path)
}
