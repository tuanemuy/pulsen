# ADR: ワークフロー定義エラーの「どのファイルか」は自由形式のメッセージに載せる (置き換え済み)

## ステータス

置き換え済み(Issue #9)。回避策の前提だった「ポート表を変えられない」が消えたため、解決先は `WorkflowLoadError::Parse { resolved_from }` の構造化フィールドが持つ。自由形式のメッセージへの前置に残るのは `Io { message }` のみ

## コンテキスト

`.adr/1-schema-error-location-is-logical.md` は「スキーマ違反は対象ファイルの絶対パスと論理位置で示す」と決めている。config.yaml は読む対象が1つに定まるため CLI の文言層が案内にパスを補えるが、ワークフロー定義は `--workflow` の解釈で解決先が決まり、解決先を知っているのはストアのアダプターだけである。

一方、パスを載せられる場所は限られている。`WorkflowLoadError`(`NotFound` / `Parse` / `Io`)と `WorkflowParseError` の12種は spec のポート表で確定しており、フィールドを増やすとポート表との1:1一致が壊れる。`UnknownKey` / `InvalidValue` の `location` はポートの契約として論理位置そのもの(`statuses.queued.prompt`)を指し、適合テストが値の一致で固定している。

## 決定

解決先の絶対パスは、意味が固定されていない**自由形式のメッセージ**にだけ前置する。

- `WorkflowLoadError::Io { message }` — `<絶対パス>: <原因>`
- `WorkflowParseError::YamlSyntax { message }` — 同上
- `NotFound { attempted }` は既にパスを持つ

`location` には載せない。構造の破れ(`NoAction` / `MissingNext` 等)はステータス名で位置を示し、パスは持たせない。

## 検討した代替案

- `WorkflowLoadError` にパスのフィールドを足す — spec のポート表と食い違う
- `location` にパスを前置する — `location` の意味(論理位置)が呼び出し側ごとに揺れる

## 影響

- 利用者が最も困る「存在するが読めない」「YAML として壊れている」の2つで、どのファイルの話かが必ず出る
- トレードオフ: 構造の破れではパスが出ない。これらは定義の中の位置(ステータス名)で特定でき、対象ファイルは利用者が `--workflow` で指定した1つに限られる
- Issue #9 で、退けた代替案「`WorkflowLoadError` にパスのフィールドを足す」を採った。ポート表の側を `Parse { error: WorkflowParseError, resolved_from: PathBuf }` に改められたため、退けた理由(ポート表との1:1一致が壊れる)が成立しなくなったからである。パースの失敗は構造の破れを含めてすべて解決先を伴うようになり、上のトレードオフは解消した。`YamlSyntax { message }` への前置は構造化フィールドとの二重表示になるため外し、前置に残るのは `Io { message }` だけになった
- 置き換え後の規則(解決先は構造化フィールドで示し、示せない経路だけ自由形式へ前置する)は `spec/domains/definition.md#workflowstore` のポート契約が持つ。ワークフロー定義のエラー種を増やす後続の変更は、そちらを基準に判断する
