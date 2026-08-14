# レビュー 005 — Use Case / CLI

### Use Case / CLI

#### Blockers

なし

#### Warnings

なし

#### 所見(確認ラウンドとしての検証内容)

ゼロベースで契約(`.thread/3/plan.md` AC-1〜AC-6)と `spec/usecases/execution.md` の手続き定義に突き合わせ、
本観点の指摘対象を洗い直した。結果、Blocker・Warning ともに検出しなかった。以下は確認した事実。

**配線に徹しているか**

`application/tick/` の4手続きに残っているのは分岐の選択と報告の組み立てだけで、判断は
すべてドメインへ委ねている。`Settled`(`observe.rs:250`)は `DefaultJudgement` /
`JudgeConclusion` を報告分類(`RunFailureCause`)と対にするだけの写像で、結論そのものは
`JudgementService` が決める。`RemnantsLeft::of`(`mod.rs:261`)も同型。`branch_of`
(`mod.rs:580`)・`issue_outcome`(`cli/render.rs:107`)・`written`(`tests/tick_notify.rs:75`)は
いずれもワイルドカードなしの網羅 `match`。

**共通手続き notify の順序と notify_cmd 未定義時の非記録**

`commit`(`mod.rs:472`)が「`save` → `Freeze::Frozen` なら `frozen` 計上 → `notify`」の1点で、
`notify`(`notify.rs:44`)が「`deliver` → `Delivered` のときだけ `mark_notified` → `commit`」。
`deliver` は `notify_cmd` が None なら `Delivery::NotConfigured` を返して書き込みも起動も
行わない。catch-up の保存は `Freeze::NotFrozen` を通しており(ADR-097)、`Branch::Notify`
経由の通知が `frozen` を再計上しない。`NOTIFY_TIMEOUT` は常に `Some` で渡る。
順序はテストダブルの共有採番(`notify_steps` / `RecordSeq`)で
`SavedFrozen → RanNotifyCmd → SavedNotifiedAt` として主張されており、`save_degraded` 経路も
`SavedDegradedNotifiedAt` で区別されている。

**手続きD の分岐**

`observe`(`observe.rs:31`)は `read_exit` の結果が `Some` のときに `RunningDecision::Judge` を
直に作り、`observe_aliveness` を呼ばない(2段規則)。`終了コードがあれば生存を観測せずに判定へ進む`
が `processes.calls()` の空で主張。`kill` の `KillError` と `starttime_of` の `Err(Io)` は
どちらも `summary.errors` を積むだけで `save` を呼ばず、対応するテストが
`harness.tasks.saved().is_empty()` を主張している。`DiedWithoutExit` は
`try_kill_remnants` → `fail_run` → `save` の順序を共有採番(`died_without_exit_steps`)で
主張し、保存に失敗しても残存の報告が積まれることも別テストで押さえている。

**報告の分類と表示(依頼された「他の文言に同種の食い違いが残っていないか」の確認)**

`TickIssue` の全26変種について push 箇所を機械的に列挙し、複数箇所から積まれる変種を特定した。
複数箇所から積まれるのは3つだけ。

| 変種 | push 箇所 | 文言の広さ |
|---|---|---|
| `MissingCurrentAttempt` | `confirm_spawn.rs:45` / `observe.rs:36` | 「観測の前提となる現在 attempt がありません」— 実行状態を名指ししない(ラウンド4 の修正) |
| `RunFileUnreadable` | `confirm_spawn.rs:160,170` / `observe.rs:52` | 「runディレクトリのファイルを読めません(…)」— ファイル名は `RunFileError` 側が持つ |
| `SaveFailed` | `mod.rs:485`(`save`) / `notify.rs:84`(`save_degraded`) | 「タスクファイルを保存できません」— どちらの書き先でも成立 |

残る23変種はいずれも単一の push 箇所を持ち、文言が名指しする状態と積まれる状況が一致する。
特に確認したもの:

- `MissingProcessIdent`「起動確認済みですが同定情報がありません」— `observe.rs:43` のみ。手続きD に
  入るのは `ExecutionState::Running` だけなので「起動確認済み」は過不足がない。
