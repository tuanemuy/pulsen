//! ProcessController の適合スイートを `SystemProcessController` に適用する。
//!
//! テスト用エージェントは `examples/agent_probe` を使う(ADR-074)。見つからない場合は
//! フックが `None` を返してスキップになるが、**スキップ許容集合には入れない** —
//! 「examples を作り忘れた」が緑にならないようにする。

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use pulsen::adapter::clock::SystemClock;
use pulsen::adapter::process::{IdentitySource, SystemProcessController};
use pulsen_conformance::{AgentBehavior, ProcessControllerHarness, Restore};
use pulsen_domain::definition::CommandLine;
use pulsen_domain::execution::WrapperLaunchSpec;
use pulsen_domain::task::{
    AttemptNumber, Clock, RunDirPath, StateRoot, TaskId, Timestamp, WorktreePath,
};
use tempfile::TempDir;

/// run ファイルの出現を待つ期限。負荷の高い環境でも spawn から書き込みまでが収まる余裕を取る。
const RUN_FILE_DEADLINE: Duration = Duration::from_secs(30);
/// 出現を確かめる間隔。
const POLL_INTERVAL: Duration = Duration::from_millis(50);

/// run ディレクトリの導出に使うタスクID(形式を満たす固定値)。
const TASK_ID: &str = "20260811t091530-k3f9qa1b";

/// 一時ディレクトリに worktree とログの置き場を用意するハーネス。
struct SystemProcessControllerHarness {
    root: TempDir,
    controller: SystemProcessController,
    failing_identity: SystemProcessController,
    failing_self_exe: SystemProcessController,
}

impl SystemProcessControllerHarness {
    fn new() -> Self {
        let root = tempfile::tempdir().expect("一時ディレクトリを作れる");
        let self_exe = PathBuf::from(env!("CARGO_BIN_EXE_pulsen"));
        // 存在しないパスを取得元として注入すると、どのプラットフォームでも「取得機構
        // そのものの失敗」に落ちる(ADR-067 の写像表・ADR-068)。本番のインスタンスは
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
        Self {
            root,
            controller: SystemProcessController::new(
                self_exe,
                IdentitySource::platform_default(),
                SystemClock::new(),
            ),
            failing_identity,
            failing_self_exe,
        }
    }

    /// デタッチ性を検証するフィクスチャの実行ファイル。
    fn spawn_probe_program(&self) -> Option<PathBuf> {
        example_program("spawn_probe")
    }

    /// `<state_root>/runs/<task-id>/attempt-1` を作って返す。
    ///
    /// ラッパーは run ディレクトリから状態のルートを復元するため、パスは
    /// `RunDirPath::derive` の像でなければならない(ADR-079)。
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
}

/// ディレクトリへ書き込めない状態にする。
///
/// 制限が実際に効いたことを確認してから `Some` を返す(ADR-027)。
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

/// シグナルによる終了を作れないプラットフォームでのみスキップされるケース。
const SIGNAL_CASES: [&str; 1] = ["tc_port_process_controller_024"];

/// この環境でスキップを許容するケース。
///
/// 取得機構の失敗(TC-005)は取得元の注入で作れるため、権限にも root の可否にも依存せず
/// 走る(ADR-068)。`agent_probe` の不在は許容しない — 作り忘れを緑にしないため。
fn allowed_skips() -> Vec<&'static str> {
    let mut allowed = Vec::new();
    if !pulsen_conformance::permission_restrictions_effective() {
        allowed.extend(PERMISSION_CASES);
    }
    if !cfg!(unix) {
        allowed.extend(SIGNAL_CASES);
    }
    allowed
}

pulsen_conformance::process_controller_identity_conformance!(
    SystemProcessControllerHarness::new(),
    allowed_skips()
);

// `spawn` スイートはどのケースもスキップを許容しない。起動不能(TC-003)は自バイナリの
// 注入で確定的に走り、デタッチ性(TC-002)は `examples/spawn_probe` を要するが、examples の
// 不在は許容しない — 作り忘れを緑にしないため(ADR-074 / ADR-075)。
pulsen_conformance::process_controller_spawn_conformance!(
    SystemProcessControllerHarness::new(),
    Vec::new()
);
