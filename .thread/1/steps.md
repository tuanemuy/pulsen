# 実装手順 — Issue #1

## 設計

### クレート構成とモジュール境界

ヘキサゴナルの層境界を**コンパイラに守らせる**ため、3クレートのワークスペースにする（根拠と代替案は adr.md ADR-019）。

```
Cargo.toml                       # [workspace] members / rust-version = "1.89"
crates/
  pulsen-domain/                 # ドメイン + ポート。[dependencies] は空
    src/lib.rs
    src/definition/              # ADR-017 の definition ドメイン
      mod.rs  name.rs  duration.rs  command.rs  template.rs
      agent.rs  config.rs  workflow.rs  reference.rs  snapshot.rs
      assembler.rs  validator.rs  port.rs
    src/task/                    # ADR-017 の task ドメイン
      mod.rs  id.rs  path.rs  branch.rs  time.rs  process.rs
      attempt.rs  counters.rs  failure.rs  state.rs  task.rs  degraded.rs  port.rs
    src/execution/               # ADR-017 の execution ドメイン（本スライスはポートのみ）
      mod.rs  port.rs
  pulsen-conformance/            # 適合スイート + テストダブル（deps: pulsen-domain）
    src/lib.rs  src/config_store.rs  src/workflow_store.rs  src/task_repository.rs
    src/clock.rs  src/task_id_generator.rs  src/exclusive_lock.rs  src/worktree_manager.rs
    src/doubles/                 # スクリプト式のポート実装（adr.md ADR-028）
      mod.rs  clock.rs  task_id.rs  lock.rs  worktree.rs  task_repository.rs  stores.rs
  pulsen/                        # bin + lib。adapter / application / cli / util
    src/main.rs                  # 薄い entrypoint（cli::run の呼び出しと exit code）
    src/lib.rs
    src/util/                    # 共通ユーティリティ（CLAUDE.md「個別に再実装しない」）
      mod.rs  atomic.rs  fsdir.rs
    src/adapter/
      mod.rs
      config_store.rs  workflow_store.rs  yaml.rs
      task_repository.rs  task_file.rs   # 永続化DTOと符号化・復号
      clock.rs  task_id.rs  lock.rs  worktree.rs
    src/application/
      mod.rs  home.rs            # グローバルホームのレイアウト（PulsenHome。adr.md ADR-031）
      register_task.rs           # UC-task-001
    src/cli/
      mod.rs  args.rs  add.rs  render.rs  exit.rs  wire.rs   # wire = 合成ルート
    examples/lock_holder.rs      # ロック保持プロセスのフィクスチャ（adr.md ADR-032）
    tests/                       # 適合テストの適用 + add の受け入れテスト
```

- **依存方向**: `pulsen` → `pulsen-domain`、`pulsen-conformance` → `pulsen-domain`。`pulsen-conformance` は `pulsen` の **`[dev-dependencies]`** にだけ入れる（本番バイナリに載せない）。逆向きの依存は Cargo が拒否する。
- **ポートの帰属**は spec/domains/index.md に従い、トレイトを各ドメインモジュールの `port.rs` に置く（`definition::port::{ConfigStore, WorkflowStore}` / `task::port::{TaskRepository, TaskIdGenerator, Clock}` / `execution::port::{WorktreeManager, ExclusiveLock}`）。
- **ポートのメソッドはスライス単位で足す**。本スライスの `WorktreeManager` は `validate_repo` / `head_branch` / `branch_exists` の3つだけを宣言する（`create` / `remove` は worktree スライスで追加）。未実装メソッドをトレイトに宣言してスタブを置くことはしない。
- **合成ルートは `cli::wire`** 1箇所。ホーム解決（`--home` > `PULSEN_HOME` > `~/.pulsen/`）・カレントディレクトリの取得・アダプターの構築と結線をここだけで行う（adr.md ADR-030・adr.md ADR-031）。`application` は `pulsen_domain` と std 以外を import しない。
- **lint**: workspace lints には全クレート共通のもの（`unsafe_code = "forbid"` 等）だけを置く。`clippy::wildcard_enum_match_arm` は `pulsen-domain` の `[lints.clippy]` にのみ設定する（adr.md ADR-029）。

### ドメインモデルへの影響（新規実装。既存コードは無い）

**definition ドメイン**（DOM-definition-001〜056 全件）

- 名前系 newtype 7種のうち、制約を持つ6種（`AgentName` / `ModelName` / `SkillName` / `Prompt` / `StatusName` / `WorkflowName`）は `parse(String) -> Result<Self, NameError>` のみで生成する（`Prompt` は非空のみ）。制約を持たない `InputText` は総関数 `new(String) -> Self` で生成する — `Err` になる経路のない `Result` を呼び出し側に畳ませないため。フィールドは全型とも非公開で、生成経路が1つである点は変わらない。
- `DurationSpec`（秒に正規化。`1m == 60s`）/ `TimeoutSpec = Limited | Unlimited`。
- `RawCommand` / `PlainCommand`: 文字列は単純空白分割（クォート解釈なし）、配列は要素そのまま（空文字列トークンは配列形式でのみ許容）、0トークンは `CommandError::Empty`。
- `CommandTemplate`（`tokens: Vec<Vec<Segment>>`、`Segment = Literal | Hole(Placeholder)`）と `SkillInputTemplate`。`parse` は `allowed` に無い名前を `UnknownPlaceholder`、閉じない `{`・空 `{}` を `MalformedBrace` にする。エスケープ機構は設けない。
- `expand` は1パス置換（置換結果を再走査しない）。`PlaceholderValues { input, model, workspace: PathBuf }` は `skill` を持たない。
- `RawAgentDefinition`（構造のみ）→ `parse` → `AgentDefinition`（内容検証済み）が**参照時検証の境界**。`render_input` / `build_command_line` を持つ。
- `GlobalConfig` は全キー任意 + 既定値（`judge_attempt_limit=3` / `judge_timeout=60s` / `spawn_fail_limit=3` / `run_retention=None` / `notify_cmd=None` / `agents=空`）。組み込み定数（リトライ2・timeout 1h）は config のキーにしない（ADR-014）。
- `WorkflowDefinition` はワークフロー名を持たない。不変条件（`initial ∈ statuses`・全 `AgentRun.next ∈ statuses`）を生成時に保証し、`status` / `effective_agent` / `effective_model` / `effective_timeout` / `effective_retry_limit` を持つ。循環・自己参照・到達不能は許容（ADR-010）。
- `WorkflowRef`（`Name | Path`）の `parse` はファイル存在に依存しない決定的規則。**区切り文字集合は定数として切り出し**、判定は集合を引数に取る純粋関数に閉じる（adr.md ADR-034）。`display_name(declared)` が表示名を決める。
- `WorkflowSnapshot` は `WorkflowDefinition` を包む newtype。生成経路は「登録時検証の通過」と「`rehydrate`（永続化からの再構築。config との再突き合わせなし）」の2つだけ。
- `WorkflowAssembler::assemble(RawWorkflowDoc) -> Result<ParsedWorkflow, WorkflowParseError>`（純粋）。`RawWorkflowDoc` / `RawStatusDoc` / `RawCommandDoc` は**ドメイン定義の DTO**（未パース文字列のみを持ち、外部クレートに依存しない）。`RawStatusDoc` が全キーを保持することで `ForbiddenKey` を検出できる。
- `RegistrationValidator::validate(def, &GlobalConfig) -> Result<WorkflowSnapshot, Vec<RegistrationError>>`（純粋）。全 AgentRun ステータスを走査してエラーを**全件集める**。
- ポート `ConfigStore::load` / `WorkflowStore::load` と `LoadedWorkflow` / `ConfigLoadError` / `WorkflowLoadError`。

**task ドメイン**（本スライス分）

