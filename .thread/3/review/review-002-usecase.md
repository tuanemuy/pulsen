### Use Case / CLI

#### 前提と検証の方法

- 契約は `.thread/3/plan.md`(AC-1〜AC-8)と `spec/usecases/execution.md`(共通手続き notify・処理フロー 1〜9・手続きD)、`spec/flows/index.md`(F1 / F3 / F8)、`spec/pages/index.md` §縮退状態の共通規則 ※5、`spec/testcases/execution/tick.md`(走査と分岐 / 手続きD / 共通手続き notify)を基準にした。既決の判定は `.thread/3/review/triage.md` で確認した(全20件 `fix`。`wont-fix` / `defer` はない)。
- 手元で品質ゲートを実行した: `cargo fmt --check` / `cargo clippy --all-targets --all-features -- -D warnings` は無警告、`tick_observe`(29件)/ `tick_notify`(11件)/ `tick_scan`(17件)/ `cli_tick`(27件)はすべて緑。
- スコープ逸脱は見つからなかった。`Branch::Cleanup` のアームは空のまま(`// 終端処理(手続きB)は Issue #6 が入れる。`)、`archived` / `gc_deleted` / `gc_errors` に値の入る経路はなく、`abort` / `retry` / `set_status` / `attempt_exists` は grep しても現れない。`DegradedTask` に足されたのは `mark_notified` だけ。

#### 契約との突き合わせ(合致を確認した点)

