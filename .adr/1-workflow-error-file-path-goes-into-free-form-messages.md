# ADR: ワークフロー定義エラーの「どのファイルか」は自由形式のメッセージに載せる

## ステータス

置き換え済み(Issue #9)。回避策の前提だった「ポート表を変えられない」が消えたため、解決先は `WorkflowLoadError::Parse { resolved_from }` の構造化フィールドが持つ。自由形式のメッセージへの前置に残るのは `Io { message }` のみ

## コンテキスト

`.adr/1-schema-error-location-is-logical.md` は「スキーマ違反は対象ファイルの絶対パスと論理位置で示す」と決めている。config.yaml は読む対象が1つに定まるため CLI の文言層が案内にパスを補えるが、ワークフロー定義は `--workflow` の解釈で解決先が決まり、解決先を知っているのはストアのアダプターだけである。

一方、パスを載せられる場所は限られている。`WorkflowLoadError`(`NotFound` / `Parse` / `Io`)と `WorkflowParseError` の12種は spec のポート表で確定しており、フィールドを増やすとポート表との1:1一致が壊れる。`UnknownKey` / `InvalidValue` の `location` はポートの契約として論理位置そのもの(`statuses.queued.prompt`)を指し、適合テストが値の一致で固定している。

## 決定

解決先の絶対パスは、**構造化フィールドで示せる経路では構造化フィールドが持つ**。示せない経路にかぎり、意味が固定されていない自由形式のメッセージへ前置する。両方で示すと同じパスが1つの案内に2回現れるためである。

- `WorkflowLoadError::Parse { error: WorkflowParseError, resolved_from: PathBuf }` — 解決先は `resolved_from` が持つ。`WorkflowParseError` の12種はどれもパスを持たないので、`YamlSyntax { message }` にも前置しない
- `WorkflowLoadError::NotFound { attempted }` — 解決を試みたパスを構造として持つ
- `WorkflowLoadError::Io { message }` — 前置が残るのはこの1つだけ(`<絶対パス>: <原因>`)

`location` には載せない。`location` はポートの契約として論理位置そのもの(`statuses.queued.prompt`)を指し、適合テストが値の一致で固定しているためである(`.adr/1-schema-error-location-is-logical.md`)。構造の破れ(`NoAction` / `MissingNext` 等)はステータス名で位置を示し、対象ファイルは `Parse` の `resolved_from` が受ける。

## 検討した代替案

- `location` にパスを前置する — `location` の意味(論理位置)が呼び出し側ごとに揺れる

## 影響

- 利用者が最も困る「存在するが読めない」「YAML として壊れている」の2つで、どのファイルの話かが必ず出る
- Issue #9 の改訂により、パースの失敗はすべて(構造の破れを含めて)解決先を伴うようになった。当初のトレードオフ「構造の破れではパスが出ない」は解消した
- ワークフロー定義のエラー種を増やす後続の変更では、解決先を構造化フィールドで示せるかどうかを基準に判断する。示せるなら自由形式へ前置しない
