//! 共通手続き notify の振る舞い(UC-execution-001)と、判定確定からの遷移。
//!
//! ポートはすべてテストダブルに差し替える(ADR-028)。通知コマンドの非0終了・timeout・
//! 起動不能と、`mark_notified` 後の保存失敗は実アダプターでは外から作れない。

mod tick_fixture;

use pulsen::application::tick::TickIssue;
use pulsen_conformance::doubles::{
    ScriptedCommandRunner, ScriptedRunStore, ScriptedTaskRepository,
};
use pulsen_domain::execution::{CommandCompletion, ExitCode};
use pulsen_domain::task::{
    ExecutionState, ExecutionStateKind, SaveError, StopReason, TransitionError,
};

use tick_fixture::{
    AgentRunSpec, Harness, NOW, TASK, after, agent_run, at, command, config_notifying,
    degraded_entry_with, repository, snapshot_with, task, task_id,
};

/// 通知コマンドが成功したことにする台本。
fn succeeding() -> ScriptedCommandRunner {
    ScriptedCommandRunner::new().with_run([CommandCompletion::Exited(ExitCode::new(0))])
}

/// 未通知の凍結を1件だけ持つ走査結果。
fn frozen_entries() -> Vec<pulsen_domain::task::TaskEntry> {
    vec![task(TASK).stopped(StopReason::RetryLimitExceeded).entry()]
}

#[test]
fn 未通知の凍結は環境変数と組み込みtimeoutつきで通知され通知時刻が残る() {
    let harness = Harness {
        config: config_notifying("notify-send 凍結"),
        tasks: repository(frozen_entries()),
        commands: succeeding(),
        ..Harness::new()
    };
    harness.clock.set(after(60));

    let summary = harness.completed();

    assert_eq!(summary.notified, vec![task_id(TASK)]);
    let call = harness.commands.calls().pop().expect("通知が実行される");
    assert_eq!(call.cmd, command("notify-send 凍結"));
    assert_eq!(
        call.env,
        vec![
            ("TASK_ID".to_owned(), TASK.to_owned()),
            ("WORKFLOW".to_owned(), "implement".to_owned()),
            ("TASK_STATUS".to_owned(), "queued".to_owned()),
        ]
    );
    assert_eq!(
        call.timeout.map(|spec| spec.seconds()),
        Some(60),
        "組み込みの NOTIFY_TIMEOUT が必ず適用される"
    );
    assert_eq!(
        harness.saved(TASK).execution(),
        &ExecutionState::Stopped {
            reason: StopReason::RetryLimitExceeded,
            notified_at: Some(after(60)),
        }
    );
}

#[test]
fn 通知は凍結の保存が済んでから実行され成功してはじめて追記される() {
    let harness = Harness {
        config: config_notifying("notify"),
        tasks: repository(vec![
            task(TASK)
                .snapshot(snapshot_with(
                    Some("claude"),
                    None,
                    agent_run(AgentRunSpec {
                        retries: Some(0),
                        ..AgentRunSpec::default()
                    }),
                ))
                .running(1)
                .entry(),
        ]),
        runs: ScriptedRunStore::new().with_read_exit([Ok(Some(ExitCode::new(1)))]),
        commands: succeeding(),
        ..Harness::new()
    };

    let summary = harness.completed();

    assert_eq!(summary.frozen, vec![task_id(TASK)]);
    assert_eq!(
        summary.notified,
        vec![task_id(TASK)],
        "同じ tick で通知する"
    );
    let saved = harness.tasks.saved();
    assert_eq!(saved.len(), 2, "凍結の保存と通知の追記で2回書く");
    assert_eq!(
        saved[0].execution(),
        &ExecutionState::Stopped {
            reason: StopReason::RetryLimitExceeded,
            notified_at: None,
        },
        "先に書く凍結は未通知である"
    );
    assert!(
        matches!(
            saved[1].execution(),
            ExecutionState::Stopped {
                notified_at: Some(_),
                ..
            }
        ),
        "通知が成功した後にだけ通知時刻が追記される"
    );
}

#[test]
fn 通知コマンドが未定義なら通知も通知時刻の記録も行わない() {
    let harness = Harness {
        tasks: repository(frozen_entries()),
        ..Harness::new()
    };

    let summary = harness.completed();

    assert!(summary.is_empty(), "サマリーに現れない");
    assert!(
        harness.commands.calls().is_empty(),
        "通知コマンドを起動しない"
    );
    assert!(
        harness.tasks.saved().is_empty(),
        "「通知した」という虚偽の記録を作らない"
    );
}

#[test]
fn 通知コマンドを後から定義すると次のtickが未通知の凍結を拾って通知する() {
    let unconfigured = Harness {
        tasks: repository(frozen_entries()),
        ..Harness::new()
    };
    assert!(unconfigured.completed().is_empty(), "定義前は通知されない");

    let configured = Harness {
        config: config_notifying("notify"),
        tasks: repository(frozen_entries()),
        commands: succeeding(),
        ..Harness::new()
    };

    assert_eq!(
        configured.completed().notified,
        vec![task_id(TASK)],
        "永続化された未通知の凍結から同じ判断が再導出される"
    );
}

