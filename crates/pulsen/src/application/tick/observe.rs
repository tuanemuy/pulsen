//! 手続きD: 観測・判定(Running)(UC-execution-006)。
//!
//! 分類は2段で行う。exit ファイルがあれば実行は終了しており、生存観測は**行わない** —
//! 観測を先に置くと、`starttime_of` が失敗する環境で判定が永久に遅延する。
//!
//! 生存観測と終了操作の失敗では**状態を変更しない**。生きたままのプロセスを持つタスクを
//! failed にすると、次の tick が同じ worktree で再起動して並走に至る。次の tick が同じ
//! 決定を再導出するので、書かずに報告するだけで復旧は閉じる。

use pulsen_domain::definition::{PlainCommand, StatusDefinition, TimeoutSpec};
use pulsen_domain::execution::{
    AliveDecision, CommandCompletion, CommandRunner, DefaultJudgement, ExclusiveLock, ExitCode,
    IdentityCheck, Io, JudgeConclusion, JudgeOutcome, JudgementService, KillError,
    ProcessController, RunStore, RunningClassifier, RunningDecision, WorktreeManager,
};
use pulsen_domain::task::{Clock, ProcessIdent, RunDirPath, Task, TaskRepository, Timestamp};

use super::{Freeze, Persisted, RemnantsLeft, RunFailureCause, Tick, TickIssue, TickSummary};

