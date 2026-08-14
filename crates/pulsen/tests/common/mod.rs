//! 統合テストが共有するフィクスチャ。
//!
//! 統合テストは必要なフィクスチャだけを使うため、テストごとに未使用の項目が残る。
#![allow(dead_code)]

use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::LazyLock;
use std::time::{Duration, Instant};

use pulsen::adapter::process;
use pulsen_conformance::{Restore, SkipBudget};
use serde_json::Value;
use tempfile::TempDir;

pub mod git;
pub mod lock;

/// グローバルホームを指す環境変数。
pub const HOME_ENV: &str = "PULSEN_HOME";

/// ユーザーのホームディレクトリを指す環境変数(プラットフォームで名前が違う)。
const USER_HOME_ENV: [&str; 2] = ["HOME", "USERPROFILE"];

/// グローバルホームの直下にある状態の置き場。書き込み系が必要に応じて自動作成する。
const STATE_DIR: &str = "state";

/// 権限制限が効かない環境(root 実行・非 POSIX 等)でのみスキップされるケース。
const PERMISSION_CASES: [&str; 4] = [
    "tc_task_register_task_016",
    "tc_task_register_task_021",
    "tc_exec_run_wrapper_014",
    "tc_exec_run_wrapper_016",
];

/// 保持プロセスの合図が期限内に返らない環境でのみスキップされるケース。
const LOCK_HOLDER_CASES: [&str; 1] = ["tc_task_register_task_017"];

/// 一時ディレクトリ自体が git リポジトリ配下にある環境でのみスキップされるケース。
const OUTSIDE_REPOSITORY_CASES: [&str; 1] = ["tc_task_register_task_036"];

/// この環境でスキップを許容するケース。
static SKIPS: LazyLock<SkipBudget> = LazyLock::new(|| SkipBudget::new(allowed_skips()));

/// 前提を作れるかどうかを実際に調べて、許容するケースを決める。
fn allowed_skips() -> Vec<&'static str> {
    let mut allowed = Vec::new();
    if !pulsen_conformance::permission_restrictions_effective() {
        allowed.extend(PERMISSION_CASES);
    }
    // 許容するのは、スキップの宣言だけで「なぜ走らなかったか」と「次に何をすればよいか」が
    // 定まる能力に限る(ADR-073)。実行ファイルが無い場合と起動できない場合は定まらないので、
    // 緑にせずケースの失敗にする。
    match lock::holder_capability() {
        lock::HolderCapability::SignalTimedOut => allowed.extend(LOCK_HOLDER_CASES),
        lock::HolderCapability::Available(_)
        | lock::HolderCapability::ProgramMissing
        | lock::HolderCapability::ProgramUnusable(_) => {}
    }
    if !git::tmpdir_outside_repository() {
        allowed.extend(OUTSIDE_REPOSITORY_CASES);
    }
    allowed
}

/// フィクスチャが前提を用意できなかったことを記録する。
///
/// libtest は成功したテストの標準出力を握り潰すため、`println!` して `return` する形では
/// スキップと成功を区別できない。適合スイートと同じ宣言(`SkipBudget`)を受け入れテストにも
/// 使い、宣言していないケースのスキップはそのケース自身の失敗として現れるようにする
/// (ADR-055)。
pub fn skipped(case: &str, fixture: &'static str) {
    SKIPS.record(case, fixture);
}

/// `claude` エージェントだけを定義したグローバル設定。
pub const CONFIG: &str = "\
agents:
  claude:
    cmd: claude {input}
";

/// 期限つきポーリングの既定の期限。
///
/// 負荷の高い環境でも spawn からファイル出現までが収まる余裕を取る。flaky が再発した
/// ときに動かす起点をこの1箇所にする。
pub const WAIT_TIMEOUT: Duration = Duration::from_secs(30);

/// 期限つきポーリングの間隔。
const POLL_INTERVAL: Duration = Duration::from_millis(50);

/// エージェント実行1件とクリーンアップ1件を持つ定義。
pub const WORKFLOW: &str = "\
agent: claude
initial: queued
statuses:
  queued:
    prompt: 実装して
    next: done
  done:
    run: cleanup
";

/// 一時ディレクトリに置いたグローバルホーム。
///
/// `state/` は作らない — 書き込み系が必要に応じて自動作成する領域であり、
/// フィクスチャが先回りして作ると自動作成の検証ができなくなる。
pub struct Home {
    dir: TempDir,
}

