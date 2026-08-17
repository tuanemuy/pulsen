//! タスク詳細ユースケースの振る舞い。
//!
//! ポートはテストダブルに差し替える。実アダプターでは外から作れない状況(存在確認の
//! 機構失敗・exit の破損・スナップショットだけが読めないタスク・アーカイブ側での解決)を
//! ここで消化する。
//!
//! **`ExclusiveLock` を渡す口が無いこと**は型が示すため、テストでは主張しない。

mod usecase_fixture;

use std::collections::BTreeMap;

use pulsen::application::show_task::{
    ExitOutcome, RetryLimitInfo, RunDirPresence, ShowTask, ShowTaskError, ShowTaskInput,
    SnapshotInfo, TaskDetail,
};
use pulsen_conformance::doubles::{ScriptedRunStore, ScriptedTaskRepository};
use pulsen_domain::definition::{GlobalConfig, GlobalConfigInput, StatusDefinition};
use pulsen_domain::execution::{ExitCode, Io, RunFileError};
use pulsen_domain::task::{
    ExecutionState, ExecutionStateKind, FailureKind, ReadError, StopReason, TaskFilePath, TaskId,
    TaskIdError, TaskLookup, Timestamp,
};

use usecase_fixture::{
    AgentRunSpec, agent_run, at, degraded, failure, run_dir, snapshot_of, snapshot_with,
    state_root, status, task, task_id,
};

/// 既定のタスクID。
const TASK: &str = "20260812t090000-k3f9qa1b";

/// 上限を明示したグローバル設定。
fn config_limits(judge: u32, spawn: u32) -> GlobalConfig {
    GlobalConfig::parse(GlobalConfigInput {
        judge_attempt_limit: Some(judge),
        spawn_fail_limit: Some(spawn),
        ..GlobalConfigInput::default()
    })
    .expect("受理される")
}

/// 既定のグローバル設定(上限は組み込みの既定値)。
fn config() -> GlobalConfig {
    GlobalConfig::parse(GlobalConfigInput {
        agents: Some(BTreeMap::new()),
        ..GlobalConfigInput::default()
    })
    .expect("受理される")
}

/// 解決結果を1件だけ返すリポジトリ。
fn repository(lookup: TaskLookup) -> ScriptedTaskRepository {
    ScriptedTaskRepository::new().with_find([Ok(lookup)])
}

/// attempt ディレクトリが在り、exit が未記録の run ストア。
fn present_without_exit() -> ScriptedRunStore {
    ScriptedRunStore::new()
        .with_attempt_exists([Ok(true)])
        .with_read_exit([Ok(None)])
}

/// 現在 attempt を持たないタスク向けの run ストア(1度も呼ばれない)。
fn untouched() -> ScriptedRunStore {
    ScriptedRunStore::new()
}

fn show(tasks: &ScriptedTaskRepository, runs: &ScriptedRunStore, id: &str) -> TaskDetail {
    ShowTask::new(&config(), &state_root(), tasks, runs)
        .execute(ShowTaskInput {
            task_id: id.to_owned(),
        })
        .expect("表示できる")
}

fn show_error(tasks: &ScriptedTaskRepository, runs: &ScriptedRunStore, id: &str) -> ShowTaskError {
    ShowTask::new(&config(), &state_root(), tasks, runs)
        .execute(ShowTaskInput {
            task_id: id.to_owned(),
        })
        .expect_err("拒否される")
}

/// 現在 attempt の要約(あることを前提とする)。
fn attempt_of(detail: &TaskDetail) -> &pulsen::application::show_task::AttemptSummary {
    detail.attempt.as_ref().expect("現在 attempt がある")
}

