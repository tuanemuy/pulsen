//! 手続きA(起動)の振る舞い。
//!
//! ポートはすべてテストダブルに差し替える(ADR-028)。worktree 作成の失敗・
//! `prepare_attempt` の失敗・spawn の同期エラーは実アダプターでは外から作れない。
//!
//! 「launching を記録した後の失敗では状態を変更しない」は、状態を戻した実装でも
//! 単体では緑になりうる。保存されたタスクの実行状態と attempt 番号で主張する。

mod tick_fixture;

use pulsen::application::tick::TickIssue;
use pulsen_conformance::doubles::{
    ProcessControllerCall, RunStoreCall, ScriptedProcessController, ScriptedRunStore,
    ScriptedWorktreeManager, WorktreeManagerCall,
};
use pulsen_domain::definition::{CommandLine, StatusDefinition};
use pulsen_domain::execution::{Io, SpawnError, WorktreeError, WrapperLaunchSpec};
use pulsen_domain::task::{
    AttemptRef, ExecutionState, FailureKind, FailureNote, StopReason, WorkspacePlanner,
};

use tick_fixture::{
    AgentRunSpec, Harness, NOW, TASK, agent_run, at, attempt_number, branch, config_with, prompt,
    repo, repository, repository_failing_save, run_dir, skill, snapshot_with, task, task_id,
    worktree_root,
};

/// worktree 作成と spawn が成功する既定のポート。
fn ready() -> (
    ScriptedWorktreeManager,
    ScriptedRunStore,
    ScriptedProcessController,
) {
    (
        ScriptedWorktreeManager::new().with_create([Ok(())]),
        ScriptedRunStore::new().with_prepare_attempt([Ok(run_dir(TASK, 1))]),
        ScriptedProcessController::new().with_spawn_wrapper([Ok(())]),
    )
}

#[test]
fn ワークスペース未確定のタスクは導出したworktreeを確保してから起動する() {
    let (worktrees, runs, processes) = ready();
    let harness = Harness {
        tasks: repository(vec![task(TASK).entry()]),
        worktrees,
        runs,
        processes,
        ..Harness::new()
    };

    let summary = harness.completed();

    let workspace = WorkspacePlanner::derive(&worktree_root(), &task_id(TASK));
    assert_eq!(
        harness.worktrees.calls(),
        vec![WorktreeManagerCall::Create {
            repo: repo(),
            base: branch("main"),
            ws: workspace.clone(),
        }]
    );
    let saved = harness.tasks.saved();
    assert_eq!(saved.len(), 2, "ワークスペース確定と起動記録が保存される");
    assert_eq!(saved[0].workspace(), Some(&workspace));
    assert_eq!(saved[0].execution(), &ExecutionState::Pending);
    assert_eq!(
        saved[1].execution(),
        &ExecutionState::Launching {
            recorded_at: at(NOW)
        }
    );
    assert_eq!(summary.launched, vec![task_id(TASK)]);
}

#[test]
fn ワークスペース確定済みのタスクはworktree操作を行わない() {
    let (_, runs, processes) = ready();
    let harness = Harness {
        tasks: repository(vec![task(TASK).workspace().entry()]),
        runs,
        processes,
        ..Harness::new()
    };

    harness.completed();

    assert!(harness.worktrees.calls().is_empty());
    assert_eq!(harness.tasks.saved().len(), 1, "起動記録だけが保存される");
}

#[test]
fn worktree作成の失敗はツール操作の失敗として記録され次tickで再試行される() {
    let harness = Harness {
        tasks: repository(vec![task(TASK).entry()]),
        worktrees: ScriptedWorktreeManager::new().with_create([Err(WorktreeError::Failed {
            message: "git worktree add に失敗".to_owned(),
        })]),
        ..Harness::new()
    };

    let summary = harness.completed();

    let saved = harness.saved(TASK);
    assert_eq!(saved.execution(), &ExecutionState::Failed);
    assert_eq!(saved.counters().attempt_count(), 1);
    assert_eq!(saved.workspace(), None, "確定しない");
    assert_eq!(
        saved.last_failure().map(FailureNote::kind),
        Some(FailureKind::WorktreeCreate)
    );
    assert_eq!(
        saved.last_failure().map(FailureNote::message),
        Some("git worktree add に失敗")
    );
    assert!(summary.launched.is_empty());
    assert!(summary.frozen.is_empty());
}

