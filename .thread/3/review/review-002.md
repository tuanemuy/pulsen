# PR Review #002 — [tick] 観測・判定・ステータス遷移(リトライ・凍結・通知)

**PR:** #16
**Date:** 2026-08-14
**Round:** 2回目

## Summary

- Blockers: 4
- Warnings: 18（重複除去後）
- Verdict: **BLOCKED**

## レイヤー別ファイル

- Domain: review-002-domain.md（B: 0 / W: 2）
- Adapter / Ports: review-002-adapter.md（B: 1 / W: 6）
- Use Case / CLI: review-002-usecase.md（B: 0 / W: 4）
- General（計画ドキュメント）: review-002-general.md（B: 3 / W: 6）

## カバレッジ

- 各観点の申告: Domain 確認19 / スキップ32、Adapter 確認33 / スキップ18、UseCase 確認28 / スキップ23、General 確認4 / スキップ47（いずれも一覧51件と一致）
- 確認申告ゼロのファイル: `.thread/3/review/` 配下の7ファイルのみ。**意図的**（レビューの中間成果物で、Phase 8 で削除する。全観点にスキップ可と明示して起動した）
- コード・計画ドキュメントの44ファイルはいずれか1観点以上が確認済み

## ラウンド1 からの変化

- コード側の Blocker は解消（Domain 1 → 0、Adapter は別原因で新規1件、UseCase 0 → 0）
- 前ラウンドの32件（コード21 + ドキュメント11）はすべて解消済みであることを各観点が確認
- 新規 Blocker の内訳: コード1件（Adapter B-001）、計画ドキュメント3件（General。`plan.md` はラウンド1 の General の担当外だったため未追随のまま残っていた）

## 指摘一覧

重複する指摘は代表IDに束ねた（括弧内が同一指摘の別観点でのID）。

### Blockers

- [B-001] `adapter/process.rs:terminate` / 誤殺 — `KillIdent` をプラットフォーム形式として parse せず `/bin/kill` へ素通ししており、`-1` / `-0` / `0` でユーザーの全プロセス（または tick 自身と呼び出し元シェル）を巻き込む（Adapter B-001）
- [B-002] `.thread/3/plan.md:63-72` — 「実行環境が前提を作れないとスキップで終わる行」の表が probe 化後の許容集合と食い違う（`command-runner-005` / `process-controller-010` は許容集合に入らない、011〜016 の割り方も違う）（General B-001）
- [B-003] `.thread/3/plan.md:44,49` — 「既に実装済みで確認だけを行う行」の `DOM-task-053` 行が、本スライスで `AlreadyNotified` を足して6変種にした事実を落としている（General B-002）
- [B-004] `.thread/3/plan.md:96-97` — 「spec との差分として提起するもの」が実装の最終形と数字が合わない（tick 出力 DTO が「10フィールド」のまま。実装は11。`AlreadyNotified` の差分も欠落。`steps.md:259` と割れている）（General B-003 / UseCase W-004）

### Warnings

- [W-001] `adapter/process.rs:terminate::command` / 契約 — POSIX は SIGTERM のみで成功判定が外部コマンドの終了ステータス依存。SIGTERM を捕まえるエージェントが生存したまま `Ok` → `fail_run` → 同一 worktree 並走。Windows（`/F`）と保証が食い違う（Adapter W-001）
- [W-002] `adapter/process.rs:terminate` / 実体依存 — busybox（Alpine 実測）では `--` がオペランド扱いで、終了させているのに rc=1 → `KillError::Failed`。ubuntu（procps-ng）/ macOS では `--` 付きが正しい（Adapter W-002）
- [W-003] `adapter/process.rs:unit_is_live` / why コメント — `Err(Io)` が呼び出し側で `Ok(false)` と同じ `NotIdentifiable` に畳まれ、コメントが謳う「報告されない」の解消が実際には起きていない（Adapter W-003）
- [W-004] `doubles/task_repository.rs:saved_degraded` / テストの実効性 — `RecordSeq` が `saved` にしかなく、縮退タスク再通知の順序（通知 → `mark_notified` 保存）が主張できていない（Adapter W-004）
- [W-005] `HOOKS.md:49` / 正本性 — CommandRunner の「judge_probe 未ビルド」行が probe を要さない TC-003 / 004 を巻き込んでおり、ProcessController の同種の行と不整合（Adapter W-005）
- [W-006] `HOOKS.md:78-80` / 正本性 — OS 別ユニットテストの内訳（`ps` 系6件）が本 PR の追加（6→8）とずれる（Adapter W-006）
- [W-007] `domain/execution/judgement.rs:default_judgement` / 型設計 — 「2値しか返さない」規則が返り値型に無く、`observe.rs:269-275` に到達不能な `Skipped` アームが生きている（ADR-009 が `classify_alive` に当てた手当ての判定側の残り）（Domain W-001）
- [W-008] `application/tick/mod.rs:533-535 Freeze::of_recorded_failure` / コメント — 前提の列挙が本スライスで増えた呼び出し元（`fail_run` / `record_judge_failure`）を数えておらず、成り立つ理由が検算できない（Domain W-002）
- [W-009] `tests/tick_observe.rs:313` + `tests/cli_tick.rs:889` / テストの実効性 — 判定上限超過の凍結について `frozen` 計上と同一 tick 通知を主張するテストが1件もなく、`Freeze` の取り違えが素通しになる（UseCase W-001）
- [W-010] `cli/render.rs:101-113` + `application/tick/observe.rs:186-194` / 報告の分類 — `RemnantsUnhandled` が「失敗を記録」の見出しに入るため、保存に失敗した tick で記録していない失敗が記録済みに見える（UseCase W-002）
- [W-011] `tests/tick_scan.rs:205` / テスト範囲 — 走査レベルの複数タスクのケースが本スライスで配線した Running / Completed / Stopped を1つも含まず、`judged` / `transitioned` / `notified` が同時に載ることを誰も見ていない（UseCase W-003）
- [W-012] `.thread/3/testing.md:812-813` — 判定 timeout の「`sleep 120` が残っていない」期待が ADR-001 の「孫は残りうる（残存は許容）」と食い違う（General W-001）
- [W-013] `.thread/3/testing.md:752` — スナップショット破損への再通知の期待に、ADR-012 の要点（報告は通知と独立に積まれる）が入っていない（General W-002）
- [W-014] `.thread/3/plan.md:115-138` — 手動確認の表に、testing.md が実行・記帳している TC-20 / intervention TC-15 が無い（General W-003）
- [W-015] `.thread/3/plan.md:113` — `cat exit` の期待値が `{"code":0}` の綴りのまま（testing.md 側は値で書く形に修正済み）（General W-004）
- [W-016] `.thread/3/adr.md:ADR-002` — 「既定は絶対パスで固定」が Windows の `taskkill`（PATH 解決名）と一致せず、同ファイルの ADR-007 と表現が割れている（General W-005）
- [W-017] `.thread/3/testing.md:890` — 影響確認に、実装上起こりえない「`SystemCommandRunner` の構築失敗」の確認が残っている（General W-006）
- [W-018] `.thread/3/plan.md:53` — チェックが付く上限「125行」が、部分消化3行のうち `UC-execution-002` を二重に数えていて実際は126行（General 初稿 W-001。改訂版では落ちたが事実として残るため保持）
