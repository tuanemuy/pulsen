//! TaskRepository の適合ケース(`spec/testcases/ports/task-repository.md` の44行)。
//!
//! 1行 = 1ケース関数 = 1 `#[test]`。タスクのフィクスチャは `Task::rehydrate` /
//! `DegradedTask::rehydrate` で組み、破損の前提条件はハーネスのフックが「何が壊れて
//! いるか」だけを受け取る。永続化技術には依存しない。
//!
//! 破損の分類に対して固定するのは「どの区分で返るか」と「読めるはずのフィールドが
//! 読めること」までにする。理由の文言はアダプターが決めてよい。

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;

use pulsen_domain::definition::{
    AgentInput, AgentName, Prompt, StatusDefinition, StatusName, WorkflowDefinition, WorkflowName,
    WorkflowSnapshot,
};
use pulsen_domain::task::{
    ArchiveError, AttemptNumber, AttemptRef, BranchName, CreateError, DegradedTask,
    DegradedTaskFields, ExecutionState, FailureKind, FailureNote, KillIdent, Pid, ProcessIdent,
    ProcessStartTime, ReadError, RepoPath, RetryCounters, RunDirPath, SaveError, StartTimeRecord,
    StateRoot, StopReason, Target, Task, TaskEntry, TaskFields, TaskId, TaskLookup, TaskRecord,
    TaskRepository, Timestamp, Workspace, WorktreePath,
};

use crate::{Area, CaseOutcome, TaskRepositoryHarness, require};

/// エージェント実行のステータス。遷移先は `DONE`。
const QUEUED: &str = "queued";
/// 何もしないステータス。
const WAITING: &str = "waiting";
/// クリーンアップのステータス。
const DONE: &str = "done";
/// スナップショットに定義の無いステータス名。
const UNDEFINED_STATUS: &str = "どこにも定義がないステータス";

pub fn tc_port_task_repository_001_作成でディレクトリごと用意され読み戻せる(
    harness: &impl TaskRepositoryHarness,
) -> CaseOutcome {
    let task = task("task-a");

    create(harness, &task);

    assert_eq!(active_intact(harness, task.id()), task);
    CaseOutcome::Ran
}

pub fn tc_port_task_repository_002_現役に同じidがあれば作成は衝突する(
    harness: &impl TaskRepositoryHarness,
) -> CaseOutcome {
    let task = task("task-a");
    create(harness, &task);

    assert_eq!(harness.repo().create(&task), Err(CreateError::Conflict));
    CaseOutcome::Ran
}

pub fn tc_port_task_repository_003_アーカイブに同じidがあれば作成は衝突する(
    harness: &impl TaskRepositoryHarness,
) -> CaseOutcome {
    let task = task("task-a");
    create(harness, &task);
    archive(harness, task.id());

    assert_eq!(harness.repo().create(&task), Err(CreateError::Conflict));
    CaseOutcome::Ran
}

pub fn tc_port_task_repository_004_破損したファイルがあっても作成は衝突し内容を書き換えない(
    harness: &impl TaskRepositoryHarness,
) -> CaseOutcome {
    let task = task("task-a");
    create(harness, &task);
    require!(harness.corrupt_whole_record(Area::Active, task.id()));
    let before = require!(harness.record_bytes(Area::Active, task.id()));

    assert_eq!(harness.repo().create(&task), Err(CreateError::Conflict));

    assert_eq!(
        harness.record_bytes(Area::Active, task.id()),
        Some(before),
        "破損ファイルを上書きせず修復の材料を消さない"
    );
    CaseOutcome::Ran
}

pub fn tc_port_task_repository_005_書き込み先を用意できなければ作成は入出力エラーになる(
    harness: &impl TaskRepositoryHarness,
) -> CaseOutcome {
    let task = task("task-a");
    let _restore = require!(harness.make_unwritable(Area::Active));

    match harness.repo().create(&task) {
        Err(CreateError::Io { message }) => assert!(!message.is_empty(), "理由が空"),
        other => panic!("書き込めない状況では入出力エラーになる: {other:?}"),
    }
    CaseOutcome::Ran
}

pub fn tc_port_task_repository_006_保存した内容は直後の検索で読み戻せる(
    harness: &impl TaskRepositoryHarness,
) -> CaseOutcome {
    let task = task("task-a");
    create(harness, &task);
    let advanced = Task::rehydrate(TaskFields {
        execution: ExecutionState::Failed,
        counters: RetryCounters::rehydrate(1, 0, 2),
        updated_at: moment(60),
        ..fields("task-a")
    })
    .expect("不変条件1を満たす");

    assert_eq!(harness.repo().save(&advanced), Ok(()));

    assert_eq!(active_intact(harness, task.id()), advanced);
    CaseOutcome::Ran
}

pub fn tc_port_task_repository_007_作成していないidの保存は見つからない(
    harness: &impl TaskRepositoryHarness,
) -> CaseOutcome {
    assert_eq!(
        harness.repo().save(&task("task-a")),
        Err(SaveError::NotFound)
    );
    CaseOutcome::Ran
}

pub fn tc_port_task_repository_008_アーカイブ済みタスクの保存は見つからない(
    harness: &impl TaskRepositoryHarness,
) -> CaseOutcome {
    let task = task("task-a");
    create(harness, &task);
    archive(harness, task.id());

    assert_eq!(harness.repo().save(&task), Err(SaveError::NotFound));
    CaseOutcome::Ran
}

