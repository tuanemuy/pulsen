# PR Review #003 — [test] ロック保持フィクスチャの合図タイムアウトをスキップ、実行ファイル不在を失敗として区別する

**PR:** #14
**Date:** 2026-08-13
**Round:** 3回目

## Summary

- Blockers: 0
- Warnings: 13
- Verdict: **APPROVED**（Blocker ゼロ。完了判定は台帳の fix ゼロ）

## レイヤー別ファイル

- Test: review-003-test.md（B: 0 / W: 3）
- 型設計・アーキテクチャ整合: review-003-type-design.md（B: 0 / W: 4）
- ドキュメント整合・契約遵守: review-003-docs.md（B: 0 / W: 6）

並行性の観点は、2周目で「実装（型・分岐・後始末の構造）は変えない」方針に入り、以降の差分が doc とコメントに限られたため、この周では立てていない。3周目に挙がった指摘もすべて記述側で、実装の挙動には触れていない。

AC-1〜AC-13 は全件充足（docs 観点の判定）。実装の挙動に関わる指摘はゼロ。

## カバレッジ

- 確認申告ゼロのファイル: なし（`.thread/13/review/` 配下のレビュー成果物はスキップ扱い）

## 指摘一覧

- [W-039] lock.rs:28-29 / Available の doc — 「合図が期限内に返る」の断定が、`.adr/073` の「根拠は経路で2つに分かれる」と食い違う（type-design W-001 / docs W-001）
- [W-040] .adr/073:39 / 射程 — 「それ以外はすべて能力ありとして扱い」が同 ADR の4区分の振り分けと逆に読める（type-design W-003 / docs W-002）
- [W-041] testing.md:87,106,120 / steps.md:276 — 2周目の `use` 統一に手順書の期待値が追随していない（test W-001 / docs W-004 / docs W-006）
- [W-042] .thread/13/adr.md:150 / 作業ログの旧断定 — 「期限内に返ったことは確かなので `Available`」が `Disconnected` の扱いと食い違う（test W-002）
- [W-043] lock.rs:kill_and_wait の doc — 「待たない」が本文の無期限 `wait()` と逆に読める（test W-003）
- [W-044] lock.rs:14-17 / SIGNAL_DEADLINE の doc — 2周目に `.adr/073` から外した断定（probe 成立後は異常）が定数の doc に残る（type-design W-002）
- [W-045] testing.md 確認項目13 手順1 / 期待値 — 「ヒット0件」が実物と不一致（docs W-003）
- [W-046] steps.md:308 / testing.md:280 / 事実誤り — 「実測は `e524981` 時点で example を含まない」が成り立たない（type-design W-004）
- [W-047] AC-10 の出典 — 出典 run が親コミットのまま（test / docs W-005）。最終のコード変更コミットの run に寄せる
