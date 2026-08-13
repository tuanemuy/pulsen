# レビュー 007 — Test

対象: PR #11 / `issue/2/tick-agent-run-launch`(ベース `main`)

`cargo test --workspace` を実行して全ターゲット緑（合計 320 件超、失敗 0）であること、`--nocapture` の `SKIP` 行が既存の Clock 由来 4 件のみで本スライスの新規スイートには 1 件も出ないこと（= TC-port-run-store-007/017・TC-port-process-controller-023/024/025・tc_exec_run_wrapper_014/016・tc_exec_tick_015 がこの環境で実際に走ったこと）を確認したうえでレビューした。

## Test

### Blockers

なし。

Issue #2 のチェックリストにある TC-* 行（`TC-exec-tick-001〜027 / 028〜055 / 068〜086`、`TC-exec-run-wrapper-001〜027`、`TC-port-run-store-001〜021`、`TC-port-process-controller-001〜005・017〜027`、`TC-port-worktree-manager-010〜016`）を ID 単位で突き合わせた結果、テストの無い行は無かった。plan.md が部分消化と宣言した 14 行・スキップ表の行についても、消化範囲どおりのテストが置かれ、スキップは `SkipBudget` に載っていて黙って通らない。

### Warnings

- **[W-001]** シグナル死の受け入れテストで「エージェントは起動されている」を主張する assert が、その主張を支えていない。
  場所: `crates/pulsen/tests/cli_wrapper.rs::シグナルで死んだエージェントは非ゼロの符号化値としてexitファイルに現れる`（`assert!(launch.run_dir().join("stdout.log").is_file(), "エージェントは起動されている")`）。
  理由: `SystemProcessController::run_agent`（`crates/pulsen/src/adapter/process.rs:160-195`）は cwd 検査 → `File::create(stdout)` / `File::create(stderr)` → `Command::spawn` の順なので、コマンド不在（127）でも起動不能（126）でも `stdout.log` は必ず作られる。この assert が真でもエージェントが起動した証拠にならない。POSIX では直後の `#[cfg(unix)] assert_eq!(code, 128 + 6)` が事実を確定させるが、非 POSIX では残るのが `assert_ne!(code, 0)` だけで、「起動できずに 126/127 になった」でも緑になる。TC-exec-run-wrapper-015 の主張（`exit code を持たない終了`）がプラットフォームによって空洞化する。
  提案: プラットフォーム非依存に「起動後に死んだ」を主張できる形にする。`assert!(code != 126 && code != 127, ...)` を置けば起動不能の符号化値と区別でき、`stdout.log` の存在に依存した誤った説明を外せる（unix 限定の `128 + 6` の主張はそのまま残す）。

- **[W-002]** `HOOKS.md` の「環境で走らなくなりうる行」の表が、`agent_probe` に依存する行を 2 行分多く挙げている。
  場所: `crates/pulsen-conformance/HOOKS.md:41`（`TC-port-process-controller-001 / 002 / 003 / 017〜027` / 判定「ハーネスが `agent_command` を提供するか」）。
  理由: `tc_port_process_controller_022` は `missing_command`、`023` は `non_executable_command` しか使わず（`crates/pulsen-conformance/src/process_controller.rs:152-180`）、`agent_command` を呼ばない。HOOKS.md はフック対応表の正本（ADR-027 をこの PR で「正本は HOOKS.md 一本」に改めている）なので、次のスライスがこの表からスキップ宣言を組むと、実際の依存と食い違った宣言になる。今の実装は probe 不在を許容集合に入れない方針なので実害は出ていない。
  提案: 該当行を `001 / 002 / 003 / 017〜021 / 024〜027` に絞り、022 / 023 は依存するフック（`missing_command` / `non_executable_command`）で別に書く。

- **[W-003]** WorktreeManager 適合ハーネスが「置き場をシンボリックリンク経由にする」前提を、黙って実体パスへ落とす。
  場所: `crates/pulsen/tests/conformance_worktree.rs::worktree_root()`（`symlink_dir` の失敗を `let _ =` で捨て、`if link.is_dir() { link } else { real }` で実体へフォールバック）。
  理由: HOOKS.md:210 は「同定の鍵は物理パスなので（ADR-077）、置き場が実体そのものだと正規化の分岐がどのケースからも実行されない」と、リンク経由であることを `create` 系 8 ケースの前提として明示している。リンクを作れない環境（Windows の非開発者モード等）ではこの前提が消えるが、ケースは全部そのまま「PASS」と表示され、正規化の分岐を一度も通していないことがどこにも現れない。このスイートが持つ「スキップは宣言しないと失敗になる」規律（ADR-055）の外側に、報告されない縮退が 1 つ残っている。
  提案: フォールバックを可視化する。`symlink_dir` が失敗したら `worktree_root()` を `Option` にして `create` 系ケースを `require!` 経由のスキップに落とす（`SkipBudget` の宣言で管理できるようになる）か、少なくとも `SkipBudget::record` と同じ形で標準出力に落ちたことを書き出す。