pub fn tc_port_task_repository_009_縮退保存は壊れたスナップショットを温存する(
    harness: &impl TaskRepositoryHarness,
) -> CaseOutcome {
    let task = task("task-a");
    create(harness, &task);
    require!(harness.corrupt_snapshot(Area::Active, task.id()));
    let broken = require!(harness.snapshot_bytes(Area::Active, task.id()));
    let degraded = active_degraded(harness, task.id());

    let aborted = DegradedTask::rehydrate(DegradedTaskFields {
        execution: ExecutionState::Stopped {
            reason: StopReason::Aborted,
            notified_at: None,
        },
        updated_at: moment(60),
        ..degraded_fields(&degraded)
    });
    assert_eq!(harness.repo().save_degraded(&aborted), Ok(()));

    let reloaded = active_degraded(harness, task.id());
    assert_eq!(
        reloaded.execution(),
        &ExecutionState::Stopped {
            reason: StopReason::Aborted,
            notified_at: None,
        }
    );
    assert_eq!(reloaded.updated_at(), moment(60));
    assert_eq!(
        harness.snapshot_bytes(Area::Active, task.id()),
        Some(broken),
        "壊れたスナップショットは修復の材料として温存される"
    );
    CaseOutcome::Ran
}

pub fn tc_port_task_repository_010_現役にないidの縮退保存は見つからない(
    harness: &impl TaskRepositoryHarness,
) -> CaseOutcome {
    assert_eq!(
        harness.repo().save_degraded(&degraded("task-a")),
        Err(SaveError::NotFound)
    );
    CaseOutcome::Ran
}

pub fn tc_port_task_repository_011_書き込めない状態の保存は入出力エラーになる(
    harness: &impl TaskRepositoryHarness,
) -> CaseOutcome {
    let task = task("task-a");
    create(harness, &task);
    let _restore = require!(harness.make_unwritable(Area::Active));

    match harness.repo().save(&task) {
        Err(SaveError::Io { message }) => assert!(!message.is_empty(), "理由が空"),
        other => panic!("書き込めない状況では入出力エラーになる: {other:?}"),
    }
    CaseOutcome::Ran
}

pub fn tc_port_task_repository_012_書き込めない状態の縮退保存は入出力エラーになる(
    harness: &impl TaskRepositoryHarness,
) -> CaseOutcome {
    let task = task("task-a");
    create(harness, &task);
    require!(harness.corrupt_snapshot(Area::Active, task.id()));
    let degraded = active_degraded(harness, task.id());
    let _restore = require!(harness.make_unwritable(Area::Active));

    match harness.repo().save_degraded(&degraded) {
        Err(SaveError::Io { message }) => assert!(!message.is_empty(), "理由が空"),
        other => panic!("書き込めない状況では入出力エラーになる: {other:?}"),
    }
    CaseOutcome::Ran
}

pub fn tc_port_task_repository_013_全ての任意フィールドを持つタスクが往復する(
    harness: &impl TaskRepositoryHarness,
) -> CaseOutcome {
    let task = full_task("task-a");

    create(harness, &task);

    let reloaded = active_intact(harness, task.id());
    assert_eq!(reloaded, task);
    assert_eq!(reloaded.snapshot(), task.snapshot());
    assert_eq!(reloaded.workspace(), task.workspace());
    assert_eq!(reloaded.current_attempt(), task.current_attempt());
    assert_eq!(reloaded.last_failure(), task.last_failure());
    assert_eq!(reloaded.counters(), task.counters());
    CaseOutcome::Ran
}

pub fn tc_port_task_repository_014_実行状態は付随データごと復元される(
    harness: &impl TaskRepositoryHarness,
) -> CaseOutcome {
    let states = [
        ExecutionState::Pending,
        ExecutionState::Launching {
            recorded_at: moment(30),
        },
        ExecutionState::Running,
        ExecutionState::Completed,
        ExecutionState::Failed,
        ExecutionState::Stopped {
            reason: StopReason::JudgeLimitExceeded,
            notified_at: Some(moment(90)),
        },
    ];
    create(harness, &task("task-a"));

    for state in states {
        let task = Task::rehydrate(TaskFields {
            execution: state.clone(),
            ..fields("task-a")
        })
        .expect("不変条件1を満たす");

        assert_eq!(harness.repo().save(&task), Ok(()));

        assert_eq!(active_intact(harness, task.id()).execution(), &state);
    }
    CaseOutcome::Ran
}

pub fn tc_port_task_repository_015_何も作成していなければ検索は見つからない(
    harness: &impl TaskRepositoryHarness,
) -> CaseOutcome {
    assert_eq!(found(harness, &task_id("task-a")), TaskLookup::NotFound);
    CaseOutcome::Ran
}

pub fn tc_port_task_repository_016_作成済みタスクは現役として見つかる(
    harness: &impl TaskRepositoryHarness,
) -> CaseOutcome {
    let task = task("task-a");
    create(harness, &task);

    assert_eq!(
        found(harness, task.id()),
        TaskLookup::Active(TaskRecord::Intact(task))
    );
    CaseOutcome::Ran
}

pub fn tc_port_task_repository_017_アーカイブ済みタスクはアーカイブとして見つかる(
    harness: &impl TaskRepositoryHarness,
) -> CaseOutcome {
    let task = task("task-a");
    create(harness, &task);
    archive(harness, task.id());

    assert_eq!(
        found(harness, task.id()),
        TaskLookup::Archived(TaskRecord::Intact(task))
    );
    CaseOutcome::Ran
}

pub fn tc_port_task_repository_018_双方に置かれたidは現役として返る(
    harness: &impl TaskRepositoryHarness,
) -> CaseOutcome {
    let task = task("task-a");
    create(harness, &task);
    require!(harness.place_in_both_areas(task.id()));

    assert_eq!(
        found(harness, task.id()),
        TaskLookup::Active(TaskRecord::Intact(task))
    );
    CaseOutcome::Ran
}

