# レビュー 005 — Domain

## Domain

### Blockers

なし

### Warnings

なし

**問題なし。** 契約(`.thread/2/plan.md` AC-2〜AC-6)と `spec/domains/{task,execution,definition}.md`、CLAUDE.md の関数型ドメインモデリング方針に照らして、ドメイン層に是正を要する点は見つからなかった。以下は確認した内容。

### 確認した事実

**AC-2(execution の値・`RunDirPath` の導出)**

- `ExitCode`(`new` / `get` / `is_success`)と `PidFileContent`(`pid` / `kill_ident`)が I/O なしの値として実装され、`ExitCode` は数値の一致で等価になる。
- `RunDirPath` の6導出(`pid_file` / `starttime_file` / `exit_file` / `stdout_log` / `stderr_log` / `marker_file`)がファイル名定数から純粋に導出され、ユニットテストが `derive` の結果との相対パスで固定している。ファイル名の文字列がドメインの1箇所(`RunDirPath` の associated const)にしかなく、アダプター(`FsRunStore`)もラッパーもここを通る。
- `RunDirPath::state_root` は `derive` との一致を最終条件にしており、`attempt-01` / `attempt-+1`(どちらも `u32` としては読める)を弾く。逆写像の全域性が「読めること」ではなく「`derive` が出力する値であること」で定義されていて、表記ゆれからの復元が起きない。

**AC-3(`LaunchingClassifier`)**

- `classify` / `classify_recheck` とも `(pid, starttime)` の4組合せを網羅 `match` で書き、ワイルドカードが無い。`(Some, None)` だけが `Err(InconsistentRunFiles::MissingStartTime)` に落ち、`(None, Some)` は猶予経路に合流する — spec の「starttime のみは書き込み順序の正常な中間状態」と一致する。
- 境界は経過 30秒 = `KeepWaiting` / 31秒 = `SuspectSpawnFailure` / 巻き戻り(負)= `KeepWaiting` の3件がユニットテストで直接主張されている。飽和は `Timestamp::elapsed_since` 側(既存)で閉じており、分類器が符号を扱わない。
- `GRACE_PERIOD` はドメインの定数として1箇所にあり、外側(ユースケース・アダプター・テスト)に 30 の直値が複製されていない(grep 済み)。
- `InconsistentRunFiles` が message を持たない分類になっており、文言は `cli::render` が組み立てる(ADR-073)。spec の `{ message: String }` との差は ADR に spec 追従として積まれている。

**AC-4(問い合わせ5種・`WorkspacePlanner`)**

- `current_status_def` / `is_agent_run` / `is_wait` / `is_cleanup` / `applicable_retry_limit` はいずれも `StatusDefinition` の3値を網羅 `match` で分岐する。`applicable_retry_limit` は AgentRun / Cleanup とも `snapshot.effective_retry_limit` に委譲し、Cleanup の 2 は既存の組み込みデフォルトからそのまま返る(`effective_retry_limit` が Cleanup をワークフロー既定より先に組み込みデフォルトへ落とすことを確認)。Wait は `None`。
- `next_attempt_number` は `current_attempt` が None なら `AttemptNumber::FIRST`、あれば `next()`。
- `WorkspacePlanner::derive` の2つの `expect` は成立する — `WorktreeRoot` は絶対パスで、`TaskId` の文字集合(`[a-z0-9-]`・先頭英数字・1〜64文字)は `BranchName` の禁止条件(空白・制御文字・先頭 `-`・`..`・`/` 始まり終わり・`.lock` 終わり)のいずれにも触れない。文字集合の端(`a` / `0` / `a--b` / `abc-` / 最大長)を通すテストもある。不変条件違反にのみパニックするという方針に沿う。

**AC-5(遷移6種・`TransitionError`)**

- 6種すべてが `self` を消費して新しい `Task` を返し、全経路で `updated_at = now` を更新する。`last_failure` は失敗記録の3経路でのみ上書きされ、成功時にクリアされない。
- 前提検査は `ensure_restartable`(Pending | Failed)/ `ensure_launching`(Launching)に集約され、どちらも6状態を網羅 `match` する。実行状態6値を回して前提の一致・不一致を主張するテストが4つの遷移に付いていて、`_` による取りこぼしが起きない。
- カウンタ規則: `limit_exceeded(count, limit) = count > limit` が1箇所にあり、3つの遷移が共有する。「上限と等しい回数では凍結しない」「上限を超えると凍結する」「上限0なら初回で凍結する」が個別にテストされている。`confirm_running` は `spawn_fail_count` だけを 0 にし、`attempt_count` / `judge_attempt_count` を保持することが `(1,2,3) → (1,2,0)` の形で主張されている。
- `record_launching` は番号を採番して `RunDirPath::derive` を内部で呼び、`AttemptRef::launching`(`pub(super)`)が番号と run ディレクトリを同時に受け取る唯一の生成口になっている。番号とパスの食い違いが型で書けない。
- `record_spawn_failure_in_place` は実行状態も `current_attempt` も変えず、`spawn_fail_count` だけを進める(Pending / Failed の両方でテスト済み)。
- 状態遷移でカウンタが混ざらないように、`record_tool_failure` が受け取る種別は `ToolFailureKind`(3値)に閉じられている。`SpawnFail` / `JudgeFail` を `attempt_count` で数える帳簿が型として書けない。
- `TransitionError` は5変種で、すべてテストから到達している。`MissingCurrentAttempt` は spec の `InvariantViolated { message }` を分類に置き換えたもので、ADR-088 が spec 追従として積んでいる。

