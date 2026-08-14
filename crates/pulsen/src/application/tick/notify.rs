//! 共通手続き: 凍結の確定と通知(UC-execution-001)。
//!
//! 順序は「stopped を書く(`notified_at: None`)→ 通知を実行する → 成功時だけ
//! `notified_at` を追記する」に固定する。逆順にすると、失敗した通知が永久に再送されない
//! (requirements §8 の at-least-once の破れ)。クラッシュ・通知失敗のどの時点でも
//! 「`notified_at` のない stopped」が残る限り、次の tick が同じ判断を再導出する。
//!
//! 通知の実行と `mark_notified` の保存を分けるのは、後者だけが対象の型で変わるため。
//! 通知に要る3値(タスクID・ワークフロー名・タスクステータス)はいずれもスナップショット
//! 非依存で、`Task` からも `DegradedTask` からも同じ形で取れる — これが「通知は定義
//! 非依存」の実体であり、破損したタスクの凍結も通知される根拠になる。

use pulsen_domain::definition::{StatusName, WorkflowName};
use pulsen_domain::execution::{
    CommandCompletion, CommandRunner, ExclusiveLock, NotificationService, ProcessController,
    RunStore, WorktreeManager,
};
use pulsen_domain::task::{Clock, DegradedTask, Task, TaskId, TaskRepository};

use super::{Freeze, Persisted, Tick, TickIssue, TickSummary};

/// 通知の結末。
enum Delivery {
    /// 通知が成功した。
    Sent,
    /// notify_cmd が未定義。通知も `notified_at` の記録も行わない。
    NotConfigured,
    /// 通知が失敗した。`notified_at` を書かず、次の tick が再通知する。
    Failed(String),
}

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
    /// 未通知の凍結を通知し、成功したら通知時刻を残す。
    pub(super) fn notify(&self, task: Task, summary: &mut TickSummary) {
        let id = task.id().clone();
        match self.deliver(&id, task.workflow_name(), task.task_status()) {
            // 「通知した」という虚偽の記録を作らない。後から notify_cmd が定義されれば、
            // 次の tick が未通知の stopped として検出して catch-up する。
            Delivery::NotConfigured => {}
            Delivery::Failed(message) => report_failure(id, message, summary),
            Delivery::Sent => match task.mark_notified(self.clock.now()) {
                // 凍結の計上は遷移を呼んだ側が行う(ADR-097)。ここで `Frozen` を通すと、
                // 過去の凍結が catch-up のたびに再計上される。
                Ok(notified) => match self.commit(&notified, Freeze::NotFrozen, summary) {
                    Persisted::Saved => summary.notified.push(id),
                    Persisted::Failed => {}
                },
                Err(error) => self.report_transition(id, error, summary),
            },
        }
    }

    /// スナップショットが読めないタスクの未通知の凍結を通知する。
    ///
    /// 保存だけが `save_degraded` になる — 読めないスナップショットを元の内容のまま
    /// 書き戻すことで、修復の材料を消さない。
    pub(super) fn notify_degraded(&self, task: DegradedTask, summary: &mut TickSummary) {
        let id = task.id().clone();
        match self.deliver(&id, task.workflow_name(), task.task_status()) {
            Delivery::NotConfigured => {}
            Delivery::Failed(message) => report_failure(id, message, summary),
            Delivery::Sent => match task.mark_notified(self.clock.now()) {
                Ok(notified) => match self.tasks.save_degraded(&notified) {
                    Ok(()) => summary.notified.push(id),
                    Err(error) => summary
                        .errors
                        .push(TickIssue::SaveFailed { task_id: id, error }),
                },
                Err(error) => self.report_transition(id, error, summary),
            },
        }
    }

    /// 通知コマンドを組み立てて実行する。
    ///
    /// timeout は組み込みの `NOTIFY_TIMEOUT` を必ず適用する — ハングした通知コマンドが
    /// 排他ロックを保持したまま tick を塞ぐことを防ぐ(ADR-018)。
    fn deliver(&self, id: &TaskId, workflow: &WorkflowName, status: &StatusName) -> Delivery {
        let Some(notify_cmd) = self.config.notify_cmd() else {
            return Delivery::NotConfigured;
        };
        let env = NotificationService::notify_env(id, workflow, status);

        match self
            .commands
            .run(notify_cmd, &env, Some(&NotificationService::NOTIFY_TIMEOUT))
        {
            CommandCompletion::Exited(code) if code.is_success() => Delivery::Sent,
            CommandCompletion::Exited(code) => Delivery::Failed(format!(
                "通知コマンドが終了コード {} で終了しました",
                code.get()
            )),
            CommandCompletion::TimedOut => Delivery::Failed(format!(
                "通知コマンドが {} 秒のうちに終了しませんでした",
                NotificationService::NOTIFY_TIMEOUT.seconds()
            )),
            CommandCompletion::FailedToStart { message } => {
                Delivery::Failed(format!("通知コマンドを起動できませんでした: {message}"))
            }
        }
    }
}

/// 通知の失敗を報告する。状態は変更していない(次の tick が再通知する)。
fn report_failure(task_id: TaskId, message: String, summary: &mut TickSummary) {
    summary
        .errors
        .push(TickIssue::NotifyFailed { task_id, message });
}
