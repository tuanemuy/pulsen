# PR Review #002 — [test] ロック保持フィクスチャの合図タイムアウトをスキップ、実行ファイル不在を失敗として区別する

**PR:** #14
**Date:** 2026-08-13
**Round:** 2回目

## Summary

- Blockers: 0
- Warnings: 20
- Verdict: **APPROVED**（Blocker ゼロ。完了判定は台帳の fix ゼロ）

## レイヤー別ファイル

- Test: review-002-test.md（B: 0 / W: 6）
- 型設計・アーキテクチャ整合: review-002-type-design.md（B: 0 / W: 6）
- 並行性・プロセス管理・堅牢性: review-002-concurrency.md（B: 0 / W: 3）
- ドキュメント整合・契約遵守: review-002-docs.md（B: 0 / W: 5）

1周目の Blocker 2件（`.adr/073` 未起票 / adr.md の Status）は解消を実物で確認済み。AC-1〜AC-13 は全件充足（docs 観点の判定）。

## カバレッジ

- 確認申告ゼロのファイル: なし（`.thread/13/review/` 配下のレビュー成果物はスキップ扱い）

## 指摘一覧

- [W-025] lock.rs:94-100,141-145 / probe の判定基準 — `Disconnected` を失敗側へ倒したのに probe では `Available` に合流する（type-design W-001 / concurrency W-003 / docs W-003）
- [W-026] steps.md / testing.md / PR本文 / AC-10 の出典 — 突き合わせの出典が修正前コミットの run のまま（test W-005 / docs W-004）
- [W-027] testing.md:276,280,363 / steps.md:302,308,110 — 1周目の HOOKS.md 修正に手順書の期待値が追随していない（test W-003 / docs W-002）
- [W-028] .thread/13/adr.md:211,225 / 作業ログの古さ — `SignalUnreadable(Child)` の綴りと「実地検証を持てない」が現状と食い違う（test W-004 / type-design W-005）
- [W-029] .adr/073 / 成立条件の欠落 — 「単一テストターゲット指定は5件の失敗」を無条件に書いている（docs W-001）
- [W-030] .adr/073:41 vs :70 / 理由の不整合 — probe 成立後のタイムアウトを失敗に倒す理由が同 ADR の別記述と噛み合わない（test W-002）
- [W-031] .adr/073:24, lock.rs:30 / スキップ判定の残余 — 合図を返さなくなったフィクスチャの退行が「環境の能力」として緑になる（test W-001）
- [W-032] HOOKS.md:47 / 読み替え先 — 「判定列の括弧で読む」の括弧を持つ行が1行だけで、`tc_task_register_task_017` の意味を引けない（test W-006 / docs W-005）
- [W-033] lock.rs:102,119-122 / 合成 io::Error — `stdout` 取得失敗の `ErrorKind::Other` が「起動できない」と名乗る（type-design W-002）
- [W-034] common/mod.rs:37-56 / 導線 — `allowed_skips()` に振り分けの理由も `.adr/073` への導線も無い（type-design W-003）
- [W-035] conformance_lock.rs:66-71 / kill_holder — `kill`/`wait` の失敗を `None` に畳んでスキップ経路へ流す（type-design W-004）
- [W-036] conformance_lock.rs:9,63,108 / 体裁 — 同じモジュールの項目を `use` とフルパスで混在（type-design W-006）
- [W-037] lock.rs:205-208 / kill_and_wait の期限 — `kill` が効かない子で `OnceLock` 待ちの全スレッドが停止しうる（concurrency W-001）
- [W-038] lock.rs:124 / thread::spawn のパニック — `Child` が kill も wait もされず落ちる（concurrency W-002）
