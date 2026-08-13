# レビュー009 — Test

観点: TC-* 行の ID 単位の突き合わせ / アサーションの形骸化 / 適合フックの契約 / スキップ宣言と実態の一致 / flaky と後始末 / 決定性 / 役割分担

環境: macOS (Darwin 25.4.0)、非 root、TMPDIR は git リポジトリ外。`cargo test --workspace` は全緑(exit 0)、`SKIP` 行の出力は 0 件 — この環境ではスキップ許容集合が空になり、宣言と実態が一致している。

## Test

### Blockers

なし

### Warnings

なし

## 所見(指摘ではない)

- **TC-* 行の突き合わせ**: Issue #2 のチェックリストにある TC 行(tick 44行・run-wrapper 27行・run-store 21行・process-controller 16行・worktree-manager 7行)は、いずれも対応するテストが実在し実際に走る。ID を名前に持つのはポート適合スイートと受け入れの一部(`tc_exec_run_wrapper_014` / `016` のスキップ宣言)だけで、tick / run-wrapper の行は仕様の言葉で名付けたテストが担う。以下は「1つのテストに複数行が集約される」ため確認した点で、いずれも行の主張は落ちていない。
  - `TC-exec-tick-053` / `054`(launching 記録後・prepare 後のクラッシュ)は、read 系にはどちらも `Ok(None)` として現れるため `起動記録の後にクラッシュした状態は猶予時間の判断に合流する` が両方を消化する(テスト内のコメントで明示)。
  - `TC-exec-tick-082` / `083`(tick 先行 / ラッパー先行)は、観測できる帰結が「再読で pid を検出して running へ取り込む」で同一。順序そのものは `猶予超過ではマーカーを書いてからpidを再確認する` が `RunStoreCall` の並びで別に主張している。
  - `TC-exec-run-wrapper-020`(`{workspace}` を参照しない agent_cmd でも cwd は worktree)は、ユースケース側(`RunAgent { cwd: workspace() }` を `{workspace}` 非参照のコマンドで主張)と適合ケース `TC-port-process-controller-019`(`CheckCwd`)の組で成立している。
  - `TC-exec-tick-055`(config 修正が次 tick で反映)は plan では手動確認扱いだが、`cli_tick.rs::エージェント定義を壊すと起動できず直せば次のtickで起動する` が実バイナリで自動化している。
- **形骸化の検査**: 「何も起きない」系の主張が空虚にならない造りになっている。`repository()` の `save` は常に成功する台本なので `tasks.saved().is_empty()` は「save が呼ばれなかった」と同値、`saved()` が成否によらず値を積む点も doc で明示されている。`ScriptedRunStore` / `ScriptedProcessController` の `calls()` は書き込み順序(starttime → pid → マーカー確認)とマーカー順序プロトコルを値ではなく並びで主張しており、状態を戻す実装が単体で緑にならない。適合フック側も `attempt_dir_present` / `run_dir_is_empty` が**前後で反転すること**まで主張しているため、定数を返すハーネスはどちらかの側で落ちる。
- **適合フックの契約**: `HOOKS.md` の総数(9ポート169行 = A 38 / B 113 / C 18)はポート別の内訳と一致し、`RunStoreHarness` / `ProcessControllerHarness` / `WorktreeManagerHarness` のフック一覧も trait の実体と一致する。台帳行に対応しない追加ケース2件(RunStore の write 系ディレクトリ作成、WorktreeManager の `prunable` 復旧)は件数に数えず「追加ケース」として区別されており、ADR-077 の復旧2分岐が両方走ることが担保されている。
- **スキップ宣言と実態**: 権限依存(run-store 007/017、process-controller 023/025、run-wrapper 014/016)は `permission_restrictions_effective()`、シグナル死(process-controller 024)は `cfg!(unix)`、シンボリックリンク経由の置き場(worktree-manager 010/012〜016 と追加ケース)は実際にリンクを張れるかの実行時判定で決まる。`agent_probe` / `spawn_probe` / `lock_holder` の不在は**許容集合に入れない**方針が守られており(`conformance_process_controller.rs` の `spawn` スイートは `Vec::new()`、`cli_wrapper.rs::probe_program` は `expect`)、examples の作り忘れが緑にならない。
- **flaky と後始末**: 非同期の待ち合わせはすべて `wait_until` / `wait_for_run_files` の期限つきポーリングで、待ち条件が**これから観測する成果物そのもの**(exit を読むなら exit、ログを読むならログ)に立っている。滞留を作る `agent_probe wait-for` は解放をテスト側のファイルが決める形で、生存の窓が実行環境の速さに依存しない。滞留させたケースはいずれも一時ディレクトリの削除前に解放と `RELEASED` の観測まで済ませており、孫プロセスと TempDir 削除の競合が残らない。`TC-port-run-store-016` の読み手停止は `Drop`(`StopOnDrop`)に載っており、書き手のパニックがハングに化けない。
- **決定性**: 猶予境界(30 / 31 / 巻き戻り)は `SettableClock` に対するユースケーステストとドメインのユニットテストの両方で消化され、実時間を待たない。作業ディレクトリというプロセス全体の状態を壊す前提は `cli_tick_missing_cwd.rs` として独立した実行ファイルに隔離されている。
- **役割分担**: 実アダプターで外から作れない状況(ロック機構の異常・`list_active` の Io・`RunFileError` 3分類・マーカー書き込み失敗・spawn の同期エラー・`save` 失敗)はすべてダブルに対するユースケーステスト、契約への適合は適合スイート、合成ルートとクロス tick の引き継ぎは実バイナリの受け入れ、という切り分けが守られている。`128+シグナル番号` の具体値は適合スイートではなく POSIX のアダプターユニットテストに置かれ、適合ケースは「非0の符号化値」までにとどめている(ADR-074)。