#[test]
fn worktree作成の失敗がリトライ上限を超えるとタスクは凍結される() {
    let snapshot = snapshot_with(
        Some("claude"),
        None,
        agent_run(AgentRunSpec {
            retries: Some(0),
            ..AgentRunSpec::default()
        }),
    );
    let harness = Harness {
        tasks: repository(vec![task(TASK).snapshot(snapshot).entry()]),
        worktrees: ScriptedWorktreeManager::new().with_create([Err(WorktreeError::Failed {
            message: "対象リポジトリが無い".to_owned(),
        })]),
        ..Harness::new()
    };

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
fn テンプレート展開の失敗はどの経路でも同期的なspawn失敗として記録される() {
    for (label, harness) in expansion_failures() {
        let summary = harness.completed();

        let saved = harness.saved(TASK);
        assert_eq!(saved.execution(), &ExecutionState::Pending, "{label}");
        assert_eq!(saved.counters().spawn_fail_count(), 1, "{label}");
        assert_eq!(saved.counters().attempt_count(), 0, "{label}");
        assert_eq!(
            saved.current_attempt(),
            None,
            "{label}: attempt は採番されない"
        );
        assert_eq!(
            saved.last_failure().map(FailureNote::kind),
            Some(FailureKind::SpawnFail),
            "{label}"
        );
        assert!(
            saved
                .last_failure()
                .is_some_and(|note| !note.message().is_empty()),
            "{label}: 原因が残る"
        );
        assert!(
            harness.runs.calls().is_empty(),
            "{label}: runディレクトリを作らない"
        );
        assert!(
            harness.processes.calls().is_empty(),
            "{label}: 起動もしない"
        );
        assert!(summary.launched.is_empty(), "{label}");
    }
}

#[test]
fn 失敗確定のタスクの展開失敗は実行状態を変えない() {
    let harness = Harness {
        tasks: repository(vec![
            task(TASK)
                .workspace()
                .execution(ExecutionState::Failed)
                .entry(),
        ]),
        config: config_with(vec![], None),
        ..Harness::new()
    };

    harness.completed();

    let saved = harness.saved(TASK);
    assert_eq!(saved.execution(), &ExecutionState::Failed);
    assert_eq!(saved.counters().spawn_fail_count(), 1);
}

#[test]
fn 展開失敗の加算が上限と等しければ凍結せず上限を超えれば凍結する() {
    let cases = [(2, ExecutionState::Pending, false), (3, stopped(), true)];

    for (spawn_fail, expected, frozen) in cases {
        let harness = Harness {
            tasks: repository(vec![
                task(TASK).workspace().counters(0, 0, spawn_fail).entry(),
            ]),
            config: config_with(vec![], Some(3)),
            ..Harness::new()
        };

        let summary = harness.completed();

        assert_eq!(
            harness.saved(TASK).execution(),
            &expected,
            "{spawn_fail}回目"
        );
        assert_eq!(summary.frozen.is_empty(), !frozen, "{spawn_fail}回目");
    }
}

#[test]
fn 起動記録は次の番号を採番し導出したrunディレクトリを渡す() {
    let harness = Harness {
        tasks: repository(vec![
            task(TASK)
                .workspace()
                .execution(ExecutionState::Failed)
                .attempt(1)
                .entry(),
        ]),
        runs: ScriptedRunStore::new().with_prepare_attempt([Ok(run_dir(TASK, 2))]),
        processes: ScriptedProcessController::new().with_spawn_wrapper([Ok(())]),
        ..Harness::new()
    };

    harness.completed();

    let saved = harness.saved(TASK);
    assert_eq!(
        saved.current_attempt().map(AttemptRef::number),
        Some(attempt_number(2)),
        "過去の attempt 番号は再利用されない"
    );
    assert_eq!(
        saved.current_attempt().map(AttemptRef::run_dir),
        Some(&run_dir(TASK, 2))
    );
    assert_eq!(
        harness.runs.calls(),
        vec![RunStoreCall::PrepareAttempt {
            id: task_id(TASK),
            number: attempt_number(2),
        }]
    );
    assert_eq!(spawned(&harness).run_dir(), &run_dir(TASK, 2));
}

#[test]
fn 起動記録の後にrunディレクトリを用意できなくても状態は変わらずspawnは行われる() {
    let harness = Harness {
        tasks: repository(vec![task(TASK).workspace().entry()]),
        runs: ScriptedRunStore::new().with_prepare_attempt([Err(Io::Failed {
            message: "attempt ディレクトリを作れない".to_owned(),
        })]),
        processes: ScriptedProcessController::new().with_spawn_wrapper([Ok(())]),
        ..Harness::new()
    };

    let summary = harness.completed();

    assert_eq!(
        summary.errors,
        vec![TickIssue::PrepareAttemptFailed {
            task_id: task_id(TASK),
            message: "attempt ディレクトリを作れない".to_owned(),
        }]
    );
    assert_eq!(
        harness.saved(TASK).execution(),
        &ExecutionState::Launching {
            recorded_at: at(NOW)
        },
        "launching のまま猶予時間経路に委ねる"
    );
    assert_eq!(summary.launched, vec![task_id(TASK)]);
}

#[test]
fn spawnの同期エラーは状態を変更せず報告のみになる() {
    let harness = Harness {
        tasks: repository(vec![task(TASK).workspace().entry()]),
        runs: ScriptedRunStore::new().with_prepare_attempt([Ok(run_dir(TASK, 1))]),
        processes: ScriptedProcessController::new().with_spawn_wrapper([Err(SpawnError::Failed {
            message: "自身のバイナリを起動できない".to_owned(),
        })]),
        ..Harness::new()
    };

    let summary = harness.completed();

    assert_eq!(
        summary.errors,
        vec![TickIssue::SpawnFailed {
            task_id: task_id(TASK),
            message: "自身のバイナリを起動できない".to_owned(),
        }]
    );
    assert!(summary.launched.is_empty());
    let saved = harness.saved(TASK);
    assert_eq!(
        saved.execution(),
        &ExecutionState::Launching {
            recorded_at: at(NOW)
        },
        "起動待ちへ戻すと遅延起動したラッパーと次の attempt が並走しうる"
    );
    assert_eq!(saved.counters().spawn_fail_count(), 0);
}

#[test]
fn 起動するコマンドにはステータス上書きの定義とモデルと当該worktreeが展開される() {
    let snapshot = snapshot_with(
        Some("claude"),
        Some("sonnet"),
        agent_run(AgentRunSpec {
            agent: Some("reviewer"),
            model: Some("opus"),
            ..AgentRunSpec::default()
        }),
    );
    let harness = Harness {
        tasks: repository(vec![task(TASK).snapshot(snapshot).workspace().entry()]),
        config: config_with(
            vec![
                ("claude", "claude {input}", None),
                (
                    "reviewer",
                    "review -m {model} --dir {workspace} {input}",
                    None,
                ),
            ],
            None,
        ),
        runs: ScriptedRunStore::new().with_prepare_attempt([Ok(run_dir(TASK, 1))]),
        processes: ScriptedProcessController::new().with_spawn_wrapper([Ok(())]),
        ..Harness::new()
    };

    harness.completed();

    let workspace = WorkspacePlanner::derive(&worktree_root(), &task_id(TASK));
    let spec = spawned(&harness);
    assert_eq!(
        spec.agent_cmd().tokens(),
        [
            "review",
            "-m",
            "opus",
            "--dir",
            &workspace.path().as_path().display().to_string(),
            "実装して",
        ]
    );
    assert_eq!(spec.workspace(), workspace.path());
}

#[test]
fn スキル入力はエージェント定義のskill_inputで変換されて渡る() {
    let snapshot = snapshot_with(
        Some("claude"),
        None,
        agent_run(AgentRunSpec {
            input: skill("review"),
            ..AgentRunSpec::default()
        }),
    );
    let harness = Harness {
        tasks: repository(vec![task(TASK).snapshot(snapshot).workspace().entry()]),
        config: config_with(
            vec![("claude", "claude {input}", Some("/skill {skill}"))],
            None,
        ),
        runs: ScriptedRunStore::new().with_prepare_attempt([Ok(run_dir(TASK, 1))]),
        processes: ScriptedProcessController::new().with_spawn_wrapper([Ok(())]),
        ..Harness::new()
    };

    harness.completed();

    assert_eq!(
        spawned(&harness).agent_cmd(),
        &CommandLine::rehydrate(vec!["claude".to_owned(), "/skill review".to_owned()])
            .expect("受理される")
    );
}

#[test]
fn ワークスペース確定の保存に失敗すればそのタスクの処理は打ち切られる() {
    let harness = Harness {
        tasks: repository_failing_save(vec![task(TASK).entry()]),
        worktrees: ScriptedWorktreeManager::new().with_create([Ok(())]),
        ..Harness::new()
    };

    let summary = harness.completed();

    assert!(matches!(
        summary.errors.as_slice(),
        [TickIssue::SaveFailed { .. }]
    ));
    assert!(summary.launched.is_empty());
    assert!(
        harness.runs.calls().is_empty(),
        "run ディレクトリに触れない"
    );
    assert!(harness.processes.calls().is_empty(), "起動もしない");
}

#[test]
fn 起動記録の保存に失敗すればrunディレクトリを用意せず起動もしない() {
    let harness = Harness {
        tasks: repository_failing_save(vec![task(TASK).workspace().entry()]),
        ..Harness::new()
    };

    let summary = harness.completed();

    assert!(matches!(
        summary.errors.as_slice(),
        [TickIssue::SaveFailed { .. }]
    ));
    assert!(harness.runs.calls().is_empty());
    assert!(harness.processes.calls().is_empty());
    assert!(summary.launched.is_empty());
}

#[test]
fn worktree作成の直後にクラッシュしても同じワークスペースが再導出される() {
    let harness = Harness {
        tasks: repository(vec![task(TASK).entry()]),
        worktrees: ScriptedWorktreeManager::new().with_create([Ok(()), Ok(())]),
        runs: ScriptedRunStore::new()
            .with_prepare_attempt([Ok(run_dir(TASK, 1)), Ok(run_dir(TASK, 1))]),
        processes: ScriptedProcessController::new().with_spawn_wrapper([Ok(()), Ok(())]),
        ..Harness::new()
    };

    harness.completed();
    harness.completed();

    let calls = harness.worktrees.calls();
    assert_eq!(calls.len(), 2, "確定を保存できていないので毎回確保を試みる");
    assert_eq!(calls[0], calls[1], "同じワークスペースが再導出される");
}

#[test]
fn 起動待ちへ戻ったタスクは次のtickで新しいattempt番号で再起動される() {
    let harness = Harness {
        tasks: repository(vec![
            task(TASK).workspace().attempt(1).counters(0, 0, 1).entry(),
        ]),
        runs: ScriptedRunStore::new().with_prepare_attempt([Ok(run_dir(TASK, 2))]),
        processes: ScriptedProcessController::new().with_spawn_wrapper([Ok(())]),
        ..Harness::new()
    };

    harness.completed();

    let spec = spawned(&harness);
    assert_eq!(spec.run_dir(), &run_dir(TASK, 2));
    assert_ne!(
        spec.run_dir(),
        &run_dir(TASK, 1),
        "過去の試行の残骸と混同しない"
    );
}

/// spawn に渡された起動仕様。
fn spawned(harness: &Harness) -> WrapperLaunchSpec {
    match harness.processes.calls().as_slice() {
        [ProcessControllerCall::SpawnWrapper { spec }] => spec.clone(),
        calls => panic!("ラッパーが1回だけ起動される: {calls:?}"),
    }
}

fn stopped() -> ExecutionState {
    ExecutionState::Stopped {
        reason: StopReason::SpawnFailLimitExceeded,
        notified_at: None,
    }
}

/// 展開の5経路それぞれを失敗させるハーネス。
fn expansion_failures() -> Vec<(&'static str, Harness)> {
    let base = || task(TASK).workspace();
    let single = |definition: StatusDefinition| snapshot_with(Some("claude"), None, definition);

    vec![
        (
            "実効エージェント名が解決できない",
            Harness {
                tasks: repository(vec![
                    base()
                        .snapshot(snapshot_with(
                            None,
                            None,
                            agent_run(AgentRunSpec::default()),
                        ))
                        .entry(),
                ]),
                ..Harness::new()
            },
        ),
        (
            "エージェントが config.agents に無い",
            Harness {
                tasks: repository(vec![base().entry()]),
                config: config_with(vec![("other", "other {input}", None)], None),
                ..Harness::new()
            },
        ),
        (
            "エージェント定義のテンプレートが不正",
            Harness {
                tasks: repository(vec![base().entry()]),
                config: config_with(vec![("claude", "claude {unknown}", None)], None),
                ..Harness::new()
            },
        ),
        (
            "skill 指定だが skill_input が無い",
            Harness {
                tasks: repository(vec![
                    base()
                        .snapshot(single(agent_run(AgentRunSpec {
                            input: skill("review"),
                            ..AgentRunSpec::default()
                        })))
                        .entry(),
                ]),
                ..Harness::new()
            },
        ),
        (
            "テンプレートが参照する値がない",
            Harness {
                tasks: repository(vec![
                    base()
                        .snapshot(single(agent_run(AgentRunSpec {
                            input: prompt("実装して"),
                            ..AgentRunSpec::default()
                        })))
                        .entry(),
                ]),
                config: config_with(vec![("claude", "claude -m {model} {input}", None)], None),
                ..Harness::new()
            },
        ),
    ]
}
