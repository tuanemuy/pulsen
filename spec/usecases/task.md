# ユースケース: Task

タスク帳簿に対する登録・照会・手動操作のユースケース。CLI(pages)の add / retry / set-status / ls / show に対応する。実行の観測を伴う操作(tick / abort / wrapper)は [execution.md](execution.md)。

共通事項:

- すべてのユースケースは実行前にグローバルホームを解決し(フラグ > `PULSEN_HOME` > 既定 `~/.pulsen/`)、`ConfigStore::load` でグローバル設定を読み込む。`NotFound` / `Invalid` / `Io` は即座に非0で終了する(pages ※1。状態は変更しない)
- 状態を変更するユースケース(RegisterTask / RetryTask / SetTaskStatus)は `ExclusiveLock::try_acquire` を取得して行う。`None`(競合)なら「別の操作が実行中」として非0で終了する(何も変更しない)。`Err(LockError::Failed)`(ロック機構自体の異常)も実行環境エラーとして非0で終了する。読み取り(ListTasks / ShowTask)はロックを取得しない
- CLI引数の文字列 → ドメイン型の変換(parse)は各ユースケースの入力境界で一度だけ行う

## RegisterTask(add)

### 概要

ワークフローと対象を検証し、スナップショットを埋め込んだタスクを pending で登録する。実行はしない(ADR-007)。

### 入力DTO

| フィールド | 型 | 必須 | バリデーション |
|---|---|---|---|
| `workflow` | `String` | 必須 | `WorkflowRef::parse`(パス区切り文字 / `.yaml` / `.yml` 判定) |
| `repo` | `String` | 必須 | 絶対パスへ正規化後 `RepoPath::parse` |
| `base` | `Option<String>` | 任意 | 指定時 `BranchName::parse`。省略時はリポジトリの HEAD から解決 |

### 出力DTO

| フィールド | 型 |
|---|---|
| `task_id` | `TaskId` |
| `workflow_name` | `WorkflowName`(WorkflowRef の表示名規則で決定) |
| `resolved_from` | `PathBuf`(解決したワークフロー定義のパス。名前指定時の確認表示用) |

### 処理フロー

1. ロック取得(`ExclusiveLock::try_acquire`)
2. `WorkflowRef::parse` → `WorkflowStore::load` で `LoadedWorkflow { parsed, resolved_from }` を得る(`NotFound` は解決を試みた絶対パスを添えてエラー。`resolved_from` が出力DTOの同名フィールドになる)
3. `WorkflowRef::display_name(declared_name)` で表示名を決定する
4. 対象の検証(`WorktreeManager`): `validate_repo` → `base` 省略時は `head_branch` で解決(`DetachedHead` / `EmptyRepository` は `--base` の明示を案内してエラー)→ `branch_exists` が false ならエラー
5. `RegistrationValidator::validate(definition, config)` → `WorkflowSnapshot`(エラーは全件まとめて表示。未定義エージェント参照時は `config.agents` の定義済み名一覧を添える)
6. `TaskIdGenerator::generate` → `Task::register(id, workflow_name, target, snapshot, now)`
7. `TaskRepository::create`(`Conflict` なら ID を再発行して1回だけ再試行。再衝突はエラー)
8. タスクIDを表示して 0 で終了

### トランザクション境界

- UnitOfWork: 不要(単一タスクの `create` のみ。検証はすべて読み取り)

### エラーケース

| 条件 | 種類 |
|---|---|
| ロック競合 | 実行環境(非0。登録前のためタスクは作られない) |
| ワークフロー解決失敗(`NotFound` / `Io`) | 入力(解決を試みたパスを表示) |
| ワークフローのパースエラー(`WorkflowParseError` 全種) | 入力(位置・原因を表示) |
| 表示名の決定失敗(`display_name` の `NameError`。パス指定でファイル名由来の名前が不正) | 入力 |
| リポジトリ不在・非リポジトリ / ブランチ不在 / HEAD 解決不能 | 入力 |
| 対象検証の git 操作自体の失敗(`TargetError::Failed`) | 実行環境 |
| 登録時検証エラー(`RegistrationError` 全種) | 入力(全件列挙) |
| ID 衝突の再発 / `create` の Io | 実行環境 |

いずれもタスクは作られない(pages: 検証エラー時に部分的な変更を残さない)。

## RetryTask(retry)

### 概要

