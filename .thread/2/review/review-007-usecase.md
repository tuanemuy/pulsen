# レビュー 007 — Use Case

対象: PR #11(`issue/2/tick-agent-run-launch`)/ 契約: `.thread/2/plan.md`(AC-10 / AC-12〜AC-15 / AC-17)

## Use Case

### Blockers

なし

### Warnings

なし

指摘なし(問題なし)。以下は「確認した結果、契約どおりであること」の記録。

#### 処理フロー1〜9(AC-12)

`Tick::execute` は spec の順に「ロック取得 → `list_active` → エントリごとの分岐 → サマリー」だけを行う。
ロック競合は `TickOutcome::Skipped`(CLI で 0)、`LockError::Failed` と `ReadError::Io` は
`TickError` で非0、いずれも書き込みを伴わない(`tick_scan.rs` の3件が主張)。
分岐は `branch_of` が実行状態 × 動作種別の網羅 `match` で判断を値(`Branch`)にしてから
手続きへ渡す形になっており、`Corrupt` / `SnapshotUnreadable` は報告のみ、`Intact` は実行状態で
分岐する。手順8(gc)・手順6/7(advance / notify)は ADR-065 のとおり空アームで、
`tick_scan.rs::配線していない分岐のタスクには…` が「起動しない・書き込まない」だけを主張していて
下流スライスの期待を先取りしていない。

#### 手続きA(AC-13)

順序は `ensure_workspace`(derive → `create` → `confirm_workspace` → `save`)→ `expand`(5段)→
`record_launching` → `save` → `prepare_attempt` → `spawn_wrapper` で spec と一致する。
失敗時の遷移も一致 — worktree 作成失敗は `record_tool_failure(WorktreeCreate, …, applicable_retry_limit)`、
展開失敗5経路はすべて `record_spawn_failure_in_place(…, config.spawn_fail_limit)`、
`prepare_attempt` の失敗と `spawn_wrapper` の同期エラーは**状態を変更せず報告のみ**。
`save` が失敗した時点でそのタスクの処理を打ち切る(`Persisted::Failed`)ため、
起動記録を永続化できないまま spawn する経路が存在しない(`tick_launch.rs::起動記録の保存に失敗すれば…`)。

#### 手続きC・マーカー順序プロトコル(AC-14)

冒頭の `current_attempt` None 検査 → `read_pid_file` / `read_starttime` → `classify` →
(`SuspectSpawnFailure` のとき)`write_invalidation_marker` → 再読 → `classify_recheck` の順で、
マーカー書き込みの失敗では状態を変更せずスキップする。競合窓を追っても二重起動は生じない:

- tick が `SpawnFailed` を確定できるのは「マーカー書き込み後の再読で pid が不在」のときだけ。
  ラッパーは pid を書いた**後**にマーカーを確認するので、この観測が成立する世界では
  ラッパーのマーカー確認は必ずマーカー書き込みより後になり、エージェントは起動されない。
- 逆にラッパーが先にマーカーを確認して起動した場合、pid はその前に書かれているので
  再読は `ConfirmRunning` になり、pending へ戻らない。

`read_run_files` が pid → starttime の順で読むのも正しい。逆順だと、2回の読み取りの間に
ラッパーが両方を書いた正常な起動が「pid あり・starttime なし」と観測され、健全なタスクが
順序破れとして launching に滞留する。この why はコメントにも残っている。

#### `RunWrapper` の順序(AC-10)

`own_identity` → `write_starttime` → `write_pid_file` → `marker_exists` → `run_agent` → `write_exit`。
starttime が pid より先であること・マーカー確認が pid の後であることは、値に現れないので
ダブルの `calls()` の並びで主張されている(`tests/run_wrapper.rs`)。`marker_exists` の
`Ok(true)` と `Err(Io)` はどちらも起動せず正常終了、ステップ1〜3の失敗は何も書き残さず終了、
exit の書き込み失敗は結末を変えない。config もロックも型の上で持たない(`RunWrapper` の
ジェネリック引数は `RunStore` / `ProcessController` の2つだけ)。

#### サマリーの集計と表示

タスクファイルに書き込んだ経路がサマリーのいずれかを必ず埋めることを、全経路で追って確認した
(起動 → `launched`、取り込み → `confirmed_running`、記録した失敗 → `errors` + 上限超過なら
`frozen`、遷移前提の破れ・保存失敗 → `errors`)。`frozen` は遷移を呼んだ側が渡す `Freeze` で決まり、
保存に失敗した凍結は数えない(`tick_launch.rs::凍結を伴う遷移を保存できなければ…`)。
`errors` を構造化した `TickIssue` にして文言を `cli::render` が組み立てるのは ADR-073 のとおりで、
永続化される失敗要因(展開失敗・spawn 未観測)だけがユースケース側で文になっている。

