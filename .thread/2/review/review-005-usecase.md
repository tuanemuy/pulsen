# レビュー005 — Use Case

## Use Case

### Blockers

なし。

`spec/usecases/execution.md` の Tick 処理フロー 1〜9・手続きA・手続きC・RunWrapper のいずれも、順序と失敗時の状態遷移が spec と一致している。スコープ外の手続き(B / D / E / 共通手続き notify)は配線されておらず、宣言も呼び出しも存在しない。

### Warnings

- **[W-001]** run ファイルを読む順序(pid → starttime)が正しさを担っているのに、その理由がコードにもテスト名にも書かれていない。
  - 場所: `crates/pulsen/src/application/tick/confirm_spawn.rs` の `read_run_files`(142〜173行)と、順序を固定している `crates/pulsen/tests/tick_confirm_spawn.rs:92`(`猶予時間内にpidが現れていなければ書き込みを一切行わない`)
  - 理由: ラッパーはロックを取らずに `write_starttime` → `write_pid_file` の順で書く。tick が pid を先に読む限り「pid が Some なら starttime も必ず Some」が成立し、`InconsistentRunFiles::MissingStartTime` は本当の順序破れのときにしか出ない。逆順(starttime → pid)で読むと、2回の読み取りの間にラッパーが両方を書き終えた場合に「starttime なし・pid あり」という観測が正常な起動に対して生じ、健全なタスクが `errors` に載って launching のまま1 tick 滞留する。cron 運用ではサマリーが唯一の窓なので、偽の破れ報告は運用者を破損 run ファイルの手動削除(手続きC の復旧導線)へ誘導しうる。実装は正しい順序になっており、テストも `RunStoreCall` の並びでその順序を固定しているが、モジュール doc(1〜6行)がマーカーの順序だけを説明していて読み取り順序には触れていないため、順序を入れ替えた変更が「なぜ落ちたのか」を読み手に伝えられない。
  - 提案: `read_run_files` の doc に「pid を先に読むのは、ラッパーが starttime → pid の順で書くため。逆順だと2回の読み取りの間に両方が書かれた正常な起動が順序破れとして観測される」を1文足す。テスト側は現状の主張で十分(順序は `calls()` で固定済み)。

### 確認した点(主要)

