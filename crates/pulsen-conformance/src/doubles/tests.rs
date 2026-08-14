//! ダブルが「与えた結果を順に返し、呼び出しを記録する」ことの確認。

use std::collections::BTreeMap;
use std::path::PathBuf;

use pulsen_domain::definition::{
    AgentInput, AgentName, CommandLine, Prompt, StatusDefinition, StatusName, WorkflowDefinition,
    WorkflowLoadError, WorkflowName, WorkflowRef, WorkflowSnapshot, WorkflowStore,
};
use pulsen_domain::execution::{
    ExclusiveLock, ExitCode, Io, LockError, PidFileContent, ProcessController, RunStore,
    SpawnError, TargetError, WorktreeManager, WrapperIdentity, WrapperLaunchSpec,
};
use pulsen_domain::task::{
    AttemptNumber, BranchName, Clock, CreateError, KillIdent, Pid, ProcessStartTime, ReadError,
    RepoPath, RunDirPath, SaveError, StartTimeRecord, StateRoot, Target, Task, TaskId,
    TaskIdGenerator, TaskRepository, Timestamp, WorktreePath,
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
    let repository =
        ScriptedTaskRepository::new().with_create([Err(CreateError::Conflict), Ok(())]);
    let first = task("t1");
    let second = task("t2");

    assert_eq!(repository.create(&first), Err(CreateError::Conflict));
    assert_eq!(repository.create(&second), Ok(()));
    assert_eq!(repository.created(), vec![first, second]);
}

#[test]
fn リポジトリは走査の結果を順に返す() {
    let repository = ScriptedTaskRepository::new().with_list_active([
        Ok(Vec::new()),
        Err(ReadError::Io {
            message: "走査できない".to_owned(),
        }),
    ]);

    assert_eq!(repository.list_active(), Ok(Vec::new()));
    assert_eq!(
        repository.list_active(),
        Err(ReadError::Io {
            message: "走査できない".to_owned()
        })
    );
}

#[test]
fn リポジトリは保存の結果を順に返して渡されたタスクを記録する() {
    let repository = ScriptedTaskRepository::new().with_save([
        Ok(()),
        Err(SaveError::Io {
            message: "書き込めない".to_owned(),
        }),
    ]);
    let first = task("t1");
    let second = task("t2");

    assert_eq!(repository.save(&first), Ok(()));
    assert_eq!(
        repository.save(&second),
        Err(SaveError::Io {
            message: "書き込めない".to_owned()
        })
    );
    assert_eq!(repository.saved(), vec![first, second]);
}

#[test]
fn 可変クロックは置いた時刻を返し過去へも戻せる() {
    let clock = SettableClock::new(moment());
    let later = Timestamp::parse_rfc3339("2026-08-11T09:16:00Z").expect("受理される");

    assert_eq!(clock.now(), moment());
    clock.set(later);
    assert_eq!(clock.now(), later);
    clock.set(moment());
    assert_eq!(clock.now(), moment());
}

#[test]
fn runストアはメソッドごとの結果を返して呼び出しを記録する() {
    let store = ScriptedRunStore::new()
        .with_write_starttime([Ok(())])
        .with_write_pid_file([Err(Io::Failed {
            message: "書き込めない".to_owned(),
        })])
        .with_marker_exists([Ok(true)]);
    let run_dir = run_dir();

    assert_eq!(store.write_starttime(&run_dir, &starttime()), Ok(()));
    assert_eq!(
        store.write_pid_file(&run_dir, &pid_content()),
        Err(Io::Failed {
            message: "書き込めない".to_owned()
        })
    );
    assert_eq!(store.marker_exists(&run_dir), Ok(true));
    assert_eq!(
        store.calls(),
        vec![
            RunStoreCall::WriteStarttime {
                run_dir: run_dir.clone(),
                record: starttime(),
            },
            RunStoreCall::WritePidFile {
                run_dir: run_dir.clone(),
                content: pid_content(),
            },
            RunStoreCall::MarkerExists { run_dir },
        ]
    );
}

#[test]
fn プロセスコントローラーはメソッドごとの結果を返して呼び出しを記録する() {
    let controller = ScriptedProcessController::new()
        .with_own_identity([Ok(identity())])
        .with_run_agent([ExitCode::new(7)])
        .with_spawn_wrapper([Err(SpawnError::Failed {
            message: "起動できない".to_owned(),
        })]);
    let spec = WrapperLaunchSpec::new(run_dir(), agent_cmd(), workspace());

    assert_eq!(controller.own_identity(), Ok(identity()));
    assert_eq!(
        controller.run_agent(
            &agent_cmd(),
            &workspace(),
            &run_dir().stdout_log(),
            &run_dir().stderr_log(),
        ),
        ExitCode::new(7)
    );
    assert_eq!(
        controller.spawn_wrapper(&spec),
        Err(SpawnError::Failed {
            message: "起動できない".to_owned()
        })
    );
    assert_eq!(
        controller.calls(),
        vec![
            ProcessControllerCall::OwnIdentity,
            ProcessControllerCall::RunAgent {
                cmd: agent_cmd(),
                cwd: workspace(),
                stdout: run_dir().stdout_log(),
                stderr: run_dir().stderr_log(),
            },
            ProcessControllerCall::SpawnWrapper { spec },
        ]
    );
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

fn absolute(segments: &[&str]) -> PathBuf {
    let mut path = if std::path::MAIN_SEPARATOR == '\\' {
        PathBuf::from("C:\\")
    } else {
        PathBuf::from("/")
    };
    for segment in segments {
        path.push(segment);
    }
    path
}

fn repo() -> RepoPath {
    RepoPath::parse(absolute(&["repos", "pulsen"])).expect("受理される")
}

fn run_dir() -> RunDirPath {
    RunDirPath::derive(
        &StateRoot::parse(absolute(&["home", "u", ".pulsen", "state"])).expect("受理される"),
        &task_id("20260811t091530-k3f9qa1b"),
        AttemptNumber::parse(1).expect("受理される"),
    )
}

fn workspace() -> WorktreePath {
    WorktreePath::parse(absolute(&["home", "u", ".pulsen", "worktrees", "t1"])).expect("受理される")
}

fn agent_cmd() -> CommandLine {
    CommandLine::rehydrate(vec!["claude".to_owned(), "実装して".to_owned()]).expect("受理される")
}

fn starttime() -> StartTimeRecord {
    StartTimeRecord::new(
        ProcessStartTime::parse("Wed Aug 12 11:44:13 2026".to_owned()).expect("受理される"),
        moment(),
    )
}

fn pid_content() -> PidFileContent {
    PidFileContent::new(Pid::new(4242), kill_ident())
}

fn identity() -> WrapperIdentity {
    WrapperIdentity::new(Pid::new(4242), kill_ident(), starttime())
}

fn kill_ident() -> KillIdent {
    KillIdent::parse("-4242".to_owned()).expect("受理される")
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
