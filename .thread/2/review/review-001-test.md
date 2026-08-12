# レビュー結果 — Issue #2 / 観点: Test

対象 PR: #11(ベース `main` / ブランチ `issue/2/tick-agent-run-launch`)
基準: `CLAUDE.md`(テスト方針)、`.thread/2/plan.md`(受け入れ基準・テスト方針・チェックを付ける基準)、
`spec/testcases/execution/{tick,run-wrapper}.md`、`spec/testcases/ports/{run-store,process-controller,worktree-manager}.md`、
`crates/pulsen-conformance/HOOKS.md`、Issue #2 のチェックリスト208行

実行して確認したこと(この環境: macOS / 非 root / TMPDIR は `/var/folders/...`):

- `cargo test` — 全ターゲット緑(失敗0)。
- `cargo test --test conformance_run_store --test conformance_process_controller --test conformance_worktree -- --nocapture` — `SKIP` 行は **0件**。RunStore 21件・ProcessController 16件(13+3)・WorktreeManager 17件(16+追加1)がすべて実際に走っている。
- `pulsen --home <空ホーム> tick`(config 不在 / YAML 破損)を手で実行 — どちらも exit 1・`state/` を作らない。**振る舞いは正しいがテストが無い**(B-001)。

## Test

### Blockers

- **[B-001]** `TC-exec-tick-017`(config.yaml が存在しない・パース不能・読めない → 非0 で終了・状態は変更しない)を検証するテストが1つも無い
  - 場所: `crates/pulsen/tests/cli_tick.rs`(不在)、`crates/pulsen/src/cli/render.rs:531` 以降のユニットテスト(`TickCommandError::Wire` の分岐が無い)
  - 理由: Issue #2 のチェックリスト行(TC-exec-tick-017)であり、plan.md AC-12 が明示的に列挙している。`Tick` ユースケースは `&GlobalConfig` を受け取る形なので config 読み込みの失敗は CLI 層にしか現れず、受け入れテストが唯一の観測点になる。`cli_tick.rs` の全テストは `probe_home()` 経由で必ず有効な config を置くため、この経路は**どのテストも通っていない**。手で確認したとおり実装は正しく非0で終わるが、`wire::compose` の呼び出し順を変える・エラーを握り潰す変更が入っても検知できない。`cli_add_error.rs` の `tc_task_register_task_014` は `add` の経路であって tick の裏付けにならない
  - 提案: `cli_tick.rs` に2ケース追加する。(1) `Home::uninitialized()` のまま `tick` → `assert_rejected()` かつ `home.state_dir()` が作られないこと、(2) `home.write_config("agents: [壊れた\n")` → `assert_rejected()` かつ登録済みタスクファイルが `Untouched::assert_unchanged()` で不変であること。`cli_wrapper.rs` の「グローバル設定が不在でも破損していても動作は変わらない」と対になり、「ラッパーは読まない / tick は読む」の対比も出る

- **[B-002]** `TC-exec-tick-035`(attempt_count > 0 の failed タスクで worktree 作成が成功しても attempt_count・judge_attempt_count はリセットされない)が、どの層でも主張されていない
  - 場所: `crates/pulsen-domain/src/task/task.rs:826`(`ワークスペースは未確定のときだけ確定できる`)、`crates/pulsen/tests/tick_launch.rs:42`(`ワークスペース未確定のタスクは導出したworktreeを確保してから起動する`)
  - 理由: この行の PASS 要件はカウンタの保存そのもの(requirements §6.4・ADR-009 の「途中のツール操作の成功ではリセットしない」)。ドメイン側のテストは `fields()`(= `RetryCounters::initial()` = 0,0,0)のタスクにしか `confirm_workspace` を掛けておらず、ユースケース側の起動テストもカウンタを一切見ていない。**`confirm_workspace` の中で `counters` を初期化する実装に変えても全テストが緑のまま通る**ので、主張が空虚になっている。steps.md ステップ4 は「TC-exec-tick-035 の境界もここで裏付ける」と書いているが、実装が伴っていない
  - 提案: (a) ドメイン: `counters: RetryCounters::rehydrate(2, 1, 3)` かつ `execution: Failed` のタスクに `confirm_workspace` を掛け、`counters()` が丸ごと不変であることを assert する。(b) ユースケース: `tick_launch.rs` に「attempt_count > 0 の failed タスクで worktree 作成が成功する」ケースを足し、保存された2件目(launching 記録後)で `attempt_count` / `judge_attempt_count` が入力値のままであること、かつ起動処理が続行すること(`summary.launched` に載ること)を assert する

