### General Review（計画ドキュメント）

前ラウンド（review-001-general.md）が `steps.md` / `testing.md` に出した B-001〜005 / W-001〜006 は、**11件すべてが実装の最終形に合わせて修正済み**であることを実装と突き合わせて確認した（詳細はカバレッジ節）。本ラウンドで残る指摘は、前ラウンドで対象外だった `plan.md` / `adr.md` を含めた4ファイル間の整合に集中している。

#### Blockers

- **[B-001]** 「実行環境が前提を作れないとスキップで終わる行」の表が、probe 化後の許容集合と食い違う
  - 場所: `.thread/3/plan.md:63-72`（とくに `:68` と `:69`）
  - 理由: 実装のスキップ許容集合は `crates/pulsen/tests/conformance_command_runner.rs:167-179` と `crates/pulsen/tests/conformance_process_controller.rs:486-575` に確定しており、表と3点食い違う。(1) `:68` が `TC-port-command-runner-005` を「実行単位の終了を作れるプラットフォームか」でスキップしうる行に数えているが、実装は `PERMISSION_CASES` に 004 の1件しか置かず、コメントで「exit code を持たない終了(TC-005)は許容しない」と明記している（steps.md:172 も「005 は前提を作れない環境が無いため載せない」と書き、`HOOKS.md:47` の表にも 005 の行は無い）。(2) `:69` が `TC-port-process-controller-010` を `permission_restrictions_effective` でスキップする行としているが、実装は「取得機構の失敗(TC-010)は取得元の注入だけで作れる。前提を作れない環境が無いため、この集合には現れない」（同ファイル `:487-489`）で、どちらの許容集合にも入っていない。(3) `:68` / `:70` の 011〜016 の割り方（011/012/014 と 013/015/016）が、probe が実際に使う2集合（`EXECUTION_UNIT_CASES` = 011/012/013/015、`PARTIAL_TERMINATION_CASES` = 014/016）と一致しない。この表は AC-8 の記帳（「実行環境がスキップにした行にはチェックを付けず理由を残す」）が参照する唯一の一覧で、ステップ15 の実行者がここを読むと、実際には常に走る2行を「環境の都合で未チェック」として記帳しうる
  - 提案: 表を probe の2集合に書き直す。`TC-port-command-runner-004`（`permission_restrictions_effective`）／`TC-port-process-controller-011` / `012` / `013` / `015`（実行単位を起こせるか）／`TC-port-process-controller-014` / `016`（実行単位の一部だけを終了させられるか）の3行にし、`TC-port-command-runner-005` と `TC-port-process-controller-010` は表から外して「注入で確定的に走るため許容集合に入れない（ADR-007 / ADR-013）」と1行添える

- **[B-002]** 「既に実装済みで、本スライスは確認だけを行う行」の `DOM-task-053` 行が、本スライスで変種を1つ足した事実を落としている
  - 場所: `.thread/3/plan.md:44`（表の前文「新規実装は不要」）、`.thread/3/plan.md:49`
  - 理由: `crates/pulsen-domain/src/task/transition.rs` の `TransitionError` は6変種で、本スライスが `AlreadyNotified` を足している（adr.md ADR-006、steps.md:67 / :148 / :150 がいずれも「実装済み5種の確認 + `AlreadyNotified` の追加で6種」と書く）。plan.md だけが「(5変種)」「新規実装は不要」のままで、同じ事実が2つの文書で割れている。この表は #5 が `abort` / `retry` を足すときに `TransitionError` の現在形を読む位置にあり、「本スライスでは変種は増えていない」という誤った前提を引き継ぐ
  - 提案: `:49` の「(5変種)」を「(6変種。うち `AlreadyNotified` は本スライスで追加。ADR-006)」に、確認すること欄を「5種を区別すること（PASS 要件）を確認したうえで `AlreadyNotified` を足す」に直す。`:44` の「新規実装は不要」は `DOM-execution-002` にだけ掛かる書き方にする

