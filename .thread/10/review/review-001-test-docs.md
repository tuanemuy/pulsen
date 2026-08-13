# レビュー 001 — テスト・ドキュメント整合（PR #12 / Issue #10）

対象: `origin/main`(9d54376) → `HEAD`(80e293d)、9ファイル。
検証には CI の実ログ（run 31657976822 / 31658342024）とローカル実行（macOS, GNU Awk 5.4.1）を使った。

## テスト・ドキュメント整合

### Blockers

- **[B-001]** HOOKS.md と progress.md が記録する「テストバイナリ数 19」が実測と一致しない。AC-8 の成果物である実測記録に、測っていない数が実測として載っている
  - 場所: `crates/pulsen-conformance/HOOKS.md:45`、`.thread/10/progress.md:38`
  - 理由: 3ランナーの実ログを取得して数え直した結果、**テストバイナリは 15 個**（`pulsen` lib / `pulsen` bin / 統合テスト11 / `pulsen-conformance` lib / `pulsen-domain` lib）で、`Doc-tests` の3ランナーを足しても `test result:` 行は **18** である（win / mac / ubuntu すべて 18）。19 になるのは **ジョブログ全体に対して `grep -c 'test result:'` を掛けたとき**だけで、19本目は「走らなかったケースを報告する」ステップがエコーした `if [ "$(grep -c '^test result:' test.log || true)" = "0" ]` というスクリプト行である（Windows ジョブログ 853行目）。つまりこの数字は測定対象ではなく自分の測定コードを1件数えている。AC-8 は「3ランナーで実測した成立/不成立が記録されている」ことを求めており、HOOKS.md はクレート内に残る恒久ドキュメントなので、検証できない数字を実測として置くと後続スライスがそれを基準にしてしまう
  - 提案: 「テストバイナリ数は3 OS とも19で同数」を削るか、`テストバイナリは3 OS とも15本（`test result:` 行は Doc-tests を含めて18）で同数` に直す。数字を残すなら、何を数えたのか（`Running` 行か `test result:` 行か）を1語で添える

- **[B-002]** progress.md が「Issue #10 のコメントにも残してある」と書いているが、Issue #10 のコメントは0件。plan.md がスコープに明記した記録先が埋まっていない
  - 場所: `.thread/10/progress.md:44`（`この事実は PR 本文と Issue #10 のコメントにも残してある。`）
  - 理由: `gh api repos/:owner/:repo/issues/10/comments` は `0` を返す。plan.md スコープ「含まれないもの」の最終項（47行目）は「**本 Issue 完了時点では、その中核が未検証のまま残る。**この事実は PR 本文・Issue #10 のコメント・`.thread/10/progress.md` に残す（ステップ12）。#10 のクローズが『クロスプラットフォームは検証済み』と読まれないようにするため」と、記録先を3つ並列で要求している。3つ並べた理由が「Issue のトラッキング上は要件が満たされたように見える」ことの回避なので、Issue 側が欠けるとこの要求が守ろうとしたものがそのまま失われる。加えて、progress.md が「残してある」と完了形で書いているため、レビューで気づかなければ未達のまま閉じる
  - 提案: Issue #10 にコメントを1件付けて「CI が実測したのは Issue #1 時点のコード（`origin/main` + 本 PR の吸収）であり、PR #11 が追加するプロセス同定・デタッチ起動の Windows 挙動は未検証」を残す。実施後に progress.md の記述をそのまま残す（順序を逆にしない）

