# Domain

## 前提と検証方法

- 基準は `CLAUDE.md`(ヘキサゴナル + 関数型ドメインモデリング)と `.thread/1/plan.md` の受け入れ基準。spec 側は `spec/domains/{definition,task,execution}.md` と `spec/inventory/domain.md` の DOM-* 行を1行ずつ実装と突き合わせた。
- 機械的確認:
  - `crates/pulsen-domain/Cargo.toml` の `[dependencies]` は空(AC-1)。
  - `grep -rn "cfg(unix)|cfg(windows)|cfg(target)" crates/pulsen-domain/` → 0件。`std::fs|std::io|std::process|std::time|std::env|std::thread|std::sync` → 0件。`Rc|RefCell|Cell|Mutex` → 0件(ドメイン型は所有データのみで `Send`)。
  - `cargo test -p pulsen-domain` → 163 passed / 0 failed。`cargo clippy -p pulsen-domain --all-targets -- -D warnings` → 警告なし(`wildcard_enum_match_arm = warn` を有効にした状態)。
  - ドメイン内の `_ =>` アームは `&str` / `char` / `i64` に対するもの(`Placeholder::parse` / `DurationSpec::parse` の単位 / `assemble_run_status` の `run` 値 / `days_in_month`)だけで、**ドメイン enum への `_` は1つもない**。
  - パニックは `SkillInputTemplate::render` の `unreachable!` 1箇所のみ(不変条件違反)。`todo!` / `unimplemented!` / 非テストの `unwrap()` は0件。

## 受け入れ基準の判定(Domain 観点)

| AC | 判定 | 根拠 |
|---|---|---|
| AC-2 | 満たす | `NameError` 2種 / `DurationError` 2種 / `CommandError` 1種 / `TemplateError` 2種 / `ExpansionError` 1種 / `AgentDefError` 3種のすべてに、仕様の言葉で名付けたユニットテストがある |
| AC-3 | 満たす | `effective_agent/model/timeout/retry_limit` の優先順位、`display_name` の4規則。`WorkflowRef::parse_with_separators` に `POSIX_SEPARATORS` / `WINDOWS_SEPARATORS` の両方を明示的に渡すテストあり(`reference.rs:102-111`) |
| AC-4 | 満たす | `WorkflowParseError` 12種のうちアセンブラ生成の10種すべてに個別テスト。循環・自己参照・到達不能・終端なしはいずれも受理(`assembler.rs:801-824`, `workflow.rs:267-282`) |
| AC-5 | 満たす | `RegistrationError` 5種すべてに個別テストがあり、`検証エラーは最初の1件で打ち切らず全件返る` / `同一ステータスの複数の不足はまとめて返る` の2本で全件収集を固定している |
| AC-6 | 満たす | `Task::register` の事後条件、`rehydrate` の不変条件1検証、`ExecutionState` 6状態の付随データ、`DegradedTask`、`Timestamp` の RFC3339 往復 |
| AC-7 | 満たす | 下記「ポート表との突き合わせ」参照。未実装メソッドの宣言・スタブは0件 |
| スコープ逸脱 | なし | `Task` の遷移関数は `register` / `rehydrate` のみ。`DegradedTask` は `rehydrate` + 読み取りのみ(`abort` / `retry` / `mark_notified` は未定義)。`WorkspacePlanner`、execution の分類・判定・gc サービス、RunStore / ProcessController / CommandRunner、`WorktreeManager::create/remove` はいずれも未定義で、スコープ外の先回り実装はない |

### ポート表との突き合わせ(AC-7)

