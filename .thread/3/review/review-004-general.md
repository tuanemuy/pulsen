### General Review（計画ドキュメント）

#### Blockers

なし。

#### Warnings

- **[W-001]** ステップ5 の `RecordSeq` の適用範囲が2メソッドのままで、ADR-014（同 PR）と実装の4メソッドに追いついていない
  - 場所: `.thread/3/steps.md:156`（および `.thread/3/steps.md:38,41,42` のモジュール増分）
  - 理由: ステップ5 は「`ScriptedTaskRepository` の `save` と `ScriptedCommandRunner` の `run` に…採番を付け、`saved_in_order()` / `calls_in_order()` を足す」と書く。一方 `.thread/3/adr.md:413` の ADR-014 は採番を掛ける対象を4つ（`TaskRepository::save` / `TaskRepository::save_degraded` / `CommandRunner::run` / `ProcessController::try_kill_remnants`）と定め、実装もそのとおりになっている（`crates/pulsen-conformance/src/doubles/task_repository.rs:95,104` の `saved_in_order` / `saved_degraded_in_order`、`crates/pulsen-conformance/src/doubles/process.rs:135` の `calls_in_order`、`crates/pulsen-conformance/src/doubles/command_runner.rs:57`）。テスト側も `crates/pulsen/tests/tick_notify.rs:115` と `crates/pulsen/tests/tick_observe.rs:110` が両アクセサを実際に使っている。モジュール増分の並びも `doubles/process.rs` を「`starttime_of` / `kill` / `try_kill_remnants` のスクリプト」としか書かず、採番アクセサが載ったことが読めない。steps.md は #5 / #6 がダブルへ順序の契約を足すときに読む唯一の手順書なので、ここが2メソッドのままだと「またぐ順序を持つポートには採番を足す」という ADR-014 の基準が半分しか伝わらない。
  - 提案: ステップ5 の該当文を4メソッド（`save` / `save_degraded` / `run` / `try_kill_remnants`）と4アクセサ（`saved_in_order` / `saved_degraded_in_order` / `CommandRunner::calls_in_order` / `ProcessController::calls_in_order`）に揃え、ADR-014 の「順序の契約がポートをまたぐメソッドだけに掛ける」基準を1行で引く。モジュール増分（:39, :41, :42）にも `saved_degraded_in_order` と `ProcessController` 側の `calls_in_order` を書き足す。

- **[W-002]** ステップ6 の「同表に足すのは…だけ」が、実際に `HOOKS.md` へ足した行数と食い違う
  - 場所: `.thread/3/steps.md:164`
  - 理由: 「`HOOKS.md` の…「環境で走らなくなりうる行」の表を更新する。同表に足すのは実行単位を要する行(011 / 012 / 013 / 015 と 014 / 016)だけで」と書くが、実際の `crates/pulsen-conformance/HOOKS.md:46-48` は3行を足している — 実行ファイル不在の行（`TC-port-process-controller-007 / 011〜016`）、実行単位を要する行（011 / 012 / 013 / 015）、その一部だけの終了を要する行（014 / 016）。1行目は「スキップ許容集合には入れない」区分で、同じ steps.md のステップ7（`:172`）は CommandRunner 側で同種の行を「足すのは2行」と数え入れている。同一文書内で数え方が割れており、`005` / `010` を載せないという正しい判断も同じ文にあるため、読み手がどちらの規則で表を保守すべきか決まらない。
  - 提案: ステップ7 の数え方に揃えて「足すのは3行 — 実行ファイル不在の 007 / 011〜016（スキップ許容集合には入れない）、実行単位を要する 011 / 012 / 013 / 015、その一部だけの終了を要する 014 / 016。注入で確定的に走る 005 / 010 は載せない」と書き直す。

- **[W-003]** 「実運用ホームの非汚染」のフィクスチャB の範囲が、実際に作る3ディレクトリのうち1つしか挙げていない
  - 場所: `.thread/3/testing.md:901`
  - 理由: 「B は `$HOME/pulsen-manual-test` に閉じている」とあるが、フィクスチャB（`.thread/3/testing.md:295-299`）は `SETUP_HOME="$HOME/pulsen-manual-test"` に加えて `SETUP_REPO="$HOME/pulsen-test-repo"` と `SETUP_WORK="$HOME/pulsen-manual-work"` を作り、冒頭で3つとも `rm -rf` する。後片付け（`:909-910`）は3つとも正しく消しているので、食い違っているのは「閉じている」の記述だけ。手動確認の実行者はこの行を「ホーム直下に増えるのは1ディレクトリ」と読むため、実行前後の差分確認（`ls -a $HOME`）で余計な2つを異常と誤認するか、逆に既存の `$HOME/pulsen-test-repo` が消える破壊的操作に気づかない。
  - 提案: 「B は `$HOME/pulsen-manual-test` / `$HOME/pulsen-test-repo` / `$HOME/pulsen-manual-work` の3つ（手順書の `PULSEN_HOME` / `REPO` / `WORK` と同じパス）に閉じている」と直す。