pub fn tc_port_task_repository_019_走査対象を読み取れない検索は入出力エラーになる(
    harness: &impl TaskRepositoryHarness,
) -> CaseOutcome {
    let task = task("task-a");
    create(harness, &task);
    let _restore = require!(harness.make_unreadable(Area::Active));

    match harness.repo().find(task.id()) {
        Err(ReadError::Io { message }) => assert!(!message.is_empty(), "理由が空"),
        other => panic!("機構の失敗は値のエラーとして届く: {other:?}"),
    }
    CaseOutcome::Ran
}

pub fn tc_port_task_repository_020_ファイル全体の破損は破損として返る(
    harness: &impl TaskRepositoryHarness,
) -> CaseOutcome {
    let task = task("task-a");
    create(harness, &task);
    require!(harness.corrupt_whole_record(Area::Active, task.id()));

    assert_corrupt(found(harness, task.id()), task.id());
    CaseOutcome::Ran
}

pub fn tc_port_task_repository_021_タスク側フィールドの破れは全体の破損として返る(
    harness: &impl TaskRepositoryHarness,
) -> CaseOutcome {
    let task = task("task-a");
    create(harness, &task);
    require!(harness.break_task_field(Area::Active, task.id()));

    assert_corrupt(found(harness, task.id()), task.id());
    CaseOutcome::Ran
}

pub fn tc_port_task_repository_022_スナップショットだけが読めないタスクは縮退として返る(
    harness: &impl TaskRepositoryHarness,
) -> CaseOutcome {
    let task = full_task("task-a");
    create(harness, &task);
    require!(harness.corrupt_snapshot(Area::Active, task.id()));

    let degraded = active_degraded(harness, task.id());

    assert!(!degraded.snapshot_error().is_empty(), "理由が空");
    assert_task_side_fields(&degraded, &task);
    CaseOutcome::Ran
}

pub fn tc_port_task_repository_023_スナップショットの欠落も縮退として返る(
    harness: &impl TaskRepositoryHarness,
) -> CaseOutcome {
    let task = full_task("task-a");
    create(harness, &task);
    require!(harness.drop_snapshot_field(Area::Active, task.id()));

    let degraded = active_degraded(harness, task.id());

    assert!(!degraded.snapshot_error().is_empty(), "理由が空");
    assert_task_side_fields(&degraded, &task);
    CaseOutcome::Ran
}

pub fn tc_port_task_repository_024_スナップショットにないタスクステータスは縮退として返る(
    harness: &impl TaskRepositoryHarness,
) -> CaseOutcome {
    let task = task("task-a");
    create(harness, &task);
    require!(harness.set_task_status_outside_snapshot(Area::Active, task.id(), UNDEFINED_STATUS));

    let degraded = active_degraded(harness, task.id());

    assert_eq!(degraded.task_status().as_str(), UNDEFINED_STATUS);
    assert!(!degraded.snapshot_error().is_empty(), "理由が空");
    CaseOutcome::Ran
}

pub fn tc_port_task_repository_025_スナップショットの構造不変条件の破れは縮退として返る(
    harness: &impl TaskRepositoryHarness,
) -> CaseOutcome {
    let task = task("task-a");
    create(harness, &task);
    require!(harness.break_snapshot_invariant(Area::Active, task.id()));

    let degraded = active_degraded(harness, task.id());

    assert!(!degraded.snapshot_error().is_empty(), "理由が空");
    CaseOutcome::Ran
}

pub fn tc_port_task_repository_026_状態間整合の破れは復号で検証されない(
    harness: &impl TaskRepositoryHarness,
) -> CaseOutcome {
    let inconsistent = Task::rehydrate(TaskFields {
        execution: ExecutionState::Running,
        current_attempt: None,
        ..fields("task-a")
    })
    .expect("再構築が検証するのは不変条件1だけ");

    create(harness, &inconsistent);

    assert_eq!(active_intact(harness, inconsistent.id()), inconsistent);
    CaseOutcome::Ran
}

pub fn tc_port_task_repository_027_アーカイブ側の全体破損も破損として返る(
    harness: &impl TaskRepositoryHarness,
) -> CaseOutcome {
    let task = task("task-a");
    create(harness, &task);
    archive(harness, task.id());
    require!(harness.corrupt_whole_record(Area::Archived, task.id()));

    assert_corrupt(found(harness, task.id()), task.id());
    CaseOutcome::Ran
}

pub fn tc_port_task_repository_028_アーカイブ側のスナップショット破損は縮退として返る(
    harness: &impl TaskRepositoryHarness,
) -> CaseOutcome {
    let task = full_task("task-a");
    create(harness, &task);
    archive(harness, task.id());
    require!(harness.corrupt_snapshot(Area::Archived, task.id()));

    let degraded = match found(harness, task.id()) {
        TaskLookup::Archived(TaskRecord::SnapshotUnreadable(degraded)) => degraded,
        other => panic!("アーカイブ側の縮退として返る: {other:?}"),
    };

    assert!(!degraded.snapshot_error().is_empty(), "理由が空");
    assert_task_side_fields(&degraded, &task);
    CaseOutcome::Ran
}

