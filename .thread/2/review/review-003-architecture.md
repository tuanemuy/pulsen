# レビュー003 — Architecture / CLI

## Architecture / CLI

### Blockers

なし

### Warnings

- **[W-001]** タスクファイルへ書き込んだ tick の報告が「スキップ」の見出しに落ちる / 場所: `crates/pulsen/src/cli/render.rs:84-100`(`recorded_failure`)、`crates/pulsen/src/cli/render.rs:70-76` / 理由: `recorded_failure` は `TickIssue` を「失敗を記録」と「スキップ」の2つに割り、後者の doc に「何も記録せず次の tick がそのまま再試行するスキップ」と書いている。しかし `PrepareAttemptFailed` と `SpawnFailed` はどちらも `record_launching` → `save` が成功した**後**に積まれる報告で、タスクファイルは書き換わり attempt 番号も消費されている。次の tick はそのまま再試行するのではなく、`Launching` として手続きC(猶予経路)へ入る。とくに `PrepareAttemptFailed` は spawn が成功すれば同じタスクが `launched` にも積まれるため(`crates/pulsen/src/application/tick/launch.rs:59-74`、`crates/pulsen/tests/tick_launch.rs:336-363` が `summary.launched` と `summary.errors` の同時充填を主張している)、表示は

  ```
    起動: <task-id>
    スキップ(1件):
      - <task-id>: attempt の runディレクトリを用意できません(...)
  ```

  となり、同一タスクが「起動」と「スキップ」に同時に現れる。cron 運用ではこの出力が唯一の窓で、「起動待ちのまま何も起きなかった」と読めてしまう。ADR-084 が「書き込んだ tick が処理対象なしと表示される」ことを構成として潰したのと同じ理由で、見出しの語義も書き込みの有無と食い違わせるべきではない。/ 提案: `SpawnFailed` / `PrepareAttemptFailed` を「スキップ」から外す。ADR-090 の2分割は「カウンタを消費したか」で切っているので、この2つはどちらにも属さない第3の見出し(例: 「起動後の不備」— launching は記録済みで次 tick が猶予経路で分類する、と読める語)に分けるか、少なくとも第2見出しの語を「スキップ」から中立な語(「報告」等)に改め、`recorded_failure` の doc の「何も記録せず次の tick がそのまま再試行する」を実態に合わせて書き直す。

- **[W-002]** AC-1 の機械的確認の期待値が実際の grep 結果と合っていない / 場所: `.thread/2/testing.md:50-52`、`.thread/2/plan.md:18`(AC-1) / 理由: 両者とも「`crates/pulsen/src/` 側のヒットは `util/atomic.rs` と `adapter/process.rs` だけである」としているが、記載の grep をそのまま実行すると3ファイルがヒットする。

  ```
  $ grep -rnE 'cfg\([^)]*\b(unix|windows|target_os|target_family)\b' crates/*/src/
  crates/pulsen-conformance/src/lib.rs      (2件・testing.md が明示的に除外済み)
  crates/pulsen/src/adapter/process.rs      (8件)
  crates/pulsen/src/adapter/task_repository.rs:278  #[cfg(all(test, unix))]
  crates/pulsen/src/util/atomic.rs          (2件)
  ```

  `adapter/task_repository.rs:278` は Issue #1 から在るテストモジュール限定の `cfg` で、本 PR の変更ではない。ただし AC-1 は本スライスの受け入れ基準として掲げられており、`pulsen-conformance/src/lib.rs` のヒットには除外の理由が添えてあるのにこの1件には無いため、ステップ19 でこの確認を実行する者は「AC-1 が満たせていない」か「見なかったことにする」のどちらかに落ちる。どちらも受け入れ基準として機能しない。/ 提案: `testing.md` の期待値に `adapter/task_repository.rs` のテスト限定 `cfg` を除外対象として明記する(`pulsen-conformance/src/lib.rs` と同じ書き方で、本番の実行経路に乗らないことを理由に添える)。`plan.md` AC-1 の本文も同様に揃える。

### カバレッジ

