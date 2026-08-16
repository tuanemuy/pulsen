# 実装手順 — Issue #4

## 設計

**この Issue は spec-to-issues が起票した縦スライスであり、設計は spec に確定済みである。** 本節は新しい設計を導出せず、チェックリスト各行の「定義場所」と、乗るべき既存実装のパターンを指し示す。判断に迷ったら以下の spec を直接読む。

正本:

- `spec/scenario/monitoring.md` — 5シナリオ(一覧と絞り込み / 詳細 / 実行ログ / stopped 原因調査 / 直接閲覧・修復)
- `spec/pages/index.md#ls` / `#show` / `#縮退状態の共通規則`(表と ※1〜※9)
- `spec/usecases/task.md#listtasksls` / `#showtaskshow`(入力DTO・出力DTO・処理フロー・エラーケース)
- `spec/domains/execution.md#runstore`(`attempt_exists` の行と契約)
- `spec/testcases/task/list-tasks.md` / `spec/testcases/task/show-task.md` / `spec/testcases/ports/run-store.md`

### モジュールの増分

```
crates/pulsen-domain/src/
  execution/port.rs            + RunStore::attempt_exists                     DOM-execution-038
crates/pulsen/src/
  adapter/run_store.rs         + FsRunStore::attempt_exists                   ADP-runstore-005
  application/list_tasks.rs (新規)  ListTasks / 入力DTO / TaskRow / UnreadableRow  UC-task-004
  application/show_task.rs (新規)   ShowTask / TaskDetail / RetryLimitInfo /
                                    AttemptSummary / ShowTaskError            UC-task-005
  application/mod.rs           + list_tasks / show_task
  cli/args.rs                  + Command::Ls(LsArgs) / Command::Show(ShowArgs) PAGE-ls-001〜005 / PAGE-show-001
  cli/ls.rs (新規)              ホーム解決 → ListTasks の実行
  cli/show.rs (新規)            ホーム解決 → ShowTask の実行
  cli/mod.rs                   + Ls / Show のアームと終了コード
  cli/render/ (分割。adr.md ADR-002)
    ls.rs (新規)                一覧・空表示・破損報告・エラー文言           PAGE-ls-006〜011
    show.rs (新規)              詳細の全項目と縮退の注記                     PAGE-show-002〜011
crates/pulsen-conformance/
  src/run_store.rs             + TC-port-run-store-022 / 023 とケース一覧      TC-port-run-store-022 / 023
  src/doubles/run_store.rs     + with_attempt_exists / RunStoreCall::AttemptExists
  HOOKS.md                     RunStore の節を 21行 → 23行
crates/pulsen/tests/
  list_tasks.rs (新規)          ダブルに対する ListTasks の振る舞い
  show_task.rs (新規)           ダブルに対する ShowTask の振る舞い
  cli_ls.rs / cli_show.rs (新規) 実バイナリの受け入れ
  common/mod.rs                + アーカイブ側へタスクファイルを置くフィクスチャ(adr.md ADR-003)
```

### ドメインモデルへの影響

**ドメインに足すのはポートのメソッド1つだけ**(チェックリストの DOM 行が `DOM-execution-038` の1行しかないことがそのまま範囲を示している)。値オブジェクト・遷移関数・エラー型の追加はない。

| 要素 | 定義場所 | 台帳 | 既存実装との関係 |
|---|---|---|---|
| `RunStore::attempt_exists(run_dir) -> Result<bool, Io>` | `spec/domains/execution.md#runstore` | DOM-execution-038 | `crates/pulsen-domain/src/execution/port.rs` の `RunStore` トレイト(現在9メソッド)に足す。gc 用の `list_runs` / `delete_attempt` / `remove_task_dir_if_empty` は #6 待ちで**宣言しない** |

`attempt_exists` の存在理由は契約の doc コメントに書く — read 系の `Ok(None)` では「空ディレクトリ」と「ディレクトリごと不在」を区別できず、show の「runディレクトリは存在しない(gc 済み等)」表示がその区別を要求する。

表示に要るドメインの読み取り口はすべて実装済みで、追加は不要:

