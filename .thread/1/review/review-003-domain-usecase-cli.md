# Domain / Use Case / CLI

3周目(収束判定)。ラウンド1・2の判定台帳(`.thread/1/review/triage.md`)を先に読み、`wont-fix` の3件(W-004 `unreachable!` / W-012 「次回の tick で実行されます。」/ W-017 `create` の TOCTOU)は蒸し返していない。本ラウンドは「実装に残る本当の欠陥」と「2周目の修正が生んだ回帰」だけを対象とした。

**結論: 問題点ゼロ。** 本観点で報告すべき Blocker・Warning はない。

## Blockers

なし

## Warnings

なし

## 検証したこと

### 受け入れ基準(本観点に関わるもの)

| 基準 | 結果 | 確認方法 |
|---|---|---|
| AC-1(ビルド・テスト・lint・ドメインの外部クレート非依存) | 合格 | `cargo fmt --check` 差分なし / `cargo clippy --all-targets -- -D warnings` 警告なし / `cargo test` 全スイート緑(ユニット167件を含め 0 failed)。`crates/pulsen-domain/Cargo.toml` の `[dependencies]` は空 |
| AC-2 / AC-3(値オブジェクトと `effective_*` / `display_name`) | 合格 | `parse` 以外の生成口がないこと、`effective_*` の優先順位、`display_name` の4規則と区切り文字集合(`POSIX_SEPARATORS` / `WINDOWS_SEPARATORS` の両方をテストが明示的に渡す)を確認 |
| AC-4(`WorkflowParseError` 10種) | 合格 | `assemble` の全分岐と、`InvalidValue` の `location` 12箇所がテストで1件ずつ固定されていること(生成箇所も `grep` で 12 と一致)を確認。循環・自己参照・到達不能は受理 |
| AC-5(登録時検証を全件まとめて返す) | 合格 | 下記「2周目の修正の検証」参照 |
| AC-6(`Task::register` / `rehydrate` / `ExecutionState` 6状態 / `Timestamp` 往復) | 合格 | `rehydrate` は不変条件1のみ検証し、2〜4は遷移関数へ委ねる契約どおり。RFC3339 は自前の days-from-civil / civil-from-days で往復し、表現可能範囲を 0001〜9999 に閉じて `to_rfc3339` の出力が常に `parse_rfc3339` に受理される形になっている |
| AC-7(ポート表との1:1一致) | 合格 | `definition/port.rs`・`task/port.rs`・`execution/port.rs` を spec のポート表(definition.md:282-311 / task.md:265-312 / execution.md の WorktreeManager・ExclusiveLock)と逐条照合。メソッド名・引数・戻り値・エラー種が一致し、未実装メソッドの宣言・スタブは0件 |
| AC-13 / AC-14(ホーム解決の3段優先順位と `add` の処理順) | 合格 | `wire::resolve_home` が `--home` > `PULSEN_HOME`(空文字は未設定扱い、why 付き) > `~/.pulsen`、`compose` が起動時に `ConfigStore::load`。`RegisterTask::execute` は ロック → `WorkflowRef::parse` → `load` → `display_name` → 対象検証 → 登録時検証 → ID発行 → `create` の順で、`Conflict` は1回だけ再発行して再試行する |
| AC-15(拒否時にタスクが作られない・ロックが解放される) | 合格 | 失敗経路はすべて `create` の前に `?` で戻り、`RegisterTaskError` のどの分岐からもタスク生成に到達しない。ロックは `let _guard = ...`(`let _ = ...` ではない)で関数末尾まで保持され、早期 return でもドロップで解放される |

### ヘキサゴナル・関数型ドメインモデリング

