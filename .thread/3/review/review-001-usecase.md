### Use Case / CLI

#### 前提と検証の方法

- 契約は `.thread/3/plan.md`(AC-1〜AC-8)と `spec/usecases/execution.md`(共通手続き notify・処理フロー 1〜9・手続きD)、`spec/flows/index.md`(F1 / F3 / F8)、`spec/pages/index.md` §縮退状態の共通規則 ※5、`spec/testcases/execution/tick.md`(走査と分岐 / 手続きD / 共通手続き notify)を基準にした。
- 手元で品質ゲートを実行した: `cargo clippy --all-targets --all-features -- -D warnings` は無警告、`cargo test` は全スイート緑(`cli_tick` 27件 / `conformance_command_runner` 16件 / `conformance_process_controller` 27件を含む)。AC-7 の機械的な部分は満たされている。
- スコープ逸脱は見つからなかった。`Branch::Cleanup` のアームは空のまま(`// 終端処理(手続きB)は Issue #6 が入れる。`)、`archived` / `gc_deleted` / `gc_errors` に値の入る経路はなく、`abort` / `retry` / `set-status` は追加されていない。`DegradedTask` に足されたのは `mark_notified` だけ。

#### 契約との突き合わせ(合致を確認した点)

| 契約 | 実装 | 判定 |
|---|---|---|
| 共通手続き notify の順序「stopped を書く → notify_cmd → 成功時のみ `notified_at`」 | `tick/mod.rs:409-429` の `commit` が `save` 成功後にだけ `notify` を呼び、`notify.rs:50-58` が `Delivery::Sent` のときにだけ `mark_notified` → `commit` する | ○ |
| `Exited(0)` のときだけ成功。非0 / `TimedOut` / `FailedToStart` は何もしない | `notify.rs:97-108` | ○ |
| notify_cmd が None なら `notified_at` を書かない(catch-up が効く) | `notify.rs:88-90` の `Delivery::NotConfigured` は保存を一切行わない。`tick_notify.rs:122-161` が catch-up まで見ている | ○ |
| 凍結の計上を保存後の状態から導出しない(ADR-097) | 通知アームは `Freeze::NotFrozen` を通す(`notify.rs:53`)。`cli_tick.rs` の再通知ケースが「さらに次の tick では増えない」まで見ている | ○ |
| 手続きD 冒頭の不変条件2・3 の検査 | `observe.rs:35-47`。read_exit より前に両方を見ており、報告分類も分かれている(ADR-004) | ○ |
| exit が Some なら生存観測を行わない | `observe.rs:59-62`。`tick_observe.rs:128-137` が `processes.calls()` の空で主張しており、この主張は実効的 | ○ |
| `KillOnTimeout` の kill 失敗で状態を変更せず報告のみ | `observe.rs:143-148`。`tick_observe.rs:510-536` が `saved()` の空で主張 | ○ |
| `DiedWithoutExit` は `try_kill_remnants` → `fail_run`、結果は報告のみ | `observe.rs:151-159`。`tick_observe.rs:560-582` が `ProcessControllerCall` の並びで順序を主張しており実効的 | ○ |
| `starttime_of` の `Err(Io)` で状態を変更しない | `observe.rs:114-124` | ○ |
| `Corrupt` は報告のみ / `Completed` は `advance` → `save`(`TransitionError` は報告してスキップ)/ `Stopped` は未通知のみ notify | `tick/mod.rs:337-368`・`393-402`・`381-385` | ○ |
| 1タスク1tick1ステップ(completed の遷移は次の tick) | `observe.rs:87-94` は `complete_run` の保存で止まる。`tick_observe.rs:65-83` と `cli_tick.rs` の受け入れテストが実バイナリで一周を確認 | ○ |
| 1タスクの失敗を `errors` に積んで続行 / tick パスの exit は 0 / `list_active` の Io だけ非0 | `tick/mod.rs:317-334`・`cli/mod.rs:42-56` | ○ |
| ADR-092 / ADR-005: 書き込んだ経路は必ずサマリーに現れる | `judged` の新設で `complete_run` が拾われ、`skip_run`→`skipped_back` / `advance`→`transitioned` / `mark_notified`→`notified` / `fail_run`・`record_judge_failure`→`errors` が埋まる。`is_empty` も更新済み(`tick/mod.rs:246-258`)。`cli_tick.rs` の「判定と遷移と凍結と通知はサマリーに現れる」が実出力で見ている | ○ |
| 排他ロックの保持時間 | `execute` が `_guard` を全走査のあいだ保持し、その中で `judge_timeout` / `NOTIFY_TIMEOUT` の待機に入る。ADR-018 の承知のうえの設計で、`SystemCommandRunner` は必ず timeout を渡される(`observe.rs:79-80` / `notify.rs:95`)ので上界が閉じている | ○ |

