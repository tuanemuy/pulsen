# レビュー 002 — テスト・ドキュメント整合（PR #12 / Issue #10）

対象: `origin/main`(9d54376) → `HEAD`(2c99c1b)、14ファイル。前回の指摘・台帳の判定は前提にせず、差分・CI ログ・GitHub 上の記録から見直した。

検証に使った実測:

- run 31661056619（`2c99c1b`、全7ジョブ success）と run 31657976822（`af24360`、同）の全ジョブログを取得し、`Running` 行・`test result:` 行・`^SKIP ` 行を数え直した。`gh run view --log` は ESC を `^[` の2文字に落とすため、復元してから ci.yml の awk をそのまま掛けている。
- `util/atomic.rs` / `adapter/task_file.rs` に対する変異テスト6本（下記 W-001 に結果）。すべて実行後にファイルを復元済み（`git diff` は空）。

## テスト・ドキュメント整合

### Blockers

なし。

テストの緩和は差分に無い。`#[ignore]`・テストを外す `cfg(windows)`・アサーションの削除・期待値の弱化はいずれも 0 件で、増えた `#[cfg(windows)]` / `#[cfg(not(windows))]` は分類関数 `transiently_denied` の2版であってテストの除外ではない。`読み手は旧内容か新内容のどちらかだけを観測する` は `if let Ok(...)` から `read_atomic(...).expect("読み手は常に読める")` へ**強められて**おり、ポート契約と同じ強さになっている。`task_file.rs` のフィクスチャ可搬化も検査内容を失っていない（変異テストで確認: `to_vec_pretty` → `to_vec` にすると当該テストを含む3件が落ちる。整形の検査は生きている）。AC-8 の実測記録は run 31657976822 のログと**全数一致**した（テストバイナリ15本 / `test result:` 18行 / Windows の実在スキップ11件 / unix 1件 / `pulsen` lib が Windows 63・unix 66 で、他の14バイナリは3 OS 完全同数）。

### Warnings

- **[W-001]** 「上限内なら置き換わる / 移動する」の2件が、本番の配線（`write_atomic` → `persist_with_retry`、`rename_atomic` → `rename_with_retry`）を一切通っておらず、再試行を本番から丸ごと外しても緑のまま通る
  - 場所: `crates/pulsen/src/util/atomic.rs:297`（`置換が一時的に拒まれても上限内なら置き換わる`）、同 `:404`（`移動が一時的に拒まれても上限内なら移動する`）
  - 理由: どちらのテストも `retry_while_transient` を直接呼び、`persist` / `fs::rename` を呼ぶクロージャを**テスト側で組み直している**。変異テストの結果:
    - `persist_with_retry` の中身を `temp.persist(path)` 1回だけに置き換える（= `write_atomic` が再試行しなくなる）→ **14件すべて緑**
    - `rename_with_retry` の中身を `fs::rename(from, to)` 1回だけに置き換える（= `rename_atomic` が再試行しなくなる）→ **14件すべて緑**
    - 対照として `retry_while_transient` のループ自体を1回で打ち切ると3件が落ちる（ループの検証自体は効いている）

    つまり検証されているのは共通ループであって、公開関数がそのループに繋がっていることではない。`rename_atomic` の再試行は ADR-012 が実測より先に入れた分であり、Windows 側でも裏が無い — TC-port-task-repository-044 は再試行が入る前（`af24360`）の Windows でも緑で、この窓を踏んでいない。結果として `rename_atomic` の再試行は**どのプラットフォームのどのテストからも観測されていない**。名前が仕様の言葉で書かれているぶん、読み手は本番の振る舞いが押さえられていると読む（CLAUDE.md「テストは振る舞いを表す…実装の内部構造に依存させない」に対して、いまの形は内部構造の写しになっている）。
  - あわせて `transiently_denied` 自体を検証するテストが1件も無い。`cfg(not(windows))` 版を `true` に変えても全68件が緑（unix では待ち時間が伸びるだけなので観測できない）。分類は「時間で解けない失敗を再試行で遅らせない」ための唯一の歯止めで、Windows では `ERROR_ACCESS_DENIED`(5) / `ERROR_SHARING_VIOLATION`(32) だけを真とする、という主張がコメントにしか無い。
  - 提案: (a) 阻害要因を別スレッドから数 ms 後に取り除き（再試行の予算は累計 511ms あるので余裕がある）、`persist_with_retry` / `rename_with_retry` を**そのまま呼ぶ**形に直す。分類クロージャを副作用フックに使う形へは戻さない（R1 util W-003）。(b) それを採らないなら、テスト名を「共通ループの検証」であることが分かる名前に改め、本番の配線が unix では原理的に観測できない（分類が常に偽のため）ことを ADR-012 の Consequences に1行残す。(c) `#[cfg(windows)] #[test]` で分類器の真偽（5 / 32 は真、他は偽）を1件足すと、Windows ジョブで実際に走る。

