# Domain

2周目。`triage.md` の wont-fix 3件(W-004 `SkillInputTemplate` の `unreachable!` / W-012 add の成功文言 / W-017 `create` の TOCTOU)は蒸し返さない。ゼロベースで再走査したうえで、1周目の修正(`describe()` のドメイン集約・`effective_*` の doc・`WorkflowAssembler` の前提条件明文化)が新たに生んだ問題を重点的に見た。

## 前提と検証方法

- 基準は `CLAUDE.md`(ヘキサゴナル + 関数型ドメインモデリング)と `.thread/1/plan.md` の受け入れ基準。spec 側は `spec/domains/{definition,task,execution}.md` と `spec/inventory/domain.md` の DOM-* 行を実装と突き合わせた。
- 機械的確認(いずれも本ラウンドで再実行):
  - `crates/pulsen-domain/Cargo.toml` の `[dependencies]` は空(AC-1)。ワークスペース側 `[workspace.dependencies]` にもドメインが引く外部クレートはない。
  - `grep -rn "cfg(unix)|cfg(windows)|cfg(target)" crates/pulsen-domain/` → 0件。`std::fs|std::io|std::process|std::time|std::env|std::thread|std::sync` → 0件。`Rc<|RefCell|Cell<|Mutex|Arc<` → 0件(ドメイン型は所有データのみで `Send`)。
  - `cargo test -p pulsen-domain` → **163 passed / 0 failed**。`cargo clippy -p pulsen-domain --all-targets -- -D warnings` → 警告なし(`wildcard_enum_match_arm = warn` 有効)。
  - ドメイン内の `_ =>` は 4箇所で、対象はすべて `&str` / `char` / `i64`(`Placeholder::parse` / `DurationSpec::parse` の単位 / `assemble_run_status` の `run` 値 / `days_in_month`)。**ドメイン enum への `_` は 0件**。
  - 非テストのパニックは `SkillInputTemplate::render` の `unreachable!` 1箇所のみ(wont-fix 済み)。`todo!` / `unimplemented!` / 非テストの `unwrap()` は 0件。

## 1周目の修正の検証(Domain 分)

| 指摘 | 判定 | 根拠 |
|---|---|---|
| W-001(重複ステータス名) | 意図どおり修正済み | `assembler.rs:172-175` に前提条件、`:205` に why。**前提が実際に成立することも確認した** — `adapter/yaml.rs:192-201` の `serde_yaml_ng::from_str::<Value>` は入れ子のマッピングでも重複キーを構文エラーにし(`yaml.rs:274-276` が `agents.claude.cmd` の重複で確認)、`key_text`(:239-252)が非文字列キーを弾くので、`statuses:` 直下に同一 `String` キーが2つ届く経路がない |
| W-002(説明文の二重定義) | ほぼ修正済み。ただし残件あり | `adapter/config_store.rs` の `describe_name_error` / `describe_duration_error` / `describe_command_error` は削除され `error.describe()` になった。しかし `cli/render.rs:330-335` に3つ目の `NameError` 文言が残る(下記 W-001) |
| W-003(`effective_*` の `None` 枝) | 修正済み。doc の根拠に難あり | `None` 枝は分離され、`snapshot.rs` の `Wait` に対する `effective_retry_limit` の期待も削除済み。doc の根拠が他ドメインの不変条件に依存している点は下記 W-003 |

## 受け入れ基準の判定(Domain 観点)

