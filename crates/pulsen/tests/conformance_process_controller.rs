//! ProcessController の適合スイートを `SystemProcessController` に適用する。
//!
//! テスト用エージェントは `examples/agent_probe` を使う。見つからない場合は
//! フックが `None` を返してスキップになるが、**スキップ許容集合には入れない** —
//! 「examples を作り忘れた」が緑にならないようにする。

use std::cell::RefCell;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use pulsen::adapter::clock::SystemClock;
use pulsen::adapter::process::{IdentitySource, SystemProcessController, TerminatorSource};
use pulsen::adapter::run_store::FsRunStore;
use pulsen_conformance::{AgentBehavior, ExecutionUnit, ProcessControllerHarness, Restore};
use pulsen_domain::definition::CommandLine;
use pulsen_domain::execution::{ProcessController, RunStore, WrapperLaunchSpec};
use pulsen_domain::task::{
    AttemptNumber, Clock, Pid, RunDirPath, StateRoot, TaskId, Timestamp, WorktreePath,
};
use tempfile::TempDir;

/// run ファイルの出現を待つ期限。負荷の高い環境でも spawn から書き込みまでが収まる余裕を取る。
const RUN_FILE_DEADLINE: Duration = Duration::from_secs(30);
/// 出現を確かめる間隔。
const POLL_INTERVAL: Duration = Duration::from_millis(50);

/// run ディレクトリの導出に使うタスクID(形式を満たす固定値)。
const TASK_ID: &str = "20260811t091530-k3f9qa1b";

/// 実行単位に滞留させる上限。解放が来なくてもプロセスが残り続けないための歯止め。
const UNIT_LIFETIME: Duration = Duration::from_secs(20);

/// 一時ディレクトリに worktree とログの置き場を用意するハーネス。
struct SystemProcessControllerHarness {
    root: TempDir,
    controller: SystemProcessController,
    failing_identity: SystemProcessController,
    failing_self_exe: SystemProcessController,
    failing_terminator: SystemProcessController,
    /// 起動時のインスタンスと縁の切れた、新規に構成したコントローラ。
    restarted: SystemProcessController,
    /// 滞留中の実行単位を解放するためのパス。ハーネスの終わりに必ず書く。
    releases: RefCell<Vec<PathBuf>>,
}

impl SystemProcessControllerHarness {
    fn new() -> Self {
        let root = tempfile::tempdir().expect("一時ディレクトリを作れる");
        let self_exe = PathBuf::from(env!("CARGO_BIN_EXE_pulsen"));
        // 存在しないパスを取得元として注入すると、どのプラットフォームでも「取得機構
        // そのものの失敗」に落ちる。本番のインスタンスは
        // イミュータブルなまま保たれる。
        let failing_identity = SystemProcessController::new(
            self_exe.clone(),
            IdentitySource::new(root.path().join("no-such-identity-source")),
            SystemClock::new(),
        );
        // 同型に、存在しないパスを自バイナリとして注入したコントローラで起動不能を作る。
        let failing_self_exe = SystemProcessController::new(
            root.path().join("no-such-pulsen"),
            IdentitySource::platform_default(),
            SystemClock::new(),
        );
        // 終了操作の実体を存在しないパスにすると、同定はできるのに終了だけが失敗する。
        let failing_terminator = SystemProcessController::new(
            self_exe.clone(),
            IdentitySource::platform_default(),
            SystemClock::new(),
        )
        .with_terminator_source(TerminatorSource::new(
            root.path().join("no-such-terminator"),
        ));
        Self {
            root,
            controller: SystemProcessController::new(
                self_exe.clone(),
                IdentitySource::platform_default(),
                SystemClock::new(),
            ),
            failing_identity,
            failing_self_exe,
            failing_terminator,
            restarted: SystemProcessController::new(
                self_exe,
                IdentitySource::platform_default(),
                SystemClock::new(),
            ),
            releases: RefCell::new(Vec::new()),
        }
    }