- **[W-002]** 「`SkipBudget` 自己テストの `SKIP` 行は全 OS で3件」が Windows では成立せず、除外件数の合図が初回から偽陽性になる
  - 場所: `crates/pulsen-conformance/HOOKS.md:55`、`.thread/10/steps.md:173`、`.thread/10/testing.md:119`、`.github/workflows/ci.yml:157-160`、`.thread/10/adr.md:176`
  - 理由: run 31661056619 / 31657976822 の Windows ジョブのログに ci.yml の awk をそのまま掛けると、サマリーは **`除外: SkipBudget 自己テスト 2 件`** になる（実在ケースの列挙は期待どおり11件）。ubuntu / macOS は3件。3件目の `tc_port_clock_0051_別のケース` は、libtest の `test tests::並行フックを持たないハーネスでは… ... ` と**同じ行に混線して出力される**ため `^SKIP ` に当たらない（Windows ログ 639行目）。
    - testing.md 確認項目4 の期待結果3 は「除外件数が『3 件』と表示されている（3 から動いていれば実在ケースのスキップを巻き込んでいる可能性がある）」と書いており、Windows では**必ず**この確認が不一致になる。ADR-005 が「件数が 3 から動くことが唯一の合図」と位置づけた歯止めが、初回から鳴りっぱなしの状態で入ることになる。
    - さらに、この混線は実在ケースの `SKIP` 行にも同じように起こりうる経路で、現に1行が行頭一致から漏れている。可視化しか役割を持たない機構（判定は `SkipBudget`）なので緑を偽装する類ではないが、AC-6(b) が「列挙されること」に置いた価値は Windows で目減りしている。
  - 提案: 抽出の照合を行頭一致から行内一致（`line ~ /(^|\.\.\. )SKIP /` 等。`test.log` は cargo の出力しか含まないので誤検出の面は小さい）に変えて混線を拾えるようにするか、それを採らないなら HOOKS.md / steps.md / testing.md / ci.yml の「3件」を実測（unix 3・Windows 2、理由は行の混線）に合わせる。数字を実測に合わせるだけなら、混線で実在ケースが落ちうることを1行添える。

- **[W-003]** マージ候補（`2c99c1b`）が CI を通っている事実が記録に反映されておらず、`.thread/10/progress.md` は「まだ CI を通していない」と**逆のこと**を書いている
  - 場所: `.thread/10/progress.md:3`、同 `:54-59`（「全緑のあとに入った変更（CI 未実行）… 3ランナーでの結果はまだ無い。次の push で取る。」）
  - 理由: run 31661056619（`2c99c1b` = HEAD）は全7ジョブ success で、progress.md が「CI 未実行」と列挙している5点（`--no-fail-fast`・非 root アサートの fail-open 解消・除外件数の表示・MSRV の env 経由・`read_atomic`）はすべてこの run で回っている。実際に確認したところ、この run の Windows でも実在スキップは11件・unix は1件で期待集合と一致していた。**この Issue の主題が「宣言を実測に変える」ことなので、マージ候補が未実測だと読める記録を残したまま閉じるのは主題そのものと噛み合わない。** 同じ理由で、PR #12 本文の確認結果表が `af24360` を出典に据えたまま「その分の CI 結果はこの PR の最新 run を参照」と読み手に委ねている点、HOOKS.md が自ら定めた運用（`:30`「実測に置き換えるのは次に CI を回した人で、そのとき出典の run を下記の節に書き足す」）に反して2本目の run を書き足していない点も同じ食い違いの現れ。
  - 提案: progress.md の当該節を「run 31661056619（`2c99c1b`）で全7ジョブ緑。スキップの実測も期待集合と一致」に置き換える。PR 本文の確認結果表の出典も最新 run に更新する。HOOKS.md の3列は `af24360` の観測のまま残してよい（AC-8 は「#11 マージ前の実測」を求めており、使った commit も明記されている）が、2本目の run で同じ集合が再現したことを1行足すと運用の宣言と揃う。