pub fn tc_port_task_repository_029_現役の走査は検索と同じ区分で列挙する(
    harness: &impl TaskRepositoryHarness,
) -> CaseOutcome {
    let intact = task("task-a");
    let whole = task("task-b");
    let snapshot_only = task("task-c");
    let archived = task("task-d");
    for task in [&intact, &whole, &snapshot_only, &archived] {
        create(harness, task);
    }
    archive(harness, archived.id());
    require!(harness.corrupt_whole_record(Area::Active, whole.id()));
    require!(harness.corrupt_snapshot(Area::Active, snapshot_only.id()));
    require!(harness.corrupt_whole_record(Area::Archived, archived.id()));

    let entries = listed(harness.repo().list_active());

    assert_eq!(entries.len(), 3, "アーカイブ側は現れない: {entries:?}");
    assert_eq!(
        entry_for(&entries, intact.id()),
        Some(TaskEntry::Record(TaskRecord::Intact(intact.clone())))
    );
    assert_eq!(
        found(harness, intact.id()),
        TaskLookup::Active(TaskRecord::Intact(intact))
    );
    assert!(
        matches!(
            entry_for(&entries, whole.id()),
            Some(TaskEntry::Corrupt { .. })
        ),
        "全体破損は Corrupt として列挙される: {entries:?}"
    );
    assert_corrupt(found(harness, whole.id()), whole.id());
    assert!(
        matches!(
            entry_for(&entries, snapshot_only.id()),
            Some(TaskEntry::Record(TaskRecord::SnapshotUnreadable(_)))
        ),
        "スナップショット破損は縮退として列挙される: {entries:?}"
    );
    let _ = active_degraded(harness, snapshot_only.id());
    assert_eq!(entry_for(&entries, archived.id()), None);
    CaseOutcome::Ran
}

pub fn tc_port_task_repository_030_命名形式に合致しないエントリは列挙されない(
    harness: &impl TaskRepositoryHarness,
) -> CaseOutcome {
    let task = task("task-a");
    create(harness, &task);
    require!(harness.put_unnamed_entry(Area::Active));

    let entries = listed(harness.repo().list_active());

    assert_eq!(
        entries,
        vec![TaskEntry::Record(TaskRecord::Intact(task))],
        "形式外のエントリは Corrupt としても現れない"
    );
    CaseOutcome::Ran
}

pub fn tc_port_task_repository_031_アーカイブ移動は移動先を用意して行われる(
    harness: &impl TaskRepositoryHarness,
) -> CaseOutcome {
    let task = task("task-a");
    create(harness, &task);

    assert_eq!(harness.repo().archive(task.id()), Ok(()));

    assert_eq!(
        found(harness, task.id()),
        TaskLookup::Archived(TaskRecord::Intact(task))
    );
    CaseOutcome::Ran
}

pub fn tc_port_task_repository_032_アーカイブ直後に現役から消えてアーカイブに現れる(
    harness: &impl TaskRepositoryHarness,
) -> CaseOutcome {
    let task = full_task("task-a");
    create(harness, &task);

    archive(harness, task.id());

    assert_eq!(listed(harness.repo().list_active()), Vec::new());
    assert_eq!(
        listed(harness.repo().list_archived()),
        vec![TaskEntry::Record(TaskRecord::Intact(task.clone()))]
    );
    assert_eq!(
        found(harness, task.id()),
        TaskLookup::Archived(TaskRecord::Intact(task))
    );
    CaseOutcome::Ran
}

pub fn tc_port_task_repository_033_作成していないidのアーカイブは見つからない(
    harness: &impl TaskRepositoryHarness,
) -> CaseOutcome {
    assert_eq!(
        harness.repo().archive(&task_id("task-a")),
        Err(ArchiveError::NotFound)
    );
    CaseOutcome::Ran
}

pub fn tc_port_task_repository_034_アーカイブ済みidの再アーカイブは見つからない(
    harness: &impl TaskRepositoryHarness,
) -> CaseOutcome {
    let task = task("task-a");
    create(harness, &task);
    archive(harness, task.id());

    assert_eq!(
        harness.repo().archive(task.id()),
        Err(ArchiveError::NotFound)
    );
    CaseOutcome::Ran
}

pub fn tc_port_task_repository_035_移動先を用意できないアーカイブは入出力エラーになる(
    harness: &impl TaskRepositoryHarness,
) -> CaseOutcome {
    let task = full_task("task-a");
    create(harness, &task);
    let _restore = require!(harness.make_unwritable(Area::Archived));

    match harness.repo().archive(task.id()) {
        Err(ArchiveError::Io { message }) => assert!(!message.is_empty(), "理由が空"),
        other => panic!("移動先を用意できなければ入出力エラーになる: {other:?}"),
    }

    assert_eq!(
        found(harness, task.id()),
        TaskLookup::Active(TaskRecord::Intact(task)),
        "部分的な移動を残さない"
    );
    CaseOutcome::Ran
}

pub fn tc_port_task_repository_036_走査対象ディレクトリが無ければ空の一覧になる(
    harness: &impl TaskRepositoryHarness,
) -> CaseOutcome {
    assert_eq!(listed(harness.repo().list_active()), Vec::new());
    assert_eq!(listed(harness.repo().list_archived()), Vec::new());
    CaseOutcome::Ran
}

pub fn tc_port_task_repository_037_現役の走査はアーカイブしていないタスクだけを返す(
    harness: &impl TaskRepositoryHarness,
) -> CaseOutcome {
    let kept = task("task-a");
    let moved = task("task-b");
    let another = task("task-c");
    for task in [&kept, &moved, &another] {
        create(harness, task);
    }
    archive(harness, moved.id());

    let entries = listed(harness.repo().list_active());

    assert_eq!(entry_for(&entries, moved.id()), None);
    assert_eq!(
        entry_for(&entries, kept.id()),
        Some(TaskEntry::Record(TaskRecord::Intact(kept)))
    );
    assert_eq!(
        entry_for(&entries, another.id()),
        Some(TaskEntry::Record(TaskRecord::Intact(another)))
    );
    assert_eq!(entries.len(), 2);
    CaseOutcome::Ran
}

pub fn tc_port_task_repository_038_アーカイブの走査はアーカイブしたタスクだけを返す(
    harness: &impl TaskRepositoryHarness,
) -> CaseOutcome {
    let kept = task("task-a");
    let moved = task("task-b");
    let another = task("task-c");
    for task in [&kept, &moved, &another] {
        create(harness, task);
    }
    archive(harness, moved.id());

    assert_eq!(
        listed(harness.repo().list_archived()),
        vec![TaskEntry::Record(TaskRecord::Intact(moved))]
    );
    CaseOutcome::Ran
}

