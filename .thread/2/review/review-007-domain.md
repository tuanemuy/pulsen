# レビュー007 — Domain

## Domain

### Blockers

なし。

`spec/domains/task.md` / `spec/domains/execution.md` の値オブジェクト・遷移関数・問い合わせ・ドメインサービス・ポート契約と、AC-2〜AC-6 の要求を1つずつ突き合わせたが、ブロックすべき不整合は見つからなかった。以下は確認した主要な点。

- **関数型ドメインモデリング**: 遷移6種はすべて `self` を消費して新しい `Task` を返し、前提の破れは `Result<_, TransitionError>` の値になる。生成口は閉じている(`AttemptRef::launching` / `with_process`、`RetryCounters::increment_*` / `reset_spawn_fail`、`FailureNote::record`、`ToolFailureKind::recorded` はいずれも `pub(super)`)ので、番号と run ディレクトリが食い違う `AttemptRef` も、カウンタと失敗種別が食い違う帳簿も型として書けない。
- **パニックの位置**: ドメイン側の `expect` は `Task::current_status_def`(不変条件1。`rehydrate` / `register` の両生成経路で保証される)と `WorkspacePlanner::derive`(`TaskId` の文字集合がパス・git 参照名として常に安全)の2箇所だけで、どちらも不変条件違反に限られている。手動修復で破られうる不変条件2は `TransitionError::MissingCurrentAttempt` として値で返っている。
- **ワイルドカード `match` の不在**: 新規・変更したドメインコードに `_ =>` は1件もない。`ensure_restartable` / `ensure_launching` / `is_agent_run` 系 / `applicable_retry_limit` / `classify` / `classify_recheck` / `ToolFailureKind::recorded` はいずれも全変種を列挙している。
- **遷移の純粋性・外部依存の不在**: `pulsen-domain` に依存クレートの追加はなく、新規ドメインコードが触る `std` は `std::path` / `std::ffi::OsStr` だけ。ターゲット述語つき `cfg`(`unix` / `windows` / `target_os` / `target_family`)は `crates/pulsen-domain/src/` に1件も無い(AC-1)。
- **事後条件・カウンタ規則**: `limit_exceeded(count, limit) = count > limit` を1関数に集約し、3つの記録系遷移が共有している。等号では凍結しない・`confirm_running` は `spawn_fail_count` だけをリセットする・`record_spawn_failure_in_place` は実行状態も `current_attempt` も変えない・`record_spawn_failure` は `current_attempt` を保持する、がいずれもユニットテストで主張されている。上限0の即時凍結、`u32` の飽和加算も確認した。
- **`LaunchingClassifier` の境界**: 猶予は `elapsed_since`(巻き戻りを0に飽和)と `>` の組で判定され、30秒 = `KeepWaiting` / 31秒 = `SuspectSpawnFailure` / 負 = `KeepWaiting` が網羅されている。`(None, Some)` が猶予経路へ合流し、`(Some, None)` だけが `Err` になる場合分けも `classify` / `classify_recheck` の両方で一致している。分類サービスは run_dir もタスクIDも受け取らず、報告用の文脈を持ち込んでいない。
- **表示知識の不在**: `TransitionError::InvalidState.expected` は `&'static [ExecutionStateKind]`、`InconsistentRunFiles` は分類のみの enum で、完成文言はドメインに無い(ADR-073 / ADR-089 と整合)。spec の `expected: &'static str` / `InvariantViolated { message: String }` からの逸脱は adr.md に「spec 追従の提起」として記録済み。`describe()` は既存(main)からの規約で、タスクファイルに残る失敗要因の説明をドメインに1つ置くという既定の位置づけを踏襲している。
- **ドメインロジックの外部漏れ**: attempt の採番は `record_launching` だけが行い、ユースケースは返った `AttemptRef` から番号を読み直している。run ディレクトリのパス導出は `RunDirPath` に閉じ、`FsRunStore::prepare_attempt` も `RunDirPath::derive` に委譲している。猶予時間の値は `LaunchingClassifier::GRACE_PERIOD` からしか読まれていない。ラッパーの state root 復元は `derive` の逆写像として `RunDirPath` に置かれ、往復一致で検証されている(`attempt-01` / `attempt-+1` のような数値として読めるが `derive` が出力しない表記を `None` に落とすこと込みでテスト済み)。
- **ポートの1:1**(AC-6): `RunStore` 9・`ProcessController` 3・`WorktreeManager` の `create` 1 のみで、`attempt_exists` / `list_runs` / `delete_attempt` / `remove_task_dir_if_empty` / `starttime_of` / `kill` / `try_kill_remnants` / `remove` の宣言・スタブはリポジトリ全体に存在しない。
- **テスト**: `cargo test -p pulsen-domain` は 220件が通る。遷移の前提状態検査は `every_execution_state()` による6状態の網羅で書かれており、変種が増えたときに検査が漏れない形になっている。

### Warnings

