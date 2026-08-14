# 実装手順 — Issue #3

## 設計

**この Issue は spec-to-issues が起票した縦スライスであり、設計は spec に確定済みである。** 本節は新しい設計を導出せず、チェックリスト各行の「定義場所」を実装の順序に並べ替えて指し示す。判断に迷ったら以下の spec を直接読む。

### モジュールの増分

```
crates/pulsen-domain/src/
  execution/value.rs           + JudgeOutcome / JudgeConclusion / CommandCompletion
  execution/running.rs (新規)   Aliveness / RunningDecision / AliveDecision
                               IdentityCheck / RunningClassifier
  execution/judgement.rs (新規) JudgementService
  execution/notification.rs (新規) NotificationService / NotifyOutcome(NOTIFY_TIMEOUT を含む)
  execution/port.rs            + ProcessController::{starttime_of, kill, try_kill_remnants}
                               + RemnantOutcome / KillError / CommandRunner
  task/task.rs                 + complete_run / skip_run / fail_run / record_judge_failure
                                 / advance / mark_notified
  task/counters.rs             + pub(super) のリセット関数
  task/degraded.rs             + mark_notified
  task/transition.rs           + TransitionError::AlreadyNotified
crates/pulsen/src/
  adapter/process.rs           + starttime_of / kill / try_kill_remnants / TerminatorSource(OS 依存はここだけ)
  adapter/command_runner.rs (新規) SystemCommandRunner
  application/tick/observe.rs (新規) UC-execution-006(手続きD)
  application/tick/notify.rs (新規)  UC-execution-001(共通手続き notify)
  application/tick/mod.rs      + Observe / Advance / Notify / AlreadyNotified アームの配線
                                 SnapshotUnreadable の再通知、TickSummary に judged
                                 CommandRunner のジェネリック引数、TickIssue の新分類
  cli/wire.rs                  + CommandRunner の構築
  cli/render.rs                + judged / transitioned / skipped_back / notified の表示と新 TickIssue の文言
  examples/judge_probe.rs (新規) 判定・通知コマンドのプローブ
crates/pulsen-conformance/
  src/process_controller.rs    + observation スイート(TC-006〜016 の11件)
  src/command_runner.rs (新規)  TC-port-command-runner-001〜016
  src/lib.rs                   + CommandRunnerHarness、ProcessControllerHarness にフック追加
  src/doubles/mod.rs           + RecordSeq(ダブルをまたぐ順序の採番)
  src/doubles/process.rs       + starttime_of / kill / try_kill_remnants のスクリプト
                               + calls_in_order(採番は try_kill_remnants にだけ載る)
  src/doubles/run_store.rs     + with_read_exit / RunStoreCall::ReadExit
  src/doubles/task_repository.rs + saved_in_order / saved_degraded_in_order
  src/doubles/command_runner.rs (新規) ScriptedCommandRunner(calls_in_order を持つ)
  HOOKS.md                     + CommandRunner 16行の節、ProcessController を16行 → 27行
crates/pulsen/tests/
  conformance_process_controller.rs + observation スイートの適用
  conformance_command_runner.rs (新規)
  tick_observe.rs / tick_notify.rs (新規。ダブルに対するユースケーステスト)
  tick_scan.rs                 未配線アームの主張を差し替え
  cli_tick.rs                  + 判定・遷移・凍結・通知の受け入れ
  tick_fixture/mod.rs          + running / completed / stopped / degraded のタスク組み立て
```

### ドメインモデルへの影響

`spec/domains/execution.md`(値オブジェクト・ドメインサービス・ポート)と `spec/domains/task.md`(振る舞い遷移関数・エラー型・DegradedTask)の記述を**そのまま**実装する。既存の `LaunchingClassifier` / `ExitCode` / `PidFileContent` と同じ流儀(依存ポートなしの純粋サービス、`self` を消費する遷移関数、分類だけを持つエラー型)に揃える。

| 要素 | 定義場所 | 台帳 |
|---|---|---|
| `JudgeOutcome` / `JudgeConclusion` | `spec/domains/execution.md#judgeoutcome-judgeconclusion` | DOM-execution-004 / 005 |
| `RunningDecision` / `AliveDecision` / `Aliveness` | `spec/domains/execution.md#分類の決定直和型` | DOM-execution-008 / 009 |
| `CommandCompletion` | `spec/domains/execution.md#commandcompletion` | DOM-execution-010 |
| `IdentityCheck::check` | `spec/domains/execution.md#identitycheck` | DOM-execution-012 |
| `RunningClassifier::classify_alive` | `spec/domains/execution.md#runningclassifier` | DOM-execution-017 |
| `JudgementService`(+ 3関数) | `spec/domains/execution.md#judgementservice` | DOM-execution-018〜021 |
| `NotificationService`(+ `notify_env` / `interpret_notify_completion` / `NOTIFY_TIMEOUT`) | `spec/domains/execution.md#notificationservice` | DOM-execution-022 / 071 |
| `Task` の遷移6種 | `spec/domains/task.md#振る舞い遷移関数` | DOM-task-038〜041 / 043 / 045 |
| `TransitionError` | `spec/domains/task.md#エラー型` | DOM-task-053(実装済み5種の確認 + `AlreadyNotified` の追加で6種) |
| `DegradedTask::mark_notified` | `spec/domains/task.md#degradedtaskスナップショット破損タスク` | DOM-task-059 |

`RunningClassifier` の**2段規則**(exit が Some なら生存観測を行わず即 `Judge`、exit が None のときだけ観測して `classify_alive`)は、`classify_alive` の返り値を `RunningDecision` から `Judge` を除いた3値 `AliveDecision` にすることで型に表れる(adr.md ADR-009)。`RunningDecision` は4値のまま走査全体の分類として残し、`From<AliveDecision>` で持ち上げる — doc コメントと `unreachable!()` で担保すると、規則が守られていることを型が何も述べない。分類の1段目はユースケース側(手続きD)にある。

### ポート

