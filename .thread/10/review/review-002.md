# PR Review #002 — [ci] MSRV とクロスプラットフォーム(Linux/macOS/Windows)を検証する CI を用意する

**PR:** #12
**Date:** 2026-08-13
**Round:** 2回目

## Summary

- Blockers: 1
- Warnings: 10
- Verdict: **BLOCKED**

## レイヤー別ファイル

- CI・ビルド基盤: review-002-ci.md（B: 1 / W: 2）
- 共通ユーティリティ・並行性・OS 抽象: review-002-util.md（B: 0 / W: 4）
- テスト・ドキュメント整合: review-002-test-docs.md（B: 0 / W: 4）

## カバレッジ

- 確認申告ゼロのファイル: なし（ci 観点と test-docs 観点が変更ファイル全量を確認。util 観点はソース3件と主要ドキュメントを確認）

## 指摘一覧

- [B-001] ci.yml:SKIP抽出/行頭一致がlibtestの進捗行と混線（ci）
- [W-001] progress.md:実測記録/CI未実行の記述が陳腐化（ci、test-docs W-003 と同一 Key）
- [W-002] HOOKS.md:実測/lib件数がHEADと不一致（ci）
- [W-001] util/atomic.rs:doc/511msが合成経路と粒度で過小（util）
- [W-002] util/atomic.rs:テスト/本番の配線を通らない（util、test-docs W-001 と同一 Key）
- [W-003] util/atomic.rs:MAX_ATTEMPTS/whyコメントが実態と不一致（util）
- [W-004] util/atomic.rs:並行テスト/読み手の無制限スピン（util）
- [W-004] Issue#10コメント:測定コミットの誤り（test-docs）
