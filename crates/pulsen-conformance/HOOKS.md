# 適合テストの行 × ハーネスのフック

`spec/testcases/ports/*.md` のうち、これまでのスライスで扱った9ポート169行を、どう組み立てるかで分類した表。フックは spec の前提条件から導く（ADR-027）。行を足す・フックを足すときはこの表も更新する。

台帳行に対応しない追加ケースは、件数に数えず「追加ケース」として区別して載せる。

区分の意味は次のとおり。

| 区分 | 意味 |
|---|---|
| A | ポートのメソッドとドメイン型だけで組める（フック不要） |
| B | ハーネスのフックで組める |
| C | spec が「再現できるアダプター環境に限る」と明示する行。フックが `None` を返した環境ではスキップし、チェックリストにチェックを付けない |

区分は行ごとに1つで、C の行もフックを経由する（フックが `Some` を返す環境では実行される）。

| 区分 | 件数 |
|---|---|
| A | 38 |
| B | 113 |
| C | 18 |
| 合計 | 169 |

スキップは libtest の出力では成功と区別できないため、スイートを適用するテストファイルが「この環境でスキップを許容するケース」を集合として宣言する（`SkipBudget`）。集合の外のスキップはそのケースの失敗として現れる。集合は環境の能力から実行時に決める（`.adr/055-conformance-skip-budget.md`）。

## 環境で走らなくなりうる行

**何が前提を壊すかは行ごとに違う**。共通の述語 `permission_restrictions_effective` で導けるのは権限操作に依存する12行だけで、区分 C の残る6行のうち4行はそれぞれ別の能力を要求する。残る2行（TC-port-process-controller-003 / 005）は spec の文面こそ「再現できるアダプター環境に限る」だが、**別ハンドルの注入**で確定的に走る（ADR-068）。005 は前提を作れない環境が無いためこの表に現れず、003 は注入とは別に `launch_spec` がテスト用エージェントを要するため、その行にだけ現れる。区分 B にも、フックの前提が環境で成立しない行がある。宣言を組むときはこの表で読む。

| ID | 区分 | 前提を作れない環境 | 判定 |
|---|---|---|---|
| TC-port-config-store-023 | C | 読み取れないファイルを作れない（root 実行・非 POSIX・権限を持たないファイルシステム） | `permission_restrictions_effective` |
| TC-port-workflow-store-030 | C | 同上 | `permission_restrictions_effective` |
| TC-port-task-repository-005 / 011 / 012 / 035 | C | 書き込めないディレクトリを作れない（同上） | `permission_restrictions_effective` |
| TC-port-task-repository-019 / 041 | C | 読み取れないディレクトリを作れない（同上） | `permission_restrictions_effective` |
| TC-port-run-store-007 | C | 読み取れないファイルを作れない（同上） | `permission_restrictions_effective` |
| TC-port-run-store-017 | C | 書き込めない attempt ディレクトリを作れない（同上） | `permission_restrictions_effective` |
| TC-port-process-controller-023 | C | 実行権限のない実体を作れない（同上） | `permission_restrictions_effective` |
| TC-port-process-controller-025 | C | 書き込めないログの置き場を作れない（同上） | `permission_restrictions_effective` |
| TC-port-process-controller-024 | B | exit code を持たない終了（シグナル死）を作れない（非 POSIX） | `agent_probe abort` がシグナル死になるプラットフォームか |
| TC-port-process-controller-001 / 002 / 003 / 017〜027 | B | テスト用エージェント（`examples/agent_probe`）がビルドされていない（単一のテストターゲットを指定した実行） | ハーネスが `agent_command` を提供するか。**スキップ許容集合には入れない** — 作り忘れを緑にしないため |
| TC-port-process-controller-002 | B | 上記に加えて、デタッチ性のフィクスチャ（`examples/spawn_probe`）がビルドされていない | ハーネスが `spawn_from_other_process` を提供するか。同じく**スキップ許容集合には入れない** |
| TC-port-clock-003 | C | 実時刻を観測できない（時刻を注入するアダプター） | ハーネスが `observe_wall_clock` を提供するか |
| TC-port-clock-005 | C | 時刻を過去へ巻き戻せない（実時計のアダプター） | ハーネスが `rewind` を提供するか |
| TC-port-exclusive-lock-007 | C | ロック機構自体を利用不能にできない | ハーネスが `unusable_lock` を提供するか |
| TC-port-worktree-manager-009 | C | 必ず失敗するハンドルか、コミットのあるリポジトリを用意できない | ハーネスが `failing_manager` / `repo_with_commit` / `head_branch_name` を提供するか |
| TC-port-worktree-manager-003 | B | git リポジトリでない実在のディレクトリを作れない（一時ディレクトリの置き場自体がリポジトリ配下） | ハーネスが `non_repo_dir` を提供するか |
| TC-port-exclusive-lock-002 / 003 / 004 / 005 | B | 別プロセスにロックを保持させる実行ファイルが無い（単一テストターゲットを指定した実行では example がビルドされない） | ハーネスが `hold_from_other_process` / `try_acquire_from_other_process` を提供するか |

