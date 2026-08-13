# レビュー 001 — ドキュメント整合・契約遵守（PR #14 / Issue #13）

対象: `origin/main...HEAD`（`3d8f68c` / `aacc9af`）。契約は `.thread/13/plan.md`。

## ドキュメント整合・契約遵守

### Blockers

- **[B-001]** `.adr/073` が起票されておらず、コードと HOOKS.md が指す唯一の正本 `.adr/068` は、現在の振る舞いと**逆のこと**を明記している
  - 場所: `crates/pulsen/tests/common/lock.rs:21` / `crates/pulsen/tests/conformance_lock.rs:104` / `crates/pulsen-conformance/HOOKS.md:45` → 参照先 `.adr/068-skip-judgement-stays-in-skip-budget.md:43`
  - 理由: 3箇所とも「実行ファイルの不在はスキップにせずケースの失敗にする（HOOKS.md / ADR-068）」と述べ、根拠として 068 を名指しする。ところが `.adr/068:43` の「決定」欄は「単一テストターゲット指定では example がビルドされず、ロック保持のフィクスチャが消えてロック系のケースが**「宣言済みスキップ」に化ける**」と書いており、参照を辿った読み手は反対の記述に着く。`.thread/13/adr.md` ADR-002 自身がこのズレを認識し、「068 は書き換えない代わりに、現行の述語へ辿る導線を後から来た `.adr/073` の側に置く」と決めている。その 073 が存在しないので、導線が片方も無い状態で「068 を見よ」とだけ言っている。`ls .adr/` の最大採番は 072 のままで、`grep -rn "073" crates/pulsen/ crates/pulsen-conformance/` は0件。
  - 提案: steps.md ステップ10 を実施する。(a) `.adr/073-*.md` を `.adr/038` の書式（`## ステータス` / 承認済み）で起票し、能力側と失敗側を分ける基準（スキップの宣言だけで「なぜ走らなかったか」と「次に何をすればよいか」が定まるか）と `ProgramUnusable` を失敗側に置く理由を「決定」に、068 の帰結が「4件＋1件が失敗する」へ改まった旨を「影響」に書く。(b) `lock.rs:21` の `PROGRAM_MISSING` doc と `conformance_lock.rs:104` の `allowed_skips()` doc に 073 を併記する（068 の参照は残す）。(c) `HOOKS.md:45` の括弧も `.adr/068` 単独から 073 併記に改める。

- **[B-002]** `.thread/13/adr.md` の全7エントリの Status が `Proposed` のままで、昇格済みか作業ログ限りかが判別できない
  - 場所: `.thread/13/adr.md:9,44,81,110,140,168,191`
  - 理由: AC-12 が「`.thread/13/adr.md` の各エントリの Status 行から、昇格済みか作業ログ限りかが判別できる」を明示的に求めている。`adr-guide.md` の2層構造（作業ログを片付けフェーズで記録基準にかけ、満たすものだけを `.adr/` へ転記）も、判定結果が残っていなければ機能しない。steps.md ステップ10 手順1 は ADR-001 / 002 / 003 / 005 / 006 を1本に畳んで昇格、ADR-004 / 007 は作業ログ限りと判定済みなので、書き写すだけの作業が未了。
  - 提案: 昇格分は `→ .adr/073-... に昇格`、残りは作業ログ限りである旨と理由（波及テストを満たさない）を Status 行に書く。

### Warnings