    /// 子プロセスを起こして滞留するエージェントを、別プロセス経由でデタッチ起動する。
    ///
    /// 起動を別プロセスに任せるのは、テストプロセスがラッパーの親のままだと終了後に
    /// ゾンビとして残り、「実行単位に属する全プロセスが終了する」の観測が壊れるため。
    fn spawn_unit(&self) -> Option<ExecutionUnit> {
        let run_dir = self.prepared_run_dir()?;
        let pid_file = self.dir("unit-pids");
        let release = self.dir("unit-release");
        self.releases.borrow_mut().push(release.clone());

        let agent_cmd = self.probe_command(vec![
            "spawn-child".to_owned(),
            pid_file.to_str()?.to_owned(),
            release.to_str()?.to_owned(),
            UNIT_LIFETIME.as_millis().to_string(),
        ])?;
        let spec = WrapperLaunchSpec::new(run_dir.clone(), agent_cmd, self.worktree()?);
        self.spawn_from_other_process(&spec)?;

        // 同定情報一式(run ディレクトリの pid ファイル)と、エージェント・その子の PID が
        // 揃うまで待つ。待ち条件はこれから読む成果物そのものに立てる。
        if !wait_until(|| run_dir.pid_file().is_file() && members_of(&pid_file).len() == 2) {
            return None;
        }
        let content = FsRunStore::new(StateRoot::parse(self.dir("state")).ok()?)
            .read_pid_file(&run_dir)
            .ok()??;
        let mut members = vec![content.pid()];
        members.extend(members_of(&pid_file));
        Some(ExecutionUnit {
            kill_ident: content.kill_ident().clone(),
            members,
        })
    }

    /// デタッチ性を検証するフィクスチャの実行ファイル。
    fn spawn_probe_program(&self) -> Option<PathBuf> {
        example_program("spawn_probe")
    }

    /// `<state_root>/runs/<task-id>/attempt-1` を作って返す。
    ///
    /// ラッパーは run ディレクトリから状態のルートを復元するため、パスは
    /// `RunDirPath::derive` の像でなければならない。
    fn prepared_run_dir(&self) -> Option<RunDirPath> {
        let state_root = StateRoot::parse(self.dir("state")).ok()?;
        let run_dir = RunDirPath::derive(
            &state_root,
            &TaskId::parse(TASK_ID.to_owned()).ok()?,
            AttemptNumber::parse(1).ok()?,
        );
        fs::create_dir_all(run_dir.as_path()).ok()?;
        Some(run_dir)
    }

    fn dir(&self, name: &str) -> PathBuf {
        self.root.path().join(name)
    }

    /// テスト用エージェントを起動するコマンドを組む。
    fn probe_command(&self, tokens: Vec<String>) -> Option<CommandLine> {
        let mut all = vec![example_program("agent_probe")?.to_str()?.to_owned()];
        all.extend(tokens);
        CommandLine::rehydrate(all).ok()
    }
}

/// 出力ディレクトリの `examples/` にある実行ファイル。
///
/// パッケージ全体を対象にした `cargo test` は example もビルドするため、バイナリと
/// 同じ出力ディレクトリの `examples/` に置かれる。
fn example_program(name: &str) -> Option<PathBuf> {
    let binary = Path::new(env!("CARGO_BIN_EXE_pulsen"));
    let program = binary
        .parent()?
        .join("examples")
        .join(format!("{name}{}", env::consts::EXE_SUFFIX));
    program.is_file().then_some(program)
}