- **[W-001]** attempt ディレクトリの命名規則がドメインの外に複製されている。
  - 場所: `crates/pulsen/src/cli/render.rs:272-279`(`push_attempts` の `format!("{dir}/attempt-{}", number.get())`)
  - 理由: `attempt-<n>` という run ディレクトリのレイアウト語彙は `RunDirPath::ATTEMPT_PREFIX` を単一の定義箇所として本 PR で明示的に導入したのに、表示側が同じ文字列を直書きしている。`RunDirPath::state_root` の doc が「レイアウトの知識を合成ルートに漏らさないため、復元を `derive` の直下に置く」と述べている方針と、同じ PR の中で食い違う。接頭辞を変えたときに gc の表示だけが黙って古い綴りのまま残る。
  - なお本スライスでは `gc_deleted` / `gc_errors` を埋める経路が無いため実害は出ないが、`crates/pulsen/src/cli/render.rs` のユニットテストが `.../attempt-1` という綴りを固定しているので、複製は #6 にそのまま引き継がれる。
  - 提案: `GcPlan` の要素はパースできない孤児のディレクトリ名を含むので `RunDirPath::derive` は使えない。`RunDirPath` に `attempt_dir_name(number: AttemptNumber) -> String`(`ATTEMPT_PREFIX` を使う)を1つ足して表示側がそれを呼ぶか、それを #6 の作業として残すなら、`push_attempts` に「命名規則の定義箇所はドメインであり、gc の配線時に導出へ寄せる」ことを why として書き添える。

### カバレッジ

一覧101行に対して 確認 25 / スキップ 76。

- 確認(25):
  - `.thread/2/plan.md`, `.thread/2/adr.md`
  - `crates/pulsen-domain/src/definition/agent.rs`, `crates/pulsen-domain/src/definition/template.rs`
  - `crates/pulsen-domain/src/execution/launching.rs`, `crates/pulsen-domain/src/execution/mod.rs`, `crates/pulsen-domain/src/execution/port.rs`, `crates/pulsen-domain/src/execution/value.rs`
  - `crates/pulsen-domain/src/task/attempt.rs`, `crates/pulsen-domain/src/task/counters.rs`, `crates/pulsen-domain/src/task/failure.rs`, `crates/pulsen-domain/src/task/mod.rs`, `crates/pulsen-domain/src/task/path.rs`, `crates/pulsen-domain/src/task/planner.rs`, `crates/pulsen-domain/src/task/task.rs`, `crates/pulsen-domain/src/task/transition.rs`
  - `crates/pulsen-conformance/src/run_store.rs`, `crates/pulsen-conformance/src/process_controller.rs` — ポート契約の表現がドメインの契約と一致するか、分類ロジックを再実装していないかの確認まで
  - `crates/pulsen/src/adapter/run_store.rs` — レイアウト導出をドメインに委譲しているかの確認まで
  - `crates/pulsen/src/application/run_wrapper.rs`, `crates/pulsen/src/application/tick/mod.rs`, `crates/pulsen/src/application/tick/launch.rs`, `crates/pulsen/src/application/tick/confirm_spawn.rs` — ドメインロジックの外部漏れ(採番・猶予・上限判定・分類の再実装)の確認まで
  - `crates/pulsen/src/cli/render.rs`, `crates/pulsen/src/cli/wrapper.rs` — 表示・parse 境界にドメインの語彙が複製されていないかの確認まで
- スキップ(76):
  - `.adr/027-port-conformance-suite-and-harness-hooks.md`, `crates/pulsen-conformance/HOOKS.md` — 適合ハーネスの契約文書。Adapter / Test 観点の担当
  - `.thread/2/progress.md`, `.thread/2/steps.md`, `.thread/2/testing.md` — 進捗・手順の記録で、ドメインの判断材料にならない
  - `.thread/2/review/` 配下36ファイル(`review-001-*` 〜 `review-006-*`, `review-001` / `003` / `004` / `005` / `006`, `triage.md`)— 本ラウンドはゼロベースのため読まない指示
  - `crates/pulsen-conformance/src/lib.rs`, `crates/pulsen-conformance/src/doubles/clock.rs`, `.../doubles/mod.rs`, `.../doubles/process.rs`, `.../doubles/run_store.rs`, `.../doubles/task_repository.rs`, `.../doubles/tests.rs`, `.../doubles/worktree.rs`, `crates/pulsen-conformance/src/worktree_manager.rs` — テストダブル・ハーネスの実装で Test / Adapter 観点の担当
  - `crates/pulsen/examples/agent_probe.rs`, `crates/pulsen/examples/spawn_probe.rs` — 適合テスト用の被験プログラム
  - `crates/pulsen/src/adapter/mod.rs`, `crates/pulsen/src/adapter/process.rs`, `crates/pulsen/src/adapter/worktree.rs` — OS 依存の実装で Adapter 観点の担当(ドメインへの `cfg` 漏れが無いことは grep で確認済み)
  - `crates/pulsen/src/application/mod.rs` — モジュール宣言のみ
  - `crates/pulsen/src/cli/add.rs`, `crates/pulsen/src/cli/args.rs`, `crates/pulsen/src/cli/mod.rs`, `crates/pulsen/src/cli/tick.rs`, `crates/pulsen/src/cli/wire.rs` — 引数定義・合成ルート・終了コードで Architecture / Usecase 観点の担当
  - `crates/pulsen/tests/` 配下15ファイル(`cli_tick.rs`, `cli_tick_missing_cwd.rs`, `cli_usage.rs`, `cli_wrapper.rs`, `common/git.rs`, `common/mod.rs`, `conformance_process_controller.rs`, `conformance_run_store.rs`, `conformance_worktree.rs`, `register_task.rs`, `run_wrapper.rs`, `tick_confirm_spawn.rs`, `tick_fixture/mod.rs`, `tick_launch.rs`, `tick_scan.rs`)— 受け入れ・ユースケーステストで Test / Usecase 観点の担当。ドメインの遷移・分類のユニットテストは各ドメインソース内にあり、そちらは確認済み
