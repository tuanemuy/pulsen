# ADR: ジョブは fmt 1つ + OS マトリクス2組に分け、clippy は test ジョブへ同居させる

## ステータス

承認済み

## コンテキスト

CI が回すのは `build` / `test` / `clippy --all-targets -- -D warnings` / `fmt --check` / MSRV で、いずれも 3 OS のマトリクスが要る(`.adr/10-msrv-read-from-manifest-and-linked-on-three-os.md`)。分割の粒度に選択肢がある。

1. 1ジョブに全部を詰める — マトリクスにすると fmt が3回走る。どのコマンドで落ちたかはログを追えば分かるが、PR のチェック一覧では見えない
2. コマンドごとにジョブを分ける(build / test / clippy を別ジョブ)— チェック一覧は読みやすいが、clippy ジョブが依存を丸ごと再コンパイルする(キャッシュを入れない以上、これは純粋な重複。`.adr/10-no-dependency-cache-locked-dependency-graph.md`)
3. **プラットフォーム非依存なものだけ分離し、同じ toolchain と `target/` を共有できるものは同居させる**

## 決定

- `fmt`(ubuntu 1つ): 現行 stable の rustfmt で `cargo fmt --all --check`。整形結果は OS に依存せず、rustfmt の `newline_style` は既定 Auto なので Windows の CRLF チェックアウトでも結果が変わらない。3回走らせる意味がない
- `test`(3 OS): 前提の検査 → build → test → スキップ報告 → clippy。`target/` を共有できるので clippy が依存を再コンパイルしない。順序は「動くか」→「綺麗か」
- `msrv`(3 OS): `cargo build --all-targets` のみ(`.adr/10-msrv-read-from-manifest-and-linked-on-three-os.md`)
- 全マトリクスに `fail-fast: false` を付ける。1 OS の失敗で他をキャンセルすると、「Windows だけの問題か、全 OS の問題か」を1回の実行で判断できない
- `timeout-minutes` をジョブに付け、ハングを打ち切る
- **`run` のシェルはワークフロー既定(`defaults.run.shell: bash`)で全ジョブ・全ステップに揃える。** 指定の無い `run` は Windows でだけ pwsh になるため、放っておくと同じジョブの中で2種類のシェルが混ざる。`--` の扱い・終了コードの伝播・リダイレクトの意味がステップごとに変わる状態は、Windows の赤を切り分けるときに変数を1つ増やす。bash に寄せれば `-eo pipefail` が全ステップに効き、`tee` を挟むステップだけの特例も要らなくなる。Windows の bash は Git for Windows 同梱のもので、`$GITHUB_STEP_SUMMARY` / `$GITHUB_OUTPUT` への追記も msys のパス変換が効くので支障はない

## 影響

- 合計7ジョブで、PR のチェック一覧が「fmt / 3 OS の stable / 3 OS の MSRV」と読める。重複コンパイルが無い
- トレードオフ: test ジョブが落ちたときに build / test / clippy のどれで落ちたかはログを開かないと分からない。ステップ名で判別できるので実害は小さい
- トレードオフ: `fail-fast: false` により、明らかに全 OS 共通の失敗でも3 OS 分の時間を使う。切り分け情報のほうが価値が高い
