//! タスク登録ユースケースの振る舞い。
//!
//! ポートはすべてテストダブルに差し替える(ADR-028)。実プロセス・実ファイルシステムを
//! 使わずに、実アダプターでは外から作れない状況(ID衝突・ロック機構の異常・git 操作の
//! 失敗・`create` の入出力エラー)まで含めて分岐を網羅する。

use std::collections::BTreeMap;
use std::path::PathBuf;

use pulsen::application::register_task::{
    RegisterTask, RegisterTaskError, RegisterTaskInput, RegisteredTask,
};
use pulsen_conformance::doubles::{
    FixedClock, LockOutcome, ScriptedExclusiveLock, ScriptedTaskIdGenerator,
    ScriptedTaskRepository, ScriptedWorkflowStore, ScriptedWorktreeManager, WorktreeManagerCall,
};
use pulsen_domain::definition::{
    AgentName, GlobalConfig, GlobalConfigInput, LoadedWorkflow, NameError, RawAgentDefinition,
    RawCommand, RawStatusDoc, RawWorkflowDoc, RegistrationError, StatusName, WorkflowAssembler,
    WorkflowLoadError, WorkflowName, WorkflowRef,
};
use pulsen_domain::execution::TargetError;
use pulsen_domain::task::{
    BranchName, BranchNameError, CreateError, ExecutionState, RepoPath, Task, TaskId, Timestamp,
};

/// 対象リポジトリの絶対パス。合成ルートが絶対化した後の値を模す。
fn repo_path() -> PathBuf {
    if std::path::MAIN_SEPARATOR == '\\' {
        PathBuf::from("C:\\repos\\pulsen")
    } else {
        PathBuf::from("/repos/pulsen")
    }
}

/// 合成ルートが絶対化した後の対象リポジトリ。
fn target_repo() -> RepoPath {
    RepoPath::parse(repo_path()).expect("受理される")
}

fn branch(name: &str) -> BranchName {
    BranchName::parse(name.to_owned()).expect("受理される")
}

fn workflow_name(name: &str) -> WorkflowName {
    WorkflowName::parse(name.to_owned()).expect("受理される")
}

fn agent_name(name: &str) -> AgentName {
    AgentName::parse(name.to_owned()).expect("受理される")
}

fn status_name(name: &str) -> StatusName {
    StatusName::parse(name.to_owned()).expect("受理される")
}

fn task_id(id: &str) -> TaskId {
    TaskId::parse(id.to_owned()).expect("受理される")
}

fn now() -> Timestamp {
    Timestamp::parse_rfc3339("2026-08-11T09:15:30Z").expect("受理される")
}

/// `claude` エージェントだけを定義したグローバル設定。
fn config() -> GlobalConfig {
    config_with_agents(vec![("claude", "claude {input}")])
}

fn config_with_agents(agents: Vec<(&str, &str)>) -> GlobalConfig {
    let agents = agents
        .into_iter()
        .map(|(name, cmd)| {
            (
                agent_name(name),
                RawAgentDefinition::new(RawCommand::parse_text(cmd).expect("受理される"), None),
            )
        })
        .collect::<BTreeMap<_, _>>();
    GlobalConfig::parse(GlobalConfigInput {
        agents: Some(agents),
        ..GlobalConfigInput::default()
    })
    .expect("受理される")
}

/// エージェント実行1件とクリーンアップ1件を持つ定義。
fn workflow_doc(declared_name: Option<&str>, agent: Option<&str>) -> RawWorkflowDoc {
    RawWorkflowDoc {
        declared_name: declared_name.map(str::to_owned),
        default_agent: agent.map(str::to_owned),
        default_model: None,
        initial: Some("queued".to_owned()),
        statuses: vec![
            (
                "queued".to_owned(),
                RawStatusDoc {
                    prompt: Some("実装して".to_owned()),
                    next: Some("done".to_owned()),
                    ..RawStatusDoc::default()
                },
            ),
            (
                "done".to_owned(),
                RawStatusDoc {
                    run: Some("cleanup".to_owned()),
                    ..RawStatusDoc::default()
                },
            ),
        ],
    }
}