配線の分離も守られている。判定の解釈は `JudgementService`、生存の分類は `IdentityCheck` / `RunningClassifier`、通知 env は `NotificationService`、上限超過の判断は `Task` の遷移関数にあり、`observe.rs` / `notify.rs` は観測 → 判断 → 実行の順に並べているだけである。唯一の例外が W-002。

#### Blockers

なし

#### Warnings

- **[W-001]** 報告の文言をユースケース層で組み立てており、ADR-081(`.adr/081-tick-errors-are-structured-values.md`)の「原因は分類として持ち、`cli::render` が文言へ落とす」に反する
  - 場所: `crates/pulsen/src/application/tick/observe.rs:240-272`(`judgement_detail` / `timeout_detail` / `report_remnants`)、`crates/pulsen/src/cli/render.rs:222-231`
  - 理由: `TickIssue::RunFailed { message }` と `RemnantsUnhandled { message }` に入るのは完成した日本語文(「実行が終了コード 1 で終了しました」「残存プロセスを誤殺なく同定できませんでした(終了操作は行っていません)」)で、`render` は `format!("{}: {message}")` と素通しするだけになっている。既存の `WorktreeCreateFailed { message }` はポートが返した不透明な原因を運んでいるのに対し、こちらはユースケースが著者である。`JudgeFailed { detail }` は `record_judge_failure` で帳簿に永続化される値の再利用なので `.thread/2/adr.md` の例外に載るが、`RunFailed` / `RemnantsUnhandled` はどこにも永続化されない表示専用の文字列で、例外の根拠がない。実害として、ユースケース層のテストが原因を分類で主張できず(`tick_observe.rs` は `matches!(_, TickIssue::RunFailed { .. })` までしか書けない)、「timeout で失敗した」と「exit を残さず死んだ」と「判定 failed」の取り違えを検出できない。付随して、判定コマンドが exit 10 を返した場合でも文言はエージェント側の exit code を載せる(`observe.rs:103` が `judgement_detail(&exit)` を渡す)ため、表示から失敗の由来が読めない
  - 提案: `RunFailed { task_id, cause: RunFailureCause }`(`Judged(ExitCode)` / `JudgedByCommand(ExitCode)` / `TimedOut(TimeoutSpec)` / `DiedWithoutExit`)と `RemnantsUnhandled { task_id, outcome: RemnantOutcome }` にして、文言は `cli::render` の網羅 `match` で組む。`RemnantOutcome` はドメインの公開型なのでそのまま運べる