| 表示項目 | 取得元 |
|---|---|
| タスクステータス / 実行状態 | `Task::task_status` / `execution` / `execution_kind`(`ExecutionStateKind` は `VALID` と `parse` を持つ。`crates/pulsen-domain/src/task/state.rs`) |
| workspace_path / branch | `Task::workspace() -> Option<&Workspace>`(`path` / `branch`) |
| 3つのカウンタ | `Task::counters() -> RetryCounters` |
| リトライ上限 | `Task::applicable_retry_limit() -> Option<u32>`(Wait は `None`。Cleanup は組み込み 2 を返す) |
| judge / spawn の上限 | `GlobalConfig::judge_attempt_limit` / `spawn_fail_limit` |
| 現在attemptと同定情報 | `Task::current_attempt() -> Option<&AttemptRef>` → `number` / `run_dir` / `process() -> Option<&ProcessIdent>` |
| ログ・exit のパス | `RunDirPath::stdout_log` / `stderr_log` / `exit_file`(`crates/pulsen-domain/src/task/path.rs`) |
| 定義済みステータス一覧 | `Task::snapshot()` → `WorkflowSnapshot::statuses()` のキー |
| 凍結要因 / notified_at | `ExecutionState::Stopped { reason, notified_at }`(`StopReason` は4値) |
| 直近の失敗要因 | `Task::last_failure() -> Option<&FailureNote>`(`FailureKind` は5値) |
| スナップショット保存先 | `TaskFilePath::active` / `archived`(ADR-015 によりタスクファイル自身) |
| スナップショット破損時 | `DegradedTask` の同名アクセサ群 + `snapshot_error()`(`crates/pulsen-domain/src/task/degraded.rs`) |

### ポート・アダプター

`spec/testcases/ports/run-store.md` の2行(TC-022 / 023)が期待結果の正本。

- `crates/pulsen/src/adapter/run_store.rs` の `FsRunStore` に実装する。既存の `marker_exists` が同じ形(パスの存在を `Result<bool, Io>` に畳む)なので、それに揃える。対象はファイルではなく **attempt ディレクトリ自身**(`run_dir.as_path()`)。
- 契約の共通規則(機構失敗は `Err(Io)` の値として返す・パニックしない)は既存の read 系と同じ扱い。
- ハーネス(`RunStoreHarness`)への**フック追加は不要** — TC-022 は `prepare_attempt` 済み・ファイル未書き込み、TC-023 は attemptディレクトリ不在で、どちらも既存のポートメソッドと既存フックだけで前提が組める。

### ユースケース / アプリケーションロジック

`spec/usecases/task.md` の ListTasks / ShowTask を、書かれている入力DTO・出力DTO・処理フローの順序どおりに実装する。既存の `crates/pulsen/src/application/register_task.rs` と同じ流儀に揃える — ポートはジェネリック引数で受け取り、文言は組まず原因を値として返し、CLI引数の文字列 → ドメイン型の parse は入力境界で一度だけ行う。

**`ExclusiveLock` はジェネリック引数にも `new` にも置かない。** 読み取りがロックを取らないこと(pages 共通事項・縮退表の「—(非取得)」)を型で示す。`RegisterTask` / `Tick` からコピーするときに落とし忘れないこと。

| ユースケース | 依存ポート | 定義場所 | 台帳 |
|---|---|---|---|
| `ListTasks` | `TaskRepository`(`list_active` / `list_archived`) | `spec/usecases/task.md#listtasksls` | UC-task-004 |
| `ShowTask` | `TaskRepository`(`find`)、`RunStore`(`attempt_exists` / `read_exit`)、`GlobalConfig`、`StateRoot` | `spec/usecases/task.md#showtaskshow` | UC-task-005 |

出力DTOで新設する型と、それが型で担保するもの:

| 型 | 形 | 担保するもの |
|---|---|---|
| `TaskRow` | `{ task_id, workflow_name, repo, branch: Option<BranchName>, task_status, execution_state: ExecutionStateKind, attempt_count, updated_at, archived: bool, snapshot_unreadable: bool }` | 一覧の表示項目(PAGE-ls-006)。`snapshot_unreadable` が立っていても**行として出る**ため絞り込みの対象になる |
| `UnreadableRow` | `{ path: PathBuf, message: String }` | `Corrupt` だけがここに来る(修復の入口。pages ※5) |
| `RetryLimitInfo` | `Applicable(u32) \| NotApplicable \| Unknown` | `NotApplicable`(Wait。適用対象がない)と `Unknown`(スナップショット破損で導出不能)を `Option<u32>` に潰さない |
| `AttemptSummary` | `{ number, run_dir, process: Option<ProcessIdent>, exit, stdout_log, stderr_log, run_dir_exists }` | attempt「なし」と「あり・同定情報未取得」を区別する |
| runディレクトリ / exit の読み取り結果 | 「不在 / 値 / 読めなかった(理由)」の3値 | `attempt_exists` / `read_exit` の失敗を**エラーに昇格させず**注記に落とす(pages 縮退表 show 行・TC-task-show-task-021 / 022) |

`ShowTask` の解決順は `TaskRepository::find` の契約(現役 → アーカイブ)に委ねる。`TaskLookup` の4値(`Active` / `Archived` / `NotFound` / `Corrupt`)と `TaskRecord` の2値(`Intact` / `SnapshotUnreadable`)を網羅 `match` で捌く。

### UI / プレゼンテーション

CLI が「画面」。`crates/pulsen/src/cli/` の既存構成に乗る。

- **引数**: `crates/pulsen/src/cli/args.rs` の `Command` に `Ls(LsArgs)` / `Show(ShowArgs)` を足す。`--home` は `global = true` なので追加の宣言は要らない。`--state` / `--status` は `Option<String>` のまま受け、値の検証はユースケースの入力境界で行う(clap の `value_parser` で弾かない — 有効値一覧の文言を表示層の管理下に置くため。既存の `--base` と同じ扱い)。
- **実行**: `cli/ls.rs` / `cli/show.rs` を `cli/add.rs`(45行)と同じ形で置く。`wire::compose(home)` でホーム解決と config 読み込み(pages ※1 の非0はここで起きる)を済ませ、`runtime.tasks()` / `runtime.runs()` / `runtime.config()` / `runtime.state_root()` を渡す。**`runtime.lock()` は渡さない。**
- **分岐と終了コード**: `cli/mod.rs::run` に `Ls` / `Show` のアームを足す。成功は標準出力 + `exit::SUCCESS`、失敗は標準エラー + `exit::FAILURE`(pages exit code 規約)。tick のロックスキップのような例外は無い。
- **文言**: `cli/render.rs`(1456行)を `cli/render/` に分割し、`ls.rs` / `show.rs` を新設する(adr.md ADR-002)。項目の並べ方は既存の `push_field` / `problem` の流儀に揃える。一覧の形式は adr.md ADR-001 で決める。

## 実装ステップ

依存方向の順(domain → adapter → 適合・ダブル → usecase → cli → test)に並べる。各ステップの見出しに消化する台帳IDを書く。

### 1. `RunStore` に `attempt_exists` を宣言する — DOM-execution-038

- **対象ファイル:** `crates/pulsen-domain/src/execution/port.rs`
- **spec:** `spec/domains/execution.md#runstore`(メソッド表の `attempt_exists` 行と、その下の契約の箇条書き)
- **変更内容:** `fn attempt_exists(&self, run_dir: &RunDirPath) -> Result<bool, Io>;` を `RunStore` トレイトに足す。doc コメントに「attempt ディレクトリ自体の存在確認。read 系の `Ok(None)` では『空ディレクトリ』と『ディレクトリごと不在』を区別できないため、show の『存在しない』表示がこれを要する」を書く。
- **理由:** show の runディレクトリ不在表示(PAGE-show-010)が依存する唯一の観測手段。gc 用の3メソッドは #6 の範囲なので宣言しない。

### 2. `FsRunStore` に `attempt_exists` を実装する — ADP-runstore-005