#[test]
fn 一度も実行されていないタスクはワークスペースもattemptも縮退で示される() {
    let tasks = repository(task(TASK).found());

    let detail = show(&tasks, &untouched(), TASK);

    assert_eq!(detail.workflow_name.as_str(), "implement");
    assert_eq!(detail.target.repo(), &usecase_fixture::repo());
    assert_eq!(detail.task_status, status("queued"));
    assert_eq!(detail.execution, ExecutionState::Pending);
    assert!(detail.workspace.is_none(), "ワークスペースは未作成");
    assert!(detail.attempt.is_none(), "attempt は無い");
    assert_eq!(detail.counters.attempt_count(), 0);
    assert_eq!(detail.updated_at, at(usecase_fixture::NOW));
    assert!(!detail.archived);
}

#[test]
fn 起動記録済みで同定情報が未取り込みのattemptは未取得として現れる() {
    let tasks = repository(
        task(TASK)
            .launching(at(usecase_fixture::NOW))
            .attempt(1)
            .found(),
    );

    let detail = show(&tasks, &present_without_exit(), TASK);
    let attempt = attempt_of(&detail);

    assert_eq!(attempt.number.get(), 1);
    assert!(
        attempt.process.is_none(),
        "PID・kill同定子・starttime は一組で未取得になる"
    );
    assert_eq!(
        attempt.run_dir_presence,
        RunDirPresence::Present {
            exit: ExitOutcome::Absent
        }
    );
}

#[test]
fn 実行履歴のあるタスクはrunディレクトリを示す() {
    let tasks = repository(task(TASK).running(1).found());

    let detail = show(&tasks, &present_without_exit(), TASK);
    let attempt = attempt_of(&detail);

    assert_eq!(attempt.run_dir, run_dir(TASK, 1));
    let process = attempt.process.as_ref().expect("同定情報がある");
    assert_eq!(process.pid().get(), 4242);
    assert_eq!(process.kill_ident().as_str(), "-4242");
}

#[test]
fn 記録されたexitは値として読み取られる() {
    let tasks = repository(task(TASK).completed(1).found());
    let runs = ScriptedRunStore::new()
        .with_attempt_exists([Ok(true)])
        .with_read_exit([Ok(Some(ExitCode::new(0)))]);

    let detail = show(&tasks, &runs, TASK);

    assert_eq!(
        attempt_of(&detail).run_dir_presence,
        RunDirPresence::Present {
            exit: ExitOutcome::Recorded(ExitCode::new(0))
        }
    );
}

#[test]
fn ツール操作の失敗で凍結したタスクは要因と直近の失敗を示す() {
    let tasks = repository(
        task(TASK)
            .stopped_notified(StopReason::RetryLimitExceeded, at("2026-08-12T10:00:00Z"))
            .last_failure(failure(
                FailureKind::WorktreeCreate,
                "git worktree add に失敗",
                at("2026-08-12T09:59:00Z"),
            ))
            .found(),
    );

    let detail = show(&tasks, &untouched(), TASK);

    assert_eq!(
        detail.execution,
        ExecutionState::Stopped {
            reason: StopReason::RetryLimitExceeded,
            notified_at: Some(at("2026-08-12T10:00:00Z")),
        }
    );
    let note = detail.last_failure.expect("失敗要因がある");
    assert_eq!(note.kind(), FailureKind::WorktreeCreate);
    assert_eq!(note.message(), "git worktree add に失敗");
}

#[test]
fn エージェント実行の失敗で凍結したタスクは直前実行への参照を示す() {
    let tasks = repository(
        task(TASK)
            .running(1)
            .stopped(StopReason::RetryLimitExceeded)
            .found(),
    );
    let runs = ScriptedRunStore::new()
        .with_attempt_exists([Ok(true)])
        .with_read_exit([Ok(Some(ExitCode::new(1)))]);

    let detail = show(&tasks, &runs, TASK);
    let attempt = attempt_of(&detail);

    assert_eq!(
        detail.execution,
        ExecutionState::Stopped {
            reason: StopReason::RetryLimitExceeded,
            notified_at: None,
        }
    );
    assert_eq!(
        attempt.run_dir_presence,
        RunDirPresence::Present {
            exit: ExitOutcome::Recorded(ExitCode::new(1))
        }
    );
    assert_eq!(attempt.run_dir, run_dir(TASK, 1));
    assert!(
        detail.last_failure.is_none(),
        "エージェント実行自体の失敗は失敗要因に記録されない"
    );
}

