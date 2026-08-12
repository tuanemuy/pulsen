# レビュー(2周目) — Test 観点

対象: PR #11 / ブランチ `issue/2/tick-agent-run-launch` / 契約 `.thread/2/plan.md`

`cargo test --workspace` を実行して確認した(全スイート緑、新規スライス由来のスキップは 0 件。SKIP 行は既存の `tc_port_clock_004 / 005` とダブルのテストのみ)。実装を意図的に壊すミューテーションを 5 件試し、いずれも該当テストが落ちることを確かめた(下記「ミューテーション確認」)。

## Test

### Blockers

なし。

Issue #2 のチェックリストにある TC-* 行(`TC-exec-tick-001〜027 / 028〜055 / 068〜086`、`TC-exec-run-wrapper-001〜027`、`TC-port-run-store-001〜021`、`TC-port-process-controller-001〜005 / 017〜027`、`TC-port-worktree-manager-010〜016`)を ID 単位で突き合わせ、対応するテストが存在し実際に走って通っていることを確認した。適合スイートは 1行 = 1ケース関数 = 1 `#[test]` で台帳と対応し、スキップ許容集合(`SkipBudget`)は環境の能力から導かれていて、`agent_probe` / `spawn_probe` の不在を許容していない。

### Warnings

- **[W-001]** 滞留エージェントによる「ロックFDを継承しない」の検証が、負荷次第で空虚に成立しうる
  - 場所: `crates/pulsen/tests/cli_tick.rs:162`(`滞留するエージェントを起動したままでも次のtickは競合しない`)、モードは `SLEEP: ["sleep", "800"]`(`crates/pulsen/tests/cli_tick.rs:25`)
  - 理由: この主張が意味を持つのは「2回目の tick を打つ時点でラッパーがまだ生きている」ときだけ。エージェントの滞留は 800ms しかなく、待ち合わせは `pid` の出現までなので、負荷の高い環境で 2 回目のプロセス起動が 800ms を超えて遅れるとラッパーは既に終了しており、FD を継承していてもロックは解放済みで緑になる。検出したい欠陥(継承)がある状態でも通ってしまう窓が残る。
  - 提案: 2 回目の tick の前後で「ラッパーがまだ生きている」ことを成果物で確かめる(例: `exit` が**未出現**であることを 2 回目の tick の直後に主張する)か、滞留時間を 2 回目の tick の所要より十分長く取り(現状どおり末尾で `exit` を待つので TempDir 削除との競合は増えない)、空虚な合格を潰す。

- **[W-002]** 適合ケース TC-port-process-controller-002(デタッチ性)が `wait_for_run_files` の戻り値だけに立っており、常に真を返すハーネスでも通る
  - 場所: `crates/pulsen-conformance/src/process_controller.rs:304-316`
  - 理由: 同じスイートの TC-001 は `run_dir_is_empty` の**起動前後の反転**まで主張していて、定数を返すハーネスがどちらかの側で落ちる設計になっている。TC-002 だけがその規律から外れ、観測が `wait_for_run_files` 1点に閉じている。TC-001 が真の観測を強制するので `wait_for_run_files` が定数 `true` でもスイート全体は緑にならない、という間接的な支えしかない(TC-002 単体は空虚)。実際、`detach()` から `process_group(0)` を落とすミューテーションでもこのケースは緑のままだった(落ちたのは受け入れテストの kill 同定子の主張)。
  - 提案: TC-001 と同じ形で `spawn_from_other_process` の**前**に `run_dir_is_empty` が真、後に偽であることを主張する(ハーネス側の追加実装は不要)。

- **[W-003]** `TC-exec-run-wrapper-019`(境界値: exit code 0 / 非0 / 126 / 127 / 128+n)の実バイナリ経路が 0 / 3 / 42 のみ
  - 場所: `crates/pulsen/tests/cli_wrapper.rs:134`(`エージェントの終了コードはそのままexitファイルに現れる`)
  - 理由: 126 / 127 はラッパー自身が「起動不能」「コマンド不在」として書く符号化値と同じ値で、「エージェントがその値で終わった場合も素通しする」ことは実バイナリでは確かめていない(素通し自体はダブルのユースケーステスト `run_wrapper.rs:261` が 0 / 1 / 126 / 127 / 134 で確認済み)。行の主眼が境界値である以上、実経路でも同じ値域を通しておきたい。
  - 提案: 既存のループの値を `[0, 3, 42, 126, 127, 134]` に広げる(`agent_probe exit <n>` で足りる)。

