# 実装計画 — Issue #3: tick による観測・判定・ステータス遷移(リトライ・凍結・通知)

**Issue:** #3
**作成日:** 2026-08-14
**複雑度:** 中規模
**実装方針:** steps.md

---

## 目的

running タスクの終了を観測して completed / failed / skipped に判定し、`advance` でタスクステータスを次へ進める。連続失敗がリトライ上限・判定上限を超えたら stopped で凍結し、notify_cmd で at-least-once に通知する。Issue #2 が値だけ用意して空のまま残した tick の3アーム(`Branch::Observe` / `Branch::Advance` / `Branch::Notify`)と `SnapshotUnreadable` の再通知を埋め、`Tick::commit` の凍結地点(`.thread/2/adr.md` ADR-074)に共通手続き notify を差し込む。

## 受け入れ基準

| # | 基準(検証可能な形で) | 由来 | 対応ステップ |
|---|---|---|---|
| AC-1 | **実装チェックリスト全128行の実装**(Issue #3 本文参照。スタブ・仮実装・部分実装は不可)。各行は `spec/inventory/{layer}.md` の台帳行と1:1で対応し、台帳の PASS 要件を**すべて**満たす実装とテストが存在し、そのテストが実際に走って通った行にだけチェックを付ける | Issue 完了条件 | 全ステップ |
| AC-2 | **exit 0 の観測 → completed → 次 tick で next へ**: エージェントが exit 0 で終わった running タスクに `pulsen tick` を打つと、その tick では `complete_run` までで止まり(1タスク1tick1ステップ)、次の tick の `advance` でタスクステータスが `next` へ進んで pending に戻る。`attempt_count` / `judge_attempt_count` は 0 にリセットされる | Issue 検証1、`spec/scenario/task-execution.md#tickによるタスクの自動進行登録から完了まで` フロー4〜7、TC-exec-tick-025 | 9, 10, 14 |
| AC-3 | **一過性失敗の自動リトライと回復**: 非0 exit(または judge の exit 10)が `fail_run` で failed になり `attempt_count` を消費する。次の tick が同じタスクステータスを新しい attempt 番号で再起動し、completed / skipped の確定で連続失敗カウンタが 0 に戻る | Issue 検証2、`spec/scenario/task-execution.md#失敗したエージェント実行が自動リトライで回復する`、TC-exec-tick-121 | 5, 10, 14 |
| AC-4 | **上限超過での凍結と at-least-once 通知**: リトライ上限・判定上限・spawn 上限の超過が `Stopped` を保存し、その `save` 直後の同一 tick 内で `TASK_ID` / `WORKFLOW` / `TASK_STATUS` と NOTIFY_TIMEOUT(60秒)を伴って notify_cmd がシェル非経由で起動され、`Exited(0)` のときだけ `mark_notified` → `save` が走る。notify_cmd を失敗させると `notified_at` が残らず、次の tick が同じ判断を再導出して再通知する。notify_cmd 未定義なら通知も `notified_at` の記録も行わず、後から定義すると次の tick が catch-up する | Issue 検証3、`spec/scenario/intervention.md#stopped確定の通知を受け取る`、`spec/flows/index.md#f3-stopped-通知at-least-once` | 3, 5, 9, 14 |
| AC-5 | **判定 exit 20 でのポーリング周回**: 判定コマンドの exit 20 が `skip_run` になり、タスクステータス不変のまま pending へ戻って `skipped_back` に記録される。通知は行われず、次の tick が同じタスクステータスを新しい attempt で起動する。judge 未定義のステータスでエージェントが exit 20 で終わった場合は failed に分類される(デフォルト判定は2値) | Issue 検証4、`spec/scenario/task-execution.md#人間が介入するまで繰り返すポーリング型ワークフロー`、`spec/flows/index.md#f8-ポーリング循環skipped-ループ`、ADR-008 | 3, 10, 14 |
| AC-6 | **timeout kill と exit 記録なしの死亡検出**: exit が無く生存(照合一致)で `starttime.wall` からの経過が timeout を超えたら `kill` してから `fail_run` する。`kill` の失敗では状態を変更せず報告のみ行う。exit が無く `starttime_of` が `Ok(None)`(または記録済み `starttime.ident` と不一致)なら `try_kill_remnants` をベストエフォートで試みてから `fail_run` する。exit が Some なら生存観測を**行わない** | Issue 検証5、TC-exec-tick-093〜095・101・103・117 | 2, 6, 10, 14 |
| AC-7 | **品質ゲートと隔離の維持**: `cargo build` / `cargo test` / `cargo clippy -- -D warnings` / `cargo fmt --check` が通る。`pulsen-domain` の `[dependencies]` は空のまま、本番依存は ADR-023 の6クレートから増えない。`pulsen-domain` の `unsafe_code = "forbid"` と `pulsen` の `unsafe_code = "deny"` はそのままで、`#[allow(unsafe_code)]` は `adapter/process.rs` の Windows ハンドル抑止モジュール1箇所(ADR-100)から増やさない。`crates/*/src/` のターゲット述語つき `cfg`(`unix` / `windows` / `target_os` / `target_family`)は `crates/pulsen-domain/` に1件も現れず、`crates/pulsen/src/` 側のヒットは `util/atomic.rs` / `adapter/process.rs` / `adapter/task_repository.rs` の3ファイルから増えない(CommandRunner のアダプターは `run_agent` と同じ符号化関数を共有し、OS 依存分岐を自前で持たない) | CLAUDE.md 技術方針、Issue #2 AC-1、ADR-023 / ADR-075 / ADR-100 | 全ステップ |
| AC-8 | **記帳**: 部分消化になった行・実行環境がスキップにした行にはチェックを付けず、前者は消化した範囲と引き取り先を、後者はスキップした理由と確認した環境(OS・root か否か・TMPDIR の位置)を Issue のコメントに残す。spec との食い違い(下記「spec との差分として提起するもの」)も同じコメントで提起する | Issue 完了条件 | 15 |

## スコープ

### 含まれないもの

- **手続きB(終端処理)・手続きE(gc)** と `WorktreeManager::remove` / `RunStore::{list_runs, delete_attempt, remove_task_dir_if_empty}` / `GcPolicy` / `RemoveOutcome` — Issue #6。`Branch::Cleanup` のアームは引き続き配線しない(`.thread/2/adr.md` ADR-101)。アーカイブに到達しない以上、`archived` / `gc_deleted` / `gc_errors` は本スライスでも値の入る経路を持たない。
- **abort / retry / set-status** と `Task::{abort, retry, set_status}` / `DegradedTask::{abort, retry}` — Issue #5。stopped に至る4経路のうち abort だけは本スライスに無く、通知の起点は上限超過3経路に限る。`DegradedTask` には本スライスで `mark_notified` **だけ**を足す。
- **`RunStore::attempt_exists`** と ls / show の表示 — Issue #4。手動確認で `show` / `ls` が読む値は `state/tasks/<task-id>.json` の直読で代替する。
- **CI ワークフロー・MSRV 検証・Windows 実機での検証** — Issue #10。本スライスは `#[cfg]` の隔離を grep で機械的に確認するに留める(AC-7)。
- 並列度制御・イベント駆動(webhook 等)— requirements が明示的に持たないと定めている。
- `judge` / `notify_cmd` の**コマンド実体**の登録時検証 — spec が「登録時に検証しない」と定めている(`spec/manual-tests/setup.md` TC-38)。

## チェックリスト行にチェックを付ける基準

Issue #1 / #2 で確立した基準をそのまま使う。**チェックを付ける**のは、台帳行(`spec/inventory/*.md`)の PASS 要件を**すべて**満たす実装とテストが存在し、そのテストが実際に走って通っている行。**チェックを付けない**のは、環境が前提を作れずスキップで終わった行と、スライス境界により PASS 要件の一部しか消化していない行。

### 既に実装済みの部分を確認してから消化する行

Issue #2 の実装が、#2 のチェックリストに無いまま結果として満たしている行が2つある。`DOM-execution-002` は**新規実装が不要で、PASS 要件との一致を確認してチェックを付ける**。`DOM-task-053` は PASS 要件(5種を区別する)を #2 の実装が満たしているが、本スライスで変種を1つ足す。

| 行 | 現在の実装 | 確認すること |
|---|---|---|
| `DOM-execution-002` `ExitCode.is_success` | `crates/pulsen-domain/src/execution/value.rs`(単体テスト付き) | 0 のときだけ真であること |
| `DOM-task-053` `TransitionError` | `crates/pulsen-domain/src/task/transition.rs`(6変種。うち `AlreadyNotified` は本スライスで追加。adr.md ADR-006) | 5種を区別すること(PASS 要件)を確認したうえで `AlreadyNotified` を足す。`InvariantViolated { message }` は ADR-096 により `MissingCurrentAttempt` に置き換わっており、**台帳の字義とは変種名が異なる**(下記の spec 差分) |

`ADP-process-002`(`starttime_of` の実装)も、三値を返す共有関数 `adapter::process::identity::observe`(ADR-075 / ADR-076)が #2 で完成しているため、ポートメソッドを足して委譲するだけになる。行そのものは未消化なので、適合ケース(TC-port-process-controller-006〜010)を通してからチェックする。

### 本スライスで部分消化になることが計画時点で分かっている行

| 行 | 消化する範囲 | 残る部分 | 引き取り先 |
|---|---|---|---|
| `UC-flow-001`(F1) | Pending / Failed → Launching / Failed / 状態不変(#2 で消化済み)、Launching → Running / Pending(同上)、Running → Completed / Pending / Failed / Running、Completed → Pending(`advance`)、上限超過3経路 → Stopped | Cleanup 終端(アーカイブ)、abort による Stopped、Stopped → Pending(retry / set-status)、任意ステータスへの手動遷移。「全状態に脱出遷移がある」ことの主張が閉じない | #5 / #6 |
| `UC-flow-003`(F3) | 上限超過3経路の stopped 記録 → notify_cmd 実行 → `mark_notified`、未通知 stopped の catch-up、DegradedTask への再通知、通知失敗・クラッシュ時の再通知、notify_cmd 未定義時の保持 | abort を起点とする stopped の通知(4経路目)、retry / set-status による未通知のままの stopped 離脱(CLI 経路での確認) | #5 |
| `UC-flow-008`(F8) | skipped による同一ステータスの周回、completed による `next` への遷移と循環定義での復帰、completed / skipped でのカウンタリセット、一過性失敗の failed → リトライ | 出口(人間の abort → set-status で Cleanup へ)、周回で蓄積した attempt の gc | #5 / #6 |

`UC-execution-002`(tick 全体)は #2 の checklist 行で、本スライスの完了後も `Branch::Cleanup` と手続きE が残るため引き続き未チェックのままとする(引き取りは #6)。

### 実行環境が前提を作れないとスキップで終わる行

| 行 | 前提を作れない環境 | 判定 |
|---|---|---|
| `TC-port-command-runner-004` | 実行権限のない実体を作れない(root 実行・権限を持たないファイルシステム) | `permission_restrictions_effective` |
| `TC-port-process-controller-011` / `012` / `013` / `015` | 実行単位(プロセスグループ相当)そのものを起こせない | 実行単位を1度起こす probe が `Unavailable` を返すか(adr.md ADR-013) |
| `TC-port-process-controller-014` / `016` | 実行単位は起こせるが、その一部だけを終了させられない | 同じ probe が `WholeOnly` または `Unavailable` を返すか(同上) |

`TC-port-command-runner-005`(外部からの強制終了)と `TC-port-process-controller-010`(起動時刻の取得機構の失敗)は表に載せない — 前者は期待が「非0の符号化値」までで `judge_probe abort` がどのプラットフォームでも非0を返し、後者は取得元の注入で確定的に走るため、前提を作れない環境が無い(adr.md ADR-007 / ADR-013)。`013` / `015` / `016` も注入で状況を作るが、その操作を向ける実行単位そのものは要るため上の表に残る。

フィクスチャの実行ファイル(`examples/agent_probe` / `examples/spawn_probe` / `examples/judge_probe`)の不在は、どの行でも許容集合に入れない — 作り忘れを緑にしない(ADR-068 / ADR-073)。

スキップ許容集合は環境の能力から実行時に決める(ADR-055 / ADR-073)。判定の正本は `crates/pulsen/tests/conformance_command_runner.rs` の `allowed_skips` と `crates/pulsen/tests/conformance_process_controller.rs` の `observation_allowed_skips`。チェックが付く行数は実行環境で決まり、上限は128行から部分消化3行を除いた **125行**。確定した数と一覧はステップ15 で Issue のコメントに残す。

## リスクと注意点

- **`CommandRunner` の timeout を追加依存なしで実装する**: std に「子プロセスを期限つきで待つ」API は無く、ADR-023 が本番依存を6クレートに閉じ、`unsafe_code = "forbid"` が効いている。判定 timeout(`judge_timeout`)と `NOTIFY_TIMEOUT`(60秒)は両方ともこのポートに乗るため、ここが破れると新規依存または lint 緩和の判断が要る。**ステップ7 で最初に実測して確かめる**(adr.md ADR-001)。
- **`kill` / `try_kill_remnants` を `unsafe` なしで組む**: ADR-075 が決めたのはデタッチ起動・同定情報の観測までで、終了操作は未決。`KillIdent` は POSIX で `-<pgid>`、Windows で `<pid>` の**永続化された不透明値**であり、誤った対象に効くと無関係なプロセス群を殺す(adr.md ADR-002 / ADR-015)。`try_kill_remnants` の `NotIdentifiable` は「いかなるプロセスも終了させない」ことが期待結果なので、同定できないときに広めに殺す実装は適合ケースで検出されない — 誤殺しない側に倒す判断をコードの why として残す。
- **`kill` の失敗で状態を変更しない**: `KillOnTimeout` で `kill` が失敗したときに `fail_run` を呼ぶと、生存したままのプロセスを持つタスクが failed → 再起動 → **同一 worktree での並走**に至る。`starttime_of` の `Err(Io)` も同じ理由で Alive / Dead のどちらにも写像しない。どちらも「状態を変更せず報告のみ」で、次 tick が同じ決定を再導出する。
- **exit が Some なら生存観測を行わない**: `RunningClassifier` の2段規則。観測を先に置くと、`starttime_of` が失敗する環境で判定が永久に遅延する(TC-exec-tick-103)。`classify_alive` に `Judge` を返させない型の形がこの規則の担保になる。
- **通知の順序を逆にしない**: 「stopped を書く(`notified_at: None`)→ notify_cmd → 成功時のみ `mark_notified`」。`mark_notified` を先に書くと、通知が失敗した凍結が永久に通知されない(欠落)。二重通知は許容する(requirements §8)。
- **凍結の判定を保存後の状態から導出しない**: ADR-097。catch-up 通知は `mark_notified` した `Stopped` を保存するため、`commit` が状態から `frozen` を決めると過去の凍結が毎 tick 再計上される。通知アームの保存は `Freeze::NotFrozen` を通す。
- **DegradedTask の再通知は `save_degraded` を使う**: 現在の `Tick::commit` は `&Task` 専用。スナップショット破損タスクへの通知(`PAGE-tick-007` / `TC-exec-tick-158`)を欠くと、`SnapshotUnreadable` かつ未通知の凍結が at-least-once を破る(adr.md ADR-003)。
- **手続きD 冒頭の不変条件検査**: 不変条件2(`current_attempt` が Some)に加えて不変条件3(`current_attempt.process` が Some)を検査する。既存の `TickIssue::MissingCurrentAttempt` は前者専用なので、報告分類の扱いを決める(adr.md ADR-004)。
- **判定・通知の実行中は排他ロックを保持したまま tick がブロックする**: `judge_timeout`(既定60秒)× 判定対象タスク数 + `NOTIFY_TIMEOUT`(60秒)× 凍結タスク数。spec が承知のうえで組み込み timeout を置いた設計(ADR-018)であり、本スライスで新しい緩和は入れない。手動確認(setup TC-39)で tick が5秒強ブロックすることを実際に観測する。
- **`Tick` のジェネリック引数が7つになる**: `CommandRunner` を足すと `<R, L, K, W, S, P, C>`。合成ルート(`cli::wire`)は `notify_cmd` を `runtime.config()` から既に読めるので、足すのはランナーの構築だけ。
- **既存テストの書き換えが要る**: `crates/pulsen/tests/tick_scan.rs` は未配線アーム(Running / Completed / Stopped)に対して「エージェントを起動しない」ことだけを主張している(ADR-101)。アームを埋める時点で、この主張は各手続きの期待に書き換える。
- **`ScriptedRunStore::read_exit` は現在パニックする**: `crates/pulsen-conformance/src/doubles/run_store.rs`。`with_read_exit` と `RunStoreCall::ReadExit` を足さないと手続きDのユースケーステストが1件も書けない。
- **`crates/pulsen-conformance/src/process_controller.rs` 冒頭の「8行」は誤り**: 実際に残っているのは `TC-port-process-controller-006〜016` の**11行**(`starttime_of` 5 / `kill` 3 / `try_kill_remnants` 3)。ステップ6 でコメントと `HOOKS.md` の集計を直す。
- **時刻依存テストの安定性**: timeout の境界(等号・超過・巻き戻り・`Unlimited`・未指定の組み込み1h)は実時間で待てない。`SettableClock` とダブルに対するユースケース層テストで消化し(TC-exec-tick-106〜111)、受け入れテストでは実時間で作れる範囲(`sleeper` ワークフローの 10s)だけを扱う。
- **判定の冪等性**: 同じ exit・同じ定義に対して常に同じ結論を導く(TC-exec-tick-118 / 119)。判定コマンド自体の冪等性は利用者の責務であり、テスト用の判定コマンドは制御ファイル(`judge-exit`)を読むだけの形にして、ツール側の冪等性だけを主張する。

## spec との差分として提起するもの

転記の過程で、spec の字義と現在の実装が食い違う点が見つかった。**本スライスは実装側に合わせ、spec 追従を Issue のコメントで提起する**(勝手に spec を書き換えない)。

- `spec/domains/task.md` のエラー型は5種(`TransitionError::InvariantViolated { message: String }` を含む)だが、実装は6種 — `InvariantViolated` は ADR-096 により `MissingCurrentAttempt`(分類のみ)へ置き換わり、本スライスが `AlreadyNotified` を足す(adr.md ADR-006)。`DOM-task-053` の PASS 要件「5種を区別する」は満たすが、変種名と種類数が一致しない。
- `spec/usecases/execution.md` の tick 出力 DTO は9フィールドだが、実装は `confirmed_running` と `judged` を加えた11フィールド(ADR-094 / adr.md ADR-005)。本スライスは `judged` / `transitioned` / `skipped_back` / `notified` を初めて埋める側なので、この差分が表示に現れる。
- `spec/domains/execution.md:109` の `RunningClassifier::classify_alive` は `-> RunningDecision` だが、実装は `Judge` を除いた3値の `AliveDecision` を返す(adr.md ADR-009)。2段規則の1段目(exit が Some なら観測なしで即 `Judge`)はユースケース側(`crates/pulsen/src/application/tick/observe.rs`)にあり、`DOM-execution-017` の PASS 要件が1段目を `classify_alive` に含めている字義とは配置が異なる。`RunningDecision` は4値のまま残るので `DOM-execution-008` の値数要件は満たす。
- `spec/domains/execution.md:119` の `JudgementService::default_judgement` は `-> JudgeOutcome` だが、実装は2値の `DefaultJudgement` を返す(adr.md ADR-016)。`JudgeOutcome` は3値のまま残るので `DOM-execution-004` を、2値であることが `DOM-execution-019` の PASS 要件を満たす。
- `spec/domains/execution.md` の `LaunchingClassifier` は `InconsistentRunFiles { message: String }` だが、実装は破れの種別だけを持つ(ADR-081)。本スライスでは変更しない(記載のみ)。

## テスト方針

- **ドメイン**(`cargo test` のユニットテスト、I/O なし):
  - `IdentityCheck::check` の3分岐(`None` → Dead / 不一致 → Dead / 一致 → Alive)。
  - `RunningClassifier::classify_alive` の3分岐と timeout の境界 — 経過が等号(未超過)・超過・負(0 に飽和)・`Unlimited`(どれだけ経過しても `KeepRunning`)・未指定(組み込み1h)。起点が `starttime.wall` であること。
  - `JudgementService` — `default_judgement` の 0 / 非0(20 も failed)、`interpret_judge_completion` の 0 / 10 / 20 / それ以外 / `TimedOut` / `FailedToStart`、`judge_env` の4変数(`EXIT_CODE` は10進文字列)。
  - `NotificationService::notify_env` の3変数と `NOTIFY_TIMEOUT = 60秒`。
  - `Task` の遷移5種 + `mark_notified` — 前提状態の不一致(6状態それぞれからの呼び出し)、`complete_run` / `skip_run` のカウンタリセット(`spawn_fail_count` は触らない)、`fail_run` の `attempt_count += 1` と `judge_attempt_count = 0`、`record_judge_failure` の `Running` 維持と `last_failure = JudgeFail`、上限の等号(凍結しない)と +1(凍結する)、`retries: 0` での即凍結、`advance` の `next` 参照と非 AgentRun での `NotAgentRunStatus`、`mark_notified` の `notified_at: Some` 済みからの拒否。`DegradedTask::mark_notified` も同じ規則で。
  - テスト名は仕様の言葉(日本語)で付ける。
- **ポート適合テスト**(`pulsen-conformance`): ProcessController に11件(TC-port-process-controller-006〜016)を追加し、CommandRunner の新スイート16件(TC-port-command-runner-001〜016)を1行1関数で置く。ハーネスのフックは「状況の意味」だけを受け取り(ADR-027)、権限操作系は制限が実際に効いたことを確認してから `Some` を返す。`kill` / `try_kill_remnants` は実プロセスを使い、期待は契約の語彙(「実行単位に属する全プロセスが終了する」)で書いてプラットフォーム固有の機構名に踏み込まない(ADR-082)。`HOOKS.md` は冒頭の対象ポート数・総行数・区分別件数表と「環境で走らなくなりうる行」の表を更新し、ProcessController の節を16行 → 27行に、CommandRunner の節(16行)を新設する。
- **ユースケース**(ダブルに対するテスト、実プロセス・実ファイルシステムなし): `crates/pulsen/tests/tick_observe.rs` / `tick_notify.rs`(新規)と `tick_scan.rs`(既存の未配線アームの主張を差し替え)。実アダプターでは外から作れない分岐をここで消化する — `read_exit` の `Corrupt` / `Io`、`starttime_of` の `Err(Io)`、`kill` の `KillError`、`try_kill_remnants` の `NotIdentifiable` / `Failed`、判定コマンドの `TimedOut` / `FailedToStart` / プロトコル外 exit、timeout の境界5種、上限の等号と +1、`save` の失敗、通知コマンドの非0 / `TimedOut` / `FailedToStart`、`mark_notified` 後の `save` 失敗、notify_cmd 未定義。
- **順序の検証**: 「stopped の `save` → notify_cmd → `mark_notified` の `save`」は `ScriptedTaskRepository` と `ScriptedCommandRunner` の呼び出し記録の並びで主張する。「exit が Some なら `starttime_of` を呼ばない」も `ScriptedProcessController::calls()` が空であることで主張する。
- **受け入れテスト**(`crates/pulsen/tests/`、実バイナリ): `cli_tick.rs` を延長し、`examples/agent_probe` を使って「起動 → running → 判定 → 遷移」の1周を実際に回す。判定コマンド・通知コマンドは制御ファイルを読んで exit code を返す小さなプローブ(`examples/`)を使い、実在のコマンドに依存しない。非同期に完了するラッパーは既存の `wait_until` で待ち合わせ、**待ち条件はこれから観測する成果物そのもの**(`exit` を読むなら `exit` の出現)に立てる。検証する筋は次のとおり — exit 0 → completed → 次 tick で next へ / 非0 → failed → 次 tick で新 attempt / judge の 0・10・20 の3分岐 / 上限超過での stopped と notify_cmd の実行 / notify_cmd を失敗させた次 tick での再通知 / notify_cmd 未定義での無通知 / スナップショットのみ破損した未通知 stopped への再通知 / timeout 超過での kill と failed。
- **手動確認**: Issue の「検証 / 手順書」に揃える。本スライスに無いコマンド(`ls` / `show` / `abort` / `retry` / `set-status`)と終端処理(#6)を要する手順は実行しない。`show` / `ls` で読む値は `state/tasks/<task-id>.json` の直読で代替し、run ディレクトリのファイルは JSON なので `cat exit` の出力は `0` ではなく `.code` に終了コードを持つ整形 JSON になる(ADR-080)。期待値は綴りではなく `jq '.code'` で読める値で書く。

  | 手順書 | ID | 本スライスでの実行範囲 |
  |---|---|---|
  | task-execution.md | TC-03 | 手順1〜9・11(`show` / `ls` はタスクファイル直読で代替)。手順10(クリーンアップとアーカイブ)は #6 のため実行しない。**手順12(元YAMLの復元)は実行範囲外だが必ず実行する** — 改変済みの `pipeline.yaml` を後続の TC-04 以降が引き継ぐため |
  | task-execution.md | TC-05 | 手順1〜4 と手順6。手順5 のうち `done` への遷移までは確認し、アーカイブ(#6)は確認しない。一過性失敗 → 再実行 → completed でのカウンタリセットがこの TC の主眼 |
  | task-execution.md | TC-06 | 全手順。skipped 周回で `attempt_count` が消費されず通知も起きないこと |
  | task-execution.md | TC-07 | 手順1〜5。手順6〜8(`abort` / `set-status`)は #5 |
  | task-execution.md | TC-13 | 全手順(手順4 の `show` は直読)。リトライ上限の**等号では凍結しない**ことと超過での凍結・通知 |
  | task-execution.md | TC-14 | 全手順。timeout kill と、実行中の連続 tick が状態を変えないこと(冪等性) |
  | task-execution.md | TC-15 | 全手順。判定失敗の上限超過が**エージェントを再実行せずに**凍結すること(`attempt_count` は 0 のまま・run ディレクトリは `attempt-1` のみ) |
  | task-execution.md | TC-17 | 全手順。exit 127 が spawn 失敗ではなく通常の failed 経路に落ちること(`spawn_fail_count` は 0) |
  | task-execution.md | TC-19 | 手順1〜5。手順6(`abort` / `set-status` での片付け)は #5 |
  | task-execution.md | TC-20 | 手順1〜4・6(手順1 の2件目は `draft.yaml` の代わりに `pipeline` で登録する)。手順5 の `ls` は #4、手順7 の `set-status` は #5、手順3 の期待のうちアーカイブは #6 のため確認しない。1タスクの失敗が走査を止めないことと連続 tick の冪等性 |
  | task-execution.md | TC-21 | 手順1〜5(手順2 の PID はタスクファイル直読で得る)。手順6(`abort`)は #5。exit 記録なしのプロセス死亡の検出 |
  | task-execution.md | TC-22 | 全手順。`retries: 0` での初回失敗の即時凍結 |
  | task-execution.md | TC-23 | 全手順。デフォルト判定での exit 20 が failed になること |
  | setup.md | TC-09 | 全手順。**前提として事前準備1〜3・TC-01 手順1 を先行実行する**。judge の exit 0 が completed になり `next` へ進むこと |
  | setup.md | TC-10 | 手順1〜4。手順5(`set-status` での片付け)は #5 |
  | setup.md | TC-11 | 手順1〜4。手順5 は実行しない — 片付けの `set-status` が #5 で、回復(`judge-exit` を 0 に戻して completed へ進む筋)は同じ `judge-demo` の TC-09 で消化済み |
  | setup.md | TC-35 | **手順2 の `abort` を上限超過での凍結に読み替える**(`abort` は #5)。`fail` 相当のワークフローを登録して tick を繰り返し、notify_cmd 未定義でも stopped の確定が正常に動作し `notify.log` に行が増えないことを確認する。手順4 の回復(config への `notify_cmd` の復元)は必ず行う |
  | setup.md | TC-37 | 手順1〜3。手順4(`retry`)は #5。プロトコル外の exit code が判定失敗として凍結に至ること |
  | setup.md | TC-38 | 手順1〜2。手順3(`abort` での片付け)は #5 — 代わりに放置する。この TC の `judge-missing.yaml` は `judge` が `/no/such/judge.sh` で `judge-exit` を読まないため回復の手立てが無く、以降 running のまま毎 tick 再判定されて判定上限超過で凍結し、そこで止まる |
  | setup.md | TC-39 | 手順1〜3 と手順4 の `judge_timeout` の復元。判定 timeout が判定失敗として扱われ、tick がその間ブロックすること |
  | setup.md | TC-47 | 手順1〜2。手順3(`abort`)は #5。シグナル死の符号化値が `EXIT_CODE` として判定コマンドへ渡ること |
  | intervention.md | TC-01 | 手順1〜3・5・7・8(手順3 の `ls` と手順4・6 の `ls --state` / `show` はタスクファイル直読で代替)。凍結までの間は通知されず、凍結の瞬間にちょうど1行だけ通知されること、通知済みの stopped が再通知されないこと |
  | intervention.md | TC-15 | 全手順。**手順2 の `abort` を上限超過での凍結に読み替える**(`abort` は #5)。手順3 の `show` は直読。notify_cmd 未定義では通知も `notified_at` の記録も行われず、後から定義した次の tick が catch-up すること。setup TC-35 と同じ筋なので1系列で消化する |
  | intervention.md | TC-24 | **手順2 の `abort` を上限超過での凍結に読み替える**(同上)。必ず失敗する notify_cmd で凍結させ、`notified_at` が残らないこと・notify_cmd を戻した次の tick で再通知されること・さらに次の tick では増えないことを確認する |

  上表で落とす手順は、Issue の完了条件に従い「見送る行はチェックせず理由を Issue のコメントに残す」運用に合わせる。
