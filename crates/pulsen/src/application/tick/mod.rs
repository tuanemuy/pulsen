//! 1回の tick パスのユースケース(UC-execution-002)。
//!
//! 全タスクを走査し、実行状態(と、起動待ち・失敗確定では現ステータスの動作種別)ごとの
//! 手続きへ分岐する。
//!
//! 1タスクの処理失敗は `errors` に記録して残りを続行する。tick 全体を
//! 失敗させるのは、走査そのものができない場合とロック機構の異常だけ。
//!
//! タスクファイルに書き込んだ経路は、必ずサマリーのいずれかのフィールドを埋める。
//! 埋めないと、状態を変えた tick が「処理対象なし」と表示される。

mod confirm_spawn;
mod launch;
mod notify;
mod observe;

use std::path::PathBuf;

use pulsen_domain::definition::{AgentInput, GlobalConfig, StatusDefinition, TimeoutSpec};
use pulsen_domain::execution::{
    CommandRunner, ExclusiveLock, ExitCode, InconsistentRunFiles, LockError, NotifyFailureCause,
    ProcessController, RemnantOutcome, RunFileError, RunStore, WorktreeManager,
};
use pulsen_domain::task::{
    AttemptNumber, Clock, ExecutionState, ExecutionStateKind, ReadError, SaveError, StateRoot,
    Task, TaskEntry, TaskId, TaskRecord, TaskRepository, Timestamp, TransitionError, WorktreeRoot,
};

/// tick 1回の結末。
///
/// 変種の大きさの差は許容する — この値は tick 1回につき1つだけ作られ、`Box` による
/// 間接化は結末の形を変えるだけで得るものがない。
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::large_enum_variant)]
pub enum TickOutcome {
    /// 別の操作がロックを保持していたため、何もせずに終えた。
    Skipped,
    /// tick パスを実行した。
    Completed(TickSummary),
}

/// tick パス自体を実行できなかった原因。
///
/// どちらも状態を変更しない。ロック競合は失敗ではなく `TickOutcome::Skipped`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TickError {
    /// ロック機構自体の異常。
    LockFailed {
        /// 原因の説明。
        message: String,
    },
    /// 走査そのものの失敗。
    Scan {
        /// 原因の説明。
        message: String,
    },
}

