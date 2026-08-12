# Architecture / Spec-conformance

3周目（収束判定）。`.thread/1/review/triage.md` のラウンド1・2を先に読み、`wont-fix` 3件（W-004 `unreachable!` / W-012 「次回の tick で実行されます。」/ W-017 `create` の TOCTOU）は蒸し返していない。本ラウンドは「実装に残る本当の欠陥」と「2周目の修正が生んだ回帰」だけを見た。

**Blocker は0件。** Warning 3件はいずれも出荷物・正本ドキュメントの記述が実態と食い違う箇所で、実装の振る舞いを変えるものではない。

## 実行結果

| 確認 | 結果 |
|---|---|
| `cargo build` | 成功 |
| `cargo test` | 456件 / 0 failed / 0 ignored。2回実行して差分なし |
| `cargo clippy --all-targets -- -D warnings` | 警告0 |
| `cargo fmt --check` | 差分なし |
| `grep -rn "todo!\|unimplemented!\|FIXME\|TODO\|XXX" crates/` | 0件 |
| `grep -rn "cfg(unix)\|cfg(windows)" crates/*/src/` | 4件（`pulsen-conformance/src/lib.rs` の権限 probe 2件、`pulsen/src/util/atomic.rs` の `sync_dir` 2件）。`pulsen-domain` は0件 |
| `unwrap()` / `expect(` の本番経路 | 0件（`#[cfg(test)]` より前の行を全 `.rs` について走査） |
| 変更ファイル一覧 | `git diff main...HEAD --name-status` と `changed-files-003.txt` が完全一致（149件） |

## 受け入れ基準 AC-1〜AC-20 の検証

| AC | 判定 | 根拠 |
|---|---|---|
| AC-1 | 合格 | 上表のとおり。`crates/pulsen-domain/Cargo.toml` の `[dependencies]` は空（コメントで意図も固定）。OS 依存分岐は本番では `util/atomic.rs` のみ |
| AC-2 | 合格 | 名前系7種・`DurationSpec` / `TimeoutSpec` / `RawCommand` / `PlainCommand` / `CommandTemplate` / `SkillInputTemplate` / `RawAgentDefinition` / `AgentDefinition` が実装され、ユニット167件が全分岐を通す |
| AC-3 | 合格 | `workflow.rs:163-210` の `effective_*` が「ステータス上書き > ワークフローデフォルト > 既定」の順で解決。`reference.rs:8-10` が `POSIX_SEPARATORS` / `WINDOWS_SEPARATORS` を定数に切り出し、`parse_with_separators` にテストが両方を明示的に渡す |
| AC-4 | 合格 | `WorkflowParseError` は12バリアント（アダプター生成の `YamlSyntax` / `UnknownKey` + assembler 生成の10種）で spec/domains/definition.md:199-210 と一致。循環・自己参照・到達不能は `WorkflowDefinition::new` が受理する |
| AC-5 | 合格 | `RegistrationValidator::validate` は `errors` に積み続けて最後に返す。`RegistrationError` 5種。`push_once`（validator.rs:140）が畳むのは `status` を持たない2種の**値まで同一**な重複だけで、打ち切りではない |
| AC-6 | 合格 | `Task::register` / `Task::rehydrate` / `DegradedTask` が実装され、`rehydrate` は不変条件1を `RehydrateError::StatusNotInSnapshot` で返す。`Timestamp` は自前の暦計算で RFC3339 を往復する |
| AC-7 | 合格 | `task/port.rs` を spec/domains/task.md:265-312、`execution/port.rs` を spec/domains/execution.md:222-274 と逐条照合。`TaskRepository` 7メソッド・`TargetError` 5種・`LockError` 1種・`ConfigLoadError` 3種・`WorkflowLoadError` 3種が一致。未実装メソッドの宣言・スタブは0件（`execution/port.rs:3-5` が後続スライスで足すことを明記） |
| AC-8 | 合格 | `pulsen-conformance` が独立クレート（依存は `pulsen-domain` のみ）。ケース関数は125件ちょうどで、`spec/testcases/ports/` の125行と1:1。`HOOKS.md` の対応表は125行・区分集計 A 28 / B 85 / C 12 が実測と一致。原子性3ケースは `concurrent_repo` に隔離されている。ただし W-001 |
| AC-9 | 合格 | `conformance_config_store` 24件が緑 |
| AC-10 | 合格 | `conformance_workflow_store` 31件が緑 |
| AC-11 | 合格 | `conformance_task_repository` 44件が緑 |
| AC-12 | 合格 | clock 5 + task-id 5 + lock 7 + worktree 9 = 26件が実装され、`--nocapture` で SKIP になるのは `tc_port_clock_005` の1件だけ。**TC-004 は実走している**（`conformance_time_id.rs:40-44` の `advance` が 1.1 秒の実時間待ちを提供し `Some` を返す） |
| AC-13 | 合格 | `wire::resolve_home` が `--home` > `PULSEN_HOME`（空文字は未設定扱い・why 付き）> `env::home_dir()/.pulsen`。`compose` は結線前に `FsConfigStore::load` を行い、`NotFound` は `render::config_error` が「未初期化である旨」「解決後のホームパス」「作成すべき config.yaml のパス」の3点を出して非0で終了する |
| AC-14 | 合格 | `RegisterTask::execute` が ロック → `WorkflowRef::parse` → `load` → `display_name` → 対象検証 → 登録時検証 → ID発行 → `create` の順。`Conflict` は `retried` フラグで1回だけ再発行して再試行し、再衝突は `Create(Conflict)` として返す |
| AC-15 | 合格 | 拒否側35ケース（`cli_add_error.rs` 31 + `cli_add_boundary.rs` の TC-053/054/055/058）が `has_no_task()` と `untouched.assert_unchanged()` を通す |
| AC-16 | 合格 | `cli_add_boundary.rs` が TC-049〜067 の19件を実バイナリで検証 |
| AC-17 | 合格 | `task_file.rs` が人間可読な JSON を書き、`Task::register` の事後条件（`task_status = initial` / pending / カウンタ全0 / workspace・attempt・failure 未設定）を TC-009/010/011 が観測。`state/` 配下は `util::fsdir::ensure_dir` が自動作成 |
| AC-18 | 合格 | TC-012 / 018 / 040 / 047 / 048 の5件が `tests/register_task.rs`（テストダブルのみ・I/O なし）に、残る62件が CLI 3ファイルに割れており、67件が漏れなく1回ずつ存在する |
| AC-19 | 合格 | アトミック置換は `util/atomic.rs` の2関数のみ、排他ロックは `adapter/lock.rs` のみ。アダプター側に `fs::rename` / `NamedTempFile` / `try_lock` の直接使用なし |
| AC-20 | 合格 | 下記「チェックリスト記帳の検証」 |

