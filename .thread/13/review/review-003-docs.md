# レビュー003 — ドキュメント整合・契約遵守

対象: PR #14 / ベース `main` / HEAD `0668dcc`
契約: `.thread/13/plan.md`（AC-1〜AC-13）
既出判定: `.thread/13/review/triage.md`（`wont-fix` / `defer` は再指摘しない）

本レビューで実地に実行したもの: `cargo fmt --all --check`（差分なし）/ `cargo clippy --workspace --all-targets --locked -- -D warnings`（警告0）/ `cargo test --workspace --locked --no-fail-fast -- --nocapture`（全バイナリ緑・実在の `SKIP` は `tc_port_clock_005` の1件のみ・ロック系5件は0件）/ testing.md 確認項目1・2・4・10・12・13・14 とエッジケース1 の各 grep / `.adr/` と `crates/` の参照先の実在確認 / HEAD `0668dcc` に対する CI run 31686303217 の7ジョブと Windows ジョブの `SKIP` 集合。

### ドキュメント整合・契約遵守

#### Blockers

なし

#### Warnings

- **[W-001]** `HolderCapability::Available` の doc だけが、2周目で広げた「`Available` を名乗る根拠」の片方しか述べていない
  - 場所: `crates/pulsen/tests/common/lock.rs:28-29`（`/// 保持プロセスを起動でき、合図が期限内に返る。`）。対比する記述は同ファイル `:97-100`（probe のコメント「合図を観測しないまま Available に数える」）と `.adr/073:39`（「`Available` を名乗る根拠は経路によって『合図が期限内に返った』と『期限の超過を観測しなかった』の2つに分かれる」）
  - 理由: `probe_holder` は `Started::Signaled` と `Started::SignalUnreadable` を同じ腕で受け、後者（子が読めない出力を返した場合と、`RecvTimeoutError::Disconnected` で読み取りが結果を返さずに終わった場合）でも `Available` を返す。2周目の W-025 の手当ては `Started::SignalUnreadable` の doc・probe のコメント・`.adr/073` を広げたが、能力の型の変種 doc だけが元の狭い述語のまま残っている。結果として同じ変種に、同一ファイル内の3行離れた位置で相反する述語が付いている（1周目 docs W-002「同一ファイル内で同じ状態に2つの述語が付く」と同型で、そのときは fix 判定）。`Available` を「合図が返った証拠」と読んだ人は、`spawn_holder` の probe 成立後タイムアウトのメッセージ（「probe は同じ手順で成立している」）を、実際には合図を観測していない probe に対しても成り立つ主張として読んでしまう
  - 提案: 変種 doc を「保持プロセスを起動でき、期限の超過を観測しなかった（合図が期限内に返った場合と、合図を観測しないまま読み取りが終わった場合）」の趣旨に広げる。ADR とコメントは既にこの形なので、揃えるのは1行

- **[W-002]** `.adr/073` の「それ以外はすべて能力ありとして扱い」に射程が無く、同じ「決定」の4区別の振り分けと逆のことを述べる形になっている
  - 場所: `.adr/073-holder-capability-skip-vs-fail.md:39`（2周目で追加された文）。矛盾する相手は同ファイル `:22-27` の振り分け
  - 理由: この1文の中で「能力側へ倒す」＝`SignalTimedOut`（スキップ許容側）、「能力あり」＝`Available` と、同じ語幹が2つの意味で使われている。そのうえで「能力側へ倒すのは期限の超過を実際に観測したときだけで、**それ以外はすべて能力ありとして扱い**」と無条件に書かれているため、文字どおりには `ProgramMissing` / `ProgramUnusable` も `Available` に数えることになる。実装は `probe_holder` で `holder_program()` が `None` なら `ProgramMissing`、`Started::SpawnFailed` なら `ProgramUnusable` と分けており（`lock.rs:90-108`）、一致しない。恒久の正本で、しかも `.adr/068` の帰結を改める側の文書なので、後からこの経路を触る人が振り分けの根拠として引く可能性がある
  - 提案: 射程を「起動して合図の経路まで進んだ子についての観測」に限る（例: 「起動できた子については、能力側へ倒すのは期限の超過を実際に観測したときだけで、それ以外は能力ありとして扱い」）。振り分け全体を述べるのは `:22-27` の役目なので、ここは probe の判定基準の話に閉じればよい

