# レビュー002 — ドキュメント整合・契約遵守

対象 PR: #14 / ベース: main / HEAD: `b344401`
契約: `.thread/13/plan.md`（AC-1〜AC-13）

## ドキュメント整合・契約遵守

### 1周目 Blocker の解消確認

- **`.adr/073` 未起票（triage: adr-073-未起票 / fix）— 解消。** `.adr/073-holder-capability-skip-vs-fail.md` が実在し、既存最大採番 072 の次。見出しは `## ステータス` / `## コンテキスト` / `## 決定` / `## 検討した代替案` / `## 影響` で `.adr/038` の書式に一致、ステータス語は **承認済み**。`## 決定` に能力側と失敗側を分ける基準（「なぜ走らなかったか」と「次に何をすればよいか」がスキップの宣言だけから定まるか）が置かれ、`ProgramUnusable` を失敗側に置く理由がその基準から導かれている。`## 影響` に `.adr/068` の帰結が改まった旨が1項として立っている。`.adr/068-*.md` は差分ゼロ（書き換えていない）。作業の経緯・レビューの経緯の混入は無く、`.thread/` への参照も無い。昇格しなかった ADR-004 / 007 は「`crates/pulsen/tests/common/lock.rs` に閉じ、覆しても波及先が同ファイルに留まる」＝波及テスト不成立という切り分けで、adr-guide の記録基準に沿う（区別の**理由**だけは 073 の基準として昇格済み、という分け方も妥当）。
- **`.thread/13/adr.md` の Status（triage: adr-status-未更新 / fix）— 解消。** 7エントリすべてに Status 行があり、ADR-001 / 002 / 003 / 005 / 006 は `Accepted → .adr/073-... の「決定」の「…」に昇格`、ADR-004 / 007 は `Accepted(作業ログ限り。昇格しない) — …理由` の形で、昇格済みか作業ログ限りかが判別できる。冒頭の索引文とも一致する。

### 機械的な確認（すべて期待どおり）

- `grep -n -- "--test " .github/workflows/ci.yml` → **0件**。
- `grep -n "SIGNAL_DEADLINE\|holder_capability\|holder_program\|HolderCapability\|lock_holder\|allowed_skips" crates/pulsen-conformance/HOOKS.md` → **0件**（適用側固有の名前が正本に入っていない。「判定」列だけでなく「前提を作れない環境」列にも掛かっている）。
- `grep -rn "OnceLock\|LazyLock" crates/pulsen/tests/` → **6件ちょうど**（testing.md 確認項目1 手順3 の期待どおり。1周目 docs W-005 は解消）。
- `grep -rn "holder_program" crates/pulsen/tests/` → `common/lock.rs` の定義と `probe_holder` からの1呼び出しのみ。`pub` は外れている。
- 参照の実在確認: `crates/pulsen-conformance/src/lib.rs:236-237` は `probe_permission_restrictions` の doc コメント（「フックが掛ける制限と同じ手順で判定するため…」）で一致、`crates/pulsen/tests/common/git.rs:90` は `tmpdir_outside_repository()` の定義行で一致。`.adr/027` / `032` / `055` / `060` / `068` / `071` はいずれも実在し、073 が引用している内容と本文が一致する。SKIP 行の文言（`crates/pulsen-conformance/src/lib.rs:202`）は `ハーネスが {hook} を提供しないため…` で、HOOKS.md に追記した「文言もフック水準になる」の記述と一致する。
- 実地再現: `chmod 000 target/debug/examples/lock_holder` → `cargo test -p pulsen --test conformance_lock` で適合4件が失敗し、メッセージは `ロック保持フィクスチャ(examples/lock_holder)を起動できなかった(probe の起動時に観測した理由): Permission denied (os error 13)`。testing.md 確認項目9 手順5 の期待文言と**逐語で一致**。成果物を退避した状態では `…の実行ファイルが無い。単一のテストターゲットを指定した実行では example がビルドされないため、`cargo test --workspace` のように example をビルドする形で実行する` が出て、確認項目8 手順5 の3点を満たす。いずれも実行後に復元済み（`git status` はクリーン、実行ビットは 755）。
- `cargo fmt --all --check` / `cargo clippy --workspace --all-targets --locked -- -D warnings` / `cargo test --workspace --locked --no-fail-fast -- --nocapture` はいずれも緑。実在の `SKIP` は `tc_port_clock_005` の1件のみで、ロック系5件は0件。
- CLAUDE.md「指摘への弁明や修正の経緯を残さない」に反するコメントは、1周目の修正で入った分を含めてコード側に**無い**。追加されたコメントはいずれも why（起動を試みない理由・読み取りの異常を能力に数えない理由・原因を名指ししない理由）に限られている。

