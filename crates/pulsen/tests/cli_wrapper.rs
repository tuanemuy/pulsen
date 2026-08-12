//! ラッパーモード(内部コマンド)の受け入れ(PAGE-wrapper-001〜005)。
//!
//! 実バイナリを `spawn_wrapper` と同じ argv で起動し、結果を run ディレクトリのファイル
//! として観測する。標準出力には何も要求しない。

mod common;

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use common::{Home, agent_probe, scratch, wrapper};
use serde_json::Value;
use tempfile::TempDir;

/// タスクIDの形を満たす値。run ディレクトリの復元は `derive` との一致で決まる。
const TASK_ID: &str = "20260811t091530-k3f9qa1b";

/// tick が用意する run ディレクトリと worktree を模した置き場。
struct Launch {
    dir: TempDir,
}

impl Launch {
    fn new() -> Self {
        let launch = Self { dir: scratch() };
        fs::create_dir_all(launch.workspace()).expect("worktree を作れる");
        launch
    }

    /// `<state_root>/runs/<task-id>/attempt-<n>` の形に合う run ディレクトリ。
    fn run_dir(&self) -> PathBuf {
        self.dir
            .path()
            .join("state")
            .join("runs")
            .join(TASK_ID)
            .join("attempt-1")
    }

    fn workspace(&self) -> PathBuf {
        self.dir.path().join("worktree")
    }

