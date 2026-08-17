//! 手続きC(spawn 確認)の振る舞い。
//!
//! ポートはすべてテストダブルに差し替える。猶予時間の境界は実時間で待てず、
//! run ファイルの読み取り失敗も実ファイルシステムでは外から作れない。
//!
//! 「マーカーを書いてから pid を再確認する」順序は結果の値に現れないため、呼び出しの
//! 記録で主張する — 順序が逆になると、遅れて起動したラッパーと次の attempt が並走する。

mod usecase_fixture;

use pulsen::application::tick::TickIssue;
use pulsen_conformance::doubles::{RunStoreCall, ScriptedRunStore};
use pulsen_domain::execution::{InconsistentRunFiles, Io, RunFileError};
use pulsen_domain::task::{
    AttemptRef, ExecutionState, ExecutionStateKind, FailureKind, FailureNote, StopReason,
};

use usecase_fixture::{
    Harness, NOW, TASK, after, at, config_with, pid_content, process_ident, repository,
    repository_failing_save, run_dir, starttime, task, task_id,
};

/// 起動記録済み・attempt 1 のタスク1件を走査する。
fn launching(runs: ScriptedRunStore) -> Harness {
    Harness {
        tasks: repository(vec![
            task(TASK).workspace().launching(at(NOW)).attempt(1).entry(),
        ]),
        runs,
        ..Harness::new()
    }
}

/// 観測が「不在」になるストア。
fn absent() -> ScriptedRunStore {
    ScriptedRunStore::new()
        .with_read_pid_file([Ok(None)])
        .with_read_starttime([Ok(None)])
}

#[test]
fn pidとstarttimeが揃っていれば同定情報を取り込んでrunningになる() {
    let harness = launching(
        ScriptedRunStore::new()
            .with_read_pid_file([Ok(Some(pid_content()))])
            .with_read_starttime([Ok(Some(starttime()))]),
    );

    let summary = harness.completed();

    let saved = harness.saved(TASK);
    assert_eq!(saved.execution(), &ExecutionState::Running);
    assert_eq!(
        saved.current_attempt().and_then(AttemptRef::process),
        Some(&process_ident())
    );
    assert_eq!(saved.counters().spawn_fail_count(), 0);
    assert!(summary.errors.is_empty());
    assert_eq!(
        summary.confirmed_running,
        vec![task_id(TASK)],
        "取り込みを行った tick は処理対象なしにならない"
    );
}

#[test]
fn 起動確認は起動時のspawn失敗カウンタだけをリセットする() {
    let harness = Harness {
        tasks: repository(vec![
            task(TASK)
                .workspace()
                .launching(at(NOW))
                .attempt(1)
                .counters(1, 2, 3)
                .entry(),
        ]),
        runs: ScriptedRunStore::new()
            .with_read_pid_file([Ok(Some(pid_content()))])
            .with_read_starttime([Ok(Some(starttime()))]),
        ..Harness::new()
    };

    harness.completed();

    let counters = harness.saved(TASK).counters();
    assert_eq!(counters.spawn_fail_count(), 0);
    assert_eq!(counters.attempt_count(), 1);
    assert_eq!(counters.judge_attempt_count(), 2);
}

#[test]
fn 猶予時間内にpidが現れていなければ書き込みを一切行わない() {
    let harness = launching(absent());
    harness.clock.set(after(30));

    let summary = harness.completed();

    assert!(summary.is_empty(), "サマリーに現れない");
    assert!(harness.tasks.saved().is_empty(), "書き込みは発生しない");
    assert_eq!(
        harness.runs.calls(),
        vec![
            RunStoreCall::ReadPidFile {
                run_dir: run_dir(TASK, 1),
            },
            RunStoreCall::ReadStarttime {
                run_dir: run_dir(TASK, 1),
            },
        ],
        "マーカーも書かない"
    );
}

#[test]
fn 猶予時間の超過は経過が30秒を上回ったときにだけ成立する() {
    let cases = [(30, false), (31, true), (-3600, false)];

    for (elapsed, exceeded) in cases {
        let harness = launching(
            ScriptedRunStore::new()
                .with_read_pid_file([Ok(None), Ok(None)])
                .with_read_starttime([Ok(None), Ok(None)])
                .with_write_invalidation_marker([Ok(())]),
        );
        harness.clock.set(after(elapsed));

        harness.completed();

        let wrote_marker = harness
            .runs
            .calls()
            .iter()
            .any(|call| matches!(call, RunStoreCall::WriteInvalidationMarker { .. }));
        assert_eq!(wrote_marker, exceeded, "経過 {elapsed} 秒");
    }
}