## カバレッジ

確認（65）:

- `/Users/hikaru/github.com/tuanemuy/pulsen/.adr/027-port-conformance-suite-and-harness-hooks.md`
- `/Users/hikaru/github.com/tuanemuy/pulsen/.thread/2/plan.md`
- `/Users/hikaru/github.com/tuanemuy/pulsen/.thread/2/progress.md`
- `/Users/hikaru/github.com/tuanemuy/pulsen/.thread/2/testing.md`
- `/Users/hikaru/github.com/tuanemuy/pulsen/crates/pulsen-conformance/HOOKS.md`
- `/Users/hikaru/github.com/tuanemuy/pulsen/crates/pulsen-conformance/src/lib.rs`
- `/Users/hikaru/github.com/tuanemuy/pulsen/crates/pulsen-conformance/src/process_controller.rs`
- `/Users/hikaru/github.com/tuanemuy/pulsen/crates/pulsen-conformance/src/run_store.rs`
- `/Users/hikaru/github.com/tuanemuy/pulsen/crates/pulsen-conformance/src/worktree_manager.rs`
- `/Users/hikaru/github.com/tuanemuy/pulsen/crates/pulsen-conformance/src/doubles/clock.rs`
- `/Users/hikaru/github.com/tuanemuy/pulsen/crates/pulsen-conformance/src/doubles/mod.rs`
- `/Users/hikaru/github.com/tuanemuy/pulsen/crates/pulsen-conformance/src/doubles/process.rs`
- `/Users/hikaru/github.com/tuanemuy/pulsen/crates/pulsen-conformance/src/doubles/run_store.rs`
- `/Users/hikaru/github.com/tuanemuy/pulsen/crates/pulsen-conformance/src/doubles/task_repository.rs`
- `/Users/hikaru/github.com/tuanemuy/pulsen/crates/pulsen-conformance/src/doubles/tests.rs`
- `/Users/hikaru/github.com/tuanemuy/pulsen/crates/pulsen-conformance/src/doubles/worktree.rs`
- `/Users/hikaru/github.com/tuanemuy/pulsen/crates/pulsen-domain/src/definition/agent.rs`
- `/Users/hikaru/github.com/tuanemuy/pulsen/crates/pulsen-domain/src/definition/template.rs`
- `/Users/hikaru/github.com/tuanemuy/pulsen/crates/pulsen-domain/src/execution/launching.rs`
- `/Users/hikaru/github.com/tuanemuy/pulsen/crates/pulsen-domain/src/execution/port.rs`
- `/Users/hikaru/github.com/tuanemuy/pulsen/crates/pulsen-domain/src/execution/value.rs`
- `/Users/hikaru/github.com/tuanemuy/pulsen/crates/pulsen-domain/src/task/attempt.rs`
- `/Users/hikaru/github.com/tuanemuy/pulsen/crates/pulsen-domain/src/task/counters.rs`
- `/Users/hikaru/github.com/tuanemuy/pulsen/crates/pulsen-domain/src/task/failure.rs`
- `/Users/hikaru/github.com/tuanemuy/pulsen/crates/pulsen-domain/src/task/path.rs`
- `/Users/hikaru/github.com/tuanemuy/pulsen/crates/pulsen-domain/src/task/planner.rs`
- `/Users/hikaru/github.com/tuanemuy/pulsen/crates/pulsen-domain/src/task/task.rs`
- `/Users/hikaru/github.com/tuanemuy/pulsen/crates/pulsen-domain/src/task/transition.rs`
- `/Users/hikaru/github.com/tuanemuy/pulsen/crates/pulsen/examples/agent_probe.rs`
- `/Users/hikaru/github.com/tuanemuy/pulsen/crates/pulsen/examples/spawn_probe.rs`
- `/Users/hikaru/github.com/tuanemuy/pulsen/crates/pulsen/src/adapter/process.rs`
- `/Users/hikaru/github.com/tuanemuy/pulsen/crates/pulsen/src/adapter/run_store.rs`
- `/Users/hikaru/github.com/tuanemuy/pulsen/crates/pulsen/src/adapter/worktree.rs`
- `/Users/hikaru/github.com/tuanemuy/pulsen/crates/pulsen/src/application/run_wrapper.rs`
- `/Users/hikaru/github.com/tuanemuy/pulsen/crates/pulsen/src/application/tick/confirm_spawn.rs`
- `/Users/hikaru/github.com/tuanemuy/pulsen/crates/pulsen/src/application/tick/launch.rs`
- `/Users/hikaru/github.com/tuanemuy/pulsen/crates/pulsen/src/application/tick/mod.rs`
- `/Users/hikaru/github.com/tuanemuy/pulsen/crates/pulsen/src/cli/mod.rs`
- `/Users/hikaru/github.com/tuanemuy/pulsen/crates/pulsen/src/cli/render.rs`
- `/Users/hikaru/github.com/tuanemuy/pulsen/crates/pulsen/src/cli/tick.rs`
- `/Users/hikaru/github.com/tuanemuy/pulsen/crates/pulsen/tests/cli_tick.rs`
- `/Users/hikaru/github.com/tuanemuy/pulsen/crates/pulsen/tests/cli_tick_missing_cwd.rs`
- `/Users/hikaru/github.com/tuanemuy/pulsen/crates/pulsen/tests/cli_usage.rs`
- `/Users/hikaru/github.com/tuanemuy/pulsen/crates/pulsen/tests/cli_wrapper.rs`
- `/Users/hikaru/github.com/tuanemuy/pulsen/crates/pulsen/tests/common/git.rs`
- `/Users/hikaru/github.com/tuanemuy/pulsen/crates/pulsen/tests/common/mod.rs`
- `/Users/hikaru/github.com/tuanemuy/pulsen/crates/pulsen/tests/conformance_process_controller.rs`
- `/Users/hikaru/github.com/tuanemuy/pulsen/crates/pulsen/tests/conformance_run_store.rs`
- `/Users/hikaru/github.com/tuanemuy/pulsen/crates/pulsen/tests/conformance_worktree.rs`
- `/Users/hikaru/github.com/tuanemuy/pulsen/crates/pulsen/tests/register_task.rs`
- `/Users/hikaru/github.com/tuanemuy/pulsen/crates/pulsen/tests/run_wrapper.rs`
- `/Users/hikaru/github.com/tuanemuy/pulsen/crates/pulsen/tests/tick_confirm_spawn.rs`
- `/Users/hikaru/github.com/tuanemuy/pulsen/crates/pulsen/tests/tick_fixture/mod.rs`
- `/Users/hikaru/github.com/tuanemuy/pulsen/crates/pulsen/tests/tick_launch.rs`
- `/Users/hikaru/github.com/tuanemuy/pulsen/crates/pulsen/tests/tick_scan.rs`

