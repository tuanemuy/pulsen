# レビュー 003 — CI・ビルド基盤

**対象:** PR #12 / base `main` / HEAD `1766c7b`
**観点:** CI・ビルド基盤（偽陰性・シェルの落とし穴・OS 差・サプライチェーン・why コメントの整合）
**前提とした実測:** run 31663925152（`1766c7b`、全7ジョブ success）

前回までの指摘は前提にせず、差分・ADR・CI ログをゼロから引き直して検証した。

---

## 検証方法（再現手順）

偽陰性の有無は読みだけでは決まらないので、主要な主張を実行して確かめた。

### 1. スキップ報告 awk を実 CI ログで再現

`gh run view --job <id> --log` は ESC を `^[` に落とし、各行に `ジョブ名\tステップ名\tタイムスタンプ ` を前置するので、`テストする` ステップの区間だけを取り出して両方を復元し、`test.log` を再構成した。

```sh
perl -ne 'next unless /^[^\t]*\tテストする\t/; s/^[^\t]*\t[^\t]*\t\S+ //; s/\^\[/\x1b/g; print' \
  <job.log> > <os>-test-recon.log
```

その上で、ワークフローの awk プログラムを**そのまま**3種類の実装で回した。

| 実装 | 対応するランナー | 結果 |
|---|---|---|
| gawk 5.4.1 | （参考） | 一致 |
| mawk 1.3.4 | ubuntu-latest（Debian 系の既定） | 一致 |
| BWK awk 20200816 (`/usr/bin/awk`) | macos-latest | 一致 |

出力（3実装とも同一）:

| OS | 除外 | 実在の SKIP |
|---|---|---|
| ubuntu | 3 件 | 1 件（`tc_port_clock_005_巻き戻した時刻はそのまま返る`） |
| macOS | 3 件 | 1 件（同上） |
| Windows | 3 件 | 11 件（`tc_port_clock_005` + 権限系10 = 適合行8 + CLI 受け入れ `tc_task_register_task_016` / `021`） |

**AC-6(d) の期待集合（steps.md L212 / L216-218 に実行前から記載: unix 1件・Windows 11件）と完全一致。** 観測値の事後書き写しは発生していない。ロック系5件・`non_repo_dir` 系2件はどの OS にも現れておらず、フィクスチャ側の欠陥も出ていない。

awk の移植性についても具体的に確認した。`gsub(esc "\\[[0-9;]*m", "", line)` の動的正規表現、`(Running|Doc-tests) ` の ERE 交替、`match` + `RSTART` / `RLENGTH` + 2引数 `substr`、未初期化変数への `%d` — いずれも POSIX awk の範囲で、mawk・BWK awk・gawk で同じ結果を返す。`-v esc="$(printf '\033')"` は生の ESC バイトを渡すため、mawk の `-v` エスケープ処理の対象にならない（バックスラッシュを含まない）。マルチバイトについても、mawk/BWK はバイト、gawk は文字で一貫しており `match` と `substr` の単位が揃うので破綻しない。

