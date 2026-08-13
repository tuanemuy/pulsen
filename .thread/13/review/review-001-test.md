# レビュー 001 — Test（テストフィクスチャ設計・スキップ判定の正しさ）

対象: PR #14 / `origin/main...HEAD`（`aacc9af`）
基準: `CLAUDE.md`（テスト方針）、`.thread/13/plan.md`（AC-1〜AC-13・スコープ）、`crates/pulsen-conformance/HOOKS.md`、`.adr/027` `.adr/032` `.adr/055` `.adr/060` `.adr/068` `.adr/071`

## 方法 — 4経路を実地で踏んだ

コードだけで「probe の判定が本番の踏む経路と一致するか」は決まらないので、レビュー用の git worktree（`HEAD` の detached、別 `CARGO_TARGET_DIR`）を作って4経路を実際に踏んだ。作業ツリーには一切触れていない（worktree は撤去済み、`git status` はクリーン）。

| 経路 | 作り方 | 実測 |
|---|---|---|
| `Available` | 素の `cargo test -p pulsen --locked --no-fail-fast -- --nocapture` | 緑。`SKIP` は `tc_port_clock_005` の1件のみで、ロック系5件は出ない |
| `SignalTimedOut` | `SIGNAL_DEADLINE` を `Duration::from_nanos(1)` に一時変更 | **緑**。`tc_port_exclusive_lock_002/003/004/005` と `tc_task_register_task_017` の**ちょうど5件**が `SKIP` 行として出る（AC-9 / AC-3 を満たす） |
| probe 成立後のタイムアウト | `start_holder` に呼び出し回数カウンタを差し込み、2回目以降だけ 1ns | **赤**。5件が失敗し、メッセージは `保持プロセスの合図が 10s 以内に返らなかった。probe は同じ手順で成立している(この環境で繰り返し起きるなら SIGNAL_DEADLINE を見直す)`。`SKIP` には出ない（AC-5 を満たす） |
| `ProgramMissing` | `target/debug/examples/lock_holder` を削除して `--test conformance_lock` / `--test cli_add_error` | **赤**。前者4件・後者1件が失敗し、メッセージに不在と `cargo test --workspace` の案内が載る（AC-4 前半を満たす） |
| `ProgramUnusable` | ビルド済み `lock_holder` を `chmod 000` して `--test conformance_lock` | **赤**。`ロック保持フィクスチャ(examples/lock_holder)を起動できなかった: Permission denied (os error 13)`（AC-4 後半を満たす。plan / steps は「実地では踏まない」としているが unix では踏める — W-006） |

付随して確認したもの。

- `cargo clippy --workspace --all-targets --locked -- -D warnings` は警告0（AC-11。`clippy::enum_variant_names` は `Started` の変種名で発火しない）。
- `.github/workflows/ci.yml` に書き加えられた成立条件「単一のテストターゲットを指定した実行は example をビルドしないだけで、過去にビルドされた成果物を消しはしない」は実測どおり。削除しなければ `--test conformance_lock` は緑のまま通り、削除すれば4件が失敗し、`cargo test -p pulsen`（ターゲット無指定）は成果物を作り直して緑に戻る（AC-13 の記述は事実に一致）。
- スコープ逸脱なし。`crates/pulsen-conformance/src/` と `crates/pulsen/examples/lock_holder.rs` に差分は無く、`SIGNAL_DEADLINE` の**値**も変わっていない（差分は doc コメントのみ。AC-7）。`cfg(windows)` の決め打ちも入っていない。`.adr/068` も書き換わっていない。

判定の要点を先に書く。**スキップ許容集合の導出は、`SignalTimedOut` という1つの観測だけを能力として受け入れ、実行ファイルの不在も起動の失敗も許容集合に入れない。2つの `allowed_skips()` はワイルドカードを使わず同じ4区別を同じ側へ振り分けており、フィクスチャの用意漏れが緑になる経路は塞がっている（実測で確認）。** 以下は残った穴と、主張の裏付けに関する指摘。

## Blockers

なし。

4経路すべてで宣言（`allowed_skips()`）と実態（`SKIP` / 失敗）が一致し、フィクスチャの用意漏れが緑になる経路は見つからなかった。

## Warnings

