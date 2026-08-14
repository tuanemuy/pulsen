### Domain

#### 前提と検証手順

- 基準にしたのは `CLAUDE.md`(関数型ドメインモデリング・ヘキサゴナル)、`spec/domains/task.md`(遷移関数表・不変条件)、`spec/domains/execution.md`(2段規則・判定プロトコル・通知)、`spec/flows/index.md`(F1 / F3 / F8)、`spec/inventory/domain.md`(DOM-task-038〜045 / DOM-execution-004・005・008・009・010・012・017〜022)、`.thread/3/plan.md`(AC-1〜AC-7)、`.thread/3/adr.md`。
- 品質ゲート(AC-7)は実際に走らせて確認した — `cargo fmt --check` / `cargo clippy --all-targets -- -D warnings` / `cargo test` すべて成功(pulsen-domain 264 件を含む全スイート緑)。`crates/pulsen-domain/Cargo.toml` の `[dependencies]` は空、`unsafe_code = "forbid"` は維持、`#[allow(unsafe_code)]` は `adapter/process.rs:350` の1箇所のみ、`crates/pulsen-domain/` にターゲット述語つき `cfg` は0件。AC-7 は満たしている。
- spec の遷移規則との突き合わせは表の事後条件を1行ずつ照合した。結果は「規則の側は一致、型の側に1点の破れ」。

#### 遷移規則の照合結果(spec/domains/task.md との1対1)

| メソッド | 前提状態 | カウンタ操作 | 上限判定 | 判定 |
|---|---|---|---|---|
| `complete_run` | `Running` のみ(`ensure_running`) | `attempt_count = 0` / `judge_attempt_count = 0` / `spawn_fail_count` 非対象 | — | 一致 |
| `skip_run` | `Running` のみ | 同上。`task_status` 不変・`Pending` へ | — | 一致(ADR-008) |
| `fail_run` | `Running` のみ | `attempt_count += 1` / `judge_attempt_count = 0` / `last_failure` は触らない | `count > limit` | 一致(`last_failure` を書かないのは「エージェント実行自体の失敗はここに記録しない」の正しい実装) |
| `record_judge_failure` | `Running` のみ | `judge_attempt_count += 1` / `attempt_count` 非対象 / `last_failure = JudgeFail` | `count > limit` | 一致(未超過で `Running` 維持も一致) |
| `advance` | `Completed` かつ AgentRun | 非対象 | — | 一致 |
| `mark_notified` | `Stopped { notified_at: None }` | 非対象 | — | 一致 |

- 上限の境界は `limit_exceeded`(加算後 `> limit`)を共有しており、等号で凍結しない・`retries: 0` で初回凍結の両方がユニットテストで押さえられている。ADR-009(連続失敗カウンタ)とも矛盾しない — `reset_run_failures` が `spawn_fail_count` に触れないことがテストの主張として明記されている。
- `JudgementService` は 0 / 10 / 20 / それ以外の4値と `default_judgement` の2値性(20 も failed)を満たす。`interpret_judge_completion` は `TimedOut` / `FailedToStart` も `JudgeFailure` に落とし、3つの原因が `detail` から判別できることをテストが主張している(ADR-008 に一致)。
- `IdentityCheck::check` の3分岐、`classify_alive` の timeout 境界(等号=未超過 / 超過 / 巻き戻り0飽和 / `Unlimited` / 未指定1h)はドメイン+ユースケースの両層で網羅されている。`Timestamp::elapsed_since` が `saturating_sub` + `try_from(..).unwrap_or(0)` で負を0に潰すことも確認した。
- 冪等性: `KeepRunning` は書き込みを1回も起こさず(テストで主張)、通知の判断は `Stopped.notified_at` という永続化された事実からのみ再導出される(`branch_of`)。順序「stopped の save → notify_cmd → mark_notified の save」も `commit` の1箇所に閉じており、逆順の余地がない。

