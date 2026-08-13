# 実装手順 — Issue #2

## 設計

Issue #1 で確定した構成(3クレート・ポートは各ドメインの `port.rs`・合成ルートは `cli::wire` 1箇所・適合スイートは `pulsen-conformance`)をそのまま使い、**増分だけ**を足す。設計はドメイン → ポート → アダプター → ユースケース → CLI の順に外へ広げる。

### モジュールの増分

```
crates/pulsen-domain/src/
  definition/template.rs            + CommandLine::rehydrate(adr.md ADR-079)
  task/path.rs                      + RunDirPath の6つのファイル配置関数 / RunDirPath::state_root(adr.md ADR-078)
  task/planner.rs        (新規)      WorkspacePlanner
  task/counters.rs                  + pub(super) の更新関数(増加・リセット)
  task/attempt.rs                   + launching 記録専用の採番コンストラクタ
  task/failure.rs                   + 空メッセージを既定文言に畳む総関数
  task/transition.rs     (新規)      TransitionError
  task/task.rs                      + 遷移関数6種・問い合わせ5種
  execution/value.rs     (新規)      ExitCode / PidFileContent
  execution/launching.rs (新規)      LaunchingDecision / LaunchingRecheck / LaunchingClassifier / InconsistentRunFiles
  execution/port.rs                 + RunStore / ProcessController / WorktreeManager::create と付随の値・エラー型
crates/pulsen/src/
  adapter/run_store.rs   (新規)      FsRunStore
  adapter/process.rs     (新規)      SystemProcessController(OS 依存はここだけ)
  adapter/worktree.rs               + create
  application/run_wrapper.rs (新規)  UC-execution-009
  application/tick/mod.rs (新規)     UC-execution-002(走査・分岐・サマリー)
  application/tick/launch.rs (新規)  UC-execution-003(手続きA)
  application/tick/confirm_spawn.rs (新規) UC-execution-005(手続きC)
  cli/args.rs                       + Tick / Wrapper(hide)
  cli/tick.rs / cli/wrapper.rs (新規)
  cli/wire.rs                       + RunStore / ProcessController / state_root / worktree_root / compose_wrapper
  cli/render.rs                     + tick サマリーと TickIssue の文言
  examples/agent_probe.rs / spawn_probe.rs (新規。adr.md ADR-082)
crates/pulsen-conformance/
  HOOKS.md                          + 44行分の対応表(+ 台帳行に対応しない追加ケース1件)・冒頭の集計・区分表・「環境で走らなくなりうる行」
  src/lib.rs                        + RunStoreHarness / ProcessControllerHarness、WorktreeManagerHarness にフック追加
  src/run_store.rs (新規) / src/process_controller.rs (新規。identity_and_agent / spawn の2モジュール)
  src/worktree_manager.rs           + create の7ケース + ADR-085 由来の追加1ケース
  src/doubles/{run_store,process}.rs (新規) / doubles の拡張
crates/pulsen/tests/
  conformance_run_store.rs / conformance_process_controller.rs (新規)
  conformance_worktree.rs           + create のフック
  run_wrapper.rs / tick_scan.rs / tick_launch.rs / tick_confirm_spawn.rs (新規。ダブルに対するユースケーステスト)
  cli_tick.rs / cli_wrapper.rs (新規) / cli_usage.rs (更新)
  common/mod.rs                     + tick 起動ビルダー・runディレクトリ / worktree の確認ヘルパー・`wait_until`・config フィクスチャ
                                    + タスクファイルを直に組み立てる / 差し替えるヘルパー(failed 状態・スナップショットのみ破損・`state/archive/` 直置き)
  common/git.rs                     + worktree・ブランチの確認ヘルパー / ブランチだけを置く組み立てヘルパー
```

### ドメインモデルへの影響

**execution ドメイン(実質新規)**

- `ExitCode(i32)` — `is_success()` は 0 か否か。ラッパーが書く符号化値(exit code / 128+シグナル番号 / 127 / 126)をそのまま保持し、意味づけはしない。
- `PidFileContent { pid: Pid, kill_ident: KillIdent }` — pid ファイルの内容。この出現が「同定情報一式が揃った」というシグナルになる(starttime → pid の書き込み順序が前提)。
- `LaunchingDecision = ConfirmRunning(ProcessIdent) | KeepWaiting | SuspectSpawnFailure`、`LaunchingRecheck = ConfirmRunning(ProcessIdent) | SpawnFailed`、`InconsistentRunFiles = MissingStartTime`(破れの種別だけを持ち、利用者に見せる言葉は `cli::render` が決める。adr.md ADR-081 / ADR-094)。
- `LaunchingClassifier`(依存ポートなしの純粋サービス)
  - `const GRACE_PERIOD: DurationSpec`(30秒。`DurationSpec::from_secs_unchecked` は `pub(crate)` なので同一クレートから書ける)
  - `classify(recorded_at, now, pid, starttime) -> Result<LaunchingDecision, InconsistentRunFiles>`
  - `classify_recheck(pid, starttime) -> Result<LaunchingRecheck, InconsistentRunFiles>`
  - 経過は `Timestamp::elapsed_since`(巻き戻りは0に飽和)。超過は `> GRACE_PERIOD` のみ。
  - `ProcessIdent` は `PidFileContent` と `StartTimeRecord` から組み立てる。`InconsistentRunFiles` に run_dir やタスクIDを持たせない(文脈の付与は報告側の責務)。

**task ドメイン(追加)**

- `RunDirPath` に6つの導出関数(`pid_file` / `starttime_file` / `exit_file` / `stdout_log` / `stderr_log` / `marker_file`)。ファイル名は `pid` / `starttime` / `exit` / `stdout.log` / `stderr.log` / `invalidated` の定数。
- `RunDirPath::state_root(&self) -> Option<StateRoot>` — `derive` の逆写像(adr.md ADR-078)。`derive` の直下に置き、レイアウト知識を1箇所に保つ。
- `WorkspacePlanner::derive(worktree_root, id) -> Workspace` — `path = <worktree_root>/<task-id>`、`branch = pulsen/<task-id>`。`TaskId` の文字集合制約により常に有効な値になるので**全域関数**とし、`parse` の失敗は不変条件違反として `expect` で落とす(CLAUDE.md「パニックは不変条件違反にのみ使う」。既存のプロパティ的テスト `タスクidから導出したブランチ名は常に受理される` が裏付け)。
- `TransitionError = InvalidState { expected: &'static [ExecutionStateKind], actual: ExecutionStateKind } | WorkspaceAlreadySet | WorkspaceNotSet | NotAgentRunStatus { status } | MissingCurrentAttempt`(分類だけを持ち、文言は `cli::render` が組み立てる。adr.md ADR-096)。
- `Task` の遷移6種はいずれも `self` を消費し `now: Timestamp` で `updated_at` を更新する。
  - `confirm_workspace(self, ws, now)` — `workspace = None` が前提。Some なら `WorkspaceAlreadySet`。
  - `record_launching(self, state_root, now) -> Result<(Task, RunDirPath), TransitionError>` — 前提は `Pending | Failed` × AgentRun × workspace 確定済み。`next_attempt_number()` で採番し、`RunDirPath::derive` で導出した run_dir を `AttemptRef`(`process: None`)に載せて `Launching { recorded_at: now }` にする。**番号とパスの食い違いを構成で排除する**ため、`AttemptRef` の採番コンストラクタは番号と run_dir を同時に受け取り `task` モジュール内に閉じる。
  - `confirm_running(self, process, now)` — 前提 `Launching`。`current_attempt` が None なら `MissingCurrentAttempt`。`process` を取り込み `spawn_fail_count` **だけ**を 0 にする。
  - `record_spawn_failure(self, message, spawn_fail_limit, now)` — 前提 `Launching`。`spawn_fail_count += 1`。超過なら `Stopped { SpawnFailLimitExceeded, notified_at: None }`、でなければ `Pending`。`last_failure = SpawnFail`。
  - `record_spawn_failure_in_place(self, message, spawn_fail_limit, now)` — 前提 `Pending | Failed`。`spawn_fail_count += 1`。超過なら `Stopped`、でなければ**実行状態を変えない**。attempt 採番なし。
  - `record_tool_failure(self, kind, message, retry_limit, now)` — 前提 `Pending | Failed`。`attempt_count += 1`。超過なら `Stopped { RetryLimitExceeded }`、でなければ `Failed`。
  - 上限超過の判定(`count > limit`)は private ヘルパー1つに集約する。3箇所で書くと等号の扱いがずれる。
