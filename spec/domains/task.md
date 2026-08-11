# Task

タスク集約 — スケジューラーの帳簿。タスクステータス(ユーザー定義)・実行状態(固定6値)・カウンタ・スナップショット・実行メタデータの参照を持ち、すべての状態遷移を「self を消費して新しい値を返す」純粋関数として定義する。tick / CLI はこのドメインの遷移関数の呼び出しとして表現され、判断(遷移の決定)と副作用(プロセス・ファイル操作)は分離される。

仕様の根拠: requirements §4.1・§5・§6(全節)・§8・§9・§10、ADR-001・002・004・008・009・012・014・015・016。

## ユビキタス言語

| 英語名 | 日本語名 | 定義 |
|---|---|---|
| Task | タスク | ワークフローのインスタンス。1タスク = 1worktree。集約ルート |
| TaskStatus | タスクステータス | ユーザーがYAMLで定義した状態(`StatusName`)。ツールが変更するのは completed による遷移と手動遷移のみ |
| ExecutionState | 実行状態 | ツールが固定管理する6状態(pending / launching / running / completed / failed / stopped) |
| Target | 対象 | タスクが作用するリポジトリとベースブランチ |
| Workspace | ワークスペース | タスクに割り当てられた worktree のパスとブランチ。最初の実行時に確定し以降不変 |
| AttemptRef | 現在attempt参照 | 現在(最新)のattempt番号・runディレクトリパス・プロセス同定情報。launching記録でのみ置き換わる |
| RetryCounters | カウンタ | attempt_count / judge_attempt_count / spawn_fail_count。いずれも連続失敗を数える(ADR-009) |
| StopReason | 凍結要因 | stopped に至った4経路のいずれか |
| FailureNote | 直近の失敗要因 | ツール操作の失敗(worktree作成・削除、アーカイブ移動、spawn失敗)および判定失敗の記録 |
| DegradedTask | スナップショット破損タスク | タスクファイルは読めるがスナップショットフィールドが読めないタスク。許される操作が型で制限される |

## 値オブジェクト

### TaskId

- フィールド: `String`
- バリデーション: 1〜64文字。使える文字は `[a-z0-9-]`、先頭は英数字。ファイル名の主部・gitブランチ名の構成要素として常に安全な集合に制限する
- 生成: `parse(s: String) -> Result<Self, TaskIdError>`。発行は `TaskIdGenerator` ポート
- エラー型: `TaskIdError = Empty | TooLong | InvalidChar { char, position } | InvalidLeadingChar`
- 等価性: 文字列の完全一致

### RepoPath / WorktreePath

- フィールド: `PathBuf`
- バリデーション: **絶対パスであること**(tick は外部スケジューラーから任意のカレントディレクトリで起動されるため、相対パスを帳簿に載せない。add が登録前に絶対化する)
- 等価性: パスの完全一致(正規化はアダプター・ユースケースの責務)

### BranchName

- フィールド: `String`
- バリデーション: 非空。空白・制御文字を含まない。先頭が `-` でない。`..` を含まない。`/` 始まり・終わりでない。`.lock` で終わらない(git 参照名として有効な実用サブセット)
- 等価性: 文字列の完全一致

### Workspace

- フィールド: `path: WorktreePath`、`branch: BranchName`
- 等価性: 両フィールドの一致

### Timestamp

- フィールド: UTC の日時(秒精度)
- 生成: `Clock` ポート、またはRFC3339文字列のパース(タスクファイルの直列化表現もRFC3339)
- 振る舞い: 全順序比較、`elapsed_since(&self, earlier) -> DurationSpec相当の秒数`
- 等価性: 時刻の一致

### AttemptNumber

- フィールド: `u32`
- バリデーション: 1 以上
- 振る舞い: `next(&self) -> AttemptNumber`(+1)。launching記録のたびに採番され、過去の番号は再利用しない(単調増加。requirements §4.1)

### StateRoot / WorktreeRoot

- いずれも `PathBuf` の newtype。バリデーション: 絶対パスであること
- `StateRoot` はグローバルホーム配下の `state/`、`WorktreeRoot` は `worktrees/` を指す(値の解決はアプリケーション層の配線)
- 決定的なパス導出(RunDirPath / TaskFilePath / WorkspacePlanner)の起点として使い、生の `Path` をドメインの導出関数に渡さない

