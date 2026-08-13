# レビュー 003 — Test（テストフィクスチャ設計・スキップ判定の正しさ）

対象: PR #14 / `origin/main...HEAD`（`0668dcc`。1周目 `aacc9af` / 2周目 `b344401`）
基準: `CLAUDE.md`（テスト方針）、`.thread/13/plan.md`（AC-1〜AC-13・スコープ）、`crates/pulsen-conformance/HOOKS.md`、`.adr/027` `.adr/032` `.adr/055` `.adr/060` `.adr/068` `.adr/071` `.adr/073`
前提: `.thread/13/review/triage.md` の判定に従い、`wont-fix`（probe の多重化 / 許容方針の複製 / probe の `expect` / stderr を捨てる / `thread::spawn` のパニック）と `defer`（`release` の期限・`kill_and_wait` の期限 → #15）は再指摘しない。

## 方法 — 2周目の修正が触った5箇所を、宣言と実態の両側から当て直した

2周目は「コードの実装（型・分岐・後始末の構造）は変えない」方針だったので、判定経路そのものは 002 で測った形から動いていない。したがって今回は (a) 変更が実際に doc とコメントに閉じているか、(b) 触った箇所が他の記述・検証手順と食い違いを作っていないか、の2点に絞った。

- `git diff origin/main...HEAD -- crates/pulsen/tests/ .github/workflows/ci.yml crates/pulsen-conformance/HOOKS.md` を読み、`SIGNAL_DEADLINE` の値・`match` の腕・`kill_and_wait` の呼び出し位置（5箇所）・`Started` / `HolderCapability` の変種がいずれも 002 時点と同じであることを確認した。
- `cargo clippy --workspace --all-targets --locked -- -D warnings` を回して exit 0（AC-11。2周目で `use` を触ったため未使用インポートの有無を機械的に見た）。
- 判定の1点性を grep で当て直した — `holder_program` は `common/lock.rs` の定義と `probe_holder` からの1呼び出しだけ、`holder_capability` は定義・`spawn_holder`・2つの `allowed_skips()`・`use` 行の5行、`OnceLock` / `LazyLock` は6行（`CAPABILITY` / `SKIPS` / `OUTSIDE`）、`lock.rs` の中に `release` の呼び出しは無し。
- スイート側（`crates/pulsen-conformance/src/exclusive_lock.rs` / `lib.rs`）でロック系4件がスキップに落ちうる口を数え直した — `require!(hold_from_other_process)`（002 / 003 / 005）と `require!(try_acquire_from_other_process)`（004）だけで、いずれも `spawn_holder` → `holder_capability()` の1点を見る。`kill_holder` / `release_holder` の `None` は `Available` の環境では宣言の外なので失敗になり、`SignalTimedOut` の環境では手前で先にスキップへ落ちて到達しない（2周目に入った `conformance_lock.rs:67-69` のコメントはこの通りで、正しい）。

**結論として、2周目の修正はスキップ判定に新しい穴を開けていない。** `use` の統一は綴りだけの変更で、`common/mod.rs` に入った導線と `kill_holder` のコメントはいずれも実物と一致し、`.adr/073` に足した3点（`SignalTimedOut` にフィクスチャの退行も入る／probe 成立後の期限超過を「起きえない」に寄せない／`kill` が届く限りという条件）も現在のコードの読み方を狭めていない。残る指摘は、2周目が**コード側を触った結果に手順書・作業ログ・doc の一部が追随していない**という、1周目→2周目に出た W-027 と同じ形の取り残しである。

## Blockers

なし。

## Warnings

- **[W-001]** 2周目の `use` 統一（W-036）に手順書と検証手順が追随しておらず、確認項目2・3・4 をいま手順どおり実行すると3箇所が現物と食い違う
  - 場所: `.thread/13/testing.md:87`（確認項目2 手順3 の期待結果）、`.thread/13/testing.md:106`（確認項目3 手順3）、`.thread/13/testing.md:120`（確認項目4 手順1）、`.thread/13/steps.md:247` `:251` `:266` `:276`（ステップ4・5 の指示と code block）
  - 理由: 2周目の単位C が `conformance_lock.rs:9` を `use common::lock::{HolderCapability, hold, holder_capability, release, spawn_holder};` に寄せ、`:63` / `:111` をフルパスから短縮形へ変えた。その結果、
    - `grep -rn "holder_capability" crates/pulsen/tests/` の**実際のヒットは5行**（`common/lock.rs` の定義と `spawn_holder` の呼び出し、`common/mod.rs` の `allowed_skips()`、`conformance_lock.rs` の `use` 行と `allowed_skips()`）になったが、`testing.md:87` は「3箇所。`spawn_holder` からの呼び出しを加えて…」＝4行を期待している。**説明の付かないヒットが1件出る。**
    - `testing.md:106` は `hold_from_other_process` の本文を `common::lock::hold(&self.lock_path())` と逐語で期待しているが、現物は `hold(&self.lock_path())`。
    - `testing.md:120` は `match common::lock::holder_capability()` を期待しているが、現物は `match holder_capability()`。腕の形（`_` なし・4区別）は一致しているので、判定そのものは正しい。
    - `steps.md:251` `:266` の code block と `:247` の指示文はフルパスの形のまま、`:276` は `use` を `common::lock::{HolderCapability, release, spawn_holder}` に整理せよと書いている（現物は `hold` / `holder_capability` を含む5項目）。
    これは1周目 → 2周目で `fix` にした W-027（正本とコードにだけ修正が入り、手順書に波及していない）とまったく同じ形で、単位C がコードを触ったのに単位E の対象行が W-026 / W-027 由来の行に限られていたために再発している。手順書は次にこの経路を触る人が読む道具で、`steps.md:276` に至っては 2周目に統一した `use` を**元へ戻す指示**として残っている。実害の中心は AC-2 / AC-3 / AC-6 の検証がそのままでは不合格に見える点で、フィクスチャの挙動そのものには影響しない。
  - 提案: `testing.md:87` を「ヒットは5行（`common/lock.rs` の定義と `spawn_holder` からの呼び出し・`common/mod.rs` の `allowed_skips()`・`conformance_lock.rs` の `use` 行と `allowed_skips()`）」に、`:106` / `:120` を短縮形の綴りに合わせる。`steps.md:247` `:251` `:266` `:276` も同じく現物へ揃える（`:276` は `{HolderCapability, hold, holder_capability, release, spawn_holder}`）。直したあと、確認項目2・3・4 を手順どおり1回通して期待結果が全部一致することまで見る。

