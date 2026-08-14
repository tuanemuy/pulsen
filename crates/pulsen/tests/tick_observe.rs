//! 手続きD(観測・判定)の振る舞い(UC-execution-006)。
//!
//! ポートはすべてテストダブルに差し替える(ADR-028)。実アダプターでは外から作れない分岐
//! — `read_exit` の破損・`starttime_of` の機構失敗・`kill` の失敗・残存終了の3値・判定
//! コマンドの timeout と起動不能・timeout の境界 — はここでしか主張できない。

mod tick_fixture;

use pulsen::application::tick::{RemnantsLeft, RunFailureCause, TickIssue, TickSummary};
use pulsen_conformance::doubles::{
    ProcessControllerCall, ScriptedCommandRunner, ScriptedProcessController, ScriptedRunStore,
    ScriptedTaskRepository,
};
use pulsen_domain::definition::{PlainCommand, StatusDefinition, TimeoutSpec, WorkflowDefinition};
use pulsen_domain::execution::{
    CommandCompletion, ExitCode, KillError, RemnantOutcome, RunFileError,
};
use pulsen_domain::task::{
    AttemptRef, ExecutionState, ExecutionStateKind, RetryCounters, SaveError, StopReason, Task,
    TaskEntry, TaskFields, TaskRecord, WorkspacePlanner,
};

use tick_fixture::{
    AgentRunSpec, Harness, NOW, TASK, after, agent_run, at, attempt_number, command,
    config_judging, kill_ident, observed_starttime, process_ident, repository,
    repository_failing_save, reused_starttime, run_dir, snapshot_with, status, target, task,
    task_id, timeout_secs, workflow_name, worktree_root,
};

/// exit ファイルの読み取りだけを台本に持つストア。
fn exit_of(code: Option<i32>) -> ScriptedRunStore {
    ScriptedRunStore::new().with_read_exit([Ok(code.map(ExitCode::new))])
}

/// 判定コマンドを持たない running タスク1件のハーネス。
fn running_with(runs: ScriptedRunStore) -> Harness {
    Harness {
        tasks: repository(vec![task(TASK).running(1).entry()]),
        runs,
        ..Harness::new()
    }
}

/// ステータス定義を差し替えた running タスク1件のハーネス。
fn running_status(status: StatusDefinition, runs: ScriptedRunStore) -> Harness {
    Harness {
        tasks: repository(vec![
            task(TASK)
                .snapshot(snapshot_with(Some("claude"), None, status))
                .running(1)
                .entry(),
        ]),
        runs,
        ..Harness::new()
    }
}

/// 判定コマンドつきのエージェント実行ステータス。
fn judged_status(judge: &str) -> StatusDefinition {
    agent_run(AgentRunSpec {
        judge: Some(command(judge)),
        ..AgentRunSpec::default()
    })
}

/// workspace が未確定のまま起動確認済みになったタスク(不変条件4の破れ)。
///
/// 起動確認済みのタスクを組む経路は必ず workspace を確定させるため、ここだけ帳簿を直に組む。
fn running_without_workspace(judge: StatusDefinition) -> TaskEntry {
    let task = Task::rehydrate(TaskFields {
        id: task_id(TASK),
        workflow_name: workflow_name(),
        target: target(),
        snapshot: snapshot_with(Some("claude"), None, judge),
        task_status: status("queued"),
        execution: ExecutionState::Running,
        workspace: None,
        current_attempt: Some(AttemptRef::rehydrate(
            attempt_number(1),
            run_dir(TASK, 1),
            Some(process_ident()),
        )),
        counters: RetryCounters::initial(),
        last_failure: None,
        updated_at: at(NOW),
    })
    .expect("不変条件1を満たす");
    TaskEntry::Record(TaskRecord::Intact(task))
}

#[test]
fn 終了コード0はデフォルト判定で判定確定になりカウンタが0に戻る() {
    let harness = Harness {
        tasks: repository(vec![task(TASK).running(1).counters(2, 1, 3).entry()]),
        runs: exit_of(Some(0)),
        ..Harness::new()
    };

    let summary = harness.completed();

    assert_eq!(summary.judged, vec![task_id(TASK)]);
    assert!(
        summary.transitioned.is_empty(),
        "遷移は次の tick に委ねる(1タスク1tick1ステップ)"
    );
    let saved = harness.saved(TASK);
    assert_eq!(saved.execution(), &ExecutionState::Completed);
    assert_eq!(saved.counters(), RetryCounters::rehydrate(0, 0, 3));
    assert_eq!(saved.task_status().as_str(), "queued");
}