---

#### Blockers

- **[B-001]** `classify_alive` が返せない `Judge` を返り値型に含んでおり、2段規則の担保が型ではなく実行時 `unreachable!` になっている
  - 場所: `crates/pulsen-domain/src/execution/running.rs:64-83`(返り値型 `RunningDecision`)、`crates/pulsen/src/application/tick/observe.rs:160-162`(`unreachable!`)
  - 理由: `plan.md` のリスク節が「**`classify_alive` に `Judge` を返させない型の形がこの規則の担保になる**」と明示しており、`running.rs:50-55` のドキュメントコメント自身も「だから `classify_alive` は `Judge` を返さない」と書いている。ところが返り値型は `Judge(ExitCode)` を含む4値のままで、型は何も担保していない。担保しているのは呼び出し側の `unreachable!("生存の分類は判定を返さない")` という**パニック**であり、CLAUDE.md の「不正な状態を型で表現不能にする」「パニックは不変条件違反にのみ使う」の両方に反する。関数の値域は集約の不変条件ではなく、型で表現できる性質である。実際 `RunningDecision::Judge` は本番コードのどこでも構築されておらず(`grep` で構築箇所ゼロ、唯一のヒットが上記 `unreachable!` アーム)、spec が定めた4値のうち1値が死んだ状態で、網羅 `match` が「起こらない分岐」を1つ抱えている。
  - 提案: どちらかを採る。(a) 1段目の判断もユースケースで**値にしてから** `match` する — `let decision = match exit { Some(exit) => RunningDecision::Judge(exit), None => { /* 観測 */ RunningClassifier::classify_alive(..) } };` としてから4アームを網羅すれば、`Judge` が構築される経路が生まれ `unreachable!` が消え、spec の `RunningDecision`(DOM-execution-008)も4値のまま生きる。(b) `classify_alive` の返り値を3値の新しい型(例 `AliveDecision = KeepRunning | KillOnTimeout | DiedWithoutExit`)にし、`RunningDecision` をその埋め込み(`Judge(ExitCode) | Alive(AliveDecision)`)にする。(a) のほうが spec の語彙を変えずに済み、変更も `observe` の1関数に閉じる。

#### Warnings

- **[W-001]** スナップショット破損 × 未通知 stopped のタスクが、notify_cmd 未定義のとき tick の出力から完全に消える
  - 場所: `crates/pulsen/src/application/tick/mod.rs:344-364`、`crates/pulsen/src/application/tick/notify.rs:66-81`(`Delivery::NotConfigured => {}`)
  - 理由: `spec/usecases/execution.md:43` は「`SnapshotUnreadable(degraded)` → 定義依存の判断はすべてスキップして**報告**。**ただし** `Stopped { notified_at: None }` なら共通手続き notify を実行する」と書いており、notify の追加であって報告の置換とは読めない。現実装は両者を排他にしたため、未通知の凍結かつ notify_cmd 未定義(spec が正規の構成と認めている状態)では `errors` にも `notified` にも何も積まれず、**破損したタスクが毎 tick 完全に無言でスキップされる**。`ls` / `show` は #4 なので、この tick が唯一の可視化経路である。`main` では毎 tick `SnapshotUnreadable` が報告されていたので、可観測性の後退でもある。
  - 提案: `notify_degraded` に入る経路でも `TickIssue::SnapshotUnreadable` を積む(通知の成否とは独立の「修復が必要である」という報告)。少なくとも `Delivery::NotConfigured` の枝では積む。