### RunDirPath

- フィールド: `PathBuf`(絶対パス)
- 生成: `derive(state_root: &StateRoot, id: &TaskId, n: AttemptNumber) -> Self` = `<state_root>/runs/<task-id>/attempt-<n>`。決定的導出だが、requirements §4.1・§9 に従いタスクファイルにも記録する(人間が直接辿れるようにする)
- 等価性: パスの一致

### TaskFilePath

- 決定的導出関数(レイアウトの単一の定義箇所。アダプターも同じ導出を使い、show の「スナップショット保存先パス」表示(ADR-015 によりタスクファイル自体のパス)・monitoring.md の直接閲覧の導線をポートの外にレイアウト知識を漏らさず満たす):
  - `active(state_root: &StateRoot, id: &TaskId) -> PathBuf` = `<state_root>/tasks/<task-id>.json`
  - `archived(state_root: &StateRoot, id: &TaskId) -> PathBuf` = `<state_root>/archive/<task-id>.json`

### プロセス同定系

- `Pid(u32)`
- `KillIdent(String)` — 非空。プラットフォーム実装が定義する不透明なkill同定子(POSIX: プロセスグループID、Windows: ジョブオブジェクト名等。requirements §4.3)。等価性は文字列一致
- `ProcessStartTime(String)` — 非空。OSから同一の取得手段で得た起動時刻の不透明表現。**等価比較のみ**に使い、可搬な意味・順序を持たせない(requirements §4.3)
- `StartTimeRecord { ident: ProcessStartTime, wall: Timestamp }` — starttimeファイル・タスクファイルに記録される起動時刻。`ident` はPID再利用照合(§6.2)に、`wall` はtimeout起点(§6.1)に使う。§6.1 は「記録済みstarttimeを起点にtimeoutを判定する」と定めるが、照合用の値は不透明で時間演算できないため、ラッパーが書き込み時点の壁時計時刻を併記する(why: 2つの要求 — 同定の不透明性と経過時間の計測 — を1つの値では満たせない)
- `ProcessIdent { pid: Pid, kill_ident: KillIdent, starttime: StartTimeRecord }` — running への遷移時にタスクファイルへ取り込まれる同定情報一式。以降の kill はこの値だけで実行できる(requirements §4.3)

### AttemptRef

- フィールド: `number: AttemptNumber`、`run_dir: RunDirPath`、`process: Option<ProcessIdent>`
- 生成: `record_launching` の内部でのみ生成される(`run_dir` は `number` から `RunDirPath::derive` で導出され、番号とパスの整合が構成で保証される)
- 意味: 現在(最新)のattemptへの参照。launching記録で丸ごと新しい値(`process: None`)に置き換わり、`confirm_running` で `process` が取り込まれる。**それ以外のどの遷移でも書き換えられない** — 実行状態が failed / completed / stopped / pending に変わっても保持され、gc保護規則の決定性(requirements §9.2)と、stopped 後の孤児プロセス調査(monitoring.md「タスクファイルのPID・starttimeを手がかりに」)・show の実行メタデータ表示(requirements §9 の属性列挙)を成立させる

### RetryCounters

- フィールド: `attempt_count: u32`、`judge_attempt_count: u32`、`spawn_fail_count: u32`(初期値すべて 0)
- 意味: いずれも**連続した失敗**の数(ADR-009)。リセット規則は遷移関数の事後条件として定義する(下表)
- 等価性: 3フィールドの一致

### FailureNote

- フィールド: `kind: FailureKind`、`message: String`(非空)、`at: Timestamp`
- `FailureKind = WorktreeCreate | WorktreeRemove | ArchiveMove | SpawnFail | JudgeFail`
- 意味: 直近のツール操作の失敗(requirements §9)および判定失敗の記録。`JudgeFail` は requirements §9 の列挙にはないが、§8「凍結に至った要因はタスクファイルに記録し、参照可能にする」を JudgeLimitExceeded 経路で満たすために含める(プロトコル外の exit code・判定timeout・起動不能のどれかを message で判別できる)。エージェント実行自体の失敗はここに記録しない(証跡はrunディレクトリの exit / ログにある)。上書きは新しい失敗の発生時のみで、成功時にクリアしない(「直近の」失敗要因)

