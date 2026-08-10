# 004: abort は全域・retry は stopped 専用とする

## ステータス

承認済み

## コンテキスト

requirements の初版では abort は launching / running(ADR-002 で pending を追加)を対象とし、failed(リトライ待ち)や completed(遷移待ち)のタスクを止められるかが未定義だった。retry も適用対象の実行状態が明示されていなかった。failed のタスクは次のtickで自動リトライされるため、「これ以上リトライさせたくない」を表現する手段が要る。

## 前提

- abort は「タスクの停止(実行中なら kill を伴う)」に一般化済み(ADR-002)
- failed は次のtickで自動的に再実行される
- リトライ上限(attempt_count)は無人運用の安全弁である

## 決定

- **abort**: stopped・アーカイブ済み以外のすべての実行状態(pending / launching / running / failed / completed)に許可する。プロセスが存在すれば照合付き kill、存在しなければ stopped の記録のみ。実行状態ごとの例外を設けず「kill 対象の有無だけが分岐」という総則にする
- **retry**: stopped のタスクに対してのみ受理する。それ以外は拒否し、failed は「放置すれば自動リトライされる」、pending は「既に実行待ち」と案内する

## 検討した代替案

- completed への abort を拒否する(成功済みなので次ステータスで判断させる) — 実行状態ごとの例外が増え、仕様も実装も複雑になる割に守るものがないため不採用
- stopped 以外への retry を受理してカウンタのみリセットする — 生きているタスクのリトライ上限を人手で骨抜きにでき、無人運用の安全弁を損なうため不採用

## 影響

- 「リトライを止めて凍結する」は failed への abort で表現できる
- 全ケースが abort → set-status → retry の組み合わせで表現できるという既存原則が、実行状態の全域で成立する
- stopped 経路は引き続き「各種上限超過」「人間による abort」の閉集合のまま