- **[W-002]** `.thread/13/adr.md` ADR-005 が、`Disconnected` を失敗側へ倒した1周目の修正と食い違う理由づけのまま残っている
  - 場所: `.thread/13/adr.md:150`（`合図を読み取れなかった場合も、期限内に返ったことは確かなので Available に数える`）
  - 理由: `start_holder` は `RecvTimeoutError::Disconnected`（読み取りスレッドが結果を送らずに消えた場合）も `Started::SignalUnreadable` に寄せ、`probe_holder` はそれを `Available` に数える（`crates/pulsen/tests/common/lock.rs:101-105` / `:147-150`）。この経路では**子の応答そのものを観測していない**ので、「期限内に返ったことは確か」は成り立たない。1周目の W-025 はこの点を突いた指摘で、`.adr/073:39` は既に「`Available` を名乗る根拠は経路によって『合図が期限内に返った』と『期限の超過を観測しなかった』の2つに分かれる」と直っており、`lock.rs:98-100` のコメントも同じ形に揃っている。作業ログ側だけが旧い断定を残していて、しかもそこは「なぜ `HolderCapability` に区別を足さないか」を決めた Decision の本文＝次に読む人が根拠として当たる場所である（2周目 W-028 で ADR-007 について同じ形の取り残しを直したのと同じ理由が、ADR-005 にも掛かる）。
  - 提案: `:150` を `.adr/073:39` と同じ水準に直す — 「合図を読み取れなかった場合も `Available` に数える。能力側へ倒すのは期限の超過を実際に観測したときだけで、読み取りが結果を返さずに終わった場合はその観測が無いまま `Available` に入る（倒れる向きは失敗側）」の趣旨。判断そのもの（`locked` を捨てる・区別を足さない）は変えなくてよい。

- **[W-003]** `kill_and_wait` の doc「待たない」が、本文の無期限 `wait()` とも、2周目に条件つきへ直した `.adr/073` とも逆のことを述べている
  - 場所: `crates/pulsen/tests/common/lock.rs:209`（`/// 保持プロセスを畳む。正常終了できるかは測っていないので待たない。`）と `:210-213` の本文（`let _ = holder.kill(); let _ = holder.wait();`）
  - 理由: 2周目は W-037 の「本PRで直す分」として `.adr/073:47` と `.thread/13/adr.md` ADR-005 の断定を条件つきに直し、`.adr/073:47` は「**終了の回収そのものには期限が無い**ので、`kill` を受けても即座には終われない相手に当たれば、揃えたあとの形もそこで止まる」と明記した。ところがその回収を実装している関数の doc は「待たない」とだけ書いてあり、正本が「期限の無い待ちが1つ残っている」と述べている場所を、コード側が「待ちは無い」と読ませる。`lock.rs` の中で期限の無い待ちを探す読み手（#15 の担当者はまさにこれをやる）はこの1行で通り過ぎる。意図としては「`release` の待ち＝正常終了の待ちはしない」だろうが、関数名が `kill_and_wait` で本文が `wait()` を呼んでいる以上、この綴りは事実と逆に読める。実装は #15 の射程なので変えなくてよい。
  - 提案: doc を「保持プロセスを畳む。正常終了できるかは測っていないので、その正常終了は待たずに `kill` してから終了を回収する（回収自体に期限は無い — `.adr/073` / Issue #15）」の趣旨に直す。1行で足り、`.adr/073:47` と同じ条件が2箇所で同じ形になる。

## 観点ごとの所見（指摘に至らなかったもの）

