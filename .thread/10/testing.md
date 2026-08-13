# 動作確認計画 — Issue #10: CI を用意して MSRV とクロスプラットフォームを検証する

**Issue:** #10
**作成日:** 2026-08-13

---

## 確認環境

この Issue の変更を確認するために必要な手順のみ記載する（プロジェクト全体のセットアップは省略）。

本 Issue の成果物は `.github/workflows/ci.yml` と、CI が赤にした箇所への条件付き修正である。**確認の主舞台は GitHub Actions の実行結果**であり、ローカルでできるのは「push 前の静的な確認」と「CI が観測した内容の突き合わせ」に限られる。Web UI は無い。

### 検証環境の起動

`.envrc` は `use flake` のみ。direnv がこのリポジトリで許可済みであれば、リポジトリに `cd` するだけで `flake.nix` の devShell（cargo / rustc / clippy / rustfmt / git）に入る。direnv を使わない場合は各コマンドを `nix develop -c <command>` で包む。

```sh
cd /Users/hikaru/github.com/tuanemuy/pulsen_2
cargo --version    # devShell が効いていることの確認
gh --version       # CI の実行結果の取得に使う（gh 2.93.0 を確認済み）
ruby -ryaml -e 'puts Psych::VERSION'   # ワークフロー YAML の構文確認に使う（Psych 3.1.0 を確認済み）
jq --version       # cargo metadata の読み出し確認に使う（jq-1.7.1-apple を確認済み）
```

`gh` / `ruby` / `jq` は `flake.nix` の `buildInputs` に無い。いずれもシステム側のものを使う（Issue #2 の確認でも `jq` を同じ扱いにしている）。

CI が回すのと同じ内容をローカルで先に通しておく（`origin/main` 時点で macOS 上は全緑であることを計画時に実測済み）。

```sh
cargo build --workspace --locked
cargo test --workspace --locked --no-fail-fast
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo fmt --all --check
```

`cargo fmt` には `--locked` を付けない（受け付けない。adr.md ADR-006）。ローカルの rustfmt は nixpkgs の 1.97.1 で、CI の現行 stable とは版が違う。ここが緑でも CI の fmt ジョブが赤になりうる（AC-3・ステップ10 の対象）。

### CI の起動条件

ワークフローは `push`（main）・`pull_request`・`workflow_dispatch` で起動する。**`workflow_dispatch` はワークフローファイルが既定ブランチに到達するまで使えず、`push` は main 限定**なので、マージ前の実行手段は **PR 一択**である（steps.md ステップ5）。ブランチを push しただけでは何も走らない。

CI の実行結果は次で取る。

```sh
gh pr checks <PR番号>
gh run list --branch <ブランチ名> --limit 10
gh run view <run-id>
gh run view --job <job-id> --log        # ジョブのログ全文
gh run view <run-id> --json jobs -q '.jobs[] | "\(.name)\t\(.conclusion)"'
```

ジョブサマリー（`$GITHUB_STEP_SUMMARY` に書いた内容）は `gh run view` のログには出ない。**ブラウザで run のページを開いて読む**。

### デプロイ方法

なし。ワークフローファイルは PR にコミットした時点でその PR 上で有効になり、main へのマージで `push`・`workflow_dispatch` の経路も有効になる。別途のデプロイ手順は存在しない。

---

## 確認項目

### 1. ワークフロー YAML が構文として成立し、ジョブ構成が意図どおり

- **対応する受け入れ基準:** AC-1, AC-7
- **目的:** push する前に、YAML の構文エラーとジョブ構成の取り違えを潰す。
- **手順:**
  1. `ruby -ryaml -e 'y = Psych.load_file(".github/workflows/ci.yml"); puts y.keys.inspect'` を実行する。
  2. `ruby -ryaml -e 'y = Psych.load_file(".github/workflows/ci.yml"); y["jobs"].each { |k, v| puts "#{k}\t#{(v.dig("strategy","matrix","os") || ["-"]).join(",")}" }'` でジョブ名と OS マトリクスを一覧する。
  3. `grep -n "uses:" .github/workflows/ci.yml` で使っている Action を列挙する。
  4. `grep -n "shell:" .github/workflows/ci.yml` を実行する。
- **期待結果:**
  1. 例外を出さずにパースでき、`true`（`on:` が YAML の真偽値として読まれる）・`name`・`permissions`・`concurrency`・`defaults`・`env`・`jobs` が並ぶ。
  2. fmt が1ジョブ（マトリクスなし）、stable と msrv がそれぞれ `ubuntu-latest,macos-latest,windows-latest` の3 OS。**合計7ジョブ**。
  3. `actions/checkout@v7` **だけ**。第三者製 Action が1つも無い。
  4. ワークフロー直下の `defaults.run.shell: bash` の1件だけで、ステップごとの `shell:` 指定が無い。
