### General Review（計画ドキュメント）

#### Blockers

なし

#### Warnings

なし

**この観点の指摘はゼロ。** 4ファイルの記述を実装の最終形・spec・`spec/manual-tests/` と突き合わせ、実測値はすべて自分でコマンドを実行して確かめた。事実の食い違いは1件も残っていない。

#### 実測で確かめた点（食い違い無し）

**ラウンド4 で入った3件の修正**

- `steps.md:157` の `RecordSeq` は4メソッド（`TaskRepository::save` / `save_degraded` / `CommandRunner::run` / `ProcessController::try_kill_remnants`）と4アクセサになっており、`crates/pulsen-conformance/src/doubles/{task_repository.rs:95,104, command_runner.rs:57, process.rs:135}` と一致する。`saved_degraded_in_order` も `crates/pulsen/tests/tick_notify.rs:115` で実際に使われている。モジュール増分（`steps.md:40,42,43`）にも4アクセサが載った。
- `steps.md:165` の「同表に足すのは3行」は `crates/pulsen-conformance/HOOKS.md:46-48` の3行（007 / 011〜016、011 / 012 / 013 / 015、014 / 016）と一致し、`steps.md:173` の CommandRunner 側「2行」も `HOOKS.md:49-50`（004、`001 / 002 / 005〜016`）と一致する。`judge_probe` を要する行の範囲も実際の行と同じ。
- `testing.md:901` のフィクスチャB は `$HOME/pulsen-manual-test` / `$HOME/pulsen-test-repo` / `$HOME/pulsen-manual-work` の3つを挙げており、`testing.md:295-299` の `rm -rf` 対象・`:909-910` の後片付けと一致する。

**ラウンド4 で変わったコード側との整合**

- `cli/render.rs:180` の `MissingCurrentAttempt` は「観測の前提となる現在 attempt がありません(タスクファイルの修復が必要です)」、`:219` の `MissingProcessIdent` は「起動確認済みですが同定情報がありません(pid ファイルからの修復が必要です)」。`testing.md` エッジケース4 の期待「原因（現在 attempt が無い / 同定情報が無い）が読める形」「2つの破れが区別できる文言」を満たす。
- `crates/pulsen/tests/cli_tick.rs:901` に `プロトコル外の判定はエージェントを再実行せず判定上限の超過で凍結する` が実在し、`steps.md:244` の最後の受け入れ項目と一致する。
- `HOOKS.md:59` の正本段落は「保持プロセス・テスト用エージェント・テスト用コマンド・デタッチ性のフィクスチャ」を列挙し、`steps.md:165,173`・`plan.md:73` の「実行ファイルの不在は許容集合に入れない」と齟齬がない。

**testing.md を手順書として実行可能か**