/// 個別タスクの処理をスキップした理由。
///
/// 文言ではなく分類として持ち、利用者に見せる言葉は `cli::render` が決める。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TickIssue {
    /// タスクファイル全体を読めない。
    CorruptTaskFile {
        /// 対象のファイル。
        path: PathBuf,
        /// 読めない理由。
        message: String,
    },
    /// スナップショットだけを読めない(定義依存の判断ができない)。
    SnapshotUnreadable {
        /// 対象のタスク。
        task_id: TaskId,
        /// 読めない理由。
        message: String,
    },
    /// 現在 attempt がない(不変条件2の破れ)。手続きC・D のどちらの冒頭検査からも積まれる。
    MissingCurrentAttempt {
        /// 対象のタスク。
        task_id: TaskId,
    },
    /// 遷移の前提が成立しない(手動修復による不変条件の破れ)。
    Transition {
        /// 対象のタスク。
        task_id: TaskId,
        /// 成立しなかった前提。
        error: TransitionError,
    },
    /// run ファイルを読めない。
    RunFileUnreadable {
        /// 対象のタスク。
        task_id: TaskId,
        /// 読み取りの失敗。
        error: RunFileError,
    },
    /// ラッパーの書き込み順序(starttime → pid)が破れている。
    InconsistentRunFiles {
        /// 対象のタスク。
        task_id: TaskId,
        /// 破れの種別。
        kind: InconsistentRunFiles,
    },
    /// worktree を作成できず、ツール操作の失敗として記録した。
    WorktreeCreateFailed {
        /// 対象のタスク。
        task_id: TaskId,
        /// 原因の説明。
        message: String,
    },
    /// エージェントの起動コマンドを展開できず、spawn 失敗として記録した。
    CommandExpansionFailed {
        /// 対象のタスク。
        task_id: TaskId,
        /// 原因の説明。
        message: String,
    },
    /// 無効化マーカーを書けない。
    MarkerWriteFailed {
        /// 対象のタスク。
        task_id: TaskId,
        /// 原因の説明。
        message: String,
    },
    /// attempt の run ディレクトリを用意できない。
    PrepareAttemptFailed {
        /// 対象のタスク。
        task_id: TaskId,
        /// 原因の説明。
        message: String,
    },
    /// ラッパーの起動が同期エラーで失敗した。状態もカウンタも変えていない。
    SpawnFailed {
        /// 対象のタスク。
        task_id: TaskId,
        /// 原因の説明。
        message: String,
    },
    /// 猶予時間のうちに起動を観測できず、spawn 失敗として記録した。
    ///
    /// 同期エラーと分けるのは結末が違うため — こちらは `spawn_fail_count` を消費し、
    /// 上限を超えれば凍結する。分類が同じだと、実装が両者を取り違えても報告は変わらない。
    SpawnNotObserved {
        /// 対象のタスク。
        task_id: TaskId,
        /// 原因の説明。
        message: String,
    },
    /// 起動確認済みなのに現在 attempt が同定情報を持たない(不変条件3の破れ)。
    ///
    /// 不変条件2の破れと分けるのは、破れの事実も人間に求める修復も違うため — あちらは
    /// attempt 参照そのものが失われており、こちらは attempt はあるが pid ファイルから
    /// 復元できる可能性がある。同じ文言に畳むと、表示が修復の入口を示せなくなる。
    MissingProcessIdent {
        /// 対象のタスク。
        task_id: TaskId,
    },
    /// 生存観測の機構自体が失敗した。生死のどちらにも写像せず、状態を変更していない。
    ObservationFailed {
        /// 対象のタスク。
        task_id: TaskId,
        /// 原因の説明。
        message: String,
    },
    /// timeout を超えた実行を終了させられなかった。状態を変更していない。
    KillFailed {
        /// 対象のタスク。
        task_id: TaskId,
        /// 原因の説明。
        message: String,
    },
    /// 残存プロセスの終了を確認できなかった(ベストエフォートの報告)。
    RemnantsUnhandled {
        /// 対象のタスク。
        task_id: TaskId,
        /// 残った後始末。
        remnants: RemnantsLeft,
    },
    /// 判定自体が壊れており、判定失敗として記録した。
    JudgeFailed {
        /// 対象のタスク。
        task_id: TaskId,
        /// 判定できなかった原因。
        detail: String,
    },
    /// 実行の失敗を記録した。
    RunFailed {
        /// 対象のタスク。
        task_id: TaskId,
        /// 失敗と判断した根拠。
        cause: RunFailureCause,
    },
    /// 判定コマンドを持つステータスなのに workspace が未確定(不変条件4の破れ)。
    ///
    /// 遷移の前提の破れと分けるのは、遷移関数を一度も呼んでいないため — 起きたのは
    /// 「判定コマンドへ渡す `WORKSPACE` を組めなかった」であり、修復も帳簿の
    /// `workspace` を埋めることになる。
    MissingWorkspace {
        /// 対象のタスク。
        task_id: TaskId,
    },
    /// 凍結を通知できなかった。`notified_at` は書いていない。
    NotifyFailed {
        /// 対象のタスク。
        task_id: TaskId,
        /// 通知が届かなかった原因。
        cause: NotifyFailureCause,
    },
    /// タスクファイルを保存できない。
    SaveFailed {
        /// 対象のタスク。
        task_id: TaskId,
        /// 保存の失敗。
        error: SaveError,
    },
}

/// 実行を失敗として確定させた根拠。
///
/// 「誰が失敗と判断したか」を分類として持つ — デフォルト判定と判定コマンドを畳むと、
/// 判定コマンドが失敗と判定した場合にエージェント側の終了コードが失敗の根拠として
/// 読めてしまう(エージェントが 0 で終わっていても失敗になりうる)。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunFailureCause {
    /// 判定コマンドを持たないステータスのデフォルト判定が、非 0 終了を失敗とした。
    DefaultJudgement {
        /// 実行の終了コード。
        exit: ExitCode,
    },
    /// 判定コマンドが失敗と判定した。
    JudgeCommand {
        /// 実行の終了コード。判定コマンドが受け取った材料であり、失敗の根拠ではない。
        exit: ExitCode,
    },
    /// timeout を超えた実行を終了させた。
    TimedOut {
        /// 適用されていた timeout。
        timeout: TimeoutSpec,
    },
    /// 終了コードを残さずに死亡していた。
    DiedWithoutExit,
}