- **確認ポイント:** `${{ }}` を含む値がフロー記法（`{ }`）で書かれていないこと — 構文エラーになる（adr.md ADR-007 / steps.md ステップ1 の設計規則）。手順1 が例外を投げたらここで止める。

### 2. PR 上で CI が起動し、7ジョブすべてが結果を返す

- **対応する受け入れ基準:** AC-1, AC-2, AC-4
- **目的:** CI が実際に動き、OS マトリクスと MSRV ジョブが揃って走ることを確認する。
- **手順:**
  1. PR を作成（または push）して CI を起動する。
  2. `gh pr checks <PR番号>` で全チェックの一覧を見る。
  3. `gh run view <run-id> --json jobs -q '.jobs[] | "\(.name)\t\(.conclusion)"'` で各ジョブの結論を取る。
  4. 同じ run のページで `fail-fast` の効き方を見る（1 OS が赤のとき、他の OS が cancelled になっていないか）。
- **期待結果:** 7ジョブすべてが `success` または `failure` を返し、`cancelled` が無い。3 OS の stable ジョブが互いに独立して最後まで走る。
- **確認ポイント:** **初回は Windows が赤になる前提**（plan.md リスク）。赤であること自体は失敗ではなく、この確認の目的は「7ジョブが揃って結果を返すこと」。赤の中身は確認項目6 で扱う。

### 3. MSRV ジョブが `Cargo.toml` の宣言を唯一の出典としている

- **対応する受け入れ基準:** AC-4
- **目的:** ワークフローに版数がハードコードされておらず、`rust-version` を変えれば CI が追従することを確認する。
- **手順:**
  1. `grep -n "1\.89" .github/workflows/ci.yml` を実行する。
  2. ローカルで読み出しコマンドを実行する: `cargo metadata --format-version 1 --no-deps --locked | jq -r '[.packages[].rust_version] | unique | .[]'`
  3. CI の msrv ジョブのログで、読み出した版数を表示している行（`MSRV: 1.89`）と、導入した toolchain が名乗る行（`rustc "+$MSRV" --version`）の両方を見る。
  4. `cargo build --workspace --all-targets --locked` が msrv ジョブで実行されていることをログで確認する。
- **期待結果:**
  1. **ヒット0件**（版数がワークフローに書かれていない）。
  2. `1.89` の1行のみ。
  3. ログに `MSRV: 1.89` が出ており、続く `rustc 1.89.0 …` の行と版が一致している。宣言（`Cargo.toml`）から実際にビルドする toolchain までがログだけで一本に繋がること。
  4. `cargo check` ではなく `cargo build --all-targets` が走っている（Issue が要求する「ビルド」であること）。
- **確認ポイント:** 3 OS すべての msrv ジョブで同じ版数が読めていること。手順2 が空・複数行・`null` を返す場合は読み出しが壊れている。

### 4. スキップが緑に紛れていない

- **対応する受け入れ基準:** AC-6
- **目的:** Issue 本文「補足: CI で落ちうる既知の点」の1点目・3点目に対して、CI が「静かな緑」を作っていないことを確認する。
- **手順:**
  1. unix（ubuntu / macOS）の stable ジョブのログで、非 root アサートのステップが成功していることを見る。
  2. `grep -n "container:" .github/workflows/ci.yml` と `grep -n -- "--test " .github/workflows/ci.yml` を実行する。
  3. 各 OS の run ページでジョブサマリーを開き、除外件数と `SKIP ` 行の一覧を読む。**3 OS 分を並べて読む** — 除外の単位はケース名ではなくテストバイナリの区間なので、1 OS だけ件数がずれる形が起こりうる。
  4. その一覧を steps.md ステップ3 に**実行前から書いてある期待集合**と突き合わせる。
- **期待結果:**
  1. 非 root アサートが緑（ランナーは非 root で走っている）。
  2. どちらも**ヒット0件** — `container:` を使わず、`cargo test --test <名前>` の単一ターゲット指定もしていない。
  3. サマリーに SKIP 行が列挙されており、`pulsen-conformance` の lib ユニットテスト区間（`SkipBudget` 自身を検証する架空ケース3件）が除外されている。除外件数が **3 OS すべてで**「3 件」と表示されている（1 OS だけであっても 3 から動いていれば、実在ケースのスキップを巻き込んでいるか、抽出が架空ケースを取りこぼしている。adr.md ADR-005）。
  4. **unix は `tc_port_clock_005` の1件のみ、Windows はそれに権限系10件を加えた11件。**