- アダプター型の import は `cli/wire.rs` の1箇所だけ(`grep crate::adapter` で `application/` と他の `cli/*` は0件)。`cli/mod.rs` の doc の主張と実体が一致している。
- ユースケースはポートをジェネリック引数で受け、判断は `WorkflowRef` / `RegistrationValidator` / `Task::register` に委ねている。分類・判定ロジックの漏れ出しはない。
- `unwrap` / `panic!` / `todo!` は本番経路に0件(唯一の `unreachable!` は `template.rs:198`、1周目 W-004 で `wont-fix` 判定済み)。`match` のワイルドカードは enum に対しては0件で、残る `_ =>` は文字列・数値・単位文字に対するもの(`duration.rs:50` / `template.rs:35` / `time.rs:194` / `assembler.rs:321`)。ドメインクレートは `wildcard_enum_match_arm = "warn"` を `-D warnings` 下で通している。
- `#[allow(...)]` は2件(`task/port.rs:93` `large_enum_variant`、`task/mod.rs:18` `module_inception`)で、どちらも why が付いている。TODO / FIXME / 仮実装は0件。

### 2周目の修正の検証(重点)

- **`definition/validator.rs` の `push_once`** — spec(definition.md:270「エラーは全ステータス分をまとめて返す(最初の1件で打ち切らない)」)と矛盾しない。畳むのは `UnknownAgent` / `InvalidAgentDefinition` の**値まで完全に同一**な重複だけで、この2種は spec:274-280 で `status` を持たないエージェント単位の誤りなので、情報量は落ちない。打ち切りではないことは `tests/register_task.rs:693` が3件(2ステータスにまたがる `MissingSkillInput` + `MissingModel` × 2)で、ドメイン側も `検証エラーは最初の1件で打ち切らず全件返る` / `別々の未定義エージェントはそれぞれ返る` で押さえている。`errors.contains` の線形探索は、要素数がワークフロー定義のステータス数(数十)で頭打ちになるため仕様上も実装上も妥当。
- **`definition/assembler.rs` の `ForbiddenKey`** — 平行配列を解消し、キー名と述語を組で並べる形になった。テストは6キーすべてをループで回し、`cleanup` 側も別テストで押さえている。`InvalidValue` の `location` は生成箇所12件と検証12件が1対1。
- **`definition/workflow.rs` の `effective_*` doc** — 説明が definition ドメイン内で閉じ(「引数のステータス名がこの定義に属することは呼び出し側の責務」)、task ドメインの不変条件への言及は補足に留まった。spec:165-168 のシグネチャは維持され、AC-7 を壊していない。
- **`definition/snapshot.rs`** — `Wait` に対する `effective_*` の期待は消え、`AgentRun`(`timeout: none` / `retries: 0` / 既定の agent・model)への委譲で置き換わった。spec が「規定しない」と書く振る舞いを固定するテストは残っていない。
- **`definition/name.rs` の `describe()` doc と `cli/render.rs`** — `render.rs` に `name_error` は残っておらず、`NameError` / `BranchNameError` の文言はすべて `describe()` を文中に埋め込む形になった。doc が主張する「制約の言葉は表示側が持たない」が実体と一致している。
- **`cli/render.rs` の単体テスト17件** — 文言の全文 pin は過剰ではないと判断する。(a) この層の責務が「エラー値 → 表示テキスト」そのものであり、pin する対象と責務が一致している。(b) 実アダプターでは作れない `WireError` 6分岐・`Target::Failed`・`LockFailed`・`Create` 2分岐・`InvalidRepoPath` を net として押さえており、1周目 W-035 の劣化が起きた無テスト領域が塞がっている。(c) ドメイン由来の文言は `NameError::Empty.describe()` のように**式として**参照しており、文字列を二重定義していない(一元化の主張と整合)。受け入れテスト側は `contains` で要点だけを見ており、粒度が二重化しているわけでもない。
- **`application/home.rs`** — `worktree_root()` に why が付き、モジュール doc に「未使用の `pub` を残す基準」が書かれた(`.adr/061` と整合)。1周目 W-011 で `Runtime::home()` を落とした基準との割れは解消している。
- **`adapter/task_file.rs` の `describe()` 置換(本観点からの裏取り)** — 新設された `describe()`(`TaskIdError` / `BranchNameError` / `TimestampError` / `AttemptNumberError` / `ProcessValueError` / `FailureNoteError` / `AbsolutePathError`)はすべて呼び出し元があり(`task_file.rs` 27箇所・`render.rs` 6箇所・`assembler.rs` 3箇所・`config_store.rs` 4箇所)、「使われないドメイン API を足した」形にはなっていない。