#### Blockers

なし

#### Warnings

- **[W-001]** `.adr/073` が「単一のテストターゲットを指定した実行は5件の失敗になる」を無条件の事実として書いており、同じ PR が ci.yml と PR 本文では明示的に条件付きに直した成立条件（`target/` に example の成果物が残っていないこと）が正本から落ちている。
  - 場所: `.adr/073-holder-capability-skip-vs-fail.md`「影響」2項目め（「その実行の仕方では適合4件と受け入れ1件が**失敗する**」）と最後から2つめのトレードオフ（「単一のテストターゲットを指定した実行が、これまでの緑から5件の失敗に変わる」）。対する記述は `.github/workflows/ci.yml:140-143`（「失敗するのは、target/ にその成果物が残っていない場合である」）と `.thread/13/testing.md` エッジケース4（成果物を消さずに `--test` 指定で回すと**緑**）。
  - 理由: AC-13 が ci.yml にこの成立条件を書かせたのは、無条件の言明が事実と食い違うからである。同じ言明を、コードの doc コメント（`lock.rs:21` / `conformance_lock.rs:105`）が典拠として指す正本の側に無条件のまま残すと、073 だけを読んだ人は「`--test` 指定なら必ず赤になる」と誤って予測する。実際には `target/debug/examples/lock_holder` が残っている限り緑になることを、この PR 自身がエッジケース4で実地に確かめている。`.adr/068` の当該記述は「判断時点の記録」として据え置く方針なので、現行の述語を述べる責任は後から来た 073 の側にある。
  - 提案: 「影響」の当該2箇所に、ci.yml と同じ成立条件を1文添える（例: 「ただし単一ターゲット指定は example をビルドしないだけで既存の成果物を消さないため、失敗するのは `target/` にその成果物が残っていない場合である」）。文言はすでに ci.yml にあるので流用でよい。

- **[W-002]** `.thread/13/testing.md` の期待結果が、1周目の fix を当てたあとの実物と食い違う箇所が4つある。testing.md は「書かれた手順で再現できる」ことが AC の検証手段そのものなので、手順どおり実行すると説明の付かない不一致が出る。
  - 場所 / 内容:
    - `testing.md:276`（確認項目12 手順1 の期待結果）— 「前提を作れない環境」列が「保持プロセスの合図が期限内に返らない**（初回起動のスキャン・高負荷）**」の趣旨になっていること、を期待している。1周目の test W-004（前提列の原因推定 / fix）でこの括弧は落としたので、`HOOKS.md:43` に括弧は無い。
    - `testing.md:280`（同 手順5 の期待結果）— 区分 B の5件が走った理由に「`--workspace` で example がビルドされ」に加えて**「合図が期限内に返った」**ことも含まれている、を期待している。1周目の docs W-003（実測に解釈を足した / fix）でこの追記を落としたので、`HOOKS.md:58` は元のままで「合図が期限内に返った」を含まない。
    - `testing.md:363`（エッジケース1 手順2 の期待結果）— `kill_and_wait` の呼び出し元を「**3経路4箇所**」としているが、続けて列挙されているのは5箇所（`start_holder` の2経路・`probe_holder`・`spawn_holder` の `SignalUnreadable` 腕・`hold` の `!locked` 腕）で、実物も5箇所（`lock.rs:98,120,138,170,190`）。1周目の fix（読み取りエラーを捨てる）で `spawn_holder` に1箇所増えたぶん、列挙だけ更新されて件数が取り残されている。
    - `testing.md:364` と `testing.md:413` — 「`release` を使うのは**正常に保持できたプロセスを畳むときだけ**」「`release` の呼び出しが正常系だけに残っていること」と断じているが、`conformance_lock.rs:79` の `try_acquire_from_other_process` は `locked` の値によらず `release` を呼ぶ。1周目の type-design W-006（fix）は「実装は妥当なので `.adr/073` 側で射程を明示して文言と実装を一致させる」と決着しており、073 は実際に射程を probe と `lock.rs` の失敗経路の2つに限定した。その決着が testing.md（および `steps.md:110`）の断定形に反映されていない。
  - 理由: 4件とも1周目の fix の副作用で、記述が事実より古い。とくに前2件は確認項目12（AC-8 の検証手段）を手順どおり実行すると不一致で止まる。
  - 提案: 276 は括弧を削る。280 は「`--workspace` で example がビルドされ、保持プロセスが3 OS で機能した」に合わせる（HOOKS.md 側は据え置き）。363 は「4経路5箇所」に直す。364 / 413 と `steps.md:110` は「`release` を使うのは `lock.rs` の中では正常に保持できたプロセスを畳むときだけ（適合ハーネスの `try_acquire_from_other_process` は `.adr/073` が射程外と明示した経路）」の形にする。