- 値オブジェクト: `TaskId`（`[a-z0-9-]`・1〜64・先頭英数字）/ `RepoPath` / `WorktreePath`（絶対パス）/ `BranchName` / `Workspace` / `Timestamp` / `AttemptNumber`（1以上・`next`）/ `StateRoot` / `WorktreeRoot` / `RunDirPath::derive` / `TaskFilePath::active|archived` / `Pid` / `KillIdent` / `ProcessStartTime` / `StartTimeRecord` / `ProcessIdent` / `AttemptRef` / `RetryCounters` / `FailureNote` / `FailureKind` / `StopReason` / `ExecutionStateKind`（`parse` + `StateKindError`）/ `Target`。
- `Timestamp` は UTC 秒の Unix 秒を保持し、`from_unix_secs`（`Clock` 実装からの唯一の生成口）と `parse_rfc3339` / `to_rfc3339`（`YYYY-MM-DDTHH:MM:SSZ` 固定）を**ドメインに持つ**（adr.md ADR-020）。暦変換は days-from-civil / civil-from-days の純粋関数。`elapsed_since` は負を0に丸める。
- `ExecutionState` は6状態を付随データごと表現（`Launching { recorded_at }` / `Stopped { reason, notified_at }`）。`kind()` を持つ。
- `Task` はフィールド非公開 + 読み取りアクセサ。生成経路は `register`（新規）と `rehydrate`（永続化からの再構築。不変条件1を検証し `RehydrateError::StatusNotInSnapshot` を返す）だけ。**本スライスでは遷移関数を追加しない**。`rehydrate` は6状態・全 Optional フィールドを受け付ける公開経路であり、適合テストのフィクスチャ（TC-port-task-repository-006/013/014）はここから組む。
- `DegradedTask` は `Task` から `snapshot` を除いた全フィールド + `snapshot_error`。再構築コンストラクタと読み取りだけを持つ（`set_status` / `advance` / `record_launching` / `current_status_def` は**定義しない**）。TC-port-task-repository-009 の「タスク側フィールドを変更して `save_degraded`」は、`DegradedTask::abort` がスコープ外（DOM-task-057）のため**再構築コンストラクタで変更後の値を直接組み立てて**表現する。
- ポート `TaskRepository`（7メソッド + `TaskLookup` / `TaskRecord` / `TaskEntry` / `CreateError` / `SaveError` / `ReadError` / `ArchiveError`）、`TaskIdGenerator`、`Clock`。

**execution ドメイン**（本スライス分）

- `WorktreeManager`: `validate_repo` / `head_branch` / `branch_exists` と `TargetError`（5種）。
- `ExclusiveLock`: `try_acquire() -> Result<Option<Box<dyn LockGuard>>, LockError>`。`LockGuard` は**ドメインが定義するマーカートレイト**（`Drop` で解放されることだけを契約とし、実体はアダプターが持つ。adr.md ADR-022）。

### ユースケース / アプリケーションロジック

- `application::home::PulsenHome`: 解決済みのホームパスから `config_path` / `workflows_dir` / `state_root` / `worktree_root` / `lock_path` を導出する（adr.md ADR-031）。ホームの**解決**（`--home` > `PULSEN_HOME` > 既定）は `cli::wire` が行う。
- `application::register_task::RegisterTask`: ポート（`ConfigStore` は事前に読み込み済みの `GlobalConfig` として受け取る / `WorkflowStore` / `WorktreeManager` / `TaskIdGenerator` / `Clock` / `TaskRepository` / `ExclusiveLock`）を**ジェネリック引数**で受け取る。入力DTO `{ workflow: String, repo: PathBuf（絶対化済み）, base: Option<String> }` → 出力DTO `{ task_id, workflow_name, resolved_from }`。処理は spec の順序どおり:
  1. `ExclusiveLock::try_acquire`（`Ok(None)` = 「別の操作が実行中」、`Err` = 実行環境エラー。いずれも非0で何も変更しない）
  2. `WorkflowRef::parse` → `WorkflowStore::load`
  3. `WorkflowRef::display_name(parsed.declared_name)`
  4. `WorktreeManager::validate_repo` → `base` 省略時は `head_branch`、指定時は `branch_exists`
  5. `RegistrationValidator::validate`（エラーは全件返す）
  6. `TaskIdGenerator::generate` → `Task::register(id, workflow_name, target, snapshot, clock.now())`
  7. `TaskRepository::create`（`Conflict` は**ID再発行して1回だけ再試行**。再衝突は実行環境エラー）
- 入力文字列 → ドメイン型の変換はユースケースの入力境界で一度だけ行う。`--repo` の絶対化（`std::path::absolute`）と cwd の取得は `cli::wire` が行い、ユースケースには絶対パスだけが入る（adr.md ADR-030）。存在検証は `validate_repo` が行う。
- エラーは `RegisterTaskError` として値で返し、**文言の組み立ては CLI 層**に置く（ユースケースは原因の構造を返すだけ）。
- **ロック取得より前に config 読み込みがある**（PAGE-common-003 / ※1）。config.yaml 不在・パース不能で終わる場合、`state/` も `state/lock` も作られない。ロック取得まで進んだうえで検証エラーになる場合は `state/` と `state/lock` が残るが、これは ※3 が許容するツール管理領域の自動作成であり、縮退規則4「部分的な変更を残さない」の対象（= タスクの状態）ではない。TC-task-register-task-060 の期待「必要なディレクトリが自動作成され、登録が成功する」とも整合する。

### アダプター / 永続化 / 外部連携

**グローバルホームのレイアウト（本スライスで確定）**

```
<home>/
  config.yaml                      # グローバル設定（ツールは生成しない）
  workflows/<name>.yaml            # ワークフロー定義（ツールは生成しない）
  state/
    lock                           # 排他ロックのファイル（ホームごとに1つ）
    tasks/<task-id>.json           # 現役タスク
    archive/<task-id>.json         # アーカイブ済みタスク
    runs/<task-id>/attempt-<n>/    # runディレクトリ（本スライスでは作らない）
  worktrees/<task-id>              # タスクの worktree（本スライスでは作らない）
```

`state/` 配下は書き込み系が必要に応じて自動作成する（pages ※3）。ロックファイルは `state/lock`（ツール管理領域であり、タスクファイルの命名形式に合致しないため走査に混ざらない）。

**タスクファイルの直列化形式**（人間可読 JSON・スナップショット埋め込み。ADR-015 / adr.md ADR-025）

```jsonc
{
  "task_id": "20260811t091530-k3f9qa1b",
  "workflow_name": "implement",
  "target": { "repo": "/abs/path", "base_branch": "main" },
  "task_status": "queued",
  "execution": { "state": "pending" },            // launching は recorded_at、stopped は reason/notified_at を持つ
  "workspace": null,                               // { "path": ..., "branch": ... }
  "current_attempt": null,                         // { "number", "run_dir", "process": null | {...} }
  "counters": { "attempt_count": 0, "judge_attempt_count": 0, "spawn_fail_count": 0 },
  "last_failure": null,                            // { "kind", "message", "at" }
  "updated_at": "2026-08-11T09:15:30Z",            // RFC3339 / UTC / 秒精度
  "snapshot": {
    "default_agent": "shell", "default_model": null, "initial": "queued",
    "statuses": {
      "queued":  { "action": "agent_run", "input": { "prompt": "..." }, "agent": null, "model": null,
                   "timeout": null, "retries": null, "judge": null, "next": "implemented" },
      "waiting": { "action": "wait" },
      "done":    { "action": "cleanup" }
    }
  }
}
```

