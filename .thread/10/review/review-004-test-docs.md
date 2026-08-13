# レビュー R4 — テスト・ドキュメント整合（PR #12）

対象: `origin/main...HEAD`（HEAD = `2e706df`）全23ファイル。ゼロベースで再検証した。

検証に使った実測: run 31665658371（`e19c973`、全7ジョブ success）の3 OS 分のジョブログを API から取得し、`ci.yml` の awk をそのまま適用してサマリーを復元した。`util/atomic.rs` は隔離した worktree（`git worktree add --detach`、検証後に削除）で6種の変異を当てた。作業ツリーは無変更。

#### Blockers

- **[B-001]** `crates/pulsen-conformance/HOOKS.md:55` — 出典 run の実測と lib ユニットテストの件数が合わない（R2 と同じ Key の再発）

  「`pulsen` の lib ユニットテストが Windows 68件・unix 71件になる差はこれである」と書いてあるが、この節が出典として掲げる run 31665658371（`e19c973`）の実測は **Windows 69件・unix 72件**である。

  ```
  # run 31665658371 / test (windows-latest) のログ
  test result: ok. 69 passed
  # run 31665658371 / test (ubuntu-latest) のログ
  test result: ok. 72 passed
  # ローカル（HEAD、macOS）
  $ cargo test -p pulsen --lib -- --list | grep -c ': test$'
  72
  ```

  ズレの原因は `e19c973` が `util/atomic.rs` にテストを1件（`再試行に費やす待ちの合計は公称する上限と一致する`）足したこと。`682bca6`「実測の出典を最後にコードが変わったコミットに揃える」は出典ラベルを `1766c7b` → `e19c973` に張り替えたが、そのラベルから導かれる件数を取り直していない。68/71 は `1766c7b` 時点の値である。

  差の3件（`#[cfg(all(test, unix))] mod tests`）という主張自体は 72 − 69 = 3 で成立しており、崩れているのは絶対値だけ。ただし HOOKS.md はリポジトリに残る恒久ドキュメントで、AC-8 が求める「3ランナーで実測した記録」そのものである。出典 run から再導出した人が 69/72 を得て記録と食い違う状態は、`triage.md` の R2 で一度 fix した `HOOKS.md:実測/lib件数がHEADと不一致`（当時は 63/66 vs 65/68）と同じ形の再発にあたる。修正は2つの数値を 69 / 72 に直すだけ。

  同じ節の他の実測値は再導出して全数一致した（テストバイナリ15本・`test result:` 18行・除外3件・スキップ集合 unix 1件 / Windows 11件・右3列の実行/スキップ）。動かすのはこの1文の2箇所だけでよい。

#### Warnings

- **[W-001]** PR #12 本文「確認結果（run 31663925152 / コミット `1766c7b` = 現 HEAD）」— 最後にコードとワークフローが変わったコミットを指していない

  `e19c973` は `.github/workflows/ci.yml`（msrv ジョブの版の名乗り・`persist-credentials: false`）と `util/atomic.rs` を変えたコード変更であり、`progress.md:20` が定めた「出典は最後にコードまたはワークフローが変わったコミットに対する実測とする」に照らしても、PR 本文が引くべき出典は run 31665658371 / `e19c973` である。ドキュメントのみのコミットを追いかけない方針とは別の話で、いま本文が引いている run は**マージされるワークフローそのものを回していない**。

  - `1766c7b` は HEAD ではない（HEAD は `2e706df`、最後のコード変更は `e19c973`）。
  - 「吸収とレビュー反映を挟んで4回緑」は5回（`progress.md:22-29` の表と食い違う）。
  - リポジトリ内のドキュメント（`progress.md` / `HOOKS.md` / `testing.md`）は `e19c973` / 31665658371 に揃っており、**本文だけが1つ前の run を最終と呼んでいる**。R3 の W-001 と鏡像の状態になった。

  実害は出典ラベルに閉じる — run 31665658371 は7ジョブすべて success で、スキップ集合も除外件数も本文の記載と一致することを実測で確認した。直すのは run 番号・コミット・「4回」の3点。

  同種のごく小さいものとして `progress.md:3` の「コミット `e19c973` = 現在の HEAD」がある。出典を `e19c973` に置くこと自体は方針どおりで正しく、`= 現在の HEAD` の3語だけが偽（HEAD は `2e706df`）。