- **確認ポイント:**
  - 非 root アサートのログに `uid=<数値>` が出ていること。このステップは `id` 自体の失敗と数値でない出力も失敗として扱う（CI が独自に持つ唯一の合否判定を、判定できなかったときに通す側へ倒さない。adr.md ADR-005）。
  - サマリーが「なし」と出た場合、本当に SKIP が0件なのか、テストが走っていないのかを区別する（走っていない場合は「テストが走っていない」と表示される設計。steps.md ステップ3）。
  - **一致しなかった場合、観測値をそのまま期待値に書き写して閉じない。** 期待集合の更新には (1) 当初の予測値 (2) 観測値 (3) HOOKS.md のどの行・どの probe の見立てを誤ったかまで遡った理由、の3点が PR 本文に揃うことが条件（AC-6(d)）。
  - ロック系5件・`non_repo_dir` 系2件が現れた場合はフィクスチャ側の欠陥として扱い、期待値の書き換えでは閉じない。

### 5. `SkipBudget` が宣言外のスキップを実際に落とすこと（回帰確認）

- **対応する受け入れ基準:** AC-6
- **目的:** 「合否判定は `SkipBudget` に一本化し、CI は可視化だけ持つ」（adr.md ADR-005）が成立していること、つまり CI が緑なのは SkipBudget が働いているからだと確かめる。
- **手順:**
  1. ローカルで、適合スイートを適用しているテストファイルの `SkipBudget` 宣言から許容ケースを1つ一時的に削る。
  2. `cargo test --workspace --locked` を実行する。
  3. 変更を戻し、再度 `cargo test --workspace --locked` が緑に戻ることを確認する。
- **期待結果:** 手順2 でそのケースが**失敗**する（スキップが静かに緑にならない）。手順3 で緑に戻る。
- **確認ポイント:** この確認は CI ではなくローカルで行う（CI に恒久的な細工を入れない）。手順1 の変更を**コミットしない**こと。

### 6. Windows で初めて build / test / clippy の結果が得られている

- **対応する受け入れ基準:** AC-5, AC-8
- **目的:** Issue の主目的である「macOS しか確認できていない」状態の解消を確認する。
- **手順:**
  1. Windows の stable ジョブのログで、build / test / clippy の3つが**いずれも実行されている**ことを見る（clippy は先行が赤でも走る設計。ステップ2）。
  2. 赤があれば、その原因箇所を特定する。
  3. `grep -rnE '(cfg!?|cfg_attr)\([^)]*(unix|windows|target_os|target_family|target_env|target_arch|target_pointer_width)' crates/pulsen-domain/src/` を実行する。
  4. 条件付き修正が入った場合、触ったファイルの一覧を `git diff --name-only origin/main...HEAD` で確認する。
  5. `crates/pulsen-conformance/HOOKS.md` の「環境で走らなくなりうる行」に3ランナーの実測が記録されていることを確認する。
- **期待結果:**
  1. 3コマンドすべてに結果がある（テストが落ちても clippy の指摘が同じ実行で得られている）。
  3. **ヒット0件** — ドメイン層に OS 依存が漏れていない。
  4. OS 差の吸収が `crates/pulsen/src/util/` / `crates/pulsen/src/adapter/` / `crates/pulsen/tests/` / `crates/pulsen-conformance/src/` に収まっている（MSRV 回避・lint / 整形の追従で `cli/` `application/` `pulsen-domain/` `Cargo.toml` が変わるのは別軸。adr.md ADR-008）。
  5. どの OS でどの行が走り、どの行がスキップされたかが表に反映されている。
- **確認ポイント:** 撤退条件（adr.md ADR-008）を適用した場合、`continue-on-error` で緑に見せていないこと。外したジョブの数と理由がワークフローの why コメント・PR 本文・Issue #10 のコメントに残っていること。

### 7. fmt ジョブが現行 stable で走っている

- **対応する受け入れ基準:** AC-3
- **目的:** ローカル（nixpkgs 1.97.1）と CI の rustfmt の版差が CI に映ること、版を固定して逃げていないことを確認する。
- **手順:**
  1. fmt ジョブのログで `rustup update stable` を踏んでいる行と、`rustc --version` / `cargo fmt --version` の表示を見る。
  2. `ls rustfmt.toml && cat rustfmt.toml` を実行する。
  3. `cargo fmt --all --check` が実行されていることをログで確認する。
- **期待結果:**
  1. 現行 stable を導入したうえで実行しており、使った版がログに残っている。
  2. `rustfmt.toml` の内容が `edition = "2024"` のみで、版固定や整形の抑止（`disable_all_formatting` 等）が加わっていない。
  3. `--all --check` が付いている。
- **確認ポイント:** 赤になった場合の解消は「CI の stable で `cargo fmt --all` を掛け直した差分をコミット」であり、`rustfmt.toml` での抑止ではない（AC-3・ステップ10）。

---

## エッジケース・異常系