- **[W-001]** 合図を読み取れなかった理由が捨てられ、パニックが原因を名指しできない
  - 場所: `crates/pulsen/tests/common/lock.rs:112`（`read_line(&mut signal).ok()`）と `crates/pulsen/tests/common/lock.rs:147`（`panic!("保持プロセスの合図を読み取れなかった")`）
  - 理由: `.thread/13/adr.md` ADR-007 は「現行は `spawn` の `io::Error` を `.ok()?` で捨てているため、診断に要る情報がどこにも出ない」を `ProgramUnusable` を分ける理由の一つに挙げ、その経路では実際に `io::Error` を文字列で運んでメッセージに載せている（W-006 の実測どおり機能している）。ところが `SignalUnreadable` の側は同じ捨て方が残ったままで、新しく名前を付けた区別が**名前だけあって原因を持たない**。この経路が踏まれるのは「合図は期限内に返ったが読めなかった」という、まず起きない・起きたら確実に異常な状況で、そこに立ち会った人が手にできる材料が `stderr` は `Stdio::null()`、`io::Error` は破棄、終了コードは `kill_and_wait` が捨てる、で何も残らない。同じ基準（診断に要る情報を捨てない）を1つの関数の中で使い分ける理由が読み取れない。
  - 提案: リーダースレッドの `read` を `Result<String, io::Error>` として送り（`sender.send(read)` の型を変える）、`Started::SignalUnreadable(Child, String)` で理由を運んでパニック文言に載せる。`ProgramUnusable` と同型で、期限の無い待ちも増えない（ADR-007 が `stderr` の `piped` を退けた理由には抵触しない）。

- **[W-002]** probe のコメントが、コードが保証しない帰結を断定している
  - 場所: `crates/pulsen/tests/common/lock.rs:81-82`（`// 合図を読み取れなかった場合も、期限内に返ったことは確かなので能力はある(読めなかったこと自体は最初のケースで失敗として現れる)。`）
  - 理由: 括弧の中は成り立たない。本番のケースは probe とは**別の起動**で `start_holder` をやり直すので、probe で1度 `SignalUnreadable` を観測したことが、最初のケースでも同じ観測になることを何も保証しない（間欠的なら probe の観測はどこにも現れずに消える）。前半（期限内に返った＝能力はある）は正しく、後半だけが検証されていない主張になっている。`CLAUDE.md` は「残すのは現在の形が成り立つ理由」としており、成り立たない帰結でこの分岐を正当化していると、次に触る人がこの腕の安全性を過大評価する。
  - 提案: 「probe が測るのは合図が期限内に返るかだけで、読めたかどうかは能力の判定に入れない（読み取りの異常はケース側で失敗として扱う）」のように、`probe_holder` が実際に決めていることだけを述べる。ADR-005 の「probe は合図が期限内に返ったかだけで決める」がそのまま使える。

- **[W-003]** HOOKS.md が `.adr/068` を、068 が扱っていない区別の典拠にしている
  - 場所: `crates/pulsen-conformance/HOOKS.md:45`
  - 理由: 追記された段落は「実行ファイルが無い場合と、実行ファイルはあるが起動できない場合は…いずれもスキップにせずケースの失敗にする（`.adr/068-*.md`）」と書くが、`.adr/068` が述べているのは前者（`--workspace` を選ぶこと自体が構造的な回避で、許容集合には入れない）だけで、**「起動できない場合」は本Issueが新設した区別**であり 068 のどこにも無い。正本の表の脇で、根拠に当たると別のことが書いてある状態になる。`steps.md` ステップ10 が新番号を足す対象は `lock.rs` と `conformance_lock.rs` の doc コメントだけで、HOOKS.md はその一覧に入っていないため、このまま残る。
  - 提案: ステップ10 で `.adr/073` を起票する際に、この行の典拠も `.adr/068` / `.adr/073` の併記に直す（対象ファイルにも HOOKS.md を足す）。073 を待たずに直すなら、括弧を落として「フィクスチャの用意の問題なのでスキップにしない」だけを述べる形でも整合する。

- **[W-004]** 「前提を作れない環境」列の括弧が、測っていない原因を挙げ、実際の条件より狭く読ませる
  - 場所: `crates/pulsen-conformance/HOOKS.md:43`（`保持プロセスの合図が期限内に返らない（初回起動のスキャン・高負荷）`）
  - 理由: (1) 「初回起動のスキャン・高負荷」は Issue が立てた見立てであって、この PR も過去の実測もそれを測っていない（3ランナーでは一度も `SignalTimedOut` に倒れていない）。正本の表に原因の推定が事実の形で載る。(2) 実際にこの腕へ落ちる条件は「期限内に合図が返らない」全般で、**保持プロセス側が無応答になるフィクスチャのバグも同じ腕に入る**（子が異常終了する場合は EOF → `locked == false` → 失敗として現れるので拾えるが、合図を書かずに生き続ける形は環境の遅さと区別が付かない）。原因を2つ名指しすると、その残余が読み手から見えなくなる。
  - 提案: 括弧を落として「保持プロセスの合図が期限内に返らない」だけにするか、原因を挙げるなら「（環境の遅さ。原因の内訳は測っていない）」のように測っていないことを明示する。HOOKS.md 自身が3 OS 列について「観測値を先に書き写さない」規律を敷いているので、原因欄にも同じ規律が要る。

