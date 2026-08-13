# レビュー006 — Test

対象: PR #11(`issue/2/tick-agent-run-launch`)/ 契約: `.thread/2/plan.md`

`cargo test --workspace` を実行して確認した(exit 0・全ターゲット緑・`ignored` 0・許容集合外のスキップ 0)。
実行環境: macOS(darwin 25.4.0)・非 root・TMPDIR は git リポジトリの外。権限依存の区分 C 行
(`tc_port_run_store_007` / `017`、`tc_port_process_controller_023` / `025`、`tc_exec_run_wrapper_014` / `016`)と
シグナル死の `tc_port_process_controller_024` はいずれも**走っている**(スキップではない)。

| ターゲット | 件数 |
|---|---|
| `conformance_run_store` | 22(台帳21 + 追加1) |
| `conformance_process_controller` | 16(identity/agent 13 + spawn 3) |
| `conformance_worktree` | 17(既存9 + 新規7 + 追加1) |
| `tick_scan` / `tick_launch` / `tick_confirm_spawn` / `run_wrapper` | 12 / 19 / 21 / 9 |
| `cli_tick` / `cli_tick_missing_cwd` / `cli_wrapper` / `cli_usage` | 16 / 1 / 15 / 6 |
| `pulsen-domain` ユニット | 220 |

## Test

### Blockers

なし。

### Warnings

なし。

## 確認した観点

### テストケース定義とのカバレッジ(ID 単位)

Issue #2 のチェックリストの TC-* 行 84件(tick 51 / run-wrapper 27 は重複を除き実数、run-store 21・
process-controller 16・worktree-manager 7)を ID 単位で突き合わせ、**未消化の行は無かった**。
特に確認した対応:

- **走査と分岐**: 001/002/004/006/007/012/013/014 は `tick_scan.rs` と `cli_tick.rs`、015/016/018 は
  ダブル(実アダプターでは外から作れない)、017/019/027 は `cli_tick.rs` の実バイナリ経路。
- **手続きA**: 028〜055 のうち展開失敗の5経路(039〜043)は `expansion_failures()` の5ハーネスが
  それぞれ別の分岐(実効エージェント名 / config 不在 / `RawAgentDefinition::parse` / `MissingSkillInput` /
  `ExpansionError`)に確実に落ちる構成になっている。境界の 048(`retries: 0`)/ 049(等号)/ 050(上限+1)も個別にある。
- **手続きC**: 068〜086 のすべて。境界3件(077/078/079)は `SettableClock` で 30 / 31 / −3600 秒を作り、
  実時間に依存しない。082/083 の「両順序で並走が起きない」は、再読での取り込みと
  「マーカー書き込み → pid 再確認」の呼び出し順(`RunStoreCall` の並び)の2本で押さえている。
- **RunWrapper**: 001〜027。順序(021/022)はダブルの `calls()` の並びで主張し、025(アトミック性)は
  `TC-port-run-store-016` に委ねる分担が spec の役割分担と一致している。
- **ポート適合**: `HOOKS.md` の件数(A38 / B113 / C18 / 合計169)は 125 + 21 + 16 + 7 として整合し、
  `permission_restrictions_effective` で導ける12行の内訳も表と一致する。

### フックの契約が緩くないか

- `attempt_dir_present`(TC-port-run-store-001)・`run_dir_is_empty` + `wait_for_run_files`
  (TC-port-process-controller-001)は**起動の前後で観測が反転すること**まで主張しており、真偽を定数で
  返すハーネスはどちらかの側で落ちる。
- `worktree_present` は「登録が `ws.branch` を指し、かつ実体が在る」ことまで見ており、
  ディレクトリを掘っただけの実装は通らない。`worktree_marker` は不在を空文字列として返すため、
  内容が消えたことがスキップに化けない。
- 権限操作系(`make_unreadable` / `make_attempt_unwritable` / `unwritable_log_path` /
  `non_executable_command` / `deny_write`)はいずれも制限が実際に効いたことを確かめてから `Some` を返す。
  `deny_write` が非破壊の開き方で確認しているのは、先置きした内容を失わないために必要。
- テストダブルは台本を使い切ると `panic!` する(既定値を黙って返さない)ため、呼び出し回数の
  食い違いが緑にならない。

### 黙ったスキップが無いか

