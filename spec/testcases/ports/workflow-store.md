# 適合テスト: WorkflowStore

対象の契約: [Definition ドメイン: WorkflowStore](../../domains/definition.md#workflowstore)(関連: [WorkflowAssembler](../../domains/definition.md#workflowassembler)、[WorkflowParseError](../../domains/definition.md#workflowparseerror登録時パースエラーの列挙)、ADR-010・013)

WorkflowStore のすべてのアダプター実装が共通で通す適合テストスイート。前提条件は、契約に書かれたフィクスチャ(グローバルホーム配下 `workflows/` または任意パスへのワークフローYAMLの配置)とポートのメソッド呼び出しのみで組み立てる。パースは「アダプターが YAML → `RawWorkflowDoc` に変換(`YamlSyntax` / `UnknownKey` を検出)し、ドメインの `WorkflowAssembler::assemble` で検証する」構成が契約であるため、厳格パース(ADR-013)の全エラー種もポート越しに検証する。

## 名前解決

| 前提条件 | 操作 | 期待結果 | 実装ステータス |
|---|---|---|---|
| `workflows/impl.yaml` として有効な定義を置く | `load(Name("impl"))` | `Ok(LoadedWorkflow)`。`resolved_from` = `<home>/workflows/impl.yaml` の絶対パス | |
| `workflows/impl.yml`(拡張子 `.yml`)のみを置く | `load(Name("impl"))` | `Err(NotFound)`。`attempted` = `<home>/workflows/impl.yaml`(`.yml` へのフォールバックはしない) | |
| `workflows/` に該当ファイルが無い(ディレクトリ自体の不在を含む) | `load(Name("missing"))` | `Err(NotFound)`。`attempted` = `<home>/workflows/missing.yaml` の絶対パス(add の案内用) | |
| 任意の場所に有効な定義ファイルを置く | `load(Path(絶対パス))` | `Ok`。`resolved_from` = そのパス | |
| プロセスのカレントディレクトリからの相対位置に有効な定義ファイルを置く | `load(Path(相対パス))` | `Ok`。相対パスはカレントディレクトリから解決され、`resolved_from` は実際に読み込んだ絶対パス | |
| 指定パスにファイルが無い | `load(Path(不在のパス))` | `Err(NotFound)`。`attempted` = 解決を試みた絶対パス | |

## 正常系パース

| 前提条件 | 操作 | 期待結果 | 実装ステータス |
|---|---|---|---|
| `workflow:` キーを持つ有効な定義(`initial`・AgentRun(`prompt`)・`run: wait`・`run: cleanup` を含む)を置く | `load` | `Ok`。`parsed.declared_name` = `workflow:` の値、`definition` に initial・statuses が正規化されて入る。表示名の決定は行われない(呼び出し側の `WorkflowRef::display_name`) | |
| トップレベルに `agent` / `model`(ワークフローデフォルト)を持つ有効な定義を置く | `load` | `Ok`。`definition.default_agent` / `default_model` に値が取り込まれる(黙って落とさない — 実効値解決(ステータス > ワークフローデフォルト)の入力になる) | |
| `workflow:` キーの無い有効な定義を置く | `load` | `Ok`。`declared_name` = None | |
| `skill` 指定のステータスと、`agent` / `model` / `timeout` / `retries` / `judge` / `next` の全キーを使うステータスを含む定義を置く | `load` | `Ok`。各値が対応するドメイン型(`TimeoutSpec`・`PlainCommand` 等)に落ちる | |
| `timeout: none` を指定したステータスを含む定義を置く | `load` | `Ok`。当該ステータスの timeout は `Unlimited` | |
| `retries: 0` を指定したステータスを含む定義を置く | `load` | `Ok`。0 は正当な値(0 = 初回失敗で即 stopped。config.yaml の「1 以上」の規則とは異なる) | |
| `next` が自ステータスを指す(自己参照)定義・複数ステータスで循環を成す定義を置く | `load` | `Ok`(循環・自己参照は正当な表現。ADR-010) | |
| 遷移経路のない到達不能ステータスを含む定義を置く | `load` | `Ok`(到達不能ステータスは許容) | |
| `judge` のトークンに波括弧(`{...}`)を含む定義を置く | `load` | `Ok`。`PlainCommand` はプレースホルダ展開・検査をせず文字どおり保持される | |
| グローバル設定に存在しないエージェント名を参照する定義を置く | `load` | `Ok`(グローバル設定との突き合わせは本ポートの責務外。`RegistrationValidator` が担う) | |

## パースエラー(ADR-013 の全エラー種)

| 前提条件 | 操作 | 期待結果 | 実装ステータス |
|---|---|---|---|
| YAML として不正な内容(構文エラー・重複キー)のファイルを置く | `load` | `Err(Parse(YamlSyntax))`。message・location を含む | |
| トップレベルに許容外のキー(`workflow` / `agent` / `model` / `initial` / `statuses` 以外)を含む定義を置く | `load` | `Err(Parse(UnknownKey))` | |
| ステータス内にスキーマ外のキー(`prmopt` 等の typo)を含む定義を置く | `load` | `Err(Parse(UnknownKey))` | |
| `run: wait` / `run: cleanup` のステータスにエージェント実行系のキー(`judge`・`next` 等)を併記した定義を置く | `load` | `Err(Parse(ForbiddenKey))`(`Wait` / `Cleanup` に許されるキーは `run` のみ) | |
| `initial` キーの無い定義を置く | `load` | `Err(Parse(MissingInitial))` | |
| `initial` が `statuses` に無い名前を指す定義を置く | `load` | `Err(Parse(InitialNotFound))` | |
| `statuses` が空・欠落した定義を置く | `load` | `Err(Parse(EmptyStatuses))` | |
| 動作宣言(`prompt` / `skill` / `run`)の無いステータスを含む定義を置く | `load` | `Err(Parse(NoAction))` | |
| 動作宣言が複数あるステータス(`prompt` と `skill`、`prompt` と `run` 等)を含む定義を置く | `load` | `Err(Parse(MultipleActions))` | |
| `run` の値が `cleanup` / `wait` 以外の定義を置く | `load` | `Err(Parse(UnknownRunValue))` | |
| AgentRun ステータスに `next` の無い定義を置く | `load` | `Err(Parse(MissingNext))` | |
| `next` が `statuses` に無い名前を指す定義を置く | `load` | `Err(Parse(NextNotFound))` | |
| 値の生成エラーを含む定義(空の `prompt`、`timeout: 0s`、空文字列の `judge`、前後空白を含むステータス名等)を置く | `load` | `Err(Parse(InvalidValue))`(`NameError` / `DurationError` / `CommandError` を包む) | |

## エラー・可視性

| 前提条件 | 操作 | 期待結果 | 実装ステータス |
|---|---|---|---|
| ファイルは存在するが読み取れない(権限不足等。再現できるアダプター環境に限る) | `load` | `Err(Io)`。message を含む | |
| `load` に成功した後、同じファイルを別の有効な定義に書き換える | 再度 `load` | 書き換え後の定義が返る(呼び出し時点のファイル内容。スナップショットは呼び出し側の責務) | |

## 対象外

- 一意性・並行性: 読み取り専用のため関与しない(契約どおり)
- 表示名の決定(`WorkflowRef::display_name`)・グローバル設定との突き合わせ(`RegistrationValidator`): ドメインテストが担う