- 符号化・復号は `adapter::task_file` の DTO（serde derive）が担当し、ドメイン型は serde を持たない（adr.md ADR-020）。DTO → ドメインは必ず `parse` / `rehydrate` を通す。
- 復号の分類（adr.md ADR-025）:

  | 状態 | 分類 |
  |---|---|
  | ファイル全体が JSON として不正 | `Corrupt { path, message }` |
  | 有効な JSON だが、タスク側フィールドの型・値制約・未知キーが破れている | `Corrupt` |
  | 有効な JSON で、タスク側フィールドは読めるが、`snapshot` の値がスナップショットとして解釈できない（型不一致・必須キー欠落・フィールド不在・`initial ∉ statuses`・`next ∉ statuses`・`task_status ∉ statuses`） | `SnapshotUnreadable(DegradedTask)` |
  | 不変条件2〜4（状態間整合）の破れ | 検証しない（`Intact`） |

  「snapshot だけが壊れている」フィクスチャは `"snapshot": "壊れた"` / `{"initial": 123}` / `snapshot` キーの削除 / `task_status` の差し替えで作る（JSON 構文自体を壊すとファイル全体が `Corrupt` になるため）。
- `snapshot` フィールドは復号時に `Box<serde_json::value::RawValue>` として**生のまま保持**し、`save_degraded` は既存ファイルを読み直してその生バイト列を書き戻す（破損スナップショットの温存）。DTO の `Option<Box<RawValue>>` には `#[serde(skip_serializing_if = "Option::is_none")]` を付け、**キーの不在を不在のまま**書き戻す（既定の直列化ではキー削除が `"snapshot": null` に化け、「元の内容のまま温存する」契約から外れる）。
- タスク側の未知キーは拒否（`deny_unknown_fields`）。書き込みは常に全体をアトミック置換する。

**アダプター一覧**

| アダプター | 実装方針 |
|---|---|
| `FsConfigStore` | `config.yaml` を読み、YAML → `Value` 化（構文エラー・重複キーはここで `Invalid`）→ 手書きスキーマ走査で `GlobalConfig`。未知キーは `Invalid`。空ファイル・null は全デフォルトで `Ok`。テンプレート内容は検証しない。キャッシュしない |
| `FsWorkflowStore` | `new(workflows_dir, base_dir)`。`Name(n)` → `<workflows_dir>/<n>.yaml` 固定（`.yml` フォールバックなし）、`Path(p)` → 相対なら `base_dir` で絶対化（adr.md ADR-030）。YAML → `Value` → `RawWorkflowDoc`（構文エラー・重複キー = `YamlSyntax`、スキーマ外キー = `UnknownKey`）→ `WorkflowAssembler::assemble`。`resolved_from` は絶対パス |
| `FsTaskRepository` | 上記 JSON 形式。`create` は現役・アーカイブ横断の存在確認（**デコード可否によらない**）→ `Conflict`。`save` / `save_degraded` は現役に無ければ `NotFound`。`archive` は `rename` による移動（移動先ディレクトリ自動作成）。走査は `<task-id>.json` 形式のエントリのみ。ディレクトリ不在は空結果 |
| `SystemClock` | `SystemTime::now()` の `UNIX_EPOCH` からの経過秒 → `Timestamp`（サブ秒切り捨て）。`duration_since` の `Err` は epoch 前の負の秒に写す（`unwrap` しない。adr.md ADR-036） |
| `DefaultTaskIdGenerator` | `<UTC yyyymmdd 't' hhmmss>-<base36 8桁乱数>`（24文字。桁数の根拠は adr.md ADR-026）。**時刻は構築時に受け取った `Clock` から取り**、時刻成分は `Timestamp::to_rfc3339()` の出力から導出する（暦計算をアダプターで再実装しない）。エントロピーは `new(clock) -> Result<Self, _>` で一度だけ取り、以降は内部 PRNG（adr.md ADR-036）。`TaskId` 制約を常に満たす |
| `FileExclusiveLock` | `state/lock` を開き `File::try_lock()`。`WouldBlock` → `Ok(None)`、`Error(io)` と open 失敗 → `Err(Failed)`。`LockGuard` 実装が `File` を保持し Drop で解放（adr.md ADR-022） |
| `GitCliWorktreeManager` | `new(git_program: PathBuf)` で git 実行ファイルのパスを注入（合成ルートが既定 `"git"` を渡す）。`git -C <repo> ...` へシェルアウト（adr.md ADR-024）。判定は下表に固定する |

`GitCliWorktreeManager` の判定表（adr.md ADR-024。空リポジトリが `symbolic-ref` で exit 0 を返す実測、およびメタデータ破損が一律 exit 128 になる実測に基づく）:

- `validate_repo`: パス不在 → `NotFound` / **git の起動自体に失敗 → `Failed`** / 起動できて `rev-parse --show-toplevel` が非0 → `NotARepository`（メタデータ破損もここに落ちる）/ exit 0 → `Ok`（サブディレクトリ指定も受理）
- `head_branch`（どちらかの**起動**に失敗したら `Failed`）:

  | `symbolic-ref --short HEAD` | `rev-parse --verify --quiet HEAD` | 結果 |
  |---|---|---|
  | exit 0 | exit 0 | 出力を `BranchName::parse` に通す。成功 → `Ok(ブランチ名)` / 失敗 → `Err(Failed)`（git では有効でもドメインの実用サブセットに乗らない名前。`unwrap` しない） |
  | exit 0 | 非0 | `Err(EmptyRepository)` |
  | 非0 | exit 0 | `Err(DetachedHead)` |
  | 非0 | 非0 | `Err(Failed)` |

- `branch_exists`: `show-ref --verify --quiet refs/heads/<b>` の exit 0 → `true` / exit 1 → `false` / それ以外と起動失敗 → `Failed`
- `TargetError::Failed` の到達経路は「git を起動できない」1本。適合テストのハーネスは**存在しないパスで構築した2つ目の `GitCliWorktreeManager`** を `failing_manager()` として返し、3メソッドとも `Failed` に落とす（本番アダプターはイミュータブルなまま。adr.md ADR-027）
- 共通: 起動する git の環境から `GIT_DIR` / `GIT_WORK_TREE` / `GIT_INDEX_FILE` を除去する。ユーザーのグローバル設定は本番では尊重する（テストフィクスチャ側の固定は adr.md ADR-033）

**共通ユーティリティ（`pulsen::util`）**

- `atomic::write_atomic(path, bytes)`: 同一ディレクトリに一時ファイル作成 → 書き込み → `sync_all` → `rename`（既存を置換）→ 可能なら親ディレクトリを fsync。失敗時に一時ファイルを残さない。**アトミック置換の実装はここ1箇所**（TaskRepository・後続の RunStore が共用）。
- `atomic::rename_atomic(from, to)`: アーカイブ移動用。
- `fsdir::ensure_dir(path)`: 親含む作成（冪等）。

### ポート適合テストの土台

`pulsen-conformance` クレートは、ポートのトレイトだけに依存する**再利用可能なスイート**を提供する（adr.md ADR-027）。

