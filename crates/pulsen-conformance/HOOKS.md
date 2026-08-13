# 適合テストの行 × ハーネスのフック

`spec/testcases/ports/*.md` のうち、本スライスで扱う7ポート125行を、どう組み立てるかで分類した表。フックは spec の前提条件から導く（ADR-027）。行を足す・フックを足すときはこの表も更新する。

区分の意味は次のとおり。

| 区分 | 意味 |
|---|---|
| A | ポートのメソッドとドメイン型だけで組める（フック不要） |
| B | ハーネスのフックで組める |
| C | spec が「再現できるアダプター環境に限る」と明示する行。フックが `None` を返した環境ではスキップし、チェックリストにチェックを付けない |

区分は行ごとに1つで、C の行もフックを経由する（フックが `Some` を返す環境では実行される）。

| 区分 | 件数 |
|---|---|
| A | 28 |
| B | 85 |
| C | 12 |
| 合計 | 125 |

スキップは libtest の出力では成功と区別できないため、スイートを適用するテストファイルが「この環境でスキップを許容するケース」を集合として宣言する（`SkipBudget`）。集合の外のスキップはそのケースの失敗として現れる。集合は環境の能力から実行時に決める（`.adr/055-conformance-skip-budget.md`）。

## 環境で走らなくなりうる行

**何が前提を壊すかは行ごとに違う**。共通の述語 `permission_restrictions_effective` で導けるのは権限操作に依存する8行だけで、区分 C の残る4行はそれぞれ別の能力を要求する。区分 B にも、フックの前提が環境で成立しない行がある。宣言を組むときはこの表で読む。

右3列は3ランナーでの実測（下記「3ランナーでの実測」を参照）。`実行` は前提が成立してケースが走ったこと、`スキップ` は成立せず `SKIP` 行が出たこと、`未測定` はまだ CI で観測していないことを指す。現在の3列はすべて run 31666626824 の観測である。

**行を足すときは3列を `未測定` で埋める。** 実測は CI を回して初めて得られるので、行の追加と同時には書けない。空欄にしないのは、空欄だと「まだ測っていない」と「書き忘れた」が区別できないため。`未測定` を実測に置き換えるのは次に CI を回した人で、そのとき出典の run を下記の節に書き足す。左4列（区分・前提を作れない環境・判定）は設計なので、CI を待たずにその場で埋める。

| ID | 区分 | 前提を作れない環境 | 判定 | ubuntu | macOS | Windows |
|---|---|---|---|---|---|---|
| TC-port-config-store-023 | C | 読み取れないファイルを作れない（root 実行・非 POSIX・権限を持たないファイルシステム） | `permission_restrictions_effective` | 実行 | 実行 | スキップ |
| TC-port-workflow-store-030 | C | 同上 | `permission_restrictions_effective` | 実行 | 実行 | スキップ |
| TC-port-task-repository-005 / 011 / 012 / 035 | C | 書き込めないディレクトリを作れない（同上） | `permission_restrictions_effective` | 実行 | 実行 | スキップ |
| TC-port-task-repository-019 / 041 | C | 読み取れないディレクトリを作れない（同上） | `permission_restrictions_effective` | 実行 | 実行 | スキップ |
| TC-port-clock-003 | C | 実時刻を観測できない（時刻を注入するアダプター） | ハーネスが `observe_wall_clock` を提供するか | 実行 | 実行 | 実行 |
| TC-port-clock-005 | C | 時刻を過去へ巻き戻せない（実時計のアダプター） | ハーネスが `rewind` を提供するか | スキップ | スキップ | スキップ |
| TC-port-exclusive-lock-007 | C | ロック機構自体を利用不能にできない | ハーネスが `unusable_lock` を提供するか | 実行 | 実行 | 実行 |
| TC-port-worktree-manager-009 | C | 必ず失敗するハンドルか、コミットのあるリポジトリを用意できない | ハーネスが `failing_manager` / `repo_with_commit` / `head_branch_name` を提供するか | 実行 | 実行 | 実行 |
| TC-port-worktree-manager-003 | B | git リポジトリでない実在のディレクトリを作れない（一時ディレクトリの置き場自体がリポジトリ配下） | ハーネスが `non_repo_dir` を提供するか | 実行 | 実行 | 実行 |
| TC-port-exclusive-lock-002 / 003 / 004 / 005 | B | 別プロセスにロックを保持させる実行ファイルが無い（単一テストターゲットを指定した実行では example がビルドされない） | ハーネスが `hold_from_other_process` / `try_acquire_from_other_process` を提供するか | 実行 | 実行 | 実行 |

## 3ランナーでの実測

出典は GitHub Actions の run 31666626824（コミット `e524981`、全7ジョブ success）。`ubuntu-latest` / `macos-latest` / `windows-latest` の各ランナーで、非 root（unix ジョブは `id -u` が 0 でないことを直接アサートする）・ジョブのコンテナ指定なし・`cargo test --workspace --locked --no-fail-fast -- --nocapture` を実行し、`SKIP ` 行を採取したもの。**測定したのは `e524981`（Issue #10 の CI・その吸収・レビュー反映まで適用した時点）で、PR #11 が足す適合スイートと example は含まない。** 走ったテストバイナリは3 OS とも15本（`Running` 行を数えたもの。`test result:` 行は Doc-tests 3本を含めて18）で同数、スキップした分を除けば実行された適合ケースの数に OS 差は無い。