#### 1タスクの失敗での続行・冪等性

`process` は1件ごとに完結し、どの失敗も `errors` に積んで次のエントリへ進む
(`tick_scan.rs::あるタスクの処理が失敗しても残りのタスクは続行する`)。同じ永続状態から
同じ判断を再導出することは、連続 tick を回すテスト(状態が変化しない群・破損 run ファイルの継続・
worktree 作成直後のクラッシュ)で主張されている。

#### スコープ外の不実装(AC-13/14 の境界)

notify・手続きB・D・E・`advance`・`attempt_exists` はいずれも呼び出しもスタブも無い。
`TickSummary` に残る未使用フィールド(`transitioned` / `skipped_back` / `notified` / `archived` /
`gc_*`)は spec の出力DTO をそのまま保つという ADR-065 / ADR-086 の判断によるもので、
表示側も含めてユニットテストで固定されている。

#### 実効的なテスト(AC-17)

AC-17 が挙げる9項目(ロック機構の異常・`list_active` の Io・worktree 作成失敗と上限超過・
`prepare_attempt` 失敗・`spawn_wrapper` の同期エラー・`RunFileError` の各種・マーカー書き込み失敗・
猶予境界 30/31/巻き戻り・`save` 失敗)はすべてダブルに対するユースケーステストで消化されている。
ダブルは台本を使い切ると `panic!` するため、余分な呼び出しが素通りしない。
`cargo test --test tick_scan --test tick_launch --test tick_confirm_spawn --test run_wrapper` を
実行して 12 + 19 + 21 + 9 = 61 件すべてが通ることを確認した。

#### クロス tick の受け入れ(AC-15)

`cli_tick.rs` はラッパーの非同期完了を `wait_until` で待ち合わせ、条件を**これから観測する
成果物そのもの**(`exit` を読むなら `exit`、pid を見るなら `pid`、エージェントの滞留はログの合図)に
立てている。F2(起動 → 次 tick の running 取込)・F4(同一 worktree の内容が引き継がれる)・
F6(base の先端からのブランチ作成)がそれぞれ独立したテストとして存在する。

### カバレッジ

- 確認(31): `.thread/2/adr.md`, `.thread/2/plan.md`,
  `crates/pulsen/src/application/mod.rs`, `crates/pulsen/src/application/run_wrapper.rs`,
  `crates/pulsen/src/application/tick/mod.rs`, `crates/pulsen/src/application/tick/launch.rs`,
  `crates/pulsen/src/application/tick/confirm_spawn.rs`,
  `crates/pulsen/src/cli/mod.rs`, `crates/pulsen/src/cli/args.rs`, `crates/pulsen/src/cli/tick.rs`,
  `crates/pulsen/src/cli/wrapper.rs`, `crates/pulsen/src/cli/render.rs`, `crates/pulsen/src/cli/wire.rs`,
  `crates/pulsen/src/adapter/process.rs`,
  `crates/pulsen-domain/src/execution/launching.rs`, `crates/pulsen-domain/src/execution/port.rs`,
  `crates/pulsen-domain/src/task/planner.rs`, `crates/pulsen-domain/src/task/task.rs`,
  `crates/pulsen-domain/src/task/transition.rs`,
  `crates/pulsen-conformance/src/doubles/process.rs`, `crates/pulsen-conformance/src/doubles/run_store.rs`,
  `crates/pulsen-conformance/src/doubles/task_repository.rs`,
  `crates/pulsen/tests/tick_fixture/mod.rs`, `crates/pulsen/tests/tick_scan.rs`,
  `crates/pulsen/tests/tick_launch.rs`, `crates/pulsen/tests/tick_confirm_spawn.rs`,
  `crates/pulsen/tests/run_wrapper.rs`, `crates/pulsen/tests/cli_tick.rs`,
  `crates/pulsen/tests/cli_tick_missing_cwd.rs`, `crates/pulsen/tests/cli_wrapper.rs`,
  `crates/pulsen/tests/cli_usage.rs`
