# レビュー R3 — テスト・ドキュメント整合（PR #12）

対象: `origin/main...HEAD`（HEAD = `1766c7b`）全18ファイル。
検証に使った実測: run 31663925152（`1766c7b`、全7ジョブ success）の3 OS 分のジョブログを API から取得し、`test.log` に相当する区間を復元してワークフローの awk をそのまま適用した。`util/atomic.rs` は隔離した worktree で本番コードに8種の変異を当て、テストが落ちることを確認した（作業ツリーは無変更）。

#### Blockers

なし。

テストの緩和は差分に存在しない。`#[ignore]` 0 件、テストを除外する `cfg(windows)` 0 件、アサーションの削除・期待値の骨抜き 0 件。増えた `#[cfg(windows)]` / `#[cfg(not(windows))]` は分類関数 `transiently_denied` の2版で、テストの除外ではない。`読み手は旧内容か新内容のどちらかだけを観測する` は `if let Ok(...)`（読めない瞬間を許容）から `read_atomic(...).expect("読み手は常に読める")` へ**強化**されており、ポート契約と同じ強さになった。`task_file.rs` の整形テストも `<repo>` の差し込みだけで、pretty print・キー順・末尾改行の検査は1通りのリテラルのまま残っている。

#### Warnings

- **[W-001]** `.thread/10/progress.md` / `crates/pulsen-conformance/HOOKS.md` / `.thread/10/testing.md` — 最終コミットと出典 run の記述が HEAD と食い違う

  - `progress.md:3` は「run 31662960664、コミット `9675b2f` = 現在の HEAD」と書くが、HEAD は `1766c7b`（run 31663925152）。
  - `progress.md:22-27` の run 表に最新の run 31663925152（`1766c7b`）の行が無い。`progress.md:29` の「緑になった3つの run」も、実際には緑は4回。
  - `progress.md:59` の「実測したのは `9675b2f`（本 PR の変更をすべて適用した時点）」、`HOOKS.md:47` の「測定したのは `9675b2f`（Issue #10 の CI・その吸収・**レビュー反映まで適用した時点**）」、`testing.md:219` の同旨の記述は、いずれも `1766c7b` でレビュー反映（SKIP 抽出の awk 変更）がもう一段入ったあとでは成り立たない。
  - PR #12 本文だけが run 31663925152 / `1766c7b` を正しく指しており、本文とリポジトリ内ドキュメントが別の run を最終と呼んでいる。

  実害は出典ラベルに閉じる。**記録されている中身は `1766c7b` でも1件残らず一致する**ことを実測で確認した（下記「実測の再現」）。したがって直すのは run 番号・コミット・「現在の HEAD」「3つの run」の4点だけで、表の中身・件数は動かない。逆に言えば、この4点を直さないと「どの版のワークフローがこのサマリーを出したか」が永久に1コミットずれたまま残る — 最後の変更がまさにサマリー抽出そのものなので、再導出しようとした人が違う awk を読むことになる。

- **[W-002]** `.thread/10/review/` の中間成果物8ファイルが差分に含まれている

  `review-001-*.md` / `review-001.md` / `review-002-*.md` / `triage.md` は、指摘・弁明・修正の経緯そのものである。main の先例は PR #8 マージ前の `38fe9c5`「chore: レビューの中間成果物を削除」で `.thread/1/review/` を全削除しており、`origin/main` に残っているのは `.thread/1/` の adr / plan / progress / steps / testing / manual-test だけ。CLAUDE.md「残すのは現在の形が成り立つ理由（why / why not）だけ」もこの運用と揃う。片付けフェーズで削除する前提なら現状で問題ないが、**この差分のままマージすると先例と食い違う。**

  なお、指摘から得た結論のうち残すべきものは既に恒久的な場所へ移されている（読み手側の吸収 → ADR-013 と `util/atomic.rs` の doc、抽出が行頭一致では足りない理由 → ADR-005 とワークフローの why コメント）。削除しても失われる根拠は無い。

#### 実測の再現（AC-6 / AC-8 の裏取り）

run 31663925152 の3 OS 分のジョブログから `cargo test` ステップの出力を復元し、`ci.yml` の awk をそのまま適用した結果:

| OS | 除外 | 実在の SKIP |
|---|---|---|
| ubuntu | 3 件 | 1 件（`tc_port_clock_005_巻き戻した時刻はそのまま返る`） |
| macOS | 3 件 | 1 件（同上） |
| Windows | 3 件 | 11 件（上記 + `tc_task_register_task_016` / `021` + 適合8件） |