- **[B-003]** PR #11 へのコメントが0件。progress.md の「steps.md のステップ1〜12 がすべて完了」が事実と違う
  - 場所: `.thread/10/progress.md:3`、`.thread/10/progress.md:46-50`
  - 理由: `gh api repos/:owner/:repo/issues/11/comments` は `0` を返す。steps.md ステップ12（329行目）は「PR #11 にコメントを1件残し、次の3件を伝える」を成果物として要求し、plan.md 55行目は「**PR #11 へのコメントとして実際に伝える**。記録先が Issue #10 と PR #10 だけだと、引き受ける側にそれが届く経路が無い」と、なぜコメントでなければならないかまで書いている。progress.md 46行目の「（#11 にコメントで伝える）」は括弧書きの予告のようにも読めるが、3行目が「ステップ1〜12 がすべて完了」と断言しているため、文書全体としては完了と主張している。HOOKS.md 54行目の「その更新は #11 の責務とする」も、#11 側がこの文を読む経路が無ければ宣言のままで終わる
  - 提案: PR #11 にコメントを1件残す（(1) HOOKS.md の実測更新、(2) `task_file.rs` / `util/atomic.rs` とのコンフリクト解消、(3) `ProcessController` が Windows で赤になった場合の対応）。実施できないなら progress.md 3行目を「ステップ12 のうち #11 への引き継ぎコメントが未了」と実態に合わせる

- **[B-004]** PR #12 の本文が CI 実行前の内容のまま更新されておらず、AC-5 の対象外である3ファイルの変更理由が PR 本文から辿れない
  - 場所: PR #12 本文（`.thread/10/adr.md:335-443` の ADR-010/011/012 に対応する記述が無い）
  - 理由: plan.md「レビューで見る観点」（33行目）は「**AC-5 の対象外のファイルに、AC-5 以外の理由が付いているか。** … 変更理由ごとに、どの基準の下で入った差分かが PR 本文から辿れること」を明示のレビュー基準にしている。現在の PR 本文は `ci.yml` の設計だけを述べ、`crates/pulsen/src/util/atomic.rs`（再試行の追加）・`crates/pulsen/src/adapter/task_file.rs`（フィクスチャの可搬化）・`HOOKS.md`（AC-8 の記録）のいずれにも触れていない。さらに「Implementation Plan: 設計判断は `.thread/10/adr.md` の ADR-001〜009」と書いてあるが、プロダクションコードを実際に変えた判断は ADR-010 / 011 / 012 のほうであり、本文の指す範囲が実際の差分をカバーしていない。「既知の見込み」節も「**初回は Windows が赤になる前提**」と未来形のまま残っており、赤が出て吸収済みであることが本文から読めない
  - 提案: PR 本文に「CI 初回実行で Windows が赤になった6件と、その吸収（AC-5）」の節を足し、ADR-010〜012 を参照させる。参照する ADR 範囲を `ADR-001〜012` に直す

### Warnings

- **[W-001]** `util/atomic.rs` に足した打ち切りテストが「上限」を一切観測していない。`MAX_ATTEMPTS` を 1 にしても 1000 にしても、また分類が偽のときに再試行しない性質を壊しても、4件すべてが緑のまま通る
  - 場所: `crates/pulsen/src/util/atomic.rs:244`（`一時的な拒否が続けば上限で打ち切って元のエラーを返す`）、同 `:344`（移動側）
  - 理由: テスト名は「上限で打ち切って」と主張しているが、アサーションは `error.raw_os_error().is_some()` と「一時ファイルが残っていない」の2つだけで、試行が何回行われたかを観測していない。検証されているのは「いつかは Err で返る（無限ループしない）」と「独自エラーに差し替えない」までで、上限の存在は名前が言っているだけになる。あわせて、ADR-010 が分類を設けた最大の理由「時間で解けない失敗を再試行で遅らせない」（`is_transient` が偽なら即座に返す）に対応するテストが1件も無い。この経路は unix の実行では常に通る道（`transiently_denied` は `cfg(not(windows))` で常に偽）でありながら、既存テストのどれも試行回数を見ていないため、`retry_while_transient` の分類分岐が壊れても実測で気づけない。CLAUDE.md「テストは振る舞いを表す。仕様の言葉で名付け」に対し、名前と検証内容がずれている
  - 提案: 分類のクロージャに `Cell<u32>` のカウンタを持たせ、(a) 打ち切り時に `MAX_ATTEMPTS` 回呼ばれたこと、(b) `|_| false` を渡したときは1回で返ること（`一時的でない拒否は再試行せずに返る`）を足す。seam（分類を引数に取る形）は ADR-010/012 が「打ち切りの検証を分類から独立させ、全プラットフォームで走らせる」ために置いたものなので、その seam をカウントに使うのが素直