pub fn tc_port_task_repository_039_現役の個別の破損は走査全体を失敗させない(
    harness: &impl TaskRepositoryHarness,
) -> CaseOutcome {
    match mixed_area(harness, Area::Active) {
        Ok(entries) => assert_eq!(entries.len(), 3),
        Err(hook) => return CaseOutcome::skipped(hook),
    }
    CaseOutcome::Ran
}

pub fn tc_port_task_repository_040_アーカイブの個別の破損は走査全体を失敗させない(
    harness: &impl TaskRepositoryHarness,
) -> CaseOutcome {
    match mixed_area(harness, Area::Archived) {
        Ok(entries) => assert_eq!(entries.len(), 3),
        Err(hook) => return CaseOutcome::skipped(hook),
    }
    CaseOutcome::Ran
}

pub fn tc_port_task_repository_041_走査対象を読み取れなければ入出力エラーになる(
    harness: &impl TaskRepositoryHarness,
) -> CaseOutcome {
    create(harness, &task("task-a"));
    {
        let _restore = require!(harness.make_unreadable(Area::Active));

        match harness.repo().list_active() {
            Err(ReadError::Io { message }) => assert!(!message.is_empty(), "理由が空"),
            other => panic!("走査自体の失敗は空リストに写像しない: {other:?}"),
        }
    }
    {
        let _restore = require!(harness.make_unreadable(Area::Archived));

        match harness.repo().list_archived() {
            Err(ReadError::Io { message }) => assert!(!message.is_empty(), "理由が空"),
            other => panic!("走査自体の失敗は空リストに写像しない: {other:?}"),
        }
    }
    CaseOutcome::Ran
}

pub fn tc_port_task_repository_042_反復する保存の途中経過は読み手に観測されない(
    harness: &impl TaskRepositoryHarness,
) -> CaseOutcome {
    let repo = require!(harness.concurrent_repo());
    let small = task("task-a");
    let large = Task::rehydrate(TaskFields {
        snapshot: wide_snapshot(),
        task_status: status(WAITING),
        execution: ExecutionState::Failed,
        counters: RetryCounters::rehydrate(7, 8, 9),
        updated_at: moment(120),
        ..fields("task-a")
    })
    .expect("不変条件1を満たす");
    repo.create(&small).expect("作成できる");

    let writing = AtomicBool::new(true);
    let observations = AtomicUsize::new(0);
    thread::scope(|scope| {
        scope.spawn(|| {
            while writing.load(Ordering::Relaxed) {
                match repo.find(small.id()) {
                    Ok(TaskLookup::Active(TaskRecord::Intact(observed))) => assert!(
                        observed == small || observed == large,
                        "書きかけの内容を観測した"
                    ),
                    other => panic!("完全な保存内容だけが観測される: {other:?}"),
                }
                for entry in repo.list_active().expect("走査できる") {
                    match entry {
                        TaskEntry::Record(TaskRecord::Intact(observed)) => assert!(
                            observed == small || observed == large,
                            "書きかけの内容を観測した"
                        ),
                        other => panic!("完全な保存内容だけが観測される: {other:?}"),
                    }
                }
                observations.fetch_add(1, Ordering::Relaxed);
            }
        });

        for _ in 0..30 {
            repo.save(&large).expect("保存できる");
            repo.save(&small).expect("保存できる");
        }
        yield_until_observed(&observations);
        writing.store(false, Ordering::Relaxed);
    });

    assert!(
        observations.load(Ordering::Relaxed) > 0,
        "読み手が一度も観測していない"
    );
    CaseOutcome::Ran
}

pub fn tc_port_task_repository_043_失敗した保存は部分的な結果を残さない(
    harness: &impl TaskRepositoryHarness,
) -> CaseOutcome {
    let existing = task("task-a");
    create(harness, &existing);
    let absent = task("task-b");

    assert_eq!(harness.repo().save(&absent), Err(SaveError::NotFound));

    assert_eq!(found(harness, absent.id()), TaskLookup::NotFound);
    assert_eq!(
        listed(harness.repo().list_active()),
        vec![TaskEntry::Record(TaskRecord::Intact(existing.clone()))]
    );

    // 書き込みが始まってから失敗する分岐。前提を用意できない環境ではこの分岐だけを
    // 飛ばす — 行の主張(部分的な結果が残らない)は NotFound 分岐が常に観測する。
    let advanced = Task::rehydrate(TaskFields {
        snapshot: wide_snapshot(),
        task_status: status(WAITING),
        execution: ExecutionState::Failed,
        counters: RetryCounters::rehydrate(3, 4, 5),
        updated_at: moment(90),
        ..fields("task-a")
    })
    .expect("不変条件1を満たす");
    if let Some(_restore) = harness.make_unwritable(Area::Active) {
        match harness.repo().save(&advanced) {
            Err(SaveError::Io { message }) => assert!(!message.is_empty(), "理由が空"),
            other => panic!("書き込めない状況では入出力エラーになる: {other:?}"),
        }
    }

    assert_eq!(active_intact(harness, existing.id()), existing.clone());
    assert_eq!(
        listed(harness.repo().list_active()),
        vec![TaskEntry::Record(TaskRecord::Intact(existing))]
    );
    CaseOutcome::Ran
}

/// 読み手が最初の観測を終えるまで実行を譲る(TC-042 / TC-044)。
///
/// 観測が0回のまま書き手が走り切ると「中間状態を観測しなかった」が空虚に成立する。
/// 読み手がアサーションで落ちたときに待ち続けないよう、譲る回数に上限を置く。
fn yield_until_observed(observations: &AtomicUsize) {
    for _ in 0..YIELD_LIMIT {
        if observations.load(Ordering::Relaxed) > 0 {
            return;
        }
        thread::yield_now();
    }
}