/// ベストエフォートの残存終了のあとに残った後始末。
///
/// `RemnantOutcome::Killed` は後始末を残さないため、この分類には現れない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemnantsLeft {
    /// 実行単位を同定できず、いかなるプロセスも終了させていない。
    NotIdentifiable,
    /// 同定できたが終了操作が失敗した。
    Failed {
        /// 原因の説明。
        message: String,
    },
}

impl RemnantsLeft {
    /// 残存終了の結末のうち、報告を要するものだけを取り出す。
    fn of(outcome: RemnantOutcome) -> Option<Self> {
        match outcome {
            RemnantOutcome::Killed => None,
            RemnantOutcome::NotIdentifiable => Some(Self::NotIdentifiable),
            RemnantOutcome::Failed { message } => Some(Self::Failed { message }),
        }
    }
}

/// tick パスの結果。
///
/// 1タスクは1tickで1ステップしか進まないため、進んだステップごとに別のフィールドで受ける
/// (`confirmed_running` は launching → running の取込、`judged` は判定の確定、
/// `transitioned` は次ステータスへの前進)。まとめると、どのステップが起きたかを結果から
/// 読み分けられなくなる。`archived` / `gc_deleted` / `gc_errors` は対応する手続きが未実装で、
/// まだ値の入る経路を持たない。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TickSummary {
    /// 起動したタスク。
    pub launched: Vec<TaskId>,
    /// 起動を確認して running へ取り込んだタスク。
    pub confirmed_running: Vec<TaskId>,
    /// 判定 completed を確定したタスク。
    pub judged: Vec<TaskId>,
    /// タスクステータスが遷移したタスク。
    pub transitioned: Vec<TaskId>,
    /// skipped 判定で起動待ちへ戻したタスク。
    pub skipped_back: Vec<TaskId>,
    /// 凍結したタスク。
    pub frozen: Vec<TaskId>,
    /// 通知したタスク。
    pub notified: Vec<TaskId>,
    /// 終端処理を終えたタスク。
    pub archived: Vec<TaskId>,
    /// スキップ・破損・観測失敗の報告。
    pub errors: Vec<TickIssue>,
    /// gc で削除した attempt。
    pub gc_deleted: Vec<(String, AttemptNumber)>,
    /// gc で削除できなかった attempt。
    pub gc_errors: Vec<(String, AttemptNumber)>,
}

impl TickSummary {
    /// 記録すべきことが1つも起きなかったか。
    ///
    /// タスクファイルに書き込んだ tick は、書き込みの内容がいずれかのフィールドに
    /// 現れるため偽になる。
    pub fn is_empty(&self) -> bool {
        self.launched.is_empty()
            && self.confirmed_running.is_empty()
            && self.judged.is_empty()
            && self.transitioned.is_empty()
            && self.skipped_back.is_empty()
            && self.frozen.is_empty()
            && self.notified.is_empty()
            && self.archived.is_empty()
            && self.errors.is_empty()
            && self.gc_deleted.is_empty()
            && self.gc_errors.is_empty()
    }
}

/// 全タスクを走査し、実行状態ごとの手続きへ分岐する。
///
/// ポートはジェネリック引数で受け取り、実アダプターとテストダブルのどちらにも同じ
/// 制御フローが乗ることを型で示す。
pub struct Tick<'a, R, L, K, W, S, P, C> {
    config: &'a GlobalConfig,
    state_root: &'a StateRoot,
    worktree_root: &'a WorktreeRoot,
    tasks: &'a R,
    lock: &'a L,
    clock: &'a K,
    worktrees: &'a W,
    runs: &'a S,
    processes: &'a P,
    commands: &'a C,
}