- **[W-003]** `testing.md` 確認項目13 手順1 の期待結果が実物と合わず、手順どおり実行すると AC-13 が未達に見える
  - 場所: `.thread/13/testing.md:288`（手順1）と `:293`（期待結果1「**ヒット0件。**」）。実物は `.github/workflows/ci.yml:105`
  - 理由: 実際に `grep -n "宣言済みスキップ" .github/workflows/ci.yml` を回すと1件ヒットする（`# root で走ると chmod が効かず、権限系10件が「宣言済みスキップ」として静かに緑になる。` — 非 root アサートの why コメントで、本Issueの対象外）。テストステップ側の該当文は正しく「失敗する」に改まっているので事実としては AC-13 を満たすが、手順書は0件を期待しているため、この確認を実行した人は「改め漏れがある」と誤判断して止まる。1周目 docs W-005（`grep の期待結果不足` / fix）と同じ欠落で、対象の grep が違うだけ
  - 提案: 期待結果1 に「ヒットは `ci.yml:105` の非 root アサートについての1件だけで、テストステップの why コメント塊には残っていない」を足す

- **[W-004]** `testing.md` 確認項目2 手順3 の期待結果が、2周目の `use` 整理に追随していない
  - 場所: `.thread/13/testing.md:80`（手順3）と `:87`（期待結果3「…の3箇所」）
  - 理由: 実際に `grep -rn "holder_capability" crates/pulsen/tests/` を回すと5行出る（`conformance_lock.rs:9` の `use` 行・`conformance_lock.rs:111`・`common/lock.rs:85`・`common/lock.rs:162`・`common/mod.rs:49`）。期待結果は `use` 行に触れておらず、2周目の W-036（`use` とフルパスの混在 / fix）で `conformance_lock.rs` がフルパスから `use` へ寄せたぶんが手順書に反映されていない。W-003 と同じく、手順どおり実行した人が説明の付かないヒットに当たる
  - 提案: 期待結果3 に `use` 行を1つ足し、「定義1・`use` 1・呼び出し3」の内訳にする

- **[W-005]** AC-10 の出典が HEAD ではなく親コミット `b344401` の run のままになっている
  - 場所: `.thread/13/steps.md:381`・`.thread/13/testing.md:338`・PR 本文「3 OS CI の実測との突き合わせ」（いずれも run 31683976608 / `b344401`）
  - 理由: HEAD は `0668dcc` で、`b344401` との差は run の対象に入っていない。2周目 W-026 が「出典が1コミット古いと『宣言と実態が割れていないことを、マージする木で確かめた』という記録にならない」として `aacc9af` → `b344401` に差し替えさせた判断が、同じ形で1つ後ろにずれている。**ただし今回は substance が伴わない** — `0668dcc` の `crates/` 側の差分は doc コメント・`use` 文・コメント追加だけで振る舞いを変えておらず、本レビューで HEAD `0668dcc` に対する run 31686303217 を確認したところ7ジョブすべて success、Windows ジョブの実在の `SKIP` は `tc_port_clock_005` ＋権限系10件の計11件、ロック系5件は0件で、`steps.md:372-378` の予測と一致している。したがって直しは参照の1行差し替えで足りる
  - 提案: `steps.md` 手順4・5 / `testing.md` 確認項目15 実測 / PR 本文の出典を run 31686303217（コミット `0668dcc`）へ更新する。内訳は変わらない

