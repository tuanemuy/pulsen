# 実装計画 — Issue #13: ロック保持フィクスチャの合図タイムアウトが、スキップではなく失敗として現れる

**Issue:** #13
**作成日:** 2026-08-13
**複雑度:** 中〜大規模
**実装方針:** steps.md

---

## 目的

ロック保持フィクスチャの「実行ファイルが無い(＝フィクスチャの作り忘れ。緑にしてはいけない)」と「合図が期限内に返らない(＝環境の能力。スキップでよい)」を型で区別し、`SkipBudget` の許容集合に入るのを後者だけにする。

## 受け入れ基準

| # | 基準(検証可能な形で) | 由来 | 対応ステップ |
|---|---|---|---|
| AC-1 | `crates/pulsen/tests/common/lock.rs` に、`Available` / `SignalTimedOut` / `ProgramMissing` / `ProgramUnusable` を区別する能力の型と、それを1度だけ評価する probe が1つだけ置かれている(Issue が挙げる3区別に「実行ファイルはあるが起動できない」を足したもの。理由は adr.md ADR-007) | Issueコメント「方針」1点目 | 1, 2 |
| AC-2 | `spawn_holder` / `hold` / `try_acquire_from_other_process` と、`conformance_lock.rs` / `common/mod.rs` の `allowed_skips()` が、いずれもこの probe の結果だけを見ている。`holder_program()` が private になっており、直接見る呼び出し側が残っていないことをコンパイラが保証する。このうち `try_acquire_from_other_process` は `spawn_holder` 経由で同じ1点を見るため本文の変更が要らず、**変更不要であることの確認**で満たす(ステップ4) | Issueコメント「方針」2点目 | 2, 3, 4, 5 |
| AC-3 | `TC-port-exclusive-lock-002 / 003 / 004 / 005` と `tc_task_register_task_017` の5件が許容集合に入るのは、probe が `SignalTimedOut` を返したときだけである(`allowed_skips()` はワイルドカードを使わず4つの区別を網羅する) | Issue本文「2 が起きると SkipBudget 違反として失敗する」 | 4, 5 |
| AC-4 | 保持プロセスを用意できない環境では5件がスキップではなく失敗として現れ、失敗メッセージが原因を取り違えない — 実行ファイルが無い場合は不在と回避方法(example をビルドする形＝`--workspace` での実行)を、起動できない場合は `spawn` が返した `io::Error` の内容を、それぞれ載せる | Issueコメント「方針」3点目 | 1, 3, 8 |
| AC-5 | probe が `Available` と判定したあとに合図がタイムアウトした場合は、スキップではなく失敗として現れ、メッセージが「probe は同じ手順で成立している」ことを述べる | Issueコメント「方針」4点目 | 3, 8 |
| AC-6 | `spawn_holder` / `hold` / `hold_from_other_process` の `None` の意味が1つ(合図が期限内に返らない環境)に絞られており、ロックを取得できなかった場合と合図を読み取れなかった場合が `None` に混ざらない | Issue本文「2つを区別できる形にする」 | 3, 4 |
| AC-7 | `SIGNAL_DEADLINE` の値が変わっていない(検証で一時的に書き換えた分が `git diff` に残っていない) | Issue本文「`SIGNAL_DEADLINE` を延ばすだけの対処は取らない」 | 8 |
| AC-8 | `crates/pulsen-conformance/HOOKS.md` が新しい述語で揃っている — (a)「環境で走らなくなりうる行」の TC-port-exclusive-lock-002 / 003 / 004 / 005 の行で、「前提を作れない環境」が合図の期限超過になり、「判定」がフック水準の書き方のまま、この適用先での実態を括弧で補う形になっている(適用先固有の関数名・定数名を主語にしない縛りは、判定列だけでなく「前提を作れない環境」列にも掛ける)、(b) ExclusiveLock 節の前書きも同じ述語に改まっている、(c) 実行ファイルの不在が「前提を作れない環境」ではなくケースの失敗であることが表の近くに明記され、文書内に反対のことを述べる文が残っていない | Issueコメント「あわせて更新する箇所」1点目 | 6 |
| AC-9 | `SIGNAL_DEADLINE` を極小にした状態で `cargo test -p pulsen` が緑になり、5件が `SKIP` 行として現れる(＝タイムアウト側がスキップ許容になっている) | AC-3 の実地検証 | 8 |
| AC-10 | `cargo test --workspace --locked` が3 OS(ubuntu / macOS / Windows)で緑になり、`SKIP` 行の集合が実行前の予測と一致する。予測は HOOKS.md の「判定」列から導いた内訳の形で書かれており、`pulsen-conformance` の lib ユニットテストが出す架空の3行を数えないことが明示されている | `.adr/068`「実行前に予測し、実行後に突き合わせる」 | 9 |
| AC-11 | `cargo fmt --all --check` と、CI と同じ形の `cargo clippy --workspace --all-targets --locked -- -D warnings` が通る(新しく置く型・バリアントの名前が既定の warn 級 lint に掛からないことを含む) | `CLAUDE.md` 技術方針 / `.github/workflows/ci.yml` | 8 |
| AC-12 | `.adr/073-*.md` が `.adr/038` の書式(`## ステータス` / 承認済み)で起票され、「合図タイムアウト＝環境の能力・実行ファイル不在＝失敗」という区別の理由が正本に残っている。4つの区別のどれをどちら側に置くかを決められる基準が書かれており、`ProgramUnusable`(実行ファイルはあるが起動できない)を失敗側に置く理由もそこから読める。`.adr/068` が挙げた帰結(単一テストターゲット指定でロック系が「宣言済みスキップ」に化ける)が失敗側へ改まったことも、068 だけを読んだ人が辿れる形で書かれている。区別の理由の所在として、`lock.rs` の `PROGRAM_MISSING` と `conformance_lock.rs` の `allowed_skips()` の doc コメントから 073 を辿れる。`.thread/13/adr.md` の各エントリの Status 行から、昇格済みか作業ログ限りかが判別できる | Issueコメント「あわせて更新する箇所」3点目 / `.adr/035` `.adr/038` | 10 |
| AC-13 | `.github/workflows/ci.yml` の why コメントが、単一テストターゲット指定の帰結を「4件＋1件が宣言済みスキップに化ける」から「4件＋1件が失敗する(実行ファイルの不在は環境の能力ではないので許容集合に入れない)」へ改めており、成立条件(`target/` に example のビルド成果物が残っていないこと)と `--workspace` のままにする理由が残っている | `.adr/068`「静かな緑を作る条件をワークフローに記録する」 | 7 |