## 適用範囲

TaskRepository / Clock / TaskIdGenerator / ExclusiveLock / WorktreeManager / RunStore / ProcessController のフックは、破損・状況の**意味**だけを受け取る。この7ポートのスイートは、フックを実装するだけで別の実装（in-memory 等）にも適用できる。

ConfigStore / WorkflowStore の入力系フック（`put_config` / `put_named` / `put_named_with_ext` / `put_at_absolute` / `put_at_relative`）は **YAML ソースを受け取る**。「YAML 構文エラー」「重複キー」を前提とする行（TC-port-config-store-014 / TC-port-workflow-store-017）は、表現そのものを渡す口が無ければ組み立てられないためで、この2ポートのスイートは YAML 表現に結合している（`.adr/053-conformance-yaml-source-hooks.md`）。

## ConfigStore（24行 / A 0・B 23・C 1）

| ID | 前提条件 | 区分 | 組み立て手段 |
|---|---|---|---|
| TC-port-config-store-001 | 全キーを有効な値で記述 | B | `put_config` |
| TC-port-config-store-002 | 空マッピング | B | `put_config` |
| TC-port-config-store-003 | 空ファイル・null ドキュメント | B | `put_config` |
| TC-port-config-store-004 | 一部のキーのみ | B | `put_config` |
| TC-port-config-store-005 | 単位の異なる等価な期間値 | B | `put_config` |
| TC-port-config-store-006 | `cmd` を文字列形式 | B | `put_config` |
| TC-port-config-store-007 | `cmd` を配列形式（空文字列トークンを含む） | B | `put_config` |
| TC-port-config-store-008 | `notify_cmd` を文字列形式・配列形式 | B | `put_config` |
| TC-port-config-store-009 | `cmd` に未知プレースホルダ | B | `put_config` |
| TC-port-config-store-010 | `cmd` に波括弧不正 | B | `put_config` |
| TC-port-config-store-011 | `skill_input` に `{skill}` 以外 | B | `put_config` |
| TC-port-config-store-012 | `notify_cmd` のトークンに波括弧 | B | `put_config` |
| TC-port-config-store-013 | config.yaml が存在しない | B | `remove_config` + `home_path`（`NotFound` が含むホームパスの期待値） |
| TC-port-config-store-014 | YAML 構文エラー | B | `put_config` |
| TC-port-config-store-015 | スキーマに無いトップレベルキー | B | `put_config` |
| TC-port-config-store-016 | 組み込み定数に相当するキー | B | `put_config` |
| TC-port-config-store-017 | `agents` のエントリ内の未知キー | B | `put_config` |
| TC-port-config-store-018 | `agents` のエントリに `cmd` が無い | B | `put_config` |
| TC-port-config-store-019 | 型不一致 | B | `put_config` |
| TC-port-config-store-020 | `judge_attempt_limit` / `spawn_fail_limit` が 0 | B | `put_config` |
| TC-port-config-store-021 | 期間形式の不正 | B | `put_config` |
| TC-port-config-store-022 | 空のコマンド（空文字列・空配列） | B | `put_config` |
| TC-port-config-store-023 | 存在するが読み取れない | C | `put_config` + `make_unreadable` |
| TC-port-config-store-024 | 有効な内容を別の有効な内容へ置き換える | B | `put_config` 2回 |

## WorkflowStore（31行 / A 0・B 30・C 1）

