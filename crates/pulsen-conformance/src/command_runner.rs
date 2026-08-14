//! CommandRunner の適合ケース(`spec/testcases/ports/command-runner.md` の16行)。
//!
//! 標準出力・標準エラーは捕捉されない契約なので、コマンドが受け取った引数・環境変数・
//! 作業ディレクトリの観測は exit code かコマンド自身が書き出すファイルで表す。期待結果は
//! 契約の語彙で書き、プラットフォーム固有の具体値には踏み込まない(ADR-082)。

use std::time::{Duration, Instant};

use pulsen_domain::definition::DurationSpec;
use pulsen_domain::execution::{CommandCompletion, CommandRunner, ExitCode};

use crate::{CaseOutcome, CommandBehavior, CommandRunnerHarness, require};

pub fn tc_port_command_runner_001_成功したコマンドの終了結果はゼロになる(
    harness: &impl CommandRunnerHarness,
) -> CaseOutcome {
    let command = require!(harness.command(CommandBehavior::Exit(0)));

    let completion = harness.runner().run(&command, &[], None);

    assert_eq!(completion, CommandCompletion::Exited(ExitCode::new(0)));
    CaseOutcome::Ran
}

pub fn tc_port_command_runner_002_非ゼロの終了コードはそのまま返る(
    harness: &impl CommandRunnerHarness,
) -> CaseOutcome {
    let command = require!(harness.command(CommandBehavior::Exit(5)));

    let completion = harness.runner().run(&command, &[], None);

    assert_eq!(completion, CommandCompletion::Exited(ExitCode::new(5)));
    CaseOutcome::Ran
}

pub fn tc_port_command_runner_003_存在しないコマンドは起動不能として返る(
    harness: &impl CommandRunnerHarness,
) -> CaseOutcome {
    let command = require!(harness.missing_command());

    let completion = harness.runner().run(&command, &[], None);

    match completion {
        CommandCompletion::FailedToStart { message } => {
            assert!(!message.is_empty(), "原因が説明される");
        }
        other => panic!("非0の `Exited` と区別される: {other:?}"),
    }
    CaseOutcome::Ran
}

pub fn tc_port_command_runner_004_実行できない実体は起動不能として返る(
    harness: &impl CommandRunnerHarness,
) -> CaseOutcome {
    let command = require!(harness.non_executable_command());

    let completion = harness.runner().run(&command, &[], None);

    assert!(
        matches!(completion, CommandCompletion::FailedToStart { .. }),
        "起動不能として返る: {completion:?}"
    );
    CaseOutcome::Ran
}

pub fn tc_port_command_runner_005_exitコードを持たない終了は非ゼロの符号化値になる(
    harness: &impl CommandRunnerHarness,
) -> CaseOutcome {
    let command = require!(harness.command(CommandBehavior::Abort));

    let completion = harness.runner().run(&command, &[], None);

    match completion {
        // 具体値(POSIX 慣例の 128+シグナル番号)はプラットフォーム実装の性質であり、
        // 契約が要求するのは「非0の `Exited` になる」ことまで。
        CommandCompletion::Exited(code) => assert!(!code.is_success(), "非0の符号化値: {code:?}"),
        other => panic!("`TimedOut` / `FailedToStart` にならない: {other:?}"),
    }
    CaseOutcome::Ran
}

pub fn tc_port_command_runner_006_シェルのメタ文字は解釈されずリテラルで渡る(
    harness: &impl CommandRunnerHarness,
) -> CaseOutcome {
    let tokens: Vec<String> = METACHARACTER_TOKENS
        .iter()
        .map(|token| (*token).to_owned())
        .collect();
    let command = require!(harness.command(CommandBehavior::CheckArgs(tokens)));

    let completion = harness.runner().run(&command, &[], None);

    assert_eq!(
        completion,
        CommandCompletion::Exited(ExitCode::new(0)),
        "展開・連結・再分割のいずれも起きない = シェルを介さない直接起動"
    );
    CaseOutcome::Ran
}

pub fn tc_port_command_runner_007_プレースホルダは展開されず文字どおり渡る(
    harness: &impl CommandRunnerHarness,
) -> CaseOutcome {
    let tokens: Vec<String> = PLACEHOLDER_TOKENS
        .iter()
        .map(|token| (*token).to_owned())
        .collect();
    let command = require!(harness.command(CommandBehavior::CheckArgs(tokens)));

    let completion = harness.runner().run(&command, &[], None);

    assert_eq!(completion, CommandCompletion::Exited(ExitCode::new(0)));
    CaseOutcome::Ran
}

