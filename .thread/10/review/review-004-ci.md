# レビュー 004 — CI・ビルド基盤

**対象:** PR #12 / `issue/10/ci-msrv-cross-platform` → `main`（HEAD `2e706df`）
**契約:** `.thread/10/plan.md`（AC-1〜AC-4 / AC-6 / AC-7）、`.thread/10/adr.md`（ADR-001〜013）
**結論:** マージ可。CI・ビルド基盤の観点で実害のある指摘は無い。

## 実測で確認したこと

前提として与えられた「run 31665658371 で全7ジョブ緑」は再確認済み。加えて、その後のドキュメントのみのコミットに対しても
**run 31665868052（`2e706df` = 現 HEAD）が success** で、現在の HEAD が緑であることを確認した。

CI ログ（run 31665658371）から独自に採取した検証結果:

- `actions/checkout@v7` は実在し `3d3c42e5aac5ba805825da76410c181273ba90b1` に解決。
  ログ全体に deprecation / warning アノテーションは 0 件。
- 実行シェルが全ステップで `/usr/bin/bash --noprofile --norc -e -o pipefail {0}`。
  `tee` を挟んだテストステップで cargo の失敗が伝播する前提（ワークフローの why コメント）は成立している。
- `test (ubuntu-latest)` の前提検査が `/usr/bin/awk` `/usr/bin/cat` `/usr/bin/grep` `/usr/bin/id` `/usr/bin/tee` を解決。
  非 root アサートは `uid=1001` を出して通過。
- 3 OS の msrv ジョブが `MSRV: 1.89` を読み出し、`rustc 1.89.0 (29483883e 2025-08-04)` を名乗ってビルド。
  ワークフローに版数のハードコードは無い（AC-4 の「唯一の出典」が成立）。
- `SKIP ` 行をログから直接数えたところ、**unix は実在ケース1件（`tc_port_clock_005`）＋架空3件、
  Windows は実在ケース11件（`tc_port_clock_005` ＋ 権限系 8 ＋ `tc_task_register_task_016` / `021`）＋架空3件**。
  steps.md ステップ3 の事前期待集合、および HOOKS.md「3ランナーでの実測」の記述と完全に一致する（AC-6(d) / AC-8）。

`sort` / `container:` / `--test ` の綴りはワークフローに 0 件（ADR-009）。
`crates/pulsen-domain/src/` の target 述語つき `cfg` も 0 件（AC-5 の機械確認部分）。

## awk の実装差（ubuntu-latest は mawk）

スキップ報告の awk プログラムを合成ログに掛け、`gawk` / `gawk --traditional` / `gawk --posix` の3モードで
同一の出力（除外3件・列挙1件）になることを確認した。使っている機能は POSIX awk の範囲に収まっている。

- `-v esc="$(printf '\033')"` の値に逆スラッシュが無いので、`-v` の escape 処理系差の影響を受けない。
- 動的正規表現 `esc "\\[[0-9;]*m"` は文字列リテラル経由で `\[` になり、意図どおり `[` のリテラルとして解釈される。
- `match()` をパターン位置で使う形、`RSTART` / `RLENGTH`、未初期化変数の `%d`（→ `0`）はいずれも実装差が無い。

ubuntu-latest の `/usr/bin/awk` は mawk だが、そこで得られた実測結果が上の期待集合と一致しているため、
実装差の懸念は理論と実測の両方で潰れている。

## シェルの落とし穴

いずれも why コメントの主張と実際の挙動が一致していることを確認した。

- `uid=$(id -u)` を代入で受けてから `case` で検査する形。`-e` 下では代入内のコマンド置換の失敗が
  その場で落ちるため、`if [ "$(id -u)" -eq 0 ]` の fail-open を回避できている。
  数値でない出力も `'' | *[!0-9]*` で失敗側へ倒れる。
- `[ "$(grep -c 'test result:' test.log)" = "0" ]` — `[` の引数位置ではコマンド置換の終了コードが捨てられるため、
  該当なしの `exit 1` がステップの失敗にならない。コメントの説明どおり。