## チェックリスト記帳の検証（重点）

`gh issue view 1` の実測: `- [x]` **345件** / `- [ ]` **1件**（`TC-port-clock-005`）/ 合計 346件。plan.md の基準どおり。

**「走ったか」の裏取り**（記帳の前提条件）:

- `cargo test -- --nocapture | grep -i skip` の出力は4行。うち3行は `pulsen-conformance/src/lib.rs:763-777` の `SkipBudget` 自身のユニットテスト（`tc_port_clock_004_時刻の前進` / `tc_port_clock_0051_別のケース` は実在しないケース名を使った接頭辞照合の検証）で、実スイートのスキップは `tc_port_clock_005_巻き戻した時刻はそのまま返る` の1件だけ。**チェックが付いた345行のうち、走らなかったものは無い。**
- 実行環境は uid 501（非 root）・`TMPDIR=/var/folders/…/T/`（リポジトリ外）で、Issue コメントの記載と一致する。権限操作系8件・`TC-task-register-task-016/017/021/036` はいずれも実走している。

**サンプリング検証**（30行以上）:

| 群 | 件数 | 検証方法 |
|---|---|---|
| `TC-port-*` | 125 | `grep -ohE "^pub fn tc_port_[a-z_0-9]+" crates/pulsen-conformance/src/*.rs` で TC ID を抽出 → 125個・ポート別内訳（clock 5 / config-store 24 / exclusive-lock 7 / task-id-generator 5 / task-repository 44 / workflow-store 31 / worktree-manager 9）がチェックリストの行と1:1。全件が `cargo test` で緑 |
| `TC-task-register-task-*` | 67 | `fn tc_task_register_task_NNN` を4ファイルから抽出 → 001〜067 が欠番・重複なし（normal 12 / error 31 / boundary 19 / usecase 5）|
| `DOM-definition-*` / `DOM-task-*` / `DOM-execution-*` | 34件を抽出サンプル | `InputText` / `placeholders` / `expand` / `render_input` / `build_command_line` / `SkillInputTemplate` / `effective_agent`〜`effective_retry_limit` / `display_name` / `WorkflowSnapshot::rehydrate` / `RegistrationValidator` / `RegistrationError` / `RawCommandDoc` / `elapsed_since` / `AttemptNumber` / `RunDirPath` / `StartTimeRecord` / `StopReason` / `DegradedTask` / `TaskRepository::archive` / `branch_exists` / `head_branch` / `validate_repo` / `Target` / `ExecutionState::kind` / `RetryCounters` / `FailureNote` / `FailureKind` / `ProcessIdent` / `AttemptRef` / `ExecutionStateKind` / `WorkflowAssembler` ほかを実体まで辿り、**すべて実装が存在**。空実装・常に成功を返す関数は無い |
| `ADP-*` | 15 | `TaskRepository` 7メソッド + config/workflow/taskid/clock/lock/worktree 3メソッド = 15 がすべてアダプターで実装され、対応する適合スイートが緑 |
| `PAGE-common-007` / `PAGE-common-011` | 2 | 「提供しない」という否定的主張が `tests/cli_usage.rs:52-72` で観測可能な形（サブコマンド集合 = `add` + clap の `help`、`--json` / `--format` / `--output` の不在）に固定されている |