### StopReason

- 直和型: `RetryLimitExceeded | JudgeLimitExceeded | SpawnFailLimitExceeded | Aborted`
- stopped に至る4経路(requirements §6.4)と1対1に対応する

### ExecutionStateKind

- 直和型: `Pending | Launching | Running | Completed | Failed | Stopped`(データなしの判別子)
- 生成: `parse(s: &str) -> Result<Self, StateKindError>` — `ls --state` の入力検証(小文字の6値のみ受理)
- エラー型: `StateKindError = Unknown { given: String, valid: [&'static str; 6] }`(有効値一覧を案内に使う。pages ls)
- `ExecutionState::kind()` で状態から導出する

## エンティティ

### Task(集約ルート)

#### フィールド

| フィールド | 型 | 制約 |
|---|---|---|
| `id` | `TaskId` | 不変 |
| `workflow_name` | `WorkflowName` | 不変。表示名(WorkflowRef の規則で決定) |
| `target` | `Target { repo: RepoPath, base_branch: BranchName }` | 不変 |
| `snapshot` | `WorkflowSnapshot` | 不変(ADR-015。タスクファイルに埋め込み) |
| `task_status` | `StatusName` | `snapshot.statuses` に存在すること |
| `execution` | `ExecutionState` | 下記の直和型 |
| `workspace` | `Option<Workspace>` | None = 未確定(最初の起動前)。一度 Some になったら不変(requirements §5) |
| `current_attempt` | `Option<AttemptRef>` | launching記録でのみ置き換わる。他のどの遷移でもクリアしない(値オブジェクト AttemptRef を参照) |
| `counters` | `RetryCounters` | |
| `last_failure` | `Option<FailureNote>` | |
| `updated_at` | `Timestamp` | すべての遷移で現在時刻に更新 |

#### ExecutionState(直和型)

```
ExecutionState =
  Pending
| Launching { recorded_at: Timestamp }        // 起動記録済み・spawn確認未了。recorded_at は猶予時間の起点
| Running                                     // 起動確認済み・分類未了(同定情報は current_attempt.process)
| Completed                                   // 判定確定。次tickが next へ遷移させる
| Failed                                      // 失敗確定。次tickが再試行する(ツール操作の失敗を含む。ADR-012)
| Stopped   { reason: StopReason, notified_at: Option<Timestamp> }
```

- `Launching.recorded_at`: 猶予時間(30秒・組み込み定数)の判定はクラッシュを跨いでも成立する必要があるため、起動記録の時刻を永続化する
- `Stopped.notified_at`: stopped の記録は常に `notified_at: None` で行う(過去の凍結の通知記録を引き継がない。requirements §8)

#### 不変条件

1. `task_status` は常に `snapshot` に定義されたステータスである
2. `Launching` / `Running` / `Completed` のとき `current_attempt` は `Some` である
3. `Running` のとき `current_attempt.process` は `Some` である(`confirm_running` で取り込まれ、以降クリアされない)
4. `task_status` が `AgentRun` のステータスで `Launching` 以降に進むとき、`workspace` は `Some` である(worktree確定後にのみ起動する)
5. attempt番号は単調増加する(`current_attempt` がクリアされないことにより、次番号 = 現番号 + 1 が常に導出できる)
6. `workspace` は一度確定したら変更されない
7. `Running` / `Completed` になった直後の `spawn_fail_count` は 0 である(起動確認でリセット)
8. `Stopped` 以外の状態は `notified_at` を持たない(型で表現済み)

検証の境界: 不変条件 1(および snapshot 自体の構造)は TaskRepository のアダプターがデコード時に検証し、破れは `SnapshotUnreadable` として返す(`Intact(Task)` に対しては常に成立する — `current_status_def` の全域シグネチャの根拠)。不変条件 2〜4 は手動修復で破られたままデコードを通り得るため、遷移関数が前提として検査し、崩れていれば `TransitionError::InvariantViolated` を返す(tick はそのタスクをスキップして報告し、修復は人間に委ねる)。パニックは「遷移関数自身が事後条件を破った」場合(プログラミングエラー)に限る。

