# レビュー 003 — Use Case

対象: PR #11(`issue/2/tick-agent-run-launch`)/ 契約: `.thread/2/plan.md`

## Use Case

### Blockers

なし

### Warnings

- **[W-001]** 「保存できた遷移だけを集計する」規則がテストで守られていない / 場所: `crates/pulsen/src/application/tick/mod.rs:319-336`(`commit`)、`crates/pulsen/src/application/tick/launch.rs:113-123`、`crates/pulsen/tests/tick_confirm_spawn.rs:544-553` / 理由: ADR-084 / ADR-086 は「凍結は `frozen`、記録した失敗は `errors`、ただし**保存できたときにだけ**積む」を tick の不変として置いている。実装はこれを満たすが、破っても落ちるテストが1つも無い。使い捨てのコピー(`/private/tmp/.../scratchpad/mut`)で次の2つのミューテーションを入れたところ、`tick_scan` / `tick_launch` / `tick_confirm_spawn` の51件が**すべて緑のまま**だった。(1) `commit` で `save` の結果を待たずに `frozen` を積む、(2) `ensure_workspace` の `Persisted::Failed` でも `WorktreeCreateFailed` を積む。どちらも「永続化されなかった状態変更をサマリーが起きたことにして表示する」結末で、cron 運用ではこの出力が唯一の窓であるという ADR-084 の前提に直接反する。`tick_confirm_spawn.rs:549-552` の `assert!(summary.frozen.is_empty(), "永続化できていない凍結は数えない")` はこの規則を主張しているつもりだが、その経路は `confirm_running`(`Freeze::NotFrozen`)なので、規則を壊しても空虚に成立する / 提案: 上限超過の遷移が `save` に失敗するケースを1件足す(例: `retries: 0` のスナップショット × `worktrees.create` 失敗 × `repository_failing_save` で、`summary.frozen` が空・`summary.errors` が `SaveFailed` の1件だけになること)。同時に `tick_confirm_spawn.rs:549-552` の主張を、規則を実際に張る側のテストへ移すか、空虚でない主張に置き換える。

### 確認した仕様適合(記録)

Blocker / Warning に至らなかったが、指示された観点の確認結果を残す。

- **Tick の処理フロー 1〜9**: ロック取得 → 走査 → エントリ分岐 → サマリーの順で `spec/usecases/execution.md` と一致。ロック競合は `TickOutcome::Skipped`(0)、`LockError::Failed` と `list_active` の Io だけが `TickError`(非0・書き込みなし)。`Corrupt` は報告のみ、`SnapshotUnreadable` は定義依存の判断をせず報告。
- **手続きA の順序**: worktree確保 → 展開5段 → `record_launching` → `save` → `prepare_attempt` → `spawn_wrapper`。`prepare_attempt` の失敗と `spawn_wrapper` の同期エラーは状態を変更せず報告のみで、launching のまま猶予経路に委ねる(`tick_launch.rs` が保存済みタスクの実行状態と attempt 番号で主張)。展開失敗5経路はすべて `record_spawn_failure_in_place`(attempt 不採番・runディレクトリ不作成)。
- **手続きC の順序と競合窓**: `current_attempt` None の冒頭検査 → `read_pid_file` → `read_starttime` → `classify` → `SuspectSpawnFailure` なら `write_invalidation_marker` → 再読 → `classify_recheck`。マーカー書き込みが pid 再確認より**先**であることは `tick_confirm_spawn.rs:163-193` が `RunStoreCall` の並びで固定している。読みの順序が pid → starttime である点も重要で(starttime を先に読むと「starttime 不在 → その後 pid 出現」を書き込み順序の破れと誤報告しうる)、こちらも呼び出しの並びで固定されている。マーカー書き込み失敗で pending へ戻さないことも主張済み。
- **RunWrapper の順序**: `own_identity` → `write_starttime` → `write_pid_file` → `marker_exists` → `run_agent` → `write_exit`。starttime が pid より先であること、マーカー確認が pid の後であることは `run_wrapper.rs:93-119` が呼び出しの並びで固定。マーカーあり・`Err(Io)` のどちらも `run_agent` を呼ばない(`processes.calls()` が `OwnIdentity` の1件だけであることで主張)。config を読まず(`wire::compose_wrapper` は `run_dir` から `StateRoot` を逆導出するだけ)、ロックも取らない。
- **1タスクの失敗と続行**: `errors` に積んで残りを続行することを、走査・起動の両側で主張(`tick_scan.rs:242-275`)。
- **書き込んだ tick が「処理対象なし」にならないこと**: 書き込みを伴う全経路(worktree確定 → 展開失敗、起動、起動確認、spawn失敗復帰、凍結、`save` 失敗)がサマリーのいずれかを埋めることを追跡し、`ワークスペースを確定しただけで終わるtickは存在しない` を含むテストで固定されている。
- **冪等性**: 状態が変わらないタスク群での連続実行に書き込みが発生しないこと、worktree 作成直後のクラッシュで同じワークスペースが再導出されること、破損 runファイルの削除でマーカープロトコルに合流することを、それぞれ tick を2回以上回すテストで確認。
- **スコープ外の不実装**: `Cleanup` / `Observe` / `Advance` / `Notify` のアームは空で、`advance` / notify / `WorktreeManager::remove` / `list_runs` / `delete_attempt` / `starttime_of` / `kill` / `GcPolicy` などの呼び出しも宣言も無い(grep 済み)。未配線アームに対しては「書き込まない・エージェントを起動しない」だけを主張しており、後続スライスの期待を先取りしていない。
- **判断と副作用の分離**: 分岐の判別は `branch_of` が値(`Branch`)にしてから手続きへ渡し、遷移の判断は `Task` の遷移関数と `LaunchingClassifier` に閉じている。ユースケース側に残るのは観測 → 判断 → 実行の配線と報告のみ。
- **ポート面**: `RunStore` 9・`ProcessController` 3・`WorktreeManager::create` の追加のみで、スコープ外メソッドの宣言・スタブは無い(AC-6)。
- テスト実行: `cargo test --test tick_scan --test tick_launch --test tick_confirm_spawn --test run_wrapper` は 51 件すべて緑。

