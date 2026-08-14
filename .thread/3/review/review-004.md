# PR Review #004 — [tick] 観測・判定・ステータス遷移(リトライ・凍結・通知)

**PR:** #16
**Date:** 2026-08-14
**Round:** 4回目

## Summary

- Blockers: 0
- Warnings: 6
- Verdict: **BLOCKED**（Blocker はゼロだが、fix と仕分ける指摘が残るため次ラウンドへ）

## レイヤー別ファイル

- Domain: review-004-domain.md（B: 0 / W: 0）
- Adapter / Ports: review-004-adapter.md（B: 0 / W: 1）
- Use Case / CLI: review-004-usecase.md（B: 0 / W: 2）
- General（計画ドキュメント）: review-004-general.md（B: 0 / W: 3）

## カバレッジ

- 各観点の申告はいずれも変更ファイル一覧と1対1で対応
- 確認申告ゼロのファイル: `.thread/3/review/` 配下の中間成果物のみ（意図的。Phase 8 で削除する）

## ラウンド推移

| 観点 | R1 | R2 | R3 | R4 |
|---|---|---|---|---|
| Domain | B1 / W7 | B0 / W2 | B0 / W2 | **B0 / W0** |
| Adapter / Ports | B1 / W7 | B1 / W6 | B0 / W2 | **B0 / W1** |
| Use Case / CLI | B0 / W8 | B0 / W4 | B0 / W2 | **B0 / W2** |
| General（計画doc） | B5 / W6 | B3 / W6 | B1 / W8 | **B0 / W3** |
| 合計 | B7 / W25 | B4 / W18 | B1 / W14 | **B0 / W6** |

**全観点で Blocker ゼロ。** 各観点の収束の所見も「収束済み」「修正すれば収束」で一致している。

## 指摘一覧

### Blockers

なし。

### Warnings

- [W-001] `cli/render.rs:177-180` / 報告文 — `TickIssue::MissingCurrentAttempt` の表示文言が launching 限定のまま。本 PR が `observe.rs:33-38` で同変種を手続きD（Running）からも積むようにしたのに、表示は「起動記録済みですが現在 attempt がありません」で帳簿の `"state": "running"` と食い違う。隣の `MissingProcessIdent` は正しく「起動確認済み」と書いており同一手続き内で呼び方が割れている。**ラウンド3 W-001（`TransitionError` 側、fix 済み）の兄弟変種の見落とし**（UseCase W-001）
- [W-002] `tests/cli_tick.rs:933-970` / テストの主張 — 受け入れテスト `判定と遷移と凍結と通知はサマリーに現れる` が「凍結」を主張していない。`patch_task` で直に stopped を置くため `Branch::Notify` を通り `summary.frozen` は空のままで、`frozen` だけが実バイナリのサマリー表示まで通っていない（UseCase W-002）
- [W-003] `crates/pulsen-conformance/HOOKS.md:59` / 正本性 — 本 PR が書き換えた「フィクスチャの実行ファイルが無い場合」の括弧内列挙から、同じ PR が `:50` に足した「テスト用コマンド（`examples/judge_probe`）」だけが落ちている。実装と `:50` の行は正しく、正本の記述だけが実際より狭い（Adapter W-001）
- [W-004] `.thread/3/steps.md:156`（+ `:38,41,42`）— ステップ5 の `RecordSeq` の適用範囲が `save` / `run` の2メソッドのままで、同 PR の ADR-014（4メソッド）と実装に追いついていない（General W-001）
- [W-005] `.thread/3/steps.md:164` — 「HOOKS.md の表に足すのは 011/012/013/015 と 014/016 だけ」が実際の3行（実行ファイル不在行を含む）と食い違い、同文書ステップ7 の数え方とも割れている（General W-002）
- [W-006] `.thread/3/testing.md:901` — 「フィクスチャB は `$HOME/pulsen-manual-test` に閉じている」が、実際に作って `rm -rf` する `$HOME/pulsen-test-repo` / `$HOME/pulsen-manual-work` を落としている（General W-003）

## 所見

残る6件はいずれも「前ラウンドの修正が別ファイル・兄弟変種へ波及しなかった取りこぼし」で、状態遷移・永続化・at-least-once の正しさには影響しない。W-006 は手動確認の実行者が読む行なので Phase 4 の前に直す価値がある。
