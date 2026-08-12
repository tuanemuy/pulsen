# PR Review #001 — [tick] エージェント実行の起動(worktree確保・デタッチ起動・spawn確認)

**PR:** #11
**Date:** 2026-08-13
**Round:** 1回目

## Summary

- Blockers: 4
- Warnings: 23
- Verdict: **BLOCKED**

## レイヤー別ファイル

- Domain: review-001-domain.md（B: 0 / W: 5）
- Use Case: review-001-usecase.md（B: 0 / W: 4）
- Adapter / Infrastructure: review-001-adapter.md（B: 0 / W: 4）
- Test: review-001-test.md（B: 2 / W: 5）
- Architecture / CLI: review-001-architecture.md（B: 2 / W: 5）

## カバレッジ

- 確認申告ゼロのファイル: なし（変更62ファイルすべてに1体以上の確認申告あり）

## 指摘一覧

- [B-001] `cli/render.rs:tick_summary` — 状態を書き換えた tick が「処理対象のタスクはありませんでした。」と表示する（Architecture）
- [B-002] `crates/**:ADR参照` — 実装コードの ADR-NNN が `.adr/` の正本と衝突し、同じ番号が2つの別文書を指す（Architecture）
- [B-003] `tests/cli_tick.rs/カバレッジ` — TC-exec-tick-017（config 不在・破損で tick が非0・状態不変）の検証が無い（Test）
- [B-004] `task/task.rs:confirm_workspace/カバレッジ` — TC-exec-tick-035（worktree 作成成功でカウンタがリセットされない）がどの層でも主張されていない（Test）
- [W-001] `execution/port.rs:RunStore/write系のディレクトリ作成` — 契約として宣言されているのに適合ケースが無い（Domain）
- [W-002] `tick/launch.rs:attempt番号の導出源` — 遷移関数の外で再導出され整合の保証が呼び出し側に分岐（Domain / Use Case）
- [W-003] `task/task.rs:record_launching/初回採番` — `current_attempt=None` からの採番を主張するユニットテストが無い（Domain）
- [W-004] `task/path.rs:state_root/否定ケース` — ADR-015 が根拠に挙げる `attempt-+1` が否定ケースに無い（Domain）
- [W-005] `tick/mod.rs:is_stopped/execution_kind` — 同じ判別をユースケースが手書きで再実装（Domain）
- [W-006] `tick/confirm_spawn.rs:read_run_files/RunFileError` — `read_starttime` の失敗経路が一度も注入されていない（Use Case）
- [W-007] `cli/render.rs:tick_summary/表示契約` — タスク0件と「走査したが全員待ち」が区別できない（Use Case）
- [W-008] `cli/wire.rs:compose_wrapper/依存` — ラッパーが使わない `current_exe()` の解決を要求している（Use Case / Architecture）
- [W-009] `adapter/worktree.rs:create/達成済み判定` — git の `prunable` 注記だけに依存し実体の存在を確かめていない（Adapter）
- [W-010] `adapter/process.rs:identity(windows)::observe` — PowerShell の非終端エラーが「プロセス不在」に畳まれる（Adapter）
- [W-011] `conformance/process_controller.rs:spawn::tc_002/デタッチ性` — `detach()` を消してもテストが全緑のまま（Adapter）
- [W-012] `adapter/process.rs:run_agent/ログ順序` — stderr 側だけが開けない順序で `stdout.log` を作ってから 126 を返す（Adapter）
- [W-013] `tests/common/mod.rs:LOCK_HOLDER_CASES/スキップID` — スキップIDが `tc_exec_tick_016`（実体は TC-015）（Test）
- [W-014] `conformance_worktree.rs:worktree_present/アサーション` — TC-010 が `is_dir()` 止まり（Test）
- [W-015] `conformance/process_controller.rs:spawn/フック契約` — 常に `Some(true)` を返すハーネスでも3件とも通る（Test）
- [W-016] `tests/tick_confirm_spawn.rs/TC-084` — 「繰り返しても滞留し凍結しない」が tick 1回でしか確かめられていない（Test）
- [W-017] `tests/tick_confirm_spawn.rs/TC-085` — 固有ケースが無く他ケースの観測に相乗り（Test）
- [W-018] `execution/launching.rs:InconsistentRunFiles/文言` — ドメインが完成文言を持ち CLI がそのまま出す（Architecture）
- [W-019] `task/task.rs:execution_kind,is_wait,is_cleanup/未使用pub` — 呼び出しの無い `pub` に why が無い（Architecture）
- [W-020] `cli/render.rs:push_attempts/到達不能` — 到達不能なサマリー表示経路がテストでも実行されない（Architecture）
- [W-021] `execution/port.rs:read_exit/未使用` — 本番の呼び出し元が無く、モジュール doc の宣言規則と食い違う（Architecture）
