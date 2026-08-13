# レビュー 008 — Architecture / CLI

## Architecture / CLI

### Blockers

なし。

依存方向・レイヤーの責務配置・合成ルートの設計・未実装メソッドの不在・終了コード・`wrapper` の引数往復・出力と縮退状態の共通規則・標準出力/標準エラーの使い分けについて、契約(`.thread/2/plan.md` の AC-1 / AC-11 / AC-12 / AC-18)と `spec/pages/index.md`・`.adr/` に照らして修正を要する破れは見つからなかった。

主な確認結果:

- 依存方向: `pulsen-domain` の `[dependencies]` は空、`unsafe_code = "forbid"` は維持。`crates/pulsen/src/application/` から `crate::adapter` / `crate::cli` への参照は0件。`pulsen-conformance` の依存は `pulsen-domain` のみ(適用側の `crates/pulsen/tests/` だけが `pulsen` を引く)。
- AC-1 のターゲット述語つき `cfg` の隔離: `crates/pulsen-domain/src/` は0件、`crates/pulsen/src/` は `util/atomic.rs` / `adapter/process.rs` / `adapter/task_repository.rs`(`#[cfg(all(test, unix))]`)の3ファイルのみ。`cargo fmt --check` / `cargo clippy --all-targets -- -D warnings` / `cargo test` はいずれも成功(この環境で実行)。
- 未実装メソッドの宣言・スタブ: `execution/port.rs` は `RunStore` 9・`ProcessController` 3・`WorktreeManager` 4(既存3 + `create`)で、AC-6 が「宣言しない」と定めた8メソッド(`attempt_exists` / `list_runs` / `delete_attempt` / `remove_task_dir_if_empty` / `starttime_of` / `kill` / `try_kill_remnants` / `remove`)はいずれも存在しない。`todo!` / `unimplemented!` / `#[allow(dead_code)]` も0件。
- スコープ外の配線の不在: `Branch` の未配線アーム(`Cleanup` / `Observe` / `Advance` / `Notify`)は空で引き取り先の why だけを持ち、`tests/tick_scan.rs::配線していない分岐のタスクにはエージェントを起動せず書き込みもしない` が否定的な主張として固定している(ADR-065 の Consequences どおり)。
- 合成ルート: `compose()` に残るのはホームの解決と config の読み込みだけで、`current_exe` / `current_dir` / 乱数は `wire::process_controller()` / `Runtime::workflow_store()` / `wire::id_generator()` に切り出されている(ADR-068 / ADR-091)。ラッパーは `WrapperRuntime` として別型で、ホームも config も読まない(ADR-070)。`RunDirPath::state_root` は `derive` との一致で受理する(ADR-079)。
- 終了コード: `add` 成功0 / 失敗非0、`tick` はロック競合0(`TickOutcome::Skipped`)・`LockError::Failed` と `list_active` の Io が非0、`wrapper` は `Ran` / `Suppressed` が0・`Silent` と起動引数の不正が非0(ADR-081)。pages の exit code 規約と一致する。
- 標準出力 / 標準エラー: サマリー・スキップ報告は標準出力、失敗は標準エラー。`wrapper` は標準出力に何も書かない(`cli_wrapper.rs::起動引数どおりのエージェントが実行され終了結果がrunディレクトリに現れる` が `stdout.is_empty()` を主張)。
- `wrapper` の引数の往復: argv の定数は `adapter/process.rs` の `pub const` に1箇所化され、`tests/common/mod.rs::wrapper()` が同じ定数から argv を組んで実バイナリを通す。`trailing_var_arg` + `allow_hyphen_values` により `--model` / 空文字列 / メタ文字がリテラルのまま渡ることを `シェルのメタ文字や空文字列を含むトークンはリテラルのまま渡る` が主張する。
- 縮退状態の共通規則: config 不在・破損は `tick` が非0で状態を変えず、`wrapper` は影響を受けない(規則1)。`Corrupt` / `SnapshotUnreadable` は報告のみで書き込まない(規則2)。1タスクの失敗で全体を落とさない(規則3)。`ADR-084` の「書き込んだ経路は必ずサマリーのいずれかを埋める」が `commit` / `errors` の設計として成立している。
- ADR 参照の解決: `crates/` と `.thread/2/` に現れる `ADR-NNN` はすべて `.adr/` または `.thread/2/adr.md` に実在する(未解決0件)。
- 呼び出しの無い `pub`: `Task::is_wait` / `is_cleanup` は本スライスに呼び出しが無いが、`is_agent_run` の doc に「spec が一組で定める読み取り口」という why があり AC-4 が要求する行でもある(ADR-061 の要件を満たす)。`Runtime::state_root()` / `worktree_root()` の復活にも why が添えられている。

### Warnings

