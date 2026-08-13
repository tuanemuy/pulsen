# レビュー005 — Architecture / CLI

### Architecture / CLI

#### Blockers

なし

#### Warnings

- **[W-001]** `steps.md` の3箇所が、ラッパー用合成の実装（および ADR-068 の決定）と食い違ったまま残っている。

  場所: `.thread/2/steps.md` L205（設計「`cli::wire`」）、L288（ステップ10 の変更内容）、L346（ステップ17 の変更内容）

  理由: 3箇所とも「ラッパーの `SystemProcessController` は `wire::process_controller()` で組む」「`std::env::current_exe()` は `wire::process_controller()` の中だけで読み、`cli/tick.rs` と `cli/wrapper.rs` から呼ぶ」と書いている。実装は `wire::compose_wrapper` が `SystemProcessController::without_self_exe(...)` を組み（`crates/pulsen/src/cli/wire.rs:215-229`）、`cli/wrapper.rs` は `wire::process_controller()` を一切呼ばない。この形は ADR-068 の「ラッパーの合成だけは `self_exe` を持たない構成を使う」節が明示的に決めたもので、実装が正しく ADR に従っている。食い違っているのは steps.md の側だけである。

  この記述は「`current_exe()` の失敗経路がラッパーにも1本ある」と読める形になっており、実装が意図的に消した失敗経路（`current_exe()` の失敗だけでラッパーが何も書かずに非0終了し、tick の猶予経路が spawn 失敗を積む）を、後続スライスの読み手が再導入しうる。ADR-068 の 3引数固定から外れることは同 ADR の節が根拠を持って述べており、その根拠が計画側に反映されていない。

  提案: L205 / L288 / L346 の該当文を ADR-068 の後半節に合わせ、「ラッパーは `SystemProcessController::without_self_exe(identity_source, clock)` を組み、`current_exe()` を読まない」旨へ直す。あわせて L346 の「`cli/tick.rs` と（ステップ10 で作った）`cli/wrapper.rs` から呼ぶ」は「`cli/tick.rs` からのみ呼ぶ」に改める。

#### 確認した観点と結論