- `MissingWorkspace`「判定コマンドへ渡すワークスペースが未確定です」— `observe.rs:90`(判定コマンドを
  持つステータスの分岐)のみ。
- `KillFailed`「timeout を超えた実行を終了させられません」— `observe.rs:167`(`KillOnTimeout`)のみ。
- `RunFailureCause::DefaultJudgement`「実行が終了コード N で終了しました」— `Settled::by_default` から
  しか作られず、判定コマンド経由は `JudgeCommand` に分かれる(ラウンド1 の指摘の形が保たれている)。
- `RunFailureCause::TimedOut { timeout: Unlimited }` の文言だけは分類より**広い**側に倒してあり
  (`cli/render.rs:275`)、`classify_alive` が無制限で `KillOnTimeout` を返さない事実に依存しない全域の
  書き方になっている。狭すぎる方向の食い違いではない。
- `TransitionError::MissingCurrentAttempt`(`cli/render.rs:314`)も実行状態を名指ししない形で、
  `tests/tick_notify.rs:530` が「判定確定のタスクを起動記録済みとは述べない」を主張している。

4分類(`IssueOutcome`)の割り当ても、書き込みの有無と一致していることを push 箇所ごとに確認した。
`Recorded` に落ちる4変種(`WorktreeCreateFailed` / `CommandExpansionFailed` / `SpawnNotObserved` /
`JudgeFailed` / `RunFailed`)はすべて `Persisted::Saved` の枝でだけ積まれ、`RemnantsUnhandled` だけが
保存と独立に積まれて `CleanupLeft` に分かれる。

**走査レベルの分岐と `Branch::Notify` / `AlreadyNotified`**

`branch_of` は `Stopped` を `notified_at` で2変種に割り、`AlreadyNotified` は無処理。
`tests/tick_scan.rs` の `通知済みの凍結には何もしない` が `commands.calls()` の空で主張する。
`SnapshotUnreadable` は報告を必ず積んだうえで未通知の凍結だけ notify へ進み、
`通知コマンドが未定義でもスナップショット破損の未通知凍結は報告される` と
`スナップショット破損の凍結以外は定義依存の判断をすべてスキップして報告する` の2本が両側を押さえる。

**1タスク1tick1ステップ・冪等性・exit code・サマリー**

`observe` の判定確定は `complete_run` + `save` で止まり、遷移は次 tick の `advance`。
`tests/tick_scan.rs:実行状態の異なる複数のタスクがそれぞれの分岐で1ステップ進む` が
launched / confirmed_running / judged / transitioned / notified の5分岐を1回の走査で同時に主張し、
かつ各タスクが1ステップしか進まないことを保存内容で確かめている。
書き込みを起こさない tick の冪等性は `状態が変化しないタスク群には連続実行しても書き込みが発生しない`
(running を含む形に拡張済み)。exit code は `cli/mod.rs:43` で `Skipped` / `Completed` とも 0、
`TickError` のみ非0 で spec のエラーケース表どおり。

**排他ロックの保持時間**

`execute`(`mod.rs:378`)の `_guard` が走査全体を覆い、判定・通知の起動もこの中で行われる。
組み込み timeout(`judge_timeout` / `NOTIFY_TIMEOUT`)が上限を与える設計(ADR-018)で、
本スライスで新たな緩和も悪化も入っていない。

**ラウンド4 で入れ替わった受け入れテストの実効性**

`tests/cli_tick.rs:944 判定と遷移と凍結と通知はサマリーに現れる` は `patch_task` による帳簿の
直置きをやめ、`retries: 0` の `once` ワークフローを別登録して実際に上限超過で凍結させる筋になっている。
フレークしうる箇所を追った結果、非決定性は見当たらない —
遷移済みの `advancing` は `done`(`run: cleanup`)へ移って `Branch::Cleanup` の無処理に落ちるため
以降の tick に干渉せず、`freezing` の起動・完走・取り込み・凍結は `wait_for_exit` と
`launch_and_confirm` で同期が取れている。制御ファイルの切り替え(`set_control(&control, 1)`)は
`advancing` の判定が終わったあとに置かれている。主張も見出しとIDの組
(`判定確定: {id}` / `遷移: {id}` / `凍結: {id}` / `通知: {id}`)で、見出しだけの緩い一致ではない。
手元で4回連続実行してすべて緑、`tick_scan` / `tick_observe` / `tick_notify` の58件も全緑。