- **対象ファイル:** `crates/pulsen/src/adapter/run_store.rs`
- **spec:** `spec/domains/execution.md#runstore`、`spec/inventory/adapter.md`(ADP-runstore-005)
- **変更内容:** attempt ディレクトリ(`run_dir.as_path()`)の存在を `Result<bool, Io>` に畳む。既存の `marker_exists` と同じ形にし、機構の失敗(`NotFound` 以外の I/O エラー)を `Err(Io)` として値で返す。
- **理由:** ポートの契約を満たす唯一の実装。

### 3. 適合ケースとダブルを揃える — TC-port-run-store-022 / TC-port-run-store-023

- **対象ファイル:** `crates/pulsen-conformance/src/run_store.rs`、`crates/pulsen-conformance/src/doubles/run_store.rs`、`crates/pulsen-conformance/HOOKS.md`、`crates/pulsen/tests/conformance_run_store.rs`
- **spec:** `spec/testcases/ports/run-store.md`(`attempt_exists` の2行)
- **変更内容:**
  - 適合ケースを1行1関数で足す。TC-022 は `prepare_attempt` 済み・**ファイルを1つも書いていない**状態で `Ok(true)`、TC-023 は attemptディレクトリ不在で `Ok(false)`。`run_store_conformance!` のケース一覧に2件を追加する(`conformance_run_store.rs` 側の変更はスイート適用のマクロ経由なので不要)。
  - `ScriptedRunStore` に `with_attempt_exists` と `RunStoreCall::AttemptExists` を足す。台本を使い切ったらパニックする既存の流儀に揃える。
  - `HOOKS.md` の RunStore の節の見出し(「本スライス該当の21行」)と区分別の件数を 23行に更新する。
- **理由:** 適合ケースが無ければアダプターの実装が無主張になり、ダブルが無ければステップ11 の ShowTask ユースケーステストが1件も書けない。

### 4. `ListTasks` ユースケースを実装する — UC-task-004

- **対象ファイル:** `crates/pulsen/src/application/list_tasks.rs`(新規)、`crates/pulsen/src/application/mod.rs`
- **spec:** `spec/usecases/task.md#listtasksls`、`spec/pages/index.md#ls`、`spec/inventory/usecase.md`(UC-task-004)
- **変更内容:** 入力DTO(`status: Option<String>` / `state: Option<String>` / `all: bool`)・出力DTO(`rows` / `unreadable`)・エラー型を spec のとおりに置く。処理フローは (1) `list_active`、`all` なら `list_archived` も、(2) `TaskEntry::Corrupt` は `unreadable` へ・`TaskRecord::SnapshotUnreadable` は行に含めて印を立てる、(3) `status` と `state` を AND で適用(`all` は絞り込みではなく拡張なので、拡張後の集合に適用する)、(4) 該当0件も成功。`state` は `ExecutionStateKind::parse` を入力境界で1度だけ呼び、`StateKindError::Unknown { given, valid }` をエラー値として返す。`status` は検証しない。**`ExclusiveLock` を持たない。**
- **理由:** ls の判断のすべてがここに集まる。表示層は並べるだけにする。

### 5. `ShowTask` ユースケースを実装する — UC-task-005

- **対象ファイル:** `crates/pulsen/src/application/show_task.rs`(新規)、`crates/pulsen/src/application/mod.rs`
- **spec:** `spec/usecases/task.md#showtaskshow`、`spec/pages/index.md#show` と `#縮退状態の共通規則`(※4 / ※6 / ※8 / ※9)
- **変更内容:** 入力DTO(`task_id: String` → `TaskId::parse`)・出力DTO(spec の表の全フィールド)・エラー型(`InvalidTaskId` / `NotFound` / `Corrupt { path, message }` / `Read`)を置く。処理フローは (1) `find`、(2) `Intact` はスナップショット由来の項目(`defined_statuses` と `limits.retry`)を含めて構成し、`SnapshotUnreadable` は読める項目をすべて載せて `snapshot_error` を Some・`defined_statuses` を None・`limits.retry` を `Unknown` にする、(3) `attempt` は `RunStore::attempt_exists` で存在確認し、存在すれば `read_exit` で exit を補完する。**どちらの失敗も `Err` に昇格させず、「読めなかった」を値として出力DTOに載せる。** `limits.judge` / `limits.spawn` は config から常に埋める。`task_file_path` は `TaskFilePath::active` / `archived`(アーカイブ側かどうかは `TaskLookup` の判別で決まる)。**workspace_path の存在検証は行わない。ロックも取得しない。**
- **理由:** show の判断のすべてがここに集まる。縮退の場合分けを表示層に漏らすと、`match` の網羅が文言の分岐に化ける。