- AC-7 の3つの grep（`:50-52`）を実行: cfg のヒットは `util/atomic.rs` 4 / `adapter/process.rs` 20 / `adapter/task_repository.rs` 1 の3ファイルで記載どおり。`pulsen-domain` は0件、`adapter/command_runner.rs` も0件。`#[allow(unsafe_code)]` は `adapter/process.rs:454` の1件。`-A 8` で `pulsen` の本番依存7行（`pulsen-domain` / `clap` / `getrandom` / `serde` / `serde_json` / `serde_yaml_ng` / `tempfile`）が全て出て、`pulsen-domain` の `[dependencies]` は空。
- 影響確認の grep（`:891`）: `command_runner()` は `cli/wire.rs:251`（定義）と `cli/tick.rs:30`（呼び出し）の2件で `cli/add.rs` に無い。`SystemCommandRunner::new() -> Self` は失敗しない。
- 直読の JSON パス（`:284,286`）: `.execution.state` / `.execution.reason` / `.execution.notified_at` / `.task_status` / `.counters.{attempt_count,judge_attempt_count,spawn_fail_count}` / `.current_attempt.{number,run_dir}` / `.current_attempt.process.{pid,kill_ident}` / `.last_failure.{kind,message}` はすべて `adapter/task_file.rs:43-134` の DTO と一致。`execution` は `#[serde(tag = "state")]` の直和で、記載どおり同階層に `reason` / `notified_at` / `recorded_at` が並ぶ。
- エッジケース1 の `jq '.snapshot.statuses = "broken"'`（`:731`）は成立する — `TaskFileDto` の `snapshot` は `Box<RawValue>` として先に切り離され（`task_file.rs:206,246-276`）、タスク側フィールドが読める破れが `SnapshotUnreadable` になる。`application/tick/mod.rs:411-429` が報告を無条件に積んでから未通知 stopped だけを notify へ回すので、`:752` の「通知」と「スキップ」に同時に現れる期待も成立する。
- 期待文言（`:752,859`）: 「埋め込まれたワークフロー定義を読めません」= `render.rs:174`、「runディレクトリのファイルを読めません」= `render.rs:189`。
- サマリー行の並び（`:893`）と報告の4見出し（`:717,893`）: `render.rs:60-69` の「起動 / 起動確認 / 判定確定 / 遷移 / 実行待ちへ復帰 / 凍結 / 通知 / 終端処理 / gcで削除 / gcで削除できず」、`:83-86` の「失敗を記録 / 起動の結果が未確定 / スキップ / 後始末が残っている」と完全一致。`RemnantsUnhandled` だけが `IssueOutcome::CleanupLeft`（`render.rs:120`）。
- 定数と既定値: `NOTIFY_TIMEOUT` 60秒（`notification.rs:35`）、`DEFAULT_JUDGE_ATTEMPT_LIMIT` 3・`DEFAULT_JUDGE_TIMEOUT` 60s（`definition/config.rs:55,57`）、`DEFAULT_RETRY_LIMIT` 2（`definition/workflow.rs:105`）。`testing.md` の「上限（既定3）の等号では凍結せず」「wf-fail は目安10回の tick」「`fail.yaml` の `retries: 1` で 2 > 1 が凍結」はすべてこの値で成り立つ。
- CLI 前提（`:12,895`）: `args.rs` の `Command` は `Add` / `Tick` / `Wrapper(hide)` の3つで、`--home` は global フラグ（`args.rs:13`）。`examples/` は `agent_probe` / `judge_probe` / `lock_holder` / `spawn_probe` の4つで `:32` の記述どおり。run ディレクトリの整形 JSON は `adapter/run_store.rs:172` の `to_vec_pretty`。
- `.thread/2/testing.md` への参照（`:891,897`）: 確認項目2 / 3、エッジケース6 はいずれも実在（`.thread/2/testing.md:258,283,642`）。
- 手順書の TC 番号と手順番号: 対象24件すべての実際の手順数を数えて突き合わせた。task-execution TC-03(12)・05(6)・06(5)・07(8)・13(7)・14(6)・15(7)・17(5)・19(6)・20(7)・21(6)・22(3)・23(3)、setup TC-09(5)・10(5)・11(5)・35(4)・37(4)・38(3)・39(4)・47(3)、intervention TC-01(8)・15(5)・24(5)。落とす手順の番号（TC-07 の6〜8 が abort / set-status、TC-20 の5 が ls・7 が set-status、intervention TC-01 の4・6 が `ls --state` / `show`、TC-15 / 24 の手順2 が `abort`、setup TC-35 の手順4 が notify_cmd の復元、TC-39 の手順4 が復元）も原文と一致。`spec/manual-tests/intervention.md:25` の `PMT` は `$HOME/pulsen-manual-test` で、`testing.md:73` の読み替えの根拠が実在する。
- スクリプトの実行可能性: `sed -i.orig 's/prompt: "echo planning .*"/…/'` は `pipeline.yaml` の当該行に一致する。`sed -i.bak 's/^judge_timeout: 60s$/judge_timeout: 5s/'` はフィクスチャB の config に一致。`grep -v '^notify_cmd:'` はフィクスチャC の config に一致。`workflow:` キーを持たない `sigkill.yaml` / `judge-missing.yaml` / `judge-hang.yaml` はパス登録で通る（`definition/assembler.rs:176` の `declared_name` は `Option`）。

**steps.md と実装の最終形**

- ポート表（`:79-84`）のシグネチャ6件は `execution/port.rs:249,252,255,350` と `RemnantOutcome`(3値) / `KillError`(1値) に一致。
- ステップ8 の「`TickIssue` を8つ足す」は実際の増分（`MissingProcessIdent` / `ObservationFailed` / `KillFailed` / `RemnantsUnhandled` / `JudgeFailed` / `RunFailed` / `MissingWorkspace` / `NotifyFailed`）とちょうど一致。
- ステップ10 の手続きDの0〜3は `spec/usecases/execution.md:93-107` の順序と、`application/tick/observe.rs` の呼び出し（`starttime_of` → `classify_alive` → `kill` / `try_kill_remnants` → `fail_run`、`record_judge_failure(detail, judge_attempt_limit, now)`）に一致。
- ステップ6 が直すと書いた `process_controller.rs` 冒頭の「8行」は現在「11件」になっている。ステップ5 が足すと書いた `ScriptedRunStore::with_read_exit` / `RunStoreCall::ReadExit` も実在（`doubles/run_store.rs:33,122`）。
- ステップ12 の並び順、ステップ13 の受け入れ9項目は `cli_tick.rs` のテスト名と対応が付く。