#[test]
fn 終了コードが非0ならデフォルト判定で失敗確定になる() {
    let harness = Harness {
        tasks: repository(vec![task(TASK).running(1).counters(0, 2, 0).entry()]),
        runs: exit_of(Some(1)),
        ..Harness::new()
    };

    let summary = harness.completed();

    let saved = harness.saved(TASK);
    assert_eq!(saved.execution(), &ExecutionState::Failed);
    assert_eq!(
        saved.counters(),
        RetryCounters::rehydrate(1, 0, 0),
        "判定は成立しているので判定のカウンタは 0 に戻る"
    );
    assert!(reported_run_failure(&summary), "{:?}", summary.errors);
}

#[test]
fn デフォルト判定は終了コード20も失敗として扱う() {
    let harness = running_with(exit_of(Some(20)));

    harness.completed();

    assert_eq!(
        harness.saved(TASK).execution(),
        &ExecutionState::Failed,
        "skipped は判定コマンドでのみ生じる"
    );
}

#[test]
fn シグナル死の符号化値も非0として失敗に分類される() {
    let harness = running_with(exit_of(Some(128 + 15)));

    harness.completed();

    assert_eq!(harness.saved(TASK).execution(), &ExecutionState::Failed);
}

#[test]
fn 終了コードがあれば生存を観測せずに判定へ進む() {
    let harness = running_with(exit_of(Some(0)));

    harness.completed();

    assert!(
        harness.processes.calls().is_empty(),
        "観測の一過性失敗で判定を遅延させない(2段規則)"
    );
}

#[test]
fn 判定コマンドは環境変数と判定timeoutつきでシェルを介さず起動される() {
    let mut harness = running_status(judged_status("judge.sh --strict"), exit_of(Some(7)));
    harness.commands =
        ScriptedCommandRunner::new().with_run([CommandCompletion::Exited(ExitCode::new(0))]);

    harness.completed();

    let call = harness.commands.calls().pop().expect("判定が実行される");
    assert_eq!(
        call.cmd,
        PlainCommand::parse_text("judge.sh --strict").expect("受理される")
    );
    let workspace = WorkspacePlanner::derive(&worktree_root(), &task_id(TASK));
    assert_eq!(
        call.env,
        vec![
            ("TASK_ID".to_owned(), TASK.to_owned()),
            (
                "WORKSPACE".to_owned(),
                workspace.path().as_path().to_string_lossy().into_owned()
            ),
            ("EXIT_CODE".to_owned(), "7".to_owned()),
            (
                "RUN_DIR".to_owned(),
                run_dir(TASK, 1).as_path().to_string_lossy().into_owned()
            ),
        ]
    );
    assert_eq!(call.timeout.map(|spec| spec.seconds()), Some(60));
}

#[test]
fn 判定コマンドの終了コードは4値のプロトコルとして帳簿に反映される() {
    for (code, expected) in [
        (0, ExecutionStateKind::Completed),
        (10, ExecutionStateKind::Failed),
        (20, ExecutionStateKind::Pending),
    ] {
        let mut harness = running_status(judged_status("judge"), exit_of(Some(0)));
        harness.commands =
            ScriptedCommandRunner::new().with_run([CommandCompletion::Exited(ExitCode::new(code))]);

        let summary = harness.completed();

        let saved = harness.saved(TASK);
        assert_eq!(saved.execution_kind(), expected, "exit {code}");
        assert_eq!(saved.task_status().as_str(), "queued", "exit {code}");
        if code == 20 {
            assert_eq!(summary.skipped_back, vec![task_id(TASK)]);
            assert_eq!(saved.counters(), RetryCounters::initial());
            assert!(summary.notified.is_empty(), "周回では通知しない");
        }
    }
}