- **[W-001]** 正本（HOOKS.md）とコードが、`ProgramUnusable` を失敗側に倒す理由として `.thread/13/adr.md` ADR-002 が明示的に否定した説明を採っている
  - 場所: `crates/pulsen-conformance/HOOKS.md:45` / `crates/pulsen/tests/common/lock.rs:131`
  - 理由: HOOKS.md:45 は「実行ファイルが無い場合と、実行ファイルはあるが起動できない場合は、環境の能力ではなく**フィクスチャの用意の問題**なので」と書き、`lock.rs:131` も「いずれも**環境の能力ではない**ので」と書く。ところが ADR-002（`.thread/13/adr.md:52`）は「`ProgramUnusable`（Defender の隔離・`noexec` マウント・実行形式の不一致）は実行の仕方の誤りではなく、`crates/pulsen/tests/` の中では手の打ちようがない**環境の性質**であり、その点で `SignalTimedOut` と同じ顔をしている」と述べ、だからこそ「ビルド構成の誤りだから失敗」という理由では振り分けられないとして別の基準（スキップの宣言だけで次の一手が定まるか）を立てている。結論（失敗にする）は同じだが、正本が述べる理由が誤っている。`ProgramMissing` だけを指す `lock.rs:20-21` の `PROGRAM_MISSING` doc（「環境の能力ではなくビルド構成の誤り」）は正しい。
  - 提案: 2箇所の理由を基準側の言葉に寄せる。例: 「実行ファイルが無い場合は原因も回避方法も一意なので、起動できない場合は理由が `spawn` の `io::Error` にしか無く次の一手が宣言から定まらないので、いずれもスキップにせずケースの失敗にする」。B-001 の 073 起票と同時に直すのが自然。

- **[W-002]** `SIGNAL_DEADLINE` の doc コメントが、能力の型自身の doc と食い違う述語で probe の判定を説明している
  - 場所: `crates/pulsen/tests/common/lock.rs:14-15`
  - 理由: 「probe ではこの期限を超えたことが**「この環境は保持プロセスを使えない」**という能力の判定になり」と書くが、対応する `HolderCapability::SignalTimedOut`（`lock.rs:30`）の doc は「**起動はできるが**、合図が期限内に返らない（環境の遅さ）」で、probe が観測しているのも起動成功後の期限超過だけ。同ファイル内で同じ状態に2つの述語が付いている。さらに `.thread/13/testing.md:180`（確認項目7 確認ポイント）は「メッセージが『この環境では保持プロセスを使えない』と読める文言になっていないこと」を明示的な確認事項に挙げており、その文言がまさに定数の doc に入っている。
  - 提案: 「probe ではこの期限を超えたことが『この環境は合図を期限内に返せない』という能力の判定になり」に改める。

- **[W-003]** HOOKS.md「3ランナーでの実測」に、その実測が含んでいないものを根拠にした解釈が書き足された
  - 場所: `crates/pulsen-conformance/HOOKS.md:56`（出典は同ファイル:49）
  - 理由: 56行に「後者は `--workspace` で example がビルドされ、**起動した保持プロセスの合図が期限内に返り**、ロックを保持する別プロセスが3 OS で機能したことを意味する」を足したが、49行は「**測定したのは `e524981`（…）で、PR #11 が足す適合スイートと example は含まない**」と明記している。含まれていない測定に対して、合図が期限内に返ったという新しい解釈を重ねる形になり、出典と主張が噛み合わない。plan.md「スコープ / 含まれないもの」は47行・59行の古さを本Issueの対象外としているので、既存の噛み合わなさ自体は本PRの責任ではないが、本PRはそこに主張を1つ**足している**点で新規。
  - 提案: 今回の run（31681471522）でロック系5件が3 OS とも `SKIP` に現れないことは実測できているので、この一文の出典をそちらに寄せるか、47/59行の古さを解消する側に倒すのが筋。少なくとも「合図が期限内に返り」を e524981 の観測として述べる形は避ける。

- **[W-004]** AC-10 の突き合わせ結果がどこにも記録されておらず、PR 本文は「残り」と明記している
  - 場所: PR #14 本文「残り」節 / `.thread/13/`（記録なし）
  - 理由: `.thread/13/testing.md:329`（確認項目15 確認ポイント）は「手順1 の書き出しを PR 本文に残しておく（予測 → 実測 → 突き合わせの順序が守られたことの記録になる）」を求めるが、PR 本文には予測の内訳が無く、代わりに「3 OS の CI 実測と…突き合わせ（AC-10）」が未了として並んでいる。実体としては run 31681471522 の7ジョブが success で、`SKIP` の実測は ubuntu / macOS が `tc_port_clock_005` の1件、Windows がそれに権限系10件を加えた11件（架空3行を除く）、ロック系5件は3 OS とも0件で、steps.md ステップ9 の予測と完全に一致している（本レビューで確認）。つまり「一致しなかった」のではなく「一致したことが記録されていない」。
  - 提案: 突き合わせ結果と出典 run を PR 本文（または `.thread/13/` の記録）に残し、「残り」から外す。HOOKS.md の3 OS 列は予測どおり動かないので変更不要。