| AC | 判定 | 根拠 |
|---|---|---|
| AC-2 | 満たす | `NameError` 2種 / `DurationError` 2種 / `CommandError` 1種 / `TemplateError` 2種 / `ExpansionError` 1種 / `AgentDefError` 3種のすべてに仕様の言葉で名付けたユニットテストがある。生成は `parse` 経由のみ(`InputText::new` は spec の「制約なし」行。W-032 で spec 追従提起済み) |
| AC-3 | 満たす | `effective_agent/model/timeout/retry_limit` の優先順位、`display_name` の4規則。`reference.rs:102-111` が `POSIX_SEPARATORS` / `WINDOWS_SEPARATORS` を明示的に渡して `\` の両扱いを検証。`platform_separators` は `MAIN_SEPARATOR` 比較で `#[cfg]` を持たない(ADR-037 どおり) |
| AC-4 | 満たす | アセンブラ生成の10種すべてに個別テスト。循環・自己参照・到達不能・終端なしはいずれも受理(`assembler.rs:793-816`, `workflow.rs:279-294`) |
| AC-5 | 満たす | `RegistrationError` 5種すべてに個別テスト。`検証エラーは最初の1件で打ち切らず全件返る` / `同一ステータスの複数の不足はまとめて返る` の2本で全件収集を固定 |
| AC-6 | 満たす | `Task::register` の事後条件、`rehydrate` の不変条件1検証、`ExecutionState` 6状態の付随データ(不変条件8は型で表現)、`DegradedTask`、`Timestamp` の RFC3339 往復(両端・両端+1・epoch 前・閏年を含む) |
| AC-7 | 満たす | 下記「ポート表との突き合わせ」参照。未実装メソッドの宣言・スタブは0件 |
| スコープ逸脱 | なし | `Task` の遷移関数は `register` / `rehydrate` のみ。`DegradedTask` は `rehydrate` + 読み取りのみ。`WorkspacePlanner`、execution の分類・判定・gc、RunStore / ProcessController / CommandRunner、`WorktreeManager::create/remove` はいずれも未定義 |

### ポート表との突き合わせ(AC-7)

- `TaskRepository`(`task/port.rs:130-151`): 7メソッド・引数・戻り値とも spec 表(task.md:272-280)と一致。`CreateError`(Conflict/Io)/ `SaveError`(NotFound/Io)/ `ArchiveError`(NotFound/Io)/ `ReadError`(Io)/ `TaskLookup` 4種 / `TaskRecord` 2種 / `TaskEntry` 2種。`list_*` を `ReadError` に寄せた点は ADR-039 の明示的な決定。
- `ConfigStore::load` / `WorkflowStore::load` / `LoadedWorkflow` / `ConfigLoadError` 3種 / `WorkflowLoadError` 3種(`definition/port.rs`): definition.md:287-306 と一致。契約(読み取り専用・キャッシュしない・名前解決規則・`.yml` フォールバックなし)はドキュメンテーションコメントに落ちている。
- `WorktreeManager`(`execution/port.rs:32-41`): 本スライス対象の `validate_repo` / `head_branch` / `branch_exists` のみで、`create` / `remove` の先回り宣言はない。`TargetError` 5種は execution.md の綴りと一致。
- `ExclusiveLock::try_acquire` / `LockError::Failed` / `LockGuard`: spec の `Result<Option<LockGuard>, LockError>` を `Box<dyn LockGuard>` に写した形。契約4点(単一ロック・非ブロッキング・ドロップで解放・プロセス間)がコメントにある。
- `TaskIdGenerator::generate` / `Clock::now`: 無謬シグネチャのまま(ADR-036 が構築時と総関数へ失敗を吸収)。

### DOM-* 行の突き合わせ

Issue #1 のチェックリスト対象(definition 001〜056、task 001〜032・056・060・062〜079、execution 059/060/061/065/068/069/070)を1行ずつ確認し、**未実装・仮実装は0件**。スコープ外の行(task 033〜055・057〜059・061、execution の分類/判定/gc/RunStore/ProcessController/CommandRunner)は実装も宣言もされていない。

### ドメイン境界(外側との漏れ)

- ドメイン → 外: `#[cfg]` / I/O / 外部クレートの侵入なし(上記 grep)。
- 外 → ドメイン: `FsTaskRepository` はディレクトリも命名形式フィルタも `TaskFilePath::{active,archived,active_dir,archived_dir,file_name,parse_file_name}` 経由(ADR-044)。`FsConfigStore` / `FsWorkflowStore` は「テキスト → 値 → 未パース DTO」までで、値の生成と意味の検証は `GlobalConfig::parse` / `WorkflowAssembler::assemble` に渡している。`SourceLocation` は YAML クレート由来の型ではなくドメインの値。
- `RegisterTask`(`application/register_task.rs:127-184`)は spec の処理フロー順(ロック → 解決 → 表示名 → 対象検証 → 登録時検証 → ID発行 → create。ADR-048)どおりで、判断ロジックも文言も自前で持たない。