- msrv ジョブの `case $(printf '%s\n' "$versions" | grep -c .) in` も同じ理屈で `-e` の対象外。
  `versions` が空なら 0 件で失敗、複数種類なら `*)` で失敗、と AC-4 の3条件を尽くしている。
- `${{ }}` は `run` の本文に一度も現れず、`steps.msrv.outputs.version` は `env:` 越しに渡して引用符で受けている。
  フォークが書き換えられる `Cargo.toml` 由来の値がシェル構文に混ざる経路が無い。

## サプライチェーン

- Action は `actions/checkout@v7` の1つだけ。第三者製はゼロで、toolchain はランナー同梱の `rustup` を直接叩く（AC-7）。
- 全ジョブで `persist-credentials: false`。同じジョブで走るテストコードから `GITHUB_TOKEN` が読めない。
- `permissions: contents: read` がワークフロー全体に掛かる。`pull_request_target` は使っていない。
- SHA ピンではなくメジャータグ固定である点は ADR-001 が理由つきで採った判断であり、
  他に Action を持たない構成では守る面が1つに閉じている。本 PR の範囲では実害にならない。

## Blockers

なし。

## Warnings

なし。

前ラウンドまでの指摘（`sort` の排除、行のどこでも `SKIP` を拾う抽出、区間を閉じる `test result:`、
`--no-fail-fast`、非 root アサートの fail-open 回避、再試行上限のユニットテスト化）はいずれも
現在のワークフロー・実装に反映済みで、実測でも裏が取れている。
残る差分はドキュメントのみで、CI・ビルド基盤の観点から新たに挙げるべき実害は見つからなかった。

## カバレッジ

| ファイル | 扱い |
|---|---|
| `.github/workflows/ci.yml` | 精読。AC-1〜AC-4 / AC-6 / AC-7 を条文単位で突き合わせ、awk を3実装モードで実行検証し、CI ログで実挙動を確認 |
| `crates/pulsen-conformance/HOOKS.md` | 精読。3ランナーの実測記録（AC-8）を CI ログの `SKIP` 行と突き合わせて一致を確認 |
| `crates/pulsen/src/util/atomic.rs` | 差分を精読。ビルド基盤の観点で MSRV 適合（`NonZeroU32::new(...).expect(...)` の const 評価は 1.83 以降・実測で 1.89 緑）と再試行上限の検証手段を確認。ドメイン設計・並行性の妥当性は util 観点のレビューに委ねる |
| `crates/pulsen/src/adapter/task_repository.rs` | 差分を精読。`fs::read` → `read_atomic` の置換のみで、CI 観点の論点なし |
| `crates/pulsen/src/adapter/task_file.rs` | 差分を精読。フィクスチャの `MAIN_SEPARATOR` 化と期待値のプレースホルダー化。Windows でのビルド・テスト成立を CI 実測で確認 |
| `.thread/10/plan.md` | 精読。受け入れ基準とスコープの検証に使用 |
| `.thread/10/adr.md` | 精読（ADR-001〜013）。実装との一致を確認 |
| `.thread/10/testing.md` | CI 関連の確認手順（Action 列挙・シェル・版数ハードコード・`sort` / コンテナ指定 / 単一ターゲット指定・ドメイン層 grep）のみ確認。手順の記述と実際の grep 結果が一致 |
| `.thread/10/progress.md` | スキップ。進行記録であり CI・ビルド基盤の判断材料にならない |
| `.thread/10/steps.md` | スキップ（ステップ3 の事前期待集合のみ、plan.md / HOOKS.md 経由で間接的に照合） |
| `.thread/10/review/review-001*.md`（4件） | スキップ。ゼロベースのラウンドのため過去ラウンドの指摘は参照していない |
| `.thread/10/review/review-002*.md`（4件） | 同上 |
| `.thread/10/review/review-003*.md`（4件） | 同上 |
| `.thread/10/review/triage.md` | 同上 |