impl Home {
    /// 有効なグローバル設定を置いたホーム。
    pub fn new() -> Self {
        let home = Self::uninitialized();
        home.write_config(CONFIG);
        home
    }

    /// config.yaml を置かないホーム。
    pub fn uninitialized() -> Self {
        let dir = tempfile::tempdir().expect("一時ホームを作れる");
        fs::create_dir_all(dir.path().join("workflows")).expect("workflows を作れる");
        Self { dir }
    }

    /// 解決後のホームパス。
    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    /// グローバル設定のパス。
    pub fn config_path(&self) -> PathBuf {
        self.path().join("config.yaml")
    }

    /// グローバル設定を置く(既存があれば置き換える)。
    pub fn write_config(&self, text: &str) {
        fs::write(self.config_path(), text).expect("config.yaml を書ける");
    }

    /// グローバル設定を取り除く。
    pub fn remove_config(&self) {
        fs::remove_file(self.config_path()).expect("config.yaml を消せる");
    }

    /// 名前で解決されるワークフロー定義のパス `workflows/<name>.yaml`。
    pub fn workflow_path(&self, name: &str) -> PathBuf {
        self.workflows_dir().join(format!("{name}.yaml"))
    }

    /// 名前で解決されるワークフロー定義を置く。
    pub fn write_workflow(&self, name: &str, text: &str) -> PathBuf {
        let path = self.workflow_path(name);
        fs::write(&path, text).expect("ワークフロー定義を書ける");
        path
    }

    /// `workflows/` にファイル名を指定して置く(`.yml` 等の検証に使う)。
    pub fn write_workflow_file(&self, file_name: &str, text: &str) -> PathBuf {
        let path = self.workflows_dir().join(file_name);
        fs::write(&path, text).expect("ワークフロー定義を書ける");
        path
    }

    /// 名前解決の基点。
    pub fn workflows_dir(&self) -> PathBuf {
        self.path().join("workflows")
    }

    /// 状態のルート。
    pub fn state_dir(&self) -> PathBuf {
        self.path().join(STATE_DIR)
    }

    /// 現役タスクのディレクトリ。
    pub fn tasks_dir(&self) -> PathBuf {
        self.state_dir().join("tasks")
    }

    /// アーカイブ済みタスクのディレクトリ。
    pub fn archive_dir(&self) -> PathBuf {
        self.state_dir().join("archive")
    }

    /// run ディレクトリのルート。
    pub fn runs_dir(&self) -> PathBuf {
        self.state_dir().join("runs")
    }

    /// attempt の run ディレクトリ `state/runs/<task-id>/attempt-<n>`。
    pub fn run_dir(&self, id: &str, attempt: u32) -> PathBuf {
        self.runs_dir().join(id).join(format!("attempt-{attempt}"))
    }

    /// worktree のルート。
    pub fn worktrees_dir(&self) -> PathBuf {
        self.path().join("worktrees")
    }

    /// タスクの worktree `worktrees/<task-id>`。
    pub fn worktree(&self, id: &str) -> PathBuf {
        self.worktrees_dir().join(id)
    }

    /// 現役タスクファイルのパス。
    pub fn task_path(&self, id: &str) -> PathBuf {
        self.tasks_dir().join(format!("{id}.json"))
    }

    /// ちょうど1件だけ作られたタスクのID。
    pub fn only_task_id(&self) -> String {
        self.only_task()["task_id"]
            .as_str()
            .expect("タスクIDは文字列である")
            .to_owned()
    }

    /// タスクファイルの内容。
    pub fn task(&self, id: &str) -> Value {
        let bytes = fs::read(self.task_path(id)).expect("タスクファイルを読める");
        serde_json::from_slice(&bytes).expect("タスクファイルは JSON である")
    }

    /// タスクファイルを読み、書き換えて置き直す。
    ///
    /// 本スライスの CLI(`add` / `tick`)だけでは作れない前提(失敗確定・破損した
    /// スナップショット・ワークスペース未確定のまま残った残骸)を作るために使う。
    /// DTO を組み直さず実物を書き換えるので、直列化形式の知識がテストに漏れない。
    pub fn patch_task(&self, id: &str, patch: impl FnOnce(&mut Value)) {
        let mut task = self.task(id);
        patch(&mut task);
        let text = serde_json::to_vec_pretty(&task).expect("JSON に符号化できる");
        fs::write(self.task_path(id), text).expect("タスクファイルを書ける");
    }