- `TaskRepository`(`task/port.rs:130-151`): 7メソッド・引数・戻り値とも spec 表と一致。`CreateError`(Conflict/Io)/ `SaveError`(NotFound/Io)/ `ArchiveError`(NotFound/Io)/ `ReadError`(Io)/ `TaskLookup` 4種 / `TaskRecord` 2種 / `TaskEntry` 2種。`list_*` の戻りエラーを `ReadError` に寄せた点は spec 表の綴り(`Io`)と字面が違うが、台帳に `Io` 型の行はなく ADR-039 で明示的に決めている。
- `TaskIdGenerator::generate` / `Clock::now`: 無謬シグネチャのまま(ADR-036 で失敗を構築時と総関数に吸収)。
- `ConfigStore::load` / `WorkflowStore::load` / `LoadedWorkflow` / `ConfigLoadError` 3種 / `WorkflowLoadError` 3種(`definition/port.rs`): spec 表と一致。
- `WorktreeManager`(`execution/port.rs:32-41`): 本スライス対象の `validate_repo` / `head_branch` / `branch_exists` の3つのみ。`TargetError` 5種。
- `ExclusiveLock::try_acquire` / `LockError::Failed` / `LockGuard`(マーカートレイト): spec の `Result<Option<LockGuard>, LockError>` を `Box<dyn LockGuard>` に写した形で、契約(非ブロッキング・取得不能は `Ok(None)`・ドロップで解放)はドキュメンテーションコメントに落ちている。

### DOM-* 行の突き合わせ

Issue #1 のチェックリスト対象(definition 001〜056、task 001〜032・056・060・062〜079、execution 059/060/061/065/068/069/070)を1行ずつ確認し、**未実装・仮実装は0件**。スコープ外の行(task 033〜055・057〜059・061、execution の分類/判定/gc/RunStore/ProcessController/CommandRunner)は実装されておらず、宣言だけ置く形にもなっていない。

### `Timestamp` の自前実装(重点確認)

- `days_from_civil` / `civil_from_days` は Howard Hinnant のアルゴリズムと式単位で一致(`shifted_month = (month + 9) % 12` は `m > 2 ? m-3 : m+9` と同値、`DAYS_PER_ERA - 1 = 146096`)。負の年・era で `div_euclid` / `rem_euclid` を使っており、epoch 前で1日ずれる truncating division の罠を踏んでいない(`epochより前の時刻も往復する` が -1 秒を含めて固定)。
- うるう年判定 `year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)` は 1900(非閏)/ 2000(閏)/ 2024(閏)をテストで押さえている。
- 範囲は `MIN_UNIX_SECS = -62135596800`(0001-01-01T00:00:00Z)/ `MAX_UNIX_SECS = 253402300799`(9999-12-31T23:59:59Z)で、いずれも正しい値。両端・両端+1 の4ケースがテスト済みで、`to_rfc3339 ∘ parse_rfc3339 = id` が全域で成立する(5桁年に出ない)。
- `parse_rfc3339` は20バイト固定・区切り位置・ASCII 数字を先に検査するため、マルチバイト入力でスライス境界が割れない。`decimal` は固定幅スライスのみに掛かるので桁あふれしない。
- `elapsed_since` は巻き戻りを 0 に丸める(`Clock` が単調性を要求しない契約と一致)。

## Blockers

なし

## Warnings

- **[W-001]** `WorkflowAssembler::assemble` が重複ステータス名を黙って握り潰す
  - 場所: `crates/pulsen-domain/src/definition/assembler.rs:196-202`
  - 理由: 入力 `RawWorkflowDoc.statuses` は `Vec<(String, RawStatusDoc)>` で重複を表現できるのに、`BTreeMap::insert` で後勝ちに畳んでいる。現状は `adapter::yaml` が YAML の重複キーを `YamlSyntax` で落とすため実害が出ないが、**「重複を落とす責務がアダプター側にある」ことがドメインの型にも契約にも現れていない**。後続スライスが in-memory の `WorkflowStore` や別のフロントエンド(将来の生成コマンド等)から `assemble` を直接呼ぶと、ステータスが1つ静かに消えたまま登録が成功しうる。`WorkflowParseError` に重複用の種別が無いのは spec どおりなので、少なくとも検出して既存の種別に写すか、契約として明文化しておきたい。
  - 提案: `statuses.insert(...)` の戻り値が `Some` のとき `WorkflowParseError::UnknownKey { location: "statuses", key: name }`(= スキーマ外の重複キー)相当へ写すか、それが spec の意図とずれるなら「重複はアダプターが `YamlSyntax` として弾く前提」を `WorkflowAssembler::assemble` のドキュメンテーションコメントに前提条件として書く。