pub fn tc_port_command_runner_008_呼び出しプロセスの環境が継承される(
    harness: &impl CommandRunnerHarness,
) -> CaseOutcome {
    let (name, value) = require!(harness.caller_env());
    let command = require!(harness.command(CommandBehavior::CheckEnv { name, value }));

    let completion = harness.runner().run(&command, &[], None);

    assert_eq!(completion, CommandCompletion::Exited(ExitCode::new(0)));
    CaseOutcome::Ran
}

pub fn tc_port_command_runner_009_env引数の変数が追加される(
    harness: &impl CommandRunnerHarness,
) -> CaseOutcome {
    let name = require!(harness.absent_env_name());
    let command = require!(harness.command(CommandBehavior::CheckEnv {
        name: name.clone(),
        value: ADDED_VALUE.to_owned(),
    }));

    let completion = harness
        .runner()
        .run(&command, &[(name, ADDED_VALUE.to_owned())], None);

    assert_eq!(completion, CommandCompletion::Exited(ExitCode::new(0)));
    CaseOutcome::Ran
}

pub fn tc_port_command_runner_010_env引数の値が継承環境を上書きする(
    harness: &impl CommandRunnerHarness,
) -> CaseOutcome {
    let (name, inherited) = require!(harness.caller_env());
    let overridden = format!("{inherited}{OVERRIDE_SUFFIX}");
    let command = require!(harness.command(CommandBehavior::CheckEnv {
        name: name.clone(),
        value: overridden.clone(),
    }));

    let completion = harness.runner().run(&command, &[(name, overridden)], None);

    assert_eq!(completion, CommandCompletion::Exited(ExitCode::new(0)));
    CaseOutcome::Ran
}

pub fn tc_port_command_runner_011_作業ディレクトリは呼び出しプロセスのままになる(
    harness: &impl CommandRunnerHarness,
) -> CaseOutcome {
    let cwd = require!(harness.caller_current_dir());
    let command = require!(harness.command(CommandBehavior::CheckCwd(cwd)));

    let completion = harness.runner().run(&command, &[], None);

    assert_eq!(
        completion,
        CommandCompletion::Exited(ExitCode::new(0)),
        "ポートは作業ディレクトリを変更しない"
    );
    CaseOutcome::Ran
}

pub fn tc_port_command_runner_012_timeoutを超えたコマンドは終了させられる(
    harness: &impl CommandRunnerHarness,
) -> CaseOutcome {
    let evidence = require!(harness.evidence_path());
    let command = require!(harness.command(CommandBehavior::Record {
        after: BEYOND_TIMEOUT,
        evidence: evidence.clone(),
    }));

    let completion = harness
        .runner()
        .run(&command, &[], Some(&seconds(SHORT_TIMEOUT_SECS)));

    assert_eq!(completion, CommandCompletion::TimedOut);
    // 証跡は終了直前に書かれる。timeout を過ぎても現れないことが「起動されたプロセスが
    // 終了させられている」ことの観測になる。
    std::thread::sleep(BEYOND_TIMEOUT);
    assert!(
        !evidence.is_file(),
        "timeout 後に生存していない({} が現れない)",
        evidence.display()
    );
    CaseOutcome::Ran
}

pub fn tc_port_command_runner_013_timeout内に終わればそのまま返る(
    harness: &impl CommandRunnerHarness,
) -> CaseOutcome {
    let command = require!(harness.command(CommandBehavior::Exit(0)));

    let completion = harness
        .runner()
        .run(&command, &[], Some(&seconds(GENEROUS_TIMEOUT_SECS)));

    assert_eq!(completion, CommandCompletion::Exited(ExitCode::new(0)));
    CaseOutcome::Ran
}

pub fn tc_port_command_runner_014_timeout未指定なら打ち切られない(
    harness: &impl CommandRunnerHarness,
) -> CaseOutcome {
    let command = require!(harness.command(CommandBehavior::Sleep(COMMAND_RUNTIME)));

    let started = Instant::now();
    let completion = harness.runner().run(&command, &[], None);

    assert!(started.elapsed() >= COMMAND_RUNTIME, "終了まで待つ");
    assert_eq!(completion, CommandCompletion::Exited(ExitCode::new(0)));
    CaseOutcome::Ran
}