    /// 任意の内容のファイルを現役タスクの置き場に置く(パース不能なファイルを混ぜる)。
    pub fn write_raw_task(&self, id: &str, text: &str) -> PathBuf {
        fs::create_dir_all(self.tasks_dir()).expect("置き場を作れる");
        let path = self.task_path(id);
        fs::write(&path, text).expect("タスクファイルを書ける");
        path
    }

    /// タスクファイルをアーカイブへ直接移す(走査対象から外れることの観測に使う)。
    pub fn move_task_to_archive(&self, id: &str) -> PathBuf {
        fs::create_dir_all(self.archive_dir()).expect("アーカイブの置き場を作れる");
        let to = self.archive_dir().join(format!("{id}.json"));
        fs::rename(self.task_path(id), &to).expect("アーカイブへ移せる");
        to
    }

    /// 排他ロックのパス。
    pub fn lock_path(&self) -> PathBuf {
        self.state_dir().join("lock")
    }

    /// 現役タスクのファイル一覧(タスクIDの昇順)。
    pub fn task_files(&self) -> Vec<PathBuf> {
        let Ok(entries) = fs::read_dir(self.tasks_dir()) else {
            return Vec::new();
        };
        let mut paths: Vec<PathBuf> = entries
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|path| path.extension() == Some(OsStr::new("json")))
            .collect();
        paths.sort();
        paths
    }

    /// 現役タスクの内容一覧。
    pub fn tasks(&self) -> Vec<Value> {
        self.task_files()
            .iter()
            .map(|path| {
                let bytes = fs::read(path).expect("タスクファイルを読める");
                serde_json::from_slice(&bytes).expect("タスクファイルは JSON である")
            })
            .collect()
    }

    /// ちょうど1件だけ作られたタスクの内容。
    pub fn only_task(&self) -> Value {
        let mut tasks = self.tasks();
        assert_eq!(tasks.len(), 1, "タスクがちょうど1件作られている");
        tasks.remove(0)
    }

    /// タスクが1件も作られていないか。
    pub fn has_no_task(&self) -> bool {
        self.task_files().is_empty()
    }

    /// 利用者が用意するリソース(グローバル設定とワークフロー定義)のパス。
    pub fn resources(&self) -> Vec<PathBuf> {
        let mut paths = vec![self.config_path()];
        if let Ok(entries) = fs::read_dir(self.workflows_dir()) {
            paths.extend(entries.filter_map(|entry| entry.ok().map(|entry| entry.path())));
        }
        paths.sort();
        paths
    }

    /// グローバル設定とワークフロー定義の現在の内容と、置き場のエントリ一覧を控える。
    pub fn untouched(&self) -> Untouched {
        Untouched::of(self.resources())
            .with_listings([self.workflows_dir(), self.path().to_path_buf()])
    }
}

/// 実行の前後で内容が変わらないことを確かめる対象。
///
/// 「読めないリソースには書き込まない」(PAGE-common-006 規則2)の観測可能な帰結を、
/// 利用者が用意したファイルのバイト列と、置き場のエントリ一覧で確かめる。一覧を見ない
/// と、既存ファイルを書き換えない代わりに新しいファイルを増やす実装を見逃す。
pub struct Untouched {
    entries: Vec<(PathBuf, Option<Vec<u8>>)>,
    listings: Vec<(PathBuf, Vec<PathBuf>)>,
}

impl Untouched {
    /// 現在の内容を控える。存在しないパスは「不在」として控える。
    pub fn of(paths: impl IntoIterator<Item = PathBuf>) -> Self {
        let entries = paths
            .into_iter()
            .map(|path| {
                let content = fs::read(&path).ok();
                (path, content)
            })
            .collect();
        Self {
            entries,
            listings: Vec::new(),
        }
    }

    /// ホームの外に置いたリソースも控える。
    pub fn with_entries(mut self, paths: impl IntoIterator<Item = PathBuf>) -> Self {
        self.entries.extend(paths.into_iter().map(|path| {
            let content = fs::read(&path).ok();
            (path, content)
        }));
        self
    }