## スコープ

### 含まれないもの

- **`SIGNAL_DEADLINE` の値の変更。** Issue本文が明示的に否定している(閾値を動かしても区別は付かない)。検証で一時的に書き換えたら必ず元に戻す。
- **`cfg(windows)` による決め打ち分岐。** `.adr/071` が却下している。
- **`crates/pulsen-conformance/src/` の変更。** `ExclusiveLockHarness` の trait 定義もフックのシグネチャも `SkipBudget` も変えない。フックが `Option` を返す契約は、別実装(in-memory 等)がスイートを適用できることの前提であり、能力の区別は適用側(`crates/pulsen/tests/`)に閉じる。同クレートの `HOOKS.md`(ドキュメント)は AC-8 の対象で、これだけを書き換える。
- **`HOOKS.md` の「3ランナーでの実測」のうち、判定の言い換えに関わらない記述。** 「測定したのは `e524981` で PR #11 が足す適合スイートと example は含まない」(47行目)と「その更新は #11 の責務とする」(59行目)は現状に対して古いが、本Issueで直す対象ではない。書き換えるのは、5件が走った理由の述べ方だけ。
- **他の probe(`permission_restrictions_effective` / `tmpdir_outside_repository`)や、それらが判定する行の扱い。**
- **`crates/pulsen/examples/lock_holder.rs` の変更。** 保持プロセス側の挙動(合図の書き出し・stdin での保持)は本Issueの対象外。検証のために一時的な `sleep` を入れる形も取らない。
- **`.adr/068` の書き換え。** 同 ADR は既に「実行ファイル不在は許容集合に入れない」としており、直すのは実装側。トレードオフ欄が名指しする `holder_program()` は本Issueの後 private になるが、`.adr/` は判断が下された時点の記録であり、決定そのもの(環境退行に機械的な歯止めが無い)は本Issueの後も真のまま変わらない。現在どの述語が許容集合を決めているかは正本の `HOOKS.md`(AC-8)とコードが持つ。
- **`SkipBudget` の宣言が不当に広げられた場合の CI 側の歯止め**(`.adr/068` が「実際に退行を踏んだ時点で判断する」としたもの)。本Issueは判定の区別を作るところまで。

## リスクと注意点

