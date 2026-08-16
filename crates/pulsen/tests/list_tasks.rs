//! タスク一覧ユースケースの振る舞い。
//!
//! ポートはテストダブルに差し替える。実アダプターでは外から作れない状況(走査の
//! 入出力エラー・スナップショットだけが読めないタスク・アーカイブ側の破損)と、
//! `--state` の6値の網羅をここで消化する。
//!
//! **`ExclusiveLock` を渡す口が無いこと**は型が示すため、テストでは主張しない。

mod usecase_fixture;

use pulsen::application::list_tasks::{ListTasks, ListTasksError, ListTasksInput, TaskList};
use pulsen_conformance::doubles::ScriptedTaskRepository;
use pulsen_domain::task::{
    ExecutionState, ExecutionStateKind, ReadError, StateKindError, StopReason, TaskEntry,
};

use usecase_fixture::{corrupt_entry, degraded, degraded_entry_with, status, task};

/// 現役のみを与えるリポジトリ。
fn active(entries: Vec<TaskEntry>) -> ScriptedTaskRepository {
    ScriptedTaskRepository::new().with_list_active([Ok(entries)])
}

/// 現役とアーカイブの両方を与えるリポジトリ。
fn both(active: Vec<TaskEntry>, archived: Vec<TaskEntry>) -> ScriptedTaskRepository {
    ScriptedTaskRepository::new()
        .with_list_active([Ok(active)])
        .with_list_archived([Ok(archived)])
}

/// 絞り込みなしの入力。
fn plain() -> ListTasksInput {
    ListTasksInput::default()
}

fn with_status(value: &str) -> ListTasksInput {
    ListTasksInput {
        status: Some(value.to_owned()),
        ..ListTasksInput::default()
    }
}

fn with_state(value: &str) -> ListTasksInput {
    ListTasksInput {
        state: Some(value.to_owned()),
        ..ListTasksInput::default()
    }
}

fn list(tasks: &ScriptedTaskRepository, input: ListTasksInput) -> TaskList {
    ListTasks::new(tasks).execute(input).expect("一覧できる")
}

/// 行として出たタスクIDの並び。
fn ids(list: &TaskList) -> Vec<String> {
    list.rows
        .iter()
        .map(|row| row.task_id.as_str().to_owned())
        .collect()
}

/// 6つの実行状態それぞれを持つタスクの走査結果。
fn every_state() -> Vec<TaskEntry> {
    vec![
        task("20260812t090000-state001").entry(),
        task("20260812t090000-state002")
            .launching(usecase_fixture::at("2026-08-12T09:00:00Z"))
            .attempt(1)
            .entry(),
        task("20260812t090000-state003").running(1).entry(),
        task("20260812t090000-state004").completed(1).entry(),
        task("20260812t090000-state005")
            .execution(ExecutionState::Failed)
            .entry(),
        task("20260812t090000-state006")
            .stopped(StopReason::Aborted)
            .entry(),
    ]
}

#[test]
fn 特定のタスクステータスを持つタスクだけが絞り込まれる() {
    let tasks = active(vec![
        task("20260812t090000-aaaa0001").status("queued").entry(),
        task("20260812t090000-aaaa0002").status("done").entry(),
    ]);

    let list = list(&tasks, with_status("done"));

    assert_eq!(ids(&list), vec!["20260812t090000-aaaa0002"]);
}

#[test]
fn 指定した実行状態のタスクだけが絞り込まれる() {
    let tasks = active(vec![
        task("20260812t090000-aaaa0001").entry(),
        task("20260812t090000-aaaa0002").running(1).entry(),
    ]);

    let list = list(&tasks, with_state("running"));

    assert_eq!(ids(&list), vec!["20260812t090000-aaaa0002"]);
    assert_eq!(list.rows[0].execution_state, ExecutionStateKind::Running);
}

#[test]
fn タスクステータスと実行状態は論理積で絞り込まれる() {
    let tasks = active(vec![
        // タスクステータスだけ一致。
        task("20260812t090000-aaaa0001").status("done").entry(),
        // 実行状態だけ一致。
        task("20260812t090000-aaaa0002")
            .status("queued")
            .running(1)
            .entry(),
        // 両方一致。
        task("20260812t090000-aaaa0003")
            .status("done")
            .running(1)
            .entry(),
    ]);

    let list = list(
        &tasks,
        ListTasksInput {
            status: Some("done".to_owned()),
            state: Some("running".to_owned()),
            all: false,
        },
    );

    assert_eq!(ids(&list), vec!["20260812t090000-aaaa0003"]);
}

#[test]
fn 既定ではアーカイブ済みのタスクは一覧に現れない() {
    // `--all` を付けないとアーカイブ側は走査されない(台本を与えていない
    // `list_archived` が呼ばれればパニックする)。
    let tasks = active(vec![task("20260812t090000-aaaa0001").entry()]);

    let list = list(&tasks, plain());

    assert_eq!(ids(&list), vec!["20260812t090000-aaaa0001"]);
    assert!(list.rows.iter().all(|row| !row.archived));
}

