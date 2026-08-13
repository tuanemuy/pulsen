# レビュー 006 — Use Case

### Use Case

**問題なし。** Blocker・Warning ともに挙げるものがない。

#### Blockers

なし

#### Warnings

なし

#### 確認した論点

- **`Tick` の処理フロー(spec 1〜9)**: `execute` は「ロック取得 → `list_active` → エントリごとの分岐 → サマリー」の順で、ロック競合は `TickOutcome::Skipped`(CLI で 0)・`LockError::Failed` と `list_active` の Io は `TickError`(非0・書き込みなし)。ガードは `_guard` に束縛され tick 全体で保持される。`process` は `Corrupt` → 報告のみ、`SnapshotUnreadable` → 報告のみ、`Intact` → `branch_of` の網羅 `match` へ、と spec 手順2 と一致する。gc(手順8)はスコープ外で、呼び出しもスタブも存在しない。
- **サマリーの語義**: spec の出力DTO 9フィールドをすべて持ち、`confirmed_running` だけを追加している(ADR-086)。追加は「起動確認が spec のどのフィールドにも集計されない」ことへの対処で、`is_empty` が全フィールドを見るため「タスクファイルに書き込んだ tick は必ずいずれかのフィールドを埋める」不変(ADR-084)が構成として成立している。書き込み経路を全部たどっても破れは見つからなかった — 起動確認 → `confirmed_running`、上限超過 → `frozen`、記録した失敗3種 → `errors`、保存失敗 → `errors`(`SaveFailed`)。`render::tick_summary` は `errors` を「失敗を記録 / 起動の結果が未確定 / スキップ」に網羅 `match` で振り分けており、書き込みの有無と見出しの語が食い違わない。
- **手続きA の順序と失敗時の遷移**: `ensure_workspace`(未確定時のみ `derive` → `create` → `confirm_workspace` → `save`、失敗は `record_tool_failure(WorktreeCreate, ..., applicable_retry_limit)` → `save` → 終了)→ `expand` 5段(a〜e のいずれの失敗も `record_spawn_failure_in_place` → `save` → 終了)→ `record_launching` → `save` → `prepare_attempt`(失敗は報告のみで spawn を続行)→ `spawn_wrapper`(同期エラーでも状態を変えない)。`save` 失敗時に後続の副作用へ進まないこと(ワークスペース確定の保存失敗で run ディレクトリに触れない・起動記録の保存失敗で spawn しない)も実装・テストとも成立している。
- **手続きC の順序**: 冒頭の `current_attempt` None 検査 → `read_pid_file` / `read_starttime`(`RunFileError` は報告してスキップ・書き込みなし)→ `classify` 3分岐 → `SuspectSpawnFailure` で `write_invalidation_marker`(`Err(Io)` は状態を変えず報告してスキップ)→ 再読 → `classify_recheck` 2分岐 → 上限超過で `Stopped` + `frozen`。ディレクトリ不在は `Ok(None)` として猶予経路に合流する。
- **二重起動を防ぐマーカー順序プロトコル**: 競合窓を検討したが残っていない。`SpawnFailed` に至るのはマーカー書き込み**後**の再読で pid が不在だったときだけであり、ラッパーは pid 書き込み**後**にマーカーを確認するため、そのラッパーのマーカー確認は必ずツールの書き込み後に起きて抑止される。逆に pid が再読で現れれば `ConfirmRunning` へ進み pending へ戻さない。`read_run_files` が pid → starttime の順で読むのも正しい — 逆順だと2回の読み取りの間に両方が書かれた健全な起動が「pid あり・starttime なし」と観測され、順序破れとして滞留する。`RunWrapper` 側も `own_identity` → `write_starttime` → `write_pid_file` → `marker_exists` → `run_agent` → `write_exit` で、starttime が pid より先、マーカー確認が pid の後になっている。`marker_exists` の `Err(Io)` も未起動終了(安全側)。
- **1タスクの失敗の隔離と冪等性**: `process` はエントリごとに独立し、どの失敗経路も `summary.errors` に積んで次のエントリへ進む。連続実行で判断が変わらないこと(状態不変のタスク群・worktree 作成直後のクラッシュ・破損 run ファイルの継続)がテストで主張されている。
- **スコープ外の手続き**: `Cleanup` / `Running` / `Completed` / `Stopped` のアームは空で、コメントが引き取り先の Issue を示すだけ。`notify` の呼び出しも無く、`commit` が唯一の `save` 経路として #3 の追加点を1箇所に閉じている。`Freeze` を遷移の呼び出し側から渡す形にしているため、catch-up 通知が過去の凍結を再計上する縮退も避けられている。未配線アームが何もしない(書き込まない・worktree を触らない・ラッパーを起動しない)ことはテストで主張されている。
- **判断と副作用の分離**: 分類は `LaunchingClassifier`、遷移は `Task` の遷移関数、`branch_of` は判別だけを値にする純粋関数。ユースケースはポート越しの観測と実行の配線に徹しており、`config` を読むのは tick 側だけで `RunWrapper` は config もロックも扱わない(`compose_wrapper` はホームも `current_exe` も解決しない)。
- **実効的なテスト**: AC-17 の一覧(ロック異常・`list_active` の Io・worktree 作成失敗と上限超過・展開失敗5経路・`prepare_attempt` 失敗・`spawn_wrapper` 同期エラー・`RunFileError` の各種・マーカー書き込み失敗・猶予境界 30/31/巻き戻り・`save` 失敗)がすべてテストダブルに対するユースケーステストで消化されている。順序の主張はダブルの `calls()` の並びで行われ(`猶予超過ではマーカーを書いてからpidを再確認する` / `同定情報の記録はstarttimeを先に書きマーカー確認はpidの後に行われる`)、結果の値に現れない契約が実際に守られている。AC-15 のクロスtickは `cli_tick.rs` が「これから観測する成果物そのもの」(`exit` / `pid`)を待ち条件にして待ち合わせている。AC-12 の「任意の cwd から起動しても同じ結果」は `compose` から `current_dir()` を外した上で `cli_tick_missing_cwd.rs` が作業ディレクトリ消失まで含めて裏付けている。
- **コード・コメント**: 残っているのは why / why not に限られ、指摘への弁明や修正の経緯は見当たらない。`#[allow]` にも理由が添えられている。
- **実行確認**: `cargo test -p pulsen --test tick_scan --test tick_launch --test tick_confirm_spawn --test run_wrapper` は 4 スイートすべて緑(tick_scan 12 / tick_launch 19 ほか)。

