# ブラウザ検証の結果 — Issue #10

**Issue:** #10 / **PR:** #12
**実施日:** 2026-08-13
**対象:** run 31666626824（コミット `e524981`、全7ジョブ success）
**ツール:** agent-browser 0.33.2

---

## 実施範囲の判断

**製品側のブラウザ検証は該当なし。** 次を実際に確認した。

- `package.json` が存在しない（`ls package.json` → No such file）。Rust のワークスペースで、dev / start 系の起動スクリプトそのものが無い
- UI ファイルが1件も無い（`*.html` / `*.tsx` / `*.jsx` / `*.vue` / `*.svelte` を `target/` と `.direnv/` を除いて探索 → 0件）
- 成果物は CLI（`crates/pulsen` の bin `pulsen`）と CI ワークフローのみ

したがって `.thread/10/testing.md` の確認項目のうち画面操作を要するものは、**AC-6(b) の「ジョブサマリーに `SKIP ` 行が列挙される」1件だけ**である。これは製品の UI ではなく検証成果物（GitHub Actions の run ページ）だが、`gh run view` のログには出ず API でも取得できないため、ブラウザが唯一の確認手段になる。ここだけを対象に実施した。

## 結果

| # | 確認項目 | 結果 |
|---|---|---|
| 1 | run ページが開き、7ジョブすべてが success と表示される | **PASS** |
| 2 | 各 OS のジョブサマリーに除外件数と `SKIP ` 行の一覧が描画される | **未確認**（下記） |

### 1. run の状態（PASS）

`https://github.com/tuanemuy/pulsen/actions/runs/31666626824` を開き、次を確認した。

- Status: **Success**、Total duration 1m 36s
- All jobs に `fmt` / `test (ubuntu-latest)` / `test (macos-latest)` / `test (windows-latest)` / `msrv (ubuntu-latest)` / `msrv (macos-latest)` / `msrv (windows-latest)` の**7つ**が並ぶ（AC-1 のジョブ構成）
- Triggered via pull request #12、コミット `e524981`

### 2. ジョブサマリーの描画（未確認）

**読めなかった。** run ページのサマリー領域が "There was an error while loading." を返し、ジョブページ（`/job/94342476669`）でも各ステップが "This step has been truncated due to its large size." になって本文が描画されない。ページに `Sign in` が出ており、**agent-browser のセッションが GitHub に未認証**であることが原因。

認証にはユーザーの GitHub 資格情報が要るため、こちらの判断では行っていない。

**この未確認は変更起因の FAIL ではない。** 検証環境（未認証ブラウザ）の制約であって、実装の欠陥ではない。

### サマリーの内容について、代わりに取った裏取り

ジョブサマリーの中身は、ワークフローの報告ステップが `$GITHUB_STEP_SUMMARY` へ書いた awk の出力そのものである。そこで **`.github/workflows/ci.yml` から報告ステップの `run` 本文を機械的に抽出し、CI の実ログを復元したものに対して実行**して出力を確認した（testing.md 確認項目4 の代替）。

復元手順: `gh run view --job <id> --log` の出力からジョブ名・ステップ名・タイムスタンプの接頭辞を剥がし、`^[`（2文字）を実 ESC に戻す。

| OS | 除外件数 | 実在スキップの一覧 |
|---|---|---|
| ubuntu-latest | 3 | 1件（`tc_port_clock_005_巻き戻した時刻はそのまま返る`） |
| macos-latest | 3 | 1件（同上） |
| windows-latest | 3 | 11件（上記 + 権限系10件） |

架空3件（`tc_port_clock_004_時刻の前進` / `tc_port_clock_0051_別のケース` / `tc_port_clock_005_時刻の巻き戻し`）はどの OS の一覧にも現れない。**steps.md ステップ3 に実行前から書かれた期待集合と完全に一致**しており、期待値の事後更新は発生していない（AC-6(d)）。

残る未検証は「この文字列が GitHub のページ上で Markdown として正しく描画されるか」だけで、生成される内容自体は確認済みである。

## 起票した Issue

なし（変更起因の FAIL はゼロ）。

## 人が確認する場合

サインイン済みのブラウザで次を開き、各ジョブの Summary を読む。

```
https://github.com/tuanemuy/pulsen/actions/runs/31666626824
```