/// 解決先のパスつきで読み込めたワークフローを組み立てる。
fn loaded(declared_name: Option<&str>, agent: Option<&str>, resolved_from: &str) -> LoadedWorkflow {
    let parsed =
        WorkflowAssembler::assemble(workflow_doc(declared_name, agent)).expect("受理される");
    LoadedWorkflow::new(parsed, PathBuf::from(resolved_from))
}

fn implement_workflow() -> LoadedWorkflow {
    loaded(
        None,
        Some("claude"),
        "/home/u/.pulsen/workflows/implement.yaml",
    )
}

/// ユースケースに渡すポート一式。
struct Doubles {
    config: GlobalConfig,
    workflows: ScriptedWorkflowStore,
    worktrees: ScriptedWorktreeManager,
    ids: ScriptedTaskIdGenerator,
    clock: FixedClock,
    tasks: ScriptedTaskRepository,
    lock: ScriptedExclusiveLock,
}

impl Doubles {
    /// ロックを取得でき、対象の検証も登録も成功する既定の台本。
    fn new() -> Self {
        Self {
            config: config(),
            workflows: ScriptedWorkflowStore::new([Ok(implement_workflow())]),
            worktrees: ScriptedWorktreeManager::new()
                .with_validate_repo([Ok(())])
                .with_head_branch([Ok(branch("main"))])
                .with_branch_exists([Ok(true)]),
            ids: ScriptedTaskIdGenerator::new([task_id("20260811t091530-k3f9qa1b")]),
            clock: FixedClock::new(now()),
            tasks: ScriptedTaskRepository::new([Ok(())]),
            lock: ScriptedExclusiveLock::new([LockOutcome::Acquired]),
        }
    }

    fn register(&self, input: RegisterTaskInput) -> Result<RegisteredTask, RegisterTaskError> {
        RegisterTask::new(
            &self.config,
            &self.workflows,
            &self.worktrees,
            &self.ids,
            &self.clock,
            &self.tasks,
            &self.lock,
        )
        .execute(input)
    }

    /// 作成されたタスク(`create` へ渡された最後の値)。
    fn created(&self) -> Vec<Task> {
        self.tasks.created()
    }
}

/// 名前指定・ベースブランチ明示の入力。
fn input(workflow: &str, base: Option<&str>) -> RegisterTaskInput {
    RegisterTaskInput {
        workflow: workflow.to_owned(),
        repo: repo_path(),
        base: base.map(str::to_owned),
    }
}

#[test]
fn 名前指定の登録ではその名前が表示名になり解決先が返る() {
    let doubles = Doubles::new();

    let registered = doubles
        .register(input("implement", Some("main")))
        .expect("登録できる");

    assert_eq!(registered.task_id, task_id("20260811t091530-k3f9qa1b"));
    assert_eq!(registered.workflow_name, workflow_name("implement"));
    assert_eq!(
        registered.resolved_from,
        PathBuf::from("/home/u/.pulsen/workflows/implement.yaml")
    );
    assert_eq!(
        doubles.workflows.requested(),
        vec![WorkflowRef::Name(workflow_name("implement"))]
    );
}

#[test]
fn 登録直後のタスクは初期ステータスの実行待ちになる() {
    let doubles = Doubles::new();

    doubles
        .register(input("implement", Some("main")))
        .expect("登録できる");

    let created = doubles.created();
    let task = created.first().expect("1件作成される");
    assert_eq!(task.task_status().as_str(), "queued");
    assert_eq!(task.execution(), &ExecutionState::Pending);
    assert_eq!(task.counters().attempt_count(), 0);
    assert_eq!(task.counters().judge_attempt_count(), 0);
    assert_eq!(task.counters().spawn_fail_count(), 0);
    assert_eq!(task.workspace(), None);
    assert_eq!(task.current_attempt(), None);
    assert_eq!(task.last_failure(), None);
    assert_eq!(task.updated_at(), now());
    assert_eq!(
        task.target().repo(),
        &RepoPath::parse(repo_path()).expect("受理される")
    );
    assert_eq!(task.target().base_branch(), &branch("main"));
}

