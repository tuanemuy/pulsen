# レビュー 002 — Domain

## Domain

### Blockers

なし

### Warnings

- **[W-001]** `record_spawn_failure` だけ前提状態の不一致がテストされていない
  - 場所: `crates/pulsen-domain/src/task/task.rs:332`(実装)/ `crates/pulsen-domain/src/task/task.rs:1030-1090`(テスト)
  - 理由: 本スライスで足した遷移のうち `record_launching` / `confirm_running` / `record_spawn_failure_in_place` / `record_tool_failure` は `every_execution_state()` を回して「どの状態から呼ぶと `InvalidState` になるか」を6状態分主張している。`record_spawn_failure` にだけこの掃引が無く、3つのテストはいずれも `Launching` からの成功経路しか通らない。前提の検査は `ensure_launching` を `confirm_running` と共有しているので現時点の実装は正しいが、`record_spawn_failure` が独自の前提(たとえば不変条件2の検査)を持った瞬間に、退行を捕まえるテストがどこにも無くなる。plan.md AC-5 は「前提状態の不一致・事後条件・カウンタ規則が網羅される」を求めており、6遷移のうち1つがこの網羅から外れている。あわせて、`record_spawn_failure` が `attempt_count` / `judge_attempt_count` を保持すること(spec の「リセットは `spawn_fail_count` だけ」と対になる事後条件)も主張されていない — `猶予超過のspawn失敗は起動待ちへ戻し失敗要因を残す` は `spawn_fail_count` しか見ていない。
  - 提案: 他の4遷移と同じ形で `猶予超過のspawn失敗は起動記録済みからのみ記録できる` を足し、`every_execution_state()` を回して `Launching` 以外が `InvalidState { expected: LAUNCHING, .. }` になることを主張する。既存の成功経路テストのアサーションを `counters()` 全体の比較(例: `RetryCounters::rehydrate(1, 2, 3)` から `rehydrate(1, 2, 4)`)に上げ、他の2カウンタが動かないことを明示する。