- ポートごとに `Harness` トレイトを定義する。**フックは破損・状況の「意味」だけを受け取り**、実現方法（生 JSON の配置・権限操作・プロセス起動）はハーネス実装の内側に閉じる。生の文字列を受け渡す `put_raw` / `read_raw` は置かない。
- **フックの一覧は spec の前提条件から導く**。ステップ9の成果物に「spec/testcases/ports/\*.md の125行 × フック」の対応表を含め、各行を「ポートのメソッドだけで組める / このフックで組める / spec が明示するスキップ可」のいずれかで埋める。埋まらない行が残ったらフックを足す（adr.md ADR-027）。

  ```rust
  pub trait TaskRepositoryHarness {
      type Repo: TaskRepository;
      fn repo(&self) -> &Self::Repo;

      /// 破損・配置のフィクスチャ。提供できない実装は None を返しスキップされる
      fn corrupt_whole_record(&self, _area: Area, _id: &TaskId) -> Option<()> { None }   // TC-020 / 029
      fn break_task_field(&self, _area: Area, _id: &TaskId) -> Option<()> { None }       // TC-021
      fn corrupt_snapshot(&self, _area: Area, _id: &TaskId) -> Option<()> { None }       // TC-022 / 028
      fn drop_snapshot_field(&self, _area: Area, _id: &TaskId) -> Option<()> { None }    // TC-023
      fn set_task_status_outside_snapshot(&self, _area: Area, _id: &TaskId, _s: &str) -> Option<()> { None } // TC-024
      fn break_snapshot_invariant(&self, _area: Area, _id: &TaskId) -> Option<()> { None } // TC-025
      fn place_in_both_areas(&self, _id: &TaskId) -> Option<()> { None }                 // TC-018
      fn put_unnamed_entry(&self, _area: Area) -> Option<()> { None }                    // TC-030
      fn record_bytes(&self, _area: Area, _id: &TaskId) -> Option<Vec<u8>> { None }      // TC-004
      fn snapshot_bytes(&self, _area: Area, _id: &TaskId) -> Option<Vec<u8>> { None }    // TC-009

      /// 「再現できるアダプター環境に限る」ケース。
      /// 制限が実際に効いたことを確認してから Some を返す（効かなければ復元して None）
      fn make_unreadable(&self, _area: Area) -> Option<Restore> { None }
      fn make_unwritable(&self, _area: Area) -> Option<Restore> { None }

      /// 原子性の観測面（TC-042〜044）だけが使う。Sync 境界をスイート全体へ伝播させない
      fn concurrent_repo(&self) -> Option<&(dyn TaskRepository + Sync)> { None }
  }
  ```

  他のポートも同じ方針で、spec の前提条件と1対1に対応させる。**対象アクセサ**（関連型と共有参照の取得口）は全ハーネス共通の形にする — `TaskRepositoryHarness { type Repo; fn repo(&self) -> &Self::Repo }` / `ConfigStoreHarness { type Store; fn store(&self) -> &Self::Store }` / `WorkflowStoreHarness { type Store; fn store(&self) -> &Self::Store }` / `ExclusiveLockHarness { type Lock; fn lock(&self) -> &Self::Lock }` / `WorktreeManagerHarness { type Manager; fn manager(&self) -> &Self::Manager }` / `ClockHarness { type Clock; fn clock(&self) -> &Self::Clock }`。対象は共有参照でしか渡らないため、**構築時に注入した値がイミュータブルなアダプターを「壊す」型のフックは置けない**。壊れた状態が要るケースは `concurrent_repo` / `failing_manager` と同じ「別ハンドルを返すスキップ可能フック」にする（adr.md ADR-027）。

  | ハーネス | フック |
  |---|---|
  | `ConfigStoreHarness` | `put_config(text)` / `remove_config()`（TC-013）/ `home_path()`（`NotFound` が含む解決後ホームパスの期待値）/ `make_unreadable()` |
  | `WorkflowStoreHarness` | `put_named(name, text)`（上書きも兼ねる。TC-031）/ `put_named_with_ext(name, ext, text)`（TC-002）/ `expected_path_for_name(name)`（`attempted` の期待値。TC-002/003/006）/ `put_at_absolute(text) -> PathBuf`（TC-004）/ `put_at_relative(text) -> (相対, 絶対)`（TC-005）/ `missing_absolute_path()`（TC-006）/ `make_unreadable(name)` |
  | `ExclusiveLockHarness` | `hold_from_other_process() -> Option<Holder>` / `kill_holder(h)` / `release_holder(h)` / `try_acquire_from_other_process() -> Option<bool>`（TC-004/005。`None` = 別プロセス未対応でスキップ、`Some(false)` = 競合）/ `separate_home()` / `break_lock_location()` |
  | `WorktreeManagerHarness` | `repo_with_commit()` / `repo_without_commit()` / `detached_repo()` / `non_repo_dir()` / `missing_path()`（TC-002）/ `head_branch_name()` / `failing_manager() -> Option<&Self::Manager>`（TC-009。**3メソッドとも `Err(Failed)` を返す実装**を返す契約。git ハーネスは存在しないパスで構築した2つ目の manager を保持するだけでよい） |
  | `ClockHarness` | `observe_wall_clock()`（TC-003）/ `advance()`（TC-004。「時刻が確実に前進した状態にする」。実時間を待つ実装を含む）/ `rewind()`（TC-005）。RFC3339 往復は `Timestamp` 自身が持つためフック不要（adr.md ADR-020） |

- 各ケースは `pub fn case_<spec-id>_<名前>(h: &impl Harness)` として1関数1ケースで書き、`spec/testcases/ports/*.md` の行と1:1に対応させる。
- `#[macro_export] macro_rules! task_repository_conformance!($setup:expr)` が各ケースに対する `#[test]` 関数を生成する。アダプター側は1行で全ケースを適用でき、テスト結果もケース単位で出る。
- フックが `None` を返したケースはスキップし、スキップした旨と理由を出力する。**スキップで終わった行はチェックリストにチェックを付けず、理由を Issue のコメントに残す**（plan.md「リスクと注意点」）。
- **権限操作系のフック（`make_unreadable` / `make_unwritable`）は、制限が実際に効いたことを確認してから `Some` を返す**（掛ける → 読み書きを試す → 通ってしまったら復元して `None`）。`chmod 000` は root では効かないため、確認せずに `Some` を返すと `Err(Io)` を期待するケースがスキップに落ちずに FAIL する（adr.md ADR-027）。root 実行・Windows・特殊なファイルシステムをこの1つの規則で吸収する。
- 後続スライスの in-memory アダプターや別プラットフォーム実装も、フックを実装してマクロを呼ぶだけで検証できる。`concurrent_repo` を除く全ケースが `Sync` を要求しないため、`RefCell` ベースの実装にもそのまま適用できる。

### ポートのテストダブル

`pulsen-conformance::doubles` に**スクリプト式**のポート実装を置く（adr.md ADR-028）。範囲は「add の異常系検証に必要なポートに限る」。

| ダブル | 与えるもの |
|---|---|
| `ScriptedTaskIdGenerator` | 発行する `TaskId` の列 |
| `ScriptedExclusiveLock` | `Ok(Some)` / `Ok(None)` / `Err(Failed)` |
| `ScriptedWorktreeManager` | `validate_repo` / `head_branch` / `branch_exists` の結果（`TargetError` 5分岐を含む） |
| `ScriptedTaskRepository` | `create` の結果列（`Ok` / `Conflict` / `Io`）と、作成された `Task` の記録 |
| `FixedClock` | 固定 `Timestamp` |
| `ScriptedWorkflowStore` | `load` の結果 |

`ScriptedConfigStore` は置かない — `RegisterTask` は config を読み込み済みの `GlobalConfig` として受け取り、`ConfigStore` ポートを引数に取らないため、ダブルを使う場所が無い（adr.md ADR-028）。

汎用の in-memory ストア（適合テスト全件を通す実装）は作らない — 後続スライスの範囲。

### CLI / プレゼンテーション

- `clap`（derive）で `pulsen [--home <dir>] <subcommand>`。本スライスのサブコマンドは `add` のみ（`--workflow` 必須 / `--repo` 必須 / `--base` 任意）。
- `cli::wire` が合成ルート: ホーム解決（`--home` > `PULSEN_HOME` > `std::env::home_dir()` + `.pulsen`）→ `PulsenHome` 導出 → `ConfigStore::load` → 各アダプター構築（`FsWorkflowStore` には `std::env::current_dir()` を `base_dir` として渡す）→ `--repo` の絶対化 → `RegisterTask` の実行。
- 出力は人間可読テキスト。成功は stdout、エラーは stderr。exit code は 0（成功）/ 1（入力・状態・実行環境エラー）。引数の使い方の誤りは clap 既定の 2。機械可読形式は提供しない。config.yaml / ワークフローYAML の生成サブコマンドも設けない。
- `cli::render` が spec の案内文言を組み立てる。少なくとも次を満たす:
  - config.yaml 不在: 「グローバルホームが未初期化」+ **解決後のホームパス** + config.yaml の作成が必要である旨
  - config.yaml パース不能: エラー位置（行・列）
  - ワークフロー名の解決失敗: **解決を試みた絶対パス**
  - 未定義エージェント: **config.yaml に定義済みのエージェント名一覧**
  - detached HEAD / 空リポジトリ: `--base` の明示指定の案内
  - 登録時検証エラー: **全件列挙**
  - ロック競合: 「別の操作が実行中」
  - 成功: タスクID + 解決したワークフロー名 + 解決先パス

