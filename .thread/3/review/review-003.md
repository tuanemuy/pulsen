# PR Review #003 — [tick] 観測・判定・ステータス遷移(リトライ・凍結・通知)

**PR:** #16
**Date:** 2026-08-14
**Round:** 3回目

## Summary

- Blockers: 1（計画ドキュメントのみ。コード3観点はゼロ）
- Warnings: 14
- Verdict: **BLOCKED**

## レイヤー別ファイル

- Domain: review-003-domain.md（B: 0 / W: 2）
- Adapter / Ports: review-003-adapter.md（B: 0 / W: 2）
- Use Case / CLI: review-003-usecase.md（B: 0 / W: 2）
- General（計画ドキュメント）: review-003-general.md（B: 1 / W: 8）

## カバレッジ

- 各観点の申告: Domain 確認29 / スキップ28、Adapter 確認30 / スキップ27、UseCase 確認 / スキップとも一覧と1対1、General 確認4 / スキップ53（いずれも一覧57件と一致）
- 確認申告ゼロのファイル: `.thread/3/review/` 配下の中間成果物のみ（意図的。Phase 8 で削除する）

## ラウンド推移

| 観点 | R1 | R2 | R3 |
|---|---|---|---|
| Domain | B1 / W7 | B0 / W2 | B0 / W2 |
| Adapter / Ports | B1 / W7 | B1 / W6 | B0 / W2 |
| Use Case / CLI | B0 / W8 | B0 / W4 | B0 / W2 |
| General（計画doc） | B5 / W6 | B3 / W6 | B1 / W8 |
| 合計 | B7 / W25 | B4 / W18 | B1 / W14 |

コード側は3ラウンドで Blocker が解消。残る Blocker 1件と Warning の過半は計画ドキュメントの実装追随。

## 指摘一覧

### Blockers

- [B-001] `.thread/3/steps.md:180`（ステップ8）— 報告の見出しが3分類のままで、ADR-017 と `render.rs` の4分類（`RemnantsUnhandled` → 「後始末が残っている」）と食い違う（General B-001）

### Warnings — コード

- [W-001] `cli/render.rs:transition_error` + `domain/task/transition.rs` / 報告文 — `MissingCurrentAttempt` の意味を Running・Completed へ広げたのに文言が「起動記録済みなのに現在 attempt が無い」のままで、`Branch::Advance` 経由で実行状態と食い違う行が出る。この経路の報告を見るテストも無く、既存テストが古い文言をピン留めしている（Domain W-001）
- [W-002] `application/tick/mod.rs:518-529` / why コメント — `Freeze` のコメントが本 PR で入った catch-up 通知の経路を「#3 が入れる将来の経路」として参照しており、現在の呼び出し元（`notify.rs:57`）を検算できない（UseCase W-001）
- [W-003] `tests/tick_observe.rs:663-674` / テストの実効性 — 「残存終了は失敗の確定より先に試みる」に対し、アサーションは `ProcessController` 内2呼び出しの前後しか見ておらず、`try_kill_remnants` → `fail_run` → `save` のポートをまたぐ順序が `save` 先行の実装でも緑で通る（UseCase W-002）
- [W-004] `crates/pulsen-conformance/HOOKS.md:44,45` / 正本性 — フィクスチャ実行ファイル依存の行が本スライスの11行（TC-007 / 011〜016）を拾っていない。CommandRunner 側は漏れなく列挙しているのに ProcessController 側だけ未追随（Adapter W-001）
- [W-005] `adapter/process.rs:191,674` / Windows の終了操作 — Windows は `Graceful` / `Forced` が同一の `taskkill /T /F` なのに2段目を実際に起動し、無意味な再実行 + 最大4秒のロック保持 + pid 再利用による誤殺の窓が2回開く（Adapter W-002）

### Warnings — 計画ドキュメント

- [W-006] `.thread/3/plan.md:95-101` + `steps.md:259` / spec 差分の記帳 — `classify_alive -> AliveDecision` / `default_judgement -> DefaultJudgement`（spec は `RunningDecision` / `JudgeOutcome`）の乖離と、`DOM-execution-017` の1段目がユースケース側にある事実が、AC-8 が参照する差分一覧に載っていない（Domain W-002）
- [W-007] `steps.md:108,164` / `testing.md:692` — `kill` / `try_kill_remnants` の典拠が ADR-002 のみで、ADR-015 が置き換えた3点へ導かない（General W-001）
- [W-008] `.thread/3/testing.md:717` — 見出しの分類軸が「タスクファイルに何を残したか」のままで ADR-017 の一般化を反映せず、同じ文中で自己矛盾（General W-002）
- [W-009] `.thread/3/testing.md:891` — `grep -rn 'command_runner()'` の期待「1件だけ」は実測2件（General W-003）
- [W-010] `.thread/3/testing.md:52,55` — AC-7 の grep の cfg 件数 `4 / 12 / 1` が baseline（4/10/1）とも実装後（4/20/1）とも不一致。`-A 3` では pulsen の6依存を確認できない（General W-004）
- [W-011] `.thread/3/plan.md:138`（setup TC-38）— 指示された後始末（`judge-exit` を 0 に戻して completed で進める）が `judge-missing.yaml` では実行不能で、testing.md と割れている（General W-005）
- [W-012] `.thread/3/plan.md:135` / `testing.md:915`（setup TC-11）— 実行範囲が手順5 前半のぶんずれ、記帳も食い違う（General W-006）
- [W-013] `.thread/3/testing.md:73,369`（フィクスチャC）— `PMT` の読み替えが未説明で「パスは手順書どおり」宣言と矛盾。戻すとフィクスチャB のホームを `rm -rf` で壊す（General W-007）
- [W-014] `.thread/3/adr.md:424`（ADR-014 Consequences）— 採番アクセサ「2つに限り」が Decision・実装（3つ）と反する（General W-008）
