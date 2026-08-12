# レビュー002: Use Case 観点

対象: PR #11 (`issue/2/tick-agent-run-launch`) / 契約: `.thread/2/plan.md`

## Use Case

### Blockers

なし。

spec の処理フロー(1〜9)・手続きA・手続きC・RunWrapper の順序、マーカー順序プロトコル、
1タスク失敗の隔離、スコープ外手続きの不在は、いずれも `spec/usecases/execution.md` と
plan.md のとおり成立していることを確認した。二重起動の競合窓も、ラッパー
(starttime → pid → マーカー確認)と tick(マーカー書き込み → pid 再確認)の交錯を
4通り追った限り残っていない。

### Warnings

- **[W-001]** 未配線アームが「起動しない」ことを主張するテストが1件も無い
  - 場所: `crates/pulsen/src/application/tick/mod.rs:287-302` / `crates/pulsen/tests/tick_scan.rs:6`
  - 理由: `tick_scan.rs` の冒頭は「本スライスで配線しないアーム(Cleanup / Running /
    Completed / Stopped)には期待を持たせない — 期待を書くと Issue #3 / #6 がアームを
    埋めた時点で書き換えになる(ADR-065)」としており、実際に該当タスクを走査する
    テストが存在しない。ADR-065 の理由づけは**肯定的な**期待(何が起きるか)には
    当てはまるが、「起動されない」という否定的な主張はスライスをまたいで永久に真で、
    #3 / #6 がアームを埋めても書き換えにならない。しかもこの主張は安全性そのもの —
    `Running` が誤って `Branch::Launch` に落ちれば、エージェントが実行中のタスクに
    対して新しい attempt で2本目のラッパーが起動する。新 attempt の run ディレクトリ
    には無効化マーカーが無いので、本スライスの中核であるマーカープロトコルは
    この誤りを一切防がない。`Stopped`(凍結後の再起動)・Pending × `Cleanup`
    (終端処理待ちのタスクにエージェントを起動)も同種で、plan.md の手動確認 TC-20 が
    「T20h には何も行われない」を期待している対象でもある。現状の実装は正しいが、
    正しさを守るものが `branch_of` の目視だけになっている。
  - 提案: `tick_scan.rs` に「`Running` / `Completed` / `Stopped` / Pending × `Cleanup`
    のタスクは、いずれも `tasks.saved()` が空・`worktrees.calls()` と `runs.calls()` と
    `processes.calls()` が空のまま終わる」1件を足す。サマリーの中身には期待を置かない
    (ADR-065 の趣旨を保ったまま、書き込みと起動が起きないことだけを固定できる)。

- **[W-002]** `commit` が「保存後の実行状態が Stopped であること」から `frozen` を導出しており、
  ADR-066 が定めた #3 の拡張点でそのまま誤集計になる
  - 場所: `crates/pulsen/src/application/tick/mod.rs:309-325`
  - 理由: `frozen` の語義は「このtickで凍結した」(spec 出力DTO「凍結」)だが、実装は
    「保存した結果が Stopped だった」で判定している。ADR-066 は「#3 は共通手続き notify の
    呼び出しをこの関数の中に足すだけでよい」と設計しているところ、共通手続き notify の
    ステップ4 は `mark_notified(now)` → `save` であり、その保存対象は
    `Stopped { notified_at: Some(..) }` のままである。この保存が `commit` を通れば、
    **何tick も前に凍結したタスクが、通知に成功したtickで毎回 `frozen` に載る**。
    spec 手順7(`Stopped` アーム)の catch-up 通知も同じ経路を通るため、#3 のマージ時に
    ほぼ確実に踏む。今のスライスでは Stopped を保存する経路が遷移直後しか無いため
    表面化しないだけで、判定の根拠が最初から間違っている。
  - 提案: 凍結の判定を状態の観測ではなく遷移の結果に移す。`commit` に「この保存が
    凍結を意味するか」を呼び出し側から渡す(例: `commit_transition(&task, Outcome::Frozen)`)
    か、`freeze` 相当の専用関数を分けて `mark_notified` の保存はそこを通さない。

