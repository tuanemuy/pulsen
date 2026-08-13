# 065: CI は GitHub Actions に置き、toolchain の出典をランナー同梱の rustup に一本化する

## ステータス

承認済み

## コンテキスト

CLAUDE.md は「安定版ツールチェーンを前提とし、`cargo fmt` / `cargo clippy` を通る状態を保つ」「特定の OS に依存しない(Linux / macOS / Windows)」と定めるが、検証環境は Nix flake の devShell 1つだけで、rustc は nixpkgs-unstable の 1.97.1、実機は macOS のみだった。Windows を含む3 OS を回せる CI が要る。

CI プロバイダの選択肢:

1. **GitHub Actions** — リポジトリは GitHub の public。3 OS のホストランナーが無料で使え、PR のチェック統合が標準。
2. GitLab CI / Circle CI 等の外部サービス — リポジトリのホストと分離し、アカウントと連携の管理が増える。Windows ランナーは有料か自前。
3. self-hosted ランナー — Windows 実機を自前で用意する必要がある。「Windows 実機での挙動が未検証」という状態を解消するには最も遠回り。

toolchain の入手手段:

1. **rustup**(3ランナーとも同梱) — Windows を含む3 OS で同じ手順が通る。MSRV 用の別バージョン導入も `rustup toolchain install` の1行。
2. Nix(`nix develop -c ...`) — 開発環境と完全に一致するが、**Windows で成立しない**。Linux / macOS だけ Nix、Windows だけ rustup にすると、CI の中に2種類の toolchain 供給経路が生まれ、赤の原因を切り分けにくくなる。また nixpkgs は「宣言した MSRV の版」を狙って入れる手段としては不向き。
3. `dtolnay/rust-toolchain` 等の第三者製 Action — 広く使われているが、public リポジトリに第三者製 Action を入れるとサプライチェーンの面が増える。ADR-023 は依存の選定を「用途が1〜2モジュールに閉じ、std で足りるものは足さない」という基準で行っており、CI でも同じ基準を当てるなら、`rustup` の3行で済むものに Action を足す理由がない。

toolchain の指定をリポジトリに置く手段として `rust-toolchain.toml` もある。ローカルでも同じ版が使われるという利点があるが、このプロジェクトの開発環境は Nix flake の devShell で、rustup は入っていない(`which rustup` → not found)。`rust-toolchain.toml` は rustup のシムが読むファイルなので、devShell の中では完全に無視される。置いた場合、「ローカルでは nixpkgs の 1.97.1、CI では `rust-toolchain.toml` の版」という二重の出典になり、どちらが正かがファイルからは読み取れない。

## 決定

GitHub Actions を使う。toolchain はランナー同梱の `rustup` を直接叩いて用意し、CI が使う Action は GitHub 公式の `actions/checkout` **だけ**にする。

- stable ジョブ: `rustup update stable --no-self-update` → `rustup default stable` → `rustup component add clippy`
- fmt ジョブも同じ手順を踏む(ADR-067 の「常に現行 stable」は整形にも掛かる)。導入する component が `rustfmt` である点だけが違う
- MSRV ジョブ: `rustup toolchain install <版数> --profile minimal --no-self-update` → `cargo +<版数> ...`(ADR-066)
- **`rust-toolchain.toml` は置かない。** ワークフローが `rustup default stable` と `cargo +<rust-version>` で明示し、ローカルは `flake.nix` が唯一の出典であり続ける
- Nix devShell は開発環境として残し、CI では使わない。CI と devShell で rustc の版が違うことは受け入れる — むしろ「1つの版でしか通らない」状態から抜けることが CI 導入の目的である

`actions/checkout` は**唯一の例外として残す Action** なので、固定方法も同じサプライチェーン基準で決める。**現行のメジャータグ(`actions/checkout@v7`)で固定する。** 固定するのはメジャーだが、**古いメジャーに留まり続けることは固定ではなく放置である** — `actions/checkout` の旧メジャーは Node ランタイムの世代とともに非推奨になり、いずれ警告付きで動く状態になる。導入時点の現行メジャーを採り、以後はその非推奨の警告を更新の合図とする。SHA ピンは改竄への耐性が一段上だが、更新のたびに SHA を手で追う運用が要る。GitHub 公式 org のリポジトリはアカウント侵害の面が第三者製より小さく、他に Action を持たない構成では守るべき面が1つしかないので、メジャータグの安定性と読みやすさを採る。ブランチ参照(`@main`)や暗黙の最新は取らない。

