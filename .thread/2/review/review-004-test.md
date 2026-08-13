# レビュー 004 — Test

対象: PR #11(`issue/2/tick-agent-run-launch`)/ ベース `main` / 変更 81 ファイル
契約: `.thread/2/plan.md`、Issue #2 のチェックリスト 208 行

## Test

### Blockers

なし。

### Warnings

なし。

指摘はゼロ。**問題なし**と判断した。以下は判断の根拠。

### 判断の根拠

#### TC-* 行の ID 単位の突き合わせ(120 行)

| 群 | 行数 | 実装 | 確認内容 |
|---|---|---|---|
| `TC-port-run-store-001〜021` | 21 | `crates/pulsen-conformance/src/run_store.rs` | 1行 = 1ケース関数 = 1 `#[test]`。追加ケース(write 系の置き場作成)は台帳行と区別して命名 |
| `TC-port-process-controller-001〜005・017〜027` | 16 | `crates/pulsen-conformance/src/process_controller.rs` | `identity_and_agent`(13)と `spawn`(3)の2スイート |
| `TC-port-worktree-manager-010〜016` | 7 | `crates/pulsen-conformance/src/worktree_manager.rs` | ADR-077 の `prunable` 復旧を追加ケースで別途実行 |
| `TC-exec-run-wrapper-001〜027` | 27 | `tests/run_wrapper.rs`(001〜006・010〜012・018・020〜024)/ `tests/cli_wrapper.rs`(007・009・013〜017・019・026)/ `tests/cli_tick.rs`(008)/ `run_store.rs` の TC-016(025) | — |
| `TC-exec-tick-001〜027・028〜055・068〜086` | 49 | `tests/tick_scan.rs` / `tick_launch.rs` / `tick_confirm_spawn.rs` / `cli_tick.rs` | — |

未対応の ID は無い。ID が1本のテストに収斂しているのは `TC-exec-tick-082` / `083` の組だけで、この2行は「pid が現れた順序」が違うだけで tick の観測(初回読みは不在・再読で pid あり)としては同一であり、ダブルに対して区別できる差が無い。重複した関数を置く価値は無いと判断した。

`TC-exec-run-wrapper-026`(何も出力しないエージェント)はログの空を直接主張していないが、「ラッパーがログを汚さない」は `シェルのメタ文字や空文字列を含むトークンはリテラルのまま渡る`(stdout.log がトークン列と完全一致)と `cli_tick` の `stdout == "実装して"` が別途固定しており、行の主張(exit が書かれ、異常ではない)は満たされている。

`TC-exec-run-wrapper-027` は plan の部分消化14行の1つ(操作の主語が次の tick)。消化範囲「kill された attempt に exit が残らない」は、`run_wrapper.rs` の呼び出し順の主張(`WriteExit` が `RunAgent` の後の最後の呼び出し)から導ける。

#### アサーションの実質性

- ダブルは台本を使い切ると `panic!` する(`ScriptedRunStore` / `ScriptedProcessController` / `ScriptedTaskRepository` / `ScriptedWorktreeManager`)。想定外の呼び出しが既定値で素通りして緑になる経路が無い。
- `ScriptedTaskRepository::saved()` は `save` の成否によらず渡された値を積む。`saved().is_empty()` は「保存が失敗した」ではなく「保存を試みてすらいない」の主張になっている(`マーカーを書けなければ状態を変更せず報告してスキップする` など)。
- 順序(starttime → pid、pid → マーカー確認、マーカー書き込み → pid 再確認)は結果の値に出ないため、呼び出し記録(`calls()`)の並びを完全一致で主張している。`猶予超過ではマーカーを書いてからpidを再確認する` は5呼び出しの列を丸ごと固定している。
- 「保存できた遷移だけを積む」は凍結を伴う保存の失敗でしか主張できないため、`凍結を伴う遷移を保存できなければ凍結にも失敗の記録にも数えない` を専用に置いている。
- 「書き込んだ tick が処理対象なしにならない」(ADR-084)を、worktree 失敗・展開失敗・spawn 未観測・取込の各経路でサマリー側からも主張している。

#### 適合スイートが「常に真を返すハーネス」を通さないか

- `TC-port-run-store-001` は `attempt_dir_present` の観測が `prepare_attempt` の**前後で反転すること**まで主張する。定数を返すハーネスはどちらかの側で落ちる。
- `TC-port-process-controller-001` も `run_dir_is_empty` の反転を主張し、`wait_for_run_files` だけに依らない。
- `TC-port-worktree-manager-010/011/016` の `worktree_present` は登録が `ws.branch` を指すことと実体の存在の両方を見る(ディレクトリを掘っただけの実装が通らない)。
- 権限操作のフック(`make_unreadable` / `make_attempt_unwritable` / `unwritable_log_path` / `non_executable_command`)は、制限が実際に効いたことを確かめてから `Some` を返す。効かない環境では `None` を返し、`SkipBudget` が集合外のスキップをそのケースの失敗にする。

