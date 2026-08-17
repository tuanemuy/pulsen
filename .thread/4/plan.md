# 実装計画 — Issue #4: 状態の確認と追跡(ls / show)

**Issue:** #4
**作成日:** 2026-08-16
**複雑度:** 中規模
**実装方針:** steps.md

---

## 目的

`pulsen ls` でタスクの一覧と絞り込み(タスクステータス・実行状態・アーカイブ含む)を、`pulsen show <task-id>` で1タスクの全属性・スナップショットの定義済みステータス・最新attemptの実行メタデータ参照を確認できるようにする。どちらも読み取り専用で排他ロックを取得しない。

## 受け入れ基準

| # | 基準(検証可能な形で) | 由来 | 対応ステップ |
|---|---|---|---|
| AC-1 | **実装チェックリスト全92行の実装**(Issue #4 本文参照。スタブ・仮実装・部分実装は不可)。各行は `spec/inventory/{layer}.md` の台帳行と1:1で対応し、台帳の PASS 要件を**すべて**満たす実装とテストが存在し、そのテストが実際に走って通った行にだけチェックを付ける | Issue 完了条件 | 全ステップ |
| AC-2 | **タスク一覧とステータス絞り込みが end-to-end で動く**: 異なるワークフロー・タスクステータス・実行状態のタスクが登録された状態で `pulsen ls` を打つと、各行にタスクID・ワークフロー名・リポジトリ・ブランチ・タスクステータス・**実行状態**・attempt_count・更新日時が並んで 0 で終わる。`--status` / `--state` は AND、`--all` は対象集合の拡張として拡張後に絞り込みが適用される。`--state` に固定6値以外(大文字混じり・空文字を含む)を渡すと有効値6つを添えて非0 | `spec/scenario/monitoring.md#タスク一覧とステータス絞り込み`、Issue 検証1・2、PAGE-ls-001〜011 | 4, 6, 7, 8, 10, 12 |
| AC-3 | **タスク詳細の確認が end-to-end で動く**: `pulsen show <task-id>` が、ワークフロー名・対象(リポジトリ・ベースブランチ)・タスクステータス・実行状態・workspace_path・branch・3つのカウンタと適用上限・現在attemptのrunディレクトリパス/PID/kill同定子/starttime・直近の失敗要因・notified_at・更新日時・スナップショットの定義済みステータス一覧・スナップショット保存先パスを表示して 0 で終わる。未実行タスクは attempt が「なし」・workspace が「未作成」、launching で同定情報未取り込みは該当項目が「未取得」、アーカイブ済みはその旨と worktree 削除済みの明示 | `spec/scenario/monitoring.md#タスク詳細の確認`、Issue 検証3・4、PAGE-show-001〜006・008 | 5, 6, 9, 11, 12 |
| AC-4 | **エージェント実行ログの確認の起点になる**: show が現在attemptの `stdout.log` / `stderr.log` / `exit` のパスと exit の値を示し、runディレクトリが不在(未作成・gc 済み)なら「存在しない」と明示して 0 で終わる。exit の読み取り・runディレクトリの存在確認が失敗しても、当該項目に読めない旨を注記して表示を続け 0 で終わる。spawn 失敗で pending へ戻った痕跡と取り違えないよう、示すのは**タスクファイルが指す現在attempt**のパスである | `spec/scenario/monitoring.md#エージェント実行ログの確認`、PAGE-show-004・010、`spec/domains/execution.md#runstore` | 1, 2, 3, 5, 9, 11 |
| AC-5 | **stopped タスクの原因調査ができる**: stopped のタスクで凍結要因(`StopReason` の4値)と notified_at が表示され、リトライ上限超過・判定上限超過・連続spawn失敗が `attempt_count` / `judge_attempt_count` / `spawn_fail_count` と併記された上限から判別できる。ツール操作の失敗で凍結した場合は `last_failure`(FailureNote)が、同期spawn失敗経路では attempt が「なし」であることが表示される | `spec/scenario/monitoring.md#stoppedタスクの原因調査`、PAGE-show-002・005 | 5, 9, 11 |
| AC-6 | **タスクファイルの直接閲覧・修復の入口になる**: パース不能なタスクファイル(`Corrupt`)が混ざっても `ls` は一覧全体を失敗させず、ファイルパスと読めない旨を報告して 0 で終わる。スナップショットのみ読めないタスクは行として表示され印が付き、絞り込みの対象にもなる。同じファイルを `show` で指すとパースエラーの内容とファイルパスを表示して非0で終わる。スナップショットのみ破損なら読める項目をすべて表示し、定義済みステータス一覧を出さずに注記して 0 で終わる | `spec/scenario/monitoring.md#タスクファイルの直接閲覧修復`、PAGE-ls-008、PAGE-show-007・009、pages ※5 / ※6 | 4, 5, 7, 8, 9, 10, 11, 12 |
| AC-7 | **読み取り専用であることが外形で確認できる**: `ls` / `show` はどちらも `ExclusiveLock` を受け取らず(型として持たない)、別の操作が排他ロックを保持している最中でも通常どおり結果を返して 0 で終わる。`show` は workspace_path の存在検証を行わない(worktree を手で削除しても表示は変わらず 0) | pages 共通事項・縮退表(ロック競合の「—(非取得)」)、PAGE-show-011、TC-task-list-tasks-025、TC-task-show-task-035 | 4, 5, 9, 12 |
| AC-8 | **品質ゲートと隔離の維持**: `cargo build` / `cargo test` / `cargo clippy -- -D warnings` / `cargo fmt --check` が通る。`pulsen-domain` の `[dependencies]` は空のまま、本番依存は増やさない。`pulsen-domain` の `unsafe_code = "forbid"` と `pulsen` の `unsafe_code = "deny"` はそのままで、`#[allow(unsafe_code)]` は `adapter/process.rs` の1箇所から増やさない。ターゲット述語つき `cfg` は `crates/pulsen/src/` の3ファイル(`util/atomic.rs` / `adapter/process.rs` / `adapter/task_repository.rs`)から増やさない | CLAUDE.md 技術方針、Issue #3 AC-7 | 全ステップ |
| AC-9 | **記帳**: 部分消化になった行・実行環境がスキップにした行にはチェックを付けず、前者は消化した範囲と引き取り先を、後者はスキップした理由と確認した環境(OS・root か否か)を Issue のコメントに残す。下記「spec との差分として提起するもの」も同じコメントで提起する | Issue 完了条件 | 13 |

## スコープ

### 含まれないもの

- **abort / retry / set-status** と `Task::{abort, retry, set_status}` / `DegradedTask::{abort, retry}` / `PAGE-abort-*` / `PAGE-retry-*` / `PAGE-set-status-*` — Issue #5。show は `StopReason::Aborted` を含む4値を表示側で網羅するが、**abort で凍結したタスクを CLI で作る経路は本スライスに無い**(自動テストではダブル・フィクスチャで作る)。`show` が案内する「定義済みステータス一覧」の用途(set-status の遷移先確認)も #5 で初めて使われる。
- **終端処理・アーカイブ・runディレクトリの gc** と `RunStore::{list_runs, delete_attempt, remove_task_dir_if_empty}` / `WorktreeManager::remove` / `GcPolicy` / tick の `Branch::Cleanup` — Issue #6。本スライスが足す `RunStore` のメソッドは `attempt_exists` **だけ**で、gc 用の3メソッドは引き続き宣言しない(Issue #1 で確立した「本スライスで使わないメソッドは宣言しない」規約)。**`state/tasks/` から `state/archive/` へタスクを移す経路も本スライスに無い**(下記リスク参照)。
- **機械可読形式(JSON 等)の出力** — pages 共通事項が本フェーズのスコープ外と定めている。
- **ログの中身の表示・追跡・ページング** — show が示すのは `stdout.log` / `stderr.log` / `exit` の**パスと exit の値**まで。閲覧・`tail -f` は利用者が別の手段で行う(`spec/scenario/monitoring.md#エージェント実行ログの確認`)。
- **過去attemptの列挙** — show が示すのはタスクファイルが指す**現在attempt**のみ。attempt 番号違いのディレクトリを辿るのは利用者の操作。
- **一覧の並び順・ページング・件数制限** — `TaskRepository` の契約が「走査は全件読み込み。絞り込み・並び順は呼び出し側で行い、ページングは持たない」と定めており、spec は並び順を規定していない。
- **`--status` の値の検証・タイポ補正・似たIDの列挙** — タスクステータスはユーザー定義語彙のため検証しない(該当0件)。show の「見つからない」も補助は必須ではない。
- **CI・MSRV・Windows 実機での検証** — Issue #10(完了済み)の枠組みに乗るだけで、本スライスで新しい検証は足さない。

## チェックリスト行にチェックを付ける基準

Issue #1 / #2 / #3 で確立した基準をそのまま使う。**チェックを付ける**のは、台帳行(`spec/inventory/*.md`)の PASS 要件を**すべて**満たす実装とテストが存在し、そのテストが実際に走って通っている行。**チェックを付けない**のは、環境が前提を作れずスキップで終わった行と、スライス境界により PASS 要件の一部しか消化していない行。

### 本スライスで部分消化になることが計画時点で分かっている行

| 行 | 消化する範囲 | 残る部分 | 引き取り先 |
|---|---|---|---|
| `PAGE-ls-004` / `PAGE-show-008`(アーカイブ済み) | `state/archive/` を対象集合に含める・アーカイブ済みの印と注記・`state/archive/<task-id>.json` のパス表示。前提は `TaskRepository::archive`(#1 実装済み)とフィクスチャで作る | **tick の終端処理でアーカイブ済みタスクが生まれる経路**が無いため、手順書(`spec/manual-tests/monitoring.md` の `TASK_D`、`cleanup.md` TC-13〜15)を end-to-end で通せない | #6 |
| `TC-task-show-task-031`(gc で削除済みのrunディレクトリ) | `attempt_exists` が false のときに「存在しない」と表示する挙動。前提はディレクトリを直接削除して作る | gc がディレクトリを消す経路そのもの | #6 |

`PAGE-show-005` の `StopReason::Aborted` は表示側の網羅(`match` の全アーム)としては本スライスで満たすが、abort 経路で凍結したタスクを CLI で作れないため、手順書 TC-15 の実行は #5 待ちになる。行自体は他3経路と `FailureNote` の表示で PASS 要件を満たすため消化扱いとする。

## リスクと注意点

- **アーカイブ済みタスクを作る経路が本スライスに無い**: `state/archive/` へ移すのは tick の終端処理(#6)だけで、Issue 本文の「依存」も #2 しか挙げていない。自動テストは `TaskRepository::archive`(#1 で実装済み)またはフィクスチャの直接配置で前提を作る。**受け入れテストで `archive` を呼ぶための CLI が無い**ため、`crates/pulsen/tests/common/` に「アーカイブ側へタスクファイルを置く」フィクスチャを足すことになる(adr.md ADR-003)。
- **`show` の表示項目が多く、縮退の場合分けが項目ごとに違う**: workspace(未作成 / 実在 / 削除済み)、attempt(なし / あり)、process(未取得 / 取得済み)、run ディレクトリ(存在 / 不在 / 確認失敗)、exit(なし / あり / 読めない)、retry 上限(`Applicable` / `NotApplicable` / `Unknown`)、defined_statuses(あり / なし)。CLAUDE.md が `match` のワイルドカードを禁じているため、**それぞれを直和型で表して網羅 `match` に落とす**。`Option<T>` の入れ子で表すと `NotApplicable`(適用対象がない)と `Unknown`(スナップショット破損で導出不能)が同じ `None` に潰れ、TC-task-show-task-029 が主張する区別が消える。
- **ls の破損報告は2系統ある**: `Corrupt`(行にならず `unreadable` へ・パスと理由)と `SnapshotUnreadable`(行として出て印が付き、**絞り込みの対象にもなる**)。後者を `unreadable` 側に寄せると TC-task-list-tasks-021 が落ちる。pages ※5 の字義との差は下記「spec との差分」で提起する。
- **`attempt_exists` / `read_exit` の失敗をエラーに昇格させない**: どちらも「当該項目を読めない旨の注記付きで表示を継続して 0」。`?` で早期 return すると TC-task-show-task-021 / 022 が非0になる。ユースケースの戻り値の型に「読めなかった」を値として載せる。
- **`show` は workspace_path の存在検証を行わない**: 表示のために `Path::exists()` を呼びたくなるが、pages ※9 が明示的に禁じている。アーカイブ済みの「削除済み」注記は**アーカイブ済みであるという事実から**導き、ファイルシステムには問わない。
- **`ls` / `show` がロックを取得しない**: ユースケースの構築に `ExclusiveLock` を渡さないことで型として担保する。既存の `RegisterTask` / `Tick` は `lock` をジェネリック引数に持つため、同じ形をコピーすると誤って足しやすい。
- **`render.rs` が既に 1456 行**: ls / show の文言を同じファイルに足すと 2000 行を超える。表示層の分割方針は adr.md ADR-002 で決める。
- **一覧の出力形式を spec が定めていない**: pages は「人間可読なテキスト」までしか定めず、列の並び・区切り・整列は表示層の裁量。破損報告と一覧を1つの出力にどう同居させるかも含めて adr.md ADR-001 で決める。決めずに書くとテストが綴りに結合する。
- **`--state` の parse 位置**: 入力DTOは `Option<String>` で受け、`ExecutionStateKind::parse`(既に `VALID` の6値を `StateKindError::Unknown` に載せて返す)をユースケースの入力境界で1度だけ呼ぶ。clap の `value_parser` で先に弾くと、有効値一覧の文言が clap のエラー整形に乗って PAGE-ls-011 の「有効な値の一覧を添えて非0」が表示層の管理下から外れる。既存の `--base`(`allow_hyphen_values` でドメインに渡す)と同じ流儀。
- **`--status` に空文字列を渡された場合**: 検証しない語彙なので「該当0件で 0」。`--state` の空文字列(非0)と扱いが逆になる。TC-task-list-tasks-017 と 018 が隣り合わせでこの差を主張する。
- **`show` の「スナップショット保存先パス」はタスクファイル自身のパス**: ADR-015(`2026-08-11-snapshot-embedded-in-task-file.md`)によりスナップショットはタスクファイルに埋め込まれている。別ファイルのパスを組み立てないこと。現役なら `state/tasks/<id>.json`、アーカイブ済みなら `state/archive/<id>.json`(`TaskFilePath::active` / `archived`)。
- **`AttemptRef::process()` は3値をまとめて持つ**: `ProcessIdent { pid, kill_ident, starttime }` は `Option` が外側に1つあるだけで、pid だけ取り込み済みという状態は型として存在しない。spec の出力DTOは `pid?` / `kill_ident?` / `starttime?` と個別に任意で書いているが、「未取得」は3項目まとめて起きる。
- **時刻の表示**: `Timestamp` の表示形式は既存の `render.rs` / タスクファイルの直列化と揃える。更新日時・notified_at・starttime の wall が同じ形式で読めないと、TC-task-show-task-008(at-least-once の検証に使える)の目的が果たせない。
- **既存テストへの影響**: `RunStore` トレイトにメソッドを1つ足すと、実装している `FsRunStore` と `ScriptedRunStore` の両方に追加が要る。`ScriptedRunStore` は「台本を使い切ったらパニック」する流儀なので、`attempt_exists` の台本を与えないユースケーステストは書けない。

## spec との差分として提起するもの

転記の過程で、spec の記述どうし、あるいは spec と本スライスの依存関係が食い違う点が見つかった。**本スライスは下記の解釈で実装し、spec 追従は Issue のコメントで提起する**(勝手に spec を書き換えない)。

- **`ls` のスナップショット破損の報告形式**: `spec/pages/index.md` の ※5 は「ファイルパスと読めない旨を一覧に含めて報告する」を `Corrupt` と `SnapshotUnreadable` の両方に掛けており、`PAGE-ls-008` の PASS 要件も両方を同じ文で扱う。一方 `spec/usecases/task.md` の ListTasks 出力DTOは、パスを持つ `UnreadableRow` に載るのは `Corrupt` だけで、`SnapshotUnreadable` は通常の `TaskRow` に `snapshot_unreadable: bool` が立つだけ(パスを持たない)と定める。**実装はユースケース側に従う**(`SnapshotUnreadable` はタスクIDで特定でき、絞り込みの対象にもなるため行として出す必然がある)。
- **Issue 本文の「依存」が #6 を挙げていない**: チェックリストの `PAGE-ls-004` / `PAGE-show-008` / `TC-task-list-tasks-006` / `007` / `008` / `023` / `024` / `TC-task-show-task-016` / `017` はアーカイブ済みタスクを前提とするが、`state/archive/` へタスクを移すのは Issue #6 の終端処理である。**実装と自動テストは #6 を待たずに可能**(前提はフィクスチャで作れる)だが、Issue の「検証」欄が指す手順書は通せない(下記)。
- **Issue の「検証 / 手順書」が本スライスに無いコマンドを使う**: `spec/manual-tests/monitoring.md` の事前準備は `TASK_D`(cleanup 到達でアーカイブ済み → #6)と `TASK_G`(abort 経路の stopped → #5)を要求し、TC-09 手順4 と TC-15 は `pulsen abort` を直接使う。`spec/manual-tests/cleanup.md` の TC-13〜15・17 はアーカイブ済み(#6)を、TC-23 は gc(#6)を前提とする。**手順書のうち #5 / #6 を要する手順は本スライスでは実行せず、実行範囲をステップ13 で Issue のコメントに残す**(Issue #3 で確立した扱い)。
- **`spec/domains/execution.md` の `RunStore` 表と実装のメソッド数**: 表は13メソッドを挙げるが、実装は本スライスの `attempt_exists` を足しても10メソッド(`list_runs` / `delete_attempt` / `remove_task_dir_if_empty` が #6 待ち)。`DOM-execution-038` の PASS 要件は `attempt_exists` 単体なので満たすが、ポート全体としては未完である。

## テスト方針

- **ポート適合テスト**(`crates/pulsen-conformance/src/run_store.rs`): `attempt_exists` の2行(TC-port-run-store-022 / 023)を1行1関数で足し、`run_store_conformance!` のケース一覧と `HOOKS.md` の RunStore の節(21行 → 23行)を更新する。どちらのケースも新しいフックを要さない — 前提は `prepare_attempt` の呼び出しと「attemptディレクトリ自体が不在」(既存の `attempt_dir_present` で確認できる)だけで組める。「空ディレクトリ」と「ディレクトリごと不在」の区別が要点なので、TC-022 は**ファイルを1つも書かない**状態で `Ok(true)` を主張する。
- **ユースケーステスト**(ダブルに対するテスト、実プロセス・実ファイルシステムなし。`crates/pulsen/tests/list_tasks.rs` / `show_task.rs` を新設): 実アダプターでは外から作りにくい状況と、分岐の網羅をここで消化する — `list_active` / `list_archived` の `ReadError::Io`、`find` の `Corrupt` / `NotFound` / `Archived`、`SnapshotUnreadable`(DegradedTask)、`attempt_exists` の `Err(Io)`、`read_exit` の `Corrupt` / `Io`、`--state` の6値と不正値・大文字・空文字、`--status` の未知値、AND 絞り込みと `--all` の合成、`RetryLimitInfo` の3値(AgentRun の `retries` 上書きあり/なし・Cleanup の常に2・Wait の `NotApplicable`・DegradedTask の `Unknown`)、`StopReason` の4値と `FailureNote`、TaskId の境界値(空・1文字・64文字・65文字・不正文字・先頭ハイフン)。**`ExclusiveLock` をユースケースに渡さないこと**は型で示し、テストでは主張しない(渡せないものは書けない)。
- **受け入れテスト**(`crates/pulsen/tests/cli_ls.rs` / `cli_show.rs`、実バイナリ + 一時ホーム): 表示そのものと exit code を検証する。前提は `pulsen add` / `pulsen tick`(実アダプター)で作れるものはそれで作り、作れないもの(アーカイブ済み・破損ファイル)はタスクファイルの直接配置で作る。検証する筋 — 複数タスクの一覧に実行状態とタスクステータスが**併記**されること / 該当なしの空表示 / `--state` 不正値の有効値一覧つき非0 / 破損ファイル混在時に一覧が失敗しないこと / 実行履歴のあるタスクの show でログ・exit のパスが出ること / 存在しないIDの非0 / config.yaml 不在の非0 / **別プロセスがロックを保持したままでも 0 で結果が返ること**(既存の `examples/lock_holder` と `tests/common/lock.rs` を使う) / tick の書き込みと同時の読み取りで書きかけを観測しないこと。
- **テスト名は仕様の言葉(日本語)で付ける**。実装の内部構造(DTO のフィールド名・関数名)には依存させない。
- **手動確認**: Issue の「検証 / 手順書」に揃える。範囲の確定は `.thread/4/testing.md`「本スライスでの実行範囲」を正とする — `spec/manual-tests/monitoring.md` は TC-01〜TC-08・TC-09(手順1〜3)・TC-10〜TC-14・TC-16〜TC-33・TC-34(手順2〜6)を実行し、TC-09 手順4・TC-15・TC-34 手順1 と `cleanup.md` の TC-13〜15・17・23 は実行範囲から外す(理由をステップ13 のコメントに残す)。アーカイブ済みを前提とする TC(TC-05・06・10・11・25)は、手順書自身が TC-34 で定める手動移動(`git worktree remove` → `state/archive/` へ `mv`)で前提を作って**表示としては消化する**が、アーカイブが生まれる経路そのものは #6 まで確認できない。abort 由来の表示は自動テストのフィクスチャで担保する。