- **[W-003]** 書き込みを伴う「記録した失敗」が、CLI では「スキップ」という見出しで表示される
  - 場所: `crates/pulsen/src/cli/render.rs`(`tick_summary` の `"  スキップ({}件):\n"`)
  - 理由: `TickIssue::WorktreeCreateFailed` / `CommandExpansionFailed` / 猶予経路の
    `SpawnFailed` は、いずれも**タスクファイルに書き込んだ後**に積まれる報告で、
    `attempt_count` / `spawn_fail_count` を消費し、上限を超えれば同じtickで凍結する。
    これを「スキップ」と表示すると、「何も記録されず次tickでそのまま再試行される」
    (`CorruptTaskFile` / `RunFileUnreadable` / `MarkerWriteFailed` / `MissingCurrentAttempt`
    と同じ扱い)と読める。ADR-084 は「cron 運用ではこの出力が唯一の窓」を根拠に
    「処理対象なし」の判定を設計しており、その唯一の窓が、カウンタを消費した失敗と
    消費しなかったスキップを同じ語で束ねている。ADR-086 が `errors` に載せる判断をした
    こと自体は spec の `errors` の定義に収まるが、見出しの語まで「スキップ」に
    寄せる根拠は無い。
  - 提案: 見出しを中立な語(「報告」等)にするか、記録した失敗(`WorktreeCreateFailed` /
    `CommandExpansionFailed` / 猶予経路の `SpawnFailed`)を「失敗を記録」の別見出しに
    分ける。`render.rs` の既存テストは見出し文字列を直接主張しているので、
    どちらでも1箇所の変更で固定できる。

- **[W-004]** `TickIssue::SpawnFailed` が結末の異なる2つの経路を兼ねている
  - 場所: `crates/pulsen/src/application/tick/mod.rs:131-137` /
    `tick/launch.rs:70-73` / `tick/confirm_spawn.rs:98-117`
  - 理由: (a) `spawn_wrapper` の同期エラー(状態不変・`Launching` 維持・カウンタ非消費・
    猶予経路が後で分類する)と、(b) 猶予超過の再確認で確定した spawn 失敗
    (`Pending` 復帰・`spawn_fail_count` 加算・上限超過なら凍結)が同じ分類になっている。
    運用上は「まだ起動中かもしれない」と「起動をあきらめてカウンタを1つ消費した」で
    次に取る行動が違う。テストも `tick_launch.rs:355` と `tick_confirm_spawn.rs:221` で
    同じ `TickIssue::SpawnFailed` を主張するため、実装が両者を取り違えても
    (例えば同期エラー側で `record_spawn_failure` を呼んでも)分類の主張は緑のままになる
    — 取り違えは plan.md「リスクと注意点」が名指しした並走の失敗モードそのもの。
    現状は saved の実行状態を別途主張しているので落ちるが、分類が防いでいるわけではない。
  - 提案: 猶予経路の確定を別の変種(例: `SpawnNotObserved`)にして、分類だけで
    「カウンタを消費したか」が読めるようにする。ADR-086 は `errors` に載せることを
    決めただけで、1つの変種に畳むことまでは根拠づけていない。

- **[W-005]** ADR-084 の不変(書き込んだ経路は必ずサマリーを埋める)が `commit` では担保されず、
  呼び出し側の規律に依存している
  - 場所: `crates/pulsen/src/application/tick/mod.rs:10-11, 309-325` /
    `tick/launch.rs:94-98`
  - 理由: モジュールの doc は「タスクファイルに書き込んだ経路は、必ずサマリーのいずれかの
    フィールドを埋める(ADR-084 / ADR-086)」と不変として書き、ADR-086 は
    「書き込みを行った tick が必ずサマリーに現れ、ADR-084 の不変が**構成として**成立する」
    としている。しかし `commit` は成功時に `frozen` 以外を何も積まず、集計は呼び出し側に
    委ねられている。とくに `confirm_workspace` の `save`(`ensure_workspace` の成功経路)は
    自分の集計先を持たず、「この後に必ず起動記録・展開失敗・保存失敗のいずれかが積まれる」
    という**間接的な**理由でしか不変を満たしていない。#6 の手続きB のように
    「保存して終わる」中間ステップが増えると、構成では止められない。
  - 提案: `commit` の署名で集計先を要求する(W-002 の提案と同じ形にできる)か、
    少なくとも「ワークスペースを確定しただけで終わるtickは存在しない」ことを
    テストで固定する。

### カバレッジ