- **[W-002]** `TransitionError` が6変種になったのに、変種の識別を主張するテストが5種のままで `AlreadyNotified` を含まない
  - 場所: `crates/pulsen-domain/src/task/task.rs:1943`(`fn 遷移の前提の破れは5種を区別する`)、`crates/pulsen-domain/src/task/transition.rs:35`
  - 理由: 台帳 `DOM-task-053` の PASS 要件は「変種を区別すること」で、`plan.md` は本スライスでこの行を「確認だけを行う行」に数えている。ところが ADR-006 が `AlreadyNotified` を足して6変種になったのに、テストの配列は5要素のまま・テスト名も「5種」と言い切っている。新しい変種が既存のどれとも区別されることが1件も主張されていない(`AlreadyNotified` が返ることのテストはあるが、他変種との非同値は主張されていない)。名前が実装の現状と食い違ったテストは、次のスライスが `abort` / `retry` を足すときに同じ形で腐る。
  - 提案: 配列に `TransitionError::AlreadyNotified` を足し、テスト名を「遷移の前提の破れは6種を区別する」または種類数を数に埋め込まない名(例「遷移の前提の破れは変種ごとに区別される」)に改める。

- **[W-003]** 判定確定系の遷移関数が不変条件2・3を検査しておらず、破れの検査がユースケースにしか無い
  - 場所: `crates/pulsen-domain/src/task/task.rs:415`(`complete_run`)・`431`(`skip_run`)・`446`(`fail_run`)・`468`(`record_judge_failure`)・`496`(`advance`)
  - 理由: `spec/domains/task.md:164` は「不変条件 2〜4 は手動修復で破られたままデコードを通り得るため、**遷移関数が前提として検査し**、崩れていれば `TransitionError` を返す」と定めている。`confirm_running`(:305)は実際に `MissingCurrentAttempt` で検査しているのに、本スライスが足した5つは `ensure_running` / `ensure_completed`(実行状態の判別子だけ)しか見ていない。現状は `observe`(`observe.rs:35-47`)が不変条件2・3を先に弾くので tick からは到達しないが、**規則の実体がドメインの外にある**ため、#5 の `abort` / CLI 経路や将来の呼び出し元が同じ検査を書き忘れても型もテストも気づけない。とくに `advance` は `Completed` かつ `current_attempt = None` を素通しして `Pending` にするので、次の `record_launching` が `next_attempt_number = 1` を採番して既存の `attempt-1` に衝突しうる(不変条件5の破れ)。
  - 提案: 最低限 `advance` に `current_attempt` の存在検査(`MissingCurrentAttempt`)を足す。ほかの4つも `ensure_running` の中で「`Running` かつ `current_attempt.process` が `Some`」まで見れば、規則がドメインに戻り、`observe` 冒頭の検査は報告の解像度を上げるための先回り(ADR-004 の目的)として重複しても意味を保つ。

- **[W-004]** 判定コマンドが exit 10 を返したときの報告文が、エージェントの終了コードを「失敗の根拠」として提示してしまう
  - 場所: `crates/pulsen/src/application/tick/observe.rs:102-104` と `241-243`(`judgement_detail`)
  - 理由: `JudgeOutcome::Failed` はデフォルト判定(exit 非0)と判定コマンドの exit 10 の**両方**から到達するのに、`judgement_detail(&exit)` は常にエージェントの exit を文面にする。judge が exit 10 を返し、エージェントが exit 0 だった場合(`spec/scenario/setup.md` TC-10 の主経路)、利用者は `実行が終了コード 0 で終了しました` という自己矛盾した行を読むことになる。この文字列は `TickIssue::RunFailed` 経由で `cli/render.rs` から実際に表示される。
  - 提案: 判定の出所で文面を分ける — デフォルト判定なら現行のまま、判定コマンド経由なら「判定コマンドが失敗(終了コード 10)と判定しました(実行の終了コードは {exit})」のように、判断した主体を明示する。