- **[W-002]** ドメインエラーの説明文がドメインとアダプターで二重定義されている
  - 場所: `crates/pulsen-domain/src/definition/assembler.rs:423-455` と `crates/pulsen/src/adapter/config_store.rs:211-229`
  - 理由: `NameError` / `DurationError` / `CommandError` に対する日本語の説明("空文字列は指定できません"・"前後に空白を含められません"・"期間の形式が不正です: {given}"・"期間に 0 は指定できません"・"コマンドが空です")が、ドメイン(`invalid_name` / `invalid_duration` / `invalid_command`)とアダプター(`describe_name_error` / `describe_duration_error` / `describe_command_error`)に**同じ文言で2箇所**存在する。同じドメインエラーの説明が2つの層に散っていると、片方だけ直したときに config.yaml とワークフローYAMLで同じ誤りの案内が食い違う。根にあるのは `WorkflowParseError::InvalidValue` が `message: String` に畳んで**原因のエラー値を捨てている**ことで、CLAUDE.md の「エラーは値として返す」からもわずかに外れている(spec の `InvalidValue { location, message }` は "NameError / DurationError / CommandError を包む" と書いている)。
  - 提案: 説明の生成をドメインの1箇所に寄せる。最小の変更は `NameError` / `DurationError` / `CommandError` に `describe()`(または `Display`)をドメイン側で1つ持たせ、アダプターはそれを呼ぶだけにすること。踏み込むなら `InvalidValue { location, cause: ValueError }`(`ValueError = Name(NameError) | Duration(DurationError) | Command(CommandError)`)にして、文言の組み立てを CLI 層へ寄せる。

- **[W-003]** `effective_*` が「存在しないステータス名」と「上書きなし」を区別せず既定値を返す
  - 場所: `crates/pulsen-domain/src/definition/workflow.rs:138-180`
  - 理由: 4つの `effective_*` はいずれも `self.statuses.get(status)` が `None` のときに `Wait` / `Cleanup` と同じ枝へ落ち、`effective_timeout` は 1h を、`effective_retry_limit` は 2 を返す。ステータス名の取り違え(タイプミス・スナップショット外の名前)が値として観測できず、既定値として通ってしまう。不変条件1により実運用では `task_status ∈ statuses` が保証されるが、その保証は `Task` 側にあり `WorkflowDefinition` の API には現れていない。関連して `snapshot.rs:120-123` のテストは、spec が「呼び出しを規定しない」と明記している `Wait` への `effective_retry_limit` に対して `2` を期待として固定しており、未規定の振る舞いをテストで確定させている。
  - 提案: `effective_*` を `&StatusDefinition`(あるいは検証済みの `&StatusName`)を取る形にして「存在しないステータス」を型で排除する。それが呼び出し側に重いなら、`None` 枝を `Wait` / `Cleanup` と分けたうえで、`Wait` に対する呼び出しが未規定であることをドキュメンテーションコメントに書き、テストからは `Wait` のケースを外す。

- **[W-004]** `SkillInputTemplate` が `Segment` を共有するため `render` に `unreachable!` が残る
  - 場所: `crates/pulsen-domain/src/definition/template.rs:196-199`
  - 理由: `SkillInputTemplate::parse` は `allowed = &[Placeholder::Skill]` を渡すので `Hole` は必ず `Skill` だが、型は `Segment`(= 任意の `Placeholder` を持てる)のままなので、`render` が到達不能アームでパニックを持たざるを得ない。CLAUDE.md の「不正な状態を型で表現不能にする」を、追加コストほぼ0で満たせる箇所でパニックに委ねている。`render` は spec 上「失敗しえない」(戻り値が `InputText`)ので、ここに `unreachable!` があると無謬性の根拠がコメント頼りになる。
  - 提案: `SkillInputTemplate` の断片を専用の型(例: `SkillSegment = Literal(String) | SkillHole`)にする。`segments()` の公開シグネチャが変わるが、現状 `pulsen` 側からは使われていないので影響は閉じている。