## カバレッジ

確認(47): `.adr/027-port-conformance-suite-and-harness-hooks.md`, `.thread/2/plan.md`, `.thread/2/progress.md`, `crates/pulsen-conformance/HOOKS.md`, `crates/pulsen-conformance/src/lib.rs`, `crates/pulsen-conformance/src/run_store.rs`, `crates/pulsen-conformance/src/process_controller.rs`, `crates/pulsen-conformance/src/worktree_manager.rs`, `crates/pulsen-conformance/src/doubles/mod.rs`, `crates/pulsen-conformance/src/doubles/clock.rs`, `crates/pulsen-conformance/src/doubles/process.rs`, `crates/pulsen-conformance/src/doubles/run_store.rs`, `crates/pulsen-conformance/src/doubles/task_repository.rs`, `crates/pulsen-conformance/src/doubles/worktree.rs`, `crates/pulsen-conformance/src/doubles/tests.rs`, `crates/pulsen-domain/src/definition/agent.rs`, `crates/pulsen-domain/src/definition/template.rs`, `crates/pulsen-domain/src/execution/launching.rs`, `crates/pulsen-domain/src/execution/value.rs`, `crates/pulsen-domain/src/task/attempt.rs`, `crates/pulsen-domain/src/task/counters.rs`, `crates/pulsen-domain/src/task/failure.rs`, `crates/pulsen-domain/src/task/path.rs`, `crates/pulsen-domain/src/task/planner.rs`, `crates/pulsen-domain/src/task/task.rs`, `crates/pulsen-domain/src/task/transition.rs`, `crates/pulsen/examples/agent_probe.rs`, `crates/pulsen/examples/spawn_probe.rs`, `crates/pulsen/src/adapter/process.rs`, `crates/pulsen/src/adapter/run_store.rs`, `crates/pulsen/src/adapter/worktree.rs`, `crates/pulsen/src/cli/render.rs`, `crates/pulsen/tests/cli_tick.rs`, `crates/pulsen/tests/cli_tick_missing_cwd.rs`, `crates/pulsen/tests/cli_usage.rs`, `crates/pulsen/tests/cli_wrapper.rs`, `crates/pulsen/tests/common/mod.rs`, `crates/pulsen/tests/common/git.rs`, `crates/pulsen/tests/conformance_process_controller.rs`, `crates/pulsen/tests/conformance_run_store.rs`, `crates/pulsen/tests/conformance_worktree.rs`, `crates/pulsen/tests/register_task.rs`, `crates/pulsen/tests/run_wrapper.rs`, `crates/pulsen/tests/tick_confirm_spawn.rs`, `crates/pulsen/tests/tick_fixture/mod.rs`, `crates/pulsen/tests/tick_launch.rs`, `crates/pulsen/tests/tick_scan.rs`