- **合成ルート（`cli::wire`）の切り出し**: `compose()` に残るのはホーム解決とグローバル設定の読み込みだけになり、pages「縮退状態の共通規則1」（各コマンドは自身の動作に必要なリソースだけを検証する）が `current_exe` / `current_dir` / 乱数の3つで揃った（ADR-068 / ADR-091）。切り出し先の呼び名（`Runtime::workflow_store()` / `wire::id_generator()` / `wire::process_controller()`）が「参照を返すアクセサ」ではなく「構築して `Result` を返す」ことを名前で示している点も一貫している。`WrapperRuntime` を別型にしてホーム解決と config 読み込みが紛れ込む余地を型で閉じた形（ADR-070）も、`compose` との差が「構成そのものの違い」に限られており妥当。
- **`add` の失敗経路と文言**: `compose` から外れた2つは `cli/add.rs` が `absolute_repo` の後に呼ぶ形になり、いずれもロック取得・タスク作成の前で、pages 規則4（部分的な変更を残さない）を保つ。`WireError::{CurrentDirUnavailable, IdGenerator}` と `cli::render::wire_error` の文言はそのまま残っており、相対パスのワークフロー解決（`cli_add_boundary.rs::register_by_relative_path`）・相対パスのリポジトリ（`cli_add_normal.rs::tc_task_register_task_008`）が実バイナリ経由で緑。`cli_add_normal` / `cli_add_boundary` / `cli_add_error` の64件を実行して全緑を確認した。検出順が「config → repo → cwd → 乱数」へ動くが、単一の失敗に対する文言と終了コードは変わらない。
- **未実装メソッドの宣言・スタブ**: `crates/pulsen-domain/src/execution/port.rs` の宣言は `RunStore` 9件・`ProcessController` 3件・`WorktreeManager` は既存3件 + `create` の1件で、AC-6 の除外リスト（`attempt_exists` / `list_runs` / `delete_attempt` / `remove_task_dir_if_empty` / `starttime_of` / `kill` / `try_kill_remnants` / `remove`）は1つも現れない。`todo!` / `unimplemented!` / `TODO` / `FIXME` はリポジトリ全体で0件。`#[allow(dead_code)]` はテスト間で共有する `tests/common/mod.rs` と `tests/tick_fixture/mod.rs` の2つだけ。
- **スコープ外の実装**: `Tick::dispatch` の `Cleanup` / `Observe` / `Advance` / `Notify` は網羅 `match` のアームとして空のまま置かれ、引き取り先の Issue 番号がコメントに書かれている（ADR-065）。`TickSummary` の6フィールド（`transitioned` / `skipped_back` / `notified` / `archived` / `gc_deleted` / `gc_errors`）は本スライスでは埋まらないが、spec の出力DTO の形を保つ決定（ADR-065 / ADR-086）に従ったもので、`render::tick_summary` は「値の入っている項目だけを並べる」一般規則として書かれており、フィールドごとの特別扱いを持たない。`Task::commit` に notify の呼び出し口を1箇所に集約してある形（ADR-066）も、#3 が足す位置を構成として示している。
- **終了コード**: `add` は成功0 / 失敗非0。`tick` はロック競合 = `TickOutcome::Skipped` → 0（pages exit code 規約の唯一の例外）、サマリー表示 → 0、`TickError::{LockFailed, Scan}` → 非0。`wrapper` は `Ran` / `Suppressed` → 0、`Silent` → 非0、起動引数の不正（相対パス・0トークン・形式外の run_dir）→ 非0（ADR-081、spec の1行と一致）。`cli_tick.rs` / `cli_wrapper.rs` が実バイナリでこれらを主張している。
- **`wrapper` の引数の往復**: argv の組み立て（`adapter/process.rs` の `WRAPPER_SUBCOMMAND` / `RUN_DIR_FLAG` / `WORKSPACE_FLAG` / `COMMAND_SEPARATOR`）と受理（`cli/args.rs` の `trailing_var_arg` + `allow_hyphen_values`）が同じ定数を共有し、`tests/common/mod.rs::wrapper` が同じ定数から実バイナリを起動する。`cli_wrapper.rs::シェルのメタ文字や空文字列を含むトークンはリテラルのまま渡る` が `--model` / 空文字列 / `$HOME` / `*` / `a && b` / `>out.txt` / `{input}` の往復を主張しており、定義箇所が2つに分かれることの危険が実効的に塞がれている。
- **縮退状態の共通規則との整合**: config 不在・パース不能で `tick` が非0・無変更（`cli_tick.rs` の2件が `Untouched` で worktree と runディレクトリの不作成まで主張）、ロック競合で 0 スキップ・pending のまま、パース不能タスクファイルは報告のみで書き込まず残りのタスクは起動される、スナップショット破損は報告のみで `state/runs/` も作らない、`state/tasks/` 未作成は 0 で「処理対象なし」、アーカイブ済みは走査対象外 — いずれも受け入れテストで主張されている。「読めないリソースには書き込まない」が `TickIssue::{CorruptTaskFile, SnapshotUnreadable}` の処理に書き込み経路を持たないことで構成として成立している。
- **書き込みを行った tick の報告**: ADR-084 の不変（タスクファイルへ書いた経路は必ずサマリーのいずれかを埋める）を経路ごとに追跡した。手続きA は `launched` / `CommandExpansionFailed` / `WorktreeCreateFailed` / `SpawnFailed` / `PrepareAttemptFailed` / `SaveFailed` / `Transition` のいずれかに必ず落ち、手続きC は `confirmed_running` / `SpawnNotObserved` / `MarkerWriteFailed` / `RunFileUnreadable` / `InconsistentRunFiles` / `SaveFailed` / `Transition` のいずれかに必ず落ちる。`KeepWaiting` だけが無報告だが、この経路は書き込みを一切発生させない。`commit` が `Freeze` を引数で受け取り保存後の状態から凍結を導出しない形（ADR-089）も、#3 の catch-up 通知が過去の凍結を再計上する縮退を先に潰している。`render::issue_outcome` が報告を「失敗を記録 / 起動の結果が未確定 / スキップ」の3見出しに網羅 `match` で振り分けており、分類が増えたときに振り分け先を決めないとコンパイルが通らない。
- **標準出力と標準エラー**: 成功の出力（`registered` / `tick_skipped` / `tick_summary`）は `println!`、失敗（`add_error` / `tick_error` / `wrapper_error`）は `eprintln!`。tick のサマリーは `errors` を含んでも成功出力なので標準出力に出る — cron の登録行（`spec/manual-tests/setup.md` TC-06 手順2）が `>> cron.log 2>&1` で両方を拾うため、運用上の窓も塞がらない。`wrapper` は成功時に標準出力へ何も出さないこと（`cli_wrapper.rs` が `run.stdout.is_empty()` を主張）で、pages「結果はすべてrunディレクトリのファイルとして現れる」を満たす。
- **ADR 参照の解決**: コードと `.thread/2/*.md`（review/ を除く）が参照する ADR 番号は 009 / 010 / 013〜016 / 019〜021 / 023〜033 / 036 / 037 / 042 / 043 / 048 / 050 / 053 / 055 / 060〜063 と 065〜079 / 081 / 083 / 084 / 086〜088 / 090 / 091。前半はすべて `.adr/` に、後半はすべて `.thread/2/adr.md` に解決する。`.adr/` の最大が 064、`adr.md` が 065 始まりで番号の衝突は無い。`.adr/` の欠番（041 / 047 / 056〜059）はいずれも参照されていないため dangling も無い。`.adr/027` の変更は「対応表の正本を `HOOKS.md` 一本にする」という根拠つきの改訂で、恒久の決定記録を後続スライスのたびに書き換え続ける形を解消しており妥当。
- **呼び出しの無い `pub`**: `Runtime::state_root()` / `worktree_root()` は ADR-061 の「必要になったスライスで理由つきで戻す」に沿って why つきで復活し、`cli/tick.rs` が実際に呼んでいる。`adapter::process` の4定数は統合テストから見えるために `pub` で、「CLI のパーサと定義箇所が分かれるため、往復テストで受理を主張する」という why が添えられている。`WrapperOutcome::Ran(ExitCode)` のペイロードは CLI では捨てられるが `tests/run_wrapper.rs` が終了コードの透過を主張するのに使う。`TickSummary` の未使用フィールドは ADR-065 / ADR-086 が理由を持つ。呼び出しも why も無い `pub` は見つからなかった。
- **依存方向とレイヤーの責務**: `pulsen-domain` の `[dependencies]` は空のまま、`unsafe_code = "forbid"` も維持。ターゲット述語つき `cfg` は `crates/pulsen-domain/` に0件、`crates/pulsen/src/` は `util/atomic.rs` / `adapter/process.rs` / `adapter/task_repository.rs`（`#[cfg(all(test, unix))]`）の3ファイルのみで AC-1 を満たす（`crates/pulsen-conformance/src/lib.rs` の2件は権限操作の能力プローブで、AC-1 の対象外かつテスト支援クレートに閉じている）。`Tick` / `RunWrapper` はポートをジェネリック引数で受け取り、アダプター型を一切知らない。文言の組み立てはすべて `cli::render` にあり、ユースケースは分類だけを返す（ADR-073 / ADR-088）。`FsRunStore` はレイアウトの知識を持たず `RunDirPath::derive` に従う。
- **`.thread/2/` と実装の食い違い**: `plan.md` の AC-1 / AC-6 / AC-11 / AC-12 / AC-18、スコープの「含まれないもの」はいずれも実装と一致した。`progress.md` の「spec 追従の提起」10件も実装の逸脱と1対1に対応している。食い違いは W-001 の `steps.md` 3箇所のみ。
- **残す必要のない記述**: 読んだ CLI / アプリケーション層のコメントはすべて why / why not か普遍的な仕様の説明で、修正の経緯・指摘への弁明・自明な言い換えは見つからなかった。
- **ビルドとテスト**: `cargo fmt --check` と `cargo clippy --workspace --all-targets -- -D warnings` が緑。`cli_usage` / `cli_tick` / `cli_tick_missing_cwd` / `cli_wrapper` / `register_task` / `cli_add_normal` / `cli_add_boundary` / `cli_add_error` を実行して全緑。作業ツリーは本レビューファイル以外に変更なし。