#[test]
fn starttimeだけが現れている状態は猶予時間の判断に合流する() {
    let harness = launching(
        ScriptedRunStore::new()
            .with_read_pid_file([Ok(None)])
            .with_read_starttime([Ok(Some(starttime()))]),
    );
    harness.clock.set(after(30));

    assert!(harness.completed().is_empty());
    assert!(harness.tasks.saved().is_empty());
}

#[test]
fn 起動記録の後にクラッシュした状態は猶予時間の判断に合流する() {
    // run ディレクトリを用意する前のクラッシュ(ディレクトリ不在)も、用意した後・spawn
    // 前のクラッシュ(空のディレクトリ)も、read 系には同じ `Ok(None)` として現れる。
    let harness = launching(absent());
    harness.clock.set(after(1));

    assert!(harness.completed().is_empty());
    assert!(harness.tasks.saved().is_empty());
}

#[test]
fn 猶予超過ではマーカーを書いてからpidを再確認する() {
    let harness = launching(
        ScriptedRunStore::new()
            .with_read_pid_file([Ok(None), Ok(None)])
            .with_read_starttime([Ok(None), Ok(None)])
            .with_write_invalidation_marker([Ok(())]),
    );
    harness.clock.set(after(31));

    harness.completed();

    let run_dir = run_dir(TASK, 1);
    assert_eq!(
        harness.runs.calls(),
        vec![
            RunStoreCall::ReadPidFile {
                run_dir: run_dir.clone(),
            },
            RunStoreCall::ReadStarttime {
                run_dir: run_dir.clone(),
            },
            RunStoreCall::WriteInvalidationMarker {
                run_dir: run_dir.clone(),
            },
            RunStoreCall::ReadPidFile {
                run_dir: run_dir.clone(),
            },
            RunStoreCall::ReadStarttime { run_dir },
        ]
    );
}

#[test]
fn 再確認でもpidが無ければspawn失敗として起動待ちへ戻す() {
    let harness = launching(
        ScriptedRunStore::new()
            .with_read_pid_file([Ok(None), Ok(None)])
            .with_read_starttime([Ok(None), Ok(None)])
            .with_write_invalidation_marker([Ok(())]),
    );
    harness.clock.set(after(31));

    let summary = harness.completed();

    let saved = harness.saved(TASK);
    assert_eq!(saved.execution(), &ExecutionState::Pending);
    assert_eq!(saved.counters().spawn_fail_count(), 1);
    assert_eq!(
        saved.last_failure().map(FailureNote::kind),
        Some(FailureKind::SpawnFail)
    );
    assert_eq!(
        saved.current_attempt().map(AttemptRef::number),
        Some(usecase_fixture::attempt_number(1)),
        "attempt 参照は起動記録でのみ置き換わる"
    );
    assert!(summary.frozen.is_empty());
    assert!(
        matches!(
            summary.errors.as_slice(),
            [TickIssue::SpawnNotObserved { .. }]
        ),
        "起動待ちへ戻した tick は処理対象なしにならない"
    );
}

#[test]
fn マーカー書き込み後の再読でpidが現れていれば起動待ちへ戻さずrunningへ取り込む() {
    let harness = launching(
        ScriptedRunStore::new()
            .with_read_pid_file([Ok(None), Ok(Some(pid_content()))])
            .with_read_starttime([Ok(None), Ok(Some(starttime()))])
            .with_write_invalidation_marker([Ok(())]),
    );
    harness.clock.set(after(31));

    harness.completed();

    assert_eq!(
        harness.saved(TASK).execution_kind(),
        ExecutionStateKind::Running
    );
}

#[test]
fn マーカーを書けなければ状態を変更せず報告してスキップする() {
    let harness = launching(
        ScriptedRunStore::new()
            .with_read_pid_file([Ok(None)])
            .with_read_starttime([Ok(None)])
            .with_write_invalidation_marker([Err(Io::Failed {
                message: "マーカーを書けない".to_owned(),
            })]),
    );
    harness.clock.set(after(31));

    let summary = harness.completed();

    assert_eq!(
        summary.errors,
        vec![TickIssue::MarkerWriteFailed {
            task_id: task_id(TASK),
            message: "マーカーを書けない".to_owned(),
        }]
    );
    assert!(
        harness.tasks.saved().is_empty(),
        "マーカー無しで起動待ちへ戻すと遅延起動したラッパーが並走しうる"
    );
}

