//! ラッパーモードのユースケースの振る舞い。
//!
//! ポートはすべてテストダブルに差し替える。実プロセス・実ファイルシステムでは
//! 外から作れない状況(同定情報の取得失敗・書き込みの失敗・マーカー確認の失敗)まで含めて
//! 分岐を網羅する。書き込みの**順序**は結果の値に現れないため、呼び出しの記録で主張する。

use std::path::PathBuf;

use pulsen::application::run_wrapper::{RunWrapper, WrapperOutcome};
use pulsen_conformance::doubles::{
    ProcessControllerCall, RunStoreCall, ScriptedProcessController, ScriptedRunStore,
};
use pulsen_domain::definition::CommandLine;
use pulsen_domain::execution::{ExitCode, Io, PidFileContent, WrapperIdentity, WrapperLaunchSpec};
use pulsen_domain::task::{
    AttemptNumber, KillIdent, Pid, ProcessStartTime, RunDirPath, StartTimeRecord, StateRoot,
    TaskId, Timestamp, WorktreePath,
};

fn absolute(segments: &[&str]) -> PathBuf {
    let mut path = if std::path::MAIN_SEPARATOR == '\\' {
        PathBuf::from("C:\\")
    } else {
        PathBuf::from("/")
    };
    for segment in segments {
        path.push(segment);
    }
    path
}

fn run_dir() -> RunDirPath {
    RunDirPath::derive(
        &StateRoot::parse(absolute(&["home", "u", ".pulsen", "state"])).expect("受理される"),
        &TaskId::parse("20260811t091530-k3f9qa1b".to_owned()).expect("受理される"),
        AttemptNumber::parse(2).expect("受理される"),
    )
}

fn workspace() -> WorktreePath {
    WorktreePath::parse(absolute(&["home", "u", ".pulsen", "worktrees", "t1"])).expect("受理される")
}

fn agent_cmd() -> CommandLine {
    CommandLine::rehydrate(vec!["claude".to_owned(), "実装して".to_owned()]).expect("受理される")
}

fn spec() -> WrapperLaunchSpec {
    WrapperLaunchSpec::new(run_dir(), agent_cmd(), workspace())
}

fn starttime() -> StartTimeRecord {
    StartTimeRecord::new(
        ProcessStartTime::parse("Wed Aug 12 11:44:13 2026".to_owned()).expect("受理される"),
        Timestamp::parse_rfc3339("2026-08-12T11:44:14Z").expect("受理される"),
    )
}

fn identity() -> WrapperIdentity {
    WrapperIdentity::new(
        Pid::new(4242),
        KillIdent::parse("-4242".to_owned()).expect("受理される"),
        starttime(),
    )
}

fn pid_content() -> PidFileContent {
    PidFileContent::new(
        Pid::new(4242),
        KillIdent::parse("-4242".to_owned()).expect("受理される"),
    )
}

/// 同定情報を取得でき、書き込みもすべて成功するストアとコントローラー。
fn ready(exit: ExitCode) -> (ScriptedRunStore, ScriptedProcessController) {
    (
        ScriptedRunStore::new()
            .with_write_starttime([Ok(())])
            .with_write_pid_file([Ok(())])
            .with_marker_exists([Ok(false)])
            .with_write_exit([Ok(())]),
        ScriptedProcessController::new()
            .with_own_identity([Ok(identity())])
            .with_run_agent([exit]),
    )
}

fn execute(runs: &ScriptedRunStore, processes: &ScriptedProcessController) -> WrapperOutcome {
    RunWrapper::new(runs, processes).execute(&spec())
}

#[test]
fn 同定情報の記録はstarttimeを先に書きマーカー確認はpidの後に行われる() {
    let (runs, processes) = ready(ExitCode::new(0));

    assert_eq!(
        execute(&runs, &processes),
        WrapperOutcome::Ran(ExitCode::new(0))
    );

    assert_eq!(
        runs.calls(),
        vec![
            RunStoreCall::WriteStarttime {
                run_dir: run_dir(),
                record: starttime(),
            },
            RunStoreCall::WritePidFile {
                run_dir: run_dir(),
                content: pid_content(),
            },
            RunStoreCall::MarkerExists { run_dir: run_dir() },
            RunStoreCall::WriteExit {
                run_dir: run_dir(),
                code: ExitCode::new(0),
            },
        ]
    );
}

#[test]
fn 同定情報は取得してからエージェントを起動するまでの間に記録される() {
    let (runs, processes) = ready(ExitCode::new(0));

    execute(&runs, &processes);

    assert_eq!(
        processes.calls(),
        vec![
            ProcessControllerCall::OwnIdentity,
            ProcessControllerCall::RunAgent {
                cmd: agent_cmd(),
                cwd: workspace(),
                stdout: run_dir().stdout_log(),
                stderr: run_dir().stderr_log(),
            },
        ],
        "作業ディレクトリは worktree、ログの位置は run ディレクトリの導出に従う"
    );
}