#### カバレッジ

確認（36）:
`.adr/027-port-conformance-suite-and-harness-hooks.md`,
`.thread/2/adr.md`,
`.thread/2/plan.md`,
`.thread/2/progress.md`,
`.thread/2/steps.md`,
`.thread/2/testing.md`,
`crates/pulsen-conformance/src/lib.rs`,
`crates/pulsen-domain/src/execution/launching.rs`,
`crates/pulsen-domain/src/execution/port.rs`,
`crates/pulsen-domain/src/execution/value.rs`,
`crates/pulsen-domain/src/task/planner.rs`,
`crates/pulsen-domain/src/task/transition.rs`,
`crates/pulsen/examples/agent_probe.rs`,
`crates/pulsen/examples/spawn_probe.rs`,
`crates/pulsen/src/adapter/mod.rs`,
`crates/pulsen/src/adapter/process.rs`,
`crates/pulsen/src/adapter/run_store.rs`,
`crates/pulsen/src/application/mod.rs`,
`crates/pulsen/src/application/run_wrapper.rs`,
`crates/pulsen/src/application/tick/confirm_spawn.rs`,
`crates/pulsen/src/application/tick/launch.rs`,
`crates/pulsen/src/application/tick/mod.rs`,
`crates/pulsen/src/cli/add.rs`,
`crates/pulsen/src/cli/args.rs`,
`crates/pulsen/src/cli/mod.rs`,
`crates/pulsen/src/cli/render.rs`,
`crates/pulsen/src/cli/tick.rs`,
`crates/pulsen/src/cli/wire.rs`,
`crates/pulsen/src/cli/wrapper.rs`,
`crates/pulsen/tests/cli_tick.rs`,
`crates/pulsen/tests/cli_tick_missing_cwd.rs`,
`crates/pulsen/tests/cli_usage.rs`,
`crates/pulsen/tests/cli_wrapper.rs`,
`crates/pulsen/tests/common/mod.rs`,
`crates/pulsen/tests/register_task.rs`,
`crates/pulsen/tests/run_wrapper.rs`

