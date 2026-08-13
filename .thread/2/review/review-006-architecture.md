# レビュー6周目 — Architecture / CLI

## Architecture / CLI

問題なし。Blocker・Warning ともに指摘なし。

以下は判断の根拠として確認した事実。

### 依存方向とレイヤーの責務

- `crates/pulsen-domain/Cargo.toml` の `[dependencies]` は空のまま。`execution/launching.rs` / `task/planner.rs` / `task/transition.rs` の `use` は同クレート内のみで、I/O にも外部クレートにも触れていない。
- `crates/pulsen/src/application/` から `crate::adapter` / `crate::cli` への参照は 0 件（`application/mod.rs` の doc コメントで合成ルートを指す1行のみ）。`Tick` / `RunWrapper` はポートをジェネリック引数で受け取り、受け入れテストとダブルテストの双方に同じ制御フローが乗っている。
- OS 依存分岐はアダプターに隔離されている。`grep -rnE 'cfg\([^)]*\b(unix|windows|target_os|target_family)\b' crates/*/src/` のヒットは `adapter/process.rs` / `adapter/task_repository.rs`（`#[cfg(all(test, unix))]`）/ `util/atomic.rs` / `pulsen-conformance/src/lib.rs`（能力プローブ）だけで、`crates/pulsen-domain/` は 0 件。AC-1 の期待値どおり。
- 起動時刻・PGID の取得は `adapter/process.rs` の `identity::observe` 1関数に閉じ、戻り値は三値（取得できた / 不在 / 機構失敗）。#3 の `starttime_of` が署名を変えずに乗れる形になっている。

### 合成ルート（`cli::wire`）

- `compose()` に残るのはホームの解決とグローバル設定の読み込みだけで、`current_exe()` / `current_dir()` / 乱数はそれぞれ `wire::process_controller()` / `Runtime::workflow_store()` / `wire::id_generator()` に切り出され、`add` と `tick` が自分に要る資源だけを解決する（ADR-068 / ADR-091）。pages「縮退状態の共通規則1」に沿う。
- ラッパーは `WrapperRuntime` / `compose_wrapper(run_dir)` として型ごと分離され、ホームも config も読まない。`SystemProcessController::without_self_exe` の適合契約対象外である旨も doc に書かれている。
- `Runtime::state_root()` / `worktree_root()` は `cli/tick.rs` から呼ばれており、ADR-061 が求める「戻すなら why を添える」も満たしている。

### 未実装メソッド・スコープ外の実装

- `grep -nE 'fn (attempt_exists|list_runs|delete_attempt|remove_task_dir_if_empty|starttime_of|kill|try_kill_remnants|remove)\(' crates/pulsen-domain/src/execution/port.rs` は 0 件。`RunStore` 9・`ProcessController` 3・`WorktreeManager::create` 1 で AC-6 と一致する。
- `todo!` / `unimplemented!` / `TODO` / `FIXME` は 0 件。
- `Task` の `pub fn` は登録・再構築・アクセサ・問い合わせ5種・遷移6種のみ。`advance` / `abort` / `retry` / `set_status` / `mark_notified` / `complete_run` / `skip_run` / `fail_run` / `record_judge_failure` は存在しない。`GcPolicy` / `NotificationService` / `RunningClassifier` / `JudgementService` / `IdentityCheck` / `CommandRunner` / `RemoveOutcome` も 0 件。
- 呼び出しの無い `pub`：`RunStore::read_exit`（`write_exit` と対の往復を適合契約として閉じるため、`port.rs` 冒頭に why あり）、`Task::is_wait` / `is_cleanup`（動作種別の問い合わせ3種を一組で置く why あり）、`TickSummary` の未配線フィールド（ADR-065 / spec の出力DTO をそのまま持つ旨が struct doc にある）。いずれも理由が添えられている。`gc_deleted` / `gc_errors` の `(String, AttemptNumber)` も `spec/usecases/execution.md` の DTO 表と一致する。

### `tick` / `add` / `wrapper` の終了コードと出力

実バイナリで確認した。

- `add` 成功 0（stdout）/ 失敗 非0（stderr）。
- `tick` 成功 0、サマリーは stdout。ロック競合は 0 でスキップ表示、`LockError::Failed` と `list_active` の Io は非0（stderr）。config 不在・破損は非0。対象リポジトリの外（`/tmp`）から起動しても `add` → `tick`（起動）→ `tick`（起動確認）→ `tick`（処理対象なし）が同じ結果になる。作業ディレクトリを失った状態でも 0 で終わる専用の受け入れテストがある。
- `wrapper` はヘルプの一覧に現れず（`Commands:` は `add` / `tick` / `help`）、`pulsen wrapper --help` は到達できる。起動引数が不正（相対パス・0トークン・`derive` の像でない run_dir）ならいずれも run ディレクトリに何も書かず非0（stderr）。成功時は stdout に何も出さず、成果は `pid` / `starttime` / `exit` / ログとして現れる。
- 引数の往復を実バイナリで確認した。`-- <cmd...>` の後ろは `--model` / 空文字列トークン / さらなる `--` / `--home` / `--run-dir` を含めてもすべてリテラルのままエージェントへ渡る。`spawn_wrapper` が組む argv の定数（`WRAPPER_SUBCOMMAND` / `RUN_DIR_FLAG` / `WORKSPACE_FLAG` / `COMMAND_SEPARATOR`）は CLI 側のテストからも同じものが参照され、実 `spawn_wrapper` 経路は適合ケース TC-001 / 002 と `cli_tick` が通している。

