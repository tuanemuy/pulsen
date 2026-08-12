//! Execution ドメインが外界に要求する操作。
//!
//! 宣言するのは、そのメソッドを呼ぶスライスが揃ったものだけにする(未実装メソッドの
//! 宣言とスタブは置かない)。worktree の削除、プロセスの観測・終了、コマンド実行のポートは
//! それらを使うスライスで足す。

use std::path::{Path, PathBuf};

use crate::definition::CommandLine;
use crate::task::{
    AttemptNumber, BranchName, KillIdent, Pid, RepoPath, RunDirPath, StartTimeRecord, TaskId,
    Workspace, WorktreePath,
};

use super::value::{ExitCode, PidFileContent};

/// 機構そのものの失敗の不透明な報告。
///
/// ポートをまたいで共有してよいのは「呼び出し側が分類に使わない報告用のエラー」に限る。
/// 呼び出し側が分岐に使うものは `RunFileError` のように専用の型を持たせる — 条件を
/// 添えずに共有すると、分類の必要なエラーまでこの型に潰れる。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Io {
    /// 機構の失敗。
    Failed {
        /// 原因の説明。
        message: String,
    },
}

/// run ディレクトリのファイル読み取りの失敗。
///
/// 「不在」は `Ok(None)` でありエラーではない。呼び出し側は内容の破損と機構の失敗を
/// 区別して報告する。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunFileError {
    /// 内容を解釈できない。
    Corrupt {
        /// 対象のファイル。
        path: PathBuf,
        /// 原因の説明。
        message: String,
    },
    /// 読み取り機構の失敗。
    Io {
        /// 原因の説明。
        message: String,
    },
}

/// run ディレクトリの読み書き。
///
/// 契約:
/// - `prepare_attempt` は親を含めて attempt ディレクトリを作成し、冪等である。返るパスは
///   `RunDirPath::derive` の導出結果と一致する。既存の内容には触れない
/// - read 系(`read_pid_file` / `read_starttime` / `read_exit`)は、ファイルの不在も
///   **attempt ディレクトリ自体の不在も** `Ok(None)` とする。launching 記録と
///   `prepare_attempt` の間のクラッシュで run ディレクトリが無いまま観測されるのは正常な
///   復旧経路であり、猶予時間経路の分類に合流させる
/// - read 系は内容を解釈できないとき `Corrupt`、読み取り機構の失敗のとき `Io` を返す。
///   どちらも不在の `Ok(None)` とは区別される
/// - write 系(`write_starttime` / `write_pid_file` / `write_exit` /
///   `write_invalidation_marker`)はアトミック置換で行い、並行する読み手が書きかけの内容を
///   観測しない。失敗した場合も部分的な内容を残さない
/// - **write 系はいずれも書き込み先のディレクトリを必要に応じて作る。** `prepare_attempt`
///   が失敗した後でも spawn は行われる設計なので、ラッパーが自力でディレクトリを作って
///   書けることが自己修復の前提になる
/// - `write_invalidation_marker` は冪等で、既にマーカーがあっても `Ok` を返す
/// - 機構の失敗はすべて値(`Err`)として返し、パニックしない
pub trait RunStore {
    /// attempt の run ディレクトリを用意する。
    fn prepare_attempt(&self, id: &TaskId, number: AttemptNumber) -> Result<RunDirPath, Io>;

    /// pid ファイルを読む。
    fn read_pid_file(&self, run_dir: &RunDirPath) -> Result<Option<PidFileContent>, RunFileError>;

    /// starttime ファイルを読む。
    fn read_starttime(&self, run_dir: &RunDirPath)
    -> Result<Option<StartTimeRecord>, RunFileError>;

    /// exit ファイルを読む。
    fn read_exit(&self, run_dir: &RunDirPath) -> Result<Option<ExitCode>, RunFileError>;

    /// 無効化マーカーを書く。
    fn write_invalidation_marker(&self, run_dir: &RunDirPath) -> Result<(), Io>;

    /// 無効化マーカーの有無を返す。
    fn marker_exists(&self, run_dir: &RunDirPath) -> Result<bool, Io>;

    /// starttime ファイルを書く。
    fn write_starttime(&self, run_dir: &RunDirPath, record: &StartTimeRecord) -> Result<(), Io>;

    /// pid ファイルを書く。
    fn write_pid_file(&self, run_dir: &RunDirPath, content: &PidFileContent) -> Result<(), Io>;

    /// exit ファイルを書く。
    fn write_exit(&self, run_dir: &RunDirPath, code: &ExitCode) -> Result<(), Io>;
}

/// ラッパーモードの起動引数へ直列化される値。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WrapperLaunchSpec {
    run_dir: RunDirPath,
    agent_cmd: CommandLine,
    workspace: WorktreePath,
}

impl WrapperLaunchSpec {
    /// ラッパーに渡す3値の組を作る。
    pub fn new(run_dir: RunDirPath, agent_cmd: CommandLine, workspace: WorktreePath) -> Self {
        Self {
            run_dir,
            agent_cmd,
            workspace,
        }
    }

    /// attempt の run ディレクトリ。
    pub fn run_dir(&self) -> &RunDirPath {
        &self.run_dir
    }

    /// 起動するエージェントのコマンド。
    pub fn agent_cmd(&self) -> &CommandLine {
        &self.agent_cmd
    }

    /// エージェントの作業ディレクトリになる worktree。
    pub fn workspace(&self) -> &WorktreePath {
        &self.workspace
    }
}