#[test]
fn パス指定の登録では定義の宣言名が表示名になる() {
    let doubles = Doubles {
        workflows: ScriptedWorkflowStore::new([Ok(loaded(
            Some("my-flow"),
            Some("claude"),
            "/tmp/flows/custom.yaml",
        ))]),
        ..Doubles::new()
    };

    let registered = doubles
        .register(input("/tmp/flows/custom.yaml", Some("main")))
        .expect("登録できる");

    assert_eq!(registered.workflow_name, workflow_name("my-flow"));
}

#[test]
fn パス指定で宣言名がなければファイル名から表示名が決まる() {
    let doubles = Doubles {
        workflows: ScriptedWorkflowStore::new([Ok(loaded(
            None,
            Some("claude"),
            "/tmp/flows/custom.yaml",
        ))]),
        ..Doubles::new()
    };

    let registered = doubles
        .register(input("/tmp/flows/custom.yaml", Some("main")))
        .expect("登録できる");

    assert_eq!(registered.workflow_name, workflow_name("custom"));
}

#[test]
fn ベースブランチを省略するとheadのブランチが対象になる() {
    let doubles = Doubles::new();

    doubles
        .register(input("implement", None))
        .expect("登録できる");

    let repo = RepoPath::parse(repo_path()).expect("受理される");
    assert_eq!(
        doubles.worktrees.calls(),
        vec![
            WorktreeManagerCall::ValidateRepo { repo: repo.clone() },
            WorktreeManagerCall::HeadBranch { repo },
        ]
    );
    let created = doubles.created();
    assert_eq!(
        created
            .first()
            .expect("1件作成される")
            .target()
            .base_branch(),
        &branch("main")
    );
}

#[test]
fn ベースブランチを指定すると存在を確認して対象になる() {
    let doubles = Doubles {
        worktrees: ScriptedWorktreeManager::new()
            .with_validate_repo([Ok(())])
            .with_branch_exists([Ok(true)]),
        ..Doubles::new()
    };

    doubles
        .register(input("implement", Some("develop")))
        .expect("登録できる");

    let repo = RepoPath::parse(repo_path()).expect("受理される");
    assert_eq!(
        doubles.worktrees.calls(),
        vec![
            WorktreeManagerCall::ValidateRepo { repo: repo.clone() },
            WorktreeManagerCall::BranchExists {
                repo,
                branch: branch("develop"),
            },
        ]
    );
    let created = doubles.created();
    assert_eq!(
        created
            .first()
            .expect("1件作成される")
            .target()
            .base_branch(),
        &branch("develop")
    );
}

#[test]
fn 同じ指定を繰り返しても重複排除されず別idのタスクになる() {
    let first = Doubles::new();
    first
        .register(input("implement", Some("main")))
        .expect("登録できる");

    let second = Doubles {
        ids: ScriptedTaskIdGenerator::new([task_id("20260811t091531-zzzz0001")]),
        ..Doubles::new()
    };
    let registered = second
        .register(input("implement", Some("main")))
        .expect("登録できる");

    assert_eq!(registered.task_id, task_id("20260811t091531-zzzz0001"));
    assert_ne!(
        first.created().first().expect("1件作成される").id(),
        second.created().first().expect("1件作成される").id()
    );
}

#[test]
fn tc_task_register_task_012_id衝突は再発行して1回だけ再試行される() {
    let doubles = Doubles {
        ids: ScriptedTaskIdGenerator::new([
            task_id("20260811t091530-k3f9qa1b"),
            task_id("20260811t091530-p7x2m4c5"),
        ]),
        tasks: ScriptedTaskRepository::new([Err(CreateError::Conflict), Ok(())]),
        ..Doubles::new()
    };

    let registered = doubles
        .register(input("implement", Some("main")))
        .expect("登録できる");

    assert_eq!(registered.task_id, task_id("20260811t091530-p7x2m4c5"));
    assert_eq!(
        doubles.ids.issued(),
        vec![
            task_id("20260811t091530-k3f9qa1b"),
            task_id("20260811t091530-p7x2m4c5"),
        ]
    );
    assert_eq!(doubles.created().len(), 2);
}

