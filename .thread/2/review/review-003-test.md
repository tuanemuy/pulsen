# レビュー 003 — Test

対象: PR #11(`issue/2/tick-agent-run-launch` → `main`)/ 契約: `.thread/2/plan.md`

検証: `cargo test --workspace` は全緑（この環境でのスキップは `tc_port_clock_005` の1件のみで、本スライスの追加行はすべて実行された）。
ミューテーションは使い捨ての `git worktree`（`scratchpad/mut`）で行い、本ツリーには一切変更を残していない。

- 猶予境界 `>` → `>=`: `execution::launching::tests` 2件 + `tick_confirm_spawn` 3件が落ちる
- `RunWrapper` の starttime / pid 書き込み順の入れ替え: `run_wrapper` 3件が落ちる
- `FsRunStore::write` をアトミック置換から分割書きに置換: `tc_port_run_store_016` / `017` が落ちる

いずれもアサーションが実装に噛んでいることを確認した。

## Test

### Blockers

なし。

Issue #2 のチェックリストの TC-* 行を ID 単位で突き合わせた結果、テストの無い行は無い。特に確認した対応は次のとおり。

- `TC-exec-tick-001/002/004/006/007/012/013/014/015/016/018/019` → `tests/tick_scan.rs`、`017/027` と `015` の受け入れ側 → `tests/cli_tick.rs`
- `TC-exec-tick-028〜051/053/054` → `tests/tick_launch.rs`、`052`（ブランチのみ残存）→ `tests/cli_tick.rs:247` と `conformance_worktree` の TC-013、`055`（config 修正の即時反映）→ `tests/cli_tick.rs:295`
- `TC-exec-tick-068〜086` → `tests/tick_confirm_spawn.rs`（境界 30 / 31 / 巻き戻りは `launching(...)` + `SettableClock` で決定的）
- `TC-exec-run-wrapper-001〜006/010〜012/018〜022` → `tests/run_wrapper.rs`、`007/009/013/015/017/019` → `tests/cli_wrapper.rs`、`008` → `tests/cli_tick.rs:167`、`024` → `tests/tick_confirm_spawn.rs:230`、`025` → `TC-port-run-store-016`
- `TC-port-run-store-001〜021` / `TC-port-process-controller-001〜005・017〜027` / `TC-port-worktree-manager-010〜016` はすべて1行1ケース関数で存在し、この環境で全件が `Ran` になる

適合スイートのフックは「破損・状況の意味」だけを受け取る形が保たれており、常に真を返す実装が通る緩さも見当たらない。`TC-port-run-store-001` は `prepare_attempt` の前後で `attempt_dir_present` が反転することまで、`TC-port-process-controller-001` は `run_dir_is_empty` が起動の前後で反転することまで主張している。`TC-port-worktree-manager-015` はフィクスチャ側でも「占有 worktree の登録が `ws.branch` 以外を指すこと」を先に落として空虚な成立を防いでいる。スキップは適合スイート・受け入れテストとも `SkipBudget` を経由し、宣言外のスキップはそのケースの失敗になるため、黙って落ちる行は無い。

### Warnings

- **[W-001]** `TC-exec-run-wrapper-014`（実行不能なコマンド → exit 126）と `016`（ログを開けない → エージェント未起動・126）の裏付けが、区分 C の適合行 `TC-port-process-controller-023` / `025` にしか無い / 場所: `crates/pulsen/tests/cli_wrapper.rs`（当該ケースが無い）, `crates/pulsen/tests/conformance_process_controller.rs:304-325` / 理由: 023 / 025 は `permission_restrictions_effective()` が偽の環境（root 実行・権限を持たないファイルシステム）でスキップされる。その環境では 014 / 016 を支えるものが `run_wrapper.rs` の「終了結果をそのまま書く」だけになり、「126 が返る」側の主張が消える。ところが plan.md の「実行環境がスキップにした行」表には `TC-port-process-controller-023 / 025` しか載っておらず、014 / 016 は無条件にチェックが付く扱いになっている（AC-18 の「スキップで終わった行にはチェックを付けない」が働かない） / 提案: `cli_wrapper.rs` に 014 / 016 に対応するケースを置き、前提を作れない環境では `common::skipped("tc_exec_run_wrapper_014", ...)` で `SkipBudget` に載せる（`tc_exec_tick_015` と同じ扱い）。テストを増やさないなら、plan.md のスキップ表に「023 / 025 がスキップされた環境では `TC-exec-run-wrapper-014` / `016` にもチェックを付けない」を書き足し、ステップ19 の Issue コメントで連動させる。