- **[W-006]** `.thread/13/steps.md` の一部が、2周目の修正後のコードと食い違ったまま残っている（低）
  - 場所: `.thread/13/steps.md:276`（`use` を `common::lock::{HolderCapability, release, spawn_holder}` に整理する）と `:41,45,71,75`（ステップ1 のコード片に置いた変種 doc）
  - 理由: 実物の `use` は `{HolderCapability, hold, holder_capability, release, spawn_holder}`（2周目 W-036）で、変種 doc も `SignalTimedOut` / `ProgramUnusable` / `SignalUnreadable` / `SpawnFailed` の4つが2周目に広げられている。`steps.md:86` の `(HOOKS.md / ADR-068)` はステップ10-4 で 073 を足す設計なので意図どおりだが、上の2点は追随漏れ。手順を再実行すると `use` 行がコンパイルエラーになる程度の実害
  - 提案: `:276` の `use` 一覧を実物に合わせる。コード片の doc は現行の文面へ寄せるか、「最終形は `lock.rs` を見る」と1文添える

#### 受け入れ基準の充足判定

- **AC-1: 充足** — `crates/pulsen/tests/common/lock.rs:27-38` に `HolderCapability`（`Available(HolderProgram)` / `SignalTimedOut` / `ProgramMissing` / `ProgramUnusable(io::Error)` の4変種ちょうど）、`:85-88` に `OnceLock` で1度だけ評価する `holder_capability()`。`crates/pulsen/tests/` 全体の `OnceLock`/`LazyLock` は6行で、ロック能力の `static` はこの1件だけ（他は `git.rs` の `OUTSIDE` と `mod.rs` の `SKIPS`）。`HolderProgram` はフィールドも `path()` も非公開で、パスが `lock.rs` の外へ出ない。ただし `Available` の doc は W-001。
- **AC-2: 充足** — `holder_program()` から `pub` が外れ、`grep -rn "holder_program" crates/pulsen/tests/` は定義と `probe_holder` からの1呼び出しのみ。`spawn_holder`（`:162`）と2つの `allowed_skips()`（`conformance_lock.rs:111` / `common/mod.rs:49`）はいずれも `holder_capability()` だけを見る。`try_acquire_from_other_process` は差分ゼロ（`git diff origin/main...HEAD` で本文に変更なし）で、`spawn_holder` 経由で同じ1点を見る。`cargo build` / clippy が通ることで private 化に取り残された呼び出し側が無いことも確認。
- **AC-3: 充足** — `conformance_lock.rs:110-117` と `common/mod.rs:49-54` の `match` はどちらもワイルドカードを使わず4変種を網羅し、`SignalTimedOut` の腕だけが `LOCK_HOLDER_CASES`（4件 / 1件）を許容集合へ入れる。
- **AC-4: 充足** — `PROGRAM_MISSING`（`lock.rs:22-24`）が不在・「単一のテストターゲットを指定した実行では example がビルドされない」・`cargo test --workspace` の3点を述べる。起動失敗は `spawn_holder` の `ProgramUnusable` 腕（`:167-170`）と `Started::SpawnFailed` 腕（`:182-184`）の双方で `{error}` を載せ、前者は「(probe の起動時に観測した理由)」で読み分けられる。実地再現（`chmod 000` の逐語一致）は2周目に済み。
- **AC-5: 充足** — `lock.rs:178-181` のパニック文言に「保持プロセスの合図が … 以内に返らなかった」と「probe は同じ手順で成立している」が両方入り、「この環境で繰り返し起きるなら SIGNAL_DEADLINE を見直す」も読める。`.adr/073:41` の「一度きりの期限超過が異常だと言い切れるわけでもない／繰り返し起きるなら閾値の見直し」とも一致し、2周目 W-030 の掘り崩しは解消している。
- **AC-6: 充足** — `spawn_holder` が `None` を作るのは `HolderCapability::SignalTimedOut` の腕1箇所のみ。`hold` は `?` の伝播だけで、`!locked` は `kill_and_wait` + パニック。`hold_from_other_process` は `hold(&self.lock_path())` の1行に委ねられ、判断が1箇所に集まっている。`RecvTimeoutError::Disconnected` は `SignalUnreadable`（失敗側）へ寄り、`Err(_)` は無い。
- **AC-7: 充足** — `SIGNAL_DEADLINE` は `Duration::from_secs(10)` のままで、`git diff origin/main...HEAD` 上も定数行は context（変更なし）。doc コメントだけが変わっている。`grep -rn "AtomicUsize\|from_nanos" crates/pulsen/tests/` は0件。
- **AC-8: 充足** — (a) `HOOKS.md:43` の「前提を作れない環境」列は「保持プロセスの合図が期限内に返らない」で原因の推定を含まず、「判定」列はフック水準の主語のまま「（この適用先では、保持プロセスを1回起動して合図が期限内に返るかで決まる）」を括弧で補う形。`grep -n "SIGNAL_DEADLINE\|holder_capability\|holder_program\|HolderCapability" crates/pulsen-conformance/HOOKS.md` は0件で、両列とも適用側の名前を主語にしていない。(b) ExclusiveLock 節の前書き（`:209`）も同じ述語に改まっている。(c) 実行ファイル不在・起動失敗が「前提を作れない環境」ではなくケースの失敗であることが表直後の `:45` に明記され、典拠は `.adr/068` と `.adr/073` の併記（1周目 test W-003 の解消）。旧述語を述べる文は文書内に残っていない（`grep -n "実行ファイル"` のヒットは `:45` と `:209` の2件のみで、どちらも新しい向き）。
- **AC-9: 充足（記録による）** — 手順は `testing.md` 確認項目6・`steps.md` ステップ8-2 に残り、1・2周目で実地に踏まれている。本レビューでは `SIGNAL_DEADLINE` を書き換える再実行は行っていない（AC-7 の状態を崩さないため）。対照側の `Available` 経路は本レビューで実行し、緑・ロック系5件の `SKIP` 0件を確認した。
- **AC-10: 充足（ただし W-005）** — 予測は commit `3d8f68c`（PR 作成前）に HOOKS.md の判定列から1行ずつ導いた内訳の形で `steps.md:372-378` / `testing.md:322-328` に書かれ、`pulsen-conformance` の架空3行を数えない旨も明示されている。突き合わせの結果も記録されている。事実としても成立する — HEAD `0668dcc` の run 31686303217 は7ジョブ success で、Windows の実在 `SKIP` は11件、ロック系5件は0件（本レビューで確認）。記録が指す run が親コミットのものである点だけ W-005。
- **AC-11: 充足** — 本レビューで `cargo fmt --all --check`（差分なし）と `cargo clippy --workspace --all-targets --locked -- -D warnings`（警告0）を実行。`Started` の変種名は `clippy::enum_variant_names` に掛かっていない。
- **AC-12: 充足（ただし W-002）** — `.adr/073-holder-capability-skip-vs-fail.md` は `## ステータス` / `## コンテキスト` / `## 決定` / `## 検討した代替案` / `## 影響`（`.adr/038` の書式）・ステータス **承認済み** で起票済み。振り分けの基準が「決定」の先頭（`:22`）に置かれ、`ProgramUnusable` を失敗側に置く理由（`:26`）がそこから読める。`.adr/068` の帰結が「4件＋1件が失敗する」へ改まったことは「影響」`:68` に1項として立ち、成立条件（`target/` に成果物が残っていないこと）も入って ci.yml と揃った（2周目 W-029 の解消）。`.adr/068` 自体は差分ゼロ。`lock.rs:21`（`PROGRAM_MISSING`）と `conformance_lock.rs:108`（`allowed_skips()`）の doc から 073 を辿れ、`common/mod.rs:47` にも導線が入った（2周目 W-034 の解消）。`.thread/13/adr.md` の7エントリすべての Status 行から昇格済み／作業ログ限りが判別できる。
- **AC-13: 充足** — `.github/workflows/ci.yml:137-144` が「4件＋1件が失敗する（実行ファイルの不在は環境の能力ではないので、SkipBudget の許容集合に入れない）」に改まり、成立条件（単一ターゲット指定は example をビルドしないだけで既存の成果物を消さない → 失敗するのは `target/` に残っていない場合）と `--workspace` のままにする理由が両方残っている。`grep -n -- "--test " .github/workflows/ci.yml` は **0件**（`.thread/10` ADR-009 の綴りを残さない縛りが守られている）。`run:` の実行内容は無変更。

