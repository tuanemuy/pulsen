//! CommandRunner の適合スイートを `SystemCommandRunner` に適用する。
//!
//! テスト用コマンドは `examples/judge_probe` を使う(ADR-082)。見つからない場合は
//! フックが `None` を返してスキップになるが、**スキップ許容集合には入れない** —
//! 「examples を作り忘れた」が緑にならないようにする。

use std::cell::Cell;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use pulsen::adapter::command_runner::SystemCommandRunner;
use pulsen_conformance::{CommandBehavior, CommandRunnerHarness};
use pulsen_domain::definition::PlainCommand;
use tempfile::TempDir;

/// 呼び出しプロセスに設定されていない変数名。
const ABSENT_ENV_NAME: &str = "PULSEN_CONFORMANCE_ABSENT";

/// 一時ディレクトリに証跡と期待ファイルの置き場を用意するハーネス。
struct SystemCommandRunnerHarness {
    root: TempDir,
    runner: SystemCommandRunner,
    /// 証跡・期待ファイルの名前を一意にする連番。
    serial: Cell<u32>,
}

impl SystemCommandRunnerHarness {
    fn new() -> Self {
        Self {
            root: tempfile::tempdir().expect("一時ディレクトリを作れる"),
            runner: SystemCommandRunner::new(),
            serial: Cell::new(0),
        }
    }

    /// 呼び出しごとに違う一時パス。
    fn unique(&self, prefix: &str) -> PathBuf {
        let serial = self.serial.get();
        self.serial.set(serial + 1);
        self.root.path().join(format!("{prefix}-{serial}"))
    }

    /// テスト用コマンドを組む。
    fn probe(&self, tokens: Vec<String>) -> Option<PlainCommand> {
        let mut all = vec![example_program("judge_probe")?.to_str()?.to_owned()];
        all.extend(tokens);
        PlainCommand::parse_tokens(all).ok()
    }
}

/// 出力ディレクトリの `examples/` にある実行ファイル。
fn example_program(name: &str) -> Option<PathBuf> {
    let binary = Path::new(env!("CARGO_BIN_EXE_pulsen"));
    let program = binary
        .parent()?
        .join("examples")
        .join(format!("{name}{}", env::consts::EXE_SUFFIX));
    program.is_file().then_some(program)
}

impl CommandRunnerHarness for SystemCommandRunnerHarness {
    type Runner = SystemCommandRunner;

    fn runner(&self) -> &Self::Runner {
        &self.runner
    }

    fn command(&self, behavior: CommandBehavior) -> Option<PlainCommand> {
        let tokens = match behavior {
            CommandBehavior::Exit(code) => vec!["exit".to_owned(), code.to_string()],
            CommandBehavior::Abort => vec!["abort".to_owned()],
            CommandBehavior::CheckArgs(tokens) => {
                // 期待はファイルで渡す。引数で渡すと、シェルが解釈した場合に期待側も
                // 同じように歪んで照合が通ってしまう。
                let expected = self.unique("expected");
                fs::write(&expected, format!("{}\n", tokens.join("\n"))).ok()?;
                let mut all = vec!["check-args".to_owned(), expected.to_str()?.to_owned()];
                all.extend(tokens);
                all
            }
            CommandBehavior::CheckEnv { name, value } => {
                vec!["check-env".to_owned(), name, value]
            }
            CommandBehavior::CheckCwd(path) => {
                vec!["check-cwd".to_owned(), path.to_str()?.to_owned()]
            }
            CommandBehavior::Print { stdout, stderr } => {
                vec!["print".to_owned(), stdout, stderr]
            }
            CommandBehavior::Sleep(duration) => {
                vec!["sleep".to_owned(), duration.as_millis().to_string()]
            }
            CommandBehavior::Record { after, evidence } => vec![
                "record".to_owned(),
                after.as_millis().to_string(),
                evidence.to_str()?.to_owned(),
            ],
        };
        self.probe(tokens)
    }

    fn missing_command(&self) -> Option<PlainCommand> {
        PlainCommand::parse_tokens(vec![
            self.root
                .path()
                .join("no-such-command")
                .to_str()?
                .to_owned(),
        ])
        .ok()
    }

    fn non_executable_command(&self) -> Option<PlainCommand> {
        let path = self.root.path().join("not-executable");
        fs::write(&path, b"#!/bin/sh\nexit 0\n").ok()?;
        deny_execute(&path)?;
        PlainCommand::parse_tokens(vec![path.to_str()?.to_owned()]).ok()
    }

    fn caller_env(&self) -> Option<(String, String)> {
        // 実行中プロセスの環境の書き換えは安全に行えないため、既にあるものを教える。
        // 継承の検証にはこれで足りる。
        let value = env::var("PATH").ok()?;
        (!value.is_empty()).then_some(("PATH".to_owned(), value))
    }

    fn absent_env_name(&self) -> Option<String> {
        env::var(ABSENT_ENV_NAME)
            .is_err()
            .then(|| ABSENT_ENV_NAME.to_owned())
    }

    fn caller_current_dir(&self) -> Option<PathBuf> {
        env::current_dir().ok()
    }

    fn evidence_path(&self) -> Option<PathBuf> {
        Some(self.unique("evidence"))
    }
}

/// 実行できない実体にする。実際に起動できないことを確かめてから `Some` を返す。
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

#[cfg(not(unix))]
fn deny_execute(_path: &Path) -> Option<()> {
    None
}

/// 権限制限が効かない環境でのみスキップされるケース(HOOKS.md の区分 C)。
const PERMISSION_CASES: [&str; 1] = ["tc_port_command_runner_004"];

/// この環境でスキップを許容するケース。
///
/// exit code を持たない終了(TC-005)は許容しない — 期待は「非0の符号化値」までであり
/// (ADR-082)、`judge_probe abort` はどのプラットフォームでも非0を返す。
fn allowed_skips() -> Vec<&'static str> {
    let mut allowed = Vec::new();
    if !pulsen_conformance::permission_restrictions_effective() {
        allowed.extend(PERMISSION_CASES);
    }
    allowed
}

pulsen_conformance::command_runner_conformance!(SystemCommandRunnerHarness::new(), allowed_skips());