- **[W-003]** 読み取りスレッドが結果を返さずに消えた経路（`RecvTimeoutError::Disconnected`）を `Started::SignalUnreadable` に寄せているが、その変種の doc と `.adr/073` の probe 判定基準は、いずれも「応答／合図が期限内に返った」ことを前提に書かれていて、この経路を含んでいない。
  - 場所: `crates/pulsen/tests/common/lock.rs:55`（`/// 期限内に応答はあったが、合図を読み取れなかった。`）、同 `:141`（`// 期限を1ミリ秒も測っていないので、…`）、`.adr/073` 「決定」の「**probe の判定基準は「合図が期限内に返ったか」だけに限る。**…返ってきた合図を読み取れたかも、判定には入れない」。
  - 理由: `Disconnected` が返るのは読み取りスレッドが `send` に到達せず消えたときで、子からの応答は**無い**。それでも `SignalUnreadable` に載り、probe ではこの変種が `Available` に数えられる。doc は「応答はあった」、ADR は「返ってきた合図を読み取れたか」と、いずれも合図が返ってきたことを前提にした述語で書かれているので、この経路を読み手が辿れない。また `:141` の「期限を1ミリ秒も測っていない」は、読み取りスレッドが期限の途中で消えた場合には成り立たない（`recv_timeout` はその時点で `Disconnected` を返す）ので、検証していないことを断定している。挙動そのもの（最終的にケース側の失敗になる）は妥当で、直すのは文言。
  - 提案: `SignalUnreadable` の doc を「合図を読み取れなかった（子が読めない出力を返した場合と、読み取りが結果を返さずに終わった場合）」の形に広げる。`:141` は「期限の超過を観測していないので、環境の能力（＝スキップを許容する側）には倒さない」に改める。073 は「合図が期限内に返ったか**だけ**を見る（読み取りに失敗した場合も、その失敗の理由は判定に入れずケース側の失敗として現れる）」の趣旨に1語足せば射程が合う。

- **[W-004]** AC-10 の突き合わせが、HEAD ではなく親コミットの CI run を出典にしている。
  - 場所: `.thread/13/steps.md` ステップ9 手順4・5 と PR 本文「3 OS CI の実測との突き合わせ」（いずれも `run 31681471522`）。
  - 理由: run 31681471522 の headSha は `aacc9af`（HEAD `b344401` の親）で、その後の「fix: レビュー1周目の指摘を反映する」が `lock.rs` の分類そのもの（`Disconnected` の振り分け・`Available` のペイロード・失敗経路の後始末）を書き換えている。AC-10 が検証したい対象はまさにその述語なので、出典が1コミット古いと「宣言と実態が割れていないことを、マージする木で確かめた」という記録にならない。なお事実としては成立している — HEAD に対する run 31683976608 は7ジョブすべて success で、ジョブログの `SKIP` は ubuntu / macOS が `tc_port_clock_005` の1件（＋架空3行）、Windows が権限系10件を加えた11件、ロック系5件は3 OS とも0件で、手順1 の予測と一致する（本レビューで確認済み）。
  - 提案: steps.md 手順4・5 と PR 本文の run 番号を 31683976608 に差し替える（内訳は同じ）。予測（手順1）は commit `3d8f68c` で先に書かれており、予測 → 実測の順序は保たれているので、順序の記録は崩れない。