| ID | 前提条件 | 区分 | 組み立て手段 |
|---|---|---|---|
| TC-port-workflow-store-001 | 名前で解決される位置に有効な定義 | B | `put_named` + `expected_path_for_name`（`resolved_from` の期待値） |
| TC-port-workflow-store-002 | `.yml` のみを置く | B | `put_named_with_ext` + `expected_path_for_name` |
| TC-port-workflow-store-003 | 該当ファイルが無い | B | `expected_path_for_name` |
| TC-port-workflow-store-004 | 任意の絶対パスに有効な定義 | B | `put_at_absolute` |
| TC-port-workflow-store-005 | 相対パスで参照できる位置に有効な定義 | B | `put_at_relative` |
| TC-port-workflow-store-006 | 指定パスにファイルが無い | B | `missing_absolute_path` |
| TC-port-workflow-store-007 | `workflow:` キーを持つ有効な定義 | B | `put_named` |
| TC-port-workflow-store-008 | トップレベルの `agent` / `model` | B | `put_named` |
| TC-port-workflow-store-009 | `workflow:` キーの無い定義 | B | `put_named` |
| TC-port-workflow-store-010 | `skill` 指定と全キーを使うステータス | B | `put_named` |
| TC-port-workflow-store-011 | `timeout: none` | B | `put_named` |
| TC-port-workflow-store-012 | `retries: 0` | B | `put_named` |
| TC-port-workflow-store-013 | 自己参照・循環 | B | `put_named` |
| TC-port-workflow-store-014 | 到達不能ステータス | B | `put_named` |
| TC-port-workflow-store-015 | `judge` のトークンに波括弧 | B | `put_named` |
| TC-port-workflow-store-016 | 未定義のエージェント名を参照 | B | `put_named` |
| TC-port-workflow-store-017 | YAML 構文エラー・重複キー | B | `put_named` |
| TC-port-workflow-store-018 | トップレベルの許容外キー | B | `put_named` |
| TC-port-workflow-store-019 | ステータス内のスキーマ外キー | B | `put_named` |
| TC-port-workflow-store-020 | `wait` / `cleanup` にエージェント実行系キーを併記 | B | `put_named` |
| TC-port-workflow-store-021 | `initial` キーが無い | B | `put_named` |
| TC-port-workflow-store-022 | `initial` が `statuses` に無い | B | `put_named` |
| TC-port-workflow-store-023 | `statuses` が空・欠落 | B | `put_named` |
| TC-port-workflow-store-024 | 動作宣言の無いステータス | B | `put_named` |
| TC-port-workflow-store-025 | 動作宣言が複数あるステータス | B | `put_named` |
| TC-port-workflow-store-026 | `run` の値が `cleanup` / `wait` 以外 | B | `put_named` |
| TC-port-workflow-store-027 | AgentRun に `next` が無い | B | `put_named` |
| TC-port-workflow-store-028 | `next` が `statuses` に無い | B | `put_named` |
| TC-port-workflow-store-029 | 値の生成エラーを含む定義 | B | `put_named` |
| TC-port-workflow-store-030 | 存在するが読み取れない | C | `put_named` + `make_unreadable` |
| TC-port-workflow-store-031 | 成功後に別の有効な定義へ書き換える | B | `put_named` 2回 |

## TaskRepository（44行 / A 21・B 17・C 6）

タスク・破損タスクのフィクスチャは `Task::rehydrate` / `DegradedTask` の再構築コンストラクタで組む（6状態・全 Optional フィールドを受け付ける公開経路）。破損させる対象の ID は、フック呼び出しの前に `create` 済みであることを前提にしてよい。