    /// ディレクトリ直下のエントリ一覧も控える。
    pub fn with_listings(mut self, dirs: impl IntoIterator<Item = PathBuf>) -> Self {
        self.listings = dirs
            .into_iter()
            .map(|dir| {
                let entries = listed_entries(&dir);
                (dir, entries)
            })
            .collect();
        self
    }

    /// 控えた時点から内容もエントリの顔ぶれも変わっていないことを確かめる。
    pub fn assert_unchanged(&self) {
        for (path, expected) in &self.entries {
            assert_eq!(
                fs::read(path).ok().as_deref(),
                expected.as_deref(),
                "{} の内容が変わっている",
                path.display()
            );
        }
        for (dir, expected) in &self.listings {
            assert_eq!(
                &listed_entries(dir),
                expected,
                "{} のエントリの顔ぶれが変わっている",
                dir.display()
            );
        }
    }
}

/// ディレクトリ直下のエントリ一覧(パスの昇順)。
///
/// ディレクトリも数える — ファイルだけを見ると、新しいディレクトリを作ってその中に書く
/// 書き込みを見逃す。除くのは `state/` だけで、これは書き込み系が必要に応じて自動作成する
/// 管理領域であり、利用者が用意したリソースの不変とは別の話になる。
fn listed_entries(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.file_name() != Some(OsStr::new(STATE_DIR)))
        .collect();
    paths.sort();
    paths
}

/// git リポジトリのフィクスチャ。
pub struct Repo {
    dir: TempDir,
}

impl Repo {
    /// `main` にコミットが1つあるリポジトリ。
    pub fn with_commit() -> Self {
        let repo = Self::empty();
        git::commit(repo.path(), "README.md").expect("コミットできる");
        repo
    }

    /// コミットのない空のリポジトリ。
    pub fn empty() -> Self {
        let dir = tempfile::tempdir().expect("一時ディレクトリを作れる");
        git::init_repo(dir.path()).expect("git リポジトリを作れる");
        Self { dir }
    }

    /// HEAD がブランチを指していないリポジトリ。
    pub fn detached() -> Self {
        let repo = Self::with_commit();
        git::detach_head(repo.path()).expect("HEAD を切り離せる");
        repo
    }

    /// ブランチを追加する(HEAD は動かさない)。
    pub fn create_branch(&self, name: &str) -> &Self {
        git::create_branch(self.path(), name).expect("ブランチを作れる");
        self
    }

    /// リポジトリのパス。
    pub fn path(&self) -> &Path {
        self.dir.path()
    }
}

/// 一時ディレクトリ。相対パス指定のカレントディレクトリや、ホーム外の定義ファイル置き場に使う。
pub fn scratch() -> TempDir {
    tempfile::tempdir().expect("一時ディレクトリを作れる")
}

/// `pulsen add` の1回の実行。
pub struct Add {
    home_flag: Option<OsString>,
    home_env: Option<OsString>,
    user_home: Option<OsString>,
    workflow: OsString,
    repo: OsString,
    base: Option<OsString>,
    cwd: Option<PathBuf>,
}

/// `pulsen add --workflow <workflow> --repo <repo>` を組み立てる。
pub fn add(workflow: impl AsRef<OsStr>, repo: impl AsRef<OsStr>) -> Add {
    Add {
        home_flag: None,
        home_env: None,
        user_home: None,
        workflow: workflow.as_ref().to_owned(),
        repo: repo.as_ref().to_owned(),
        base: None,
        cwd: None,
    }
}

impl Add {
    /// `--home` フラグを付ける。
    pub fn home(mut self, path: &Path) -> Self {
        self.home_flag = Some(path.as_os_str().to_owned());
        self
    }

    /// 環境変数 `PULSEN_HOME` を与える。
    pub fn home_env(mut self, path: &Path) -> Self {
        self.home_env = Some(path.as_os_str().to_owned());
        self
    }

    /// 既定のホーム `~/.pulsen/` の基点になるユーザーのホームディレクトリを与える。
    ///
    /// 既定へ落ちる経路の観測に使う。指定しなくても実ユーザーのホームには落ちない
    /// (`run` が毎回一時ディレクトリを向ける)が、そこを覗くにはパスが要る。
    pub fn user_home(mut self, path: &Path) -> Self {
        self.user_home = Some(path.as_os_str().to_owned());
        self
    }