- **[B-003]** 「spec との差分として提起するもの」が、ステップ15 で実際に提起する内容と数字が合わない
  - 場所: `.thread/3/plan.md:96`、`.thread/3/plan.md:97`
  - 理由: `:97` は「実装は `confirmed_running` を加えた10フィールド」だが、`crates/pulsen/src/application/tick/mod.rs:276-299` の `TickSummary` は `judged` を含む11フィールド（adr.md ADR-005）。`:96` の `TransitionError` の差分も `MissingCurrentAttempt` の1点だけを挙げ、`AlreadyNotified` に触れていない。steps.md:259 は同じ提起について「実装は6種(`MissingCurrentAttempt` と `AlreadyNotified`)、tick 出力 DTO は実装は11フィールド(`confirmed_running` と `judged`)」と正しく書いており、**契約と手順で同じ事実が別の数字で書かれている**。AC-8 は「spec との食い違い（下記「spec との差分として提起するもの」）も同じコメントで提起する」と plan.md の本節を名指ししているので、記帳の正本が誤っている状態になる
  - 提案: `:96` に `AlreadyNotified` の1件を足し、`:97` を「実装は `confirmed_running` と `judged` を加えた11フィールド(ADR-094 / ADR-005)」に直す。steps.md:259 の記述と一字一句そろえる

#### Warnings

- **[W-001]** 判定 timeout の期待「`sleep 120` が残っていない」が、ADR-001 が明示的に許容した孫プロセスの残存と食い違う
  - 場所: `.thread/3/testing.md:812`、`.thread/3/testing.md:813`、対する `.thread/3/adr.md:39`
  - 理由: エッジケース3 の判定コマンドは `["sh", "-c", "sleep 120"]` で、`SystemCommandRunner` が終了させるのは直接の子（`sh`）だけである。ADR-001 の Consequences は「`kill` するのは起動した直接の子だけで、その子が起こした孫は残りうる（残存は許容する）」と明記し、`spec/testcases/ports/command-runner.md` の TC-012 の期待も「起動されたプロセスは終了させられている」までしか要求していない。多くの `sh` は `sh -c '単一コマンド'` を exec に畳むため実際には残らない見込みだが、畳まない実体では期待が外れ、実行者は契約どおりの実装を不合格と読む
  - 提案: 期待結果を「`run` が返った時点で直接の子（判定コマンド）が終了していること」に改め、`pgrep` の結果については「孫が残りうることは ADR-001 が許容した範囲」と但し書きを添える。あるいは判定コマンドを `sh -c` を挟まない形にして、直接の子＝観測対象にそろえる

- **[W-002]** スナップショット破損への再通知の期待に、ADR-012 の要点（報告は通知と独立に積まれる）が入っていない
  - 場所: `.thread/3/testing.md:752`
  - 理由: 手順4・5 の期待は `notify.log` の1行と `notified_at` の記録・スナップショットの温存までで、同じ tick のサマリーに TD が現れることに触れていない。実装（`crates/pulsen/src/application/tick/mod.rs:411-430`）は `SnapshotUnreadable` の報告を実行状態によらず先に積んでから未通知の凍結だけ notify へ進むので、この tick では TD が**「スキップ」と「通知」の両方に**現れる。ADR-012 の Consequences もそう書いている。これは前ラウンドで実装が直った点（triage の `application/tick/mod.rs + notify.rs / 可観測性`）であり、手動確認がその回帰を捕まえる唯一の位置にあるのに、期待に書かれていないと「通知が出たから合格」で通ってしまう
  - 提案: 手順4・5 の期待に「サマリーの「スキップ」に TD が『埋め込まれたワークフロー定義を読めません』として現れ、同時に「通知」にも TD が現れる（報告は通知に置き換わらない。ADR-012）」を足す

