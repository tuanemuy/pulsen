//! ラッパーモード(内部コマンド)の受け入れ(PAGE-wrapper-001〜005)。
//!
//! 実バイナリを `spawn_wrapper` と同じ argv で起動し、結果を run ディレクトリのファイル
//! として観測する。標準出力には何も要求しない。

mod common;

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use common::{Home, agent_probe, scratch, wait_until, wrapper};
use serde_json::Value;
use tempfile::TempDir;

/// タスクIDの形を満たす値。run ディレクトリの復元は `derive` との一致で決まる。
const TASK_ID: &str = "20260811t091530-k3f9qa1b";

/// 滞留したエージェントの解放が来ないときの上限(ミリ秒)。
///
/// 滞留はラッパーの終了をエージェントの実行中に起こすためのもので、終わりはテストが置く
/// 解放ファイルが決める。この上限は解放し損ねたエージェントを残さないための歯止めであって、
/// 生存の窓の長さではない。
const HOLD_LIMIT_MILLIS: &str = "120000";

/// `agent_probe wait-for` が滞留に入るときに標準出力へ書く合図。
const WAITING: &str = "waiting";

/// `agent_probe wait-for` が解放を観測して終わるときに標準出力へ書く合図。
const RELEASED: &str = "released";

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

/// エージェントの標準出力のログに合図が現れているか。
fn log_has(run_dir: &Path, signal: &str) -> bool {
    fs::read_to_string(run_dir.join("stdout.log"))
        .is_ok_and(|log| log.lines().any(|line| line == signal))
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

    // 126 / 127 / 128+n はラッパー自身が起動不能・コマンド不在・シグナル死に使う符号化値
    // でもある。エージェントがその値で終わったときも書き換えずに通すことを含める。
    for code in [0, 3, 42, 126, 127, 134] {
        let launch = Launch::new();

        wrapper(launch.run_dir(), launch.workspace())
            .agent_cmd(probe(&program, &["exit", &code.to_string()]))
            .run()
            .assert_succeeded();

        assert_eq!(launch.exit_code(), code, "exit code {code}");
    }
}

/// シグナル死の符号化は POSIX 慣例の `128+シグナル番号`。シグナルの概念が無い環境では
/// 符号化値が「非0」までしか決まらないため、具体値の主張は POSIX に限る。
#[test]
fn シグナルで死んだエージェントは非ゼロの符号化値としてexitファイルに現れる() {
    let program = probe_program();
    let launch = Launch::new();

    wrapper(launch.run_dir(), launch.workspace())
        .agent_cmd(probe(&program, &["abort"]))
        .run()
        .assert_succeeded();

    assert!(
        launch.run_dir().join("stdout.log").is_file(),
        "エージェントは起動されている"
    );
    let code = launch.exit_code();
    assert_ne!(code, 0, "非0で符号化される");
    #[cfg(unix)]
    assert_eq!(code, 128 + 6, "SIGABRT で死んだ");
}

/// エージェント実行中にラッパーが死ぬと、attempt に結末が残らない(TC-exec-run-wrapper-027)。
/// 次の tick はこの不在(exit なし・プロセス死亡)から failed を導く。
#[test]
fn エージェントの実行中にラッパーが終了させられるとexitは書かれない() {
    let program = probe_program();
    let release_dir = scratch();
    let release = release_dir.path().join("release");
    let launch = Launch::new();

    let running = wrapper(launch.run_dir(), launch.workspace())
        .agent_cmd(probe(
            &program,
            &[
                "wait-for",
                &release.display().to_string(),
                HOLD_LIMIT_MILLIS,
            ],
        ))
        .start();

    // 滞留の合図を待つ。ログの存在だけではエージェントの起動が済んだことまで決まらない。
    // 合図を待てば、終了させる時点が「エージェント実行中・exit 書き込み前」であることが
    // 実行環境の速さに依存しない。
    let run_dir = launch.run_dir();
    wait_until("エージェントの滞留の合図", &run_dir, || {
        log_has(&run_dir, WAITING)
    });

    running.kill();

    // 解放と、それを観測したエージェントの終わりまでを先に済ませる。exit を書きうる
    // 唯一のプロセスは終了済みなので主張は変わらず、主張が落ちた場合でも一時ディレクトリ
    // の削除と競合するプロセスが残らない。
    fs::write(&release, "").expect("解放ファイルを置ける");
    wait_until("エージェントの解放の合図", &run_dir, || {
        log_has(&run_dir, RELEASED)
    });

    assert_eq!(
        launch.run_files(),
        vec!["pid", "starttime", "stderr.log", "stdout.log"],
        "同定情報とログは残り、exit だけが現れない"
    );
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
fn 実行できない実体を指すエージェントは起動不能として符号化される() {
    let launch = Launch::new();
    let agent = launch.dir.path().join("not-executable");
    fs::write(&agent, "エージェントではない実体").expect("実体を置ける");
    if deny_execute(&agent).is_none() {
        common::skipped("tc_exec_run_wrapper_014", "deny_execute");
        return;
    }

    wrapper(launch.run_dir(), launch.workspace())
        .agent_cmd([agent.into_os_string()])
        .run()
        .assert_succeeded();

    assert_eq!(
        launch.exit_code(),
        126,
        "実体はあるので、コマンド不在(127)とは別の符号化値になる"
    );
}

#[test]
fn ログを開けなければエージェントを起動せず起動不能として符号化される() {
    /// 先置きするログの内容。エージェントが起動されればこの内容は残らない。
    const PLACED: &str = "先に置いた内容";

    let program = probe_program();
    let launch = Launch::new();
    fs::create_dir_all(launch.run_dir()).expect("run ディレクトリを作れる");
    let stdout_log = launch.run_dir().join("stdout.log");
    fs::write(&stdout_log, PLACED).expect("ログを先置きできる");
    if deny_write(&stdout_log).is_none() {
        common::skipped("tc_exec_run_wrapper_016", "deny_write");
        return;
    }

    wrapper(launch.run_dir(), launch.workspace())
        .agent_cmd(probe(&program, &["print", "出力", ""]))
        .run()
        .assert_succeeded();

    assert_eq!(launch.exit_code(), 126, "起動不能として符号化される");
    assert_eq!(
        fs::read_to_string(&stdout_log).ok().as_deref(),
        Some(PLACED),
        "エージェントは起動されない"
    );
    for name in ["starttime", "pid"] {
        assert!(
            launch.run_dir().join(name).is_file(),
            "{name} の書き込みは通る"
        );
    }
}

/// 実行できない実体にする。実際に起動できないことを確かめてから `Some` を返す(ADR-027)。
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

/// 書き込み用に開けないファイルにする。制限が実際に効いたことを確かめてから `Some` を返す。
///
/// 確認は非破壊の開き方で行う — `File::create` は成功したときに先置きした内容を失う。
#[cfg(unix)]
fn deny_write(path: &Path) -> Option<()> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = fs::metadata(path).ok()?.permissions();
    permissions.set_mode(0o444);
    fs::set_permissions(path, permissions).ok()?;

    fs::OpenOptions::new()
        .write(true)
        .open(path)
        .is_err()
        .then_some(())
}

#[cfg(not(unix))]
fn deny_write(_path: &Path) -> Option<()> {
    None
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