impl<R, L, K, W, S, P, C> Tick<'_, R, L, K, W, S, P, C>
where
    R: TaskRepository,
    L: ExclusiveLock,
    K: Clock,
    W: WorktreeManager,
    S: RunStore,
    P: ProcessController,
    C: CommandRunner,
{
    /// exit を観測し、判定または生存分類へ進む。
    pub(super) fn observe(&self, task: Task, summary: &mut TickSummary) {
        let id = task.id().clone();
        let Some(attempt) = task.current_attempt() else {
            summary
                .errors
                .push(TickIssue::MissingCurrentAttempt { task_id: id });
            return;
        };
        let run_dir = attempt.run_dir().clone();
        let Some(process) = attempt.process().cloned() else {
            summary
                .errors
                .push(TickIssue::MissingProcessIdent { task_id: id });
            return;
        };

        let exit = match self.runs.read_exit(&run_dir) {
            Ok(exit) => exit,
            Err(error) => {
                summary
                    .errors
                    .push(TickIssue::RunFileUnreadable { task_id: id, error });
                return;
            }
        };

        // 1段目(exit の有無)はここで値にする。exit があれば生存を観測しない — 観測の
        // 一過性の失敗で判定を遅延させない(2段規則)。
        let decision = match exit {
            Some(exit) => RunningDecision::Judge(exit),
            None => match self.observe_aliveness(&task, &process, summary) {
                Some(alive) => alive.into(),
                // 観測の機構が失敗した。報告済みで、状態は変更しない。
                None => return,
            },
        };

        match decision {
            RunningDecision::Judge(exit) => self.judge(task, &run_dir, exit, summary),
            // 書き込みを1回も起こさない(冪等性の実体)。
            RunningDecision::KeepRunning => {}
            RunningDecision::KillOnTimeout => self.kill_on_timeout(task, &process, summary),
            RunningDecision::DiedWithoutExit => {
                self.fail_died_without_exit(task, &process, summary)
            }
        }
    }

    /// 終了した実行を判定し、結論を帳簿に反映する。
    fn judge(&self, task: Task, run_dir: &RunDirPath, exit: ExitCode, summary: &mut TickSummary) {
        let id = task.id().clone();
        let settled = match judge_command(&task) {
            None => Settled::by_default(exit),
            Some(judge) => {
                let Some(workspace) = task.workspace() else {
                    // 不変条件4の破れ。判定コマンドへ渡す文脈が揃わないので、起動も
                    // 書き込みも行わない。
                    summary
                        .errors
                        .push(TickIssue::MissingWorkspace { task_id: id });
                    return;
                };
                let env = JudgementService::judge_env(&id, workspace.path(), &exit, run_dir);
                let timeout = self.config.judge_timeout();
                Settled::by_command(&self.commands.run(&judge, &env, Some(&timeout)), exit)
            }
        };

        let now = self.clock.now();
        match settled {
            Settled::Completed => match task.complete_run(now) {
                Ok(completed) => match self.commit(&completed, Freeze::NotFrozen, summary) {
                    // 遷移は次の tick の `advance` に委ねる(1タスク1tick1ステップ)。
                    Persisted::Saved => summary.judged.push(id),
                    Persisted::Failed => {}
                },
                Err(error) => self.report_transition(id, error, summary),
            },
            Settled::Skipped => match task.skip_run(now) {
                Ok(skipped) => match self.commit(&skipped, Freeze::NotFrozen, summary) {
                    Persisted::Saved => summary.skipped_back.push(id),
                    Persisted::Failed => {}
                },
                Err(error) => self.report_transition(id, error, summary),
            },
            Settled::Failed(cause) => {
                self.record_run_failure(task, cause, now, summary);
            }
            Settled::JudgeBroken { detail } => {
                self.record_judge_failure(task, detail, now, summary);
            }
        }
    }

    /// exit の無い実行の生存を観測して分類する。
    ///
    /// 観測の機構が失敗したときは報告して `None` を返す — 機構の失敗は生死のどちらにも
    /// 写像しない(次の tick が再観測する)。
    fn observe_aliveness(
        &self,
        task: &Task,
        process: &ProcessIdent,
        summary: &mut TickSummary,
    ) -> Option<AliveDecision> {
        let observed = match self.processes.starttime_of(process.pid()) {
            Ok(observed) => observed,
            Err(Io::Failed { message }) => {
                summary.errors.push(TickIssue::ObservationFailed {
                    task_id: task.id().clone(),
                    message,
                });
                return None;
            }
        };

        let recorded = process.starttime();
        Some(RunningClassifier::classify_alive(
            IdentityCheck::check(observed.as_ref(), recorded.ident()),
            &recorded.wall(),
            &effective_timeout(task),
            &self.clock.now(),
        ))
    }

    /// timeout を超えた実行を終了させ、失敗として確定する。
    fn kill_on_timeout(&self, task: Task, process: &ProcessIdent, summary: &mut TickSummary) {
        match self.processes.kill(process.kill_ident()) {
            Ok(()) => {
                let cause = RunFailureCause::TimedOut {
                    timeout: effective_timeout(&task),
                };
                let now = self.clock.now();
                self.record_run_failure(task, cause, now, summary);
            }
            // 生存したままのプロセスを持つタスクを failed にすると、再起動が同じ worktree
            // で並走する。状態を変更せず次の tick に委ねる。
            Err(KillError::Failed { message }) => summary.errors.push(TickIssue::KillFailed {
                task_id: task.id().clone(),
                message,
            }),
        }
    }

    /// 残存の終了をベストエフォートで試みてから、失敗として確定する。
    fn fail_died_without_exit(
        &self,
        task: Task,
        process: &ProcessIdent,
        summary: &mut TickSummary,
    ) {
        let id = task.id().clone();
        let remnants = self.processes.try_kill_remnants(process.kill_ident());
        let now = self.clock.now();
        self.record_run_failure(task, RunFailureCause::DiedWithoutExit, now, summary);

        // 残存の終了はベストエフォートで、結果は分類に影響しない。プロセスが残っている
        // という事実はタスクファイルを書けたかと直交するので、保存に失敗した tick でも
        // 報告する — 後始末は人間が行う。
        if let Some(remnants) = RemnantsLeft::of(remnants) {
            summary.errors.push(TickIssue::RemnantsUnhandled {
                task_id: id,
                remnants,
            });
        }
    }

    /// 実行の失敗を確定して保存する。
    fn record_run_failure(
        &self,
        task: Task,
        cause: RunFailureCause,
        now: Timestamp,
        summary: &mut TickSummary,
    ) {
        let id = task.id().clone();
        let limit = retry_limit(&task);
        match task.fail_run(limit, now) {
            Ok(failed) => {
                if let Persisted::Saved =
                    self.commit(&failed, Freeze::of_recorded_failure(&failed), summary)
                {
                    summary
                        .errors
                        .push(TickIssue::RunFailed { task_id: id, cause });
                }
            }
            Err(error) => self.report_transition(id, error, summary),
        }
    }

    /// 判定自体の破れを記録する。エージェントは再実行しない(次の tick が再判定する)。
    fn record_judge_failure(
        &self,
        task: Task,
        detail: String,
        now: Timestamp,
        summary: &mut TickSummary,
    ) {
        let id = task.id().clone();
        match task.record_judge_failure(detail.clone(), self.config.judge_attempt_limit(), now) {
            Ok(recorded) => {
                if let Persisted::Saved =
                    self.commit(&recorded, Freeze::of_recorded_failure(&recorded), summary)
                {
                    summary.errors.push(TickIssue::JudgeFailed {
                        task_id: id,
                        detail,
                    });
                }
            }
            Err(error) => self.report_transition(id, error, summary),
        }
    }
}

