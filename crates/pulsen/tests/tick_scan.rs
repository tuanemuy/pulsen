//! tick の走査と分岐の振る舞い(UC-execution-002 の骨格)。
//!
//! ポートはすべてテストダブルに差し替える(ADR-028)。ロック機構の異常・走査自体の失敗は
//! 実アダプターでは外から作れないため、分岐の網羅はここで行う。
//!
//! 手続きごとの振る舞いは `tick_launch` / `tick_confirm_spawn` / `tick_observe` /
//! `tick_notify` にある。ここで扱うのは走査レベルの性質 — 分岐の選択・1件の失敗が
//! 残りを止めないこと・「1タスク1tick1ステップ」・書き込みを起こさない tick の冪等性。

mod tick_fixture;

use pulsen::application::tick::{TickError, TickIssue, TickOutcome};
use pulsen_conformance::doubles::{
    LockOutcome, ScriptedExclusiveLock, ScriptedProcessController, ScriptedRunStore,
    ScriptedTaskRepository,
};
use pulsen_domain::execution::RunFileError;
use pulsen_domain::task::{ExecutionState, ExecutionStateKind, ReadError, StopReason};

use tick_fixture::{
    Harness, NOW, TASK, absolute, after, at, config_notifying, corrupt_entry, degraded_entry,
    degraded_entry_with, observed_starttime, pid_content, repository, run_dir, starttime, task,
    task_id,
};

#[test]
fn 別の操作がロックを保持していればスキップして状態を変更しない() {
    let harness = Harness {
        lock: ScriptedExclusiveLock::new([LockOutcome::Busy]),
        tasks: repository(vec![task(TASK).workspace().entry()]),
        ..Harness::new()
    };

    assert_eq!(harness.run(), Ok(TickOutcome::Skipped));
    assert!(harness.tasks.saved().is_empty(), "書き込みは発生しない");
}

#[test]
fn ロック機構自体の異常はtickパスの失敗になる() {
    let harness = Harness {
        lock: ScriptedExclusiveLock::new([LockOutcome::Failed {
            message: "ロックファイルを開けない".to_owned(),
        }]),
        ..Harness::new()
    };

    assert_eq!(
        harness.run(),
        Err(TickError::LockFailed {
            message: "ロックファイルを開けない".to_owned(),
        })
    );
    assert!(harness.tasks.saved().is_empty(), "書き込みは発生しない");
}

#[test]
fn 走査自体が失敗すればtickパスの失敗になり状態を変更しない() {
    let harness = Harness {
        tasks: ScriptedTaskRepository::new().with_list_active([Err(ReadError::Io {
            message: "タスクの置き場を読めない".to_owned(),
        })]),
        ..Harness::new()
    };

    assert_eq!(
        harness.run(),
        Err(TickError::Scan {
            message: "タスクの置き場を読めない".to_owned(),
        })
    );
    assert!(harness.tasks.saved().is_empty(), "書き込みは発生しない");
}

#[test]
fn タスクが1件もなければ何も記録されない() {
    let harness = Harness::new();

    assert!(harness.completed().is_empty());
}

#[test]
fn パース不能なタスクファイルは報告のみで書き込まない() {
    let harness = Harness {
        tasks: repository(vec![corrupt_entry(TASK, "JSON として読めない")]),
        ..Harness::new()
    };

    let summary = harness.completed();

    assert_eq!(
        summary.errors,
        vec![TickIssue::CorruptTaskFile {
            path: absolute(&["home", "u", ".pulsen", "state", "tasks"])
                .join(format!("{TASK}.json")),
            message: "JSON として読めない".to_owned(),
        }]
    );
    assert!(harness.tasks.saved().is_empty(), "書き込みは発生しない");
}

#[test]
fn スナップショットだけが読めないタスクは定義依存の判断をスキップして報告される() {
    let harness = Harness {
        tasks: repository(vec![degraded_entry(TASK, "statuses が空")]),
        ..Harness::new()
    };

    let summary = harness.completed();

    assert_eq!(
        summary.errors,
        vec![TickIssue::SnapshotUnreadable {
            task_id: task_id(TASK),
            message: "statuses が空".to_owned(),
        }]
    );
    assert!(harness.tasks.saved().is_empty(), "書き込みは発生しない");
}

#[test]
fn 待機ステータスのタスクには起動待ちでも失敗確定でも何も起こらない() {
    for execution in [ExecutionState::Pending, ExecutionState::Failed] {
        let harness = Harness {
            tasks: repository(vec![
                task(TASK).status("waiting").execution(execution).entry(),
            ]),
            ..Harness::new()
        };

        let summary = harness.completed();

        assert!(summary.is_empty(), "サマリーに現れない");
        assert!(harness.tasks.saved().is_empty(), "書き込みは発生しない");
        assert!(
            harness.worktrees.calls().is_empty(),
            "worktree 操作は行わない"
        );
        assert!(harness.runs.calls().is_empty(), "run ファイルにも触れない");
    }
}

