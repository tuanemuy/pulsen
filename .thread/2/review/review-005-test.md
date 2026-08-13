# レビュー005 — Test

## Test

### Blockers

なし。

`spec/testcases/execution/tick.md` / `run-wrapper.md` / `spec/testcases/ports/{run-store,process-controller,worktree-manager}.md` のうち Issue #2 のチェックリストに載る TC-* 行を ID 単位で突き合わせ、いずれにも対応するテストが存在することを確認した（下記「TC 突き合わせ」）。この環境で `cargo test --workspace` は全件緑で、`--nocapture` で観測できたスキップは既存の `tc_port_clock_004 / 005`（`advance` / `rewind` を持たない実クロック）だけだった。本スライスが追加した適合ケース・受け入れケースはすべて実行されている。

### Warnings

- **[W-001]** テスト名が主張する内容をアサーションが裏付けていない。
  場所: `crates/pulsen/tests/cli_tick.rs` `tickはヘルプに現れる利用者向けのコマンドである`。
  理由: 実際のアサーションは `run.assert_succeeded()` と `assert!(!run.stdout.is_empty())` の2つだけで、「ヘルプ（の一覧）に現れる」ことも「利用者向けである」ことも主張していない。`--help` が 0 で終わって何か出る、はほぼ任意の clap サブコマンドで成立するので、退行を捕まえる力を持たない。名前が主張する内容は `crates/pulsen/tests/cli_usage.rs` の `利用者向けに提供するサブコマンドはタスク登録とtickだけである`（`subcommands(&stdout)` の完全一致）と `内部のラッパーはヘルプに現れないが実行はできる` がすでに厳密に主張している。
  提案: この test を落とす（`cli_usage.rs` 側が同じ主張をより強く持っている）か、`tick --help` の出力に対して確かめたい固有の性質（例: 引数を取らないこと）を明示的に assert する。

- **[W-002]** `TC-exec-run-wrapper-027` の「本スライスで消化する範囲」を直接主張するテストが無い。
  場所: `crates/pulsen/tests/cli_wrapper.rs` / `crates/pulsen/tests/run_wrapper.rs`（該当ケース不在）、`.thread/2/plan.md` 部分消化表、`.thread/2/steps.md` ステップ9「消化するチェックリスト … TC-exec-run-wrapper-021〜027」。
  理由: plan.md は 027 を部分消化としつつ、消化する範囲を「ラッパーごと kill された attempt に exit が残らないこと」と明記している。しかしラッパー（またはその実行単位）を kill する経路のテストはリポジトリ内に存在しない（`tests/` の `kill` 出現箇所は `conformance_lock.rs::kill_holder` と `kill_ident` の名前のみ）。実質の裏付けとしては `cli_tick.rs::滞留するエージェントを起動したままでも次のtickは競合しない` が「エージェントが生きている間 `exit` は現れない」を主張しており、`exit` が run_agent 復帰後にのみ書かれることは押さえられているが、kill そのものは実行されていない。
  提案: 行にチェックは付かない前提なので、テストを足すか、Issue のコメントに残す消化範囲の記述を実態（「exit はエージェントの終了後にのみ書かれることまで。kill 経路の観測は #3」）に合わせるかのどちらかで、宣言と実装を一致させる。kill する受け入れテストを足す場合は、デタッチした実行単位への kill と `exit` 書き込みの競合で flaky になりやすいので、`agent_probe wait-for` で滞留させたうえで pid ファイル出現後に kill する形に限る。

### 個別に確認した論点

**プロセスの作業ディレクトリを消す受け入れテスト（`tests/cli_tick_missing_cwd.rs`）** — 他テストへの影響と後始末はいずれも問題なし。

- 独立した実行ファイル（＝独立プロセス）に1 test だけを置いており、`env::set_current_dir` の効果が他のテストへ漏れない。同一バイナリ内の並行実行も起こらない。ファイル冒頭の `#![cfg(unix)]` により、前提を作れない Windows ではファイルごと消える。
- 後始末は二重に効いている。`env::set_current_dir(&original)` で復帰し、`gone`（`TempDir`）の Drop は削除済みディレクトリに対する `remove_dir_all` のエラーを無視する。復帰前にパニックしてもプロセスがそこで終わるため残留しない。
- 子プロセスの起動は `env!("CARGO_BIN_EXE_pulsen")`（絶対パス）で、`detached_home` が作る一時ホームも `std::env::temp_dir()` 起点の絶対パスなので、cwd 不在は起動経路に影響しない。`Home::new()` は `state/` を作らないため、走査は空結果になり主張が「処理対象なし」で閉じる。
- 実際に `cargo test --test cli_tick_missing_cwd` を単体で走らせて緑、`cargo test --workspace` 全体でも緑であることを確認した。