### spec との適合(本観点)

- `spec/usecases/task.md` RegisterTask の入力DTO・出力DTO・処理フロー7段・エラーケース表8行と、`RegisterTaskError` の全分岐を突き合わせて欠落・余剰なし。
- `spec/pages/index.md` の add(成功時にタスクID・ワークフロー名・解決先を表示、`--base` 省略時の HEAD 解決不能は明示指定を案内、未定義エージェントは定義済み一覧を添える、解決を試みたパスを添える)を `render.rs` の各分岐で確認。exit code は 成功=0 / 入力・状態・実行環境=非0 / 引数の使い方=2 / `--help`=0(標準出力)。
- `spec/testcases/task/register-task.md` の67行を通読し、本スライスで扱う振る舞い(名前指定で `workflow:` を使わない、パス指定で宣言名 > ファイル名の語幹、空文字の `--workflow` は入力境界で拒否、`retries: 0` / `timeout: none` / `statuses` 1件の受理、`judge` の `{...}` 非展開)がすべて実装側の分岐として存在することを確認。

## カバレッジ

一覧(`.thread/1/review/changed-files-003.txt`、149件)と1対1で対応する。

### 確認(本観点の対象・全文を読んだ)

- `crates/pulsen-domain/Cargo.toml`(AC-1 の依存空を確認)
- `crates/pulsen-domain/src/lib.rs`
- `crates/pulsen-domain/src/definition/agent.rs`, `crates/pulsen-domain/src/definition/assembler.rs`, `crates/pulsen-domain/src/definition/command.rs`, `crates/pulsen-domain/src/definition/config.rs`, `crates/pulsen-domain/src/definition/duration.rs`, `crates/pulsen-domain/src/definition/mod.rs`, `crates/pulsen-domain/src/definition/name.rs`, `crates/pulsen-domain/src/definition/port.rs`, `crates/pulsen-domain/src/definition/reference.rs`, `crates/pulsen-domain/src/definition/snapshot.rs`, `crates/pulsen-domain/src/definition/template.rs`, `crates/pulsen-domain/src/definition/validator.rs`, `crates/pulsen-domain/src/definition/workflow.rs`
- `crates/pulsen-domain/src/execution/mod.rs`, `crates/pulsen-domain/src/execution/port.rs`
- `crates/pulsen-domain/src/task/attempt.rs`, `crates/pulsen-domain/src/task/branch.rs`, `crates/pulsen-domain/src/task/counters.rs`, `crates/pulsen-domain/src/task/degraded.rs`, `crates/pulsen-domain/src/task/failure.rs`, `crates/pulsen-domain/src/task/id.rs`, `crates/pulsen-domain/src/task/mod.rs`, `crates/pulsen-domain/src/task/path.rs`, `crates/pulsen-domain/src/task/port.rs`, `crates/pulsen-domain/src/task/process.rs`, `crates/pulsen-domain/src/task/state.rs`, `crates/pulsen-domain/src/task/task.rs`, `crates/pulsen-domain/src/task/time.rs`
- `crates/pulsen/src/application/home.rs`, `crates/pulsen/src/application/mod.rs`, `crates/pulsen/src/application/register_task.rs`
- `crates/pulsen/src/cli/add.rs`, `crates/pulsen/src/cli/args.rs`, `crates/pulsen/src/cli/exit.rs`, `crates/pulsen/src/cli/mod.rs`, `crates/pulsen/src/cli/render.rs`, `crates/pulsen/src/cli/wire.rs`
- `crates/pulsen/src/lib.rs`, `crates/pulsen/src/main.rs`

### 確認(裏取りのために参照した・本観点の主対象ではない)