**AC-6(ポート宣言)**

- `RunStore` は9メソッド、`ProcessController` は3メソッド、`WorktreeManager` は `create` の1追加のみ。`attempt_exists` / `list_runs` / `delete_attempt` / `remove_task_dir_if_empty` / `starttime_of` / `kill` / `try_kill_remnants` / `remove` はいずれも宣言されておらず、スタブも無い。`RunningClassifier` / `JudgementService` / `GcPolicy` / `IdentityCheck` / `CommandRunner` / `NotificationService` も追加されていない(スコープ外の遵守)。
- 付随する値・エラー型(`WrapperLaunchSpec` / `WrapperIdentity` / `SpawnError` / `WorktreeError` / `RunFileError` / `Io`)が spec の形と一致し、すべてフィールド非公開 + アクセサになっている。
- ポートの doc が宣言する契約のうち spec のポート表に無いもの(write 系がディレクトリを必要に応じて作る)は ADR-072 で決定され、適合スイートに追加ケースとして主張が置かれ、`progress.md` の spec 追従一覧にも載っている。

**層の分離**

- `crates/pulsen-domain/` にターゲット述語つき `cfg`(`unix` / `windows` / `target_os` / `target_family`)は1件も無い。`Cargo.toml` の `[dependencies]` も空のまま。
- ドメインに `std::fs` / `std::process` / `std::env` の使用は無い(`MAIN_SEPARATOR` の分岐はテストと既存の `reference.rs` のみ)。
- 判断はすべてドメインにある。ユースケース側(`tick/launch.rs` / `tick/confirm_spawn.rs` / `run_wrapper.rs`)は分類器と遷移関数の呼び出しと配線に徹しており、猶予時間・上限比較・attempt 採番・パス導出・ブランチ名の生成のいずれも外側で再実装されていない。`prepare_attempt` の戻り値ではなく遷移関数が返した `run_dir` で spawn する形になっていて、導出経路が2つに割れていない。
- `128+n` / `127` / `126` の符号化はアダプター(`adapter/process.rs`)に閉じ、ドメインの `ExitCode` は符号化値をそのまま保持するだけ。
- ドメインのエラー型から利用者向けの完成文言が外れており(`TransitionError` / `InconsistentRunFiles`)、`cli::render` の網羅 `match` が文言を組み立てる。永続化される失敗要因の文言だけが `describe`(ADR-082)としてドメインに1箇所ある — main に13箇所ある既存の規約と同じ形で、CLI 側の重複定義は削除されている。
- ドメイン crate のワイルドカード `match` は既存の `&str` / 数値に対するもののみで、今回追加された enum の分岐はすべて網羅列挙。
- コード・コメントに修正の経緯や指摘への弁明は無く、残っているのは why / why not(飽和加算を選ぶ理由、逆写像を `derive` の一致で閉じる理由、write 系がディレクトリを作る理由、順序の契約が二重起動を防ぐ理由)に限られる。

**テストの実効性**

- `cargo test -p pulsen-domain` = 220 passed / 0 failed、`cargo clippy -p pulsen-domain --all-targets` = 警告なし(レビュー時に実行)。
- テスト名は仕様の言葉(日本語)で書かれ、内部構造ではなく振る舞いを主張している。実行状態6値・`(pid, starttime)` の4組合せ・上限の等号/超過/0・時計の巻き戻りといった、spec がエッジとして挙げる条件が個別のケースとして落ちている。

### カバレッジ