- **[W-002]** `crates/pulsen/src/util/atomic.rs:362-376` `時間で解けない拒否に再試行の予算を使わない` — 公開関数が `transiently_denied` を渡している配線を、どのプラットフォームのテストも殺さない

  このテストのコメントは「予算を使うかどうかは、**公開関数が渡している分類**がこの拒否をどう見るかで決まる」と主張する。しかし `budget_spent_on` は `transiently_denied` を直接呼び直しているだけで、`read_atomic` / `rename_atomic` が実際に何を渡しているかを観測していない。変異テストで確認した（変異はすべて元に戻し、worktree も削除済み）。

  | 変異 | 現 HEAD | `1766c7b`（壁時計版） |
  |---|---|---|
  | `read_atomic` の分類を `\|_\| true` に | **全緑（生存）** | `時間で解けない拒否に再試行の予算を使わない` が失敗 |
  | `rename_atomic` の分類を `\|_\| true` に | **全緑（生存）** | — |
  | `write_atomic` の分類を `\|_\| true` に | **全緑（生存）** | — |

  生存した場合の振る舞いは unix でも実害がある側に倒れる — `NotFound` が一時的な拒否として扱われ、存在しないタスクの `lookup` が 511ms 眠ってから返る（排他ロックを保持したまま）。`一時的な拒否と分類されるのは共有違反とアクセス拒否だけ` は分類関数そのものしか見ないので、この配線には届かない。

  これは裁定4（壁時計アサートの除去）の残りで、`e19c973` はフレークの除去と引き換えにこの1件のカバレッジを手放している。ADR-012 の Consequences も「公開関数が実際に返したエラーを同じ分類に掛け」と、いま実装されている形をそのまま書いている。**壁時計を戻す提案ではない**（ランナー依存の赤を持ち込むほうが高くつく）。ただしテストの名前とコメントが検証していないものを主張している状態は残るので、少なくとも「分類そのものを見ている」と読める言い方へ揃えるか、`retry_waits` と同じく配線を観測できる seam を置くかのどちらかで閉じるのが筋。**マージを止める性質のものではない。**

#### 実測の再現（AC-6 / AC-8 の裏取り）

run 31665658371 の3 OS 分のジョブログから `cargo test` の出力を復元し、`ci.yml` の awk をそのまま適用した。

| OS | 除外 | 実在の SKIP |
|---|---|---|
| ubuntu | 3 件 | 1 件（`tc_port_clock_005_巻き戻した時刻はそのまま返る`） |
| macOS | 3 件 | 1 件（同上） |
| Windows | 3 件 | 11 件（上記 + 適合8件 + `tc_task_register_task_016` / `021`） |

Windows の11件は HOOKS.md の右3列と行単位で一致（config-store-023 / workflow-store-030 / task-repository-005・011・012・019・035・041 + clock-005 + CLI 受け入れ2件）。ロック系5件・`non_repo_dir` 系2件はどの OS にも現れず、フィクスチャ側の欠陥は顕在化していない。steps.md:212 の**実行前に書かれた**期待集合（unix 1件 / Windows 11件）と一致しており、AC-6(d) の事後更新は発生していない。

テストバイナリは3 OS とも15本、`test result:` は18行（Doc-tests 3本を含む）で、HOOKS.md:47 の記載どおり。lib の件数だけが B-001。

#### 変異テスト（`crates/pulsen/src/util/atomic.rs`）

`e19c973` で入った `retry_waits()` と `再試行に費やす待ちの合計は公称する上限と一致する` に歯があるかを確認した。

| 変異 | 結果 |
|---|---|
| バックオフ `*2` → `*4` | 失敗（予算が約87秒に伸びるのを検出。R3 で穴だった経路が塞がった） |
| `MAX_ATTEMPTS` 10 → 11 | 失敗 |
| `FIRST_RETRY_WAIT` 1ms → 2ms | 失敗 |
| 公開関数3つの分類を `\|_\| true` へ | 全緑（W-002） |

回数・初回の待ち・伸び幅の3軸すべてに歯が立っており、doc と ADR が根拠にする 511ms は `retry_waits()` を唯一の出典として固定されている。

#### テストの緩和

差分に無い。`#[ignore]` 0 件、テストを除外する `cfg` 0 件（増えた `#[cfg(windows)]` / `#[cfg(not(windows))]` は分類関数 `transiently_denied` の2版で、テストの除外ではない）、アサーションの削除・期待値の骨抜き 0 件。`読み手は旧内容か新内容のどちらかだけを観測する` は読めない瞬間の許容を捨てて `read_atomic(...).expect("読み手は常に読める")` に**強化**され、ポート契約と同じ強さになっている。`task_file.rs` の整形テストも `<repo>` の差し込みだけで、pretty print・キー順・末尾改行の検査は1通りのリテラルのまま残る。

#### カバレッジ