#[test]
fn 判定確定のタスクは次のステータスへ遷移して起動待ちへ戻る() {
    let harness = Harness {
        tasks: repository(vec![task(TASK).completed(1).entry()]),
        ..Harness::new()
    };
    harness.clock.set(after(10));

    let summary = harness.completed();

    assert_eq!(summary.transitioned, vec![task_id(TASK)]);
    let saved = harness.saved(TASK);
    assert_eq!(saved.task_status().as_str(), "done");
    assert_eq!(saved.execution(), &ExecutionState::Pending);
    assert_eq!(saved.updated_at(), after(10));
    assert!(
        harness.processes.calls().is_empty(),
        "遷移の tick はエージェントを起動しない(1タスク1tick1ステップ)"
    );
}

#[test]
fn 通知済みの凍結には何もしない() {
    let harness = Harness {
        config: config_notifying("notify"),
        tasks: repository(vec![
            task(TASK)
                .stopped_notified(StopReason::RetryLimitExceeded, at(NOW))
                .entry(),
        ]),
        ..Harness::new()
    };

    let summary = harness.completed();

    assert!(summary.is_empty(), "サマリーに現れない");
    assert!(harness.tasks.saved().is_empty(), "書き込みは発生しない");
    assert!(
        harness.commands.calls().is_empty(),
        "再通知は未通知のものだけを対象とする"
    );
}

#[test]
fn 終端処理のステータスは配線されるまで何も起こさない() {
    let harness = Harness {
        tasks: repository(vec![
            task(TASK).status("done").workspace().attempt(1).entry(),
        ]),
        ..Harness::new()
    };

    let summary = harness.completed();

    assert!(summary.is_empty(), "サマリーに現れない");
    assert!(harness.tasks.saved().is_empty(), "書き込まない");
    assert!(
        harness.worktrees.calls().is_empty(),
        "worktree 操作を行わない"
    );
}

#[test]
fn 実行状態の異なる複数のタスクがそれぞれの分岐で1ステップずつ処理される() {
    let corrupt = "20260812t090000-corrupt1";
    let degraded = "20260812t090000-degrade1";
    let waiting = "20260812t090000-waiting1";
    let launching = "20260812t090000-launchg1";
    let harness = Harness {
        tasks: repository(vec![
            corrupt_entry(corrupt, "JSON として読めない"),
            degraded_entry(degraded, "statuses が空"),
            task(waiting).status("waiting").entry(),
            task(TASK).workspace().entry(),
            task(launching)
                .workspace()
                .launching(at(NOW))
                .attempt(1)
                .entry(),
        ]),
        runs: ScriptedRunStore::new()
            .with_prepare_attempt([Ok(run_dir(TASK, 1))])
            .with_read_pid_file([Ok(Some(pid_content()))])
            .with_read_starttime([Ok(Some(starttime()))]),
        processes: ScriptedProcessController::new().with_spawn_wrapper([Ok(())]),
        ..Harness::new()
    };

    let summary = harness.completed();

    assert_eq!(summary.launched, vec![task_id(TASK)]);
    assert_eq!(
        summary
            .errors
            .iter()
            .map(issue_task)
            .collect::<Vec<Option<String>>>(),
        vec![None, Some(degraded.to_owned())],
        "破損したファイルとスナップショット破損だけが報告される"
    );
    assert_eq!(
        harness.saved(launching).execution_kind(),
        ExecutionStateKind::Running,
        "launching は spawn 確認へ進む"
    );
    assert_eq!(
        harness.saved(TASK).execution(),
        &ExecutionState::Launching {
            recorded_at: at(NOW)
        },
        "起動待ちのエージェント実行は起動へ進む"
    );
}

#[test]
fn あるタスクの処理が失敗しても残りのタスクは続行する() {
    let broken = "20260812t090000-broken12";
    let harness = Harness {
        tasks: repository(vec![
            task(broken)
                .workspace()
                .launching(at(NOW))
                .attempt(1)
                .entry(),
            task(TASK).workspace().entry(),
        ]),
        runs: ScriptedRunStore::new()
            .with_read_pid_file([Err(RunFileError::Io {
                message: "pid ファイルを読めない".to_owned(),
            })])
            .with_prepare_attempt([Ok(run_dir(TASK, 1))]),
        processes: ScriptedProcessController::new().with_spawn_wrapper([Ok(())]),
        ..Harness::new()
    };

    let summary = harness.completed();

    assert_eq!(
        summary.errors,
        vec![TickIssue::RunFileUnreadable {
            task_id: task_id(broken),
            error: RunFileError::Io {
                message: "pid ファイルを読めない".to_owned(),
            },
        }]
    );
    assert_eq!(summary.launched, vec![task_id(TASK)], "残りは起動される");
}

#[test]
fn 実行可能なタスクが複数あればすべて起動される() {
    let other = "20260812t090000-second12";
    let harness = Harness {
        tasks: repository(vec![
            task(TASK).workspace().entry(),
            task(other).workspace().entry(),
        ]),
        runs: ScriptedRunStore::new()
            .with_prepare_attempt([Ok(run_dir(TASK, 1)), Ok(run_dir(other, 1))]),
        processes: ScriptedProcessController::new().with_spawn_wrapper([Ok(()), Ok(())]),
        ..Harness::new()
    };

    let summary = harness.completed();

    assert_eq!(summary.launched, vec![task_id(TASK), task_id(other)]);
}

