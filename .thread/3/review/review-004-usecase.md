### Use Case / CLI

#### Blockers

なし

#### Warnings

- **[W-001]** `TickIssue::MissingCurrentAttempt` の表示文言が launching 限定のまま、手続きD(Running)からも積まれる
  - 場所: `crates/pulsen/src/cli/render.rs:177-180`(定義側の doc は `crates/pulsen/src/application/tick/mod.rs:79-83`、新しい生成元は `crates/pulsen/src/application/tick/observe.rs:33-38`)
  - 理由: 本 PR は同変種の生成元を手続きC(Launching)だけから手続きD(Running)へ広げた(`observe.rs` の冒頭検査)。ところが表示は `"{id}: 起動記録済みですが現在 attempt がありません(タスクファイルの修復が必要です)"` のままで、実際の帳簿は `"state": "running"`(起動確認済み)である。同じ手続きDの隣の分類 `MissingProcessIdent` は正しく「起動確認済みですが同定情報がありません」と書いており、同一手続きの2つの報告で実行状態の呼び方が食い違う。ラウンド3 W-001(`TransitionError::MissingCurrentAttempt` の文言を実行状態非依存にした)とまったく同じ「意味を広げたのに文言が旧経路のまま」で、そちらは fix 済みだが `TickIssue` 側の兄弟変種が残っている。triage に該当キーは無い(ラウンド3 の Key は `cli/render.rs:311`、こちらは `:177`)。
  - 補強: `tick_notify.rs:530-538` は `TransitionError` 側について「`起動記録` と述べない」ことを表示文字列で主張しているのに、`TickIssue` 側を作る `tick_scan.rs:起動確認済みなのに同定情報がないタスクは報告してスキップする` は分類の等値だけを見ており、文言の食い違いを誰も検出できない。
  - 提案: 文言を実行状態非依存にする(例: `"{id}: 遷移の前提となる現在 attempt がありません(タスクファイルの修復が必要です)"`)。`mod.rs:79-83` の doc コメントも「起動記録済みなのに」を落として不変条件2の破れとしてだけ述べる。あわせて `tick_scan.rs` の当該ケースに `tick_summary` の文字列アサーション(`!report.contains("起動記録")`)を足し、`TransitionError` 側と同じ形で守る。

- **[W-002]** 受け入れテストの名前が「凍結」を主張しているが、凍結はサマリーに現れないケースを組んでいる
  - 場所: `crates/pulsen/tests/cli_tick.rs:933-970`(`判定と遷移と凍結と通知はサマリーに現れる`)
  - 理由: 3本目の tick は `patch_task` で `execution` を直に `stopped`(`notified_at` 無し)へ置いてから打つため、`branch_of` は `Branch::Notify` を選び `commit` を `Freeze::Frozen` で通らない。したがって `summary.frozen` は空のままで、`push_ids` は空の項目を出さないので「凍結」という語は stdout に一度も現れない。アサーションも `判定確定` / `遷移` / `通知` の3つしか無く、テスト名が検証していないことを名乗っている。結果として「凍結が実バイナリのサマリーに現れる」ことを主張する受け入れケースは1件も無い(`リトライ上限の超過は凍結を保存して同じtickで通知する` はタスクファイルだけを見る。表示は `cli/render.rs` の単体テストが見るに留まる)。CLAUDE.md の「テストは振る舞いを表す。仕様の言葉で名付ける」から外れ、ADR-092 が「書き込んだ tick が唯一の窓に現れる」ことを構成として潰した対象のうち `frozen` だけが端から端まで通っていない。
  - 提案: 帳簿を直に置く手順の前に「上限超過で実際に凍結する tick」の stdout を見るか(`retries: 0` の別タスクを1件足せば同じ tick で `凍結` と `通知` が並ぶ)、それが重いなら名前から「凍結」を外して主張と一致させる。

#### カバレッジ