/// 条件が満たされるまで期限つきで待つ。
fn wait_until(condition: impl Fn() -> bool) -> bool {
    let deadline = Instant::now() + RUN_FILE_DEADLINE;
    loop {
        if condition() {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(POLL_INTERVAL);
    }
}

impl ProcessControllerHarness for SystemProcessControllerHarness {
    type Controller = SystemProcessController;

    fn controller(&self) -> &Self::Controller {
        &self.controller
    }

    fn observe_wall_clock(&self) -> Option<Timestamp> {
        Some(SystemClock::new().now())
    }

    fn failing_identity_controller(&self) -> Option<&Self::Controller> {
        Some(&self.failing_identity)
    }

    fn worktree(&self) -> Option<WorktreePath> {
        let dir = self.dir("worktree");
        fs::create_dir_all(&dir).ok()?;
        WorktreePath::parse(dir).ok()
    }

    fn missing_worktree(&self) -> Option<WorktreePath> {
        WorktreePath::parse(self.dir("worktree-gone")).ok()
    }

    fn log_paths(&self) -> Option<(PathBuf, PathBuf)> {
        let dir = self.dir("logs");
        fs::create_dir_all(&dir).ok()?;
        Some((dir.join("stdout.log"), dir.join("stderr.log")))
    }

    fn unwritable_log_path(&self) -> Option<(PathBuf, Restore)> {
        let dir = self.dir("logs-readonly");
        fs::create_dir_all(&dir).ok()?;
        let restore = deny_dir_write(&dir)?;
        Some((dir.join("stdout.log"), restore))
    }

    fn agent_command(&self, behavior: AgentBehavior) -> Option<CommandLine> {
        let tokens = match behavior {
            AgentBehavior::Exit(code) => vec!["exit".to_owned(), code.to_string()],
            AgentBehavior::Print { stdout, stderr } => vec!["print".to_owned(), stdout, stderr],
            AgentBehavior::CheckCwd(path) => {
                vec!["check-cwd".to_owned(), path.as_path().to_str()?.to_owned()]
            }
            AgentBehavior::EchoArgs(mut tokens) => {
                let mut all = vec!["echo-args".to_owned()];
                all.append(&mut tokens);
                all
            }
            AgentBehavior::Sleep(duration) => {
                vec!["sleep".to_owned(), duration.as_millis().to_string()]
            }
            AgentBehavior::Abort => vec!["abort".to_owned()],
        };
        self.probe_command(tokens)
    }

    fn missing_command(&self) -> Option<CommandLine> {
        CommandLine::rehydrate(vec![
            self.dir("no-such-agent").to_str()?.to_owned(),
            "--print".to_owned(),
        ])
        .ok()
    }

    fn non_executable_command(&self) -> Option<CommandLine> {
        let path = self.dir("not-executable");
        fs::write(&path, b"#!/bin/sh\nexit 0\n").ok()?;
        deny_execute(&path)?;
        CommandLine::rehydrate(vec![path.to_str()?.to_owned()]).ok()
    }

    fn launch_spec(&self, behavior: AgentBehavior) -> Option<WrapperLaunchSpec> {
        Some(WrapperLaunchSpec::new(
            self.prepared_run_dir()?,
            self.agent_command(behavior)?,
            self.worktree()?,
        ))
    }

    fn wait_for_run_files(&self, spec: &WrapperLaunchSpec) -> Option<bool> {
        let run_dir = spec.run_dir().clone();
        Some(wait_until(|| {
            [
                run_dir.starttime_file(),
                run_dir.pid_file(),
                run_dir.exit_file(),
            ]
            .iter()
            .all(|path| path.is_file())
        }))
    }

    fn spawn_from_other_process(&self, spec: &WrapperLaunchSpec) -> Option<()> {
        let status = std::process::Command::new(self.spawn_probe_program()?)
            .arg(env!("CARGO_BIN_EXE_pulsen"))
            .arg(spec.run_dir().as_path())
            .arg(spec.workspace().as_path())
            .args(spec.agent_cmd().tokens())
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .status()
            .ok()?;
        status.success().then_some(())
    }

    fn failing_controller(&self) -> Option<&Self::Controller> {
        Some(&self.failing_self_exe)
    }

    fn run_dir_is_empty(&self, spec: &WrapperLaunchSpec) -> Option<bool> {
        let mut entries = fs::read_dir(spec.run_dir().as_path()).ok()?;
        Some(entries.next().is_none())
    }

    fn terminated_pid(&self) -> Option<Pid> {
        // 起動して終了させ、回収まで済ませる。回収しないとゾンビとして観測され、
        // 「終了を確認済み」の前提が成立しない。
        let mut child = std::process::Command::new(example_program("agent_probe")?)
            .args(["sleep", "60000"])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .ok()?;
        let pid = Pid::new(child.id());
        child.kill().ok()?;
        child.wait().ok()?;
        Some(pid)
    }

    fn live_execution_unit(&self) -> Option<ExecutionUnit> {
        self.spawn_unit()
    }

    fn detached_execution_unit(&self) -> Option<(ExecutionUnit, &Self::Controller)> {
        Some((self.spawn_unit()?, &self.restarted))
    }

    fn orphaned_execution_unit(&self) -> Option<ExecutionUnit> {
        let unit = self.spawn_unit()?;
        // ラッパー(先頭のメンバー)だけを終了させ、エージェントとその子を実行単位に
        // 属したまま残す。
        let (wrapper, remnants) = unit.members.split_first()?;
        terminate_one(*wrapper)?;
        if !wait_until(|| {
            matches!(
                self.controller.starttime_of(*wrapper),
                Ok(None) | Err(pulsen_domain::execution::Io::Failed { .. })
            )
        }) {
            return None;
        }
        Some(ExecutionUnit {
            kill_ident: unit.kill_ident,
            members: remnants.to_vec(),
        })
    }

    fn failing_terminator_controller(&self) -> Option<&Self::Controller> {
        Some(&self.failing_terminator)
    }
}

impl Drop for SystemProcessControllerHarness {
    fn drop(&mut self) {
        // 滞留したままのエージェントが一時ディレクトリの削除と競合しないよう、解放を
        // 先に書く。上限つきの滞留なので、書けなくてもいずれ終わる。
        for release in self.releases.borrow().iter() {
            let _ = fs::write(release, b"release");
        }
    }
}

/// PID を1行1件で記録したファイルの内容。
fn members_of(path: &Path) -> Vec<Pid> {
    fs::read_to_string(path).map_or_else(
        |_| Vec::new(),
        |text| {
            text.lines()
                .filter_map(|line| line.trim().parse().ok())
                .map(Pid::new)
                .collect()
        },
    )
}

/// プロセス1つだけを終了させる(実行単位ではなく単体)。
#[cfg(unix)]
fn terminate_one(pid: Pid) -> Option<()> {
    std::process::Command::new("/bin/kill")
        .args(["-TERM", &pid.get().to_string()])
        .stdin(std::process::Stdio::null())
        .status()
        .ok()?
        .success()
        .then_some(())
}

#[cfg(not(unix))]
fn terminate_one(_pid: Pid) -> Option<()> {
    None
}

/// ディレクトリへ書き込めない状態にする。
///
/// 制限が実際に効いたことを確認してから `Some` を返す。
#[cfg(unix)]
fn deny_dir_write(dir: &Path) -> Option<Restore> {
    let restore = set_mode(dir, 0o555)?;
    let probe = dir.join("probe");
    if fs::write(&probe, b"probe").is_ok() {
        let _ = fs::remove_file(&probe);
        return None;
    }
    Some(restore)
}

/// 実行できない実体にする。実際に起動できないことを確かめてから `Some` を返す。
///
/// 復元は要らない — ハーネスが作ったファイルで、一時ディレクトリごと消える。
#[cfg(unix)]
fn deny_execute(path: &Path) -> Option<()> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = fs::metadata(path).ok()?.permissions();
    permissions.set_mode(0o644);
    fs::set_permissions(path, permissions).ok()?;

    std::process::Command::new(path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_err_and(|error| error.kind() == std::io::ErrorKind::PermissionDenied)
        .then_some(())
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) -> Option<Restore> {
    use std::os::unix::fs::PermissionsExt;

    let original = fs::metadata(path).ok()?.permissions();
    let mut restricted = original.clone();
    restricted.set_mode(mode);
    fs::set_permissions(path, restricted).ok()?;

    let target = path.to_path_buf();
    Some(Restore::new(move || {
        let _ = fs::set_permissions(&target, original);
    }))
}

#[cfg(not(unix))]
fn deny_dir_write(_dir: &Path) -> Option<Restore> {
    None
}

#[cfg(not(unix))]
fn deny_execute(_path: &Path) -> Option<()> {
    None
}

/// 権限制限が効かない環境でのみスキップされるケース(HOOKS.md の区分 C)。
const PERMISSION_CASES: [&str; 2] = [
    "tc_port_process_controller_023",
    "tc_port_process_controller_025",
];

/// この環境でスキップを許容するケース。
///
/// 取得機構の失敗(TC-005)は取得元の注入で作れるため、権限にも root の可否にも依存せず
/// 走る。exit コードを持たない終了(TC-024)が要求するのは `agent_command` の提供
/// だけで、期待も「非0の符号化値」までなので、前提を作れない環境が無い。
/// `agent_probe` の不在は許容しない — 作り忘れを緑にしないため。
fn allowed_skips() -> Vec<&'static str> {
    if pulsen_conformance::permission_restrictions_effective() {
        Vec::new()
    } else {
        PERMISSION_CASES.to_vec()
    }
}