- **[W-004]** Issue #10 のコメントが「実測対象は `origin/main`（9d54376）時点のコード」と書いているが、その commit の Windows は赤で終わっている
  - 場所: https://github.com/tuanemuy/pulsen/issues/10#issuecomment-5275112376（「実測対象は **`origin/main`（9d54376、Issue #1 の walking skeleton）時点のコード**です」）
  - 理由: 実測は `af24360`（`origin/main` + 本 PR の吸収）で、HOOKS.md `:47` は「測定したのは `af24360`（Issue #10 の CI とその吸収まで適用した時点）」と正しく書いている。`origin/main` 相当のコード（ci.yml だけを足した `09e282c`）に対する run 31656955322 は **failure**（Windows の `-p pulsen --lib` 6件）で、9d54376 のコードで3 OS が緑になった事実は無い。Issue #10 のコメントは「クロスプラットフォーム検証済みと読まれないようにする」ための記録（plan.md スコープの最終項）であり、AC-8 の記録と並ぶ外向きの一次資料なので、どの時点を測ったかがここで食い違うと、後から範囲を判断する人がどちらを信じるかで結論が変わる。
  - 提案: コメントを編集して「実測対象は本 PR 適用後・PR #11 マージ前のコード（`af24360`。`origin/main` に本 PR の Windows 吸収を足した状態）」に直す。sha を残すなら実際に測った sha を書く。

### 指摘に至らなかった点（記録）

- HOOKS.md の実測表・「3ランナーでの実測」節は、run 31657976822 の実ログと数値・行単位で一致した。区分 C 12行の内訳（件数表の C = 12）と表の行も一致し、追記した3列が既存の区分 A / B / C の分類や件数表を動かしていない。
- 「スキップした分を除けば実行された適合ケースの数に OS 差は無い」は、per-binary の突き合わせで裏が取れた（`pulsen` lib 以外の14バイナリは3 OS とも同数）。Windows で3件少ない理由（`#[cfg(all(test, unix))]` の symlink テスト）も同じ節に書かれている。
- ADR-010 の Decision 内に残る「この項目は ADR-012 が置き換えた」は ADR の supersede 記録であり、CLAUDE.md が禁じる「弁明・経緯」（対象はコードとテスト）には当たらない。ADR-012 / ADR-013 は ADR-010 の前提（「未観測」を「赤でない」と読んだこと）を明示して置き換えており、3者の間に矛盾は無い。
- 差分に入ったコードコメントはいずれも why / why not に収まっている。指摘への弁明や作業履歴は残っていない（`task_file.rs` の「綴りは差し込みにして期待値から追い出す」、`atomic.rs` の「打ち切りを表す独自のエラーに差し替えず…」など）。
- ADR-013 の対象外宣言（`config_store` / `workflow_store` には置換の窓が無い）はコードと一致する。`write_atomic` / `rename_atomic` の呼び出し元は `adapter/task_repository.rs` のみで、読み取りも同ファイルの3経路がすべて `read_atomic` に寄っている。
- AC-5 の grep は 0 件、AC-7 の「使わないと決めた道具の綴り」（`sort` / コンテナ指定 / 単一テストターゲット指定）も ci.yml で 0 件。ADR-009 の運用は保たれている。
- PR 本文の変更理由表（4ファイル × 理由 × ADR）は、実際の差分と1対1で対応している。plan.md「レビューで見る観点」が求める「変更理由ごとに、どの基準の下で入った差分かが PR 本文から辿れる」は満たされている。

## カバレッジ

- 確認: `.github/workflows/ci.yml`, `crates/pulsen/src/util/atomic.rs`, `crates/pulsen/src/adapter/task_file.rs`, `crates/pulsen/src/adapter/task_repository.rs`, `crates/pulsen-conformance/HOOKS.md`, `.thread/10/plan.md`, `.thread/10/adr.md`, `.thread/10/progress.md`, `.thread/10/testing.md`, `.thread/10/review/triage.md`, `.thread/10/review/review-001-test-docs.md`
- 確認（本観点の該当箇所のみ精読）: `.thread/10/steps.md` — 設計節（AC-5 の grep・吸収先の層の表）、ステップ3（期待集合と除外の運用）、ステップ12（記録先）。ジョブ定義の逐条は ci.yml 側と突き合わせた範囲にとどめた
- スキップ: `.thread/10/review/review-001-ci.md`, `.thread/10/review/review-001-util.md` — R1 の別観点の記録。指摘タイトルと triage.md の判定だけを突き合わせ、全文は読んでいない（本観点の対象は台帳と、そこから辿れる現在のコード・ドキュメントの状態）

PR #12 本文・Issue #10 のコメント・PR #11 のコメントは、いずれも全文を読んで実測と突き合わせた（W-003 / W-004）。