**台帳の網羅性**: `steps.md:422-455` の対応表を件数で検算すると、DOM-definition 56 + DOM-task 52 + DOM-execution 7 + ADP 15 + UC 3 + PAGE-common 11 + PAGE-add 10 + TC-task-register-task 67 + TC-port 125 = **346**。Issue のチェックリスト行数と一致し、どのステップにも割り当てられていない行は無い。

**スタブ・仮実装**: `todo!` / `unimplemented!` / `TODO` / `FIXME` / `XXX` は0件。スコープ外要素（`RunStore` / `ProcessController` / `CommandRunner` / 各分類サービス / `WorkspacePlanner` / `WorktreeManager::create` / `remove` / `Task` の register・rehydrate 以外の遷移）は宣言すら存在しない（`grep` で0件）。`DegradedTask::execution_kind` は台帳 DOM-task-060（「`execution_kind` 等スナップショット非依存の各フィールド参照」）が名指ししている行なのでスコープ内。

## `.adr/` と索引の検証

- **索引の双方向一致**: `.thread/1/adr.md` の Status 行が挙げる `.adr/` ファイル名は全件実在し（欠損0）、逆に `.adr/019` 以降の38ファイルはすべて `adr.md` から参照されている（未参照0）。Status にファイル名を持たないエントリは ADR-041 / 047 の2件だけで、これは `adr.md:5` が「既存 `.adr/` エントリに反映済み」と明記している。1周目 W-028 が成立させた「Status 行 = 起票済みの索引」は破れていない。
- **書式**: `.adr/` 全56ファイル（既存18 + 本スライス38）が `## ステータス` / `承認済み` で統一（ADR-038 のとおり）。`Proposed` のまま残るエントリは無い。
- **決定と実装の一致**（2周目に新設・更新された分を重点）:
  - ADR-023: `std::env::home_dir()` の非推奨解除を「1.87.0」と版数で確定させ、MSRV 1.89（ADR-022 の `File::try_lock` 由来）がそれより後であることを示した。実際に 1.87.0 で解除されており記述は正しい。未検証（MSRV での実ビルド）は `progress.md:52` に残作業として記録済み
  - ADR-024: `validate_repo` の5段（`try_exists` の `Err` → `Failed` を含む）と「`Failed` は対象の分類を確定できない状況に限る」が `adapter/worktree.rs` の実装・`INHERITED_GIT_ENV` の7変数と一致
  - ADR-027: フック表の42フック名が `lib.rs` のトレイトメソッドと1文字違わず一致。ダングリングなし
  - ADR-030: 「`base_dir` は絶対パス」の前提が追記され、合成ルート（`env::current_dir()`）が満たしている
  - ADR-050: ワークフロー定義のスキーマ違反を ADR-054 の例外として明記し、`progress.md:9` と Issue コメント7件目と範囲が揃っている
  - ADR-055 / 060 / 061 / 062: それぞれ `SkipBudget` の集合 + probe、ロック TC-003 の別スレッド化、`worktree_root()` を残す基準、`detached_home()` と実装が一致