## 実装ステップ

依存方向の順（domain → port → util → 適合スイート/adapter → doubles → usecase → cli → 受け入れテスト）に並べる。各ステップは単体でレビュー可能で、テストが通る状態で終える。適合ケースは**対応するアダプターと同じステップ**に置き、「契約を書く → 実装が通す」でステップが閉じるようにする。

各ステップの完了時に、そのステップで形が確定した `.adr/` エントリの Status を `Proposed` から `Accepted` へ更新する（ADR とステップの対応は adr.md ADR-035 の表）。

### 1. ワークスペースと開発環境の初期化

- **対象ファイル:** `Cargo.toml`、`crates/pulsen-domain/Cargo.toml`、`crates/pulsen-conformance/Cargo.toml`、`crates/pulsen/Cargo.toml`、`rustfmt.toml`、`flake.nix`、`.gitignore`、`.adr/1-domain-crate-workspace.md`〜`.adr/1-infallible-ports-absorb-failure-at-construction.md`
- **変更内容:** 3クレートのワークスペースを作る（edition 2024、`resolver = "3"`、`rust-version = "1.89"`）。`pulsen-domain` の `[dependencies]` は空にする。`pulsen` の `[dependencies]` は `clap` / `serde` / `serde_json`（`raw_value`）/ `serde_yaml_ng` / `getrandom` / `tempfile`、`[dev-dependencies]` に `pulsen-conformance`（adr.md ADR-023）。`pulsen` に bin `pulsen`（`src/main.rs`）と lib を置く。lint は workspace に `unsafe_code = "forbid"` 等の共通分のみ、`clippy::wildcard_enum_match_arm` は `pulsen-domain` にのみ設定する（adr.md ADR-029）。`flake.nix` の devShell に `git` を追加する。adr.md の各エントリを `.adr/019`〜`.adr/036` として **Status: Proposed で**起票する（adr.md ADR-035。ADR-019・022・023・029・035 はこのステップで `Accepted` に上げる）。
- **理由:** クレート境界で依存方向を強制するのがこのプロジェクトのアーキテクチャ方針を守る最も確実な手段であり、以降の全ステップの前提になる。後続10スライスを縛る判断は正本（`.adr/`）に置く。

### 2. definition ドメイン: 値オブジェクト基盤

- **対象ファイル:** `crates/pulsen-domain/src/definition/{name,duration,command,template}.rs`
- **変更内容:** 名前系 newtype 7種 + `NameError`、`DurationSpec` + `DurationError`、`TimeoutSpec`、`RawCommand` / `PlainCommand` + `CommandError`、`AgentInput`、`Placeholder`、`CommandTemplate` / `SkillInputTemplate` / `CommandLine` / `PlaceholderValues` + `TemplateError` / `ExpansionError`、`placeholders` / `expand` / `render`。ユニットテストで全分岐（空・前後空白・`0s`・単位なし・連続空白・空配列・空文字列トークン・未知プレースホルダ・閉じない `{`・空 `{}`・1パス展開・値の欠落）を網羅する。
- **理由:** 以降のすべての定義構造がこの層の `parse` に乗る。ここで「境界で一度だけ検証する」規約を確定する。
- **消化するチェックリスト:** DOM-definition-001〜023, 030, 031

### 3. definition ドメイン: 定義構造と実効値・表示名の規則

- **対象ファイル:** `crates/pulsen-domain/src/definition/{agent,config,workflow,reference,snapshot}.rs`
- **変更内容:** `RawAgentDefinition` / `AgentDefinition` / `AgentDefError`（`parse` / `render_input` / `build_command_line`）、`GlobalConfig`（既定値つき）、`WorkflowDefinition`（不変条件を生成時に保証）+ `StatusDefinition` + `effective_*` 4種、`WorkflowRef`（`parse` / `display_name`。区切り文字集合を定数化し判定は集合を引数に取る純粋関数に閉じる。adr.md ADR-034）、`WorkflowSnapshot`（`rehydrate` / 委譲メソッド / `definition`）。ユニットテストで実効値の優先順位、`display_name` の4規則、区切り文字集合 `&['/']` と `&['/', '\\']` の両方、`{model}` を参照しないテンプレートに model を渡しても許容されることを検証する。
- **理由:** スナップショットの中身と、登録時検証・実行時展開の両方が使う「実効値」の解決規則をここで固定する。ドメインのユニットテストがプラットフォームで結果を変えないようにする。
- **消化するチェックリスト:** DOM-definition-024〜029, 032〜039, 041〜044

### 4. definition ドメイン: 組み立てと登録時検証

- **対象ファイル:** `crates/pulsen-domain/src/definition/{assembler,validator}.rs`
- **変更内容:** `RawWorkflowDoc` / `RawStatusDoc` / `RawCommandDoc` / `ParsedWorkflow` と `WorkflowAssembler::assemble`、`WorkflowParseError`（12種の定義。うち `YamlSyntax` / `UnknownKey` はアダプターが生成する）、`RegistrationValidator::validate` と `RegistrationError`（5種）。ユニットテストで `ForbiddenKey`〜`InvalidValue` の各分岐、循環・自己参照・到達不能・終端なしの受理、検証エラーの全件収集を検証する。
- **理由:** ADR-013 の厳格スキーマと ADR-010 の許容範囲の境目を、I/O なしの純粋関数として確定する。
- **消化するチェックリスト:** DOM-definition-040, 045〜050, 056

### 5. task ドメイン: 値オブジェクト群と `Timestamp`

- **対象ファイル:** `crates/pulsen-domain/src/task/{id,path,branch,time,process,attempt,counters,failure,state}.rs`
- **変更内容:** `TaskId` + `TaskIdError`、`RepoPath` / `WorktreePath`、`BranchName`、`Workspace`、`Timestamp`（Unix 秒・全順序・`elapsed_since` は負を0に丸める・`from_unix_secs(i64) -> Result<Self, TimestampError>`（範囲検証つき。`Clock` 実装が `Timestamp` を作る唯一の口）・`parse_rfc3339` / `to_rfc3339`。日数と秒への分解は `div_euclid` / `rem_euclid`、表現可能範囲は 0001-01-01〜9999-12-31 に閉じる。adr.md ADR-020）、`AttemptNumber`、`StateRoot` / `WorktreeRoot`、`RunDirPath::derive`、`TaskFilePath::active|archived`、`Pid` / `KillIdent` / `ProcessStartTime` / `StartTimeRecord` / `ProcessIdent`、`AttemptRef`、`RetryCounters`、`FailureNote` / `FailureKind`、`StopReason`、`ExecutionStateKind` + `StateKindError`、`Target`。ユニットテストでパス導出、ブランチ名の安全性（`TaskId` 制約の帰結）、RFC3339 の往復・うるう年・不正日付（`2026-02-30`・`2026-13-01`）・形式外（オフセット付き・サブ秒）の拒否、epoch 前（負の Unix 秒）の往復、範囲の両端（0001-01-01T00:00:00Z・9999-12-31T23:59:59Z）と範囲外の拒否（`from_unix_secs` / `parse_rfc3339` の両経路）を検証する。
- **理由:** 帳簿の語彙とファイルレイアウトの決定的導出をドメイン側に置き、アダプターにレイアウト知識を漏らさないため。`Timestamp` の直列化表現をドメインに置くことで、適合スイートが Clock TC-002 を書けるようにする。
- **消化するチェックリスト:** DOM-task-001〜027, 078