impl<'a, R, L, K, W, S, P, C> Tick<'a, R, L, K, W, S, P, C>
where
    R: TaskRepository,
    L: ExclusiveLock,
    K: Clock,
    W: WorktreeManager,
    S: RunStore,
    P: ProcessController,
    C: CommandRunner,
{
    /// 読み込み済みのグローバル設定・配置・各ポートを結線する。
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        config: &'a GlobalConfig,
        state_root: &'a StateRoot,
        worktree_root: &'a WorktreeRoot,
        tasks: &'a R,
        lock: &'a L,
        clock: &'a K,
        worktrees: &'a W,
        runs: &'a S,
        processes: &'a P,
        commands: &'a C,
    ) -> Self {
        Self {
            config,
            state_root,
            worktree_root,
            tasks,
            lock,
            clock,
            worktrees,
            runs,
            processes,
            commands,
        }
    }

    /// spec の処理フローの順に実行する。
    pub fn execute(&self) -> Result<TickOutcome, TickError> {
        let _guard = match self.lock.try_acquire() {
            Ok(Some(guard)) => guard,
            Ok(None) => return Ok(TickOutcome::Skipped),
            Err(LockError::Failed { message }) => return Err(TickError::LockFailed { message }),
        };

        let entries = self
            .tasks
            .list_active()
            .map_err(|ReadError::Io { message }| TickError::Scan { message })?;

        let mut summary = TickSummary::default();
        for entry in entries {
            self.process(entry, &mut summary);
        }
        Ok(TickOutcome::Completed(summary))
    }

    /// 走査結果1件を分岐処理する。
    fn process(&self, entry: TaskEntry, summary: &mut TickSummary) {
        match entry {
            TaskEntry::Corrupt { path, message } => {
                summary
                    .errors
                    .push(TickIssue::CorruptTaskFile { path, message });
            }
            // 定義依存の判断はすべてスキップして報告する。報告は実行状態によらない —
            // 修復が要るという事実は通知の成否と独立で、報告を欠くと notify_cmd 未定義の
            // 破損タスクが毎 tick 無言で消える。
            //
            // ただし未通知の凍結への再通知は定義に依存しない(必要な3値はスナップショット
            // 非依存のフィールドから得られる)ため行う — 欠くと、破損したタスクの凍結が
            // 永久に通知されない。
            TaskEntry::Record(TaskRecord::SnapshotUnreadable(degraded)) => {
                summary.errors.push(TickIssue::SnapshotUnreadable {
                    task_id: degraded.id().clone(),
                    message: degraded.snapshot_error().to_owned(),
                });
                match degraded.execution() {
                    ExecutionState::Stopped {
                        notified_at: None, ..
                    } => self.notify_degraded(degraded, summary),
                    ExecutionState::Pending
                    | ExecutionState::Launching { .. }
                    | ExecutionState::Running
                    | ExecutionState::Completed
                    | ExecutionState::Failed
                    | ExecutionState::Stopped {
                        notified_at: Some(_),
                        ..
                    } => {}
                }
            }
            TaskEntry::Record(TaskRecord::Intact(task)) => self.dispatch(task, summary),
        }
    }

    /// 実行状態と動作種別で手続きを選ぶ。
    fn dispatch(&self, task: Task, summary: &mut TickSummary) {
        match branch_of(&task) {
            Branch::Launch { input } => self.launch(task, input, summary),
            Branch::ConfirmSpawn { recorded_at } => self.confirm_spawn(task, recorded_at, summary),
            // 待機ステータスは tick が進めるものを持たない。
            Branch::Wait => {}
            // 手続きB(終端処理)は未実装。
            Branch::Cleanup => {}
            Branch::Observe => self.observe(task, summary),
            Branch::Advance => self.advance(task, summary),
            Branch::Notify => self.notify(task, summary),
            // 再通知は未通知のものだけを対象とする。
            Branch::AlreadyNotified => {}
        }
    }

    /// タスクステータスを次へ進める。
    ///
    /// 分岐は判定確定(成功)のタスクだけをここへ導くので、前提が成立しないのは手動修復に
    /// よる破れに限る。修復は人間に委ねる(書き込まない)。
    fn advance(&self, task: Task, summary: &mut TickSummary) {
        let id = task.id().clone();
        match task.advance(self.clock.now()) {
            Ok(advanced) => match self.commit(&advanced, Freeze::NotFrozen, summary) {
                Persisted::Saved => summary.transitioned.push(id),
                Persisted::Failed => {}
            },
            Err(error) => self.report_transition(id, error, summary),
        }
    }

    /// 遷移の結果を永続化し、凍結ならサマリーに記録して同じ tick で通知する。
    ///
    /// stopped を書いたすべての経路がここを通るため、通知の共通手続きはこの1箇所から
    /// 呼べる。順序は「stopped を書く → 通知を実行する → `notified_at` を追記する」で
    /// 固定する — 逆にすると、失敗した通知が永久に再送されない。
    fn commit(&self, task: &Task, freeze: Freeze, summary: &mut TickSummary) -> Persisted {
        match self.tasks.save(task) {
            Ok(()) => {
                match freeze {
                    Freeze::Frozen => {
                        summary.frozen.push(task.id().clone());
                        self.notify(task.clone(), summary);
                    }
                    Freeze::NotFrozen => {}
                }
                Persisted::Saved
            }
            Err(error) => {
                summary.errors.push(TickIssue::SaveFailed {
                    task_id: task.id().clone(),
                    error,
                });
                Persisted::Failed
            }
        }
    }

    /// 遷移の前提が成立しなかったことを報告する。
    ///
    /// 分岐は前提を満たすタスクだけをここへ導くので、成立しないのは手動修復による
    /// 不変条件の破れに限る。修復は人間に委ねる(書き込まない)。
    fn report_transition(
        &self,
        task_id: TaskId,
        error: TransitionError,
        summary: &mut TickSummary,
    ) {
        summary
            .errors
            .push(TickIssue::Transition { task_id, error });
    }
}

