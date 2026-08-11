# Definition

ユーザーが記述する定義 — グローバル設定(config.yaml)とワークフロー定義(YAML)— の構造・検証・コマンドテンプレート展開を担うドメイン。値オブジェクト中心で、エンティティ(可変な同一性を持つもの)は存在しない。定義は登録時に一度だけ検証され、以降は不変の値として扱われる(parse, don't validate)。

仕様の根拠: requirements §3・§3.1・§7・§7.2・§10、ADR-013・014・015。

## ユビキタス言語

| 英語名 | 日本語名 | 定義 |
|---|---|---|
| GlobalConfig | グローバル設定 | config.yaml の内容。エージェント定義・通知コマンド・各種グローバルデフォルト |
| AgentDefinition | エージェント定義 | エージェント名に対応するコマンドテンプレートと skill_input 変換規則 |
| CommandTemplate | コマンドテンプレート | プレースホルダを含み得るトークン列。展開して CommandLine になる |
| Placeholder | プレースホルダ | テンプレート中の `{name}`。cmd では input / model / workspace、skill_input では skill のみ有効 |
| CommandLine | コマンドライン | 展開済みのトークン列。先頭がプログラム、以降が引数。シェルを介さず直接起動される |
| PlainCommand | 無展開コマンド | 判定・通知用のトークン列。プレースホルダ展開は行われない(requirements §6.3) |
| WorkflowDefinition | ワークフロー定義 | ステータスの集合と初期ステータス・デフォルト(エージェント・モデル) |
| StatusDefinition | ステータス定義 | 1ステータスの動作(エージェント実行 / 何もしない / クリーンアップ)と付随設定 |
| WorkflowSnapshot | スナップショット | 登録時検証を通過した正規化済みワークフロー定義。タスクファイルに埋め込まれる(ADR-015) |
| WorkflowRef | ワークフロー参照 | `add` に渡される名前またはファイルパス |
| DurationSpec | 期間 | `<数値><単位>`(s / m / h / d)で表す期間 |
| TimeoutSpec | タイムアウト指定 | 期間による制限、または `none`(無制限) |

## 値オブジェクト

### 名前系(文字列 newtype)

| 型 | フィールド | バリデーション |
|---|---|---|
| `AgentName` | `String` | 非空。前後空白なし |
| `ModelName` | `String` | 非空。前後空白なし |
| `SkillName` | `String` | 非空。前後空白なし |
| `Prompt` | `String` | 非空 |
| `StatusName` | `String` | 非空。前後空白なし(CLI引数・YAML キーとして使われる) |
| `WorkflowName` | `String` | 非空。前後空白なし |
| `InputText` | `String` | 制約なし(`{input}` へ渡る値。プロンプトまたは skill_input の展開結果) |

いずれも `parse(s: String) -> Result<Self, NameError>` でのみ生成する。等価性は文字列の完全一致。

- エラー型: `NameError = Empty | SurroundingWhitespace`(前後空白)

### DurationSpec

- フィールド: `seconds: u64`(正規化済みの秒数)
- バリデーション: `parse(s: &str) -> Result<Self, DurationError>` は `<正の10進整数><単位>` のみ受理(単位 `s` / `m` / `h` / `d`)。`0s` 等のゼロ・単位なし・負値・空白混入はエラー
- エラー型: `DurationError = InvalidFormat { given } | Zero`
- 等価性: 秒数の一致(`1m` と `60s` は等価)

### TimeoutSpec

- 直和型: `Limited(DurationSpec) | Unlimited`
- YAML表現: 期間文字列 → `Limited`、文字列 `none` → `Unlimited`。それ以外はエラー
- 等価性: 構造の一致

### RawCommand(構造解釈済みトークン列)

YAMLの「文字列またはトークン配列」からトークン列へ落とす共通規則(requirements §3.1)。

- フィールド: `tokens: Vec<String>`
- バリデーション: 文字列形式は単純な空白分割(連続空白は1区切り、クォートはグルーピングとして解釈しない)。配列形式は各要素をそのままトークンとする(空文字列トークンは配列形式でのみ許容する — 意図的な空引数の表現)。分割後 1 トークン以上であること(空文字列・空配列はエラー)
- エラー型: `CommandError = Empty`(トークン0個)
- 等価性: トークン列の一致

### PlainCommand

- フィールド: `tokens: Vec<String>`(先頭がプログラム)
- バリデーション: `RawCommand` と同じ生成規則。プレースホルダの検査は行わない(`{...}` を含んでいても文字どおり渡る。requirements §6.3)
- 用途: `judge` / `notify_cmd`
- 等価性: トークン列の一致

### AgentInput

- 直和型: `Prompt(Prompt) | Skill(SkillName)`
- ステータスの動作(エージェント実行)に与える入力。プロンプトかスキルのどちらか一方(requirements §3)
- 等価性: 構造の一致

### Placeholder

- 直和型: `Input | Model | Workspace | Skill`
- `parse(name: &str)`: `input` / `model` / `workspace` / `skill` のみ。それ以外は `UnknownPlaceholder`

### CommandTemplate

- フィールド: `tokens: Vec<TemplateToken>`、`TemplateToken = Vec<Segment>`、`Segment = Literal(String) | Hole(Placeholder)`
- 生成: `parse(raw: &RawCommand, allowed: &[Placeholder]) -> Result<Self, TemplateError>`
  - 各トークンを走査し、`{` から次の `}` までをプレースホルダ名として解釈する。`allowed` に含まれない名前は `TemplateError::UnknownPlaceholder{token, name}`
  - 対応する `}` のない `{`、および空の `{}` は `TemplateError::MalformedBrace{token}`。エスケープ機構は提供しない(リテラルの `{` を含むコマンドは表現できない。この制約は登録時エラーとして顕在化する)
- 振る舞い:
  - `placeholders(&self) -> BTreeSet<Placeholder>` — テンプレートが参照するプレースホルダの集合
  - `expand(&self, values: &PlaceholderValues) -> Result<CommandLine, ExpansionError>` — トークン単位で `Hole` を値に置換する。参照するプレースホルダに値がなければ `ExpansionError::MissingValue(placeholder)`。展開は1パスで、置換された値は再走査しない
- `PlaceholderValues { input: Option<InputText>, model: Option<ModelName>, workspace: Option<PathBuf> }` — `workspace` は素の `PathBuf` で受ける(Task ドメインの `WorktreePath` からの変換は呼び出し側の責務。Definition は他ドメインに依存しない。ADR-017)。`skill` の値は持たない — `Placeholder::Skill` は `SkillInputTemplate::render` 専用で、`cmd` テンプレートには現れない(requirements §3.1)
- 等価性: トークン構造の一致

### CommandLine

- フィールド: `tokens: Vec<String>`(1 以上。先頭がプログラム)
- 生成: `CommandTemplate::expand` の結果としてのみ生成する
- 等価性: トークン列の一致

### RawAgentDefinition と AgentDefinition

グローバル設定の `agents` の各値は、読み込み時には**構造のみ**検証し(文字列 / 配列の形)、テンプレート内容(プレースホルダの正当性)は参照時に検証する(pages「内容の検証は参照時に行う」。setup.md: 壊れたテンプレートを含む config でも他エージェントのタスクは動く)。

- `RawAgentDefinition { cmd: RawCommand, skill_input: Option<String> }`
  - `cmd`: 必須。`RawCommand` の生成規則で構造化する
  - `skill_input`: 任意。単一の文字列テンプレートのまま保持する(トークン列ではない。`{input}` へ流し込まれる値の変換規則であり、コマンドではないため)
- `RawAgentDefinition::parse(&self) -> Result<AgentDefinition, AgentDefError>` — 参照時の検証境界
  - `cmd` を `CommandTemplate::parse(raw, &[Input, Model, Workspace])` で検証
  - `skill_input` があれば `SkillInputTemplate::parse(s)`(許容プレースホルダは `{skill}` のみ)で検証
- `AgentDefinition { cmd: CommandTemplate, skill_input: Option<SkillInputTemplate> }`
  - エラー型:

    ```
    AgentDefError =
      InvalidCmd(TemplateError)          // cmd のテンプレート不備(未知プレースホルダ・波括弧不正)
    | InvalidSkillInput(TemplateError)   // skill_input のテンプレート不備({skill} 以外の参照等)
    | MissingSkillInput                  // skill 入力が要るのに skill_input 未定義
    ```

  - 振る舞い:
    - `render_input(&self, input: &AgentInput) -> Result<InputText, AgentDefError>` — `Prompt(p)` はそのまま `InputText` へ、`Skill(s)` は `skill_input` で変換(`skill_input` 未定義なら `AgentDefError::MissingSkillInput`)
    - `build_command_line(&self, input: &InputText, model: Option<&ModelName>, workspace: &Path) -> Result<CommandLine, ExpansionError>` — `PlaceholderValues` を組んで `cmd.expand` を呼ぶ。値があるのにテンプレートが参照しない場合は許容(単に渡らない。requirements §3.1)

### SkillInputTemplate

- フィールド: `segments: Vec<Segment>`(`Hole` は `Skill` のみ)
- 生成: `parse(s: &str) -> Result<Self, TemplateError>` — CommandTemplate と同じ波括弧規則。許容プレースホルダは `{skill}` のみ
- 振る舞い: `render(&self, skill: &SkillName) -> InputText`
- 等価性: 構造の一致

### GlobalConfig

- フィールド:

  | フィールド | 型 | 制約 |
  |---|---|---|
  | `agents` | `BTreeMap<AgentName, RawAgentDefinition>` | 省略時は空マップ |
  | `notify_cmd` | `Option<PlainCommand>` | 省略時 None(通知しない) |
  | `judge_attempt_limit` | `u32` | 1 以上。省略時 3(判定失敗の再判定上限) |
  | `judge_timeout` | `DurationSpec` | 省略時 60s(判定コマンドのtimeout) |
  | `spawn_fail_limit` | `u32` | 1 以上。省略時 3(連続spawn失敗の上限) |
  | `run_retention` | `Option<DurationSpec>` | 省略時 None(gc無効。ADR-011) |

- すべてのキーは任意(pages 共通事項)。完全に空の config.yaml(空ファイル・null ドキュメント)は「全キー省略」として **`Ok`(全デフォルト)**とする。**未知のキーは構造エラー**とする(typo の無言破棄を防ぐ。ADR-013)。検証は二層である: 構造(YAML構文・キーの正当性・型・期間の形式)は読み込み時、内容(テンプレートのプレースホルダ等)は参照時(pages「内容の検証は参照時に行う」)
- リトライ上限のデフォルト(2)・timeout のデフォルト(1h)・猶予時間(30s)・判定コマンド exit code プロトコル(0 / 10 / 20)は config.yaml のキーではなく**組み込み定数**(ADR-014)

### WorkflowDefinition

ワークフロー名は保持しない(表示名はタスク登録時に WorkflowRef の規則で決定され、`Task.workflow_name` にのみ記録される。定義とタスクで名前が食い違う状態を構造的に排除する)。

- フィールド:

  | フィールド | 型 | 制約 |
  |---|---|---|
  | `default_agent` | `Option<AgentName>` | ワークフロー単位のデフォルトエージェント |
  | `default_model` | `Option<ModelName>` | ワークフロー単位のデフォルトモデル |
  | `initial` | `StatusName` | 必須。`statuses` に存在すること |
  | `statuses` | `BTreeMap<StatusName, StatusDefinition>` | 1 件以上 |

- 不変条件:
  - `initial ∈ statuses`
  - すべての `AgentRun` の `next ∈ statuses`(循環・自己参照・到達不能は許容。ADR-010)
- 振る舞い:
  - `status(&self, name: &StatusName) -> Option<&StatusDefinition>`
  - `effective_agent(&self, status: &StatusName) -> Option<&AgentName>` — ステータスの上書き > `default_agent`
  - `effective_model(&self, status: &StatusName) -> Option<&ModelName>` — 同上
  - `effective_timeout(&self, status: &StatusName) -> TimeoutSpec` — ステータスの指定 > `Limited(1h)`(組み込みデフォルト)
  - `effective_retry_limit(&self, status: &StatusName) -> u32` — `AgentRun` はステータスの `retries` > 2(組み込みデフォルト)、`Cleanup` は常に 2(ADR-014)。`Wait` に対する呼び出しは規定しない(attempt_count を消費する操作が存在せず適用対象がない。呼び出し側が動作種別で分岐してから使う)
  - effective 系(agent / model / timeout / retry_limit)は `AgentRun` 前提のメソッドであり(retry_limit のみ `Cleanup` にも適用)、`Wait` / `Cleanup` にこれらのキーは存在しない(呼び出し側は動作種別で分岐してから使う)

### StatusDefinition(直和型)

```
StatusDefinition =
  AgentRun {
    input:   AgentInput,            // Prompt(Prompt) | Skill(SkillName)
    agent:   Option<AgentName>,     // ワークフローデフォルトの上書き
    model:   Option<ModelName>,
    timeout: Option<TimeoutSpec>,
    retries: Option<u32>,           // リトライ上限の上書き(0 以上。0 = 初回失敗で即 stopped)
    judge:   Option<PlainCommand>,  // 判定コマンド。省略時は exit code 判定
    next:    StatusName,            // 必須
  }
| Wait                              // run: wait
| Cleanup                           // run: cleanup
```

- YAML との対応: 動作は `prompt` / `skill` / `run` の**いずれか1つ**で宣言する。`run` の値は `cleanup` / `wait` のみ
- 許されるキー(ADR-013。これ以外はエラー):
  - `AgentRun`: `prompt` または `skill`(排他)+ `agent` / `model` / `timeout` / `retries` / `judge` / `next`
  - `Wait` / `Cleanup`: `run` のみ

### WorkflowParseError(登録時パースエラーの列挙)

エラー型はドメインが定義するが、`YamlSyntax` / `UnknownKey` の2種は WorkflowStore アダプター(テキスト → `RawWorkflowDoc` の変換時)が生成する。残りは `WorkflowAssembler` が生成する。

```
WorkflowParseError =
  YamlSyntax { message, location }          // YAML として不正(重複キー含む)。アダプターが生成
| UnknownKey { location, key }              // スキーマ外のキー(ADR-013)。アダプターが生成
| ForbiddenKey { status, key }              // 動作種別に無関係なキー(ADR-013)
| MissingInitial                            // initial 欠落
| InitialNotFound { initial }               // initial の参照先なし
| EmptyStatuses                             // statuses が空・欠落
| NoAction { status }                       // 動作宣言なし
| MultipleActions { status, keys }          // prompt / skill / run が複数
| UnknownRunValue { status, value }         // run が cleanup / wait 以外
| MissingNext { status }                    // AgentRun の next 欠落
| NextNotFound { status, next }             // next の参照先なし
| InvalidValue { location, message }        // 名前系・期間・コマンドの生成エラー(NameError / DurationError / CommandError を包む)
```

### WorkflowRef

- 直和型: `Name(WorkflowName) | Path(PathBuf)`
- 生成: `parse(arg: &str) -> Result<Self, NameError>` — 値がパス区切り文字(`/`、Windows では `\` も)を含むか、`.yaml` / `.yml` で終わる場合は `Path`、それ以外は `Name`(ファイルの存在に依存しない決定的規則。pages add)
- 振る舞い: `display_name(&self, declared: Option<&WorkflowName>) -> Result<WorkflowName, NameError>` — タスクに記録するワークフロー名の決定規則(requirements §7)
  - `Name(n)` → `n`(YAML の `workflow:` キーは**使わない**。キーが表示名に使われるのはファイルパス指定のときのみ)
  - `Path(p)` → `declared`(YAML の `workflow:` キーの値)。なければ `p` のファイル名から拡張子を除いたもの(`WorkflowName::parse` を通す。空になる等の不正はエラー)

### WorkflowSnapshot

- フィールド: `WorkflowDefinition`(非公開)を包む newtype
- 生成(2経路):
  - 登録時: `RegistrationValidator::validate` を通過した定義から生成する
  - 永続化からの再構築: `rehydrate(def: WorkflowDefinition) -> Self` — TaskRepository アダプターがデコード時に使う。`WorkflowDefinition` の構造不変条件(`initial ∈ statuses`・`next ∈ statuses`)は `WorkflowDefinition` の生成時に検証済みであり、**グローバル設定との再突き合わせは行わない**(登録後の config.yaml 編集 — エージェント定義の削除等 — は読み込みを失敗させず、実行時の spawn失敗経路として表面化させる。requirements §7.1・setup.md)
- 保証: 構造検証済み(WorkflowParseError なし)。登録時点ではグローバル設定との突き合わせ済み(RegistrationError なし)だが、**登録後の config.yaml 編集との整合は保証しない**(グローバル設定はスナップショットされない。requirements §7.1)
- 振る舞い: `WorkflowDefinition` の全メソッドを委譲。`definition(&self) -> &WorkflowDefinition`
- 直列化: タスクファイルへ正規化された構造として埋め込む(元YAMLテキストではない。ADR-015)

## ドメインサービス

### WorkflowAssembler

- 責務: 構造化済み入力(`RawWorkflowDoc`)を厳格スキーマで `ParsedWorkflow` に組み立てる。**YAML 構文解析はドメインの責務ではない**(ドメインは外部クレートに依存しない。CLAUDE.md): テキスト → `RawWorkflowDoc` の変換と、構文エラー(`YamlSyntax`)・スキーマ外キー(`UnknownKey`)の検出は WorkflowStore アダプターの責務とし、エラー型(`WorkflowParseError`)の定義だけをドメインが持つ(ConfigStore の配置と対称)
- 入力型 `RawWorkflowDoc`(ドメイン定義のプレーンな DTO。所有データのみ・外部クレート非依存):

  ```
  RawWorkflowDoc {
    declared_name:  Option<String>,            // workflow: キー
    default_agent:  Option<String>,            // agent:
    default_model:  Option<String>,            // model:
    initial:        Option<String>,
    statuses:       Vec<(String, RawStatusDoc)>,
  }
  RawStatusDoc {
    prompt: Option<String>, skill: Option<String>, run: Option<String>,
    agent: Option<String>, model: Option<String>,
    timeout: Option<String>, retries: Option<u32>,
    judge: Option<RawCommandDoc>,              // 文字列 or トークン配列(RawCommand の入力)
    next: Option<String>,
  }
  ```

- メソッド: `assemble(doc: RawWorkflowDoc) -> Result<ParsedWorkflow, WorkflowParseError>`
  - `ParsedWorkflow { declared_name: Option<WorkflowName>, definition: WorkflowDefinition }` — `declared_name` の表示名への採否は関知しない(WorkflowRef の `display_name` 規則で呼び出し側が決める)
  - 検証内容: 動作宣言の排他(`NoAction` / `MultipleActions` / `UnknownRunValue`)、動作種別に無関係なキー(`ForbiddenKey`。ADR-013 — `RawStatusDoc` が全キーを保持するため検出できる)、`MissingInitial` / `InitialNotFound` / `EmptyStatuses` / `MissingNext` / `NextNotFound`、値の生成(`InvalidValue`: 名前系・期間・コマンド)
- 依存ポート: なし(純粋)

### RegistrationValidator

- 責務: ワークフロー定義とグローバル設定の突き合わせ検証(登録時検証の定義側。requirements §10)
- メソッド: `validate(def: WorkflowDefinition, config: &GlobalConfig) -> Result<WorkflowSnapshot, Vec<RegistrationError>>`
  - すべての `AgentRun` ステータスについて:
    - 実効エージェント名が解決できること(ステータス上書き > ワークフローデフォルト。どちらもなければ `MissingAgent { status }`)
    - 実効エージェント名が `config.agents` に存在すること(なければ `UnknownAgent { name, defined: Vec<AgentName> }`)
    - `RawAgentDefinition::parse` が成功すること(テンプレート不備は `InvalidAgentDefinition { agent, error }`)
    - `input` が `Skill` の場合、エージェント定義に `skill_input` があること(`MissingSkillInput { status, agent }`)
    - `cmd` が `{model}` を参照する場合、実効モデルが解決できること(`MissingModel { status, agent }`)
  - エラーは全ステータス分をまとめて返す(最初の1件で打ち切らない)
- 依存ポート: なし(純粋。config は値で受け取る)

```
RegistrationError =
  MissingAgent { status }
| UnknownAgent { name, defined }
| InvalidAgentDefinition { agent, error: AgentDefError }
| MissingSkillInput { status, agent }
| MissingModel { status, agent }
```

## ポート

### ConfigStore

- 目的: グローバルホームの config.yaml を読み込み、構造検証済みの `GlobalConfig` を返す
- メソッド: `load(&self) -> Result<GlobalConfig, ConfigLoadError>`
- エラー:
  - `NotFound { home: PathBuf }` — config.yaml が存在しない(「グローバルホームが未初期化」。解決後のホームパスを含む。pages ※1)
  - `Invalid { message, location }` — YAML構文エラー・構造エラー(未知キー・型不一致・不正な期間等)
  - `Io { message }` — 存在するが読めない(権限不足・I/O障害)。pages 縮退表では ※1 と同じ非0系に写像する
- 契約:
  - 読み取り専用。ロック不要
  - テンプレート内容(プレースホルダ)の検証は行わない(参照時検証の原則)。構造(cmd が文字列 / 配列であること等)までを担保する
  - デコード(YAML → ドメイン型)はアダプター境界の責務
  - 可視性: 呼び出し時点のファイル内容を返す(キャッシュしない。グローバル設定は各実行時に解決される。requirements §7.1)

### WorkflowStore

- 目的: `WorkflowRef` からワークフロー定義を解決・読み込み・パースする
- メソッド: `load(&self, wf_ref: &WorkflowRef) -> Result<LoadedWorkflow, WorkflowLoadError>`
  - `LoadedWorkflow { parsed: ParsedWorkflow, resolved_from: PathBuf }` — `resolved_from` は実際に読み込んだ絶対パス(add の成功表示「解決したワークフロー名と解決先」(pages)の供給元。名前解決の知識をポートの内側に閉じる)
- エラー:
  - `NotFound { attempted: PathBuf }` — 解決を試みた絶対パスを含む(pages add の案内用)
  - `Parse(WorkflowParseError)`
  - `Io { message }` — 存在するが読めない(権限不足・I/O障害)
- 契約:
  - 名前解決の規則: `Name(n)` → `<home>/workflows/<n>.yaml`(固定。`.yml` へのフォールバックはしない)。`Path(p)` → そのパス(相対はプロセスのカレントディレクトリから解決)
  - アダプターが YAML テキストを `RawWorkflowDoc` に変換し(構文エラー・スキーマ外キーはここで `YamlSyntax` / `UnknownKey` として検出)、ドメインの `WorkflowAssembler::assemble` で検証する。表示名の決定はしない(呼び出し側が `WorkflowRef::display_name` で行う)
  - 読み取り専用。可視性: 呼び出し時点のファイル内容
  - 一意性・並行性: 関与しない(読むだけ)

## ユースケース(概要)

このドメインの型・サービスは以下のユースケースから使われる(詳細は Phase 2):

- add: WorkflowRef 解決 → WorkflowStore.load → `display_name` で表示名決定 → RegistrationValidator.validate → WorkflowSnapshot をタスクへ埋め込む
- tick(起動): スナップショットの実効値解決 + AgentDefinition の参照時パースとテンプレート展開(失敗は同期spawn失敗経路。ADR-016)
- tick(判定・通知): `judge` の PlainCommand、`notify_cmd`、judge_attempt_limit / judge_timeout / spawn_fail_limit / run_retention の参照
- 全コマンド: ConfigStore.load(起動時の設定読み込み)