同じスキップの集合は先行する5つの緑の run（31657976822 = `af24360`、31661056619 = `2c99c1b`、31662960664 = `9675b2f`、31663925152 = `1766c7b`、31665658371 = `e19c973`）でも再現している。下の3列を書き換える理由になるのは集合が動いたときだけで、run を重ねること自体では動かない。

- **unix（ubuntu / macOS）で成立しなかったのは TC-port-clock-005 の1件だけ。** これは環境ではなくアダプターの性質による恒久スキップで（`SystemClockHarness` は実時計を巻き戻せないため `rewind` を提供しない）、どの OS でも走らない。
- **Windows で成立しなかったのは、上の1件に権限系を加えた計11件。** 適合行は表の `permission_restrictions_effective` を判定に持つ8行（TC-port-config-store-023 / TC-port-workflow-store-030 / TC-port-task-repository-005・011・012・019・035・041）で、残る2件は同じ述語を使う CLI 側の受け入れケース（TC-task-register-task-016 / 021）。
- **区分 C のうち TC-port-clock-003 / TC-port-exclusive-lock-007 / TC-port-worktree-manager-009 は3 OS すべてで走った。** いずれも判定がハーネス側のフック提供の有無で、実アダプターのハーネスが `observe_wall_clock` / `unusable_lock` / `failing_manager` を提供している。
- **区分 B の TC-port-worktree-manager-003 と TC-port-exclusive-lock-002 / 003 / 004 / 005 も3 OS すべてで走った。** 前者は一時ディレクトリの置き場が git リポジトリ配下にならず前提が成立したこと、後者は `--workspace` で example がビルドされ、ロックを保持する別プロセスが3 OS で機能したことを意味する。
- **適合スイートの外に、Windows では存在しないテストが3件ある。** `crates/pulsen/src/adapter/task_repository.rs` の `#[cfg(all(test, unix))] mod tests`（宙ぶらりんの symlink で作った「読めないエントリ」を「消えたエントリ」と取り違えないことを確認する3件）は Windows でコンパイルされず、**`SKIP` としても現れない**。`pulsen` の lib ユニットテストが Windows 69件・unix 72件になる差はこれである。上の「適合ケースの数に OS 差は無い」は適合ケースについての主張で、この3件はその外側にある Windows のカバレッジ差として記録しておく。

ログには実在の適合ケースと形の区別が付かない `SKIP tc_port_clock_004_時刻の前進` / `tc_port_clock_0051_別のケース` / `tc_port_clock_005_時刻の巻き戻し` の3行が全 OS で出る。これは `pulsen-conformance` の lib ユニットテストが `SkipBudget` 自身を検証するために `record` を直接呼ぶもので、架空のケース名を持つ。上の集計にも CI のジョブサマリーにも含めない — 走らなかった適合ケースとして数えられると、実測が示す内容が変わってしまうため。

この実測は PR #11 がスイートと example を足した時点で部分的に古くなる。その更新は #11 の責務とする。

## 適用範囲

TaskRepository / Clock / TaskIdGenerator / ExclusiveLock / WorktreeManager のフックは、破損・状況の**意味**だけを受け取る。この5ポートのスイートは、フックを実装するだけで別の実装（in-memory 等）にも適用できる。

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

## WorktreeManager（本スライス該当の9行 / A 0・B 8・C 1）

`create` / `remove` の12行は worktree スライスでポートにメソッドを足すときに扱う。

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

## フック一覧

| ハーネス | フック |
|---|---|
| `ConfigStoreHarness` | `put_config` / `remove_config` / `home_path` / `make_unreadable` |
| `WorkflowStoreHarness` | `put_named` / `put_named_with_ext` / `expected_path_for_name` / `put_at_absolute` / `put_at_relative` / `missing_absolute_path` / `make_unreadable` |
| `TaskRepositoryHarness` | `corrupt_whole_record` / `break_task_field` / `corrupt_snapshot` / `drop_snapshot_field` / `set_task_status_outside_snapshot` / `break_snapshot_invariant` / `place_in_both_areas` / `put_unnamed_entry` / `record_bytes` / `snapshot_bytes` / `make_unreadable` / `make_unwritable` / `concurrent_repo` |
| `ClockHarness` | `observe_wall_clock` / `advance` / `rewind` |
| `TaskIdGeneratorHarness` | `another_generator` |
| `ExclusiveLockHarness` | `hold_from_other_process` / `kill_holder` / `release_holder` / `try_acquire_from_other_process` / `separate_home` / `unusable_lock` |
| `WorktreeManagerHarness` | `repo_with_commit` / `repo_without_commit` / `detached_repo` / `non_repo_dir` / `missing_path` / `head_branch_name` / `absent_branch_name` / `failing_manager` |

対象アクセサ（`fn store` / `fn repo` / `fn clock` / `fn generator` / `fn lock` / `fn manager`）はフックではなく、すべてのケースが使う。

この一覧と `.adr/027-port-conformance-suite-and-harness-hooks.md` のフック表は同じものを指す。フックを足すときは両方を更新する。