- 確認: `.thread/2/plan.md`, `.thread/2/adr.md`, `.thread/2/progress.md`, `.thread/2/steps.md`, `.thread/2/testing.md`, `crates/pulsen-conformance/HOOKS.md`, `crates/pulsen-conformance/src/lib.rs`, `crates/pulsen-domain/src/execution/port.rs`, `crates/pulsen-domain/src/task/path.rs`, `crates/pulsen-domain/src/task/planner.rs`, `crates/pulsen/examples/agent_probe.rs`, `crates/pulsen/examples/spawn_probe.rs`, `crates/pulsen/src/adapter/mod.rs`, `crates/pulsen/src/adapter/process.rs`, `crates/pulsen/src/adapter/run_store.rs`, `crates/pulsen/src/adapter/worktree.rs`, `crates/pulsen/src/application/mod.rs`, `crates/pulsen/src/application/run_wrapper.rs`, `crates/pulsen/src/application/tick/mod.rs`, `crates/pulsen/src/application/tick/launch.rs`, `crates/pulsen/src/application/tick/confirm_spawn.rs`, `crates/pulsen/src/cli/mod.rs`, `crates/pulsen/src/cli/args.rs`, `crates/pulsen/src/cli/render.rs`, `crates/pulsen/src/cli/tick.rs`, `crates/pulsen/src/cli/wire.rs`, `crates/pulsen/src/cli/wrapper.rs`, `crates/pulsen/tests/cli_tick.rs`, `crates/pulsen/tests/cli_usage.rs`, `crates/pulsen/tests/cli_wrapper.rs`, `crates/pulsen/tests/common/mod.rs`, `crates/pulsen/tests/tick_fixture/mod.rs`, `crates/pulsen/tests/tick_scan.rs`, `crates/pulsen/tests/tick_launch.rs`, `crates/pulsen/tests/tick_confirm_spawn.rs`(35件)
- スキップ: `.thread/2/review/review-001.md`, `.thread/2/review/review-001-adapter.md`, `.thread/2/review/review-001-architecture.md`, `.thread/2/review/review-001-domain.md`, `.thread/2/review/review-001-test.md`, `.thread/2/review/review-001-usecase.md`, `.thread/2/review/review-002-adapter.md`, `.thread/2/review/review-002-architecture.md`, `.thread/2/review/review-002-domain.md`, `.thread/2/review/review-002-test.md`, `.thread/2/review/review-002-usecase.md`, `.thread/2/review/triage.md` — 過去ラウンドの成果物。ゼロベースで見るため読まない指示による(12件)
- スキップ: `crates/pulsen-domain/src/definition/agent.rs`, `crates/pulsen-domain/src/definition/template.rs`, `crates/pulsen-domain/src/execution/mod.rs`, `crates/pulsen-domain/src/execution/value.rs`, `crates/pulsen-domain/src/execution/launching.rs`, `crates/pulsen-domain/src/task/mod.rs`, `crates/pulsen-domain/src/task/task.rs`, `crates/pulsen-domain/src/task/transition.rs`, `crates/pulsen-domain/src/task/attempt.rs`, `crates/pulsen-domain/src/task/counters.rs`, `crates/pulsen-domain/src/task/failure.rs` — 遷移規則・カウンタ規則・分類の直和型そのものの妥当性は Domain 観点の担当。本観点では「ドメインが std 以外に依存せず I/O を持たない」ことと、ポート越しの依存方向だけを外側から確認した(11件)
- スキップ: `crates/pulsen-conformance/src/run_store.rs`, `crates/pulsen-conformance/src/process_controller.rs`, `crates/pulsen-conformance/src/worktree_manager.rs` — 適合スイート各行と台帳の対応は Adapter / Test 観点の担当。本観点ではハーネスの口(`lib.rs`)と `HOOKS.md` の対応表だけを見た(3件)
- スキップ: `crates/pulsen-conformance/src/doubles/mod.rs`, `crates/pulsen-conformance/src/doubles/clock.rs`, `crates/pulsen-conformance/src/doubles/process.rs`, `crates/pulsen-conformance/src/doubles/run_store.rs`, `crates/pulsen-conformance/src/doubles/task_repository.rs`, `crates/pulsen-conformance/src/doubles/worktree.rs`, `crates/pulsen-conformance/src/doubles/tests.rs` — テストダブルの記録・スクリプト機構は Test 観点の担当(7件)
- スキップ: `crates/pulsen/tests/conformance_run_store.rs`, `crates/pulsen/tests/conformance_process_controller.rs`, `crates/pulsen/tests/conformance_worktree.rs`, `crates/pulsen/tests/run_wrapper.rs`, `crates/pulsen/tests/register_task.rs`, `crates/pulsen/tests/common/git.rs` — 適合ハーネスの実装・ユースケース単体の網羅・git フィクスチャは Adapter / Test / Usecase 観点の担当(6件)

確認35 + スキップ39 = 74。

### 確認した事実(指摘に至らなかったもの)

