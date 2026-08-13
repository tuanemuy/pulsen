# レビュー008 — Test 観点

## Test

### Blockers

なし。

Issue #2 のチェックリストにある TC-* 行（TC-exec-tick 42行 / TC-exec-run-wrapper 27行 / TC-port-run-store 21行 / TC-port-process-controller 16行 / TC-port-worktree-manager 7行 = 113行）を ID 単位で突き合わせた結果、対応するテストが1つも見当たらない行は無かった。適合スイートは 1行 = 1ケース関数 = 1 `#[test]` で ID が関数名に入っており、突き合わせが機械的に成立する。`cargo test` はこの環境で全緑（新規スイートのスキップ0件）、`conformance_worktree` / `cli_tick` / `cli_wrapper` / `conformance_process_controller` の12回連続実行でも失敗しなかった。

### Warnings

- **[W-001]** `TC-exec-run-wrapper-026`（エージェントが何も出力せず終了した → **ログファイルは空のまま** exit が書かれる）の、この行だけが持つ主張が主張されていない。
  - 場所: `crates/pulsen/tests/cli_wrapper.rs:107` `起動引数どおりのエージェントが実行され終了結果がrunディレクトリに現れる`
  - 理由: `agent_probe exit 0` は何も出力しないので**状況そのものは通っている**が、アサートするのは `run_files()` の顔ぶれと `exit_code() == 0` だけで、`stdout.log` / `stderr.log` の中身は見ていない。この行の残り（TC-003 と重ならない部分）は「ログが空である＝ラッパー自身がログを汚さない」ことなので、ラッパーが診断行をログへ書き足す実装に変わっても緑のままになる。他のケースは `EchoArgs` / `Print` で「エージェントが書いた内容と一致すること」を見ているだけで、「エージェントが書かなければ空であること」は誰も見ていない。
  - 提案: 同テストに `stdout.log` / `stderr.log` が空文字列であることの主張を足す（`ログを開けなければ…` が先置き内容の不変を見ているのと同じ形）。

- **[W-002]** スキップ宣言と実態・契約が食い違っている（`tc_exec_tick_015`）。
  - 場所: `crates/pulsen/tests/common/mod.rs:39` `LOCK_HOLDER_CASES`、`crates/pulsen/tests/cli_tick.rs:430` `別の操作がロックを保持していればスキップして0で終わる`、`.thread/2/plan.md`「実行環境が前提を作れないとスキップで終わる行」の表
  - 理由: 2点ある。(1) plan.md のスキップ表は7行（`TC-port-run-store-007/017`・`TC-port-process-controller-023/024/025`・`TC-exec-run-wrapper-014/016`）で、`tc_exec_tick_015` を含まない。ステップ19 は「スキップした理由と確認した環境を Issue のコメントに残す」運用なので、契約側の表と実装側の宣言が食い違っていると記帳の入力が定まらない。(2) その宣言は実際には効かない — テストは `probe_home()` が `agent_probe().expect("cargo test は examples をビルドする")` で落ちる方が先で、`lock::holder_program()` に到達する前にパニックする。`agent_probe` と `lock_holder` は同じパッケージの example なので、「片方だけ無い」実行は起こらない。宣言は死んだままで、スキップ許容集合が「この環境で何が走らなかったか」を表すという ADR-055 の意味を薄める。
  - 提案: `LOCK_HOLDER_CASES` から `tc_exec_tick_015` を落とす（この受け入れテストは examples 前提で確定的に走る）か、plan.md のスキップ表に足して両者を揃える。どちらでもよいが、片方だけ直すと食い違いが残る。

### 確認したこと（Blocker が無いと判断した根拠）

- **ID 単位の突き合わせ**: `spec/testcases/execution/tick.md` / `run-wrapper.md` / `spec/testcases/ports/*.md` の行番号から TC 番号を復元し、チェックリストの全 TC-* 行に対応するテストを特定した。境界（`TC-exec-tick-077/078/079` の 30秒 / 31秒 / 巻き戻り、`049/050` の等号 / 上限+1、`048` の `retries: 0`）はドメインのユニットテストとユースケーステストの両方で消化されている。順序の主張（`TC-exec-run-wrapper-021/022`、`TC-exec-tick-082/083`）はダブルの `calls()` の並びで書かれており、値では表せない契約が形骸化していない。
- **役割分担**: 実アダプターで外から作れない状況（ロック機構の異常・`list_active` の Io・`RunFileError` 3種・マーカー書き込み失敗・猶予境界・`save` 失敗）はすべてダブルに対するユースケーステストに、符号化値・デタッチ性・アトミック性は適合スイートに、合成ルートの結線とクロス tick の引き継ぎは実バイナリの受け入れテストに分かれている。重複はあるが、どれも別のことを主張している。
- **アサーションの形骸化**: 適合フックには「観測が前後で反転すること」の規律が要る箇所（`attempt_dir_present` の TC-001、`run_dir_is_empty` の TC-001）に実際に反転の主張が入っており、定数を返すハーネスがどちらかの側で落ちる。`worktree_present` は登録が `ws.branch` を指すことまで見ており、「ブランチを作ってディレクトリを掘っただけ」を通さない。`worktree_marker` は不在を `None`（スキップ）ではなく空文字列で返すので、成果物が消えたことがスキップに化けない。
- **flaky リスクと後始末**: 受け入れテストの待ち合わせはすべて「これから観測する成果物そのもの」に条件を立てている（`exit` を読むなら `exit` の出現、滞留を観測するならログの `waiting` 行）。ラッパーを起動する `cli_tick` / `cli_wrapper` のテストは例外なく `exit` かエージェントの解放合図まで待ってから一時ディレクトリを落としており、孫プロセスの書き込みと削除が競合する経路が残っていない。権限を落とすフックはすべて `Restore` をケースの生存期間だけ保持する形で、一時ディレクトリの削除を妨げない。
- **HOOKS.md の整合**: 冒頭の総数（9ポート169行）・区分別件数（A 38 / B 113 / C 18）・各節の見出しの件数が、実際の対応表の行と一致している。追加ケース2件（`write_準備を経ない書き込みも置き場ごと作って残る` / `create_prunable_…`）は台帳行に数えず区別して載っている。フック一覧は `RunStoreHarness` / `ProcessControllerHarness` / `WorktreeManagerHarness` の実際のメソッドと一致する。
- **スキップの実態**: `cargo test -- --nocapture` の `SKIP` 行は Clock の既存4件のみ。本スライスで足した RunStore 22件・ProcessController 16件・WorktreeManager 8件はすべてこの環境で走っている。`agent_probe` / `spawn_probe` の不在をスキップ許容集合に入れない判断が、`conformance_process_controller.rs` と `cli_wrapper.rs` の両方で守られている。