### 6. task ドメイン: Task / DegradedTask

- **対象ファイル:** `crates/pulsen-domain/src/task/{task,degraded}.rs`
- **変更内容:** `ExecutionState`（6状態 + `kind`）、`Task`（非公開フィールド + 読み取りアクセサ）、`Task::register`、`Task::rehydrate` + `RehydrateError`、`DegradedTask`（再構築 + 読み取り）。`register` の事後条件（`task_status = initial` / pending / 全 None / カウンタ全0）と `rehydrate` の不変条件1検証をユニットテストで検証する。`rehydrate` と `DegradedTask` の再構築コンストラクタは、6状態・全 Optional フィールドを受け付ける公開経路にする（適合テストのフィクスチャ供給経路）。遷移関数は追加しない。
- **理由:** 登録と永続化からの再構築という2つの生成経路だけを先に確定し、後続スライスの遷移関数が乗る土台にする。
- **消化するチェックリスト:** DOM-task-028〜032, 056, 060, 079

### 7. ポートの定義

- **対象ファイル:** `crates/pulsen-domain/src/definition/port.rs`、`crates/pulsen-domain/src/task/port.rs`、`crates/pulsen-domain/src/execution/port.rs`
- **変更内容:** `ConfigStore` / `WorkflowStore` + `LoadedWorkflow` / `ConfigLoadError` / `WorkflowLoadError`、`TaskRepository`（7メソッド）+ `TaskLookup` / `TaskRecord` / `TaskEntry` / `CreateError` / `SaveError` / `ReadError` / `ArchiveError`、`TaskIdGenerator`、`Clock`、`WorktreeManager`（3メソッド）+ `TargetError`、`ExclusiveLock` + `LockError` + `LockGuard`（マーカートレイト）。契約はドキュメンテーションコメントに書く。レビューでは spec/domains/{definition,task,execution}.md のポート表と行単位で突き合わせる（AC-7）。
- **理由:** 適合テストとアダプターの両方がこの型を参照する。ここが後続スライスとの互換の要になるため、spec の表をそのまま写す。
- **消化するチェックリスト:** DOM-definition-051〜055、DOM-task-062〜077、DOM-execution-059, 060, 061, 065, 068, 069, 070

### 8. 共通ユーティリティ（アトミック置換・ディレクトリ）

- **対象ファイル:** `crates/pulsen/src/util/{atomic,fsdir}.rs`
- **変更内容:** `write_atomic` / `rename_atomic`（一時ファイル → fsync → rename、失敗時に残骸を残さない）、`ensure_dir`。ユニットテストで置換の原子性（読み手が常に旧内容か新内容のどちらかを見る）と一時ファイルの後始末を検証する。
- **理由:** CLAUDE.md がアトミック性・排他の個別再実装を禁じている。ここを1箇所に集約してからアダプターを書く。
- **消化するチェックリスト:** UC-flow-007（アトミック更新の実装基盤）

### 9. ポート適合テストの枠組み

- **対象ファイル:** `crates/pulsen-conformance/src/lib.rs`（`Harness` トレイト群・マクロ生成の骨格・スキップ報告）、`crates/pulsen-conformance/HOOKS.md`（対応表）
- **変更内容:** ポートごとの `Harness` トレイト（対象アクセサは `type X` + `fn x(&self) -> &Self::X` に統一。フックは意図レベルで、既定実装は `None`。「対象を壊す」型ではなく `concurrent_repo` / `failing_manager` のように**別ハンドルを返す**型にする。権限操作系フックは制限が効いたことを確認してから `Some` を返す。adr.md ADR-027）、`#[test]` を生成するマクロの仕組み、スキップした理由の出力。**成果物として「spec/testcases/ports/\*.md の125行 × フック」の対応表を作り**、各行を「ポートのメソッドだけで組める / このフックで組める / spec が明示するスキップ可」のいずれかで埋める（埋まらない行が残ったらフックを足す。adr.md ADR-027）。原子性の3ケース（TC-port-task-repository-042〜044）は `concurrent_repo` フック経由でのみ書き、`Sync` 境界がスイートの他の41ケースへ伝播しないことをコンパイルで確かめる。ケース関数はこの時点では置かない（ステップ10〜14で、対応するアダプターと同時に足す）。
- **理由:** ハーネスの形（フックの粒度）が後続スライスの in-memory 実装の適用可否を決めるため、ケースの物量から切り離して単独でレビューできるようにする。対応表を成果物に含めることで、フックが spec 由来であることが枠組み単体でレビューできる。
- **消化するチェックリスト:** TC-port-* 全件の枠組み（ケースと PASS は 10〜14）

### 10. ConfigStore: 適合ケース24件とアダプター

- **対象ファイル:** `crates/pulsen-conformance/src/config_store.rs`、`crates/pulsen/src/adapter/{yaml,config_store}.rs`、`crates/pulsen/tests/conformance_config_store.rs`
- **変更内容:** YAML テキスト → `Value`（構文エラー・重複キーの検出とエラー位置の取得）を `yaml.rs` に集約する。config-store の適合ケース24件を書き、`FsConfigStore`（手書きスキーマ走査・未知キーは `Invalid`・空ファイル/null は全デフォルトで `Ok`・テンプレート内容は検証しない・キャッシュしない）で全件 PASS させる。
- **理由:** 「構造は読み込み時・内容は参照時」という二層検証（ADR-013）の実装位置をここで確定する。YAML 基盤を先に置くことで次のステップが薄くなる。
- **消化するチェックリスト:** ADP-config-001、TC-port-config-store-001〜024

### 11. WorkflowStore: 適合ケース31件とアダプター

- **対象ファイル:** `crates/pulsen-conformance/src/workflow_store.rs`、`crates/pulsen/src/adapter/workflow_store.rs`、`crates/pulsen/tests/conformance_workflow_store.rs`
- **変更内容:** workflow-store の適合ケース31件を書き、`FsWorkflowStore`（`new(workflows_dir, base_dir)`・`.yml` フォールバックなし・相対パスは `base_dir` 基準・`resolved_from` は絶対パス・未知キーを `UnknownKey`・構文エラーと重複キーを `YamlSyntax` にして `WorkflowAssembler` へ渡す）で全件 PASS させる。相対パス解決のケース（TC-005）は cwd を書き換えずに `base_dir` の注入で検証する（adr.md ADR-030）。
- **理由:** 厳格パース（ADR-013）の全エラー種をポート越しに検証する契約を、cwd というプロセス全体の可変状態に依存せずに確定する。
- **消化するチェックリスト:** ADP-workflowstore-001、TC-port-workflow-store-001〜031

### 12. TaskRepository: 適合ケース44件とアダプター