**受け入れテストの flaky リスク** — `crates/pulsen/tests/cli_tick.rs` でラッパーを spawn するテストは、いずれも終了前に `wait_for_exit`（`exit` の出現待ち）を通しており、一時ホームの削除と孫プロセスの書き込みが競合する経路が閉じている。待ち条件が「これから観測する成果物そのもの」に立っている点も plan.md AC-15 のとおりで、`次のtickはpidの出現をもってrunningへ取り込む` が `pid` ではなく `exit` を待つなど、含意関係を取り違えていない。`滞留するエージェント…` の「2回目の tick がラッパー生存中に走る」は解放ファイルという外部条件で決まるため、環境の速さに依存しない。

**時刻依存・順序依存** — 猶予境界（30 / 31 / 巻き戻り）は `SettableClock` に対するユースケーステストとドメインユニットテストの両方で決定的に消化されている。書き込み順序の主張（starttime→pid→マーカー確認、マーカー書き込み→pid 再確認）は `RunStoreCall` / `ProcessControllerCall` の列に対する完全一致で書かれており、部分一致に緩んでいない。

**適合スイートのフック契約** — `attempt_dir_present` / `run_dir_is_empty` / `worktree_present` はいずれも「前後で観測が反転すること」まで主張しており、定数を返すハーネスが素通りしない。`worktree_marker` が不在を空文字列で返してスキップと区別する規律、権限系フックが「制限が実際に効いたことを確かめてから `Some`」を返す規律も守られている。`SkipBudget` は宣言外のスキップをそのケースの失敗にし、受け入れテスト側（`tests/common/mod.rs`）にも同じ宣言が入っている。`agent_probe` / `spawn_probe` の不在はスキップ許容集合に入っておらず、作り忘れが緑にならない。

**テストダブル** — `Scripted*` は台本を使い切ると必ずパニックし、扱わないメソッドもパニックする。`tick_scan.rs::配線していない分岐のタスクにはエージェントを起動せず書き込みもしない` が `runs.calls().is_empty()` を主張できるのは、誤って配線した瞬間に台本切れで落ちるからで、アサーションが空虚になっていない。

**HOOKS.md** — 冒頭の集計（9ポート169行 / A 38・B 113・C 18）と各ポート節の行数・区分が一致することを検算した（ConfigStore 24 + WorkflowStore 31 + TaskRepository 44 + Clock 5 + TaskIdGenerator 5 + ExclusiveLock 7 + WorktreeManager 16 + RunStore 21 + ProcessController 16 = 169、A/B/C も一致）。台帳行に対応しない追加ケース2件（RunStore の write 系ディレクトリ作成、WorktreeManager の `prunable`）は件数に数えず区別して記載されている。

**残す必要のない記述** — テストコード・フィクスチャのコメントは、いずれも「現在の形が成り立つ理由」（なぜ待ち条件をその成果物に立てるのか、なぜ読み手の停止を `Drop` に載せるのか、なぜ占有 worktree の前提をその場で assert するのか、なぜ配線しないアームに期待を持たせないのか）を説明しており、指摘への弁明や修正の経緯は見当たらなかった。

### TC 突き合わせ（ID 単位）

- Tick 走査と分岐 001 / 002 / 004 / 006 / 007 / 012 / 013 / 014 / 015 / 016 / 017 / 018 / 019 / 027 — `tests/tick_scan.rs` と `tests/cli_tick.rs`（012 はアーカイブ後の空走査、027 は `state/tasks/` 未作成）。
- Tick 手続きA 028〜055 — `tests/tick_launch.rs`（展開失敗の5経路は `expansion_failures()` が 039〜043 を1件ずつ、境界は 048 / 049 / 050）、051 / 052 / 055 は `tests/cli_tick.rs` と `conformance_worktree.rs`（TC-port-worktree-manager-012 / 013）で裏打ち。
- Tick 手続きC 068〜086 — `tests/tick_confirm_spawn.rs`。082 / 083 は tick 側の観測としては同一（再読で pid 検出 → running）なので1ケースで両方を消化している。
- RunWrapper 001〜026 — `tests/run_wrapper.rs`（010 / 011 / 012 / 018 / 021 / 022 / 023 の内部分岐と順序）と `tests/cli_wrapper.rs`（実バイナリ: 001 / 003 / 004 / 007 / 009 / 013〜017 / 019 / 020）、008 は `tests/cli_tick.rs`、024 は `tests/tick_confirm_spawn.rs`、025 は `TC-port-run-store-016`。027 は W-002。
- TC-port-run-store-001〜021 — `crates/pulsen-conformance/src/run_store.rs` の21ケース＋追加1件、`tests/conformance_run_store.rs` で `FsRunStore` に適用。
- TC-port-process-controller-001〜005 / 017〜027 — `crates/pulsen-conformance/src/process_controller.rs` の2スイート16ケース、`tests/conformance_process_controller.rs` で `SystemProcessController` に適用。`128+シグナル番号` の具体値は `adapter/process.rs` のユニットテストに置かれ、適合スイートは「非0」に留めている。
- TC-port-worktree-manager-010〜016 — `crates/pulsen-conformance/src/worktree_manager.rs` の7ケース＋`prunable` の追加1件。

### カバレッジ

