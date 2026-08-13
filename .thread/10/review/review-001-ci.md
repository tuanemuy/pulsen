# レビュー 001 — CI・ビルド基盤

**PR:** #12 / ベース `main` / ブランチ `issue/10/ci-msrv-cross-platform`
**契約:** `.thread/10/plan.md`
**観点:** CI・ビルド基盤（偽陰性・シェルの落とし穴・依存の抜け・サプライチェーン・実行時間・コメントの真偽）

---

## 総括

**緑の構造そのものは健全である。** `continue-on-error` は無く、`|| true` は 1 箇所だけで負荷のかかるコマンドには掛かっておらず、ステップの `if:` はどれも「先行の失敗を隠さない」側に倒れている。7ジョブが緑ならば「3 OS で build / test / clippy が実際に走って通り、fmt が現行 stable で通り、MSRV toolchain で 3 OS がリンクまで通った」ことを意味する — この含意は成立している。Blocker は無い。

一方、**赤のときの情報量と、緑の可視化が依存している前提の守り方**に穴がある。指摘は全てそこに集中している。特に W-001 は「1回の実行で全部の指摘を揃える」というこのワークフロー自身の設計原則（マトリクスの `fail-fast: false`、clippy の `if: success() || failure()`）が、テストターゲット間だけ適用されていないという内部矛盾で、しかも本 PR の作業中に実際にコストとして顕在化している（ADR-012 が「未観測」を根拠に推測で修正せざるを得なかった）。

---

## 前提の検証（このレビューで実測したもの）

指摘の根拠になるシェル挙動は、思い込みで書かず手元の bash で実測した。

