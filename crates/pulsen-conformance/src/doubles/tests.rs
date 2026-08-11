//! ダブルが「与えた結果を順に返し、呼び出しを記録する」ことの確認。

use std::collections::BTreeMap;
use std::path::PathBuf;

use pulsen_domain::definition::{
    AgentInput, AgentName, Prompt, StatusDefinition, StatusName, WorkflowDefinition,
    WorkflowLoadError, WorkflowName, WorkflowRef, WorkflowSnapshot, WorkflowStore,
};
use pulsen_domain::execution::{ExclusiveLock, LockError, TargetError, WorktreeManager};
use pulsen_domain::task::{
    BranchName, Clock, CreateError, RepoPath, Target, Task, TaskId, TaskIdGenerator,
    TaskRepository, Timestamp,
};

use super::*;

#[test]
fn 固定クロックはいつ呼んでも同じ時刻を返す() {
    let clock = FixedClock::new(moment());

    assert_eq!(clock.now(), moment());
    assert_eq!(clock.now(), moment());
}

#[test]
fn idジェネレーターは与えた順にidを発行して記録する() {
    let generator = ScriptedTaskIdGenerator::new([task_id("t1"), task_id("t2")]);

    assert_eq!(generator.generate(), task_id("t1"));
    assert_eq!(generator.generate(), task_id("t2"));
    assert_eq!(generator.issued(), vec![task_id("t1"), task_id("t2")]);
}

#[test]
fn ロックは与えた結果を順に返す() {
    let lock = ScriptedExclusiveLock::new([
        LockOutcome::Acquired,
        LockOutcome::Busy,
        LockOutcome::Failed {
            message: "置き場が無い".to_owned(),
        },
    ]);

    assert!(matches!(lock.try_acquire(), Ok(Some(_))));
    assert!(matches!(lock.try_acquire(), Ok(None)));
    assert_eq!(
        lock.try_acquire().err(),
        Some(LockError::Failed {
            message: "置き場が無い".to_owned()
        })
    );
    assert_eq!(lock.attempts(), 3);
}

#[test]
fn ワークツリーマネージャーはメソッドごとの結果を返して呼び出しを記録する() {
    let manager = ScriptedWorktreeManager::new()
        .with_validate_repo([Ok(())])
        .with_head_branch([Err(TargetError::DetachedHead)])
        .with_branch_exists([Ok(false)]);
    let repo = repo();

    assert_eq!(manager.validate_repo(&repo), Ok(()));
    assert_eq!(manager.head_branch(&repo), Err(TargetError::DetachedHead));
    assert_eq!(manager.branch_exists(&repo, &branch()), Ok(false));
    assert_eq!(
        manager.calls(),
        vec![
            WorktreeManagerCall::ValidateRepo { repo: repo.clone() },
            WorktreeManagerCall::HeadBranch { repo: repo.clone() },
            WorktreeManagerCall::BranchExists {
                repo,
                branch: branch()
            },
        ]
    );
}

#[test]
fn リポジトリは作成の結果を順に返して渡されたタスクを記録する() {
    let repository = ScriptedTaskRepository::new([Err(CreateError::Conflict), Ok(())]);
    let first = task("t1");
    let second = task("t2");

    assert_eq!(repository.create(&first), Err(CreateError::Conflict));
    assert_eq!(repository.create(&second), Ok(()));
    assert_eq!(repository.created(), vec![first, second]);
}

#[test]
fn ワークフローストアは読み込みの結果を返して参照を記録する() {
    let attempted = PathBuf::from("workflows/implement.yaml");
    let store = ScriptedWorkflowStore::new([Err(WorkflowLoadError::NotFound {
        attempted: attempted.clone(),
    })]);
    let wf_ref = WorkflowRef::Name(workflow_name());

    assert_eq!(
        store.load(&wf_ref),
        Err(WorkflowLoadError::NotFound { attempted })
    );
    assert_eq!(store.requested(), vec![wf_ref]);
}

fn moment() -> Timestamp {
    Timestamp::parse_rfc3339("2026-08-11T09:15:30Z").expect("受理される")
}

fn task_id(id: &str) -> TaskId {
    TaskId::parse(id.to_owned()).expect("受理される")
}

fn workflow_name() -> WorkflowName {
    WorkflowName::parse("implement".to_owned()).expect("受理される")
}

fn branch() -> BranchName {
    BranchName::parse("main".to_owned()).expect("受理される")
}

fn repo() -> RepoPath {
    let path = if std::path::MAIN_SEPARATOR == '\\' {
        PathBuf::from("C:\\repos\\pulsen")
    } else {
        PathBuf::from("/repos/pulsen")
    };
    RepoPath::parse(path).expect("受理される")
}

fn task(id: &str) -> Task {
    Task::register(
        task_id(id),
        workflow_name(),
        Target::new(repo(), branch()),
        snapshot(),
        moment(),
    )
}

fn snapshot() -> WorkflowSnapshot {
    let queued = StatusName::parse("queued".to_owned()).expect("受理される");
    let done = StatusName::parse("done".to_owned()).expect("受理される");
    let definition = WorkflowDefinition::new(
        Some(AgentName::parse("shell".to_owned()).expect("受理される")),
        None,
        queued.clone(),
        BTreeMap::from([
            (
                queued,
                StatusDefinition::AgentRun {
                    input: AgentInput::Prompt(
                        Prompt::parse("実装してください".to_owned()).expect("受理される"),
                    ),
                    agent: None,
                    model: None,
                    timeout: None,
                    retries: None,
                    judge: None,
                    next: done.clone(),
                },
            ),
            (done, StatusDefinition::Cleanup),
        ]),
    )
    .expect("構造不変条件を満たす");
    WorkflowSnapshot::rehydrate(definition)
}
