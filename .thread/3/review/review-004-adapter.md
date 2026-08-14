### Adapter / Ports

#### Blockers

なし

#### Warnings

- **[W-001]** 「スキップ許容条件の正本」が列挙するフィクスチャの種別から、本 PR が足した `examples/judge_probe` だけが落ちている
  - 場所: `crates/pulsen-conformance/HOOKS.md:59`
  - 理由: この段落は本 PR で書き換えられ、括弧内で「保持プロセス・テスト用エージェント・デタッチ性のフィクスチャ」とフィクスチャの種別を列挙する形になった。ところが同じ PR が `:50` に足した行は「テスト用コマンド(`examples/judge_probe`)」という別の種別で、この列挙に入っていない。実装側(`crates/pulsen/tests/conformance_command_runner.rs:173` の `allowed_skips`)は `judge_probe` の不在を許容集合に入れておらず、`:50` の行自身も「**スキップ許容集合には入れない**」と書いているので、**実装と行は正しく、この段落の列挙だけが実際より狭い**。HOOKS.md はフックとスキップ許容条件の正本であり、ラウンド2 の `HOOKS.md:49 / 正本性`・ラウンド3 の `HOOKS.md / 正本性` と同じ「正本の記述が実際の集合とずれる」種類の残り。
  - 提案: 括弧を「保持プロセス・テスト用エージェント・テスト用コマンド・デタッチ性のフィクスチャ」にする。1語の追加で閉じる。

#### 確認した内容(所見)

指摘に至らなかったが、依頼された論点について確認した結果を残す。

- **`terminate` の最終形**: `ESCALATES` による分岐は ADR-015 の Decision(`crates/pulsen/src/adapter/process.rs:190-232`)と過不足なく一致する。POSIX は `-TERM` → 猶予 → `-KILL` → 猶予、Windows は `taskkill /T /F` 1段のみで2段目を起動しない。`UnitTarget::parse` は POSIX で `-1` / `-0` / `0` / 非数値を、Windows で `0` を弾き、`UNTARGETABLE_IDENTS` の6値すべてについて「終了操作を1度も起動しない」ことを**痕跡を残す実体の注入**(`tracing_terminator`)で外から観測している(`process.rs:1535` / `:1555`)。`Err(Io)` は `gone_within_grace` で即 false になり待たない。既に消滅した実行単位への `kill` が `Ok` になる帰結は ADR-015 のトレードオフとして明示され、`終了ステータスが非0でも実行単位が消えていれば成功になる`(`:1578`)がその形を固定している。昇格の実効性は `猶予のうちに消えない実行単位は捕捉できない終了へ昇格させる`(`:1590`)が `trap '' TERM` の実行単位で実測している。排他ロックの保持時間は昇格あり4秒 / なし2秒で、`TERMINATION_GRACE` の doc コメントと ADR-015 の両方が判定 timeout(60秒)との比で根拠を書いている。
- **OS 依存の隔離(AC-7)**: `crates/pulsen-domain/src/` のターゲット述語つき `cfg` は 0 件、`crates/pulsen/src/` のヒットは `adapter/process.rs`(20)・`util/atomic.rs`(4)・`adapter/task_repository.rs`(1)の3ファイルのまま。`#[allow(unsafe_code)]` は `adapter/process.rs:454` の1箇所のみ。本番依存は6クレート(clap / getrandom / serde / serde_json / serde_yaml_ng / tempfile)から増えていない。`CommandRunner` のアダプターは `cfg` を1つも持たず、符号化を `process::encode` で `run_agent` と共有している。`cargo build` / `cargo test --workspace --locked` / `cargo clippy --workspace --all-targets -- -D warnings` / `cargo fmt --check` をこの環境(macOS)で実行し、すべて通ることを確認した(適合ケースの `SKIP` は `TC-port-clock-005` の1件のみで、これは実時計アダプターの恒久スキップ)。
- **`CommandRunner` の timeout**: 期限は `started.elapsed() >= limit` で全域。超過・`wait` 失敗のどちらでも `kill` → `wait` で回収しており、ゾンビは残らない。シェル非経由・`env_clear` なしの上書き・cwd 不変はすべて適合ケース(TC-006〜011)が exit code で固定している。TC-012 は「証跡が timeout 後も現れない」ことで終了を観測しており、定数を返す実装では通らない形になっている。
- **適合スイートの実効性**: CommandRunner 16行 / ProcessController 27行が spec の台帳と1:1で並び、期待も契約の語彙(「実行単位に属する全プロセスが終了する」「非0の符号化値」)に留まっている。フックが `None` を返す経路はすべて `require!` によるスキップで、`allowed_skips` / `observation_allowed_skips` に入らない限りケースの失敗になる。能力判定 `probe_execution_unit` は実行単位を実際に1度起こし、その一部だけを終了させられるかまで実測してから `Partitionable` / `WholeOnly` / `Unavailable` / `ProgramMissing` を決めており、`ProgramMissing` は許容集合を空にする(作り忘れが緑にならない)。許容集合の分割(実行単位を要する4行 / 一部終了を要する2行)は HOOKS.md の行と一致する。
- **`HOOKS.md` の正本性**: 総数(10ポート196行)・区分別件数(A 41 / B 132 / C 23)・各ポートの小計を再計算し、すべて表の行数と一致することを確認した。区分 C の 23行の内訳(権限系13 + 別能力4 + 注入6)も本文の記述と合う。本 PR で足した5行の実測列はすべて `未測定`。ProcessController 側の `terminated_pid` / `spawn_unit` が要する実行ファイルの記述も実装と一致する。ずれは W-001 の1点のみ。
- **テストダブルの忠実性**: `RecordSeq` を `save` / `save_degraded` / `run` / `try_kill_remnants` の4メソッドに載せる設計は、順序の契約が**ポートをまたぐ**箇所とちょうど一致している。`Option<RecordSeq>` で `spawn_wrapper` / `own_identity` / `run_agent` / `kill` を採番から外す判断も、`kill` の失敗時は「保存が1件も無い」ことが先後を含意するため穴にならない(`observe.rs:157-171` が失敗時に `record_run_failure` を呼ばない形)。`tick_notify.rs:100-144` / `tick_observe.rs:110-140` は実際に3系統の記録を1本に並べ替えて主張しており、順序を逆にした実装を落とせる。
- **セキュリティ**: 判定・通知コマンドはシェル非経由の直接起動で、プレースホルダ展開も行わない(TC-006 / 007 が期待をファイル経由で渡してこれを固定している)。誤殺については parse が実行単位を一意に指さない値を境界で弾き、`try_kill_remnants` は列挙できたときにだけ終了を実行する。PID / PGID 再利用に対する弱さは ADR-002 / ADR-015 とコード中の why が実態どおりに述べている。環境変数は契約どおり継承 + 上書きで、`env_clear` を呼ばないことが why として書かれている。
- **経緯コメント**: `crates/` 配下の追加行を「指摘 / レビュー / ラウンド / 以前は / 修正した」等で走査したが、修正の経緯や弁明を残す記述は1件も無かった。3ラウンド分の修正が積まれたコードにも、残っているのは why / why not だけである。