    /// `--base` を付ける。
    pub fn base(mut self, branch: impl AsRef<OsStr>) -> Self {
        self.base = Some(branch.as_ref().to_owned());
        self
    }

    /// 起動時のカレントディレクトリ。相対パスの解決基準になる。
    pub fn cwd(mut self, dir: &Path) -> Self {
        self.cwd = Some(dir.to_path_buf());
        self
    }

    /// 実バイナリを起動して結果を集める。
    pub fn run(self) -> Run {
        let mut command = Command::new(env!("CARGO_BIN_EXE_pulsen"));
        let sandbox = detached_home(&mut command);
        if let Some(home) = self.home_env {
            command.env(HOME_ENV, home);
        }
        if let Some(home) = self.user_home {
            for variable in USER_HOME_ENV {
                command.env(variable, &home);
            }
        }
        if let Some(dir) = self.cwd {
            command.current_dir(dir);
        }
        if let Some(home) = self.home_flag {
            command.arg("--home").arg(home);
        }
        command
            .arg("add")
            .arg("--workflow")
            .arg(self.workflow)
            .arg("--repo")
            .arg(self.repo);
        if let Some(base) = self.base {
            command.arg("--base").arg(base);
        }

        let output = command.output().expect("pulsen を起動できる");
        drop(sandbox);
        Run {
            code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        }
    }
}

/// `pulsen tick` の1回の実行。
pub struct Tick {
    home_flag: Option<OsString>,
    cwd: Option<PathBuf>,
}

/// `pulsen tick` を組み立てる。
pub fn tick() -> Tick {
    Tick {
        home_flag: None,
        cwd: None,
    }
}

impl Tick {
    /// `--home` フラグを付ける。
    pub fn home(mut self, path: &Path) -> Self {
        self.home_flag = Some(path.as_os_str().to_owned());
        self
    }

    /// 起動時のカレントディレクトリ。
    ///
    /// tick は外部スケジューラーから任意の作業ディレクトリで起動される。帳簿がすべて
    /// 絶対パスで閉じていれば、対象リポジトリの外から起動しても結果は変わらない。
    pub fn cwd(mut self, dir: &Path) -> Self {
        self.cwd = Some(dir.to_path_buf());
        self
    }

    /// 実バイナリを起動して結果を集める。
    pub fn run(self) -> Run {
        let mut command = Command::new(env!("CARGO_BIN_EXE_pulsen"));
        let sandbox = detached_home(&mut command);
        if let Some(dir) = self.cwd {
            command.current_dir(dir);
        }
        if let Some(home) = self.home_flag {
            command.arg("--home").arg(home);
        }
        command.arg("tick");

        let output = command.output().expect("pulsen を起動できる");
        drop(sandbox);
        Run {
            code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        }
    }
}

/// テスト用エージェントを `probe` として定義したグローバル設定。
///
/// 実在するエージェント(`claude` 等)に依存せずに起動経路を通すためのフィクスチャ。
/// `cmd` は配列形式で組む — 実行ファイルのパスに空白が入っても壊れない。
pub fn probe_config(program: &Path, mode: &[&str]) -> String {
    let mut text = String::from("agents:\n  probe:\n    cmd:\n");
    text.push_str(&format!(
        "      - {}\n",
        yaml_scalar(&program.display().to_string())
    ));
    for token in mode {
        text.push_str(&format!("      - {}\n", yaml_scalar(token)));
    }
    text
}