### Warnings

- **[W-001]** ロック競合の受け入れテストのスキップ宣言IDが実際のケースとずれている
  - 場所: `crates/pulsen/tests/common/mod.rs:34`(`LOCK_HOLDER_CASES` に `"tc_exec_tick_016"`)、`crates/pulsen/tests/cli_tick.rs:357`(`common::skipped("tc_exec_tick_016", "lock::hold")`)
  - 理由: このテストが確かめるのは「別の操作が排他ロックを保持している → スキップして 0 で終わる」= **TC-exec-tick-015**。TC-exec-tick-016 は `LockError::Failed` で非0 になる行で、`tick_scan.rs:37` のユースケーステストが担当しており、環境要因でスキップされることはない。`SkipBudget` の宣言と `SKIP` 行はそのまま「どのチェックリスト行が走らなかったか」の根拠として Issue のコメントに転記される(plan.md「チェックを付ける基準」)ため、`lock_holder` が無い環境では**走った 016 を未チェックにし、走らなかった 015 にチェックを付ける**という取り違えが起きる
  - 提案: 定数と呼び出しの両方を `tc_exec_tick_015` に直す

- **[W-002]** `TC-port-worktree-manager-010` が「`ws.path` に `ws.branch` の worktree として用意された」ことを観測していない
  - 場所: `crates/pulsen-conformance/src/worktree_manager.rs`(`tc_port_worktree_manager_010_...`)、`crates/pulsen/tests/conformance_worktree.rs`(`fn worktree_present` = `path.is_dir()`)
  - 理由: spec の期待は「`ws.path` に worktree が用意され、その HEAD は base の先端から作成された新ブランチ `ws.branch` を指す」。現状の主張は「パスがディレクトリとして在る」+「ブランチが在る」+「ブランチ先端 = base 先端」までで、**登録の有無も HEAD の指す先も見ていない**。`git branch` して `mkdir` しただけの実装でもこの行は通る(TC-012 / TC-015 が間接的に拾うが、行単体の主張としては緩い)。ハーネス側には既に `common::git::worktree_registration`(branch / prunable を返す)があり、`workspace_with_prunable_registration` の前提確認に使われているのに、観測側では使っていない
  - 提案: `worktree_present` を「登録が在り、かつ実体が在る」まで見る実装にする(TC-011 / TC-016 の真偽の主張はそのまま成立する)か、TC-010 に `worktree_registration` 由来の「登録が `ws.branch` を指す」観測用フックを1つ足す

- **[W-003]** `spawn` スイートの3件は、真偽を返すフックが常に `Some(true)` を返すハーネスでも全部通る
  - 場所: `crates/pulsen-conformance/src/process_controller.rs`(`spawn` モジュール)、`crates/pulsen-conformance/src/lib.rs:707`(`wait_for_run_files`)・`:728`(`run_dir_is_empty`)
  - 理由: TC-001 / TC-002 は `wait_for_run_files` が真であることだけを、TC-003 は `run_dir_is_empty` が真であることだけを主張する。両方とも「真であること」しか要求しないので、ハーネスが定数を返す実装でもスイートは緑になる。RunStore の `attempt_dir_present` は **`prepare_attempt` の前後で観測が反転すること**まで主張して同じ穴を塞いでいる(ADR-012・HOOKS.md にも明記)のに、`spawn` 側だけこの規律が適用されていない
  - 提案: TC-003 で `run_dir_is_empty` が起動前に真、`failing_controller` での `spawn_wrapper` 失敗後も真、という形にするか、TC-001 の冒頭で `run_dir_is_empty` が真 → `spawn_wrapper` → `wait_for_run_files` が真 → `run_dir_is_empty` が偽、まで主張して同一フックの反転を要求する