#### カバレッジ

- 確認(23):
  `.thread/2/plan.md`(契約), `.thread/2/adr.md`(ADR-065 / 066 / 073 / 084 / 086 / 091 の該当節),
  `crates/pulsen/src/application/mod.rs`, `crates/pulsen/src/application/run_wrapper.rs`,
  `crates/pulsen/src/application/tick/mod.rs`, `crates/pulsen/src/application/tick/launch.rs`,
  `crates/pulsen/src/application/tick/confirm_spawn.rs`,
  `crates/pulsen/src/cli/mod.rs`, `crates/pulsen/src/cli/args.rs`, `crates/pulsen/src/cli/tick.rs`,
  `crates/pulsen/src/cli/wrapper.rs`, `crates/pulsen/src/cli/wire.rs`, `crates/pulsen/src/cli/render.rs`,
  `crates/pulsen/src/cli/add.rs`,
  `crates/pulsen/tests/tick_fixture/mod.rs`, `crates/pulsen/tests/tick_scan.rs`,
  `crates/pulsen/tests/tick_launch.rs`, `crates/pulsen/tests/tick_confirm_spawn.rs`,
  `crates/pulsen/tests/run_wrapper.rs`, `crates/pulsen/tests/cli_tick.rs`,
  `crates/pulsen/tests/cli_tick_missing_cwd.rs`, `crates/pulsen/tests/cli_wrapper.rs`,
  `crates/pulsen/tests/cli_usage.rs`

- スキップ(72):
  - `.adr/027-port-conformance-suite-and-harness-hooks.md` — 適合ハーネスのフック規約。ポート適合の観点。
  - `.thread/2/progress.md`, `.thread/2/steps.md`, `.thread/2/testing.md` — 進行管理・手順の記録で、ユースケースの契約ではない。
  - `.thread/2/review/`(30ファイル) — 過去ラウンドの成果物。ゼロベース指示により読まない。
  - `crates/pulsen-conformance/`(12ファイル: `HOOKS.md`, `src/lib.rs`, `src/process_controller.rs`, `src/run_store.rs`, `src/worktree_manager.rs`, `src/doubles/{mod,clock,process,run_store,task_repository,tests,worktree}.rs`) — ポート適合スイートとダブルの実装。ダブルは `tick_fixture` 経由で挙動を確認済みだが、実装自体はテスト/アダプターの観点。
  - `crates/pulsen-domain/`(14ファイル: `definition/{agent,template}.rs`, `execution/{launching,mod,port,value}.rs`, `task/{attempt,counters,failure,mod,path,planner,task,transition}.rs`) — 分類・遷移・ポート定義。ドメインの観点。ユースケースからは呼び出しの順序と結果の扱いだけを確認した。
  - `crates/pulsen/examples/{agent_probe,spawn_probe}.rs` — 適合スイートと受け入れテストの補助バイナリ。テストの観点。
  - `crates/pulsen/src/adapter/{mod,process,run_store,worktree}.rs` — OS・ファイルシステム・git の詳細。アダプターの観点。
  - `crates/pulsen/tests/{common/git.rs,common/mod.rs}` — 受け入れテストの補助(待ち合わせ・git 操作)。テストの観点。
  - `crates/pulsen/tests/{conformance_process_controller,conformance_run_store,conformance_worktree}.rs` — 適合スイートの適用。アダプター/テストの観点。
  - `crates/pulsen/tests/register_task.rs` — `add` のユースケーステスト。本スライスの変更は `Runtime` 分解に伴う結線のみで、`cli/add.rs` 側で確認済み。
