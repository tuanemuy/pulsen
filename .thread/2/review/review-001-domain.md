# レビュー 001 — Domain

対象: PR #11(`issue/2/tick-agent-run-launch` / base `main`)
契約: `.thread/2/plan.md`(AC-2 / AC-3 / AC-4 / AC-5 / AC-6 を中心に検証)

## Domain

### Blockers

なし。

`cargo test -p pulsen-domain` は 216 件すべて通る。`crates/pulsen-domain/Cargo.toml` の `[dependencies]` は空のまま、`unsafe_code = "forbid"` も維持されている。ターゲット述語つき `cfg`(`unix` / `windows` / `target_os` / `target_family`)は `crates/pulsen-domain/` に1件も無く、`crates/pulsen/src/` 側のヒットも `util/atomic.rs` と `adapter/process.rs` の2ファイルに閉じている(AC-1 の grep 条件を満たす)。ドメイン層に `Rc` / `RefCell` / `Cell` / `Arc` / `Mutex` は無く、値はすべて所有データで `Send` を満たす。`match` のワイルドカードは本 PR で1件も増えていない(既存の5件はいずれも本 PR 対象外のファイル)。スコープ外の要素(`RunningClassifier` / `JudgementService` / `IdentityCheck` / `GcPolicy` / `Task::{complete_run, skip_run, fail_run, record_judge_failure, advance, abort, retry, set_status, mark_notified}` / `attempt_exists` / `list_runs` / `delete_attempt` / `remove_task_dir_if_empty` / `starttime_of` / `kill` / `try_kill_remnants` / `remove`)は宣言もスタブも一切入っていない。

### Warnings

- **[W-001]** `RunStore` の port doc が「write 系はいずれも書き込み先のディレクトリを必要に応じて作る」を**契約**として宣言しているが、適合スイートに対応するケースが1件も無い
  - 場所: `crates/pulsen-domain/src/execution/port.rs:56-58`(doc の該当箇条)、`crates/pulsen-conformance/src/run_store.rs`(対応ケース不在)、`crates/pulsen/src/application/tick/launch.rs:52-60`(この契約に依存する経路)
  - 理由: `spec/domains/execution.md` のポート表がディレクトリ作成を明記しているのは `write_invalidation_marker` の1メソッドだけで、`write_starttime` / `write_pid_file` / `write_exit` には無い。本 PR はこれを ADR-008 の判断としてポート契約に**格上げ**し、`launch.rs` は「`prepare_attempt` が失敗しても spawn は続ける — ラッパーの write 系が自分でディレクトリを作るから」という理由でその契約に依存している。ところが適合スイートで検証されるのは `TC-port-run-store-018`(マーカーのディレクトリごと作成)だけで、`write_starttime` 系のディレクトリ作成を主張するケースは無く、`FsRunStore` のユニットテストにも無い(実装は `util::atomic::write_atomic` の `ensure_dir` に暗黙に乗っているだけ)。契約を満たさない2つ目の RunStore 実装が適合スイートを緑のまま通り、`prepare_attempt` 失敗後の自己修復が黙って壊れる。「テストでは実アダプターを差し替えられることを設計の健全性の指標とする」(CLAUDE.md)に照らして、契約と検証の間に穴がある。
  - 提案: 台帳行に対応しない追加ケースを1件置く(AC-8 が WorktreeManager で ADR-013 のために同じことをしている前例がある) — 「attempt ディレクトリが無い状態で `write_starttime` を呼ぶと `Ok` になり、直後の `read_starttime` が同じ値を返す」。あわせて `.thread/2/progress.md` の「spec 追従の提起」に本件(spec が規定していない write 系の契約追加)を1行足す。現在この一覧には `CommandLine::rehydrate` / `RunDirPath::state_root` / wrapper の終了コード / tick の `errors` の4件しか無く、同種の「spec が規定していない箇所」であるこの契約が漏れている。