#[test]
fn 判定自体が壊れていれば起動確認済みのまま判定のカウンタだけが進む() {
    for (label, completion) in [
        ("プロトコル外", CommandCompletion::Exited(ExitCode::new(1))),
        ("timeout", CommandCompletion::TimedOut),
        (
            "起動不能",
            CommandCompletion::FailedToStart {
                message: "実体が見つかりません".to_owned(),
            },
        ),
    ] {
        let mut harness = running_status(judged_status("judge"), exit_of(Some(0)));
        harness.commands = ScriptedCommandRunner::new().with_run([completion]);

        let summary = harness.completed();

        let saved = harness.saved(TASK);
        assert_eq!(
            saved.execution(),
            &ExecutionState::Running,
            "{label}: エージェントは再実行せず次の tick が再判定する"
        );
        assert_eq!(
            saved.counters(),
            RetryCounters::rehydrate(0, 1, 0),
            "{label}: 実行のカウンタは動かない"
        );
        assert!(
            matches!(summary.errors.as_slice(), [TickIssue::JudgeFailed { .. }]),
            "{label}: 判定失敗として報告される({:?})",
            summary.errors
        );
    }
}

#[test]
fn 判定コマンドを持つステータスでworkspaceが未確定なら判定を起動せず報告する() {
    let harness = Harness {
        tasks: repository(vec![running_without_workspace(judged_status("judge"))]),
        runs: exit_of(Some(0)),
        ..Harness::new()
    };

    let summary = harness.completed();

    assert_eq!(
        summary.errors,
        vec![TickIssue::MissingWorkspace {
            task_id: task_id(TASK),
        }]
    );
    assert!(
        harness.commands.calls().is_empty(),
        "判定コマンドへ渡す文脈が揃わないので起動しない"
    );
    assert!(harness.tasks.saved().is_empty(), "書き込まない");
}

#[test]
fn 実行の失敗の根拠は判断した主体ごとに区別される() {
    let default_judgement = running_with(exit_of(Some(1))).completed();
    assert!(
        default_judgement.errors.contains(&TickIssue::RunFailed {
            task_id: task_id(TASK),
            cause: RunFailureCause::DefaultJudgement {
                exit: ExitCode::new(1),
            },
        }),
        "{:?}",
        default_judgement.errors
    );

    let mut judged = running_status(judged_status("judge"), exit_of(Some(0)));
    judged.commands =
        ScriptedCommandRunner::new().with_run([CommandCompletion::Exited(ExitCode::new(10))]);
    let by_command = judged.completed();

    assert!(
        by_command.errors.contains(&TickIssue::RunFailed {
            task_id: task_id(TASK),
            cause: RunFailureCause::JudgeCommand {
                exit: ExitCode::new(0),
            },
        }),
        "判定コマンドが失敗と判定した場合、実行の終了コード 0 は失敗の根拠にならない({:?})",
        by_command.errors
    );
}

#[test]
fn 判定失敗は上限と等しい回数では凍結せず超えると凍結する() {
    for (judge_count, expected, frozen) in [
        (2, ExecutionStateKind::Running, Vec::new()),
        (3, ExecutionStateKind::Stopped, vec![task_id(TASK)]),
    ] {
        let harness = Harness {
            config: config_judging(3),
            tasks: repository(vec![
                task(TASK)
                    .snapshot(snapshot_with(Some("claude"), None, judged_status("judge")))
                    .running(1)
                    .counters(0, judge_count, 0)
                    .entry(),
            ]),
            runs: exit_of(Some(0)),
            commands: ScriptedCommandRunner::new()
                .with_run([CommandCompletion::Exited(ExitCode::new(1))]),
            ..Harness::new()
        };

        let summary = harness.completed();

        let saved = harness.saved(TASK);
        assert_eq!(
            saved.execution_kind(),
            expected,
            "judge_attempt_count={judge_count}"
        );
        assert_eq!(
            summary.frozen, frozen,
            "凍結の計上は保存された実行状態と一致する(judge_attempt_count={judge_count})"
        );
        assert_eq!(
            saved.counters().attempt_count(),
            0,
            "エージェントは再実行されていない"
        );
    }
}

#[test]
fn 判定失敗の後に失敗が確定すると判定のカウンタはリセットされる() {
    let harness = Harness {
        tasks: repository(vec![task(TASK).running(1).counters(0, 2, 0).entry()]),
        runs: exit_of(Some(1)),
        ..Harness::new()
    };

    harness.completed();

    assert_eq!(
        harness.saved(TASK).counters(),
        RetryCounters::rehydrate(1, 0, 0)
    );
}

#[test]
fn 実行の失敗は上限と等しい回数では凍結せず超えると凍結する() {
    for (attempt_count, expected) in [
        (1, ExecutionStateKind::Failed),
        (2, ExecutionStateKind::Stopped),
    ] {
        let mut harness = running_with(exit_of(Some(1)));
        harness.tasks = repository(vec![
            task(TASK).running(1).counters(attempt_count, 0, 0).entry(),
        ]);

        harness.completed();

        assert_eq!(
            harness.saved(TASK).execution_kind(),
            expected,
            "attempt_count={attempt_count}(既定の上限は2)"
        );
    }
}

