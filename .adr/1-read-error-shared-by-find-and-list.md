# ADR: 走査系ポートメソッドの読み取りエラーは `ReadError` に統一する

## ステータス

承認済み

## コンテキスト

spec/domains/task.md の TaskRepository のポート表は、`find` の失敗を `ReadError`(`Io` のみ)とし、`list_active` / `list_archived` の失敗を `Io` と書いている。`Io` という独立した型は spec のどこにも定義がなく、spec/inventory/domain.md にも `ReadError`(DOM-task-068)だけが行として立っている。

## 決定

`find` / `list_active` / `list_archived` の3メソッドがいずれも `ReadError` を返す。`ReadError` は `Io { message }` の1種のみとし、「個別のタスクファイルの破損はエラーではなく結果の値(`TaskLookup::Corrupt` / `TaskEntry::Corrupt`)として返る」という契約をポートのドキュメンテーションコメントに書く。

## 検討した代替案

- `ListError { Io { message } }` を別に定義する — 台帳に無い型が1つ増えるうえ、`ReadError` との違いを説明できない。アダプターと適合テストが同じ内容の分岐を2箇所に持つことになる

## 影響

- 台帳(DOM-task-068)と1対1のまま、読み取り経路のエラーが1つの型に閉じる
- アダプターと適合テストが入出力エラーの分岐を1箇所しか持たない
- トレードオフ: spec のポート表の綴り(`Io`)と字面が一致しない。表の意図(読み取り経路の失敗は入出力エラーだけ)は保たれる