- **[W-002]** `is_wait` / `is_cleanup` と `WorkspacePlanner::BRANCH_PREFIX` に本番の呼び出しが無く、doc が書いている用途が実態と食い違う
  - 場所: `crates/pulsen-domain/src/task/task.rs:214-236`、`crates/pulsen-domain/src/task/planner.rs:12`
  - 理由: `is_agent_run` は `record_launching` の前提検査で使われているが、`is_wait` / `is_cleanup` の呼び出しは自身のユニットテストにしか無い。tick の分岐(`branch_of`)は `current_status_def()` に対する網羅 `match` を通しており(`crates/pulsen/src/application/tick/mod.rs:381-387`)、`is_wait` を経由しない。にもかかわらず `is_agent_run` の doc は「真偽だけが要る経路(`record_launching` の前提検査、**待機の素通し**、手続きBの終端処理)がこれらを使い」と書いており、3つ挙げた用途のうち2つは本スライスに存在しない(1つは #6)。`BRANCH_PREFIX` も `pub` だが `planner.rs` の内側と自テストからしか参照されない。ADR-061 は「呼び出しの無い `pub` は落とし、必要になったスライスで理由つきで戻す」と定めており、doc の why が現在の形を説明していない状態は、その規約を形だけ満たしている。plan.md AC-4 が問い合わせ5種の実装を要求している以上、削除ではなく doc の是正が筋になる。
  - 提案: doc から実在しない用途(「待機の素通し」)を落とし、「spec が一組で定める読み取り口で、`is_wait` / `is_cleanup` の呼び出し側は #3 / #6 が入れる」と現在の事実だけを書く。`BRANCH_PREFIX` は `pub` を落として `const` を private にするか、`pub` のままにするなら「終端処理(#6)がブランチ名の同定に使う」といった戻す理由を添える。

- **[W-003]** `record_tool_failure` の `kind` が `SpawnFail` / `JudgeFail` も受理し、カウンタ規則と矛盾する組み合わせを型で排除できていない
  - 場所: `crates/pulsen-domain/src/task/task.rs:387`(`record_tool_failure(self, kind: FailureKind, ...)`)
  - 理由: `FailureKind` は `WorktreeCreate | WorktreeRemove | ArchiveMove | SpawnFail | JudgeFail` の5値で、後ろの2つは専用の遷移(`record_spawn_failure` / `record_spawn_failure_in_place` / #3 の `record_judge_failure`)が `spawn_fail_count` / `judge_attempt_count` を進めながら記録するものである。`record_tool_failure(FailureKind::SpawnFail, ...)` は今の型で書けてしまい、書けた場合は `attempt_count` を進めつつ `SpawnFail` の失敗要因を残す — カウンタと失敗種別が食い違った帳簿が生まれ、`applicable_retry_limit` の併記(ADR-014)が実態と合わなくなる。CLAUDE.md の「不正な状態を型で表現不能にする」に照らすと、ここは表現可能なまま残っている。spec/domains/task.md の表も `kind` と書いているので spec 準拠ではあるが、spec がドメインモデリング規約に負ける理由は無い。
  - 提案: ツール操作の失敗だけを表す種別(`ToolFailureKind = WorktreeCreate | WorktreeRemove | ArchiveMove`)を足して `record_tool_failure` の引数をそれに絞り、`FailureNote::record` の中で `FailureKind` へ写す。spec 側は「`kind` はツール操作の3種に限る」の追従提起として adr.md に残す。

- **[W-004]** `InconsistentRunFiles` が spec の形から変わっているのに、ADR / spec追従の記録が無い
  - 場所: `crates/pulsen-domain/src/execution/launching.rs:39-42`
  - 理由: spec/domains/execution.md:99 は `InconsistentRunFiles { message: String }` と定めるが、実装はデータを持たない `enum InconsistentRunFiles { MissingStartTime }` になっている。モデリングとしてはこちらが正しい(文言をドメインに置かず、破れの種別だけを値にする — ADR-073 の方針と整合する)が、同種の spec 逸脱は本スライスで `CommandLine::rehydrate`(ADR-071)・`RunDirPath::state_root`(ADR-070 / 079)・`TickIssue`(ADR-073)・`Io` の共有(ADR-078)がいずれも ADR と「spec 追従の提起」で明示されているのに、この1件だけ記録が無い。次のスライスが spec の表を読んで `message: String` を前提に書くと、`classify` の呼び出し側が壊れる。
  - 提案: ADR-073 に「`LaunchingClassifier` が返す `InconsistentRunFiles` も同じ規則で分類として持つ(文言は `cli::render`)」の一文を足すか、独立した ADR を起こし、Issue のコメントの spec 追従提起の一覧に加える。

- **[W-005]** `AttemptRef::rehydrate` の doc が「新規採番は後続スライスが担う」のまま残っている
  - 場所: `crates/pulsen-domain/src/task/attempt.rs:67-73`
  - 理由: 直下に `launching(number, run_dir)` が入ったことで、「新規採番は起動記録(**後続スライス**)が担い、そこでは `run_dir` を `number` から導出して番号とパスの整合を構成で保証する」は、同じファイルの十数行下にある実装を未来形で説明する文になった。CLAUDE.md は「残すのは現在の形が成り立つ理由(why / why not)だけ」と定めており、スライスの進行状況はその理由に当たらない。しかも整合保証の説明が `rehydrate` と `launching` の doc に二重化している(`launching` 側にも「番号と run ディレクトリを1つの生成口で同時に受け取り、両者の食い違いを構成で排除する」がある)。
  - 提案: `rehydrate` の doc を「永続化からの再構築。新規採番は [`AttemptRef::launching`] が担う」に縮め、整合保証の why は `launching` 側の1箇所に残す。

### 確認した設計上の要点(所見)

- `pulsen-domain/Cargo.toml` の `[dependencies]` は空のまま。`crates/pulsen-domain/src/` の `cfg(` は `#[cfg(test)]` の25件のみで、`unix` / `windows` / `target_os` / `target_family` の述語は1件も無い(AC-1)。
- ドメインに新規のワイルドカード `match` は無い。`ensure_restartable` / `ensure_launching` / `is_agent_run` / `is_wait` / `is_cleanup` / `applicable_retry_limit` / `classify` / `classify_recheck` はいずれも変種を列挙している。既存の `_ =>` 4件(`duration.rs:50` / `assembler.rs:321` / `template.rs:35` / `time.rs:194`)はどれも文字・数値に対する分岐で、本差分の追加ではない。
- 遷移6種はすべて `self` を消費して新しい `Task` を返し、`updated_at` を更新する。`confirm_running` の `mut self` は `current_attempt.take()` のためだけで、返すのは新しい値。カウンタ更新は `RetryCounters` の `pub(super)` メソッド(`increment_attempt` / `increment_spawn_fail` / `reset_spawn_fail`)に閉じ、いずれも `self` 消費で新値を返す。
- カウンタ規則は spec どおり。`limit_exceeded(count, limit) = count > limit` を3遷移で共有し、等号で凍結しないことが `record_spawn_failure` / `record_spawn_failure_in_place` / `record_tool_failure` それぞれの境界テストで守られている。`confirm_running` は `reset_spawn_fail` だけを呼び、`RetryCounters::rehydrate(1, 2, 3)` → `rehydrate(1, 2, 0)` の比較で他2つの保持が主張されている。`record_spawn_failure_in_place` は実行状態も `current_attempt` も変えず(`next_attempt_number()` が 2 のままであることまで主張)、`confirm_workspace` はカウンタを変えない(専用テストあり)。
- `LaunchingClassifier` の境界は正しい。`Timestamp::elapsed_since` が `saturating_sub` + `u64::try_from(..).unwrap_or(0)` で巻き戻りを 0 に飽和させ、`classify` が `>` で比較するため 30 秒は超過せず 31 秒で超過する。テストは 0 / 29 / 30 / 31 / -3600 と、`starttime` のみが現れた中間状態(30 / 31 の両方)を通す。`classify_recheck` は pid 不在なら starttime の有無を問わず `SpawnFailed`、pid のみは `Err` で、spec の場合分けと一致する。
- パニックは不変条件違反に限られている。`current_status_def` の `expect` は不変条件1(生成経路が `register` と `rehydrate` の2つだけで、どちらも保証する)に、`WorkspacePlanner::derive` の2つの `expect` は「絶対パス + `TaskId` の文字集合」に依拠する。`BranchName::parse` の制約(非空・空白/制御なし・先頭 `-` なし・`..` なし・`/` 始まり/終わりなし・`.lock` 終端なし)は `pulsen/<task-id>` が常に満たすことを確認した(`branch.rs:63-91` と `TaskId` の `[a-z0-9-]`・先頭英数字の制約)。
- レイアウトの知識は `RunDirPath` に閉じている。6つの導出関数はすべて定数経由で、`FsRunStore` は `run_dir.pid_file()` 等を呼ぶだけでファイル名を持たない。`state_root()` の逆写像は `derive` との一致を受理条件にしており(ADR-079)、`attempt-01` / `attempt-+1` / `attempt-0` / `runs` 以外の段 / 不正なタスクIDがすべて `None` になることがテーブル駆動で守られている。
- ポートの宣言は AC-6 と一致する。`RunStore` 9メソッド・`ProcessController` 3メソッド・`WorktreeManager` に `create` 1つで、`attempt_exists` / `list_runs` / `delete_attempt` / `remove_task_dir_if_empty` / `starttime_of` / `kill` / `try_kill_remnants` / `remove` の宣言もスタブも無い。`RunStore` の doc が spec に無い契約(「write 系はいずれも書き込み先のディレクトリを必要に応じて作る」)を1つ足しているが、理由が添えられており、適合スイートの追加1件(`write_準備を経ない書き込みも置き場ごと作って残る`、`crates/pulsen-conformance/src/run_store.rs:381`)が実際に検証している。
- ドメインの判断がユースケースへ漏れていない。`Tick::launch` / `confirm_spawn` は「ポートで観測 → ドメインで判断 → ポートで実行」に徹し、猶予時間の比較・上限超過の判定・attempt の採番・run ディレクトリの導出はいずれもドメイン側でしか行われていない(`launch.rs:46-51` は採番をドメインに閉じたうえで番号を読み出すだけ)。逆にドメイン側へ I/O・表示の知識が入っている箇所も見当たらない — `describe()` 群は文言の定義箇所をドメインに1つ置く既存方針(ADR-082)の踏襲で、`std::fmt::Display` も serde も持ち込んでいない。
- `cargo test -p pulsen-domain` は 217 件すべて通る。

### カバレッジ

- 確認: `.thread/2/plan.md`, `.thread/2/adr.md`, `crates/pulsen-domain/src/definition/agent.rs`, `crates/pulsen-domain/src/definition/template.rs`, `crates/pulsen-domain/src/execution/launching.rs`, `crates/pulsen-domain/src/execution/mod.rs`, `crates/pulsen-domain/src/execution/port.rs`, `crates/pulsen-domain/src/execution/value.rs`, `crates/pulsen-domain/src/task/attempt.rs`, `crates/pulsen-domain/src/task/counters.rs`, `crates/pulsen-domain/src/task/failure.rs`, `crates/pulsen-domain/src/task/mod.rs`, `crates/pulsen-domain/src/task/path.rs`, `crates/pulsen-domain/src/task/planner.rs`, `crates/pulsen-domain/src/task/task.rs`, `crates/pulsen-domain/src/task/transition.rs`, `crates/pulsen/src/application/tick/mod.rs`, `crates/pulsen/src/application/tick/launch.rs`, `crates/pulsen/src/application/tick/confirm_spawn.rs`, `crates/pulsen/src/application/run_wrapper.rs`, `crates/pulsen/src/adapter/run_store.rs`, `crates/pulsen/src/cli/wrapper.rs`, `crates/pulsen-conformance/src/run_store.rs`

  (後半7件はドメイン単体ではなく「ドメインに置くべきロジックの漏れ」「ドメインが宣言した契約の検証有無」を判定するために読んだ。)

- スキップ: `.thread/2/progress.md` — 進捗記録。ドメインの判定材料にならない
- スキップ: `.thread/2/review/review-001-adapter.md` — 前回ラウンドの成果物(指示によりゼロベース)
- スキップ: `.thread/2/review/review-001-architecture.md` — 同上
- スキップ: `.thread/2/review/review-001-domain.md` — 同上
- スキップ: `.thread/2/review/review-001-test.md` — 同上
- スキップ: `.thread/2/review/review-001-usecase.md` — 同上
- スキップ: `.thread/2/review/review-001.md` — 同上
- スキップ: `.thread/2/review/triage.md` — 同上
- スキップ: `.thread/2/steps.md` — 実装手順書。契約は plan.md / spec で判定する
- スキップ: `.thread/2/testing.md` — 手動確認手順。Test 観点
- スキップ: `crates/pulsen-conformance/HOOKS.md` — 適合ハーネスの対応表。Test / Adapter 観点
- スキップ: `crates/pulsen-conformance/src/doubles/clock.rs` — テストダブル。Test 観点
- スキップ: `crates/pulsen-conformance/src/doubles/mod.rs` — 同上
- スキップ: `crates/pulsen-conformance/src/doubles/process.rs` — 同上
- スキップ: `crates/pulsen-conformance/src/doubles/run_store.rs` — 同上
- スキップ: `crates/pulsen-conformance/src/doubles/task_repository.rs` — 同上
- スキップ: `crates/pulsen-conformance/src/doubles/tests.rs` — 同上
- スキップ: `crates/pulsen-conformance/src/doubles/worktree.rs` — 同上
- スキップ: `crates/pulsen-conformance/src/lib.rs` — 適合スイートの配線。Test 観点
- スキップ: `crates/pulsen-conformance/src/process_controller.rs` — ProcessController の適合ケース。Adapter / Test 観点
- スキップ: `crates/pulsen-conformance/src/worktree_manager.rs` — WorktreeManager の適合ケース。Adapter / Test 観点
- スキップ: `crates/pulsen/examples/agent_probe.rs` — 適合テスト用のプローブ。Test 観点
- スキップ: `crates/pulsen/examples/spawn_probe.rs` — 同上
- スキップ: `crates/pulsen/src/adapter/mod.rs` — アダプターの再公開のみ。Adapter 観点
- スキップ: `crates/pulsen/src/adapter/process.rs` — プラットフォーム実装。Adapter 観点(ドメインへの `cfg` 混入が無いことは grep で確認済み)
- スキップ: `crates/pulsen/src/adapter/worktree.rs` — git CLI 実装。Adapter 観点
- スキップ: `crates/pulsen/src/application/mod.rs` — モジュール宣言のみ
- スキップ: `crates/pulsen/src/cli/args.rs` — 引数定義。CLI 観点
- スキップ: `crates/pulsen/src/cli/mod.rs` — サブコマンド配線。CLI 観点
- スキップ: `crates/pulsen/src/cli/render.rs` — 文言の組み立て。CLI 観点
- スキップ: `crates/pulsen/src/cli/tick.rs` — tick サブコマンド。CLI 観点
- スキップ: `crates/pulsen/src/cli/wire.rs` — 合成ルート。Architecture 観点
- スキップ: `crates/pulsen/tests/cli_tick.rs` — 受け入れテスト。Test 観点
- スキップ: `crates/pulsen/tests/cli_usage.rs` — 同上
- スキップ: `crates/pulsen/tests/cli_wrapper.rs` — 同上
- スキップ: `crates/pulsen/tests/common/git.rs` — テストヘルパー。Test 観点
- スキップ: `crates/pulsen/tests/common/mod.rs` — 同上
- スキップ: `crates/pulsen/tests/conformance_process_controller.rs` — 適合スイートの適用。Adapter / Test 観点
- スキップ: `crates/pulsen/tests/conformance_run_store.rs` — 同上
- スキップ: `crates/pulsen/tests/conformance_worktree.rs` — 同上
- スキップ: `crates/pulsen/tests/register_task.rs` — 既存の受け入れテスト。Test 観点
- スキップ: `crates/pulsen/tests/run_wrapper.rs` — ユースケーステスト。Usecase / Test 観点
- スキップ: `crates/pulsen/tests/tick_confirm_spawn.rs` — 同上
- スキップ: `crates/pulsen/tests/tick_fixture/mod.rs` — 同上
- スキップ: `crates/pulsen/tests/tick_launch.rs` — 同上
- スキップ: `crates/pulsen/tests/tick_scan.rs` — 同上