- **[W-004]** `TC-exec-run-wrapper-014 / 015 / 016`(実行不能 → 126 / シグナル死 → 128+n / ログを開けない → 126)に実バイナリでの合成経路がない
  - 場所: `crates/pulsen/tests/cli_wrapper.rs`(該当ケースなし)。現状は `run_agent` 側の符号化を適合スイート(`tc_port_process_controller_023 / 024 / 025`)とアダプターのユニットテスト(`crates/pulsen/src/adapter/process.rs:797`、`128+6` を固定)で、`run_agent` の戻り値が `exit` にそのまま載ることをダブルのユースケーステストで、それぞれ別々に確認している。
  - 理由: 合成の証明が 2 層に分かれており、実バイナリで閉じているのは 127(コマンド不在)と 126(worktree 不在)の 2 経路だけ。層をまたぐ結線が壊れたときに落ちるのは、たまたまこの 2 経路に乗っている場合に限られる。
  - 提案: シグナル死は `agent_probe abort` を使った実バイナリ 1 ケースで閉じられる(`exit` に非0が載ることまで)。権限に依存する 014 / 016 は適合スイートと同じくスキップ判定が要るので、合成の裏付けとしては 015 の 1 件を足すだけでも足りる。

- **[W-005]** F6(ブランチが base から作られる)の受け入れ側の主張が「ブランチが存在する」までしかない
  - 場所: `crates/pulsen/tests/cli_tick.rs:95-98`、`crates/pulsen/tests/cli_tick.rs:450-455`
  - 理由: 「base の先端から作られた」ことは適合ケース TC-port-worktree-manager-010 が主張し、tick が渡す base はユースケーステスト(`tick_launch.rs:56-62` の `WorktreeManagerCall::Create { base }`)が主張しているので二重の穴ではないが、受け入れ側の名前(「base から作られたブランチが存在する」)と主張(存在のみ)が食い違っている。
  - 提案: `git::branch_tip(repo, &branch_of(&id)) == git::branch_tip(repo, "main")` を主張する(ヘルパーは既にある)。

- **[W-006]** `HOOKS.md` の「環境で走らなくなりうる行」表が本文と矛盾する
  - 場所: `crates/pulsen-conformance/HOOKS.md:28` と `crates/pulsen-conformance/HOOKS.md:41`
  - 理由: 28 行目は「TC-port-process-controller-003 / 005 は別ハンドルの注入で確定的に走るため**この表に現れない**」と書くが、41 行目の行は `TC-port-process-controller-001 / 002 / 003 / 017〜027` として 003 を表に載せている。この表はスキップ許容集合を組むときの根拠なので、どの行がどの能力に依存するかが読み取れないと宣言が実態からずれる(テストファイル側の `allowed_skips()` は 003 を許容しておらず、実装は正しい)。
  - 提案: 41 行目の列挙から 003 を外すか、28 行目の但し書きを「`agent_probe` には依存するが権限・root には依存しない」と直す。

### カバレッジ

確認(テスト観点で差分・内容を精読したもの):