- **[W-002]** attempt 番号が遷移関数の外で再導出され、「番号と run ディレクトリの整合を構成で保証する」というドメイン側の設計が呼び出し側で分岐している
  - 場所: `crates/pulsen/src/application/tick/launch.rs:41`(`let number = task.next_attempt_number();`)と `:54`(`self.runs.prepare_attempt(&id, number)`)
  - 理由: `AttemptRef::launching` は `pub(super)` に閉じられ、doc は「番号と run ディレクトリを1つの生成口で同時に受け取り、両者の食い違いを構成で排除する」と述べている。`record_launching` も同じ意図で `run_dir` を戻り値として返している。ところが呼び出し側は採番を `next_attempt_number()` でもう一度自前に行い、その値を `prepare_attempt` に渡している。現状は同じ純粋関数を同じタスクに適用しているので値は必ず一致し、`tick_launch.rs:232` の受け入れテストも番号 2 で一致を主張しているため**今は不具合ではない**。しかし `record_launching` の採番規則が変わった瞬間(番号を飛ばす・リセットするなど)、`prepare_attempt` だけが別の attempt ディレクトリを作り、タスクファイルの `run_dir` と食い違う。構成で排除したはずの食い違いが呼び出し側に復活している。
  - 提案: `record_launching` の戻り値から取る。`recorded.current_attempt()` の番号を使うか、`(Task, RunDirPath)` に番号も含める(あるいは `RunDirPath` だけで `prepare_attempt` を呼べる形にする)。導出点をドメインの1箇所に戻す。

- **[W-003]** `record_launching` を「まだ attempt が無いタスク」に適用する経路のドメインユニットテストが無い
  - 場所: `crates/pulsen-domain/src/task/task.rs:833-871`(`起動記録は次の番号を採番し番号どおりのrunディレクトリを導出する` / `起動記録は起動待ちと失敗確定からのみ行える`)
  - 理由: 成功経路のテストはいずれも `current_attempt: Some(attempt(1))` から始まり、番号 2 への採番だけを主張している。`current_attempt = None`(= すべてのタスクの初回起動)から `AttemptNumber::FIRST` と `attempt-1` の run ディレクトリが導出されることをドメイン層で主張しているテストは無く、`次のattempt番号は現在attemptがなければ最初の番号になる`(`next_attempt_number` 単体)と、ユースケース層の `ワークスペース未確定のタスクは導出したworktreeを確保してから起動する` からの間接的な裏付けに頼っている。plan.md「テスト方針」は遷移6種の事後条件をドメインのユニットテストで網羅すると定めており、主経路が抜けている。
  - 提案: `起動記録は次の番号を採番し...` を `current_attempt` 有無の2ケースでループさせる(None → `AttemptNumber::FIRST` と `RunDirPath::derive(.., 1)`、Some(1) → 2)。

- **[W-004]** `RunDirPath::state_root` の否定ケースに、コメントが根拠として名指ししている `attempt-+1` が無い
  - 場所: `crates/pulsen-domain/src/task/path.rs:153-155`(コメント)と `:330-345`(`規定の形でないrunディレクトリからはstateルートを復元しない`)
  - 理由: コメントは「番号の表記ゆれ(`attempt-01` / `attempt-+1`)は数値としては読めるが、`derive` はその値を出力しない」と2つの例を挙げ、ADR-015 も同じ2例を挙げている。テストの一覧には `attempt-01` はあるが `attempt-+1` が無い。Rust の `u32::from_str` は先頭の `+` を受理する(`"+1".parse::<u32>() == Ok(1)`)ため、この入力は `AttemptNumber::parse` を通過して `derive` との一致検査だけが弾いている。一致検査を外す・緩めるリファクタリングが入ってもテストは緑のままになる。
  - 提案: 否定ケースの配列に `vec!["state", "runs", "20260811t091530-k3f9qa1b", "attempt-+1"]` を足す。

- **[W-005]** AC-4 が要求する問い合わせ `execution_kind` に本番の呼び出し側が1つも無く、ユースケースが同じ判別を手書きの `match` で再実装している
  - 場所: `crates/pulsen/src/application/tick/mod.rs:373-382`(`fn is_stopped`)、`crates/pulsen-domain/src/task/task.rs:182-185`(`execution_kind`)
  - 理由: `Task::execution_kind()`(DOM-task-048)はテスト以外から呼ばれておらず、一方で tick 側は `is_stopped` として `ExecutionState` の全6値に対する `match` を自前で書いている。網羅 `match` なので誤りは無く、`ExecutionState` に変種が増えればコンパイルエラーになる点も規約どおり。ただし「実行状態の判別子で判断する」というドメインの問い合わせ口が用意されている以上、判別のロジックがユースケースに重複して置かれている状態になる。
  - 提案: `is_stopped` を `task.execution_kind() == ExecutionStateKind::Stopped` に寄せる(ドメインの問い合わせ口を本番経路で使い、判別の定義箇所を1つにする)。`is_wait` / `is_cleanup` も本番の呼び出しは無いが、これらは Issue の台帳行(DOM-task-051)と AC-4 が明示的に要求しているため落とす対象ではない。