#[test]
fn spawn失敗の加算が上限を超えるとタスクは凍結される() {
    let harness = Harness {
        tasks: repository(vec![
            task(TASK)
                .workspace()
                .launching(at(NOW))
                .attempt(1)
                .counters(0, 0, 3)
                .entry(),
        ]),
        config: config_with(vec![("claude", "claude {input}", None)], Some(3)),
        runs: ScriptedRunStore::new()
            .with_read_pid_file([Ok(None), Ok(None)])
            .with_read_starttime([Ok(None), Ok(None)])
            .with_write_invalidation_marker([Ok(())]),
        ..Harness::new()
    };
    harness.clock.set(after(31));

    let summary = harness.completed();

    assert_eq!(
        harness.saved(TASK).execution(),
        &ExecutionState::Stopped {
            reason: StopReason::SpawnFailLimitExceeded,
            notified_at: None,
        }
    );
    assert_eq!(summary.frozen, vec![task_id(TASK)]);
}

#[test]
fn runファイルを読めなければ報告してスキップする() {
    let failures = [
        RunFileError::Corrupt {
            path: run_dir(TASK, 1).pid_file(),
            message: "JSON として読めない".to_owned(),
        },
        RunFileError::Io {
            message: "pid ファイルを読めない".to_owned(),
        },
    ];

    for error in failures {
        let harness = launching(ScriptedRunStore::new().with_read_pid_file([Err(error.clone())]));

        let summary = harness.completed();

        assert_eq!(
            summary.errors,
            vec![TickIssue::RunFileUnreadable {
                task_id: task_id(TASK),
                error: error.clone(),
            }]
        );
        assert!(harness.tasks.saved().is_empty(), "書き込まない");
    }
}

#[test]
fn starttimeを読めなければ報告してスキップする() {
    let error = RunFileError::Corrupt {
        path: run_dir(TASK, 1).starttime_file(),
        message: "JSON として読めない".to_owned(),
    };
    let harness = launching(
        ScriptedRunStore::new()
            .with_read_pid_file([Ok(None)])
            .with_read_starttime([Err(error.clone())]),
    );

    let summary = harness.completed();

    assert_eq!(
        summary.errors,
        vec![TickIssue::RunFileUnreadable {
            task_id: task_id(TASK),
            error,
        }]
    );
    assert!(harness.tasks.saved().is_empty(), "書き込まない");
}

#[test]
fn マーカーを書いた後にstarttimeを読めなければ報告してスキップする() {
    let error = RunFileError::Io {
        message: "starttime ファイルを読めない".to_owned(),
    };
    let harness = launching(
        ScriptedRunStore::new()
            .with_read_pid_file([Ok(None), Ok(None)])
            .with_read_starttime([Ok(None), Err(error.clone())])
            .with_write_invalidation_marker([Ok(())]),
    );
    harness.clock.set(after(31));

    let summary = harness.completed();

    assert_eq!(
        summary.errors,
        vec![TickIssue::RunFileUnreadable {
            task_id: task_id(TASK),
            error,
        }]
    );
    assert!(
        harness.tasks.saved().is_empty(),
        "再読が成立しない観測から spawn 失敗を確定させない"
    );
}

#[test]
fn runファイルの破損が続く限り繰り返しても報告とスキップだけが続く() {
    let error = RunFileError::Corrupt {
        path: run_dir(TASK, 1).pid_file(),
        message: "JSON として読めない".to_owned(),
    };
    let harness = launching(
        ScriptedRunStore::new().with_read_pid_file(std::iter::repeat_n(Err(error.clone()), 3)),
    );
    harness.clock.set(after(31));

    for round in 1..=3 {
        let summary = harness.completed();

        assert_eq!(
            summary.errors,
            vec![TickIssue::RunFileUnreadable {
                task_id: task_id(TASK),
                error: error.clone(),
            }],
            "{round}回目"
        );
        assert!(summary.frozen.is_empty(), "{round}回目: 凍結しない");
        assert!(
            harness.tasks.saved().is_empty(),
            "{round}回目: 書き込まない"
        );
    }
}