### 1. `Cargo.lock` が古いと全ジョブが即座に落ちる

- **目的:** `--locked` が意図どおり依存グラフを固定していることを確認する（adr.md ADR-006）。
- **手順:**
  1. ローカルで `Cargo.lock` の任意の依存のバージョン行を1つ書き換える。
  2. `cargo build --workspace --locked` を実行する。
  3. 変更を戻す（`git checkout Cargo.lock`）。
- **期待結果:** 手順2 が「lock file needs to be updated」系のエラーで失敗する。手順3 で復旧する。
- **確認ポイント:** `--locked` が無ければ黙って `Cargo.lock` が書き換わる。CI ではこれが起きないことが要点。**手順1 の変更をコミットしない。**

### 2. `rust-version` を変えると MSRV ジョブが追従する

- **目的:** 版数がハードコードされていないことを、読み出し経路を通して確認する。
- **手順:**
  1. ローカルで `Cargo.toml` の `workspace.package.rust-version` を一時的に別の値（例: `1.90`）に変える。
  2. `cargo metadata --format-version 1 --no-deps --locked | jq -r '[.packages[].rust_version] | unique | .[]'` を実行する。
  3. 変更を戻す。
- **期待結果:** 手順2 が `1.90` を返す（3クレートとも `rust-version.workspace = true` なので継承が解決される）。
- **確認ポイント:** 複数行返る場合は、いずれかのメンバーが個別に `rust-version` を持っている。その場合 CI は失敗する設計（ステップ4）。**手順1 の変更をコミットしない。**

### 3. ランナー同梱ツールの前提が崩れたときにその場で失敗する

- **目的:** rustup / cargo / jq / awk / grep / id / tee の不在が「cargo の不可解な失敗」ではなく前提検査の失敗として現れることを確認する（AC-7）。
- **手順:**
  1. 各ジョブのログ先頭の前提検査ステップを見る。
  2. `grep -n "sort" .github/workflows/ci.yml` を実行する。
- **期待結果:**
  1. fmt は `rustup`、stable は `rustup` と `awk` / `cat` / `grep` / `id` / `tee`、msrv は `rustup` / `cargo` / `jq` / `grep` を確認している。`rustup` / `cargo` / `jq` は版を表示し、残りは `command -v` で解決先パスを表示している。
  2. **ヒット0件** — `sort` はどのジョブも使わない（Windows で `System32\sort.exe` を掴む事故を避けるため。adr.md ADR-001）。
- **確認ポイント:** Windows のログで `command -v` の解決先が Git for Windows 側（`/usr/bin/...`）を指していること。

### 4. PR の連続 push で古い実行だけが打ち切られる

- **目的:** `concurrency` が PR に限定されており、main の実測履歴を消さないことを確認する（AC-1）。
- **手順:**
  1. PR に短い間隔で2回 push する。
  2. `gh run list --branch <ブランチ名> --limit 5` で実行の状態を見る。
- **期待結果:** 1回目の実行が `cancelled` になり、2回目が走る。
- **確認ポイント:** マージ後、main への push で走った実行は後続の push でも `cancelled` にならない（`cancel-in-progress` が `github.event_name == 'pull_request'` のときだけ真）。この確認はマージ後にしかできないので、マージ前は式の内容を目視で確認する。

---

## 既存機能への影響確認

- **プロダクションコードへの影響:** ワークフローファイルの追加自体はビルド対象に影響しない。影響が出るのは条件付き修正（ステップ6〜10）が発火した場合のみで、その場合はローカルで `cargo test --workspace --locked` が引き続き緑であることを確認する（macOS 458件 PASS が計画時のベースライン）。
- **`.adr/022` / `.adr/023` との整合:** MSRV を上げた場合のみ。`rust-version` の新しい値が `.adr/022`（`std::fs::File::try_lock` の安定化 = 1.89）・`.adr/023`（`std::env::home_dir()` の非推奨解除 = 1.87.0）の根拠と矛盾しないことを読み合わせる（AC-4）。
- **PR #11（tick）への影響:** 本 Issue が先行マージされる。条件付き修正が入ったのは `crates/pulsen/src/adapter/task_file.rs` / `crates/pulsen/src/util/atomic.rs` / `crates/pulsen/src/adapter/task_repository.rs` の3ファイルで、ここが衝突面になる（ステップ10 の `cargo fmt --all` 掛け直しは発火していないので、ソースツリー全域には広がっていない）。衝突の解消と、#11 が追加するコードの Windows 挙動は #11 側の責務（plan.md スコープ「PR #11 とのマージ順」）。**この Issue の CI が実測したのは `e524981`（PR #11 のマージ前）のコードであり、プロセス同定・デタッチ起動の Windows 挙動は未検証のまま残る。**
