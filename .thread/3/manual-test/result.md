# 動作確認の結果 — Issue #3

**実行日:** 2026-08-14
**対象コミット:** `cfff109`（Phase 3 の APPROVED 時点）
**環境:** macOS（Darwin 25.4.0）/ 非 root / 実運用ホーム（`$HOME/.pulsen`）は未作成のまま非汚染

## ブラウザ検証: スキップ

`../../../.claude/skills/issue-implement/SKILL.md` Phase 4 のスキップ条件を実際に確認した:

- **Web UI なし** — `package.json` が存在せず（dev / start 系スクリプトが無い）、UI ファイル（`.html` / `.tsx` / `.jsx` / `.vue`）もリポジトリに1件も無い
- **`testing.md` に画面操作項目が1件もない** — 確認項目17件はすべてターミナル上の CLI 操作
- `agent-browser --version` は 0.33.2 で動作するが、上記2条件により対象が存在しない

代わりに `testing.md` の CLI 実機検証を実行した。

## サマリー

| フィクスチャ | 由来 | PASS | FAIL | 実行不可 |
|---|---|---|---|---|
| A | `spec/manual-tests/task-execution.md` | 11 | 0 | 0 |
| B | `spec/manual-tests/setup.md` | 7 | 0 | 1 |
| C | `spec/manual-tests/intervention.md` | 3 | 0 | 0 |
| **合計** | | **21** | **0** | **1** |

**変更起因の FAIL はゼロ。**

実行不可の1件（setup TC-35）は `testing.md` の割り当ての重複によるもので、実装の問題ではない。TC-35 が主張する筋（notify_cmd 未定義での凍結 → 後から定義して catch-up）は**フィクスチャC の確認項目9 で PASS 済み**なので、検証としての欠落は無い。

## カバレッジ

`testing.md` の確認項目17件（正常系11 + エッジケース・異常系6）は、すべていずれかのフィクスチャで消化した。

| 項目 | 消化したフィクスチャ |
|---|---|
| 確認項目1（exit 0 → completed → next） | A |
| 確認項目2（judge 定義ありの exit 0） | B |
| 確認項目3（一過性失敗の自動リトライ） | A（手順1〜6）/ B（手順7〜9） |
| 確認項目4（skipped 周回） | A（手順1〜11）/ B（手順12） |
| 確認項目5（デフォルト判定の2値） | A |
| 確認項目6（シグナル死の EXIT_CODE） | B |
| 確認項目7（リトライ上限の等号と超過） | A（手順1〜8）/ C（手順9〜11） |
| 確認項目8（判定失敗の上限超過） | A（手順1〜7）/ B（手順8） |
| 確認項目9（通知の失敗と再通知 / catch-up） | C |
| 確認項目10（timeout kill と冪等性） | A |
| 確認項目11（exit 記録なしの死亡検出） | A |
| エッジケース1（縮退 stopped への再通知） | C |
| エッジケース2（判定コマンドの実体が無い） | B |
| エッジケース3（判定 timeout と tick のブロック） | B |
| エッジケース4（不変条件の破れ） | A |
| エッジケース5（run ファイル破損での滞留） | A |
| エッジケース6（1タスクの失敗が他を止めない） | A |

## 受け入れ基準との対応

| 基準 | 実機で確認したこと | 判定 |
|---|---|---|
| AC-2 | exit 0 の観測でその tick は `complete_run` までで止まり、次の tick の `advance` で `next` へ進む。カウンタがリセットされる | PASS（確認項目1・2） |
| AC-3 | 非0 exit / judge exit 10 が `fail_run` で failed になり `attempt_count` を消費、次 tick が新 attempt で再起動、成功でカウンタが 0 に戻る | PASS（確認項目3・5・6） |
| AC-4 | 上限超過が `Stopped` を保存し、同一 tick 内で notify_cmd が起動、`Exited(0)` のときだけ `mark_notified`。失敗すると `notified_at` が残らず次 tick が再通知。未定義なら通知も記録もせず、後から定義すると catch-up | PASS（確認項目7・8・9、エッジケース1） |
| AC-5 | judge の exit 20 が `skip_run` になりタスクステータス不変で pending へ戻る。judge 未定義での exit 20 は failed | PASS（確認項目4・5） |
| AC-6 | timeout 超過で kill してから failed。exit 記録なしのプロセス死亡も検出 | PASS（確認項目10・11） |

## 実機で確認した主な事実

- **1タスク1tick1ステップ** — 判定確定の tick では `completed` 止まりで、遷移は次の tick
- **上限の等号では凍結しない** — `attempt_count` が `retry_limit` と等しい間は failed で、超過して初めて `stopped`
- **凍結と通知が同一 tick** — サマリーの「凍結」と「通知」に同じ ID が並ぶ
- **二重通知しない** — 通知済みの stopped は次の tick で `notify.log` の md5・行数とも不変
- **報告は通知に置き換わらない** — スナップショット破損の未通知 stopped で「通知」と「スキップ」が同一 tick に両方出る（ADR-012）
- **判定失敗はエージェントを再実行しない** — `judge_attempt_count` だけが増え、run ディレクトリは `attempt-1` のみ、`attempt_count` は 0 のまま
- **判定コマンドは直接の子だけを終了させる** — `judge_timeout` 超過後に `sleep 180` が残っていない
- **既定のリトライ上限は 2**、凍結までに要した tick は 9回（起動 → 起動確認 → 判定の3刻み × 3 attempt）

## 検証が見つけた手順書の記述の誤り

実機検証は `testing.md` 側の記述の誤り・不足を9件検出した（実装の問題ではない）。いずれも `testing.md` に反映済み。詳細は各フィクスチャの結果ファイルを参照。

とくに **`last_failure` の記述**は、フィクスチャA と B が独立に「エージェント実行の失敗（`fail_run`）では `last_failure` は書かれない」ことを実測し、`spec/domains/task.md` の `FailureNote` 定義（「ツール操作の失敗および判定失敗の記録」）と照合して**実装が spec どおりで手順書が誤り**と結論づけた。

## 起票した Issue

なし（変更と無関係の FAIL はゼロ）。

## 詳細

- `result-fixture-a.md` — task-execution 由来の11件
- `result-fixture-b.md` — setup 由来の8件
- `result-fixture-c.md` — intervention 由来の3件