- **対象ファイル:** `crates/pulsen-conformance/src/task_repository.rs`、`crates/pulsen/src/adapter/{task_file,task_repository}.rs`、`crates/pulsen/tests/conformance_task_repository.rs`
- **変更内容:** task-repository の適合ケース44件（破損系は意図レベルのフック経由）を書き、上記 JSON 形式の DTO と符号化・復号（`Corrupt` / `SnapshotUnreadable` の区別は adr.md ADR-025 の表に従う、`snapshot` の生バイト温存、タスク側の未知キー拒否）、7メソッドの実装（`create` の横断一意性、`archive` の原子的移動と移動先自動作成、走査の命名形式フィルタ、ディレクトリ不在の空結果、書き込み系のディレクトリ自動作成）で全件 PASS させる。フィクスチャの `Task` / `DegradedTask` は `rehydrate` / 再構築コンストラクタで組む。fs ハーネスは `concurrent_repo` を実装し、原子性の3件（TC-042〜044）も走らせる。あわせて **adr.md ADR-025 が求める spec 追従の提起**（spec/testcases/ports/task-repository.md の「スナップショットフィールドのみを構文不正な内容に置き換える」を、実装可能な「有効な JSON だがスナップショットとして解釈できない」へ言い換える提案）を Issue のコメントに残す。
- **理由:** 本番の永続バックエンドとタスクファイル形式をここで確定する。以降の全スライスがこの形式に乗る。分類の境界が実装で確定した時点が、spec 側へ判断を戻す適切なタイミングになる。
- **消化するチェックリスト:** ADP-taskrepo-001〜007、TC-port-task-repository-001〜044、PAGE-common-004、PAGE-common-009、UC-flow-007

### 13. Clock / TaskIdGenerator: 適合ケース10件とアダプター

- **対象ファイル:** `crates/pulsen-conformance/src/{clock,task_id_generator}.rs`、`crates/pulsen/src/adapter/{clock,task_id}.rs`、`crates/pulsen/tests/conformance_time_id.rs`
- **変更内容:** clock 5件・task-id-generator 5件の適合ケースを書き、`SystemClock`（秒精度 UTC。`SystemTime` の経過秒を `Timestamp::from_unix_secs` に通す）と `DefaultTaskIdGenerator`（構築時に受け取った `Clock` 由来の時刻成分 + base36 **8桁**乱数。`TaskId` 制約を常に満たす）で PASS させる。桁数は「1万回発行して重複しない」（task-id-generator TC-002）から逆算した値であり、6桁では約2.3%の確率でフレーキーに落ちる（adr.md ADR-026）。RFC3339 往復のケース（clock TC-002）は `Timestamp` 自身の変換で書く。`SystemClock` のハーネスは `observe_wall_clock`（`SystemTime` を直接読む）と `advance`（実時間を1秒待つ）を実装し、clock TC-003・004 も走らせる。巻き戻し（TC-005）はシステム時計を過去に設定できないため `rewind()` が `None` を返してスキップし、理由を出力する（チェックは付けない）。**どちらのアダプターも `unwrap` を持たない**（adr.md ADR-036）: `DefaultTaskIdGenerator::new(clock)` が `Result` を返してエントロピー取得を1回に閉じ、`generate` は内部 PRNG で無謬に保つ。`SystemClock::now` は `duration_since` の `Err` を epoch 前の負の秒に写し、表現可能範囲外は端に飽和させる。
- **理由:** 時刻と ID の生成規則をここで固定する。どちらも外部フィクスチャを要さず、他のアダプターより先に閉じられる。無謬なポートに載る失敗の扱いをここで決めておかないと、実装時に `unwrap` が既定になる。
- **消化するチェックリスト:** ADP-clock-001、ADP-taskid-001、TC-port-clock-001〜005、TC-port-task-id-generator-001〜005

### 14. ExclusiveLock / WorktreeManager: 適合ケース16件とアダプター

- **対象ファイル:** `crates/pulsen-conformance/src/{exclusive_lock,worktree_manager}.rs`、`crates/pulsen/src/adapter/{lock,worktree}.rs`、`crates/pulsen/examples/lock_holder.rs`、`crates/pulsen/tests/{conformance_lock,conformance_worktree,common/git.rs}.rs`
- **変更内容:** exclusive-lock 7件・worktree-manager 9件の適合ケースを書き、`FileExclusiveLock`（`std::fs::File::try_lock`。`WouldBlock` = `Ok(None)`、機構異常 = `Err(Failed)`、`LockGuard` が `File` を保持）と `GitCliWorktreeManager`（上記判定表）で PASS させる。
  - ロックのフィクスチャ: `examples/lock_holder.rs`（引数のパスをロックし `locked` を1行出力して stdin が閉じるまで保持）。テストは `CARGO_BIN_EXE_pulsen` の親ディレクトリ配下 `examples/lock_holder` として解決する（adr.md ADR-032）。`LockError::Failed` はロックパスにディレクトリを置いて再現する（環境非依存）。
  - git のフィクスチャ: `git init -b main`、`GIT_CONFIG_GLOBAL` / `GIT_CONFIG_SYSTEM` の無効化、`GIT_DIR` 等の除去、`-c user.name/-c user.email` 付きコミット、「git リポジトリでないディレクトリ」は `rev-parse --show-toplevel` が失敗することを確認してから使う（adr.md ADR-033）。
  - `TargetError::Failed`（worktree-manager TC-009）は、ハーネスの `failing_manager()` が**存在しないパスを `git_program` として構築した2つ目の `GitCliWorktreeManager`** を返すことで再現する（adr.md ADR-024 / ADR-027）。本番アダプターに内部可変性を持ち込まず、シム・権限操作も要らない。3メソッドとも起動失敗 → `Failed` に落ち、`NotFound` / `NotARepository` / `DetachedHead` / `EmptyRepository` と区別されることを検証する。メタデータ破壊（`.git/HEAD` の破壊等）では `validate_repo` が `NotARepository` を返すため使わない。
- **理由:** OS 依存の3操作（ロック・git・プロセス起動）をアダプターに閉じ込め、クロスプラットフォームの分岐をここだけに留める。フィクスチャの再現性をここで確定する。
- **消化するチェックリスト:** ADP-lock-001、ADP-worktree-001〜003、TC-port-exclusive-lock-001〜007、TC-port-worktree-manager-001〜009

### 15. ポートのテストダブル

- **対象ファイル:** `crates/pulsen-conformance/src/doubles/*.rs`
- **変更内容:** `ScriptedTaskIdGenerator` / `ScriptedExclusiveLock` / `ScriptedWorktreeManager` / `ScriptedTaskRepository` / `FixedClock` / `ScriptedWorkflowStore` の6種を実装する（adr.md ADR-028）。`RegisterTask` が受け取らない `ConfigStore` のダブルは作らない。汎用の in-memory ストアも作らない。各ダブルは「与えた結果列を順に返し、記録した呼び出しを検査できる」だけの最小構成にする。
- **理由:** 実アダプターでは外から状況を作れない TC（ID衝突・`LockError::Failed`・`TargetError::Failed`・`create` の I/O エラー）を消化するために必要で、同時に「ポートが本当に差し替え可能な形になっているか」を本スライスで1度検証する（CLAUDE.md）。
- **消化するチェックリスト:** （チェックリスト行は持たない。AC-18 の前提）

### 16. アプリケーション層（ホームレイアウトと RegisterTask）

- **対象ファイル:** `crates/pulsen/src/application/{home,register_task}.rs`、`crates/pulsen/tests/register_task.rs`
- **変更内容:** `PulsenHome`（`config_path` / `workflows_dir` / `state_root` / `worktree_root` / `lock_path` の導出と `StateRoot` / `WorktreeRoot` の生成。adr.md ADR-031）と、`RegisterTask`（ポートをジェネリック引数で受け取り、上記の処理フローと `RegisterTaskError` を返す）。テストは**すべてテストダブル**に対して書き、実プロセス・実ファイルシステムを使わない。正常系（名前/パス指定・`--base` 省略/指定・`Conflict` 1回再試行）と、実アダプターでは作れない異常系（TC-012 / TC-018 / TC-040 / TC-047 / TC-048）、および各エラー種で `TaskRepository::create` が呼ばれないことを検証する。
- **理由:** 「ポート越しに観測し、ドメインに判断させ、ポート越しに実行する」流れをここで初めて組み立てる。判断ロジックがユースケースへ漏れていないかの確認点になり、差し替え可能性がここで行使される。
- **消化するチェックリスト:** UC-task-001、UC-task-006（共通事項のうち**ロック方針と parse 境界**。ホーム解決と `ConfigStore::load` はステップ17）、PAGE-common-002、PAGE-common-003、TC-task-register-task-012, 018, 040, 047, 048