#### 黙ったスキップが無いか

`cargo test`(全体)を実行し、全ターゲット緑。`-- --nocapture` で拾える `SKIP` 行はこの環境で `tc_port_clock_005`(既存・`rewind` 未提供)の1件だけで、本スライスで足した44行 + 追加2件はすべて**実際に走って通っている**。

- 適合スイート: `conformance_run_store` 22件、`conformance_process_controller` 16件、`conformance_worktree` 17件がいずれも full pass。
- `agent_probe` / `spawn_probe` の不在はスキップ許容集合に入れていない(作り忘れが緑にならない)。`cli_wrapper.rs` も `expect` で落とす。
- 受け入れテスト側のスキップ(`tc_exec_run_wrapper_014` / `016`、`tc_exec_tick_015`)も `common::skipped` 経由で同じ `SkipBudget` に載っており、宣言は環境の能力(`permission_restrictions_effective` / `lock::holder_program`)から実行時に決まる。

#### flaky リスク・時刻依存・順序依存

- 猶予時間の境界(30 / 31 / 巻き戻り)は `SettableClock` に対するユースケース層テストで消化しており、実時間を待つ箇所が無い。受け入れテストは猶予内の経路しか通らない。
- 受け入れテストの待ち合わせは `common::wait_until` に集約され、**これから観測する成果物そのもの**に条件を立てている。ログや `exit` を読む前は `exit` の出現を待ち(`wait_for_exit`)、pid だけを待って直後にログを読む書き方は無い。期限切れは「何が現れなかったか」と観測先の一覧を添えて落ちる。
- 滞留プロセスの後始末: `滞留するエージェントを起動したままでも次のtickは競合しない` は解放ファイルで終端を決め(環境の速さに依存しない)、最後に必ず解放して `exit` まで待つ。`agent_probe wait-for` 側にも上限がある。他の `cli_tick` の各テストは終了前に必ず `wait_for_exit` を通し、一時ホームの削除と孫プロセスの書き込みが競合しない。
- `TC-port-run-store-016`(並行読み取り)は読み手の停止を `Drop` に載せ(書き手のパニックでハングしない)、書き込みと**重なった**観測の回数を下限つきで主張し、周回数と待ち期限の両方に上限がある。空虚な成立にもハングにもならない。
- 適合ケース `TC-port-worktree-manager-015` の間欠失敗(`.thread/2/progress.md` に既知の制限として記録)は、フィクスチャ側の前提検査・`create` 直前の主張・`Ok` アームでの `git worktree list --porcelain` の生出力添付まで入っており、再発時に「前提が消えた」のか「同定が外れた」のかが名指しできる形になっている。原因未特定のまま同定ロジックを触らない判断も妥当。今回の全体実行でも再現せず、追加で打てる手が無いため指摘としては挙げない。

#### 役割分担が plan.md どおりか

- ドメイン: `LaunchingClassifier` の全分岐と境界3件、`Task` の遷移6種と問い合わせ5種、`WorkspacePlanner`、`RunDirPath` の6パスと `state_root` の逆導出、`CommandLine::rehydrate` の往復と0トークン拒否 — いずれも I/O 無しのユニットテストで、仕様の言葉で命名されている。
- ユースケース: `tick_*` / `run_wrapper.rs` はポートを全部ダブルに差し替え、AC-17 の一覧(ロック異常・`list_active` の Io・worktree 失敗と上限超過・展開失敗5経路・`prepare_attempt` 失敗・`spawn_wrapper` 同期エラー・`RunFileError` 3種・マーカー書き込み失敗・猶予境界・`save` 失敗)を実プロセス・実ファイルシステム無しで消化している。
- 適合: `spec/testcases/ports/*.md` と1行1関数で対応。`128+シグナル番号` の具体値はアダプターのユニットテスト(`シグナルで終了したエージェントは128足すシグナル番号に符号化される`)に置き、スイートは「非0の符号化値」までに留めている(ADR-074)。
- 受け入れ: 実バイナリ・実 git・実プロセスで F2 / F4 / F6 とクロス tick の引き継ぎを通し、`examples/agent_probe` を config のエージェント定義として使って実在エージェントに依存していない。
- `HOOKS.md` は 9ポート169行・区分別 A38 / B113 / C18 の内訳が本文の各表と一致しており、「環境で走らなくなりうる行」の表もテスト側のスキップ宣言と整合している。

#### 残す必要のない記述

テスト・フィクスチャ・ダブルのコメントは、いずれも「なぜその形か / なぜそうしないか」(順序を呼び出し記録で主張する理由、配線しないアームに期待を持たせない理由、待ち条件を成果物そのものに立てる理由、`Drop` に停止を載せる理由、置き場をシンボリックリンク経由にする理由)に限られている。指摘への弁明・修正の経緯・`#[ignore]` ・`TODO` / `FIXME` は1件も無い。