- **[W-005]** `HOOKS.md` に追記した「適用先で実際に成立しなかった条件は、この表の「判定」列の括弧で読む」が、表全体に掛かる読み方として書かれているが、括弧でこの補足を持つ行は TC-port-exclusive-lock-002 / 003 / 004 / 005 の1行だけ。
  - 場所: `crates/pulsen-conformance/HOOKS.md:47`。
  - 理由: 他の行（`permission_restrictions_effective` の各行、`rewind`、`unusable_lock` など）の判定列に同じ性質の括弧は無く、読み手がその行で括弧を探すと何も見つからない。断定の射程が実物より広い。
  - 提案: 「適用先で実際に成立しなかった条件を判定列の括弧に持つ行は、その括弧で読む」程度に射程を絞るか、この文を該当行の直下に置く。

### 受け入れ基準の充足判定

- **AC-1: 充足** — `crates/pulsen/tests/common/lock.rs:27-36` に `HolderCapability`（`Available(HolderProgram)` / `SignalTimedOut` / `ProgramMissing` / `ProgramUnusable(io::Error)` の4変種ちょうど）、`:82-85` に `OnceLock` で1度だけ評価する `holder_capability()`。`crates/pulsen/tests/` 全体で `OnceLock`/`LazyLock` は6行で、ロック能力の `static` はこの1件だけ。`HolderProgram` はフィールドも `path()` も非公開で、パスが `lock.rs` の外へ出ない。
- **AC-2: 充足** — `holder_program()` から `pub` が外れ、`grep -rn "holder_program" crates/pulsen/tests/` は `lock.rs` 内の2件のみ。`spawn_holder` / `hold` と2つの `allowed_skips()` はいずれも `holder_capability()` の結果だけを見る。`try_acquire_from_other_process` は本文無変更（`spawn_holder` 経由で同じ1点を見る）で、`cargo build --workspace --locked` / clippy が通ることで private 化に取り残された呼び出し側が無いことも確認。
- **AC-3: 充足** — `conformance_lock.rs:107-113` と `common/mod.rs:46-51` の `match` はどちらもワイルドカードを使わず4変種を網羅し、`SignalTimedOut` の腕だけが `LOCK_HOLDER_CASES`（4件 / 1件）を許容集合へ入れる。
- **AC-4: 充足** — 実地で両経路を踏んで確認。`ProgramMissing` は不在と回避方法（`cargo test --workspace` で example をビルドする形）を、`ProgramUnusable` は `spawn` が返した `Permission denied (os error 13)` を、それぞれ失敗メッセージに載せる。いずれも `SKIP` にはならない。
- **AC-5: 充足** — `lock.rs:173-176` のパニック文言に「保持プロセスの合図が … 以内に返らなかった」と「probe は同じ手順で成立している」が両方入り、繰り返すなら `SIGNAL_DEADLINE` を見直す旨も読める。この経路の実地再現（`start_holder` への一時的な差し込み）は PR 本文の記録による。
- **AC-6: 充足** — `spawn_holder` が `None` を作るのは `HolderCapability::SignalTimedOut` の腕1箇所だけ。`hold` は `?` の伝播のみで、`!locked` は `kill_and_wait` + パニック。`hold_from_other_process` は `common::lock::hold` の1行に委ねられ、`!locked` の判断が1箇所に集まっている。読み取り失敗も起動失敗も `None` に落ちない。
- **AC-7: 充足** — `SIGNAL_DEADLINE` は `Duration::from_secs(10)` のままで、差分は doc コメントのみ。`grep -rn "AtomicUsize\|from_nanos" crates/pulsen/tests/` は0件。
- **AC-8: 充足** — (a) `HOOKS.md:43` の「前提を作れない環境」列は「保持プロセスの合図が期限内に返らない」、「判定」列はフック水準の主語のまま括弧でこの適用先での実態を補う形。適用側の関数名・定数名は両列とも0件。(b) ExclusiveLock 節の前書き（`:209`）も同じ述語に改まっている。(c) 実行ファイルの不在（と起動不能）がケースの失敗であることが表の直後（`:45`）と節の前書きに明記され、旧述語（不在 → 前提を作れない環境 → スキップ）を述べる文は文書内に残っていない。
- **AC-9: 充足（記録による）** — `SIGNAL_DEADLINE` を極小にした実行で5件が `SKIP` として現れ緑になることは PR 本文と testing.md 確認項目6 の記録による。本レビューでは対照となる `Available` 経路（`cargo test --workspace --locked --no-fail-fast -- --nocapture` が緑・ロック系5件の `SKIP` は0件）と、`ProgramMissing` / `ProgramUnusable` の2経路を HEAD で再現して確認した。
- **AC-10: 充足（ただし W-004）** — 予測は commit `3d8f68c`（PR 作成前）に HOOKS.md の判定列から導いた内訳の形で書かれ、架空3行を数えない旨も明示されている。実測との一致も事実として成立する（HEAD に対する run 31683976608 のジョブログを本レビューで確認: unix 1件 / Windows 11件 / ロック系5件は3 OS とも0件）。記録が指す run が親コミットのものである点だけ W-004。
- **AC-11: 充足** — `cargo fmt --all --check` は差分なし、`cargo clippy --workspace --all-targets --locked -- -D warnings` は警告0。`Started` の変種名は `clippy::enum_variant_names` に掛かっていない。
- **AC-12: 充足** — `.adr/073-holder-capability-skip-vs-fail.md` が `.adr/038` の書式・ステータス **承認済み** で起票済み。振り分けの基準が「決定」の先頭に置かれ、`ProgramUnusable` を失敗側に置く理由がそこから読める。`.adr/068` の帰結が「4件＋1件が失敗する」へ改まったことが「影響」に1項として立ち、068 だけを読んだ人が辿れる。`lock.rs:21`（`PROGRAM_MISSING`）と `conformance_lock.rs:105`（`allowed_skips()`）の doc から 073 を辿れ、068 の参照も残っている。`.thread/13/adr.md` の Status 行から昇格済み / 作業ログ限りが判別できる。
- **AC-13: 充足** — `.github/workflows/ci.yml:137-144` の why コメントが「4件＋1件が失敗する（実行ファイルの不在は環境の能力ではないので、SkipBudget の許容集合に入れない）」に改まり、成立条件（単一ターゲット指定は example をビルドしないだけで成果物を消さない → 失敗するのは `target/` に残っていない場合）と `--workspace` のままにする理由が残っている。`grep -n "宣言済みスキップ" .github/workflows/ci.yml` の残り1件は非 root アサートのコメントで、本 Issue の対象外。`run:` の実行内容は無変更。

