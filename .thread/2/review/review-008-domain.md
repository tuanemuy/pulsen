# レビュー 008 — Domain

## Domain

### Blockers

なし

### Warnings

なし

**問題なし。** 指摘に足るものは見つからなかった。

### 検証したこと

- **純粋性・依存**: `pulsen-domain/Cargo.toml` の `[dependencies]` は空のまま。ドメインの本番経路に `cfg(` は1件も無い(`cfg(test)` のみ)。I/O・外部クレート・表示の完成文言(`TransitionError` / `InconsistentRunFiles` は分類のみを持ち、文言は `cli::render`)がドメインに無いこと。
- **パニックの範囲**: 本番経路の `expect` は3箇所だけで、いずれも不変条件の言明 — `Task::current_status_def`(不変条件1は `register` / `rehydrate` の両生成経路で保証)、`WorkspacePlanner::derive` の 2件(`TaskId` の文字集合 `[a-z0-9-]`・先頭英数字から作る `<worktree_root>/<id>` は常に絶対パス、`pulsen/<id>` は `BranchName::parse` の全制約 — 空・空白/制御文字・先頭 `-`・`..`・前後 `/`・`.lock` 終端 — を構造上満たす)。`unwrap` / `panic!` / `todo!` は無い。
- **網羅 `match`**: 追加された `match` にワイルドカードアームは無い。`classify` / `classify_recheck` は `(pid, starttime)` の4組を明示、`ensure_restartable` / `ensure_launching` / `Freeze::of_recorded_failure` / `is_agent_run` 系は実行状態・動作種別を列挙している。
- **遷移の事後条件とカウンタ規則**(AC-5): `record_launching` は次番号を採番して `run_dir` を内部導出し番号との一致を構成で保証(`AttemptRef::launching` は `pub(super)`)、カウンタは触らない。`confirm_running` は `spawn_fail_count` だけを 0 にし `attempt_count` / `judge_attempt_count` と attempt 番号・run ディレクトリを保持。`record_spawn_failure_in_place` は実行状態も `current_attempt` も変えない。上限超過は `limit_exceeded`(`count > limit`)に集約され、等号では凍結しない。凍結は常に `notified_at: None`。`ToolFailureKind` により「`attempt_count` を進める遷移に spawn/judge の失敗種別を渡す」帳簿が型として書けない。
- **`LaunchingClassifier` の境界**(AC-3): 30秒=`KeepWaiting` / 31秒=`SuspectSpawnFailure` / 巻き戻り(`elapsed_since` の 0 飽和)=`KeepWaiting`、pid+starttime は猶予の内外によらず `ConfirmRunning`、pid のみは `Err(MissingStartTime)`、再確認の pid なしは starttime の有無を問わず `SpawnFailed`。すべてユニットテストで押さえられている。
- **値と導出**(AC-2 / AC-4): `ExitCode::is_success`・`PidFileContent`・`RunDirPath` の6導出・`attempt_dir_name`・`state_root()` の逆写像(`derive` との一致で表記ゆれ `attempt-01` / `attempt-+1` を弾く)・`WorkspacePlanner::derive` はいずれも純粋で、テストが値まで固定している。
- **ポートの形**(AC-6): `RunStore` 9・`ProcessController` 3・`WorktreeManager` に `create` 1 で spec のポート表と一致し、`attempt_exists` / `list_runs` / `delete_attempt` / `remove_task_dir_if_empty` / `starttime_of` / `kill` / `try_kill_remnants` / `remove` の宣言もスタブも無い。付随する値・エラー型(`WrapperLaunchSpec` / `WrapperIdentity` / `SpawnError` / `WorktreeError` / `RunFileError` / `Io`)はすべて値で返り、契約は doc コメントに書かれている。
- **ドメインロジックの外部漏れ・レイアウトの複製**: 猶予時間・上限規則・採番・ワークスペース導出・run ディレクトリのファイル名は、いずれも本番コードではドメインの1箇所からしか出ていない(`GRACE_PERIOD` の参照は `confirm_spawn` の文言生成のみ、`attempt-` / `pid` / `invalidated` 等のリテラルは `RunDirPath` の外の本番コードに無い)。ラッパーの合成ルートは `RunDirPath::state_root()` を通し、レイアウトを組み立て直していない。
- **テストの実効性**: ドメインのユニットテスト221件が通ることを確認したうえで、使い捨てコピー(scratchpad)に対する5つのミューテーション — 猶予の `>` → `>=`、`limit_exceeded` の `>` → `>=`、`reset_spawn_fail` が `attempt_count` も 0 にする、`record_spawn_failure_in_place` が常に `Pending` を返す、`state_root()` の往復検査の削除 — がすべて対応するテストで落ちることを確認した(コピーは削除済み。作業ツリーは無変更)。
- **spec との差分**: `TransitionError`(`expected: &'static [ExecutionStateKind]` / `InvariantViolated` → `MissingCurrentAttempt`)・`ToolFailureKind` の新設・`CommandLine::rehydrate`・`RunDirPath::state_root`・write 系のディレクトリ自動作成は、いずれも `.thread/2/adr.md`(ADR-070 / 071 / 073 とその周辺)に判断と spec 追従の提起として記録されている。`describe()` を持つエラー型は Issue #1 で確立済みの規約に沿う。