/// YAML のダブルクォート表記(パスやプレースホルダをそのまま1トークンにする)。
fn yaml_scalar(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

/// `probe` エージェントを使うワークフロー定義。
pub const PROBE_WORKFLOW: &str = "\
agent: probe
initial: queued
statuses:
  queued:
    prompt: 実装して
    next: done
  done:
    run: cleanup
";

/// 条件が満たされるまで期限つきでポーリングする。
///
/// **待ち条件はこれから観測する成果物そのものに立てる。** ラッパーの書き込み順序は
/// starttime → pid →(マーカー確認)→ ログ生成 → exit なので、pid の出現はログや `exit`
/// の存在を含意しない。pid だけを待って直後にログを assert すると、負荷の高い環境で
/// 落ちる(期限を伸ばしても直らない種類の失敗になる)。
///
/// タイムアウトは「何が現れなかったか」と観測先の一覧を添えて落とす — cron 実行の
/// 調査可能性と同じ理由で、待ち合わせの失敗も原因が読み取れる形で報告する。
pub fn wait_until(expected: &str, observed: &Path, condition: impl Fn() -> bool) {
    let deadline = Instant::now() + WAIT_TIMEOUT;
    loop {
        if condition() {
            return;
        }
        if Instant::now() >= deadline {
            panic!(
                "{:?} 以内に{expected}が現れなかった。{} の内容: {:?}",
                WAIT_TIMEOUT,
                observed.display(),
                listed_entries(observed)
            );
        }
        std::thread::sleep(POLL_INTERVAL);
    }
}

/// `pulsen wrapper` の1回の実行。
pub struct Wrapper {
    home_env: Option<OsString>,
    run_dir: OsString,
    workspace: OsString,
    agent_cmd: Vec<OsString>,
}

/// ラッパーモードの起動を組み立てる。
///
/// argv は `SystemProcessController::spawn_wrapper` が使うのと同じ定数から組む。
/// サブコマンド名とフラグ名の定義箇所はアダプターと CLI のパーサに分かれるため、
/// 実バイナリを通した往復でしか受理を主張できない。
pub fn wrapper(run_dir: impl AsRef<OsStr>, workspace: impl AsRef<OsStr>) -> Wrapper {
    Wrapper {
        home_env: None,
        run_dir: run_dir.as_ref().to_owned(),
        workspace: workspace.as_ref().to_owned(),
        agent_cmd: Vec::new(),
    }
}

impl Wrapper {
    /// 環境変数 `PULSEN_HOME` を与える。
    ///
    /// ラッパーはホームも config も読まないので、ここで壊れたホームを指しても動作は
    /// 変わらない。読まないことの観測に使う。
    pub fn home_env(mut self, path: &Path) -> Self {
        self.home_env = Some(path.as_os_str().to_owned());
        self
    }

    /// エージェントのコマンドのトークン列を与える。
    pub fn agent_cmd(mut self, tokens: impl IntoIterator<Item = OsString>) -> Self {
        self.agent_cmd = tokens.into_iter().collect();
        self
    }

    /// 実バイナリを起動して結果を集める。
    pub fn run(self) -> Run {
        let (mut command, sandbox) = self.command();

        let output = command.output().expect("pulsen を起動できる");
        drop(sandbox);
        Run {
            code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        }
    }

    /// 実バイナリを起動し、終了を待たずに実行中のラッパーを返す。
    ///
    /// エージェントの実行中に外からラッパーを終わらせる観測に使う。`run()` は終了を
    /// 待つため、その状況を作れない。
    pub fn start(self) -> RunningWrapper {
        let (mut command, sandbox) = self.command();

        let child = command.spawn().expect("pulsen を起動できる");
        RunningWrapper {
            child,
            _sandbox: sandbox,
        }
    }

    /// 起動する実バイナリと、起動が終わるまで保持する一時ホーム。
    fn command(self) -> (Command, TempDir) {
        let mut command = Command::new(env!("CARGO_BIN_EXE_pulsen"));
        let sandbox = detached_home(&mut command);
        if let Some(home) = self.home_env {
            command.env(HOME_ENV, home);
        }
        command
            .arg(process::WRAPPER_SUBCOMMAND)
            .arg(process::RUN_DIR_FLAG)
            .arg(self.run_dir)
            .arg(process::WORKSPACE_FLAG)
            .arg(self.workspace)
            .arg(process::COMMAND_SEPARATOR)
            .args(self.agent_cmd);
        (command, sandbox)
    }
}

/// 実行中のラッパー。
pub struct RunningWrapper {
    child: Child,
    /// ホーム解決を切り離す一時ホーム。プロセスが終わるまで保持する。
    _sandbox: TempDir,
}

impl RunningWrapper {
    /// ラッパーを即座に終了させ、終了を待ち取る。
    ///
    /// 猶予を与えない終了なので、ラッパーは自身の後始末を挟めない。エージェントは
    /// ラッパーの子であり、この終了では道連れにならずに残る。
    pub fn kill(mut self) {
        self.child.kill().expect("ラッパーを終了させられる");
        self.child.wait().expect("終了を待ち取れる");
    }
}

/// テスト用エージェントの実行ファイル。
///
/// パッケージ全体を対象にした `cargo test` は example もビルドするため、バイナリと同じ
/// 出力ディレクトリの `examples/` に置かれる。
pub fn agent_probe() -> Option<PathBuf> {
    example_program("agent_probe")
}

/// 出力ディレクトリの `examples/` にある実行ファイル。
pub fn example_program(name: &str) -> Option<PathBuf> {
    let binary = Path::new(env!("CARGO_BIN_EXE_pulsen"));
    let program = binary
        .parent()?
        .join("examples")
        .join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
    program.is_file().then_some(program)
}

/// 任意の引数で実バイナリを1回起動する。
///
/// 引数の解釈そのもの(使い方の誤り・`--help`)を確かめる経路。ホームには触れないため
/// グローバルホームの指定は取らない。
pub fn run_cli(arguments: &[&str]) -> Run {
    let mut command = Command::new(env!("CARGO_BIN_EXE_pulsen"));
    let sandbox = detached_home(&mut command);
    command.args(arguments);

    let output = command.output().expect("pulsen を起動できる");
    drop(sandbox);
    Run {
        code: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

/// 起動する `pulsen` のホーム解決を、実行環境から切り離す。
///
/// `PULSEN_HOME` を落とすだけでは、ホームを指定し忘れたテストが既定の `~/.pulsen/` に
/// 落ちて開発者の実ホームに登録してしまう。ユーザーのホームも毎回作る一時ディレクトリへ
/// 向け、3段の優先順位(フラグ・環境変数・既定)のどこを踏んでも一時ディレクトリの外に
/// 出ないようにする(ADR-062)。戻り値は起動が終わるまで保持する。
fn detached_home(command: &mut Command) -> TempDir {
    let sandbox = tempfile::tempdir().expect("一時ホームを作れる");
    command.env_remove(HOME_ENV);
    for variable in USER_HOME_ENV {
        command.env(variable, sandbox.path());
    }
    sandbox
}

/// 実行の結果。
pub struct Run {
    /// 終了コード。
    pub code: Option<i32>,
    /// 標準出力。
    pub stdout: String,
    /// 標準エラー出力。
    pub stderr: String,
}

impl Run {
    /// 0 で終了したか。
    pub fn succeeded(&self) -> bool {
        self.code == Some(0)
    }

    /// 成功を確かめる(失敗時は標準エラー出力を添えて落とす)。
    pub fn assert_succeeded(&self) -> &Self {
        assert!(
            self.succeeded(),
            "0 で終了する(code={:?}): {}",
            self.code,
            self.stderr
        );
        self
    }

    /// 非0で終了したことを確かめる。
    pub fn assert_rejected(&self) -> &Self {
        assert!(
            matches!(self.code, Some(code) if code != 0),
            "非0で終了する(code={:?}): {}",
            self.code,
            self.stdout
        );
        self
    }

    /// 案内に含まれるべき語がすべて出ていることを確かめる。
    pub fn assert_reports(&self, expected: &[&str]) -> &Self {
        for text in expected {
            assert!(
                self.stderr.contains(text),
                "案内に `{text}` が含まれる: {}",
                self.stderr
            );
        }
        self
    }
}

/// ファイルを読み取れない状態にする(ファイル専用)。
///
/// 制限が実際に効いたことを確認してから `Some` を返す(ADR-027)。
///
/// 確認は `fs::read` の成否で行うため、ディレクトリに渡すと制限の有無にかかわらず
/// `Err`(EISDIR)になり「効いた」と誤判定する。ディレクトリ版は
/// `conformance_task_repository.rs` の `deny_dir_read` / `deny_dir_write` にある。
#[cfg(unix)]
pub fn deny_read(path: &Path) -> Option<Restore> {
    use std::os::unix::fs::PermissionsExt;

    assert!(
        path.is_file(),
        "deny_read はファイル専用: {}",
        path.display()
    );
    let original = fs::metadata(path).ok()?.permissions();
    let mut denied = original.clone();
    denied.set_mode(0o000);
    fs::set_permissions(path, denied).ok()?;

    let target = path.to_path_buf();
    let restore = Restore::new(move || {
        let _ = fs::set_permissions(&target, original);
    });

    if fs::read(path).is_ok() {
        return None;
    }
    Some(restore)
}

#[cfg(not(unix))]
pub fn deny_read(path: &Path) -> Option<Restore> {
    assert!(
        path.is_file(),
        "deny_read はファイル専用: {}",
        path.display()
    );
    None
}
