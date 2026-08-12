//! ProcessController の適合スイートを `SystemProcessController` に適用する。
//!
//! テスト用エージェントは `examples/agent_probe` を使う(ADR-010)。見つからない場合は
//! フックが `None` を返してスキップになるが、**スキップ許容集合には入れない** —
//! 「examples を作り忘れた」が緑にならないようにする。

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use pulsen::adapter::clock::SystemClock;
use pulsen::adapter::process::{IdentitySource, SystemProcessController};
use pulsen_conformance::{AgentBehavior, ProcessControllerHarness, Restore};
use pulsen_domain::definition::CommandLine;
use pulsen_domain::task::{Clock, Timestamp, WorktreePath};
use tempfile::TempDir;

/// 一時ディレクトリに worktree とログの置き場を用意するハーネス。
struct SystemProcessControllerHarness {
    root: TempDir,
    controller: SystemProcessController,
    failing_identity: SystemProcessController,
}

impl SystemProcessControllerHarness {
    fn new() -> Self {
        let root = tempfile::tempdir().expect("一時ディレクトリを作れる");
        let self_exe = PathBuf::from(env!("CARGO_BIN_EXE_pulsen"));
        // 存在しないパスを取得元として注入すると、どのプラットフォームでも「取得機構
        // そのものの失敗」に落ちる(ADR-003 の写像表・ADR-004)。本番のインスタンスは
        // イミュータブルなまま保たれる。
        let failing_identity = SystemProcessController::new(
            self_exe.clone(),
            IdentitySource::new(root.path().join("no-such-identity-source")),
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
        }
    }

    fn dir(&self, name: &str) -> PathBuf {
        self.root.path().join(name)
    }

    /// テスト用エージェントの実行ファイル。
    ///
    /// パッケージ全体を対象にした `cargo test` は example もビルドするため、バイナリと
    /// 同じ出力ディレクトリの `examples/` に置かれる。
    fn probe_program(&self) -> Option<PathBuf> {
        let binary = Path::new(env!("CARGO_BIN_EXE_pulsen"));
        let program = binary
            .parent()?
            .join("examples")
            .join(format!("agent_probe{}", env::consts::EXE_SUFFIX));
        program.is_file().then_some(program)
    }

    /// テスト用エージェントを起動するコマンドを組む。
    fn probe_command(&self, tokens: Vec<String>) -> Option<CommandLine> {
        let mut all = vec![self.probe_program()?.to_str()?.to_owned()];
        all.extend(tokens);
        CommandLine::rehydrate(all).ok()
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
/// 走る(ADR-004)。`agent_probe` の不在は許容しない — 作り忘れを緑にしないため。
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