#[test]
fn リトライ上限が0のステータスは最初の失敗で凍結する() {
    let harness = running_status(
        agent_run(AgentRunSpec {
            retries: Some(0),
            ..AgentRunSpec::default()
        }),
        exit_of(Some(1)),
    );

    let summary = harness.completed();

    assert_eq!(
        harness.saved(TASK).execution(),
        &ExecutionState::Stopped {
            reason: StopReason::RetryLimitExceeded,
            notified_at: None,
        }
    );
    assert_eq!(summary.frozen, vec![task_id(TASK)]);
}

#[test]
fn 判定は同じ終了コードと同じ定義に対して同じ結論を導く() {
    let judged = || {
        let harness = running_with(exit_of(Some(0)));
        harness.completed();
        harness.saved(TASK).execution().clone()
    };

    assert_eq!(judged(), ExecutionState::Completed);
    assert_eq!(judged(), ExecutionState::Completed, "冪等である");
}

#[test]
fn 判定確定の保存に失敗しても次のtickが同じ結論を再導出する() {
    let failing = Harness {
        tasks: ScriptedTaskRepository::new()
            .with_list_active([Ok(vec![task(TASK).running(1).entry()])])
            .with_save([Err(SaveError::Io {
                message: "タスクファイルを書けない".to_owned(),
            })]),
        runs: exit_of(Some(0)),
        ..Harness::new()
    };
    let summary = failing.completed();
    assert!(
        summary.judged.is_empty(),
        "永続化できていないので記録しない"
    );

    let retried = running_with(exit_of(Some(0)));

    assert_eq!(retried.completed().judged, vec![task_id(TASK)]);
}

#[test]
fn exitファイルを読めなければ報告してスキップする() {
    let harness = running_with(ScriptedRunStore::new().with_read_exit([Err(
        RunFileError::Corrupt {
            path: run_dir(TASK, 1).exit_file(),
            message: "JSON として読めない".to_owned(),
        },
    )]));

    let summary = harness.completed();

    assert!(
        matches!(
            summary.errors.as_slice(),
            [TickIssue::RunFileUnreadable { .. }]
        ),
        "{:?}",
        summary.errors
    );
    assert!(harness.tasks.saved().is_empty(), "書き込まない");
}

#[test]
fn 生存観測の機構が失敗すれば状態を変更せず報告する() {
    let mut harness = running_with(exit_of(None));
    harness.processes = ScriptedProcessController::new().with_starttime_of([Err(
        pulsen_domain::execution::Io::Failed {
            message: "取得元を起動できない".to_owned(),
        },
    )]);

    let summary = harness.completed();

    assert!(
        matches!(
            summary.errors.as_slice(),
            [TickIssue::ObservationFailed { .. }]
        ),
        "{:?}",
        summary.errors
    );
    assert!(
        harness.tasks.saved().is_empty(),
        "機構の失敗を生死のどちらにも写像しない"
    );
}

#[test]
fn 生存していてtimeout未超過なら書き込みを1回も起こさない() {
    for elapsed in [0, 1, 60] {
        let mut harness = running_status(
            agent_run(AgentRunSpec {
                timeout: Some(timeout_secs(60)),
                ..AgentRunSpec::default()
            }),
            exit_of(None),
        );
        harness.processes =
            ScriptedProcessController::new().with_starttime_of([Ok(Some(observed_starttime()))]);
        // 起点は記録済み starttime の壁時計成分(NOW の1秒前)。
        harness.clock.set(after(elapsed - 1));

        let summary = harness.completed();

        assert_eq!(summary, TickSummary::default(), "{elapsed}秒");
        assert!(harness.tasks.saved().is_empty(), "{elapsed}秒");
    }
}