#### ライフサイクル(実行状態の遷移)

```
                    ┌──────────────(spawn失敗: 猶予超過)──────────────┐
                    ▼                                                  │
 register ─→ Pending ──(launching記録)──→ Launching ──(pid取込)──→ Running
                ▲ ▲                                                    │
                │ │                      ┌───(判定: completed)──→ Completed ──(advance: next へ)──→ Pending
                │ │                      │                                                          (タスクステータス変更)
                │ └──(判定: skipped)─────┤
                │                        ├───(判定: failed / timeout kill / プロセス死亡)──→ Failed ──(次tickで再起動)
                │                        │                                                    │
                └────(retry / set-status)┤                                                    │
                                         ▼                                                    ▼
                                      Stopped ←──(各上限超過 / abort)─────────────────────────┘
```

- `Failed` からの再起動経路は `Pending` と同じ扱い(launching記録から始まる。ADR-012)
- ツール操作の失敗(worktree作成・削除、アーカイブ移動)は `Pending` / `Failed` から `Failed` へ(ADR-012)
- 同期spawn失敗(テンプレート展開失敗)は状態を変えない(`Pending` / `Failed` のまま。ADR-016)
- すべての「上限超過」は加算後の値が上限を**超えた**(`count > limit`)ときに成立する(例: リトライ上限2 = 初回 + 2回のリトライがすべて失敗したら凍結)

#### 振る舞い(遷移関数)

すべて `self` を消費して新しい `Task` を返す。`now: Timestamp` を受け取り `updated_at` を更新する。前提状態を満たさない呼び出しは `Err(TransitionError)`。