- 確認: `crates/pulsen/src/application/tick/mod.rs`, `crates/pulsen/src/application/tick/notify.rs`, `crates/pulsen/src/application/tick/observe.rs`, `crates/pulsen/src/application/tick/launch.rs`, `crates/pulsen/src/application/tick/confirm_spawn.rs`, `crates/pulsen/src/cli/render.rs`, `crates/pulsen/src/cli/tick.rs`, `crates/pulsen/src/cli/wire.rs`
- 確認: `crates/pulsen/tests/tick_observe.rs`, `crates/pulsen/tests/tick_notify.rs`, `crates/pulsen/tests/tick_scan.rs`, `crates/pulsen/tests/tick_fixture/mod.rs`, `crates/pulsen/tests/cli_tick.rs`, `crates/pulsen/tests/common/mod.rs`
- 確認: `crates/pulsen-conformance/src/doubles/mod.rs`, `crates/pulsen-conformance/src/doubles/command_runner.rs`, `crates/pulsen-conformance/src/doubles/process.rs`, `crates/pulsen-conformance/src/doubles/run_store.rs`, `crates/pulsen-conformance/src/doubles/task_repository.rs`
- 確認: `.thread/3/plan.md`, `.thread/3/adr.md`(ADR-001 / ADR-015 / ADR-017 を中心に)
- スキップ: `.thread/3/review/*`(21ファイル)— レビューの中間成果物。Phase 8 で削除される
- スキップ: `.thread/3/steps.md`, `.thread/3/testing.md` — 手順書・手動確認の記帳で、本観点の対象は plan.md の受け入れ基準と adr.md の決定。General 観点が見る
- スキップ: `crates/pulsen-domain/src/execution/{judgement.rs,notification.rs,running.rs,port.rs,value.rs,mod.rs}`, `crates/pulsen-domain/src/task/{task.rs,transition.rs,counters.rs,degraded.rs}` — ドメイン層。判断の置き場所の確認に要る範囲(`interpret_notify_completion` / `NOTIFY_TIMEOUT` / `default_judgement` の値域)だけ読み、内部設計は Domain 観点
- スキップ: `crates/pulsen/src/adapter/command_runner.rs`, `crates/pulsen/src/adapter/process.rs`, `crates/pulsen/src/adapter/mod.rs` — アダプター層。排他ロック保持時間の上限の確認に要る `TERMINATION_GRACE` / `ESCALATES` / timeout の実装だけ読み、残りは Adapter 観点
- スキップ: `crates/pulsen-conformance/HOOKS.md`, `crates/pulsen-conformance/src/lib.rs`, `crates/pulsen-conformance/src/command_runner.rs`, `crates/pulsen-conformance/src/process_controller.rs`, `crates/pulsen/tests/conformance_command_runner.rs`, `crates/pulsen/tests/conformance_process_controller.rs` — ポート適合スイートと宣言。契約適合は Adapter 観点
- スキップ: `crates/pulsen/examples/agent_probe.rs`, `crates/pulsen/examples/judge_probe.rs` — 受け入れ・適合のフィクスチャ実行ファイル。`cli_tick.rs` からの使われ方(制御ファイルと環境変数の証跡)だけ確認した

#### 受け入れ基準の確認(本観点の範囲)

- AC-2(exit 0 → completed → 次 tick で next): `observe.rs:101-108` が `complete_run` で止め、遷移は `Branch::Advance` の次 tick。`tick_observe.rs:145-163` / `cli_tick.rs:551-573` で end-to-end に主張。カウンタのリセットも両方で確認。**満たす**
- AC-3(一過性失敗の自動リトライと回復): `record_run_failure` → `fail_run`、`tick_observe.rs:407-441` / `cli_tick.rs:576-602`。**満たす**
- AC-4(上限超過での凍結と at-least-once): `commit(Freeze::Frozen)` → `frozen` 記録 → 同一 tick の `notify`。順序(`SavedFrozen` → `RanNotifyCmd` → `SavedNotifiedAt`)は `RecordSeq` の共有採番で1本の列として主張されており、ポートをまたぐ逆順の実装を実際に落とせる形になっている(`tick_notify.rs:100-144,184-283`)。notify_cmd 未定義時の非記録・catch-up・DegradedTask の再通知も揃う。**満たす**
- AC-5(exit 20 の周回): `Settled::Skipped` → `skip_run` → `skipped_back`、通知なし。デフォルト判定が2値であることで exit 20 が failed になることも型と `tick_observe.rs:186-196` で担保。**満たす**
- AC-6(timeout kill と exit 記録なしの死亡検出): `observe.rs:59-66` が exit の有無を1段目として値にし、`Some` では `starttime_of` を呼ばない(`tick_observe.rs:208-217` が `processes.calls()` の空で主張)。`kill` 失敗・`starttime_of` の `Err(Io)` はいずれも書き込まず報告のみ。`DiedWithoutExit` の `try_kill_remnants` → `fail_run` → `save` の順序も `RecordSeq` で主張済み。**満たす**
- スコープ逸脱: 無し。`Branch::Cleanup` は未配線のまま(`mod.rs:443`)で、`archived` / `gc_deleted` / `gc_errors` に値の入る経路は増えていない。`Tick` のジェネリック引数の追加は `CommandRunner` 1つに閉じ、`wire::command_runner` は `compose` に載せず tick の経路だけで組んでいる