- 受け入れテスト側も `SkipBudget`(`common::skipped`)を通しており、宣言していないケースの
  スキップはそのケースの失敗になる。宣言は実行時の述語(`permission_restrictions_effective` /
  `lock::holder_program()` / `tmpdir_outside_repository()`)から組まれている。
- `examples/agent_probe` / `spawn_probe` の不在は**許容集合に入れていない**(`expect` で落ちる)。
  作り忘れが緑にならない。

### 受け入れテストの flaky / 後始末

- **ラッパーを kill する経路**(`cli_wrapper.rs::エージェントの実行中にラッパーが終了させられるとexitは書かれない`):
  kill する時点が「エージェント実行中・exit 書き込み前」であることを、ログへの滞留の合図(`waiting`)で
  待ち合わせており実行環境の速さに依存しない。kill の後に**解放ファイルの設置と解放の合図(`released`)の
  待ち合わせを済ませてから**アサートしているため、主張が落ちた場合でも一時ディレクトリの削除と競合する
  プロセスが残らない。`RunningWrapper::kill` は `wait()` まで行い、ゾンビも残さない。
  上限(120秒)は解放し損ねたときの歯止めであって生存の窓ではなく、`assert` の根拠になっていない。
- **作業ディレクトリを消す経路**(`cli_tick_missing_cwd.rs`): プロセス全体の状態を触るため
  **独立した実行ファイルにテスト1件だけ**を置き、`#![cfg(unix)]` で前提が作れない環境ごと外している。
  cwd の復元はアサーションより前に済ませているので、主張が落ちても後続に漏れない。
  cargo は統合テストをファイル単位の別プロセスで走らせるため、他ファイルの相対パス解決にも影響しない。
- **滞留エージェントを跨ぐ tick**(`cli_tick.rs::滞留するエージェントを起動したままでも次のtickは競合しない`):
  「2回目の tick がラッパーの生存中に走った」ことを `exit` の不在で確定させたうえで、末尾で解放 →
  `exit` の出現まで待ち切っており、孫プロセスを残したままホームを消さない。
- 待ち合わせ(`common::wait_until`)は**これから観測する成果物そのもの**に条件を立てており
  (pid の出現でログや `exit` を代用していない)、タイムアウト時は「何が現れなかったか」と
  run ディレクトリの一覧を添えて落ちる。期限は `WAIT_TIMEOUT` の1箇所に集約されている。

### 決定性・アサーションの実質

- 時刻依存はすべて `SettableClock` / `FixedClock` 経由で、実時間を待つ受け入れテストは無い。
- 順序依存の主張は `RunStoreCall` / `ProcessControllerCall` / `WorktreeManagerCall` の記録で書かれており、
  走査順はダブルに与えた列で決まる。
- `run.stdout.contains("スキップしました")` など文言に対する主張は、`render.rs` の実際の出力文字列と
  一致しており空虚に成立しない(`tick_skipped()` = 「…今回の tick はスキップしました。」)。
- `TickSummary::is_empty()` は `errors` を含む全項目を見るため、`summary.is_empty()` の主張が
  報告の見落としを許さない。
- 「保存できた遷移だけを積む」は、凍結を伴う保存が失敗する経路
  (`凍結を伴う遷移を保存できなければ凍結にも失敗の記録にも数えない`)でしか主張できないという
  観察のうえで、その経路のテストが置かれている。

### 役割分担

plan.md のテスト方針どおりに分かれている — ドメイン(I/O なし)/ ポート適合(`pulsen-conformance`)/
ユースケース(ダブル・実プロセスも実FSも使わない)/ 受け入れ(実バイナリ)。
プラットフォーム固有の具体値(`128+シグナル番号`)は適合スイートに持ち込まず POSIX の
アダプターユニットテストが固定しており、適合側は「非0の符号化値」までにとどめている(ADR-074)。

### 残す必要のない記述

テスト・フィクスチャ・examples に、指摘への弁明や修正の経緯を残す記述は無い(`TODO` / `FIXME` /
「暫定」の類も 0 件)。コメントはいずれも「なぜこの形か / なぜそうしないか」を説明している。

## カバレッジ

