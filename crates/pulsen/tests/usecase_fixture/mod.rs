//! ユースケーステストが共有するフィクスチャ。
//!
//! ポートはすべてテストダブルに差し替える。実プロセス・実ファイルシステムは
//! 使わない — ここで作るのは「実アダプターでは外から作れない状況」(入出力エラーの注入・
//! 猶予時間の境界・spawn の同期エラー)だからで、実物を使うと前提そのものが作れない。
//!
//! テストごとに使うフィクスチャが違うため、未使用の項目が残る。
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::PathBuf;

use pulsen::application::tick::{Tick, TickError, TickOutcome, TickSummary};
use pulsen_conformance::doubles::{
    LockOutcome, ScriptedCommandRunner, ScriptedExclusiveLock, ScriptedProcessController,
    ScriptedRunStore, ScriptedTaskRepository, ScriptedWorktreeManager, SettableClock,
};
use pulsen_domain::definition::{
    AgentInput, AgentName, DurationSpec, GlobalConfig, GlobalConfigInput, ModelName, PlainCommand,
    Prompt, RawAgentDefinition, RawCommand, SkillName, StatusDefinition, StatusName, TimeoutSpec,
    WorkflowDefinition, WorkflowName, WorkflowSnapshot,
};
use pulsen_domain::task::{
    AttemptNumber, AttemptRef, BranchName, DegradedTask, DegradedTaskFields, ExecutionState,
    FailureKind, FailureNote, KillIdent, Pid, ProcessIdent, ProcessStartTime, RepoPath,
    RetryCounters, RunDirPath, SaveError, StartTimeRecord, StateRoot, StopReason, Target, Task,
    TaskEntry, TaskFields, TaskId, TaskLookup, TaskRecord, Timestamp, WorkspacePlanner,
    WorktreeRoot,
};

/// 既定のタスクID。
pub const TASK: &str = "20260812t090000-k3f9qa1b";

/// 既定の現在時刻。
pub const NOW: &str = "2026-08-12T09:00:00Z";

/// プラットフォームに合わせた絶対パスを組む。
pub fn absolute(segments: &[&str]) -> PathBuf {
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

pub fn at(text: &str) -> Timestamp {
    Timestamp::parse_rfc3339(text).expect("受理される")
}

/// 既定の現在時刻から指定秒だけ進んだ(負なら戻った)時刻。
pub fn after(seconds: i64) -> Timestamp {
    Timestamp::from_unix_secs(at(NOW).unix_secs() + seconds).expect("受理される")
}

pub fn task_id(raw: &str) -> TaskId {
    TaskId::parse(raw.to_owned()).expect("受理される")
}

pub fn status(name: &str) -> StatusName {
    StatusName::parse(name.to_owned()).expect("受理される")
}

pub fn branch(name: &str) -> BranchName {
    BranchName::parse(name.to_owned()).expect("受理される")
}

pub fn agent_name(name: &str) -> AgentName {
    AgentName::parse(name.to_owned()).expect("受理される")
}

pub fn workflow_name() -> WorkflowName {
    WorkflowName::parse("implement".to_owned()).expect("受理される")
}

pub fn state_root() -> StateRoot {
    StateRoot::parse(absolute(&["home", "u", ".pulsen", "state"])).expect("受理される")
}

pub fn worktree_root() -> WorktreeRoot {
    WorktreeRoot::parse(absolute(&["home", "u", ".pulsen", "worktrees"])).expect("受理される")
}

pub fn repo() -> RepoPath {
    RepoPath::parse(absolute(&["repos", "pulsen"])).expect("受理される")
}

pub fn target() -> Target {
    Target::new(repo(), branch("main"))
}

/// タスクIDから決定的に導出される run ディレクトリ。
pub fn run_dir(raw: &str, number: u32) -> RunDirPath {
    RunDirPath::derive(&state_root(), &task_id(raw), attempt_number(number))
}

pub fn attempt_number(number: u32) -> AttemptNumber {
    AttemptNumber::parse(number).expect("受理される")
}

/// 起動記録済み(同定情報なし)の attempt 参照。
pub fn attempt(raw: &str, number: u32) -> AttemptRef {
    AttemptRef::rehydrate(attempt_number(number), run_dir(raw, number), None)
}

pub fn starttime() -> StartTimeRecord {
    StartTimeRecord::new(
        ProcessStartTime::parse("Wed Aug 12 08:59:59 2026".to_owned()).expect("受理される"),
        at("2026-08-12T08:59:59Z"),
    )
}

pub fn pid_content() -> pulsen_domain::execution::PidFileContent {
    pulsen_domain::execution::PidFileContent::new(Pid::new(4242), kill_ident())
}

/// 記録済み starttime と一致する観測値(照合が `Alive` になる)。
pub fn observed_starttime() -> ProcessStartTime {
    starttime().ident().clone()
}

/// 記録済み starttime と食い違う観測値(PID 再利用の再現)。
pub fn reused_starttime() -> ProcessStartTime {
    ProcessStartTime::parse("Wed Aug 12 09:30:00 2026".to_owned()).expect("受理される")
}

pub fn kill_ident() -> KillIdent {
    KillIdent::parse("-4242".to_owned()).expect("受理される")
}

/// pid ファイルと starttime が揃ったときに取り込まれる同定情報。
pub fn process_ident() -> ProcessIdent {
    ProcessIdent::new(Pid::new(4242), kill_ident(), starttime())
}

/// エージェント実行ステータスの内訳。既定はプロンプト入力・上書きなし。
pub struct AgentRunSpec<'a> {
    pub input: AgentInput,
    pub agent: Option<&'a str>,
    pub model: Option<&'a str>,
    pub timeout: Option<TimeoutSpec>,
    pub retries: Option<u32>,
    pub judge: Option<PlainCommand>,
    pub next: &'a str,
}