/// `yield_until_observed` が譲る回数の上限。
const YIELD_LIMIT: usize = 10_000;

pub fn tc_port_task_repository_044_アーカイブ移動の中間状態は読み手に観測されない(
    harness: &impl TaskRepositoryHarness,
) -> CaseOutcome {
    let repo = require!(harness.concurrent_repo());
    let task = full_task("task-a");
    repo.create(&task).expect("作成できる");

    let moving = AtomicBool::new(true);
    let observations = AtomicUsize::new(0);
    thread::scope(|scope| {
        scope.spawn(|| {
            while moving.load(Ordering::Relaxed) {
                match repo.find(task.id()) {
                    Ok(TaskLookup::Active(TaskRecord::Intact(observed)))
                    | Ok(TaskLookup::Archived(TaskRecord::Intact(observed))) => {
                        assert_eq!(observed, task, "移動の途中経過を観測した");
                    }
                    other => panic!("常にどちらか一方の完全な内容が観測される: {other:?}"),
                }
                for entry in repo.list_active().expect("走査できる") {
                    assert_eq!(
                        entry,
                        TaskEntry::Record(TaskRecord::Intact(task.clone())),
                        "移動の途中経過を観測した"
                    );
                }
                for entry in repo.list_archived().expect("走査できる") {
                    assert_eq!(
                        entry,
                        TaskEntry::Record(TaskRecord::Intact(task.clone())),
                        "移動の途中経過を観測した"
                    );
                }
                observations.fetch_add(1, Ordering::Relaxed);
            }
        });

        repo.archive(task.id()).expect("アーカイブできる");
        yield_until_observed(&observations);
        moving.store(false, Ordering::Relaxed);
    });

    assert!(
        observations.load(Ordering::Relaxed) > 0,
        "読み手が一度も観測していない"
    );
    assert_eq!(
        repo.find(task.id()).expect("検索できる"),
        TaskLookup::Archived(TaskRecord::Intact(task))
    );
    assert_eq!(repo.list_active().expect("走査できる"), Vec::new());
    CaseOutcome::Ran
}

/// 正常・全体破損・スナップショット破損を混在させて走査する。
///
/// `Err` は前提条件を用意できなかったフックの名前。
fn mixed_area(
    harness: &impl TaskRepositoryHarness,
    area: Area,
) -> Result<Vec<TaskEntry>, &'static str> {
    let intact = task("task-a");
    let whole = task("task-b");
    let snapshot_only = task("task-c");
    for task in [&intact, &whole, &snapshot_only] {
        create(harness, task);
        if area == Area::Archived {
            archive(harness, task.id());
        }
    }
    if harness.corrupt_whole_record(area, whole.id()).is_none() {
        return Err("corrupt_whole_record");
    }
    if harness.corrupt_snapshot(area, snapshot_only.id()).is_none() {
        return Err("corrupt_snapshot");
    }

    let entries = listed(match area {
        Area::Active => harness.repo().list_active(),
        Area::Archived => harness.repo().list_archived(),
    });

    assert_eq!(
        entry_for(&entries, intact.id()),
        Some(TaskEntry::Record(TaskRecord::Intact(intact)))
    );
    assert!(
        matches!(
            entry_for(&entries, whole.id()),
            Some(TaskEntry::Corrupt { .. })
        ),
        "全体破損は Corrupt として列挙される: {entries:?}"
    );
    assert!(
        matches!(
            entry_for(&entries, snapshot_only.id()),
            Some(TaskEntry::Record(TaskRecord::SnapshotUnreadable(_)))
        ),
        "スナップショット破損は縮退として列挙される: {entries:?}"
    );
    Ok(entries)
}

fn create(harness: &impl TaskRepositoryHarness, task: &Task) {
    match harness.repo().create(task) {
        Ok(()) => {}
        Err(error) => panic!("作成できる: {error:?}"),
    }
}

fn archive(harness: &impl TaskRepositoryHarness, id: &TaskId) {
    match harness.repo().archive(id) {
        Ok(()) => {}
        Err(error) => panic!("アーカイブできる: {error:?}"),
    }
}

fn found(harness: &impl TaskRepositoryHarness, id: &TaskId) -> TaskLookup {
    harness.repo().find(id).expect("検索できる")
}

fn listed(result: Result<Vec<TaskEntry>, ReadError>) -> Vec<TaskEntry> {
    result.expect("走査できる")
}

fn active_intact(harness: &impl TaskRepositoryHarness, id: &TaskId) -> Task {
    match found(harness, id) {
        TaskLookup::Active(TaskRecord::Intact(task)) => task,
        other => panic!("現役の読めるタスクとして返る: {other:?}"),
    }
}

fn active_degraded(harness: &impl TaskRepositoryHarness, id: &TaskId) -> DegradedTask {
    match found(harness, id) {
        TaskLookup::Active(TaskRecord::SnapshotUnreadable(task)) => task,
        other => panic!("現役のスナップショット破損タスクとして返る: {other:?}"),
    }
}

fn entry_for(entries: &[TaskEntry], id: &TaskId) -> Option<TaskEntry> {
    entries
        .iter()
        .find(|entry| match entry {
            TaskEntry::Record(TaskRecord::Intact(task)) => task.id() == id,
            TaskEntry::Record(TaskRecord::SnapshotUnreadable(task)) => task.id() == id,
            TaskEntry::Corrupt { path, .. } => path.to_string_lossy().contains(id.as_str()),
        })
        .cloned()
}

fn assert_corrupt(lookup: TaskLookup, id: &TaskId) {
    match lookup {
        TaskLookup::Corrupt { path, message } => {
            assert!(
                path.to_string_lossy().contains(id.as_str()),
                "破損したファイルを指す: {}",
                path.display()
            );
            assert!(!message.is_empty(), "理由が空");
        }
        other => panic!("ファイル全体の破損として返る: {other:?}"),
    }
}

