//! 共通手続き notify の振る舞い(UC-execution-001)と、判定確定からの遷移。
//!
//! ポートはすべてテストダブルに差し替える(ADR-028)。通知コマンドの非0終了・timeout・
//! 起動不能と、`mark_notified` 後の保存失敗は実アダプターでは外から作れない。

mod tick_fixture;

use pulsen::application::tick::TickIssue;
use pulsen_conformance::doubles::{
    RecordSeq, ScriptedCommandRunner, ScriptedRunStore, ScriptedTaskRepository,
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

/// 通知の共通手続きで観測できる出来事。
#[derive(Debug, PartialEq, Eq)]
enum NotifyStep {
    /// 未通知の凍結を保存した。
    SavedFrozen,
    /// 通知コマンドを起動した。
    RanNotifyCmd,
    /// 通知時刻を追記して保存した。
    SavedNotifiedAt,
    /// 凍結以外の内容を保存した。
    SavedOther,
}

/// 保存と通知コマンドの起動を、起きた順に1本の列へ並べる。
///
/// 順序の契約(凍結を書く → 通知を実行する → 通知時刻を追記する)はポートをまたぐため、
/// ダブルごとの列を別々に見ても「どちらが先か」は主張できない。共有の採番で並べ直して
/// はじめて、通知を先に起動する実装を落とせる。
fn notify_steps(harness: &Harness) -> Vec<NotifyStep> {
    let saves = harness
        .tasks
        .saved_in_order()
        .into_iter()
        .map(|(seq, task)| {
            let step = match task.execution() {
                ExecutionState::Stopped {
                    notified_at: None, ..
                } => NotifyStep::SavedFrozen,
                ExecutionState::Stopped {
                    notified_at: Some(_),
                    ..
                } => NotifyStep::SavedNotifiedAt,
                ExecutionState::Pending
                | ExecutionState::Launching { .. }
                | ExecutionState::Running
                | ExecutionState::Completed
                | ExecutionState::Failed => NotifyStep::SavedOther,
            };
            (seq, step)
        });
    let notifications = harness
        .commands
        .calls_in_order()
        .into_iter()
        .map(|(seq, _)| (seq, NotifyStep::RanNotifyCmd));

    let mut steps = saves.chain(notifications).collect::<Vec<(RecordSeq, _)>>();
    steps.sort_by_key(|(seq, _)| *seq);
    steps.into_iter().map(|(_, step)| step).collect()
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
    assert_eq!(
        notify_steps(&harness),
        vec![
            NotifyStep::SavedFrozen,
            NotifyStep::RanNotifyCmd,
            NotifyStep::SavedNotifiedAt,
        ],
        "凍結を書いてから通知し、成功してはじめて通知時刻を追記する"
    );
    assert_eq!(
        harness.saved(TASK).execution(),
        &ExecutionState::Stopped {
            reason: StopReason::RetryLimitExceeded,
            notified_at: Some(at(NOW)),
        }
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
fn 通知の対象は未通知の凍結だけである() {
    // 通知アームに入るのは凍結だけで、そのうち通知済みのものは何もしない。ほかの実行状態は
    // 別の手続きへ分岐するため、通知コマンドの起動にも凍結の書き戻しにも至らない。
    for (label, execution) in [
        ("起動待ち", ExecutionState::Pending),
        ("失敗確定", ExecutionState::Failed),
        (
            "通知済みの凍結",
            ExecutionState::Stopped {
                reason: StopReason::RetryLimitExceeded,
                notified_at: Some(at(NOW)),
            },
        ),
    ] {
        let harness = Harness {
            config: config_notifying("notify"),
            tasks: repository(vec![task(TASK).workspace().execution(execution).entry()]),
            ..Harness::new()
        };

        let summary = harness.completed();

        assert!(summary.notified.is_empty(), "{label}: 通知として記録しない");
        assert!(
            harness.commands.calls().is_empty(),
            "{label}: 通知コマンドを起動しない"
        );
        assert!(
            harness
                .tasks
                .saved()
                .iter()
                .all(|task| task.execution_kind() != ExecutionStateKind::Stopped),
            "{label}: 凍結として書き戻さない"
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