- **[W-005]** 不変条件4の破れをユースケースがドメインのエラー値を自作して報告しており、その分岐にテストが無い
  - 場所: `crates/pulsen/src/application/tick/observe.rs:73-77`
  - 理由: `TransitionError::WorkspaceNotSet` は spec 上 `record_launching` の前提検査が返すエラーだが、ここではユースケースが遷移関数を呼ばずに値を組み立てて `report_transition` に渡している。ドメインの判断をドメインの外で合成する形で、「エラーは値として返る(返すのはドメイン)」の境界が1箇所ぼやける。加えて `crates/pulsen/tests/` 全体を見ても、**判定コマンドを持つステータスで `workspace` が未確定**という分岐を通すテストが1件も無い(`grep WorkspaceNotSet` のヒットは実装と `render.rs` のみ)。不変条件2・3の破れは `tick_scan.rs:384` で両方テストされているのに、4だけ抜けている。
  - 提案: `tick_observe.rs` に「判定コマンドを持つステータスで workspace が未確定なら報告してスキップする」を1件足す(`ScriptedTaskRepository` に workspace 未設定の Running を置けば作れる)。エラー分類そのものは、`MissingProcessIdent` と同様に `TickIssue` 側の変種にするほうが層の責務としては素直。

- **[W-006]** `Branch::Notify { notified: bool }` が真偽値フラグで、分岐が enum の外に残っている
  - 場所: `crates/pulsen/src/application/tick/mod.rs:381-385`・`509-513`・`530-532`
  - 理由: `Branch` は「タスク1件に対して選ぶ分岐」を実行状態の網羅として表す型で、ほかの変種はすべて分岐そのものを名前で表している。`Notify { notified }` だけが `if !notified { }` という**enum の外の分岐**を呼び出し側に残しており、CLAUDE.md の「OR関係はデータ付き enum」からずれる。`notified: true` の枝が何もしないことは変種名からは読めない。
  - 提案: `Branch::Notify` と `Branch::AlreadyNotified`(または既存の「何もしない」意味の変種)に割って、`dispatch` の `match` だけで完結させる。

- **[W-007]** `DegradedTask::mark_notified` の「凍結していない状態からは拒否」テストが `Pending` 1状態しか通していない
  - 場所: `crates/pulsen-domain/src/task/degraded.rs:271-282`
  - 理由: `Task` 側の同じ規則は `every_execution_state()` で5状態すべてを回している(`task.rs` の `凍結していないタスクには通知を記録できない`)。`DegradedTask` は「Task と同じ規則」であることが台帳 `DOM-task-059` の主張なのに、テストは1状態だけを見ている。規則の実体は `mark_notified_state` に共有されているので実害の確率は低いが、テストが主張しているのは共有の事実ではなく1点の挙動である。
  - 提案: `DegradedTask` 側も全状態を回すか、「規則を共有していること」を主張する形(同じ入力で `Task` と `DegradedTask` が同じ `Err` を返す)に書き換える。

#### 良かった点(意図的に確認して問題が無かったもの)

- 通知の順序と保存先の分離(ADR-003)が `deliver`(3値だけを受ける)と保存(`save` / `save_degraded`)に素直に割れており、`Task` / `DegradedTask` をトレイトで先に抽象化しなかった判断も #5 の差異を考えると妥当。
- 凍結の受け渡し(ADR-097)が守られている — catch-up 通知の保存は `Freeze::NotFrozen` を通しており、過去の凍結が毎 tick 再計上されない。`notify.rs:51-53` の why コメントがその理由をコードの現在形で説明している。
- `kill` 失敗・`starttime_of` の `Err(Io)` で**状態を変更しない**規則が両方とも実装され、それぞれユースケーステストで守られている(`終了操作に失敗したtimeout超過は状態を変更せず報告する` / `生存観測の機構が失敗すれば状態を変更せず報告する`)。同一 worktree での並走を避ける理由もコメントに残っている。
- `fail_run` が `last_failure` を書かないこと(spec の「エージェント実行自体の失敗はここに記録しない」)を落とさずに実装している。見落としやすい非対称で、テストの `reset_run_failures` の主張と合わせて意図が読める。
- ドメイン層に外部クレート・I/O・`cfg(target_*)` が1件も無く、新規サービス3つ(`RunningClassifier` / `JudgementService` / `NotificationService`)はいずれも依存ポートを持たない純粋関数の集まり。テストも実ファイルシステム・実プロセスを使っていない。
- テスト名がすべて仕様の言葉(日本語)で、実装の内部構造ではなく振る舞いを指している。弁明・修正の経緯を残すコメントは実装・テストのどちらにも見つからなかった(`修正` / `指摘` / `以前は` / `TODO` / `FIXME` / `暫定` を新規差分に対して grep して0件)。