#[test]
fn 全件指定ではアーカイブ済みのタスクも印つきで現れる() {
    let tasks = both(
        vec![task("20260812t090000-aaaa0001").entry()],
        vec![task("20260812t090000-aaaa0002").running(1).entry()],
    );

    let list = list(
        &tasks,
        ListTasksInput {
            all: true,
            ..ListTasksInput::default()
        },
    );

    assert_eq!(
        ids(&list),
        vec!["20260812t090000-aaaa0001", "20260812t090000-aaaa0002"]
    );
    assert!(!list.rows[0].archived);
    assert!(list.rows[1].archived);
    assert!(
        list.rows[1].branch.is_some(),
        "成果の回収に使うブランチはアーカイブ済みでも載る"
    );
}

#[test]
fn 全件指定は対象集合の拡張であり拡張後に絞り込みが適用される() {
    let tasks = both(
        vec![
            task("20260812t090000-aaaa0001").status("done").entry(),
            task("20260812t090000-aaaa0002").status("queued").entry(),
        ],
        vec![
            task("20260812t090000-aaaa0003").status("done").entry(),
            task("20260812t090000-aaaa0004").status("queued").entry(),
        ],
    );

    let list = list(
        &tasks,
        ListTasksInput {
            status: Some("done".to_owned()),
            state: None,
            all: true,
        },
    );

    assert_eq!(
        ids(&list),
        vec!["20260812t090000-aaaa0001", "20260812t090000-aaaa0003"],
        "現役とアーカイブの両方から条件一致だけが残る"
    );
}

#[test]
fn 絞り込みに一致するタスクがなければ空の一覧になる() {
    let tasks = active(vec![
        task("20260812t090000-aaaa0001").status("queued").entry(),
    ]);

    let list = list(&tasks, with_status("done"));

    assert!(list.rows.is_empty());
    assert!(list.unreadable.is_empty());
}

#[test]
fn 存在しないタスクステータス名は検証されず該当0件になる() {
    let tasks = active(vec![task("20260812t090000-aaaa0001").entry()]);

    let list = list(&tasks, with_status("どのワークフローにも無い名前"));

    assert!(list.rows.is_empty(), "値は検証せず該当なしとして扱う");
}

#[test]
fn 空文字列のタスクステータスは検証されず該当0件になる() {
    let tasks = active(vec![task("20260812t090000-aaaa0001").entry()]);

    let list = list(&tasks, with_status(""));

    assert!(list.rows.is_empty());
}

#[test]
fn 実行状態の6値はすべて受理されそれぞれ該当タスクだけが残る() {
    for (given, expected) in [
        ("pending", "20260812t090000-state001"),
        ("launching", "20260812t090000-state002"),
        ("running", "20260812t090000-state003"),
        ("completed", "20260812t090000-state004"),
        ("failed", "20260812t090000-state005"),
        ("stopped", "20260812t090000-state006"),
    ] {
        let tasks = active(every_state());

        let list = list(&tasks, with_state(given));

        assert_eq!(ids(&list), vec![expected.to_owned()], "{given}");
    }
}

#[test]
fn 固定6値以外の実行状態は有効値一覧を添えて拒否される() {
    for given in ["Pending", "PENDING", "queued", ""] {
        let tasks = ScriptedTaskRepository::new();

        let error = ListTasks::new(&tasks)
            .execute(with_state(given))
            .expect_err("拒否される");

        assert_eq!(
            error,
            ListTasksError::InvalidState(StateKindError::Unknown {
                given: given.to_owned(),
                valid: ExecutionStateKind::VALID,
            }),
            "{given}"
        );
    }
}

#[test]
fn 現役の走査自体の失敗は実行環境のエラーになる() {
    let tasks = ScriptedTaskRepository::new().with_list_active([Err(ReadError::Io {
        message: "タスクの置き場を読めない".to_owned(),
    })]);

    let error = ListTasks::new(&tasks)
        .execute(plain())
        .expect_err("失敗する");

    assert_eq!(
        error,
        ListTasksError::Scan(ReadError::Io {
            message: "タスクの置き場を読めない".to_owned(),
        })
    );
}

#[test]
fn アーカイブの走査自体の失敗も実行環境のエラーになる() {
    let tasks = ScriptedTaskRepository::new()
        .with_list_active([Ok(Vec::new())])
        .with_list_archived([Err(ReadError::Io {
            message: "アーカイブの置き場を読めない".to_owned(),
        })]);

    let error = ListTasks::new(&tasks)
        .execute(ListTasksInput {
            all: true,
            ..ListTasksInput::default()
        })
        .expect_err("失敗する");

    assert_eq!(
        error,
        ListTasksError::Scan(ReadError::Io {
            message: "アーカイブの置き場を読めない".to_owned(),
        })
    );
}

#[test]
fn スナップショットだけが読めないタスクは印つきの行として現れる() {
    let tasks = active(vec![
        task("20260812t090000-aaaa0001").entry(),
        degraded_entry_with(
            "20260812t090000-aaaa0002",
            "statuses が空",
            ExecutionState::Pending,
        ),
    ]);

    let list = list(&tasks, plain());

    assert_eq!(
        ids(&list),
        vec!["20260812t090000-aaaa0001", "20260812t090000-aaaa0002"]
    );
    assert!(!list.rows[0].snapshot_unreadable);
    assert!(list.rows[1].snapshot_unreadable);
    assert!(
        list.unreadable.is_empty(),
        "パス付きの報告に載るのはファイル全体が読めないものだけ"
    );
}