| ID | 前提条件 | 区分 | 組み立て手段 |
|---|---|---|---|
| TC-port-task-repository-001 | 状態ディレクトリに何も無い | A | `create` → `find` |
| TC-port-task-repository-002 | `create` 済みの ID | A | `create` 2回 |
| TC-port-task-repository-003 | `create` → `archive` 済みの ID | A | `create` → `archive` → `create` |
| TC-port-task-repository-004 | 現役に JSON として不正な内容 | B | `corrupt_whole_record(Active)` + `record_bytes`（内容の不変を観測） |
| TC-port-task-repository-005 | 書き込み先を用意できない | C | `make_unwritable(Active)` |
| TC-port-task-repository-006 | `create` 済みのタスク | A | `save` → `find` |
| TC-port-task-repository-007 | `create` していない ID | A | `save` |
| TC-port-task-repository-008 | `create` → `archive` 済み | A | `archive` → `save` |
| TC-port-task-repository-009 | スナップショットのみ破損したタスク | B | `corrupt_snapshot(Active)` + `snapshot_bytes`（温存を観測） |
| TC-port-task-repository-010 | 現役に存在しない ID の DegradedTask | A | `save_degraded` |
| TC-port-task-repository-011 | 書き込み先へ書き込めない | C | `make_unwritable(Active)` |
| TC-port-task-repository-012 | スナップショット破損 + 書き込めない | C | `corrupt_snapshot(Active)` + `make_unwritable(Active)` |
| TC-port-task-repository-013 | 全 Optional フィールドを持つタスク | A | `create` → `find` |
| TC-port-task-repository-014 | 各実行状態のタスク | A | `create` / `save` → `find` |
| TC-port-task-repository-015 | 何も作成していない | A | `find` |
| TC-port-task-repository-016 | `create` 済み | A | `find` |
| TC-port-task-repository-017 | `create` → `archive` 済み | A | `find` |
| TC-port-task-repository-018 | 同一 ID を双方に配置 | B | `place_in_both_areas` |
| TC-port-task-repository-019 | 走査対象を読み取れない | C | `make_unreadable(Active)` |
| TC-port-task-repository-020 | ファイル全体が JSON として不正 | B | `corrupt_whole_record(Active)` |
| TC-port-task-repository-021 | タスク側フィールドの値制約の破れ | B | `break_task_field(Active)` |
| TC-port-task-repository-022 | スナップショットのみ解釈できない | B | `corrupt_snapshot(Active)` |
| TC-port-task-repository-023 | スナップショットの削除 | B | `drop_snapshot_field(Active)` |
| TC-port-task-repository-024 | `task_status` が statuses に無い | B | `set_task_status_outside_snapshot(Active)` |
| TC-port-task-repository-025 | スナップショットの構造不変条件の破れ | B | `break_snapshot_invariant(Active)` |
| TC-port-task-repository-026 | 不変条件2〜4の破れ | A | 不整合な `Task::rehydrate` の結果を `create` → `find`（デコードでは検証しない） |
| TC-port-task-repository-027 | アーカイブ側に全体破損 | B | `corrupt_whole_record(Archived)` |
| TC-port-task-repository-028 | アーカイブ側にスナップショット破損 | B | `corrupt_snapshot(Archived)` |
| TC-port-task-repository-029 | 現役側の各破損フィクスチャ | B | `corrupt_whole_record` / `corrupt_snapshot`（両 Area）→ `list_active` |
| TC-port-task-repository-030 | 命名形式に合致しないエントリ | B | `put_unnamed_entry(Active)` |
| TC-port-task-repository-031 | `create` 済み（`state/archive/` 不在） | A | `archive` → `find` |
| TC-port-task-repository-032 | `archive` 直後 | A | `list_active` / `list_archived` / `find` |
| TC-port-task-repository-033 | `create` していない ID | A | `archive` |
| TC-port-task-repository-034 | `archive` 済みの ID | A | `archive` 2回 |
| TC-port-task-repository-035 | 移動先を用意できない | C | `make_unwritable(Archived)` |
| TC-port-task-repository-036 | 走査対象ディレクトリが存在しない | A | `list_active` / `list_archived` |
| TC-port-task-repository-037 | 複数タスクのうち1つを `archive` | A | `list_active` |
| TC-port-task-repository-038 | 同上 | A | `list_archived` |
| TC-port-task-repository-039 | 現役に正常・全体破損・スナップショット破損が混在 | B | `corrupt_whole_record(Active)` + `corrupt_snapshot(Active)` |
| TC-port-task-repository-040 | アーカイブ側に同じ混在 | B | `corrupt_whole_record(Archived)` + `corrupt_snapshot(Archived)` |
| TC-port-task-repository-041 | 走査対象が存在するが読み取れない | C | `make_unreadable(Active)` / `make_unreadable(Archived)` |
| TC-port-task-repository-042 | 別スレッドが読み続ける中で `save` を反復 | B | `concurrent_repo` |
| TC-port-task-repository-043 | `save` が `Err`（NotFound / Io）を返した | A | `save` → `find` / `list_active`。Io 分岐は `make_unwritable(Active)` があればそこも観測する（無ければその分岐だけ飛ばす。行の主張は NotFound 分岐が常に観測するためスキップにはしない） |
| TC-port-task-repository-044 | 別スレッドが読み続ける中で `archive` | B | `concurrent_repo` |