#### カバレッジ

- 確認: `crates/pulsen/src/adapter/process.rs`, `crates/pulsen/src/adapter/command_runner.rs`, `crates/pulsen/src/adapter/mod.rs`, `crates/pulsen/src/cli/wire.rs`, `crates/pulsen/src/cli/tick.rs`, `crates/pulsen/src/application/tick/observe.rs`, `crates/pulsen/src/application/tick/notify.rs`, `crates/pulsen-domain/src/execution/port.rs`, `crates/pulsen-domain/src/execution/value.rs`, `crates/pulsen-domain/src/execution/mod.rs`, `crates/pulsen-conformance/HOOKS.md`, `crates/pulsen-conformance/src/lib.rs`, `crates/pulsen-conformance/src/command_runner.rs`, `crates/pulsen-conformance/src/process_controller.rs`, `crates/pulsen-conformance/src/doubles/mod.rs`, `crates/pulsen-conformance/src/doubles/command_runner.rs`, `crates/pulsen-conformance/src/doubles/process.rs`, `crates/pulsen-conformance/src/doubles/run_store.rs`, `crates/pulsen-conformance/src/doubles/task_repository.rs`, `crates/pulsen/tests/conformance_command_runner.rs`, `crates/pulsen/tests/conformance_process_controller.rs`, `crates/pulsen/tests/tick_notify.rs`, `crates/pulsen/tests/tick_observe.rs`, `crates/pulsen/tests/tick_fixture/mod.rs`, `crates/pulsen/tests/common/mod.rs`, `crates/pulsen/tests/cli_tick.rs`, `crates/pulsen/examples/judge_probe.rs`, `crates/pulsen/examples/agent_probe.rs`, `.thread/3/plan.md`, `.thread/3/adr.md`, `.thread/3/review/triage.md`
- スキップ: `crates/pulsen-domain/src/execution/judgement.rs`, `crates/pulsen-domain/src/execution/notification.rs`, `crates/pulsen-domain/src/execution/running.rs` — ドメインサービスの判断ロジック。ポートへの入出力(`judge_env` / `notify_env` / `NOTIFY_TIMEOUT` の適用)は呼び出し側で確認済み
- スキップ: `crates/pulsen-domain/src/task/counters.rs`, `crates/pulsen-domain/src/task/degraded.rs`, `crates/pulsen-domain/src/task/task.rs`, `crates/pulsen-domain/src/task/transition.rs` — ドメインの遷移とカウンタ。ポート・アダプターに接しない
- スキップ: `crates/pulsen/src/application/tick/mod.rs`, `crates/pulsen/src/application/tick/launch.rs`, `crates/pulsen/src/application/tick/confirm_spawn.rs` — ユースケースの分岐と報告。ポートの型引数(`C: CommandRunner`)の追加のみ確認した
- スキップ: `crates/pulsen/src/cli/render.rs` — 報告の文言組み立て。表示層でアダプターに接しない
- スキップ: `crates/pulsen/tests/tick_scan.rs` — 走査アームの主張の差し替え。ダブルの使い方はほかのテストで確認済み
- スキップ: `.thread/3/steps.md`, `.thread/3/testing.md` — 手順書と手動確認の記録。General 観点の担当
- スキップ: `.thread/3/review/plan-001.md`, `plan-002.md`, `plan-003.md`, `review-001.md`, `review-001-adapter.md`, `review-001-domain.md`, `review-001-general.md`, `review-001-usecase.md`, `review-002.md`, `review-002-adapter.md`, `review-002-domain.md`, `review-002-general.md`, `review-002-usecase.md`, `review-003.md`, `review-003-adapter.md`, `review-003-domain.md`, `review-003-general.md`, `review-003-usecase.md` — レビューの中間成果物(Phase 8 で削除)。ゼロベースのレビューのため過去ラウンドの結論は読んでいない