impl Default for AgentRunSpec<'_> {
    fn default() -> Self {
        Self {
            input: prompt("実装して"),
            agent: None,
            model: None,
            timeout: None,
            retries: None,
            judge: None,
            next: "done",
        }
    }
}

pub fn prompt(text: &str) -> AgentInput {
    AgentInput::Prompt(Prompt::parse(text.to_owned()).expect("受理される"))
}

pub fn skill(name: &str) -> AgentInput {
    AgentInput::Skill(SkillName::parse(name.to_owned()).expect("受理される"))
}

pub fn agent_run(spec: AgentRunSpec) -> StatusDefinition {
    StatusDefinition::AgentRun {
        input: spec.input,
        agent: spec.agent.map(agent_name),
        model: spec.model.map(model_name),
        timeout: spec.timeout,
        retries: spec.retries,
        judge: spec.judge,
        next: status(spec.next),
    }
}

pub fn command(text: &str) -> PlainCommand {
    PlainCommand::parse_text(text).expect("受理される")
}

pub fn timeout_secs(seconds: u64) -> TimeoutSpec {
    TimeoutSpec::Limited(DurationSpec::parse(&format!("{seconds}s")).expect("受理される"))
}

pub fn model_name(name: &str) -> ModelName {
    ModelName::parse(name.to_owned()).expect("受理される")
}

/// 先頭のステータスを初期ステータスとするスナップショット。
pub fn snapshot_of(
    default_agent: Option<&str>,
    default_model: Option<&str>,
    statuses: Vec<(&str, StatusDefinition)>,
) -> WorkflowSnapshot {
    let initial = status(statuses.first().expect("1件以上").0);
    let definition = WorkflowDefinition::new(
        default_agent.map(agent_name),
        default_model.map(model_name),
        initial,
        statuses
            .into_iter()
            .map(|(name, definition)| (status(name), definition))
            .collect::<BTreeMap<_, _>>(),
    )
    .expect("不変条件を満たす");
    WorkflowSnapshot::rehydrate(definition)
}

/// 初期のエージェント実行ステータスと、その遷移先だけを持つスナップショット。
pub fn snapshot_with(
    default_agent: Option<&str>,
    default_model: Option<&str>,
    queued: StatusDefinition,
) -> WorkflowSnapshot {
    snapshot_of(
        default_agent,
        default_model,
        vec![("queued", queued), ("done", StatusDefinition::Cleanup)],
    )
}

/// 動作種別3つを揃えた既定のスナップショット。
pub fn snapshot() -> WorkflowSnapshot {
    snapshot_of(
        Some("claude"),
        None,
        vec![
            ("queued", agent_run(AgentRunSpec::default())),
            ("waiting", StatusDefinition::Wait),
            ("done", StatusDefinition::Cleanup),
        ],
    )
}