- **[W-005]** `.thread/13/testing.md` 確認項目1 手順3 の期待結果が、実際の grep 出力を説明しきれていない
  - 場所: `.thread/13/testing.md:64,69`
  - 理由: 手順3 は `grep -rn "OnceLock\|LazyLock" crates/pulsen/tests/` を指示し、期待結果3 は `lock.rs` の1件と `common/mod.rs` の `SKIPS` だけを挙げる。実際には `common/git.rs:10` / `git.rs:91`（`tmpdir_outside_repository` の `OnceLock`）もヒットして計6行になる。手順どおりに実行した読み手が、説明の付かないヒット2件に当たる。
  - 提案: 期待結果に `common/git.rs` の `OUTSIDE`（別の probe のキャッシュ）を1行足す。

- **[W-006]** `.thread/13/testing.md` 確認項目14 のコマンドが現状では実行できない
  - 場所: `.thread/13/testing.md:291-295`
  - 理由: 手順2〜4 が `.adr/073-*.md` を前提にしており、`grep -n "^## " .adr/073-*.md` は glob が展開されず失敗する。手順5 の `grep -n "^### Status" -A 2 .thread/13/adr.md` は実行できるが、出力は全件 `Proposed` で期待結果を満たさない。B-001 / B-002 の帰結だが、「testing.md に書かれた手順で再現できるか」という観点では確認項目14 が丸ごと未通過であることを記録しておく。
  - 提案: B-001 / B-002 の解消と同時に確認項目14 を実際に通す。

### 受け入れ基準の充足判定

