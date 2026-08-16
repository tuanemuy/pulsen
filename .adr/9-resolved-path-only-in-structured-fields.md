# ADR: 解決先パスは構造化フィールドで示し、自由形式メッセージへの前置は構造化フィールドを持たない変種にだけ残す

## ステータス

承認済み

## コンテキスト

`.adr/1-workflow-error-file-path-goes-into-free-form-messages.md` は「解決先の絶対パスは意味の固定されていない自由形式のメッセージにだけ前置する」と決めていた。その回避策が成り立つ前提は「`WorkflowLoadError` の3種と `WorkflowParseError` の12種は spec のポート表で確定しており、フィールドを増やすとポート表との1:1一致が壊れる」だった。Issue #9 の spec 追従でポート表そのものを `Parse { error: WorkflowParseError, resolved_from: PathBuf }` へ改めたため、この前提は消えた。

前提が消えた結果、どの変種が解決先をメッセージへ前置するかを決め直す必要が出た。`WorkflowParseError` の12種(`YamlSyntax` / `UnknownKey` / `ForbiddenKey` / `MissingInitial` ほか)はすべて `Parse` の内側にあるため、`Parse { resolved_from }` の1フィールドで12種すべての解決先が示せる。この状態で `YamlSyntax { message }` の前置を残すと、構文エラーの案内に同じ絶対パスが2回並ぶ。

## 前提

- 解決先を知っているのはストアのアダプターだけで、CLI は `WorkflowRef` の解決結果を持たない
- `location` はポートの契約として論理位置そのもの(`statuses.queued.prompt`)を指し、適合テストが値の一致で固定している(`.adr/1-schema-error-location-is-logical.md`)
- ポート機構の失敗は分類に使わない単一の `Io { message }` で報告する(`.adr/2-port-mechanism-failure-is-single-io-error.md`)

## 決定

**構造化フィールドで解決先を示せる経路は、自由形式のメッセージへ前置しない。前置を残すのは構造化フィールドを持たない変種だけ。**

- `NotFound { attempted }` / `Parse { resolved_from }` — 解決先は構造化フィールドが示す。内側の `WorkflowParseError` 12種はパスを持たず、`YamlSyntax { message }` の前置も外して `message` は原因のみにする
- `Io { message }` — 前置を残す。解決先を載せる構造上の置き場がなく、CLI 側も解決先を知らないため、ここだけは自由形式のメッセージが唯一の運び手になる

変種ごとに前置の要否を個別に決めるのではなく、**「構造化フィールドで示せるなら前置しない」という規則**にする。エラー種が増えても判断が要らず、二重表示が構造として起こらない。

現行の規則そのものは `spec/domains/definition.md#workflowstore` のポート契約が持つ(`.adr/9-superseded-adr-keeps-its-original-decision.md`)。

## 検討した代替案

- **変種ごとに前置の要否を個別に決める(従来の形)** — エラー種が増えるたびに同じ判断を繰り返すことになり、構造化フィールドを持つ変種に前置が付く経路を規則が禁じない。二重表示は実際に `YamlSyntax` で起きていた
- **`Io { message }` にも解決先のフィールドを足し、前置を全廃する** — `Io` は「存在するが読めない」機構の失敗で、呼び出し側が分類に使わない不透明な報告である。前置を1箇所消すために、共有してよい条件(`.adr/2-port-mechanism-failure-is-single-io-error.md`)の外へ構造を足すことになる
- **`location` にパスを前置する** — `location` の意味(論理位置)が呼び出し側ごとに揺れる。ポート表を改める前から退けている

## 影響

- 同じパスが1つの案内に2回現れない。解決先の出所が `cli::render` の1箇所に定まり、表示の形を変えるときの変更先も1箇所になる
- `.adr/1-schema-error-location-is-logical.md` が残していた「スキーマ違反ではパスが出ない」トレードオフが、前置の有無で変種ごとに挙動が割れない形で解消する
- 前置を担う `at()` の利用者が `Io` の1経路に減る。関数として残す価値は薄くなるが、`read_error` の中へ畳む整理は解決先の示し方とは別の判断になる
- トレードオフ: 「構造化フィールドを持つか」で前置が決まるため、ポート表にフィールドを足す変更は表示の形も同時に動かす。両者の対応はポートの適合テストが固定する