- 確認: `.thread/2/plan.md`, `.thread/2/steps.md`, `.thread/2/testing.md`, `crates/pulsen-conformance/HOOKS.md`, `crates/pulsen-conformance/src/doubles/clock.rs`, `crates/pulsen-conformance/src/doubles/mod.rs`, `crates/pulsen-conformance/src/doubles/process.rs`, `crates/pulsen-conformance/src/doubles/run_store.rs`, `crates/pulsen-conformance/src/doubles/task_repository.rs`, `crates/pulsen-conformance/src/doubles/tests.rs`, `crates/pulsen-conformance/src/doubles/worktree.rs`, `crates/pulsen-conformance/src/lib.rs`, `crates/pulsen-conformance/src/process_controller.rs`, `crates/pulsen-conformance/src/run_store.rs`, `crates/pulsen-conformance/src/worktree_manager.rs`, `crates/pulsen-domain/src/definition/agent.rs`, `crates/pulsen-domain/src/definition/template.rs`, `crates/pulsen-domain/src/execution/launching.rs`, `crates/pulsen-domain/src/execution/mod.rs`, `crates/pulsen-domain/src/execution/port.rs`, `crates/pulsen-domain/src/execution/value.rs`, `crates/pulsen-domain/src/task/attempt.rs`, `crates/pulsen-domain/src/task/counters.rs`, `crates/pulsen-domain/src/task/failure.rs`, `crates/pulsen-domain/src/task/mod.rs`, `crates/pulsen-domain/src/task/path.rs`, `crates/pulsen-domain/src/task/planner.rs`, `crates/pulsen-domain/src/task/task.rs`, `crates/pulsen-domain/src/task/transition.rs`, `crates/pulsen/examples/agent_probe.rs`, `crates/pulsen/examples/spawn_probe.rs`, `crates/pulsen/src/adapter/mod.rs`, `crates/pulsen/src/adapter/process.rs`, `crates/pulsen/src/adapter/run_store.rs`, `crates/pulsen/src/adapter/worktree.rs`, `crates/pulsen/src/application/mod.rs`, `crates/pulsen/src/application/run_wrapper.rs`, `crates/pulsen/src/application/tick/confirm_spawn.rs`, `crates/pulsen/src/application/tick/launch.rs`, `crates/pulsen/src/application/tick/mod.rs`, `crates/pulsen/src/cli/add.rs`, `crates/pulsen/src/cli/args.rs`, `crates/pulsen/src/cli/mod.rs`, `crates/pulsen/src/cli/render.rs`, `crates/pulsen/src/cli/tick.rs`, `crates/pulsen/src/cli/wire.rs`, `crates/pulsen/src/cli/wrapper.rs`, `crates/pulsen/tests/cli_tick.rs`, `crates/pulsen/tests/cli_tick_missing_cwd.rs`, `crates/pulsen/tests/cli_usage.rs`, `crates/pulsen/tests/cli_wrapper.rs`, `crates/pulsen/tests/common/git.rs`, `crates/pulsen/tests/common/mod.rs`, `crates/pulsen/tests/conformance_process_controller.rs`, `crates/pulsen/tests/conformance_run_store.rs`, `crates/pulsen/tests/conformance_worktree.rs`, `crates/pulsen/tests/register_task.rs`, `crates/pulsen/tests/run_wrapper.rs`, `crates/pulsen/tests/tick_confirm_spawn.rs`, `crates/pulsen/tests/tick_fixture/mod.rs`, `crates/pulsen/tests/tick_launch.rs`, `crates/pulsen/tests/tick_scan.rs`（62ファイル） / スキップ: `.adr/027-port-conformance-suite-and-harness-hooks.md` — フック一覧の正本は `crates/pulsen-conformance/HOOKS.md`（同ファイル末尾の宣言）であり、本観点では正本側を確認した、`.thread/2/adr.md` — 設計判断の記録。テスト観点の契約は plan.md / steps.md 側で読んだ、`.thread/2/progress.md` — 進捗記録で、TC 行の突き合わせには使わない、`.thread/2/review/review-001-adapter.md`, `.thread/2/review/review-001-architecture.md`, `.thread/2/review/review-001-domain.md`, `.thread/2/review/review-001-test.md`, `.thread/2/review/review-001-usecase.md`, `.thread/2/review/review-001.md`, `.thread/2/review/review-002-adapter.md`, `.thread/2/review/review-002-architecture.md`, `.thread/2/review/review-002-domain.md`, `.thread/2/review/review-002-test.md`, `.thread/2/review/review-002-usecase.md`, `.thread/2/review/review-003-adapter.md`, `.thread/2/review/review-003-architecture.md`, `.thread/2/review/review-003-domain.md`, `.thread/2/review/review-003-test.md`, `.thread/2/review/review-003-usecase.md`, `.thread/2/review/review-003.md`, `.thread/2/review/review-004-adapter.md`, `.thread/2/review/review-004-architecture.md`, `.thread/2/review/review-004-domain.md`, `.thread/2/review/review-004-test.md`, `.thread/2/review/review-004-usecase.md`, `.thread/2/review/review-004.md`, `.thread/2/review/triage.md` — 過去ラウンドの成果物。本ラウンドはゼロベースで見る指示のため読まない（24ファイル）