- **[W-005]** probe が1回きりで、負荷の山に当たった1回が5件の運命を決める
  - 場所: `crates/pulsen/tests/common/lock.rs:69-90`
  - 理由: `.adr/068` / `.thread/13/adr.md` ADR-001 が受け入れ済みのトレードオフだが、本 PR で**許容集合の条件がタイミング依存になった**ことは新しい。これまで許容側に落ちる条件（実行ファイルの不在）は決定的で、同じコード・同じ環境なら常に同じ集合だった。これからは、コードが1文字も変わらなくても、probe の1回が期限を超えた run だけ5件が静かに消えて緑になる。しかも `cli_add_error` 側では probe が権限系・git 系のスキップ（Windows では必ず起きる）に引きずられて起動するため、ロックと無関係なタイミングで測ることもある。歯止めは人が `SKIP` 一覧を読むことだけ（CI 側の歯止めは plan がスコープ外と明記）。
  - 提案: probe だけ2回試み、2回とも期限を超えたときに `SignalTimedOut` とする。偽陽性の確率が二乗に落ち、追加コストは「既に遅いと判定した環境で1プロセス・1期限ぶん」だけで、`Available` の環境（＝CI の常態）には一切乗らない。判定の意味（環境の能力）も変わらない。採らない判断をするなら、「1回きりでよい理由」を ADR-001 のトレードオフ欄より一段強い形（何回試しても同じという根拠）で残したい。

- **[W-006]** `ProgramUnusable` を「実地で踏めない」としているが、unix では安定して踏める
  - 場所: `.thread/13/steps.md:334` / `.thread/13/testing.md:202-216` / `.thread/13/plan.md:65`
  - 理由: 「3 OS で安定して再現する手段が無いため実地では踏まない。コードレビューで確認する（AC-4 のこの半分だけは実地検証を持たない）」としているが、ビルド済みの `lock_holder` を `chmod 000` するだけで unix では確定的に再現でき、本レビューで実際に踏んで `ロック保持フィクスチャ(examples/lock_holder)を起動できなかった: Permission denied (os error 13)` を確認した（所要1分弱、後始末は `chmod 755` のみ）。「3 OS すべてで再現できない」ことは「どこでも検証できない」ことを意味しない。AC-4 の半分を検証なしで残す理由が実際には無い。
  - 提案: `testing.md` の確認項目9 に unix 限定の実地手順（`chmod 000` → `--test conformance_lock` → `chmod 755`）を足し、「Windows では手段が無いのでコードレビューによる」に限定を狭める。`spawn` の `io::Error` が案内に載ることは、コードの読みではなく実物のメッセージで裏付くほうが強い。

- **[W-007]** `SKIP` 行の文言が旧い述語のままで、CI ログだけを読む人には「フィクスチャ未提供」に見える
  - 場所: `crates/pulsen/tests/cli_add_error.rs:130`（`common::skipped("tc_task_register_task_017", "lock::hold")`）と、そこから出る `SKIP tc_task_register_task_017: ハーネスが lock::hold を提供しないため、この環境では前提条件を用意できない`
  - 理由: 本 PR 後、この行が出る条件は「合図が期限内に返らなかった」の1つだけになった。にもかかわらず文言は「ハーネスが `lock::hold` を提供しない」＝フィクスチャ側の未提供を述べており、本 PR がまさに失敗側へ倒したはずの状態（用意漏れ）と同じ顔で出る。AC-10 は「`SKIP` 集合を予測と突き合わせる」ことを検証手段に据えていて、その読み手が最初に見るのがこの文言になる。文面の生成は `pulsen-conformance/src/`（スコープ外）だが、適用側が渡している `"lock::hold"` は適用側の持ち物で、しかも関数名という実装の内部構造で書かれている（`CLAUDE.md`「テストは仕様の言葉で表す」）。
  - 提案: 最小の手当ては、`cli_add_error.rs` が渡すラベルを条件を述べる語（例: `"lock::hold(合図の期限超過)"`）にすること。適合スイート側は `require!` がフック名を出すので揃えられないが、HOOKS.md の「判定」列がフック水準の主語を保つと決めた（ADR-003）以上、ログもフック水準で読ませる約束なのだと HOOKS.md か PR 本文に1文書いておけば、突き合わせる人が誤読しない。ラベルを触らない判断をするなら、後者だけでも要る。

## 観点ごとの所見（指摘に至らなかったもの）