### 6. `ls` / `show` を CLI に足して結線する — PAGE-ls-001 / 002 / 003 / 004 / 005、PAGE-show-001

- **対象ファイル:** `crates/pulsen/src/cli/args.rs`、`crates/pulsen/src/cli/ls.rs`(新規)、`crates/pulsen/src/cli/show.rs`(新規)、`crates/pulsen/src/cli/mod.rs`
- **spec:** `spec/pages/index.md#ls`(構文とオプション3つ・併用時の合成規則)、`#show`(構文)、`#共通事項`
- **変更内容:** `Command::Ls(LsArgs { status: Option<String>, state: Option<String>, all: bool })` と `Command::Show(ShowArgs { task_id: String })` を足す。`cli/ls.rs` / `cli/show.rs` は `cli/add.rs` と同形で `wire::compose(home)` → ユースケース実行までを行う。`cli/mod.rs::run` にアームを足す。合成規則(`--status` と `--state` は AND、`--all` は対象集合の拡張)はステップ4 のユースケースが持つので、CLI は値を渡すだけ。
- **理由:** 構文と結線がこの2コマンドの入口。ここで `runtime.lock()` を渡さないことが AC-7 の担保になる。

### 7. `ls` の表示を書く — PAGE-ls-006 / 007 / 008 / 009 / 010

- **対象ファイル:** `crates/pulsen/src/cli/render/ls.rs`(新規)
- **spec:** `spec/pages/index.md#ls` の「機能」と「状態」、`spec/inventory/frontend.md`(PAGE-ls-006〜010)、`spec/scenario/monitoring.md#タスク一覧とステータス絞り込み` フロー2〜3
- **変更内容:** タスクID・ワークフロー名・リポジトリ・ブランチ・タスクステータス・実行状態・attempt_count・更新日時を並べる(PAGE-ls-006)。**タスクステータスと実行状態は常に併記**し、同名になり得ても区別できる形にする(PAGE-ls-007)。`Corrupt` はパスと読めない旨を、`SnapshotUnreadable` は行に印を付けて報告する(PAGE-ls-008)。アーカイブ済みの行にはその旨の印を付け、ブランチも出す(PAGE-ls-009)。該当0件は空である旨(PAGE-ls-010)。形式は adr.md ADR-001 に従う。
- **理由:** 一覧の読み取りやすさが「注意が必要なタスクを素早く見つける」というシナリオの目的そのもの。

### 8. `ls` / `show` の拒否の文言を書く — PAGE-ls-011、PAGE-show-007

- **対象ファイル:** `crates/pulsen/src/cli/render/ls.rs`、`crates/pulsen/src/cli/render/show.rs`(新規)
- **spec:** `spec/pages/index.md#ls` の「状態」、`#縮退状態の共通規則`(タスク不在・タスクファイル パース不能の show 列)
- **変更内容:** `--state` の不正値は `StateKindError::Unknown` の `valid` から有効な値6つを添えて非0(PAGE-ls-011)。show のタスク不在は「見つからない」ことを明確に示し、パース不能はパースエラーの内容とファイルパスを表示して非0(PAGE-show-007)。**いずれも書き込みを行わない**。既存の `problem(headline, details)` の形に揃える。
- **理由:** 「無言で空を返さない」「修復の入口を消さない」が spec の要求。

### 9. `show` の詳細表示と縮退の注記を書く — PAGE-show-002 / 003 / 004 / 005 / 006 / 008 / 009 / 010 / 011