#[test]
fn スナップショットだけが読めないタスクも絞り込みの対象になる() {
    let entries = vec![
        degraded_entry_with(
            "20260812t090000-aaaa0001",
            "statuses が空",
            ExecutionState::Running,
        ),
        degraded_entry_with(
            "20260812t090000-aaaa0002",
            "statuses が空",
            ExecutionState::Pending,
        ),
    ];

    let by_state = list(&active(entries.clone()), with_state("running"));
    let by_status = list(&active(entries), with_status("queued"));

    assert_eq!(
        ids(&by_state),
        vec!["20260812t090000-aaaa0001"],
        "実行状態は読めているので絞り込みが効く"
    );
    assert_eq!(
        by_status.rows.len(),
        2,
        "タスクステータスも読めている: {:?}",
        ids(&by_status)
    );
    assert!(
        by_status
            .rows
            .iter()
            .all(|row| row.task_status == status("queued"))
    );
}

#[test]
fn 読めないタスクファイルが混ざっても一覧は失敗しない() {
    let tasks = active(vec![
        corrupt_entry("20260812t090000-aaaa0001", "JSON として読めない"),
        task("20260812t090000-aaaa0002").entry(),
    ]);

    let list = list(&tasks, plain());

    assert_eq!(ids(&list), vec!["20260812t090000-aaaa0002"]);
    assert_eq!(list.unreadable.len(), 1);
    assert!(
        list.unreadable[0]
            .path
            .ends_with("20260812t090000-aaaa0001.json"),
        "修復の入口としてパスが載る: {:?}",
        list.unreadable[0].path
    );
    assert_eq!(list.unreadable[0].message, "JSON として読めない");
}

#[test]
fn アーカイブ側の読めないタスクファイルも報告され一覧は失敗しない() {
    let tasks = both(
        vec![task("20260812t090000-aaaa0001").entry()],
        vec![corrupt_entry(
            "20260812t090000-aaaa0002",
            "JSON として読めない",
        )],
    );

    let list = list(
        &tasks,
        ListTasksInput {
            all: true,
            ..ListTasksInput::default()
        },
    );

    assert_eq!(ids(&list), vec!["20260812t090000-aaaa0001"]);
    assert_eq!(list.unreadable.len(), 1);
}

#[test]
fn 読めないタスクファイルの報告は絞り込みで消えない() {
    let tasks = active(vec![
        corrupt_entry("20260812t090000-aaaa0001", "JSON として読めない"),
        task("20260812t090000-aaaa0002").status("queued").entry(),
    ]);

    let list = list(&tasks, with_status("該当しない名前"));

    assert!(list.rows.is_empty());
    assert_eq!(
        list.unreadable.len(),
        1,
        "修復の入口は絞り込みの結果に左右されない"
    );
}

#[test]
fn 一覧の各行は表示に要る項目をすべて持つ() {
    let tasks = active(vec![
        task("20260812t090000-aaaa0001")
            .workflow("review")
            .status("done")
            .running(1)
            .counters(2, 0, 0)
            .entry(),
    ]);

    let list = list(&tasks, plain());
    let row = &list.rows[0];

    assert_eq!(row.task_id.as_str(), "20260812t090000-aaaa0001");
    assert_eq!(row.workflow_name.as_str(), "review");
    assert_eq!(row.repo, usecase_fixture::repo());
    assert!(row.branch.is_some());
    assert_eq!(row.task_status, status("done"));
    assert_eq!(row.execution_state, ExecutionStateKind::Running);
    assert_eq!(row.attempt_count, 2);
    assert_eq!(row.updated_at, usecase_fixture::at(usecase_fixture::NOW));
}

/// スナップショットが読めなくても、行に出る列は通常の行と同じ値で埋まる
/// (印が1つ立つだけ、という解釈を派生値まで含めて固定する)。
#[test]
fn スナップショットだけが読めない行も表示に要る項目をすべて持つ() {
    let tasks = active(vec![
        degraded("20260812t090000-aaaa0002", "statuses が空")
            .status("done")
            .execution(ExecutionState::Running)
            .workspace()
            .counters(2, 0, 0)
            .entry(),
    ]);

    let list = list(&tasks, plain());
    let row = &list.rows[0];

    assert_eq!(row.task_id.as_str(), "20260812t090000-aaaa0002");
    assert_eq!(row.workflow_name, usecase_fixture::workflow_name());
    assert_eq!(row.repo, usecase_fixture::repo());
    assert!(row.branch.is_some());
    assert_eq!(row.task_status, status("done"));
    assert_eq!(row.execution_state, ExecutionStateKind::Running);
    assert_eq!(row.attempt_count, 2);
    assert_eq!(row.updated_at, usecase_fixture::at(usecase_fixture::NOW));
    assert!(row.snapshot_unreadable);
}