- **既存 ADR-001〜018 との矛盾**: なし。ADR-017 の依存方向（definition ← task ← execution）は `grep` で確認（`definition/` から他ドメインへの `use` は0件、`task/` → `definition/`、`execution/port.rs` → `task/` のみ）。ADR-010（循環許容）・ADR-013（未知キー拒否）・ADR-015（スナップショット埋め込み）はいずれも実装と整合する。

## 依存方向・合成ルート・スコープ

- `crate::adapter` を import するのは `cli/wire.rs` の1箇所のみ（`grep -rl` で確認）。`application/` からアダプターへの依存は0件、`adapter/` `util/` から `application` / `cli` への逆流も0件。
- `cli/mod.rs:3-4` の doc（「アダプターへの依存は `wire` の1箇所に閉じ」）が実体と一致する（1周目 W-005 の修正が保たれている）。
- plan.md「含まれないもの」の混入は0件。CI 設定（`.github/`）も無い。

## `.thread/1/` ドキュメントと実装の一致

- `progress.md:18` の「TC-port-clock-005 の1件を除き全件が実行された」は実測と一致。:28 の「CLI 4件はこの環境では実走するため、許容集合は空」も `common/mod.rs:41-53` の述語と実行結果に一致。
- `testing.md:44-50` の AC-1 grep 手順は `crates/*/src/` に限定され、期待（`pulsen-domain` 0件・`pulsen/src` は `util/atomic.rs` だけ）が実測と一致（2周目 R2-W-008 の修正が効いている）。:180 の手動手順3 は `HOME="$FAKEHOME"` を前置きしており、実 `~/.pulsen/` に触れない。
- `steps.md:57` の `InputText` の記述は実装（`new`）と一致。:418 の拒否側の要件は AC-15 と同じ2条件になっている。:458 の PAGE-common 系6行の注記が plan.md の基準と揃っている。
- `HOOKS.md` に `.thread/` 参照は0件、`break_lock_location` のような現存しない識別子も0件（2周目 R2-W-021 の修正が効いている）。

## Issue #1 のコメント（10件）の検証

10件すべてを読み、事実関係を照合した。

- 記帳コメントのテスト件数表（14バイナリ・合計456）は `cargo test` の実測と**バイナリ単位まで完全一致**（60 / 21 / 31 / 12 / 5 / 24 / 7 / 44 / 10 / 31 / 9 / 22 / 13 / 167）。適合スイート6バイナリの合計125も一致。
- 実行環境の記載（macOS・非 root・`TMPDIR` はリポジトリ外）は `id -u` = 501、`TMPDIR=/var/folders/…` で裏が取れる。
- 「SKIP の報告が `tc_port_clock_005` の1件だけ」は実測と一致。
- spec 追従の提起5件（ADR-025 / ADR-050 / ADR-051 / `InputText` / エージェント単位のエラー重複）は、いずれも該当コードの現状と一致しており、提起の前提（`Path::new(".yaml").file_stem()` が `Some(".yaml")` を返す、`load` の失敗時にユースケースが解決先パスを持たない等）も正しい。
- PAGE-common 系6行の扱いのコメントは、基準が plan.md:56 / steps.md:458 と同一の文で書かれており三者が揃っている。

## コメントの質（CLAUDE.md）

`crates/` 配下の全 `.rs` を対象に「指摘 / レビュー / 1周目 / 2周目 / ラウンド / 以前は / もともと / 修正した / 変更した / 当初」を grep して**0件**。`HOOKS.md` も0件。2ラウンドの修正を経ても、コード・テスト・出荷ドキュメントに経緯は漏れていない。`.adr/` の「当初は〜だった」（024・027）は ADR の Context として適切な用法。

## Blockers

なし。

## Warnings