## Clock（5行 / A 2・B 1・C 2）

| ID | 前提条件 | 区分 | 組み立て手段 |
|---|---|---|---|
| TC-port-clock-001 | なし | A | `now` |
| TC-port-clock-002 | なし | A | `now` + `Timestamp::to_rfc3339` / `parse_rfc3339`（変換はドメインが持つ。ADR-020） |
| TC-port-clock-003 | 呼び出しの前後で実時刻を観測できる（spec は「テスト中に時刻改変が起きないアダプター環境に限る」とする） | C | `observe_wall_clock` |
| TC-port-clock-004 | 時刻が進んだ状態 | B | `advance` |
| TC-port-clock-005 | 時刻を過去へ巻き戻した状態 | C | `rewind` |

## TaskIdGenerator（5行 / A 4・B 1・C 0）

| ID | 前提条件 | 区分 | 組み立て手段 |
|---|---|---|---|
| TC-port-task-id-generator-001 | なし | A | `generate` + `TaskId::parse` |
| TC-port-task-id-generator-002 | なし | A | `generate` を1万回 |
| TC-port-task-id-generator-003 | なし | A | `generate` を連続で |
| TC-port-task-id-generator-004 | 同じ構成のジェネレーターを複数 | B | `another_generator` |
| TC-port-task-id-generator-005 | なし | A | `generate` + `WorktreePath` / `BranchName::parse` / `TaskFilePath::active` の導出（基点は `std::env::temp_dir()`。ファイルは作らない） |

## ExclusiveLock（7行 / A 1・B 5・C 1）

TC-002〜005 は別プロセスに保持させる実行ファイルを要するため、環境で走らなくなりうる（「環境で走らなくなりうる行」を参照）。

| ID | 前提条件 | 区分 | 組み立て手段 |
|---|---|---|---|
| TC-port-exclusive-lock-001 | 誰も保持していない | A | `try_acquire` |
| TC-port-exclusive-lock-002 | 別プロセスが保持中 | B | `hold_from_other_process` + `release_holder` |
| TC-port-exclusive-lock-003 | 別プロセスが保持し続けている | B | `hold_from_other_process`（取得の試行を別スレッドに置き、期限までに返ることを観測。ADR-060）+ `release_holder` |
| TC-port-exclusive-lock-004 | ガードを取得後にドロップ済み | B | `try_acquire_from_other_process`（同一プロセス内の再取得は契約外） |
| TC-port-exclusive-lock-005 | 保持プロセスを強制終了 | B | `hold_from_other_process` + `kill_holder` |
| TC-port-exclusive-lock-006 | 異なるホームのロックを別ハンドルが保持中 | B | `separate_home` |
| TC-port-exclusive-lock-007 | ロック機構自体が利用不能 | C | `unusable_lock` |

## WorktreeManager（本スライス該当の16行 / A 0・B 15・C 1、+ 追加ケース1件）

`remove` の5行は、それをポートにメソッドとして足すスライスで扱う。

`create` のケースが使う `ws.path` の置き場（worktree_root）は**シンボリックリンク経由のパス**として組む。同定の鍵は物理パスなので（ADR-077）、置き場が実体そのものだと正規化の分岐がどのケースからも実行されない。