- **対象ファイル:** `crates/pulsen/src/cli/render/show.rs`
- **spec:** `spec/pages/index.md#show` の「機能」と「状態」、`#縮退状態の共通規則`(※4 / ※6 / ※9)、`spec/scenario/monitoring.md#タスク詳細の確認` フロー2 と `#stoppedタスクの原因調査` フロー2
- **変更内容:**
  - 全属性(PAGE-show-002): ワークフロー名・対象・タスクステータス・実行状態・workspace_path・branch・3カウンタと適用上限の併記・現在attemptの番号/runディレクトリ/PID/kill同定子/starttime・`last_failure`・notified_at・更新日時。attempt「なし」/ workspace「未作成」/ 同定情報「未取得」の3つの縮退表示を出し分ける。上限の併記は `RetryLimitInfo` の3値を網羅 `match` で捌き、`NotApplicable` は併記なし・`Unknown` は導出不能である旨。
  - 定義済みステータス一覧とスナップショット保存先パス(PAGE-show-003)。保存先はタスクファイル自身のパス(ADR-015)。
  - 最新attemptの `stdout.log` / `stderr.log` / `exit` のパスと exit の値(PAGE-show-004)。
  - stopped の凍結要因(`StopReason` 4値)と `FailureNote`(PAGE-show-005)。
  - 成功時は 0(PAGE-show-006)。
  - アーカイブ済みはその旨と worktree 削除済みを明示して 0(PAGE-show-008、※4)。
  - スナップショット破損はタスクファイル由来の項目を表示し、スナップショット由来の項目に読めない旨を注記して 0(PAGE-show-009、※6)。
  - runディレクトリ・中身の不在は「存在しない」と注記して 0。存在確認・exit 読み取りの失敗も注記して表示を継続(PAGE-show-010)。
  - workspace_path は表示のみで存在検証を行わない(PAGE-show-011、※9)。
- **理由:** show の出力が worktree・runディレクトリ・タスクファイルへ直接向かうときの唯一の道標になる。

### 10. `ListTasks` のユースケーステストを書く — TC-task-list-tasks-003 / 004 / 005 / 006 / 007 / 008 / 010 / 012 / 013 / 014 / 015 / 016 / 017 / 018 / 020 / 021 / 024

- **対象ファイル:** `crates/pulsen/tests/list_tasks.rs`(新規)
- **spec:** `spec/testcases/task/list-tasks.md`(正常系・異常系・境界値・エッジケース)
- **変更内容:** `ScriptedTaskRepository` に対して分岐を網羅する。絞り込み3種と併用(003 / 004 / 005)、既定はアーカイブ非表示・`--all` での表示・`--all` と絞り込みの併用(006 / 007 / 008)、絞り込みで0件(010)、`--state` の不正値・6値すべて・大文字混じり・空文字(012 / 015 / 016 / 017)、走査の `ReadError::Io`(013 / 014)、未知のタスクステータス名で0件(018)、`SnapshotUnreadable` の行としての表示と絞り込み対象(020 / 021)、アーカイブ側の `Corrupt`(024)。
- **理由:** `ReadError::Io` と `SnapshotUnreadable` は実アダプターでは環境に依存して作りにくく、`--state` の6値の網羅はダブルのほうが安く確実に回る。

### 11. `ShowTask` のユースケーステストを書く — TC-task-show-task-004 / 005 / 006 / 007 / 008 / 011〜017 / 019〜034 / 037 / 038

