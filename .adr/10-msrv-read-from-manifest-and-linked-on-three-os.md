# ADR: MSRV は `Cargo.toml` の `rust-version` を読み出して固定し、3 OS すべてでリンクまで検証する

## ステータス

承認済み

## コンテキスト

`rust-version = "1.89"` は `.adr/1-std-file-lock-and-lockguard-marker-trait.md`(`std::fs::File::try_lock` の安定化)を根拠に宣言されているが、その版で一度もビルドされていなかった。CI の MSRV ジョブでどう版を決めるかに選択肢がある。

1. ワークフローに `1.89` を直接書く — 単純だが、`Cargo.toml` を上げたときにワークフローの更新を忘れると、CI は古い版を検証し続ける。宣言と検証対象が黙ってズレる
2. **`Cargo.toml` から読み出す** — `cargo metadata --format-version 1 --no-deps` の `rust_version` を `jq` で取り出す。`jq` は3ランナーとも同梱。ズレようがない
3. `rust-toolchain.toml` を置いて rustup に解決させる — MSRV とは別に「既定の toolchain」を宣言するファイルであり、意味が混ざる。加えて Nix devShell(rustup 無し)からは無視されるため、ツールチェーン指定の出典が2つに割れる(`.adr/10-ci-on-github-actions-with-runner-rustup.md`)

読み出す対象にも選択肢がある。3クレートはいずれも `rust-version.workspace = true` で、`workspace.package.rust-version` が唯一の宣言である。`cargo metadata` は継承を解決したうえで3パッケージすべてに `rust_version` を載せるので、`select(.name == "pulsen")` で1パッケージだけを見ても現時点では同じ値が取れる。ただしそれは継承が全員に効いている結果にすぎず、将来どれかが個別に `rust-version` を持てば、CI が検証するのは `pulsen` の値だけになり、他クレートの宣言は一度も実測されない。「唯一の出典」と実装が指すものが割れる。

検証コマンドの範囲にも幅がある。`cargo check --workspace --all-targets`(型検査まで)/ **`cargo build --workspace --all-targets`**(コード生成・リンクまで)/ `cargo test`(実行まで)。宣言が意味するのは「`rust-version` に固定した toolchain でビルドできること」である。`check` はリンク段階でしか出ない失敗(リンカの差、単相化後にのみ現れるエラー)を取りこぼし、それが最も起きやすいのは一度もリンクされたことのない Windows 側コードである。stable ジョブが見ているのは stable でのリンクであって MSRV でのリンクではない。依存45クレート・クリーンビルド55秒という規模なら、`check` から `build` へ上げる追加コストは1ジョブあたり数十秒に収まる。

## 決定

- 版数はワークフローに書かず、`cargo metadata --format-version 1 --no-deps` の**全メンバーの `rust_version`** を読み出す。`null` を除いた値が空なら失敗させ、値が複数種類に割れていても失敗させる(どの版で検証すべきかが一意に決まらない状態を黙って通さない)。一意に定まった値を `rustup toolchain install` と `cargo +<版数>` に渡す
- MSRV ジョブは stable と同じ3 OS のマトリクスで回す。MSRV は言語と std の API 表面の話なので1 OS で足りるという考え方が一般的だが、このプロジェクトには `#[cfg(unix)]` / `#[cfg(not(unix))]` で分岐するコードがある。ubuntu だけで検証すると **Windows でしかコンパイルされないコードの API 要求は一度も検査されない**。ローカルの clippy が `incompatible_msrv` を出さないことも、同じ理由で unix 側のコードについてしか保証していない
- コマンドは `cargo build --workspace --all-targets --locked`。`--all-targets` にするのは、テストと example もその版でコンパイル・リンクできることを宣言の意味に含めるため。テストの**実行**までは stable ジョブが見ているので MSRV では重ねない
- 読み出した版数はワークフローの式展開ではなく `env` 越しにシェルへ渡す。`${{ }}` は `run` のスクリプトにテキストとして展開されるため、フォークが自由に書き換えられる `Cargo.toml` の値をシェルの構文に混ぜることになる。cargo が `rust-version` に数値形しか通さないことへ安全性を預けると、根拠がワークフローの外にしか無い状態になる

## 影響

- `Cargo.toml` を1箇所直せば CI の検証対象が自動的に追随する。MSRV の引き上げがワークフローの変更を伴わない。1クレートだけ MSRV を上げた場合も、割れが失敗として現れるので気づける
- cfg で分岐するコードの MSRV が OS ごとに、しかもリンクまで検査される。`check` では見えない Windows 固有のリンク失敗が MSRV でも捕まる
- トレードオフ: ジョブが3つ増える。public リポジトリなので課金は無いが、実行時間とログの量は増える。`build` にしたぶん `check` より1ジョブあたり数十秒重い
- トレードオフ: MSRV の意味を「コンパイル可能性」ではなく「リンクまで通る」と定義することになる。実際に踏む失敗を捕まえられる側に倒した
- トレードオフ: `cargo metadata` の実行にランナー同梱の stable cargo を使うため、MSRV ジョブが2つの toolchain を触る。読み出しに失敗したら明示的に落ちるようにして、黙って空文字で進まないようにする