スキップ（53）:

- `.thread/2/review/review-001-adapter.md`, `.thread/2/review/review-001-architecture.md`, `.thread/2/review/review-001-domain.md`, `.thread/2/review/review-001-test.md`, `.thread/2/review/review-001-usecase.md`, `.thread/2/review/review-001.md`, `.thread/2/review/review-002-adapter.md`, `.thread/2/review/review-002-architecture.md`, `.thread/2/review/review-002-domain.md`, `.thread/2/review/review-002-test.md`, `.thread/2/review/review-002-usecase.md`, `.thread/2/review/review-003-adapter.md`, `.thread/2/review/review-003-architecture.md`, `.thread/2/review/review-003-domain.md`, `.thread/2/review/review-003-test.md`, `.thread/2/review/review-003-usecase.md`, `.thread/2/review/review-003.md`, `.thread/2/review/review-004-adapter.md`, `.thread/2/review/review-004-architecture.md`, `.thread/2/review/review-004-domain.md`, `.thread/2/review/review-004-test.md`, `.thread/2/review/review-004-usecase.md`, `.thread/2/review/review-004.md`, `.thread/2/review/triage.md` — 過去のレビュー成果物。ゼロベースで見るため読まない指示。
- `crates/pulsen-conformance/HOOKS.md`, `crates/pulsen-conformance/src/doubles/clock.rs`, `crates/pulsen-conformance/src/doubles/mod.rs`, `crates/pulsen-conformance/src/doubles/process.rs`, `crates/pulsen-conformance/src/doubles/run_store.rs`, `crates/pulsen-conformance/src/doubles/task_repository.rs`, `crates/pulsen-conformance/src/doubles/tests.rs`, `crates/pulsen-conformance/src/doubles/worktree.rs`, `crates/pulsen-conformance/src/process_controller.rs`, `crates/pulsen-conformance/src/run_store.rs`, `crates/pulsen-conformance/src/worktree_manager.rs` — 適合スイートとテストダブルの中身。Adapter / Test 観点の担当。
- `crates/pulsen-domain/src/definition/agent.rs`, `crates/pulsen-domain/src/definition/template.rs`, `crates/pulsen-domain/src/execution/mod.rs`, `crates/pulsen-domain/src/task/attempt.rs`, `crates/pulsen-domain/src/task/counters.rs`, `crates/pulsen-domain/src/task/failure.rs`, `crates/pulsen-domain/src/task/mod.rs`, `crates/pulsen-domain/src/task/path.rs`, `crates/pulsen-domain/src/task/task.rs` — ドメイン型の内部設計。Domain 観点の担当（依存方向と外部クレート不使用は別途確認済み）。
- `crates/pulsen/src/adapter/worktree.rs` — git CLI へのシェルアウトと worktree 同定の詳細。Adapter 観点の担当。
- `crates/pulsen/tests/common/git.rs` — git フィクスチャ。Test 観点の担当。
- `crates/pulsen/tests/conformance_process_controller.rs`, `crates/pulsen/tests/conformance_run_store.rs`, `crates/pulsen/tests/conformance_worktree.rs` — 適合スイートの適用。Adapter / Test 観点の担当。
- `crates/pulsen/tests/tick_confirm_spawn.rs`, `crates/pulsen/tests/tick_fixture/mod.rs`, `crates/pulsen/tests/tick_launch.rs`, `crates/pulsen/tests/tick_scan.rs` — ダブルに対するユースケーステスト。Usecase / Test 観点の担当。

確認36 + スキップ53 = 89。