/// スナップショット以外のフィールドがすべて読めることを確かめる。
fn assert_task_side_fields(degraded: &DegradedTask, task: &Task) {
    assert_eq!(degraded.id(), task.id());
    assert_eq!(degraded.workflow_name(), task.workflow_name());
    assert_eq!(degraded.target(), task.target());
    assert_eq!(degraded.task_status(), task.task_status());
    assert_eq!(degraded.execution(), task.execution());
    assert_eq!(degraded.workspace(), task.workspace());
    assert_eq!(degraded.current_attempt(), task.current_attempt());
    assert_eq!(degraded.counters(), task.counters());
    assert_eq!(degraded.last_failure(), task.last_failure());
    assert_eq!(degraded.updated_at(), task.updated_at());
}

fn degraded_fields(task: &DegradedTask) -> DegradedTaskFields {
    DegradedTaskFields {
        id: task.id().clone(),
        workflow_name: task.workflow_name().clone(),
        target: task.target().clone(),
        task_status: task.task_status().clone(),
        execution: task.execution().clone(),
        workspace: task.workspace().cloned(),
        current_attempt: task.current_attempt().cloned(),
        counters: task.counters(),
        last_failure: task.last_failure().cloned(),
        updated_at: task.updated_at(),
        snapshot_error: task.snapshot_error().to_owned(),
    }
}

fn task(id: &str) -> Task {
    Task::rehydrate(fields(id)).expect("不変条件1を満たす")
}

/// 全ての任意フィールドが埋まったタスク。
fn full_task(id: &str) -> Task {
    Task::rehydrate(TaskFields {
        task_status: status(DONE),
        execution: ExecutionState::Stopped {
            reason: StopReason::RetryLimitExceeded,
            notified_at: Some(moment(180)),
        },
        workspace: Some(workspace(id)),
        current_attempt: Some(attempt(id)),
        counters: RetryCounters::rehydrate(3, 2, 1),
        last_failure: Some(
            FailureNote::parse(
                FailureKind::SpawnFail,
                "エージェントを起動できません".to_owned(),
                moment(120),
            )
            .expect("受理される"),
        ),
        updated_at: moment(180),
        ..fields(id)
    })
    .expect("不変条件1を満たす")
}

fn degraded(id: &str) -> DegradedTask {
    let fields = fields(id);
    DegradedTask::rehydrate(DegradedTaskFields {
        id: fields.id,
        workflow_name: fields.workflow_name,
        target: fields.target,
        task_status: fields.task_status,
        execution: fields.execution,
        workspace: fields.workspace,
        current_attempt: fields.current_attempt,
        counters: fields.counters,
        last_failure: fields.last_failure,
        updated_at: fields.updated_at,
        snapshot_error: "スナップショットを解釈できません".to_owned(),
    })
}

fn fields(id: &str) -> TaskFields {
    TaskFields {
        id: task_id(id),
        workflow_name: WorkflowName::parse("implement".to_owned()).expect("受理される"),
        target: Target::new(
            RepoPath::parse(absolute(&["repos", "pulsen"])).expect("受理される"),
            branch("main"),
        ),
        snapshot: snapshot(),
        task_status: status(QUEUED),
        execution: ExecutionState::Pending,
        workspace: None,
        current_attempt: None,
        counters: RetryCounters::initial(),
        last_failure: None,
        updated_at: moment(0),
    }
}

fn snapshot() -> WorkflowSnapshot {
    let definition = WorkflowDefinition::new(
        Some(AgentName::parse("shell".to_owned()).expect("受理される")),
        None,
        status(QUEUED),
        BTreeMap::from([
            (status(QUEUED), agent_run("実装してください", DONE)),
            (status(WAITING), StatusDefinition::Wait),
            (status(DONE), StatusDefinition::Cleanup),
        ]),
    )
    .expect("構造不変条件を満たす");
    WorkflowSnapshot::rehydrate(definition)
}

/// 内容が大きく異なるスナップショット。原子性の観測で新旧の混在を検出しやすくする。
fn wide_snapshot() -> WorkflowSnapshot {
    let mut statuses = BTreeMap::from([
        (
            status(QUEUED),
            agent_run(&"長いプロンプト".repeat(64), DONE),
        ),
        (status(WAITING), StatusDefinition::Wait),
        (status(DONE), StatusDefinition::Cleanup),
    ]);
    for index in 0..32 {
        statuses.insert(
            status(&format!("step-{index}")),
            agent_run(&format!("{index} 番目の作業"), DONE),
        );
    }

    let definition = WorkflowDefinition::new(None, None, status(QUEUED), statuses)
        .expect("構造不変条件を満たす");
    WorkflowSnapshot::rehydrate(definition)
}

fn agent_run(prompt: &str, next: &str) -> StatusDefinition {
    StatusDefinition::AgentRun {
        input: AgentInput::Prompt(Prompt::parse(prompt.to_owned()).expect("受理される")),
        agent: None,
        model: None,
        timeout: None,
        retries: None,
        judge: None,
        next: status(next),
    }
}

fn workspace(id: &str) -> Workspace {
    Workspace::new(
        WorktreePath::parse(absolute(&["worktrees", id])).expect("受理される"),
        branch(&format!("pulsen/{id}")),
    )
}

fn attempt(id: &str) -> AttemptRef {
    let number = AttemptNumber::parse(2).expect("受理される");
    let state_root = StateRoot::parse(absolute(&["state"])).expect("受理される");
    AttemptRef::rehydrate(
        number,
        RunDirPath::derive(&state_root, &task_id(id), number),
        Some(ProcessIdent::new(
            Pid::new(4242),
            KillIdent::parse("-4242".to_owned()).expect("受理される"),
            StartTimeRecord::new(
                ProcessStartTime::parse("871234".to_owned()).expect("受理される"),
                moment(60),
            ),
        )),
    )
}