| ID | 前提条件 | 区分 | 組み立て手段 |
|---|---|---|---|
| TC-port-worktree-manager-001 | コミットのあるリポジトリ | B | `repo_with_commit` |
| TC-port-worktree-manager-002 | 存在しないパス | B | `missing_path` |
| TC-port-worktree-manager-003 | リポジトリでない実在のディレクトリ | B | `non_repo_dir` |
| TC-port-worktree-manager-004 | HEAD がブランチを指す | B | `repo_with_commit` + `head_branch_name` |
| TC-port-worktree-manager-005 | detached HEAD | B | `detached_repo` |
| TC-port-worktree-manager-006 | コミットのない空リポジトリ | B | `repo_without_commit` |
| TC-port-worktree-manager-007 | 指定ブランチが存在する | B | `repo_with_commit` + `head_branch_name` |
| TC-port-worktree-manager-008 | 指定ブランチが存在しない | B | `repo_with_commit` + `absent_branch_name` |
| TC-port-worktree-manager-009 | git 操作自体が失敗する | C | `failing_manager` + `repo_with_commit` + `head_branch_name`（3メソッドとも `Failed`） |
| TC-port-worktree-manager-010 | base があり、ブランチもパスも未使用 | B | `unused_workspace` + `head_branch_name` + `branch_tip`（新ブランチが base の先端から作られたこと）+ `worktree_present`（`ws.path` が `ws.branch` の worktree として**登録され**、実体も在ること。実体の有無だけを見ると、ブランチを作ってディレクトリを掘っただけの実装が通る） |
| TC-port-worktree-manager-011 | worktree_root 自体が未作成 | B | `workspace_under_missing_root` + `worktree_present` |
| TC-port-worktree-manager-012 | 自タスクの worktree が存在し、内容に変更がある | B | `unused_workspace` → `create` → `put_worktree_marker` + `worktree_marker`（内容の不変を観測） |
| TC-port-worktree-manager-013 | 登録なし・コミットの積まれた `ws.branch` のみ存在 | B | `workspace_with_orphan_branch` + `branch_tip` + `worktree_marker`（先端の不変と成果物の復帰） |
| TC-port-worktree-manager-014 | `ws.path` に worktree でない通常のディレクトリ | B | `workspace_over_plain_dir` + `worktree_marker` + `branch_exists` |
| TC-port-worktree-manager-015 | `ws.path` に別ブランチの worktree | B | `workspace_over_other_branch` + `worktree_marker` + `branch_exists` |
| TC-port-worktree-manager-016 | base に指定したブランチが存在しない | B | `absent_branch_name` + `unused_workspace` + `worktree_present` + `branch_exists` |
| （追加ケース・ADR-077） | 自タスクの登録は残るが実体が消えている（`prunable`） | B | `workspace_with_prunable_registration` + `branch_tip` + `worktree_marker`。復旧の2分岐（`prunable` からの `add -f` と、登録なし・ブランチのみからの `add`）を**どちらも**実行させるための追加。TC-013 を `prunable` 側に寄せると、`add`（`-f` なし）の分岐がどのケースからも実行されない |

## RunStore（本スライス該当の21行 / A 10・B 9・C 2、+ 追加ケース1件）

`attempt_exists` / `list_runs` / `delete_attempt` / `remove_task_dir_if_empty` の22行は、それらをポートにメソッドとして足すスライスで扱う。

run ディレクトリのファイルの位置はケース側が契約の語彙（`RunDirPath` の導出関数）で指し、フックは対象の**種別**（`RunFileKind`）だけを受け取る。

