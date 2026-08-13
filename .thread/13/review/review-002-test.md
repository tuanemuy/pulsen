# レビュー 002 — Test（テストフィクスチャ設計・スキップ判定の正しさ）

対象: PR #14 / `origin/main...HEAD`（`b344401`。1周目は `aacc9af`）
基準: `CLAUDE.md`（テスト方針）、`.thread/13/plan.md`（AC-1〜AC-13・スコープ）、`crates/pulsen-conformance/HOOKS.md`、`.adr/027` `.adr/032` `.adr/055` `.adr/060` `.adr/068` `.adr/071` `.adr/073`
前提: `.thread/13/review/triage.md` の判定に従い、`wont-fix`（W-005 / W-011 / W-015 / W-016）と `defer`（W-017 → #15）は再指摘しない。

## 方法 — 修正後のコードで判定経路を踏み直した

1周目の修正（`HolderProgram` / `SignalUnreadable { holder, error }` / `Disconnected` を失敗側へ / `.adr/073`）は `start_holder` の腕そのものを触っているので、宣言と実態の対応は測り直さないと分からない。レビュー用の git worktree（`HEAD` の detached、別 `CARGO_TARGET_DIR`）で `conformance_lock` を4通り走らせた（作業ツリーには触れていない。worktree は撤去済み、`lock_holder` のプロセス残りなし）。

| 作った状態 | 結果 |
|---|---|
| 素（example ビルド済み） | 緑・7件実行。ロック系の `SKIP` は0件 |
| `SIGNAL_DEADLINE` を `from_nanos(1)` | **緑**。`002` / `003` / `004` / `005` の**ちょうど4件**が `SKIP`（AC-3 / AC-9） |
| `target/debug/examples/lock_holder` を削除 | **赤**。4件が失敗し `SKIP` は0件。不在と `cargo test --workspace` の案内が載る（AC-4） |
| example はビルド済みだが**合図を返さない**（`lock_holder` に `sleep` を差し込み） | **緑**。4件が `SKIP`（10.01s で終了 = probe の1回だけが期限を待つ） |

最後の1行が W-001。そのほか、3 OS の CI は修正後の `b344401` でも run 31683976608 が7ジョブ success で、ログの `SKIP` 行は unix 1件 / Windows 11件（+ 架空3行）、ロック系5件は3 OS とも0件だった（予測と一致。W-005 はその記録の出典についての指摘）。

判定の要点。**許容集合は `SignalTimedOut` の1点からしか広がらず、2つの `allowed_skips()` はワイルドカードなしで同じ4区別を同じ側へ振り分けている。`HolderProgram` でパスが `lock.rs` の外に出なくなり、`Disconnected` を失敗側へ倒したことも許容集合を狭める向きにしか効いていない。1周目の修正がスキップ判定に新しい穴を開けてはいない。** 残るのは、`SignalTimedOut` という腕そのものが最初から抱えている残余（W-001）と、正本・手順書側の記述の古さである。

## Blockers

なし。

## Warnings

- **[W-001]** 合図を返さなくなったフィクスチャの退行が、「環境の能力」として静かに緑になる。正本はこの腕を「環境の遅さ」としか述べていない
  - 場所: `.adr/073-holder-capability-skip-vs-fail.md:24`（`合図が期限内に返らない(SignalTimedOut) → 能力側`）、`crates/pulsen/tests/common/lock.rs:30`（`起動はできるが、合図が期限内に返らない(環境の遅さ)`）
  - 理由: `lock_holder` がビルドされていて起動もできるが合図を書かない状態（今回は `sleep` の差し込みで作ったが、保持プロセス側の退行・`try_acquire` が返らないファイルシステム・stdout のバッファリング変更などで同じ形になる）で `conformance_lock` を回すと、**未変更の `SIGNAL_DEADLINE` のまま4件が `SKIP` になり、スイートは緑で終わる**（実測。所要 10.01s）。plan.md の目的が切り分けようとした2つ — 「フィクスチャの用意の問題（緑にしてはいけない）」と「環境の能力（スキップでよい）」— のうち、前者の**別の形**がまだ後者に潰れている。実行ファイルの不在は塞がったが、実行ファイルが**壊れている**場合は塞がっていない。probe が本番と同じ手順を踏む以上この2つは原理的に区別できず、機械的な歯止め（CI 側）は plan.md がスコープ外としているので、直すべきは実装ではなく**正本の述べ方**だと考える。現状の `.adr/073` は `SignalTimedOut` を能力側に置く理由を「観測が『合図が期限内に返らなかった』に閉じており、環境の遅さという説明と、負荷の低い環境なら成立するという見通しが宣言から読める」と書いており、`SKIP` 一覧を読む人（AC-10 の歯止めはこの人だけ）に「遅いマシンなのだろう」と読ませる。フィクスチャが壊れた場合も同じ行が出ることは、どこにも書かれていない。
  - 提案: `.adr/073` の当該箇所か `## 影響` のトレードオフに1文、「この腕には保持プロセス側が合図を返さなくなった退行も入る。probe は本番と同じ手順を踏むため両者を区別できず、`SKIP` 一覧に5件が並んだときは環境の遅さと同じだけフィクスチャの退行も疑う」旨を足す。`lock.rs:30` の `(環境の遅さ)` も原因を1つに決め打たない書き方（`合図が期限内に返らない`）に寄せる。実装を触るなら、probe が `SignalTimedOut` に倒れた回だけ `lock_holder` の終了コード・生死を添えて `SKIP` の前に出す形が最小だが、区別が付かない事実は変わらないので、記述の手当てで足りると判断している。