## Blockers

なし

## Warnings

- **[W-001]** `NameError` の説明文が2つの層で別々の文言のまま残っている(1周目 W-002 の修正の取りこぼし)
  - 場所: `crates/pulsen-domain/src/definition/name.rs:12-23` と `crates/pulsen/src/cli/render.rs:330-335`
  - 理由: 1周目の修正で `NameError::describe()` をドメインに置き、doc に「**説明の定義箇所をドメインに1つ置く。層ごとに書くと同じ誤りの案内が食い違う**」と明記した。しかし `render.rs` は同じ `NameError` に対して別の文言(`Empty` → 「空文字列です」/ `SurroundingWhitespace` → 「前後に空白を含みます」)を持ち続けており、`describe()` の「空文字列は指定できません」/「前後に空白を含められません」と食い違う。実際の出力で並ぶ: ワークフロー定義の `prompt: ""` は `render.rs:214-216` 経由で「`statuses.queued.prompt` の値が不正です: 空文字列は指定できません」、`--workflow ""` は `render.rs:117-120` 経由で「--workflow の値が不正です。 原因: 空文字列です」。**doc が主張する「定義箇所は1つ」が成立していない**。同じことは `describe()` を消した `adapter/config_store.rs` 側では解消しており、扱いが非対称。
  - 提案: `render.rs:125` は「ファイル名から決めた名前が{}」という文中に埋める用途なので `describe()` をそのまま流用すると日本語が壊れる。したがって (a) `describe()` の doc を「**定義ファイル(config.yaml / ワークフロー定義YAML)の値制約の説明**を1箇所に置く」へ狭めて、CLI 引数向けの述語形が別に要る理由を why として残すか、(b) `render.rs:117-120` のような文中に埋めない箇所だけ `describe()` を呼ぶようにして、`name_error` を「文中に埋める述語形」専用と doc に書く、のいずれか。いま残っているのは「一元化したと書いてあるが一元化されていない」状態で、これが一番まずい。

- **[W-002]** `ForbiddenKey` の検出がキー名と存在フラグの平行配列に依存している
  - 場所: `crates/pulsen-domain/src/definition/assembler.rs:226`(`AGENT_RUN_KEYS`)と `:332-347`(`present` 配列と `zip`)
  - 理由: 報告するキー名(`AGENT_RUN_KEYS`)と、実際に存在を見る式(`options.agent.is_some()` …)が**別々の配列に同じ順序で並んでいるだけ**で、対応関係が型に現れていない。どちらか一方に要素を足す・並べ替えると、`ForbiddenKey { key }` が実際に書かれていたキーとは違う名前を報告するようになるが、コンパイルは通り、テスト(`:574-610`)は `judge` / `next` / `agent` の3件しか見ないので気づけない。CLAUDE.md の「不正な状態を型で表現不能にする」を、追加コストほぼ0で満たせる箇所で規約に委ねている。`StatusOptions` を分解代入で組み立てている(`:288-295`)のとは対照的に、ここだけ位置合わせが暗黙。
  - 提案: 平行配列をやめ、対応を1箇所にまとめる。`for (key, is_present) in [("agent", options.agent.is_some()), ("model", options.model.is_some()), ...]` のように組で書けば、キー名と述語が隣り合い、ずれようがなくなる。`AGENT_RUN_KEYS` 定数を別に公開する必要がないなら合わせて畳む。