| ID | 前提条件 | 区分 | 組み立て手段 |
|---|---|---|---|
| TC-port-run-store-001 | runディレクトリ階層が未作成 | B | `expected_run_dir` + `attempt_dir_present`（`prepare_attempt` の**前後で観測が反転すること**まで主張する。定数を返すハーネスはどちらかの側で落ちる。ADR-076） |
| TC-port-run-store-002 | 準備済みで write 系を書き込み済み | A | `prepare_attempt` 2回 → read 系（`attempt_exists` は宣言しないため、内容の不変で観測する。ADR-076） |
| TC-port-run-store-003 | 準備済み・pid 未書き込み | A | `prepare_attempt` → `read_pid_file` |
| TC-port-run-store-004 | attempt ディレクトリ自体が不在 | B | `expected_run_dir`（準備しないまま読む） |
| TC-port-run-store-005 | `write_pid_file` 済み | A | `write_pid_file` → `read_pid_file` |
| TC-port-run-store-006 | pid の位置に解釈不能な内容 | B | `put_unreadable_content(Pid)` |
| TC-port-run-store-007 | pid は存在するが読み取り自体が失敗 | C | `make_unreadable(Pid)` |
| TC-port-run-store-008 | 準備済み・starttime 未書き込み | A | `prepare_attempt` → `read_starttime` |
| TC-port-run-store-009 | attempt ディレクトリ自体が不在 | B | `expected_run_dir` |
| TC-port-run-store-010 | `write_starttime` 済み | A | `write_starttime` → `read_starttime` |
| TC-port-run-store-011 | starttime の位置に解釈不能な内容 | B | `put_unreadable_content(StartTime)` |
| TC-port-run-store-012 | 準備済み・exit 未書き込み | A | `prepare_attempt` → `read_exit` |
| TC-port-run-store-013 | attempt ディレクトリ自体が不在 | B | `expected_run_dir` |
| TC-port-run-store-014 | `write_exit` 済み（0 と非0） | A | `write_exit` → `read_exit` |
| TC-port-run-store-015 | exit の位置に解釈不能な内容 | B | `put_unreadable_content(Exit)` |
| TC-port-run-store-016 | write 系の連続書き込み中 | B | `concurrent_store`（別スレッドが読み続ける。読み手の停止は `Drop` に載せる。ADR-063） |
| TC-port-run-store-017 | write 系の書き込みが失敗する | C | `make_attempt_unwritable`（読み取りは残す — 失敗後に従前の値が読めることが行の主張） |
| TC-port-run-store-018 | attempt ディレクトリ自体が不在 | B | `expected_run_dir` → `write_invalidation_marker` → `marker_exists`（ディレクトリごと作られたことは `attempt_dir_present` があればそこも観測する。無ければその分岐だけ飛ばす） |
| TC-port-run-store-019 | マーカー書き込み済み | A | `write_invalidation_marker` 2回 |
| TC-port-run-store-020 | 準備済み・マーカー未書き込み | A | `marker_exists` |
| TC-port-run-store-021 | マーカー書き込み済み | A | `write_invalidation_marker` → `marker_exists` |
| （追加ケース・write 系の置き場作成） | 準備を経ていない attempt ディレクトリが不在 | B | `expected_run_dir` → `write_starttime` / `write_pid_file` / `write_exit` → 各 read 系（`attempt_dir_present` があれば書き込み前の不在もそこで観測する）。台帳行がディレクトリ作成を主張するのはマーカー（TC-018）だけで、`prepare_attempt` の失敗後も起動を続ける経路が残る3つの write 系の同じ契約に乗っている |

## ProcessController（本スライス該当の16行 / A 0・B 12・C 4）

`starttime_of` / `kill` / `try_kill_remnants` の8行は、それらをポートにメソッドとして足すスライスで扱う。

スイートは2つに分かれる（ADR-075）。`identity_and_agent` は `own_identity` / `run_agent` の13行で、アダプター単体で閉じる。`spawn` は `spawn_wrapper` の3行で、ラッパーモード（実バイナリ）の実装を前提にする。1つのテストファイルに両方を適用する。

テスト用エージェントは `agent_command` に**振る舞いの意味**（`AgentBehavior`）だけを渡して組む。プラットフォーム固有のコマンド名やシェルをケースに持ち込まないため（ADR-074）。