- `crates/pulsen/src/adapter/lock.rs` — ロックの取得失敗・自動作成がユースケースの「ロック解放」「state/ 自動作成」の前提を満たすかの確認のため
- `crates/pulsen/src/adapter/task_id.rs` — `Conflict` 再発行が実際に別のIDになるか(`generate` の `verified` フォールバックが再試行を無意味にしないか)の確認のため
- `crates/pulsen/tests/register_task.rs` — ユースケースの振る舞い(処理順・全件返却・ロック未取得時に何も観測しないこと)の裏取り
- `crates/pulsen/tests/cli_usage.rs` — exit code 規約とサブコマンド集合の裏取り
- `.thread/1/plan.md`, `.thread/1/review/triage.md` — 契約と既決着の判定の把握(指示による)
- (一覧外)`spec/domains/definition.md`, `spec/domains/task.md`, `spec/domains/execution.md`, `spec/usecases/task.md`, `spec/pages/index.md`, `spec/testcases/task/register-task.md`, `CLAUDE.md` — 適合の判定基準として

### スキップ

- `.adr/019-domain-crate-workspace.md`, `.adr/020-no-serde-in-domain-timestamp-conversion-in-domain.md`, `.adr/021-yaml-value-then-hand-written-schema-walk.md`, `.adr/022-std-file-lock-and-lockguard-marker-trait.md`, `.adr/023-dependency-selection.md`, `.adr/024-git-cli-shell-out-and-target-classification.md`, `.adr/025-task-file-json-and-corrupt-classification.md`, `.adr/026-task-id-format.md`, `.adr/027-port-conformance-suite-and-harness-hooks.md`, `.adr/028-usecase-error-paths-via-test-doubles.md`, `.adr/029-wildcard-enum-match-arm-lint-domain-only.md`, `.adr/030-workflow-store-base-dir-injection.md`, `.adr/031-pulsen-home-layout-in-application-layer.md`, `.adr/032-lock-holder-example-fixture.md`, `.adr/033-git-fixture-reproducibility.md`, `.adr/034-workflow-ref-separator-set-as-constant.md`, `.adr/035-file-slice-adrs-from-019.md`, `.adr/036-infallible-ports-absorb-failure-at-construction.md`, `.adr/037-platform-separator-set-without-cfg.md`, `.adr/038-adr-filing-format.md`, `.adr/039-read-error-shared-by-find-and-list.md`, `.adr/040-rehydrate-takes-field-bundle.md`, `.adr/042-absent-yaml-value-is-omission.md`, `.adr/043-store-adapters-receive-injected-paths.md`, `.adr/044-task-file-layout-in-domain.md`, `.adr/045-task-file-dto-generic-over-snapshot.md`, `.adr/046-no-skippable-hooks-for-post-operation-observation.md`, `.adr/048-parse-inputs-at-spec-flow-position.md`, `.adr/049-base-flag-allows-hyphen-values.md`, `.adr/050-schema-error-location-is-logical.md`, `.adr/051-undisplayable-name-fixture-is-whitespace-stem.md`, `.adr/052-acceptance-test-harness-in-tests-common.md`, `.adr/053-conformance-yaml-source-hooks.md`, `.adr/054-workflow-error-file-path-goes-into-free-form-messages.md`, `.adr/055-conformance-skip-budget.md`, `.adr/060-non-blocking-lock-case-observes-from-a-second-thread.md`, `.adr/061-unused-public-accessors-are-kept-only-for-verified-layout.md`, `.adr/062-acceptance-tests-detach-the-user-home.md` — ADR の起票状況・記述の整合は arch/spec 観点の担当。実装判断の根拠として必要な範囲(ADR-010 / 013 / 014 / 015 / 026 / 028 / 030 / 031 / 036 / 037 / 048 / 061)はコード内の参照コメントで確認済み
- `.thread/1/adr.md`, `.thread/1/progress.md`, `.thread/1/steps.md`, `.thread/1/testing.md`, `.thread/1/review/changed-files-001.txt`, `.thread/1/review/changed-files-002.txt`, `.thread/1/review/review-001.md`, `.thread/1/review/review-001-adapter.md`, `.thread/1/review/review-001-arch-spec.md`, `.thread/1/review/review-001-domain.md`, `.thread/1/review/review-001-test.md`, `.thread/1/review/review-001-usecase-cli.md`, `.thread/1/review/review-002-adapter.md`, `.thread/1/review/review-002-arch-spec.md`, `.thread/1/review/review-002-domain.md`, `.thread/1/review/review-002-test.md`, `.thread/1/review/review-002-usecase-cli.md` — 作業記録・過去ラウンドのレビュー成果物。記述の整合は arch/spec 観点の担当(既決着の判定は triage.md で把握済み)
- `Cargo.lock`, `Cargo.toml`, `crates/pulsen/Cargo.toml`, `flake.nix`, `rustfmt.toml` — ワークスペース・依存・ツールチェーン設定。本観点の対象外(ドメインクレートの依存空だけ AC-1 として確認)
- `crates/pulsen-conformance/Cargo.toml`, `crates/pulsen-conformance/HOOKS.md`, `crates/pulsen-conformance/src/clock.rs`, `crates/pulsen-conformance/src/config_store.rs`, `crates/pulsen-conformance/src/doubles/clock.rs`, `crates/pulsen-conformance/src/doubles/lock.rs`, `crates/pulsen-conformance/src/doubles/mod.rs`, `crates/pulsen-conformance/src/doubles/stores.rs`, `crates/pulsen-conformance/src/doubles/task_id.rs`, `crates/pulsen-conformance/src/doubles/task_repository.rs`, `crates/pulsen-conformance/src/doubles/tests.rs`, `crates/pulsen-conformance/src/doubles/worktree.rs`, `crates/pulsen-conformance/src/exclusive_lock.rs`, `crates/pulsen-conformance/src/lib.rs`, `crates/pulsen-conformance/src/task_id_generator.rs`, `crates/pulsen-conformance/src/task_repository.rs`, `crates/pulsen-conformance/src/workflow_store.rs`, `crates/pulsen-conformance/src/worktree_manager.rs` — ポート適合スイートとテストダブル。test 観点の担当
- `crates/pulsen/examples/lock_holder.rs` — 適合テストのフィクスチャ。test 観点の担当
- `crates/pulsen/src/adapter/clock.rs`, `crates/pulsen/src/adapter/config_store.rs`, `crates/pulsen/src/adapter/mod.rs`, `crates/pulsen/src/adapter/task_file.rs`, `crates/pulsen/src/adapter/task_repository.rs`, `crates/pulsen/src/adapter/workflow_store.rs`, `crates/pulsen/src/adapter/worktree.rs`, `crates/pulsen/src/adapter/yaml.rs` — アダプター実装。adapter 観点の担当(`task_file.rs` は `describe()` の呼び出し元としてのみ参照)
- `crates/pulsen/src/util/atomic.rs`, `crates/pulsen/src/util/fsdir.rs`, `crates/pulsen/src/util/mod.rs` — 共通ユーティリティ(アトミック置換・ディレクトリ作成)。adapter 観点の担当
- `crates/pulsen/tests/cli_add_boundary.rs`, `crates/pulsen/tests/cli_add_error.rs`, `crates/pulsen/tests/cli_add_normal.rs`, `crates/pulsen/tests/common/git.rs`, `crates/pulsen/tests/common/lock.rs`, `crates/pulsen/tests/common/mod.rs`, `crates/pulsen/tests/conformance_config_store.rs`, `crates/pulsen/tests/conformance_lock.rs`, `crates/pulsen/tests/conformance_task_repository.rs`, `crates/pulsen/tests/conformance_time_id.rs`, `crates/pulsen/tests/conformance_workflow_store.rs`, `crates/pulsen/tests/conformance_worktree.rs` — 受け入れテスト・適合テストの適用側。test 観点の担当(実行結果が全件緑であることのみ AC-1 として確認)