- `Task` の問い合わせ5種 — `execution_kind()`(`ExecutionState::kind` への委譲)/ `current_status_def()`(不変条件1により全域)/ `next_attempt_number()`(None なら `AttemptNumber::FIRST`)/ `is_agent_run` / `is_wait` / `is_cleanup` / `applicable_retry_limit()`(AgentRun / Cleanup とも `snapshot().effective_retry_limit(task_status)` に委譲、Wait は None。Cleanup の 2 は `WorkflowDefinition::DEFAULT_RETRY_LIMIT` から返るので、この値をドメインに書き直さない)。
- `RetryCounters` に `pub(super)` の更新関数(`increment_attempt` / `increment_spawn_fail` / `reset_spawn_fail`)を足す。公開 API は増やさない — 帳簿の更新規則は遷移関数の事後条件としてのみ観測されるべきものだから。
- `FailureNote` に「空メッセージを既定文言に畳む」総関数を足し、遷移関数がそれを使う(呼び出し側の文字列が空でも `Result` を増やさない。ADR-036 の「既定値を与える総関数は使ってよい」に沿う)。

**definition ドメイン(追加1点)**

- `CommandLine::rehydrate(tokens: Vec<String>) -> Result<Self, CommandError>` — プロセス境界(ラッパーの起動引数)からの再構築専用。0トークンは `Empty`(adr.md ADR-079)。

### ポート

`crates/pulsen-domain/src/execution/port.rs` に**本スライスで使うメソッドだけ**を足す。未実装メソッドの宣言は置かない。`RunStore` / `ProcessController` はステップ5、`WorktreeManager::create` は既存の実装を同時に追えるステップ7 で宣言する。

```rust
pub enum Io { Failed { message: String } }               // 機構失敗の不透明な報告用(adr.md ADR-086)
pub enum RunFileError { Corrupt { path: PathBuf, message: String }, Io { message: String } }

pub trait RunStore {
    fn prepare_attempt(&self, id: &TaskId, number: AttemptNumber) -> Result<RunDirPath, Io>;
    fn read_pid_file(&self, run_dir: &RunDirPath) -> Result<Option<PidFileContent>, RunFileError>;
    fn read_starttime(&self, run_dir: &RunDirPath) -> Result<Option<StartTimeRecord>, RunFileError>;
    fn read_exit(&self, run_dir: &RunDirPath) -> Result<Option<ExitCode>, RunFileError>;
    fn write_invalidation_marker(&self, run_dir: &RunDirPath) -> Result<(), Io>;
    fn marker_exists(&self, run_dir: &RunDirPath) -> Result<bool, Io>;
    fn write_starttime(&self, run_dir: &RunDirPath, record: &StartTimeRecord) -> Result<(), Io>;
    fn write_pid_file(&self, run_dir: &RunDirPath, content: &PidFileContent) -> Result<(), Io>;
    fn write_exit(&self, run_dir: &RunDirPath, code: &ExitCode) -> Result<(), Io>;
}

pub struct WrapperLaunchSpec { /* run_dir: RunDirPath, agent_cmd: CommandLine, workspace: WorktreePath */ }
pub struct WrapperIdentity  { /* pid: Pid, kill_ident: KillIdent, starttime: StartTimeRecord */ }
pub enum SpawnError { Failed { message: String } }

pub trait ProcessController {
    fn spawn_wrapper(&self, spec: &WrapperLaunchSpec) -> Result<(), SpawnError>;
    fn own_identity(&self) -> Result<WrapperIdentity, Io>;
    fn run_agent(&self, cmd: &CommandLine, cwd: &WorktreePath, stdout: &Path, stderr: &Path) -> ExitCode;
}

pub enum WorktreeError { Failed { message: String } }
// WorktreeManager に1メソッド追加
fn create(&self, repo: &RepoPath, base: &BranchName, ws: &Workspace) -> Result<(), WorktreeError>;
```

各トレイトには既存の `TaskRepository` / `ExclusiveLock` と同じ `/// 契約:` の箇条書きを付ける(read系の `Ok(None)` にディレクトリ不在を含めること、write系のアトミック置換と非観測性、**write系はいずれも書き込み先のディレクトリを必要に応じて作ること**(adr.md ADR-080)、`spawn_wrapper` が起動後の成否に関知しないこと、`run_agent` が失敗しないこと、`create` の冪等性の境界)。

### ユースケース / アプリケーションロジック

**RunWrapper**(`application/run_wrapper.rs`)

```rust
pub struct RunWrapper<'a, S, P> { runs: &'a S, processes: &'a P }
pub enum WrapperOutcome { Ran(ExitCode), Suppressed, Silent }   // Silent = 何も書き残さず終了
pub fn execute(&self, spec: &WrapperLaunchSpec) -> WrapperOutcome
```

`own_identity` → `write_starttime` → `write_pid_file` → `marker_exists` → `run_agent` → `write_exit` の順。`own_identity` / `write_starttime` / `write_pid_file` の失敗は `Silent`(何も書き残さず終了)。`marker_exists` が `true` でも `Err` でも `Suppressed`(エージェントを起動せず正常終了)。`write_exit` の失敗は `Ran` のまま終える(exit を書けないまま終了 = 次tickが「exitなし・プロセス死亡」と分類する)。config も読まずロックも取らない。

**Tick**(`application/tick/`)

```rust
pub struct Tick<'a, R, L, K, W, S, P> {
    config: &'a GlobalConfig,
    state_root: &'a StateRoot,
    worktree_root: &'a WorktreeRoot,
    tasks: &'a R, lock: &'a L, clock: &'a K,
    worktrees: &'a W, runs: &'a S, processes: &'a P,
}
pub enum TickOutcome { Skipped, Completed(TickSummary) }
pub enum TickError { LockFailed { message: String }, Scan { message: String } }
pub fn execute(&self) -> Result<TickOutcome, TickError>
```

- `Ok(None)` のロック競合は `TickOutcome::Skipped`(CLI が 0 で終える)。`Err(LockError::Failed)` と `list_active` の Io は `TickError`(非0・状態は変更しない)。
- 分岐は網羅 `match`。本スライスで配線するのは `Corrupt` / `SnapshotUnreadable` / Pending・Failed × (Wait | AgentRun) / `Launching` の4系統で、残りのアームには引き取り先のスライスを why コメントとして残す(adr.md ADR-101)。
- サマリー DTO は spec の9フィールドに `confirmed_running` を加えた10フィールドを持ち(adr.md ADR-094)、本スライスで値が入るのは `launched` / `confirmed_running` / `frozen` / `errors`。`errors` は構造化した `TickIssue`(adr.md ADR-081)。
- 上限超過で `Stopped` を書いた直後の処理を private な `freeze` 相当に集約し、`frozen` に記録する。notify の呼び出しは #3 がここに足す(adr.md ADR-074)。

**手続きA**(`tick/launch.rs`)— spec の順序どおり。