**弁明・経緯のコメント**

`crates/pulsen/src/application/` `crates/pulsen/src/cli/` と本観点のテスト4本を
`指摘|レビュー|ラウンド|修正した|以前は|変更前|TODO|FIXME|元々` で走査し、該当なし。
残っているコメントはいずれも「現在の形が成り立つ理由(why / why not)」に収まっている。

#### カバレッジ

- 確認: `crates/pulsen/src/application/tick/mod.rs`, `crates/pulsen/src/application/tick/observe.rs`,
  `crates/pulsen/src/application/tick/notify.rs`, `crates/pulsen/src/application/tick/launch.rs`,
  `crates/pulsen/src/application/tick/confirm_spawn.rs`, `crates/pulsen/src/cli/render.rs`,
  `crates/pulsen/src/cli/tick.rs`, `crates/pulsen/src/cli/wire.rs`,
  `crates/pulsen/tests/cli_tick.rs`, `crates/pulsen/tests/tick_observe.rs`,
  `crates/pulsen/tests/tick_notify.rs`, `crates/pulsen/tests/tick_scan.rs`,
  `crates/pulsen/tests/tick_fixture/mod.rs`, `crates/pulsen/tests/common/mod.rs`,
  `.thread/3/plan.md`, `.thread/3/review/triage.md`
- スキップ: `.thread/3/review/` 配下(triage.md を除く23ファイル) — Phase 8 で削除されるレビューの中間成果物
- スキップ: `.thread/3/adr.md`, `.thread/3/steps.md`, `.thread/3/testing.md` — 手順書・決定記録で、
  本観点の対象(ユースケース配線と CLI 表示)を持たない(General 観点)
- スキップ: `crates/pulsen-domain/src/execution/judgement.rs`,
  `crates/pulsen-domain/src/execution/notification.rs`, `crates/pulsen-domain/src/execution/running.rs`,
  `crates/pulsen-domain/src/execution/mod.rs`, `crates/pulsen-domain/src/execution/port.rs`,
  `crates/pulsen-domain/src/execution/value.rs`, `crates/pulsen-domain/src/task/counters.rs`,
  `crates/pulsen-domain/src/task/degraded.rs`, `crates/pulsen-domain/src/task/task.rs`,
  `crates/pulsen-domain/src/task/transition.rs` — ドメイン層(Domain 観点)。
  ユースケースからの呼び出し面(戻り値の網羅・引数の順序)だけは上記の確認に含めた
- スキップ: `crates/pulsen/src/adapter/command_runner.rs`, `crates/pulsen/src/adapter/process.rs`,
  `crates/pulsen/src/adapter/mod.rs` — アダプター層(Adapter 観点)。
  合成ルートからの構築(`wire::command_runner`)だけは確認済み
- スキップ: `crates/pulsen-conformance/src/command_runner.rs`,
  `crates/pulsen-conformance/src/process_controller.rs`, `crates/pulsen-conformance/src/lib.rs`,
  `crates/pulsen-conformance/HOOKS.md`, `crates/pulsen/tests/conformance_command_runner.rs`,
  `crates/pulsen/tests/conformance_process_controller.rs` — ポート適合テストのハーネス(Adapter 観点)
- スキップ: `crates/pulsen-conformance/src/doubles/command_runner.rs`,
  `crates/pulsen-conformance/src/doubles/mod.rs`, `crates/pulsen-conformance/src/doubles/process.rs`,
  `crates/pulsen-conformance/src/doubles/run_store.rs`,
  `crates/pulsen-conformance/src/doubles/task_repository.rs` — テストダブルの実装。
  ユースケーステストから使う API(`with_read_exit` / `calls_in_order` / `saved_degraded_in_order` /
  `RecordSeq`)の振る舞いは呼び出し側で確認した
- スキップ: `crates/pulsen/examples/judge_probe.rs`, `crates/pulsen/examples/agent_probe.rs` —
  テスト用フィクスチャの実行ファイル。受け入れテストからの利用面(制御ファイル・既定 exit)は確認済み