- 確認: `.thread/2/plan.md`, `.thread/2/adr.md`, `.thread/2/steps.md`, `.thread/2/progress.md`, `crates/pulsen-domain/src/definition/agent.rs`, `crates/pulsen-domain/src/definition/template.rs`, `crates/pulsen-domain/src/execution/launching.rs`, `crates/pulsen-domain/src/execution/mod.rs`, `crates/pulsen-domain/src/execution/port.rs`, `crates/pulsen-domain/src/execution/value.rs`, `crates/pulsen-domain/src/task/attempt.rs`, `crates/pulsen-domain/src/task/counters.rs`, `crates/pulsen-domain/src/task/failure.rs`, `crates/pulsen-domain/src/task/mod.rs`, `crates/pulsen-domain/src/task/path.rs`, `crates/pulsen-domain/src/task/planner.rs`, `crates/pulsen-domain/src/task/task.rs`, `crates/pulsen-domain/src/task/transition.rs`, `crates/pulsen/src/application/mod.rs`, `crates/pulsen/src/application/run_wrapper.rs`, `crates/pulsen/src/application/tick/confirm_spawn.rs`, `crates/pulsen/src/application/tick/launch.rs`, `crates/pulsen/src/application/tick/mod.rs`, `crates/pulsen/src/adapter/mod.rs`, `crates/pulsen/src/adapter/process.rs`, `crates/pulsen/src/adapter/run_store.rs`, `crates/pulsen/src/adapter/worktree.rs`, `crates/pulsen/src/cli/add.rs`, `crates/pulsen/src/cli/render.rs`, `crates/pulsen/src/cli/tick.rs`, `crates/pulsen/src/cli/wire.rs`, `crates/pulsen/src/cli/wrapper.rs`, `crates/pulsen-conformance/src/doubles/run_store.rs`, `crates/pulsen-conformance/src/run_store.rs`(34件。ドメイン層は全量、その外側はドメインの判断・語彙・パス導出が漏れていないかの観点で確認)
- スキップ: `.adr/027-port-conformance-suite-and-harness-hooks.md` — 適合ハーネスのフック規約の文書で、ドメインの型・遷移に関わらない(adapter / test 観点)
- スキップ: `.thread/2/review/review-001-adapter.md`, `.thread/2/review/review-001-architecture.md`, `.thread/2/review/review-001-domain.md`, `.thread/2/review/review-001-test.md`, `.thread/2/review/review-001-usecase.md`, `.thread/2/review/review-001.md`, `.thread/2/review/review-002-adapter.md`, `.thread/2/review/review-002-architecture.md`, `.thread/2/review/review-002-domain.md`, `.thread/2/review/review-002-test.md`, `.thread/2/review/review-002-usecase.md`, `.thread/2/review/review-003-adapter.md`, `.thread/2/review/review-003-architecture.md`, `.thread/2/review/review-003-domain.md`, `.thread/2/review/review-003-test.md`, `.thread/2/review/review-003-usecase.md`, `.thread/2/review/review-003.md`, `.thread/2/review/review-004-adapter.md`, `.thread/2/review/review-004-architecture.md`, `.thread/2/review/review-004-domain.md`, `.thread/2/review/review-004-test.md`, `.thread/2/review/review-004-usecase.md`, `.thread/2/review/review-004.md`, `.thread/2/review/triage.md` — 過去ラウンドの成果物。ゼロベースでレビューする指示により読まない
- スキップ: `.thread/2/testing.md` — 手動確認の手順書で、ドメインの契約を規定しない
- スキップ: `crates/pulsen-conformance/HOOKS.md` — 適合ハーネスの対応表(adapter / test 観点)
- スキップ: `crates/pulsen-conformance/src/doubles/clock.rs`, `crates/pulsen-conformance/src/doubles/mod.rs`, `crates/pulsen-conformance/src/doubles/process.rs`, `crates/pulsen-conformance/src/doubles/task_repository.rs`, `crates/pulsen-conformance/src/doubles/tests.rs`, `crates/pulsen-conformance/src/doubles/worktree.rs` — 結果を注入するだけのテストダブル。ドメイン規則の再実装が無いことは grep で確認し、代表として `doubles/run_store.rs` のみ通読した
- スキップ: `crates/pulsen-conformance/src/lib.rs`, `crates/pulsen-conformance/src/process_controller.rs`, `crates/pulsen-conformance/src/worktree_manager.rs` — ポート適合スイート(adapter / test 観点)。ドメインの契約と1:1で対応しているかは、`RunStore` の追加契約を主張する `run_store.rs` の確認で代表させた
- スキップ: `crates/pulsen/examples/agent_probe.rs`, `crates/pulsen/examples/spawn_probe.rs` — 適合テスト用の外部プログラム。ドメインを参照しない
- スキップ: `crates/pulsen/src/cli/args.rs`, `crates/pulsen/src/cli/mod.rs` — 引数定義とサブコマンドの結線(CLI 観点)。ドメイン型への parse は `cli/wrapper.rs` 側で確認した
- スキップ: `crates/pulsen/tests/cli_tick.rs`, `crates/pulsen/tests/cli_tick_missing_cwd.rs`, `crates/pulsen/tests/cli_usage.rs`, `crates/pulsen/tests/cli_wrapper.rs`, `crates/pulsen/tests/common/git.rs`, `crates/pulsen/tests/common/mod.rs`, `crates/pulsen/tests/conformance_process_controller.rs`, `crates/pulsen/tests/conformance_run_store.rs`, `crates/pulsen/tests/conformance_worktree.rs`, `crates/pulsen/tests/register_task.rs`, `crates/pulsen/tests/run_wrapper.rs`, `crates/pulsen/tests/tick_confirm_spawn.rs`, `crates/pulsen/tests/tick_fixture/mod.rs`, `crates/pulsen/tests/tick_launch.rs`, `crates/pulsen/tests/tick_scan.rs` — ユースケース・受け入れ・適合の各テスト(test / usecase 観点)。ドメインの遷移と分類が実効的なテストで守られているかは、`pulsen-domain` 内のユニットテスト(220件、実行して全通過を確認)で判定した