- **[W-002]** 受け入れテストの前提が実時間 5 秒に依存し、崩れたときスキップではなく失敗として現れる / 場所: `crates/pulsen/tests/cli_tick.rs:28`（`const SLEEP: [&str; 2] = ["sleep", "5000"]`）と `crates/pulsen/tests/cli_tick.rs:180`（`assert!(!run_dir.join("exit").is_file(), "2回目の tick はラッパーの生存中に走る")`）/ 理由: このテストが主張したいのは「ラッパーがロック FD を継承していない」ことで、そのためにラッパーが生きている間に2回目の `pulsen tick` を完走させる必要がある。生存の窓は `sleep 5000` の実時間だけで決まり、`pid` の出現から2回目の `tick` プロセスの起動・git 操作・保存までが 5 秒を超えると前提が崩れて `assert!` が落ちる。同じファイルの待ち合わせは `WAIT_TIMEOUT = 30秒`（`tests/common/mod.rs:78`）を見込んでおり、「負荷時にはファイル1つの出現に 30 秒かかりうる」と認めている一方で、tick 1回の完走にはその1/6しか見ていない。負荷の高い CI で再現性の低い赤を生む形になっている / 提案: 生存の窓を実時間から外す。`examples/agent_probe` に「指定パスのファイルが現れるまで待つ」モードを足し、テストが2回目の tick を終えてから解放ファイルを置く形にすれば、待ち時間を伸ばさずに前提が決定的になる。実時間のまま直すなら、`SLEEP` を `WAIT_TIMEOUT` と同じ尺度（30秒以上）に引き上げ、末尾の `wait_for_exit` の期限もそれに合わせる。

- **[W-003]** `HOOKS.md` が「`.adr/027` のフック表と同じものを指す」と宣言しているが、この PR で両者が食い違った / 場所: `crates/pulsen-conformance/HOOKS.md:306`（「この一覧と `.adr/027-port-conformance-suite-and-harness-hooks.md` のフック表は同じものを指す。フックを足すときは両方を更新する。」）/ 理由: 本スライスで `RunStoreHarness` / `ProcessControllerHarness` の2ハーネスと `WorktreeManagerHarness` の8フック（`unused_workspace` 〜 `branch_tip`）が増え、HOOKS.md の総行数も 125 → 169 になったが、`.adr/027` 側は 125 行・旧ポート一覧のまま更新されていない（`.adr/` はこの PR の変更ファイルに1件も含まれない）。適合スイートの網羅性を「対応表で構造的に担保する」という ADR-027 の狙いそのものが、正本が2つに割れた時点で効かなくなる / 提案: `.adr/027` のフック表とポート一覧・行数を現状に合わせて更新する。ADR を後から書き換えたくないなら、HOOKS.md 側の「両方を更新する」という文を「正本は HOOKS.md で、ADR-027 は決定時点の記録」と改め、二重管理をやめる。

### カバレッジ