pub fn tc_port_command_runner_015_呼び出しはコマンドの終了まで戻らない(
    harness: &impl CommandRunnerHarness,
) -> CaseOutcome {
    let evidence = require!(harness.evidence_path());
    let command = require!(harness.command(CommandBehavior::Record {
        after: COMMAND_RUNTIME,
        evidence: evidence.clone(),
    }));

    let completion = harness.runner().run(&command, &[], None);

    assert_eq!(completion, CommandCompletion::Exited(ExitCode::new(0)));
    assert!(evidence.is_file(), "戻った時点で証跡が観測できる(同期実行)");
    CaseOutcome::Ran
}

pub fn tc_port_command_runner_016_出力は結果に含まれず捕捉されない(
    harness: &impl CommandRunnerHarness,
) -> CaseOutcome {
    let command = require!(harness.command(CommandBehavior::Print {
        stdout: "標準出力の内容".to_owned(),
        stderr: "標準エラーの内容".to_owned(),
    }));

    let completion = harness.runner().run(&command, &[], None);

    // 結果の型は出力を運ぶ変種を持たない。捕捉しない契約は「結果に現れない」ことと
    // 「呼び出しプロセスの出力へ流れる」ことの両面だが、後者は結果からは観測できない —
    // ここで固定できるのは、実装が出力を握り潰して結果へ畳まないことまで。
    assert_eq!(completion, CommandCompletion::Exited(ExitCode::new(0)));
    CaseOutcome::Ran
}

/// シェルを経由すると原形をとどめないトークン。
const METACHARACTER_TOKENS: [&str; 6] = ["*", "$HOME", "&&", ">out.txt", "引数 の 中の 空白", ";"];

/// テンプレートのプレースホルダ文字列。判定・通知コマンドでは展開されない。
const PLACEHOLDER_TOKENS: [&str; 3] = ["{input}", "{model}", "{workspace}"];

/// `env` 引数で足す値。
const ADDED_VALUE: &str = "pulsen-conformance-added";

/// 継承した値と確実に食い違わせるための接尾辞。
const OVERRIDE_SUFFIX: &str = "-overridden";

/// timeout を確実に超えるコマンドの実行時間。
const BEYOND_TIMEOUT: Duration = Duration::from_secs(10);

/// 超過を作るための短い timeout。ポーリング間隔より十分大きく取る(adr ADR-001)。
const SHORT_TIMEOUT_SECS: u64 = 1;

/// 超過しないことが確かな timeout。
const GENEROUS_TIMEOUT_SECS: u64 = 60;

/// コマンドを実行し続けさせる時間。
const COMMAND_RUNTIME: Duration = Duration::from_millis(300);

/// 秒数から期間を作る。
fn seconds(value: u64) -> DurationSpec {
    DurationSpec::parse(&format!("{value}s")).expect("正の秒数は受理される")
}

/// CommandRunner の適合スイートをアダプターに適用する。
#[macro_export]
macro_rules! command_runner_conformance {
    ($setup:expr, $allowed_skips:expr) => {
        use $crate::command_runner as __pulsen_conformance_command_runner;

        $crate::conformance_cases!(
            __pulsen_conformance_command_runner,
            $setup,
            __PULSEN_CONFORMANCE_COMMAND_RUNNER_SKIPS = $allowed_skips,
            [
                tc_port_command_runner_001_成功したコマンドの終了結果はゼロになる,
                tc_port_command_runner_002_非ゼロの終了コードはそのまま返る,
                tc_port_command_runner_003_存在しないコマンドは起動不能として返る,
                tc_port_command_runner_004_実行できない実体は起動不能として返る,
                tc_port_command_runner_005_exitコードを持たない終了は非ゼロの符号化値になる,
                tc_port_command_runner_006_シェルのメタ文字は解釈されずリテラルで渡る,
                tc_port_command_runner_007_プレースホルダは展開されず文字どおり渡る,
                tc_port_command_runner_008_呼び出しプロセスの環境が継承される,
                tc_port_command_runner_009_env引数の変数が追加される,
                tc_port_command_runner_010_env引数の値が継承環境を上書きする,
                tc_port_command_runner_011_作業ディレクトリは呼び出しプロセスのままになる,
                tc_port_command_runner_012_timeoutを超えたコマンドは終了させられる,
                tc_port_command_runner_013_timeout内に終わればそのまま返る,
                tc_port_command_runner_014_timeout未指定なら打ち切られない,
                tc_port_command_runner_015_呼び出しはコマンドの終了まで戻らない,
                tc_port_command_runner_016_出力は結果に含まれず捕捉されない,
            ]
        );
    };
}