- **[W-004]** `TC-exec-tick-084`(runファイルの破損が続く → tick を**繰り返し**実行しても報告とスキップが続き launching のまま滞留する)を tick 1回でしか確かめていない
  - 場所: `crates/pulsen/tests/tick_confirm_spawn.rs:295`(`runファイルを読めなければ報告してスキップする`)
  - 理由: この行の主張は「繰り返しても stopped に至らない(= 通知されない launching 滞留になる)」ことで、1回の tick では「報告して書き込まない」までしか出ない。`spawn_fail_count` を破損のたびに加算する実装(= いずれ凍結する実装)に変わっても、このテストは緑のまま通る。同じ形の冪等性の主張は `tick_scan.rs:247` が不在系で2回回して書いているので、破損系だけ片手落ちになっている
  - 提案: 破損を返す台本を2〜3回分与えて `harness.completed()` を繰り返し、毎回 `errors` に同じ報告が出ること・`tasks.saved()` が空のままであること・`summary.frozen` が空であることを assert する

- **[W-005]** `TC-exec-tick-085`(人間が破損した runファイルを削除した → 不在として無効化マーカープロトコルに合流する)に固有のケースが無い
  - 場所: `crates/pulsen/tests/tick_confirm_spawn.rs`(不在)
  - 理由: 期待される結末は「不在 → 猶予 → マーカー → spawn失敗分類」で、`起動記録の後にクラッシュした状態は猶予時間の判断に合流する` と `再確認でもpidが無ければspawn失敗として起動待ちへ戻す` の2つが合わせて同じ観測を覆っている。前提(破損 → 削除)がダブルの台本上は「不在」と区別できないため実害は小さいが、行と関数の1:1対応が崩れており、後から「この行はどこで消化したか」が追えない
  - 提案: 破損 → 不在の順に台本を並べた1ケース(1回目の tick で `Corrupt` を報告、2回目で不在から `SpawnFailed` に決着)を置くと、行の前提そのものが表現できる

### カバレッジ

一覧(`changed-files.txt`)の実体は **62 行**(`git diff --stat origin/main...HEAD` も「62 files changed」)で、依頼文の 61 とは1件ずれる。以下は 62 件と1:1で対応する(確認 61 + スキップ 1)。

確認:

- `.thread/2/plan.md`, `.thread/2/steps.md`, `.thread/2/testing.md`, `.thread/2/progress.md`
- `crates/pulsen-conformance/HOOKS.md` — 区分表(A38/B113/C18 = 169)と各節の件数が一致することを再計算して確認。本スライス分44行(RunStore 21・ProcessController 16・WorktreeManager 7)+ 追加ケース1件、フック一覧もトレイト定義と一致
- `crates/pulsen-conformance/src/lib.rs`, `crates/pulsen-conformance/src/run_store.rs`, `crates/pulsen-conformance/src/process_controller.rs`, `crates/pulsen-conformance/src/worktree_manager.rs`
- `crates/pulsen-conformance/src/doubles/mod.rs`, `doubles/clock.rs`, `doubles/process.rs`, `doubles/run_store.rs`, `doubles/task_repository.rs`, `doubles/tests.rs`, `doubles/worktree.rs` — 台本を使い切った呼び出しがパニックする(暗黙の既定値を返さない)ことを確認
- `crates/pulsen-domain/src/execution/launching.rs` — 猶予境界 0/29/30/31/負、`(pid, starttime)` の4象限、`classify_recheck` の3分岐まで網羅
- `crates/pulsen-domain/src/execution/value.rs`, `crates/pulsen-domain/src/task/path.rs`(6パスと `state_root` の逆写像・形式外9パターン), `crates/pulsen-domain/src/task/planner.rs`, `crates/pulsen-domain/src/task/task.rs`(遷移6種を実行状態6値で網羅・等号では凍結しない・`in_place` は状態も attempt も変えない), `crates/pulsen-domain/src/task/counters.rs`, `crates/pulsen-domain/src/task/failure.rs`, `crates/pulsen-domain/src/task/attempt.rs`, `crates/pulsen-domain/src/task/transition.rs`
- `crates/pulsen-domain/src/definition/template.rs`(`rehydrate` の往復・空文字列トークン・0トークン拒否), `crates/pulsen-domain/src/definition/agent.rs`
- `crates/pulsen-domain/src/execution/mod.rs`, `crates/pulsen-domain/src/execution/port.rs`, `crates/pulsen-domain/src/task/mod.rs` — 宣言されたメソッドが適合スイートの行と1:1か(未実装スタブが無いか)を確認
- `crates/pulsen/examples/agent_probe.rs`, `crates/pulsen/examples/spawn_probe.rs` — `echo-args` の照合を呼び出し側に置く設計(自己照合にしていない)ことを確認
- `crates/pulsen/src/adapter/process.rs`(`128+シグナル番号` の具体値をユニットテストで固定・cwd 不在でログも作らない), `crates/pulsen/src/adapter/run_store.rs`(人間可読 JSON・値制約違反は `Corrupt`), `crates/pulsen/src/adapter/worktree.rs`(ユニットテストは無く適合スイートで裏付ける形であることを確認), `crates/pulsen/src/adapter/mod.rs`
- `crates/pulsen/src/application/run_wrapper.rs`, `crates/pulsen/src/application/tick/mod.rs`, `tick/launch.rs`, `tick/confirm_spawn.rs`, `crates/pulsen/src/application/mod.rs` — テストが到達しない分岐の有無を確認(`report_transition` は分岐の前提上到達不能な防御経路のみ)
- `crates/pulsen/src/cli/render.rs`(サマリー・スキップ理由・wrapper エラーのユニットテスト), `crates/pulsen/src/cli/tick.rs`, `crates/pulsen/src/cli/wrapper.rs`, `crates/pulsen/src/cli/wire.rs`, `crates/pulsen/src/cli/args.rs`, `crates/pulsen/src/cli/mod.rs`
- `crates/pulsen/tests/tick_fixture/mod.rs`, `tick_scan.rs`, `tick_launch.rs`, `tick_confirm_spawn.rs`, `run_wrapper.rs` — 異常系・境界値がすべてダブルに対して書かれ、実FS・実プロセスに落ちていないことを確認
- `crates/pulsen/tests/cli_tick.rs` — 非同期成果物の待ち合わせが「これから観測する成果物そのもの」(`exit` を読むなら `exit`、pid を見るなら pid)に立っていること、spawn を伴う全ケースが TempDir 破棄前に `wait_for_exit` で完了を待っていること、実時間 sleep への依存が無いことを1件ずつ確認
- `crates/pulsen/tests/cli_wrapper.rs`, `cli_usage.rs`, `register_task.rs`
- `crates/pulsen/tests/conformance_run_store.rs`, `conformance_process_controller.rs`, `conformance_worktree.rs` — 権限操作フック(`deny_file_read` / `deny_dir_write` / `deny_execute`)がいずれも制限の実効を probe してから `Some` を返すこと、スキップ許容集合が `permission_restrictions_effective()` / `cfg!(unix)` という**環境の能力**から決まり、`agent_probe` / `spawn_probe` の不在は許容していないことを確認
- `crates/pulsen/tests/common/mod.rs`(`skipped` が `SkipBudget` 経由で宣言外のスキップを失敗にすること), `crates/pulsen/tests/common/git.rs`

スキップ:

- `.thread/2/adr.md` — 設計判断の記録。テストの網羅性・実効性の判定には ADR-004 / 010 / 011 / 012 / 013 / 027 / 055 / 063 の該当箇所だけを参照し、文書そのもののレビューは Domain / Usecase 観点に委ねる(1件)