- 確認: `.thread/2/plan.md`, `.thread/2/adr.md`,
  `crates/pulsen/src/application/mod.rs`, `crates/pulsen/src/application/tick/mod.rs`,
  `crates/pulsen/src/application/tick/launch.rs`,
  `crates/pulsen/src/application/tick/confirm_spawn.rs`,
  `crates/pulsen/src/application/run_wrapper.rs`,
  `crates/pulsen/src/cli/args.rs`, `crates/pulsen/src/cli/mod.rs`,
  `crates/pulsen/src/cli/render.rs`, `crates/pulsen/src/cli/tick.rs`,
  `crates/pulsen/src/cli/wire.rs`, `crates/pulsen/src/cli/wrapper.rs`,
  `crates/pulsen-domain/src/execution/launching.rs`,
  `crates/pulsen-domain/src/execution/port.rs`(ポート表とユースケースの呼び出しの1:1、
  スコープ外メソッドの不在),
  `crates/pulsen-domain/src/task/task.rs`(`current_status_def` / `record_launching` /
  `confirm_running` の前提と事後条件),
  `crates/pulsen-conformance/src/doubles/mod.rs`,
  `crates/pulsen-conformance/src/doubles/clock.rs`,
  `crates/pulsen-conformance/src/doubles/process.rs`,
  `crates/pulsen-conformance/src/doubles/run_store.rs`,
  `crates/pulsen-conformance/src/doubles/task_repository.rs`,
  `crates/pulsen/tests/tick_fixture/mod.rs`, `crates/pulsen/tests/tick_scan.rs`,
  `crates/pulsen/tests/tick_launch.rs`, `crates/pulsen/tests/tick_confirm_spawn.rs`,
  `crates/pulsen/tests/run_wrapper.rs`, `crates/pulsen/tests/cli_tick.rs`,
  `crates/pulsen/tests/cli_wrapper.rs`, `crates/pulsen/tests/cli_usage.rs`
- スキップ: `.thread/2/review/review-001-adapter.md`,
  `.thread/2/review/review-001-architecture.md`, `.thread/2/review/review-001-domain.md`,
  `.thread/2/review/review-001-test.md`, `.thread/2/review/review-001-usecase.md`,
  `.thread/2/review/review-001.md`, `.thread/2/review/triage.md` —
  ゼロベースでレビューする指示により読まない
- スキップ: `.thread/2/progress.md`, `.thread/2/steps.md`, `.thread/2/testing.md` —
  工程記録・手動確認手順。契約は plan.md に閉じており、Use Case の判定材料にならない
- スキップ: `crates/pulsen-conformance/HOOKS.md`,
  `crates/pulsen-conformance/src/lib.rs`,
  `crates/pulsen-conformance/src/process_controller.rs`,
  `crates/pulsen-conformance/src/run_store.rs`,
  `crates/pulsen-conformance/src/worktree_manager.rs`,
  `crates/pulsen-conformance/src/doubles/tests.rs`,
  `crates/pulsen-conformance/src/doubles/worktree.rs`,
  `crates/pulsen/tests/conformance_process_controller.rs`,
  `crates/pulsen/tests/conformance_run_store.rs`,
  `crates/pulsen/tests/conformance_worktree.rs` —
  ポート契約の適合スイートとそのハーネス。Adapter / Test 観点の担当で、
  ユースケースはポートの契約を前提として使うだけ
- スキップ: `crates/pulsen/src/adapter/mod.rs`, `crates/pulsen/src/adapter/process.rs`,
  `crates/pulsen/src/adapter/run_store.rs`, `crates/pulsen/src/adapter/worktree.rs`,
  `crates/pulsen/examples/agent_probe.rs`, `crates/pulsen/examples/spawn_probe.rs` —
  アダプター実装と検証用プログラム。Adapter 観点の担当
- スキップ: `crates/pulsen-domain/src/definition/agent.rs`,
  `crates/pulsen-domain/src/definition/template.rs`,
  `crates/pulsen-domain/src/execution/mod.rs`,
  `crates/pulsen-domain/src/execution/value.rs`,
  `crates/pulsen-domain/src/task/attempt.rs`, `crates/pulsen-domain/src/task/counters.rs`,
  `crates/pulsen-domain/src/task/failure.rs`, `crates/pulsen-domain/src/task/mod.rs`,
  `crates/pulsen-domain/src/task/path.rs`, `crates/pulsen-domain/src/task/planner.rs`,
  `crates/pulsen-domain/src/task/transition.rs` —
  ドメインの値・遷移・導出。Domain 観点の担当で、ユースケースからは呼び出し側として
  必要な範囲(上記「確認」の3関数と `LaunchingClassifier`)だけを追った
- スキップ: `crates/pulsen/tests/common/git.rs`, `crates/pulsen/tests/common/mod.rs`,
  `crates/pulsen/tests/register_task.rs` —
  受け入れテストのフィクスチャと既存スライスのテスト。Test 観点の担当