確認（44件）: `.thread/2/plan.md`, `crates/pulsen-conformance/HOOKS.md`, `crates/pulsen-conformance/src/lib.rs`, `crates/pulsen-conformance/src/run_store.rs`, `crates/pulsen-conformance/src/process_controller.rs`, `crates/pulsen-conformance/src/worktree_manager.rs`, `crates/pulsen-conformance/src/doubles/clock.rs`, `crates/pulsen-conformance/src/doubles/mod.rs`, `crates/pulsen-conformance/src/doubles/process.rs`, `crates/pulsen-conformance/src/doubles/run_store.rs`, `crates/pulsen-conformance/src/doubles/task_repository.rs`, `crates/pulsen-conformance/src/doubles/tests.rs`, `crates/pulsen-conformance/src/doubles/worktree.rs`, `crates/pulsen-domain/src/execution/launching.rs`, `crates/pulsen-domain/src/execution/value.rs`, `crates/pulsen-domain/src/task/path.rs`, `crates/pulsen-domain/src/task/planner.rs`, `crates/pulsen-domain/src/task/task.rs`, `crates/pulsen-domain/src/task/counters.rs`, `crates/pulsen-domain/src/task/failure.rs`, `crates/pulsen-domain/src/task/attempt.rs`, `crates/pulsen-domain/src/definition/template.rs`, `crates/pulsen-domain/src/definition/agent.rs`, `crates/pulsen/examples/agent_probe.rs`, `crates/pulsen/examples/spawn_probe.rs`, `crates/pulsen/src/adapter/process.rs`, `crates/pulsen/src/adapter/run_store.rs`, `crates/pulsen/src/adapter/worktree.rs`, `crates/pulsen/src/application/run_wrapper.rs`, `crates/pulsen/src/cli/render.rs`, `crates/pulsen/tests/cli_tick.rs`, `crates/pulsen/tests/cli_usage.rs`, `crates/pulsen/tests/cli_wrapper.rs`, `crates/pulsen/tests/common/git.rs`, `crates/pulsen/tests/common/mod.rs`, `crates/pulsen/tests/conformance_process_controller.rs`, `crates/pulsen/tests/conformance_run_store.rs`, `crates/pulsen/tests/conformance_worktree.rs`, `crates/pulsen/tests/register_task.rs`, `crates/pulsen/tests/run_wrapper.rs`, `crates/pulsen/tests/tick_confirm_spawn.rs`, `crates/pulsen/tests/tick_fixture/mod.rs`, `crates/pulsen/tests/tick_launch.rs`, `crates/pulsen/tests/tick_scan.rs`

スキップ（30件）:

- `.thread/2/review/review-001.md`, `.thread/2/review/review-001-adapter.md`, `.thread/2/review/review-001-architecture.md`, `.thread/2/review/review-001-domain.md`, `.thread/2/review/review-001-test.md`, `.thread/2/review/review-001-usecase.md`, `.thread/2/review/review-002-adapter.md`, `.thread/2/review/review-002-architecture.md`, `.thread/2/review/review-002-domain.md`, `.thread/2/review/review-002-test.md`, `.thread/2/review/review-002-usecase.md`, `.thread/2/review/triage.md` — 過去ラウンドの指摘。ゼロベースでレビューする指示により読んでいない
- `.thread/2/adr.md`, `.thread/2/progress.md`, `.thread/2/steps.md`, `.thread/2/testing.md` — 計画側の記録。契約は `plan.md` に取り、テストの実体は `HOOKS.md` と各テストファイルで突き合わせた
- `crates/pulsen-domain/src/execution/mod.rs`, `crates/pulsen-domain/src/execution/port.rs`, `crates/pulsen-domain/src/task/mod.rs`, `crates/pulsen-domain/src/task/transition.rs` — 型・ポート宣言と再公開のみでテストを持たない。ポートの形は適合スイートのケース関数が使う側から確認した
- `crates/pulsen/src/adapter/mod.rs`, `crates/pulsen/src/application/mod.rs`, `crates/pulsen/src/cli/args.rs`, `crates/pulsen/src/cli/mod.rs`, `crates/pulsen/src/cli/tick.rs`, `crates/pulsen/src/cli/wire.rs`, `crates/pulsen/src/cli/wrapper.rs` — 結線と CLI のパーサ。振る舞いは `cli_tick.rs` / `cli_wrapper.rs` / `cli_usage.rs` の受け入れテストで判断した
- `crates/pulsen/src/application/tick/mod.rs`, `crates/pulsen/src/application/tick/launch.rs`, `crates/pulsen/src/application/tick/confirm_spawn.rs` — ユースケース実装。振る舞いは `tick_scan.rs` / `tick_launch.rs` / `tick_confirm_spawn.rs` の網羅性とミューテーションで判断した
