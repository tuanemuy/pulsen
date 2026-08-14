### Domain

#### Blockers

なし

#### Warnings

- **[W-001]** `default_judgement` の返り値型が「2値しか返さない」規則を述べておらず、呼び出し側に到達不能な `Skipped` アームが残っている
  - 場所: `crates/pulsen-domain/src/execution/judgement.rs:25-31`、`crates/pulsen/src/application/tick/observe.rs:269-275`
  - 理由: `spec/domains/execution.md#judgementservice` と台帳 `DOM-execution-019` は `default_judgement` を「0 = `Completed`、非0 = `Failed`(2値。`Skipped` は返さない)」と定めるが、返り値は3値の `JudgeOutcome` のままで、規則の担保が doc コメントにしか無い。実際に `Settled::by_default` は `JudgeOutcome::Skipped => Self::Skipped` という**生きたアーム**を持ち、万一この経路が構築されれば判定コマンドを持たないステータスで skipped 周回が起きる — ADR-008 が禁じた挙動が、型ではなく規約でしか止まっていない。これは `.thread/3/adr.md` ADR-009 が `classify_alive` に対して「規則の担保がコメントとパニック経路になっている」として `AliveDecision` を導入したのと同じ形の残りであり、判定側だけが対称の手当てを受けていない
  - 提案: ADR-009 と同じ手を判定側にも当てる。`DefaultJudgement`(`Completed` / `Failed`)を足して `default_judgement` の返り値をそれに絞り、`From<DefaultJudgement> for JudgeOutcome` で3値へ埋め込む(`JudgeOutcome` は `DOM-execution-004` の3値要件があるため残す)。`Settled::by_default` の網羅 `match` は2アームになり、「デフォルト判定から skipped は導かれない」が型の主張になる

- **[W-002]** `Freeze::of_recorded_failure` の前提コメントが、本スライスで増えた呼び出し元を数えていない
  - 場所: `crates/pulsen/src/application/tick/mod.rs:533-535`
  - 理由: doc は「前提: 遷移前は凍結ではない(**3つの記録系遷移**は起動待ち・失敗確定・**起動記録済み**しか受け付けない)」と書くが、本スライスで `fail_run` / `record_judge_failure`(いずれも前提は**起動確認済み**)が呼び出し元に加わり、実際は5箇所・4状態になっている。結論(遷移後の `Stopped` = 今回凍結した)は依然成り立つものの、成り立つ**理由**として挙げられた集合が実際の集合と食い違っており、読み手が前提を検算できない。CLAUDE.md の「残すのは現在の形が成り立つ理由だけ」から外れる
  - 提案: 前提の列挙を実際の呼び出し元(`record_spawn_failure` / `record_spawn_failure_in_place` / `record_tool_failure` / `fail_run` / `record_judge_failure`)が受け付ける状態、すなわち「起動待ち・失敗確定・起動記録済み・起動確認済みのいずれか」に直す。件数の明示(「3つの」)は増減のたびに腐るので落とす

#### 確認した点(問題なし)

以下は仕様と突き合わせて一致を確認した。