/// `claude` エージェントだけを定義したグローバル設定。
pub fn config() -> GlobalConfig {
    config_with(vec![("claude", "claude {input}", None)], None)
}

/// エージェント定義と `spawn_fail_limit` を指定したグローバル設定。
pub fn config_with(
    agents: Vec<(&str, &str, Option<&str>)>,
    spawn_fail_limit: Option<u32>,
) -> GlobalConfig {
    let agents = agents
        .into_iter()
        .map(|(name, cmd, skill_input)| {
            (
                agent_name(name),
                RawAgentDefinition::new(
                    RawCommand::parse_text(cmd).expect("受理される"),
                    skill_input.map(str::to_owned),
                ),
            )
        })
        .collect::<BTreeMap<_, _>>();
    GlobalConfig::parse(GlobalConfigInput {
        agents: Some(agents),
        spawn_fail_limit,
        ..GlobalConfigInput::default()
    })
    .expect("受理される")
}

/// 通知コマンドを定義したグローバル設定。
pub fn config_notifying(notify_cmd: &str) -> GlobalConfig {
    GlobalConfig::parse(GlobalConfigInput {
        notify_cmd: Some(command(notify_cmd)),
        ..GlobalConfigInput::default()
    })
    .expect("受理される")
}

/// 判定の上限を指定したグローバル設定。通知コマンドは定義しない。
pub fn config_judging(judge_attempt_limit: u32) -> GlobalConfig {
    GlobalConfig::parse(GlobalConfigInput {
        judge_attempt_limit: Some(judge_attempt_limit),
        ..GlobalConfigInput::default()
    })
    .expect("受理される")
}

/// タスク1件を組み立てる。既定は「起動待ち・エージェント実行ステータス・未確定」。
pub struct TaskBuilder {
    fields: TaskFields,
}

pub fn task(raw: &str) -> TaskBuilder {
    TaskBuilder {
        fields: TaskFields {
            id: task_id(raw),
            workflow_name: workflow_name(),
            target: target(),
            snapshot: snapshot(),
            task_status: status("queued"),
            execution: ExecutionState::Pending,
            workspace: None,
            current_attempt: None,
            counters: RetryCounters::initial(),
            last_failure: None,
            updated_at: at(NOW),
        },
    }
}

impl TaskBuilder {
    pub fn snapshot(mut self, snapshot: WorkflowSnapshot) -> Self {
        self.fields.snapshot = snapshot;
        self
    }

    pub fn workflow(mut self, name: &str) -> Self {
        self.fields.workflow_name = WorkflowName::parse(name.to_owned()).expect("受理される");
        self
    }

    pub fn last_failure(mut self, note: FailureNote) -> Self {
        self.fields.last_failure = Some(note);
        self
    }

    pub fn updated_at(mut self, at: Timestamp) -> Self {
        self.fields.updated_at = at;
        self
    }

    pub fn status(mut self, name: &str) -> Self {
        self.fields.task_status = status(name);
        self
    }

    pub fn execution(mut self, execution: ExecutionState) -> Self {
        self.fields.execution = execution;
        self
    }

    /// 起動記録済みにする(猶予時間の起点を与える)。
    pub fn launching(self, recorded_at: Timestamp) -> Self {
        self.execution(ExecutionState::Launching { recorded_at })
    }

    /// タスクIDから導出したワークスペースを確定済みにする。
    pub fn workspace(mut self) -> Self {
        self.fields.workspace = Some(WorkspacePlanner::derive(&worktree_root(), &self.fields.id));
        self
    }

    /// 現在 attempt を与える。
    pub fn attempt(mut self, number: u32) -> Self {
        let raw = self.fields.id.as_str().to_owned();
        self.fields.current_attempt = Some(attempt(&raw, number));
        self
    }

    /// 起動確認済みにする(同定情報つきの attempt とワークスペースを揃える)。
    pub fn running(self, number: u32) -> Self {
        let raw = self.fields.id.as_str().to_owned();
        let mut task = self.workspace().execution(ExecutionState::Running);
        task.fields.current_attempt = Some(AttemptRef::rehydrate(
            attempt_number(number),
            run_dir(&raw, number),
            Some(process_ident()),
        ));
        task
    }

    /// 判定確定(成功)にする。
    pub fn completed(self, number: u32) -> Self {
        self.workspace()
            .attempt(number)
            .execution(ExecutionState::Completed)
    }

