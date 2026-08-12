//! 手続きA: 起動(Pending / Failed × AgentRun)。
//!
//! 順序は「worktree確保 → テンプレート展開 → launching記録 → `prepare_attempt` →
//! spawn」に固定する(ADR-016)。launching 記録が復旧の起点であり、そこから後の失敗
//! (`prepare_attempt` の失敗・`spawn_wrapper` の同期エラー)では**状態を変更しない** —
//! pending へ戻すと、遅れて起動したラッパーと次の attempt が同じ worktree で並走しうる。

use pulsen_domain::definition::{AgentInput, CommandLine};
use pulsen_domain::execution::{
    ExclusiveLock, Io, ProcessController, RunStore, SpawnError, WorktreeError, WorktreeManager,
    WrapperLaunchSpec,
};
use pulsen_domain::task::{
    Clock, FailureKind, Task, TaskRepository, WorkspacePlanner, WorktreePath,
};

use super::{Persisted, Tick, TickIssue, TickSummary};

impl<R, L, K, W, S, P> Tick<'_, R, L, K, W, S, P>
where
    R: TaskRepository,
    L: ExclusiveLock,
    K: Clock,
    W: WorktreeManager,
    S: RunStore,
    P: ProcessController,
{
    /// worktree を確保し、コマンドを展開して、ラッパーをデタッチ起動する。
    pub(super) fn launch(&self, task: Task, input: AgentInput, summary: &mut TickSummary) {
        let Some(task) = self.ensure_workspace(task, summary) else {
            return;
        };

        let workspace = workspace_path(&task);
        let agent_cmd = match self.expand(&task, &input, &workspace) {
            Ok(agent_cmd) => agent_cmd,
            Err(message) => return self.record_expansion_failure(task, message, summary),
        };

        let id = task.id().clone();
        let now = self.clock.now();
        let (recorded, run_dir) = match task.record_launching(self.state_root, now) {
            Ok(recorded) => recorded,
            Err(error) => return self.report_transition(id, error, summary),
        };
        // 採番はドメインの遷移関数だけが行う。番号と run ディレクトリを別々に導出すると、
        // 採番規則が変わったときに帳簿と実体が食い違う。
        let number = recorded
            .current_attempt()
            .expect("起動記録は現在 attempt を置く")
            .number();
        match self.commit(&recorded, summary) {
            Persisted::Saved => {}
            Persisted::Failed => return,
        }

        // `prepare_attempt` の失敗でも spawn は続ける。ラッパーの write 系は書き込み先の
        // ディレクトリを自分で作るので、ここで止めるとむしろ自己修復の余地を捨てる。
        match self.runs.prepare_attempt(&id, number) {
            Ok(_) => {}
            Err(Io::Failed { message }) => summary.errors.push(TickIssue::PrepareAttemptFailed {
                task_id: id.clone(),
                message,
            }),
        }

        let spec = WrapperLaunchSpec::new(run_dir, agent_cmd, workspace);
        match self.processes.spawn_wrapper(&spec) {
            Ok(()) => summary.launched.push(id),
            Err(SpawnError::Failed { message }) => summary.errors.push(TickIssue::SpawnFailed {
                task_id: id,
                message,
            }),
        }
    }

    /// ワークスペースを確保する。確定済みなら worktree 操作を行わない。
    ///
    /// 確保できなかった場合は失敗を記録済みで `None` を返す。
    fn ensure_workspace(&self, task: Task, summary: &mut TickSummary) -> Option<Task> {
        if task.workspace().is_some() {
            return Some(task);
        }

        let id = task.id().clone();
        let workspace = WorkspacePlanner::derive(self.worktree_root, &id);
        let created = self.worktrees.create(
            task.target().repo(),
            task.target().base_branch(),
            &workspace,
        );

        match created {
            Ok(()) => match task.confirm_workspace(workspace, self.clock.now()) {
                Ok(confirmed) => match self.commit(&confirmed, summary) {
                    Persisted::Saved => Some(confirmed),
                    Persisted::Failed => None,
                },
                Err(error) => {
                    self.report_transition(id, error, summary);
                    None
                }
            },
            Err(WorktreeError::Failed { message }) => {
                let limit = retry_limit(&task);
                let now = self.clock.now();
                match task.record_tool_failure(
                    FailureKind::WorktreeCreate,
                    message.clone(),
                    limit,
                    now,
                ) {
                    Ok(failed) => match self.commit(&failed, summary) {
                        Persisted::Saved => summary.errors.push(TickIssue::WorktreeCreateFailed {
                            task_id: id,
                            message,
                        }),
                        Persisted::Failed => {}
                    },
                    Err(error) => self.report_transition(id, error, summary),
                }
                None
            }
        }
    }

    /// エージェント定義を引き、入力とコマンドラインを展開する。
    ///
    /// 展開は起動のたびに行う — グローバル設定はスナップショットされないので、定義の
    /// 修正は次の tick で反映される。失敗はタスクファイルに残す説明として返す。
    fn expand(
        &self,
        task: &Task,
        input: &AgentInput,
        workspace: &WorktreePath,
    ) -> Result<CommandLine, String> {
        let status = task.task_status();
        let snapshot = task.snapshot();

        let agent = snapshot.effective_agent(status).ok_or_else(|| {
            format!(
                "ステータス `{}` の実効エージェント名を解決できません",
                status.as_str()
            )
        })?;
        let raw = self.config.agent(agent).ok_or_else(|| {
            format!(
                "エージェント `{}` は config.yaml に定義されていません",
                agent.as_str()
            )
        })?;
        let definition = raw.parse().map_err(|error| {
            format!(
                "エージェント `{}` の定義が不正です: {}",
                agent.as_str(),
                error.describe()
            )
        })?;
        let rendered = definition.render_input(input).map_err(|error| {
            format!(
                "エージェント `{}` に渡す入力を組み立てられません: {}",
                agent.as_str(),
                error.describe()
            )
        })?;
        definition
            .build_command_line(
                &rendered,
                snapshot.effective_model(status),
                workspace.as_path(),
            )
            .map_err(|error| {
                format!(
                    "エージェント `{}` のコマンドを展開できません: {}",
                    agent.as_str(),
                    error.describe()
                )
            })
    }

    /// 展開失敗を同期的な spawn 失敗として記録する。
    ///
    /// attempt を採番せず、上限を超えなければ実行状態も変えない — 起動を試みる前の
    /// 失敗であり、attempt は消費されていない。
    fn record_expansion_failure(&self, task: Task, message: String, summary: &mut TickSummary) {
        let id = task.id().clone();
        let now = self.clock.now();
        match task.record_spawn_failure_in_place(
            message.clone(),
            self.config.spawn_fail_limit(),
            now,
        ) {
            Ok(recorded) => match self.commit(&recorded, summary) {
                Persisted::Saved => summary.errors.push(TickIssue::CommandExpansionFailed {
                    task_id: id,
                    message,
                }),
                Persisted::Failed => {}
            },
            Err(error) => self.report_transition(id, error, summary),
        }
    }
}

/// 確定済みワークスペースの worktree パス。
///
/// この手続きに入るのは確保を終えたタスクだけなので、未確定は不変条件の破れになる。
fn workspace_path(task: &Task) -> WorktreePath {
    task.workspace()
        .expect("ワークスペースの確保を終えたタスクだけが展開へ進む")
        .path()
        .clone()
}

/// ツール操作の失敗に適用されるリトライ上限。
///
/// 分岐はエージェント実行ステータスのタスクだけをここへ導くので、適用対象は常にある。
fn retry_limit(task: &Task) -> u32 {
    task.applicable_retry_limit()
        .expect("エージェント実行ステータスにはリトライ上限が適用される")
}