- **[W-001]** `HOOKS.md` が「区分 C の行は権限制限が効くかどうかで走るか走らないかが変わる」と書くが、12行中4行はそうではない。この文どおりに許容集合を組むと、その4行のうち `TC-port-clock-005` は失敗する
  - 場所: `crates/pulsen-conformance/HOOKS.md:22` ／ 反例は `crates/pulsen/tests/conformance_time_id.rs:77-83`・`crates/pulsen/tests/conformance_lock.rs:99`・`crates/pulsen/tests/conformance_worktree.rs:111`
  - 理由: 区分 C の12行のうち `permission_restrictions_effective` で決まるのは8行（config-store-023 / workflow-store-030 / task-repository-005・011・012・019・035・041）だけ。残る4行は前提の性質が違う — `clock-003` / `clock-005` は時計の観測・巻き戻し、`exclusive-lock-007` はロックパスの占有、`worktree-manager-009` は存在しないパスで構築した manager で、権限とは無関係である。実際に出荷しているハーネスも文どおりには書けておらず、`conformance_time_id.rs:81` は probe を参照せず `vec!["tc_port_clock_005"]` を**無条件に**宣言し、`conformance_lock.rs:99` / `conformance_worktree.rs:111` は `Vec::new()` を渡している（`lib.rs:180-182` の `SkipBudget` doc は「環境の能力から導くと〜対応する」という条件付きの書き方なのでこの誤りは無い。直すのは `HOOKS.md` の側）。`HOOKS.md` は「後続スライスのアダプターが自分の `allowed_skips` を組むときに読む正本」（ADR-055 / AC-8）なので、この文に従って clock-005 を probe から導くと、probe が真になる環境（=通常の開発環境）で必ず失敗する。同じ過大な一般化は `conformance_worktree.rs` が `TC-port-worktree-manager-003`（TMPDIR がリポジトリ配下ならスキップ）を `Vec::new()` のまま置いていることにも現れており、CLI 側の同一前提（`common/mod.rs:34-35,49-51` の `OUTSIDE_REPOSITORY_CASES`）と扱いが割れている。
  - 提案: `HOOKS.md:22` を「区分 C の行がどの環境で走らなくなるかは行ごとに違う。権限制限（`permission_restrictions_effective`）で決まるのは8行で、残る4行（clock-003 / clock-005 / exclusive-lock-007 / worktree-manager-009）は各ハーネスが自分で判定する」に直し、区分 C の表に「何が前提を壊すか」の列を足す。あわせて `HOOKS.md:192` の TC-port-worktree-manager-009 が `failing_manager` しか挙げていない点（実際は `repo_with_commit` / `head_branch_name` も `require!` する。`worktree_manager.rs:100-102`）を他の行と同じ粒度に揃える。

- **[W-002]** `.adr/` の連番が 055 → 060 と4番飛んでおり、欠番の理由がどこにも記録されていない
  - 場所: `.adr/`（056〜059 が不在）／ `.thread/1/adr.md:5`
  - 理由: ADR-035 は本スライスの ADR を「`.adr/` の続き番号」として採番すると決め、ADR-038 は「作業ログのエントリが昇格済みかどうかは Status 行に `.adr/` のファイル名があるかで判別できる状態に保つ」と定めている。`adr.md:5` は実際に「ADR-041 / 047 は既存の `.adr/` エントリに本文を反映済みで単独のファイルを持たない」と**番号の例外を明示的に説明している**。ところが 056〜059 は `adr.md` にエントリすら無く（`grep` で `crates/` `.adr/` `.thread/` のどこにも参照が無い）、説明も無い。`.adr/` だけを見る後続スライスの担当は、4件が削除されたのか最初から使われなかったのかを判別できず、次に起票するときの番号も決められない。索引の整合を自ら明文化している以上、ここだけ黙って空くのは規約の破れである。
  - 提案: `.thread/1/adr.md:5` に1文足す（「056〜059 は採番されなかった（欠番）」等）。あるいは 060〜062 を 056〜058 に詰めて連番を回復する。どちらでもよいが、`.adr/` の読み手が欠番を「失われた決定」と誤読しない状態にすること。

