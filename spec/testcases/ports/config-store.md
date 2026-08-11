# 適合テスト: ConfigStore

対象の契約: [Definition ドメイン: ConfigStore](../../domains/definition.md#configstore)(関連: [GlobalConfig](../../domains/definition.md#globalconfig)、ADR-013・014)

ConfigStore のすべてのアダプター実装が共通で通す適合テストスイート。前提条件は、契約に書かれたフィクスチャ(グローバルホーム直下の `config.yaml` の配置・内容)とポートのメソッド呼び出しのみで組み立てる。永続化技術の実装詳細には依存しない。

## 正常系とデフォルト値

| 前提条件 | 操作 | 期待結果 | 実装ステータス |
|---|---|---|---|
| 全キー(`agents` / `notify_cmd` / `judge_attempt_limit` / `judge_timeout` / `spawn_fail_limit` / `run_retention`)を有効な値で記述した config.yaml を置く | `load` | `Ok(GlobalConfig)`。記述した各値がフィールドに反映される | |
| キーを1つも持たない(空マッピングの)config.yaml を置く | `load` | `Ok`。デフォルト値が適用される: `agents` = 空マップ、`notify_cmd` = None、`judge_attempt_limit` = 3、`judge_timeout` = 60s、`spawn_fail_limit` = 3、`run_retention` = None | |
| 完全に空の config.yaml(空ファイル・null ドキュメント)を置く | `load` | `Ok`。「全キー省略」として空マッピングと同じ全デフォルトが適用される(`Err(Invalid)` にしない) | |
| 一部のキーのみ(例: `spawn_fail_limit: 5`)を記述した config.yaml を置く | `load` | `Ok`。記述したキーはその値、未記述のキーはデフォルト値 | |
| 期間キーに単位の異なる等価値を記述する(例: `judge_timeout: 1m`) | `load` | `Ok`。`DurationSpec` は秒数に正規化され、`1m` は `60s` と等価 | |
| `agents` の `cmd` を文字列形式(連続空白・クォートを含む。例: `sh  -c "echo hi"`)で記述する | `load` | `Ok`。単純な空白分割(連続空白は1区切り)でトークン化され、クォートはグルーピングとして解釈されない(`"echo` と `hi"` は別トークン) | |
| `agents` の `cmd` を配列形式(空文字列トークンを含む)で記述する | `load` | `Ok`。各要素がそのままトークンになり、空文字列トークンは配列形式でのみ許容される | |
| `notify_cmd` を文字列形式で記述した config と配列形式で記述した config をそれぞれ置く | それぞれ `load` | いずれも `Ok`。`RawCommand` と同じ生成規則でトークン化され `Some(PlainCommand)` になる | |

## 参照時検証の原則(内容は load で検証しない)

| 前提条件 | 操作 | 期待結果 | 実装ステータス |
|---|---|---|---|
| `cmd` に未知プレースホルダ(例: `{foo}`)を含むエージェント定義を記述する | `load` | `Ok`。テンプレート内容は load では検証されず、`RawAgentDefinition` として保持される(検証は参照時の `RawAgentDefinition::parse`) | |
| `cmd` に波括弧不正(対応する `}` のない `{`、空の `{}`)を含むエージェント定義を記述する | `load` | `Ok`(同上。壊れたテンプレートを含む config でも load は通る) | |
| `skill_input` に `{skill}` 以外のプレースホルダ(例: `{input}`)を含むエージェント定義を記述する | `load` | `Ok`(同上) | |
| `notify_cmd` のトークンに波括弧(例: `{task}`)を含める | `load` | `Ok`。`PlainCommand` はプレースホルダ検査を行わず、文字どおり保持される | |

## エラー

| 前提条件 | 操作 | 期待結果 | 実装ステータス |
|---|---|---|---|
| config.yaml が存在しない | `load` | `Err(NotFound)`。解決後のグローバルホームパスを含む | |
| YAML として不正な内容(構文エラー)の config.yaml を置く | `load` | `Err(Invalid)`。message と location を含む | |
| スキーマに無いトップレベルキー(例: `run_retension` のような typo)を含む config.yaml を置く | `load` | `Err(Invalid)`(未知キーは構造エラー。ADR-013) | |
| 組み込み定数に相当するキー(`retries` / `timeout` 等)をトップレベルに記述する | `load` | `Err(Invalid)`(リトライ上限・timeout のデフォルトは config.yaml のキーではない。ADR-014) | |
| `agents` のエントリ内に `cmd` / `skill_input` 以外のキーを記述する | `load` | `Err(Invalid)` | |
| `agents` のエントリに `cmd` キーが無い(例: `skill_input` のみを記述する) | `load` | `Err(Invalid)`(`cmd` は必須。キーの有無は構造であり読み込み時に検証する — 参照時まで遅延しない) | |
| 型不一致(例: `judge_attempt_limit` に文字列、`agents` の `cmd` に数値)を含む config.yaml を置く | `load` | `Err(Invalid)` | |
| `judge_attempt_limit: 0` または `spawn_fail_limit: 0` を記述する | `load` | `Err(Invalid)`(1 以上の制約) | |
| 期間形式の不正(`0s`・未知の単位・単位なし・空白混入)を `judge_timeout` / `run_retention` に記述する | `load` | `Err(Invalid)`(`DurationError` 相当の構造エラー) | |
| `agents` の `cmd` に空文字列または空配列、あるいは `notify_cmd` に空文字列を記述する | `load` | `Err(Invalid)`(トークン0個。`CommandError::Empty` 相当) | |
| config.yaml が存在するが読み取れない(権限不足等。再現できるアダプター環境に限る) | `load` | `Err(Io)`。message を含む | |

## 可視性

| 前提条件 | 操作 | 期待結果 | 実装ステータス |
|---|---|---|---|
| 有効な config.yaml で `load` に成功した後、内容を別の有効な内容に置き換える | 再度 `load` | 置き換え後の内容が返る(キャッシュしない。呼び出し時点のファイル内容。グローバル設定は各実行時に解決される) | |

## 対象外

- 並行性: 読み取り専用・ロック不要のため、調停に関する検証はない
- テンプレート内容の正否判定そのもの: `RawAgentDefinition::parse` / `RegistrationValidator` のドメインテストが担う(本ポートは「load で検証しない」ことのみを検証する)