- **[W-002]** 「`origin/main` 時点のコードに対する測定」という限定が不正確。実際に測ったのは `origin/main` + 本 PR の吸収（`af24360`）であり、`9d54376` の Windows は赤で終わっている
  - 場所: `crates/pulsen-conformance/HOOKS.md:45`、`.thread/10/progress.md:44`（`origin/main（9d54376）時点のコード`）
  - 理由: run 31657976822 の `headSha` は `af24360`（`fix: Windows で落ちた置換とフィクスチャを吸収する` 適用後）。`9d54376` に対する実行は run 31656955322 で `failure`。「PR #11 のスイート・example を含まない」という意図は伝わるが、`9d54376` という sha を名指ししているため、読み手が `9d54376` をチェックアウトして再現しようとすると Windows が赤になる。とくに HOOKS.md はクレート内に残り、#11 以降の担当が「どの時点の測定か」を判断する唯一の出典になる
  - 提案: 「本 PR 適用後・PR #11 マージ前のコード（`af24360`）に対する測定」と書く。sha を残すなら測定に使った sha を書く

- **[W-003]** 「スキップした分を除けば実行された適合ケースの数に OS 差は無い」という記述が、`#[cfg(all(test, unix))]` によって Windows で**存在しない**テスト3件を覆い隠している。この3件は SKIP 行を出さないので、AC-6 が用意した可視化にも一切現れない
  - 場所: `crates/pulsen-conformance/HOOKS.md:45`
  - 理由: 実ログの `pulsen` lib ユニットテストは Windows 63件、unix 66件で、差の3件は `crates/pulsen/src/adapter/task_repository.rs:278` の `#[cfg(all(test, unix))] mod tests`（宙ぶらりんの symlink を使う「消えたエントリ / 読めないエントリ」の確認3件）である。これは本 PR が入れたものではなく既存だが、AC-6 が守ろうとした「スキップが緑に紛れない」の最も強い形（そもそもコンパイルされないので SKIP にすらならない）であり、そこへ「OS 差は無い」と書き足すと、Windows のカバレッジ差が記録の上で消える。文言は「適合ケース」に限定されているので厳密には嘘ではないが、同じ文の前半（テストバイナリ数の同数性）と並ぶことで全体の同等性を主張しているように読める
  - 提案: 1行足す。「`adapter::task_repository` の unix 限定テスト3件（`#[cfg(all(test, unix))]`、symlink を使う）は Windows ではコンパイルされず、SKIP としても現れない」。恒久スキップの `tc_port_clock_005` と同じ扱いで記録しておけば、#11 以降が Windows のカバレッジ差を見落とさない

- **[W-004]** 追記した実測3列が、HOOKS.md が自ら定めた運用「行を足す・フックを足すときはこの表も更新する」と噛み合わない。新しい行を足す人が3列に何を書けるかが決まっていない
  - 場所: `crates/pulsen-conformance/HOOKS.md:3`（運用の宣言）と `:28-41`（追記した列）
  - 理由: 元の表は「前提を作れない環境」「判定」という**設計**だけを持ち、ローカルで行を足せば完結した。追記後は同じ表に**特定の1 run の観測**が同居しており、行を追加する人は CI を回すまで3列を埋められない。空欄にしたときの意味（未観測なのか、走ったのか）も定義が無い。54行目の「PR #11 がスイートと example を足した時点で部分的に古くなる。その更新は #11 の責務」は「既存行が陳腐化する」ケースだけを扱っていて、「行を足すとき」には答えていない
  - 提案: 実測を「3ランナーでの実測」節の側だけに持ち、表には戻さない。表に残すなら、3列の見出しに run 番号を添えたうえで「新しい行は `未測定` と書き、次に CI を回した人が埋める」の1文を運用に足す