#[test]
fn 未通知の凍結は通知時刻が未記録であることを示す() {
    let tasks = repository(task(TASK).stopped(StopReason::JudgeLimitExceeded).found());

    let detail = show(&tasks, &untouched(), TASK);

    assert_eq!(
        detail.execution,
        ExecutionState::Stopped {
            reason: StopReason::JudgeLimitExceeded,
            notified_at: None,
        }
    );
}

#[test]
fn 凍結していないタスクは凍結要因を持たない() {
    let tasks = repository(task(TASK).found());

    let detail = show(&tasks, &untouched(), TASK);

    assert_ne!(detail.execution.kind(), ExecutionStateKind::Stopped);
}

#[test]
fn リトライ上限は上書きがあればその値が併記される() {
    let tasks = repository(
        task(TASK)
            .snapshot(snapshot_with(
                Some("claude"),
                None,
                agent_run(AgentRunSpec {
                    retries: Some(5),
                    ..AgentRunSpec::default()
                }),
            ))
            .found(),
    );

    let detail = show(&tasks, &untouched(), TASK);

    assert_eq!(detail.limits.retry, RetryLimitInfo::Applicable(5));
}

#[test]
fn リトライ上限は上書きがなければ組み込みの既定値が併記される() {
    let tasks = repository(task(TASK).found());

    let detail = show(&tasks, &untouched(), TASK);

    assert_eq!(detail.limits.retry, RetryLimitInfo::Applicable(2));
}

#[test]
fn クリーンアップステータスのリトライ上限は常に2になる() {
    let tasks = repository(
        task(TASK)
            .snapshot(snapshot_of(
                None,
                None,
                vec![("done", StatusDefinition::Cleanup)],
            ))
            .status("done")
            .found(),
    );

    let detail = show(&tasks, &untouched(), TASK);

    assert_eq!(detail.limits.retry, RetryLimitInfo::Applicable(2));
}

#[test]
fn 何もしないステータスにはリトライ上限が併記されない() {
    let tasks = repository(
        task(TASK)
            .snapshot(snapshot_of(
                None,
                None,
                vec![("waiting", StatusDefinition::Wait)],
            ))
            .status("waiting")
            .found(),
    );

    let detail = show(&tasks, &untouched(), TASK);

    assert_eq!(
        detail.limits.retry,
        RetryLimitInfo::NotApplicable,
        "適用対象がないことは導出不能と区別される"
    );
}

#[test]
fn 判定とspawnの上限は設定の値で表示される() {
    let tasks = repository(task(TASK).found());

    let detail = ShowTask::new(&config_limits(7, 9), &state_root(), &tasks, &untouched())
        .execute(ShowTaskInput {
            task_id: TASK.to_owned(),
        })
        .expect("表示できる");

    assert_eq!(detail.limits.judge, 7);
    assert_eq!(detail.limits.spawn, 9);
}

#[test]
fn アーカイブ済みのタスクはアーカイブ側の保存先とともに表示される() {
    let tasks = repository(task(TASK).completed(1).found_archived());

    let detail = show(&tasks, &present_without_exit(), TASK);

    assert!(detail.archived, "アーカイブ済みであることが分かる");
    assert_eq!(
        detail.task_file_path,
        TaskFilePath::archived(&state_root(), &task_id(TASK))
    );
}

#[test]
fn アーカイブ側で見つかった縮退タスクはアーカイブ側の保存先を指す() {
    let tasks = repository(degraded(TASK, "statuses が空").found_archived());

    let detail = show(&tasks, &untouched(), TASK);

    assert!(detail.archived, "縮退していてもアーカイブ済みだと分かる");
    assert_eq!(
        detail.task_file_path,
        TaskFilePath::archived(&state_root(), &task_id(TASK)),
        "保存先の振り分けはスナップショットの可読性に依らない"
    );
    assert_eq!(
        detail.snapshot,
        SnapshotInfo::Unreadable("statuses が空".to_owned())
    );
}