スキップ(66):

- `.thread/2/review/review-001-adapter.md`, `.thread/2/review/review-001-architecture.md`, `.thread/2/review/review-001-domain.md`, `.thread/2/review/review-001-test.md`, `.thread/2/review/review-001-usecase.md`, `.thread/2/review/review-001.md`, `.thread/2/review/review-002-adapter.md`, `.thread/2/review/review-002-architecture.md`, `.thread/2/review/review-002-domain.md`, `.thread/2/review/review-002-test.md`, `.thread/2/review/review-002-usecase.md`, `.thread/2/review/review-003-adapter.md`, `.thread/2/review/review-003-architecture.md`, `.thread/2/review/review-003-domain.md`, `.thread/2/review/review-003-test.md`, `.thread/2/review/review-003-usecase.md`, `.thread/2/review/review-003.md`, `.thread/2/review/review-004-adapter.md`, `.thread/2/review/review-004-architecture.md`, `.thread/2/review/review-004-domain.md`, `.thread/2/review/review-004-test.md`, `.thread/2/review/review-004-usecase.md`, `.thread/2/review/review-004.md`, `.thread/2/review/review-005-adapter.md`, `.thread/2/review/review-005-architecture.md`, `.thread/2/review/review-005-domain.md`, `.thread/2/review/review-005-test.md`, `.thread/2/review/review-005-usecase.md`, `.thread/2/review/review-005.md`, `.thread/2/review/review-006-adapter.md`, `.thread/2/review/review-006-architecture.md`, `.thread/2/review/review-006-domain.md`, `.thread/2/review/review-006-test.md`, `.thread/2/review/review-006-usecase.md`, `.thread/2/review/review-006.md`, `.thread/2/review/review-007-adapter.md`, `.thread/2/review/review-007-architecture.md`, `.thread/2/review/review-007-domain.md`, `.thread/2/review/review-007-test.md`, `.thread/2/review/review-007-usecase.md`, `.thread/2/review/review-007.md`, `.thread/2/review/review-008-adapter.md`, `.thread/2/review/review-008-architecture.md`, `.thread/2/review/review-008-domain.md`, `.thread/2/review/review-008-test.md`, `.thread/2/review/review-008-usecase.md`, `.thread/2/review/review-008.md`, `.thread/2/review/triage.md` — 過去のレビュー結果。ゼロベースで見るため読まない(本ラウンドの指示)
- `.thread/2/adr.md`, `.thread/2/steps.md`, `.thread/2/testing.md` — 契約は plan.md、テストの実態はテストコードで見る
- `crates/pulsen-domain/src/execution/mod.rs`, `crates/pulsen-domain/src/task/mod.rs`, `crates/pulsen/src/adapter/mod.rs`, `crates/pulsen/src/application/mod.rs` — 再公開のみでテストを持たない
- `crates/pulsen-domain/src/execution/port.rs` — ポート宣言。契約の検証は適合スイート側で見た
- `crates/pulsen/src/application/run_wrapper.rs`, `crates/pulsen/src/application/tick/mod.rs`, `crates/pulsen/src/application/tick/launch.rs`, `crates/pulsen/src/application/tick/confirm_spawn.rs` — ユニットテストを持たない実装。振る舞いは `tests/run_wrapper.rs` / `tests/tick_*.rs` 側で確認した
- `crates/pulsen/src/cli/add.rs`, `crates/pulsen/src/cli/args.rs`, `crates/pulsen/src/cli/mod.rs`, `crates/pulsen/src/cli/tick.rs`, `crates/pulsen/src/cli/wire.rs`, `crates/pulsen/src/cli/wrapper.rs` — ユニットテストを持たない実装。振る舞いは `tests/cli_*.rs` 側で確認した