- **[W-001]** サマリー表示が本スライスで値の入らないフィールドまで実装されており、plan.md の部分消化表と食い違う。
  - 場所: `crates/pulsen/src/cli/render.rs`(`tick_summary` の `push_ids(transitioned / skipped_back / notified / archived)` と `push_attempts(gc_deleted / gc_errors)`、テスト `すべての項目が埋まったサマリーは決まった順で並ぶ`)、`crates/pulsen-domain/src/task/path.rs`(`RunDirPath::attempt_dir_name` の `pub` 化)。
  - 理由: plan.md の「本スライスで部分消化になる14行」は `PAGE-tick-004` の消化範囲を `launched` / `frozen` / `errors` の表示とし、`transitioned` / `skipped_back` / `notified` / `archived` / `gc_deleted` の表示を #3 / #6 の引き取り先と明記している。実装はそれらの表示(および表にない `gc_errors`)を先に持っており、`RunDirPath::attempt_dir_name` が `pub` である唯一の理由も、本スライスでは決して値の入らない gc 表示から呼ばれることにある(ドメイン内では `derive` が使うだけなので、この呼び出しが無ければ private でよい)。ADR-065 は「サマリー DTO は spec の全フィールドを持たせる」と決めているが、**表示まで先に実装する**ことは決めていない。ドキュメントと実装のどちらが正なのかがこの PR だけからは読めず、#3 / #6 の担当者が引き取り先の表を信じて同じ表示を二重に実装しうる。
  - 提案: どちらかに寄せる。(a) 表示を本スライスで値の入る4フィールドに絞り、`attempt_dir_name` を private に戻す(引き取り先の表がそのまま正になる)。(b) 先行実装を意図として残すなら、plan.md の `PAGE-tick-004` 行と progress.md の「spec 追従の提起」に「表示は全フィールド分を先に置き、#3 / #6 はアームを埋めるだけでよい」と根拠つきで記す。いずれの場合も `gc_errors` が表にない点をあわせて解消する。

- **[W-002]** ADR-077 の決定文と `create` の復旧分岐の条件が一致していない。
  - 場所: `.thread/2/adr.md` ADR-077 の Decision(「`prunable` が付いた自タスクの登録…は `git worktree add -f` で張り直す」)と `crates/pulsen/src/adapter/worktree.rs::create`(`if present && !entry.prunable { return Ok(()) }` — 実体の有無を `path.try_exists()` で直接観測し、`prunable` 注記が無くても実体が無ければ張り直す)。
  - 理由: 実装は ADR の条件より広い(実体の直接観測を条件に加えている)。コード側にはこの broadening の why が書かれているが、ADR の Decision と Consequences は `prunable` 注記だけを分岐の鍵として記述したままになっている。ADR-077 は #6(手続きB・`remove`)が同じ同定規則を引き継ぐ根拠であり、steps.md のステップ2の記述(「`prunable` が付いていなければ達成済み」)も同じ古い条件のままなので、昇格(steps.md ステップ19)時に決定記録と実装が食い違ったまま `.adr/` に入る。
  - 提案: ADR-077 の Decision に「達成済みの条件は『鍵一致 + 自ブランチ + **実体が存在** + `prunable` でない』であり、`prunable` 注記は git のバージョンで出ないことがあるため実体の観測を鍵にする」旨を1行足し、steps.md の該当箇所も揃える。

### カバレッジ

**確認(53件)**

`.adr/027-port-conformance-suite-and-harness-hooks.md`, `.thread/2/adr.md`, `.thread/2/plan.md`, `.thread/2/progress.md`, `.thread/2/steps.md`, `.thread/2/testing.md`, `crates/pulsen-conformance/HOOKS.md`, `crates/pulsen-conformance/src/doubles/mod.rs`, `crates/pulsen-conformance/src/lib.rs`, `crates/pulsen-conformance/src/process_controller.rs`, `crates/pulsen-conformance/src/run_store.rs`, `crates/pulsen-conformance/src/worktree_manager.rs`, `crates/pulsen-domain/src/definition/agent.rs`, `crates/pulsen-domain/src/definition/template.rs`, `crates/pulsen-domain/src/execution/mod.rs`, `crates/pulsen-domain/src/execution/port.rs`, `crates/pulsen-domain/src/execution/value.rs`, `crates/pulsen-domain/src/task/mod.rs`, `crates/pulsen-domain/src/task/path.rs`, `crates/pulsen-domain/src/task/planner.rs`, `crates/pulsen-domain/src/task/task.rs`, `crates/pulsen-domain/src/task/transition.rs`, `crates/pulsen/examples/agent_probe.rs`, `crates/pulsen/examples/spawn_probe.rs`, `crates/pulsen/src/adapter/mod.rs`, `crates/pulsen/src/adapter/process.rs`, `crates/pulsen/src/adapter/run_store.rs`, `crates/pulsen/src/adapter/worktree.rs`, `crates/pulsen/src/application/mod.rs`, `crates/pulsen/src/application/run_wrapper.rs`, `crates/pulsen/src/application/tick/confirm_spawn.rs`, `crates/pulsen/src/application/tick/launch.rs`, `crates/pulsen/src/application/tick/mod.rs`, `crates/pulsen/src/cli/add.rs`, `crates/pulsen/src/cli/args.rs`, `crates/pulsen/src/cli/mod.rs`, `crates/pulsen/src/cli/render.rs`, `crates/pulsen/src/cli/tick.rs`, `crates/pulsen/src/cli/wire.rs`, `crates/pulsen/src/cli/wrapper.rs`, `crates/pulsen/tests/cli_tick.rs`, `crates/pulsen/tests/cli_tick_missing_cwd.rs`, `crates/pulsen/tests/cli_usage.rs`, `crates/pulsen/tests/cli_wrapper.rs`, `crates/pulsen/tests/common/git.rs`, `crates/pulsen/tests/common/mod.rs`, `crates/pulsen/tests/conformance_process_controller.rs`, `crates/pulsen/tests/conformance_run_store.rs`, `crates/pulsen/tests/run_wrapper.rs`, `crates/pulsen/tests/tick_confirm_spawn.rs`, `crates/pulsen/tests/tick_fixture/mod.rs`, `crates/pulsen/tests/tick_launch.rs`, `crates/pulsen/tests/tick_scan.rs`