1. `task.workspace()` が None → `WorkspacePlanner::derive(worktree_root, id)` → `WorktreeManager::create(repo, base, ws)` → 成功なら `confirm_workspace` → `save`。失敗は `record_tool_failure(WorktreeCreate, message, effective_retry_limit, now)` → `save` →(Stopped なら `frozen`)→ 終了
2. テンプレート展開(a〜e のどれが失敗しても `record_spawn_failure_in_place(message, config.spawn_fail_limit(), now)` → `save` →(Stopped なら `frozen`)→ 終了)
   a. `snapshot.effective_agent(status)`(None は展開失敗)
   b. `config.agent(name)`(不在は展開失敗)
   c. `RawAgentDefinition::parse`
   d. `AgentDefinition::render_input(status の AgentInput)`
   e. `AgentDefinition::build_command_line(input, effective_model, workspace.path())`
3. `record_launching(state_root, now)` → `save`(復旧の起点)
4. `RunStore::prepare_attempt(id, number)`(失敗は `errors` に報告のみ)
5. `ProcessController::spawn_wrapper(WrapperLaunchSpec)`(同期エラーも `errors` に報告のみ・状態を変更しない)→ `launched` に記録

**手続きC**(`tick/confirm_spawn.rs`)

0. `current_attempt` が None → `TickIssue::MissingCurrentAttempt` を報告してスキップ
1. `read_pid_file` / `read_starttime`(`RunFileError` は報告してスキップ・書き込まない)
2. `LaunchingClassifier::classify`
   - `ConfirmRunning(ident)` → `confirm_running` → `save`
   - `KeepWaiting` → 何もしない(書き込みを一切発生させない)
   - `SuspectSpawnFailure` → `write_invalidation_marker`(`Err` は状態を変更せず報告してスキップ)→ 再読 → `classify_recheck` → `ConfirmRunning` は `confirm_running` → `save`、`SpawnFailed` は `record_spawn_failure` → `save` →(Stopped なら `frozen`)
   - `Err(InconsistentRunFiles)` → 報告してスキップ

`save` の失敗はいずれの手続きでも `TickIssue::SaveFailed` として報告し、そのタスクの処理を打ち切って次のタスクへ進む。

### アダプター / 永続化 / 外部連携

**`FsRunStore`**(`adapter/run_store.rs`)— `new(state_root: StateRoot)`(ADR-043)。

- `prepare_attempt` は `RunDirPath::derive` で導出したパスを `util::fsdir::ensure_dir` で作り、そのパスを返す(冪等)。
- read系は「ファイル/ディレクトリ不在 → `Ok(None)`」「JSON として読めない・値制約違反 → `Corrupt { path, message }`」「機構失敗 → `Io`」の3分類。DTO は `serde` derive の private struct(adr.md ADR-080)。
- write系は `util::atomic::write_atomic` を呼ぶだけ。`write_invalidation_marker` は `ensure_dir` してから空バイト列を `write_atomic`(冪等)。`marker_exists` は `try_exists`(不在を I/O エラーに丸めない)。

**`SystemProcessController`**(`adapter/process.rs`)— `new(self_exe: PathBuf, identity_source: IdentitySource, clock: SystemClock)`(adr.md ADR-076)。`IdentitySource` は `PathBuf` の newtype で、`platform_default()`(POSIX 非 Linux は `ps`、Linux は procfs のルート、Windows は powershell)もこのファイルに置く。OS 依存分岐はこのファイルだけに置く(adr.md ADR-075)。

- `spawn_wrapper`: `<self_exe> wrapper --run-dir <run_dir> --workspace <workspace> -- <agent_cmd tokens...>` を stdio null・新しいプロセスグループ相当でデタッチ起動し、`Child` を待たずに drop する。フラグ名とサブコマンド名は**このファイルの `pub const`** として定義し、CLI 側のパーサがその argv をそのまま受理することを往復テストで主張する(定義箇所が2つに分かれるため)。
- `own_identity`: `std::process::id()` → `Pid`、`ProcessStartTime` と `KillIdent`(POSIX は観測した PGID から `-<pgid>`、Windows は `<pid>`)は pid を受け取る private 関数1つが同じ観測から返す(取得手段と表現の固定は adr.md ADR-075)、`wall` は注入した `Clock`。共有関数は三値(`Ok(Some)` / `Ok(None)` = 不在 / `Err(Io)` = 機構失敗)を返し、`own_identity` 側で `Ok(None)` を `Err(Io)` に畳む。
- `run_agent`: cwd の到達性 → ログの `File::create` → `Command` の直接起動、の順に確認して符号化する(126 / 127 / 128+n の分岐は adr.md ADR-075)。

**`GitCliWorktreeManager::create`**(`adapter/worktree.rs` に追加)— 既存の private `run()`(env を落とした `git -C <repo>`)を再利用する。判定は問い合わせコマンドの組み合わせで導く(ADR-024 の方針)。

1. 同定用の鍵を作る private 関数を1つ置く — `physical_key(p) = std::fs::canonicalize(p.parent()) . join(p.file_name())`(adr.md ADR-085)。`ws.path` の親(worktree_root)は事前に `ensure_dir` する。パス自体を canonicalize しないのは、実体が消えている場合に失敗して比較そのものが成立しないため
2. `git -C <repo> worktree list --porcelain` の各エントリの `worktree` 行を**同じ `physical_key` に通して**から `ws.path` の鍵と突き合わせる(**正規化は両側に対称に適用する**。生のパスの文字列比較は禁じる。片側だけの正規化は Windows の拡張長パスで必ず外れる)。鍵に変換できないエントリは不一致として扱う。一致するエントリが `ws.branch` を指し、`ws.path` が実体として存在し(`try_exists`)、かつ `prunable` が付いていなければ達成済みとして `Ok`(内容に触れない)。別ブランチなら `Failed`。一致し、`ws.branch` を指すが実体が無いか `prunable` が付いている(登録は残っているが実体が消えている)なら `worktree add -f <path> <branch>` で張り直して `Ok`
3. 登録が無く、`ws.path` が実体として存在するなら `Failed`(自動修復しない)
4. `show-ref --verify --quiet refs/heads/<ws.branch>` で既存ブランチを判定 — 存在すれば `worktree add <path> <branch>`(**`-f` なし**。先端を変えない)、しなければ `worktree add -b <branch> <path> <base>`
5. 非0終了・起動失敗はすべて `WorktreeError::Failed { message }`(分類には使わない不透明な message)

ハーネスの `worktree_root` を**シンボリックリンク経由のパス**として組み立てれば全ケースが正規化の下で走る(macOS の一時ディレクトリでは自然にそうなるが、プラットフォームによらず固定する)。復旧の2分岐は別々のケースで通す — `TC-port-worktree-manager-013`(ブランチのみ存在)の前提は台帳の字義どおり**登録なし・ブランチのみ存在(コミットが積まれている)**として作り、手順4 の `-f` なしの張り直しを通す。`prunable` 登録の張り直し(手順2)は台帳に無い ADR-085 由来の要求なので、worktree を作ってから実体だけを消すフックによる**追加ケース**を1件置く。`TC-013` を prunable 側に寄せると手順4 の分岐がどのケースからも実行されず、実装ごと落ちても適合スイートが緑になる。

**`cli::wire`** — `Runtime` に `runs: FsRunStore` と `state_root()` / `worktree_root()` のアクセサを足す(ADR-061 の「必要になったスライスで理由つきで戻す」に該当。why をコードに添える)。`SystemProcessController` の構築(= `std::env::current_exe()` の読み取り、失敗は `WireError::SelfExeUnavailable`。取得元は `IdentitySource::platform_default()`)は `compose` に載せず、`wire::process_controller() -> Result<SystemProcessController, WireError>` として `tick` の経路でだけ呼ぶ(adr.md ADR-076)。同じ規則で `Runtime::workflow_store()`(カレントディレクトリの取得)と `wire::id_generator()`(乱数の初期化)も `compose` から外し、`add` の経路でだけ呼ぶ(adr.md ADR-099)。ラッパー用には**ホームも config も読まない** `compose_wrapper(run_dir) -> Result<WrapperRuntime, WireError>` を別に置き、その中で `SystemProcessController::without_self_exe(IdentitySource::platform_default(), SystemClock::new())` を組む — ラッパーは `spawn_wrapper` を呼ばないので `current_exe()` を要さず、読めば使わないリソースの失敗で何も書けずに終わる経路が1本増える(adr.md ADR-076 の「ラッパーの合成だけは `self_exe` を持たない構成を使う」/ ADR-078)。

