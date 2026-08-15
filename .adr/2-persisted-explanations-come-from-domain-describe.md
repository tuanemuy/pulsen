# ADR: 帳簿に残る不備の説明はドメインの `describe` に1箇所だけ置く

## ステータス

承認済み

## コンテキスト

エージェント起動時のテンプレート展開失敗は `record_spawn_failure_in_place(message, ...)` で**タスクファイルに残る**。この `message` はポートが返す不透明な文字列ではなく、ドメインのエラー型（`AgentDefError` / `TemplateError` / `ExpansionError`）から組み立てるしかない。

一方 `cli/render.rs` には同じ3型を文言に落とす private 関数が既にあった。ユースケース側で別の文言を組むと、同じ不備が「登録時の案内」と「タスクファイルの失敗要因」で違う言葉になる。

`.adr/2-tick-errors-are-structured-values.md` の「文言は CLI 層で組み立てる」は tick のサマリー（`errors`）に対する規約であって、帳簿に永続化される値には適用できない — 帳簿は CLI を経由せずに読まれる。

## 決定

`TemplateError::describe` / `AgentDefError::describe` / `ExpansionError::describe` をドメインに置き、`cli/render.rs` の private 関数を削除して委譲させる。既存の `NameError` / `BranchNameError` / `AbsolutePathError` / `TaskIdError` / `AttemptNumberError` が「説明の定義箇所をドメインに1つ置く」という doc とともに持っているのと同じ形にする。

ユースケースは、経路ごとの文脈（どのステータスか・どのエージェント名か）を添えたうえで `describe()` を埋め込む。文脈の付与は報告側の責務であり、報告用の分類値に対象のパスを持たせない判断と対称になる。

## 影響

- 同じ不備が登録時の案内でも失敗要因の記録でも同じ言葉で読める
- 帳簿に残る `message` の定義箇所が「ドメインの説明 + ユースケースの文脈」の2段に固定され、層をまたいだ重複が消える
- トレードオフ: ドメインに表示用の文字列が1組増える。既存の `describe` 群と同じ位置づけなので新しい種類の責務ではない