| ファイル | 見たこと |
|---|---|
| `.github/workflows/ci.yml` | ドキュメント整合の観点のみ（CI 設計は ci 観点）。why コメントの主張を run 31665658371 のログと突き合わせ: 除外3件・3状態の切り分け・`sort` / コンテナ指定 / 単一ターゲット指定の綴りが 0 件（ADR-009）・`1.89` のハードコード 0 件・`uses:` は `actions/checkout@v7` の3箇所のみ。awk を復元ログへ適用し、コメントが述べる挙動と実出力が一致することを確認 |
| `.thread/10/adr.md` | ADR-001〜013 を実装と突き合わせ。ADR-010 の「この項目は ADR-012 が置き換えた」と ADR-012 冒頭の置換宣言は決定の系譜であって弁明ではない。ADR-010 / 012 / 013 の 511ms・10回・1ms 倍々・`MAX_ATTEMPTS`(`NonZeroU32`)・`retry_while_transient` の `Result<T, (io::Error, S)>`・分類は1つ、ADR-013 の対象外リストまで実装と一致。ADR-012 Consequences の (3) が W-002 の状態をそのまま記述していることを確認 |
| `.thread/10/plan.md` | AC-1〜AC-8 と成果物を突き合わせ。AC-6(a)(b)(c)(d) はすべて成立（(d) は事前期待と観測が一致、事後更新なし）。AC-8 は表・出典コミットの明記とも成立するが、記録された lib 件数が出典 run と食い違う（B-001）。撤退条件（ADR-008）の適用は無く、`continue-on-error` も 0 件 |
| `.thread/10/progress.md` | 実測表の6 run・吸収の表・スキップ実測・未検証範囲を run のログと照合し一致。`= 現在の HEAD` の3語のみ W-001 |
| `.thread/10/review/review-001-ci.md` | 中間成果物（Phase 8 で削除予定・既知）。R1 の判定継承の確認にのみ使用 |
| `.thread/10/review/review-001-test-docs.md` | 同上。R1 の HOOKS.md 系指摘（件数・測定コミット）の再発有無を確認 → B-001 が該当 |
| `.thread/10/review/review-001-util.md` | 同上 |
| `.thread/10/review/review-001.md` | 同上 |
| `.thread/10/review/review-002-ci.md` | 同上 |
| `.thread/10/review/review-002-test-docs.md` | 同上。R2 の `HOOKS.md:実測/lib件数がHEADと不一致` が B-001 と同一 Key であることを確認 |
| `.thread/10/review/review-002-util.md` | 同上 |
| `.thread/10/review/review-002.md` | 同上 |
| `.thread/10/review/review-003-ci.md` | 同上 |
| `.thread/10/review/review-003-test-docs.md` | 同上。R3 の変異結果との差分（新規3変異が塞がったこと・配線3変異が生存のままであること）を確認 |
| `.thread/10/review/review-003-util.md` | 同上 |
| `.thread/10/review/review-003.md` | 同上 |
| `.thread/10/review/triage.md` | R1〜R3 の判定と裁定1〜4 を確認。見送り判定はゼロで、恒久ドキュメントへ移し損ねた結論は無い。裁定3（出典の定義）が `progress.md:20` に、裁定4（待ちの列）が `retry_waits()` と ADR-010 に落ちていることを確認 |
| `.thread/10/steps.md` | ステップ3 の期待集合（unix 1件 / Windows 11件）が実行前の形で残り、観測と一致。ステップ11 の「緑になった最終実行の結果を書く」に対し HOOKS.md が run 31665658371 を引いていること、ステップ12 の記録先（PR 本文・Issue #10・PR #11）が実在することを確認 |
| `.thread/10/testing.md` | 確認項目1〜7 の期待結果をローカルで再実行（`1.89` 0件・`rustfmt.toml` は `edition = "2024"` のみ・`uses:` 3箇所・`sort` / `container:` / `--test ` 0件・ドメイン層の cfg grep 0件・`cargo metadata` が `1.89` 1行）。すべて記述どおり。219行の測定コミットは `e19c973` で正 |
| `crates/pulsen-conformance/HOOKS.md` | 追記した右3列と「3ランナーでの実測」節を run 31665658371 のログと全数照合。区分 A/B/C の件数表・ポート別内訳との整合、`未測定` の運用記述、除外される架空3件の綴りまで確認。lib 件数のみ B-001 |
| `crates/pulsen/src/adapter/task_file.rs` | フィクスチャ可搬化と `<repo>` 差し込み。整形の検査（pretty print・キー順・末尾改行）が1通りのリテラルのまま残り、緩和が無いことを確認。`absolute()` は既存の `MAIN_SEPARATOR` 分岐の作法（ADR-037）に揃う |
| `crates/pulsen/src/adapter/task_repository.rs` | 3箇所の `fs::read` → `read_atomic`。`NotFound` 分岐が初回で返る前提が保たれ「消えたエントリ / 読めないエントリ」の判別が不変であること、モジュール doc の遅延記述（`save_degraded` 2倍 / `list` N 倍）が呼び出し回数と一致することを確認。`#[cfg(all(test, unix))] mod tests` は `origin/main` から無変更 |
| `crates/pulsen/src/util/atomic.rs` | 変異6種（上表）。新規の待ち列テストは3軸すべてに歯があり、`MAX_ATTEMPTS` の why コメント（`上限 - 1` 本の列・リリースでの伸び）が現在の実装と一致することも確認。公開関数の配線のみ W-002。doc・コメントは why のみで、経緯・弁明は無い |

#### 判定

**B-001（HOOKS.md の2つの数値）を直せばマージできる。** 恒久ドキュメントに残る実測値が出典 run から再導出できない状態だけが、この PR の記録としての価値を直接損なう。W-001 はマージ前に PR 本文を1回更新すれば閉じ、W-002 は次に `util/atomic.rs` を触るときで足りる。
