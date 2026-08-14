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
    CommandRunner, ExclusiveLock, ExitCode, IdentityCheck, Io, JudgeConclusion, JudgeOutcome,
    JudgementService, KillError, ProcessController, RemnantOutcome, RunStore, RunningClassifier,
    RunningDecision, WorktreeManager,
};
use pulsen_domain::task::{
    Clock, ProcessIdent, RunDirPath, Task, TaskId, TaskRepository, Timestamp, TransitionError,
};

use super::{Freeze, Persisted, Tick, TickIssue, TickSummary};

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

        match exit {
            Some(exit) => self.judge(task, &run_dir, exit, summary),
            None => self.classify_alive(task, &process, summary),
        }
    }

    /// 終了した実行を判定し、結論を帳簿に反映する。
    fn judge(&self, task: Task, run_dir: &RunDirPath, exit: ExitCode, summary: &mut TickSummary) {
        let id = task.id().clone();
        let conclusion = match judge_command(&task) {
            // 判定コマンドを持たないステータスのデフォルト判定は2値であり、`Skipped` を
            // 導く経路が無い(ADR-008)。exit 20 も他の非0と同じ失敗になる。
            None => JudgeConclusion::Outcome(JudgementService::default_judgement(&exit)),
            Some(judge) => {
                let Some(workspace) = task.workspace() else {
                    // 不変条件4の破れ。判定コマンドへ渡す文脈が揃わないので書き込まない。
                    self.report_transition(id, TransitionError::WorkspaceNotSet, summary);
                    return;
                };
                let env = JudgementService::judge_env(&id, workspace.path(), &exit, run_dir);
                let timeout = self.config.judge_timeout();
                let completion = self.commands.run(&judge, &env, Some(&timeout));
                JudgementService::interpret_judge_completion(&completion)
            }
        };

        let now = self.clock.now();
        match conclusion {
            JudgeConclusion::Outcome(JudgeOutcome::Completed) => match task.complete_run(now) {
                Ok(completed) => match self.commit(&completed, Freeze::NotFrozen, summary) {
                    // 遷移は次の tick の `advance` に委ねる(1タスク1tick1ステップ)。
                    Persisted::Saved => summary.judged.push(id),
                    Persisted::Failed => {}
                },
                Err(error) => self.report_transition(id, error, summary),
            },
            JudgeConclusion::Outcome(JudgeOutcome::Skipped) => match task.skip_run(now) {
                Ok(skipped) => match self.commit(&skipped, Freeze::NotFrozen, summary) {
                    Persisted::Saved => summary.skipped_back.push(id),
                    Persisted::Failed => {}
                },
                Err(error) => self.report_transition(id, error, summary),
            },
            JudgeConclusion::Outcome(JudgeOutcome::Failed) => {
                self.record_run_failure(task, judgement_detail(&exit), now, summary);
            }
            JudgeConclusion::JudgeFailure { detail } => {
                self.record_judge_failure(task, detail, now, summary);
            }
        }
    }

    /// exit の無い実行の生存を観測し、分類する。
    fn classify_alive(&self, task: Task, process: &ProcessIdent, summary: &mut TickSummary) {
        let id = task.id().clone();
        let observed = match self.processes.starttime_of(process.pid()) {
            Ok(observed) => observed,
            // 機構の失敗は生死のどちらにも写像しない。次の tick が再観測する。
            Err(Io::Failed { message }) => {
                summary.errors.push(TickIssue::ObservationFailed {
                    task_id: id,
                    message,
                });
                return;
            }
        };

        let recorded = process.starttime();
        let aliveness = IdentityCheck::check(observed.as_ref(), recorded.ident());
        let timeout = effective_timeout(&task);
        let now = self.clock.now();
        let started = recorded.wall();

        match RunningClassifier::classify_alive(aliveness, &started, &timeout, &now) {
            // 書き込みを1回も起こさない(冪等性の実体)。
            RunningDecision::KeepRunning => {}
            RunningDecision::KillOnTimeout => {
                match self.processes.kill(process.kill_ident()) {
                    Ok(()) => {
                        let message = timeout_detail(&timeout);
                        self.record_run_failure(task, message, now, summary);
                    }
                    // 生存したままのプロセスを持つタスクを failed にすると、再起動が
                    // 同じ worktree で並走する。状態を変更せず次の tick に委ねる。
                    Err(KillError::Failed { message }) => {
                        summary.errors.push(TickIssue::KillFailed {
                            task_id: id,
                            message,
                        });
                    }
                }
            }
            RunningDecision::DiedWithoutExit => {
                let remnants = self.processes.try_kill_remnants(process.kill_ident());
                let message = "実行が終了コードを残さずに終わりました".to_owned();
                // 残存の終了はベストエフォートで、結果は分類に影響しない。報告は保存が
                // できたときだけ積む — 書けていない tick の報告は `SaveFailed` が正しい。
                if let Persisted::Saved = self.record_run_failure(task, message, now, summary) {
                    report_remnants(&id, remnants, summary);
                }
            }
            RunningDecision::Judge(_) => {
                unreachable!("生存の分類は判定を返さない(exit の有無は呼び出し側が分ける)")
            }
        }
    }

    /// 実行の失敗を確定して保存する。
    fn record_run_failure(
        &self,
        task: Task,
        message: String,
        now: Timestamp,
        summary: &mut TickSummary,
    ) -> Persisted {
        let id = task.id().clone();
        let limit = retry_limit(&task);
        match task.fail_run(limit, now) {
            Ok(failed) => {
                let persisted = self.commit(&failed, Freeze::of_recorded_failure(&failed), summary);
                if let Persisted::Saved = persisted {
                    summary.errors.push(TickIssue::RunFailed {
                        task_id: id,
                        message,
                    });
                }
                persisted
            }
            Err(error) => {
                self.report_transition(id, error, summary);
                Persisted::Failed
            }
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

/// 失敗と判断した根拠(終了コード)。
fn judgement_detail(exit: &ExitCode) -> String {
    format!("実行が終了コード {} で終了しました", exit.get())
}

/// timeout 超過の根拠。
fn timeout_detail(timeout: &TimeoutSpec) -> String {
    match timeout {
        TimeoutSpec::Limited(limit) => format!(
            "実行が timeout({}秒)を超えたため終了させました",
            limit.seconds()
        ),
        // 無制限では超過が成立しないため、この経路には到達しない。
        TimeoutSpec::Unlimited => "実行を終了させました".to_owned(),
    }
}

/// 残存プロセスの終了の結末を報告する。`Killed` は追加の報告を要さない。
fn report_remnants(task_id: &TaskId, outcome: RemnantOutcome, summary: &mut TickSummary) {
    let message = match outcome {
        RemnantOutcome::Killed => return,
        RemnantOutcome::NotIdentifiable => {
            "残存プロセスを誤殺なく同定できませんでした(終了操作は行っていません)".to_owned()
        }
        RemnantOutcome::Failed { message } => {
            format!("残存プロセスを終了できませんでした: {message}")
        }
    };
    summary.errors.push(TickIssue::RemnantsUnhandled {
        task_id: task_id.clone(),
        message,
    });
}