#### 実測で確かめた点（食い違い無し）

- AC-7 の3つの grep（`.thread/3/testing.md:50-57`）: cfg のヒットは `util/atomic.rs` 4 / `adapter/process.rs` 20 / `adapter/task_repository.rs` 1 で記載どおり。`adapter/command_runner.rs` は0件。`#[allow(unsafe_code)]` は `adapter/process.rs:454` の1件。`-A 8` で `pulsen` の本番依存7行が全て出る。
- 影響確認の grep（`:891`）: `command_runner()` は `cli/wire.rs:251` と `cli/tick.rs:30` の2件で、`cli/add.rs` に無い。`SystemCommandRunner::new() -> Self` で失敗しない。
- サマリー行の並びと報告の4見出し（`:717`, `:893`）: `cli/render.rs:60-69` / `:83-86` / `IssueOutcome::CleanupLeft` と一致。期待文言（「埋め込まれたワークフロー定義を読めません」「runディレクトリのファイルを読めません」等）も `tick_issue` の実装と一致。
- ADR の型名・定数名: `terminate::UnitTarget` / `TERMINATION_GRACE`(2s) / `TERMINATION_POLL`(50ms) / `ESCALATES`(POSIX true・Windows false) / `TerminatorSource` / `AliveDecision` / `DefaultJudgement` / `NotifyOutcome` / `RunFailureCause` の4変種 / `RemnantsLeft` の2変種 / `Delivery::{NotConfigured, Attempted}` / `TransitionError` 6変種 / `TickSummary` 11フィールド、いずれも実装と一致。ADR-001 の「開始時刻からの経過で測る」も `adapter/command_runner.rs:84,91` の `started.elapsed()` と一致。
- スキップ許容集合（`plan.md:63-75`）: `conformance_process_controller.rs` の `EXECUTION_UNIT_CASES`(011/012/013/015) / `PARTIAL_TERMINATION_CASES`(014/016) / 4値の `ExecutionUnitCapability`、`conformance_command_runner.rs` の `PERMISSION_CASES`(004) と一致。判定関数名も `observation_allowed_skips` / `allowed_skips` で正しい。
- `HOOKS.md` の件数（`plan.md:114`, `steps.md:172`）: ProcessController 27行・CommandRunner 16行（A0 / B15 / C1）で一致。新規行の3ランナー列は `未測定`。
- spec の行番号参照: `spec/domains/execution.md:109`(`classify_alive`) / `:119`(`default_judgement`)、`spec/manual-tests/intervention.md:25`(`PMT`) はいずれも実在。
- 手動確認の TC 番号と手順番号: task-execution TC-03(12手順・10がアーカイブ・12が復元) / TC-05(6手順) / TC-07(6〜8が abort・set-status) / TC-19(6が片付け) / TC-20(5が ls・7が set-status) / TC-21(6が abort)、setup TC-10 / 11(5が回復と片付け) / TC-37(4が retry) / TC-38(3が abort) / TC-39(4が復元) / TC-47(3が abort)、intervention TC-01(8手順) / TC-15 / TC-24。plan.md の表と testing.md の記帳（`:915`）が全 TC で一致。
- `.adr/` の既存89件と ADR-001〜017 の重複なし。ADR-013 / ADR-015 / ADR-017 はそれぞれ ADR-073 / ADR-002 / ADR-098 を「置き換えた点だけを書き、本文は書き換えない」形を明示しており、扱いが揃っている。
- 体裁（markdown-style）: 見出し階層・表・コードフェンス・強調の使い方に問題なし。`---` によるエントリ区切りは `.thread/2/adr.md` と同じ規約。

#### カバレッジ

- 確認: `.thread/3/plan.md`, `.thread/3/steps.md`, `.thread/3/testing.md`, `.thread/3/adr.md`
- スキップ: なし（担当外はコードレビューの3観点が確認）