凍結(stopped)したタスクのカウンタをリセットし pending に戻す(requirements §10)。

### 入力DTO

| フィールド | 型 | 必須 | バリデーション |
|---|---|---|---|
| `task_id` | `String` | 必須 | `TaskId::parse` |

### 出力DTO

| フィールド | 型 |
|---|---|
| `task_id` | `TaskId` |
| `task_status` | `StatusName`(再開されるタスクステータス) |
| `snapshot_warning` | `bool`(DegradedTask を受理した場合 true。修復が必要な旨を警告表示) |

### 処理フロー

1. ロック取得
2. `TaskRepository::find`。`NotFound` / `Corrupt` / `Archived`(アーカイブ済みは操作不可)はエラー
3. `Active(Intact(task))` → `task.retry(now)`。`Active(SnapshotUnreadable(degraded))` → `degraded.retry(now)`(受理し、pending に戻しても tick に拾われないためスナップショット修復が必要である旨を警告する。pages ※7)
4. `RetryError::NotStopped` は実行状態別の案内文言でエラー(failed =「放置すれば自動リトライ」、pending =「既に実行待ち」、completed =「判定済み。次のtickが遷移させる」、launching / running =「先に abort」)
5. `save` / `save_degraded` → 0 で終了

### トランザクション境界

- UnitOfWork: 不要(単一タスクの `save` のみ)

### エラーケース

| 条件 | 種類 |
|---|---|
| ロック競合 / タスク不在 / アーカイブ済み / ファイル破損(`Corrupt`) | 入力・状態(非0。書き込まない) |
| stopped 以外への retry(`NotStopped`) | ビジネスルール(状態別の案内付き) |

## SetTaskStatus(set-status)

### 概要

タスクステータスを手動遷移させ、カウンタをリセットして pending に戻す(requirements §10)。

### 入力DTO

| フィールド | 型 | 必須 | バリデーション |
|---|---|---|---|
| `task_id` | `String` | 必須 | `TaskId::parse` |
| `status` | `String` | 必須 | `StatusName::parse`(スナップショット所属の検証は遷移関数が行う) |

### 出力DTO

| フィールド | 型 |
|---|---|
| `task_id` | `TaskId` |
| `from` / `to` | `StatusName` |

### 処理フロー

1. ロック取得
2. `TaskRepository::find`。`NotFound` / `Corrupt` / `Archived` はエラー。`SnapshotUnreadable` は拒否(遷移先の検証にスナップショットが必要。pages ※7)
3. `task.set_status(status, now)`。`Active` は「先に abort せよ」、`UnknownStatus` は定義済みステータス一覧を添えてエラー
4. `save` → 0 で終了

### トランザクション境界

- UnitOfWork: 不要(単一タスクの `save` のみ)

### エラーケース

| 条件 | 種類 |
|---|---|
| ロック競合 / タスク不在 / アーカイブ済み / `Corrupt` / `SnapshotUnreadable` | 入力・状態 |
| launching / running への遷移(`SetStatusError::Active`) | ビジネスルール(「先に abort」) |
| スナップショットに無いステータス(`UnknownStatus`) | 入力(定義済み一覧を添える) |

## ListTasks(ls)

### 概要

タスクの一覧照会。絞り込み(タスクステータス・実行状態)と対象集合の拡張(アーカイブ含む)を合成する(pages ls)。

### 入力DTO

| フィールド | 型 | 必須 | バリデーション |
|---|---|---|---|
| `status` | `Option<String>` | 任意 | 検証しない(ユーザー定義語彙。未知の値は該当0件) |
| `state` | `Option<String>` | 任意 | `ExecutionStateKind::parse`(不正なら有効値一覧を添えて非0) |
| `all` | `bool` | 任意(既定 false) | |

### 出力DTO

| フィールド | 型 |
|---|---|
| `rows` | `Vec<TaskRow>` |
| `unreadable` | `Vec<UnreadableRow>`(修復の入口。pages ※5) |

- `TaskRow { task_id, workflow_name, repo: RepoPath, branch: Option<BranchName>, task_status, execution_state: ExecutionStateKind, attempt_count: u32, updated_at: Timestamp, archived: bool, snapshot_unreadable: bool }`
- `UnreadableRow { path: PathBuf, message: String }`(`Corrupt` のファイル)

### 処理フロー

