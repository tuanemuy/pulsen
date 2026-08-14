# 指摘台帳 — Issue #3

| Key | ラウンド | 判定 | 理由 | 再指摘 |
|---|---|---|---|---|
| `adapter/process.rs:terminate` | 001 | fix | Docker `ubuntu:24.04` の実測どおり Linux で成功が失敗になり、CI の ubuntu ジョブで TC-011 / 012 / 014 が落ちる真正のバグ | 0 |
| `domain/execution/running.rs:classify_alive / 型設計` | 001 | fix | 本番で構築されない変種を返り値型に残し、規則の担保を `unreachable!` に置くのは「不正な状態を型で表現不能にする」に直接反する | 0 |
| `application/tick/mod.rs + notify.rs / 可観測性` | 001 | fix | spec の「スキップして報告。**ただし** notify を実行する」は報告の置換ではなく追加であり、既定構成で最も起きやすい組み合わせが無言で消えている | 0 |
| `application/tick/observe.rs:73 / 層の責務` | 001 | fix | 遷移関数を呼ばずにドメインのエラー値を組み立てており、ADR-081 の分類主義と ADR-004 の判断（破れごとに分類を分ける）の両方から外れる。分岐にテストも無い | 0 |
| `domain/task/task.rs:1943 / テスト` | 001 | fix | ADR-006 で6変種になったのにテストが5要素のままで、`AlreadyNotified` の非同値が1件も主張されていない | 0 |
| `domain/task/task.rs:415-496 / 不変条件` | 001 | fix | `spec/domains/task.md` §不変条件の検証の境界が「遷移関数が前提として検査する」と定めており、規則の実体がドメインの外にある | 0 |
| `application/tick/observe.rs:102 / 報告文` | 001 | fix | judge exit 10 で「実行が終了コード 0 で終了しました」と自己矛盾した行が実際に表示される。W-015 の構造化で同時に解消する | 0 |
| `application/tick/mod.rs:509 / 型` | 001 | fix | `Branch` は分岐を名前で表す型（ADR-091）で、真偽値フラグだけが enum の外に分岐を残している | 0 |
| `domain/task/degraded.rs:271 / テスト` | 001 | fix | 台帳 DOM-task-059 の主張は「Task と同じ規則」であり、1状態のテストではその主張になっていない | 0 |
| `adapter/process.rs:273,616,850 / why コメント` | 001 | fix | 実装は誤殺しない側で正しいが、コメントと `.thread/3/adr.md` ADR-002 が starttime 照合と同等の再利用対策があるかのように書いており実態を超えている | 0 |
| `adapter/process.rs:1119 / Windows unit_is_live` | 001 | fix | 呼び出し前提（ラッパー死亡後）の下でコメントの説明が成り立たず、構造上つねに `NotIdentifiable` になる事実が読めない | 0 |
| `tests/conformance_process_controller.rs:503 / スキップ判定` | 001 | fix | ADR-055 / ADR-073 が `cfg` 決め打ちを明示的に却下し「環境の能力を実測してから宣言する」と定めている。既決 ADR に実装が追いついていない側の指摘 | 0 |
| `adapter/command_runner.rs:86 / timeout` | 001 | fix | `DurationSpec` が上限なしの `u64` 秒を受理する以上 `Instant + Duration` は全域でなく、`started.elapsed()` への置換で消える | 0 |
| `adapter/command_runner.rs:71,88 / 後始末` | 001 | fix | timeout 経路と扱いが揃っておらず、cron で回り続ける tick で放置された子が溜まる | 0 |
| `HOOKS.md:46 / 実測列` | 001 | fix | 同じファイル `:32` の規律「行を足すときは3列を `未測定` で埋める」から外れ、B-001 が空白に隠れていた。出典 run の食い違い（`:30` と `:63`）も同時に揃える | 0 |
| `adapter/process.rs:616 / POSIX unit_is_live` | 001 | fix | 取得元が壊れた場合が静かに `Ok(false)` になり、`observe` が持つ環境固定と終了ステータス規則を共有していない | 0 |
| `application/tick/observe.rs:240-272 + cli/render.rs:222-231 / 層の責務` | 001 | fix | ADR-081（承認済み）が「原因は分類として持ち、文言は `cli::render` が組む」と定めており、永続化されない表示専用の文字列は ADR-090 の例外にも当たらない | 0 |
| `application/tick/notify.rs:87-110 / ドメイン漏れ` | 001 | fix | `.thread/3/adr.md` ADR-003 が Consequences で約束した「#5 の AbortTask が同じ関数を呼べる」が構造上成立していない。判定側（`JudgementService`）と非対称でもある | 0 |
| `tests/tick_notify.rs:70-119 / テストの実効性` | 001 | fix | plan.md のテスト方針が「呼び出し記録の並びで主張する」と定めた順序が、独立したベクタのため実際には検証されていない | 0 |
| `application/tick/observe.rs:151-159 / 報告の欠落` | 001 | fix | 残存プロセスの有無はタスクファイルを書けたかと直交する事実で、保存が失敗した tick でこそ後始末に要る | 0 |
| `tests/tick_notify.rs:276-285 / コメント` | 001 | fix | 二重否定で字義が反転しており、CLAUDE.md の「残すのは現在の形が成り立つ理由だけ」から外れる。主張も1点に乗っている | 0 |