### カバレッジ

確認: `crates/pulsen-conformance/HOOKS.md`, `crates/pulsen-conformance/src/lib.rs`, `crates/pulsen-conformance/src/run_store.rs`, `crates/pulsen-conformance/src/process_controller.rs`, `crates/pulsen-conformance/src/worktree_manager.rs`, `crates/pulsen-conformance/src/doubles/mod.rs`, `crates/pulsen-conformance/src/doubles/clock.rs`, `crates/pulsen-conformance/src/doubles/process.rs`, `crates/pulsen-conformance/src/doubles/run_store.rs`, `crates/pulsen-conformance/src/doubles/task_repository.rs`, `crates/pulsen-conformance/src/doubles/worktree.rs`, `crates/pulsen-conformance/src/doubles/tests.rs`, `crates/pulsen-domain/src/execution/launching.rs`, `crates/pulsen-domain/src/execution/value.rs`, `crates/pulsen-domain/src/execution/port.rs`, `crates/pulsen-domain/src/task/planner.rs`, `crates/pulsen-domain/src/task/transition.rs`, `crates/pulsen-domain/src/task/task.rs`, `crates/pulsen-domain/src/task/path.rs`, `crates/pulsen-domain/src/task/failure.rs`, `crates/pulsen-domain/src/task/attempt.rs`, `crates/pulsen-domain/src/task/counters.rs`, `crates/pulsen-domain/src/definition/template.rs`, `crates/pulsen-domain/src/definition/agent.rs`, `crates/pulsen/src/adapter/process.rs`, `crates/pulsen/src/adapter/run_store.rs`, `crates/pulsen/src/adapter/worktree.rs`, `crates/pulsen/src/cli/render.rs`, `crates/pulsen/src/cli/tick.rs`, `crates/pulsen/examples/agent_probe.rs`, `crates/pulsen/examples/spawn_probe.rs`, `crates/pulsen/tests/cli_tick.rs`, `crates/pulsen/tests/cli_wrapper.rs`, `crates/pulsen/tests/cli_usage.rs`, `crates/pulsen/tests/common/mod.rs`, `crates/pulsen/tests/common/git.rs`, `crates/pulsen/tests/conformance_process_controller.rs`, `crates/pulsen/tests/conformance_run_store.rs`, `crates/pulsen/tests/conformance_worktree.rs`, `crates/pulsen/tests/register_task.rs`, `crates/pulsen/tests/run_wrapper.rs`, `crates/pulsen/tests/tick_fixture/mod.rs`, `crates/pulsen/tests/tick_scan.rs`, `crates/pulsen/tests/tick_launch.rs`, `crates/pulsen/tests/tick_confirm_spawn.rs`, `.thread/2/plan.md`, `.thread/2/progress.md`(47)

スキップ: `.thread/2/review/review-001-adapter.md`, `.thread/2/review/review-001-architecture.md`, `.thread/2/review/review-001-domain.md`, `.thread/2/review/review-001-test.md`, `.thread/2/review/review-001-usecase.md`, `.thread/2/review/review-001.md`, `.thread/2/review/review-002-adapter.md`, `.thread/2/review/review-002-architecture.md`, `.thread/2/review/review-002-domain.md`, `.thread/2/review/review-002-test.md`, `.thread/2/review/review-002-usecase.md`, `.thread/2/review/review-003-adapter.md`, `.thread/2/review/review-003-architecture.md`, `.thread/2/review/review-003-domain.md`, `.thread/2/review/review-003-test.md`, `.thread/2/review/review-003-usecase.md`, `.thread/2/review/review-003.md`, `.thread/2/review/triage.md` — 指示によりゼロベースで見るため既存レビューは読まない(18)。`.thread/2/adr.md`, `.thread/2/steps.md`, `.thread/2/testing.md`, `.adr/027-port-conformance-suite-and-harness-hooks.md` — 契約は plan.md で読み、テスト観点の判断は `HOOKS.md`(フック表の正本)と実物のテストで行った(4)。`crates/pulsen-domain/src/execution/mod.rs`, `crates/pulsen-domain/src/task/mod.rs`, `crates/pulsen/src/adapter/mod.rs`, `crates/pulsen/src/application/mod.rs`, `crates/pulsen/src/cli/mod.rs` — 再公開のみでテスト定義・テストコードを含まない(5)。`crates/pulsen/src/application/run_wrapper.rs`, `crates/pulsen/src/application/tick/mod.rs`, `crates/pulsen/src/application/tick/launch.rs`, `crates/pulsen/src/application/tick/confirm_spawn.rs`, `crates/pulsen/src/cli/args.rs`, `crates/pulsen/src/cli/wire.rs`, `crates/pulsen/src/cli/wrapper.rs` — インラインテストを持たない実装で、振る舞いは `tick_*` / `run_wrapper.rs` / `cli_*` の外部テストから観測した(7)

合計: 確認 47 + スキップ 34 = 81。
