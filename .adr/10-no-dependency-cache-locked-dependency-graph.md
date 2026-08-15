# ADR: 依存のキャッシュは入れず、`--locked` で依存グラフを固定する

## ステータス

承認済み

## コンテキスト

Rust の CI ではキャッシュがほぼ定番になっている。選択肢:

1. `Swatinem/rust-cache` — 事実上の標準だが第三者製 Action(`.adr/10-ci-on-github-actions-with-runner-rustup.md` の方針に反する)
2. `actions/cache`(公式)で `~/.cargo/registry` と `target/` を保存 — キーの設計・陳腐化・サイズ上限の管理が要る。特に `target/` は OS ×ジョブごとに数百 MB になり、古い成果物を引きずった赤の切り分けが難しい
3. `actions/cache` で `~/.cargo` のレジストリだけ — 依存のダウンロード(45クレート)を省ける。効果は10〜20秒程度
4. **キャッシュを入れない**

実測(macOS / rustc 1.97.1)では、クリーンな `cargo clippy --workspace --all-targets` が 55 秒、テスト実行の合計が約 12 秒。依存グラフは45クレートと小さい。1ジョブあたりの所要はチェックアウト・toolchain 導入を含めても数分に収まる。

## 決定

キャッシュを入れない。代わりに **依存グラフに触れる cargo コマンドすべてに `--locked` を付ける** — `build` / `test` / `clippy` / `metadata`。MSRV ジョブが版数の読み出しに使う `cargo metadata --format-version 1 --no-deps` も含める。`--no-deps` は依存解決を行わないので付けなくても結果は変わらないが、依存グラフに触れる側で例外を作ると「どの cargo コマンドに付けるか」を都度判断することになる。

`cargo fmt` にだけは付けない。**受け付けないため**で(`error: unexpected argument '--locked' found`。実測)、方針の例外ではない — `cargo fmt` は rustfmt へのラッパーで依存解決を行わず、`--` の後ろは rustfmt のオプションとして解釈される。

キャッシュの再検討は「stable ジョブの1回が10分を超える」ようになってから行う。そのときは公式の `actions/cache` でレジストリのみを対象にする(`target/` は対象にしない)。

## 影響

- ワークフローが短く、赤の原因がキャッシュの陳腐化である可能性を最初から排除できる。第三者製 Action がゼロという `.adr/10-ci-on-github-actions-with-runner-rustup.md` の方針が保てる
- `--locked` により、CI が検証する依存グラフが `Cargo.lock` と厳密に一致する。`.adr/10-msrv-read-from-manifest-and-linked-on-three-os.md` の MSRV 検証は「このロックファイルの依存グラフが宣言した版でコンパイルできる」という意味になる。ロックファイルの更新漏れは全ジョブの即時失敗として現れる
- トレードオフ: 毎回全依存をダウンロード・コンパイルする。現在の規模では許容範囲で、依存が増えたら再検討する
- トレードオフ: 依存を1つ足すだけの PR でも全ジョブがフルビルドする