#[test]
fn 破損したrunファイルが削除されれば不在として無効化マーカー経路に合流する() {
    let error = RunFileError::Corrupt {
        path: run_dir(TASK, 1).pid_file(),
        message: "JSON として読めない".to_owned(),
    };
    let harness = launching(
        ScriptedRunStore::new()
            .with_read_pid_file([Err(error.clone()), Ok(None), Ok(None)])
            .with_read_starttime([Ok(None), Ok(None)])
            .with_write_invalidation_marker([Ok(())]),
    );
    harness.clock.set(after(31));

    let reported = harness.completed();
    assert_eq!(
        reported.errors,
        vec![TickIssue::RunFileUnreadable {
            task_id: task_id(TASK),
            error,
        }]
    );
    assert!(harness.tasks.saved().is_empty(), "破損の間は書き込まない");

    let decided = harness.completed();

    let saved = harness.saved(TASK);
    assert_eq!(saved.execution(), &ExecutionState::Pending);
    assert_eq!(saved.counters().spawn_fail_count(), 1);
    assert!(matches!(
        decided.errors.as_slice(),
        [TickIssue::SpawnNotObserved { .. }]
    ));
}

#[test]
fn pidだけが現れている観測は書き込み順序の破れとして報告される() {
    let harness = launching(
        ScriptedRunStore::new()
            .with_read_pid_file([Ok(Some(pid_content()))])
            .with_read_starttime([Ok(None)]),
    );

    let summary = harness.completed();

    assert_eq!(
        summary.errors,
        vec![TickIssue::InconsistentRunFiles {
            task_id: task_id(TASK),
            kind: InconsistentRunFiles::MissingStartTime,
        }]
    );
    assert!(harness.tasks.saved().is_empty(), "書き込まない");
}

#[test]
fn 再読でpidだけが現れている観測も書き込み順序の破れとして報告される() {
    let harness = launching(
        ScriptedRunStore::new()
            .with_read_pid_file([Ok(None), Ok(Some(pid_content()))])
            .with_read_starttime([Ok(None), Ok(None)])
            .with_write_invalidation_marker([Ok(())]),
    );
    harness.clock.set(after(31));

    let summary = harness.completed();

    assert!(matches!(
        summary.errors.as_slice(),
        [TickIssue::InconsistentRunFiles { .. }]
    ));
    assert!(harness.tasks.saved().is_empty(), "書き込まない");
}

#[test]
fn 現在attemptのない起動記録済みタスクは不変条件の破れとして報告される() {
    let harness = Harness {
        tasks: repository(vec![task(TASK).workspace().launching(at(NOW)).entry()]),
        ..Harness::new()
    };

    let summary = harness.completed();

    assert_eq!(
        summary.errors,
        vec![TickIssue::MissingCurrentAttempt {
            task_id: task_id(TASK),
        }]
    );
    assert!(harness.runs.calls().is_empty(), "run ファイルにも触れない");
    assert!(harness.tasks.saved().is_empty(), "書き込まない");
}

#[test]
fn spawn失敗の記録の保存に失敗すれば報告して起動待ちへ戻さない() {
    let harness = Harness {
        tasks: repository_failing_save(vec![
            task(TASK).workspace().launching(at(NOW)).attempt(1).entry(),
        ]),
        runs: ScriptedRunStore::new()
            .with_read_pid_file([Ok(None), Ok(None)])
            .with_read_starttime([Ok(None), Ok(None)])
            .with_write_invalidation_marker([Ok(())]),
        ..Harness::new()
    };
    harness.clock.set(after(31));

    let summary = harness.completed();

    assert!(matches!(
        summary.errors.as_slice(),
        [TickIssue::SaveFailed { .. }]
    ));
}

#[test]
fn 起動確認の保存に失敗すれば報告して次のタスクへ進む() {
    let harness = Harness {
        tasks: repository_failing_save(vec![
            task(TASK).workspace().launching(at(NOW)).attempt(1).entry(),
        ]),
        runs: ScriptedRunStore::new()
            .with_read_pid_file([Ok(Some(pid_content()))])
            .with_read_starttime([Ok(Some(starttime()))]),
        ..Harness::new()
    };

    let summary = harness.completed();

    assert!(matches!(
        summary.errors.as_slice(),
        [TickIssue::SaveFailed { .. }]
    ));
    assert!(
        summary.confirmed_running.is_empty(),
        "永続化できていない取り込みは数えない"
    );
}