#### スコープ逸脱

無し。`crates/pulsen-conformance/src/` と `crates/pulsen/examples/lock_holder.rs` と `.adr/068-*.md` は差分ゼロ。`SIGNAL_DEADLINE` の値は不変。`grep -rn "cfg(windows)" crates/pulsen/tests/` は0件。他の probe（`permission_restrictions_effective` / `tmpdir_outside_repository`）とその判定行にも手が入っていない。`HOOKS.md` の変更は述語の言い換え3箇所＋追記2文に収まり、「3ランナーでの実測」の古い記述（`:51` の `e524981`、`:63` の「#11 の責務」）は plan.md のスコープ宣言どおり据え置かれている。

#### CLAUDE.md「指摘への弁明や修正の経緯を残さない」

違反なし。2周目で足されたコメント（`lock.rs:97-100` / `:146` / `common/mod.rs:45-47` / `conformance_lock.rs:67-69`）はいずれも why に閉じており、レビュー番号・指摘への応答・変更前の姿への言及は含まない。`.adr/073` にも経緯の混入は無い（「2周目で足した」等の記述は無く、すべて現在の形が成り立つ理由として書かれている）。

#### 参照の実在確認

いずれも実在し内容が一致する。`.adr/027` / `032` / `038` / `055` / `060` / `068` / `071`、`crates/pulsen-conformance/src/lib.rs:236-237`（`probe_permission_restrictions` の doc）、`crates/pulsen/tests/common/git.rs:90`（`tmpdir_outside_repository` の定義行）、`steps.md` が名指しする変更前の行番号（`HOOKS.md` 43 / 205 行、`conformance_lock.rs:112`、`common/mod.rs:46` — いずれも `origin/main` 側で一致）。`HOOKS.md:47,58` が引く `SKIP` 行の文言は `crates/pulsen-conformance/src/lib.rs:202` の `ハーネスが {hook} を提供しないため、この環境では前提条件を用意できない` と一致し、`:58` の `ハーネスが lock::hold を提供しないため…` は `cli_add_error.rs:130` の第2引数 `"lock::hold"` と一致する。

#### カバレッジ

- 確認: `.adr/073-holder-capability-skip-vs-fail.md`, `.github/workflows/ci.yml`, `.thread/13/adr.md`, `.thread/13/plan.md`, `.thread/13/review/triage.md`, `.thread/13/steps.md`, `.thread/13/testing.md`, `crates/pulsen-conformance/HOOKS.md`, `crates/pulsen/tests/common/lock.rs`, `crates/pulsen/tests/common/mod.rs`, `crates/pulsen/tests/conformance_lock.rs`
- スキップ: `.thread/13/review/fix-plan-001.md`, `.thread/13/review/fix-plan-002.md`, `.thread/13/review/review-001.md`, `.thread/13/review/review-001-concurrency.md`, `.thread/13/review/review-001-docs.md`, `.thread/13/review/review-001-test.md`, `.thread/13/review/review-001-type-design.md`, `.thread/13/review/review-002.md`, `.thread/13/review/review-002-concurrency.md`, `.thread/13/review/review-002-docs.md`, `.thread/13/review/review-002-test.md`, `.thread/13/review/review-002-type-design.md` — このレビューループ自身の成果物（`review-001-docs.md` / `review-002-docs.md` は既出指摘の射程確認のため該当箇所のみ参照）