## 検討して指摘しないことにした点

- `AttemptNumber::next` の `saturating_add`(`attempt.rs:33-35`): `u32::MAX` で単調増加が止まるが、到達には1タスクあたり約43億回の起動記録が要る。無謬に保つ理由が why としてコメントに残っており、`Result` を全呼び出し側へ広げる costs のほうが大きい。
- `WorkflowSnapshot::rehydrate` が `pub` で config との再突き合わせを行わないこと: spec が生成2経路として明示しており、`from_validated` を `pub(crate)` に閉じている分担は正しい。
- `RawCommand::parse_tokens(vec![String::new()])` が「空文字列トークン1つ」を受理すること: spec の規則("分割後 1 トークン以上"・"空文字列トークンは配列形式でのみ許容")どおり。プログラム名が空になる組み合わせは実行時に `FailedToStart` として現れる想定で、登録時に弾く根拠は spec に無い。
- `TaskFields` / `DegradedTaskFields` / `GlobalConfigInput` / `RawWorkflowDoc` / `RawStatusDoc` / `PlaceholderValues` の公開フィールド: いずれも境界で一度だけ検証される入力の束(ADR-040)で、束自体は不変条件を持たない。検証は `rehydrate` / `parse` / `assemble` に閉じている。

## ドメイン境界(外側との漏れ)

- レイアウトの知識はドメインに閉じている。`FsTaskRepository` はディレクトリも命名形式フィルタも `TaskFilePath::{active,archived,active_dir,archived_dir,parse_file_name}` 経由で得ており(`adapter/task_repository.rs:39-102`)、`state_root.join("tasks")` のような再実装はない。`state/` `worktrees/` というホーム直下のレイアウトだけが `application/home.rs` にあり、これは ADR-031 の分担どおり。
- `DefaultTaskIdGenerator` は時刻成分を `Timestamp::to_rfc3339()` から切り出しており、暦計算をアダプターで再実装していない。
- `FsConfigStore` / `FsWorkflowStore` は「テキスト → 値 → 未パース DTO」までを担い、値の生成と意味の検証(`GlobalConfig::parse` / `WorkflowAssembler::assemble`)はドメインに渡している。`ForbiddenKey`(動作種別との整合)がアダプターに漏れていない点も spec どおり。
- `RegisterTask` ユースケースは観測 → ドメインの判断(`WorkflowRef` / `RegistrationValidator` / `Task::register`)→ 実行の順で、判断ロジックを自前で持っていない。文言の組み立ても行っていない。
- 逆方向(外側の関心のドメインへの侵入)も見当たらない。`SourceLocation` は spec の `location` に対応する値で、YAML クレート由来の型ではない。

## カバレッジ