1. `TaskRepository::list_active`(`all` なら `list_archived` も)
2. `Corrupt` は `unreadable` へ、`SnapshotUnreadable` は行に含めて `snapshot_unreadable` を立てる(絞り込みの対象にもなる — 実行状態・タスクステータスは読めている)
3. 絞り込み: `status` と `state` は AND。`all` は絞り込みではなく対象集合の拡張(拡張後に絞り込み適用)
4. 一覧表示(該当なしなら空である旨)。exit 0

### トランザクション境界

- UnitOfWork: 不要(読み取りのみ。ロックも取得しない)

### エラーケース

| 条件 | 種類 |
|---|---|
| `--state` が固定6値以外 | 入力(有効値一覧) |
| `list_active` / `list_archived` の Io | 実行環境(非0。走査自体ができない場合) |
| config 読み込み失敗 | 実行環境(※1) |

## ShowTask(show)

### 概要

タスク1件の詳細照会。現役 → アーカイブの順で解決し、実行メタデータ・スナップショット情報・凍結要因への参照を表示する(pages show)。

### 入力DTO

| フィールド | 型 | 必須 | バリデーション |
|---|---|---|---|
| `task_id` | `String` | 必須 | `TaskId::parse` |

### 出力DTO

| フィールド | 型 | 備考 |
|---|---|---|
| `task_id` / `workflow_name` / `target` | | |
| `task_status` / `execution_state` | `StatusName` / `ExecutionStateKind` | |
| `workspace` | `Option<Workspace>` | None は「未作成」。アーカイブ済みは「削除済み」注記 |
| `counters` | `RetryCounters` | |
| `limits` | `{ retry: RetryLimitInfo, judge: u32, spawn: u32 }` | `RetryLimitInfo = Applicable(u32) \| NotApplicable \| Unknown`。Intact は `applicable_retry_limit`(Wait は `NotApplicable` = 併記なし)、DegradedTask は `Unknown`(スナップショット破損で導出不能。`NotApplicable` と区別して表示)。judge / spawn は config の `judge_attempt_limit` / `spawn_fail_limit`(スナップショット非依存のため常に表示) |
| `attempt` | `Option<AttemptSummary>` | `{ number, run_dir, pid?, kill_ident?, starttime?, exit: Option<ExitCode>, stdout_log, stderr_log, run_dir_exists: bool }`。None は「なし」、process 未取込は「未取得」、runディレクトリ消失(gc後)は「存在しない」を明示 |
| `last_failure` | `Option<FailureNote>` | |
| `stop_info` | `Option<{ reason: StopReason, notified_at: Option<Timestamp> }>` | stopped のみ |
| `defined_statuses` | `Option<Vec<StatusName>>` | スナップショットの定義済みステータス一覧。DegradedTask では None |
| `snapshot_error` | `Option<String>` | DegradedTask のみ Some(スナップショットが読めない理由の注記。pages ※6) |
| `task_file_path` | `PathBuf` | `TaskFilePath::active / archived`(スナップショット保存先の表示。ADR-015) |
| `archived` | `bool` | |
| `updated_at` | `Timestamp` | |

### 処理フロー

1. `TaskRepository::find`(tasks → archive)。`NotFound` はエラー、`Corrupt` はパスとエラー内容を表示して非0
2. `Intact(task)` はスナップショット由来の項目(`defined_statuses`・`limits.retry`)を含めて構成する。`SnapshotUnreadable(degraded)` は読める項目をすべて表示し、`snapshot_error` を Some・`defined_statuses` を None・`limits.retry` を `Unknown` として 0 で表示する(注記付き表示。pages ※6)
3. `attempt` の実行メタデータは `RunStore::attempt_exists(run_dir)` で存在を確認し(false なら `run_dir_exists: false` =「存在しない」表示)、存在すれば `read_exit` で exit を補完する(いずれもエラーにしない)
4. 詳細を表示して 0

### トランザクション境界

- UnitOfWork: 不要(読み取りのみ。ロックも取得しない)

### エラーケース

| 条件 | 種類 |
|---|---|
| タスク不在 | 入力(無言で空を返さない) |
| タスクファイル破損(`Corrupt`) | 状態(パースエラー内容とパスを表示) |
| `attempt_exists` / `read_exit` の Io・`RunFileError` | 当該項目を読めない旨の注記付きで表示は継続(0) |
| config 読み込み失敗 | 実行環境(※1) |