- **AC-1: 充足** — `lock.rs:27-36` に `HolderCapability`（`Available(PathBuf)` / `SignalTimedOut` / `ProgramMissing` / `ProgramUnusable(String)`）が1つだけ、`lock.rs:69-72` に `OnceLock` の probe `holder_capability()` が1つだけ。`Started`（`lock.rs:38-48`）は private で `Child` を運ぶ内部型として分離されており、`'static` に置く能力の型と役割が割れている。
- **AC-2: 充足** — `holder_program()` は `lock.rs:55` で private 化済み。`grep -rn "holder_program" crates/pulsen/` のヒットは `lock.rs:55`（定義）と `lock.rs:75`（`probe_holder` からの1呼び出し）だけ。`holder_capability()` の参照は `lock.rs:69,134` / `conformance_lock.rs:107` / `common/mod.rs:46` の4箇所。`try_acquire_from_other_process`（`conformance_lock.rs:77-81`）は差分ゼロで、`spawn_holder` 経由で同じ1点を見る形が保たれている（`git diff origin/main...HEAD -- crates/pulsen/tests/conformance_lock.rs` に本文の差分なし）。CI の build / clippy が3 OS で緑なので、古い述語を使う呼び出し側が残っていないことをコンパイラが保証している。
- **AC-3: 充足** — `conformance_lock.rs:106-113` と `common/mod.rs:46-51` の双方が `match` で4区別を網羅し、`_` を使っていない。許容集合に足すのは `SignalTimedOut` の腕だけで、`Available(_) | ProgramMissing | ProgramUnusable(_)` は空。件数も4件（`conformance_lock.rs:93-98`）＋1件（`common/mod.rs:32`）で一致。
- **AC-4: 充足** — 不在側は `PROGRAM_MISSING`（`lock.rs:22-24`）が「実行ファイルが無い」「単一のテストターゲットを指定した実行では example がビルドされない」「`cargo test --workspace` のように example をビルドする形で実行する」の3点を述べる。起動失敗側は `lock.rs:140` と `lock.rs:154` がそれぞれ `{reason}`（probe が保持した `io::Error::to_string()`）と `{error}` を埋め込み、`start_holder`（`lock.rs:99-108`）は `spawn` の `Err` も `stdout` 取得失敗も `.ok()?` で捨てずに `Started::SpawnFailed` へ運ぶ。`ProgramUnusable` 側は plan.md の宣言どおり実地検証を持たず、コードレビューで確認した。
- **AC-5: 充足** — `lock.rs:149-152` のパニックが「保持プロセスの合図が {SIGNAL_DEADLINE:?} 以内に返らなかった。probe は同じ手順で成立している（この環境で繰り返し起きるなら SIGNAL_DEADLINE を見直す）」で、能力の問題ではないことを述べている。この経路は `HolderCapability::Available` の腕からしか到達せず、許容集合は空なのでスキップに落ちない。
- **AC-6: 充足** — `spawn_holder` で `None` を作るのは `HolderCapability::SignalTimedOut` の腕（`lock.rs:137`）1箇所のみ。`hold`（`lock.rs:163-171`）は `?` の伝播だけで、`!locked` は `kill_and_wait` + パニック。`hold_from_other_process`（`conformance_lock.rs:62-64`）は `hold` の1行委譲になり、`!locked` の判断が `hold` の中の1箇所に集約された。
- **AC-7: 充足** — `SIGNAL_DEADLINE` は `Duration::from_secs(10)`（`lock.rs:18`）のまま。差分は doc コメントのみ。`grep -rn "AtomicUsize\|from_nanos" crates/pulsen/tests/` は0件、`git status --porcelain` も空。
- **AC-8: 充足**（理由付けは W-001） — (a) `HOOKS.md:43` の「前提を作れない環境」列が「保持プロセスの合図が期限内に返らない（初回起動のスキャン・高負荷）」、「判定」列がフック水準の主語を保ったまま括弧で適用先の実態を補う形。`grep -n "SIGNAL_DEADLINE\|holder_capability\|holder_program\|HolderCapability" crates/pulsen-conformance/HOOKS.md` は**0件**で、適用側固有の名前は判定列にも「前提を作れない環境」列にも漏れていない。3 OS 列は `実行` のまま。(b) ExclusiveLock 節の前書き（`HOOKS.md:207`）が同じ述語に改まっている。(c) `HOOKS.md:45` が表の直後で失敗側を明記。HOOKS.md 全体を読み、`grep -rn "holder_program\|保持させられない\|宣言済みスキップ"` をリポジトリ全体（`.thread/` 除く）に掛けた結果、旧述語を述べる文は HOOKS.md にも他の非 `.thread` ドキュメントにも残っていない（`.adr/068` は plan.md が書き換え対象外としたもの。ただし B-001）。
- **AC-9: 判定不能** — `SIGNAL_DEADLINE` を極小にした実行の記録がリポジトリ内に無く、PR 本文の「確認済み」という主張のみ。コード上は成立する（`SignalTimedOut` → 両 `allowed_skips()` が5件を許容 → `spawn_holder` が起動を試みず `None` → 呼び出し側が `common::skipped` / フックの `None` に落ちる）が、実地の緑を本レビューでは確認していない。
- **AC-10: 充足**（記録は W-004） — run 31681471522 の7ジョブが全て success。ジョブログの `SKIP` 行を採取した実測は、ubuntu / macOS が実在ケース1件（`tc_port_clock_005`）＋架空3行、Windows が11件（`tc_port_clock_005` / `tc_port_config_store_023` / `tc_port_workflow_store_030` / `tc_port_task_repository_005・011・012・019・035・041` / `tc_task_register_task_016・021`）＋架空3行で、steps.md ステップ9 手順1・testing.md 確認項目15 手順1 の予測（unix 1件 / Windows 11件、ロック系5件は0件、架空3行は数えない）と完全に一致する。予測は HOOKS.md の「判定」列から導いた内訳の形で書かれており、架空3行を数えない旨も明記されている。
- **AC-11: 充足** — CI の fmt ジョブが pass、test ジョブ（3 OS）が pass で、`cargo clippy --workspace --all-targets --locked -- -D warnings`（`.github/workflows/ci.yml:229`）は同ジョブ内のステップ。新しい enum の変種名は警告を出していない。
- **AC-12: 未充足** — `.adr/073-*.md` が存在せず（`.adr/` の最大採番は 072）、`grep -rn "073" crates/pulsen/tests/` は0件、`.thread/13/adr.md` の Status は全件 `Proposed`。B-001 / B-002。PR 本文も「残り」として明示している。
- **AC-13: 充足** — `.github/workflows/ci.yml:138-144` が「4件＋1件が失敗する（実行ファイルの不在は環境の能力ではないので、SkipBudget の許容集合に入れない）」に改まり、成立条件（「単一のテストターゲットを指定した実行は example をビルドしないだけで、過去にビルドされた成果物を消しはしない。したがって失敗するのは、target/ にその成果物が残っていない場合である」）が1文添えられ、「--workspace のままにすることがこの経路の構造的な回避である」も残っている。`grep -n "宣言済みスキップ" .github/workflows/ci.yml` はこの箇所からは消え（105行の権限系の記述のみ残存、これは別件で事実のまま）、`grep -n -- "--test " .github/workflows/ci.yml` は**0件**（`.thread/10` ADR-009 の綴り制約を維持）。`run:` の実行内容に差分なし。