- 遷移の事後条件が `spec/domains/task.md#振る舞い遷移関数` と一致する: `complete_run` / `skip_run` は `attempt_count` / `judge_attempt_count` を 0 に戻し `spawn_fail_count` を触らない、`fail_run` は `attempt_count += 1` かつ `judge_attempt_count = 0`、`record_judge_failure` は `judge_attempt_count += 1` と `last_failure = JudgeFail` のみで `Running` に留まる、`advance` は `next` へ進めて `Pending` に戻しカウンタを触らない、`mark_notified` は `notified_at` のみを書く。いずれも `self` を消費して新しい値を返す純粋関数で、`..self` により `current_attempt` / `workspace` が保たれる(不変条件5・6)
- 上限判定は `limit_exceeded(count, limit) = count > limit` の1箇所に集約され、等号で凍結せず超過で凍結する(`retries: 0` の即時凍結を含めてドメイン・ユースケース両層でテスト済み)
- `ensure_running` / `ensure_completed` の検査は過剰でも過少でもない。前者は不変条件2・3(`Running` ⇒ `current_attempt.process` が `Some`)、後者は不変条件2(と、そこから導かれる不変条件5 の根拠)だけを見ており、`Stopped` に不変条件2・3 を課さない `mark_notified` とも整合する。不変条件4 は「`Launching` 以降に進むとき」の条件であり `record_launching` 側の `WorkspaceNotSet` が引き続き担い、判定コマンドの `WORKSPACE` を組めない破れは ADR-008 のとおり tick の報告分類になっている
- `AliveDecision`(3値)と `RunningDecision`(4値)の関係が2段規則の担保になっている。`classify_alive` は `Judge` を構築できず、1段目は `observe.rs` が `RunningDecision::Judge(exit)` として値にしてから `.into()` で合流させる。`From` の写像は3値がそのまま対応し、台帳 `DOM-execution-008` の4値要件も満たす
- `interpret_judge_completion` は 0 / 10 / 20 / それ以外の4値で、`TimedOut` / `FailedToStart` を含む3原因が `detail` から判別できる。`judge_env` は4変数・`EXIT_CODE` は10進文字列
- `interpret_notify_completion` は `Exited(0)` だけを `Delivered` とし、非0・`TimedOut`・`FailedToStart` を `Failed` に落とす。規則がドメインの1関数に閉じたことで `Task` / `DegradedTask` の2本の通知アームが同じ判断を共有し、`NOTIFY_TIMEOUT = 60秒` も定数として1箇所にある
- 冪等性: `KeepRunning` は書き込みを1つも起こさず、保存に失敗した判定・通知は帳簿を変えずに次の tick が同じ結論を再導出する。`Stopped` は常に `notified_at: None` で記録され(`stopped()` ヘルパー)、過去の通知記録を引き継がない
- ドメインの純粋性: `pulsen-domain/Cargo.toml` の `[dependencies]` は空のまま、`std::fs` / `std::process` / `std::io` / `SystemTime` の参照は1件も無く、`crates/pulsen-domain/` にターゲット述語つき `cfg` は無い。本番コードに `unreachable!` / `panic!` / `unwrap()` は無い(新規の `unreachable!` は3件ともテストの補助関数)
- 網羅 `match`: 新規コードのドメイン enum に対する `_` は無く、`match exit.get()` の `other` アームだけが整数マッチ(網羅不能)。`cargo clippy -p pulsen-domain --all-targets -- -D warnings` は `wildcard_enum_match_arm` を含めて無警告、`cargo test -p pulsen-domain` は 270 件通過
- テストの実効性: ドメインのユニットテストは仕様の言葉で名付けられ、6状態それぞれからの呼び出し・上限の等号と +1・`retries: 0`・巻き戻り・`Unlimited`・`AliveDecision` の写像・`TransitionError` 6変種の非同値・`DegradedTask::mark_notified` の3ケースを網羅する。実装内部ではなく事後条件を主張しており、`tick_observe.rs` / `tick_notify.rs` がユースケース層から同じ規則(カウンタのリセット・timeout 境界・通知の順序と at-least-once)を behavior として重ねている
- 経緯・弁明のコメント: 本スライスで追加されたドメインのコメントはいずれも why / why not(なぜ2値に絞るか、なぜ `spawn_fail_count` を触らないか、なぜ逆順にしないか)であり、指摘への弁明や修正の経緯は残っていない
- スコープ: `pulsen-domain` の変更は `abort` / `retry` / `set_status`(#5)・`GcPolicy` / `RemoveOutcome`(#6)・`attempt_exists`(#4)に手を付けておらず、「含まれないもの」を越えていない

#### カバレッジ

- 確認: `.thread/3/plan.md`, `.thread/3/adr.md`, `.thread/3/review/triage.md`, `crates/pulsen-conformance/src/command_runner.rs`, `crates/pulsen-domain/src/execution/judgement.rs`, `crates/pulsen-domain/src/execution/mod.rs`, `crates/pulsen-domain/src/execution/notification.rs`, `crates/pulsen-domain/src/execution/port.rs`, `crates/pulsen-domain/src/execution/running.rs`, `crates/pulsen-domain/src/execution/value.rs`, `crates/pulsen-domain/src/task/counters.rs`, `crates/pulsen-domain/src/task/degraded.rs`, `crates/pulsen-domain/src/task/task.rs`, `crates/pulsen-domain/src/task/transition.rs`, `crates/pulsen/src/application/tick/mod.rs`, `crates/pulsen/src/application/tick/notify.rs`, `crates/pulsen/src/application/tick/observe.rs`, `crates/pulsen/tests/tick_notify.rs`, `crates/pulsen/tests/tick_observe.rs`
- スキップ: `.thread/3/review/plan-001.md`, `.thread/3/review/review-001.md`, `.thread/3/review/review-001-domain.md`, `.thread/3/review/review-001-adapter.md`, `.thread/3/review/review-001-usecase.md`, `.thread/3/review/review-001-general.md` — 前ラウンドの中間成果物(Phase 8 で削除)で、ゼロベースの判断材料にしない
- スキップ: `.thread/3/steps.md` — 実装の進行管理であり遷移規則を定めない
- スキップ: `.thread/3/testing.md` — 手動確認の手順書でドメインの主張を含まない
- スキップ: `crates/pulsen-conformance/HOOKS.md` — 適合ハーネスの環境要件の文書(アダプター観点)
- スキップ: `crates/pulsen-conformance/src/doubles/command_runner.rs`, `crates/pulsen-conformance/src/doubles/mod.rs`, `crates/pulsen-conformance/src/doubles/process.rs`, `crates/pulsen-conformance/src/doubles/run_store.rs`, `crates/pulsen-conformance/src/doubles/task_repository.rs` — テストダブルの実装(ユースケース観点)
- スキップ: `crates/pulsen-conformance/src/lib.rs`, `crates/pulsen-conformance/src/process_controller.rs` — 適合スイートの配線とプロセス操作の契約検証(アダプター観点)
- スキップ: `crates/pulsen/examples/agent_probe.rs`, `crates/pulsen/examples/judge_probe.rs` — 受け入れテスト用のプローブ実体
- スキップ: `crates/pulsen/src/adapter/command_runner.rs`, `crates/pulsen/src/adapter/mod.rs`, `crates/pulsen/src/adapter/process.rs` — ポート実装(アダプター観点)
- スキップ: `crates/pulsen/src/application/tick/confirm_spawn.rs`, `crates/pulsen/src/application/tick/launch.rs` — 変更はジェネリック引数 `C` の追随のみで判断の内容が変わっていない
- スキップ: `crates/pulsen/src/cli/render.rs`, `crates/pulsen/src/cli/tick.rs`, `crates/pulsen/src/cli/wire.rs` — 表示文言と合成ルート(general 観点)
- スキップ: `crates/pulsen/tests/cli_tick.rs`, `crates/pulsen/tests/common/mod.rs`, `crates/pulsen/tests/tick_fixture/mod.rs`, `crates/pulsen/tests/tick_scan.rs` — 受け入れテストと共有フィクスチャ(general / ユースケース観点)
- スキップ: `crates/pulsen/tests/conformance_command_runner.rs`, `crates/pulsen/tests/conformance_process_controller.rs` — 適合スイートの実アダプターへの適用(アダプター観点)