### カバレッジ

- 確認(30): `.thread/2/plan.md`, `.thread/2/adr.md`, `crates/pulsen-domain/src/definition/agent.rs`, `crates/pulsen-domain/src/definition/template.rs`, `crates/pulsen-domain/src/execution/launching.rs`, `crates/pulsen-domain/src/execution/mod.rs`, `crates/pulsen-domain/src/execution/port.rs`, `crates/pulsen-domain/src/execution/value.rs`, `crates/pulsen-domain/src/task/attempt.rs`, `crates/pulsen-domain/src/task/counters.rs`, `crates/pulsen-domain/src/task/failure.rs`, `crates/pulsen-domain/src/task/mod.rs`, `crates/pulsen-domain/src/task/path.rs`, `crates/pulsen-domain/src/task/planner.rs`, `crates/pulsen-domain/src/task/task.rs`, `crates/pulsen-domain/src/task/transition.rs`, `crates/pulsen/src/application/tick/mod.rs`, `crates/pulsen/src/application/tick/launch.rs`, `crates/pulsen/src/application/tick/confirm_spawn.rs`, `crates/pulsen/src/application/run_wrapper.rs`, `crates/pulsen/src/adapter/run_store.rs`, `crates/pulsen/src/adapter/process.rs`, `crates/pulsen/src/adapter/worktree.rs`, `crates/pulsen/src/cli/render.rs`, `crates/pulsen/src/cli/wire.rs`, `crates/pulsen/src/cli/wrapper.rs`, `crates/pulsen-conformance/src/doubles/clock.rs`, `crates/pulsen-conformance/src/doubles/mod.rs`, `crates/pulsen-conformance/src/doubles/process.rs`, `crates/pulsen-conformance/src/doubles/run_store.rs`

  ドメイン14ファイルは差分全量を読み、それ以外は「ドメインロジックの外部漏れ・レイアウト知識の複製・ドメイン型の使い方」の観点で該当箇所を読んだ。