#[test]
fn 現役のタスクの保存先はタスクファイル自身になる() {
    let tasks = repository(task(TASK).found());

    let detail = show(&tasks, &untouched(), TASK);

    assert_eq!(
        detail.task_file_path,
        TaskFilePath::active(&state_root(), &task_id(TASK))
    );
}

#[test]
fn スナップショットが読めるタスクは定義済みステータス一覧を示す() {
    let tasks = repository(task(TASK).found());

    let detail = show(&tasks, &untouched(), TASK);

    assert_eq!(
        detail.snapshot,
        SnapshotInfo::Readable(vec![status("done"), status("queued"), status("waiting")])
    );
}

#[test]
fn 現役にもアーカイブにも無いタスクは不在として拒否される() {
    let tasks = repository(TaskLookup::NotFound);

    let error = show_error(&tasks, &untouched(), TASK);

    assert_eq!(
        error,
        ShowTaskError::NotFound {
            task_id: task_id(TASK),
        }
    );
    assert_eq!(
        tasks.looked_up(),
        vec![task_id(TASK)],
        "解決はリポジトリの契約(現役 → アーカイブ)に委ねる"
    );
}

#[test]
fn 状態ディレクトリが無い場合もタスク不在として拒否される() {
    // 走査対象ディレクトリの不在は `find` の `NotFound` になる(ポートの契約)。
    let tasks = repository(TaskLookup::NotFound);

    let error = show_error(&tasks, &untouched(), TASK);

    assert!(matches!(error, ShowTaskError::NotFound { .. }));
}

#[test]
fn 読めないタスクファイルはパスと理由を添えて拒否される() {
    let path = TaskFilePath::active(&state_root(), &task_id(TASK));
    let tasks = repository(TaskLookup::Corrupt {
        path: path.clone(),
        message: "JSON として読めない".to_owned(),
    });

    let error = show_error(&tasks, &untouched(), TASK);

    assert_eq!(
        error,
        ShowTaskError::Corrupt {
            path,
            message: "JSON として読めない".to_owned(),
        }
    );
}

#[test]
fn 解決自体の失敗は実行環境のエラーになる() {
    let tasks = ScriptedTaskRepository::new().with_find([Err(ReadError::Io {
        message: "タスクの置き場を読めない".to_owned(),
    })]);

    let error = show_error(&tasks, &untouched(), TASK);

    assert_eq!(
        error,
        ShowTaskError::Read(ReadError::Io {
            message: "タスクの置き場を読めない".to_owned(),
        })
    );
}

#[test]
fn runディレクトリの存在確認の失敗は表示を止めない() {
    let tasks = repository(task(TASK).running(1).found());
    let runs = ScriptedRunStore::new().with_attempt_exists([Err(Io::Failed {
        message: "権限がありません".to_owned(),
    })]);

    let detail = show(&tasks, &runs, TASK);
    let attempt = attempt_of(&detail);

    assert_eq!(
        attempt.run_dir_presence,
        RunDirPresence::Unknown {
            message: "権限がありません".to_owned(),
        },
        "有無を確かめられていない以上、exit は読みに行かず記録の不在も主張しない"
    );
    assert_eq!(
        attempt.run_dir,
        run_dir(TASK, 1),
        "残りの項目は表示され続ける"
    );
}

#[test]
fn exitの読み取りの失敗は表示を止めず原因の分類のまま渡る() {
    for error in [
        RunFileError::Corrupt {
            path: run_dir(TASK, 1).exit_file(),
            message: "内容を解釈できない".to_owned(),
        },
        RunFileError::Io {
            message: "読み取れない".to_owned(),
        },
    ] {
        let tasks = repository(task(TASK).running(1).found());
        let runs = ScriptedRunStore::new()
            .with_attempt_exists([Ok(true)])
            .with_read_exit([Err(error.clone())]);

        let detail = show(&tasks, &runs, TASK);

        assert_eq!(
            attempt_of(&detail).run_dir_presence,
            RunDirPresence::Present {
                exit: ExitOutcome::Unreadable(error.clone())
            },
            "文言の組み立ては表示層に委ねる: {error:?}"
        );
        assert_eq!(
            attempt_of(&detail).run_dir,
            run_dir(TASK, 1),
            "残りの項目は表示され続ける"
        );
    }
}

