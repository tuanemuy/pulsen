# 096: `TransitionError` は分類だけを持ち、文言は `cli::render` が組み立てる

## ステータス

承認済み

## コンテキスト

`TransitionError::InvalidState` の `expected` には `"pending | failed"` という表示用の文字列が入り、`InvariantViolated { message: String }` は日本語の完成文をドメインが作っていた。ADR-081 は tick の報告について「文言は CLI 層で組み立てる」と決め、報告用の分類値からは完成文言を外している。

`describe` 系（ADR-090）は「タスクファイルに失敗要因として永続化される」という why が付くので別扱いだが、`TransitionError` は永続化されず表示にしか使われないため、その why が効かない。

## 決定

前提の検査はドメインに残したまま、完成文言をドメインから落とす。

- `InvalidState.expected` は `&'static [ExecutionStateKind]`（前提として受理される実行状態そのもの）にする
- `InvariantViolated { message: String }` は破れの種別を表す `MissingCurrentAttempt` に置き換える
- `cli::render` が `ExecutionStateKind::as_str()` を連結して `"pending | failed"` を組み立て、`MissingCurrentAttempt` の説明文も持つ

前提の検査をドメインに残すのは、遷移関数を全域に保つことが「不正な状態を型で表現不能にする」の一部だからである。

## 検討した代替案

- **前提の検査を呼び出し側に移し、`TransitionError` から該当の変種を落とす** — 検査が呼び出し側に散り、手続きが増えるたびに検査の書き漏らしが起こりうる。手動修復で破れた帳簿がドメインを素通りする

## 影響

- 「表示専用のエラーは分類だけを持つ」という規則がドメインのエラー型にも通る。表示の変更がドメインに触れずに済む
- `expected` がデータになり、前提の実行状態を足しても文言が自動で追随する
- トレードオフ: `spec/domains/task.md` のエラー型定義（`expected: &'static str` / `InvariantViolated { message: String }`）と一致しないため、spec 追従が必要になる
- 遷移エラーの `MissingCurrentAttempt` と tick の報告分類の `MissingCurrentAttempt` は、同じ事実の別文脈での報告になる（手続きは観測前に検出し、遷移関数は前提として検査する）。どちらも分類なので、文言の重複は `cli::render` の中だけに閉じる