### スコープ逸脱

無し。`crates/pulsen-conformance/src/` と `crates/pulsen/examples/lock_holder.rs` は差分ゼロ、`.adr/068-*.md` も差分ゼロ、`SIGNAL_DEADLINE` の値は不変、`cfg(windows)` の分岐は追加されていない。他の probe（`permission_restrictions_effective` / `tmpdir_outside_repository`）とその判定行にも手が入っていない。`HOOKS.md` の変更は述語の言い換えに関わる3箇所＋追記2文に収まり、「3ランナーでの実測」の古い記述（47行・59行の相当箇所）はスコープ宣言どおり据え置かれている。defer した `release` の期限は Issue #15（OPEN）として起票済みで、PR 本文にも記載がある。

### カバレッジ

- 確認: `.adr/073-holder-capability-skip-vs-fail.md`, `.github/workflows/ci.yml`, `.thread/13/adr.md`, `.thread/13/plan.md`, `.thread/13/review/triage.md`, `.thread/13/steps.md`, `.thread/13/testing.md`, `crates/pulsen-conformance/HOOKS.md`, `crates/pulsen/tests/common/lock.rs`, `crates/pulsen/tests/common/mod.rs`, `crates/pulsen/tests/conformance_lock.rs`
- スキップ: `.thread/13/review/fix-plan-001.md` — このレビューループ自身の成果物
- スキップ: `.thread/13/review/review-001.md` — 同上
- スキップ: `.thread/13/review/review-001-concurrency.md` — 同上
- スキップ: `.thread/13/review/review-001-docs.md` — 同上
- スキップ: `.thread/13/review/review-001-test.md` — 同上
- スキップ: `.thread/13/review/review-001-type-design.md` — 同上

（`.thread/13/review/triage.md` はレビューループの成果物だが、既出指摘の判定を知るため確認側に数えた。一覧17件と1対1で対応。）