**2周目の修正が開けた穴は無い。** (1) `common/mod.rs:46-54` に入った導線は基準（宣言だけで次の一手が定まるものだけを許容する）と `.adr/073` の参照だけで、`conformance_lock.rs` 側の doc を複製していない — 述語を2本のまま残した意図（変種が増えたとき両方の宣言側でコンパイラに判断を迫る）に、迫られた側で判断できる材料が付いた形になっている。(2) `conformance_lock.rs:67-69` の `kill_holder` コメントは実物と一致する — `Available` の環境では `allowed_skips()` が空集合で、`SignalTimedOut` の環境では `hold_from_other_process` が手前で `None` を返すため `kill_holder` に到達しない。したがって「静かなスキップにはならない」は両方の環境で成り立つ。(3) `use` の統一は綴りだけで、`HolderProgram` のタプルフィールドと `path()` は private のままなので、`Available` のペイロードは `lock.rs` の外へ出ない（AC-2 の「コンパイラが保証する」は保たれている）。(4) `.adr/073` に足した3点はいずれも許容集合を広げる向きに働かない。

**HOOKS.md の 017 の読み替え先（2周目 W-032 の手当て）。** `:58` の追記は実物と一致する — `crates/pulsen/tests/cli_add_error.rs:130` が `common::skipped("tc_task_register_task_017", "lock::hold")` を呼び、`SkipBudget::record` が `SKIP tc_task_register_task_017: ハーネスが lock::hold を提供しないため…` を出すので、`:47` の読み替え規則（判定列の括弧で読む）へ辿れる。ラベル `lock::hold` を HOOKS.md に持ち込んでいないので、適用側の名前を正本の表に入れない縛り（AC-8 / `.adr/073:53`）も崩れていない。ただしこの1文は「3ランナーでの実測」節にあり、同節は `:63` が「PR #11 がスイートと example を足した時点で部分的に古くなる」と自ら宣言している場所なので、次に実測節を書き換える人が落とさないよう注意が要る（2周目 W-006 の提案がこの置き場を指定した経緯があるため、指摘には挙げない）。

**スキップ判定そのものの検算。** `SIGNAL_DEADLINE` は `Duration::from_secs(10)` のまま（差分は doc のみ。AC-7）、`cfg(windows)` の決め打ち無し、`crates/pulsen-conformance/src/` と `crates/pulsen/examples/lock_holder.rs` に差分無し（AC-11 / スコープ）。2つの `allowed_skips()` はワイルドカードなしで同じ4区別を同じ側へ振り分けており（AC-3）、`spawn_holder` が `None` を返す腕は `SignalTimedOut` の1つだけ（AC-6）。`ProgramMissing` / `ProgramUnusable` / `SignalUnreadable` / probe 成立後のタイムアウトはすべてパニックで、それぞれ別の文言を持つ（AC-4 / AC-5）。`testing.md` の逐語期待値（確認項目7・8・9 手順5）は現物のパニック文字列と一致する（行継続 `\` の後続空白が畳まれる形まで確認した）。

**評価順と再入。** `holder_capability()` の `OnceLock::get_or_init` から `skipped` / `record` を呼ぶ経路は無く、`common/mod.rs` の `LazyLock<SkipBudget>` 初期化 → `allowed_skips()` → `holder_capability()` は一方向で環にならない。probe を先に置く形は保たれており、「観測を記録してから宣言へ反映する」実装への後退は無い（plan.md リスク欄）。

## カバレッジ

- 確認: `.adr/073-holder-capability-skip-vs-fail.md`, `.github/workflows/ci.yml`, `.thread/13/adr.md`, `.thread/13/plan.md`, `.thread/13/steps.md`, `.thread/13/testing.md`, `.thread/13/review/triage.md`, `.thread/13/review/fix-plan-002.md`, `.thread/13/review/review-002-test.md`, `crates/pulsen-conformance/HOOKS.md`, `crates/pulsen/tests/common/lock.rs`, `crates/pulsen/tests/common/mod.rs`, `crates/pulsen/tests/conformance_lock.rs`
- スキップ: `.thread/13/review/fix-plan-001.md` — レビューループ自身の成果物（1周目の判定は triage で確認済み）
- スキップ: `.thread/13/review/review-001.md` — 同上
- スキップ: `.thread/13/review/review-001-test.md` — 同上（指摘の帰結は triage と 002 で追跡済み）
- スキップ: `.thread/13/review/review-001-concurrency.md` — 同上
- スキップ: `.thread/13/review/review-001-docs.md` — 同上
- スキップ: `.thread/13/review/review-001-type-design.md` — 同上
- スキップ: `.thread/13/review/review-002.md` — 同上
- スキップ: `.thread/13/review/review-002-concurrency.md` — 同上
- スキップ: `.thread/13/review/review-002-docs.md` — 同上
- スキップ: `.thread/13/review/review-002-type-design.md` — 同上

差分外で判断の材料に読んだもの: `crates/pulsen/tests/cli_add_error.rs`（017 のスキップ経路とラベル）、`crates/pulsen-conformance/src/lib.rs`（`SkipBudget` / `require!` / `conformance_cases!` / `ExclusiveLockHarness`）、`crates/pulsen-conformance/src/exclusive_lock.rs`（4件がスキップに落ちうる口）、`crates/pulsen/tests/common/git.rs`。