/// 判定の結末。
///
/// 失敗のときだけ根拠を伴う — 「誰が失敗と判断したか」は結論と同時にしか分からず、
/// 後から結末だけを見て復元できない。
enum Settled {
    /// 成功。次のステータスへ進める。
    Completed,
    /// 見送り。タスクステータス不変のまま起動待ちへ戻す。
    Skipped,
    /// 失敗。リトライ上限を消費する。
    Failed(RunFailureCause),
    /// 判定自体が壊れた。判定のカウンタだけを消費する。
    JudgeBroken {
        /// 帳簿に残す原因の説明。
        detail: String,
    },
}

impl Settled {
    /// 判定コマンドを持たないステータスのデフォルト判定。
    ///
    /// 見送りは判定コマンドの exit 20 だけが生む。`DefaultJudgement` が2値である
    /// ことで、この経路から `Skipped` が導かれないことは型が述べる。
    fn by_default(exit: ExitCode) -> Self {
        match JudgementService::default_judgement(&exit) {
            DefaultJudgement::Completed => Self::Completed,
            DefaultJudgement::Failed => Self::Failed(RunFailureCause::DefaultJudgement { exit }),
        }
    }

    /// 判定コマンドの結末の解釈。
    fn by_command(completion: &CommandCompletion, exit: ExitCode) -> Self {
        match JudgementService::interpret_judge_completion(completion) {
            JudgeConclusion::Outcome(JudgeOutcome::Completed) => Self::Completed,
            JudgeConclusion::Outcome(JudgeOutcome::Skipped) => Self::Skipped,
            JudgeConclusion::Outcome(JudgeOutcome::Failed) => {
                Self::Failed(RunFailureCause::JudgeCommand { exit })
            }
            JudgeConclusion::JudgeFailure { detail } => Self::JudgeBroken { detail },
        }
    }
}

/// 現ステータスの判定コマンド。
fn judge_command(task: &Task) -> Option<PlainCommand> {
    match task.current_status_def() {
        StatusDefinition::AgentRun { judge, .. } => judge.clone(),
        StatusDefinition::Wait | StatusDefinition::Cleanup => None,
    }
}

/// 現ステータスに適用される timeout。
fn effective_timeout(task: &Task) -> TimeoutSpec {
    task.snapshot().effective_timeout(task.task_status())
}

/// 現ステータスに適用されるリトライ上限。
///
/// `applicable_retry_limit` ではなくスナップショットの実効値を直に引く — 動作種別が
/// 手動修復で崩れていても全域関数として値が決まり、判定の確定が報告だけで止まらない。
fn retry_limit(task: &Task) -> u32 {
    task.snapshot().effective_retry_limit(task.task_status())
}