| メソッド | シグネチャ | 前提状態 | 処理内容・事後条件 |
|---|---|---|---|
| `register` | `(id, workflow_name, target, snapshot, now) -> Task` | (生成) | `task_status = snapshot.initial`、`Pending`、workspace / attempt / failure = None、カウンタ全0 |
| `rehydrate` | `(全フィールド) -> Result<Task, RehydrateError>` | (永続化からの再構築) | TaskRepository アダプターがデコード時に使う唯一の再構築経路。不変条件1(`task_status ∈ snapshot.statuses`)を検証し、破れは `RehydrateError::StatusNotInSnapshot`(アダプターはこれを `SnapshotUnreadable` に写像する)。スナップショットは `WorkflowSnapshot::rehydrate` で再構築する(config との再突き合わせなし) |
| `confirm_workspace` | `(self, ws: Workspace, now) -> Result<Task>` | `workspace = None` | workspace を確定する。既に Some ならエラー(再確定しない) |
| `record_launching` | `(self, state_root: &StateRoot, now) -> Result<(Task, RunDirPath)>` | `Pending \| Failed`、AgentRun ステータス、workspace 確定済み | 次番号を採番し、run_dir を内部で `RunDirPath::derive(state_root, id, 次番号)` により導出して `current_attempt = Some({次番号, run_dir, process: None})`、`Launching { recorded_at: now }`。導出済みの run_dir を返す(呼び出し側はこれで `prepare_attempt` / spawn を行う)。番号とパスの食い違いを構成で排除する |
| `confirm_running` | `(self, process: ProcessIdent, now) -> Result<Task>` | `Launching` | `Running`、`current_attempt.process = Some(process)`、`spawn_fail_count = 0` |
| `record_spawn_failure` | `(self, message, spawn_fail_limit: u32, now) -> Result<Task>` | `Launching` | 猶予超過の非同期経路。`spawn_fail_count += 1`。超過なら `Stopped { SpawnFailLimitExceeded }`、でなければ `Pending`。`last_failure = SpawnFail` |
| `record_spawn_failure_in_place` | `(self, message, spawn_fail_limit: u32, now) -> Result<Task>` | `Pending \| Failed` | テンプレート展開失敗の同期経路(ADR-016)。`spawn_fail_count += 1`。超過なら `Stopped { SpawnFailLimitExceeded }`、でなければ状態不変。`last_failure = SpawnFail`。attempt採番なし |
| `complete_run` | `(self, now) -> Result<Task>` | `Running` | 判定 completed。`Completed`、`attempt_count = 0`、`judge_attempt_count = 0` |
| `skip_run` | `(self, now) -> Result<Task>` | `Running` | 判定 skipped(ADR-008)。`Pending`(タスクステータス不変)、`attempt_count = 0`、`judge_attempt_count = 0` |
| `fail_run` | `(self, retry_limit: u32, now) -> Result<Task>` | `Running` | 判定 failed / timeout kill / プロセス死亡。`attempt_count += 1`、`judge_attempt_count = 0`。超過なら `Stopped { RetryLimitExceeded }`、でなければ `Failed` |
| `record_judge_failure` | `(self, detail: String, judge_attempt_limit: u32, now) -> Result<Task>` | `Running` | 判定不能。`judge_attempt_count += 1`、`last_failure = JudgeFail { detail }`(プロトコル外の code・判定timeout・起動不能の別を残す)。超過なら `Stopped { JudgeLimitExceeded }`、でなければ `Running` のまま(次tickで再判定) |
| `record_tool_failure` | `(self, kind, message, retry_limit: u32, now) -> Result<Task>` | `Pending \| Failed` | worktree作成・削除、アーカイブ移動の失敗(ADR-012)。`attempt_count += 1`、`last_failure` 更新。超過なら `Stopped { RetryLimitExceeded }`、でなければ `Failed` |
| `advance` | `(self, now) -> Result<Task>` | `Completed`、現ステータスが AgentRun | `task_status = 現ステータスの next`、`Pending` |
| `abort` | `(self, now) -> Result<Task>` | `Stopped` 以外 | `Stopped { Aborted, notified_at: None }`(kill の成否確認はユースケースの責務。kill失敗時はこのメソッドを呼ばない) |
| `mark_notified` | `(self, now) -> Result<Task>` | `Stopped { notified_at: None }` | `notified_at = Some(now)` |
| `retry` | `(self, now) -> Result<Task, RetryError>` | `Stopped` | `Pending`、カウンタ全0。それ以外の状態は `RetryError::NotStopped(kind)`(CLI の案内文言の分岐に使う) |
| `set_status` | `(self, status: StatusName, now) -> Result<Task, SetStatusError>` | `Launching / Running` 以外 | `status ∈ snapshot` を検証(`UnknownStatus { defined }`)。`task_status = status`、`Pending`、カウンタ全0。`Launching / Running` は `SetStatusError::Active`(「先にabort」の案内) |

#### 問い合わせ(読み取り)

| メソッド | 戻り値 | 内容 |
|---|---|---|
| `execution_kind` | `ExecutionStateKind` | 実行状態の判別子 |
| `current_status_def` | `&StatusDefinition` | `snapshot` から現ステータスの定義を引く(不変条件1はアダプターのデコード検証で保証されるため常に存在) |
| `next_attempt_number` | `AttemptNumber` | `current_attempt` の次番号(None なら 1) |
| `is_agent_run / is_wait / is_cleanup` | `bool` | 現ステータスの動作種別 |
| `applicable_retry_limit` | `Option<u32>` | show の上限併記(ADR-014): AgentRun = `effective_retry_limit`、Cleanup = 2、Wait = None |

#### エラー型

```
TransitionError =
  InvalidState { expected: &'static str, actual: ExecutionStateKind }  // 前提状態の不一致
| WorkspaceAlreadySet                                                  // confirm_workspace の再確定
| WorkspaceNotSet                                                      // workspace 未確定での record_launching
| NotAgentRunStatus { status: StatusName }                             // AgentRun 前提の遷移を Wait / Cleanup で呼んだ(advance / record_launching)
| InvariantViolated { message: String }                                // 手動修復で破られた不変条件2〜4(Running なのに attempt / process が None 等)

RetryError     = NotStopped { actual: ExecutionStateKind }             // pages retry の案内文言の分岐に使う
SetStatusError = Active { actual: ExecutionStateKind }                 // launching / running。「先に abort せよ」の案内
               | UnknownStatus { given: StatusName, defined: Vec<StatusName> }
```

