# ADR: 残存の後始末は表示の第4の見出しにし、見出しの軸を「運用者が次に取る行動」へ広げる

## ステータス

承認済み

## コンテキスト

`.adr/2-spawn-not-observed-classification-and-error-headings.md` は `errors` を「タスクファイルに何を残したか」で3つの見出し(失敗を記録 / 起動の結果が未確定 / スキップ)に分けた。

`.adr/3-run-failure-cause-and-remnants-as-classifications.md` で残存の報告を保存の成否と独立に積むようにした結果、`RemnantsUnhandled` がこの3分類のどれにも収まらなくなった。実行の失敗を保存できなかった tick でも残存の報告は積まれるので、`attempt_count` が動いていないのに「失敗を記録」に現れる。「スキップ」(次の tick がそのまま再試行する)も当てはまらない — 残存終了は実行の失敗を確定させる tick でだけ試み、tick はこれを再試行しない。

## 前提

- cron 運用では tick のサマリーが唯一の窓であり(`.adr/2-empty-summary-means-nothing-to-process.md`)、見出しの語義と帳簿の状態が食い違うと運用者の読みが外れる
- 後始末の主体は tick ではなく人間で、tick は残存終了を再試行しない

## 決定

**`IssueOutcome` に第4分類 `CleanupLeft`(見出し「後始末が残っている」)を足し、`RemnantsUnhandled` だけをそこへ振る。** 見出しの語義は「タスクファイルに何を残したか」から「**報告が何を残したか(運用者が次に取る行動)**」へ広げ、OS 側に残ったものもこの軸で読めるようにする。振り分けは引き続き `cli::render` の網羅 `match` に置く。

変種名を `RemnantsLeft` にしないのは、`cli::render` が `application::tick::RemnantsLeft` を型として使っており、同一ファイル内で同名衝突するためである。

本 ADR は `.adr/2-spawn-not-observed-classification-and-error-headings.md` の見出しの数(3分類 → 4分類)と語義(帳簿に何を残したか → 報告が何を残したか)を置き換える。`.adr/2-spawn-not-observed-classification-and-error-headings.md` の本文は判断が下された時点の記録として書き換えない。

## 検討した代替案

- **`RemnantsUnhandled` を「失敗を記録」に入れる** — カウンタを消費していない tick が「失敗を記録」に現れ、見出しと帳簿の状態が食い違う
- **「スキップ」に入れる** — スキップは「次の tick がそのまま再試行する」を意味するが、tick は残存終了を再試行しない
- **残存の報告を保存に成功した tick だけに積み、3分類を維持する** — プロセスの残存という事実が保存の失敗で消える

## 影響

- カウンタを消費していない tick が「失敗を記録」に現れなくなり、見出しと帳簿の状態が食い違わない
- 後始末の主体が tick ではなく人間であることが見出しから読める
- 見出しの軸が一般化されたことで、今後 OS 側・外部側に残る報告も同じ軸で振り分けられる
- トレードオフ: 見出しが4つになる。どれも空なら見出しごと出さないので、通常運用の行数は変わらない(`.adr/2-spawn-not-observed-classification-and-error-headings.md` と同じ)