- **[W-005]** `.thread/10` の作業ログが、スレッド内連番の ADR を接頭辞なしで参照している箇所があり、同番号の `.adr/` 正本と衝突して曖昧になっている
  - 場所: `.thread/10/progress.md:25`（`（ADR-010）`）、`:27`（`（ADR-012）`）、`:20`/`:54`（`ADR-008`）、`:57`（`ADR-004`）、`.thread/10/plan.md:20`（`ADR-004`）
  - 理由: `.thread/10/adr.md:3` は「採番はこのファイル内の連番。昇格するときは `.adr/065` 以降」と宣言しており、この番号帯（001〜012）は `.adr/004-abort-retry-applicability.md`・`.adr/008-skipped-judgement-outcome.md`・`.adr/010-workflow-cycles-allowed.md`・`.adr/012-tool-op-failure-observed-as-failed.md` と正面衝突する。plan.md は同じ表の中で `adr.md ADR-005` / `adr.md ADR-008` と接頭辞を付けて曖昧さを避けているのに、AC-3 の由来欄（20行目）だけが裸の `ADR-004`、progress.md は全体が裸で書かれている。`.adr/010`（ワークフローの循環を許す）と本書 ADR-010（置換の再試行）はまったく別の話なので、読み手が誤って正本を引きうる
  - 提案: `.thread/10` 内の 001〜012 帯の参照をすべて `adr.md ADR-NNN` に揃える（plan.md が既に採っている書き方）

- **[W-006]** ADR-012 が「`PERSIST_ATTEMPTS` から `MAX_ATTEMPTS` へ改める」と書いているが、`PERSIST_ATTEMPTS` はリポジトリのどこにも存在しない
  - 場所: `.thread/10/adr.md:435`
  - 理由: `PERSIST_ATTEMPTS` を含むのはこの1行だけで、`origin/main` にも HEAD にも定数は無い。ADR-010 の Decision も上限に名前を付けていない。つまりこれはコミットに残っていない作業途中の名前を指しており、読み手は改名前の姿を辿れない。CLAUDE.md「残すのは現在の形が成り立つ理由（why / why not）だけ」の趣旨からも、記録の値は「なぜ置換専用の名前にしないか」であって「何から改名したか」ではない
  - 提案: 「上限の定数は `MAX_ATTEMPTS` とする。置換だけを指す名前が移動にも掛かる状態にしない」に直す

### 指摘に至らなかった点（記録）