- **[W-002]** 通知の結末の解釈(`Exited(0)` だけが成功)がユースケースの private メソッドにあり、ADR-003 が Consequences に書いた「#5 の `AbortTask` が同じ関数を呼べる」が構造上成立しない
  - 場所: `crates/pulsen/src/application/tick/notify.rs:87-110`(`fn deliver`)
  - 理由: `deliver` は `impl Tick` の非公開メソッドで、`Tick` のジェネリック引数7つに束縛されている。`AbortTask`(#5)は `Tick` を持たないので、同じ判断を書き直すしかない。判定側は同型の解釈が `JudgementService::interpret_judge_completion` としてドメインの純粋関数に置かれているのに、通知側だけ非対称にユースケースへ残っている。CLAUDE.md の「判断はドメイン、ユースケースは配線」および `spec/usecases/execution.md` 冒頭の原則にも照らして、規則の実体が2箇所に分かれる риск を今のうちに閉じたい。共通手続き notify は「stopped を書いた**すべての**経路」が使う契約なので、複製は at-least-once の破れが片方だけに入る形の事故になりやすい
  - 提案: `NotificationService` に `interpret_notify_completion(&CommandCompletion) -> NotifyOutcome`(`Delivered` / `Failed { detail }`)を足してドメインへ寄せ、`Delivery` はユースケースの分岐(`NotConfigured` の有無)だけを持つ。あるいは `deliver` を `Tick` から切り離した自由関数 `deliver(commands, config, id, workflow, status)` にして #5 から呼べるようにする

- **[W-003]** `RunningDecision::Judge` が本番コードのどこからも構築されず、ユースケース側に `unreachable!()` を1つ残している
  - 場所: `crates/pulsen/src/application/tick/observe.rs:160-162`、`crates/pulsen-domain/src/execution/running.rs:20-30`
  - 理由: plan.md は「`classify_alive` に `Judge` を返させない**型の形**がこの規則の担保になる」と書いていたが、実装は4値の `RunningDecision` を共有したまま doc コメントで「返さない」と述べるに留めた。結果として担保はコメントとパニックになっている。`grep` の限りで `RunningDecision::Judge` を構築するのは `running.rs` のユニットテスト1箇所だけで、本番の生存経路は3値しか使わない。tick の走査ループはパニックを捕まえないので、万一到達すればそのタスクだけでなく tick パス全体が落ち、`spec/usecases/execution.md` の「1タスクの処理失敗は `errors` に記録して残りを続行する」を破る
  - 提案: `classify_alive` の戻りを3値の型(例: `AliveDecision { KeepRunning, KillOnTimeout, DiedWithoutExit }`)にし、`RunningDecision` は 2 段規則を語る型として残すか削る。`observe.rs` から網羅 `match` の第4アームが消え、`unreachable!` も不要になる

- **[W-004]** スナップショット破損 × 未通知 stopped × notify_cmd 未定義のタスクは、tick が書き込みも報告も行わず、サマリーが「処理対象のタスクはありませんでした。」になる
  - 場所: `crates/pulsen/src/application/tick/mod.rs:347-365`、`crates/pulsen/src/application/tick/notify.rs:66-70`
  - 理由: `SnapshotUnreadable` の分岐は `Stopped { notified_at: None }` を `notify_degraded` へ回し、その場合 `TickIssue::SnapshotUnreadable` を積まない。通知が成功すれば `notified` に、失敗すれば `NotifyFailed` に現れるが、`notify_cmd` が未定義のとき(= `GlobalConfig` の既定)は `Delivery::NotConfigured` で何も起きず、この破損タスクは tick の出力から完全に消える。stopped は人間の介入まで滞留する準終端なので、この不可視は永続する。`spec/pages/index.md` の tick × スナップショット破損のセルは「0 スキップ+報告」で、※5 の但し書きは通知を**足す**読みも自然に取れる(実装は報告を**置き換える**読みを取っている)。※5 が修復の入口として `ls` を挙げている以上ただちに契約違反とまでは言い切れないが、既定構成で最も起きやすい組み合わせが黙るのは運用上の穴である
  - 提案: `notify_degraded` が `Delivery::NotConfigured` を受けたとき(あるいは常に)`TickIssue::SnapshotUnreadable` を積む。`tick_scan.rs` の「スナップショット破損の凍結以外は…」に「notify_cmd 未定義の未通知凍結も報告される」ケースを足す

- **[W-005]** 「stopped の `save` → notify_cmd 実行」の順序が実効的に検証されていない
  - 場所: `crates/pulsen/tests/tick_notify.rs:70-119`(`通知は凍結の保存が済んでから実行され成功してはじめて追記される`)
  - 理由: plan.md のテスト方針は「`ScriptedTaskRepository` と `ScriptedCommandRunner` の**呼び出し記録の並び**で主張する」と書いているが、2つのダブルは独立したベクタに記録するため並びを突き合わせられない。実際のアサーションは `saved[0].notified_at == None` / `saved[1].notified_at == Some(_)` で、これは2回の `save` の順序しか見ていない。「notify_cmd を先に起動してから stopped を保存する」実装でもこのテストは緑のまま通る。後半(通知 → `mark_notified`)は `通知が失敗した凍結には…`(`tick_notify.rs:164-198`)が `saved()` の空で確実に守っているので、破れが残るのは前半だけである。前半が破れると、保存前にクラッシュした tick が「通知は飛んだが凍結は記録されていない」状態を作る(欠落ではなく誤報側だが、requirements §8 の「書く → 実行 → 追記」の字義には反する)
  - 提案: `ScriptedCommandRunner` に「呼び出し時に観測するフック」を持たせるか、両ダブルが共有する単調増加のシーケンス番号を `CommandRunnerCall` / 保存記録に持たせて並びを1本の列で主張する。安価な代替として、`with_run` の最初の結末を返す直前に `tasks.saved().len() == 1` を確認できる形(ハーネス側に `Rc` で共有した記録を持つ)でもよい

- **[W-006]** 残存プロセスの結末の報告を `save` が成功したときだけ積んでいる
  - 場所: `crates/pulsen/src/application/tick/observe.rs:151-159`
  - 理由: `RemnantOutcome::NotIdentifiable` / `Failed` は「OS 上にプロセスが残っているかもしれない」という、タスクファイルを書けたかどうかとは直交する事実である。コメントは「書けていない tick の報告は `SaveFailed` が正しい」と述べるが、両方を積んではいけない理由にはなっていない。保存が失敗した tick でこそ、残存プロセスの情報は人間の後始末(monitoring.md が OS ツールでの後始末を求めている)に必要になる
  - 提案: `report_remnants` は `Persisted` によらず常に呼ぶ。ADR-092 の不変は `SaveFailed` が埋めるので、サマリーが空になる心配はない

- **[W-007]** 判定コマンド定義済み × `workspace` 未設定の分岐が、遷移を試みていないのに `TransitionError::WorkspaceNotSet` で報告され、かつテストがない
  - 場所: `crates/pulsen/src/application/tick/observe.rs:73-77`
  - 理由: ADR-004 は不変条件3の破れに新分類を足す判断を「破れの事実も人間に求める修復も違う」を根拠に下したのに、不変条件4の破れ(Running なのに workspace がない)はその判断が及ばず、遷移エラーの語彙に相乗りしている。ADR-096 が許した相乗りは「同じ事実の別文脈での報告」だったが、ここでは遷移関数を一度も呼んでいないので同じ事実ですらない。表示は「遷移の前提が成立しません(ワークスペースが未確定)」となり、実際に起きたこと(判定コマンドへ渡す `WORKSPACE` を作れなかった)から離れる。加えて `tick_fixture` の `TaskBuilder::running` が必ず `workspace()` を通すため、この分岐を通るテストは1件もない
  - 提案: `TickIssue` に分類を足す(`MissingWorkspace { task_id }` 等)か、少なくともこの相乗りを選んだ why をコードに残す。いずれにせよ `tick_observe.rs` に「judge 定義あり・workspace なしでは判定コマンドを起動せず書き込まない」ケースを足して分岐を閉じる

- **[W-008]** テストのコメントが振る舞いではなくテストの組み立て事情を説明しており、しかも二重否定で意味が反転している。主張も弱い
  - 場所: `crates/pulsen/tests/tick_notify.rs:276-285`
  - 理由: 「起動へ進まないよう、エージェント定義を持たない設定にしていないため、走査の結果として通知が起きないことだけを見る。」— 実際には `config_notifying` が `agents` を持たないおかげで展開失敗の経路に落ちて起動へ進まない、という事情の説明であり、字義は逆を述べている。CLAUDE.md の「残すのは現在の形が成り立つ理由(why / why not)だけ」に照らして、これは仕掛けの言い訳に近い。加えて `let _ = harness.run();` で結果を捨てており、`tasks.saved()` に `notified_at` が書かれていないことも見ていないため、テスト名(「通知の対象にならない」)の主張が `commands.calls().is_empty()` だけに乗っている
  - 提案: コメントを削るか「凍結でない実行状態は通知アームへ入らない」という規則そのものの説明に置き換える。`Harness` を `Branch::Notify` に入らない状態(例: `stopped_notified` 以外の全状態)で回し、`saved()` に `Stopped` が現れないことも併せて主張すると、実装が状態判別を誤ったときに落ちる

#### 参考: Blocker に至らないと判断した箇所

- **`errors` に正常な記録系(`RunFailed` / `JudgeFailed`)が混ざる**: ADR-094 が既に決着させており、`cli/render.rs` の `IssueOutcome::Recorded`(「失敗を記録」)で表示上は分離されている。spec の `errors` の定義(「スキップ・破損・観測失敗・kill失敗**等**の報告」)にも収まる
- **`SystemCommandRunner` が判定・通知の stdout/stderr を捕捉せず tick の出力へ流す**: ポート契約(`port.rs`)が明示的にそう定めており、サマリーの前に判定コマンドの出力が混ざるのは仕様どおり。`stdin(Stdio::null())` を置いている点はむしろ良く、判定コマンドが tick の標準入力を掴んでロックを保持したまま止まる事故を防いでいる
- **判定・通知のあいだ排他ロックを保持する**: ADR-018 / plan.md が承知のうえで組み込み timeout を置いた設計で、本スライスは新しい緩和を入れていない。上界は `judge_timeout × 判定対象数 + 60秒 × 凍結数` で閉じており、`POLL_INTERVAL = 50ms` のポーリングも待機中の負荷として妥当
- **`SystemCommandRunner::run` の `expect`**: `PlainCommand::parse` が空トークンを拒否する(`definition/command.rs:44`)ので不変条件違反にのみ使うパニックである

#### カバレッジ

- 確認: `.thread/3/adr.md`, `.thread/3/plan.md`, `crates/pulsen-conformance/src/command_runner.rs`(ケース一覧とポート契約の対応のみ), `crates/pulsen-conformance/src/doubles/command_runner.rs`, `crates/pulsen-conformance/src/doubles/mod.rs`, `crates/pulsen-conformance/src/doubles/process.rs`, `crates/pulsen-conformance/src/doubles/run_store.rs`, `crates/pulsen-conformance/src/doubles/task_repository.rs`, `crates/pulsen-domain/src/execution/judgement.rs`, `crates/pulsen-domain/src/execution/notification.rs`, `crates/pulsen-domain/src/execution/port.rs`, `crates/pulsen-domain/src/execution/running.rs`, `crates/pulsen-domain/src/execution/value.rs`, `crates/pulsen-domain/src/task/degraded.rs`, `crates/pulsen-domain/src/task/task.rs`, `crates/pulsen-domain/src/task/transition.rs`, `crates/pulsen/examples/agent_probe.rs`, `crates/pulsen/examples/judge_probe.rs`, `crates/pulsen/src/adapter/command_runner.rs`, `crates/pulsen/src/application/tick/confirm_spawn.rs`, `crates/pulsen/src/application/tick/launch.rs`, `crates/pulsen/src/application/tick/mod.rs`, `crates/pulsen/src/application/tick/notify.rs`, `crates/pulsen/src/application/tick/observe.rs`, `crates/pulsen/src/cli/render.rs`, `crates/pulsen/src/cli/tick.rs`, `crates/pulsen/src/cli/wire.rs`, `crates/pulsen/tests/cli_tick.rs`, `crates/pulsen/tests/common/mod.rs`, `crates/pulsen/tests/tick_fixture/mod.rs`, `crates/pulsen/tests/tick_notify.rs`, `crates/pulsen/tests/tick_observe.rs`, `crates/pulsen/tests/tick_scan.rs`
- スキップ: `.thread/3/steps.md` — 手順分解であり、受け入れ基準の契約は plan.md が持つ
- スキップ: `.thread/3/testing.md` — 手動確認の記録。自動テストの実効性は testcases と実テストで判定した
- スキップ: `crates/pulsen-conformance/HOOKS.md` — 適合ハーネスの環境フック文書でポート/アダプター観点
- スキップ: `crates/pulsen-conformance/src/lib.rs` — 適合スイートのエントリ整備でポート観点
- スキップ: `crates/pulsen-conformance/src/process_controller.rs` — ProcessController の適合ケース本体でポート観点
- スキップ: `crates/pulsen-domain/src/execution/mod.rs` — 再エクスポートのみ
- スキップ: `crates/pulsen-domain/src/task/counters.rs` — カウンタのリセット規則でドメイン観点
- スキップ: `crates/pulsen/src/adapter/mod.rs` — モジュール宣言のみ
- スキップ: `crates/pulsen/src/adapter/process.rs` — kill / starttime_of の OS 依存実装でアダプター観点
- スキップ: `crates/pulsen/tests/conformance_command_runner.rs` — 適合スイートの実行側でポート観点
- スキップ: `crates/pulsen/tests/conformance_process_controller.rs` — 同上