#[test]
fn 状態が変化しないタスク群には連続実行しても書き込みが発生しない() {
    let waiting = "20260812t090000-waiting2";
    let launching = "20260812t090000-launchg2";
    let running = "20260812t090000-running2";
    let harness = Harness {
        tasks: repository(vec![
            task(waiting).status("waiting").entry(),
            task(launching)
                .workspace()
                .launching(at(NOW))
                .attempt(1)
                .entry(),
            task(running).running(1).entry(),
        ]),
        runs: ScriptedRunStore::new()
            .with_read_pid_file([Ok(None), Ok(None)])
            .with_read_starttime([Ok(None), Ok(None)])
            .with_read_exit([Ok(None), Ok(None)]),
        processes: ScriptedProcessController::new().with_starttime_of([
            Ok(Some(observed_starttime())),
            Ok(Some(observed_starttime())),
        ]),
        ..Harness::new()
    };
    harness.clock.set(after(10));

    assert!(harness.completed().is_empty(), "1回目");
    assert!(harness.completed().is_empty(), "2回目も同じ判断になる");
    assert!(harness.tasks.saved().is_empty(), "書き込みは発生しない");
}

#[test]
fn スナップショット破損の凍結以外は定義依存の判断をすべてスキップして報告する() {
    for (label, execution) in [
        ("pending", ExecutionState::Pending),
        ("running", ExecutionState::Running),
        ("completed", ExecutionState::Completed),
        (
            "notified",
            ExecutionState::Stopped {
                reason: StopReason::RetryLimitExceeded,
                notified_at: Some(at(NOW)),
            },
        ),
    ] {
        let harness = Harness {
            config: config_notifying("notify"),
            tasks: repository(vec![degraded_entry_with(TASK, "statuses が空", execution)]),
            ..Harness::new()
        };

        let summary = harness.completed();

        assert_eq!(
            summary.errors,
            vec![TickIssue::SnapshotUnreadable {
                task_id: task_id(TASK),
                message: "statuses が空".to_owned(),
            }],
            "{label}"
        );
        assert!(harness.tasks.saved().is_empty(), "{label}: 書き込まない");
        assert!(
            harness.tasks.saved_degraded().is_empty(),
            "{label}: 縮退した保存も行わない"
        );
        assert!(
            harness.commands.calls().is_empty(),
            "{label}: 通知も行わない"
        );
    }
}

#[test]
fn 起動確認済みなのに同定情報がないタスクは報告してスキップする() {
    let missing_attempt = "20260812t090000-noattemp";
    let harness = Harness {
        tasks: repository(vec![
            task(missing_attempt)
                .workspace()
                .execution(ExecutionState::Running)
                .entry(),
            task(TASK)
                .workspace()
                .execution(ExecutionState::Running)
                .attempt(1)
                .entry(),
        ]),
        ..Harness::new()
    };

    let summary = harness.completed();

    assert_eq!(
        summary.errors,
        vec![
            TickIssue::MissingCurrentAttempt {
                task_id: task_id(missing_attempt),
            },
            TickIssue::MissingProcessIdent {
                task_id: task_id(TASK),
            },
        ],
        "不変条件2の破れと3の破れは別の分類として報告される"
    );
    assert!(harness.tasks.saved().is_empty(), "書き込まない");
    assert!(harness.runs.calls().is_empty(), "run ファイルにも触れない");
}

/// 報告が指すタスクID。破損したタスクファイルはIDを持たない。
fn issue_task(issue: &TickIssue) -> Option<String> {
    match issue {
        TickIssue::CorruptTaskFile { .. } => None,
        TickIssue::SnapshotUnreadable { task_id, .. }
        | TickIssue::MissingCurrentAttempt { task_id }
        | TickIssue::Transition { task_id, .. }
        | TickIssue::RunFileUnreadable { task_id, .. }
        | TickIssue::InconsistentRunFiles { task_id, .. }
        | TickIssue::WorktreeCreateFailed { task_id, .. }
        | TickIssue::CommandExpansionFailed { task_id, .. }
        | TickIssue::MarkerWriteFailed { task_id, .. }
        | TickIssue::PrepareAttemptFailed { task_id, .. }
        | TickIssue::SpawnFailed { task_id, .. }
        | TickIssue::SpawnNotObserved { task_id, .. }
        | TickIssue::MissingProcessIdent { task_id }
        | TickIssue::ObservationFailed { task_id, .. }
        | TickIssue::KillFailed { task_id, .. }
        | TickIssue::RemnantsUnhandled { task_id, .. }
        | TickIssue::JudgeFailed { task_id, .. }
        | TickIssue::RunFailed { task_id, .. }
        | TickIssue::NotifyFailed { task_id, .. }
        | TickIssue::SaveFailed { task_id, .. } => Some(task_id.as_str().to_owned()),
    }
}