/// ラッパーが観測した自身の同定情報一式。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WrapperIdentity {
    pid: Pid,
    kill_ident: KillIdent,
    starttime: StartTimeRecord,
}

impl WrapperIdentity {
    /// 同定情報一式を作る。
    pub fn new(pid: Pid, kill_ident: KillIdent, starttime: StartTimeRecord) -> Self {
        Self {
            pid,
            kill_ident,
            starttime,
        }
    }

    /// 自プロセスのプロセスID。
    pub fn pid(&self) -> Pid {
        self.pid
    }

    /// kill 同定子。
    pub fn kill_ident(&self) -> &KillIdent {
        &self.kill_ident
    }

    /// 記録された起動時刻。
    pub fn starttime(&self) -> &StartTimeRecord {
        &self.starttime
    }
}

/// ラッパーの起動自体の失敗。
///
/// 分類には使わない報告用のエラー。呼び出し側はこの失敗で状態を変更しない
/// (launching 記録は済んでおり、猶予時間経路が分類する)。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpawnError {
    /// OS レベルの起動失敗。
    Failed {
        /// 原因の説明。
        message: String,
    },
}

/// プロセスの起動と観測。
///
/// 契約:
/// - `spawn_wrapper` はツール自身のバイナリをラッパーモードで、新しいプロセスグループ相当の
///   単位でデタッチ起動する。呼び出し側プロセスが終了してもラッパーは生存して実行を完了する
/// - `spawn_wrapper` は**起動後の成否に関知しない**。`Ok` は起動できたことだけを意味し、
///   実行の結末は run ディレクトリ経由で観測される。起動自体が不可能なら `Err(SpawnError)` を
///   返し、run ディレクトリにもプロセスにも副作用を残さない
/// - `own_identity` は自プロセスの pid・kill 同定子・起動時刻を返す。kill 同定子は
///   ツールの再起動後もタスクファイルの情報だけで kill を実行できる形式とする。取得機構が
///   失敗したときは `Err(Io)` を返し、不正な同定情報で `Ok` を装わない
/// - `run_agent` は**失敗しない** — cwd はつねに worktree、標準出力・標準エラーは指定パスへ
///   リダイレクトし、起動不能は 127 / 126、シグナル死は 128+n の符号化値として常に
///   `ExitCode` を返す。リダイレクト先を開けない場合はエージェントを起動せず 126 を返す
/// - `run_agent` はシェルを介さずに直接起動する(引数はリテラルのまま渡る)。呼び出しは
///   コマンドの終了まで戻らない
pub trait ProcessController {
    /// ツール自身のバイナリをラッパーモードでデタッチ起動する。
    fn spawn_wrapper(&self, spec: &WrapperLaunchSpec) -> Result<(), SpawnError>;

    /// 自プロセスの同定情報を取得する。
    fn own_identity(&self) -> Result<WrapperIdentity, Io>;

    /// エージェントを同期実行する。
    fn run_agent(
        &self,
        cmd: &CommandLine,
        cwd: &WorktreePath,
        stdout: &Path,
        stderr: &Path,
    ) -> ExitCode;
}

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

/// worktree 操作の失敗。
///
/// 内訳は分類に使わない不透明な `message` でよい — 呼び出し側は失敗要因の記録
/// (`record_tool_failure`)にしか使わない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorktreeError {
    /// git 操作の失敗。
    Failed {
        /// 原因の説明。
        message: String,
    },
}

/// 対象(リポジトリ・ブランチ)の検証と worktree の作成。
///
/// 契約:
/// - 検証系(`validate_repo` / `head_branch` / `branch_exists`)は読み取りのみで、
///   リポジトリの内容を変更しない。分類(`NotFound` / `NotARepository` / `DetachedHead` /
///   `EmptyRepository`)と実行環境のエラー(`Failed`)を区別して値で返す
/// - `create` は `ws.path` に `ws.branch` の worktree を用意する。`ws.branch` が無ければ
///   `base` の先端から作る。`ws.path` の親(worktree_root)が存在しなければ作成する
/// - **`create` の冪等性は「`ws.path` に `ws.branch` の worktree がある」場合に限る。**
///   達成済みなら worktree の内容に一切触れずに `Ok` を返す。パスの存在だけを達成済みと
///   みなさない — 別ブランチの worktree を掴んだまま起動すると他タスクの成果を壊す
/// - 自タスクの残骸(実体が消えた登録・登録の消えたブランチ)からは張り直して `Ok` を
///   返す。いずれもブランチの先端は変えない(積まれたコミットを失わない)
/// - worktree でない通常のディレクトリ・別ブランチの worktree・`base` の不在はいずれも
///   `Failed` とし、既存の内容を自動修復しない
/// - ブランチのライフサイクルには関与しない(削除もしない)
pub trait WorktreeManager {
    /// 対象が git リポジトリであることを検証する。
    fn validate_repo(&self, repo: &RepoPath) -> Result<(), TargetError>;

    /// HEAD が指すブランチ名を取得する。
    fn head_branch(&self, repo: &RepoPath) -> Result<BranchName, TargetError>;

    /// 指定ブランチの存在有無を返す。
    fn branch_exists(&self, repo: &RepoPath, branch: &BranchName) -> Result<bool, TargetError>;

    /// タスクの worktree を用意する。
    fn create(
        &self,
        repo: &RepoPath,
        base: &BranchName,
        ws: &Workspace,
    ) -> Result<(), WorktreeError>;
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