**plan.md と実装の最終形**

- `Tick` のジェネリック引数は `<'a, R, L, K, W, S, P, C>` の7つ（`application/tick/mod.rs:325`）。
- `TickSummary` は11フィールド、`TransitionError` は6変種（`MissingCurrentAttempt` / `AlreadyNotified` を含む）で `:99-100` の spec 差分の記載どおり。
- `spec/domains/execution.md:109` が `classify_alive -> RunningDecision`、`:119` が `default_judgement -> JudgeOutcome` で、行番号参照が実在。実装は `AliveDecision` / `DefaultJudgement`（`execution/mod.rs:14,24`）。2段規則の1段目は `observe.rs` にある。
- スキップ許容集合（`:63-75`）は `conformance_process_controller.rs` の `EXECUTION_UNIT_CASES`(011/012/013/015) / `PARTIAL_TERMINATION_CASES`(014/016) / 4値の `ExecutionUnitCapability` / `observation_allowed_skips`、`conformance_command_runner.rs:167,173` の `PERMISSION_CASES`(004) / `allowed_skips` と一致。判定関数名も正しい。
- `HOOKS.md` の件数（`:114`）: ProcessController 27行 / CommandRunner 16行（A0・B15・C1）。総計も再計算して 10ポート196行・A 41 / B 132 / C 23 で本文と一致する。
- テスト方針の順序主張（`:116`）: `tick_observe.rs:214` が `processes.calls().is_empty()` で「exit が Some なら `starttime_of` を呼ばない」を、`tick_notify.rs:103,115,127` と `tick_observe.rs:110,125` が採番のマージで「またぐ順序」を主張している。

**adr.md（ADR-001〜017）**

- 型名・定数名はすべて実装と一致 — `terminate::UnitTarget`（POSIX は `-<n>` の `n >= MIN_PGID`、Windows は非0の pid）/ `TERMINATION_GRACE` 2秒 / `TERMINATION_POLL` 50ms / `terminate::ESCALATES`（POSIX `true`:590・Windows `false`:670）/ `TerminatorSource`・`with_terminator_source` / `POLL_INTERVAL` 50ms と `started.elapsed()` / `AliveDecision` / `DefaultJudgement` / `NotifyOutcome` / `Delivery` / `RunFailureCause` 4変種 / `RemnantsLeft` 2変種 / `IssueOutcome::CleanupLeft` / `RecordSeq`。既定の終了実体も POSIX `/bin/kill`・Windows `taskkill`（ADR-002 / ADR-007 の記述どおり）。
- Context が述べる問題の実在: ADR-014 が引く「spec 手続きD 3 の `DiedWithoutExit` は `try_kill_remnants` → `fail_run` → `save`」は `spec/usecases/execution.md:107` に実在。ADR-009 / ADR-016 が指す2段規則・デフォルト判定2値も spec の `:105-108` / `:119` にある。ADR-012 の「ただし」も走査の記述どおり。
- 置き換えの扱いが揃っている — ADR-013 → ADR-073、ADR-015 → ADR-002、ADR-017 → ADR-098 のいずれも「置き換えた点だけを書き、本文は書き換えない」と明示している。
- `.adr/` の既存89件と ADR-001〜017 の主題の重複なし。
- 体裁: 見出し階層・`---` によるエントリ区切り・Status / Context / Decision / Consequences の構成は `.thread/2/adr.md` と同じ規約。plan.md / steps.md / testing.md の冒頭ブロックと `---` の使い方も `.thread/2/` と揃っている。強調の見出し代用・冗長な区切り線・不自然な改行は無い。

**4ファイル間の整合**

- plan.md の手動確認の表（24 TC）と testing.md の記帳（`:915`）は、TC の集合・実行範囲・読み替え（`abort` → 上限超過での凍結、`show` / `ls` → 直読）・復元手順の3件すべてで一致する。
- plan.md の spec 差分5件と steps.md ステップ15 の提起内容は、変種名・フィールド数・型名まで同一。
- steps.md のモジュール増分・ステップ番号は plan.md の AC 表・テスト方針の記述と矛盾しない。adr.md の ADR 番号は plan.md / steps.md / testing.md からの参照先とすべて対応する。

#### カバレッジ

- 確認: `.thread/3/plan.md`, `.thread/3/steps.md`, `.thread/3/testing.md`, `.thread/3/adr.md`
- スキップ: なし（担当外のコード本体はコードレビューの3観点が確認）