- **処理フロー 1〜9**: ロック取得(`Ok(None)` → `Skipped` / `LockError::Failed` → `TickError::LockFailed`)、`list_active` の `Io` → `TickError::Scan`(状態不変)、エントリごとの分岐(`Corrupt` は報告のみ・`SnapshotUnreadable` は定義依存の判断をスキップして報告・`Intact` は実行状態で分岐)、1タスクの失敗を `errors` に積んで残りを続行、サマリー返却まで spec どおり。ガードは `let _guard` で tick パス全体を通じて保持される。
- **未配線アーム**: `Cleanup` / `Observe`(Running)/ `Advance`(Completed)/ `Notify`(Stopped)/ `Wait` はいずれも空アーム。`tick_scan.rs:143` が「書き込まない・worktree に触れない・run ファイルに触れない・ラッパーを起動しない」の4点を4状態すべてに対して主張しており、`match` はワイルドカードなしの網羅。
- **手続きA**: worktree確保 → テンプレート展開5段 → `record_launching` → `save` → `prepare_attempt` → `spawn_wrapper` の順序が固定。`record_launching` 以降の失敗(`prepare_attempt` の `Io`・`spawn_wrapper` の同期エラー)は状態もカウンタも変えず報告のみで、`tick_launch.rs:336` / `:366` が保存後の実行状態が `Launching` のままであることと `spawn_fail_count` が 0 のままであることを主張している。展開失敗は `record_spawn_failure_in_place` で attempt を採番せず実行状態も変えない(`tick_launch.rs:211` が5経路すべてを回している)。
- **手続きC**: 冒頭の `current_attempt` None 検査 → `read_pid_file` / `read_starttime`(`RunFileError` は報告してスキップ・書き込みゼロ)→ `classify` 3分岐 → `SuspectSpawnFailure` で `write_invalidation_marker`(`Err(Io)` は状態を変更せず報告)→ 再読 → `classify_recheck` 2分岐。猶予境界(30 / 31 / 巻き戻り)と `Corrupt` の継続滞留・削除後のマーカー経路への合流まで消化されている。
- **二重起動の競合窓**: ラッパーは pid 書き込み後にマーカーを確認し(`run_wrapper.rs:68`、`Ok(true) | Err(Io)` の両方で `Suppressed`)、tick はマーカー書き込み後に pid を再確認する(`confirm_spawn.rs:78` → `:91`)。pending 復帰はマーカー書き込みが成功した後の再読で pid が不在だった場合に限られ、その時点以降にラッパーが pid を書いてもマーカー確認で起動を止める。マーカー書き込みの失敗で pending に戻す経路は存在しない。書き込み順序は両側ともダブルの `calls()` で並びとして主張されている(`run_wrapper.rs:93`、`tick_confirm_spawn.rs:163`)。
- **書き込みを行った tick の報告**: タスクファイルへ `save` した全経路がサマリーのいずれかを埋める。`confirm_workspace` の保存だけで終わる tick が無いことは `tick_launch.rs:186` が明示的に主張し、`commit` の失敗は必ず `SaveFailed` を積む。`Freeze` を遷移の呼び出し側から渡す設計により、既に凍結済みのタスクを別理由で保存する将来の経路(#3 の catch-up 通知)が `frozen` を再計上しないことも構成として担保されている。
- **サマリーの語義**: `confirmed_running` の追加と `errors` の構造化・`SpawnNotObserved` の分離は、いずれも `.thread/2/progress.md` の「spec 追従の提起」に記録済み。表示側は「失敗を記録 / 起動の結果が未確定 / スキップ」を網羅 `match` で振り分けており、書き込みの有無と見出しの語義が食い違わない。
- **冪等性**: `tick_scan.rs:297`(連続実行で書き込みゼロ)、`tick_launch.rs:549`(worktree 作成直後のクラッシュから同じワークスペースが再導出される)、`tick_confirm_spawn.rs:387`(破損の継続で報告とスキップだけが続く)で永続化された事実からの再導出が主張されている。
- **判断と副作用の分離**: 分類は `LaunchingClassifier`、遷移は `Task` の遷移関数に閉じ、ユースケースは観測 → 判断 → 実行の配線のみ。`Freeze::of_recorded_failure` は遷移結果の集計先を決めるだけで、遷移の判断は行っていない。
- **テストの実効性**: `ScriptedRunStore` / `ScriptedProcessController` / `ScriptedTaskRepository` / `ScriptedWorktreeManager` はいずれも台本を使い切った呼び出しで `panic!` する。既定値を返して素通りする経路が無いため、余分な呼び出し・呼び出し漏れが緑にならない。`tick_scan` / `tick_launch` / `tick_confirm_spawn` / `run_wrapper` の46件をこのレビュー中に実行し全緑を確認した。
- **残す必要のない記述**: 修正の経緯・弁明・TODO・仮実装の痕跡は、ユースケース層・CLI 層・関連テストのいずれにも無い。

### カバレッジ

- 確認(33): `.thread/2/adr.md`, `.thread/2/plan.md`, `.thread/2/progress.md`, `.thread/2/steps.md`, `crates/pulsen-conformance/src/doubles/clock.rs`, `crates/pulsen-conformance/src/doubles/process.rs`, `crates/pulsen-conformance/src/doubles/run_store.rs`, `crates/pulsen-conformance/src/doubles/task_repository.rs`, `crates/pulsen-conformance/src/doubles/worktree.rs`, `crates/pulsen-domain/src/execution/launching.rs`, `crates/pulsen-domain/src/execution/port.rs`, `crates/pulsen-domain/src/task/task.rs`, `crates/pulsen/src/application/mod.rs`, `crates/pulsen/src/application/run_wrapper.rs`, `crates/pulsen/src/application/tick/confirm_spawn.rs`, `crates/pulsen/src/application/tick/launch.rs`, `crates/pulsen/src/application/tick/mod.rs`, `crates/pulsen/src/cli/add.rs`, `crates/pulsen/src/cli/args.rs`, `crates/pulsen/src/cli/mod.rs`, `crates/pulsen/src/cli/render.rs`, `crates/pulsen/src/cli/tick.rs`, `crates/pulsen/src/cli/wire.rs`, `crates/pulsen/src/cli/wrapper.rs`, `crates/pulsen/tests/cli_tick.rs`, `crates/pulsen/tests/cli_tick_missing_cwd.rs`, `crates/pulsen/tests/cli_usage.rs`, `crates/pulsen/tests/cli_wrapper.rs`, `crates/pulsen/tests/run_wrapper.rs`, `crates/pulsen/tests/tick_confirm_spawn.rs`, `crates/pulsen/tests/tick_fixture/mod.rs`, `crates/pulsen/tests/tick_launch.rs`, `crates/pulsen/tests/tick_scan.rs`
- スキップ(56):
  - `.adr/027-port-conformance-suite-and-harness-hooks.md` — 適合ハーネスのフック規約。ポート適合の観点
  - `.thread/2/review/review-001-adapter.md`, `.thread/2/review/review-001-architecture.md`, `.thread/2/review/review-001-domain.md`, `.thread/2/review/review-001-test.md`, `.thread/2/review/review-001-usecase.md`, `.thread/2/review/review-001.md`, `.thread/2/review/review-002-adapter.md`, `.thread/2/review/review-002-architecture.md`, `.thread/2/review/review-002-domain.md`, `.thread/2/review/review-002-test.md`, `.thread/2/review/review-002-usecase.md`, `.thread/2/review/review-003-adapter.md`, `.thread/2/review/review-003-architecture.md`, `.thread/2/review/review-003-domain.md`, `.thread/2/review/review-003-test.md`, `.thread/2/review/review-003-usecase.md`, `.thread/2/review/review-003.md`, `.thread/2/review/review-004-adapter.md`, `.thread/2/review/review-004-architecture.md`, `.thread/2/review/review-004-domain.md`, `.thread/2/review/review-004-test.md`, `.thread/2/review/review-004-usecase.md`, `.thread/2/review/review-004.md`, `.thread/2/review/triage.md` — 過去のレビュー成果物。ゼロベースで見る指示により読まない
  - `.thread/2/testing.md` — 手動確認の手順書。テスト観点
  - `crates/pulsen-conformance/HOOKS.md` — 適合スイートの対応表。ポート適合の観点
  - `crates/pulsen-conformance/src/doubles/mod.rs`, `crates/pulsen-conformance/src/lib.rs` — 再公開のみ
  - `crates/pulsen-conformance/src/doubles/tests.rs` — ダブル自身のテスト。テスト観点
  - `crates/pulsen-conformance/src/process_controller.rs`, `crates/pulsen-conformance/src/run_store.rs`, `crates/pulsen-conformance/src/worktree_manager.rs` — ポート適合スイート。アダプター観点
  - `crates/pulsen-domain/src/definition/agent.rs`, `crates/pulsen-domain/src/definition/template.rs`, `crates/pulsen-domain/src/execution/mod.rs`, `crates/pulsen-domain/src/execution/value.rs`, `crates/pulsen-domain/src/task/attempt.rs`, `crates/pulsen-domain/src/task/counters.rs`, `crates/pulsen-domain/src/task/failure.rs`, `crates/pulsen-domain/src/task/mod.rs`, `crates/pulsen-domain/src/task/path.rs`, `crates/pulsen-domain/src/task/planner.rs`, `crates/pulsen-domain/src/task/transition.rs` — ドメインのモデルと遷移。ドメイン観点(ユースケースが依存する契約は `port.rs` / `task.rs` / `launching.rs` で確認済み)
  - `crates/pulsen/examples/agent_probe.rs`, `crates/pulsen/examples/spawn_probe.rs` — テスト用フィクスチャ。テスト観点
  - `crates/pulsen/src/adapter/mod.rs`, `crates/pulsen/src/adapter/process.rs`, `crates/pulsen/src/adapter/run_store.rs`, `crates/pulsen/src/adapter/worktree.rs` — ポート実装。アダプター観点
  - `crates/pulsen/tests/common/git.rs`, `crates/pulsen/tests/common/mod.rs` — 受け入れテストのハーネス。テスト観点
  - `crates/pulsen/tests/conformance_process_controller.rs`, `crates/pulsen/tests/conformance_run_store.rs`, `crates/pulsen/tests/conformance_worktree.rs` — 適合スイートの適用。アダプター観点
  - `crates/pulsen/tests/register_task.rs` — `add` の受け入れ。本スライスの変更は結線の移動のみで、`cli/add.rs` / `cli/wire.rs` 側で確認済み