/// `save` の結果。呼び出し側は失敗時にそのタスクの処理を打ち切る。
enum Persisted {
    /// 永続化できた。
    Saved,
    /// 永続化できず、報告済み。
    Failed,
}

/// この保存が凍結を意味するか。
///
/// 凍結は遷移の結果であり、遷移を呼んだ側だけが知っている。保存後の状態が Stopped か
/// どうかで導出すると、既に凍結しているタスクを別の理由で保存する経路 — `notify` は
/// `mark_notified` した Stopped を `Freeze::NotFrozen` で保存する — が、過去の凍結を
/// 毎 tick 再計上する。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Freeze {
    /// この遷移が上限超過で凍結させた。
    Frozen,
    /// 凍結ではない。
    NotFrozen,
}

impl Freeze {
    /// 上限超過で凍結しうる遷移の結果から決める。
    ///
    /// 前提: 遷移前は凍結ではない(記録系の遷移は起動待ち・失敗確定・起動記録済み・
    /// 起動確認済みのいずれかしか受け付けない)。この前提のもとでのみ、遷移後の Stopped は
    /// 「今回凍結した」と同値。
    fn of_recorded_failure(task: &Task) -> Self {
        match task.execution_kind() {
            ExecutionStateKind::Stopped => Self::Frozen,
            ExecutionStateKind::Pending
            | ExecutionStateKind::Launching
            | ExecutionStateKind::Running
            | ExecutionStateKind::Completed
            | ExecutionStateKind::Failed => Self::NotFrozen,
        }
    }
}

/// tick がタスク1件に対して選ぶ分岐。
///
/// 実行状態(と、起動待ち・失敗確定では現ステータスの動作種別)の網羅として書く。
/// タスクを消費する手続きへ渡す前に判別を終えるため、判断だけを値にする。
enum Branch {
    /// 手続きA(起動)。
    Launch {
        /// エージェントに与える入力。動作種別の判別と同時に取り出す。
        input: AgentInput,
    },
    /// 何もしない(待機ステータス)。
    Wait,
    /// 手続きB(終端処理)。
    Cleanup,
    /// 手続きC(spawn 確認)。
    ConfirmSpawn {
        /// 猶予時間の起点。
        recorded_at: Timestamp,
    },
    /// 手続きD(観測・判定)。
    Observe,
    /// 次ステータスへの遷移。
    Advance,
    /// 未通知の凍結の通知。
    Notify,
    /// 通知済みの凍結 — 何もしない。
    AlreadyNotified,
}

/// タスクの分岐を決める。
fn branch_of(task: &Task) -> Branch {
    match task.execution() {
        ExecutionState::Pending | ExecutionState::Failed => match task.current_status_def() {
            StatusDefinition::AgentRun { input, .. } => Branch::Launch {
                input: input.clone(),
            },
            StatusDefinition::Wait => Branch::Wait,
            StatusDefinition::Cleanup => Branch::Cleanup,
        },
        ExecutionState::Launching { recorded_at } => Branch::ConfirmSpawn {
            recorded_at: *recorded_at,
        },
        ExecutionState::Running => Branch::Observe,
        ExecutionState::Completed => Branch::Advance,
        ExecutionState::Stopped {
            notified_at: None, ..
        } => Branch::Notify,
        ExecutionState::Stopped {
            notified_at: Some(_),
            ..
        } => Branch::AlreadyNotified,
    }
}