- 確認: `.adr/020-no-serde-in-domain-timestamp-conversion-in-domain.md`, `.adr/029-wildcard-enum-match-arm-lint-domain-only.md`, `.adr/034-workflow-ref-separator-set-as-constant.md`, `.adr/036-infallible-ports-absorb-failure-at-construction.md`, `.adr/037-platform-separator-set-without-cfg.md`, `.adr/039-read-error-shared-by-find-and-list.md`, `.adr/040-rehydrate-takes-field-bundle.md`, `.adr/042-absent-yaml-value-is-omission.md`, `.adr/044-task-file-layout-in-domain.md`, `.adr/048-parse-inputs-at-spec-flow-position.md`, `.adr/050-schema-error-location-is-logical.md`, `.thread/1/plan.md`, `Cargo.toml`, `crates/pulsen-domain/Cargo.toml`, `crates/pulsen-domain/src/lib.rs`, `crates/pulsen-domain/src/definition/mod.rs`, `crates/pulsen-domain/src/definition/agent.rs`, `crates/pulsen-domain/src/definition/assembler.rs`, `crates/pulsen-domain/src/definition/command.rs`, `crates/pulsen-domain/src/definition/config.rs`, `crates/pulsen-domain/src/definition/duration.rs`, `crates/pulsen-domain/src/definition/name.rs`, `crates/pulsen-domain/src/definition/port.rs`, `crates/pulsen-domain/src/definition/reference.rs`, `crates/pulsen-domain/src/definition/snapshot.rs`, `crates/pulsen-domain/src/definition/template.rs`, `crates/pulsen-domain/src/definition/validator.rs`, `crates/pulsen-domain/src/definition/workflow.rs`, `crates/pulsen-domain/src/execution/mod.rs`, `crates/pulsen-domain/src/execution/port.rs`, `crates/pulsen-domain/src/task/mod.rs`, `crates/pulsen-domain/src/task/attempt.rs`, `crates/pulsen-domain/src/task/branch.rs`, `crates/pulsen-domain/src/task/counters.rs`, `crates/pulsen-domain/src/task/degraded.rs`, `crates/pulsen-domain/src/task/failure.rs`, `crates/pulsen-domain/src/task/id.rs`, `crates/pulsen-domain/src/task/path.rs`, `crates/pulsen-domain/src/task/port.rs`, `crates/pulsen-domain/src/task/process.rs`, `crates/pulsen-domain/src/task/state.rs`, `crates/pulsen-domain/src/task/task.rs`, `crates/pulsen-domain/src/task/time.rs`, `crates/pulsen/src/adapter/config_store.rs`, `crates/pulsen/src/adapter/task_file.rs`, `crates/pulsen/src/adapter/task_id.rs`, `crates/pulsen/src/adapter/task_repository.rs`, `crates/pulsen/src/adapter/workflow_store.rs`, `crates/pulsen/src/adapter/yaml.rs`, `crates/pulsen/src/application/home.rs`, `crates/pulsen/src/application/register_task.rs`
  - うち `crates/pulsen/src/adapter/*` と `crates/pulsen/src/application/*` は「ドメインのロジック・レイアウト知識が外へ漏れていないか」「ドメイン型を迂回した生成がないか」の観点だけで読んだ(アダプター/ユースケースとしての品質は他観点の担当)。
- スキップ: `.adr/019・021〜028・030〜033・035`(14件) — YAML/ロック/git/JSON/適合テスト/配線の設計判断で、ドメインの型と振る舞いに影響しない
- スキップ: `.thread/1/{adr.md,progress.md,steps.md,testing.md}` — 進行記録・手順であり実装ではない(ADR の実体は `.adr/` 側を読んだ)
- スキップ: `Cargo.lock` — 依存解決の結果。ドメインクレートの依存が空であることは `crates/pulsen-domain/Cargo.toml` で確認済み
- スキップ: `crates/pulsen-conformance/`(18件) — ポート適合テストの枠組み。テスト観点の担当
- スキップ: `crates/pulsen/Cargo.toml`, `crates/pulsen/examples/lock_holder.rs` — 依存宣言とロック検証用フィクスチャ
- スキップ: `crates/pulsen/src/adapter/{clock.rs,lock.rs,mod.rs,worktree.rs}` — OS/git 依存の実装。アダプター観点の担当(ドメインへの `#[cfg]` 漏れがないことは grep で確認済み)
- スキップ: `crates/pulsen/src/application/mod.rs` — モジュール宣言
- スキップ: `crates/pulsen/src/cli/{add.rs,args.rs,exit.rs,mod.rs,render.rs,wire.rs}`, `crates/pulsen/src/lib.rs`, `crates/pulsen/src/main.rs` — 入出力・配線・文言。CLI 観点の担当
- スキップ: `crates/pulsen/src/util/{atomic.rs,fsdir.rs,mod.rs}` — ファイルシステムの共通ユーティリティ
- スキップ: `crates/pulsen/tests/`(13件) — 統合テスト・適合テストの適用。テスト観点の担当
- スキップ: `flake.nix`, `rustfmt.toml` — 開発環境とフォーマット設定