### CLI / プレゼンテーション

- `Command` に `Tick`(引数なし)と `#[command(hide = true)] Wrapper(WrapperArgs)` を足す(adr.md ADR-077)。`WrapperArgs` は `--run-dir` / `--workspace` / 末尾可変長のエージェントコマンドで、末尾は `trailing_var_arg` + `allow_hyphen_values` で受ける(adr.md ADR-077)。`spawn_wrapper` が組む argv をそのまま受理できることの往復テストには、`-` 始まりのトークン(`--model` 等)・空文字列トークン(`TC-port-config-store-007` が許す形)・シェルのメタ文字を含むトークン(`TC-port-process-controller-021` と同じ形)を入れる。
- `cli/tick.rs` は `compose` → `Tick::new(...)` → `execute()`。`TickOutcome::Skipped` は「別の操作が実行中のためスキップした」旨を表示して **0**。`Completed` はサマリーを表示して 0。`TickError` は非0。
- `cli/wrapper.rs` は `compose_wrapper` → `RunWrapper::execute`。引数を `RunDirPath` / `WorktreePath` / `CommandLine::rehydrate` に通すのがこのコマンドの parse 境界(ADR-048)。失敗は何も書かずに非0。
- `cli/render.rs` に tick サマリー(値の入っているフィールドだけを出す。処理対象が無ければその旨)と `TickIssue` の文言を足す。`TickIssue` はタスクIDまたはパスと原因が読み取れる形で出す。

## 実装ステップ

依存方向の順(domain → port → adapter → usecase → cli → 受け入れ)に並べる。各ステップは単体でレビュー可能で、テストが通る状態で終える。適合ケースは対応するアダプターと同じステップに置き、「契約を書く → 実装が通す」でステップが閉じるようにする。ただし `spawn_wrapper` の3件だけは `wrapper` サブコマンドを必要とするため後段に置く(adr.md ADR-083)。

**既に実装のあるトレイトにメソッドを足すステップは、その時点の全実装(アダプターとテストダブル)を同じステップで追う。** 宣言だけを先行させるとワークスペースがビルドできない状態が後続ステップまで続き、「各ステップはテストが通る状態で終える」が成立しない。`WorktreeManager::create` が該当し(`GitCliWorktreeManager` と `ScriptedWorktreeManager` に既存の impl がある)、宣言・git 実装・ダブルの更新・適合7件をステップ7 にまとめる。`RunStore` / `ProcessController` は新規トレイトなので、宣言(ステップ5)と実装(ステップ6 / 8)が分かれても他をビルド不能にしない。

計画時点で `.thread/2/adr.md` に起票した14件のうち、プロジェクト全体に効くもの(ADR-074〜086 と ADR-101 の多く)は片付けフェーズで `.adr/` への昇格を判定する。

### 1. execution ドメイン: 値オブジェクトと runディレクトリのファイル配置語彙

- **対象ファイル:** `crates/pulsen-domain/src/execution/{mod.rs,value.rs}`、`crates/pulsen-domain/src/task/path.rs`
- **変更内容:** `ExitCode`(`new` / `get` / `is_success`)と `PidFileContent`(`new` / `pid` / `kill_ident`)を追加する。`RunDirPath` に `pid_file` / `starttime_file` / `exit_file` / `stdout_log` / `stderr_log` / `marker_file` の6つの導出関数と、ファイル名の定数を追加する。あわせて `RunDirPath::state_root`(`derive` の逆写像。`<state_root>/runs/<task-id>/attempt-<n>` に合致しない値は `None`)を `derive` の直下に置く(adr.md ADR-078)。ユニットテストでパスの導出、`is_success` の 0 / 非0、`derive` → `state_root` の往復と形式外の `None` を検証する。
- **理由:** runディレクトリの語彙はドメイン側に置き、アダプターにレイアウト知識を漏らさない。以降のすべてのステップがこの語彙の上に乗る。
- **消化するチェックリスト:** DOM-execution-001, 003, 028〜033

### 2. execution ドメイン: LaunchingClassifier と分類の直和型

- **対象ファイル:** `crates/pulsen-domain/src/execution/launching.rs`、`mod.rs`
- **変更内容:** `LaunchingDecision` / `LaunchingRecheck` / `InconsistentRunFiles` / `LaunchingClassifier`(`GRACE_PERIOD` = 30秒、`classify` / `classify_recheck`)を実装する。ユニットテストで全分岐と境界を網羅する — pid+starttime → `ConfirmRunning`(`ProcessIdent` の3値が入る)、pid なし・経過29秒/30秒 → `KeepWaiting`、31秒 → `SuspectSpawnFailure`、`now < recorded_at`(巻き戻り)→ `KeepWaiting`、pid あり starttime なし → `Err`、`classify_recheck` は pid なし(starttime あり/なしの両方)→ `SpawnFailed`・pid+starttime → `ConfirmRunning`・pid あり starttime なし → `Err`。
- **理由:** 手続きCの判断をポートから完全に切り離し、時刻依存の境界を I/O なしのユニットテストで固定する。
- **消化するチェックリスト:** DOM-execution-006, 007, 013, 014, 015, 016(TC-exec-tick-077〜079 の境界もここで裏付ける)

### 3. task ドメイン: 問い合わせメソッドと WorkspacePlanner