- **[W-002]** probe 成立後のタイムアウトを失敗に倒す理由が、同じ ADR のトレードオフ欄と噛み合っていない
  - 場所: `.adr/073-holder-capability-skip-vs-fail.md:41`（`温まったあとの期限超過は環境の遅さではなく異常として読める`）と同 `:70`（`相殺できるのは cold-start 由来の遅さだけで、負荷由来の遅さは probe の測定条件に入っていない`）
  - 理由: 41行は「probe が通った＝温まった」→「以後の期限超過は遅さではない」と読ませるが、70行は probe が測っているのが cold-start ぶんだけで、負荷由来の遅さは測定条件の外だと明言している。並列に走る本番5件のほうが probe より混んだ条件にあるのだから、負荷で本番だけが期限を超える形は 70行の言うとおり残る。AC-5 の設計（失敗に倒す）自体は、赤が出る＝人が判断するという安全な向きなので支持できるが、**理由の述べ方が「起きえない」に寄っている**ぶん、実際に赤が出たときに読み手が「異常だ」と誤って狭く探すことになる。`spawn_holder` のパニック文言（`この環境で繰り返し起きるなら SIGNAL_DEADLINE を見直す`）のほうが、繰り返しかどうかで判断させる形になっていて実態に合っている。
  - 提案: 41行を「temp の差はあるが、probe が通った以上この期限超過は環境の能力の宣言としては読めない。繰り返すなら閾値の見直しとして扱う（ADR-071 の停止規則の内側）」の趣旨に直し、`.adr/073:70` と同じ前提の上に立たせる。

- **[W-003]** 1周目の修正が正本（HOOKS.md）だけに入り、手順書・検証手順は修正**前**の形を要求したまま残っている
  - 場所: `.thread/13/testing.md:276`、`.thread/13/testing.md:280`、`.thread/13/steps.md:302`、`.thread/13/steps.md:308`、`.thread/13/testing.md:137`
  - 理由: triage で `fix` にした test W-004（測っていない原因の推定）と docs W-003（実測に解釈を足した）は、`crates/pulsen-conformance/HOOKS.md` 側では正しく落ちている（43行から `（初回起動のスキャン・高負荷）` が消え、58行から `起動した保持プロセスの合図が期限内に返り、` が消えた）。ところが、その HOOKS.md をどう書くかを指示している steps.md ステップ6 と、それを検証する testing.md 確認項目12 は、いずれも**落としたはずの文言をそのまま期待値として書いている** — `steps.md:302` は「→ 『保持プロセスの合図が期限内に返らない(初回起動のスキャン・高負荷)』」、`steps.md:308` は「走った理由に『合図が期限内に返った』ことも含める」、`testing.md:276` / `:280` はそれぞれの期待結果。したがって**確認項目12 をいま手順どおり実行すると、期待結果2箇所が現物と食い違って不合格になる**（AC-8 は実際には満たされているのに、検証手順のほうが古い）。さらに `testing.md:137` の確認ポイントは「Windows の初回起動スキャンの代金を probe が先に払うぶん本番5件は温まった状態で走る」と断定しており、`.adr/073:70` が「見込める」に留めた同じ主張を、測っていないまま事実の形で残している。手順書は次にこの経路を触る人が読む道具なので、古い期待値を残すと、一度落とした主張が「手順どおりに直して」戻ってくる。
  - 提案: `steps.md:302` / `:308` と `testing.md:276` / `:280` を現物（括弧なし・実測節は変更なし）に合わせ、`testing.md:137` の断定は `.adr/073:70` と同じ語気（先払いの効果は見込みであって測っていない）に揃える。