### スコープ逸脱の確認

「含まれないもの」の越境は無い。`crates/pulsen-conformance/src/` と `crates/pulsen/examples/lock_holder.rs` と `.adr/068-*.md` はいずれも差分ゼロ、`SIGNAL_DEADLINE` の値は不変、`cfg(windows)` の分岐は追加されていない。他の probe（`permission_restrictions_effective` / `tmpdir_outside_repository`）とその判定行にも手が入っていない。HOOKS.md の変更は述語の言い換えに関わる3箇所＋追記1文に収まっている（47行・59行の古さは据え置き。ただし W-003）。

### CLAUDE.md「弁明・経緯を残さない」の確認

追加されたコード内コメント（`lock.rs:78,81-82,104,136,167`）はいずれも why / why not の形で、指摘への弁明も修正の経緯も含まない。パニックメッセージも観測に忠実で、`hold` の `!locked`（`lock.rs:168`）は「取得の合図が返らなかった」に留め、`stderr(Stdio::null())` で材料を持たない以上「取得できなかった」と断定していない（`lock.rs:167` の why コメントがその理由を1行で述べる）。

### 参照の実在確認（実際に引いたもの）

- `.adr/055` に `permission_restrictions_effective` の記述あり → `lock.rs:67` の参照は妥当。
- `.adr/060` に「フィクスチャのハングはテストの失敗より診断が難しい」に相当する記述あり → `lock.rs:17` の参照は妥当。
- `.adr/032` は `examples/lock_holder` フィクスチャの ADR → `lock.rs:1` の参照は妥当。
- `.adr/038` の見出しは `## ステータス` / `## コンテキスト` / `## 決定` / `## 影響` → AC-12 が求める書式の基準として実在（`## 検討した代替案` は 038 自身には無いが `.adr/055` 等にある）。
- `.thread/13/adr.md:48` の `conformance_lock.rs:112` / `common/mod.rs:46` は origin/main の実際の行と一致。
- `.thread/13/steps.md` の行番号参照はすべて origin/main と一致（`lock.rs` の `SIGNAL_DEADLINE` 14-16行 / `spawn_holder` 33-56行、`conformance_lock.rs` の 62-69・82-86・97・111-117行、`common/mod.rs` の 31・41-53行、`cli_add_error.rs` 123-140行、`ci.yml` 137-142行、HOOKS.md 43・47・54・59・205行）。
- `crates/pulsen-conformance/src/lib.rs:236-237` は `probe_permission_restrictions` の doc コメント（「フックが掛ける制限と同じ手順で判定するため、判定と実際のスキップが食い違わない」）→ `.thread/13/adr.md:28` の参照は正確。
- `crates/pulsen/tests/common/git.rs:90` は `tmpdir_outside_repository()` の定義行 → steps.md の参照は正確。
- `.thread/10/adr.md:325-354` に ADR-009（Status: Accepted、綴りを残さない決定）が実在。`.thread/10/testing.md:113-118` に `grep -n -- "--test "` を0件で確認する手順が実在。

### カバレッジ

- 確認: `.github/workflows/ci.yml`, `.thread/13/adr.md`, `.thread/13/plan.md`, `.thread/13/steps.md`, `.thread/13/testing.md`, `crates/pulsen-conformance/HOOKS.md`, `crates/pulsen/tests/common/lock.rs`, `crates/pulsen/tests/common/mod.rs`, `crates/pulsen/tests/conformance_lock.rs`
- スキップ: なし
