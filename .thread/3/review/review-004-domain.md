### Domain

#### Blockers

なし

#### Warnings

なし

このラウンドで Domain 観点の指摘はゼロ。以下を確認したうえでの判定。

**関数型ドメインモデリング**

- `Task::{complete_run, skip_run, fail_run, record_judge_failure, advance, mark_notified}` と `DegradedTask::mark_notified` はいずれも `self` を消費して新しい値を返し、失敗は `Result<_, TransitionError>` の値として返る。パニックは1つも増えていない。
- 実行状態を分岐するすべての `match` がワイルドカードなしで6値を列挙している(`ensure_running` / `ensure_completed` / `mark_notified_state` / `branch_of` / `dispatch` / `written` / `died_without_exit_steps`)。`RemnantOutcome` / `JudgeConclusion` / `CommandCompletion` / `NotifyOutcome` / `AliveDecision` / `DefaultJudgement` の各 `match` も同様。`crates/pulsen-domain/src` の `_ =>` 4件はいずれも本 PR の変更外(`definition/duration.rs` / `assembler.rs` / `template.rs` / `task/time.rs`)で、enum ではなく文字・数値のパターンに対するもの。
- 上限判定は `limit_exceeded(count, limit) = count > limit` の1箇所に集約され、3つの上限経路が同じ関数を通る。

**型による担保**

- `DefaultJudgement`(2値)/ `JudgeOutcome`(3値)、`AliveDecision`(3値)/ `RunningDecision`(4値)の関係が `From` で写像として書かれ、どちらも網羅 `match` で実装されている。台帳の値数要件(`DOM-execution-004` 3値・`DOM-execution-008` 4値・`DOM-execution-019` 2値)を満たす。
- `classify_alive` が `AliveDecision` を返すことで「生存観測からは `Judge` が導かれない」を型が述べ、1段目(exit の有無)は `application/tick/observe.rs:57-66` で値になる。`plan.md:101` が spec 差分として明示している配置と一致。
- `ensure_running` が不変条件2・3(現在 attempt とその同定情報)を、`ensure_completed` が不変条件2を、遷移の前提として検査する。`spec/domains/task.md:164` の「不変条件2〜4 は遷移関数が前提として検査する」に一致し、規則の実体がドメイン内にある。

**spec の遷移規則との一致**

- `complete_run` / `skip_run` = `attempt_count = 0` / `judge_attempt_count = 0`、`spawn_fail_count` 不変。`fail_run` = `attempt_count += 1` / `judge_attempt_count = 0`。`record_judge_failure` = `judge_attempt_count += 1` / `last_failure = JudgeFail` / 未超過なら `Running` 維持。`advance` = `next` 参照 + `Pending`、非 AgentRun は `NotAgentRunStatus`。`mark_notified` = 未通知の `Stopped` のみ。いずれも `spec/domains/task.md:201-208` の表と一致。
- `stopped()` が常に `notified_at: None` を書き、過去の通知記録を引き継がない(`spec/domains/task.md:151`)。
- timeout は `now.elapsed_since(&starttime.wall) > limit.seconds()`(ともに u64・巻き戻りは `elapsed_since` が 0 に飽和)で、起点・境界・飽和が `spec/domains/execution.md:109-113` どおり。

**ドメイン層の純粋性**

- `crates/pulsen-domain/Cargo.toml` の `[dependencies]` は空のまま。`crates/pulsen-domain/` に `cfg(unix|windows|target_os|target_family)` は1件も無い。`unsafe_code = "forbid"` も維持。新規3ファイル(`judgement.rs` / `notification.rs` / `running.rs`)は std のみに依存し、I/O を持たない。

**冪等性**

- `RunningDecision::KeepRunning` / `Branch::AlreadyNotified` / `Delivery::NotConfigured` のいずれも書き込みを1回も起こさない。通知の順序は `Freeze::Frozen` のアームが `frozen` の計上と `notify` を同じ位置に置き、catch-up の保存は `Freeze::NotFrozen` を通す(`mod.rs:476-480`, `notify.rs:57`)ので、過去の凍結が再計上されない。

**テストの実効性**