#[test]
fn 通知が失敗した凍結には通知時刻を残さず次のtickへ委ねる() {
    for (label, completion) in [
        ("非0", CommandCompletion::Exited(ExitCode::new(1))),
        ("timeout", CommandCompletion::TimedOut),
        (
            "起動不能",
            CommandCompletion::FailedToStart {
                message: "実体が見つかりません".to_owned(),
            },
        ),
    ] {
        let harness = Harness {
            config: config_notifying("notify"),
            tasks: repository(frozen_entries()),
            commands: ScriptedCommandRunner::new().with_run([completion]),
            ..Harness::new()
        };

        let summary = harness.completed();

        assert!(summary.notified.is_empty(), "{label}: 通知として記録しない");
        assert!(
            harness.tasks.saved().is_empty(),
            "{label}: notified_at を書かない"
        );
        assert!(
            matches!(
                summary.errors.as_slice(),
                [TickIssue::NotifyFailed { task_id: reported, .. }] if reported == &task_id(TASK)
            ),
            "{label}: 通知の失敗として報告される({:?})",
            summary.errors
        );
    }
}

#[test]
fn 通知の後の保存に失敗した凍結は次のtickが再通知する() {
    let harness = Harness {
        config: config_notifying("notify"),
        tasks: ScriptedTaskRepository::new()
            .with_list_active([Ok(frozen_entries())])
            .with_save([Err(SaveError::Io {
                message: "タスクファイルを書けない".to_owned(),
            })]),
        commands: succeeding(),
        ..Harness::new()
    };

    let summary = harness.completed();

    assert!(summary.notified.is_empty(), "通知として記録しない");
    assert!(
        matches!(
            summary.errors.as_slice(),
            [TickIssue::SaveFailed { task_id: reported, .. }] if reported == &task_id(TASK)
        ),
        "保存の失敗として報告される({:?})",
        summary.errors
    );
}

#[test]
fn スナップショットが読めない未通知の凍結にも再通知が行われる() {
    let harness = Harness {
        config: config_notifying("notify"),
        tasks: repository(vec![degraded_entry_with(
            TASK,
            "statuses が空",
            ExecutionState::Stopped {
                reason: StopReason::JudgeLimitExceeded,
                notified_at: None,
            },
        )]),
        commands: succeeding(),
        ..Harness::new()
    };
    harness.clock.set(after(30));

    let summary = harness.completed();

    assert_eq!(summary.notified, vec![task_id(TASK)]);
    assert!(
        harness.tasks.saved().is_empty(),
        "縮退したタスクは通常の保存を通らない"
    );
    let saved = harness.tasks.saved_degraded();
    assert_eq!(saved.len(), 1, "save_degraded で書き戻される");
    assert_eq!(
        saved[0].execution(),
        &ExecutionState::Stopped {
            reason: StopReason::JudgeLimitExceeded,
            notified_at: Some(after(30)),
        }
    );
    assert_eq!(
        saved[0].snapshot_error(),
        "statuses が空",
        "読めないスナップショットは修復の材料として温存される"
    );
}

#[test]
fn 凍結を離脱したタスクは通知の対象にならない() {
    for (label, execution) in [
        ("起動待ち", ExecutionState::Pending),
        ("失敗確定", ExecutionState::Failed),
    ] {
        let harness = Harness {
            config: config_notifying("notify"),
            tasks: repository(vec![task(TASK).workspace().execution(execution).entry()]),
            ..Harness::new()
        };
        // 起動へ進まないよう、エージェント定義を持たない設定にしていないため、
        // 走査の結果として通知が起きないことだけを見る。
        let _ = harness.run();

        assert!(
            harness.commands.calls().is_empty(),
            "{label}: 通知コマンドを起動しない"
        );
    }
}

#[test]
fn 判定確定の遷移で前提が破れていれば報告してスキップする() {
    let harness = Harness {
        tasks: repository(vec![
            task(TASK)
                .status("waiting")
                .execution(ExecutionState::Completed)
                .attempt(1)
                .entry(),
        ]),
        ..Harness::new()
    };

    let summary = harness.completed();

    assert_eq!(
        summary.errors,
        vec![TickIssue::Transition {
            task_id: task_id(TASK),
            error: TransitionError::NotAgentRunStatus {
                status: pulsen_domain::definition::StatusName::parse("waiting".to_owned())
                    .expect("受理される"),
            },
        }]
    );
    assert!(harness.tasks.saved().is_empty(), "書き込まない");
}

#[test]
fn 遷移先が同じステータスなら周回して起動待ちに戻る() {
    let harness = Harness {
        tasks: repository(vec![
            task(TASK)
                .snapshot(snapshot_with(
                    Some("claude"),
                    None,
                    agent_run(AgentRunSpec {
                        next: "queued",
                        ..AgentRunSpec::default()
                    }),
                ))
                .completed(1)
                .entry(),
        ]),
        ..Harness::new()
    };

    let summary = harness.completed();

    assert_eq!(summary.transitioned, vec![task_id(TASK)]);
    let saved = harness.saved(TASK);
    assert_eq!(saved.task_status().as_str(), "queued");
    assert_eq!(saved.execution_kind(), ExecutionStateKind::Pending);
    assert_eq!(saved.updated_at(), at(NOW));
}