pulsen_conformance::process_controller_identity_conformance!(
    SystemProcessControllerHarness::new(),
    allowed_skips()
);

// `spawn` スイートはどのケースもスキップを許容しない。起動不能(TC-003)は自バイナリの
// 注入で確定的に走り、デタッチ性(TC-002)は `examples/spawn_probe` を要するが、examples の
// 不在は許容しない — 作り忘れを緑にしないため。
pulsen_conformance::process_controller_spawn_conformance!(
    SystemProcessControllerHarness::new(),
    Vec::new()
);

/// 実行単位そのものを作れない環境でのみスキップされるケース。
///
/// 取得機構の失敗(TC-010)は取得元の注入だけで作れる。前提を作れない環境が無いため、この
/// 集合には現れない。終了操作の失敗(TC-013)と同定手段の喪失(TC-015)も注入で確定的に作れる
/// が、その操作を向ける実行単位そのものは要る。
const EXECUTION_UNIT_CASES: [&str; 4] = [
    "tc_port_process_controller_011",
    "tc_port_process_controller_012",
    "tc_port_process_controller_013",
    "tc_port_process_controller_015",
];

/// 実行単位は作れるが、その一部だけを終了させられない環境でスキップされるケース。
const PARTIAL_TERMINATION_CASES: [&str; 2] = [
    "tc_port_process_controller_014",
    "tc_port_process_controller_016",
];

