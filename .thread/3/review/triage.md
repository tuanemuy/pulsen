# 指摘台帳 — Issue #3

| Key | ラウンド | 判定 | 理由 | 再指摘 |
|---|---|---|---|---|
| `adapter/process.rs:terminate` | 001 | fix | Docker `ubuntu:24.04` の実測どおり Linux で成功が失敗になり、CI の ubuntu ジョブで TC-011 / 012 / 014 が落ちる真正のバグ | 1 |
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
| `adapter/process.rs:terminate / 同定子の parse`（B-001） | 002 | fix | `KillIdent` は永続化された不透明値で、手動修復・破損した run ファイルから `-1` / `-0` / `0` が到達しうる。形式を知っているのはアダプターだけなので、境界で parse するのが CLAUDE.md の「検証は境界で一度だけ」どおり。ラウンド1 の同一 Key（`terminate`）を継承して fix | 0 |
| `adapter/process.rs:terminate / 終了の保証`（W-001） | 002 | fix | 契約は `kill` の `Ok` に「実行単位に属する全プロセスが終了する」を求めており、SIGTERM を捕まえたエージェントが生きたまま `Ok` → `fail_run` → 同一 worktree 並走は契約違反。SIGKILL への昇格は依存も `unsafe` も増やさない | 0 |
| `adapter/process.rs:terminate / 実体依存`（W-002） | 002 | fix | ラウンド1 の `--` 追加が busybox で裏目に出た事実は「成否を外部コマンドの終了ステータスだけに預ける構造」の問題で、`--` の有無では解けない。成否を観測（`unit_is_live`）へ移す | 0 |
| `adapter/process.rs:unit_is_live / why コメント`（W-003） | 002 | fix | 呼び出し側で `Ok(false)` と同じ `NotIdentifiable` に畳まれる以上、コメントが謳う利益は成立していない。実態を超えたコメントはラウンド1 の同種指摘と同じ性質 | 0 |
| `doubles/task_repository.rs:saved_degraded / テストの実効性`（W-004） | 002 | fix | ラウンド1 で入れた `RecordSeq` が `Task` 経路だけを守り、`save_degraded` 経路は `mark_notified` 先行の実装でも緑になる。at-least-once の破れ方は同じ | 0 |
| `HOOKS.md:49 / 正本性`（W-005） | 002 | fix | 「スキップを許容する条件の一覧」を名乗る正本が実際より広い集合を書いており、読み手の宣言がずれる。TC-003 / 004 は judge_probe を要さない | 0 |
| `HOOKS.md:78-80 / 実測の内訳`（W-006） | 002 | fix | 本 PR のテスト追加で `ps` 系の件数が変わり、構造の記述がコミットの形と食い違う。表側の `未測定` の規律がこの段落にだけ効いていない | 0 |
| `domain/execution/judgement.rs:default_judgement / 型設計`（W-007） | 002 | fix | ラウンド1 で `classify_alive` に当てた ADR-009 の手当て（規則の担保をコメントから型へ戻す）の判定側の残り。同じ形の指摘を片方だけ直すのは一貫しない | 0 |
| `application/tick/mod.rs:533-535 / コメント`（W-008） | 002 | fix | 前提として挙げた集合が実際の呼び出し元と食い違い、読み手が結論を検算できない。CLAUDE.md の「残すのは現在の形が成り立つ理由だけ」から外れる | 0 |
| `tests/tick_observe.rs:313 + tick_notify.rs / 判定上限の凍結`（W-009） | 002 | fix | 上限超過3経路のうち判定上限だけ `Freeze` の取り違えを検出するテストが無く、AC-4 の「同一 tick 内の通知」が素通しになる | 0 |
| `cli/render.rs:101-113 / 報告の分類`（W-010） | 002 | fix | ADR-098 が見出しを「タスクファイルに何を残したか」で分けたのに、何も残さない `RemnantsUnhandled` が「失敗を記録」に出る。ADR-010 で保存と独立に積むようにした帰結で、見出し側が追随していない | 0 |
| `tests/tick_scan.rs:205 / テスト範囲`（W-011） | 002 | fix | 走査レベルで `judged` / `transitioned` / `notified` が同時に載ることを誰も見ておらず、未配線アーム時代の構成を引き継いでいる | 0 |
| `.thread/3/plan.md:63-72 / スキップ許容集合`（B-002） | 002 | fix | AC-8 の記帳が参照する唯一の一覧が probe 化後の実装（2集合）と3点食い違い、常に走る2行を「環境の都合で未チェック」と記帳しうる | 0 |
| `.thread/3/plan.md:44,49 / DOM-task-053`（B-003） | 002 | fix | 本スライスで `AlreadyNotified` を足して6変種にした事実が落ち、#5 が「変種は増えていない」という誤った前提を引き継ぐ | 0 |
| `.thread/3/plan.md:96-97 / spec 差分`（B-004） | 002 | fix | AC-8 が名指しする記帳の正本が `steps.md:259` と割れている（DTO 10 vs 11 フィールド・`AlreadyNotified` の欠落） | 0 |
| `.thread/3/testing.md:812-813 / 判定 timeout の期待`（W-012） | 002 | fix | ADR-001 が「孫は残りうる（残存は許容）」と明記した範囲を期待が超えており、契約どおりの実装を不合格と読ませる | 0 |
| `.thread/3/testing.md:752 / 再通知の期待`（W-013） | 002 | fix | ラウンド1 で直した ADR-012 の要点（報告は通知と独立に積む）を手動確認が主張しておらず、回帰を捕まえる唯一の位置が空いている | 0 |
| `.thread/3/plan.md:115-138 / 手動確認の表`（W-014） | 002 | fix | testing.md が実行・記帳している TC-20 / intervention TC-15 が契約側の表に無く、記帳と実行範囲が2件ぶん食い違う | 0 |
| `.thread/3/plan.md:113 / cat exit の期待値`（W-015） | 002 | fix | ADR-080 のとおり整形 JSON になる事実が、testing.md と plan.md で別の書かれ方をしている | 0 |
| `.thread/3/adr.md:ADR-002 / 既定の実体`（W-016） | 002 | fix | 同じファイルの ADR-007 と表現が割れており、#10 が ADR-002 だけを読むと Windows の既定を絶対パスと誤解する | 0 |
| `.thread/3/testing.md:890 / 影響確認`（W-017） | 002 | fix | `wire::command_runner` は無謬なので、実行者がこの確認を作れない | 0 |
| `.thread/3/plan.md:53 / チェック上限125行`（W-018） | 002 | wont-fix | 誤り。Issue #3 のチェックリストは実測128行で `UC-execution-002` を含まず、部分消化3行は `UC-flow-001 / 003 / 008` のみ。二重計上は無く 128 − 3 = 125 が正しい（General の改訂版でも落ちている） | 0 |