| 契約 | 実装 | 判定 |
|---|---|---|
| 配線に徹する(観測 → ドメインで判断 → 実行) | 判定の解釈は `JudgementService::interpret_judge_completion`、通知の成否解釈は `NotificationService::interpret_notify_completion`(前ラウンドでドメインへ移動済み)、生存分類は `IdentityCheck` / `RunningClassifier`、上限判断は `Task` の遷移関数。`observe.rs` / `notify.rs` に残るのは分岐の並べ方だけ | ○ |
| notify の順序「stopped を書く → notify_cmd → 成功時のみ `notified_at`」 | `tick/mod.rs:472-492` の `commit` が `save` 成功後にだけ `notify` を呼び、`notify.rs:53-63` が `NotifyOutcome::Delivered` のときにだけ `mark_notified` → `commit` する | ○ |
| notify_cmd が None なら `notified_at` を書かない(catch-up が効く) | `notify.rs:97-99` の `Delivery::NotConfigured` は保存を一切行わない。`tick_notify.rs:169-208` と `cli_tick.rs` の「通知コマンドが未定義の凍結は後から定義した次のtickで通知される」が catch-up まで実バイナリで見ている | ○ |
| 凍結の計上を保存後の状態から導出しない(ADR-097) | 通知アームは `Freeze::NotFrozen` を通す(`notify.rs:57`)。`cli_tick.rs` の再通知ケースが「さらに次の tick では増えない」まで見ている | ○ |
| 手続きD 冒頭の不変条件2・3 の検査 | `observe.rs:33-45`。`read_exit` より前に両方を見ており、`MissingCurrentAttempt` / `MissingProcessIdent` と分類も分かれる。`tick_scan.rs` の「起動確認済みなのに同定情報がないタスクは報告してスキップする」が2件を並べて主張 | ○ |
| exit が Some なら生存観測を行わない | `observe.rs:59-66` が1段目を値にしてから `match` する。`tick_observe.rs:155-164` が `processes.calls()` の空で主張しており実効的 | ○ |
| `AliveDecision`(3値)への絞り込みと `unreachable!` の除去 | `running.rs:36-54` が `AliveDecision` を返し、`From<AliveDecision> for RunningDecision` で写す。`observe.rs` の網羅 `match` にパニックは残っていない。ユニットテスト「生存の分類は判定以外の3値へそのまま対応する」が写像を閉じている | ○ |
| `KillOnTimeout` の kill 失敗で状態を変更せず報告のみ | `observe.rs:156-172`。`tick_observe.rs:600-627` が `saved()` の空で主張 | ○ |
| `DiedWithoutExit` は `try_kill_remnants` → `fail_run`、結果は報告のみ | `observe.rs:175-195`。`tick_observe.rs:650-680` が `ProcessControllerCall` の並びで順序を主張し、`683-719` が3値の結末を分類として主張 | ○ |
| `starttime_of` の `Err(Io)` で状態を変更しない | `observe.rs:135-144`。`tick_observe.rs:464-487` | ○ |
| 報告の構造化(ADR-081): 分類はユースケース、文言は `cli::render` の網羅 `match` | `RunFailed { cause: RunFailureCause }` / `RemnantsUnhandled { remnants: RemnantsLeft }` / `MissingWorkspace` が入り、文言は `render.rs:249-283` に移った。判定コマンドが失敗と判定した場合は「判定コマンドが失敗と判定しました(実行の終了コードは 0)」となり、前ラウンドの自己矛盾(judge exit 10 で「実行が終了コード 0 で終了しました」)は解消している。`render.rs` の「実行の失敗の根拠は判断した主体が読める形で示される」が4分類すべてを実出力で見ている | ○ |
| `Corrupt` は報告のみ / `SnapshotUnreadable` は報告を積んだうえで未通知凍結なら notify / `Completed` は `advance` → `save` / `Stopped` は未通知なら notify | `tick/mod.rs:398-432`・`456-465`。報告は実行状態によらず必ず積むようになり、`tick_scan.rs` の「通知コマンドが未定義でもスナップショット破損の未通知凍結は報告される」が前ラウンドの穴を閉じている | ○ |
| `Branch::Notify` / `AlreadyNotified` の分割 | `branch_of`(`tick/mod.rs:592-599`)が `notified_at` のパターンで分ける。真偽値フラグは残っていない。`tick_scan.rs`「通知済みの凍結には何もしない」が主張 | ○ |
| 1タスク1tick1ステップ | `observe.rs:101-108` は `complete_run` の保存で止まる。`tick_observe.rs:91-110` と `cli_tick.rs`「終了コード0の観測は判定確定になり次のtickが次のステータスへ進める」が実バイナリで一周を確認 | ○ |
| 1タスクの失敗を `errors` に積んで続行 / tick パスの exit は 0 / `list_active` の Io だけ非0 | `tick/mod.rs:384-394`・`cli/mod.rs:42-56` | ○ |
| 冪等性 | `tick_scan.rs`「状態が変化しないタスク群には連続実行しても書き込みが発生しない」に running を追加済み。`tick_observe.rs`「判定は同じ終了コードと同じ定義に対して同じ結論を導く」「判定確定の保存に失敗しても次のtickが同じ結論を再導出する」 | ○ |
| ADR-092 / ADR-005: 書き込んだ tick は必ずサマリーに現れる | `judged` の新設で `complete_run` が拾われ、`skip_run`→`skipped_back` / `advance`→`transitioned` / `mark_notified`→`notified` / `fail_run`・`record_judge_failure`→`errors` が埋まる。`is_empty` も更新済み。`cli_tick.rs`「判定と遷移と凍結と通知はサマリーに現れる」が実出力で見ている | ○ |
| 順序検証の実効性(`RecordSeq`) | `doubles/mod.rs` の単一 `AtomicU64` から採番し、`ScriptedTaskRepository::saved_in_order` と `ScriptedCommandRunner::calls_in_order` を `tick_notify.rs:50-81` の `notify_steps` が1本の列へ並べ直す。通知を先に起動する実装は `RanNotifyCmd` が先頭に来て落ちるので、前ラウンドの「独立したベクタで順序が見えない」は解消している | ○ |
| 経緯コメントの不在 | 変更ファイル全体を「以前 / かつては / レビュー / 指摘 / 修正し / ようにした / 変更した / だったが / していたが」で grep してヒット 0。前ラウンドで二重否定になっていた `tick_notify.rs` のコメントは「通知アームに入るのは凍結だけで…」という規則の説明に置き換わっている | ○ |

#### Blockers

なし

#### Warnings