### DegradedTask(スナップショット破損タスク)

タスクファイル自体は読めるが、埋め込まれたスナップショットフィールドが欠落・不正なタスク(手動修復の失敗等。ADR-015)。pages 縮退表 ※5〜※7 の挙動 — tick / set-status は定義に依存するため不可、abort / retry / show / ls は可 — を、**スナップショットに依存する操作を持たない型**として表現する。

- フィールド: `Task` から `snapshot` を除いたすべて + `snapshot_error: String`(読めない理由)
- 振る舞い(いずれもスナップショット非依存):

| メソッド | シグネチャ | 内容 |
|---|---|---|
| `abort` | `(self, now) -> Result<DegradedTask, TransitionError>` | Task と同じ規則(`Stopped` 以外 → `Stopped { Aborted }`) |
| `retry` | `(self, now) -> Result<DegradedTask, RetryError>` | Task と同じ規則。受理されるが tick には拾われないため、CLI が修復の必要を警告する(pages ※7) |
| `mark_notified` | `(self, now) -> Result<DegradedTask, TransitionError>` | Task と同じ規則 |
| 読み取り各種 | | `execution_kind` / 各フィールドの参照(show の注記付き表示・ls の検出報告に使う) |

- `set_status` / `advance` / `record_launching` / `current_status_def` / `applicable_retry_limit` は**定義しない**(スナップショットが必要な操作は型上呼べない)
- 不変条件: Task の不変条件のうち 1(`task_status` の snapshot 所属)と 4(AgentRun での workspace 確定 — 動作種別の判定にスナップショットが要る)は検証不能のため課さない。それ以外(2〜3・5〜8)は同じ
- **stopped の再通知は tick の責務に含まれる**: `notified_at` のない `Stopped` の DegradedTask に対して、tick はスナップショット破損を理由にスキップせず再通知を行う(通知に必要な `TASK_ID` / `WORKFLOW` / `TASK_STATUS` はすべてスナップショット非依存のフィールドから得られる)。pages 縮退表の「tick スキップ」は定義に依存する判断(起動・遷移・終端処理)に限る。これを欠くと「DegradedTask を abort → notify_cmd 失敗」の後に再通知が永遠に行われず、requirements §8 の at-least-once が破れる

## ドメインサービス

### WorkspacePlanner

- 責務: タスクIDからワークスペース(worktreeパス・ブランチ名)を決定的に導出する(requirements §5)
- メソッド: `derive(worktree_root: &WorktreeRoot, id: &TaskId) -> Workspace`
  - `path = <worktree_root>/<task-id>`、`branch = pulsen/<task-id>`(requirements §5)
  - TaskId の文字集合制約により、導出結果は常に有効なパス・ブランチ名になる
- 依存ポート: なし(純粋)

## ポート

### TaskRepository

- 目的: タスクファイル(`state/tasks/<task-id>.json`、アーカイブは `state/archive/<task-id>.json`)の読み書き・走査・アーカイブ移動
- メソッドとエラー:

| メソッド | シグネチャ | エラー |
|---|---|---|
| `create` | `(task: &Task) -> Result<(), CreateError>` | `Conflict`(同IDが現役・アーカイブのいずれかに存在)、`Io(message)` |
| `save` | `(task: &Task) -> Result<(), SaveError>` | `NotFound`(現役に存在しない)、`Io` |
| `save_degraded` | `(task: &DegradedTask) -> Result<(), SaveError>` | 同上 |
| `find` | `(id: &TaskId) -> Result<TaskLookup, ReadError>` | `Io` |
| `list_active` | `() -> Result<Vec<TaskEntry>, Io>` | (個別破損はエラーにしない。下記) |
| `list_archived` | `() -> Result<Vec<TaskEntry>, Io>` | 同上 |
| `archive` | `(id: &TaskId) -> Result<(), ArchiveError>` | `NotFound`、`Io`(移動先の作成不可・権限等) |

- 読み取りの結果型(スナップショットのみ破損したタスクを、ファイル全体の破損と区別して表現する):

  ```
  TaskRecord = Intact(Task) | SnapshotUnreadable(DegradedTask)
  TaskLookup = Active(TaskRecord) | Archived(TaskRecord) | NotFound
             | Corrupt { path: PathBuf, message: String }        // ファイル全体が読めない
  TaskEntry  = Record(TaskRecord) | Corrupt { path: PathBuf, message: String }
  ```