#[test]
fn 生存していてtimeoutを超えていればkillしてから失敗にする() {
    let mut harness = running_status(
        agent_run(AgentRunSpec {
            timeout: Some(timeout_secs(60)),
            ..AgentRunSpec::default()
        }),
        exit_of(None),
    );
    harness.processes = ScriptedProcessController::new()
        .with_starttime_of([Ok(Some(observed_starttime()))])
        .with_kill([Ok(())]);
    harness.clock.set(after(60));

    let summary = harness.completed();

    assert_eq!(
        harness.processes.calls().pop(),
        Some(ProcessControllerCall::Kill {
            ident: kill_ident()
        })
    );
    assert_eq!(harness.saved(TASK).execution(), &ExecutionState::Failed);
    assert!(
        summary.errors.contains(&TickIssue::RunFailed {
            task_id: task_id(TASK),
            cause: RunFailureCause::TimedOut {
                timeout: timeout_secs(60),
            },
        }),
        "{:?}",
        summary.errors
    );
}

#[test]
fn timeoutが無制限ならどれだけ経過しても実行を続ける() {
    let mut harness = running_status(
        agent_run(AgentRunSpec {
            timeout: Some(TimeoutSpec::Unlimited),
            ..AgentRunSpec::default()
        }),
        exit_of(None),
    );
    harness.processes =
        ScriptedProcessController::new().with_starttime_of([Ok(Some(observed_starttime()))]);
    harness.clock.set(after(86_400));

    assert!(harness.completed().is_empty());
    assert!(harness.tasks.saved().is_empty());
}

#[test]
fn timeout未指定のステータスは組み込みの1時間を超えると終了させられる() {
    let builtin = match WorkflowDefinition::DEFAULT_TIMEOUT {
        TimeoutSpec::Limited(limit) => limit.seconds(),
        TimeoutSpec::Unlimited => unreachable!("組み込みの既定は無制限ではない"),
    };
    let mut harness = running_with(exit_of(None));
    harness.processes = ScriptedProcessController::new()
        .with_starttime_of([Ok(Some(observed_starttime()))])
        .with_kill([Ok(())]);
    // 起点は NOW の1秒前なので、ここで経過は 1h + 1秒になる。
    harness
        .clock
        .set(after(i64::try_from(builtin).expect("収まる")));

    harness.completed();

    assert_eq!(harness.saved(TASK).execution(), &ExecutionState::Failed);
}

#[test]
fn 時計が巻き戻っていてもtimeoutを超えたとみなさない() {
    let mut harness = running_status(
        agent_run(AgentRunSpec {
            timeout: Some(timeout_secs(60)),
            ..AgentRunSpec::default()
        }),
        exit_of(None),
    );
    harness.processes =
        ScriptedProcessController::new().with_starttime_of([Ok(Some(observed_starttime()))]);
    harness.clock.set(after(-3600));

    assert!(harness.completed().is_empty());
    assert!(harness.tasks.saved().is_empty());
}

#[test]
fn 終了操作に失敗したtimeout超過は状態を変更せず報告する() {
    let mut harness = running_status(
        agent_run(AgentRunSpec {
            timeout: Some(timeout_secs(60)),
            ..AgentRunSpec::default()
        }),
        exit_of(None),
    );
    harness.processes = ScriptedProcessController::new()
        .with_starttime_of([Ok(Some(observed_starttime()))])
        .with_kill([Err(KillError::Failed {
            message: "終了操作を起動できない".to_owned(),
        })]);
    harness.clock.set(after(120));

    let summary = harness.completed();

    assert!(
        matches!(summary.errors.as_slice(), [TickIssue::KillFailed { .. }]),
        "{:?}",
        summary.errors
    );
    assert!(
        harness.tasks.saved().is_empty(),
        "生存したままのタスクを failed にすると同一 worktree で並走する"
    );
}

#[test]
fn 起動時刻が記録と食い違えば別プロセスとみなして失敗にする() {
    let mut harness = running_with(exit_of(None));
    harness.processes = ScriptedProcessController::new()
        .with_starttime_of([Ok(Some(reused_starttime()))])
        .with_try_kill_remnants([RemnantOutcome::Killed]);

    harness.completed();

    assert_eq!(harness.saved(TASK).execution(), &ExecutionState::Failed);
    assert!(
        !harness
            .processes
            .calls()
            .contains(&ProcessControllerCall::Kill {
                ident: kill_ident()
            }),
        "照合が一致しないプロセスは kill しない"
    );
}