- **[W-001]** 判定上限の超過による凍結について、`frozen` への計上と同一 tick 内の通知を主張するテストが1件もない
  - 場所: `crates/pulsen/tests/tick_observe.rs:313-347`(`判定失敗は上限と等しい回数では凍結せず超えると凍結する`)、`crates/pulsen/tests/cli_tick.rs:888-928`(`プロトコル外の判定はエージェントを再実行せず判定上限の超過で凍結する`)
  - 理由: AC-4 は「リトライ上限・判定上限・spawn 上限の超過が `Stopped` を保存し、その `save` 直後の同一 tick 内で notify_cmd が起動される」ことを3経路すべてに求めている。同一 tick 通知の引き金は `commit(&task, Freeze::of_recorded_failure(&task), _)` を**各呼び出し側が渡すこと**で、`Freeze::Frozen` のアームが `frozen.push` と `notify` を同時に行う(`tick/mod.rs:475-479`)。リトライ上限経路は `tick_observe.rs:387-406` と `tick_notify.rs:121-166` と `cli_tick.rs` が、spawn 上限経路は `tick_launch.rs:182,294` と `tick_confirm_spawn.rs:303` が `summary.frozen` を主張しているので、`Freeze` の取り違えはそこで落ちる。ところが判定上限経路(`observe.rs:230-243` の `record_judge_failure`)は、ユニット側も受け入れ側も**保存後のタスクファイルの実行状態しか見ていない**。ここが `Freeze::NotFrozen` に取り違えられても、`JudgeFailed` が `errors` に積まれるため `is_empty()` も偽のままで、どのテストも落ちない — 「凍結したのに通知されず、サマリーの凍結にも現れない」という at-least-once の破れが素通しになる
  - 提案: `判定失敗は上限と等しい回数では凍結せず超えると凍結する` に `summary.frozen` の主張(等号側は空、超過側は当該ID)を足す。加えて `config_notifying` + 成功する `ScriptedCommandRunner` で1件回し、`summary.notified` と `notify_steps` の並び(`SavedFrozen → RanNotifyCmd → SavedNotifiedAt`)まで見ると、3経路が同じ共通手続きに乗っていることが閉じる

- **[W-002]** `RemnantsUnhandled` を `IssueOutcome::Recorded`(見出し「失敗を記録」)に分類しているため、保存に失敗した tick で「記録していない失敗」が記録済みとして表示される
  - 場所: `crates/pulsen/src/cli/render.rs:101-113`(`issue_outcome`)、`crates/pulsen/src/application/tick/observe.rs:186-194`
  - 理由: `IssueOutcome` の doc は「報告がタスクファイルに**何を残したか**。運用者が次に取る行動はこれで分かれる」と定めている。残存プロセスの報告はタスクファイルに何も残さない事実であり、前ラウンドの修正で `Persisted` によらず常に積むようになった(これ自体は正しい)。その結果、`tick_observe.rs:721-750`(`保存に失敗しても残存の結末は報告される`)が再現する状況では、出力が「失敗を記録(1件): …残存プロセスを誤殺なく同定できませんでした」+「スキップ(1件): …タスクファイルを保存できません」になる。上段は「カウンタを消費し、上限を超えれば同じ tick で凍結する」を意味する見出しなので、実際には `attempt_count` が1つも動いていない tick で運用者を誤らせる。`Skipped`(「書き込みが無く、次の tick がそのまま再試行する」)へ寄せても、tick は残存終了を再試行しないので同じく合わない
  - 提案: `IssueOutcome` に第4の分類(例 `RemnantsLeft` / 見出し「後始末が残っている」)を足し、`RemnantsUnhandled` だけをそこへ振る。`render.rs` は網羅 `match` なので追加漏れは型が防ぎ、テストは既存の `残存プロセスの後始末は同定できたかで書き分けられる` に見出しの主張を1行足すだけで閉じる