- 契約:
  - **解決順**: `find` は tasks → archive の順で探索する(pages 共通事項)
  - **一意性**: `create` はID衝突をポートが担保し `Conflict` を返す(呼び出し側の事前確認に依存しない)
  - **原子性・可視性**: `save` / `archive` の部分的な結果(書きかけの内容、移動の中間状態)は読み手から観測されない。`archive` 後、対象は `list_active` / `find` の現役側から即座に消え、アーカイブ側に現れる(read-your-writes)
  - **並行性**: 書き込みメソッドの呼び出し側は排他ロック(ExclusiveLock)の取得を前提とする。ポートは並行書き込みを調停しない。読み取りはロックなしで常に一貫した内容を返す(原子性の帰結)
  - **破損への書き込み禁止**: `Corrupt` と報告したファイルに対して書き込みメソッドを呼んではならない(呼び出し側の責務。requirements §9)。`SnapshotUnreadable` は例外で、`save_degraded` のみ許される
  - **破損スナップショットの温存**: `save_degraded` は、ファイル内のスナップショットフィールドを**元の内容のまま**書き戻す(読めない部分を消さず、修復の材料を保存する)。往復可能性はアダプターの責務
  - **ディレクトリの不在**: 走査対象ディレクトリ(`state/tasks/` / `state/archive/`)の不在は空結果として扱う(`find` は `NotFound`)。走査はタスクファイルの命名形式(`<task-id>.json`)に合致するエントリのみを対象とし、形式外のエントリ(アトミック置換の一時ファイル残骸・手動で置かれたファイル等)は列挙せず触れない(RunStore の `attempt-<n>` 形式外の扱いと同じ規則)。書き込みメソッド(`create` / `save` / `save_degraded` / `archive`)は必要なディレクトリを自動作成する(`state/` 配下はツールが管理する領域のため。pages ※3)
  - **クエリ要件**: 走査は全件読み込み(数十〜数百タスクの規模。requirements §9)。絞り込み・並び順はユースケース側で行う。ページングなし
  - **デコード**: 直列化形式(人間可読なJSON)と検証はアダプター境界の責務。スナップショットを含む全フィールドを往復可能に保存する。デコード時に検証する範囲: (a) タスク側フィールドの構文・値制約 — 破れは `Corrupt`、(b) スナップショットの構文と構造不変条件(`initial ∈ statuses`・`next ∈ statuses`)、および `task_status ∈ snapshot.statuses` の照合 — 破れは `SnapshotUnreadable`(message に理由)。これにより `Intact(Task)` は不変条件1を常に満たす。不変条件2〜4(状態間の整合)はデコードでは検証せず、遷移関数の前提検査(`InvariantViolated`)に委ねる

### TaskIdGenerator

- 目的: タスクIDの発行
- メソッド: `generate(&self) -> TaskId`
- 契約: 呼び出しごとに実用上衝突しないIDを返す(時刻成分 + ランダム成分等。形式はアダプターに委ねるが `TaskId` の文字集合制約を満たすこと)。厳密な一意性は `TaskRepository::create` の `Conflict` がバックストップとなるため要求しない(ユースケースは Conflict 時に再発行してよい)

### Clock

- 目的: 現在時刻の取得(判断の入力を注入可能にし、時刻依存ロジックを純粋関数としてテスト可能に保つ)
- メソッド: `now(&self) -> Timestamp`
- 契約: 単調性は要求しない(壁時計。マシン再起動・時刻合わせで巻き戻り得る。猶予時間・timeout の判定は巻き戻りで過大評価しない — 経過が負なら 0 として扱う)

## ユースケース(概要)

- add: register → create(検証・スナップショットは Definition ドメイン)
- tick: 実行状態ごとの遷移関数の呼び出し(起動記録・分類反映・遷移・凍結・通知記録)
- abort / retry / set-status: 対応する遷移関数 + save
- ls / show: list_active / list_archived / find と問い合わせメソッド