#[test]
fn tc_task_register_task_047_再発行後も衝突したら実行環境のエラーになり三度目は試みない() {
    let doubles = Doubles {
        ids: ScriptedTaskIdGenerator::new([
            task_id("20260811t091530-k3f9qa1b"),
            task_id("20260811t091530-p7x2m4c5"),
        ]),
        tasks: ScriptedTaskRepository::new([
            Err(CreateError::Conflict),
            Err(CreateError::Conflict),
        ]),
        ..Doubles::new()
    };

    assert_eq!(
        doubles.register(input("implement", Some("main"))),
        Err(RegisterTaskError::Create(CreateError::Conflict))
    );
    assert_eq!(doubles.created().len(), 2);
}

#[test]
fn tc_task_register_task_048_タスクファイルの作成が入出力エラーなら実行環境のエラーになる() {
    let doubles = Doubles {
        tasks: ScriptedTaskRepository::new([Err(CreateError::Io {
            message: "書き込めない".to_owned(),
        })]),
        ..Doubles::new()
    };

    assert_eq!(
        doubles.register(input("implement", Some("main"))),
        Err(RegisterTaskError::Create(CreateError::Io {
            message: "書き込めない".to_owned()
        }))
    );
    assert_eq!(doubles.created().len(), 1);
}

#[test]
fn ロックを取得できなければ何も観測せずに終わる() {
    let doubles = Doubles {
        workflows: ScriptedWorkflowStore::new([]),
        worktrees: ScriptedWorktreeManager::new(),
        ids: ScriptedTaskIdGenerator::new([]),
        tasks: ScriptedTaskRepository::new([]),
        lock: ScriptedExclusiveLock::new([LockOutcome::Busy]),
        ..Doubles::new()
    };

    assert_eq!(
        doubles.register(input("implement", Some("main"))),
        Err(RegisterTaskError::LockBusy)
    );
    assert_eq!(doubles.workflows.requested(), vec![]);
    assert_eq!(doubles.created(), vec![]);
}

#[test]
fn tc_task_register_task_018_ロック機構自体の異常は実行環境のエラーになる() {
    let doubles = Doubles {
        workflows: ScriptedWorkflowStore::new([]),
        worktrees: ScriptedWorktreeManager::new(),
        ids: ScriptedTaskIdGenerator::new([]),
        tasks: ScriptedTaskRepository::new([]),
        lock: ScriptedExclusiveLock::new([LockOutcome::Failed {
            message: "ロックファイルを扱えない".to_owned(),
        }]),
        ..Doubles::new()
    };

    assert_eq!(
        doubles.register(input("implement", Some("main"))),
        Err(RegisterTaskError::LockFailed {
            message: "ロックファイルを扱えない".to_owned()
        })
    );
    assert_eq!(doubles.created(), vec![]);
}

#[test]
fn ワークフロー指定が空文字列なら入力境界で拒否される() {
    let doubles = Doubles {
        workflows: ScriptedWorkflowStore::new([]),
        worktrees: ScriptedWorktreeManager::new(),
        ids: ScriptedTaskIdGenerator::new([]),
        tasks: ScriptedTaskRepository::new([]),
        ..Doubles::new()
    };

    assert_eq!(
        doubles.register(input("", Some("main"))),
        Err(RegisterTaskError::InvalidWorkflowRef(NameError::Empty))
    );
    assert_eq!(doubles.created(), vec![]);
}

#[test]
fn ワークフロー定義が見つからなければ解決を試みたパスつきで拒否される() {
    let attempted = PathBuf::from("/home/u/.pulsen/workflows/missing.yaml");
    let doubles = Doubles {
        workflows: ScriptedWorkflowStore::new([Err(WorkflowLoadError::NotFound {
            attempted: attempted.clone(),
        })]),
        worktrees: ScriptedWorktreeManager::new(),
        ids: ScriptedTaskIdGenerator::new([]),
        tasks: ScriptedTaskRepository::new([]),
        ..Doubles::new()
    };

    assert_eq!(
        doubles.register(input("missing", Some("main"))),
        Err(RegisterTaskError::WorkflowLoad(
            WorkflowLoadError::NotFound { attempted }
        ))
    );
    assert_eq!(doubles.created(), vec![]);
}