- **[W-003]** 走査レベルの「実行状態の異なる複数のタスク」のテストが、本スライスで配線した3アーム(Running / Completed / Stopped)を1つも含んでいない
  - 場所: `crates/pulsen/tests/tick_scan.rs:205-254`(`実行状態の異なる複数のタスクがそれぞれの分岐で1ステップずつ処理される`)
  - 理由: `spec/testcases/execution/tick.md` の走査 正常系「実行状態の異なる複数のタスクがある → 各タスクがそれぞれの分岐で1ステップずつ処理され、サマリーに集約される。exit は 0」に対応するケースだが、並べているのは Corrupt / SnapshotUnreadable / Wait / Pending / Launching の5件のままで、未配線アームだった頃の構成を引き継いでいる。各アームの単体の振る舞いは `tick_observe` / `tick_notify` が押さえているものの、「1回の走査で複数のアームが互いに干渉せず1ステップずつ進む」という走査レベルの主張(たとえば `judged` と `transitioned` と `notified` が同じサマリーに同時に載ること)は、いまどのテストにも無い
  - 提案: このケースに running(exit 0 観測)・completed・未通知 stopped の3件を足し、`judged` / `transitioned` / `notified` がそれぞれ1件ずつ埋まることを主張する。ダブルの台本は `with_read_exit` と `config_notifying` の追加だけで済む

- **[W-004]** `plan.md` の「spec との差分として提起するもの」が出力 DTO を「10フィールド」と書いたままで、本スライス後の実装(11フィールド)と食い違う
  - 場所: `.thread/3/plan.md:97`
  - 理由: spec の tick 出力 DTO は9フィールド、実装は `confirmed_running` に加えて本スライスで `judged` を足した11フィールド(ADR-005 で決着済み)。AC-8 は「spec との食い違いも同じコメントで提起する」と定めており、提起の材料である plan.md の記載が古いままだと、Issue コメントに `judged` が落ちる。ADR-005 は `.thread/3/adr.md:133` にあるので判断自体は残るが、差分台帳としては閉じていない
  - 提案: `plan.md:97` を「`confirmed_running` と `judged` を加えた11フィールド(ADR-094 / ADR-005)」に直す。ステップ15 の記帳でも `judged` を差分として挙げる

#### 参考: Blocker / Warning に至らないと判断した箇所

- **`Settled::by_default` が `JudgeOutcome::Skipped` のアームを持つ**: `default_judgement` は2値しか返さない(doc とテスト `デフォルト判定は終了コード20も失敗として扱う` の両方が主張)。3値の共有型を全域の `match` で受けているだけでパニックも既定の取り違えも無く、`AliveDecision` のときのような「本番で構築されない変種を返り値に残す」構図には当たらない
- **`retry_limit` が `launch.rs` と `observe.rs` で別実装(`applicable_retry_limit().expect(..)` と `snapshot().effective_retry_limit(..)`)**: 前者は分岐が AgentRun を保証する位置、後者は手動修復で動作種別が崩れうる位置で、どちらもその why がコードに書かれている。値は AgentRun / Cleanup で一致し、Wait だけが分かれる
- **`MissingWorkspace` の検査が judge 定義ありの場合だけ走る**: 起きたのは「判定コマンドへ渡す `WORKSPACE` を組めなかった」であり、judge 未定義のステータスにはその事実が無い。spec の手続きD も冒頭検査に不変条件4を含めていない
- **判定・通知の実行中に排他ロックを保持する**: ADR-018 / plan.md が承知のうえで組み込み timeout を置いた設計で、`observe.rs:94-95` と `notify.rs:101-103` がどちらも必ず timeout を渡すため上界が閉じている
- **`errors` の中で凍結・通知の報告が `RunFailed` より前に並ぶ**: `commit` が凍結と通知を先に処理する構造上の順序で、見出しごとに集約されるため表示上の意味は変わらない
- **`try_kill_remnants` を PID 再利用(照合不一致)の場合にも呼ぶ**: spec の手続きD が `DiedWithoutExit` に対して一律に指示しており、`KillIdent` の再利用を検出できない旨はアダプター側の why(`adapter/process.rs:282-286`)と ADR-002 に明記されている。ユースケースの配線としては契約どおり

#### カバレッジ