#[test]
fn タスクidとして不正な文字列は入力境界で拒否される() {
    for (given, expected) in [
        ("", TaskIdError::Empty),
        (&"a".repeat(65), TaskIdError::TooLong),
        (
            "大文字を含む",
            TaskIdError::InvalidChar {
                char: '大',
                position: 0,
            },
        ),
        ("-leading", TaskIdError::InvalidLeadingChar),
    ] {
        // 検証は解決の前に行う — `find` の台本を与えていない。
        let tasks = ScriptedTaskRepository::new();

        let error = show_error(&tasks, &untouched(), given);

        assert_eq!(error, ShowTaskError::InvalidTaskId(expected), "{given}");
        assert!(tasks.looked_up().is_empty(), "{given}");
    }
}

#[test]
fn 境界の長さの有効なタスクidは受理される() {
    for given in ["a", &"a".repeat(64)] {
        let tasks = repository(TaskLookup::Active(pulsen_domain::task::TaskRecord::Intact(
            task(given).build(),
        )));

        let detail = show(&tasks, &untouched(), given);

        assert_eq!(
            detail.task_id,
            TaskId::parse(given.to_owned()).expect("受理される")
        );
    }
}

/// 落ちるのはスナップショット由来の項目だけ、という解釈を attempt の派生値
/// (run ディレクトリ・その有無・exit)と直近の失敗まで含めて固定する。
#[test]
fn スナップショットだけが読めないタスクは読める項目を残して注記される() {
    let tasks = repository(
        degraded(TASK, "statuses が空")
            .status("queued")
            .workspace()
            .attempt(1)
            .counters(2, 1, 0)
            .execution(ExecutionState::Failed)
            .last_failure(failure(
                FailureKind::JudgeFail,
                "判定コマンドが異常終了",
                at("2026-08-12T08:59:00Z"),
            ))
            .found(),
    );

    let detail = show(&tasks, &present_without_exit(), TASK);

    assert_eq!(detail.workflow_name, usecase_fixture::workflow_name());
    assert_eq!(detail.target.repo(), &usecase_fixture::repo());
    assert_eq!(detail.task_status, status("queued"));
    assert_eq!(detail.execution, ExecutionState::Failed);
    assert!(detail.workspace.is_some(), "タスクファイル由来の項目は残る");
    assert_eq!(detail.counters.attempt_count(), 2);
    let attempt = attempt_of(&detail);
    assert_eq!(attempt.number.get(), 1);
    assert_eq!(attempt.run_dir, run_dir(TASK, 1));
    assert_eq!(
        attempt.run_dir_presence,
        RunDirPresence::Present {
            exit: ExitOutcome::Absent
        }
    );
    assert_eq!(
        detail.last_failure.expect("失敗要因がある").kind(),
        FailureKind::JudgeFail
    );
    assert_eq!(detail.updated_at, at(usecase_fixture::NOW));
    assert_eq!(
        detail.snapshot,
        SnapshotInfo::Unreadable("statuses が空".to_owned()),
        "スナップショット由来の項目は出さず理由だけを残す"
    );
}

#[test]
fn スナップショット破損のリトライ上限は適用対象なしと区別される() {
    let tasks = repository(degraded(TASK, "statuses が空").found());

    let detail = show(&tasks, &untouched(), TASK);

    assert_eq!(
        detail.limits.retry,
        RetryLimitInfo::Unknown,
        "導出不能は「併記なし」ではない"
    );
    assert_ne!(detail.limits.retry, RetryLimitInfo::NotApplicable);
}