#[test]
fn 無効化マーカーがあればエージェントを起動せず正常に終える() {
    let runs = ScriptedRunStore::new()
        .with_write_starttime([Ok(())])
        .with_write_pid_file([Ok(())])
        .with_marker_exists([Ok(true)]);
    let processes = ScriptedProcessController::new().with_own_identity([Ok(identity())]);

    assert_eq!(execute(&runs, &processes), WrapperOutcome::Suppressed);

    assert_eq!(
        processes.calls(),
        vec![ProcessControllerCall::OwnIdentity],
        "エージェントは起動されない"
    );
    assert!(
        !runs
            .calls()
            .iter()
            .any(|call| matches!(call, RunStoreCall::WriteExit { .. })),
        "exit も書かれない"
    );
}

#[test]
fn マーカーの確認自体が失敗した場合もエージェントを起動しない() {
    let runs = ScriptedRunStore::new()
        .with_write_starttime([Ok(())])
        .with_write_pid_file([Ok(())])
        .with_marker_exists([Err(Io::Failed {
            message: "マーカーを確認できない".to_owned(),
        })]);
    let processes = ScriptedProcessController::new().with_own_identity([Ok(identity())]);

    assert_eq!(execute(&runs, &processes), WrapperOutcome::Suppressed);

    assert_eq!(processes.calls(), vec![ProcessControllerCall::OwnIdentity]);
}

#[test]
fn 同定情報を取得できなければ何も書き残さずに終える() {
    let runs = ScriptedRunStore::new();
    let processes = ScriptedProcessController::new().with_own_identity([Err(Io::Failed {
        message: "取得元を起動できない".to_owned(),
    })]);

    assert_eq!(execute(&runs, &processes), WrapperOutcome::Silent);

    assert_eq!(runs.calls(), Vec::new());
    assert_eq!(processes.calls(), vec![ProcessControllerCall::OwnIdentity]);
}

#[test]
fn starttimeを書けなければpidを書かずに終える() {
    let runs = ScriptedRunStore::new().with_write_starttime([Err(Io::Failed {
        message: "書き込めない".to_owned(),
    })]);
    let processes = ScriptedProcessController::new().with_own_identity([Ok(identity())]);

    assert_eq!(execute(&runs, &processes), WrapperOutcome::Silent);

    assert_eq!(
        runs.calls(),
        vec![RunStoreCall::WriteStarttime {
            run_dir: run_dir(),
            record: starttime(),
        }]
    );
    assert_eq!(processes.calls(), vec![ProcessControllerCall::OwnIdentity]);
}

#[test]
fn pidを書けなければマーカーを確認せずエージェントも起動しない() {
    let runs = ScriptedRunStore::new()
        .with_write_starttime([Ok(())])
        .with_write_pid_file([Err(Io::Failed {
            message: "書き込めない".to_owned(),
        })]);
    let processes = ScriptedProcessController::new().with_own_identity([Ok(identity())]);

    assert_eq!(execute(&runs, &processes), WrapperOutcome::Silent);

    assert_eq!(
        runs.calls(),
        vec![
            RunStoreCall::WriteStarttime {
                run_dir: run_dir(),
                record: starttime(),
            },
            RunStoreCall::WritePidFile {
                run_dir: run_dir(),
                content: pid_content(),
            },
        ],
        "starttime のみが残る中間状態で終える"
    );
    assert_eq!(processes.calls(), vec![ProcessControllerCall::OwnIdentity]);
}

#[test]
fn exitを書けなくてもエージェントの終了結果は変わらない() {
    let runs = ScriptedRunStore::new()
        .with_write_starttime([Ok(())])
        .with_write_pid_file([Ok(())])
        .with_marker_exists([Ok(false)])
        .with_write_exit([Err(Io::Failed {
            message: "書き込めない".to_owned(),
        })]);
    let processes = ScriptedProcessController::new()
        .with_own_identity([Ok(identity())])
        .with_run_agent([ExitCode::new(3)]);

    assert_eq!(
        execute(&runs, &processes),
        WrapperOutcome::Ran(ExitCode::new(3))
    );
}

#[test]
fn エージェントの終了結果は解釈されずそのまま書かれる() {
    for value in [0, 1, 126, 127, 128 + 6] {
        let (runs, processes) = ready(ExitCode::new(value));

        assert_eq!(
            execute(&runs, &processes),
            WrapperOutcome::Ran(ExitCode::new(value)),
            "{value}"
        );
        assert_eq!(
            runs.calls().last(),
            Some(&RunStoreCall::WriteExit {
                run_dir: run_dir(),
                code: ExitCode::new(value),
            }),
            "{value}"
        );
    }
}