### 17. CLI（引数・合成ルート・出力・exit code）

- **対象ファイル:** `crates/pulsen/src/cli/{args,wire,add,render,exit}.rs`、`crates/pulsen/src/main.rs`
- **変更内容:** clap による `pulsen [--home] add --workflow --repo [--base]`、`cli::wire` の合成（ホーム解決 → `PulsenHome` → `ConfigStore::load` → アダプター構築（`base_dir` に cwd を渡す）→ `--repo` の絶対化）、`cli::render` による案内文言（未初期化ホーム・パースエラー位置・解決を試みたパス・定義済みエージェント一覧・`--base` 明示の案内・検証エラー全件・ロック競合）、exit code 規約（成功 0 / エラー 1 / 使い方の誤り 2）、成功時の表示（タスクID・ワークフロー名・解決先パス）。出力は人間可読テキストのみで、機械可読形式と設定・定義の生成コマンドは提供しない。
- **理由:** CLI が「画面」であり、spec/pages の要求（表示内容・案内・exit code）を満たす唯一の層。合成ルートを1箇所に閉じることで、アプリケーション層の依存方向を保つ。
- **消化するチェックリスト:** UC-task-006（共通事項のうち**ホーム解決と `ConfigStore::load`**。ロック方針と parse 境界はステップ16）、PAGE-add-001〜010、PAGE-common-001, 002, 003, 005, 006, 007, 008, 010, 011

### 18. add 受け入れテスト: 正常系

- **対象ファイル:** `crates/pulsen/tests/cli_add_normal.rs`、`crates/pulsen/tests/common/mod.rs`
- **変更内容:** 共通フィクスチャ（一時ホーム・一時 git リポジトリ（コミットあり / detached HEAD / 空リポジトリ）・各種ワークフローYAML。git 環境の固定は adr.md ADR-033）を用意し、正常系13件のうち TC-012 を除く12件（TC-001〜011・013）をバイナリ起動レベルで検証する。登録直後のタスクファイル（`state/tasks/<task-id>.json`）の中身（タスクステータス = `initial`・pending・カウンタ全0・未設定フィールド・スナップショット埋め込み）も直接読んで確認する。
- **理由:** 実アダプターの結線が正しいことを最初に確かめる。以降の異常系テストはこのフィクスチャの上に乗る。
- **消化するチェックリスト:** TC-task-register-task-001〜011, 013

### 19. add 受け入れテスト: 異常系

- **対象ファイル:** `crates/pulsen/tests/cli_add_error.rs`
- **変更内容:** 異常系35件のうちユースケース層で消化する4件（TC-018 / 040 / 047 / 048）を除く31件（TC-014〜017・019〜039・041〜046）をバイナリ起動レベルで検証する。各ケースで exit code が非0であること、案内文言が spec の期待どおりであること、**タスクが作られないこと**、および**ワークフロー定義ファイルと config.yaml が変更されないこと**（PAGE-common-006 規則2「読めないリソースには書き込まない」の観測可能な帰結）を確認する。ロック競合（TC-017）は `examples/lock_holder` でロックを保持して再現する。権限操作でしか再現できない TC-016（config.yaml が読めない）・TC-021（ワークフロー定義が読めない）は POSIX のみで実行し、root では skip する。
- **理由:** Issue の完了条件が「チェックリスト全行の実装をレビューで確認できること」であり、TC 行はテストとして存在して初めて消化できる。異常系が最も件数が多く、単独のレビュー単位にする価値がある。
- **消化するチェックリスト:** TC-task-register-task-014〜017, 019〜039, 041〜046、PAGE-common-006

### 20. add 受け入れテスト: 境界値とエッジケース

- **対象ファイル:** `crates/pulsen/tests/cli_add_boundary.rs`
- **変更内容:** 境界値11件（TC-049〜059）とエッジケース8件（TC-060〜067）をバイナリ起動レベルで検証する。拒否側（TC-053・054・055・058）は「タスクが作られないこと」と「ワークフロー定義ファイルと config.yaml が変更されないこと」を、受理側（TC-049〜052・056・057・059・060〜067）は「登録が成功する」ことを検証する。**拒否側の2点はステップ19の異常系と同じ否定的主張である**（AC-15 が異常系と境界値の拒否ケースを同じ条件で括っている）。**TC-052（名前として `workflows/<name>.yaml` に解決され、`.yml` へフォールバックしない）は `.yaml` 版の成功と `.yml` のみ版の `NotFound` を対にして検証する** — 受理側1件だけでは後半の否定的主張（フォールバックの不在。`FsWorkflowStore` の実装分岐そのもの）が一度も検証されない。`--home` と `PULSEN_HOME` の優先順位（TC-067）、`state/` 配下の自動作成（TC-060。ステップ12でポート契約として検証した自動作成が、CLI 経由でも成立することの確認）を含む。
- **理由:** 境界値・エッジケースは受理側と拒否側が混在するため、受け入れ基準（AC-15 / AC-16）と対応づけて1つのファイルにまとめると判定の取り違えが起きにくい。
- **消化するチェックリスト:** TC-task-register-task-049〜067、PAGE-common-009

## チェックリスト消化の対応表（俯瞰）

| チェックリスト範囲 | ステップ |
|---|---|
| DOM-definition-001〜023, 030, 031 | 2 |
| DOM-definition-024〜029, 032〜039, 041〜044 | 3 |
| DOM-definition-040, 045〜050, 056 | 4 |
| DOM-definition-051〜055 | 7 |
| DOM-task-001〜027, 078 | 5 |
| DOM-task-028〜032, 056, 060, 079 | 6 |
| DOM-task-062〜077 | 7 |
| DOM-execution-059, 060, 061, 065, 068, 069, 070 | 7 |
| ADP-config-001 | 10 |
| ADP-workflowstore-001 | 11 |
| ADP-taskrepo-001〜007 | 12 |
| ADP-clock-001, ADP-taskid-001 | 13 |
| ADP-lock-001, ADP-worktree-001〜003 | 14 |
| UC-task-001 | 16 |
| UC-task-006 | 16, 17 |
| UC-flow-007 | 8, 12 |
| PAGE-common-001, 005, 007, 008, 010, 011 | 17 |
| PAGE-common-002, 003 | 16, 17 |
| PAGE-common-004 | 12 |
| PAGE-common-006 | 17, 19 |
| PAGE-common-009 | 12, 20 |
| PAGE-add-001〜010 | 17 |
| TC-task-register-task-001〜011, 013 | 18 |
| TC-task-register-task-012, 018, 040, 047, 048 | 16 |
| TC-task-register-task-014〜017, 019〜039, 041〜046 | 19 |
| TC-task-register-task-049〜067 | 20 |
| TC-port-config-store-001〜024 | 9, 10 |
| TC-port-workflow-store-001〜031 | 9, 11 |
| TC-port-task-repository-001〜044 | 9, 12 |
| TC-port-clock-001〜005, TC-port-task-id-generator-001〜005 | 9, 13 |
| TC-port-exclusive-lock-001〜007, TC-port-worktree-manager-001〜009 | 9, 14 |

PASS 条件が全コマンドを前提にしている6行（PAGE-common-002 / 003 / 005 / 006 / 010、UC-flow-007）は、本スライスに存在するコマンド（`add`）の列で消化する。規則そのもの（ホーム解決・ロック取得・exit code・縮退4規則・タスクファイルの生涯）が実装として確定していることを消化の条件とし、後続スライスがコマンドを足したときの適用は、そのコマンドの台帳行（`PAGE-tick-006` など。Issue #2〜#6 に配分済み）が受け持つ。判定基準は plan.md「チェックリスト行にチェックを付ける基準」を参照。