    /// run ディレクトリ直下のエントリ名(昇順)。
    fn run_files(&self) -> Vec<String> {
        let Ok(entries) = fs::read_dir(self.run_dir()) else {
            return Vec::new();
        };
        let mut names: Vec<String> = entries
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    fn read_json(&self, name: &str) -> Value {
        let path = self.run_dir().join(name);
        let bytes = fs::read(&path).unwrap_or_else(|error| {
            panic!("{}: 読めない: {error}", path.display());
        });
        serde_json::from_slice(&bytes).expect("JSON である")
    }

    /// exit ファイルに記録された符号化値。
    fn exit_code(&self) -> i64 {
        self.read_json("exit")["code"]
            .as_i64()
            .expect("code は整数である")
    }
}

/// テスト用エージェントの実行ファイル。
///
/// 不在はスキップにしない — 作り忘れが緑にならないようにする(ADR-074 と同じ扱い)。
fn probe_program() -> PathBuf {
    agent_probe().expect("examples/agent_probe がビルドされている")
}

/// テスト用エージェントを起動するトークン列。
fn probe(program: &Path, tokens: &[&str]) -> Vec<OsString> {
    let mut all = vec![program.as_os_str().to_owned()];
    all.extend(tokens.iter().map(|token| OsString::from(*token)));
    all
}

#[test]
fn 起動引数どおりのエージェントが実行され終了結果がrunディレクトリに現れる() {
    let program = probe_program();
    let launch = Launch::new();

    let run = wrapper(launch.run_dir(), launch.workspace())
        .agent_cmd(probe(&program, &["exit", "0"]))
        .run();

    run.assert_succeeded();
    assert!(run.stdout.is_empty(), "結果は run ディレクトリにだけ現れる");
    assert_eq!(
        launch.run_files(),
        vec!["exit", "pid", "starttime", "stderr.log", "stdout.log"]
    );
    assert_eq!(launch.exit_code(), 0);
}

#[test]
fn 同定情報はstarttimeとpidの両方に記録される() {
    let program = probe_program();
    let launch = Launch::new();

    wrapper(launch.run_dir(), launch.workspace())
        .agent_cmd(probe(&program, &["exit", "0"]))
        .run()
        .assert_succeeded();

    let pid = launch.read_json("pid");
    assert!(pid["pid"].as_u64().is_some_and(|pid| pid > 0));
    assert!(
        pid["kill_ident"]
            .as_str()
            .is_some_and(|ident| !ident.is_empty()),
        "kill 同定子は非空"
    );
    let starttime = launch.read_json("starttime");
    assert!(
        starttime["ident"]
            .as_str()
            .is_some_and(|ident| !ident.is_empty())
    );
    assert!(starttime["wall"].as_str().is_some());
}

#[test]
fn エージェントの終了コードはそのままexitファイルに現れる() {
    let program = probe_program();

    for code in [0, 3, 42] {
        let launch = Launch::new();

        wrapper(launch.run_dir(), launch.workspace())
            .agent_cmd(probe(&program, &["exit", &code.to_string()]))
            .run()
            .assert_succeeded();

        assert_eq!(launch.exit_code(), code, "exit code {code}");
    }
}

#[test]
fn 起動できないエージェントはコマンド不在として符号化される() {
    let launch = Launch::new();
    let missing = launch.dir.path().join("no-such-agent");

    wrapper(launch.run_dir(), launch.workspace())
        .agent_cmd([missing.into_os_string()])
        .run()
        .assert_succeeded();

    assert_eq!(launch.exit_code(), 127);
}

#[test]
fn worktreeが存在しなければエージェントを起動せず非ゼロが記録される() {
    let program = probe_program();
    let launch = Launch::new();
    fs::remove_dir_all(launch.workspace()).expect("worktree を消せる");

    wrapper(launch.run_dir(), launch.workspace())
        .agent_cmd(probe(&program, &["print", "出力", ""]))
        .run()
        .assert_succeeded();

    assert_eq!(launch.exit_code(), 126, "起動不能として符号化される");
    assert!(
        !launch.run_dir().join("stdout.log").exists(),
        "エージェントは起動されない"
    );
}

#[test]
fn グローバル設定が不在でも破損していても動作は変わらない() {
    let program = probe_program();

    for config in [None, Some("agents: [壊れた\n")] {
        let home = Home::uninitialized();
        if let Some(text) = config {
            home.write_config(text);
        }
        let launch = Launch::new();

        wrapper(launch.run_dir(), launch.workspace())
            .home_env(home.path())
            .agent_cmd(probe(&program, &["exit", "5"]))
            .run()
            .assert_succeeded();

        assert_eq!(launch.exit_code(), 5, "config: {config:?}");
    }
}

#[test]
fn シェルのメタ文字や空文字列を含むトークンはリテラルのまま渡る() {
    let program = probe_program();
    let launch = Launch::new();
    let tokens = ["--model", "", "$HOME", "*", "a && b", ">out.txt", "{input}"];

    wrapper(launch.run_dir(), launch.workspace())
        .agent_cmd(probe(&program, &[&["echo-args"], &tokens[..]].concat()))
        .run()
        .assert_succeeded();

    let received = fs::read_to_string(launch.run_dir().join("stdout.log")).expect("ログを読める");
    assert_eq!(
        received.lines().collect::<Vec<_>>(),
        tokens,
        "spawn_wrapper が組む argv をそのまま受理し、展開も再分割も起きない"
    );
}

#[test]
fn 相対パスの起動引数はrunディレクトリに何も書かずに拒否される() {
    let launch = Launch::new();

    let run = wrapper("state/runs/t/attempt-1", launch.workspace())
        .agent_cmd([OsString::from("true")])
        .run();

    run.assert_rejected();
    assert_eq!(launch.run_files(), Vec::<String>::new());
}

#[test]
fn 規定の形でないrunディレクトリはrunディレクトリに何も書かずに拒否される() {
    let launch = Launch::new();
    let malformed = launch
        .dir
        .path()
        .join("state")
        .join("runs")
        .join(TASK_ID)
        .join("attempt-01");

    let run = wrapper(&malformed, launch.workspace())
        .agent_cmd([OsString::from("true")])
        .run();

    run.assert_rejected();
    assert!(!malformed.exists(), "書き込みは一切行われない");
}

#[test]
fn エージェントのコマンドが0トークンなら拒否される() {
    let launch = Launch::new();

    let run = wrapper(launch.run_dir(), launch.workspace()).run();

    run.assert_rejected();
    assert_eq!(launch.run_files(), Vec::<String>::new());
}

#[test]
fn 相対パスのworktreeはrunディレクトリに何も書かずに拒否される() {
    let launch = Launch::new();

    let run = wrapper(launch.run_dir(), "worktree")
        .agent_cmd([OsString::from("true")])
        .run();

    run.assert_rejected();
    assert_eq!(launch.run_files(), Vec::<String>::new());
}