- **対象ファイル:** `crates/pulsen-domain/src/task/{task.rs,planner.rs,mod.rs}`
- **変更内容:** `Task` に `execution_kind` / `current_status_def` / `next_attempt_number` / `is_agent_run` / `is_wait` / `is_cleanup` / `applicable_retry_limit` を追加する。`WorkspacePlanner::derive(worktree_root, id) -> Workspace` を新しいモジュールに置き、ブランチ接頭辞を定数化する。ユニットテストで、AgentRun / Wait / Cleanup の各ステータスに対する動作種別と `applicable_retry_limit`(AgentRun は `retries` 指定あり/なし、Cleanup は組み込みデフォルトの 2、Wait は None)、`next_attempt_number` の None → 1 と Some(n) → n+1、導出された `Workspace` のパスとブランチ名を検証する。
- **理由:** 手続きAの分岐(動作種別)と上限の出所を、遷移関数を書く前に確定させる。`WorkspacePlanner` は手続きAと(#6 の)手続きBの両方が同じパスに到達するための単一の導出点。
- **消化するチェックリスト:** DOM-task-048〜052, 061

### 4. task ドメイン: 遷移関数と TransitionError

- **対象ファイル:** `crates/pulsen-domain/src/task/{transition.rs,task.rs,counters.rs,attempt.rs,failure.rs,mod.rs}`
- **変更内容:** `TransitionError`(5種)を定義し、`confirm_workspace` / `record_launching` / `confirm_running` / `record_spawn_failure` / `record_spawn_failure_in_place` / `record_tool_failure` を実装する。`RetryCounters` に `pub(super)` の更新関数、`AttemptRef` に launching 記録専用の採番コンストラクタ、`FailureNote` に空メッセージを既定文言へ畳む総関数を足す。上限超過の判定は private ヘルパー1つに集約する。ユニットテストで、前提状態の不一致(6状態それぞれからの呼び出し)、`confirm_workspace` の再確定拒否、`record_launching` の workspace 未確定 / 非 AgentRun / 番号とパスの整合 / `recorded_at`、`confirm_running` の `spawn_fail_count` のみリセット(`attempt_count` / `judge_attempt_count` は保持)、`record_spawn_failure_in_place` の状態不変・attempt 不変、上限の等号(凍結しない)と +1(凍結する)、`retries: 0` での即凍結、`last_failure` の種別を検証する。
- **理由:** 帳簿の遷移を純粋関数として確定させ、ユースケースが「判断」を持たない形にする。カウンタのリセット規則を事後条件として1箇所に固定する。
- **消化するチェックリスト:** DOM-task-033〜037, 042(TC-exec-tick-035, 036, 048〜050 の境界もここで裏付ける)

### 5. ポートの追加と境界の値型

- **対象ファイル:** `crates/pulsen-domain/src/execution/{port.rs,mod.rs}`、`crates/pulsen-domain/src/definition/template.rs`
- **変更内容:** `RunStore`(9メソッド)・`ProcessController`(3メソッド)を宣言し、`Io` / `RunFileError` / `WrapperLaunchSpec` / `WrapperIdentity` / `SpawnError` を定義する(`Io` の名前と共有は adr.md ADR-086)。各トレイトに `/// 契約:` の箇条書きを付ける。`CommandLine::rehydrate` を追加する(adr.md ADR-079)。ユニットテストで `CommandLine::rehydrate` の往復と0トークン拒否を検証する。
- **理由:** ドメインが外界に要求する操作をここで固定し、以降のアダプター・ユースケース・ダブルが同じ形に乗る。**本スライスで使わないメソッドは宣言しない**(Issue #1 で確立した規約)。`read_exit` だけはこのスライスに呼び出し側が無いが、チェックリスト行(`DOM-execution-037` / `ADP-runstore-004` / `TC-port-run-store-012〜015`)として要求されているので宣言する — 実際の消費者は手続きD(#3)と show(#4)。`WorktreeManager::create` をここに含めないのは、既存の impl を同時に追えるステップ7 に置くため(上記の規則)。
- **消化するチェックリスト:** DOM-execution-034〜037, 039〜043, 047, 048, 052〜055, 057

### 6. RunStore: 適合スイート21件と `FsRunStore`

- **対象ファイル:** `crates/pulsen-conformance/src/{lib.rs,run_store.rs}`、`crates/pulsen-conformance/HOOKS.md`、`crates/pulsen/src/adapter/run_store.rs`、`crates/pulsen/tests/conformance_run_store.rs`
- **変更内容:** `RunStoreHarness`(対象アクセサ + `expected_run_dir` / `attempt_dir_present(run_dir) -> bool`(環境に問う観測フック)/ `put_unreadable_content(run_dir, kind)` / `make_unreadable(run_dir, kind)` / `make_attempt_unwritable(run_dir)` / `concurrent_store`)を定義し、TC-port-run-store-001〜021 を1行1関数で実装する。TC-001 の「親を含めて attempt ディレクトリが作成された」は `prepare_attempt` の**前は false・後は true** という `attempt_dir_present` の反転と `expected_run_dir` との一致で、TC-002 の「既存の書き込み済みファイルの内容に影響しない」は再実行後に read 系が同じ値を返すことで主張する(`attempt_exists` は宣言しない。adr.md ADR-084)。`FsRunStore` を実装し、`serde` の private DTO で JSON を読み書きする(adr.md ADR-080)。書き込みは `util::atomic::write_atomic`、ディレクトリ作成は `util::fsdir::ensure_dir` を呼ぶだけにする。適用ファイルで一時ディレクトリのハーネスを組み、スキップ許容集合は `permission_restrictions_effective()` から実行時に決める(ADR-055)。並行読み取りのケースは `concurrent_store` 経由で書き、読み手の停止は `Drop` に載せる(ADR-063)。`HOOKS.md`(`crates/pulsen-conformance/HOOKS.md`)に21行分の対応表を足し、冒頭の集計と区分表・「環境で走らなくなりうる行」の表を更新する(`007` / `017` は権限操作に依存する C 区分)。`attempt_dir_present` の行には「`prepare_attempt` の前後で観測が反転することまで主張する」という使い方を書き添える — 定数を返すハーネスが緑にならない条件はフックの側にしか書けない。
- **理由:** runディレクトリの読み書きが「不在 / 破損 / 機構失敗」の3分類とアトミック性を満たすことは、手続きCの判断の前提そのもの。ここが崩れると tick が launching のまま滞留する。
- **消化するチェックリスト:** ADP-runstore-001〜004, 006〜010、TC-port-run-store-001〜021

### 7. WorktreeManager::create: 宣言・適合7件 + 追加1件・git 実装・ダブルの追随

- **対象ファイル:** `crates/pulsen-domain/src/execution/port.rs`、`crates/pulsen-conformance/src/{lib.rs,worktree_manager.rs}`、`crates/pulsen-conformance/src/doubles/worktree.rs`、`crates/pulsen-conformance/HOOKS.md`、`crates/pulsen/src/adapter/worktree.rs`、`crates/pulsen/tests/{conformance_worktree.rs,common/git.rs}`
- **変更内容:** `WorktreeManager::create` と `WorktreeError` を宣言し、`/// 契約:`(冪等性の境界)を付ける。同じステップで既存の実装2つを追う — `GitCliWorktreeManager::create` を上記の設計どおりに実装し、`ScriptedWorktreeManager` に `create` と `WorktreeManagerCall::Create` を足す。`WorktreeManagerHarness` に create 用のフック(未使用の `Workspace`、未作成の worktree_root、**登録なし・コミットの積まれた既存ブランチのみ**、**自タスクのパス・ブランチの登録は残るが実体が消えた(`prunable`)worktree**、worktree でない通常ディレクトリ、別ブランチの worktree、存在しない base ブランチ)を足し、TC-port-worktree-manager-010〜016 に adr.md ADR-085 由来の追加1件(`prunable` からの `add -f` による張り直しで、ブランチ先端が変わらず成果物が戻ること)を加えた**17件**を実装する。追加1件は台帳行に対応しないので、`HOOKS.md` の対応表では台帳行なしの追加ケースとして区別する。ハーネスの `worktree_root` はシンボリックリンク経由のパスにする。`common/git.rs` に worktree の登録状態(`prunable` を含む)とブランチ先端を確認するヘルパーを足す。`HOOKS.md` に7行分 + 追加1件を足し、冒頭の集計と区分表を更新する。
- **理由:** `create` の冪等性の境界(自タスクの残骸だけを達成済みとみなす)は、クラッシュ復旧(TC-exec-tick-051/052)の唯一の担保。git の実挙動で分岐を確定させる必要がある。宣言と2つの実装を分けないのは、トレイトにメソッドが増えた時点で既存 impl が壊れ、ワークスペースがビルドできなくなるため。
- **消化するチェックリスト:** DOM-execution-062, 066、ADP-worktree-004、TC-port-worktree-manager-010〜016

### 8. ProcessController: システム実装と `own_identity` / `run_agent` の適合13件

- **対象ファイル:** `crates/pulsen-conformance/src/{lib.rs,process_controller.rs}`、`crates/pulsen-conformance/HOOKS.md`、`crates/pulsen/src/adapter/process.rs`、`crates/pulsen/tests/conformance_process_controller.rs`、`crates/pulsen/examples/agent_probe.rs`
- **変更内容:** `ProcessControllerHarness` を定義し、`process_controller` を `identity_and_agent`(TC-004・005・017〜027 の13件)と `spawn`(TC-001〜003 の3件)の2モジュールに分けてケース関数を書く(adr.md ADR-083)。このステップでは `identity_and_agent` の13件を `tests/conformance_process_controller.rs` に**適用して通す**(`spawn` の適用はステップ11 が同じファイルに1行足す)。`ProcessControllerHarness` には `failing_identity_controller`(**存在しないパスを `identity_source` として構築した2つ目のコントローラ**)を置き、`TC-005` をこのフックで確定的に走らせる(adr.md ADR-076)。`SystemProcessController` を実装する — `new(self_exe, identity_source, clock)`、デタッチ起動・kill同定子・起動時刻の取得・`run_agent` の符号化(adr.md ADR-075 / ADR-076)。同定情報の取得は private 関数1つに閉じ、**戻り値を三値 `Result<Option<ObservedProcess>, Io>`(取得できた / 対象プロセスが不在 / 機構の失敗)にして** #3 の `starttime_of` が署名を変えずに同じ関数を使えるようにする。`own_identity` は `Ok(None)` を `Err(Io)` に畳む1行を持つ(畳むのは呼び出し側であって共有関数ではない)。結末ごとの写像は adr.md ADR-075 の表に従う — POSIX 非 Linux は「非0終了かつ stdout 空 = 不在」「起動失敗・その他の非0終了・exit 0 で空 = `Err(Io)`」、Linux は「procfs ルート不在 = `Err(Io)`」「ルートは在るが `<root>/<pid>/stat` が `NotFound` = 不在」。POSIX は `<identity_source> -o lstart=,pgid= -p <pid>` の1回の呼び出しで起動時刻と PGID を取り(`LC_ALL=C` / `TZ=UTC` を注入し `LANG` / `LC_TIME` / `LC_ALL` の継承を落とす。最終トークンが PGID、残りを trim したものが lstart)、Linux は `<procfs_root>/<pid>/stat` の「最後の `)` より後ろを空白で分割した20番目(全体の22番目 = starttime)」と「同3番目(全体の5番目 = pgrp)」を1回の読み取りから取り出す(comm に空白や `)` が入ると素朴な分割がずれる)。Linux の起動時刻は `<procfs_root>/sys/kernel/random/boot_id` と合成する(adr.md ADR-075)。アダプターのユニットテストに「異なるロケール・TZ を与えた2回の取得が等価な `ProcessStartTime` を返す」「`own_identity` の `kill_ident` が観測した PGID から作られている(新しいプロセスグループの長として起動した場合は `-<pid>` になる)」「**存在しない pid に対する共有関数の戻り値が `Ok(None)` であり `Err(Io)` に畳まれていない**」「**壊した取得元では `Ok(None)` ではなく `Err(Io)` になる**」を足す。後ろ2つは三値の区別そのものの主張で、#3 の `starttime_of` が乗る前提になる。シグナル死の具体値(`128+6`)の主張もここに置く(適合スイート側は非0に留める。adr.md ADR-082)。テスト用エージェント `examples/agent_probe.rs`(`exit` / `print` / `check-cwd` / `echo-args` / `sleep` / `abort`)を追加する(adr.md ADR-082)。`HOOKS.md` に16行分を足し、冒頭の集計と区分表・「環境で走らなくなりうる行」の表を更新する(`023` / `025` は権限操作、`024` はシグナル死を作れるプラットフォームに依存する。`005` は注入で確定的に走るのでこの表には入れない)。
- **理由:** OS 依存操作を `unsafe` なしで組めるかどうかがこのスライス最大の技術リスク。最初に実測で確かめ、破れたら計画(依存追加または lint 緩和)を見直す。
- **消化するチェックリスト:** ADP-process-005, 006、TC-port-process-controller-004, 005, 017〜027(AC-16 の worktree 不在時の符号化を含む)

### 9. RunWrapper ユースケース

- **対象ファイル:** `crates/pulsen/src/application/{mod.rs,run_wrapper.rs}`、`crates/pulsen-conformance/src/doubles/{mod.rs,run_store.rs,process.rs}`、`crates/pulsen/tests/run_wrapper.rs`
- **変更内容:** `RunWrapper` を実装する。あわせて `ScriptedRunStore` / `ScriptedProcessController`(結果列 + 呼び出し記録)をダブルに足す。ユースケーステストで、順序(starttime → pid → marker → run_agent → exit)、マーカーありでの未起動終了、`marker_exists` の `Err(Io)` でも未起動終了、`own_identity` / `write_starttime` / `write_pid_file` の失敗で何も書き残さないこと、`write_exit` の失敗でも `run_agent` の結果が変わらないこと、exit code(0 / 非0 / 126 / 127 / 128+n)がそのまま書かれること、ログのパスが `run_dir` の導出関数から取られることを検証する。
- **理由:** 「pid の後にマーカー確認」というラッパー側の順序は二重起動排除の片翼で、ダブルの呼び出し記録でしか主張できない。
- **消化するチェックリスト:** UC-execution-009、TC-exec-run-wrapper-001〜008, 010〜012, 016, 018, 021〜027

### 10. `wrapper` 隠しサブコマンドとラッパー用の合成

- **対象ファイル:** `crates/pulsen/src/cli/{args.rs,mod.rs,wrapper.rs,wire.rs,render.rs}`、`crates/pulsen/tests/{cli_wrapper.rs,cli_usage.rs}`
- **変更内容:** `Command::Wrapper` を `hide = true` で足し(adr.md ADR-077)、`cli/wrapper.rs` で引数を `RunDirPath` / `WorktreePath` / `CommandLine::rehydrate` に通して `RunWrapper` を実行する。`wire::compose_wrapper(run_dir)` は**ホームも config も読まず**、`RunDirPath::state_root` から `FsRunStore` を組み、`SystemProcessController` は `without_self_exe(IdentitySource::platform_default(), SystemClock::new())` で組む(`current_exe()` を読まない。adr.md ADR-076 / ADR-078)。`cli_usage.rs` を「ヘルプに `wrapper` が現れない」「`wrapper` は実行できる」の2主張に更新する。受け入れテストで、config.yaml が不在・破損した一時ホームでもラッパーが動くこと、起動引数が不正(相対パス・トークン0個・形式外の run_dir)なら runディレクトリに何も書かず非0で終わること、エージェントの exit code がそのまま `exit` ファイルに現れること、**`--workspace` に指定したディレクトリが存在しない状態で起動するとエージェントは実行されず `exit` に非0(126)が書かれること**(`TC-exec-run-wrapper-017` / AC-16 のラッパー経路での裏付け。ポート単位の `TC-port-process-controller-026` とは別に、実バイナリの経路で観測する)を検証する。
- **理由:** `spawn_wrapper` の適合ケース(ステップ11)と tick の起動経路(ステップ14)の両方が、実バイナリのラッパーモードを必要とする。
- **消化するチェックリスト:** PAGE-wrapper-001〜005、TC-exec-run-wrapper-009, 013〜015, 017, 019, 020

### 11. `spawn_wrapper` の適合3件とデタッチ性のフィクスチャ

- **対象ファイル:** `crates/pulsen/examples/spawn_probe.rs`、`crates/pulsen/tests/conformance_process_controller.rs`、`crates/pulsen-conformance/HOOKS.md`
- **変更内容:** ハーネスに `env!("CARGO_BIN_EXE_pulsen")` を `self_exe` として注入し(adr.md ADR-076)、`spawn` スイート(TC-port-process-controller-001〜003)の適用をステップ8 のファイルに1行足す。デタッチ性(TC-002)は `examples/spawn_probe.rs` を起動 → `wait` → runディレクトリに starttime / pid / exit が揃うまでの観測、で検証する。`SpawnError`(TC-003)は存在しないパスで構築した2つ目のコントローラ(`failing_controller`)で作る。`examples` が見つからない場合のスキップは**許容集合に入れない**(作り忘れが緑にならないようにする)。
- **理由:** ラッパーがツール本体の生存に依存せず完走することは requirements §4.1 の核で、in-process のテストでは表現できない。
- **消化するチェックリスト:** ADP-process-001、TC-port-process-controller-001〜003

### 12. テストダブルの拡張

- **対象ファイル:** `crates/pulsen-conformance/src/doubles/{mod.rs,task_repository.rs,clock.rs,tests.rs}`
- **変更内容:** `ScriptedTaskRepository` に `list_active` / `save` のスクリプトと保存された `Task` の記録を足す(現在は `create` 以外が `panic!`)。`ScriptedWorktreeManager` の `create` はステップ7 で足し済み(トレイトの宣言と同じステップで追う規則)なので、ここでは結果列の組み立てヘルパーだけを整える。`find` は本スライスに消費者がいないので足さない(#4 / #5 が必要になったときに足す)。猶予境界を作るための `SettableClock`(`now` を任意の `Timestamp` に置ける時計)を足す。ダブル自身のユニットテスト(`doubles/tests.rs`)を追記する。
- **理由:** tick の異常系・境界値は実アダプターでは外から状況を作れない(30秒の実時間待ち・I/O エラーの注入)。CLAUDE.md の「実アダプターを差し替えられることを設計の健全性の指標とする」をここで行使する。
- **消化するチェックリスト:**(直接対応する台帳行は無い。AC-17 の前提)

### 13. Tick: 走査と分岐の骨格・サマリー

- **対象ファイル:** `crates/pulsen/src/application/{mod.rs,tick/mod.rs}`、`crates/pulsen/tests/tick_scan.rs`
- **変更内容:** `Tick` の構造体・`TickOutcome` / `TickSummary` / `TickIssue` / `TickError` を定義し、処理フロー 1〜2・9 と分岐の網羅 `match` を実装する(adr.md ADR-101 / ADR-081)。本スライスで配線しないアームには引き取り先のスライスを why コメントとして残す。上限超過後の `frozen` 記録を private な集約点に置く(adr.md ADR-074)。ダブルに対するユースケーステストで、ロック競合の `Skipped`・`LockError::Failed` の `TickError`・`list_active` の Io で状態を変更せず `TickError`・`Corrupt` の報告のみ(書き込まない)・`SnapshotUnreadable` のスキップと報告・Wait ステータスの pending / failed で何も起きない・タスク0件・複数タスクの集約・1タスクの失敗で残りが続行することを検証する。
- **理由:** 走査と分岐は tick の骨格であり、後続スライスがアームを埋める土台になる。ここで exit code 規約(競合だけが 0)を確定させる。
- **消化するチェックリスト:** UC-execution-002(骨格と配線した4アーム)、PAGE-tick-002, 006、TC-exec-tick-002, 006, 012, 013, 015〜019, 027

### 14. Tick: 手続きA(起動)

- **対象ファイル:** `crates/pulsen/src/application/tick/launch.rs`、`crates/pulsen/tests/tick_launch.rs`
- **変更内容:** 手続きAを spec の順序どおりに実装する。ダブルに対するユースケーステストで、workspace 未確定 → 導出 → `create` → `confirm_workspace` → `save` の経路、workspace 確定済みで `create` を呼ばないこと、`create` 失敗の `record_tool_failure(WorktreeCreate)` と上限超過での `Stopped` + `frozen`、展開失敗の5経路(実効エージェント名なし / `config.agents` に不在 / `RawAgentDefinition::parse` 失敗 / `MissingSkillInput` / `ExpansionError`)がすべて `record_spawn_failure_in_place` に落ち実行状態が変わらないこと、上限の等号と +1、`record_launching` の attempt 採番と run_dir の記録、`prepare_attempt` 失敗と `spawn_wrapper` の同期エラーが状態を変更せず報告のみになること、spawn に渡る `WrapperLaunchSpec` の3値(ステータス上書きのエージェント定義・上書きモデル・当該タスクの worktree パス・`skill_input` 経由の `{input}`)を検証する。
- **理由:** ADR-016 の順序と「同期spawn失敗は状態を変えない」という分類は、この手続きの実装でしか守れない。
- **消化するチェックリスト:** UC-execution-003、TC-exec-tick-001, 004, 014, 028〜050, 055

### 15. Tick: 手続きC(spawn確認)

- **対象ファイル:** `crates/pulsen/src/application/tick/confirm_spawn.rs`、`crates/pulsen/tests/tick_confirm_spawn.rs`
- **変更内容:** 手続きCを spec の順序どおりに実装する。ダブルと `SettableClock` に対するユースケーステストで、`ConfirmRunning` での `confirm_running` + `save`、`KeepWaiting` で書き込みが1回も起きないこと、`SuspectSpawnFailure` での「マーカー書き込み → 再読 → 分類」の**順序**、`write_invalidation_marker` の `Err(Io)` で状態を変更せず報告してスキップすること、再読で pid が現れた場合に pending へ戻さず running へ取り込むこと、`record_spawn_failure` の上限超過で `Stopped` + `frozen`、`RunFileError`(Corrupt / Io)と `InconsistentRunFiles` の報告とスキップ、runディレクトリ不在(`Ok(None)`)が猶予経路に合流すること、猶予の境界(30秒 = `KeepWaiting` / 31秒 = 超過 / 巻き戻り = `KeepWaiting`)、`current_attempt` が None のときの不変条件違反の報告を検証する。
- **理由:** 「マーカー書き込み後に pid 再確認」は二重起動排除のもう片翼で、順序を主張するテストでしか担保できない。
- **消化するチェックリスト:** UC-execution-005、TC-exec-tick-007, 068〜086

### 16. ユースケース層の分岐網羅テストの仕上げ

- **対象ファイル:** `crates/pulsen/tests/{tick_scan.rs,tick_launch.rs,tick_confirm_spawn.rs,run_wrapper.rs}`
- **変更内容:** ステップ13〜15 で書いたテストを spec/testcases の行と突き合わせ、未消化の行を埋める。

  - `save` 失敗の扱い(`TickIssue::SaveFailed` で報告して次のタスクへ進む)
  - 冪等性 — 状態が変わらないタスク群に対して tick を連続実行しても書き込みが発生しないこと
  - spawn 失敗で pending 復帰したタスクが次の tick で新しい attempt 番号で再起動されること
  - `TC-exec-tick-051`: worktree 作成成功と `confirm_workspace` の `save` の間でクラッシュした状態(workspace 未記録のタスク + 既に存在する worktree)から次の tick を回し、同一 `Workspace` が再導出され `create` が冪等に成功すること
  - `TC-exec-tick-053`: launching 記録済みで `prepare_attempt` が済んでいない状態(runディレクトリ不在)から次の tick を回し、read 系の `Ok(None)` として猶予経路に合流すること
  - `TC-exec-tick-054`: `prepare_attempt` 済みで spawn していない状態(runディレクトリは空)から次の tick を回し、同じく猶予経路に合流すること

  いずれも「前 tick の途中状態をダブルの初期状態として組み、次の tick を1回回す」形で書く。テスト名を仕様の言葉で揃え、実装の内部構造(private 関数名・呼び出し回数の偶発的な性質)に依存していないことを確認する。
- **理由:** 冪等性(「毎回同じ判断が再導出され、書き込みが発生しない」)は個別の手続きではなく tick 全体の性質で、手続きを書き終えてからでないと主張できない。
- **消化するチェックリスト:** TC-exec-tick-046, 047, 051, 053, 054, 086(残り)、AC-17

### 17. `tick` サブコマンドと合成ルートの拡張・出力

- **対象ファイル:** `crates/pulsen/src/cli/{args.rs,mod.rs,tick.rs,wire.rs,render.rs}`、`crates/pulsen/tests/cli_usage.rs`
- **変更内容:** `Command::Tick` を足し、`wire::Runtime` に `runs` / `state_root()` / `worktree_root()` を足す(why をコードに添える。ADR-061)。`std::env::current_exe()` は `wire::process_controller()` の中だけで読み、`cli/tick.rs` からのみ呼ぶ — `compose` には載せず、ラッパー側も呼ばない(ステップ10 の `compose_wrapper` が `without_self_exe` を組む。adr.md ADR-076)。`cli/tick.rs` で `TickOutcome::Skipped` を 0、`Completed` をサマリー表示して 0、`TickError` を非0にする。`cli/render.rs` に tick サマリー(値の入っているフィールドのみ。処理対象なしの表示を含む)と `TickIssue` の文言を足す。`cli_usage.rs` を「ヘルプに現れるのは `add` / `tick` / `help`」に更新する。
- **理由:** exit code 規約(tick のロックスキップだけが例外的に 0)と、cron 運用でアラートにしないという要件がここで初めて観測可能になる。
- **消化するチェックリスト:** PAGE-tick-001, 004, 005、TC-exec-tick-012, 015〜017

### 18. 受け入れテストとクロスtickの結合確認

- **対象ファイル:** `crates/pulsen/tests/{cli_tick.rs,common/mod.rs,common/git.rs}`
- **変更内容:** `common/mod.rs` に tick 起動ビルダー(`add()` と同じく `HOME` / `USERPROFILE` を一時ディレクトリへ向ける。ADR-062。**作業ディレクトリも指定できるようにする**)と、`worktrees/` / `state/runs/<task-id>/attempt-<n>/` の確認ヘルパー、config.yaml のエージェント定義に `examples/agent_probe` を使うフィクスチャ、期限付きポーリングのヘルパー `wait_until(deadline, || 条件)` を足す。`wait_until` はタイムアウト時に「待っていた条件」と runディレクトリの一覧を添えて落とす(cron 実行時の調査可能性と同じ理由)。既定の期限は30秒・ポーリング間隔は50ms とする — 負荷の高い CI でも `spawn` からファイル出現までが収まる余裕を取り、値を1箇所に置いて flaky の再発時に動かす起点にする。ラッパーはデタッチ起動で非同期に完了するため、runディレクトリの内容を読む前と次の tick を打つ前は必ずこのヘルパーを挟む。**待ち条件はこれから観測する成果物そのものに立てる** — ログを読むならログの出現を、`exit` を読むなら `exit` の出現を待つ。ラッパーの書き込み順序は starttime → pid →(マーカー確認)→ ログ生成 → exit なので、pid の出現はログや `exit` の存在を含意せず、pid だけを待って直後にログを assert すると負荷の高い環境で落ちる(既定の期限を動かしても直らない種類の失敗になる)。この規則を `wait_until` の doc に書き、テストを増やす後続スライスにも伝える。あわせて**タスクファイルを直に組み立てる / 差し替えるヘルパー**(failed 状態のタスク・スナップショットのみ破損したタスク・`state/archive/<task-id>.json` への直置き・workspace 未確定 / 確定済みのタスク)と、リポジトリに**コミットを積んだブランチだけを置くヘルパー**(`common/git.rs`)を足す — これらは本スライスの CLI(`add` / `tick` / `wrapper`)だけでは作れない状態で、無いとテストごとに場当たりの JSON 組み立てが散らばる。滞留させたラッパーを使うケースは `agent_probe sleep` の長さを短く取り、**テストを終える前に `exit` の出現を `wait_until` で待つ** — 一時ホーム(`TempDir`)の削除と孫プロセスの書き込みが競合すると、`write_atomic` がディレクトリを作り直して削除済みのホームを部分的に復活させる。受け入れテストで次を検証する。
  - `add` → `tick`: worktree(`worktrees/<task-id>`)とブランチ(`pulsen/<task-id>`)が base から作られ、タスクファイルが `launching` になり `recorded_at` と run_dir が記録され、runディレクトリに starttime / pid / ログが現れる(**ログを読む前にログの出現を** `wait_until` で待つ。AC-15 の F2 / F4 / F6)
  - 続けて `tick`: `running` へ取り込まれ、`current_attempt.process` に pid・kill同定子・starttime が入り `spawn_fail_count` が 0 になる
  - `state/archive/<task-id>.json` に直接置いたタスクファイルは走査されない(worktree も runディレクトリも作られず、サマリーにも現れない。PAGE-tick-008)
  - `agent_probe sleep` を起動したまま次の tick を打つと、ロック競合にならずに処理が進む(ラッパーがロックFDを継承していないことの観測)
  - 作業ディレクトリを**対象リポジトリの外**(一時ディレクトリ)に向けた `tick` が、同じ結果になる(外部スケジューラーからの起動。AC-12)
  - failed からの再起動で attempt 番号が増え、runディレクトリのパスが変わり、worktree は同一のまま内容が引き継がれる
  - **ブランチのみ残存(登録も実体も無い)からの張り直し**: workspace 未確定のタスクファイルを直に組み、リポジトリには `pulsen/<task-id>` ブランチだけをコミットを積んだ状態で置いてから `tick` を回すと、`create` が `-f` なしで worktree を張り直して起動が続き、**ブランチ先端が変わらず**積まれたコミットの成果物が worktree に残る(`TC-exec-tick-052`。ADR-085 の「登録なし・ブランチのみ」分岐が tick 経路で効くことの唯一の裏付け)
  - **進行中の worktree 消失**: workspace 確定済みのタスクの `worktrees/<task-id>` を消してから `tick` を回すと、`create` は呼ばれずにそのまま spawn され、runディレクトリの `exit` に非0が現れる(tick 側に新しい分岐が生じない。`PAGE-tick-009` / AC-16。前項とは workspace が確定済みか否かで前提が分かれる)
  - エージェント定義を壊した状態での tick: `spawn_fail_count` が増え、実行状態もタスクステータスも変わらず、runディレクトリが作られない。config を直すと次の tick で起動に成功する
  - タスク0件 / `state/tasks/` 未作成 / パース不能タスクファイルの混在 / スナップショットのみ破損 / ロック競合(`examples/lock_holder` で保持)で tick が全体失敗しない(exit 0)
  - 同一リポジトリの2タスクが別々の worktree・ブランチ・runディレクトリで並行して起動される
- **理由:** ポート単位・ユースケース単位で緑でも、合成ルートの結線とクロスtickの引き継ぎが壊れていれば主経路は動かない。Issue の「検証」欄の項目はここで機械的に裏付ける。
- **消化するチェックリスト:** UC-flow-002, 004, 006、PAGE-tick-008, 009、TC-exec-tick-001, 013, 019, 027, 036, 052, 055, 086(受け入れ側の裏付け)

### 19. 手動確認・チェックリスト記帳・ADR の昇格判定

- **対象ファイル:**(コード変更なし)`.thread/2/adr.md`、`.adr/073-*.md` 以降、Issue #2 のコメント
- **変更内容:** plan.md「テスト方針」の手動確認の表に沿って `spec/manual-tests/` を実行する。適合スイートのスキップ行と、スライス境界により部分消化になった行(plan.md「チェックリスト行にチェックを付ける基準」の2つの表)を Issue のコメントにまとめ、**実際にチェックが付いた行数を確定させる**(計画時点の上限194行から、当日スキップになった行を引いた数)。確認した実行環境(OS・root か否か・TMPDIR の位置・`git` のバージョン)を明記する。`.thread/2/adr.md` の各エントリをプロジェクト全体に効くかどうかで選別し、昇格するものを `.adr/065` 以降として起票する。spec との食い違い(`CommandLine` の生成経路が2つになること、`RunDirPath` の逆写像、tick の `errors` の型)を spec 追従の提起として Issue にコメントする。
- **理由:** Issue の完了条件が「実装をレビューで確認できた行にのみチェックを付ける。見送る行は理由をコメントに残す」であり、記帳と提起までを実装作業の一部として閉じる。
- **消化するチェックリスト:**(記帳そのもの。AC-18)