    /// 未通知の凍結にする。
    pub fn stopped(self, reason: StopReason) -> Self {
        self.execution(ExecutionState::Stopped {
            reason,
            notified_at: None,
        })
    }

    /// 通知済みの凍結にする。
    pub fn stopped_notified(self, reason: StopReason, at: Timestamp) -> Self {
        self.execution(ExecutionState::Stopped {
            reason,
            notified_at: Some(at),
        })
    }

    /// カウンタを `(attempt_count, judge_attempt_count, spawn_fail_count)` で与える。
    pub fn counters(mut self, attempt_count: u32, judge: u32, spawn_fail: u32) -> Self {
        self.fields.counters = RetryCounters::rehydrate(attempt_count, judge, spawn_fail);
        self
    }

    pub fn build(self) -> Task {
        Task::rehydrate(self.fields).expect("不変条件1を満たす")
    }

    /// 走査結果1件として返す。
    pub fn entry(self) -> TaskEntry {
        TaskEntry::Record(TaskRecord::Intact(self.build()))
    }

    /// 現役側で見つかった解決結果として返す。
    pub fn found(self) -> TaskLookup {
        TaskLookup::Active(TaskRecord::Intact(self.build()))
    }

    /// アーカイブ側で見つかった解決結果として返す。
    pub fn found_archived(self) -> TaskLookup {
        TaskLookup::Archived(TaskRecord::Intact(self.build()))
    }
}

/// 失敗要因1件。
pub fn failure(kind: FailureKind, message: &str, occurred: Timestamp) -> FailureNote {
    FailureNote::parse(kind, message.to_owned(), occurred).expect("非空の説明である")
}

/// スナップショットだけが読めないタスクを組み立てる。
pub struct DegradedBuilder {
    fields: DegradedTaskFields,
}

pub fn degraded(raw: &str, snapshot_error: &str) -> DegradedBuilder {
    DegradedBuilder {
        fields: DegradedTaskFields {
            id: task_id(raw),
            workflow_name: workflow_name(),
            target: target(),
            task_status: status("queued"),
            execution: ExecutionState::Pending,
            workspace: None,
            current_attempt: None,
            counters: RetryCounters::initial(),
            last_failure: None,
            updated_at: at(NOW),
            snapshot_error: snapshot_error.to_owned(),
        },
    }
}

impl DegradedBuilder {
    pub fn status(mut self, name: &str) -> Self {
        self.fields.task_status = status(name);
        self
    }

    pub fn execution(mut self, execution: ExecutionState) -> Self {
        self.fields.execution = execution;
        self
    }

    /// タスクIDから導出したワークスペースを確定済みにする。
    pub fn workspace(mut self) -> Self {
        self.fields.workspace = Some(WorkspacePlanner::derive(&worktree_root(), &self.fields.id));
        self
    }

    /// 現在 attempt を与える。
    pub fn attempt(mut self, number: u32) -> Self {
        let raw = self.fields.id.as_str().to_owned();
        self.fields.current_attempt = Some(attempt(&raw, number));
        self
    }

    /// カウンタを `(attempt_count, judge_attempt_count, spawn_fail_count)` で与える。
    pub fn counters(mut self, attempt_count: u32, judge: u32, spawn_fail: u32) -> Self {
        self.fields.counters = RetryCounters::rehydrate(attempt_count, judge, spawn_fail);
        self
    }

    pub fn build(self) -> DegradedTask {
        DegradedTask::rehydrate(self.fields)
    }

    /// 走査結果1件として返す。
    pub fn entry(self) -> TaskEntry {
        TaskEntry::Record(TaskRecord::SnapshotUnreadable(self.build()))
    }

    /// 現役側で見つかった解決結果として返す。
    pub fn found(self) -> TaskLookup {
        TaskLookup::Active(TaskRecord::SnapshotUnreadable(self.build()))
    }

    /// アーカイブ側で見つかった解決結果として返す。
    pub fn found_archived(self) -> TaskLookup {
        TaskLookup::Archived(TaskRecord::SnapshotUnreadable(self.build()))
    }
}

/// スナップショットだけが読めないタスクの走査結果。
pub fn degraded_entry(raw: &str, snapshot_error: &str) -> TaskEntry {
    degraded(raw, snapshot_error).entry()
}