/// 実行単位のフィクスチャをこの環境で組めるか。
///
/// 区別を4つに分けるのは、許容集合に入れてよいのが、スキップにしたときに
/// 「なぜ走らなかったか」と「次に何をすればよいか」がその宣言だけから定まる能力に限るため。
/// 実行ファイルの不在は原因も回避方法も一意(example をビルドする形で実行する)なので、
/// 能力側には置かない。
enum ExecutionUnitCapability {
    /// 実行単位を起こせ、その一部だけを終了させられる。
    Partitionable,
    /// 実行単位は起こせるが、その一部だけを終了させる手段が無い。
    WholeOnly,
    /// 実行単位そのものを起こせない。この観測は原因を1つに定めず、実行単位を作れない環境と
    /// フィクスチャ側の退行を区別しない(ロック保持フィクスチャの `SignalTimedOut` と同じ性質)。
    Unavailable,
    /// フィクスチャの実行ファイル(`examples/agent_probe` / `examples/spawn_probe`)が無い。
    ProgramMissing,
}

/// 実行単位を1度だけ実際に起こして能力を決める。
///
/// 判定はフィクスチャが本番で踏む手順そのもの(実行単位を起こし、その一部だけを終了させる)で
/// 行うため、判定と実際のスキップが食い違わない(`permission_restrictions_effective` と
/// 同じ性質)。フィクスチャの実行ファイルは適用側のテストターゲットからしか解決できないので、
/// probe はスイートではなくここに置く。
fn execution_unit_capability() -> &'static ExecutionUnitCapability {
    static CAPABILITY: OnceLock<ExecutionUnitCapability> = OnceLock::new();

    CAPABILITY.get_or_init(probe_execution_unit)
}

fn probe_execution_unit() -> ExecutionUnitCapability {
    if example_program("agent_probe").is_none() || example_program("spawn_probe").is_none() {
        return ExecutionUnitCapability::ProgramMissing;
    }
    // 滞留の解放はハーネスの `Drop` が書くため、判定が終わるまで保持する。
    let harness = SystemProcessControllerHarness::new();
    let Some(unit) = harness.spawn_unit() else {
        return ExecutionUnitCapability::Unavailable;
    };
    let partitionable = unit
        .members
        .first()
        .is_some_and(|wrapper| terminate_one(*wrapper).is_some());
    // 判定には入れない後始末。滞留の上限を待たずに実行単位を畳む。
    let _ = harness.controller.kill(&unit.kill_ident);

    if partitionable {
        ExecutionUnitCapability::Partitionable
    } else {
        ExecutionUnitCapability::WholeOnly
    }
}

/// 観測スイートでスキップを許容するケース。
///
/// 宣言はプラットフォームではなく実測した能力から組む。実行ファイルの不在は許容
/// しない — 作り忘れを緑にしないため(HOOKS.md)。
fn observation_allowed_skips() -> Vec<&'static str> {
    match execution_unit_capability() {
        ExecutionUnitCapability::Partitionable => Vec::new(),
        ExecutionUnitCapability::WholeOnly => PARTIAL_TERMINATION_CASES.to_vec(),
        ExecutionUnitCapability::Unavailable => {
            let mut allowed = EXECUTION_UNIT_CASES.to_vec();
            allowed.extend(PARTIAL_TERMINATION_CASES);
            allowed
        }
        ExecutionUnitCapability::ProgramMissing => Vec::new(),
    }
}

pulsen_conformance::process_controller_observation_conformance!(
    SystemProcessControllerHarness::new(),
    observation_allowed_skips()
);