- **[W-003]** plan.md ↔ testing.md で手動確認の対象 TC がそろっていない
  - 場所: `.thread/3/plan.md:115-138`（手動確認の表）、対する `.thread/3/testing.md:864`・`.thread/3/testing.md:914`
  - 理由: testing.md はエッジケース6 で `task-execution.md` TC-20（手順1〜4・6）を、確認項目9 で `intervention.md` TC-15（読み替え）を実行し、`:914` の記帳にも両方を数えている。ところが plan.md の表にはこの2件の行が無い。前ラウンドの W-005 / W-006 は testing.md 内の食い違いとして解消されたが、契約側の表は更新されていないため、ステップ15 が「plan.md の表に沿って実行した」と記帳すると2件ぶん実行範囲が食い違う
  - 提案: plan.md の表に `task-execution.md` TC-20（手順1〜4・6。手順1 の2件目は `pipeline` で代替）と `intervention.md` TC-15（手順2 の `abort` を上限超過での凍結に読み替え。TC-35 と同じ筋を1系列で消化）の2行を足す

- **[W-004]** `cat exit` の期待値が綴りのまま残っている（testing.md 側は値で書く形に直っている）
  - 場所: `.thread/3/plan.md:113`
  - 理由: `crates/pulsen/src/adapter/run_store.rs:171` の `encode` は `to_vec_pretty` なので、出力は `{\n  "code": 0\n}`（末尾改行なし）の整形 JSON になる。前ラウンドの W-003 で testing.md:63 は「綴りではなく `jq '.code'` で読める値で書く」に直ったが、その出所である plan.md は `{"code":0}` のリテラルのままで、同じ事実が2つの書かれ方をしている
  - 提案: `:113` を「`cat exit` の出力は `0` ではなく `.code` に終了コードを持つ整形 JSON になる（ADR-080）」に直す

- **[W-005]** ADR-002 の「既定は絶対パスで固定」が Windows の実装と一致しない
  - 場所: `.thread/3/adr.md:75`
  - 理由: `crates/pulsen/src/adapter/process.rs` の `terminate::default_source` は POSIX が `/bin/kill`（絶対パス）、Windows が `taskkill`（PATH 解決の固定名）。同じファイルの ADR-007（`:216`）は「既定の実体は絶対パス(または PATH 解決の固定名)のまま」と正しく書いており、adr.md の中で表現が割れている。#10（Windows 実機検証）が ADR-002 だけを読むと、既定が絶対パスで固定されている前提で検証を組むことになる
  - 提案: `:75` を「既定は POSIX が絶対パス、Windows が PATH 解決の固定名で固定し」に直す（ADR-007 の言い回しにそろえる）

- **[W-006]** 影響確認に、実装上起こりえない「`SystemCommandRunner` の構築失敗」の確認が残っている
  - 場所: `.thread/3/testing.md:890`
  - 理由: `crates/pulsen/src/cli/wire.rs:251` の `command_runner()` は `SystemCommandRunner::new() -> Self` を返すだけで、構築は無謬。steps.md:180 が「構築が失敗しうるかは実装で決まる」と留保していた点が、実装では「失敗しない」に確定している。実行者はこの確認をどう作るか決められない
  - 提案: 「`SystemCommandRunner` の構築が外部リソースの読み取りを伴わず（`wire::command_runner` は無謬）、`add` の経路がランナーを必要としないままであること」に置き換える

#### カバレッジ