### 確認した内容(所見)

規約・設計への適合は総じて良い。以下は特筆点。

- **関数型ドメインモデリング**: `PidFileContent` / `WrapperLaunchSpec` / `WrapperIdentity` / `InconsistentRunFiles` はいずれもフィールド非公開 + アクセサ。`ExitCode` はタプル構造体の非公開フィールドで、検証対象の無い値なので `new` を持つ(既存の `Pid::new` と同型)。OR は `LaunchingDecision` / `LaunchingRecheck` / `TransitionError` / `RunFileError` / `Io` / `SpawnError` / `WorktreeError` の enum、AND は struct で表現されている。
- **parse, don't validate**: `cli/wrapper.rs:40-43` が `RunDirPath::parse` / `WorktreePath::parse` / `CommandLine::rehydrate` を通す唯一の parse 境界になっており、以降は型で保証された値だけが流れる。`RunDirPath::state_root` が `derive` との往復一致を受理条件にしている点(ADR-015)は、逆写像の像を `derive` の像に従属させる正しい設計。
- **エラーは値**: ドメインのパニックは `current_status_def`(不変条件1)・`WorkspacePlanner::derive`(TaskId の文字集合が保証)・`launch.rs` の2箇所(分岐が構成で保証)に限られ、いずれも不変条件違反。`FailureNote::record` は空メッセージを既定文言に畳んで遷移関数に失敗経路を増やさない(`.adr/036` の「既定値を与える総関数は使ってよい」に沿う)。
- **カウンタ規則**: `limit_exceeded(count, limit) = count > limit` が1箇所に集約され、3つの遷移が共有している。等号で凍結しないこと・超過で凍結することが `record_spawn_failure` / `record_spawn_failure_in_place` / `record_tool_failure` の3組すべてでテストされている。`confirm_running` は `spawn_fail_count` だけをリセットし `RetryCounters::rehydrate(1, 2, 0)` で主張済み。`record_spawn_failure_in_place` は実行状態も attempt 番号も変えないことを Pending / Failed の両方で主張済み。
- **`LaunchingClassifier`**: 分岐は `(pid, starttime)` の網羅 `match`。境界は 0 / 29 / 30 → `KeepWaiting`、31 → `SuspectSpawnFailure`、-3600 → `KeepWaiting` が主張され、`Timestamp::elapsed_since` の飽和で巻き戻りが 0 になる。`starttime` のみの中間状態が猶予判断へ合流すること・`classify_recheck` が starttime の有無を問わず `SpawnFailed` になることも独立したテストがある。ユースケース層(`tick_confirm_spawn.rs:110`)でも `[(30, false), (31, true), (-3600, false)]` として同じ境界を差し替え `Clock` で再確認している。
- **層の分離**: レイアウトの知識(`runs` / `attempt-` / `pid` / `starttime` / `exit` / `stdout.log` / `stderr.log` / `invalidated`)はすべて `task/path.rs` の定数に閉じており、`crates/pulsen/src/` 側でこれらの文字列が現れるのはテストの中だけ。ADR-018 に従い `cli/render.rs` の private な `agent_def_error` / `template_error` が削除され、ドメインの `describe()` に一本化されている(文言は同一)。ユースケース(`tick/launch.rs`・`tick/confirm_spawn.rs`・`run_wrapper.rs`)はいずれも「ポートで観測 → ドメインで判断 → ポートで実行」の配線に徹しており、判断の再実装は W-005 の1件を除いて見当たらない。
- **ポートの1:1**(AC-6): `RunStore` 9メソッド・`ProcessController` 3メソッド・`WorktreeManager` に `create` 1つ、付随する `WrapperLaunchSpec` / `WrapperIdentity` / `SpawnError` / `WorktreeError` / `RunFileError` が spec の表と一致。`Io` の共有(ADR-014)には「分類に使わない報告用に限る」という条件が doc に添えられている。
- **弁明・経緯コメント**: ドメイン層に指摘への弁明・修正履歴・TODO / FIXME・「暫定」の類は1件も無い。残っているコメントはドキュメンテーションと why / why not のみ。