- `crates/pulsen/tests/tick_scan.rs`, `crates/pulsen/tests/tick_launch.rs`, `crates/pulsen/tests/tick_confirm_spawn.rs`, `crates/pulsen/tests/tick_fixture/mod.rs`
- `crates/pulsen/tests/run_wrapper.rs`, `crates/pulsen/tests/cli_wrapper.rs`, `crates/pulsen/tests/cli_tick.rs`, `crates/pulsen/tests/cli_usage.rs`, `crates/pulsen/tests/register_task.rs`
- `crates/pulsen/tests/conformance_run_store.rs`, `crates/pulsen/tests/conformance_process_controller.rs`, `crates/pulsen/tests/conformance_worktree.rs`
- `crates/pulsen/tests/common/mod.rs`, `crates/pulsen/tests/common/git.rs`
- `crates/pulsen-conformance/src/run_store.rs`, `crates/pulsen-conformance/src/process_controller.rs`, `crates/pulsen-conformance/src/worktree_manager.rs`, `crates/pulsen-conformance/src/lib.rs`, `crates/pulsen-conformance/HOOKS.md`
- `crates/pulsen-conformance/src/doubles/run_store.rs`, `crates/pulsen-conformance/src/doubles/process.rs`, `crates/pulsen-conformance/src/doubles/clock.rs`, `crates/pulsen-conformance/src/doubles/tests.rs`, `crates/pulsen-conformance/src/doubles/task_repository.rs`, `crates/pulsen-conformance/src/doubles/worktree.rs`, `crates/pulsen-conformance/src/doubles/mod.rs`
- `crates/pulsen/examples/agent_probe.rs`, `crates/pulsen/examples/spawn_probe.rs`
- `crates/pulsen/src/application/run_wrapper.rs`, `crates/pulsen/src/application/tick/confirm_spawn.rs`, `crates/pulsen/src/adapter/run_store.rs`, `crates/pulsen/src/adapter/worktree.rs`, `crates/pulsen/src/adapter/process.rs`(ミューテーションの対象として該当箇所を精読・付随するユニットテストを確認)
- `crates/pulsen-domain/src/execution/launching.rs`, `crates/pulsen-domain/src/execution/value.rs`, `crates/pulsen-domain/src/task/task.rs`, `crates/pulsen-domain/src/task/planner.rs`, `crates/pulsen-domain/src/task/path.rs`, `crates/pulsen-domain/src/task/counters.rs`, `crates/pulsen-domain/src/task/attempt.rs`, `crates/pulsen-domain/src/task/failure.rs`, `crates/pulsen-domain/src/definition/template.rs`(いずれもユニットテストの網羅を確認)
- `.thread/2/plan.md`(契約)

スキップ:

- `.thread/2/adr.md`, `.thread/2/steps.md`, `.thread/2/progress.md`, `.thread/2/review/review-001*.md`, `.thread/2/review/triage.md` — 計画・記録の成果物であり、ゼロベースのレビュー指示により参照しない(前回レビューは読んでいない)
- `.thread/2/testing.md` — 手動確認の計画。テスト方針(自動テストとの役割分担)の確認のため冒頭のみ読み、内容の妥当性は Test 観点の対象外(手動確認は実行していない)
- `crates/pulsen/src/application/tick/mod.rs`, `crates/pulsen/src/application/tick/launch.rs`, `crates/pulsen/src/application/mod.rs` — 実装。分岐の網羅性はユースケーステスト側から突き合わせ、コードの構造はアーキ/ユースケース観点に委ねる
- `crates/pulsen/src/cli/args.rs`, `cli/mod.rs`, `cli/tick.rs`, `cli/wire.rs`, `cli/wrapper.rs` — 実装。受け入れテストからの観測で挙動を確認済み。`cli/render.rs` はテスト関数名の一覧のみ確認(サマリー表示・失敗案内のユニットテストが存在すること)
- `crates/pulsen/src/adapter/mod.rs` — 再輸出のみ
- `crates/pulsen-domain/src/execution/mod.rs`, `execution/port.rs`, `task/mod.rs`, `task/transition.rs`, `definition/agent.rs` — 型・ポート宣言。テストは `task.rs` / `launching.rs` 側に集約されており、そこで確認済み

### ミューテーション確認(実装を壊して落ちることを確かめた)

| 壊した箇所 | 落ちたテスト |
|---|---|
| `application/run_wrapper.rs` の starttime / pid の書き込み順序を入れ替え | `run_wrapper.rs` の順序3件 |
| `application/tick/confirm_spawn.rs` でマーカー書き込み失敗を無視して再確認へ進める | `マーカーを書けなければ状態を変更せず報告してスキップする` |
| `adapter/process.rs` の `detach()` から `process_group(0)` を除去 | `cli_tick.rs` の `次のtickはpidの出現をもってrunningへ取り込む`(kill 同定子。適合スイートは緑のまま = W-002) |
| `adapter/run_store.rs` の書き込みをアトミック置換から直接書き込みへ | `tc_port_run_store_016`(並行観測)・`tc_port_run_store_017`(失敗時の残骸) |
| `adapter/worktree.rs` の `create` で登録ブランチの一致検査を除去 | `tc_port_worktree_manager_015` |
| `LaunchingClassifier::classify` の猶予超過を `>` から `>=` へ | ドメイン2件 + `tick_confirm_spawn.rs` 3件 |