Windows の11件の内訳は HOOKS.md の表と行単位で一致した（config-store-023 / workflow-store-030 / task-repository-005・011・012・019・035・041 + clock-005 + CLI 受け入れ2件）。ロック系5件・`non_repo_dir` 系2件はどの OS にも現れておらず、フィクスチャ側の欠陥は顕在化していない。

HOOKS.md「3ランナーでの実測」の数値も全数一致した。

- テストバイナリ15本 / `test result:` 18行（Doc-tests 3本を含む）— 3 OS とも同数。
- `pulsen` の lib が Windows 68・unix 71。残る14バイナリ（`cli_add_boundary` 21 / `cli_add_error` 31 / `cli_add_normal` 12 / `cli_usage` 5 / `conformance_config_store` 24 / `conformance_lock` 7 / `conformance_task_repository` 44 / `conformance_time_id` 10 / `conformance_workflow_store` 31 / `conformance_worktree` 9 / `register_task` 22 / `pulsen_conformance` 13 / `pulsen_domain` 167 / `pulsen` bin 0）は3 OS 完全同数。差の3件が `adapter/task_repository.rs` の `#[cfg(all(test, unix))] mod tests`（`#[test]` を数えて3件）であることも確認した。
- 除外される架空ケース3件の綴り（`tc_port_clock_004_時刻の前進` / `tc_port_clock_0051_別のケース` / `tc_port_clock_005_時刻の巻き戻し`）も HOOKS.md:57 の記載どおり。

HOOKS.md の既存の分類とも矛盾しない。区分 C 12件の内訳は権限系8 + `permission_restrictions_effective` 以外の4（clock-003 / clock-005 / exclusive-lock-007 / worktree-manager-009）で、冒頭の件数表（A 28 / B 85 / C 12 / 計 125）とポート別内訳（C: 1+1+6+2+0+1+1）が一致する。追記した右3列は既存の左4列を1行も動かしていない。

#### 変異テスト（`crates/pulsen/src/util/atomic.rs`）

隔離した worktree（`git worktree add --detach`）で本番コードを壊し、`cargo test -p pulsen --lib` で落ちることを確認した。作業ツリーは終始無変更で、`git diff` は空、`atomic.rs` のハッシュも元のまま。

| 変異 | 結果 |
|---|---|
| `MAX_ATTEMPTS` 10 → 2 | 3件失敗（「上限内に解ければ〜」の置換・移動・読み取り） |
| 打ち切り条件から `!is_transient(&error)` を落とす（常に再試行） | 2件失敗（`一時的でない拒否は再試行せずに返る` / `時間で解けない拒否に再試行の予算を使わない`） |
| `#[cfg(not(windows))]` の `transiently_denied` を `true` に | 2件失敗（`一時的な拒否と分類されるのは共有違反とアクセス拒否だけ` / `時間で解けない拒否に〜`） |
| `wait *= 2` → `wait *= 1`（バックオフを止める） | 3件失敗（同上の「上限内に解ければ〜」3件） |
| 打ち切り時に OS のエラーを独自エラーへ差し替える | 7件失敗（`atomic` 5件 + `task_repository` の到達不能エントリ2件） |
| `read_atomic` の本体を `fs::read` へ戻す | **全緑（unix では検出されない）** |
| `rename_atomic` の `rename_with_retry` を `fs::rename` へ戻す | **全緑（同上）** |
| `write_atomic` の `persist_with_retry` を素の `persist` へ戻す | **全緑（同上）** |

後半3件が unix で生き残るのは、`transiently_denied` が unix で常に偽であり、公開関数が再試行に入る様子が**原理的に観測できない**ため。ADR-012 の Consequences がこのトレードオフを明記しており、そのうえでテストを (1) 分類そのもの（`cfg!(windows)` から期待値を導くので全 OS で走る）(2) `persist_with_retry` / `rename_with_retry` / `read_with_retry` の吸収 (3) 公開関数が一時的でない拒否に予算を使わないこと、の3つへ分ける設計になっている。実装はその3分割どおりで、seam を置いた目的（打ち切りの検証を分類から独立させる）は達成されている。

残る穴は「公開関数 → seam」の配線1行だが、`write_atomic` と `read_atomic` については Windows CI の `読み手は旧内容か新内容のどちらかだけを観測する`（初回の Windows 赤の当事者）が実際に殺す。`rename_atomic` だけは殺し手が TC-port-task-repository-044 の窓を踏むかどうかに依存し、現状は未観測 — ADR-012 が「未観測は赤でないことを意味しない」として先回りで入れた箇所そのものなので、これ以上の手当ては要らないと判断する。R2 の指摘（テストが `retry_while_transient` を直接呼んでいて本番の配線を通らない）は、3つの `*_with_retry` を直接呼ぶ形に直っており、提案 (a) のとおりに解消されている。