- スキップ(70):
  - `.adr/027-port-conformance-suite-and-harness-hooks.md` — 適合ハーネスの規約。アダプター / テスト観点
  - `.thread/2/progress.md`, `.thread/2/steps.md`, `.thread/2/testing.md` — 進捗・手順の記録。契約は `plan.md`
  - `.thread/2/review/review-001-adapter.md`, `.thread/2/review/review-001-architecture.md`,
    `.thread/2/review/review-001-domain.md`, `.thread/2/review/review-001-test.md`,
    `.thread/2/review/review-001-usecase.md`, `.thread/2/review/review-001.md`,
    `.thread/2/review/review-002-adapter.md`, `.thread/2/review/review-002-architecture.md`,
    `.thread/2/review/review-002-domain.md`, `.thread/2/review/review-002-test.md`,
    `.thread/2/review/review-002-usecase.md`,
    `.thread/2/review/review-003-adapter.md`, `.thread/2/review/review-003-architecture.md`,
    `.thread/2/review/review-003-domain.md`, `.thread/2/review/review-003-test.md`,
    `.thread/2/review/review-003-usecase.md`, `.thread/2/review/review-003.md`,
    `.thread/2/review/review-004-adapter.md`, `.thread/2/review/review-004-architecture.md`,
    `.thread/2/review/review-004-domain.md`, `.thread/2/review/review-004-test.md`,
    `.thread/2/review/review-004-usecase.md`, `.thread/2/review/review-004.md`,
    `.thread/2/review/review-005-adapter.md`, `.thread/2/review/review-005-architecture.md`,
    `.thread/2/review/review-005-domain.md`, `.thread/2/review/review-005-test.md`,
    `.thread/2/review/review-005-usecase.md`, `.thread/2/review/review-005.md`,
    `.thread/2/review/review-006-adapter.md`, `.thread/2/review/review-006-architecture.md`,
    `.thread/2/review/review-006-domain.md`, `.thread/2/review/review-006-test.md`,
    `.thread/2/review/review-006-usecase.md`, `.thread/2/review/review-006.md`,
    `.thread/2/review/triage.md` — ゼロベースで見るため既存レビューは読まない(指示)
  - `crates/pulsen-conformance/HOOKS.md` — 適合スイートの対応表。テスト観点
  - `crates/pulsen-conformance/src/doubles/clock.rs` — 差分は `SettableClock` の追加のみ。猶予境界テストの前提として挙動だけ確認
  - `crates/pulsen-conformance/src/doubles/mod.rs` — 再エクスポート
  - `crates/pulsen-conformance/src/doubles/tests.rs` — ダブル自身のテスト。テスト観点
  - `crates/pulsen-conformance/src/doubles/worktree.rs` — 台本の枯渇でパニックすることのみ確認。詳細はテスト観点
  - `crates/pulsen-conformance/src/lib.rs` — モジュール構成
  - `crates/pulsen-conformance/src/process_controller.rs`, `crates/pulsen-conformance/src/run_store.rs`,
    `crates/pulsen-conformance/src/worktree_manager.rs` — ポート適合スイート。アダプター / テスト観点
  - `crates/pulsen-domain/src/definition/agent.rs`, `crates/pulsen-domain/src/definition/template.rs` — 展開の内部。ユースケースからは呼び出し順のみ確認
  - `crates/pulsen-domain/src/execution/mod.rs`, `crates/pulsen-domain/src/task/mod.rs` — 再エクスポート
  - `crates/pulsen-domain/src/execution/value.rs`, `crates/pulsen-domain/src/task/attempt.rs`,
    `crates/pulsen-domain/src/task/counters.rs`, `crates/pulsen-domain/src/task/failure.rs`,
    `crates/pulsen-domain/src/task/path.rs` — ドメイン値。ドメイン観点
  - `crates/pulsen/examples/agent_probe.rs`, `crates/pulsen/examples/spawn_probe.rs` — テスト用プログラム。テスト観点
  - `crates/pulsen/src/adapter/mod.rs` — モジュール構成
  - `crates/pulsen/src/adapter/run_store.rs`, `crates/pulsen/src/adapter/worktree.rs` — アダプター実装。アダプター観点
  - `crates/pulsen/src/cli/add.rs` — 差分は `workflow_store` / `id_generator` の遅延解決への追従のみ(#1 のユースケース)
  - `crates/pulsen/tests/common/git.rs` — 受け入れテストの git ヘルパー。テスト観点
  - `crates/pulsen/tests/common/mod.rs` — 待ち合わせ(`wait_until`)と `wrapper` 起動の組み立てだけ確認。全体はテスト観点
  - `crates/pulsen/tests/conformance_process_controller.rs`, `crates/pulsen/tests/conformance_run_store.rs`,
    `crates/pulsen/tests/conformance_worktree.rs` — 適合スイートの適用。アダプター / テスト観点
  - `crates/pulsen/tests/register_task.rs` — #1 のユースケーステスト。本スライスの差分は結線追従