- `cargo test -p pulsen-domain` は 271 件が緑。遷移5種 + `mark_notified` について、6状態それぞれからの前提不一致(`every_execution_state` を `ExecutionStateKind::COUNT` 長の配列で回す)・上限の等号と +1・`retries: 0` の即凍結・不変条件2 と 3 の破れ・`TransitionError` 6変種の相互非同値が主張されている。
- ポートをまたぐ順序の契約は、`ScriptedProcessController` / `ScriptedTaskRepository` / `ScriptedCommandRunner` の共有採番(`RecordSeq`)で1本の列に並べ直してから主張しており(`tick_observe.rs:107-141` の `died_without_exit_steps`、`tick_notify.rs:100-145` の `notify_steps`)、片側だけを見て緑になる形になっていない。

**コメント**

- 差分に入った `//` / `///` / `//!` を経緯・弁明の語彙(ラウンド・指摘・修正・以前・当初・TODO・仮 など)で走査したが、該当なし。残っているのはすべて why / why not。ラウンド3 で指摘された「将来の #3 が入れる」型の前方参照コメント(`mod.rs` の `Freeze` / `Branch`、走査アーム)は現在の呼び出し元を指す記述に置き換わっている。

#### カバレッジ

- 確認: `crates/pulsen-domain/src/execution/judgement.rs`, `crates/pulsen-domain/src/execution/notification.rs`, `crates/pulsen-domain/src/execution/running.rs`, `crates/pulsen-domain/src/execution/value.rs`, `crates/pulsen-domain/src/execution/port.rs`, `crates/pulsen-domain/src/execution/mod.rs`, `crates/pulsen-domain/src/task/task.rs`, `crates/pulsen-domain/src/task/transition.rs`, `crates/pulsen-domain/src/task/counters.rs`, `crates/pulsen-domain/src/task/degraded.rs`, `crates/pulsen/src/application/tick/mod.rs`, `crates/pulsen/src/application/tick/observe.rs`, `crates/pulsen/src/application/tick/notify.rs`, `crates/pulsen/src/application/tick/launch.rs`, `crates/pulsen/src/application/tick/confirm_spawn.rs`, `crates/pulsen/src/cli/render.rs`, `crates/pulsen/tests/tick_observe.rs`, `crates/pulsen/tests/tick_notify.rs`, `crates/pulsen/tests/tick_scan.rs`, `crates/pulsen-conformance/src/doubles/process.rs`
- スキップ: `crates/pulsen/src/adapter/command_runner.rs`, `crates/pulsen/src/adapter/process.rs`, `crates/pulsen/src/adapter/mod.rs` — アダプター層の OS 依存実装で Adapter 観点の担当
- スキップ: `crates/pulsen-conformance/src/command_runner.rs`, `crates/pulsen-conformance/src/process_controller.rs`, `crates/pulsen-conformance/src/lib.rs`, `crates/pulsen-conformance/src/doubles/mod.rs`, `crates/pulsen-conformance/src/doubles/command_runner.rs`, `crates/pulsen-conformance/src/doubles/run_store.rs`, `crates/pulsen-conformance/src/doubles/task_repository.rs`, `crates/pulsen-conformance/HOOKS.md` — ポート適合スイートとハーネスで Adapter 観点の担当(順序採番の利用側だけ確認済み)
- スキップ: `crates/pulsen/tests/conformance_command_runner.rs`, `crates/pulsen/tests/conformance_process_controller.rs`, `crates/pulsen/tests/cli_tick.rs`, `crates/pulsen/tests/common/mod.rs`, `crates/pulsen/tests/tick_fixture/mod.rs` — 適合・受け入れテストの配線で Adapter / General 観点の担当
- スキップ: `crates/pulsen/src/cli/wire.rs`, `crates/pulsen/src/cli/tick.rs` — 合成ルートで Usecase 観点の担当
- スキップ: `crates/pulsen/examples/agent_probe.rs`, `crates/pulsen/examples/judge_probe.rs` — テスト用フィクスチャの実行ファイル
- スキップ: `.thread/3/` 配下の全ファイル(`plan.md` / `adr.md` / `steps.md` / `testing.md` / `review/*`) — 契約・計画・レビューの中間成果物で Phase 8 で削除される