#[test]
fn 表示名を決められないパス指定は拒否される() {
    let doubles = Doubles {
        workflows: ScriptedWorkflowStore::new([Ok(loaded(
            None,
            Some("claude"),
            "/tmp/flows/ impl.yaml",
        ))]),
        worktrees: ScriptedWorktreeManager::new(),
        ids: ScriptedTaskIdGenerator::new([]),
        tasks: ScriptedTaskRepository::new([]),
        ..Doubles::new()
    };

    assert_eq!(
        doubles.register(input("/tmp/flows/ impl.yaml", Some("main"))),
        Err(RegisterTaskError::UndecidableWorkflowName(
            NameError::SurroundingWhitespace
        ))
    );
    assert_eq!(doubles.created(), vec![]);
}

#[test]
fn ベースブランチが不正な形なら入力境界で拒否される() {
    let doubles = Doubles {
        ids: ScriptedTaskIdGenerator::new([]),
        tasks: ScriptedTaskRepository::new([]),
        ..Doubles::new()
    };

    assert_eq!(
        doubles.register(input("implement", Some(""))),
        Err(RegisterTaskError::InvalidBaseBranch(BranchNameError::Empty))
    );
    assert_eq!(doubles.created(), vec![]);
}

#[test]
fn リポジトリの検証が返した分類はそのまま返り登録は行われない() {
    for classification in [TargetError::NotFound, TargetError::NotARepository] {
        let doubles = Doubles {
            worktrees: ScriptedWorktreeManager::new()
                .with_validate_repo([Err(classification.clone())]),
            ids: ScriptedTaskIdGenerator::new([]),
            tasks: ScriptedTaskRepository::new([]),
            ..Doubles::new()
        };

        assert_eq!(
            doubles.register(input("implement", Some("main"))),
            Err(RegisterTaskError::Target(classification))
        );
        assert_eq!(
            doubles.worktrees.calls(),
            vec![WorktreeManagerCall::ValidateRepo {
                repo: target_repo()
            }]
        );
        assert_eq!(doubles.created(), vec![]);
    }
}

#[test]
fn headのブランチを解決できない分類はそのまま返り登録は行われない() {
    // HEAD 由来の分類を返すのは `head_branch`(ポート契約)。ベース省略時にだけ通る経路。
    for classification in [TargetError::DetachedHead, TargetError::EmptyRepository] {
        let doubles = Doubles {
            worktrees: ScriptedWorktreeManager::new()
                .with_validate_repo([Ok(())])
                .with_head_branch([Err(classification.clone())]),
            ids: ScriptedTaskIdGenerator::new([]),
            tasks: ScriptedTaskRepository::new([]),
            ..Doubles::new()
        };

        assert_eq!(
            doubles.register(input("implement", None)),
            Err(RegisterTaskError::Target(classification))
        );
        assert_eq!(
            doubles.worktrees.calls(),
            vec![
                WorktreeManagerCall::ValidateRepo {
                    repo: target_repo()
                },
                WorktreeManagerCall::HeadBranch {
                    repo: target_repo()
                },
            ]
        );
        assert_eq!(doubles.created(), vec![]);
    }
}