- 確認: `.thread/3/adr.md`, `.thread/3/plan.md`, `crates/pulsen-conformance/src/doubles/command_runner.rs`, `crates/pulsen-conformance/src/doubles/mod.rs`, `crates/pulsen-conformance/src/doubles/process.rs`, `crates/pulsen-conformance/src/doubles/run_store.rs`, `crates/pulsen-conformance/src/doubles/task_repository.rs`, `crates/pulsen-domain/src/execution/judgement.rs`, `crates/pulsen-domain/src/execution/notification.rs`, `crates/pulsen-domain/src/execution/running.rs`, `crates/pulsen-domain/src/task/degraded.rs`, `crates/pulsen-domain/src/task/task.rs`, `crates/pulsen/examples/agent_probe.rs`, `crates/pulsen/examples/judge_probe.rs`, `crates/pulsen/src/application/tick/confirm_spawn.rs`, `crates/pulsen/src/application/tick/launch.rs`, `crates/pulsen/src/application/tick/mod.rs`, `crates/pulsen/src/application/tick/notify.rs`, `crates/pulsen/src/application/tick/observe.rs`, `crates/pulsen/src/cli/render.rs`, `crates/pulsen/src/cli/tick.rs`, `crates/pulsen/src/cli/wire.rs`, `crates/pulsen/tests/cli_tick.rs`, `crates/pulsen/tests/common/mod.rs`, `crates/pulsen/tests/tick_fixture/mod.rs`, `crates/pulsen/tests/tick_notify.rs`, `crates/pulsen/tests/tick_observe.rs`, `crates/pulsen/tests/tick_scan.rs`
- スキップ: `.thread/3/review/plan-001.md`, `.thread/3/review/review-001-adapter.md`, `.thread/3/review/review-001-domain.md`, `.thread/3/review/review-001-general.md`, `.thread/3/review/review-001-usecase.md`, `.thread/3/review/review-001.md`, `.thread/3/review/triage.md`(7ファイル) — レビューの中間成果物。Phase 8 で削除される(`triage.md` は既決判定の把握のため読んだが、レビュー対象としては扱わない)
- スキップ: `.thread/3/steps.md` — 手順分解であり、受け入れ基準の契約は plan.md が持つ
- スキップ: `.thread/3/testing.md` — 手動確認の記録。自動テストの実効性は testcases と実テストで判定した
- スキップ: `crates/pulsen-conformance/HOOKS.md` — 適合ハーネスの環境フック文書でポート/アダプター観点
- スキップ: `crates/pulsen-conformance/src/command_runner.rs` — CommandRunner の適合ケース本体でポート観点
- スキップ: `crates/pulsen-conformance/src/lib.rs` — 適合スイートのエントリ整備でポート観点
- スキップ: `crates/pulsen-conformance/src/process_controller.rs` — ProcessController の適合ケース本体でポート観点
- スキップ: `crates/pulsen-domain/src/execution/mod.rs` — 再エクスポートのみ
- スキップ: `crates/pulsen-domain/src/execution/port.rs` — ポート契約の定義でポート/ドメイン観点(ユースケースからの呼び出し側は確認済み)
- スキップ: `crates/pulsen-domain/src/execution/value.rs` — 判定プロトコルの値型でドメイン観点
- スキップ: `crates/pulsen-domain/src/task/counters.rs` — カウンタのリセット規則でドメイン観点
- スキップ: `crates/pulsen-domain/src/task/transition.rs` — 遷移エラーの語彙でドメイン観点(`render.rs` の網羅 `match` 側は確認済み)
- スキップ: `crates/pulsen/src/adapter/command_runner.rs` — timeout と後始末の OS 実装でアダプター観点
- スキップ: `crates/pulsen/src/adapter/mod.rs` — モジュール宣言のみ
- スキップ: `crates/pulsen/src/adapter/process.rs` — kill / try_kill_remnants / starttime_of の OS 依存実装でアダプター観点
- スキップ: `crates/pulsen/tests/conformance_command_runner.rs` — 適合スイートの実行側でポート観点
- スキップ: `crates/pulsen/tests/conformance_process_controller.rs` — 同上