fn task_id(id: &str) -> TaskId {
    TaskId::parse(id.to_owned()).expect("受理される")
}

fn status(name: &str) -> StatusName {
    StatusName::parse(name.to_owned()).expect("受理される")
}

fn branch(name: &str) -> BranchName {
    BranchName::parse(name.to_owned()).expect("受理される")
}

/// 基準時刻からの相対で時刻を作る。
fn moment(offset: i64) -> Timestamp {
    Timestamp::parse_rfc3339("2026-08-11T09:15:30Z")
        .map(|base| {
            Timestamp::from_unix_secs(base.unix_secs() + offset).expect("表現可能な範囲に収まる")
        })
        .expect("受理される")
}

/// プラットフォームで絶対になるパスを組み立てる。
fn absolute(segments: &[&str]) -> std::path::PathBuf {
    let mut path = if std::path::MAIN_SEPARATOR == '\\' {
        std::path::PathBuf::from("C:\\")
    } else {
        std::path::PathBuf::from("/")
    };
    for segment in segments {
        path.push(segment);
    }
    path
}

/// TaskRepository の適合スイートをアダプターに適用する。
///
/// `$setup` はケースごとに評価され、ハーネスは共有されない。`$allowed_skips` は
/// この環境で許容するスキップ件数で、超えたスキップはケースの失敗になる。
#[macro_export]
macro_rules! task_repository_conformance {
    ($setup:expr, $allowed_skips:expr) => {
        use $crate::task_repository as __pulsen_conformance_task_repository;

        $crate::conformance_cases!(
            __pulsen_conformance_task_repository,
            $setup,
            __PULSEN_CONFORMANCE_TASK_REPOSITORY_SKIPS = $allowed_skips,
            [
                tc_port_task_repository_001_作成でディレクトリごと用意され読み戻せる,
                tc_port_task_repository_002_現役に同じidがあれば作成は衝突する,
                tc_port_task_repository_003_アーカイブに同じidがあれば作成は衝突する,
                tc_port_task_repository_004_破損したファイルがあっても作成は衝突し内容を書き換えない,
                tc_port_task_repository_005_書き込み先を用意できなければ作成は入出力エラーになる,
                tc_port_task_repository_006_保存した内容は直後の検索で読み戻せる,
                tc_port_task_repository_007_作成していないidの保存は見つからない,
                tc_port_task_repository_008_アーカイブ済みタスクの保存は見つからない,
                tc_port_task_repository_009_縮退保存は壊れたスナップショットを温存する,
                tc_port_task_repository_010_現役にないidの縮退保存は見つからない,
                tc_port_task_repository_011_書き込めない状態の保存は入出力エラーになる,
                tc_port_task_repository_012_書き込めない状態の縮退保存は入出力エラーになる,
                tc_port_task_repository_013_全ての任意フィールドを持つタスクが往復する,
                tc_port_task_repository_014_実行状態は付随データごと復元される,
                tc_port_task_repository_015_何も作成していなければ検索は見つからない,
                tc_port_task_repository_016_作成済みタスクは現役として見つかる,
                tc_port_task_repository_017_アーカイブ済みタスクはアーカイブとして見つかる,
                tc_port_task_repository_018_双方に置かれたidは現役として返る,
                tc_port_task_repository_019_走査対象を読み取れない検索は入出力エラーになる,
                tc_port_task_repository_020_ファイル全体の破損は破損として返る,
                tc_port_task_repository_021_タスク側フィールドの破れは全体の破損として返る,
                tc_port_task_repository_022_スナップショットだけが読めないタスクは縮退として返る,
                tc_port_task_repository_023_スナップショットの欠落も縮退として返る,
                tc_port_task_repository_024_スナップショットにないタスクステータスは縮退として返る,
                tc_port_task_repository_025_スナップショットの構造不変条件の破れは縮退として返る,
                tc_port_task_repository_026_状態間整合の破れは復号で検証されない,
                tc_port_task_repository_027_アーカイブ側の全体破損も破損として返る,
                tc_port_task_repository_028_アーカイブ側のスナップショット破損は縮退として返る,
                tc_port_task_repository_029_現役の走査は検索と同じ区分で列挙する,
                tc_port_task_repository_030_命名形式に合致しないエントリは列挙されない,
                tc_port_task_repository_031_アーカイブ移動は移動先を用意して行われる,
                tc_port_task_repository_032_アーカイブ直後に現役から消えてアーカイブに現れる,
                tc_port_task_repository_033_作成していないidのアーカイブは見つからない,
                tc_port_task_repository_034_アーカイブ済みidの再アーカイブは見つからない,
                tc_port_task_repository_035_移動先を用意できないアーカイブは入出力エラーになる,
                tc_port_task_repository_036_走査対象ディレクトリが無ければ空の一覧になる,
                tc_port_task_repository_037_現役の走査はアーカイブしていないタスクだけを返す,
                tc_port_task_repository_038_アーカイブの走査はアーカイブしたタスクだけを返す,
                tc_port_task_repository_039_現役の個別の破損は走査全体を失敗させない,
                tc_port_task_repository_040_アーカイブの個別の破損は走査全体を失敗させない,
                tc_port_task_repository_041_走査対象を読み取れなければ入出力エラーになる,
                tc_port_task_repository_042_反復する保存の途中経過は読み手に観測されない,
                tc_port_task_repository_043_失敗した保存は部分的な結果を残さない,
                tc_port_task_repository_044_アーカイブ移動の中間状態は読み手に観測されない,
            ]
        );
    };
}
