//! 手続きC: spawn 確認(Launching)。
//!
//! 順序は「read → classify → マーカー書き込み → 再読 → classify_recheck」に固定する。
//! マーカーを書いてから pid を再確認するのが、ラッパー側の「pid を書いてからマーカーを
//! 確認する」と対になる — どちらの観測が先でも、遅れて起動したラッパーと次の attempt が
//! 並走しない。マーカーを書けなかったときに pending へ戻さないのも同じ理由による。

use pulsen_domain::execution::{
    ExclusiveLock, InconsistentRunFiles, Io, LaunchingClassifier, LaunchingDecision,
    LaunchingRecheck, PidFileContent, ProcessController, RunStore, WorktreeManager,
};
use pulsen_domain::task::{
    Clock, ProcessIdent, RunDirPath, StartTimeRecord, Task, TaskId, TaskRepository, Timestamp,
};

use super::{Freeze, Persisted, Tick, TickIssue, TickSummary};

/// 1回の観測で読んだ run ファイル。
struct RunFiles {
    pid: Option<PidFileContent>,
    starttime: Option<StartTimeRecord>,
}

impl<R, L, K, W, S, P> Tick<'_, R, L, K, W, S, P>
where
    R: TaskRepository,
    L: ExclusiveLock,
    K: Clock,
    W: WorktreeManager,
    S: RunStore,
    P: ProcessController,
{
    /// run ファイルを観測し、起動確認か spawn 失敗かを確定させる。
    pub(super) fn confirm_spawn(
        &self,
        task: Task,
        recorded_at: Timestamp,
        summary: &mut TickSummary,
    ) {
        let id = task.id().clone();
        let Some(attempt) = task.current_attempt() else {
            summary
                .errors
                .push(TickIssue::MissingCurrentAttempt { task_id: id });
            return;
        };
        let run_dir = attempt.run_dir().clone();

        let Some(observed) = self.read_run_files(&id, &run_dir, summary) else {
            return;
        };
        let now = self.clock.now();
        let decision =
            LaunchingClassifier::classify(&recorded_at, &now, observed.pid, observed.starttime);

        match decision {
            Ok(LaunchingDecision::ConfirmRunning(ident)) => {
                self.confirm_running(task, ident, now, summary);
            }
            // ラッパーの書き込み待ち。書き込みを一切発生させない。
            Ok(LaunchingDecision::KeepWaiting) => {}
            Ok(LaunchingDecision::SuspectSpawnFailure) => {
                self.invalidate_and_recheck(task, run_dir, now, summary);
            }
            Err(inconsistent) => report_inconsistent(id, inconsistent, summary),
        }
    }

    /// 無効化マーカーを書いてから pid を再確認する。
    fn invalidate_and_recheck(
        &self,
        task: Task,
        run_dir: RunDirPath,
        now: Timestamp,
        summary: &mut TickSummary,
    ) {
        let id = task.id().clone();
        match self.runs.write_invalidation_marker(&run_dir) {
            Ok(()) => {}
            // マーカー無しで pending へ戻すと、遅れて起動したラッパーが新しい attempt と
            // 並走しうる。状態を変更せず次の tick に再試行を委ねる。
            Err(Io::Failed { message }) => {
                summary.errors.push(TickIssue::MarkerWriteFailed {
                    task_id: id,
                    message,
                });
                return;
            }
        }

        let Some(observed) = self.read_run_files(&id, &run_dir, summary) else {
            return;
        };
        match LaunchingClassifier::classify_recheck(observed.pid, observed.starttime) {
            Ok(LaunchingRecheck::ConfirmRunning(ident)) => {
                self.confirm_running(task, ident, now, summary);
            }
            Ok(LaunchingRecheck::SpawnFailed) => {
                let message = format!(
                    "起動から {} 秒のうちに pid ファイルが現れませんでした",
                    LaunchingClassifier::GRACE_PERIOD.seconds()
                );
                match task.record_spawn_failure(
                    message.clone(),
                    self.config.spawn_fail_limit(),
                    now,
                ) {
                    Ok(failed) => {
                        match self.commit(&failed, Freeze::of_recorded_failure(&failed), summary) {
                            Persisted::Saved => summary.errors.push(TickIssue::SpawnNotObserved {
                                task_id: id,
                                message,
                            }),
                            Persisted::Failed => {}
                        }
                    }
                    Err(error) => self.report_transition(id, error, summary),
                }
            }
            Err(inconsistent) => report_inconsistent(id, inconsistent, summary),
        }
    }

    /// 同定情報を取り込んで running にする。
    fn confirm_running(
        &self,
        task: Task,
        ident: ProcessIdent,
        now: Timestamp,
        summary: &mut TickSummary,
    ) {
        let id = task.id().clone();
        match task.confirm_running(ident, now) {
            Ok(running) => match self.commit(&running, Freeze::NotFrozen, summary) {
                Persisted::Saved => summary.confirmed_running.push(id),
                Persisted::Failed => {}
            },
            Err(error) => self.report_transition(id, error, summary),
        }
    }

    /// pid と starttime を1回ずつ読む。
    ///
    /// ファイルの不在も run ディレクトリ自体の不在も `Ok(None)` として猶予時間経路に
    /// 合流させる。読めない場合は報告してスキップし、書き込みは一切行わない。
    ///
    /// pid を先に読むのは、ラッパーが starttime → pid の順で書くため。逆順で読むと、
    /// 2回の読み取りの間に両方が書かれた正常な起動が「starttime なし・pid あり」として
    /// 観測され、健全なタスクが順序破れとして報告されたまま launching に滞留する。
    fn read_run_files(
        &self,
        id: &TaskId,
        run_dir: &RunDirPath,
        summary: &mut TickSummary,
    ) -> Option<RunFiles> {
        let pid = match self.runs.read_pid_file(run_dir) {
            Ok(pid) => pid,
            Err(error) => {
                summary.errors.push(TickIssue::RunFileUnreadable {
                    task_id: id.clone(),
                    error,
                });
                return None;
            }
        };
        let starttime = match self.runs.read_starttime(run_dir) {
            Ok(starttime) => starttime,
            Err(error) => {
                summary.errors.push(TickIssue::RunFileUnreadable {
                    task_id: id.clone(),
                    error,
                });
                return None;
            }
        };
        Some(RunFiles { pid, starttime })
    }
}

/// 書き込み順序の破れを報告する。次の tick が再観測する。
fn report_inconsistent(task_id: TaskId, kind: InconsistentRunFiles, summary: &mut TickSummary) {
    summary
        .errors
        .push(TickIssue::InconsistentRunFiles { task_id, kind });
}