#[test]
fn スナップショット破損でも判定とspawnの上限は通常どおり表示される() {
    let tasks = repository(degraded(TASK, "statuses が空").found());

    let detail = ShowTask::new(&config_limits(7, 9), &state_root(), &tasks, &untouched())
        .execute(ShowTaskInput {
            task_id: TASK.to_owned(),
        })
        .expect("表示できる");

    assert_eq!(detail.limits.judge, 7);
    assert_eq!(detail.limits.spawn, 9);
}

#[test]
fn 削除済みのrunディレクトリは存在しないこととして表示される() {
    // gc 済み(判定確定後)と、起動記録直後のクラッシュ(launching)の2経路。
    for lookup in [
        task(TASK).completed(1).found(),
        task(TASK)
            .launching(at(usecase_fixture::NOW))
            .attempt(1)
            .found(),
    ] {
        let tasks = repository(lookup);
        let runs = ScriptedRunStore::new().with_attempt_exists([Ok(false)]);

        let detail = show(&tasks, &runs, TASK);
        let attempt = attempt_of(&detail);

        assert_eq!(
            attempt.run_dir_presence,
            RunDirPresence::Absent,
            "ディレクトリごと無いので exit の記録も無い"
        );
        assert_eq!(attempt.run_dir, run_dir(TASK, 1), "パスは示され続ける");
    }
}

#[test]
fn workspaceのパスは存在を検証せずそのまま表示される() {
    // ワークスペースのパスは一時ディレクトリですらない導出値で、実体は存在しない。
    // 表示されて 0 で終わることが「存在検証を行わない」の観測可能な帰結になる。
    let tasks = repository(task(TASK).running(1).found());

    let detail = show(&tasks, &present_without_exit(), TASK);
    let workspace = detail.workspace.expect("ワークスペースは確定済み");

    assert!(!workspace.path().as_path().exists(), "実体は存在しない");
    assert!(workspace.path().as_path().ends_with(TASK));
}

#[test]
fn 無効化マーカーだけが残るattemptもタスクファイルが指す現在attemptとして示される() {
    // spawn 失敗で起動待ちへ戻った痕跡。attempt ディレクトリは在るが exit は無い。
    let tasks = repository(task(TASK).attempt(2).counters(0, 0, 1).found());

    let detail = show(&tasks, &present_without_exit(), TASK);
    let attempt = attempt_of(&detail);

    assert_eq!(attempt.number.get(), 2);
    assert_eq!(attempt.run_dir, run_dir(TASK, 2));
    assert_eq!(
        attempt.run_dir_presence,
        RunDirPresence::Present {
            exit: ExitOutcome::Absent
        }
    );
}

#[test]
fn 同期spawn失敗で凍結したタスクはattemptを持たない() {
    let tasks = repository(
        task(TASK)
            .stopped(StopReason::SpawnFailLimitExceeded)
            .counters(0, 0, 3)
            .last_failure(failure(
                FailureKind::SpawnFail,
                "エージェント `claude` は config.yaml に定義されていません",
                at("2026-08-12T09:30:00Z"),
            ))
            .found(),
    );

    let detail = show(&tasks, &untouched(), TASK);

    assert_eq!(
        detail.execution,
        ExecutionState::Stopped {
            reason: StopReason::SpawnFailLimitExceeded,
            notified_at: None,
        }
    );
    assert_eq!(
        detail.last_failure.expect("失敗要因がある").kind(),
        FailureKind::SpawnFail
    );
    assert!(
        detail.attempt.is_none(),
        "採番されないので猶予経路(runディレクトリに痕跡が残る)と判別できる"
    );
    assert_eq!(detail.counters.spawn_fail_count(), 3);
}

#[test]
fn 更新日時は帳簿の値がそのまま出る() {
    let updated: Timestamp = at("2026-08-12T11:22:33Z");
    let tasks = repository(task(TASK).updated_at(updated).found());

    let detail = show(&tasks, &untouched(), TASK);

    assert_eq!(detail.updated_at, updated);
}