/// 実行状態を指定した、スナップショットだけが読めないタスクの走査結果。
pub fn degraded_entry_with(
    raw: &str,
    snapshot_error: &str,
    execution: ExecutionState,
) -> TaskEntry {
    degraded(raw, snapshot_error).execution(execution).entry()
}

/// ファイル全体が読めないタスクの走査結果。
pub fn corrupt_entry(raw: &str, message: &str) -> TaskEntry {
    TaskEntry::Corrupt {
        path: absolute(&["home", "u", ".pulsen", "state", "tasks"]).join(format!("{raw}.json")),
        message: message.to_owned(),
    }
}

/// 走査結果を与えるリポジトリ。保存は成功する。
///
/// 同じ走査結果を複数回返す — tick を連続実行する主張(冪等性・クラッシュからの復旧)は、
/// 「前 tick の途中状態を初期状態として組み、次の tick を回す」形で書くため。
pub fn repository(entries: Vec<TaskEntry>) -> ScriptedTaskRepository {
    ScriptedTaskRepository::new()
        .with_list_active(std::iter::repeat_n(Ok(entries), SCAN_BUDGET))
        .with_save(std::iter::repeat_n(Ok(()), SAVE_BUDGET))
        .with_save_degraded(std::iter::repeat_n(Ok(()), SAVE_BUDGET))
}

/// 走査結果を与え、最初の保存が失敗するリポジトリ。
pub fn repository_failing_save(entries: Vec<TaskEntry>) -> ScriptedTaskRepository {
    ScriptedTaskRepository::new()
        .with_list_active(std::iter::repeat_n(Ok(entries), SCAN_BUDGET))
        .with_save([Err(SaveError::Io {
            message: "タスクファイルを書けない".to_owned(),
        })])
}

/// 1つのハーネスで回せる tick の回数(台本の余裕)。
const SCAN_BUDGET: usize = 3;

/// 1回の tick で起こりうる保存の回数の上限(台本の余裕)。
const SAVE_BUDGET: usize = 8;

/// 結線済みのポート一式。
///
/// 既定はロックを1回取得でき、タスクは0件、時刻は `NOW`。個別のテストは構造体更新
/// 構文で必要なポートだけを差し替える。
pub struct Harness {
    pub config: GlobalConfig,
    pub state_root: StateRoot,
    pub worktree_root: WorktreeRoot,
    pub tasks: ScriptedTaskRepository,
    pub lock: ScriptedExclusiveLock,
    pub clock: SettableClock,
    pub worktrees: ScriptedWorktreeManager,
    pub runs: ScriptedRunStore,
    pub processes: ScriptedProcessController,
    pub commands: ScriptedCommandRunner,
}

impl Harness {
    pub fn new() -> Self {
        Self {
            config: config(),
            state_root: state_root(),
            worktree_root: worktree_root(),
            tasks: repository(Vec::new()),
            lock: ScriptedExclusiveLock::new(std::iter::repeat_n(
                LockOutcome::Acquired,
                SCAN_BUDGET,
            )),
            clock: SettableClock::new(at(NOW)),
            worktrees: ScriptedWorktreeManager::new(),
            runs: ScriptedRunStore::new(),
            processes: ScriptedProcessController::new(),
            commands: ScriptedCommandRunner::new(),
        }
    }

    /// tick を1回実行する。
    pub fn run(&self) -> Result<TickOutcome, TickError> {
        Tick::new(
            &self.config,
            &self.state_root,
            &self.worktree_root,
            &self.tasks,
            &self.lock,
            &self.clock,
            &self.worktrees,
            &self.runs,
            &self.processes,
            &self.commands,
        )
        .execute()
    }

    /// tick パスが実行されたことを前提にサマリーを取る。
    pub fn completed(&self) -> TickSummary {
        match self.run() {
            Ok(TickOutcome::Completed(summary)) => summary,
            other => panic!("tick パスが実行される: {other:?}"),
        }
    }

    /// 保存されたタスクのうち指定IDのもの(最後の1件)。
    pub fn saved(&self, raw: &str) -> Task {
        let id = task_id(raw);
        self.tasks
            .saved()
            .into_iter()
            .rfind(|task| task.id() == &id)
            .expect("保存されている")
    }
}

impl Default for Harness {
    fn default() -> Self {
        Self::new()
    }
}