- スキップ(77):
  - `.adr/027-port-conformance-suite-and-harness-hooks.md`, `crates/pulsen-conformance/HOOKS.md` — 適合ハーネスのフック契約。Adapter / Test 観点。
  - `.thread/2/progress.md`, `.thread/2/steps.md`, `.thread/2/testing.md` — 進行・手順の記録で、ドメインの実装判断を含まない。
  - `.thread/2/review/review-001-adapter.md`, `.thread/2/review/review-001-architecture.md`, `.thread/2/review/review-001-domain.md`, `.thread/2/review/review-001-test.md`, `.thread/2/review/review-001-usecase.md`, `.thread/2/review/review-001.md`, `.thread/2/review/review-002-adapter.md`, `.thread/2/review/review-002-architecture.md`, `.thread/2/review/review-002-domain.md`, `.thread/2/review/review-002-test.md`, `.thread/2/review/review-002-usecase.md`, `.thread/2/review/review-003-adapter.md`, `.thread/2/review/review-003-architecture.md`, `.thread/2/review/review-003-domain.md`, `.thread/2/review/review-003-test.md`, `.thread/2/review/review-003-usecase.md`, `.thread/2/review/review-003.md`, `.thread/2/review/review-004-adapter.md`, `.thread/2/review/review-004-architecture.md`, `.thread/2/review/review-004-domain.md`, `.thread/2/review/review-004-test.md`, `.thread/2/review/review-004-usecase.md`, `.thread/2/review/review-004.md`, `.thread/2/review/review-005-adapter.md`, `.thread/2/review/review-005-architecture.md`, `.thread/2/review/review-005-domain.md`, `.thread/2/review/review-005-test.md`, `.thread/2/review/review-005-usecase.md`, `.thread/2/review/review-005.md`, `.thread/2/review/review-006-adapter.md`, `.thread/2/review/review-006-architecture.md`, `.thread/2/review/review-006-domain.md`, `.thread/2/review/review-006-test.md`, `.thread/2/review/review-006-usecase.md`, `.thread/2/review/review-006.md`, `.thread/2/review/review-007-adapter.md`, `.thread/2/review/review-007-architecture.md`, `.thread/2/review/review-007-domain.md`, `.thread/2/review/review-007-test.md`, `.thread/2/review/review-007-usecase.md`, `.thread/2/review/review-007.md`, `.thread/2/review/triage.md` — 過去ラウンドの指摘。ゼロベースで見る指示により読まない。
  - `crates/pulsen-conformance/src/lib.rs`, `crates/pulsen-conformance/src/process_controller.rs`, `crates/pulsen-conformance/src/run_store.rs`, `crates/pulsen-conformance/src/worktree_manager.rs`, `crates/pulsen-conformance/src/doubles/task_repository.rs`, `crates/pulsen-conformance/src/doubles/tests.rs`, `crates/pulsen-conformance/src/doubles/worktree.rs` — ポート実装の適合検証。Adapter / Test 観点(レイアウト・分類規則の複製が無いことだけ横断 grep で確認した)。
  - `crates/pulsen/examples/agent_probe.rs`, `crates/pulsen/examples/spawn_probe.rs` — テスト用フィクスチャのバイナリ。
  - `crates/pulsen/src/adapter/mod.rs`, `crates/pulsen/src/application/mod.rs`, `crates/pulsen/src/cli/mod.rs` — モジュール宣言の追加。
  - `crates/pulsen/src/cli/add.rs`, `crates/pulsen/src/cli/args.rs`, `crates/pulsen/src/cli/tick.rs` — CLI の引数定義と結果の受け渡し。Architecture / Usecase 観点。
  - `crates/pulsen/tests/cli_tick.rs`, `crates/pulsen/tests/cli_tick_missing_cwd.rs`, `crates/pulsen/tests/cli_usage.rs`, `crates/pulsen/tests/cli_wrapper.rs`, `crates/pulsen/tests/common/git.rs`, `crates/pulsen/tests/common/mod.rs`, `crates/pulsen/tests/conformance_process_controller.rs`, `crates/pulsen/tests/conformance_run_store.rs`, `crates/pulsen/tests/conformance_worktree.rs`, `crates/pulsen/tests/register_task.rs`, `crates/pulsen/tests/run_wrapper.rs`, `crates/pulsen/tests/tick_confirm_spawn.rs`, `crates/pulsen/tests/tick_fixture/mod.rs`, `crates/pulsen/tests/tick_launch.rs`, `crates/pulsen/tests/tick_scan.rs` — 受け入れ・ユースケース層のテスト。ドメインの遷移規則はドメインのユニットテストで検証済みで、こちらは Test / Usecase 観点。
