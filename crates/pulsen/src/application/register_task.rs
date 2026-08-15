//! タスク登録のユースケース(UC-task-001)。
//!
//! ポート越しに観測し(ワークフロー定義・対象リポジトリ)、ドメインに判断させ
//! (`WorkflowRef` / `RegistrationValidator` / `Task::register`)、ポート越しに実行する
//! (`TaskRepository::create`)。文言の組み立ては行わず、原因を値として返す。

use std::path::PathBuf;

use pulsen_domain::definition::{
    GlobalConfig, NameError, RegistrationError, RegistrationValidator, WorkflowLoadError,
    WorkflowName, WorkflowRef, WorkflowStore,
};
use pulsen_domain::execution::{ExclusiveLock, LockError, TargetError, WorktreeManager};
use pulsen_domain::task::{
    AbsolutePathError, BranchName, BranchNameError, Clock, CreateError, RepoPath, Target, Task,
    TaskId, TaskIdGenerator, TaskRepository,
};

/// 登録の入力。
///
/// `repo` は絶対化済みであることを前提とする — カレントディレクトリの読み取りは
/// 合成ルートの責務。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterTaskInput {
    /// `--workflow` の値(名前またはパス)。
    pub workflow: String,
    /// `--repo` の値(絶対化済み)。
    pub repo: PathBuf,
    /// `--base` の値。省略時は HEAD から解決する。
    pub base: Option<String>,
}

/// 登録の結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredTask {
    /// 発行されたタスクID。
    pub task_id: TaskId,
    /// タスクに記録された表示名。
    pub workflow_name: WorkflowName,
    /// 実際に読み込んだワークフロー定義のパス。
    pub resolved_from: PathBuf,
}

/// 登録の失敗。
///
/// いずれの場合もタスクは作られない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegisterTaskError {
    /// 別の操作が排他ロックを保持している。
    LockBusy,
    /// ロック機構自体の異常。
    LockFailed {
        /// 原因の説明。
        message: String,
    },
    /// `--workflow` の値をワークフロー参照として解釈できない。
    InvalidWorkflowRef(NameError),
    /// ワークフロー定義を読み込めない。
    WorkflowLoad(WorkflowLoadError),
    /// 表示名を決められない。
    UndecidableWorkflowName(NameError),
    /// `--repo` が絶対パスではない。
    InvalidRepoPath(AbsolutePathError),
    /// `--base` をブランチ名として解釈できない。
    InvalidBaseBranch(BranchNameError),
    /// 対象の検証に失敗した。
    Target(TargetError),
    /// 指定したベースブランチがリポジトリに存在しない。
    BaseBranchNotFound {
        /// 指定されたブランチ。
        branch: BranchName,
    },
    /// 登録時検証に失敗した(全件)。
    Registration(Vec<RegistrationError>),
    /// タスクファイルを作成できない。
    Create(CreateError),
}

/// ワークフローと対象を検証し、スナップショットを埋め込んだタスクを登録する。
///
/// ポートはジェネリック引数で受け取り、実アダプターとテストダブルのどちらにも同じ
/// 制御フローが乗ることを型で示す。
pub struct RegisterTask<'a, S, M, G, K, R, L> {
    config: &'a GlobalConfig,
    workflows: &'a S,
    worktrees: &'a M,
    ids: &'a G,
    clock: &'a K,
    tasks: &'a R,
    lock: &'a L,
}

impl<'a, S, M, G, K, R, L> RegisterTask<'a, S, M, G, K, R, L>
where
    S: WorkflowStore,
    M: WorktreeManager,
    G: TaskIdGenerator,
    K: Clock,
    R: TaskRepository,
    L: ExclusiveLock,
{
    /// 読み込み済みのグローバル設定と各ポートを結線する。
    ///
    /// グローバル設定を `ConfigStore` ではなく値で受け取るのは、設定の読み込みが
    /// 全コマンド共通の起動処理(pages ※1)であり、このユースケースの一部ではないため。
    pub fn new(
        config: &'a GlobalConfig,
        workflows: &'a S,
        worktrees: &'a M,
        ids: &'a G,
        clock: &'a K,
        tasks: &'a R,
        lock: &'a L,
    ) -> Self {
        Self {
            config,
            workflows,
            worktrees,
            ids,
            clock,
            tasks,
            lock,
        }
    }

    /// spec の処理フローの順に実行する。
    pub fn execute(&self, input: RegisterTaskInput) -> Result<RegisteredTask, RegisterTaskError> {
        let _guard = match self.lock.try_acquire() {
            Ok(Some(guard)) => guard,
            Ok(None) => return Err(RegisterTaskError::LockBusy),
            Err(LockError::Failed { message }) => {
                return Err(RegisterTaskError::LockFailed { message });
            }
        };

        let reference =
            WorkflowRef::parse(&input.workflow).map_err(RegisterTaskError::InvalidWorkflowRef)?;
        let loaded = self
            .workflows
            .load(&reference)
            .map_err(RegisterTaskError::WorkflowLoad)?;
        let (parsed, resolved_from) = loaded.into_parts();
        let (declared_name, definition) = parsed.into_parts();

        let workflow_name = reference
            .display_name(declared_name.as_ref())
            .map_err(RegisterTaskError::UndecidableWorkflowName)?;

        let target = self.resolve_target(input.repo, input.base)?;

        let snapshot = RegistrationValidator::validate(definition, self.config)
            .map_err(RegisterTaskError::Registration)?;

        let now = self.clock.now();
        let mut retried = false;
        loop {
            let task = Task::register(
                self.ids.generate(),
                workflow_name.clone(),
                target.clone(),
                snapshot.clone(),
                now,
            );
            match self.tasks.create(&task) {
                Ok(()) => {
                    return Ok(RegisteredTask {
                        task_id: task.id().clone(),
                        workflow_name,
                        resolved_from,
                    });
                }
                // 衝突は1回だけ再発行して再試行する。再衝突は実行環境のエラーとして返す。
                Err(CreateError::Conflict) => {
                    if retried {
                        return Err(RegisterTaskError::Create(CreateError::Conflict));
                    }
                    retried = true;
                }
                Err(error @ CreateError::Io { .. }) => {
                    return Err(RegisterTaskError::Create(error));
                }
            }
        }
    }

    /// リポジトリとベースブランチを検証して対象を決める。
    fn resolve_target(
        &self,
        repo: PathBuf,
        base: Option<String>,
    ) -> Result<Target, RegisterTaskError> {
        let repo = RepoPath::parse(repo).map_err(RegisterTaskError::InvalidRepoPath)?;
        self.worktrees
            .validate_repo(&repo)
            .map_err(RegisterTaskError::Target)?;

        let base_branch = match base {
            Some(base) => {
                let branch =
                    BranchName::parse(base).map_err(RegisterTaskError::InvalidBaseBranch)?;
                let exists = self
                    .worktrees
                    .branch_exists(&repo, &branch)
                    .map_err(RegisterTaskError::Target)?;
                if !exists {
                    return Err(RegisterTaskError::BaseBranchNotFound { branch });
                }
                branch
            }
            None => self
                .worktrees
                .head_branch(&repo)
                .map_err(RegisterTaskError::Target)?,
        };

        Ok(Target::new(repo, base_branch))
    }
}
