# ADR: 実行の失敗の根拠と残存の後始末も分類として持ち、残存の報告は保存の成否から切り離す

## ステータス

承認済み

## コンテキスト

`.adr/2-tick-errors-are-structured-values.md` は tick の `errors` を分類として持ち、文言は `cli::render` が組むと決めている。`TickIssue` のうち `RunFailed { message: String }` と `RemnantsUnhandled { message: String }` の2つだけが、この規約の外でユースケースが完成文言を組んでいた。

規約から外れた結果が実害に出ていた。失敗の文言は `judgement_detail(&exit)` が組んでいたが、この `exit` はエージェントの終了コードである。判定コマンドが exit 10(失敗)を返し、エージェント自身は 0 で終わっていた場合、報告は「実行が終了コード 0 で終了しました」になる。失敗と判断した主体が判定コマンドであることも、その終了コードが 10 だったことも読めない。

`RemnantOutcome` には `Killed` があり、これは後始末を残さない。報告の分類が文字列だったため、「後始末が残った」報告に `Killed` を載せる経路を型が禁じていなかった。

## 前提

- `errors` の分類は `cli::render` の網羅 `match` が受け、文言は表示側が組む(`.adr/2-tick-errors-are-structured-values.md`)
- ポート機構の失敗は単一の `Io` 文字列で表される(`.adr/2-port-mechanism-failure-is-single-io-error.md`)ため、そこから先は分類できない

## 決定

**どちらも分類にする。`RunFailed { cause: RunFailureCause }` と `RemnantsUnhandled { remnants: RemnantsLeft }`。**

- `RunFailureCause` は**判断の主体**で分ける(`DefaultJudgement { exit }` / `JudgeCommand { exit }` / `TimedOut { timeout }` / `DiedWithoutExit`)。`JudgeCommand` も exit を運ぶが、それは判定コマンドが受け取った材料であって失敗の根拠ではない。`cli::render` は「判定コマンドが失敗と判定しました(実行の終了コードは N)」と組み、主体と材料を書き分ける
- 判定の結末はユースケース内の `Settled` に一度写し、失敗のときだけ根拠を伴わせる。「誰が失敗と判断したか」は結論と同時にしか分からず、結末だけを見て後から復元できない
- `RemnantsLeft` は `RemnantOutcome` をそのまま運ばず、報告を要する2値(`NotIdentifiable` / `Failed { message }`)へ写す。写像は `Option` を返す関数1つに閉じ、`Killed` がこの分類に現れる状態を型で表現不能にする

**あわせて、残存の報告を保存の成否から切り離す。** プロセスが残っているという事実はタスクファイルを書けたかと直交する — 後始末は人間が OS のツールで行うので、保存に失敗した tick でも報告する。

## 検討した代替案

- **完成文言を文字列で運ぶ(従来の形)** — `.adr/2-tick-errors-are-structured-values.md` の規約の外に2変種が残り、失敗の主体を取り違えた文言が実際に出ていた。テストも文言でしか主張できない
- **`RemnantOutcome` をそのまま報告に載せる** — 後始末を残さない `Killed` が「後始末が残った」報告に現れる経路を型が禁じない
- **残存の報告を保存に成功した tick だけに積む** — 保存の失敗でプロセスの残存という事実が消える。2つは直交した事実である

## 影響

- `errors` の全変種が `.adr/2-tick-errors-are-structured-values.md` の規約に揃い、規約の外にある変種が1つも無くなる
- exit の出所が表示から読み分けられ、判定コマンドの失敗をエージェントの終了コードのせいに読む余地が消える
- ユースケース層のテストが文言ではなく根拠の分類で主張でき、`TimedOut` と `DiedWithoutExit` の取り違えを検出できる
- トレードオフ: `RemnantsLeft::Failed` は原因の説明を文字列で運ぶ。ポート機構の失敗は単一の `Io` 文字列で表す(`.adr/2-port-mechanism-failure-is-single-io-error.md`)ので、ここでこれ以上は分類できない
- トレードオフ: 保存に失敗した tick で、1つのタスクが `SaveFailed` と `RemnantsUnhandled` の両方に現れうる。どちらも別の事実の報告なので重複ではない