### 縮退状態の共通規則と「書き込んだ tick の報告」

- 縮退状態の表（config 不在・破損 → 非0 / ロック競合 → 0 スキップ / state 配下不在 → 0 / タスクファイル パース不能 → 0 スキップ+報告 / スナップショット破損 → 0 スキップ+報告 / アーカイブ済み → 走査対象外 / run ディレクトリ不在 → 猶予経路 / worktree 不在 → 実行失敗の既存経路）は、いずれも受け入れテストまたはダブルテストで観測されている。
- `TickSummary::is_empty` を「処理対象なし」の判定にし、タスクファイルへ書き込む経路（起動・起動確認・spawn失敗の復帰・worktree作成失敗・展開失敗・凍結・保存失敗）がすべていずれかのフィールドを埋める形になっている（ADR-084 / ADR-086）。`ワークスペースを確定しただけで終わるtickは存在しない` がこの不変を直接主張している。
- 報告は `cli::render` の網羅 `match` で「失敗を記録」「起動の結果が未確定」「スキップ」に振り分けられ、書き込みの有無と見出しの語義が食い違わない（ADR-090）。文言はすべて CLI 層にあり、帳簿へ永続化される説明だけがドメインの `describe` を通る（ADR-073 / ADR-082）。
- 未配線のアーム（`Cleanup` / `Running` / `Completed` / `Stopped`）は `branch_of` の網羅 `match` に引き取り先の why つきで並び、ダミー処理も報告も置かれていない（ADR-065）。`配線していない分岐のタスクにはエージェントを起動せず書き込みもしない` が否定的主張として固定されている。

### ADR 参照

- コードと `.thread/2/*.md` に現れる ADR 参照 60 件はすべて `.adr/NNN-*.md` か `.thread/2/adr.md` の `## ADR-NNN:` に解決し、dangling も両方に存在する番号の衝突も無い。`.thread/2/adr.md` は 065〜091 を採番し、`.adr/` の最大 064 と衝突しない。
- `.adr/027` の変更は対応表の正本を `HOOKS.md` 一本に寄せるもので、書式は既存のまま（ADR-038 に反しない）。

### `.thread/2/` のドキュメントと実装

- plan.md AC-1〜AC-18 の機械的に検証できる項目（依存ゼロ・cfg の隔離・未実装メソッド不在・適合件数）はすべて一致する。適合スイートの件数は RunStore 21、ProcessController 16（`identity_and_agent` 13 + `spawn` 3）、WorktreeManager 17（台帳16 + ADR-077 由来の追加1）で、`HOOKS.md` の集計と対応表も同じ。
- steps.md のモジュール増分表・ポート宣言・ユースケースの署名と実装が一致する。progress.md の「spec 追従の提起」は実装で spec から外れた点（`rehydrate` / `state_root` / wrapper の終了コード / `errors` の構造化と分類追加 / `confirmed_running` / write 系のディレクトリ作成契約 / `ToolFailureKind` / `TransitionError` / `InconsistentRunFiles` / サマリーの見出し）を網羅している。
- コードとテストに、指摘への弁明や修正の経緯を残す記述は無い（`grep` で確認）。コメントはいずれも why / why not に限られている。

### ビルドと検証

`cargo build --examples` / `cargo test`（全ターゲット緑）/ `cargo clippy --all-targets -- -D warnings` / `cargo fmt --check` はいずれも通る。作業ツリーのソースコードは変更していない。

## カバレッジ