Windows のログでは `Running unittests src/lib.rs (D:\a\...\pulsen_conformance-<hash>.exe)` のようにパス区切りが `\` になるが、区間判定は `/pulsen_conformance-/` の部分一致なので影響しない（実測で確認済み）。

### 2. `set -eo pipefail` 下のシェル挙動

実際のランナーのシェル指定は全ステップで `bash --noprofile --norc -e -o pipefail {0}`（Windows は `C:\Program Files\Git\bin\bash.EXE`）であることをログで確認した。ワークフローの why コメントが主張する 3 点を、同じフラグの bash で実行して裏を取った。

| 主張（ci.yml） | 検証 | 結果 |
|---|---|---|
| L183-184「`grep -c` の exit 1 は `if` の条件・`[` の引数では失敗にならない」 | `if [ "$(grep -c zzz /etc/hosts)" = "0" ]` | 分岐に到達し `exit=0`。主張どおり |
| L255「`case` の被検査式に置けば `-e` の対象外」 | `case $(printf '' \| grep -c .) in 0) ...` | `branch-0` に到達し `exit=0`。主張どおり |
| L101-103「代入形なら `id` の失敗でその場で落ちる」 | `uid=$(sh -c "exit 3")` | `exit=3` で即終了。fail-open していない |

`versions=$(cargo metadata ... | jq ...)` も検証した（`versions=$(false-pipeline)` → `exit=1` で即終了）。pipefail + 単純代入なので、`cargo metadata` / `jq` のどちらが落ちても版数が空のまま先へ進むことはない。

`cargo test ... 2>&1 | tee test.log` の終了コード伝播も同じ根拠で成立している（pipefail、かつ `tee` はパイプ末尾ではなくパイプ全体の失敗を拾う側）。

### 3. 偽陰性を作りうる構造の洗い出し

`if:` 条件・握り潰し・`continue-on-error` を機械的に列挙した（`grep -n "shell:\|uses:\|if:\|continue-on-error\|timeout-minutes\|fail-fast"`）。

- `continue-on-error` はコメント中の言及のみで、キーとしては 0 件。
- `if:` は 3 箇所のみ。
  - `if: runner.os != 'Windows'`（非 root アサート）— 状態関数を含まないので `success() &&` が暗黙に付き、前段が赤なら走らない。想定どおり。
  - `if: always()`（スキップ報告）— 記録専用で、3 分岐とも `exit 0`。合否に関与しないのは ADR-005 の設計そのもの。
  - `if: success() || failure()`（clippy）— キャンセル時以外は必ず走る。`continue-on-error` ではないので clippy が落ちればジョブは赤のまま。コメント L217 の主張と一致。
- `shell:` はワークフロー直下の 1 件のみ（AC-1「run ステップのシェルが OS で分かれない」を満たす）。
- ジョブ構成は Psych でパースして確認: トップキーが `["name", true, "permissions", "concurrency", "env", "defaults", "jobs"]`、`fmt`(1) + `test`(3 OS) + `msrv`(3 OS) = **7ジョブ**、両マトリクスに `fail-fast: false`。

### 4. サプライチェーン

- `uses:` は `actions/checkout@v7` の 3 箇所のみ。第三者製 Action ゼロ（AC-7 / ADR-001）。
- `${{ }}` が `run:` のスクリプト本文に展開される箇所は **0 件**。`matrix.os` は `runs-on`、`github.workflow` / `github.ref` / `github.event_name` は `concurrency`、`steps.msrv.outputs.version` は `env:` にだけ現れる。`Cargo.toml`（フォークが自由に書き換えられる）由来の値をシェル構文へ混ぜる経路は存在しない。
- `permissions: contents: read` がワークフロー全体に掛かる。シークレットの参照はなく、`pull_request_target` も使っていない。

### 5. AC の機械確認

| 確認 | コマンド | 結果 |
|---|---|---|
| 版数のハードコード無し（AC-4） | `grep -n "1\.89" .github/workflows/ci.yml` | 0 件 |
| コンテナ実行なし（AC-6c） | `grep -n "container:" .github/workflows/ci.yml` | 0 件 |
| 単一テストターゲット指定なし（AC-6c） | `grep -n -- "--test " .github/workflows/ci.yml` | 0 件 |
| `sort` 不使用（AC-7 / ADR-001） | `grep -n "sort" .github/workflows/ci.yml` | 0 件 |
| ドメイン層の target 述語つき `cfg`（AC-5） | plan.md 記載の `grep -rnE` | 0 件 |
| 整形の抑止なし（AC-3） | `cat rustfmt.toml` | `edition = "2024"` のみ |

ログからの裏取り:

- **AC-3** — fmt ジョブは `rustup update stable` を踏んだうえで `rustc 1.97.1` / `rustfmt 1.9.0-stable (8bab26f4f6 2026-07-14)` をログに残し、`cargo fmt --all --check` が緑。
- **AC-4** — 3 OS の msrv ジョブが `MSRV: 1.89` を出力し、`rustup toolchain install "$MSRV"` → `cargo "+$MSRV" build --workspace --all-targets --locked` が成功（Windows は 29.65s で Finished）。`$MSRV` は `env:` 経由で渡っている。
- **AC-7** — 前提検査の解決先が全 OS でログに出ている。Windows は `awk` / `cat` / `grep` / `id` / `tee` がすべて `/usr/bin/...`（Git for Windows 側）で、`C:\Windows\System32\` 側を掴んでいない。
- **AC-6(a)** — ubuntu / macOS で非 root アサートが success、Windows では `if: runner.os != 'Windows'` により skipped。

---

## Blockers

**なし。**

「本来落ちるべきものを見逃す構造」を探した範囲で、合否に効く経路に穴は見つからなかった。具体的には次を確認している。

- スキップ報告は `exit 0` しか返さないが、これは合否を持たない設計（ADR-005）であり、宣言外スキップの検出は `SkipBudget` 側が担う。CI が二重化していないことは意図された構造であって、偽陰性ではない。
- スキップ報告の区間除外は「迷ったら拾う・開く」側にしか倒れない（`Doc-tests pulsen_conformance` は `-` を含まないので `drop = 0` になり、区間が閉じる方向へ倒れる）。実在ケースが黙って消える唯一の経路（除外区間の境界に SKIP が連結する）は、除外件数が 3 から動くことで見える形になっており、ADR-005 が明示的に受容している。今回は 3 OS とも 3 件。
- `cargo test` は `--workspace` のままで、example（`examples/lock_holder`）がビルドされる。ロック系5件が3 OS すべてで「実行」になっていることが HOOKS.md の実測と CI ログの両方で裏付けられている。
- `--no-fail-fast` を付けても失敗時の終了コードは非ゼロのまま（`cargo test --no-fail-fast` は失敗があれば 101）。判定は緩んでいない。

## Warnings

### [W-001] `.thread/10/progress.md` — 記録が現 HEAD より 1 コミット古い

progress.md 冒頭は「run 31662960664、コミット `9675b2f` = **現在の HEAD**」と書き、「実測に使った run」の表も `9675b2f` で終わっている。実際の HEAD は `1766c7b`（`fix: SKIP の抽出を行のどこでも拾う形にする`）で、対応する run 31663925152 が表に無い。

この表は「どの変更がどの実測に載ったか」を辿るために置かれているもので、HEAD で変わったのは**まさにこのレビューの主題であるスキップ抽出 awk そのもの**である。現在の awk を検証した run が記録に無い状態は、表の目的に照らして実質的な欠落にあたる。PR 本文は `1766c7b` / run 31663925152 に更新済みなので、記録先の間で食い違ってもいる。

実害は限定的（本レビューで `1766c7b` の awk を run 31663925152 のログに対して再現し、期待集合と一致することを確認済み）。行を 1 つ足せば閉じる。

なお `HOOKS.md` 側は「測定したのは `9675b2f`」「現在の3列はすべて run 31662960664 の観測である」と出典を明示しており、こちらは正確。SKIP 行そのものは awk の変更に影響されないので、HOOKS.md の 3 列は現在も有効。

### [W-002] `.thread/10/testing.md` 確認項目3 — 期待結果がログから確認できない

期待結果 3 が「ログに `1.89` が出ており、`rustc --version` が `1.89.0` を示す」としているが、msrv ジョブは MSRV toolchain の `rustc --version` を出していない（stable ジョブと fmt ジョブは出している）。手順どおりに確認しようとするとログに無いものを探すことになる。

偽陰性ではない。`rustup toolchain install "$MSRV"` が成功し `cargo "+$MSRV" build` が成功している以上、別の toolchain へ黙ってフォールバックする経路は無い（rustup は未導入の toolchain 指定でエラーになる）。手順書と実装のどちらを合わせるかは選べる — msrv ジョブに `rustc "+$MSRV" --version` を 1 行足すと、AC-4 が求める「宣言が実測で裏付けられた」ことがログ単体で読めるようになる。

### [W-003] `crates/pulsen/src/util/atomic.rs` — 壁時計依存アサートが CI のフレーク源になりうる

`時間で解けない拒否に再試行の予算を使わない` は `read_atomic` / `rename_atomic` の所要時間が `retry_budget()`（511ms）未満であることをアサートする。分類が働いていることを時間で示す設計は妥当だが、判定の基準が壁時計である以上、ウイルス対策スキャン下の Windows ランナーで `fs::read`(NotFound) が 511ms を超えると**原因が実装側に無い赤**になる。

同種のものとして `置換が一時的に拒まれても上限内に解ければ置き換わる` 系 3 件がある（阻害要因の除去が 20ms、予算が 511ms なので余裕は 25 倍）。こちらは余裕が大きく、`読み手は旧内容か新内容のどちらかだけを観測する` も Rust の `File::open` が `FILE_SHARE_DELETE` を含むため書き手が読み手に阻まれにくく、緑の 4 run すべてで再現している。

現時点で観測されたフレークは無く、マージをブロックする材料ではない。将来 CI が「原因不明の Windows 赤」を出したときに、まずここを疑う先として記録しておく。

### [W-004] `.github/workflows/ci.yml` — `actions/checkout` の資格情報が同ジョブ内のテストコードから読める（実害ほぼ無し・任意）

`actions/checkout` は既定で `persist-credentials: true` として `GITHUB_TOKEN` を `.git/config` に書き込む。同じジョブで `cargo test` が PR のコードを実行するので、テストコードから読み出せる状態にある。

public リポジトリかつ `permissions: contents: read` なので、読み出せてもフォークからの PR で得られるのは匿名でも取れる読み取り権限だけで、実害はほぼ無い。ただし AC-7 と ADR-001 が「守るべき面を 1 つに絞る」という基準を明示的に立てている構成なので、`persist-checkout` の設定で面をもう 1 つ削れることは記しておく（`with: persist-credentials: false`。このワークフローは checkout 後に git の書き込み操作をしないので副作用が無い）。

採否は方針判断。ADR-001 が「メジャータグ固定・SHA ピンは第三者製 Action を足すときにまとめて」と決めているので、同じタイミングで一緒に見直す形でもよい。

---

## カバレッジ

変更ファイル 18 件と 1 対 1 で対応する。

| # | ファイル | 状態 | 備考 |
|---|---|---|---|
| 1 | `.github/workflows/ci.yml` | **精査** | 全 283 行を読み、awk を3実装で実 CI ログに対して再現、シェルの 4 主張を同フラグの bash で検証、`if:` / `uses:` / `${{ }}` / 禁止語を機械確認。本観点の中心 |
| 2 | `.thread/10/adr.md` | **精査** | ADR-001〜013 を全文読み、ci.yml の実装と突き合わせ。ADR-001（Action と前提検査の対象）・ADR-004（stable 非固定）・ADR-005（判定と可視化の分離・行頭に錨を打たない・3状態表示）・ADR-006（`--locked` の範囲と `cargo fmt` の例外）・ADR-007（ジョブ分割と `defaults.run.shell`）・ADR-009（使わない道具の綴りを残さない）はいずれも実装と一致。ADR-008 の撤退条件は発動していない（7ジョブ維持） |
| 3 | `.thread/10/plan.md` | **精査** | AC-1 / 2 / 3 / 4 / 6 / 7 とスコープ・リスク欄を全文読み、担当分の AC をすべて検証（上表）。スコープ除外（キャッシュ・`rust-toolchain.toml`・`.gitattributes`・第三者製 Action・dependabot 等）が実装で守られていることも確認 |
| 4 | `.thread/10/progress.md` | **精査** | 全 72 行。W-001 |
| 5 | `.thread/10/steps.md` | **部分確認** | ステップ3 の期待集合（L212 / L216-218）と区間除外の設計（L170-171）を突き合わせに使用。実装手順そのものは他観点の担当として通読していない |
| 6 | `.thread/10/testing.md` | **精査** | 全 220 行。記載された grep / Psych / ログ確認手順を実際に実行して期待結果と照合。W-002 以外は手順どおりの結果が出る |
| 7 | `crates/pulsen-conformance/HOOKS.md` | **精査（差分）** | 3列の実測記録と「3ランナーでの実測」節を、CI ログから再構成した SKIP 集合と突き合わせ。unix 1件 / Windows 11件・テストバイナリ15本・除外3件の記述はすべて実測と一致。出典 run と測定コミットの明記も適切 |
| 8 | `crates/pulsen/src/util/atomic.rs` | **精査（差分）** | CI 観点で確認: MSRV 1.89 との整合（`NonZeroU32::new().expect()` の const 評価、`thread::scope`、`Duration` 演算 — 3 OS の msrv ジョブが緑で裏付け済み）、テストの実行時間（打ち切り系3件が各 511ms、合計約1.5秒で許容）、フレーク要因（W-003）。ドメイン設計面の妥当性は util 観点の担当 |
| 9 | `crates/pulsen/src/adapter/task_repository.rs` | **精査（差分）** | `fs::read` → `read_atomic` の3箇所。CI 観点では新規の OS 依存・`cfg` 追加が無いこと、clippy `-D warnings` が3 OS で緑であることを確認 |
| 10 | `crates/pulsen/src/adapter/task_file.rs` | **精査（差分）** | フィクスチャの `MAIN_SEPARATOR` 化。`cfg` ではなく実行時判定で組んでおり、Windows で 5 件が緑に転じたことを CI ログで確認 |
| 11 | `.thread/10/review/review-001.md` | **意図的にスキップ** | 前ラウンドの記録。「前回の指摘を前提にせずゼロベースで」という本ラウンドの指示に従い、判断を汚染しないため読んでいない |
| 12 | `.thread/10/review/review-001-ci.md` | **意図的にスキップ** | 同上 |
| 13 | `.thread/10/review/review-001-util.md` | **意図的にスキップ** | 同上 |
| 14 | `.thread/10/review/review-001-test-docs.md` | **意図的にスキップ** | 同上 |
| 15 | `.thread/10/review/review-002.md` | **意図的にスキップ** | 同上 |
| 16 | `.thread/10/review/review-002-ci.md` | **意図的にスキップ** | 同上 |
| 17 | `.thread/10/review/review-002-util.md` | **意図的にスキップ** | 同上 |
| 18 | `.thread/10/review/review-002-test-docs.md` | **意図的にスキップ** | 同上 |
| 19 | `.thread/10/review/triage.md` | **意図的にスキップ** | 同上 |

（`git diff --name-status` が挙げる 18 件に `review-002.md` を加えた 19 行。`review-002.md` はディレクトリに存在するが `--name-status` の一覧に載っていないため、念のため同じ扱いで記載する。）

---

## 結論

**CI・ビルド基盤の観点でマージをブロックする問題は無い。**

担当した AC-1 / AC-2 / AC-3 / AC-4 / AC-6 / AC-7 はすべて満たされている。特に AC-6 は、スキップ報告 awk を実 CI ログに対して mawk / BWK awk / gawk の 3 実装で再現し、3 OS の出力が steps.md に**実行前から書かれていた**期待集合と一致することを確認した。観測値を期待値へ書き写す形にはなっていない。

残る 4 件はいずれも記録の鮮度・確認手順の追随性・将来のフレーク要因・任意のサプライチェーン強化で、CI の判定能力そのものには影響しない。W-001 は 1 行の追記で閉じるので、マージ前に直せるなら直しておくと記録として完結する。