### カバレッジ

一覧は 62 行(タスク文の「61ファイル」と1件ずれる。`changed-files.txt` の実行数は 62)。確認 32 + スキップ 30 = 62。

確認:

- `.thread/2/plan.md`
- `.thread/2/adr.md`
- `.thread/2/progress.md`
- `.thread/2/steps.md`
- `crates/pulsen-domain/src/definition/agent.rs`
- `crates/pulsen-domain/src/definition/template.rs`
- `crates/pulsen-domain/src/execution/launching.rs`
- `crates/pulsen-domain/src/execution/mod.rs`
- `crates/pulsen-domain/src/execution/port.rs`
- `crates/pulsen-domain/src/execution/value.rs`
- `crates/pulsen-domain/src/task/attempt.rs`
- `crates/pulsen-domain/src/task/counters.rs`
- `crates/pulsen-domain/src/task/failure.rs`
- `crates/pulsen-domain/src/task/mod.rs`
- `crates/pulsen-domain/src/task/path.rs`
- `crates/pulsen-domain/src/task/planner.rs`
- `crates/pulsen-domain/src/task/task.rs`
- `crates/pulsen-domain/src/task/transition.rs`
- `crates/pulsen/src/application/tick/mod.rs`
- `crates/pulsen/src/application/tick/launch.rs`
- `crates/pulsen/src/application/tick/confirm_spawn.rs`
- `crates/pulsen/src/application/run_wrapper.rs`
- `crates/pulsen/src/cli/wrapper.rs`
- `crates/pulsen/src/cli/wire.rs`
- `crates/pulsen/src/cli/render.rs`
- `crates/pulsen/src/adapter/run_store.rs`
- `crates/pulsen/src/adapter/process.rs`
- `crates/pulsen-conformance/src/run_store.rs`
- `crates/pulsen-conformance/src/process_controller.rs`
- `crates/pulsen-conformance/src/doubles/run_store.rs`
- `crates/pulsen-conformance/src/doubles/process.rs`
- `crates/pulsen/tests/tick_confirm_spawn.rs`

スキップ:

- `.thread/2/testing.md` — 動作確認の手順書。ドメインの型・遷移に関する主張を含まない
- `crates/pulsen-conformance/HOOKS.md` — 適合ハーネスの対応表。Test 観点
- `crates/pulsen-conformance/src/lib.rs`, `crates/pulsen-conformance/src/worktree_manager.rs`, `crates/pulsen-conformance/src/doubles/{clock.rs,mod.rs,task_repository.rs,tests.rs,worktree.rs}` — 適合ハーネスとダブルの配線。ドメインのポート宣言との整合は `port.rs` 側で確認済みで、実装の中身は Test / Adapter 観点
- `crates/pulsen/examples/{agent_probe.rs,spawn_probe.rs}` — 適合テスト用のフィクスチャプログラム。ドメイン非依存
- `crates/pulsen/src/adapter/{mod.rs,worktree.rs}` — git CLI の呼び出しとパス正規化。ドメインへのレイアウト・OS 知識の漏れが無いことは grep で確認済みで、実装は Adapter 観点
- `crates/pulsen/src/application/mod.rs` — モジュール宣言のみ
- `crates/pulsen/src/cli/{args.rs,mod.rs,tick.rs}` — clap の引数定義とコマンド配線。ドメイン型への parse は `cli/wrapper.rs` 側で確認済み
- `crates/pulsen/tests/{cli_tick.rs,cli_usage.rs,cli_wrapper.rs,conformance_process_controller.rs,conformance_run_store.rs,conformance_worktree.rs,register_task.rs,run_wrapper.rs,tick_launch.rs,tick_scan.rs}`, `crates/pulsen/tests/common/{git.rs,mod.rs}`, `crates/pulsen/tests/tick_fixture/mod.rs` — 受け入れ・ユースケーステストとその足場。ドメインの遷移・分類の事後条件を裏づける範囲(`tick_launch.rs:232` の採番、`tick_confirm_spawn.rs` の猶予境界)は W-002 / W-003 の判断材料として参照したが、テスト自体の網羅性・命名は Test 観点