確認の深さの注記:

- `crates/pulsen-conformance/src/{process_controller,run_store,worktree_manager}.rs` は本観点として「AC-7 / AC-8 / AC-9 の件数とスイートの分割(ADR-075)」まで(RunStore 21件・ProcessController 16件・WorktreeManager 台帳16行 + 追加1件)。ケースの主張内容は Test / Adapter 観点。
- `crates/pulsen-conformance/src/lib.rs` は依存方向と `attempt_dir_present` フックの位置づけ(ADR-076)まで。
- `crates/pulsen/tests/{run_wrapper,tick_scan,tick_launch,tick_confirm_spawn,tick_fixture/mod}.rs` は ADR-085 の境界(実ファイルシステム・実プロセスを使わないこと)と分岐の網羅の並びまで。`tempfile` / `std::fs` / `std::process` の参照が0件であることを確認した。
- `.thread/2/{plan,steps,testing,progress}.md` と `.thread/2/adr.md` は実装との対応(AC-1 / AC-11 / AC-12 / AC-18・ADR の決定と実装の一致)の観点で読んだ。AC-18(チェックリストの記帳)は steps.md ステップ19 の成果物であり、progress.md が「未着手(Phase 4 以降)」と明記しているため、この PR の差分だけでは検証できない — 記帳内容そのものは本レビューの対象外とした。

**スキップ(54件)**

- `.thread/2/review/review-001-adapter.md`, `review-001-architecture.md`, `review-001-domain.md`, `review-001-test.md`, `review-001-usecase.md`, `review-001.md`, `review-002-adapter.md`, `review-002-architecture.md`, `review-002-domain.md`, `review-002-test.md`, `review-002-usecase.md`, `review-003-adapter.md`, `review-003-architecture.md`, `review-003-domain.md`, `review-003-test.md`, `review-003-usecase.md`, `review-003.md`, `review-004-adapter.md`, `review-004-architecture.md`, `review-004-domain.md`, `review-004-test.md`, `review-004-usecase.md`, `review-004.md`, `review-005-adapter.md`, `review-005-architecture.md`, `review-005-domain.md`, `review-005-test.md`, `review-005-usecase.md`, `review-005.md`, `review-006-adapter.md`, `review-006-architecture.md`, `review-006-domain.md`, `review-006-test.md`, `review-006-usecase.md`, `review-006.md`, `review-007-adapter.md`, `review-007-architecture.md`, `review-007-domain.md`, `review-007-test.md`, `review-007-usecase.md`, `review-007.md`, `.thread/2/review/triage.md`(42件)— ゼロベースでレビューする指示により、`.thread/2/review/` 配下の既存ファイルは読まない。
- `crates/pulsen-conformance/src/doubles/clock.rs`, `doubles/process.rs`, `doubles/run_store.rs`, `doubles/task_repository.rs`, `doubles/tests.rs`, `doubles/worktree.rs`(6件)— テストダブルの内部実装。層の配置(`doubles/mod.rs` の公開とドメインのみへの依存)は確認済みで、スクリプトの網羅と呼び出し記録の妥当性は Test / Usecase 観点。
- `crates/pulsen-domain/src/execution/launching.rs`, `task/attempt.rs`, `task/counters.rs`, `task/failure.rs`(4件)— 分類ロジック・採番・カウンタ規則はドメイン観点(AC-3 / AC-5)。層の配置と公開範囲は `execution/mod.rs` / `task/mod.rs` の再公開で確認した。
- `crates/pulsen/tests/conformance_worktree.rs`, `crates/pulsen/tests/register_task.rs`(2件)— 既存の適用ファイルへのフック追加と既存テストの追随。Adapter / Test 観点。