- **対象ファイル:** `crates/pulsen/tests/show_task.rs`(新規)
- **spec:** `spec/testcases/task/show-task.md`(正常系・異常系・境界値・エッジケース)
- **変更内容:** `ScriptedTaskRepository` + `ScriptedRunStore` に対して分岐を網羅する。
  - launching の「未取得」(004)、exit の値(005)、stopped の2経路(ツール操作の失敗 / エージェント実行の失敗。006 / 007)、未通知の stopped(008)。
  - リトライ上限の3系統(`retries` 上書きあり / なし=組み込み 2 / Cleanup は常に 2)と Wait の併記なし(011 / 012 / 013 / 014)、judge / spawn 上限の config 由来(015)。
  - アーカイブ済みの解決と `state/archive/<id>.json` の保存先表示(016 / 017)。
  - タスク不在・`Corrupt`・`attempt_exists` / `read_exit` の失敗・不正なタスクID(019 / 020 / 021 / 022 / 023)。
  - タスクIDの境界値(空 / 65文字 / 64文字・1文字 / 不正文字・先頭ハイフン。024 / 025 / 026 / 027)。
  - `DegradedTask` の注記付き表示・`Unknown` と `NotApplicable` の区別・judge / spawn は通常どおり(028 / 029 / 030)。
  - runディレクトリ不在の2経路(gc 済み / launching 直後のクラッシュ。031 / 032)、worktree 手動削除でも存在検証しない(033)、状態ディレクトリ不在によるタスク不在(034)。
  - 無効化マーカーのみの attempt を現在attemptとして示す(037)、同期spawn失敗経路(`SpawnFailLimitExceeded` + `FailureNote::SpawnFail` + attempt なし。038)。
- **理由:** 縮退の組み合わせが多く、実アダプターで前提を作ると環境依存が入る。ダブルなら `attempt_exists` の `Err(Io)` と `read_exit` の `Corrupt` を確定的に与えられる。

### 12. 受け入れテストを書く — TC-task-list-tasks-001 / 002 / 009 / 011 / 019 / 022 / 023 / 025 / 026、TC-task-show-task-001 / 002 / 003 / 009 / 010 / 018 / 035 / 036

- **対象ファイル:** `crates/pulsen/tests/cli_ls.rs`(新規)、`crates/pulsen/tests/cli_show.rs`(新規)、`crates/pulsen/tests/common/mod.rs`
- **spec:** `spec/testcases/task/list-tasks.md` / `show-task.md`、`spec/pages/index.md#縮退状態の共通規則`
- **変更内容:** 実バイナリ + 一時ホームで検証する。
  - ls: 一覧の表示項目とタスクステータス・実行状態の併記(001 / 002)、0件の空表示(009)、config.yaml 不在の非0(011)、`Corrupt` 混在でも失敗しない報告(019)、`state/tasks/` 不在で空一覧(022)、`--all` で `state/archive/` 不在でも現役のみ表示(023)、**別プロセスがロックを保持したままでも 0**(025。既存の `examples/lock_holder` と `tests/common/lock.rs` を使う)、tick の書き込みと同時の読み取りで書きかけを観測しない(026)。
  - show: 未実行タスクの「未作成」「なし」(001)、実行履歴のあるタスクの attempt 番号・runディレクトリ・ログのパス(002)、running の PID・kill同定子・starttime(003)、定義済みステータス一覧(009)、保存先パス(010)、config.yaml 不在の非0(018)、ロック保持中でも 0(035)、tick と同時の読み取り(036)。
  - `tests/common/mod.rs` に「アーカイブ側へタスクファイルを置く」フィクスチャを足す(adr.md ADR-003)。
- **理由:** PAGE 行が主張するのは**表示**であり、綴りまで含めた確認は実バイナリの出力でしかできない。ロック非取得は「取らないこと」の外形的な確認が要る。

### 13. 記帳する — チェックリストの確定と spec 差分の提起

- **対象:** Issue #4 のコメント、チェックリストのチェック
- **変更内容:** 台帳の PASS 要件を満たしてテストが通った行にのみチェックを付ける。部分消化(`PAGE-ls-004` / `PAGE-show-008` / `TC-task-show-task-031`)は plan.md の表のとおり消化範囲と引き取り先を、実行しなかった手順書のケース(`monitoring.md` TC-05 / TC-06 / TC-09 手順4 / TC-10 / TC-15、`cleanup.md` TC-13〜15 / 17 / 23)は理由(#5 / #6 待ち)を残す。plan.md の「spec との差分として提起するもの」4点も同じコメントで提起する。
- **理由:** 完了条件が「実装をレビューで確認できた行にのみチェックを付ける。見送る行は理由をコメントに残す」と定めている。
