### Domain

ゼロベースで再レビューした。関数型ドメインモデリング(不正な状態を型で表現不能にする・OR は enum / AND は struct・parse, don't validate・エラーは値・`match` にワイルドカードなし・`self` を消費する純粋な遷移)、`pulsen-domain` の純粋性(`[dependencies]` は空・`unsafe_code = "forbid"`・ターゲット述語つき `cfg` は0件)、`spec/domains/task.md` の遷移表・不変条件・上限判定の境界との一致、`spec/domains/execution.md` の2段規則と判定プロトコル、前ラウンドで入った `DefaultJudgement` / `AliveDecision` の型としての担保と台帳(`DOM-execution-004` / `008` / `019`)の値数要件、冪等性(永続化された事実からの再導出)、テストの実効性を確認した。`cargo test -p pulsen-domain` は 271 件緑。

主要な設計判断はいずれも成立している。

- **2段規則**: `classify_alive` の返り値を `AliveDecision`(3値)に絞ることで「生存の観測からは `Judge` が導かれない」ことが型で述べられ、1段目(exit の有無)は `observe.rs` にあって `RunningDecision::Judge` を作る唯一の場所になっている。`From<AliveDecision> for RunningDecision` の写像は3値を取り違えずに対応し、`RunningDecision` は4値のまま残って `DOM-execution-008` を満たす。「exit が Some なら `starttime_of` を呼ばない」は `ScriptedProcessController::calls()` が空であることで実効的に守られている。
- **デフォルト判定は見送りを生まない**: `default_judgement` の返り値が2値の `DefaultJudgement` で、`Skipped` を導く経路は `interpret_judge_completion` の exit 20 だけ。`JudgeOutcome` は3値のまま残り `DOM-execution-004`、`DefaultJudgement` の2値が `DOM-execution-019` を満たす。`From` の写像も正しい。
- **遷移規則**: `complete_run` / `skip_run`(`reset_run_failures` — `spawn_fail_count` は触らない)、`fail_run`(`attempt_count += 1` / `judge_attempt_count = 0`)、`record_judge_failure`(`judge_attempt_count += 1` のみ・`Running` 維持・`last_failure = JudgeFail`)、`advance`(`next` 参照・`Pending`・カウンタ不変)、`mark_notified` はいずれも spec の事後条件と一致する。上限判定は `count > limit` で等号では凍結せず、`retries: 0` の即凍結・上限と +1 の境界がユニットテストと ues テストの両方で主張されている。
- **不変条件の検査がドメイン側にある**: `ensure_running` が不変条件2・3 を、`ensure_completed` が不変条件2 を遷移の前提として検査し、破れは `MissingCurrentAttempt` の値として返る(パニックしない)。`current_attempt` を失った / 同定情報を失った `Running`・`Completed` からの4遷移が個別に主張されている。
- **通知の順序と冪等性**: `commit` の `Freeze::Frozen` が「stopped を書く → notify → 成功時のみ `mark_notified`」を1箇所に固定し、catch-up 側は `Freeze::NotFrozen` を通すので過去の凍結が再計上されない。`mark_notified` の前提(`Stopped { notified_at: None }`)が `Task` / `DegradedTask` で `mark_notified_state` を共有し、`DOM-task-059` の「Task と同じ規則」が6状態 + 通知済みの全分岐で主張されている。
- **コメント**: ドメイン・ユースケースの新規コメントはすべて why / why not で、指摘への弁明や修正の経緯は残っていない。`unreachable!` は3箇所ともテストヘルパー内(既知の変種の取り出し)で、本番の規則の担保には使われていない。

以下2件は Warning。Blocker は無い。

#### Blockers

なし。

#### Warnings

- **[W-001]** `TransitionError::MissingCurrentAttempt` の意味を広げたのに、表示文言が起動記録済み(launching)限定のままになっている
  - 場所: `crates/pulsen/src/cli/render.rs:310-312`(`transition_error`)、`crates/pulsen-domain/src/task/transition.rs:31-33`、`crates/pulsen/src/application/tick/mod.rs:456-463`(`advance`)
  - 理由: 本 PR は同変種の doc を「遷移の前提となる現在 attempt、またはその同定情報が失われている」に広げ、`ensure_running`(Running・不変条件2/3)と `ensure_completed`(Completed・不変条件2)からも返るようにした。ところが `render` は `"起動記録済みなのに現在 attempt が無い"` のまま。`Branch::Advance` は事前検査を持たず `task.advance()` を直に呼ぶため、手動修復で `current_attempt` を失った Completed タスクに対して「`遷移の前提が成立しません(起動記録済みなのに現在 attempt が無い)`」という、実行状態と食い違う行が実際に出る(ドメイン側には `現在attemptを失った判定確定タスクはステータスを進められない` という到達性の証拠がある一方、この経路の報告を見るテストは無く、`render.rs` のテストは古い文言をピン留めしている)。ラウンド1 で fix 済みの「judge exit 10 で『実行が終了コード 0 で終了しました』」と同じ、修復の入口を誤らせる自己矛盾の報告である。Running 側は `observe` が `MissingCurrentAttempt` / `MissingProcessIdent` の `TickIssue` を先に積むため露出しない。
  - 提案: 文言を状態に依らない形(「遷移の前提となる現在 attempt(または同定情報)が無い」等)に直し、`render.rs` の該当テストの期待も合わせる。あわせて `advance` で前提が破れたケースのユースケーステストを1本足すと、この経路の報告が初めて実効的に守られる(`tick_notify.rs:477` は `NotAgentRunStatus` しか見ていない)。

- **[W-002]** `AliveDecision` / `DefaultJudgement` による返り値型の乖離が、AC-8 が参照する「spec との差分として提起するもの」の一覧に無い
  - 場所: `.thread/3/plan.md:95-101`、`.thread/3/steps.md:259`(ステップ15)、対応する spec は `spec/domains/execution.md:109`(`classify_alive(...) -> RunningDecision`)と `:119`(`default_judgement(exit) -> JudgeOutcome`)
  - 理由: ラウンド1・2 の指摘対応(ADR-009 / ADR-010 相当)で、spec が定めた2つのシグネチャが実装では別の型を返すようになり、spec に存在しない型が2つ(`AliveDecision` / `DefaultJudgement`)増えた。さらに `DOM-execution-017` の PASS 要件は `classify_alive` に「exit Some なら観測なしで即 `Judge`」まで含めているが、実装ではその1段目がユースケース(`observe.rs`)にある。plan.md / steps.md の差分一覧は依然として3件(`TransitionError` の変種・DTO 11フィールド・`InconsistentRunFiles`)のままで、この2件が落ちている。AC-8 は「spec との食い違いも同じコメントで提起する」を完了条件にしており、記帳の正本が実装の最終形に追いついていない(ラウンド2 の B-004 と同じ性質の欠落)。設計判断そのものは妥当なので、直すのは記帳側だけでよい。
  - 提案: plan.md「spec との差分として提起するもの」と steps.md ステップ15 に、(a) `classify_alive` の返り値が `AliveDecision`(spec は `RunningDecision`)で1段目はユースケース側にあること、(b) `default_judgement` の返り値が `DefaultJudgement`(spec は `JudgeOutcome`)であること、を追記する。台帳側は `DOM-execution-008` / `004` の値数要件を満たしたままなので、提起は spec 追従の依頼として書けば足りる。

#### カバレッジ

- 確認: `crates/pulsen-domain/src/execution/judgement.rs`, `crates/pulsen-domain/src/execution/running.rs`, `crates/pulsen-domain/src/execution/notification.rs`, `crates/pulsen-domain/src/execution/value.rs`, `crates/pulsen-domain/src/execution/mod.rs`, `crates/pulsen-domain/src/execution/port.rs`, `crates/pulsen-domain/src/task/task.rs`, `crates/pulsen-domain/src/task/degraded.rs`, `crates/pulsen-domain/src/task/counters.rs`, `crates/pulsen-domain/src/task/transition.rs`, `crates/pulsen/src/application/tick/mod.rs`, `crates/pulsen/src/application/tick/observe.rs`, `crates/pulsen/src/application/tick/notify.rs`, `crates/pulsen/src/application/tick/launch.rs`, `crates/pulsen/src/application/tick/confirm_spawn.rs`, `crates/pulsen/src/cli/render.rs`, `crates/pulsen/src/cli/tick.rs`, `crates/pulsen/src/cli/wire.rs`, `crates/pulsen/src/adapter/command_runner.rs`, `crates/pulsen/src/adapter/mod.rs`, `crates/pulsen-conformance/src/doubles/command_runner.rs`, `crates/pulsen-conformance/src/doubles/task_repository.rs`, `crates/pulsen/tests/tick_observe.rs`, `crates/pulsen/tests/tick_notify.rs`, `crates/pulsen/tests/tick_scan.rs`, `.thread/3/plan.md`, `.thread/3/steps.md`, `.thread/3/adr.md`, `.thread/3/review/triage.md`
- スキップ: `crates/pulsen/src/adapter/process.rs` — OS 依存の終了操作・同定の実装で、ドメインの型・遷移規則を持たない(Adapter 観点)
- スキップ: `crates/pulsen-conformance/src/command_runner.rs`, `crates/pulsen-conformance/src/process_controller.rs`, `crates/pulsen-conformance/src/lib.rs`, `crates/pulsen-conformance/src/doubles/mod.rs`, `crates/pulsen-conformance/src/doubles/process.rs`, `crates/pulsen-conformance/src/doubles/run_store.rs`, `crates/pulsen-conformance/HOOKS.md` — ポート適合スイートとハーネスで、ドメイン判断を含まない(Adapter 観点)。ただし `RecordSeq` の共有(順序の主張)と `read_exit` の台本追加は上記2ファイル経由で確認した
- スキップ: `crates/pulsen/tests/conformance_command_runner.rs`, `crates/pulsen/tests/conformance_process_controller.rs`, `crates/pulsen/examples/judge_probe.rs`, `crates/pulsen/examples/agent_probe.rs` — 実プロセス・実環境に対する適合ケースとプローブ(Adapter 観点)
- スキップ: `crates/pulsen/tests/cli_tick.rs`, `crates/pulsen/tests/common/mod.rs`, `crates/pulsen/tests/tick_fixture/mod.rs` — 実バイナリの受け入れテストとフィクスチャで、ドメイン規則の主張はユースケース層テスト側で確認済み(General / Usecase 観点)
- スキップ: `.thread/3/testing.md` — 手動確認の記録(General 観点)
- スキップ: `.thread/3/review/plan-001.md`, `.thread/3/review/plan-002.md`, `.thread/3/review/review-001.md`, `.thread/3/review/review-001-adapter.md`, `.thread/3/review/review-001-domain.md`, `.thread/3/review/review-001-general.md`, `.thread/3/review/review-001-usecase.md`, `.thread/3/review/review-002.md`, `.thread/3/review/review-002-adapter.md`, `.thread/3/review/review-002-domain.md`, `.thread/3/review/review-002-general.md`, `.thread/3/review/review-002-usecase.md` — レビューの中間成果物(Phase 8 で削除)。ゼロベース方針のため参照しない
