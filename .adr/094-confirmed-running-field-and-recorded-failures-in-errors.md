# 094: 起動確認だけをサマリーの新しいフィールドにし、記録した失敗は `errors` に載せる

## ステータス

承認済み

## コンテキスト

spec の tick の出力 DTO は `launched` / `transitioned` / `skipped_back` / `frozen` / `notified` / `archived` / `errors` / `gc_deleted` / `gc_errors` の9フィールドである。ところが、タスクファイルを書き換える次の4経路はどのフィールドにも集計されない。

| 経路 | 書き込む内容 |
|---|---|
| launching → running の取込 | 実行状態と `current_attempt.process` |
| spawn 失敗による起動待ち復帰 | 実行状態・`spawn_fail_count`・失敗要因 |
| worktree 作成失敗の記録 | 実行状態・`attempt_count`・失敗要因 |
| テンプレート展開失敗の記録 | `spawn_fail_count`・失敗要因 |

集計されないまま tick を終えるとサマリーが空になり、「処理対象のタスクはありませんでした。」と表示される（ADR-092 の判定）。主経路である起動確認が毎回この表示になるため、放置できない。

## 決定

失敗の記録3つは `errors` に載せる。spec の `errors` は「スキップ・破損・観測失敗・kill 失敗等の報告」であり、**記録した失敗の報告**はこの定義に収まる。報告用の分類として `TickIssue::WorktreeCreateFailed` / `CommandExpansionFailed` を持つ。報告は**保存できたときにだけ**積む — 保存できなかった tick は状態を変えておらず、そのときの報告は `SaveFailed` 1件が正しい。

launching → running の取込にだけ、フィールド `confirmed_running: Vec<TaskId>` を1つ足す。`transitioned` はタスクステータスの遷移、`skipped_back` は skipped 判定による起動待ち復帰で語義が確定しており、実行状態の取込をどちらかに混ぜると後続スライスが同じフィールドを別の意味で埋めることになる。

`errors` の形も spec の `{ task_id, path, message }` ではなく分類の列挙として持つ（ADR-081）。文言の組み立ては `cli::render` に置く。

## 検討した代替案

- **`skipped_back` に相乗りさせる** — spec が「skipped で pending 復帰」と明記しており、spawn 失敗による復帰と判定による復帰が同じ行に混ざる
- **フィールドを足さず `errors` だけで起動確認を報告する** — 起動確認は失敗でもスキップでもない正常な前進で、`errors` に出すと運用者が異常として読む

## 影響

- 書き込みを行った tick が必ずサマリーに現れ、ADR-092 の不変が構成として成立する
- 後続スライスは `transitioned` / `skipped_back` を spec の語義のまま使える
- トレードオフ: 出力 DTO が spec から1フィールド分ずれるため、spec 追従が必要になる
- トレードオフ: 1つのタスクが1回の tick で `errors` と `frozen` の両方に現れうる（上限超過で凍結した失敗）。失敗の原因と結末は別の情報なので、どちらか一方に畳まない