| 検証 | 結果 |
|---|---|
| `set -eo pipefail` 下で `if [ "$(grep -c zzz f)" = "0" ]`（`\|\| true` 無し） | **ステップは死なない**。`if` の条件内では errexit が効かず、`[` の引数中のコマンド置換の終了コードは捨てられる |
| `set -eo pipefail` 下で `uid=$(存在しないコマンド)`（単独の代入文） | **exit 127 で死ぬ**。代入形には errexit が効く |
| `set -eo pipefail` 下で `if [ "$(存在しないコマンド)" -eq 0 ]` | `[: : integer expected` を出したうえで**偽として扱われ、スクリプトは生存する** |
| `cargo metadata` は `rust-version` に何を通すか | `1.89` は通る。`1.89.0-beta.1` / `1.89.0+meta` / `1.*` / 引用符やセミコロンを含む文字列は `error: expected a version like "1.32"` で**拒否される** |
| ワークスペース内の doctest | フェンスは `crates/pulsen-conformance/src/lib.rs` の 2 組のみで、いずれも ` ```text `。`Doc-tests` セクションは出ない（区間判定の抜けを検討したが該当なし） |
| 禁止語の grep（ADR-009） | `sort` 0 件 / `container:` 0 件 / `--test ` 0 件 / `continue-on-error` はコメント1行のみ。ADR-009 の意図どおり成立している |

---

## Blockers

なし。

---

## Warnings

### **[W-001]** `cargo test` に `--no-fail-fast` が無く、赤の実行では最初に失敗したテストターゲット以降が一切走らない — 往復が増えるだけでなく、SKIP サマリーが黙って不完全になる

- 場所: `.github/workflows/ci.yml:125`（および 3 状態を宣言している `:141-143` のコメント）
- 理由:
  - `cargo test` は既定でテストターゲット単位の fail-fast であり、1 バイナリが落ちた時点で残りのバイナリを走らせない。**この PR の実作業でそれが実害として出ている** — `.thread/10/progress.md:27` と ADR-012 が書くとおり、初回の Windows は `-p pulsen --lib` の 6 件で停止し、適合スイートは一度も走らなかった。結果 `TC-port-task-repository-044` は「赤でない」のではなく「未観測」となり、ADR-012 は実測なしの推測で `rename_atomic` を直すことになった。ADR-010 が明示的に避けた「想定で変える」に踏み込んだ原因は、CI の設定 1 語である。
  - ワークフローは同じ「失敗を隠さない」原則をマトリクス（`fail-fast: false`、`:73` / `:180`）と clippy（`if: success() || failure()`、`:172`）では徹底しているのに、テストターゲット間だけ既定のまま抜けている。`:169-171` のコメント「1回の実行で「動くか」と「綺麗か」の両方の指摘が揃わないと…往復が増える」は、テストバイナリ間には適用されていない。
  - さらに**可視化の偽陰性**になる。SKIP サマリーは「到達していない / 走っていない / 走ったが 0 件」の 3 状態に分けると `:141-143` のコメントが宣言しているが、実際には**第 4 の状態「途中で打ち切られた」が存在し、それは 3 状態のどれにも分類されず「走った」扱いで部分的な SKIP 一覧が出る**。読み手には完全な一覧と区別が付かない。AC-6(d) の撤退時の読み替え（「Windows は最後に赤で終わった実行のサマリーを採る」）は、まさにこの切り詰められた一覧を突き合わせ対象にすることになる。
- 提案: `run: cargo test --workspace --locked --no-fail-fast -- --nocapture 2>&1 | tee test.log`。`--no-fail-fast` でも失敗時の終了コードは非ゼロのままなので、判定は緩まない（偽陰性を新たに作らない）。これにより第 4 の状態が構造的に消え、`:141-143` のコメントが実際に 3 状態を尽くすようになる。採らない場合は、コメントから「3状態に分ける」という網羅の主張を落とし、「打ち切られた実行では一覧が不完全になりうる」ことを明記すべき。

### **[W-002]** 非 root アサートが `if [ "$(id -u)" -eq 0 ]` の形をしており、`id` が壊れていると**無言で通る**

- 場所: `.github/workflows/ci.yml:98-103`
- 理由:
  - `if` の条件内では errexit が効かない。`id` が不在・非数値を返した場合、`[ "" -eq 0 ]` は `integer expected` を出して **exit 2 = 偽**となり、`then` 節に入らないままステップが exit 0 で成功する（実測済み。上表参照）。root で走っていても検出されない、ではなく「検査そのものが空振りしたことすら分からない」形である。
  - ADR-005 はこのアサートを **CI が独自に持つ唯一の合否判定**と位置づけ、「前提の検査は観測結果と独立なので判定が循環しない」ことを根拠に SKIP の件数判定を捨てている。その唯一の柱が、失敗時に fail-open する構文で書かれている。
  - AC-7 の「前提とするランナー同梱ツールは各ジョブ先頭の前提検査ステップで確認し、無ければその場で失敗する」という原則が、**このアサートを支えている `id` にだけ適用されていない**。`awk` / `cat` / `grep` / `tee`（記録用）は検査されているのに、`id`（判定用）は検査されていない。重要度の順序が逆転している。
- 提案: 代入形に分けて errexit を効かせる。

  ```bash
  uid=$(id -u)          # 代入形なら id の失敗でステップが落ちる（実測: exit 127）
  echo "uid=$uid"
  if [ "$uid" -eq 0 ]; then
    echo "::error::root で実行されている。SkipBudget の権限系の宣言が崩れる" >&2
    exit 1
  fi
  ```

  あわせて `:88` のツール一覧に `id` を加える（このジョブが実際に使う道具に揃える、という ADR-001 の基準にそのまま従う）。

### **[W-003]** `|| true` に付いた why コメントが、その位置では成り立たない理由を述べている

- 場所: `.github/workflows/ci.yml:145`（該当コード `:153`）
- 理由:
  - コメントは「grep の「該当なし」は `-e` の下でステップの失敗になるため、数え上げは `|| true` で切り離す」と書くが、実際には `if [ "$(grep -c ...)" = "0" ]` の位置で errexit は効かない（実測済み）。`|| true` を外してもステップは死なない。つまり **`|| true` はこの位置では完全な no-op であり、コメントはその存在理由を誤って正当化している。**
  - 同じ誤解が `.thread/10/steps.md`（「設計」節と ステップ3 の「`-e` の扱いを明示する」）に元からある。一方 MSRV ジョブ側の `:209`「grep -c は該当なしで exit 1 を返すが、case の被検査式に置けば -e の対象外になる」は**正しい**。同一ファイル内に正しい記述と誤った記述が並んでいるため、読み手は誤った側を「検証済みの一般ルール」と受け取りやすい。
  - 実害は今は無いが、この誤ったルールが一般化されると、`|| true` が本当に失敗を握り潰す位置（単独のパイプライン、代入形）に付く。本 Issue が壊しにいったのは「緑が何も保証していない」状態なので、握り潰しの根拠が誤って共有されているのは残しておきたくない。
- 提案: `|| true` を外してコメントも削るのが最も素直（挙動は変わらない）。残すなら理由を実態に合わせる — 「`if` の条件・`[` の引数ではコマンド置換の終了コードが捨てられるので errexit は効かない。`grep -c` が該当なしで返す exit 1 を明示的に無害化しておくための冗長な保険」。加えて steps.md の該当記述も直さないと、次に grep を足す人が同じ誤解を再生産する。

### **[W-004]** `${{ steps.msrv.outputs.version }}` を `run:` に直接展開している（`pull_request` トリガー）

- 場所: `.github/workflows/ci.yml:222`, `.github/workflows/ci.yml:228`
- 理由:
  - `${{ }}` はスクリプトのテキストとして展開されるため、値が制御できると任意コマンド実行になる古典的な経路である。このワークフローは `pull_request` で起動し、値の出所はフォークが自由に書き換えられる `Cargo.toml` である。
  - **現時点で悪用可能ではない。** 実測したところ cargo は `rust-version` を `X[.Y[.Z]]` の数値形にしか通さず、引用符・セミコロンを含む値は `error: expected a version like "1.32"` で拒否される。つまり安全性は `cargo metadata` のパーサという**外部の実装詳細に暗黙に依存**していて、ワークフローにもレビューにも根拠が残っていない。cargo が将来 `1.89-nightly` のような形を受理する方向に緩めれば、その日から穴になる。
  - なお `permissions: contents: read` と secrets 不使用により、仮に踏んでも public リポジトリの読み取り権限しか渡らない。**影響は小さい。それでも修正コストが 2 行なので、暗黙の依存を残す理由がない。**
- 提案: 環境変数経由に落とす。

  ```yaml
  - name: MSRV の toolchain を用意する
    env:
      MSRV: ${{ steps.msrv.outputs.version }}
    run: rustup toolchain install "$MSRV" --profile minimal --no-self-update

  - name: MSRV でビルドする
    env:
      MSRV: ${{ steps.msrv.outputs.version }}
    run: cargo "+$MSRV" build --workspace --all-targets --locked
  ```

  （`cargo "+$MSRV"` は cargo が `+` 始まりの第1引数を toolchain 指定として解釈するので成立する。）

### **[W-005]** SKIP サマリーの除外が「`pulsen_conformance-` バイナリの区間まるごと」なので、除外が広がったことに気づけない

- 場所: `.github/workflows/ci.yml:159`
- 理由:
  - 落としたいのは `SkipBudget` 自己テストの架空ケース 3 件だが、実装は「そのテストバイナリが出した SKIP 行を全部」落としている。将来 `pulsen-conformance` の lib テストに実在ケースの SKIP が 1 件でも入れば、**サマリーから黙って消える**。可視化しか役割を持たない機構なので、消えたことを他の手段で検出できない。
  - ADR-005 は「cargo の出力形式に依存する」ことをトレードオフとして挙げているが、「除外が広すぎる」面は挙げていない。現状は架空 3 件しか無いので実害ゼロ。
- 提案: 除外した行数をサマリーに出して、増減が目に入るようにする。awk 側で落とした件数を数え、`（除外: SkipBudget 自己テスト N 件）` を見出しの下に 1 行足すだけで、3 から動いたことが人の目に留まる。区間判定そのものは（ケース名での除外より腐りにくいので）変えなくてよい。

---

## 受け入れ基準の判定（自分の観点に関わるもの）

| # | 判定 | 根拠 |
|---|---|---|
| AC-1 | **満たす** | `push`(main) / `pull_request` / `workflow_dispatch`（`:3-7`）、`permissions: contents: read`（`:9-10`）、`concurrency` の `cancel-in-progress` が PR 限定（`:15-17`）、`defaults.run.shell: bash` がワークフロー直下に 1 件でステップごとの `shell:` は 0 件、ジョブは fmt(1)+test(3)+msrv(3)=7。ADR-008 の撤退は適用されていないので読み替え不要 |
| AC-2 | **満たす** | `:113` build / `:125` test / `:173` clippy が 3 OS で走り、`fail-fast: false` が両マトリクスにある。`continue-on-error` は使われていない（ヒットはコメント1行のみ） |
| AC-3 | **満たす** | `:59-63` で `rustup update stable --no-self-update` → `rustup default stable` → `rustup component add rustfmt` を踏み、`rustc --version` と `cargo fmt --version` をログに残してから `:67` で `--all --check`。`rustfmt.toml` に版固定・抑止の追加は無い（本 PR の変更ファイルに含まれない） |
| AC-4 | **満たす** | `:211-218` が `cargo metadata --no-deps --locked` の**全メンバー**の `rust_version` を対象にし、0 件と複数種類をどちらも失敗にする。ワークフローに版数のハードコードは無い。`--all-targets --locked` の `build`（`:228`）。`rust-version` の変更は本 PR に無いので `.adr/022` / `.adr/023` の整合条件は発火しない |
| AC-6 | **満たす（ただし W-001 / W-002 の留保付き）** | (a) `:92-103` に非 root アサートあり（構文の弱さは W-002）。(b) `:147-166` が 3 状態を分けて列挙し、`pulsen_conformance-` 区間を除外（除外の広さは W-005、第4の状態は W-001）。(c) `container:` / `--test ` ともに 0 件、`sort` も 0 件。(d) HOOKS.md と progress.md の実測が事前の期待集合（unix 1 件 / Windows 11 件）と一致し、期待値の事後書き換えは発生していない |
| AC-7 | **満たす** | `uses:` は `actions/checkout@v7` の 3 箇所のみで第三者製 Action は 0。前提検査は fmt=`rustup`、test=`rustup` + `awk`/`cat`/`grep`/`tee`、msrv=`rustup`/`cargo`/`jq` の版表示 + `grep` の `command -v` と、AC-7 の指定に一致。`sort` は 0 件。ただし判定を担う `id` が検査対象から漏れている（W-002） |

### スコープ逸脱の検査

**逸脱なし。** 「含まれないもの」に挙がっている項目が 1 つも混入していない — `target/` キャッシュ無し、`actions/cache` 無し、`cargo audit` / `deny` / dependabot / カバレッジ / release 無し、Nix devShell ジョブ無し、`rust-toolchain.toml` 無し、`.gitattributes` 無し、pre-commit / Makefile / xtask 無し、ブランチ保護の設定変更無し。変更ファイル 9 件はいずれも AC-1〜AC-8 のどれかに直接紐づく。

コード側 2 件の吸収先も ADR-008 の境界内に収まっている — `crates/pulsen/src/util/atomic.rs`（共通ユーティリティ、`persist` を使う置換手順を保ったまま再試行とエラー分類を足す＝「本 Issue で扱う」側の明示例）、`crates/pulsen/src/adapter/task_file.rs`（アダプター層のテストフィクスチャのみ、プロダクションコードのシグネチャ・契約は不変）。`crates/pulsen-domain/` は 1 行も触っていない。

---

## 指摘に至らなかった検討（ノイズにしないため、判断だけ残す）

理由付きで「問題なし」に倒したもの。次に読む人が同じ検討を繰り返さないよう記録する。

- **`actions/checkout` に `persist-credentials: false` を付けていない。** 既定では GITHUB_TOKEN が `.git/config` に残り、`cargo build` が走らせる依存クレートの build.rs / proc-macro（＝第三者コード）から読める。ただしこのワークフローの token は `contents: read` のみ、リポジトリは public、secrets は 1 つも使っていないので、漏れても得られるのは「誰でも取れる読み取り権限」である。付けるコストは低いが、これを指摘に格上げすると「public + read-only + secrets 無し」という前提を無視した機械的な指摘になるため見送る。
- **`paths-ignore` によるドキュメント専用 PR のスキップ。** 本 PR 自体 9 件中 6 件がドキュメントで、7 ジョブがフルビルドしている。ただし plan.md がマージ後に required checks を設定する前提を置いており、`paths-ignore` は required checks を pending のまま止める既知の落とし穴を持ち込む。足さない判断は妥当。
- **`concurrency` グループが `github.event_name` を含まない。** main への push と main での `workflow_dispatch` が同一グループになるが、`cancel-in-progress` が false なので打ち切りではなく直列化にしかならない。実測履歴を残すという目的（`:12-14`）は損なわれない。
- **`grep -c '^test result:'` を ANSI 除去前の生ログに掛けている。** libtest の `test result:` 行は行頭が着色されない（色が付くのは後続の `ok` / `FAILED`）ため、`CARGO_TERM_COLOR: always` の下でもアンカーは効く。`SKIP ` 行はテスト側の `println!` なので同様。ADR-005 の「パイプ越しには色が付かない」という書き方は厳密ではないが、結論は正しい。
- **`Doc-tests` セクションによる区間判定の抜け。** `Doc-tests` 行は `Running` に一致しないため `drop` が持ち越される経路を検討したが、ワークスペースのフェンスは 2 組ともに ` ```text ` で doctest が 1 件も存在しないため発生しない。
- **`awk` の重複除去 `!seen[line]++` が除外区間の行で汚染されないか。** `&&` の短絡により `drop` が真のときは `seen[line]++` が評価されないため、架空ケースが実在ケースを先に消す事故は起きない。順序は正しい。
- **`$GITHUB_OUTPUT` / `$GITHUB_STEP_SUMMARY` への Windows パス追記。** Git for Windows の bash は `D:\a\_temp\...` 形式をそのまま open できるうえ、二重引用符内のバックスラッシュはエスケープとして解釈されない。実際に run 31657976822 の Windows ジョブが緑なので実証済み。
- **MSRV ジョブが `rustc --version` を明示しない。** testing.md 項目 3 の期待結果は `rustc --version` が `1.89.0` を示すことだが、ワークフローにその step は無い。ただし `rustup toolchain install` の完了行が `... installed - rustc 1.89.0 (...)` を出し、`cargo +<版数>` は未導入なら失敗するため、「その版で実際にビルドされた」ことはログから確定できる。実質の穴ではない。
- **`.thread/10/` をコミットに含めること。** `git ls-files .thread` に `.thread/1/{plan,adr,steps,testing,progress}.md` と `.thread/1/manual-test/result.md` が既に入っており、ファイル構成もサブディレクトリの使い方も既存の運用と一致する。逸脱なし。なお `.thread/1/` には `review/` が無いので、本レビューファイルをコミットするなら**新しい前例を作る**ことになる点だけ意識されたい（含める / 含めないのどちらでも、運用として一貫していればよい）。

---

## カバレッジ

一覧 9 件すべてに対応する（確認 9 / スキップ 0）。

- 確認: `.github/workflows/ci.yml` — 本レビューの主対象。全 228 行を読み、`if:` 条件・終了コードの伝播・シェル挙動・コメントの真偽を 1 つずつ突き合わせた
- 確認: `.thread/10/plan.md` — 契約。AC-1 / 2 / 3 / 4 / 6 / 7 と「含まれないもの」を判定基準に使用
- 確認: `.thread/10/adr.md` — ADR-001 / 004 / 005 / 006 / 007 / 008 / 009 の設計判断と ci.yml の実装が一致するかを照合（ADR-009 の禁止語 3 種は grep で 0 件を実測）
- 確認: `.thread/10/steps.md` — ワークフローの設計意図と実装の差分を確認。W-003 の誤解の出所がここにあることを特定
- 確認: `.thread/10/testing.md` — 検証手順とワークフローの実装が対応しているかを確認（項目 3 の `rustc --version` のみ手順とステップが一致しないが実質の穴ではない）
- 確認: `.thread/10/progress.md` — 初回赤 → 2 回目緑の経緯と、W-001 の実害（適合スイート未観測）の裏取り
- 確認: `crates/pulsen-conformance/HOOKS.md` — AC-8 の実測記録が 3 OS 分揃い、期待集合（unix 1 / Windows 11）と数が整合することを確認
- 確認: `crates/pulsen/src/util/atomic.rs` — **CI 観点に限定**。ADR-008 の吸収先ディレクトリに収まっているか、`cfg` の決め打ちではなく分類関数に閉じているか、ユニットテストが Windows 以外でも打ち切りを検証できる形か（`is_transient` を引数化した seam）を確認。再試行アルゴリズム・上限値・バックオフの妥当性そのものは Rust 実装観点のレビューに委ねる
- 確認: `crates/pulsen/src/adapter/task_file.rs` — **CI 観点に限定**。変更がテストフィクスチャに閉じ、`MAIN_SEPARATOR` を見る既存の作法（ADR-037）に揃っており、プロダクションコードの契約に触れていないこと（＝ADR-008 の境界内）を確認。期待値の `<repo>` 差し込みが整形の検査を緩めていないことも確認した
- スキップ: なし