- 確認(51):
  `crates/pulsen-conformance/HOOKS.md`,
  `crates/pulsen-conformance/src/lib.rs`,
  `crates/pulsen-conformance/src/run_store.rs`,
  `crates/pulsen-conformance/src/process_controller.rs`,
  `crates/pulsen-conformance/src/worktree_manager.rs`,
  `crates/pulsen-conformance/src/doubles/mod.rs`,
  `crates/pulsen-conformance/src/doubles/clock.rs`,
  `crates/pulsen-conformance/src/doubles/process.rs`,
  `crates/pulsen-conformance/src/doubles/run_store.rs`,
  `crates/pulsen-conformance/src/doubles/task_repository.rs`,
  `crates/pulsen-conformance/src/doubles/worktree.rs`,
  `crates/pulsen-conformance/src/doubles/tests.rs`,
  `crates/pulsen/examples/agent_probe.rs`,
  `crates/pulsen/examples/spawn_probe.rs`,
  `crates/pulsen/tests/cli_tick.rs`,
  `crates/pulsen/tests/cli_tick_missing_cwd.rs`,
  `crates/pulsen/tests/cli_usage.rs`,
  `crates/pulsen/tests/cli_wrapper.rs`,
  `crates/pulsen/tests/common/mod.rs`,
  `crates/pulsen/tests/common/git.rs`,
  `crates/pulsen/tests/conformance_process_controller.rs`,
  `crates/pulsen/tests/conformance_run_store.rs`,
  `crates/pulsen/tests/conformance_worktree.rs`,
  `crates/pulsen/tests/register_task.rs`,
  `crates/pulsen/tests/run_wrapper.rs`,
  `crates/pulsen/tests/tick_confirm_spawn.rs`,
  `crates/pulsen/tests/tick_fixture/mod.rs`,
  `crates/pulsen/tests/tick_launch.rs`,
  `crates/pulsen/tests/tick_scan.rs`,
  `crates/pulsen-domain/src/definition/agent.rs`,
  `crates/pulsen-domain/src/definition/template.rs`,
  `crates/pulsen-domain/src/execution/launching.rs`,
  `crates/pulsen-domain/src/execution/value.rs`,
  `crates/pulsen-domain/src/task/attempt.rs`,
  `crates/pulsen-domain/src/task/counters.rs`,
  `crates/pulsen-domain/src/task/failure.rs`,
  `crates/pulsen-domain/src/task/path.rs`,
  `crates/pulsen-domain/src/task/planner.rs`,
  `crates/pulsen-domain/src/task/task.rs`,
  `crates/pulsen-domain/src/task/transition.rs`,
  `crates/pulsen/src/adapter/process.rs`,
  `crates/pulsen/src/adapter/run_store.rs`,
  `crates/pulsen/src/adapter/worktree.rs`,
  `crates/pulsen/src/application/tick/mod.rs`,
  `crates/pulsen/src/application/tick/launch.rs`,
  `crates/pulsen/src/cli/mod.rs`,
  `crates/pulsen/src/cli/render.rs`,
  `crates/pulsen/src/cli/tick.rs`,
  `crates/pulsen/src/cli/wrapper.rs`,
  `.adr/027-port-conformance-suite-and-harness-hooks.md`,
  `.thread/2/plan.md`
- スキップ(44):
  - `.thread/2/adr.md`, `.thread/2/progress.md`, `.thread/2/steps.md`, `.thread/2/testing.md`,
    `.thread/2/review/*`(30ファイル: review-001〜005 の各観点・統合、`triage.md`)— 計34。
    ゼロベースのレビューのため既存のレビュー成果物は読まない指示。計画文書は `plan.md` のみ読んだ。
  - `crates/pulsen-domain/src/execution/mod.rs`, `crates/pulsen-domain/src/execution/port.rs`,
    `crates/pulsen-domain/src/task/mod.rs`, `crates/pulsen/src/adapter/mod.rs`,
    `crates/pulsen/src/application/mod.rs`, `crates/pulsen/src/application/run_wrapper.rs`,
    `crates/pulsen/src/application/tick/confirm_spawn.rs`, `crates/pulsen/src/cli/add.rs`,
    `crates/pulsen/src/cli/args.rs`, `crates/pulsen/src/cli/wire.rs` — 計10。
    テストを含まない実装・再エクスポートで、振る舞いは対応するテスト側(`run_wrapper.rs` /
    `tick_confirm_spawn.rs` / `cli_wrapper.rs` / `cli_tick.rs`)から確認済み。設計の妥当性は
    domain / usecase / adapter 観点の担当。