- 依存方向: `pulsen-domain` の `[dependencies]` は空のまま。ドメインは `std` の `path` / `ffi` / `collections` しか使わず、`std::fs` / `std::process` / `std::io` / `std::env` / `std::time` の出現は0件。`Tick` / `RunWrapper` はポートをジェネリック引数で受け、アダプターの型を知らない。
- 合成ルート: `env::` / `current_exe()` / `home_dir()` / `current_dir()` の読み取りは `cli/wire.rs` の1箇所に閉じている。`compose_wrapper` はホームも `ConfigStore::load` も `current_exe()` も呼ばず、`RunDirPath::state_root()`(ドメインの逆写像)だけから `FsRunStore` を組む。`Runtime` と別型(`WrapperRuntime`)にしてある(ADR-070)。
- 未実装メソッドの宣言・スタブ: `grep -nE 'fn (attempt_exists|list_runs|delete_attempt|remove_task_dir_if_empty|starttime_of|kill|try_kill_remnants|remove)\(' crates/pulsen-domain/src/execution/port.rs` は0件。`RunStore` 9・`ProcessController` 3・`WorktreeManager` は `create` が1つ増えて4で、AC-6 の数と一致する。呼び出しの無い `read_exit` には「`write_exit` と対の読み取りで、往復が閉じていることが exit の形式の唯一の裏づけになる」という why が付いている。
- スコープ外の混入: 手続きB / D / E・notify・`ls` / `show` / `abort` / `retry` / `set-status` に属する型・メソッドは1つも実装されていない。`Branch` の未配線アーム(`Cleanup` / `Observe` / `Advance` / `Notify`)はダミー処理もエラー報告も持たず、引き取り先のスライスだけを why コメントに残している(ADR-065 のとおり)。
- レイヤーの責務配置: 表示文言は `cli/render.rs`、レイアウト(`runs/` 段・attempt 接頭辞・6ファイル名・ブランチ接頭辞)は `task/path.rs` と `task/planner.rs`、I/O は `adapter/`。帳簿に永続化される失敗要因だけがユースケースで組まれるが、ドメインの `describe()` に文脈を添える形に統一されている(ADR-082)。
- `tick` の終了コード: ロック競合 `Ok(None)` → `Skipped` → **0**(標準出力)、`LockError::Failed` / `list_active` の Io → `TickError` → 非0(標準エラー)、サマリーは常に 0。config 不在・パース不能は `compose` の段で非0。pages の exit code 規約と縮退状態の表に一致する。
- `wrapper`: `#[command(hide = true)]` でヘルプ一覧に出ず(`pulsen --help` の実行で確認)、`pulsen wrapper --help` には到達できる。`trailing_var_arg` + `allow_hyphen_values` で `--model` / 空文字列 / `$HOME` / `*` / `a && b` / `>out.txt` / `{input}` が展開も再分割もされずに往復することを `cli_wrapper.rs` が主張し、argv の組み立てはアダプターの公開定数(`WRAPPER_SUBCOMMAND` / `RUN_DIR_FLAG` / `WORKSPACE_FLAG` / `COMMAND_SEPARATOR`)をテスト側も共有して食い違いを防いでいる。引数不正・run ディレクトリの形式外は run ディレクトリに何も書かず非0。
- 標準出力 / 標準エラー: 成功と「対象なし」「ロックスキップ」は標準出力、エラーは標準エラー。`--help` / `--version` は `use_stderr()` の分岐で 0 かつ標準出力。ラッパーは成功時に標準出力へ何も書かない(`cli_wrapper.rs` が主張)。
- ADR 参照: コード中の参照は 009/010/013/014/015/016/019/020/021/024/025/026/027/028/029/030/031/032/033/036/037/042/050/053/055/060/061/062/063(`.adr/` に実在)と 065〜086 のうち使用分(`.thread/2/adr.md` に実在)。dangling は0件。`.adr/` の最大は 064、本スライスの採番は 065 以降で衝突なし。`.adr/` への昇格が未了なのは progress.md がステップ19 として明記しているとおり。
- ADR-084 の不変(書き込んだ経路は必ずサマリーのいずれかを埋める): 書き込み経路を全列挙して確認した。`confirm_workspace` の保存だけで終わる経路は存在せず(`tick_launch.rs` の「ワークスペースを確定しただけで終わるtickは存在しない」)、`save` 失敗も `SaveFailed` として `errors` に載る。表示条件は `TickSummary::is_empty` だけに従属する。
- 検証: `cargo clippy --workspace --all-targets -- -D warnings` と `cargo test --workspace` はいずれも成功(exit 0)。新規依存クレートの追加はなく、`unsafe_code = "forbid"` は維持されている。