### カバレッジ

- 確認: `.thread/2/adr.md`, `.thread/2/plan.md`, `crates/pulsen-conformance/src/doubles/clock.rs`, `crates/pulsen-conformance/src/doubles/mod.rs`, `crates/pulsen-conformance/src/doubles/process.rs`, `crates/pulsen-conformance/src/doubles/run_store.rs`, `crates/pulsen-conformance/src/doubles/task_repository.rs`, `crates/pulsen-conformance/src/doubles/worktree.rs`, `crates/pulsen-domain/src/execution/launching.rs`, `crates/pulsen-domain/src/execution/port.rs`, `crates/pulsen-domain/src/task/planner.rs`, `crates/pulsen-domain/src/task/transition.rs`, `crates/pulsen/src/adapter/process.rs`, `crates/pulsen/src/application/mod.rs`, `crates/pulsen/src/application/run_wrapper.rs`, `crates/pulsen/src/application/tick/confirm_spawn.rs`, `crates/pulsen/src/application/tick/launch.rs`, `crates/pulsen/src/application/tick/mod.rs`, `crates/pulsen/src/cli/args.rs`, `crates/pulsen/src/cli/mod.rs`, `crates/pulsen/src/cli/render.rs`, `crates/pulsen/src/cli/tick.rs`, `crates/pulsen/src/cli/wire.rs`, `crates/pulsen/src/cli/wrapper.rs`, `crates/pulsen/tests/cli_tick.rs`, `crates/pulsen/tests/cli_usage.rs`, `crates/pulsen/tests/cli_wrapper.rs`, `crates/pulsen/tests/common/mod.rs`, `crates/pulsen/tests/register_task.rs`, `crates/pulsen/tests/run_wrapper.rs`, `crates/pulsen/tests/tick_confirm_spawn.rs`, `crates/pulsen/tests/tick_fixture/mod.rs`, `crates/pulsen/tests/tick_launch.rs`, `crates/pulsen/tests/tick_scan.rs`(34件)
- スキップ:
  - `.thread/2/review/review-001-adapter.md`, `.thread/2/review/review-001-architecture.md`, `.thread/2/review/review-001-domain.md`, `.thread/2/review/review-001-test.md`, `.thread/2/review/review-001-usecase.md`, `.thread/2/review/review-001.md`, `.thread/2/review/review-002-adapter.md`, `.thread/2/review/review-002-architecture.md`, `.thread/2/review/review-002-domain.md`, `.thread/2/review/review-002-test.md`, `.thread/2/review/review-002-usecase.md`, `.thread/2/review/triage.md` — ゼロベースでのレビューのため読まない指示
  - `.thread/2/progress.md`, `.thread/2/steps.md`, `.thread/2/testing.md` — 進行管理の記録で、ユースケースの振る舞いを規定しない
  - `crates/pulsen-conformance/HOOKS.md`, `crates/pulsen-conformance/src/lib.rs`, `crates/pulsen-conformance/src/process_controller.rs`, `crates/pulsen-conformance/src/run_store.rs`, `crates/pulsen-conformance/src/worktree_manager.rs`, `crates/pulsen-conformance/src/doubles/tests.rs`, `crates/pulsen/tests/conformance_process_controller.rs`, `crates/pulsen/tests/conformance_run_store.rs`, `crates/pulsen/tests/conformance_worktree.rs` — ポート適合スイートとその台帳(アダプター / テスト観点)
  - `crates/pulsen-domain/src/definition/agent.rs`, `crates/pulsen-domain/src/definition/template.rs`, `crates/pulsen-domain/src/execution/mod.rs`, `crates/pulsen-domain/src/execution/value.rs`, `crates/pulsen-domain/src/task/attempt.rs`, `crates/pulsen-domain/src/task/counters.rs`, `crates/pulsen-domain/src/task/failure.rs`, `crates/pulsen-domain/src/task/mod.rs`, `crates/pulsen-domain/src/task/path.rs`, `crates/pulsen-domain/src/task/task.rs` — 遷移・値の内部規則(ドメイン観点。ユースケースからは呼び出し面と結果だけを見た)
  - `crates/pulsen/src/adapter/mod.rs`, `crates/pulsen/src/adapter/run_store.rs`, `crates/pulsen/src/adapter/worktree.rs`, `crates/pulsen/examples/agent_probe.rs`, `crates/pulsen/examples/spawn_probe.rs`, `crates/pulsen/tests/common/git.rs` — 実アダプターと受け入れテストの道具(アダプター / テスト観点)
  - (40件)