#### カバレッジ

| ファイル | 見たこと |
|---|---|
| `.github/workflows/ci.yml` | ドキュメントとの整合の観点のみ（CI 設計は ci 観点の担当）。why コメントの主張を実測と突き合わせた: 除外3件・3状態の切り分け・`sort` / コンテナ指定 / 単一ターゲット指定の綴りが 0 件（ADR-009 どおり）・`1.89` のハードコード 0 件・`uses:` は `actions/checkout@v7` の3箇所のみ。awk を復元ログへ適用して、コメントが述べる挙動と実際の出力が一致することを確認 |
| `.thread/10/adr.md` | ADR-010 と ADR-012 / ADR-013 の関係（ADR-010 末尾の「この項目は ADR-012 が置き換えた」）は決定の系譜であって弁明ではない。実装との突き合わせ: 上限10回・1ms 倍々・累計 511ms、`MAX_ATTEMPTS`（`NonZeroU32`）、`retry_while_transient` の `Result<T, (io::Error, S)>`、分類は `transiently_denied` 1つ、ADR-013 の対象外（`config_store` / `workflow_store` に `write_atomic` 経路が無いこと）まで確認。矛盾なし |
| `.thread/10/plan.md` | AC-6 / AC-8 の要求と成果物を突き合わせ。AC-6(a)(b)(c)(d) はすべて満たす（(d) は期待集合と観測が一致し、事後更新が発生していない）。AC-8 は記録の中身が正しく、出典コミットの明記もあるが、そのコミットが最新でない点は W-001 |
| `.thread/10/progress.md` | W-001。それ以外（吸収の表・スキップの実測・未検証範囲）は事実と一致 |
| `.thread/10/review/review-001-ci.md` | W-002（中間成果物） |
| `.thread/10/review/review-001-test-docs.md` | W-002 |
| `.thread/10/review/review-001-util.md` | W-002 |
| `.thread/10/review/review-001.md` | W-002 |
| `.thread/10/review/review-002-ci.md` | W-002 |
| `.thread/10/review/review-002-test-docs.md` | W-002。R2 の W-001（本番の配線を通らない）が今回どう解消されたかの確認に使用 |
| `.thread/10/review/review-002-util.md` | W-002 |
| `.thread/10/review/triage.md` | W-002。R1/R2 の判定がすべて fix で、見送りを恒久ドキュメントに移し損ねた項目が無いことを確認 |
| `.thread/10/steps.md` | ステップ3 の期待集合（unix 1件 / Windows 11件）が実行前に書かれた形で残っており、観測がそれと一致。ステップ11 が求める4点は HOOKS.md に揃っている。ステップ12 の記録先（PR 本文・Issue #10・PR #11）も実在を確認 |
| `.thread/10/testing.md` | 確認項目1〜7 の期待結果をローカルで再実行（`1.89` 0件・`rustfmt.toml` は `edition = "2024"` のみ・`uses:` 3箇所・`sort` / `container:` / `--test ` 0件・ドメイン層の cfg grep 0件）。すべて記述どおり。219行目の測定コミットのみ W-001 |
| `crates/pulsen-conformance/HOOKS.md` | 追記した3列と「3ランナーでの実測」節を run 31663925152 のログと全数照合（上記）。既存の区分 A/B/C と件数表との整合も確認。出典 run / コミットのみ W-001 |
| `crates/pulsen/src/adapter/task_file.rs` | フィクスチャ可搬化と `<repo>` 差し込み。整形の検査（pretty print・キー順・末尾改行）が1通りのリテラルのまま残り、緩和が無いことを確認。`absolute()` は既存の `MAIN_SEPARATOR` 分岐の作法（ADR-037）に揃っている |
| `crates/pulsen/src/adapter/task_repository.rs` | 3箇所の `fs::read` → `read_atomic`。`NotFound` 分岐が初回で返る前提が保たれ、「消えたエントリ / 読めないエントリ」の判別が変わらないこと、モジュール doc の遅延記述（`save_degraded` 2倍 / `list` N 倍）が呼び出し回数と一致することを確認。`#[cfg(all(test, unix))] mod tests` は origin/main から変更なし（Windows のカバレッジ差は HOOKS.md に記録済み） |
| `crates/pulsen/src/util/atomic.rs` | 変異テスト8種（上記）。新規テスト7件はいずれも実効的で、`一時的な拒否が続けば上限の回数だけ試みて元のエラーを返す` が定数を production と共有する点も、上限値の変化は「上限内に解ければ〜」3件が捕まえるので穴にならない。doc コメントは why のみで、経緯・弁明は無い |
