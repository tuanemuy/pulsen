# PR Review #003 — [skeleton] 基盤・グローバル設定・ワークフロー定義とタスク登録(add)

**PR:** #8
**Date:** 2026-08-12
**Round:** 3回目

## Summary

- Blockers: 0
- Warnings: 5
- Verdict: **APPROVED**（Blocker ゼロ。完了判定は台帳の fix ゼロなので、この5件を修正して4周目へ）

## レイヤー別ファイル

- Domain / Use Case / CLI: review-003-domain-usecase-cli.md（B: 0 / W: 0 — 問題点ゼロ）
- Adapter / Infrastructure / Test: review-003-adapter-test.md（B: 0 / W: 2）
- Architecture / Spec-conformance: review-003-arch-spec.md（B: 0 / W: 3）

## カバレッジ

- 確認申告ゼロのファイル: なし（arch-spec が 135件を確認、残りは他レイヤーが確認）

## 指摘一覧

- [R3-W-001] conformance_worktree.rs:108 — TMPDIR がリポジトリ配下のとき TC-003 が失敗になる（CLI 側 TC-036 と扱いが割れる）（Adapter/Test）
- [R3-W-002] conformance/task_repository.rs + util/atomic.rs — 対象が `Err` を返すと TC-042/044 が失敗ではなくハングする（Adapter/Test）
- [R3-W-003] HOOKS.md:22,192 — 区分 C の説明が12行中8行にしか当てはまらず、worktree-manager-009 のフック列挙が不足（Arch）
- [R3-W-004] .adr/ — 連番が 055 → 060 と欠番で、理由がどこにもない（Arch）
- [R3-W-005] definition/workflow.rs:60 + cli/render.rs:195,214 — `WorkflowStructureError::describe()` と `WorkflowParseError` の文言が2層で重複（Arch）
