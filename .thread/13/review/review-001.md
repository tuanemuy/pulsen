# PR Review #001 — [test] ロック保持フィクスチャの合図タイムアウトをスキップ、実行ファイル不在を失敗として区別する

**PR:** #14
**Date:** 2026-08-13
**Round:** 1回目

## Summary

- Blockers: 2
- Warnings: 26
- Verdict: **BLOCKED**

## レイヤー別ファイル

- Test: review-001-test.md（B: 0 / W: 7）
- 型設計・アーキテクチャ整合: review-001-type-design.md（B: 0 / W: 8）
- 並行性・プロセス管理・堅牢性: review-001-concurrency.md（B: 0 / W: 5）
- ドキュメント整合・契約遵守: review-001-docs.md（B: 2 / W: 6）

## カバレッジ

- 確認申告ゼロのファイル: なし（9ファイルすべて1体以上が確認）

## 指摘一覧

- [B-001] lock.rs:21 / 参照の正確性 — `.adr/073` 未起票で、正本が指す `.adr/068:43` は逆のことを述べている（docs）
- [B-002] adr.md:9 / 契約遵守 — 全7エントリの Status が `Proposed` のまま（AC-12）（docs）
- [W-001] lock.rs:112,147 / 診断性 — 合図の読み取りエラーを `.ok()` で捨て、原因を名指しできない（test / type-design W-002）
- [W-002] lock.rs:81-82 / 未検証の主張 — 「最初のケースで失敗として現れる」をコードが保証しない（test）
- [W-003] HOOKS.md:45 / 典拠の誤り — 「起動できない場合も失敗」を `.adr/068` の帰結として書いている（test）
- [W-004] HOOKS.md:43 / 未検証の主張 — 「（初回起動のスキャン・高負荷）」は測っていない原因の推定（test）
- [W-005] lock.rs:69-90 / probe 設計 — probe が1回きりで偽陽性に弱い（test）
- [W-006] steps.md:334 / 検証の省略 — `ProgramUnusable` は unix の `chmod 000` で確定的に踏める（test）
- [W-007] cli_add_error.rs:130 / 文言 — `SKIP` 行が「ハーネスが lock::hold を提供しない」のまま（test）
- [W-008] lock.rs:121 / match網羅 — `recv_timeout` の `Err(_)` が `Timeout` と `Disconnected` を潰し、潰した先が許容集合側（type-design / concurrency W-001）
- [W-009] lock.rs:29 / 公開範囲 — `Available(PathBuf)` がパスを公開し AC-2 を型で迂回できる（type-design）
- [W-010] lock.rs:35,88 / 早すぎるString化 — `ProgramUnusable(String)` が `ErrorKind` を失う（type-design）
- [W-011] conformance_lock.rs:106 / 方針の複製 — 「`SignalTimedOut` だけ許容」が2つの `match` に複製（type-design）
- [W-012] conformance_lock.rs:79 / ADR-005 との不一致 — `try_acquire_from_other_process` だけ期限の無い `release`（type-design）
- [W-013] lock.rs:140 vs :154 / 診断 — 2つのパニック文言が同一で読み分けられない（type-design）
- [W-014] lock.rs:40 / 型のdoc — `Signaled` の doc が `locked:false` の実態を述べていない（type-design）
- [W-015] lock.rs:79,110 / 堅牢性 — probe の `expect` パニックが `LazyLock` を毒す（concurrency）
- [W-016] lock.rs:98,167 / 診断可能性 — 子の stderr を捨てるため誤った原因を示唆する（concurrency）
- [W-017] lock.rs:174 / ハング経路 — 期限の無い `wait()` が `release()` に残る（concurrency）
- [W-018] adr.md:37 / 前提の誤り — ADR-001 の「probe は無負荷・単発で測る」が実態と違う（concurrency）
- [W-019] HOOKS.md:45, lock.rs:131 / 理由の不整合 — 失敗側に倒す理由が ADR-002 が否定した説明になっている（docs）
- [W-020] lock.rs:14-15 / 文言の不整合 — `SIGNAL_DEADLINE` の doc が `SignalTimedOut` の doc と食い違う（docs）
- [W-021] HOOKS.md:56 / 未検証の主張 — example を含まない実測に新しい解釈を書き足している（docs）
- [W-022] PR本文 / 記録の欠落 — AC-10 の突き合わせ結果が未記録（docs）
- [W-023] testing.md:64,69 / 再現性 — 確認項目1 手順3 の grep の期待結果が不足（docs）
- [W-024] testing.md:291-295 / 再現性 — 確認項目14 が `.adr/073` 不在で実行不能（docs）