**probe と本番の経路の一致。** `probe_holder` は `start_holder` を本番と同じ関数で呼ぶため、起動・パイプ取得・合図待ちの3段は完全に同じ道を通る。差分は2つだけで、いずれも判定を歪めない。(1) probe のロック置き場は `tempdir()` 直下（親が既存）だが、本番は `home/state/lock` で `ensure_dir` による親の作成が1回増える。ロック機構の異常はケース側で失敗として現れる（TC-007 も別ハンドルで独立に見ている）ので、probe がここを測らないことは静かな緑を作らない。(2) probe は無負荷・単発、本番は並列 — ADR-001 / ADR-006 が射程として自覚しており、W-005 がその残余。

**`locked` を probe が捨てること（ADR-005）。** 誰も保持していないパスで `locked == false` になる環境（ロックが効かないファイルシステム等）でも probe は `Available` を返し、5件は失敗する。これは妥当だと判断した。同じ環境では `TC-port-exclusive-lock-001 / 006 / 007` も落ちるので run が静かに緑になることはなく、能力の軸を「保持プロセスを使えるか」だけに閉じる判断（変異を増やさない）と釣り合っている。ただし `hold` の `!locked` パニックが「競合か機構の異常か」を名指しできないことは ADR-007 が自覚しているとおりで、W-001 と同じ材料不足（`stderr` を捨てている）が根にある。ADR-007 が `stderr` の `piped` を退けた理由（EOF まで期限なしに待つ）は、`!locked` の時点で子が既に終了している（`lock_holder` は取得に失敗したら即 `ExitCode::FAILURE`）ことを踏まえると実際には成立しないが、この経路の希少さから見て、W-001 の `io::Error` を運ぶ手当てが済めば追う価値は薄い。

**評価順・並列度への非依存。** `SkipBudget` は `LazyLock`（最初の `record` 時）で確定するが、宣言側（`allowed_skips()` → `holder_capability()`）も挙動側（`spawn_holder` → `holder_capability()`）も `OnceLock` の同じ1値を見るので、どちらが先に走っても結果は同じ。相互に呼び合う経路が無いことも確認した（`holder_capability` は `skipped` を呼ばない）ので、`LazyLock`(`SKIPS`) → `OnceLock`(`CAPABILITY`) の一方向で、デッドロックの環も無い。`SignalTimedOut` に倒した状態で全体を2回、`--test conformance_lock` 単独で1回回して、結果が同じ（緑・5件 / 4件）ことを実測した。

**後始末。** `release`（stdin を閉じて期限なしに `wait`）が残るのは `conformance_lock.rs::release_holder` と `try_acquire_from_other_process`、`cli_add_error.rs` の正常系だけで、失敗経路はすべて `kill_and_wait` に揃っている。probe が `OnceLock::get_or_init` の中で期限の無い待ちに入らないことは、この分けで担保されている（ADR-005）。全経路の実行後に `lock_holder` のプロセスは残っていない。

**`try_acquire_from_other_process` を `hold` に寄せなかったこと。** 正しい。TC-004 は `acquired` が真であることを判定するケースで、`locked == false` はフィクスチャの異常ではなくケースの観測結果になる。ここを `hold` に揃えると、フィクスチャがケースの判定を先取りする。`hold_from_other_process` だけを `hold` に寄せた形（`conformance_lock.rs:62-64`）は、`!locked` の判断を `lock.rs` の1箇所に集める効果もあって筋が通っている。

**AC-12（`.adr/073`）について。** 現時点で `.adr/073-*.md` は存在せず `.thread/13/adr.md` は全エントリ `Proposed` だが、`steps.md` ステップ10 が「実装・レビュー・3 OS の検証がすべて終わったあとの最後のステップ」と明記しているので、レビュー時点の未達は逸脱ではない。指摘としては挙げず、W-003 で「そのステップの対象ファイルに HOOKS.md が入っていない」ことだけを拾った。

## カバレッジ

- 確認: `crates/pulsen/tests/common/lock.rs`, `crates/pulsen/tests/common/mod.rs`, `crates/pulsen/tests/conformance_lock.rs`, `crates/pulsen-conformance/HOOKS.md`, `.github/workflows/ci.yml`, `.thread/13/plan.md`, `.thread/13/steps.md`, `.thread/13/adr.md`, `.thread/13/testing.md`
- スキップ: なし

差分外で判断の材料に読んだもの: `crates/pulsen/tests/cli_add_error.rs`（tc_017 と他のスキップ経路）、`crates/pulsen/examples/lock_holder.rs`（合図と終了の仕方）、`crates/pulsen/src/adapter/lock.rs`（`ensure_dir` の有無）、`crates/pulsen-conformance/src/lib.rs`（`SkipBudget` / `conformance_cases!` / `permission_restrictions_effective`）、`crates/pulsen-conformance/src/exclusive_lock.rs`（TC-002〜005 のフックの使い方）、`.adr/032` `.adr/055` `.adr/060` `.adr/068` `.adr/071`。