### カバレッジ

- 確認（50）: `.adr/027-port-conformance-suite-and-harness-hooks.md`, `.thread/2/plan.md`, `.thread/2/progress.md`, `.thread/2/steps.md`, `.thread/2/testing.md`, `crates/pulsen-conformance/HOOKS.md`, `crates/pulsen-conformance/src/lib.rs`, `crates/pulsen-conformance/src/process_controller.rs`, `crates/pulsen-conformance/src/run_store.rs`, `crates/pulsen-conformance/src/worktree_manager.rs`, `crates/pulsen-conformance/src/doubles/clock.rs`, `crates/pulsen-conformance/src/doubles/mod.rs`, `crates/pulsen-conformance/src/doubles/process.rs`, `crates/pulsen-conformance/src/doubles/run_store.rs`, `crates/pulsen-conformance/src/doubles/task_repository.rs`, `crates/pulsen-conformance/src/doubles/tests.rs`, `crates/pulsen-conformance/src/doubles/worktree.rs`, `crates/pulsen-domain/src/definition/agent.rs`, `crates/pulsen-domain/src/definition/template.rs`, `crates/pulsen-domain/src/execution/launching.rs`, `crates/pulsen-domain/src/execution/value.rs`, `crates/pulsen-domain/src/task/attempt.rs`, `crates/pulsen-domain/src/task/counters.rs`, `crates/pulsen-domain/src/task/failure.rs`, `crates/pulsen-domain/src/task/path.rs`, `crates/pulsen-domain/src/task/planner.rs`, `crates/pulsen-domain/src/task/task.rs`, `crates/pulsen-domain/src/task/transition.rs`, `crates/pulsen/examples/agent_probe.rs`, `crates/pulsen/examples/spawn_probe.rs`, `crates/pulsen/src/adapter/process.rs`, `crates/pulsen/src/adapter/run_store.rs`, `crates/pulsen/src/adapter/worktree.rs`, `crates/pulsen/src/cli/mod.rs`, `crates/pulsen/src/cli/render.rs`, `crates/pulsen/tests/cli_tick.rs`, `crates/pulsen/tests/cli_tick_missing_cwd.rs`, `crates/pulsen/tests/cli_usage.rs`, `crates/pulsen/tests/cli_wrapper.rs`, `crates/pulsen/tests/common/git.rs`, `crates/pulsen/tests/common/mod.rs`, `crates/pulsen/tests/conformance_process_controller.rs`, `crates/pulsen/tests/conformance_run_store.rs`, `crates/pulsen/tests/conformance_worktree.rs`, `crates/pulsen/tests/register_task.rs`, `crates/pulsen/tests/run_wrapper.rs`, `crates/pulsen/tests/tick_confirm_spawn.rs`, `crates/pulsen/tests/tick_fixture/mod.rs`, `crates/pulsen/tests/tick_launch.rs`, `crates/pulsen/tests/tick_scan.rs`
- スキップ（57）:
  - `.thread/2/review/` 配下の42ファイル — 本ラウンドはゼロベースのため過去のレビュー成果物を読まない指示による
  - `.thread/2/adr.md` — 設計判断の記録。ADR 番号の参照先はテスト・HOOKS.md 側の引用で追えたため本文は読んでいない
  - `crates/pulsen/src/application/mod.rs`, `crates/pulsen/src/application/run_wrapper.rs`, `crates/pulsen/src/application/tick/mod.rs`, `crates/pulsen/src/application/tick/launch.rs`, `crates/pulsen/src/application/tick/confirm_spawn.rs` — テストを持たない実装。Test 観点では対応するテスト（`tests/tick_*.rs` / `tests/run_wrapper.rs`）側から振る舞いを確認した
  - `crates/pulsen/src/cli/add.rs`, `args.rs`, `tick.rs`, `wire.rs`, `wrapper.rs` — 同上（受け入れテストから観測）。終了コードの分岐だけは `cli/mod.rs` で確認した
  - `crates/pulsen/src/adapter/mod.rs`, `crates/pulsen-domain/src/execution/mod.rs`, `crates/pulsen-domain/src/execution/port.rs`, `crates/pulsen-domain/src/task/mod.rs` — 宣言・再エクスポートのみでテストを持たない
