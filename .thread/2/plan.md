# 実装計画 — Issue #2: tick によるエージェント実行の起動(worktree確保・デタッチ起動・spawn確認)

**Issue:** #2
**作成日:** 2026-08-12
**複雑度:** 中〜大規模
**実装方針:** steps.md

---

## 目的

`pulsen tick` が全タスクを走査して実行状態ごとに分岐する骨格を作り、エージェント実行ステータスの pending / failed タスクを「worktree確保 → テンプレート展開 → launching記録 → デタッチspawn」(ADR-016)で起動し、次の tick が pid の出現をもって running へ取り込めるようにする。あわせてラッパーモード(`pulsen wrapper`)を実装し、同定情報と終了結果が runディレクトリに永続化される二重起動排除のプロトコルを成立させる。

## 受け入れ基準

| # | 基準(検証可能な形で) | 由来 | 対応ステップ |
|---|---|---|---|
| AC-1 | `cargo build` / `cargo test` / `cargo clippy -- -D warnings` / `cargo fmt --check` が通る。`pulsen-domain` の `[dependencies]` は空のまま、workspace の `unsafe_code = "forbid"` を維持したまま(新規依存クレートを足さずに)実装される。`crates/*/src/` の**ターゲット述語つき `cfg`**(`cfg(` に続く `unix` / `windows` / `target_os` / `target_family` の4つ)は `crates/pulsen-domain/` に1件も現れず、`crates/pulsen/src/` 側のヒットは `util/atomic.rs` と **`adapter/process.rs`**、および `adapter/task_repository.rs` の3ファイルだけである(`adapter/task_repository.rs` のヒットは `#[cfg(all(test, unix))]` のテストモジュールで、本番の実行経路には乗らない)。`cfg(unix)` / `cfg(windows)` だけを見る grep(Issue #1 の testing.md)では ADR-075 が必然的に生む `#[cfg(target_os = "linux")]` を素通りさせ、ドメイン層に紛れ込んでも検査が緑になるため、述語を4つに広げる | CLAUDE.md 技術方針・アーキテクチャ、requirements §4.3、adr.md ADR-075 | 全ステップ |
| AC-2 | execution ドメインに `ExitCode`(`is_success`)・`PidFileContent` が実装され、`RunDirPath` が `pid_file` / `starttime_file` / `exit_file` / `stdout_log` / `stderr_log` / `marker_file` の6つの導出関数を持つ。いずれも I/O に触れない純粋関数で、ユニットテストでパスが検証される | DOM-execution-001, 003, 028〜033 | 1 |
| AC-3 | `LaunchingClassifier` が `classify` / `classify_recheck` を持ち、`LaunchingDecision`(3値)・`LaunchingRecheck`(2値)・`InconsistentRunFiles` が型で表現される。ユニットテストで全分岐と境界が網羅される — pid+starttime → `ConfirmRunning`、pid なし猶予内 → `KeepWaiting`、pid なし猶予超過 → `SuspectSpawnFailure`、pid あり starttime なし → `Err`、`classify_recheck` の pid なし(starttime の有無を問わず)→ `SpawnFailed`。境界は経過 = 30秒(超過しない)/ 31秒(超過する)/ 負(0として扱う) | DOM-execution-006, 007, 013, 014, 015, 016、TC-exec-tick-077/078/079 | 2 |
| AC-4 | `Task` に問い合わせ5種(`execution_kind` / `current_status_def` / `next_attempt_number` / `is_agent_run`・`is_wait`・`is_cleanup` / `applicable_retry_limit`)が実装され、`WorkspacePlanner::derive(worktree_root, id)` が `path = <worktree_root>/<task-id>`・`branch = pulsen/<task-id>` を決定的に導出する。`next_attempt_number` は `current_attempt` が None なら 1、`applicable_retry_limit` は AgentRun / Cleanup とも `snapshot.effective_retry_limit(task_status)` に委譲し(Cleanup の 2 は既存の組み込みデフォルトとしてそこから返る)、Wait は None を返す | DOM-task-048〜052, 061 | 3 |
| AC-5 | `Task` の遷移関数6種(`confirm_workspace` / `record_launching` / `confirm_running` / `record_spawn_failure` / `record_spawn_failure_in_place` / `record_tool_failure`)と `TransitionError`(5種)が実装される。ユニットテストで前提状態の不一致・事後条件・カウンタ規則が網羅される — `record_launching` は次番号を採番し `run_dir` を内部導出して `Launching { recorded_at }` にする / `confirm_running` は `spawn_fail_count` だけをリセットする / 上限超過は加算後 `count > limit` のときだけ成立する(等号では凍結しない)/ `record_spawn_failure_in_place` は実行状態も attempt 番号も変えない | DOM-task-033〜037, 042、TC-exec-tick-035/036/048/049/050 | 4 |
| AC-6 | 追加するポートのメソッドが spec/domains/execution.md のポート表と1:1で一致する — `RunStore` は本スライスで使う9メソッド(`prepare_attempt` / `read_pid_file` / `read_starttime` / `read_exit` / `write_invalidation_marker` / `marker_exists` / `write_starttime` / `write_pid_file` / `write_exit`)、`ProcessController` は3メソッド(`spawn_wrapper` / `own_identity` / `run_agent`)、`WorktreeManager` に `create` が1つ増える。付随する値・エラー型(`WrapperLaunchSpec` / `WrapperIdentity` / `SpawnError` / `WorktreeError` / `RunFileError`)が定義され、**未実装メソッドの宣言・スタブが1つも無い**(`attempt_exists` / `list_runs` / `delete_attempt` / `remove_task_dir_if_empty` / `starttime_of` / `kill` / `try_kill_remnants` / `remove` は宣言しない) | DOM-execution-034〜037, 039〜043, 047, 048, 052〜055, 057, 062, 066、Issue 完了条件 | 5, 7 |
| AC-7 | RunStore の適合スイート **21件**(TC-port-run-store-001〜021)が実装され、`FsRunStore` が通す — `prepare_attempt` の親含む作成(attempt ディレクトリの存在はハーネスのフックで環境に問う)と冪等性(既に書いたファイルが read 系でそのまま読める)・`RunDirPath::derive` との一致、read系3種の「不在 = `Ok(None)`(ディレクトリ不在を含む)/ 解釈不能 = `Corrupt` / 機構失敗 = `Io`」の3分類と往復可能性、write系3種のアトミック置換(並行読み取りが書きかけを観測しない・失敗時も部分的な内容を残さない)、`write_invalidation_marker` のディレクトリ不在時作成と冪等性、`marker_exists` の真偽 | ADP-runstore-001〜004, 006〜010、TC-port-run-store-001〜021 | 6 |
| AC-8 | WorktreeManager の適合スイートが 9件から **17件**(台帳7行 = TC-port-worktree-manager-010〜016、+ adr.md ADR-085 由来の追加1件)になり、`GitCliWorktreeManager::create` がすべて通す — base からの新ブランチ+worktree 作成、worktree_root の自動作成、自タスク残骸への冪等成功(内容に触れない)、**登録なし・ブランチのみ残存**からの `worktree add`(`-f` なし・先端を変えない)、worktree でない通常ディレクトリ / 別ブランチの worktree / base 不在はいずれも `Failed` で自動修復しない。追加1件は「登録は残るが実体が消えた(`prunable`)自タスクの worktree を `add -f` で張り直す」で、ADR-085 の復旧2分岐の**両方**が適合スイートで実行されることを保証する(台帳行は増えない) | ADP-worktree-004、TC-port-worktree-manager-010〜016、adr.md ADR-085 | 7 |
| AC-9 | ProcessController の適合スイート **16件**(TC-port-process-controller-001〜005・017〜027)が `own_identity` / `run_agent` の13件と `spawn_wrapper` の3件の2スイートとして実装・適用され、この環境で走る行を `SystemProcessController` がすべて通す(`005` は注入した同定情報の取得元(adr.md ADR-076)で確定的に走る、`023` / `025` は権限操作が効く環境でのみ、`024` はシグナル死を作れる環境でのみ走る。「チェックリスト行にチェックを付ける基準」のスキップ表) — `spawn_wrapper` の `Ok` / デタッチ性(呼び出し側プロセスの終了後もラッパーが完走し runディレクトリに starttime・pid・exit が揃う)/ 起動不能時の `SpawnError` と副作用なし、`own_identity` の pid・非空 kill同定子・呼び出し前後の範囲に入る `starttime.wall`・取得機構を壊した2つ目のコントローラでの `Err(Io)`、`run_agent` の11件(exit code 透過・cwd = worktree・ログのリダイレクト・シェル非経由のリテラル一致・コマンド不在127・実行不能126・シグナル死は**非0の符号化値**・リダイレクト不能126でエージェント未起動・cwd 不在の非0・同期実行)。`128+シグナル番号` という具体値は POSIX のアダプターユニットテストで確認し、適合スイートにはプラットフォーム分岐を持ち込まない | ADP-process-001, 005, 006、TC-port-process-controller-001〜005, 017〜027 | 8, 11 |
| AC-10 | `RunWrapper` ユースケースが spec の順序(`own_identity` → `write_starttime` → `write_pid_file` → `marker_exists` → `run_agent` → `write_exit`)で動作し、**starttime が必ず pid より先に書かれる**。マーカーが存在する場合と `marker_exists` が `Err(Io)` の場合はどちらもエージェントを起動せず正常終了する。ステップ1〜3・6 の書き込み失敗は何も書き残さず終了する。config を読まず、ロックも取らない | UC-execution-009、TC-exec-run-wrapper-001〜008, 010〜012, 016, 018, 021〜027 | 9, 10 |
| AC-11 | `pulsen wrapper` が隠しサブコマンドとして存在し、`--run-dir` / `--workspace` / エージェントコマンドの3値を受け取る。ヘルプの一覧には現れないが実行できる。起動引数が不正(絶対パスでない・トークン0個・run_dir が規定の形でない)なら runディレクトリに何も書かず非0終了する。config.yaml が不在・破損した環境でも動作に影響しない。結果はすべて runディレクトリのファイル(pid / starttime / exit / ログ)として現れ、標準出力には何も要求しない | PAGE-wrapper-001〜005、TC-exec-run-wrapper-009, 013〜015, 019, 020、adr.md ADR-077/078 | 10, 18 |
| AC-12 | `pulsen tick` が spec の処理フロー 1〜9 の骨格で動作する — ロック取得(競合は **0** でスキップ・`LockError::Failed` は非0)、`list_active` の走査(Io は非0・状態は変更しない)、エントリごとの分岐(`Corrupt` は報告のみで書き込まない / `SnapshotUnreadable` は定義依存の判断をスキップして報告 / `Intact` は実行状態で分岐)、1タスクの処理失敗は `errors` に記録して残りを続行、サマリー表示と 0 終了、処理対象なしの表示。アーカイブ済みタスクは走査対象に含まれない。`state/tasks/` 未作成でも空結果として 0 で終わる。**tick は任意の作業ディレクトリ(対象リポジトリの外)から起動しても同じ結果になる**(外部スケジューラーからの起動。帳簿がすべて絶対パスで閉じていることの裏付け) | UC-execution-002、PAGE-tick-001, 002, 004, 005, 006, 008、TC-exec-tick-001, 002, 004, 006, 007, 012, 013, 014, 015, 016, 017, 018, 019, 027、`spec/scenario/setup.md#シナリオ3`、adr.md ADR-073/081 | 13, 14, 15, 17, 18 |
| AC-13 | 手続きA(起動)が spec の順序で動作する — workspace 未確定なら `WorkspacePlanner::derive` → `WorktreeManager::create` → `confirm_workspace` → `save`(失敗は `record_tool_failure(WorktreeCreate)` → `save` → 終了)、テンプレート展開5段(実効エージェント名 → `config.agents` 参照 → `RawAgentDefinition::parse` → `render_input` → `build_command_line`)のどの失敗も `record_spawn_failure_in_place` → `save` → 終了、`record_launching` → `save` → `prepare_attempt`(失敗は報告のみ)→ `spawn_wrapper`(同期エラーも状態を変更しない)。上限超過時は `Stopped` を保存して `frozen` に記録する(通知は #3。adr.md ADR-074) | UC-execution-003、TC-exec-tick-028〜055 | 14, 16, 18 |
| AC-14 | 手続きC(spawn確認)が spec の順序で動作する — 冒頭の `current_attempt` None 検査(不変条件2の破れとして報告・スキップ)、`read_pid_file` / `read_starttime`(`RunFileError` は報告してスキップ・書き込まない)、`classify` の3分岐、`SuspectSpawnFailure` での `write_invalidation_marker`(`Err(Io)` は状態を変更せず報告してスキップ)→ 再読 → `classify_recheck` の2分岐、`record_spawn_failure` の上限超過で `Stopped`。runディレクトリ不在は `Ok(None)` として猶予経路に合流する | UC-execution-005、TC-exec-tick-068〜086 | 15, 16, 18 |
| AC-15 | クロスtickのライフサイクルが成立することを、tick を複数回実行する結合テストで確認できる。ラッパーはデタッチ起動で非同期に完了するため、次の tick を打つ前と runディレクトリの内容を読む前に**期限付きポーリングで待ち合わせる**。待ち条件は**これから観測する成果物そのもの**に立てる(ログを読むならログの出現を、`exit` を読むなら `exit` の出現を待つ)— ラッパーの書き込み順序が starttime → pid → マーカー確認 → ログ生成 → exit である以上、pid の出現はログや `exit` の存在を含意しない。タイムアウトは「何が現れなかったか」と runディレクトリの一覧を添えて落とす — F2(launching記録 → runディレクトリ作成 → spawn → wrapper が starttime→pid→(マーカー確認)→実行→exit → 次tickで running 取込)、F4(最初の AgentRun 起動で worktree が作られ、以降のリトライで同一 worktree が使われ内容がリセットされない)、F6(`pulsen/<task-id>` ブランチが base から作られる) | UC-flow-002, 004, 006 | 18 |
| AC-16 | worktree が手動削除された状態での起動が「エージェント実行の失敗」として既存経路(exit ファイルに非0)に落ち、tick 側に新しい分岐が生じない | PAGE-tick-009、TC-exec-run-wrapper-017、TC-port-process-controller-026 | 8, 10, 18 |
| AC-17 | 実アダプターでは外から状況を作れない分岐が、差し替えたポート実装(テストダブル)に対するユースケーステストとして実装され、実プロセス・実ファイルシステムを使わずに通る — ロック機構の異常、`list_active` の Io、worktree 作成失敗とその上限超過、`prepare_attempt` 失敗、`spawn_wrapper` の同期エラー、`RunFileError` の各種、マーカー書き込み失敗、猶予時間の境界(30秒 / 31秒 / 巻き戻り)、`save` の失敗 | ADR-028(実アダプターを差し替えられることを設計の健全性の指標とする)、TC-exec-tick-015〜019, 037, 038, 046, 047, 072〜079 | 12(前提), 13, 14, 15(主消化), 16(仕上げ) |
| AC-18 | Issue のチェックリスト全208行が steps.md の対応表のとおりどこかのステップで扱われ、**台帳行の PASS 要件を全部満たし、そのテストが実際に走って通った行にだけ**チェックが付く。「チェックリスト行にチェックを付ける基準」の**部分消化14行**と、**実行環境がスキップにした行**にはチェックを付けず、前者は消化した範囲・残る部分・引き取り先を、後者はスキップした理由と確認した環境を Issue のコメントに残す | Issue 完了条件 | 全ステップ, 19 |

## スコープ

### 含まれないもの

- **手続きB(終端処理)・手続きE(gc)** と、それらが使う `WorktreeManager::remove` / `RunStore::{list_runs, delete_attempt, remove_task_dir_if_empty}` / `GcPolicy` / `RemoveOutcome` — Issue #6。tick の分岐は網羅 `match` として書くが、`Cleanup` のアームは配線しない(adr.md ADR-073)。
- **手続きD(観測・判定)** と、`ProcessController::{starttime_of, kill, try_kill_remnants}` / `RunningClassifier` / `JudgementService` / `IdentityCheck` / `CommandRunner` / `Task::{complete_run, skip_run, fail_run, record_judge_failure, advance}` — Issue #3。`Running` / `Completed` のアームは配線しない。
- **共通手続き notify** と `NotificationService` / `NOTIFY_TIMEOUT` / `ADP-commandrunner-001` — Issue #3。stopped の記録と `frozen` への集計までを実装する(adr.md ADR-074)。`Stopped` のアームも配線しない。
- **`RunStore::attempt_exists`** と ls / show の表示 — Issue #4。
- **abort / retry / set-status** と `Task::{abort, retry, set_status, mark_notified}` / `DegradedTask` の遷移 — Issue #5。
- **`DegradedTask` の再通知**(`SnapshotUnreadable` かつ `Stopped { notified_at: None }`)— notify に依存するため #3。本スライスの `SnapshotUnreadable` の扱いは「定義依存の判断をスキップして報告」までとする。
- CI ワークフロー・MSRV 検証・Windows 実機での検証 — Issue #10。本スライスは `#[cfg]` の隔離を grep で機械的に確認するに留める(AC-1)。
- 並列度制御・イベント駆動(webhook 等)— requirements が明示的に持たないと定めている。

## チェックリスト行にチェックを付ける基準

Issue #1 で確立した基準をそのまま使う。**チェックを付ける**のは、台帳行(`spec/inventory/*.md`)の PASS 要件を**すべて**満たす実装とテストが存在し、そのテストが実際に走って通っている行。**チェックを付けない**のは、環境が前提を作れずスキップで終わった行と、スライス境界により PASS 要件の一部しか消化していない行。Issue の完了条件が「スタブ・仮実装・部分実装は不可」である以上、部分消化は未チェックのまま消化範囲を Issue のコメントに残す扱いに統一する。

適用にあたっての線引きを2つ決める。

- 台帳の期待文の末尾にある**下流スライスへの参照**は、その行の操作の主語が本スライスの対象であれば結末の説明とみなし、チェックを妨げない(`TC-exec-run-wrapper-006` / `018` の「次tickが…failed に分類する」は、操作の主語がラッパー側だから完全消化)。操作の主語が下流の手続きである行は部分消化として扱う(`TC-exec-run-wrapper-027` は操作欄が「次の tick を観測する」)。下流の要素が**手続きの途中の必須ステップ**として書かれている行はこの例外に当たらず、部分消化として扱う — notify 句を持つ `UC-execution-003` / `UC-execution-005` / `TC-exec-tick-038` / `045` / `076` が該当し、UC 行と TC 行で判定を変えない(adr.md ADR-074)。
- 台帳の期待文が**本スライス外のメソッドを観測手段として名指ししている**場合、同じ事実を観測する代替手段で満たしたものにはチェックを付ける。該当は `TC-port-run-store-001`(`attempt_exists` → ハーネスの `attempt_dir_present`。adr.md ADR-084)の1行だけで、代替は「`prepare_attempt` の前後で観測が反転すること」まで主張する。

本スライスで部分消化になることが計画時点で分かっている行は次の14行で、チェックを付けない。

| 行 | 消化する範囲 | 残る部分 | 引き取り先 |
|---|---|---|---|
| `UC-execution-002` | 走査・分岐骨格・ロック・サマリー・exit code・本スライスで配線した4アーム | `Cleanup` / `Running` / `Completed` / `Stopped` のアーム、`SnapshotUnreadable` かつ `Stopped { notified_at: None }` の再通知、`run_retention` 時の手続きE | #3 / #6 |
| `UC-execution-003` | 手続きAの順序・worktree確保・展開失敗の5経路・launching記録・`prepare_attempt`・spawn・上限超過での `Stopped` 保存と `frozen` 記録 | `Stopped` 保存直後の notify(台帳 PASS が手続きの途中の必須ステップとして書いている) | #3 |
| `UC-execution-005` | 手続きCの全分岐とマーカー順序・上限超過での `Stopped` 保存と `frozen` 記録 | 同上 | #3 |
| `PAGE-tick-002` | 全タスクファイルの走査と、起動・launching分類 | 観測・判定・遷移・クリーンアップ・通知 | #3 / #6 |
| `PAGE-tick-004` | 全フィールドの表示と、値の入る `launched` / `confirmed_running` / `frozen` / `errors` が実際に表示されること | `transitioned` / `skipped_back` / `notified` / `archived` / `gc_deleted` / `gc_errors` は値の入る経路が無く、表示される様子を確認できない | #3 / #6 |
| `PAGE-tick-009` | 進行中の worktree 消失をエージェント実行の失敗として既存経路に落とすこと | クリーンアップでの削除済み扱いの続行 | #6 |
| `UC-flow-002`(F2) | attempt 採番 → runディレクトリ作成 → spawn → starttime/pid/マーカー/実行/exit → 次tickの running 取込 | exit 観測・判定・終端(completed / skipped / failed)確定・abort による中断 | #3 / #5 |
| `UC-flow-004`(F4) | 作成と、全ステータスで同一 worktree を使うこと(リトライ間で内容が引き継がれる) | 削除(手続きB)と終端の `remove` | #6 |
| `UC-flow-006`(F6) | base からの作成と、ツールがブランチのライフサイクルに関与しないこと | worktree 削除後もブランチが残存すること(成果物の回収可能性)の確認 | #6 |
| `TC-exec-run-wrapper-027` | ラッパーごと kill された attempt に exit が残らないこと | 次tickの「exitなし・プロセス死亡」→ failed 分類 | #3 |
| `TC-exec-tick-013` | in-scope の分岐(`Corrupt` / `SnapshotUnreadable` / Wait / AgentRun / Launching)が1ステップずつ処理され、サマリーに集約され、exit が 0 | `Cleanup` / `Running` / `Completed` / `Stopped` のタスクが1ステップ処理されること | #3 / #6 |
| `TC-exec-tick-038` | stopped の保存と `frozen` 記録 | 直後の notify 実行 | #3 |
| `TC-exec-tick-045` | 同上 | 同上 | #3 |
| `TC-exec-tick-076` | 同上 | 同上 | #3 |

`PAGE-tick-004` の表示は10フィールド分をまとめて実装する。サマリー DTO が spec の9フィールドと `confirmed_running` を持つ(adr.md ADR-094)以上、表示だけを本スライスで値の入る4つに絞ると、並びの規則が確定せず #3 / #6 が同じ表示を足し直すことになる。値の入る経路が本スライスに無いフィールドは、表示が実際に現れるところまでは #3 / #6 でしか確かめられないので、行は未チェックのままにする。`RunDirPath::attempt_dir_name` が `pub` なのは、run ディレクトリの値を持たない断片(タスクIDと attempt 番号)から名前を組む gc の表示(`gc_deleted` / `gc_errors`)がここに含まれるため。

これとは別に、**実行環境が前提を作れないとスキップで終わる行**がある。スキップで終わった行にはチェックを付けず、スキップした理由と確認した環境(OS・root か否か・TMPDIR の位置)を Issue のコメントに残す。

| 行 | 前提を作れない環境 | 判定 |
|---|---|---|
| `TC-port-run-store-007` / `017` | 読み取れないファイル・書き込めない attempt ディレクトリを作れない(root 実行・権限を持たないファイルシステム) | `permission_restrictions_effective` |
| `TC-port-process-controller-023` / `025` | 実行権限のない実体・書き込み不能なログパスを作れない(同上) | `permission_restrictions_effective` |
| `TC-exec-run-wrapper-014` / `016` | 実行権限のない実体・書き込み不能なログパスを作れない(同上)。126 の裏付けが消えるため、受け入れテスト側も `tc_exec_run_wrapper_014` / `tc_exec_run_wrapper_016` としてスキップに載せる | `permission_restrictions_effective` |
| `TC-port-process-controller-024` | シグナルによる終了を作れない(非 POSIX) | `examples/agent_probe abort` がシグナル死になるプラットフォームか |

チェックが付く行数は実行環境で決まる。上限は208行から部分消化14行を除いた **194行**で、当日スキップになった行をさらに引いた数が実際の数になる。計画時点で「必ずスキップ」と確定する行は無い — `TC-port-process-controller-005` は同定情報の取得元を注入する形(adr.md ADR-076)にしたため、権限にも root の可否にも依存せず走る。確定した数と一覧はステップ19 で Issue のコメントに残す。

## リスクと注意点

- **`unsafe_code = "forbid"` とプロセス操作**: デタッチ起動・プロセスグループ・起動時刻取得は通常 `libc` を要するが、本プロジェクトは workspace lints で `unsafe` を禁じ、ADR-023 が本番依存を6クレートに閉じている。`CommandExt::process_group` / `creation_flags` / `/proc` の読み取り / `ps` の起動で組む(adr.md ADR-075)。**この方針が破れると新規依存または lint 緩和の判断が要る**ので、ステップ8で最初に実測して確かめる。
- **起動時刻の取得手段の一貫性**: requirements §4.3 は「記録と照合は必ず同一の取得手段で行う」と定める。本スライスは記録側(`own_identity`)だけを作り、照合側(`starttime_of`)は #3。取得手段を1つの private 関数に閉じておかないと、#3 が別の手段を書いて照合が常に不一致になり、**生存中のプロセスを Dead と誤判定 → failed → 再起動 → 同一 worktree での並走**という最悪の失敗モードに直結する。関数を閉じるだけでは足りず、**戻り値を三値(取得できた / 対象プロセスが不在 / 機構の失敗)まで本スライスで確定させる** — 二値のままだと #3 の `starttime_of` が署名を変えることになり、しかも実測では「不在」がエラーの形で返る(macOS の `ps` は exit 1・stdout 空、Linux の `/proc/<pid>/stat` は `NotFound`)ため、機構失敗に畳む実装が自然に書けてしまう。畳むと #3 で running のまま永久滞留する縮退を招く(adr.md ADR-075)。
- **起動時刻の表現の非決定性**: 関数を1つにするだけでは足りない。`ps -o lstart=` の出力はロケールとタイムゾーンで変わるため(実測: 既定 `水  8/12 20:04:53 2026` / `LC_ALL=C` `Wed Aug 12 20:04:53 2026` / `LC_ALL=C TZ=UTC` `Wed Aug 12 11:04:53 2026`)、cron の tick が spawn したラッパーの記録と対話シェルの tick の照合が食い違い、同じ失敗モードに落ちる。取得時の環境を固定する(adr.md ADR-075)。
- **`ps -o lstart=` の精度**: macOS では秒精度しか取れず、1秒以内の PID 再利用は検出できない。spec は「同一マシン内での等価比較」しか要求しないので契約は満たすが、限界として記録する。Windows の取得手段は本環境で実測できない(Issue #10 に委ねる)。
- **書き込み順序の契約**: ラッパーは starttime → pid の順、tick は「マーカー書き込み → pid 再確認」の順。**どちらか一方でも逆になると二重起動が起きる**(遅延起動したラッパーと新 attempt の並走)。順序そのものを主張するテスト(TC-exec-run-wrapper-021/022、TC-exec-tick-082/083)を明示的に置く。
- **`spawn_wrapper` は「起動後の成否に関知しない」**: 同期エラーでも状態を変更しない(猶予経路が分類する)。ここで `record_spawn_failure` を呼ぶと、launching 記録済みのタスクが pending に戻りつつラッパーが遅れて起動して並走しうる。`prepare_attempt` の失敗も同じ扱い。
- **`write_invalidation_marker` の失敗で pending に戻さない**: マーカー無しの pending 復帰は遅延起動ラッパーと新 attempt の並走を招く。`Err(Io)` は状態を変更せず報告してスキップし、次 tick が再試行する(TC-exec-tick-074)。
- **`create` の冪等性の境界**: 「`ws.path` に `ws.branch` の worktree がある」場合だけが達成済み。パスの存在だけで成功にすると、別ブランチの worktree を掴んだまま起動して他タスクの成果を壊す。ブランチのみ残存時は**先端を変えずに張り直す**(積まれたコミットを失わない)。同定をパスの文字列比較で行うと、シンボリックリンクを含むホームや macOS の一時ディレクトリで判定が必ず外れる。正規化は**比較する両側に対称に**かける — 片側だけだと Windows の拡張長パス(`\\?\C:\...`)と git の `C:/...` 出力で鍵が恒常的に不一致になり、既存 worktree を持つタスクが毎 tick `Failed` を積んで凍結する。同定と分岐は adr.md ADR-085 で確定させる。
- **`CommandLine` の生成経路**: 公開コンストラクタが無く、ラッパーが argv から復元できない。`rehydrate` を足す(adr.md ADR-079)が、`DOM-definition-023` の「`expand` の結果としてのみ生成される」と食い違うので spec 追従を提起する。
- **`RunDirPath` から state root を復元する**: ラッパーは config もホームも読まないため、RunStore の構築に必要な `StateRoot` を `run_dir` から逆導出する(adr.md ADR-078)。台帳に無い追加であり、これも spec 追従の提起対象。
- **既存テストの破壊**: `crates/pulsen/tests/cli_usage.rs` の「提供するサブコマンドはタスク登録だけである」は `tick` を足した瞬間に落ちる。`wrapper` を隠す判断(adr.md ADR-077)の検証点として書き換える。
- **`Runtime` のアクセサ復活**: ADR-061 は「呼び出しの無い `pub` は落とし、必要になったスライスで理由つきで戻す」と定めた。tick は `state_root()` / `worktree_root()` を要求するので、why を添えて戻す。
- **時刻依存テストの安定性**: 猶予時間30秒の境界は実時間で待てない。境界3件(TC-077/078/079)は差し替え可能な `Clock` に対するユースケース層テストで消化する(AC-17)。受け入れテストでは猶予内(`KeepWaiting`)の経路だけを扱う。
- **適合テストが `cargo build --examples` に依存する**: `agent_probe` / `spawn_probe` が見つからなければフックが `None` を返してスキップになる。スキップ許容集合(ADR-055)に**入れない**ことで、「examples を作り忘れた」が緑にならないようにする。
- **手動テストの部分実行**: `spec/manual-tests/` の該当 TC は notify(#3)・`ls` / `show`(#4)・終端処理(#6)を含む手順を持つ。下記「テスト方針」に読み替えと実行範囲を ID 単位で書く。

## テスト方針

- **ドメイン**(`cargo test` のユニットテスト、I/O なし): `LaunchingClassifier` の全分岐と境界(30秒の等号・超過・巻き戻り)、`Task` の遷移6種の前提状態・事後条件・カウンタ規則(等号では凍結しない・`confirm_running` は `spawn_fail_count` だけをリセット・`record_spawn_failure_in_place` は状態も attempt も変えない)、問い合わせ5種、`WorkspacePlanner` の導出、`RunDirPath` の6パス、`CommandLine::rehydrate` の往復と0トークン拒否、`RunDirPath::state_root` の逆導出と形式外の `None`。テスト名は仕様の言葉(日本語)で付ける。
- **ポート適合テスト**(`pulsen-conformance`): RunStore 21件・ProcessController 16件(`own_identity` / `run_agent` の13件と `spawn_wrapper` の3件の2スイート。adr.md ADR-083)・WorktreeManager 追加7件を spec の表と1行1関数で対応させる(WorktreeManager にはこれに加え、ADR-085 の復旧2分岐を両方通すための台帳行なしの追加ケースを1件置く)。ハーネスのフックは「破損・状況の意味」だけを受け取り(ADR-027)、権限操作系は制限が実際に効いたことを確認してから `Some` を返す。並行観測のケース(TC-port-run-store-016)は `concurrent_store` のスキップ可能フックに隔離し(ADR-027)、読み手の停止フラグは `Drop` に載せる(ADR-063)。スキップ許容集合は環境の能力から実行時に決める(ADR-055)。`crates/pulsen-conformance/HOOKS.md` は44行分の対応表(と台帳行に対応しない追加ケース1件)に加え、冒頭の対象ポート数・総行数・区分別件数表と「環境で走らなくなりうる行」の表も更新する(上のスキップ表の根拠になる)。
- **ユースケース**: `Tick` / `RunWrapper` はポートをジェネリック引数で受け取り、異常系・境界値はすべてテストダブル(`pulsen-conformance::doubles`)に対して書く。実プロセス・実ファイルシステムは使わない。ここで消化するのは AC-17 の一覧(ロック異常・`list_active` の Io・worktree 作成失敗と上限超過・展開失敗の5経路・`prepare_attempt` 失敗・`spawn_wrapper` の同期エラー・`RunFileError` 3種・マーカー書き込み失敗・猶予境界3件・`save` 失敗)。
- **受け入れテスト**(`crates/pulsen/tests/`、実バイナリ): 一時ホーム + `git init` した一時リポジトリに対して `pulsen add` → `pulsen tick` を実行し、worktree(`worktrees/<task-id>` / ブランチ `pulsen/<task-id>`)の作成・タスクファイルの `launching` 記録・runディレクトリの starttime / pid / ログ / exit の出現・続けての `tick` による running 取込を検証する。エージェントは `examples/agent_probe` を config.yaml のエージェント定義として使う(実在するエージェントに依存しない)。ロック競合(0 スキップ)・パース不能タスクファイルの混在・タスク0件・`state/tasks/` 未作成・`state/archive/<task-id>.json` に置いたタスクが走査されないこともここで検証する。ラッパーはデタッチ起動で非同期に完了するため、runディレクトリの内容を読む前と次の tick を打つ前は期限付きポーリングのヘルパー(`tests/common`)で待ち合わせる — 待たずに書くとテストが実行環境の負荷に依存して落ちる。ラッパーがロックFDを継承していないこと(継承していると実行中のすべての tick がスキップされる)は、滞留するエージェントを起動したまま次の tick が競合せずに走ることで観測する。
- **順序の検証**: 「starttime は pid より先」「マーカー確認は pid 書き込みの後」は、ラッパーのユースケーステストでダブルの呼び出し記録(`calls()`)の並びを主張する。tick 側の「マーカー書き込み後に pid 再確認」も同じ形で主張する。
- **手動確認**: Issue の「検証 / 手順書」に揃える。範囲は手順書を読んで手順番号単位で決め、本スライスに無いコマンド(`ls` / `show` / `abort` / `set-status`)と notify(#3)・終端処理(#6)を要する手順は実行しない。`show` / `ls` で読む値(`attempt_count` / `spawn_fail_count` / 実行状態 / attempt の run_dir)は **`state/tasks/<task-id>.json` を直接読んで**確認する。runディレクトリのファイルは JSON なので `cat exit` の期待値は `{"code":0}` になる(adr.md ADR-080)。

  | 手順書 | ID | 本スライスでの実行範囲 |
  |---|---|---|
  | setup.md | TC-02 | 全手順。**前提として `setup.md` の事前準備1〜3 と TC-01 手順1(config.yaml の作成)を先行実行する** — TC-02 は「TC-01 完了直後」を前提にしており、config.yaml が無いと未初期化の案内になる。TC-01 手順2 の `pulsen ls`(#4)は実行せず、`state/tasks/` が空であることの直読で代替する。setup.md 内では TC-03 の登録より前に実行する(タスク0件が前提のため)。タスク0件で `tick` が対象なしを表示して 0 で終わること(AC-12 の直接の裏付け) |
  | setup.md | TC-06 | **前提として事前準備1〜3・TC-01 手順1・TC-03 手順1(`$PULSEN_HOME/workflows/implement.yaml` の作成)を先行実行する** — 手順3 が登録する `implement` ワークフローは TC-03 手順1 で作られ、TC-03 手順3 の `pulsen ls`(#4)は本スライスに無いので先行実行から外す。範囲は手順1〜3 と、手順4・5 のうち**起動フェーズまで**(cron.log にサマリーが追記され、worktree・ブランチ・runディレクトリが作られること。`ls` / `show` はタスクファイルと `worktrees/` の直読で代替)。判定・遷移(#3)以降は進まないので手順6 の成果物確認は行わない。手順7(crontab 削除)は実行し、手順8 は `set-status`(#5)と終端処理(#6)のため落とす。tick が外部スケジューラーから任意の cwd で起動されても動くこと(絶対パスの帳簿)がこの TC の主眼 |
  | setup.md | TC-34 | **前提として事前準備1〜3・TC-01 手順1・TC-03 手順1 を先行実行する**(手順1 が登録する `implement` ワークフローの出所)。範囲は手順1〜4(`show` はタスクファイル直読で代替。手順4 の**通知の受領は確認しない**(#3))。手順5 は `retry`(#5)のため実行しない — 凍結後のタスクを起動可能に戻す手段が本スライスに無い。`TC-exec-tick-055`(config 修正が次の tick で反映される)は**別タスク**で裏付ける: 新規登録 → `spawn_fail_count` が上限未満のうちに1〜2回 tick → config.yaml のエージェント定義を戻す → 次の tick で起動に成功する |
  | task-execution.md | TC-03 | 手順1〜4(worktree・ブランチ・runディレクトリの生成と、続く tick での running 取込)と手順7(`stdout.log` に `planning` が出て手順2 の編集が反映されていないこと = スナップショットが使われたこと、`pid` / `starttime` / `stderr.log` の存在)。判定・遷移・完了・終端(手順5・6・8〜11)は #3 / #6。**手順12(元YAMLの復元)は実行範囲外だが必ず実行する** — task-execution.md は記載順の実行を前提としており、改変済みの `pipeline.yaml` を後続の TC-04 / TC-12 / TC-16 / TC-20 / TC-24 / TC-25 が引き継ぐため |
  | task-execution.md | TC-04 | 手順1〜3 と手順5。同一リポジトリに2タスクを登録し、別々の worktree・ブランチ・runディレクトリで並行して起動されること、両ブランチが base から作られていることを確認する。手順5 の「3コミットが積まれている」は遷移(#3)を要するので確認せず、ブランチの存在までとする。手順4(アーカイブまでの繰り返し)は #3 / #6 |
  | task-execution.md | TC-12 | 登録後のリポジトリ消失 → tick で `record_tool_failure(WorktreeCreate)` により failed になり `attempt_count` が増えること(タスクファイル直読)、上限超過で stopped になることまで。通知の受領は確認しない(#3) |
  | task-execution.md | TC-16 | 手順1〜5。登録後の config 破壊 → 展開失敗で `spawn_fail_count` が増え実行状態が変わらないこと(タスクファイル直読)、上限超過で stopped になることまで。手順6(通知の受領)は #3。**手順7(`shellx:` → `shell:` の復元)は実行範囲外だが必ず実行する** — 後続の TC-20 / TC-24 / TC-25 が正常な `shell` 定義を前提にしており、特に TC-20 は追加登録した T20p が破損タスクファイルと同一 tick で**起動される**ことを期待するため、config を戻さないとこの期待が必ず外れる(`spawn_fail_count` が増えるだけになる)。TC-12 の `mv repo2 repo2.gone` のように後続に影響しない改変とは扱いが異なる |
  | task-execution.md | TC-20 | 手順1〜4。手順1 に **`pulsen add --workflow pipeline --repo /tmp/pulsen-test/repo` をもう1件追加**し、IDを T20p として控える — `draft.yaml` は `initial: done` / `done: run: cleanup`(Pending × Cleanup)で、Cleanup のアームは本スライスで配線しないため T20h には何も起こらず(adr.md ADR-073)、「他タスクへの影響なし」を裏付ける対象が無くなるため。手順3 の期待は「T20h はアーカイブされる」→「**T20h には何も行われない**(タスクファイルは `state/tasks/` に残り、書き込みも起動も発生しない)」と「**T20p が破損ファイルと同一 tick で起動される**(worktree・ブランチ・runディレクトリが作られる)」に読み替える。パース不能なタスクファイルが混在しても tick が 0 で終わり、破損ファイルに書き込まないことを確認する。手順5(`ls`)は #4、手順6(notify.log)は #3、手順7(`set-status`)は #5 |
  | task-execution.md | TC-24 | 手順2・4・5 と、手順6 のうち `pulsen tick; echo $?`。ロック保持は手順1・3(長時間の notify_cmd + `pulsen abort`)を使わず `examples/lock_holder` で作る — notify は #3、`abort` は #5 で、どちらも本スライスに無いため。事前に `cargo build --examples` し、別端末で `target/debug/examples/lock_holder /tmp/pulsen-test/home/state/lock` を**標準入力を開いたまま**起動する(`locked` の出力がロック取得の合図。ロックファイルは `$PULSEN_HOME/state/lock`)。保持中に手順4(`tick` が 0 でスキップ)と手順5(`add` が非0)を実行し、保持プロセスの標準入力を閉じて解放してから手順6 を実行する |
  | task-execution.md | TC-25 | 手順1〜4(手順1・3 の `show` はタスクファイル直読で代替)。スナップショットのみ破損したタスクを tick が報告してスキップし、書き込まず `state/runs/<T25>/` も作らないこと。手順4 の notify.log と再通知(#3)は確認しない。手順5(`set-status` による片付け)は #5 |

  上表で落とす手順は、Issue の完了条件に従い「見送る行はチェックせず理由を Issue のコメントに残す」運用に合わせる。