- 確認: `.adr/027-port-conformance-suite-and-harness-hooks.md`, `.thread/2/adr.md`, `.thread/2/plan.md`, `.thread/2/progress.md`, `.thread/2/steps.md`, `.thread/2/testing.md`, `crates/pulsen-conformance/HOOKS.md`, `crates/pulsen-conformance/src/lib.rs`, `crates/pulsen-conformance/src/process_controller.rs`, `crates/pulsen-conformance/src/run_store.rs`, `crates/pulsen-conformance/src/worktree_manager.rs`, `crates/pulsen-domain/src/definition/agent.rs`, `crates/pulsen-domain/src/definition/template.rs`, `crates/pulsen-domain/src/execution/launching.rs`, `crates/pulsen-domain/src/execution/mod.rs`, `crates/pulsen-domain/src/execution/port.rs`, `crates/pulsen-domain/src/execution/value.rs`, `crates/pulsen-domain/src/task/mod.rs`, `crates/pulsen-domain/src/task/path.rs`, `crates/pulsen-domain/src/task/planner.rs`, `crates/pulsen-domain/src/task/task.rs`, `crates/pulsen-domain/src/task/transition.rs`, `crates/pulsen/examples/agent_probe.rs`, `crates/pulsen/examples/spawn_probe.rs`, `crates/pulsen/src/adapter/mod.rs`, `crates/pulsen/src/adapter/process.rs`, `crates/pulsen/src/adapter/run_store.rs`, `crates/pulsen/src/adapter/worktree.rs`, `crates/pulsen/src/application/mod.rs`, `crates/pulsen/src/application/run_wrapper.rs`, `crates/pulsen/src/application/tick/confirm_spawn.rs`, `crates/pulsen/src/application/tick/launch.rs`, `crates/pulsen/src/application/tick/mod.rs`, `crates/pulsen/src/cli/add.rs`, `crates/pulsen/src/cli/args.rs`, `crates/pulsen/src/cli/mod.rs`, `crates/pulsen/src/cli/render.rs`, `crates/pulsen/src/cli/tick.rs`, `crates/pulsen/src/cli/wire.rs`, `crates/pulsen/src/cli/wrapper.rs`, `crates/pulsen/tests/cli_tick.rs`, `crates/pulsen/tests/cli_tick_missing_cwd.rs`, `crates/pulsen/tests/cli_usage.rs`, `crates/pulsen/tests/cli_wrapper.rs`, `crates/pulsen/tests/common/mod.rs`, `crates/pulsen/tests/conformance_process_controller.rs`, `crates/pulsen/tests/conformance_run_store.rs`, `crates/pulsen/tests/run_wrapper.rs`, `crates/pulsen/tests/tick_confirm_spawn.rs`, `crates/pulsen/tests/tick_launch.rs`, `crates/pulsen/tests/tick_scan.rs`
- スキップ: `.thread/2/review/review-001.md`, `.thread/2/review/review-001-adapter.md`, `.thread/2/review/review-001-architecture.md`, `.thread/2/review/review-001-domain.md`, `.thread/2/review/review-001-test.md`, `.thread/2/review/review-001-usecase.md`, `.thread/2/review/review-002-adapter.md`, `.thread/2/review/review-002-architecture.md`, `.thread/2/review/review-002-domain.md`, `.thread/2/review/review-002-test.md`, `.thread/2/review/review-002-usecase.md`, `.thread/2/review/review-003.md`, `.thread/2/review/review-003-adapter.md`, `.thread/2/review/review-003-architecture.md`, `.thread/2/review/review-003-domain.md`, `.thread/2/review/review-003-test.md`, `.thread/2/review/review-003-usecase.md`, `.thread/2/review/review-004.md`, `.thread/2/review/review-004-adapter.md`, `.thread/2/review/review-004-architecture.md`, `.thread/2/review/review-004-domain.md`, `.thread/2/review/review-004-test.md`, `.thread/2/review/review-004-usecase.md`, `.thread/2/review/review-005.md`, `.thread/2/review/review-005-adapter.md`, `.thread/2/review/review-005-architecture.md`, `.thread/2/review/review-005-domain.md`, `.thread/2/review/review-005-test.md`, `.thread/2/review/review-005-usecase.md`, `.thread/2/review/triage.md` — 過去のレビュー成果物。ゼロベースで見る指示のため読んでいない
- スキップ: `crates/pulsen-conformance/src/doubles/clock.rs`, `crates/pulsen-conformance/src/doubles/mod.rs`, `crates/pulsen-conformance/src/doubles/process.rs`, `crates/pulsen-conformance/src/doubles/run_store.rs`, `crates/pulsen-conformance/src/doubles/task_repository.rs`, `crates/pulsen-conformance/src/doubles/tests.rs`, `crates/pulsen-conformance/src/doubles/worktree.rs` — テストダブルの内部実装。ポート宣言との対応（未実装メソッド・スタブの不在）は `port.rs` 側とビルドで確認済みで、ダブルの挙動そのものは test / usecase 観点の担当
- スキップ: `crates/pulsen-domain/src/task/attempt.rs`, `crates/pulsen-domain/src/task/counters.rs`, `crates/pulsen-domain/src/task/failure.rs` — 遷移関数の内部で使われる値・カウンタの更新規則。公開 API の増分は `task/mod.rs` の再エクスポートと `task.rs` の `pub fn` 一覧で確認済みで、規則そのものは domain 観点の担当
- スキップ: `crates/pulsen/tests/common/git.rs`, `crates/pulsen/tests/conformance_worktree.rs`, `crates/pulsen/tests/register_task.rs`, `crates/pulsen/tests/tick_fixture/mod.rs` — git フィクスチャ・worktree 適合の適用・既存ユースケーステスト・ダブル用フィクスチャ。フィクスチャの置き場を分ける判断（ADR-085）は `tick_*.rs` 側の `use` で確認済みで、中身は adapter / test 観点の担当