- `crates/pulsen/src/adapter/task_file.rs:738` の期待 JSON は、プレースホルダ化しても**整形の検査を1つも失っていない**。差し替わるのは `"repo"` の値1トークンだけで、インデント・キー順・末尾改行・全フィールドの綴りは1通りのリテラルのまま残っている（構造比較への退避も、`cfg` による2通り持ちもしていない）。`encoded_repo()` は `serde_json::to_string(&Path)` で綴りを作るので、本番側が独自の直列化に変わればテストは落ちる。ADR-011 の判断どおり
- `absolute()` は既存の7箇所（`task/path.rs` ほか）と同じ `MAIN_SEPARATOR` 比較の作法。ADR-037 の「`cfg` ではなく std の定数から選ぶ」に沿っている
- 差分に `#[ignore]` も、テストを外す `cfg(windows)` も無い。`util/atomic.rs` に増えた `#[cfg(windows)]` / `#[cfg(not(windows))]` は分類関数 `transiently_denied` の2版であって、テストの除外ではない
- HOOKS.md の実測表（右3列）は**実ログと突き合わせて全行一致**した。Windows の SKIP は `tc_port_config_store_023` / `tc_port_workflow_store_030` / `tc_port_task_repository_005・011・012・019・035・041` / `tc_task_register_task_016・021` / `tc_port_clock_005` の11件、unix は `tc_port_clock_005` の1件。`permission_restrictions_effective` を持つ8行、`unusable_lock` / `failing_manager` / `non_repo_dir` / `hold_from_other_process` の各行が「実行」であることも、コード（`conformance_lock.rs:88` の `unusable_lock` が常に `Some` を返す等）と整合する
- ジョブサマリーの区間除外は**実際に効いている**。`gh run view --log` は ESC を `^[` の2文字に落とすため素の再現では除外が破れるが、ESC を復元して ci.yml の awk を掛け直すと Windows 11行 / unix 1行になり、`SkipBudget` 自己テストの架空3件（`tc_port_clock_004_時刻の前進` / `tc_port_clock_0051_別のケース` / `tc_port_clock_005_時刻の巻き戻し`）は落ちる。HOOKS.md 52行目の記述どおり
- `.thread/10/testing.md:216` の「macOS 458件 PASS が計画時のベースライン」は、HEAD のローカル実測 462件 − 本 PR が足した4件で整合する
- ADR-010 の Decision 内に残る「この項目は ADR-012 が置き換えた」の注記は、ADR の supersede 記録として妥当。CLAUDE.md が禁じる「弁明・経緯」はコードとテストが対象で、ADR には当たらない

### 細かい点（任意）

- `crates/pulsen/src/util/atomic.rs:244` のテスト名 `一時的な拒否が続けば上限で打ち切って元のエラーを返す` だけが主語を欠いている。移動側は `移動の一時的な拒否が続けば…` と主語付きなので、`cargo test` の一覧では置換側か移動側かが名前から判別できない。`置換の一時的な拒否が続けば…` に揃えると対になる

## カバレッジ

- 確認: `crates/pulsen/src/adapter/task_file.rs`, `crates/pulsen/src/util/atomic.rs`, `crates/pulsen-conformance/HOOKS.md`, `.thread/10/plan.md`, `.thread/10/steps.md`, `.thread/10/adr.md`, `.thread/10/progress.md`, `.thread/10/testing.md`, `.github/workflows/ci.yml`
- スキップ: なし

`.github/workflows/ci.yml` は本観点の対象として全文を読んだが、読んだ目的は HOOKS.md の実測記述（測定コマンド・非 root アサート・`container:` 不使用・SKIP 抽出の区間除外）が事実と一致するかの裏取りであり、ワークフロー自体の設計（ジョブ分割・`concurrency`・MSRV 読み出し）の是非は本観点では判断していない。

## 検証に使った手順

```sh
# CI の実測を取得
gh run view 31657976822 --json jobs -q '.jobs[]|"\(.databaseId)\t\(.name)\t\(.conclusion)"'
gh run view --job 94316507436 --log > win.log      # test (windows-latest)

# gh は ESC を "^[" に落とすので復元してから ci.yml の awk を掛ける
sed -n 's/^[^\t]*\tテストする\t[0-9T:.Z-]* //p' win.log | sed 's/\^\[/\x1b/g' > win.esc.log
awk -v esc="$(printf '\033')" '
  { line = $0; gsub(esc "\\[[0-9;]*m", "", line) }
  line ~ /^ *Running / { drop = (line ~ /pulsen_conformance-/) }
  !drop && line ~ /^SKIP / && !seen[line]++ { print "- " line }
' win.esc.log

# テストバイナリ数
grep -c '^test result:' win.esc.log        # => 18（うち Doc-tests 3）
grep -c 'test result:' win.log             # => 19（19本目はエコーされたスクリプト行）

# 記録先の実在確認
gh api repos/:owner/:repo/issues/10/comments -q 'length'   # => 0
gh api repos/:owner/:repo/issues/11/comments -q 'length'   # => 0
```