- **[W-003]** 2周目に新設した `WorkflowStructureError::describe()` の文言が、`cli/render.rs` の `WorkflowParseError` の文言と重複しており、同じ制約の説明が2層に別々の定義箇所を持っている
  - 場所: `crates/pulsen-domain/src/definition/workflow.rs:60-70` と `crates/pulsen/src/cli/render.rs:195-197, 214-216`
  - 理由: `describe()` は「initial が指すステータス \`X\` が statuses にありません」「ステータス \`S\` の next が指す \`N\` が statuses にありません」を返し、`render.rs` は同じ2文（末尾の `。` だけが違う）を独立に組み立てている。前者は `adapter/task_file.rs:603` 経由で `SnapshotUnreadable` の理由として、後者は登録時の案内として、**どちらも利用者に見える**。文言を片方だけ直すと、同じ「statuses に無い参照」という壊れ方が、登録時と破損タスクの表示とで違う言葉で説明される。1周目 W-002（`NameError` の説明5文が2層に同一文字列で存在する）と2周目 R2-W-009（`describe()` の doc が主張する一元化が `render.rs` で成立していない）で2度同じ判断を下しており、`describe()` の doc 自身（workflow.rs:57-59）も「説明の定義箇所をドメインに1つ置く」と書いている。エラー型が違う（`WorkflowStructureError` と `WorkflowParseError`）ため型の上では重複ではないが、利用者から見た主張は同一である。
  - 提案: `render.rs` の `InitialNotFound` / `NextNotFound` の2分岐を、対応する `WorkflowStructureError` の `describe()` を呼ぶ形にするか（`WorkflowParseError` からの写像は `assembler.rs:411-424` に既にある）、そうしない理由（層ごとに文体を変える等）を `describe()` の doc に why として残す。

## カバレッジ

一覧149件と1対1で対応する。

### 確認（全文または該当箇所を読んだ・47件）

- ADR（8件）: `.adr/023-dependency-selection.md`, `.adr/024-git-cli-shell-out-and-target-classification.md`, `.adr/027-port-conformance-suite-and-harness-hooks.md`, `.adr/030-workflow-store-base-dir-injection.md`, `.adr/038-adr-filing-format.md`, `.adr/046-no-skippable-hooks-for-post-operation-observation.md`, `.adr/050-schema-error-location-is-logical.md`, `.adr/053-conformance-yaml-source-hooks.md`
- 作業ログ（6件）: `.thread/1/adr.md`, `.thread/1/plan.md`, `.thread/1/progress.md`, `.thread/1/steps.md`, `.thread/1/testing.md`, `.thread/1/review/triage.md`
- ビルド・依存・環境（6件）: `Cargo.toml`, `crates/pulsen/Cargo.toml`, `crates/pulsen-conformance/Cargo.toml`, `crates/pulsen-domain/Cargo.toml`, `flake.nix`, `rustfmt.toml`
- 適合スイート（3件）: `crates/pulsen-conformance/HOOKS.md`, `crates/pulsen-conformance/src/lib.rs`, `crates/pulsen-conformance/src/clock.rs`
- CLI・エントリポイント（7件）: `crates/pulsen/src/lib.rs`, `crates/pulsen/src/main.rs`, `crates/pulsen/src/cli/mod.rs`, `crates/pulsen/src/cli/args.rs`, `crates/pulsen/src/cli/exit.rs`, `crates/pulsen/src/cli/add.rs`, `crates/pulsen/src/cli/wire.rs`
- アプリケーション層（3件）: `crates/pulsen/src/application/mod.rs`, `crates/pulsen/src/application/home.rs`, `crates/pulsen/src/application/register_task.rs`
- アダプター（3件）: `crates/pulsen/src/adapter/mod.rs`, `crates/pulsen/src/adapter/task_repository.rs`, `crates/pulsen/src/adapter/yaml.rs`
- 共通ユーティリティ（2件）: `crates/pulsen/src/util/mod.rs`, `crates/pulsen/src/util/atomic.rs`
- ドメイン（6件）: `crates/pulsen-domain/src/task/port.rs`, `crates/pulsen-domain/src/execution/port.rs`, `crates/pulsen-domain/src/definition/reference.rs`, `crates/pulsen-domain/src/definition/workflow.rs`, `crates/pulsen-domain/src/definition/validator.rs`, `crates/pulsen-domain/src/definition/assembler.rs`
- テスト（3件）: `crates/pulsen/tests/cli_usage.rs`, `crates/pulsen/tests/common/mod.rs`, `crates/pulsen/tests/conformance_time_id.rs`

### 確認（機械的検証のみ・88件）

本観点で問うのは「台帳行に対応する実体があるか」「宣言と実態が一致するか」なので、次の各件は個別の実装レビュー（他観点の担当）ではなく、TC ID / 型 / 関数の存在と数の照合、`grep` による規約違反の走査（`todo!` 系・`cfg(unix)`・`unwrap` / `expect`・経緯コメント・逆方向の `use`）、`cargo test` の実行結果で確認した。

- ADR の残り（30件）: `.adr/019-domain-crate-workspace.md`, `.adr/020-no-serde-in-domain-timestamp-conversion-in-domain.md`, `.adr/021-yaml-value-then-hand-written-schema-walk.md`, `.adr/022-std-file-lock-and-lockguard-marker-trait.md`, `.adr/025-task-file-json-and-corrupt-classification.md`, `.adr/026-task-id-format.md`, `.adr/028-usecase-error-paths-via-test-doubles.md`, `.adr/029-wildcard-enum-match-arm-lint-domain-only.md`, `.adr/031-pulsen-home-layout-in-application-layer.md`, `.adr/032-lock-holder-example-fixture.md`, `.adr/033-git-fixture-reproducibility.md`, `.adr/034-workflow-ref-separator-set-as-constant.md`, `.adr/035-file-slice-adrs-from-019.md`, `.adr/036-infallible-ports-absorb-failure-at-construction.md`, `.adr/037-platform-separator-set-without-cfg.md`, `.adr/039-read-error-shared-by-find-and-list.md`, `.adr/040-rehydrate-takes-field-bundle.md`, `.adr/042-absent-yaml-value-is-omission.md`, `.adr/043-store-adapters-receive-injected-paths.md`, `.adr/044-task-file-layout-in-domain.md`, `.adr/045-task-file-dto-generic-over-snapshot.md`, `.adr/048-parse-inputs-at-spec-flow-position.md`, `.adr/049-base-flag-allows-hyphen-values.md`, `.adr/051-undisplayable-name-fixture-is-whitespace-stem.md`, `.adr/052-acceptance-test-harness-in-tests-common.md`, `.adr/054-workflow-error-file-path-goes-into-free-form-messages.md`, `.adr/055-conformance-skip-budget.md`, `.adr/060-non-blocking-lock-case-observes-from-a-second-thread.md`, `.adr/061-unused-public-accessors-are-kept-only-for-verified-layout.md`, `.adr/062-acceptance-tests-detach-the-user-home.md` — Status の語・見出し書式・`adr.md` との索引双方向一致を全件照合
- 適合スイートの残り（14件）: `crates/pulsen-conformance/src/config_store.rs`, `crates/pulsen-conformance/src/exclusive_lock.rs`, `crates/pulsen-conformance/src/task_id_generator.rs`, `crates/pulsen-conformance/src/task_repository.rs`, `crates/pulsen-conformance/src/workflow_store.rs`, `crates/pulsen-conformance/src/worktree_manager.rs`, `crates/pulsen-conformance/src/doubles/mod.rs`, `crates/pulsen-conformance/src/doubles/clock.rs`, `crates/pulsen-conformance/src/doubles/lock.rs`, `crates/pulsen-conformance/src/doubles/stores.rs`, `crates/pulsen-conformance/src/doubles/task_id.rs`, `crates/pulsen-conformance/src/doubles/task_repository.rs`, `crates/pulsen-conformance/src/doubles/tests.rs`, `crates/pulsen-conformance/src/doubles/worktree.rs` — TC ID の抽出（125件・ポート別内訳）と `HOOKS.md` の対応表との照合
- アダプターの残り（7件）: `crates/pulsen/src/adapter/clock.rs`, `crates/pulsen/src/adapter/config_store.rs`, `crates/pulsen/src/adapter/lock.rs`, `crates/pulsen/src/adapter/task_file.rs`, `crates/pulsen/src/adapter/task_id.rs`, `crates/pulsen/src/adapter/workflow_store.rs`, `crates/pulsen/src/adapter/worktree.rs` — ADP-* 15行の実体、`{error:?}` の不在、`describe()` の呼び出し元、`fs::rename` / `NamedTempFile` / `try_lock` の非重複
- CLI・ユーティリティ（2件）: `crates/pulsen/src/cli/render.rs`（W-003 の箇所と `use crate::adapter` の不在、`#[cfg(test)]` の有無を確認）, `crates/pulsen/src/util/fsdir.rs`
- ドメインの残り（23件）: `crates/pulsen-domain/src/lib.rs`, `crates/pulsen-domain/src/definition/agent.rs`, `crates/pulsen-domain/src/definition/command.rs`, `crates/pulsen-domain/src/definition/config.rs`, `crates/pulsen-domain/src/definition/duration.rs`, `crates/pulsen-domain/src/definition/mod.rs`, `crates/pulsen-domain/src/definition/name.rs`, `crates/pulsen-domain/src/definition/port.rs`, `crates/pulsen-domain/src/definition/snapshot.rs`, `crates/pulsen-domain/src/definition/template.rs`, `crates/pulsen-domain/src/execution/mod.rs`, `crates/pulsen-domain/src/task/attempt.rs`, `crates/pulsen-domain/src/task/branch.rs`, `crates/pulsen-domain/src/task/counters.rs`, `crates/pulsen-domain/src/task/degraded.rs`, `crates/pulsen-domain/src/task/failure.rs`, `crates/pulsen-domain/src/task/id.rs`, `crates/pulsen-domain/src/task/mod.rs`, `crates/pulsen-domain/src/task/path.rs`, `crates/pulsen-domain/src/task/process.rs`, `crates/pulsen-domain/src/task/state.rs`, `crates/pulsen-domain/src/task/task.rs`, `crates/pulsen-domain/src/task/time.rs` — DOM-* の台帳行に対応する型・関数の実在、ドメイン間の依存方向、外部クレートの不使用
- テストの残り（11件）: `crates/pulsen/tests/cli_add_normal.rs`, `crates/pulsen/tests/cli_add_error.rs`, `crates/pulsen/tests/cli_add_boundary.rs`, `crates/pulsen/tests/common/git.rs`, `crates/pulsen/tests/common/lock.rs`, `crates/pulsen/tests/conformance_config_store.rs`, `crates/pulsen/tests/conformance_lock.rs`, `crates/pulsen/tests/conformance_task_repository.rs`, `crates/pulsen/tests/conformance_workflow_store.rs`, `crates/pulsen/tests/conformance_worktree.rs`, `crates/pulsen/tests/register_task.rs` — TC ID の割り当て（62 + 5 = 67）、無言スキップの不在、`SkipBudget` の宣言と実行結果の一致
- フィクスチャ（1件）: `crates/pulsen/examples/lock_holder.rs` — ADR-032 が定める役割と `common/lock.rs` からの参照の存在