- **[W-003]** `effective_*` の doc が、`definition` ドメインの契約を `task` ドメインの不変条件で説明している
  - 場所: `crates/pulsen-domain/src/definition/workflow.rs:137-139`(および `:152-153`, `:165-167`, `:181-183`)
  - 理由: 1周目 W-003 の修正で `None` 枝に「タスクの不変条件1(`task_status ∈ snapshot.statuses`)により実運用では到達しない」と書いた。しかし `WorkflowDefinition` は definition ドメインの型で、`spec/domains/definition.md:91` は「**Definition は他ドメインに依存しない(ADR-017)**」と定めている。ここで API の契約の根拠を task ドメインの不変条件に置くと、依存が(コンパイル単位ではなく文書上とはいえ)内向きに1本増える。加えて根拠として不完全でもある — 本PR で `effective_*` を呼んでいる唯一の場所は `validator.rs:83,120` の `RegistrationValidator` で、そこは `Task` が存在しない登録時検証であり、`None` にならない理由は「`definition.statuses()` を回して得たキーを渡しているから」であって不変条件1ではない。つまり実在する2つの呼び出し経路のうち片方しか doc の説明でカバーできていない。
  - 提案: 根拠を definition ドメイン内で閉じる形に書き換える。「引数のステータス名がこの定義に属することは呼び出し側の責務(この定義の `statuses()` から得るか、`status()` で存在を確かめてから使う)。属さない名前には適用対象がないため既定値を返す」と書けば他ドメインを参照せずに済み、`RegistrationValidator` の使い方もそのまま説明できる。`Task` 側の不変条件1に言及したいなら「タスク経由の呼び出しではこの責務が不変条件1で自動的に満たされる」という補足に留める。

- **[W-004]** 複数ステータスが同じエージェントを参照すると、`RegistrationValidator` が同一のエラーを件数分だけ返す
  - 場所: `crates/pulsen-domain/src/definition/validator.rs:59-66`(ステータスごとの呼び出し)と `:89-105`(`UnknownAgent` / `InvalidAgentDefinition` の push)
  - 理由: `UnknownAgent { name, defined }` と `InvalidAgentDefinition { agent, error }` は spec(definition.md:274-280)で**ステータスを持たない = エージェント単位の誤り**として定義されている。それをステータスごとのループで push するため、3つの `AgentRun` ステータスが未定義エージェント `missing` を参照する定義では、値まで完全に同一の `UnknownAgent` が3件返る。`render.rs:242-260` はこれを件数付きで並べるので、利用者には「検証に失敗しました(3件)」の下に区別のつかない同じ2行が3回出る — 問題は1つなのに件数が水増しされ、どこを直せばよいかの手がかりも増えない。`MissingAgent` / `MissingSkillInput` / `MissingModel` は `status` を持つので繰り返しても意味があり、扱いが非対称。既存テスト(`:337-367`)は全ステータスが別々の誤りを持つ台本なので、この振る舞いは一度も観測されていない。
  - 提案: エージェント単位の2種だけ、同じ値をすでに積んでいれば push しない(`errors.contains(&error)` で足りる。両バリアントとも `PartialEq` を導出済み。件数は数十件規模なので線形探索で問題ない)。仕様の言葉での回帰テスト(例: `同じ未定義エージェントを参照する複数ステータスは1件にまとまる`)を1本足す。spec が「全ステータス分をまとめて返す」としか書いていない以上、重複の抑制を仕様側にも反映するなら `progress.md` の「spec へ追従を提起する点」に1行足す。

## 検討して指摘しないことにした点

- `effective_*` の `Some(Wait | Cleanup)` 枝と `None` 枝が同じ値を返すこと: 分岐を分けた目的は「どの枝がどの理由で来るか」を doc と対にすることであり、triage が signature 変更を wont-fix と決めた以上、値が同じであること自体は問題ではない。
- `DurationError::describe()` だけが `String` を返し、他2つが `&'static str` を返すこと: `InvalidFormat { given }` の埋め込みが必要で、呼び出し側の `.to_owned()` の有無が揃わないのは避けられない。
- `describe()` がドメインに日本語の表示文言を置くこと: spec が `InvalidValue { location, message: String }` を型として宣言しており、`cause: ValueError` への作り替えは spec 改訂を伴う(triage が明示的に除外済み)。文言の**所在**そのものは指摘しない。残る問題は「一元化したと doc が主張しているのに一元化されていない」点だけで、それは W-001 に書いた。
- `RawCommand::parse_tokens(vec![String::new()])` が空プログラム名を受理すること・`AttemptNumber::next` の `saturating_add`・`TaskFields` 等の公開フィールド: 1周目に検討済みで結論は変わらない。
- `Task::rehydrate` が不変条件2〜4を検証しないこと: spec/domains/task.md:164 と ADR-025 が明示的に禁じている(過剰検証は tick スライスの `InvariantViolated` 経路を壊す)。
- `assemble` が最初の1件で打ち切ること: spec のシグネチャが `Result<_, WorkflowParseError>`(単数)であり、`RegistrationValidator` の全件収集とは要求が違う。

