# PR Review #001 — [tick] 観測・判定・ステータス遷移(リトライ・凍結・通知)

**PR:** #16
**Date:** 2026-08-14
**Round:** 1回目

## Summary

- Blockers: 7（重複除去後。コード2 + 計画ドキュメント5）
- Warnings: 25（重複除去後。コード19 + 計画ドキュメント6）
- Verdict: **BLOCKED**

## レイヤー別ファイル

- Domain: review-001-domain.md（B: 1 / W: 7）
- Adapter / Ports: review-001-adapter.md（B: 1 / W: 7）
- Use Case / CLI: review-001-usecase.md（B: 0 / W: 8）
- General Review（計画ドキュメント）: review-001-general.md（B: 5 / W: 6）

## カバレッジ

- 各観点の申告: Domain 確認21 / スキップ23、Adapter 確認32 / スキップ12、UseCase 確認33 / スキップ11（いずれも一覧44件と一致）
- 確認申告ゼロのファイル: `.thread/3/steps.md`, `.thread/3/testing.md` → General Review を1体追加起動して確認済み（確認申告ゼロのファイルは残らなかった）

## 指摘一覧

重複する指摘は代表IDに束ねた（括弧内が同一指摘の別観点でのID）。

### Blockers

- [B-001] `adapter/process.rs:terminate` — `/bin/kill -TERM -<pgid>`（`--` なし）が Linux で終了ステータス1を返し、`kill` / `try_kill_remnants` が成功時も失敗を返す（Adapter B-001。Docker `ubuntu:24.04` で実測確認済み）
- [B-002] `domain/execution/running.rs:classify_alive` / 型設計 — 本番で構築されない `RunningDecision::Judge` を返り値型に残し、2段規則の担保が型ではなく `observe.rs:160` の `unreachable!` パニックになっている（Domain B-001 / UseCase W-003）

### Warnings

- [W-001] `application/tick/mod.rs` + `notify.rs` / 可観測性 — スナップショット破損 × 未通知 stopped × notify_cmd 未定義が tick の出力から完全に消える（Domain W-001 / UseCase W-004）
- [W-002] `application/tick/observe.rs:73` / 層の責務 — 不変条件4の破れをユースケースが `TransitionError::WorkspaceNotSet` を自作して報告、かつ当該分岐のテストが皆無（Domain W-005 / UseCase W-007）
- [W-003] `domain/task/task.rs:1943` / テスト — `TransitionError` が6変種になったのにテストは5種のままで `AlreadyNotified` を含まない（Domain W-002）
- [W-004] `domain/task/task.rs:415-496` / 不変条件 — 判定確定系5関数が不変条件2・3を検査せず、規則の実体がユースケースにしか無い（Domain W-003）
- [W-005] `application/tick/observe.rs:102` / 報告文 — judge exit 10 のとき「実行が終了コード 0 で終了しました」と自己矛盾した行が出る（Domain W-004）
- [W-006] `application/tick/mod.rs:509` / 型 — `Branch::Notify { notified: bool }` が真偽値フラグで分岐が enum の外に残る（Domain W-006）
- [W-007] `domain/task/degraded.rs:271` / テスト — `DegradedTask::mark_notified` の拒否テストが `Pending` 1状態のみ（Domain W-007）
- [W-008] `adapter/process.rs:273,616,850` / why コメント — 「列挙してから殺す」が ADR-002 の謳う PGID 再利用の誤殺を実際には防いでおらず、コメントが実態を超えている（Adapter W-001）
- [W-009] `adapter/process.rs:1119` / Windows `unit_is_live` — 残存終了が構造上つねに `NotIdentifiable` になり、コメントの説明が前提と矛盾（Adapter W-002）
- [W-010] `tests/conformance_process_controller.rs:503` / スキップ判定 — 許容集合が `cfg!(unix)` 決め打ちで、HOOKS.md の宣言・ADR-055 / 073 と不一致（Adapter W-003）
- [W-011] `adapter/command_runner.rs:86` / timeout — `Instant::now() + limit` が極端な `judge_timeout` でパニックしうる（Adapter W-004）
- [W-012] `adapter/command_runner.rs:71,88` / 後始末 — `wait` / `try_wait` の `Err` 経路で子を終了も回収もしない（Adapter W-005）
- [W-013] `HOOKS.md:46` / 実測列 — 新規行の macOS 列がローカル実行で埋まっており、「新規行は `未測定`」の規律から外れて B-001 を隠している（Adapter W-006）
- [W-014] `adapter/process.rs:616` / POSIX `unit_is_live` — `identity_command` の環境固定と終了ステータス判定を共有していない（Adapter W-007）
- [W-015] `application/tick/observe.rs:240-272` + `cli/render.rs:222-231` / 層の責務 — 報告の完成文言をユースケース層で組み立てており ADR-081 に反する（UseCase W-001）
- [W-016] `application/tick/notify.rs:87-110` / ドメイン漏れ — 通知の成否解釈が `Tick` の private メソッドにあり、ADR-003 が約束した「#5 の AbortTask が同じ関数を呼べる」が構造上成立しない（UseCase W-002）
- [W-017] `tests/tick_notify.rs:70-119` / テストの実効性 — 「stopped の save → notify_cmd」の順序が検証されておらず逆順でも緑になる（UseCase W-005）
- [W-018] `application/tick/observe.rs:151-159` / 報告の欠落 — 残存プロセスの結末を save 成功時にしか報告しない（UseCase W-006）
- [W-019] `tests/tick_notify.rs:276-285` / コメント — テストの組み立て事情の説明が二重否定で意味反転しており、主張も `commands.calls()` のみで弱い（UseCase W-008）

