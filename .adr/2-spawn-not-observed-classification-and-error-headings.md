# ADR: 猶予超過の spawn 失敗を独立した分類にし、記録した失敗をスキップと別の見出しで表示する

## ステータス

承認済み

## コンテキスト

`TickIssue::SpawnFailed` は結末の異なる2つの経路を兼ねていた — (a) `spawn_wrapper` の同期エラー（状態不変・`Launching` 維持・カウンタ非消費・猶予経路が後で分類する）と、(b) 猶予超過の再確認で確定した spawn 失敗（`Pending` 復帰・`spawn_fail_count` 加算・上限超過なら凍結）。運用上は「まだ起動中かもしれない」と「起動をあきらめてカウンタを1つ消費した」で次に取る行動が違い、テストが分類だけを主張しても取り違えを検出できない。

同じ問題が表示にもある。`cli::render` は `errors` をすべて「スキップ」の見出しに束ねていたが、記録した失敗はタスクファイルに書き込んだ後の報告で、カウンタを消費し上限を超えれば同じ tick で凍結する。`.adr/2-empty-summary-means-nothing-to-process.md` は「cron 運用ではこの出力が唯一の窓」を根拠に「処理対象なし」の判定を設計しており、その窓が記録した失敗と素通しのスキップを同じ語で束ねていた。

## 決定

猶予超過で確定した spawn 失敗を `TickIssue::SpawnNotObserved` として分ける。`TickIssue::SpawnFailed` は同期エラー専用にする。`.adr/2-confirmed-running-field-and-recorded-failures-in-errors.md` は記録した失敗を `errors` に載せることを決めただけで、1つの変種に畳むことまでは根拠づけていない。

表示では `errors` を「タスクファイルに何を残したか」で3つの見出しに分ける。

| 見出し | 分類 | 状態 |
|---|---|---|
| 失敗を記録 | `WorktreeCreateFailed` / `CommandExpansionFailed` / `SpawnNotObserved` | 失敗を記録しカウンタを消費した |
| 起動の結果が未確定 | `PrepareAttemptFailed` / `SpawnFailed` | launching の記録を保存した後の報告で、状態は `Launching` のまま次の tick が猶予経路で分類する |
| スキップ | 残り | タスクファイルへの書き込みが無く、次の tick がそのまま再試行する |

判別は `cli::render` の網羅 `match` に置き、分類が増えたときに振り分け先を決めないと通らないようにする。

「起動の結果が未確定」を独立させるのは、`PrepareAttemptFailed` の後も spawn を続けるため、同一タスクが `launched` と `errors` の両方に載りうるからである。この2つを「スキップ」に束ねると、書き込んで attempt 番号も消費した tick が「何も起きなかった」と読める。`.adr/2-empty-summary-means-nothing-to-process.md` が「書き込んだ tick が処理対象なしと表示される」ことを構成として潰したのと同じ理由で、見出しの語義も書き込みの有無と食い違わせない。

## 影響

- 分類だけで「カウンタを消費したか」が読め、同期エラー側で `record_spawn_failure` を呼ぶような取り違えをテストが検出する
- 唯一の出力窓で「記録された失敗」と「次 tick でそのまま再試行されるスキップ」が読み分けられる
- 同一タスクが「起動」と「起動の結果が未確定」に並んでも、運用者は「起動は試みたが結末は次の tick が決める」と読める。「起動」と「スキップ」の同時掲示のような矛盾した読み方が生まれない
- トレードオフ: spec の `errors` の分類が1つ増えるため、spec 追従が必要になる
- トレードオフ: 見出しが3つになる。どれも空なら見出しごと出さないので、通常運用の行数は変わらない