## カバレッジ

- 確認(48件):
  - ドメインクレート全30件: `crates/pulsen-domain/Cargo.toml`, `crates/pulsen-domain/src/lib.rs`, `src/definition/{agent,assembler,command,config,duration,mod,name,port,reference,snapshot,template,validator,workflow}.rs`, `src/execution/{mod,port}.rs`, `src/task/{attempt,branch,counters,degraded,failure,id,mod,path,port,process,state,task,time}.rs`
  - 境界の確認のみで読んだ4件: `crates/pulsen/src/adapter/config_store.rs`(`describe()` の重複解消), `crates/pulsen/src/adapter/yaml.rs`(重複キー前提が成立するか), `crates/pulsen/src/application/register_task.rs`(ドメインの判断を迂回していないか), `crates/pulsen/src/cli/render.rs`(文言の定義箇所)。アダプター/ユースケース/CLI としての品質は他観点の担当。
  - `Cargo.toml`(ドメインクレートの依存とリント継承)
  - ドメイン設計を縛る ADR 10件: `.adr/020`, `.adr/029`, `.adr/034`, `.adr/036`, `.adr/037`, `.adr/039`, `.adr/040`, `.adr/044`, `.adr/045`, `.adr/048`
  - `.thread/1/plan.md`, `.thread/1/review/triage.md`, `.thread/1/review/review-001-domain.md`
- スキップ: `.adr/{019,021,022,023,024,025,026,027,028,030,031,032,033,035,038,042,043,046,049,050,051,052,053,054}`(24件) — YAML/ロック/git/JSON/適合テスト/配線/文書運用の設計判断で、ドメインの型と振る舞いを縛らない
- スキップ: `.thread/1/{adr.md,progress.md,steps.md,testing.md}` と `.thread/1/review/{changed-files-001.txt,review-001-adapter.md,review-001-arch-spec.md,review-001-test.md,review-001-usecase-cli.md,review-001.md}`(10件) — 進行記録と他観点のレビュー記録。ADR の実体は `.adr/` 側を読んだ
- スキップ: `Cargo.lock`(1件) — 依存解決の結果。ドメインクレートの依存が空であることは `crates/pulsen-domain/Cargo.toml` で確認済み
- スキップ: `crates/pulsen-conformance/`(18件) — ポート適合テストの枠組み。テスト観点の担当
- スキップ: `crates/pulsen/Cargo.toml`, `crates/pulsen/examples/lock_holder.rs`, `crates/pulsen/src/adapter/{clock,lock,mod,task_file,task_id,task_repository,workflow_store,worktree}.rs`, `crates/pulsen/src/application/{home,mod}.rs`, `crates/pulsen/src/cli/{add,args,exit,mod,wire}.rs`, `crates/pulsen/src/{lib,main}.rs`, `crates/pulsen/src/util/{atomic,fsdir,mod}.rs`(22件) — OS/git/ファイルシステム依存の実装・配線・入出力。アダプター/ユースケース/CLI 観点の担当(ドメインへの `#[cfg]` 漏れがないことは grep で確認済み)
- スキップ: `crates/pulsen/tests/`(14件) — 統合テスト・適合テストの適用。テスト観点の担当
- スキップ: `flake.nix`, `rustfmt.toml`(2件) — 開発環境とフォーマット設定