#### 個別に確認した点(問題なし)

- **配線に徹しているか**: 判定の解釈は `JudgementService` / `NotificationService`、凍結の成立は遷移関数、生死の分類は `IdentityCheck` + `RunningClassifier`。ユースケース側に残るのは `Settled`(結末とその根拠の組)と `Delivery`(notify_cmd の有無)だけで、どちらも「どのドメイン判断を呼ぶか」の配線に留まる。`Freeze` を呼び出し側が渡す形(ADR-097)も維持されている
- **報告の4分類**: `RunFailureCause` は「誰が失敗と判断したか」で分かれ、判定コマンドが失敗と判定したときにエージェントの exit 0 が失敗の根拠として読めない形になっている(`render.rs:255-274` と `cli/render.rs` の単体テストが文言で主張)。`RemnantsLeft` は `Killed` を持たず、報告を要する結末だけを型で表す。`IssueOutcome` の振り分けは ADR-098 の3分類 + ADR-017 の `CleanupLeft` に一致し、網羅 `match` で新変種の振り分け漏れが通らない
- **サマリー表示の網羅**: `save` に成功した全経路が `launched` / `confirmed_running` / `judged` / `transitioned` / `skipped_back` / `frozen` / `notified` / `errors` のいずれかを埋める。`save` に失敗した経路は `SaveFailed`、`SnapshotUnreadable` は通知の成否と独立に必ず報告されるため、書き込んだ tick・修復が要る tick が「処理対象なし」になる経路は無い(`tick_scan.rs` の破損系ケースが両方の向きで主張)
- **エラーハンドリングと exit code**: 個別タスクの失敗はすべて `errors` に落ち、`TickError` になるのは `list_active` の Io と `LockError::Failed` だけ。ロック競合は `TickOutcome::Skipped` で 0。spec のエラーケース表と一致
- **冪等性**: `KeepRunning` は書き込みを1回も起こさず(`tick_observe.rs:547-566`、`tick_scan.rs` の連続 tick)、判定・通知・凍結はいずれも永続化された事実からの再導出で閉じる。通知失敗・保存失敗のどちらも `notified_at` を残さないことをテストが主張
- **排他ロックの保持時間**: `judge_timeout`(既定60秒)× 判定対象数 + `NOTIFY_TIMEOUT`(60秒、`Some` で必ず適用)× 通知対象数 + 終了操作(`TERMINATION_GRACE` 2秒、`terminate::ESCALATES` が真なら2段で最大4秒)× kill 対象数。いずれも上限つきで、adr.md ADR-015 Consequences と testing.md:692 に明記されている。ユースケース側に新しい無期限の待ちは入っていない。plan.md:87 のリスク欄は判定・通知の2項だけを挙げるが、終了操作の項は本 PR で決まった ADR-015 が明記しており、記述の割れではなく後から足された項の追記漏れに留まる
- **経緯コメントの混入**: 変更されたユースケース・CLI・ダブルの追加コメントを機械的に走査したが、指摘への弁明・修正の経緯を残す行は無かった。残っているのはいずれも why / why not(2段規則を型で担保する理由、`Freeze` を呼び出し側から渡す理由、`RecordSeq` の採番対象を絞る理由、`retry_limit` に `applicable_retry_limit` を使わない理由 など)