### スキップ（14件）

- `Cargo.lock`（1件） — 生成物。依存の選定は `Cargo.toml` と ADR-023 で確認した
- `.thread/1/review/changed-files-001.txt`, `.thread/1/review/changed-files-002.txt`（2件） — 過去ラウンドの入力。本ラウンドの対象は `changed-files-003.txt` で、`git diff` との完全一致を確認済み
- `.thread/1/review/review-001.md`, `.thread/1/review/review-001-adapter.md`, `.thread/1/review/review-001-arch-spec.md`, `.thread/1/review/review-001-domain.md`, `.thread/1/review/review-001-test.md`, `.thread/1/review/review-001-usecase-cli.md`（6件） — 1周目のレビュー成果物。指摘の判定は `triage.md` に統合済みで、そちらを一次資料として読んだ
- `.thread/1/review/review-002-adapter.md`, `.thread/1/review/review-002-arch-spec.md`, `.thread/1/review/review-002-domain.md`, `.thread/1/review/review-002-test.md`, `.thread/1/review/review-002-usecase-cli.md`（5件） — 同上（2周目）

## 総括

**Blocker 0件。** 受け入れ基準 AC-1〜AC-20 はすべて合格し、Issue #1 の記帳（345/346）は実行結果と1行ずつ突き合わせても矛盾がない。スタブ・仮実装・経緯コメントは0件、依存方向と合成ルートは設計どおり、`.adr/` と `adr.md` の索引は双方向に一致している。2周目の修正が生んだ実装上の回帰は見つからなかった。

残る3件はいずれも**記述と実態のずれ**で、W-001 は後続スライスの担当を誤らせる出荷ドキュメントの過大な一般化、W-002 は正本の連番の欠番、W-003 は2周目に新設した説明文の二重定義である。実装の振る舞いはどれも変わらない。