| ID | 前提条件 | 区分 | 組み立て手段 |
|---|---|---|---|
| TC-port-process-controller-001 | 用意済みの run_dir・実在する worktree・テスト用エージェント | B | `launch_spec` + `run_dir_is_empty` + `wait_for_run_files`（結末は run ディレクトリ経由でのみ現れる。観測が起動の**前後で反転すること**まで主張する。真偽を定数で返すハーネスはどちらかの側で落ちる） |
| TC-port-process-controller-002 | 一定時間実行し続けるエージェントで `spawn_wrapper` 済み | B | `launch_spec(Sleep)` + `spawn_from_other_process`（呼び出し側プロセスの終了）+ `wait_for_run_files` |
| TC-port-process-controller-003 | ラッパーの起動自体が不可能 | C | `failing_controller`（存在しないパスを自バイナリとして注入。ADR-068）+ `run_dir_is_empty` |
| TC-port-process-controller-004 | テストプロセス自身 | B | `observe_wall_clock`（呼び出し前後の範囲）。pid は `std::process::id()` と突き合わせる |
| TC-port-process-controller-005 | 同定情報の取得機構自体が失敗する | C | `failing_identity_controller`（存在しない取得元を注入。ADR-068） |
| TC-port-process-controller-017 | exit 0 で終了するコマンド | B | `agent_command(Exit(0))` + `worktree` + `log_paths` |
| TC-port-process-controller-018 | 非0（7）で終了するコマンド | B | `agent_command(Exit(7))` |
| TC-port-process-controller-019 | 作業ディレクトリを検査するコマンド | B | `agent_command(CheckCwd(worktree))` |
| TC-port-process-controller-020 | 標準出力・標準エラーへ書くコマンド | B | `agent_command(Print)` + `log_paths`（リダイレクト先の内容を読む） |
| TC-port-process-controller-021 | シェルのメタ文字・空白・プレースホルダを含むトークン | B | `agent_command(EchoArgs(tokens))`（受け取ったトークンが標準出力に1行ずつ現れ、渡した列と一致する） |
| TC-port-process-controller-022 | 存在しないコマンド名 | B | `missing_command` |
| TC-port-process-controller-023 | 実行不能なファイル | C | `non_executable_command`（起動が拒否されることを確かめてから `Some`） |
| TC-port-process-controller-024 | 外部から強制終了されるコマンド | B | `agent_command(Abort)`（期待は**非0の符号化値**まで。`128+シグナル番号` の具体値はアダプターのユニットテストが固定する。ADR-074） |
| TC-port-process-controller-025 | stdout のリダイレクト先が開けない | C | `unwritable_log_path`（起動していれば 0 が返るため、126 が「エージェントの副作用が生じていない」の観測になる） |
| TC-port-process-controller-026 | cwd が存在しない | B | `missing_worktree` |
| TC-port-process-controller-027 | 一定時間実行してから終了するコマンド | B | `agent_command(Sleep)`（経過時間で同期実行を観測） |

## フック一覧

| ハーネス | フック |
|---|---|
| `ConfigStoreHarness` | `put_config` / `remove_config` / `home_path` / `make_unreadable` |
| `WorkflowStoreHarness` | `put_named` / `put_named_with_ext` / `expected_path_for_name` / `put_at_absolute` / `put_at_relative` / `missing_absolute_path` / `make_unreadable` |
| `TaskRepositoryHarness` | `corrupt_whole_record` / `break_task_field` / `corrupt_snapshot` / `drop_snapshot_field` / `set_task_status_outside_snapshot` / `break_snapshot_invariant` / `place_in_both_areas` / `put_unnamed_entry` / `record_bytes` / `snapshot_bytes` / `make_unreadable` / `make_unwritable` / `concurrent_repo` |
| `ClockHarness` | `observe_wall_clock` / `advance` / `rewind` |
| `TaskIdGeneratorHarness` | `another_generator` |
| `ExclusiveLockHarness` | `hold_from_other_process` / `kill_holder` / `release_holder` / `try_acquire_from_other_process` / `separate_home` / `unusable_lock` |
| `WorktreeManagerHarness` | `repo_with_commit` / `repo_without_commit` / `detached_repo` / `non_repo_dir` / `missing_path` / `head_branch_name` / `absent_branch_name` / `failing_manager` / `unused_workspace` / `workspace_under_missing_root` / `workspace_with_orphan_branch` / `workspace_with_prunable_registration` / `workspace_over_plain_dir` / `workspace_over_other_branch` / `put_worktree_marker` / `worktree_marker` / `worktree_present` / `branch_tip` |
| `RunStoreHarness` | `expected_run_dir` / `attempt_dir_present` / `put_unreadable_content` / `make_unreadable` / `make_attempt_unwritable` / `concurrent_store` |
| `ProcessControllerHarness` | `observe_wall_clock` / `failing_identity_controller` / `worktree` / `missing_worktree` / `log_paths` / `unwritable_log_path` / `agent_command` / `missing_command` / `non_executable_command` / `launch_spec` / `wait_for_run_files` / `spawn_from_other_process` / `failing_controller` / `run_dir_is_empty` |

対象アクセサ（`fn store` / `fn repo` / `fn clock` / `fn generator` / `fn lock` / `fn manager`）はフックではなく、すべてのケースが使う。

フックの一覧はこのファイルが正本。`.adr/027-port-conformance-suite-and-harness-hooks.md` のフック表は決定時点の記録なので、フックを足すときに更新するのはこのファイルだけでよい。