### 計画ドキュメント（General Review。単位7 で一括対応）

コード修正（単位1〜6）が固まってから追従させる。先に直すと単位3・5 の変更でまた古くなるため。

- [B-003] `steps.md:29,107,174,220` — tick サマリー DTO を「10フィールド」のまま書いており、ADR-005 で足した `judged`（表示は「判定確定」）が欠落（General B-001）
- [B-004] `steps.md:62,142` — `TransitionError` を「5種・新規実装不要」としたままだが、ADR-006 の `AlreadyNotified` で6種に増えている（General B-002）
- [B-005] `testing.md:879` — サマリー見出し列から「判定確定」が抜け、「初めて値が入るのは3つ」も誤り（4つ）。関数名 `render_tick` も実在せず（`tick_summary`）、ADR-101 の引用も不適当（General B-003）
- [B-006] `testing.md:284,552,609,636` — 直読の JSON パス誤り（`.kill_ident` は `.current_attempt.process.kill_ident`、`.execution.state` にオブジェクト全体を当てている3箇所）（General B-004）
- [B-007] `testing.md:55` — AC-7 の `#[cfg]` grep 期待件数が「4 / 10 / 1」のままで、実装後の実態は 4 / 12 / 1（General B-005）
- [W-020] `steps.md:174` — 新 `TickIssue` の列挙に `RunFailed` が無い（General W-001）
- [W-021] `steps.md` 全体 — 実装中に起票された ADR-005 / 006 / 007 が1件も反映されておらず、ステップ7 の HOOKS.md 更新内容も実装と不一致（General W-002）
- [W-022] `testing.md` 各所 — `exit` の期待値 `{"code":0}` が実際の整形 JSON と綴りで一致しない（General W-003）
- [W-023] `testing.md:738` — エッジケース1 の対照手順が2択で未確定、第1案は同一 task_id のファイルを2つ作る（General W-004）
- [W-024] `testing.md:286` vs `851,901` — TC-20 を「対象外」と書きながらエッジケース6 で実行対象にしており、記帳一覧からも漏れている（General W-005）
- [W-025] `testing.md:641-666,901` — intervention TC-15（catch-up 通知）が採用にも見送りにも現れない（General W-006）