（上記 55 件に加え、下記 10 件も「テストを持たない再エクスポート・結線であること」を確認したうえで確認済みに数える）

- `/Users/hikaru/github.com/tuanemuy/pulsen/crates/pulsen-domain/src/execution/mod.rs`
- `/Users/hikaru/github.com/tuanemuy/pulsen/crates/pulsen-domain/src/task/mod.rs`
- `/Users/hikaru/github.com/tuanemuy/pulsen/crates/pulsen/src/adapter/mod.rs`
- `/Users/hikaru/github.com/tuanemuy/pulsen/crates/pulsen/src/application/mod.rs`
- `/Users/hikaru/github.com/tuanemuy/pulsen/crates/pulsen/src/cli/add.rs`
- `/Users/hikaru/github.com/tuanemuy/pulsen/crates/pulsen/src/cli/args.rs`
- `/Users/hikaru/github.com/tuanemuy/pulsen/crates/pulsen/src/cli/wire.rs`
- `/Users/hikaru/github.com/tuanemuy/pulsen/crates/pulsen/src/cli/wrapper.rs`
- `/Users/hikaru/github.com/tuanemuy/pulsen/.thread/2/adr.md`
- `/Users/hikaru/github.com/tuanemuy/pulsen/.thread/2/steps.md`

スキップ（36）:

- `/Users/hikaru/github.com/tuanemuy/pulsen/.thread/2/review/review-001-adapter.md` — 過去ラウンドのレビュー記録。ゼロベースで見る指示のため読まない
- `/Users/hikaru/github.com/tuanemuy/pulsen/.thread/2/review/review-001-architecture.md` — 同上
- `/Users/hikaru/github.com/tuanemuy/pulsen/.thread/2/review/review-001-domain.md` — 同上
- `/Users/hikaru/github.com/tuanemuy/pulsen/.thread/2/review/review-001-test.md` — 同上
- `/Users/hikaru/github.com/tuanemuy/pulsen/.thread/2/review/review-001-usecase.md` — 同上
- `/Users/hikaru/github.com/tuanemuy/pulsen/.thread/2/review/review-001.md` — 同上
- `/Users/hikaru/github.com/tuanemuy/pulsen/.thread/2/review/review-002-adapter.md` — 同上
- `/Users/hikaru/github.com/tuanemuy/pulsen/.thread/2/review/review-002-architecture.md` — 同上
- `/Users/hikaru/github.com/tuanemuy/pulsen/.thread/2/review/review-002-domain.md` — 同上
- `/Users/hikaru/github.com/tuanemuy/pulsen/.thread/2/review/review-002-test.md` — 同上
- `/Users/hikaru/github.com/tuanemuy/pulsen/.thread/2/review/review-002-usecase.md` — 同上
- `/Users/hikaru/github.com/tuanemuy/pulsen/.thread/2/review/review-003-adapter.md` — 同上
- `/Users/hikaru/github.com/tuanemuy/pulsen/.thread/2/review/review-003-architecture.md` — 同上
- `/Users/hikaru/github.com/tuanemuy/pulsen/.thread/2/review/review-003-domain.md` — 同上
- `/Users/hikaru/github.com/tuanemuy/pulsen/.thread/2/review/review-003-test.md` — 同上
- `/Users/hikaru/github.com/tuanemuy/pulsen/.thread/2/review/review-003-usecase.md` — 同上
- `/Users/hikaru/github.com/tuanemuy/pulsen/.thread/2/review/review-003.md` — 同上
- `/Users/hikaru/github.com/tuanemuy/pulsen/.thread/2/review/review-004-adapter.md` — 同上
- `/Users/hikaru/github.com/tuanemuy/pulsen/.thread/2/review/review-004-architecture.md` — 同上
- `/Users/hikaru/github.com/tuanemuy/pulsen/.thread/2/review/review-004-domain.md` — 同上
- `/Users/hikaru/github.com/tuanemuy/pulsen/.thread/2/review/review-004-test.md` — 同上
- `/Users/hikaru/github.com/tuanemuy/pulsen/.thread/2/review/review-004-usecase.md` — 同上
- `/Users/hikaru/github.com/tuanemuy/pulsen/.thread/2/review/review-004.md` — 同上
- `/Users/hikaru/github.com/tuanemuy/pulsen/.thread/2/review/review-005-adapter.md` — 同上
- `/Users/hikaru/github.com/tuanemuy/pulsen/.thread/2/review/review-005-architecture.md` — 同上
- `/Users/hikaru/github.com/tuanemuy/pulsen/.thread/2/review/review-005-domain.md` — 同上
- `/Users/hikaru/github.com/tuanemuy/pulsen/.thread/2/review/review-005-test.md` — 同上
- `/Users/hikaru/github.com/tuanemuy/pulsen/.thread/2/review/review-005-usecase.md` — 同上
- `/Users/hikaru/github.com/tuanemuy/pulsen/.thread/2/review/review-005.md` — 同上
- `/Users/hikaru/github.com/tuanemuy/pulsen/.thread/2/review/review-006-adapter.md` — 同上
- `/Users/hikaru/github.com/tuanemuy/pulsen/.thread/2/review/review-006-architecture.md` — 同上
- `/Users/hikaru/github.com/tuanemuy/pulsen/.thread/2/review/review-006-domain.md` — 同上
- `/Users/hikaru/github.com/tuanemuy/pulsen/.thread/2/review/review-006-test.md` — 同上
- `/Users/hikaru/github.com/tuanemuy/pulsen/.thread/2/review/review-006-usecase.md` — 同上
- `/Users/hikaru/github.com/tuanemuy/pulsen/.thread/2/review/review-006.md` — 同上
- `/Users/hikaru/github.com/tuanemuy/pulsen/.thread/2/review/triage.md` — 同上

確認 65 + スキップ 36 = 101。

## 参考（指摘ではない）

- `progress.md` が記録している `TC-port-worktree-manager-015` の間欠失敗（原因未特定）は、フィクスチャ側（`workspace_over_other_branch` の `assert_occupied`）とケース側（`create` 直前の内容・ブランチ不在の主張）の二重の前提検査に加え、`assert_create_failed` の `Ok` アームが `ws.path` の観測をパニックに載せる形になっており、再発時に「前提が破れた」のか「同定が外れた」のかを名指しできる。現状これ以上の手当ては不要と判断した。