- **[高] `cargo test --test conformance_lock` / `cargo test --test cli_add_error` の単体実行が、これまでの「緑(スキップ)」から5件の失敗に変わる。** 意図した変更だが、開発者が単一テストターゲットで回す習慣を持っていると初見で驚く。失敗メッセージに回避方法を必ず含める。`.github/workflows/ci.yml` の why コメント(137-142行)が「宣言済みスキップに化ける」と書いており、事実が変わるので追随が要る(AC-13 / ステップ7)。
- **[中] probe が偽陽性で `SignalTimedOut` に倒れると、許容集合が黙って広がる。** 5件が走らなくても緑になる。`.adr/068` が既に記録しているトレードオフ(マージ後の環境退行に機械的な歯止めが無い)の範囲内で、サマリーの `SKIP` 一覧には現れる。probe を1回きりにしている以上、負荷の山に当たった1回が全体を決める点は残る。
- **[中] probe が本番のケースと同じ資源(プロセス起動)を奪う。** コストは1プロセスぶんで、Windows の初回起動スキャンの代金を probe が先に払うことで本番5件は温まった状態で走る(Issueコメントの見立て)。実測で CI の所要時間が目に見えて伸びていないことを確認する。内訳は下記のとおり「2バイナリ×1回」で、うち1回はロックを使わないケースの都合で発生する。
- **[中] probe を走らせるバイナリが増えると、そのぶん保持プロセスが増える。** 現状 `common::skipped` を呼ぶのは `cli_add_error.rs` だけ、`lock::*` を使うのは `conformance_lock.rs` と `cli_add_error.rs` だけなので、probe が実際に走るのは2バイナリ。将来 `common/mod.rs` を使う別のバイナリでスキップが起きると、そこでも probe が走る。
- **[中] `cli_add_error` では、ロックと無関係なスキップが probe を起こす。** `common/mod.rs` の `allowed_skips()` は `SKIPS`(`LazyLock`)の初期化時に一度だけ評価され、その中で `holder_capability()` を呼ぶ。したがって最初に `common::skipped` を通るのが権限系(`tc_task_register_task_016 / 021`)や git 系(`036`)であれば、**ロックを一度も使わないケースのスキップが保持プロセスの起動を引き起こす**。従来この経路は `holder_program()` のファイル存在確認だけで、I/O もプロセス起動も無かった。Windows では権限系が必ずスキップされるため、`cli_add_error` は毎回 probe を走らせる。実害は無い(`ProgramMissing` でも probe はパニックせず能力を返すので、ステップ8-4 の「5件だけが失敗する」も崩れない)が、所要時間を読むときと `tc_task_register_task_017` を含まない絞り込みで回すとき(合図が返らない環境では `SIGNAL_DEADLINE` ぶん待つ)の判断に効く。
- **[中] `SkipBudget` は `LazyLock` で最初の `record` 時に確定する。** probe を先に置く形でこの順序問題を回避しているので、「タイムアウトを記録してから許容集合へ反映する」実装に後退させないこと(`.adr/068` が避けた循環に戻る)。
- **[低] probe が起動した保持プロセスを取り残すと、以降のケースがロックを取れない。** probe は自分が作った一時ディレクトリのロックを使い、判定後は成否によらず `kill` + `wait` する(正常終了を待つ `release` は使わない — 測っていない性質に後始末を依存させないため)。同じ基準を `spawn_holder` / `hold` の失敗経路の後始末にも掛け、`release` を使うのは正常に保持できたプロセスを畳むときだけにする(adr.md ADR-005)。
- **[低] HOOKS.md の3 OS 列。** 判定の述語を変えても該当5件が走った事実は変わらないため列は動かない見込みだが、`.adr/068` の順序(予測 → 実測 → 突き合わせ)を守り、観測値を先に書き写さない。
- **向きの逆の効果として、`SignalTimedOut` 環境での待ち時間は減る。** 現状は4〜5件がそれぞれ `SIGNAL_DEADLINE` を待つが、変更後は probe の1回だけが待ち、残りは起動を試みずに即座に `None` を返す。

## テスト方針

フィクスチャ自身の変更であり、新しいユニットテストの置き場は無い。代わりに各経路を実地で通す — 再現手段のあるものは実際に踏み、無いものは踏まないことと代替の検証方法を明記する。手順の詳細と順序は steps.md ステップ8にある。

- **`Available` 経路:** `cargo test --workspace --locked -- --nocapture` が緑で、ロック系5件に `SKIP` 行が出ない。
- **`SignalTimedOut` 経路:** `SIGNAL_DEADLINE` を一時的に極小(`Duration::from_nanos(1)` 等)にして `cargo test -p pulsen -- --nocapture` を回し、5件が `SKIP` 行として出たうえで**緑**になることを確認する。確認後に必ず元に戻し、差分に残っていないことを `git diff` で確かめる。
- **`ProgramMissing` 経路:** ビルド成果物(`target/debug/examples/lock_holder{,.exe}`)を明示的に削除してから単一テストターゲットを指定して回す。`--test` 指定は example をビルドしないが、既存の成果物を消しもしないため、削除を省くと probe が `Available` に倒れて何も確かめられない。
- **probe 成立後のタイムアウト経路:** `start_holder` の2回目以降の呼び出しだけを一時的に遅らせて再現する(probe は1回目なので `Available` のまま)。確認後に元へ戻す。
- **`ProgramUnusable` 経路(実行ファイルはあるが起動できない):** 3 OS で安定して再現する手段が無いため実地では踏まない。`spawn` の `io::Error` がメッセージに載ることをコードレビューで確認する(AC-4 のこの半分だけは実地検証を持たない)。
- **3 OS の CI:** `.adr/068` の手順に従い、実行前に HOOKS.md から `SKIP` 集合を予測し、実行後の `test.log` / ジョブサマリーと突き合わせる。差分が出たら予測が誤っていた理由を先に特定する。
- **`cargo fmt --all --check` / `cargo clippy --workspace --all-targets --locked -- -D warnings`(CI と同じ形)。** `--all-targets` はテストターゲットも見るため、新しく置く enum の名前が既定の warn 級 lint(`clippy::enum_variant_names` 等)に掛からないこともここで分かる。
