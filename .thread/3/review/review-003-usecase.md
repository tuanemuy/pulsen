### Use Case / CLI

ゼロベースで `spec/usecases/execution.md`(共通手続き notify・処理フロー・手続きD)、`spec/flows/index.md`(F1 / F3 / F8)、`spec/pages/index.md#tick`、`spec/testcases/execution/tick.md`(走査・手続きD)と実装を突き合わせた。**Blocker は問題点ゼロ。** 以下は精度の指摘2件のみで、いずれも振る舞いを変えない。

#### Blockers

なし。

#### Warnings

- **[W-001]** `Freeze` の why コメントが、本 PR が入れた経路を「これから入るもの」として参照している
  - 場所: `crates/pulsen/src/application/tick/mod.rs:518-529`
  - 理由: 「既に凍結しているタスクを別の理由で保存する経路(**#3 の** catch-up 通知は `mark_notified` した Stopped を保存する)」とあるが、その経路は本 PR の `crates/pulsen/src/application/tick/notify.rs:57`(`commit(&notified, Freeze::NotFrozen, ...)`)として既に存在する。読み手は「まだ無い将来の経路への備え」と読み、`Freeze::NotFrozen` を渡している現在の呼び出し元を検算できない。ラウンド1で同種の指摘(`mod.rs:533-535` の前提集合が実際の呼び出し元と食い違う)を fix しており、扱いを揃えるべき箇所。
  - 提案: Issue 番号ではなく現在の呼び出し元(`notify.rs` の `mark_notified` 後の保存)を指す形に書き換える。制約そのものは正しいので、参照先だけを実在のコードへ寄せる。

- **[W-002]** 「残存終了は失敗の確定より先」の主張が、アサーションでは検証されていない
  - 場所: `crates/pulsen/tests/tick_observe.rs:663-674`
  - 理由: `assert_eq!(harness.processes.calls(), vec![StarttimeOf, TryKillRemnants], "残存終了は失敗の確定より先に試みる")` は `ProcessController` 内の2呼び出しの前後しか見ておらず、「`try_kill_remnants` → `fail_run` → `save`」というポートをまたぐ順序(spec 手続きD 3.`DiedWithoutExit`)は主張していない。`save` を先に行う実装でもこのアサーションは緑で通る。この順序には意味がある — 先に failed を永続化して残存終了の前にクラッシュすると、次の tick が同じ worktree で新 attempt を起こしながら残存プロセスが生きたままになる(`kill` 失敗で状態を変更しない判断と同じ危険)。ラウンド2の W-004(`save_degraded` 経路が `RecordSeq` の外にあり `mark_notified` 先行を素通しする)と同じ形の穴が、`ProcessController` 側に残っている。
  - 提案: どちらかに寄せる。(a) `ScriptedProcessController` に `RecordSeq` を足して `tick_notify.rs` の `notify_steps` と同じ形で1本の列として主張する(この場合 `.thread/3/adr.md` ADR-014 の「順序の契約が無いメソッドには付けない」の対象範囲を1行直す)。(b) 順序の契約はコードの why に残し、テストのメッセージを実際に検証している内容(「照合が Dead なら残存終了を試みる」)へ弱める。

#### 確認した点(問題なしと判断した箇所)

- **配線に徹しているか**: `observe` / `judge` / `notify` はいずれも「ポートで観測 → ドメインで判断 → ポートで実行」に収まっている。判定の結論は `JudgementService::{default_judgement, interpret_judge_completion}`、通知の成否は `NotificationService::interpret_notify_completion`、生存分類は `RunningClassifier::classify_alive` と `IdentityCheck::check`。ユースケース側に残るのは `Settled` / `Delivery` / `RemnantsLeft` への写像だけで、いずれも ADR-010 / ADR-011 が明示的に置き場所を決めたもの。`MissingWorkspace`(ADR-008)により、遷移関数を呼ばずにドメインのエラー値を組み立てる経路は残っていない。
- **共通手続き notify の順序**: `commit` が `save` → `Freeze::Frozen` なら `frozen` 計上 → `notify` → `Exited(0)` のときだけ `mark_notified` → `save` の順に固定されている(`mod.rs:472-492`、`notify.rs:44-65`)。`notify_cmd` が None のとき `Delivery::NotConfigured` で何も書かない(`notify.rs:49`)。`RecordSeq` による順序検証は実効的で、通知を先に起動する実装・`mark_notified` を先に保存する実装のどちらも `notify_steps` の完全一致比較で落ちる(`tick_notify.rs:183-228` / `326-361` / `390-435`)。`save_degraded` 経路にも採番が掛かっている。
- **手続きD の分岐**: exit が Some のとき `RunningDecision::Judge` を値にしてから分岐し、`starttime_of` を呼ばない(`observe.rs:59-66`、`tick_observe.rs:155-164` が `processes.calls()` の空で主張)。`kill` 失敗は `TickIssue::KillFailed` の報告のみで `saved()` が空(`tick_observe.rs:605-631`)。`starttime_of` の `Err(Io)` も `ObservationFailed` の報告のみ(`tick_observe.rs:469-491`)。不変条件2 / 3 の破れは `MissingCurrentAttempt` / `MissingProcessIdent` に分かれ、`read_exit` より前に検査される(`tick_scan.rs` の該当ケースが `runs.calls()` の空も主張)。
- **報告の分類と表示**: `RunFailureCause` の4値は判断主体で割れており、判定コマンドが失敗と判定した場合にエージェントの exit 0 が根拠として表示されない(`render.rs:257-278`、`tick_observe.rs:282-310`)。`RemnantsLeft::of` は `Killed` を `None` に写して報告分類から型で締め出している。`IssueOutcome::CleanupLeft`(「後始末が残っている」)へ振るのは `RemnantsUnhandled` だけで、ADR-017 の規則どおり。保存に失敗した tick で `SaveFailed`(スキップ)と `RemnantsUnhandled`(後始末)が別の見出しに分かれることをテストが押さえている(`render.rs` の `保存できなかった残存の報告は記録した失敗の見出しに現れない`)。`NotifyFailed` を「スキップ」に置くのも「書き込みが無く次の tick が再試行する」という見出しの語義と一致する。
- **走査レベルの分岐**: `Corrupt` は報告のみ、`SnapshotUnreadable` は実行状態によらず報告を積んだうえで未通知凍結だけが `notify_degraded` へ進む(ADR-012)。`notify_cmd` 未定義でも報告が消えないことを `tick_scan.rs` の `通知コマンドが未定義でもスナップショット破損の未通知凍結は報告される` が守る。`Branch::Notify` / `AlreadyNotified` の分割はテスト済み(`通知済みの凍結には何もしない`)。
- **1タスク1tick1ステップ**: `complete_run` の tick では `transitioned` が空(`tick_observe.rs:91-110`)、`advance` の tick ではラッパーを起動しない(`tick_scan.rs` の `判定確定のタスクは次のステータスへ遷移して起動待ちへ戻る`)、`skip_run` の次の tick は同じ exit を再判定せず新 attempt を起こす(`tick_observe.rs:756-792`)。受け入れテスト側も `launch_and_confirm` → 判定 → 遷移の順に tick を刻んでいる。
- **エラーハンドリングと exit code**: `process` はタスク1件ごとに `errors` へ積んで走査を続け、`execute` が `TickError` を返すのは `LockError::Failed` と `list_active` の Io のみ。`cli/mod.rs:42-55` が `TickOutcome::{Skipped, Completed}` の両方で `exit::SUCCESS` を返す。受け入れテストが `NotifyFailed` / `MissingProcessIdent` を含む tick で `assert_succeeded()` を主張している。
- **冪等性**: `KeepRunning` は書き込みを1回も起こさず(`tick_observe.rs:494-513`)、連続 tick で `saved()` が空(`tick_scan.rs` の `状態が変化しないタスク群には連続実行しても書き込みが発生しない` に running を追加済み)。判定確定の `save` 失敗後に同じ結論が再導出されること・通知失敗後の再通知・notify_cmd の後付け catch-up も、ユースケース層と受け入れテストの両方で押さえている。
- **サマリー表示**: 本スライスが足した書き込み経路は `complete_run` → `judged`、`skip_run` → `skipped_back`、`advance` → `transitioned`、`fail_run` → `errors`(+`frozen`)、`record_judge_failure` → `errors`(+`frozen`)、`mark_notified` → `notified` で全て埋まる(ADR-005 / ADR-092)。`TickSummary::is_empty` も新フィールドを含む。走査レベルで6分岐が同じサマリーへ集約されることを `tick_scan.rs` の `実行状態の異なる複数のタスクが…` が主張しており、CLI 側の並び順も `render.rs` の全項目テストが固定している。
- **排他ロックの保持時間**: tick 全体のブロックは「`judge_timeout`(既定60秒)× 判定対象タスク数 + `NOTIFY_TIMEOUT`(60秒)× 通知対象タスク数 + 終了操作1回あたり最大 `TERMINATION_GRACE` × 2 = 4秒 × kill / 残存終了の対象タスク数」で、いずれも上限つき。ADR-015 の Consequences が終了操作ぶんを明示し「判定の timeout に対して十分小さく取る」と結論づけており、設計上の帰結は正しく把握されている。`try_kill_remnants` は列挙に失敗した時点で `NotIdentifiable` を返して終了操作を起動しないため、全 running タスクが死亡している再起動直後のような場面でも待ちは積み上がらない。本スライスで新しい緩和を入れないという plan.md の方針とも整合する。
- **セキュリティ**: 判定・通知コマンドはシェル非経由で `PlainCommand` のトークンをそのまま渡し(`CommandRunner` の契約と適合スイート)、env は `TASK_ID` / `WORKFLOW` / `TASK_STATUS`(通知)と `TASK_ID` / `WORKSPACE` / `EXIT_CODE` / `RUN_DIR`(判定)に限る。ユースケース側でコマンド文字列を組み立てる経路は無い。
- **弁明・経緯の残留**: `application/tick/` 配下・`cli/render.rs`・新規テストのいずれにも、指摘への弁明や修正の経緯を示す記述は無い(残っているのは why / why not と分類の理由のみ)。

#### カバレッジ

- 確認: `crates/pulsen/src/application/tick/mod.rs`, `crates/pulsen/src/application/tick/observe.rs`, `crates/pulsen/src/application/tick/notify.rs`, `crates/pulsen/src/application/tick/launch.rs`, `crates/pulsen/src/application/tick/confirm_spawn.rs`, `crates/pulsen/src/cli/render.rs`, `crates/pulsen/src/cli/tick.rs`, `crates/pulsen/src/cli/wire.rs`, `crates/pulsen/tests/tick_observe.rs`, `crates/pulsen/tests/tick_notify.rs`, `crates/pulsen/tests/tick_scan.rs`, `crates/pulsen/tests/tick_fixture/mod.rs`, `crates/pulsen/tests/cli_tick.rs`, `crates/pulsen/tests/common/mod.rs`, `crates/pulsen/examples/judge_probe.rs`, `crates/pulsen-conformance/src/doubles/mod.rs`, `crates/pulsen-conformance/src/doubles/task_repository.rs`, `crates/pulsen-conformance/src/doubles/command_runner.rs`, `crates/pulsen-conformance/src/doubles/run_store.rs`, `crates/pulsen-domain/src/execution/port.rs`, `crates/pulsen-domain/src/execution/notification.rs`, `.thread/3/plan.md`, `.thread/3/adr.md`
- スキップ: `.thread/3/review/`(15ファイル) — Phase 8 で削除される中間成果物
- スキップ: `.thread/3/steps.md`, `.thread/3/testing.md` — 手順書・手動確認記録で General 観点
- スキップ: `crates/pulsen-domain/src/execution/judgement.rs`, `crates/pulsen-domain/src/execution/running.rs`, `crates/pulsen-domain/src/execution/value.rs`, `crates/pulsen-domain/src/execution/mod.rs`, `crates/pulsen-domain/src/task/task.rs`, `crates/pulsen-domain/src/task/degraded.rs`, `crates/pulsen-domain/src/task/transition.rs`, `crates/pulsen-domain/src/task/counters.rs` — ドメインの遷移・分類ロジックで Domain 観点(呼び出し側から見た契約の整合だけ確認)
- スキップ: `crates/pulsen/src/adapter/command_runner.rs`, `crates/pulsen/src/adapter/process.rs`, `crates/pulsen/src/adapter/mod.rs` — ポート実装で Adapter 観点(ロック保持時間の見積もりに要る `TERMINATION_GRACE` / `POLL_INTERVAL` の値だけ参照)
- スキップ: `crates/pulsen-conformance/src/command_runner.rs`, `crates/pulsen-conformance/src/process_controller.rs`, `crates/pulsen-conformance/src/lib.rs`, `crates/pulsen-conformance/src/doubles/process.rs`, `crates/pulsen-conformance/HOOKS.md`, `crates/pulsen/tests/conformance_command_runner.rs`, `crates/pulsen/tests/conformance_process_controller.rs`, `crates/pulsen/examples/agent_probe.rs` — ポート適合スイートとそのフィクスチャで Adapter 観点