`spec/domains/execution.md#processcontroller` / `#commandrunner` のポート表と1:1で足す。**本スライスで使わないメソッドは宣言しない**(Issue #1 で確立した規約。`RunStore::{attempt_exists, list_runs, delete_attempt, remove_task_dir_if_empty}` と `WorktreeManager::remove` は引き続き宣言しない)。

| 追加 | シグネチャ | 台帳 |
|---|---|---|
| `ProcessController::starttime_of` | `(pid: Pid) -> Result<Option<ProcessStartTime>, Io>` | DOM-execution-049 |
| `ProcessController::kill` | `(ident: &KillIdent) -> Result<(), KillError>` | DOM-execution-050 |
| `ProcessController::try_kill_remnants` | `(ident: &KillIdent) -> RemnantOutcome` | DOM-execution-051 |
| `RemnantOutcome` | `Killed \| NotIdentifiable \| Failed { message }` | DOM-execution-056 |
| `KillError` | `Failed { message }` | DOM-execution-058 |
| `CommandRunner::run` | `(cmd: &PlainCommand, env: &[(String, String)], timeout: Option<&DurationSpec>) -> CommandCompletion` | DOM-execution-067 |

各トレイトには既存の `RunStore` / `ProcessController` と同じ `/// 契約:` の箇条書きを付ける — `starttime_of` の三値と「機構失敗を死亡に写像しない」、`kill` の呼び出し前提(`IdentityCheck` が `Alive`)と「失敗時は呼び出し側が状態を変更しない」、`try_kill_remnants` の「誤殺なく同定できる場合に限る」、`CommandRunner` のシェル非経由・環境の継承と上書き・cwd 非規定・timeout・標準出力非捕捉・同期実行。

### ユースケース / アプリケーションロジック

`spec/usecases/execution.md` の**共通手続き notify**(UC-execution-001)と**手続きD**(UC-execution-006)を、書かれている順序どおりに実装する。`spec/pages/index.md#縮退状態の共通規則` ※5(PAGE-tick-007)が、スナップショット破損タスクに対して再通知**だけ**を行い、それ以外はスキップして報告することを定める。

手続きを配線するアームは4つ。凍結タスクの分岐は**通知済みかどうかで型を分ける** — 「何もしない」を通知アームの中の条件分岐にすると、分岐の網羅からその経路が消える。

| アーム | 手続き | 定義場所 |
|---|---|---|
| `Branch::Observe`(Running) | 手続きD | `spec/usecases/execution.md#手続きd-観測判定running` |
| `Branch::Advance`(Completed) | `task.advance(now)` → `save`。`TransitionError` は報告してスキップ | 同 処理フロー 6 |
| `Branch::Notify`(未通知の Stopped) | 共通手続き notify | 同 処理フロー 7 |
| `Branch::AlreadyNotified`(通知済みの Stopped) | 何もしない | 同 処理フロー 7 |
| `TaskRecord::SnapshotUnreadable` | `Stopped { notified_at: None }` なら共通手続き notify、それ以外は従来どおり報告 | 同 処理フロー 2 |

さらに `Tick::commit` の `Freeze::Frozen` の枝に共通手続き notify を差し込む(`.thread/2/adr.md` ADR-074 が「#3 がこの関数の中身を埋める」と予告した地点)。凍結かどうかは呼び出し側が渡す(ADR-097)ので、通知アームからの保存は `Freeze::NotFrozen` を通す。

### アダプター / 外部連携

`spec/testcases/ports/process-controller.md`(TC-006〜016)と `spec/testcases/ports/command-runner.md`(全16行)が期待結果の正本。OS 依存の分岐は `adapter/process.rs` と新設の `adapter/command_runner.rs` の2ファイルに閉じる(CLAUDE.md 技術方針・AC-7)。

- `starttime_of` は既存の三値関数 `identity::observe(source, pid) -> Result<Option<ObservedProcess>, Io>`(ADR-075 / ADR-076)に委譲し、`starttime` だけを取り出す。**`Ok(None)` を `Err(Io)` に畳まない**。
- `kill` / `try_kill_remnants` は `KillIdent` だけを入力に実行単位を終了させる。実装手段と `NotIdentifiable` の判定は adr.md ADR-002 / ADR-015 で確定させる — ADR-015 が ADR-002 の3点(同定子は境界で parse して組み直す / 成否は消滅の観測で決める / 昇格の段)を置き換えており、現行の決定はこの2つを合わせて読む。
- `CommandRunner` の timeout(std の安全 API だけで期限つきに待つ)は adr.md ADR-001 で確定させる。

### CLI / プレゼンテーション

`cli/render.rs` に `judged` / `transitioned` / `skipped_back` / `notified` の表示を足す。サマリー DTO は #2 の時点で10フィールド(spec の9 + `confirmed_running`)あり、そのうち3つの値の入る経路が本スライスで初めてできる。加えて `complete_run` の書き込みを受けるフィールドが spec に無いため `judged` を1つ足して11フィールドにする — 「書き込みを行った tick は必ずサマリーに現れる」(ADR-092)を主経路の exit 0 で成立させるにはこれが要る(adr.md ADR-005)。表示上は「判定確定」として「起動確認」と「遷移」の間に置く。新しい `TickIssue` の分類にも文言を足す(ADR-081 の「文言は CLI 層」)。`cli/wire.rs` は `CommandRunner` のアダプターを構築して `Tick::new` へ渡す。

## 実装ステップ

依存方向の順(domain → port → adapter → usecase → cli → 受け入れ)に並べる。各ステップは単体でレビュー可能で、テストが通る状態で終える。適合ケースは対応するアダプターと同じステップに置き、「契約を書く → 実装が通す」でステップが閉じるようにする。

**既に実装のあるトレイトにメソッドを足すステップは、その時点の全実装(アダプターとテストダブル)を同じステップで追う**(#2 で確立した規則)。`ProcessController` が該当し、宣言・`SystemProcessController` の実装・`ScriptedProcessController` の追随・適合11件をステップ5〜6 に分けずまとめる余地があるが、`kill` の実装は技術リスクが大きいため宣言(ステップ5)と実装・適合(ステップ6)を分け、ステップ5 ではダブル側の追随までを行ってワークスペースをビルド可能に保つ。

### 1. execution ドメイン: 判定・生存分類の値オブジェクト

- **対象ファイル:** `crates/pulsen-domain/src/execution/{value.rs,running.rs,mod.rs}`
- **変更内容:** `JudgeOutcome`(3値)・`JudgeConclusion`(2種)・`CommandCompletion`(3種)を `value.rs` に、`Aliveness`(2値)・`RunningDecision`(4値)・`AliveDecision`(`RunningDecision` から `Judge` を除いた3値)を新設の `running.rs` に置く。`mod.rs` から再公開する。ユニットテストは等価性と網羅 `match` が書けることを主張する程度に留め、意味づけは次ステップ以降のサービスのテストで消化する。あわせて既存の `ExitCode::is_success` が `DOM-execution-002` の PASS 要件(0 かどうかの判定)を満たすことを確認する(**新規実装は不要**)。
- **理由:** 以降のサービス・ポート・ユースケースがすべてこの語彙の上に乗る。`RunningDecision` と `AliveDecision` を先に固定しておくと、`classify_alive` が `Judge` を返さない(2段規則)ことが返り値の型で表現できる。
- **消化するチェックリスト:** DOM-execution-004, 005, 008, 009, 010(+ DOM-execution-002 は確認のみ)
- **参照:** `spec/domains/execution.md#judgeoutcome-judgeconclusion` / `#分類の決定直和型` / `#commandcompletion`

### 2. execution ドメイン: IdentityCheck と RunningClassifier

- **対象ファイル:** `crates/pulsen-domain/src/execution/running.rs`
- **変更内容:** `IdentityCheck::check(observed: Option<&ProcessStartTime>, recorded: &ProcessStartTime) -> Aliveness` と `RunningClassifier::classify_alive(aliveness, started_wall, timeout, now) -> AliveDecision` を実装する。いずれも依存ポートを持たない純粋サービス。経過は `Timestamp::elapsed_since`(巻き戻りは0に飽和)で測り、起点は `starttime.wall`。`TimeoutSpec::Unlimited` はどれだけ経過しても `KeepRunning`。ユニットテストで全分岐と境界を網羅する — `None` → Dead / 不一致 → Dead / 一致 → Alive、`Alive` × 未超過(等号を含む)→ `KeepRunning`、`Alive` × 超過 → `KillOnTimeout`、`Alive` × `Unlimited` → `KeepRunning`、`Dead` → `DiedWithoutExit`、経過が負 → `KeepRunning`。
- **理由:** timeout の境界と PID 再利用の判定を I/O なしのユニットテストで固定する。ここが緩むと、生存プロセスの Dead 誤判定 → failed → 再起動 → 同一 worktree での並走に直結する。
- **消化するチェックリスト:** DOM-execution-012, 017(TC-exec-tick-107〜111 / 117 の境界もここで裏付ける)
- **参照:** `spec/domains/execution.md#identitycheck` / `#runningclassifier`、`spec/testcases/execution/tick.md#境界値-3`

### 3. execution ドメイン: JudgementService と NotificationService

- **対象ファイル:** `crates/pulsen-domain/src/execution/{judgement.rs,notification.rs,mod.rs}`
- **変更内容:** `JudgementService`(`default_judgement` / `interpret_judge_completion` / `judge_env`)と `NotificationService`(`notify_env` / `interpret_notify_completion` / `const NOTIFY_TIMEOUT`(60秒))を実装する。`judge_env` は `TASK_ID` / `WORKSPACE` / `EXIT_CODE`(10進文字列)/ `RUN_DIR`、`notify_env` は `TASK_ID` / `WORKFLOW` / `TASK_STATUS`。通知の成否解釈もドメインに置き、結末を `NotifyOutcome`(`Delivered` / `Failed { detail }`)で返す — stopped を書く経路がすべてこの規則を共有するので、ユースケース側に散らすと at-least-once の破れが片方の経路にだけ入る。ユニットテストで、0 / 非0(20 を含む)のデフォルト判定、`Exited(0/10/20/その他)` / `TimedOut` / `FailedToStart` の解釈、`JudgeFailure.detail` と `NotifyOutcome::Failed.detail` が3つの原因を判別できること、env の変数名と値の形式を検証する。
- **理由:** 判定プロトコルの4値解釈(ADR-008)と通知の組み込み timeout(ADR-018)は spec が決めきっている定数であり、ユースケースを書く前にドメイン側で固定する。デフォルト判定が2値であること(`Skipped` を返さない)を型で示すと、`exit 20` を skipped と取り違える実装が書けなくなる。
- **消化するチェックリスト:** DOM-execution-018, 019, 020, 021, 022, 071(TC-exec-tick-089〜092 / 096〜098 / 120 の意味づけもここで裏付ける)
- **参照:** `spec/domains/execution.md#judgementservice` / `#notificationservice`、`.adr/008-skipped-judgement-outcome.md`、`.adr/018-notify-cmd-timeout.md`

### 4. task ドメイン: 判定・遷移・通知の遷移関数

- **対象ファイル:** `crates/pulsen-domain/src/task/{task.rs,counters.rs,degraded.rs,transition.rs}`
- **変更内容:** `complete_run` / `skip_run` / `fail_run` / `record_judge_failure` / `advance` / `mark_notified` を実装し、`DegradedTask::mark_notified` を同じ規則で足す。カウンタのリセットは `RetryCounters` の `pub(super)` 関数に集約し、公開 API を増やさない。上限超過の判定(`count > limit`)は #2 が置いた private ヘルパーを再利用する。ユニットテストで、前提状態の不一致(6状態それぞれからの呼び出し)、`complete_run` / `skip_run` の `attempt_count` / `judge_attempt_count` リセットと `spawn_fail_count` の保持、`skip_run` のタスクステータス不変、`fail_run` の `attempt_count += 1` かつ `judge_attempt_count = 0`、`record_judge_failure` の `Running` 維持と `last_failure = JudgeFail`、上限の等号(凍結しない)と +1(凍結する)、`retries: 0` での即凍結、`advance` の `next` 参照と非 AgentRun での `NotAgentRunStatus`、`mark_notified` の `Stopped { notified_at: None }` 以外からの拒否を検証する。`TransitionError` は #2 が置いた5種が `DOM-task-053` の PASS 要件(5種を区別する)を満たすことを確認したうえで、**`AlreadyNotified` を1種足して6種にする** — 通知済みの凍結への再通知は「前提状態の不一致」ではなく「達成済み」の拒否で、`InvalidState` に畳むと `Stopped` を期待しながら `Stopped` を拒む説明のつかない報告になる(`WorkspaceAlreadySet` と同じ形。adr.md ADR-006)。あわせて前提検査を遷移関数に載せる — `ensure_running` は `Running` に加えて `current_attempt.process` が Some であること(不変条件2・3)を、`ensure_completed` は `Completed` に加えて `current_attempt` が Some であること(不変条件5 の導出元)を検査し、どちらの破れも `MissingCurrentAttempt` で返す。手動修復で破られた不変条件はデコードを通るので(`adapter/task_file.rs` の分類表)、結末を確定させる遷移がここで止める。
- **理由:** 判断を純粋関数として確定させ、ユースケースを配線に徹させる。カウンタのリセット規則(ADR-009)を事後条件として1箇所に固定する。
- **消化するチェックリスト:** DOM-task-038, 039, 040, 041, 043, 045, 059(+ DOM-task-053 は5種の確認と `AlreadyNotified` の追加)。TC-exec-tick-112〜116 / 121 / 122 の境界もここで裏付ける
- **参照:** `spec/domains/task.md#振る舞い遷移関数` / `#エラー型` / `#degradedtaskスナップショット破損タスク`、`.adr/009-counters-track-consecutive-failures.md`

### 5. ポートの追加とテストダブルの追随

- **対象ファイル:** `crates/pulsen-domain/src/execution/{port.rs,mod.rs}`、`crates/pulsen-conformance/src/doubles/{mod.rs,process.rs,run_store.rs,task_repository.rs,command_runner.rs,tests.rs}`
- **変更内容:** `ProcessController` に `starttime_of` / `kill` / `try_kill_remnants` を足し、`RemnantOutcome` / `KillError` を定義する。`CommandRunner` トレイトを新設する。各メソッドに `/// 契約:` の箇条書きを付ける(上記「ポート」の節の内容)。同じステップで既存の実装を追う — `SystemProcessController` には**この時点では最小の実装**を置かず、ステップ6 でまとめて実装するとワークスペースが壊れるため、宣言と同時にステップ6 の内容へ進めるようコミットを分ける場合はダブル側を先に揃える。`ScriptedProcessController` に `with_starttime_of` / `with_kill` / `with_try_kill_remnants` と対応する `ProcessControllerCall` の変種を、`ScriptedRunStore` に `with_read_exit` と `RunStoreCall::ReadExit` を足す(現在の `read_exit` はパニックする)。`ScriptedCommandRunner`(結果列 + 呼び出し記録。`cmd` / `env` / `timeout` を記録する)を新設する。あわせて**ダブルをまたぐ順序を主張できるようにする**(adr.md ADR-014)— プロセス全体で単調な採番(`RecordSeq`)を、**順序の契約がポートをまたぐメソッドだけ**に掛ける。本スライスでは4つで、`TaskRepository::save` / `TaskRepository::save_degraded` / `CommandRunner::run` / `ProcessController::try_kill_remnants` に対して `saved_in_order()` / `saved_degraded_in_order()` / `ScriptedCommandRunner::calls_in_order()` / `ScriptedProcessController::calls_in_order()` を足す。ダブルごとの記録は別々の列なので、採番なしでは「stopped の `save` → `run` → `mark_notified` の `save`」も「`try_kill_remnants` → `fail_run` の `save`」も主張できない(at-least-once と残存の後始末はこの順序でしか成立しない)。`starttime_of` / `kill` には掛けない — 主張するのが「呼ばれない」「失敗しても書かない」だけで、またぐ順序を持たない。`ScriptedProcessController` は採番を `Option<RecordSeq>` として持ち、`calls_in_order()` は採番のある呼び出しだけを返す。既存アクセサ(`saved()` / `saved_degraded()` / `calls()`)のシグネチャは変えず、順序を要るテストだけが新しいアクセサを使う。`doubles/tests.rs` にダブル自身のテストを追記する。
- **理由:** ドメインが外界に要求する操作をここで固定し、以降のアダプター・ユースケースが同じ形に乗る。ダブルを先に揃えることで、ステップ6・7 のアダプター実装とステップ9・10 のユースケーステストを並行して書ける。
- **消化するチェックリスト:** DOM-execution-049, 050, 051, 056, 058, 067
- **参照:** `spec/domains/execution.md#processcontroller` / `#commandrunner`、`.adr/028-usecase-error-paths-via-test-doubles.md`

### 6. ProcessController アダプター: 観測・終了の3メソッドと適合11件

- **対象ファイル:** `crates/pulsen/src/adapter/process.rs`、`crates/pulsen-conformance/src/{lib.rs,process_controller.rs}`、`crates/pulsen-conformance/HOOKS.md`、`crates/pulsen/tests/conformance_process_controller.rs`
- **変更内容:** `SystemProcessController` に3メソッドを実装する。`starttime_of` は既存の `identity::observe` に委譲して `starttime` を取り出すだけにし、**`Ok(None)` を `Err(Io)` に畳まない**(#2 のアダプターユニットテストが既に三値の区別を主張しているので、その上に乗る)。`kill` / `try_kill_remnants` は adr.md ADR-002 / ADR-015 の決定に従って実装し(ADR-015 が置き換えた3点 — 同定子は境界で parse して組み直す / 成否は消滅の観測で決める / 昇格の段 — を含む)、OS 依存分岐はこのファイルに閉じる。適合スイートに `observation` モジュール(TC-port-process-controller-006〜016 の11件)を足し、`ProcessControllerHarness` にフックを5つ追加する — 終了を確認済みのプロセスの pid(`terminated_pid`。回収まで済ませないとゾンビとして観測され前提が成立しない)、生存中の実行単位(`live_execution_unit`)、spawn 元が終了済みの実行単位(`detached_execution_unit`)、ラッパーだけが死んだ実行単位(`orphaned_execution_unit`)、終了操作が失敗するコントローラ(`failing_terminator_controller`)。同定手段が失われた状態は #2 の `failing_identity_controller` をそのまま使う。**終了操作の実体(`TerminatorSource`)も差し替えられるようにする**(adr.md ADR-007。ADR-076 の取得元と同じ手で、既定はプラットフォームの実体、差し替えはテストのための builder に限る)— 存在しない実体を差し込めば「終了操作自体が失敗する」状況が権限操作に依存せず確定的に作れ、TC-013 / 016 が環境まかせのスキップにならない。期待結果は契約の語彙で書き、プラットフォーム固有の機構名に踏み込まない(ADR-082)。冒頭のドキュメントコメントの**「8行」を「11行」に訂正**し、`HOOKS.md` の ProcessController の節を16行 → 27行に、冒頭の集計・区分表・「環境で走らなくなりうる行」の表を更新する。同表に足すのは3行 — フィクスチャの実行ファイルを要する 007 / 011〜016(**スキップ許容集合には入れない**。作り忘れを緑にしないため)、実行単位を要する 011 / 012 / 013 / 015、その一部だけの終了を要する 014 / 016。注入で確定的に走る 005 / 010 は載せない。3ランナーの実測列は `未測定` で置く(実測は CI を回して初めて得られる)。適用先(`crates/pulsen/tests/conformance_process_controller.rs`)のスキップ許容集合は、`cfg!(unix)` のようなプラットフォーム述語ではなく**実行時の能力 probe** で決める — 実行単位を1回起こしてみて「起こせて一部だけ終了させられる / 起こせるが一部だけは終了させられない / 起こせない / フィクスチャの実行ファイルが無い」の4つを区別し、最後の1つは許容集合に入れない(adr.md ADR-013。ADR-073 の基準どおり、原因も回避方法も一意なものはスキップにしない)。
- **理由:** `kill` を `unsafe` なし・依存追加なしで組めるかどうかが本スライスの技術リスクの片方。実測で先に確かめ、破れたら計画を見直す。誤殺しないことは適合スイートの `NotIdentifiable` ケースでしか主張できない。
- **消化するチェックリスト:** ADP-process-002, 003, 004、TC-port-process-controller-006〜016
- **参照:** `spec/testcases/ports/process-controller.md`、`spec/domains/execution.md#processcontroller`、`.adr/075-process-controller-without-unsafe.md`、`.adr/076-process-controller-injects-self-exe-and-identity-source.md`

### 7. CommandRunner アダプターと適合16件

- **対象ファイル:** `crates/pulsen/src/adapter/command_runner.rs`、`crates/pulsen/src/adapter/mod.rs`、`crates/pulsen-conformance/src/{lib.rs,command_runner.rs}`、`crates/pulsen-conformance/HOOKS.md`、`crates/pulsen/tests/conformance_command_runner.rs`、`crates/pulsen/examples/judge_probe.rs`
- **変更内容:** `SystemCommandRunner` を実装する — シェル非経由の直接起動、呼び出しプロセスの環境の継承と `env` による追加・上書き、cwd の非変更、`timeout` 指定時の期限つき待機(adr.md ADR-001)と超過時のプロセス終了、起動不能の `FailedToStart`、シグナル死等の非0符号化(`run_agent` と同じ `encode` を共有する)、標準出力・標準エラーの非捕捉、同期実行。適合スイート `command_runner`(TC-port-command-runner-001〜016)を1行1関数で置き、`CommandRunnerHarness` を定義する。検査用コマンドは `examples/judge_probe.rs` として供給し(ADR-082)、引数のリテラル一致・環境変数の値・cwd の一致・完了の証跡の書き出し・標準出力への既知文字列を exit code またはファイルで表現する。`HOOKS.md` に CommandRunner の節(16行 / A 0・B 15・C 1)を新設し、冒頭の対象ポート数・総行数・区分別件数表を更新する。「環境で走らなくなりうる行」に足すのは2行 — `004`(実行権限のない実体を作れない。判定は `permission_restrictions_effective`)と、`examples/judge_probe` を要する `001 / 002 / 005〜016`(**スキップ許容集合には入れない**。作り忘れを緑にしないため。`003` は `missing_command`、`004` は `non_executable_command` で組むのでこの実行ファイルを要さない)。`005`(外部からの強制終了)は前提を作れない環境が無いため載せない — 期待が「非0の符号化値」までで、シグナル死になるかを問うフックが無い(ADR-082)。
- **理由:** timeout を追加依存なしで組めるかどうかが技術リスクのもう片方。判定と通知の両方がこのポートに乗るため、ここで契約を閉じておかないとステップ9・10 の異常系が書けない。
- **消化するチェックリスト:** ADP-commandrunner-001、TC-port-command-runner-001〜016
- **参照:** `spec/testcases/ports/command-runner.md`、`spec/domains/execution.md#commandrunner`、`.adr/082-agent-and-spawn-probes-as-examples.md`

### 8. Tick への CommandRunner の結線と TickIssue の拡張

- **対象ファイル:** `crates/pulsen/src/application/tick/mod.rs`、`crates/pulsen/src/cli/{wire.rs,tick.rs,render.rs}`、`crates/pulsen/tests/{tick_scan.rs,tick_launch.rs,tick_confirm_spawn.rs,tick_fixture/mod.rs}`
- **変更内容:** `Tick` に7つ目のジェネリック引数 `C: CommandRunner` を足し、`cli::wire` で `SystemCommandRunner` を構築して `cli/tick.rs` から渡す(構築が失敗しうるかは実装で決まる。失敗しないなら `compose` に載せてよく、`current_exe()` のような外部リソースの読み取りを伴うなら ADR-076 / ADR-099 に倣って `tick` の経路でだけ呼ぶ)。`TickIssue` に本スライスで必要な分類を8つ足す — 不変条件3の破れ(`MissingProcessIdent`。不変条件2 とは修復の入口が違うので畳まない。adr.md ADR-004)、生存観測の機構失敗(`ObservationFailed`)、`kill` の失敗(`KillFailed`)、残存終了の報告(`RemnantsUnhandled`)、判定失敗の記録(`JudgeFailed`)、**実行の失敗の記録(`RunFailed`)**、判定コマンドへ渡す workspace の未確定(`MissingWorkspace`。不変条件4の破れ。遷移関数を一度も呼んでいないので `TransitionError::WorkspaceNotSet` を自作せず分けて出す。adr.md ADR-008)、通知の失敗(`NotifyFailed`)。`RunFailed` と `RemnantsUnhandled` はメッセージ文字列ではなく分類(「誰が失敗と判断したか」/「同定できたか」)を持たせ、文言は `cli/render.rs` 側の網羅 `match` で組む(adr.md ADR-010)— 判定コマンドが失敗と判定した場合、エージェントの終了コードが 0 でも失敗になるため、文字列に畳むと根拠の主体が読めなくなる。`cli/render.rs` に `judged` / `transitioned` / `skipped_back` / `notified` の表示と新分類の文言を足し、各分類を「失敗を記録」「起動の結果が未確定」「スキップ」「後始末が残っている」の4見出しのどれに振り分けるかを網羅 `match` で決める(`RunFailed` は「失敗を記録」、`RemnantsUnhandled` は単独で「後始末が残っている」。adr.md ADR-017)。既存のユースケーステスト・受け入れテストのフィクスチャを新しい型引数に追随させる。
- **理由:** ジェネリック引数の追加はワークスペース全体に波及するため、手続きの中身を書く前に1ステップで通す。ここを手続きと同じステップに混ぜると、差分のどこが配線でどこが判断か読めなくなる。
- **消化するチェックリスト:**(直接対応する台帳行は無い。ステップ9・10 の前提)

### 9. 共通手続き notify と走査レベルの残りアーム

- **対象ファイル:** `crates/pulsen/src/application/tick/{notify.rs,mod.rs}`、`crates/pulsen/tests/tick_notify.rs`
- **変更内容:** 共通手続き notify を spec の順序どおりに実装する — `config.notify_cmd` が None なら何もしない(`notified_at` も書かない)→ `NotificationService::notify_env` → `CommandRunner::run(notify_cmd, env, NOTIFY_TIMEOUT)` → `NotificationService::interpret_notify_completion` → `Delivered` のときだけ `mark_notified(now)` → `save` / `save_degraded` → `notified` に記録(adr.md ADR-011)。`Failed { detail }`(非0 / `TimedOut` / `FailedToStart`)は `TickIssue::NotifyFailed` として報告するだけで、`notified_at` を書かない。`Task` と `DegradedTask` の両方から呼べる形にする(adr.md ADR-003)。この手続きを3箇所から呼ぶ — (a) `Tick::commit` の `Freeze::Frozen` の枝、(b) `Branch::Notify` アーム(通知アームの保存は `Freeze::NotFrozen`。`Branch::AlreadyNotified` は何もしない)、(c) `TaskRecord::SnapshotUnreadable` かつ `Stopped { notified_at: None }`。`SnapshotUnreadable` の報告そのものは実行状態によらず必ず積み、そのうえで未通知の凍結だけが notify へ進む(adr.md ADR-012)— 報告を通知で置き換えると、notify_cmd 未定義の既定構成では破損したタスクがサマリーから消える。あわせて `Branch::Advance` アーム(`task.advance(now)` → `save` → `transitioned` に記録。`TransitionError` は報告してスキップ)を配線する。ダブルに対するユースケーステストで、通知の順序(stopped の `save` → `run` → `mark_notified` の `save`)、notify_cmd 未定義での無通知かつ `notified_at` 未記録、`notified_at` 記録済みへの再通知なし、非0 / `TimedOut` / `FailedToStart` での `notified_at` 未記録、`mark_notified` 後の `save` 失敗、クラッシュ相当の中間状態(未通知 stopped)からの再導出、catch-up(notify_cmd を後から定義)、DegradedTask への再通知と `save_degraded`、stopped を離脱したタスクへの無通知、同一 tick 内での凍結 → 通知、completed の `advance` と `TransitionError` のスキップを検証する。
- **理由:** notify は stopped を書く3経路と2つの検出経路が共有する唯一の手続きで、`Tick::commit` に置き場所が予約されている(`.thread/2/adr.md` ADR-074)。at-least-once の担保は「書く → 実行 → 追記」の順序でしか成立しない。
- **消化するチェックリスト:** UC-execution-001、PAGE-tick-007、TC-exec-tick-009, 010, 011, 021, 026, 147〜159
- **参照:** `spec/usecases/execution.md#共通手続き-凍結の確定と通知notify` および処理フロー 2 / 6 / 7、`spec/pages/index.md#縮退状態の共通規則`(※5)、`spec/testcases/execution/tick.md#正常系-6` / `#異常系-6` / `#エッジケース-6`、`.adr/097-freeze-is-passed-by-the-caller-of-the-transition.md`

### 10. 手続きD: 観測・判定(Running)

- **対象ファイル:** `crates/pulsen/src/application/tick/observe.rs`、`crates/pulsen/src/application/tick/mod.rs`、`crates/pulsen/tests/tick_observe.rs`
- **変更内容:** 手続きDを spec の順序どおりに実装し、`Branch::Observe` アームへ配線する。

  0. `current_attempt` / `current_attempt.process` が None なら不変条件2〜3の破れとして報告しスキップ
  1. `RunStore::read_exit(run_dir)`(`RunFileError` は報告してスキップ・書き込まない)
  2. exit が Some → **生存観測を行わず**判定 — `judge` 未定義なら `default_judgement`、定義ありなら `judge_env`(workspace が未確定なら不変条件4の破れとして報告しスキップ。adr.md ADR-008) → `CommandRunner::run(judge, env, config.judge_timeout())` → `interpret_judge_completion`。`JudgeFailure` は `record_judge_failure(detail, config.judge_attempt_limit(), now)` → `commit`。`Completed` は `complete_run`(`judged` に記録)、`Skipped` は `skip_run`(`skipped_back` に記録)、`Failed` は `fail_run(effective_retry_limit, now)` → `commit`(失敗の根拠を伴って報告)
  3. exit が None → `starttime_of(pid)`(`Err(Io)` は報告してスキップ)→ `IdentityCheck::check` → `classify_alive(aliveness, recorded.wall, effective_timeout, now)`。`KeepRunning` は何もしない(書き込みを1回も起こさない)、`KillOnTimeout` は `kill` 成功時のみ `fail_run` → `commit`・失敗は状態を変更せず報告、`DiedWithoutExit` は `try_kill_remnants`(結果は報告のみ)→ `fail_run` → `commit`

  ダブル・`SettableClock` に対するユースケーステストで、上記の全分岐と、exit が Some のときに `starttime_of` が1度も呼ばれないこと、timeout の境界5種、上限の等号と +1、判定の冪等性(同じ exit・同じ定義で同じ結論)、判定確定後の `save` 失敗からの再導出、`judge` 未定義での exit 20 の failed 分類、シグナル死の 128+n の failed 分類、判定失敗の後の failed 確定で `judge_attempt_count` がリセットされること、skipped 確定後の次 tick が同じ exit を再判定せず新 attempt を起動すること、マーカーで未起動終了したラッパーの「exit なし・プロセス死亡」分類を検証する。
- **理由:** 本スライスの中核。exit の有無で観測の要否が変わる2段規則と、`kill` / `starttime_of` の失敗で状態を変更しない規則は、実アダプターでは外から作れないためダブルでしか主張できない。
- **消化するチェックリスト:** UC-execution-006、TC-exec-tick-008, 087〜125
- **参照:** `spec/usecases/execution.md#手続きd-観測判定running`、`spec/testcases/execution/tick.md#正常系-4` / `#異常系-4` / `#境界値-3` / `#エッジケース-4`

### 11. 走査レベルの異常系・エッジケースの仕上げ

- **対象ファイル:** `crates/pulsen/tests/{tick_scan.rs,tick_observe.rs,tick_notify.rs}`
- **変更内容:** ステップ9・10 で書いたテストを `spec/testcases/execution/tick.md#走査と分岐処理フロー-1-9` と突き合わせ、未消化の行を埋める。

  - `TC-exec-tick-020`: スナップショットのみ破損した **stopped 以外**のタスクは、定義依存の判断をすべてスキップして報告し、書き込まない(ステップ9 で足した再通知の対象外であることの裏返し)
  - `TC-exec-tick-022`: 手動修復で不変条件が破れている(Running なのに `current_attempt` / `process` が None)タスクの報告とスキップ
  - `TC-exec-tick-023`: 1タスクの処理が失敗しても `errors` に記録して残りを続行し、tick 全体は 0
  - `TC-exec-tick-024`: 状態が変化しないタスク群(Wait 滞留・猶予内待機・実行継続中)に対する連続 tick で書き込みが1回も発生しないこと(冪等性)
  - `TC-exec-tick-025`: running の exit 0 を観測した tick は `complete_run` までで止まり、`advance` を行わない(1タスク1tick1ステップ)

  あわせて `tick_scan.rs` の「未配線アームではエージェントを起動しない」という暫定の主張(ADR-101)を、各手続きの期待に書き換える。
- **理由:** 冪等性と「1タスク1tick1ステップ」は個別の手続きではなく tick 全体の性質で、アームを埋め終えてからでないと主張できない。
- **消化するチェックリスト:** TC-exec-tick-020, 022, 023, 024, 025
- **参照:** `spec/testcases/execution/tick.md#異常系` / `#エッジケース`

### 12. サマリー表示の追随

- **対象ファイル:** `crates/pulsen/src/cli/render.rs`、`crates/pulsen/tests/cli_tick.rs`
- **変更内容:** `judged` / `transitioned` / `skipped_back` / `notified` の4項目が実際に表示されること、その並び順(「起動 / 起動確認 / 判定確定 / 遷移 / 実行待ちへ復帰 / 凍結 / 通知 / 終端処理」)が保たれること、新しい `TickIssue` の分類がタスクIDと原因の読み取れる形で出ることを、実バイナリの受け入れテストで確認する。値の入る経路が本スライスでできたことにより、`PAGE-tick-004`(#2 で未チェックのまま残した行)の残り部分のうち `transitioned` / `skipped_back` / `notified` の3フィールドが観測可能になるので、**#2 のコメントに追記する形で消化状況を更新する**(`archived` / `gc_deleted` / `gc_errors` は引き続き #6)。`judged` は spec の DTO に無いフィールドなので `PAGE-tick-004` の消化には数えず、表示の追加として扱う。
- **理由:** ADR-092 の「書き込みを行った tick は必ずサマリーに現れる」が、本スライスで足した経路にも成立していることを表示側で確認する。
- **消化するチェックリスト:**(直接対応する台帳行は無い。`PAGE-tick-004` の残りの一部を #2 側で更新)

### 13. 受け入れテスト: 判定・遷移・凍結・通知の一周

- **対象ファイル:** `crates/pulsen/tests/{cli_tick.rs,common/mod.rs,tick_fixture/mod.rs}`、`crates/pulsen/examples/judge_probe.rs`
- **変更内容:** 一時ホーム + `git init` した一時リポジトリに対して `pulsen add` → `pulsen tick` を繰り返し、実バイナリで次を検証する。ラッパーはデタッチ起動で非同期に完了するため、run ディレクトリの内容を読む前と次の tick を打つ前は既存の `wait_until` で待ち合わせ、**待ち条件はこれから観測する成果物そのもの**(`exit` を読むなら `exit` の出現)に立てる。

  - exit 0 の観測 → `completed` の記録 → 次 tick で `advance` によりタスクステータスが `next` へ、実行状態は pending、カウンタは 0(AC-2)
  - 非0 exit → `failed` と `attempt_count` の加算 → 次 tick で新しい attempt 番号で再起動 → 成功で `attempt_count` が 0 に戻る(AC-3)
  - `judge` 定義ありのステータスで、判定コマンドが受け取った `TASK_ID` / `WORKSPACE` / `EXIT_CODE` / `RUN_DIR` を証跡ファイルに書き出し、0 / 10 / 20 の3分岐がそれぞれ completed / failed / skipped になる(AC-5)
  - `retries` の上限超過で `stopped` が保存され、同じ tick 内で notify_cmd が `TASK_ID` / `WORKFLOW` / `TASK_STATUS` を伴って実行され、`notified_at` が記録される(AC-4)
  - notify_cmd を失敗させると `notified_at` が残らず、成功する notify_cmd に戻した次の tick で再通知され、さらに次の tick では通知されない(AC-4)
  - notify_cmd 未定義の凍結では通知も `notified_at` の記録も起きず、後から定義した次の tick で catch-up される(AC-4)
  - スナップショットのみ破損した未通知 stopped のタスクファイルを直に置くと、tick が再通知して `save_degraded` で `notified_at` を残す(スナップショットは元の内容のまま温存される)
  - `timeout` を短く定義したステータスで滞留するエージェントを起動し、超過後の tick が kill して `failed` にする。プロセスが残っていないことを確認する(AC-6)
  - 判定コマンドがプロトコル外の exit code を返し続けると、`judge_attempt_count` が加算されて `running` のまま再判定され、上限超過で `stopped` になる。`attempt_count` は 0 のままで run ディレクトリは `attempt-1` だけ(エージェントの再実行が起きていない)

  タスクファイルを直に組み立てるヘルパー(running・completed・未通知 stopped・スナップショットのみ破損した未通知 stopped)を `tick_fixture` に足す — これらは本スライスの CLI だけでは作れない状態がある。滞留するエージェントを使うケースは、テストを終える前に `exit` の出現を `wait_until` で待つ(一時ホームの削除と孫プロセスの書き込みの競合を避ける)。
- **理由:** ポート単位・ユースケース単位で緑でも、合成ルートの結線とクロス tick の引き継ぎが壊れていれば主経路は動かない。Issue の「検証」欄の項目をここで機械的に裏付ける。
- **消化するチェックリスト:** UC-flow-001, 003, 008(いずれも部分消化。plan.md の表のとおり)
- **参照:** `spec/flows/index.md#f1-タスクの実行状態ライフサイクル` / `#f3-stopped-通知at-least-once` / `#f8-ポーリング循環skipped-ループ`、`.adr/093-usecase-and-acceptance-fixtures-are-separated.md`

### 14. 手動確認

- **対象ファイル:**(コード変更なし)
- **変更内容:** plan.md「テスト方針」の手動確認の表に沿って `spec/manual-tests/` を実行する。`show` / `ls` が読む値は `state/tasks/<task-id>.json` の直読で代替し、`abort` を前提とする3つの TC(setup TC-35 / intervention TC-15 / intervention TC-24)は上限超過での凍結に読み替える。復元手順(task-execution TC-03 手順12、setup TC-39 の `judge_timeout`、setup TC-35 の `notify_cmd`)は実行範囲外でも必ず実行し、後続の TC が壊れないようにする。
- **理由:** 判定・通知の主経路は「実際に通知が届くこと」まで確認しないと成立が見えない。手順書は spec と実装のずれを人間の目で検出する最後の網でもある。

### 15. チェックリスト記帳・spec 差分の提起・ADR の昇格判定

- **対象ファイル:**(コード変更なし)`.thread/3/adr.md`、`.adr/3-*.md`、Issue #3 のコメント、Issue #2 のコメント(`PAGE-tick-004` の追記)
- **変更内容:** 適合スイートのスキップ行と、スライス境界により部分消化になった行(plan.md の2つの表)を Issue のコメントにまとめ、**実際にチェックが付いた行数を確定させる**(上限125行から当日スキップになった行を引いた数)。確認した実行環境(OS・root か否か・TMPDIR の位置・`git` のバージョン)を明記する。plan.md「spec との差分として提起するもの」を同じコメントで spec 追従として提起する。**提起する内容は実装の最終形に合わせる** — `TransitionError` は spec の5種(`InvariantViolated { message }` を含む)に対して実装は6種(`MissingCurrentAttempt` と `AlreadyNotified`)、tick 出力 DTO は spec の9フィールドに対して実装は11フィールド(`confirmed_running` と `judged`)、`RunningClassifier::classify_alive` は spec の `-> RunningDecision`(`spec/domains/execution.md:109`)に対して実装は3値の `AliveDecision` を返し2段規則の1段目は `application/tick/observe.rs` にある、`JudgementService::default_judgement` は spec の `-> JudgeOutcome`(同 `:119`)に対して実装は2値の `DefaultJudgement` を返す、`LaunchingClassifier` の `InconsistentRunFiles` は分類のみ(記載のみで変更しない)。`.thread/3/adr.md` の各エントリをプロジェクト全体に効くかどうかで選別し、昇格するものを `.adr/3-{slug}.md` として起票する。
- **理由:** Issue の完了条件が「実装をレビューで確認できた行にのみチェックを付ける。見送る行は理由をコメントに残す」であり、記帳と提起までを実装作業の一部として閉じる。