#[test]
fn 終了コードを残さず死亡した実行は残存終了を試みてから失敗になる() {
    let mut harness = running_with(exit_of(None));
    harness.processes = ScriptedProcessController::new()
        .with_starttime_of([Ok(None)])
        .with_try_kill_remnants([RemnantOutcome::Killed]);

    let summary = harness.completed();

    assert_eq!(
        harness.processes.calls(),
        vec![
            ProcessControllerCall::StarttimeOf {
                pid: pulsen_domain::task::Pid::new(4242)
            },
            ProcessControllerCall::TryKillRemnants {
                ident: kill_ident()
            },
        ],
        "残存終了は失敗の確定より先に試みる"
    );
    assert_eq!(harness.saved(TASK).execution(), &ExecutionState::Failed);
    assert!(
        summary.errors.contains(&TickIssue::RunFailed {
            task_id: task_id(TASK),
            cause: RunFailureCause::DiedWithoutExit,
        }),
        "{:?}",
        summary.errors
    );
}

#[test]
fn 残存終了の結末は報告されるだけで分類に影響しない() {
    for (outcome, remnants) in [
        (
            RemnantOutcome::NotIdentifiable,
            RemnantsLeft::NotIdentifiable,
        ),
        (
            RemnantOutcome::Failed {
                message: "終了操作が失敗した".to_owned(),
            },
            RemnantsLeft::Failed {
                message: "終了操作が失敗した".to_owned(),
            },
        ),
    ] {
        let mut harness = running_with(exit_of(None));
        harness.processes = ScriptedProcessController::new()
            .with_starttime_of([Ok(None)])
            .with_try_kill_remnants([outcome.clone()]);

        let summary = harness.completed();

        assert_eq!(
            harness.saved(TASK).execution(),
            &ExecutionState::Failed,
            "{outcome:?}"
        );
        assert!(
            summary.errors.contains(&TickIssue::RemnantsUnhandled {
                task_id: task_id(TASK),
                remnants,
            }),
            "{outcome:?}: 後始末の残り方まで報告される({:?})",
            summary.errors
        );
    }
}

#[test]
fn 保存に失敗しても残存の結末は報告される() {
    let mut harness = Harness {
        tasks: repository_failing_save(vec![task(TASK).running(1).entry()]),
        runs: exit_of(None),
        ..Harness::new()
    };
    harness.processes = ScriptedProcessController::new()
        .with_starttime_of([Ok(None)])
        .with_try_kill_remnants([RemnantOutcome::NotIdentifiable]);

    let summary = harness.completed();

    assert!(
        summary.errors.contains(&TickIssue::RemnantsUnhandled {
            task_id: task_id(TASK),
            remnants: RemnantsLeft::NotIdentifiable,
        }),
        "残存プロセスの有無はタスクファイルを書けたかと直交する({:?})",
        summary.errors
    );
    assert!(
        summary
            .errors
            .iter()
            .any(|issue| matches!(issue, TickIssue::SaveFailed { .. })),
        "書けなかったこと自体も報告される({:?})",
        summary.errors
    );
}

#[test]
fn 見送りで起動待ちへ戻ったタスクは次のtickで新しいattemptを起動する() {
    let mut skipped = running_status(judged_status("judge"), exit_of(Some(0)));
    skipped.commands =
        ScriptedCommandRunner::new().with_run([CommandCompletion::Exited(ExitCode::new(20))]);
    assert_eq!(skipped.completed().skipped_back, vec![task_id(TASK)]);
    let pending = skipped.saved(TASK);
    assert_eq!(pending.next_attempt_number().get(), 2);

    let next = Harness {
        tasks: repository(vec![
            task(TASK)
                .snapshot(snapshot_with(Some("claude"), None, judged_status("judge")))
                .workspace()
                .attempt(1)
                .entry(),
        ]),
        runs: ScriptedRunStore::new().with_prepare_attempt([Ok(run_dir(TASK, 2))]),
        processes: ScriptedProcessController::new().with_spawn_wrapper([Ok(())]),
        ..Harness::new()
    };

    let summary = next.completed();

    assert_eq!(summary.launched, vec![task_id(TASK)]);
    assert!(
        next.runs.calls().iter().all(|call| !matches!(
            call,
            pulsen_conformance::doubles::RunStoreCall::ReadExit { .. }
        )),
        "同じ exit ファイルを再判定しない"
    );
    assert_eq!(
        next.saved(TASK).current_attempt().map(|a| a.number().get()),
        Some(2)
    );
}

/// 実行の失敗が報告されたか。
fn reported_run_failure(summary: &TickSummary) -> bool {
    summary
        .errors
        .iter()
        .any(|issue| matches!(issue, TickIssue::RunFailed { .. }))
}