ランナー同梱の前提は、各ジョブの先頭で**その場で落ちる**形に検査する。対象は「ワークフローがそのジョブで実際に使う道具」に揃える — `rustup`(全ジョブ)、`cargo` / `jq` / `grep`(MSRV ジョブ)、`awk` / `cat` / `grep` / `tee`(stable ジョブのスキップ報告)、`id`(stable ジョブの非 root アサート)。`id` を落とさないのは、それが CI の独自の合否判定(ADR-068)が乗る唯一の道具だからである。記録にしか使わない道具を検査して、判定に使う道具を検査しない理由がない。前提が崩れたときに `cargo` の不可解な失敗として現れるより、崩れた前提そのものが失敗として現れるほうが切り分けが早い。前提が実際に崩れた場合の第2手は、第三者製 Action を足すことではなく `curl https://sh.rustup.rs -sSf | sh` を自前で叩くこと。

`cargo` を MSRV ジョブにだけ入れるのは、そのジョブだけが**自分で toolchain を用意する前に** cargo を使うからである。fmt / stable ジョブは `rustup default stable` を踏んでから cargo を叩くので存在が保証されるが、MSRV ジョブは `rustup default` を一度も呼ばず、ランナーに既定 toolchain が設定済みであることに暗黙に依存して `cargo metadata` で版数を読み出す。既定が無ければ rustup のシムがそこで落ち、それが「版数の読み出しの失敗」に化ける。

検査の形は道具によって分ける。`rustup` / `cargo` / `jq` は `--version` で版まで表示する(版がログに残る利点がある)。`awk` / `cat` / `grep` / `id` / `tee` は `command -v` で存在と解決先パスだけを見る — **`--version` は移植性が無く**、macOS の BSD `tee` は `illegal option -- -` で落ちる(実測)。解決先パスを出すのは、Windows で Git for Windows の `/usr/bin/<tool>` ではなく `C:\Windows\System32\` 側の同名プログラムを掴んでいないかがログから分かるため。

**`sort` はどのジョブでも使わない。** `command -v` は存在すれば成功するので、間違ったプログラムを掴んでいても前提検査は通ってしまう。Windows の PATH 解決が `C:\Windows\System32\sort.exe` に当たった場合、それは `-u` を受け付けずに非ゼロで終わる。スキップ報告の抽出は `-e` の下で落ちないよう終了コードを切り離してあるので、**パイプライン全体が空になり「なし」という嘘のサマリーが出る** — 11件スキップしている Windows で「走らなかったケースは無い」と表示されるのは、可視化としては失敗そのものである。重複除去は区間判定で既に使っている awk に寄せ(`!seen[$0]++`)、`sort` への依存自体を持たない。道具を1つ減らすことが、掴み違いを検査で捕まえるより確実で安い。

## 影響

- 3 OS が同一の手順で回る。第三者製 Action がゼロなので、ワークフローの信頼境界が GitHub 本体だけに閉じる。MSRV の toolchain 切り替えが1行で済む
- 「ローカルは flake.nix、CI はワークフロー」と出典が1対1で対応する。無視されるファイルを置かずに済む
- ランナー前提の崩れが fail-fast で見える。第三者製 Action を持たない方針を守ったまま、逃げ道(rustup の自前導入)が用意されている
- トレードオフ: CI の toolchain と devShell の toolchain が一致しない。ローカルで緑でも CI で clippy が落ちる(またはその逆)ことが起きうる。これは検出したい種類のズレなので、一致させる方向には倒さない
- トレードオフ: rustup を使う開発者(Nix を使わない人)がリポジトリを clone しても、toolchain の指定を受け取れない。現状そういう開発者はいないため受け入れる
- トレードオフ: `actions/cache` や `Swatinem/rust-cache` が提供する高速化を捨てる(ADR-069)
- トレードオフ: メジャータグ固定なので `v7` の内部が動いたことは検知できない。SHA ピンへ上げる判断は、第三者製 Action を足すことになったときに全体でまとめて行う