- **[W-004]** `.thread/13/adr.md` ADR-007 が「`ProgramUnusable` は実地の検証を持てない」と述べたまま残っている
  - 場所: `.thread/13/adr.md:225`（`トレードオフ: ProgramUnusable の経路は3 OS で安定して再現する手段が無く、実地の検証を持てない(検証はコードレビューによる)`）、および同 `:211`（`SignalUnreadable(Child)`）
  - 理由: triage は test W-006 を `fix` とし（「unix の `chmod 000` で確定的に踏めることが実測済み」）、plan.md・steps.md・testing.md・PR 本文はいずれも「unix では実地で踏む／Windows だけコードレビュー」に直っている。作業ログの ADR-007 だけがその逆を述べていて、しかもそれが `ProgramUnusable` を独立の区別として立てた判断のトレードオフ欄という、次に読む人が根拠として当たる場所にある。あわせて 211行の `SignalUnreadable(Child)` は1周目の修正で `SignalUnreadable { holder, error }`（読み取り失敗の理由を運ぶ）に変わっており、決定の本文が現在の形と綴りも中身もずれている（fix-plan 単位3 は `ProgramUnusable(String)` の綴りだけを直す指示になっていて、この2箇所が漏れた）。
  - 提案: 225行のトレードオフを「unix では `chmod 000` で確定的に踏める。3 OS 一様の手段が無いのは Windows 側だけ」に直し、211行を `SignalUnreadable { holder, error }` と、理由をパニック文言に載せる形まで含めて現在の決定に合わせる。

- **[W-005]** AC-10（予測 → 実測 → 突き合わせ）の記録が、修正前のコミットの run を出典にしたまま
  - 場所: `.thread/13/steps.md:381-382`（ステップ9 手順4・5）、`.thread/13/testing.md:338-341`（確認項目15 の実測）、PR #14 本文「3 OS CI の実測との突き合わせ」
  - 理由: 3箇所とも出典が run 31681471522 で、これは1周目の修正前の `aacc9af` の run である。1周目の修正は `start_holder` の腕（`Ok(Err)` / `Timeout` / `Disconnected` の振り分け）という、まさに `SKIP` になるか失敗になるかを決める場所を触っているので、その run は現在の HEAD の宣言と実態を突き合わせた記録にはならない。AC-10 は「この変更の宣言が正しいこと」を確かめる唯一の3 OS 検証であり、`.adr/068` が定めた順序（予測を先に、実測を後に）は出典が現在の形を指していて初めて意味を持つ。なお実害は無い — 修正後の `b344401` で run 31683976608 が7ジョブ success、ログの `SKIP` 行も unix 1件 / Windows 11件・ロック系5件は3 OS とも0件で、予測と一致していることをこちらで確認した。
  - 提案: 3箇所の出典を run 31683976608（`b344401`）に差し替える。予測（手順1）はそのままでよく、実測と突き合わせの節だけを新しい run の観測で書き直す。

- **[W-006]** `tc_task_register_task_017` の `SKIP` 行には、HOOKS.md が新しく用意した読み替え先が無い
  - 場所: `crates/pulsen-conformance/HOOKS.md:47`（`適用先で実際に成立しなかった条件は、この表の「判定」列の括弧で読む`）と `crates/pulsen/tests/cli_add_error.rs:130`（`common::skipped("tc_task_register_task_017", "lock::hold")`）
  - 理由: 1周目の W-007（`SKIP` の文言が「フィクスチャ未提供」に見える）への手当てとして 47行の1文が入り、適合4件についてはこれで筋が通る — ログは `ハーネスが hold_from_other_process を提供しないため…` と出て、その綴りが表の「判定」列にそのまま載っているので括弧の補足へ辿れる（実測で確認）。ところが 017 は `ハーネスが lock::hold を提供しないため…` と出るのに、`tc_task_register_task_017` も `lock::hold` も HOOKS.md に**1度も現れない**（CLI 側で表に名前があるのは権限系の 016 / 021 だけ）。W-007 が名指ししていたのはまさにこの1件で、読み替え先が無いままになっている。AC-10 の突き合わせは3 OS 分の `SKIP` 一覧を人が読む作業なので、5件のうち1件だけ意味を引けない状態は、W-001 の「フィクスチャの退行を疑う」判断の入口も狭める。
  - 提案: HOOKS.md の「3ランナーでの実測」で 016 / 021 を CLI 側の同述語ケースとして挙げている56行と同じ形で、017 を「保持プロセスの述語を共有する CLI 側の受け入れケース」として1箇所に書く（表の行を増やす必要はない）。ラベル `lock::hold` を変えない判断（triage）はそのままでよい。

