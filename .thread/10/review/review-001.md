# PR Review #001 — [ci] MSRV とクロスプラットフォーム(Linux/macOS/Windows)を検証する CI を用意する

**PR:** #12
**Date:** 2026-08-13
**Round:** 1回目

## Summary

- Blockers: 4
- Warnings: 17
- Verdict: **BLOCKED**

## レイヤー別ファイル

- CI・ビルド基盤: review-001-ci.md（B: 0 / W: 5）
- 共通ユーティリティ・並行性・OS 抽象: review-001-util.md（B: 0 / W: 6）
- テスト・ドキュメント整合: review-001-test-docs.md（B: 4 / W: 6）

## カバレッジ

- 確認申告ゼロのファイル: なし（9ファイルすべてに1体以上の確認申告あり。ci 観点が9件全確認、test-docs 観点が9件全確認、util 観点が7件確認 + steps.md / testing.md をスキップ）

## 指摘一覧

- [B-001] HOOKS.md:実測/テストバイナリ数の誤り（test-docs）
- [B-002] progress.md:Issue #10 コメント/未実施を実施済みと記載（test-docs）
- [B-003] progress.md:PR #11 引き継ぎ/未実施を完了と記載（test-docs）
- [B-004] PR#12:本文/CI 実行前のまま（test-docs）
- [W-001] ci.yml:cargo test/--no-fail-fast 欠落（ci）
- [W-002] ci.yml:非rootアサート/fail-open（ci）
- [W-003] ci.yml:grep || true/コメントが不正確（ci）
- [W-004] ci.yml:msrv版数/run への直接展開（ci）
- [W-005] ci.yml:SKIPサマリー/除外の不可視（ci）
- [W-001] task_repository.rs:lookup/list/読み手側の共有違反が未処理（util）
- [W-002] util/atomic.rs:テスト/上限の未検証（util）
- [W-003] util/atomic.rs:is_transient/最終試行で呼ばれない（util）
- [W-004] util/atomic.rs:retry_while_transient/未使用の一般化（util）
- [W-005] util/atomic.rs:MAX_ATTEMPTS-1/減算オーバーフロー（util）
- [W-006] util/atomic.rs:doc/遅延特性の未記載（util）
- [W-001] util/atomic.rs:テスト/上限の未検証（test-docs、util W-002 と同一 Key）
- [W-002] HOOKS.md:実測/測定コミットの誤り（test-docs）
- [W-003] HOOKS.md:実測/OS差なしの過剰主張（test-docs）
- [W-004] HOOKS.md:実測3列/更新運用と噛み合わない（test-docs）
- [W-005] .thread/10:ADR参照/裸の連番が .adr と衝突（test-docs）
- [W-006] adr.md:ADR-012/存在しない定数を参照（test-docs）