#### カバレッジ

- 確認: `.thread/3/plan.md`, `.thread/3/adr.md`, `crates/pulsen-domain/src/execution/judgement.rs`, `crates/pulsen-domain/src/execution/mod.rs`, `crates/pulsen-domain/src/execution/notification.rs`, `crates/pulsen-domain/src/execution/port.rs`, `crates/pulsen-domain/src/execution/running.rs`, `crates/pulsen-domain/src/execution/value.rs`, `crates/pulsen-domain/src/task/counters.rs`, `crates/pulsen-domain/src/task/degraded.rs`, `crates/pulsen-domain/src/task/task.rs`, `crates/pulsen-domain/src/task/transition.rs`, `crates/pulsen/src/application/tick/mod.rs`, `crates/pulsen/src/application/tick/observe.rs`, `crates/pulsen/src/application/tick/notify.rs`, `crates/pulsen/src/application/tick/confirm_spawn.rs`, `crates/pulsen/src/application/tick/launch.rs`, `crates/pulsen/src/cli/render.rs`, `crates/pulsen/tests/tick_observe.rs`, `crates/pulsen/tests/tick_notify.rs`, `crates/pulsen/tests/tick_scan.rs`
- スキップ: `.thread/3/steps.md`, `.thread/3/testing.md` — 実装手順書・手動確認手順であり遷移規則の判定基準にならない(基準は plan.md と spec)
- スキップ: `crates/pulsen-conformance/HOOKS.md`, `crates/pulsen-conformance/src/command_runner.rs`, `crates/pulsen-conformance/src/lib.rs`, `crates/pulsen-conformance/src/process_controller.rs`, `crates/pulsen-conformance/src/doubles/mod.rs`, `crates/pulsen-conformance/src/doubles/command_runner.rs`, `crates/pulsen-conformance/src/doubles/process.rs`, `crates/pulsen-conformance/src/doubles/run_store.rs`, `crates/pulsen-conformance/src/doubles/task_repository.rs` — ポート適合スイートとテストダブルであり、ドメインの遷移規則ではなくアダプター契約の検証責務(AC-7 の `cfg` 隔離の確認にのみ機械的に参照した)
- スキップ: `crates/pulsen/src/adapter/mod.rs`, `crates/pulsen/src/adapter/command_runner.rs`, `crates/pulsen/src/adapter/process.rs` — アダプター層(OS 依存の隔離・符号化)であり Adapter 観点の担当
- スキップ: `crates/pulsen/src/cli/tick.rs`, `crates/pulsen/src/cli/wire.rs` — 合成ルートの配線であり CLI / 構成観点の担当
- スキップ: `crates/pulsen/examples/agent_probe.rs`, `crates/pulsen/examples/judge_probe.rs` — 受け入れテスト用のプローブでありドメインの規則を含まない
- スキップ: `crates/pulsen/tests/cli_tick.rs`, `crates/pulsen/tests/common/mod.rs`, `crates/pulsen/tests/tick_fixture/mod.rs`, `crates/pulsen/tests/conformance_command_runner.rs`, `crates/pulsen/tests/conformance_process_controller.rs` — 実バイナリ・実プロセスの受け入れ / 適合テストとその足場であり、ドメイン規則の実効的な担保はユースケース層テスト(tick_observe / tick_notify / tick_scan)で確認済み