- 確認: `.thread/3/plan.md`, `.thread/3/steps.md`, `.thread/3/testing.md`, `.thread/3/adr.md`
- 実装との突き合わせで参照したファイル（レビュー対象外・事実確認のみ）: `crates/pulsen/src/application/tick/{mod.rs,notify.rs}`, `crates/pulsen/src/cli/{render.rs,wire.rs,args.rs}`, `crates/pulsen/src/adapter/{task_file.rs,run_store.rs,process.rs,command_runner.rs}`, `crates/pulsen-domain/src/task/transition.rs`, `crates/pulsen-domain/src/execution/{running.rs,notification.rs}`, `crates/pulsen-domain/src/definition/{config.rs,workflow.rs}`, `crates/pulsen-conformance/{HOOKS.md,src/lib.rs,src/process_controller.rs,src/doubles/*}`, `crates/pulsen/tests/{conformance_process_controller.rs,conformance_command_runner.rs}`, `spec/manual-tests/{task-execution,setup,intervention}.md`, `spec/testcases/ports/command-runner.md`, `spec/testcases/execution/tick.md`, `spec/domains/{execution,task}.md`, `.adr/`, Issue #3 本文
- 前ラウンド指摘の解消確認（11件すべて解消）: `judged`（11フィールド）は steps.md:29 / :113 / :180 / :226 に反映済み。`TransitionError` の6種化は steps.md:67 / :148 / :150 / :259 に反映済み。tick サマリーの見出し列は `render.rs:60-69` の「起動 / 起動確認 / 判定確定 / 遷移 / 実行待ちへ復帰 / 凍結 / 通知 / 終端処理 / gcで削除 / gcで削除できず」と testing.md:892 が一致し、関数名も `cli::render::tick_summary` に修正済み。JSON パスは `.current_attempt.process.kill_ident`（testing.md:284）と `.execution` オブジェクト全体の区別（同 :286 / :554 / :611 / :638）が `adapter/task_file.rs` の DTO と一致。`cfg` の grep 件数「4 / 12 / 1」は実測（`util/atomic.rs` 4 / `adapter/process.rs` 12 / `adapter/task_repository.rs` 1）と一致し、合否をファイル集合で判定する形に改めてある。`TickIssue` の新分類は実装の8件（`MissingProcessIdent` / `ObservationFailed` / `KillFailed` / `RemnantsUnhandled` / `JudgeFailed` / `RunFailed` / `MissingWorkspace` / `NotifyFailed`）と steps.md:180 の列挙が一致。ADR-005〜007 は steps.md の該当ステップから参照されている。`exit` の期待値・TC-20 / TC-15 の記帳・エッジケース1 手順7 の一本化も testing.md 側で解消済み
- 確認して問題が無かった点: adr.md ADR-001〜014 は Context の問題も Decision の形も実装と一致した（`POLL_INTERVAL` + `started.elapsed()` によるポーリング、`-TERM -- <ident>` と `taskkill /T /F /PID`、`notify` / `notify_degraded` の2本、`TickIssue::MissingProcessIdent`、`TickSummary::judged`、`TransitionError::AlreadyNotified`、`TerminatorSource::with_terminator_source`、`TickIssue::MissingWorkspace`、`AliveDecision` + `From<AliveDecision>`、`RunFailureCause` / `RemnantsLeft::of`、`NotificationService::interpret_notify_completion`、`SnapshotUnreadable` の報告先出し、`ExecutionUnitCapability` の4値 probe、`RecordSeq` / `saved_in_order` / `calls_in_order`）。`.adr/` の既存89件と重複する ADR は無く、ADR-013 は基準を `.adr/073` に、ADR-007 は注入の形を `.adr/076` に明示的に帰している。Issue #3 のチェックリストは実際に128行あり、plan.md の「125行」（128 − 部分消化3行）も整合。`spec/manual-tests/` の参照 TC はすべて実在し、引用している手順番号もすべて各 TC の手順数の範囲内（TC-03 の手順12、TC-20 の手順6、intervention TC-01 の手順8 まで確認）。`judge_attempt_limit` の既定3・`DEFAULT_RETRY_LIMIT` 2・`NOTIFY_TIMEOUT` 60秒・`judge_env` / `notify_env` の変数名は実装と一致。報告の3見出しと各 `TickIssue` の文言も `render.rs` と一致。`Tick` のジェネリック引数は plan.md:85 のとおり7つ。steps.md が引く `spec/testcases/execution/tick.md` の連番アンカー（`#正常系-4` / `#境界値-3` / `#エッジケース-6` 等）はすべて該当節を正しく指す。見出し階層・表・コードフェンス・リンクの体裁に破綻は無い
- スキップ: なし（担当外の47ファイルはコードレビューの3観点が確認）