#[test]
fn tc_task_register_task_040_git操作自体の失敗は実行環境のエラーになる() {
    let failed = TargetError::Failed {
        message: "git を起動できない".to_owned(),
    };
    let base_branch = branch("develop");
    let failures = [
        (
            ScriptedWorktreeManager::new().with_validate_repo([Err(failed.clone())]),
            None,
            vec![WorktreeManagerCall::ValidateRepo {
                repo: target_repo(),
            }],
        ),
        (
            ScriptedWorktreeManager::new()
                .with_validate_repo([Ok(())])
                .with_head_branch([Err(failed.clone())]),
            None,
            vec![
                WorktreeManagerCall::ValidateRepo {
                    repo: target_repo(),
                },
                WorktreeManagerCall::HeadBranch {
                    repo: target_repo(),
                },
            ],
        ),
        (
            ScriptedWorktreeManager::new()
                .with_validate_repo([Ok(())])
                .with_branch_exists([Err(failed.clone())]),
            Some("develop"),
            vec![
                WorktreeManagerCall::ValidateRepo {
                    repo: target_repo(),
                },
                WorktreeManagerCall::BranchExists {
                    repo: target_repo(),
                    branch: base_branch.clone(),
                },
            ],
        ),
    ];

    for (worktrees, base, expected_calls) in failures {
        let doubles = Doubles {
            worktrees,
            ids: ScriptedTaskIdGenerator::new([]),
            tasks: ScriptedTaskRepository::new([]),
            ..Doubles::new()
        };

        assert_eq!(
            doubles.register(input("implement", base)),
            Err(RegisterTaskError::Target(failed.clone()))
        );
        assert_eq!(doubles.worktrees.calls(), expected_calls);
        assert_eq!(doubles.created(), vec![]);
    }
}

#[test]
fn 存在しないベースブランチは拒否される() {
    let doubles = Doubles {
        worktrees: ScriptedWorktreeManager::new()
            .with_validate_repo([Ok(())])
            .with_branch_exists([Ok(false)]),
        ids: ScriptedTaskIdGenerator::new([]),
        tasks: ScriptedTaskRepository::new([]),
        ..Doubles::new()
    };

    assert_eq!(
        doubles.register(input("implement", Some("develop"))),
        Err(RegisterTaskError::BaseBranchNotFound {
            branch: branch("develop")
        })
    );
    assert_eq!(doubles.created(), vec![]);
}

#[test]
fn 登録時検証のエラーは全件まとめて返り登録は行われない() {
    // `{model}` を要求し `skill_input` を持たないエージェントに、skill 指定の
    // ステータスと prompt 指定のステータスを当てて、複数ステータスにまたがる
    // 複数のエラーを起こす。
    let doubles = Doubles {
        config: config_with_agents(vec![("claude", "claude --model {model} {input}")]),
        workflows: ScriptedWorkflowStore::new([Ok(LoadedWorkflow::new(
            WorkflowAssembler::assemble(RawWorkflowDoc {
                default_agent: Some("claude".to_owned()),
                initial: Some("queued".to_owned()),
                statuses: vec![
                    (
                        "queued".to_owned(),
                        RawStatusDoc {
                            skill: Some("implement".to_owned()),
                            next: Some("reviewing".to_owned()),
                            ..RawStatusDoc::default()
                        },
                    ),
                    (
                        "reviewing".to_owned(),
                        RawStatusDoc {
                            prompt: Some("見直して".to_owned()),
                            next: Some("done".to_owned()),
                            ..RawStatusDoc::default()
                        },
                    ),
                    (
                        "done".to_owned(),
                        RawStatusDoc {
                            run: Some("cleanup".to_owned()),
                            ..RawStatusDoc::default()
                        },
                    ),
                ],
                ..RawWorkflowDoc::default()
            })
            .expect("受理される"),
            PathBuf::from("/home/u/.pulsen/workflows/implement.yaml"),
        ))]),
        ids: ScriptedTaskIdGenerator::new([]),
        tasks: ScriptedTaskRepository::new([]),
        ..Doubles::new()
    };

    assert_eq!(
        doubles.register(input("implement", Some("main"))),
        Err(RegisterTaskError::Registration(vec![
            RegistrationError::MissingSkillInput {
                status: status_name("queued"),
                agent: agent_name("claude"),
            },
            RegistrationError::MissingModel {
                status: status_name("queued"),
                agent: agent_name("claude"),
            },
            RegistrationError::MissingModel {
                status: status_name("reviewing"),
                agent: agent_name("claude"),
            },
        ]))
    );
    assert_eq!(doubles.created(), vec![]);
}