## 観点ごとの所見（指摘に至らなかったもの）

**1周目の修正が開けた穴は無い。** (1) `HolderProgram` はタプルフィールドも `path()` も private で、`lock.rs` の外から `Available` のペイロードを取り出す手段は無い（AC-2 の「コンパイラが保証する」が変種のペイロードまで届いた）。(2) `Disconnected` を `SignalUnreadable`＝失敗側へ倒したのは許容集合を**狭める**向きの変更で、この腕は「期限を測っていない」以上スキップの根拠にならないという理由と実装が一致している。(3) `probe_holder` が `Signaled` と `SignalUnreadable` を同じ `Available` に畳むことは変わっていないが、畳んだ先が失敗側でないケース（本番で読み取りに失敗したら panic）なので、静かな緑は作らない。(4) `Started` は private のままで、`spawn_holder` 以外に `start_holder` を呼ぶ経路は無い。

**`SkipBudget` の宣言と実態の対応。** `conformance_lock.rs` の4件は `hold_from_other_process`（002 / 003 / 005）と `try_acquire_from_other_process`（004）の `require!` 経由でしかスキップし得ず、両方とも `spawn_holder` → `holder_capability()` の1点を見る。`kill_holder` / `release_holder` / `separate_home` / `unusable_lock` は `None` を返さない（`kill_holder` の `None` は `Available` の環境では宣言の外なので失敗になり、`SignalTimedOut` の環境では手前の `hold` で先にスキップに落ちて到達しない）。`common/mod.rs` 側の1件も `lock::hold` 経由の1点。`SkipBudget` は宣言の余りを咎めないが、`Available` の環境では両方の宣言が空集合になるので余りも出ない。実測（1ns の状態で4件ちょうど）とも一致する。

**評価順・並列度。** `allowed_skips()` は `conformance_cases!` の `LazyLock` が最初のケースの `report` で確定させ、`common/mod.rs` は最初の `record` で確定させる。どちらも `OnceLock<HolderCapability>` の同じ1値を読み、`holder_capability()` から `skipped` / `record` を呼ぶ経路は無いので環にならない。「観測を記録してから宣言へ反映する」形への後退も無い（plan.md リスク欄の懸念どおりに保たれている）。

**probe と本番の経路差。** probe のロック置き場は `tempdir()` 直下（親が既存）、本番は `home/state/lock`（`ensure_dir` が1回増える）という差は残るが、そこで壊れる環境はケース側の失敗として現れる（`hold` の `!locked` パニック、TC-001 / 006 / 007 も落ちる）ので、静かな緑にはならない。probe が `locked` を捨てることも同じ理由で妥当。

**スコープ。** `crates/pulsen-conformance/src/` と `crates/pulsen/examples/lock_holder.rs` に差分なし、`SIGNAL_DEADLINE` の値は `Duration::from_secs(10)` のまま（差分は doc のみ。AC-7）、`cfg(windows)` の決め打ちなし、`.adr/068` は無変更。逸脱は見つからなかった。

## カバレッジ

- 確認: `crates/pulsen/tests/common/lock.rs`, `crates/pulsen/tests/common/mod.rs`, `crates/pulsen/tests/conformance_lock.rs`, `crates/pulsen-conformance/HOOKS.md`, `.github/workflows/ci.yml`, `.adr/073-holder-capability-skip-vs-fail.md`, `.thread/13/plan.md`, `.thread/13/steps.md`, `.thread/13/testing.md`, `.thread/13/adr.md`, `.thread/13/review/triage.md`, `.thread/13/review/fix-plan-001.md`, `.thread/13/review/review-001-test.md`
- スキップ: `.thread/13/review/review-001.md` — レビューループ自身の成果物（判定は triage で確認済み）
- スキップ: `.thread/13/review/review-001-concurrency.md` — 同上
- スキップ: `.thread/13/review/review-001-docs.md` — 同上
- スキップ: `.thread/13/review/review-001-type-design.md` — 同上

差分外で判断の材料に読んだもの: `crates/pulsen/tests/cli_add_error.rs`（017 と他のスキップ経路）、`crates/pulsen/examples/lock_holder.rs`、`crates/pulsen/src/adapter/lock.rs`、`crates/pulsen-conformance/src/lib.rs`（`SkipBudget` / `require!` / `conformance_cases!`）、`crates/pulsen-conformance/src/exclusive_lock.rs`、`.adr/027` `.adr/032` `.adr/055` `.adr/060` `.adr/068` `.adr/071`、GitHub Actions run 31683976608 / 31681471522。
