# 実装手順 — Issue #10

ブランチは `origin/main`(9d54376)から切る。`dev` / PR #11(Issue #2)は含めない。

## 設計

### ドメインモデルへの影響

**なし。** 本 Issue が追加するのはビルド基盤(GitHub Actions ワークフロー)だけで、エンティティ・値オブジェクト・不変条件・ポート定義のいずれにも触れない。CI が赤くなった場合でも、ドメイン層は変更対象から外す — CLAUDE.md「ドメイン層は外部クレートや I/O に依存しない」と ADR-037「ドメインに `#[cfg]` を1つも置かない」が、そもそも OS 差がドメインに現れないことを設計で保証しているためである。この保証が破れていれば `crates/pulsen-domain/` で target 述語つき `cfg` が必要になるが、その状況になったら「ドメインが環境を知っている」という設計上の欠陥として扱い、値の導出を純粋関数へ押し戻すか、環境依存の判断をポート越しへ移す。`std::path::MAIN_SEPARATOR` の比較で分岐する既存箇所(ADR-037)は cfg ではないのでこの制約に抵触しない。

AC-5 の grep はこの保証を機械的に確認する手段であり、CI を緑にする過程で「とりあえずドメインに cfg を1つ」と逃げないための歯止めでもある。歯止めが効くよう、パターンは属性形 `#[cfg(...)]` だけでなく**マクロ形 `cfg!(...)` と `cfg_attr` も拾う**。

```sh
grep -rnE '(cfg!?|cfg_attr)\([^)]*(unix|windows|target_os|target_family|target_env|target_arch|target_pointer_width)' crates/pulsen-domain/src/
```

`cfg!(windows)` はこのコードベースの既存の作法で(`crates/pulsen/tests/common/git.rs:24` の `if cfg!(windows) { "NUL" } else { "/dev/null" }`)、`#[cfg]` を避けたいときに最も手近な逃げ道でもある。属性形だけを見るパターンだと、ドメインに `if cfg!(windows)` を1行足しても 0 件のまま通ってしまう。`.thread/1/testing.md` の旧パターン(`cfg(unix)\|cfg(windows)`、対象は `crates/*/src/`)を、マクロ形・述語・対象ディレクトリの3点で強化した形になる。

### ユースケース / アプリケーションロジック

**なし。** `application/` 配下は変更しない。ホーム解決(`cli/wire.rs` の `env::home_dir()`)は Windows で `USERPROFILE` を読むが、これは std が吸収しており、受け入れテストのビルダーが `HOME` / `USERPROFILE` の両方を一時ディレクトリへ向けている(ADR-062)ため、プラットフォーム分岐は不要。

### アダプター / 永続化 / 外部連携

OS 差の吸収先はここに閉じる。現状の隔離は次のとおりで、CI が赤にした場合の修正先はこの表のどれかになる。

| 層 | ファイル | 抱えている OS 差 | CI が赤にしたときの吸収方針 |
|---|---|---|---|
| 共通ユーティリティ | `crates/pulsen/src/util/atomic.rs` | ディレクトリ fsync(`cfg(unix)` / no-op)、`persist` による置換、`fs::rename` の上書き可否 | この1ファイルに再試行やプラットフォーム分岐を集約する。`write_atomic` / `rename_atomic` の呼び出し側(`adapter/task_repository.rs`)には差を漏らさない |
| アダプター | `crates/pulsen/src/adapter/lock.rs` | `File::try_lock` の POSIX/Windows 差 | std が吸収済み(ADR-022)。差が出たらこのファイル内で `TryLockError` の分類を調整する |
| アダプター | `crates/pulsen/src/adapter/worktree.rs` | git CLI の起動と環境変数の除去 | `INHERITED_GIT_ENV` の集合や引数の組み立てをこのファイルで調整する。ただし本番アダプターはユーザーのグローバル設定を尊重する方針(ADR-024)を崩さない |
| テストフィクスチャ | `crates/pulsen/tests/common/git.rs` | `GIT_CONFIG_GLOBAL` に渡す null デバイス(ADR-033) | フィクスチャ側だけを直す。本番アダプターには入れない |
| テストフィクスチャ | `crates/pulsen/tests/common/lock.rs` | 保持プロセスの起動と合図の待ち時間(`SIGNAL_DEADLINE`) | 合図が期限内に返らない環境では、許容集合が空のままケースだけがスキップされて失敗になる。`SIGNAL_DEADLINE` かフックの述語をこのファイルで調整する |
| テストフィクスチャ | `crates/pulsen/tests/common/mod.rs`、`crates/pulsen/tests/conformance_task_repository.rs`、`crates/pulsen-conformance/src/lib.rs` | 権限操作の効き目 | 既に `cfg(not(unix))` で `None` を返す実装があり、`SkipBudget` が実行時の probe で受け止める(ADR-055)。新しい環境依存が見つかったら、`cfg` 分岐ではなく **probe を足して集合に加える**(adr.md ADR-008) |

**アダプター層に持ち込まないもの**: `tests/common/git.rs` の修正はテストフィクスチャであってアダプターではない。ADR-033 が「この固定はテスト側にだけ置く」と決めており、CI を通すために本番の `GitCliWorktreeManager` へグローバル設定の無効化を持ち込むのは方針に反する。

### ビルド基盤(本 Issue で実際に作るもの)

`.github/workflows/ci.yml` 1ファイル。ジョブ構成と、その形にする理由:

```
fmt      : ubuntu-latest              版の表示 → cargo fmt --all --check
test     : {ubuntu, macos, windows}   前提の検査 → build → test(--nocapture, ログ保存) → スキップ報告 → clippy
msrv     : {ubuntu, macos, windows}   前提の検査 → rust-version 読み出し → toolchain 導入 → cargo build --all-targets
```

- **fmt を分離**するのは、整形はプラットフォーム非依存でありマトリクスで3回走らせる意味が無いのと、最も速く返るシグナルを独立させたいため。ただし toolchain の用意手順は test ジョブと同じで、イメージ同梱の rustfmt には頼らない(adr.md ADR-004)。
- **MSRV を3 OS で回す**のは、`cfg(unix)` / `cfg(not(unix))` でコンパイルされるコードが OS ごとに違うため。1 OS だけの MSRV 検証では、もう一方でしかコンパイルされないコードの API 要求を見ていないことになる(adr.md ADR-002)。
- **test ジョブに clippy を同居**させるのは、build/test と同じ toolchain・同じ `target/` を使い回せるため。clippy を別ジョブにすると全依存を再コンパイルすることになる。順序は build → test → clippy で、まず「動くか」を見てから「綺麗か」を見る。ただし clippy は先行ステップが赤でも走らせる(adr.md ADR-007)。
- **`--locked` を、それを受け付ける全コマンドに付ける**のは、`Cargo.lock` に固定された依存グラフが MSRV 検証の対象そのものだから。CI が勝手に依存を更新したら、MSRV ジョブが緑でもローカルの再現性が保証されない。`cargo fmt` だけは `--locked` を受け付けない(adr.md ADR-006)。
- **`container:` を使わない / `cargo test --test <名前>` を使わない** — どちらも `SkipBudget` の前提を崩す(research.md 参照)。ワークフローにこの理由を why コメントとして残す。
- **スキップの合否判定を CI に持たせない** — 判断主体は `SkipBudget` に一本化し、CI は環境前提(非 root)の直接アサートと可視化だけを担う(adr.md ADR-005)。
- **シェルはワークフロー既定で bash に寄せる** — `defaults.run.shell: bash` をワークフロー直下に置き、ステップごとの `shell:` 指定は書かない(adr.md ADR-007)。

**YAML の記法**: `${{ }}` を含む値は**必ずブロック記法で書く**。YAML のフローコレクション内の plain scalar は `{` `}` `[` `]` `,` を含められないため、`concurrency: { group: ${{ github.workflow }}-${{ github.ref }}, ... }` はパーサで `did not find expected ',' or '}' while parsing a flow mapping` になる(実測)。同じ理由で `permissions: contents: read` のような入れ子マッピングの1行書きも `mapping values are not allowed in this context` で落ちる。式を含まない `strategy: { fail-fast: false, matrix: { os: [...] } }` や `env: { ... }` はフロー記法でも通るが、記法を混ぜると「どれが通ってどれが落ちるか」を都度判断することになるので、ワークフロー全体をブロック記法で書く。

`shell: bash` は GitHub Actions が `bash --noprofile --norc -eo pipefail {0}` で起動する(Windows では Git for Windows 同梱の bash)。`pipefail` は `tee` を挟んでも cargo の失敗を伝播させるために要るが、同じ `-e` が `grep` にも効き、**該当行が無いときの exit 1 でステップが失敗する**。`grep` を使う全ステップで終了コードの扱いを明示する(ステップ3)。

## 実装ステップ

ステップ 1〜4 でワークフローを組み立て、ステップ 5 で初めて CI を回す。ステップ 6〜10 は **ステップ 5 の結果に応じてのみ実施する条件付きステップ**で、どれも「CI が緑になるまで」を完了条件とする。ただしその射程には上限があり、層を越える吸収の停止規則と、緑にできない場合の撤退条件は adr.md ADR-008 に従う。**判定は CI が赤になった時点で実装者が行い、結果を PR 本文に残す** — 判定を先送りしたまま試行を繰り返さない。

ADR-008 の吸収先ディレクトリによる判定が掛かるのは **OS 差の吸収(ステップ 6〜8)** だけである。ステップ9(MSRV 回避)とステップ10(lint / 整形の追随)は Issue 本文が明示的に求めた作業で、触るファイルの場所では切らない。撤退条件のほうは条件付きステップ全体に掛かる。

### 1. ワークフローの骨格と fmt ジョブ

- **対象ファイル:** `.github/workflows/ci.yml`(新規)
- **変更内容:** ワークフロー直下の宣言は次のとおり。`${{ }}` を含む値があるため**ブロック記法で書く**(フロー記法では YAML パーサが通らない。「設計」参照)。

  ```yaml
  name: CI

  on:
    push:
      branches: [main]
    pull_request:
    workflow_dispatch:

  permissions:
    contents: read

  # 打ち切りは PR に限定する。main への連続 push で先行コミットの CI が打ち切られると
  # 「どのコミットまで緑だったか」が残らない。宣言を実測に変えるのが本 Issue の目的なので、
  # main の実測履歴が欠けるのは避ける。
  concurrency:
    group: ${{ github.workflow }}-${{ github.ref }}
    cancel-in-progress: ${{ github.event_name == 'pull_request' }}

  # 色を付けると cargo の "Running" 行が ESC で始まり、行頭一致での区間判定が効かなくなる。
  # サマリー抽出は ANSI を落としてから照合する(ステップ3)。
  env:
    CARGO_TERM_COLOR: always
    CARGO_INCREMENTAL: 0

  # 指定の無い run は Windows でだけ pwsh になる。同じジョブでシェルが混ざると
  # 終了コードの伝播とリダイレクトの意味がステップごとに変わり、Windows の赤を切り分けにくい。
  defaults:
    run:
      shell: bash
  ```

  - **`RUST_BACKTRACE` は `env` に入れない**(adr.md ADR-005)。あわせて、`pulsen-conformance` の意図的なパニック2件が緑の実行でもログに出ることを**ワークフローの why コメント**に残す: **`pulsen-conformance` の lib ユニットテストは `SkipBudget` 自身の検証で、`--nocapture` 下では宣言外スキップのパニックメッセージが2件ログに出る。緑の実行でも現れるので、`SkipBudget` 違反と読まないこと。**
  - `fmt` ジョブ: `ubuntu-latest` / `actions/checkout@v7` / **前提の検査と版の表示**(`rustup --version`)/ `rustup update stable --no-self-update` → `rustup default stable` → `rustup component add rustfmt` / `rustc --version` と `cargo fmt --version` の表示 / `cargo fmt --all --check`。
  - `actions/checkout` は**メジャータグで固定**する(`@v7`)。ブランチ参照や無指定は使わない。唯一残す Action なので固定方法も明示しておく(adr.md ADR-001)。
- **理由:** トリガーと権限・並行制御・既定シェルはワークフロー全体の前提なので最初に固定する。fmt は依存のビルドが要らず最も軽いので、骨格の動作確認を兼ねる。fmt にも `rustup update stable` を踏ませるのは、rustfmt が版によって整形結果を変えるため — clippy が新 stable・rustfmt が旧 stable だと「CI は緑だが開発者が新 stable で `cargo fmt` すると差分が出る」ズレが CI に映らない(adr.md ADR-004)。`rustup --version` を先頭に置くのは、ランナー同梱の前提が崩れたときに `cargo` の不可解な失敗ではなくその場で分かるようにするため(AC-7)。`CARGO_INCREMENTAL=0` は使い捨てランナーで増分情報を作るのが無駄で、特に Windows で効く。

### 2. stable マトリクスジョブ(build / test / clippy)

- **対象ファイル:** `.github/workflows/ci.yml`
- **変更内容:**
  - `test` ジョブ。ブロック記法で `strategy.fail-fast: false` と `strategy.matrix.os: [ubuntu-latest, macos-latest, windows-latest]`、`runs-on: ${{ matrix.os }}`、`timeout-minutes: 30`。
  - `actions/checkout@v7`。
  - **前提の検査**(先頭): このジョブが使うランナー同梱ツールの存在を確認する(AC-7)。`rustup` は版も表示し、スキップ報告(ステップ3)が依存する `awk` / `cat` / `grep` / `tee` は `command -v` で存在と解決先パスを見る(検査の形を道具ごとに分ける理由と、`sort` を使わない理由は adr.md ADR-001)。

    ```yaml
    - name: ランナー同梱ツールの前提を確認する
      run: |
        rustup --version
        for tool in awk cat grep tee; do
          command -v "$tool" || { echo "::error::$tool が見つからない" >&2; exit 1; }
        done
    ```

  - あわせて unix ランナーでは非 root であることを直接アサートする。

    ```yaml
    - name: 非 root で走っていることを確認する
      if: runner.os != 'Windows'
      # SkipBudget の権限系の宣言は permission_restrictions_effective() の probe で決まる。
      # root で走ると chmod が効かず、権限系10件が「宣言済みスキップ」として静かに緑になる。
      # スキップ件数を数えるのではなく、その前提が崩れていないことを直接見る。
      run: |
        if [ "$(id -u)" -eq 0 ]; then
          echo "::error::root で実行されている。SkipBudget の権限系の宣言が崩れる" >&2
          exit 1
        fi
        echo "uid=$(id -u)"
    ```

  - toolchain: `rustup update stable --no-self-update` → `rustup default stable` → `rustup component add clippy` → `rustc --version` の表示。第三者製 Action は使わない。
  - `cargo build --workspace --locked`
  - `cargo test --workspace --locked -- --nocapture 2>&1 | tee test.log`(ワークフロー既定の bash は `-eo pipefail` 付きなので、`tee` を挟んでも cargo の失敗が伝播する。ステップごとの `shell:` 指定は要らない)
  - clippy はステップ列の最後に置くが、**先行ステップが赤でも走らせる**(adr.md ADR-007)。

    ```yaml
    - name: clippy
      # テストが落ちても clippy まで進める。1回の実行で「動くか」と「綺麗か」の両方の
      # 指摘が揃わないと、Windows のようにどちらのリスクも抱える環境で往復が増える。
      # clippy が落ちればジョブは赤のままなので、continue-on-error とは意味が違う。
      if: success() || failure()
      run: cargo clippy --workspace --all-targets --locked -- -D warnings
    ```

  - ワークフローに why コメントを残す: **パッケージ全体を対象にした `cargo test` は example もビルドするため、ロック保持のフィクスチャ(`examples/lock_holder`、ADR-032)が存在する。単一テストターゲット指定にすると4件+1件がスキップに落ちる(Issue 補足3点目)。`--workspace` のままにすることがこの経路の構造的な回避であり、`SkipBudget` の許容集合には入れない(HOOKS.md)。**
- **理由:** Issue が明示する4コマンドのうち3つをここで満たす。`fail-fast: false` は「Windows だけ落ちたのか、全 OS で落ちたのか」を1回の実行で見分けるために必須。`--nocapture` は次のステップの前提(libtest は成功したテストの stdout を握り潰すため、`SKIP` 行を取り出すにはこれが要る。ADR-055)。非 root のアサートを CI 側の唯一の合否判定にするのは、Issue が挙げた「root コンテナで回すなら宣言値の見直しが要る」への直接の答えだから — スキップ件数という proxy ではなく前提そのものを見るので、観測結果に依存せず循環しない(adr.md ADR-005)。

### 3. スキップの可視化と、期待スキップ集合の事前確定

- **対象ファイル:** `.github/workflows/ci.yml`、本ファイル(期待集合の記録)
- **変更内容:** ステップ 2 のテスト実行の直後に**報告ステップを1つだけ**足す。合否を判定するステップは足さない。
  - **スキップの報告**(`if: always()`、全 OS): `test.log` から `^SKIP ` 行を抜き出し、重複を除いて `$GITHUB_STEP_SUMMARY` に「走らなかったケース(<OS>)」として列挙する。
  - **「スキップなし」と「テストが走っていない」を区別する**(adr.md ADR-005)。判定は2段。報告ステップは `if: always()` なので build 失敗後も走るが、`test.log` の有無だけでは足りない — build ステップで落ちればテストステップごと飛ばされて `test.log` は生まれない一方、テストステップまで到達してコンパイルに失敗した場合は `tee` がエラー出力を書いた `test.log` が残る。後者を捕まえるためにテスト結果行(`^test result:`)の有無も見て、「到達していない」「走っていない」「走ったが 0 件」の3状態に分ける。
  - **`pulsen-conformance` の lib ユニットテストの区間を除外する。** このクレートの `#[cfg(test)] mod tests` は `SkipBudget` の仕様自体を検証しており、`tc_port_clock_005_時刻の巻き戻し` / `tc_port_clock_004_時刻の前進` / `tc_port_clock_0051_別のケース` という**架空のケース名**で `SKIP ` 行を3件出す(うち2件は `#[should_panic]`)。実在の適合ケースと形が区別できないため、混ざるとサマリーの読み手が「走らなかった適合ケース」を誤って数え、HOOKS.md との突き合わせが永久に成立しなくなる。cargo はテストバイナリごとに `Running ...` 行を出すので、`Running unittests src/lib.rs (…pulsen_conformance-…)` から次の `Running ` 行までを落とす。
  - **区間の判定は ANSI エスケープを落としてから行う。** `CARGO_TERM_COLOR: always`(ステップ1)の下では cargo の `Running` 行が `\033[1m\033[92m     Running\033[0m unittests src/lib.rs (…pulsen_conformance-…)` の形になり、行頭が空白ではなく ESC になる。行頭一致で区間を判定すると除外がまるごと効かず、架空の3件がサマリーへ混ざって期待集合との突き合わせが**初回から必ず**食い違う(色なし1件 / 色あり4件。実測)。一方 `SKIP ` 行と `test result:` 行は libtest の出力で、パイプ越しでは色が付かない(実測)。除去は照合用の変数に対してだけ行い、比較対象の1行を作ってから判定する。
  - **重複除去に `sort` を使わない**(adr.md ADR-001)。区間判定に使っている awk へ寄せ、`sort` への依存自体を持たない。
  - **`-e` の扱いを明示する。** 既定シェルの bash は `-eo pipefail` 付きで起動するので、`grep` が「該当なし」で返す exit 1 がそのままステップの失敗になる。件数の数え上げは `|| true` で `-e` から切り離す。抽出は awk 1本になり、該当行が無くても exit 0 で空文字を返す。

    ```bash
    summary() { { echo "### 走らなかったケース($RUNNER_OS)"; cat; } >> "$GITHUB_STEP_SUMMARY"; }
    if [ ! -f test.log ]; then
      echo "テストステップまで到達していない(test.log が無い)。" | summary
      exit 0
    fi
    if [ "$(grep -c '^test result:' test.log || true)" = "0" ]; then
      echo "テストバイナリが1つも走っていない(test.log にテスト結果行が無い)。SKIP の有無は判断できない。" | summary
      exit 0
    fi
    skips=$(awk -v esc="$(printf '\033')" '
      { line = $0; gsub(esc "\\[[0-9;]*m", "", line) }
      line ~ /^ *Running / { drop = (line ~ /pulsen_conformance-/) }
      !drop && line ~ /^SKIP / && !seen[line]++ { print "- " line }
    ' test.log)
    if [ -z "$skips" ]; then
      echo "なし(テストは走ったが SKIP 行は無かった)" | summary
    else
      printf '%s\n' "$skips" | summary
    fi
    ```

    ESC を awk のプログラム中に直接書かず `-v` で渡すのは、正規表現定数の `\033` の解釈が実装依存だから。この形は gawk 5.4.1 と macOS 同梱の awk(20200816、GitHub の macOS ランナーと同じ系統)の両方で動くことを、色あり/色なしの `test.log` と上記4状態すべてで実測した。

  - ワークフローに why コメントを残す: **宣言集合の外のスキップは `SkipBudget` 自身がそのケースの失敗にする(ADR-055)。CI が件数で二重化すると判定が観測結果に依存して循環するので、ここは記録だけを担う。環境前提の検査はステップ2 の非 root アサートが持ち、root で走るコンテナを使うとその前提が崩れるため `container:` は使わない。**
- **期待スキップ集合(実行前に確定する):** HOOKS.md の「環境で走らなくなりうる行」から予測すると、**ubuntu / macOS は1件、Windows は11件**。Windows で追加的に落ちるのは、`permission_restrictions_effective()` が `cfg(not(unix))` で常に false になることによる権限系10件だけである。

  | 期待される SKIP | 件数 | 出る OS | 根拠 |
  |---|---|---|---|
  | 適合スイートの `tc_port_clock_005` | 1 | 全 OS | ハーネスが `rewind` を提供しないことによる恒久スキップ。`allowed_clock_skips()` が環境を見ない定数として宣言している(macOS 非 root で実測済み) |
  | TC-port-config-store-023 / TC-port-workflow-store-030 / TC-port-task-repository-005 / 011 / 012 / 019 / 035 / 041 | 8 | Windows のみ | HOOKS.md 区分 C のうち `permission_restrictions_effective` で判定する行 |
  | `tc_task_register_task_016` / `tc_task_register_task_021` | 2 | Windows のみ | 受け入れテストの `PERMISSION_CASES` |

  ロック系5件(`tc_port_exclusive_lock_002/003/004/005` + `tc_task_register_task_017`)と `non_repo_dir` 系2件(`TC-port-worktree-manager-003` / `tc_task_register_task_036`)は、`--workspace` で example がビルドされること・ランナーの一時ディレクトリがリポジトリ配下でないことから **走る**と予測する。これらがサマリーに現れたら、Issue 補足2・3が Windows で顕在化した姿なので、記録ではなくフィクスチャ側の欠陥として直す(ステップ7)。

  **この表の事後更新には条件を付ける。** 「原因を特定してから更新する」だけだと、特定したと主張すれば何でも更新できてしまい、観測値で閉じる経路が残る。上の表を書き換えてよいのは、次の3点を PR 本文に揃えたときに限る(AC-6(d))。

  1. 当初の予測値(この表の該当行)。
  2. 観測値(サマリーに出た SKIP 行)。
  3. 予測が外れた理由を、**HOOKS.md のどの行・どの probe(`permission_restrictions_effective()` / `holder_program()` / `tmpdir_outside_repository()` / `allowed_clock_skips()`)の見立てを誤ったか**まで遡って示したもの。

  3 が「ランナーの環境がそうだったから」で止まる場合、それは予測の誤りではなく**環境が宣言の前提を満たしていない**ということなので、期待値ではなくフィクスチャか probe を直す(ステップ7)。
- **理由:** 「CI がスキップを緑と誤認しない」が本 Issue の肝の一つ。件数のハードコードはせず、宣言側(`SkipBudget`)を唯一の判断主体に保ったまま可視化だけを CI が担う。期待集合を実行前に書くのは、観測結果をそのまま期待値として書き写す構造だと、15件スキップしていても手順どおりに緑で閉じられてしまうため(adr.md ADR-005)。

### 4. MSRV マトリクスジョブ

- **対象ファイル:** `.github/workflows/ci.yml`
- **変更内容:**
  - `msrv` ジョブ。`fail-fast: false`、`matrix.os` は stable と同じ3つ、`timeout-minutes: 30`。`actions/checkout@v7`。
  - **前提の検査**(先頭): `rustup --version` / `cargo --version` / `jq --version` を実行し、`grep` は `command -v` で見る。いずれもランナー同梱だが、崩れたときに `cargo` の不可解な失敗ではなくその場で分かるようにする(AC-7)。

    ```yaml
    - name: ランナー同梱ツールの前提を確認する
      run: |
        rustup --version
        cargo --version
        jq --version
        command -v grep || { echo "::error::grep が見つからない" >&2; exit 1; }
    ```

    **`cargo --version` を入れるのはこのジョブだけ理由がある。** fmt / stable ジョブは `rustup default stable` で自分の toolchain を用意してから cargo を使うが、MSRV ジョブは `rustup default` を一度も呼ばず、ランナーに既定 toolchain が設定済みであることに暗黙に依存して `cargo metadata` を叩く。既定が無ければ rustup のシムが「no override and no default toolchain set」で落ち、それが**版数の読み出しの失敗**として現れる — ADR-001 が避けたかった「不可解な失敗として現れる」形そのもの。あわせて、どの cargo が metadata を出したかがログに残る(このジョブが2つの toolchain を触ることの裏取りになる。adr.md ADR-002)。
  - 版数の読み出し(`id: msrv`): **ワークスペース全メンバーの `rust_version`** を対象にする。3クレートはいずれも `rust-version.workspace = true` で、`workspace.package.rust-version` が唯一の宣言である。`cargo metadata` は継承を解決したうえで各パッケージに `rust_version` を載せるので、1パッケージだけを見ると、将来どれかが個別に `rust-version` を持ったときに CI がそれを検証しないまま通す。

    ```bash
    versions=$(cargo metadata --format-version 1 --no-deps --locked \
      | jq -r '[.packages[].rust_version] | map(select(. != null)) | unique | .[]')
    case $(printf '%s\n' "$versions" | grep -c .) in
      0) echo "::error::rust-version が読み出せない" >&2; exit 1 ;;
      1) ;;
      *) echo "::error::メンバー間で rust-version が割れている: $versions" >&2; exit 1 ;;
    esac
    echo "version=$versions" >> "$GITHUB_OUTPUT"
    ```

    値が割れていたら失敗させるのは、どの版で検証すべきかが一意に決まらない状態を黙って通さないため。`grep -c .` は該当なしで exit 1 を返すが `case` の被検査式に置けば `-e` の対象外になる。`cargo metadata` にも `--locked` を付けるのは、`--locked` を受け付けるコマンドの側で例外を作らないため(adr.md ADR-006)。`--no-deps` は依存解決を行わないので実害の有無ではなく、決定と実装が1箇所だけ食い違う状態を残さないことが理由。
  - `rustup toolchain install ${{ steps.msrv.outputs.version }} --profile minimal --no-self-update`
  - `cargo +${{ steps.msrv.outputs.version }} build --workspace --all-targets --locked`
  - ワークフローに why コメントを残す: **版数をワークフローに書かないのは、`Cargo.toml` の `rust-version` と CI が検証する版が黙ってズレるのを構造的に防ぐため。`--all-targets` にするのは、テスト・example も MSRV でコンパイルできることまでを宣言の意味に含めるため。**
- **理由:** Issue の「MSRV ジョブ(`rust-version` に固定した toolchain でのビルド)」を、宣言と実測が原理的にズレない形で満たす(adr.md ADR-001)。`check` ではなく `build` にするのは、Issue が「ビルド」を求めており、`check` はコード生成・リンク段階でしか出ない失敗を取りこぼすため — それが最も起きやすいのは一度もリンクされたことのない Windows 側コードで、そこは stable ジョブでも MSRV でのリンクは見ていない(adr.md ADR-002)。テストの**実行**までは stable ジョブが見るので重ねない。

### 5. ブランチを push して CI を初回実行し、結果を採取する

- **対象ファイル:** なし(実行)
- **変更内容:** ブランチを push し、PR を作って7ジョブの結果を集める。各ジョブについて「成功/失敗」「失敗ならログの該当箇所」「各 OS のサマリーに出た SKIP 行の一覧」を記録する。SKIP の一覧は**ステップ3 の期待集合と突き合わせる**(観測値で期待値を上書きしない)。
  - あわせて、Issue 補足2点目(一時ディレクトリが git リポジトリ配下)が GitHub ランナーで成立するかを実際に確認する。`TC-port-worktree-manager-003` / `tc_task_register_task_036` が SKIP に現れなければ成立している。現れた場合は、なぜそのランナーで踏むのかをログ(`TMPDIR` / `RUNNER_TEMP` の実値と、そこがリポジトリ配下かどうか)から特定してステップ7 で扱う。
- **トリガーの制約:** `workflow_dispatch` は**ワークフローファイルが既定ブランチに存在しないと UI / API から起動できない**。作業ブランチの段階では使えないので、初回実行の手段は PR を開くこと一択になる。`push` のトリガーは `main` 限定なので、ブランチを push しただけでは何も走らない。
- **理由:** ここまでが無条件の作業で、ここから先は結果に依存する。**この時点で採取した SKIP 行の一覧はステップ 11 の入力になる**ので、緑になったあとの最終実行の結果で更新する。

### 6.(条件付き)Windows のコンパイル失敗・clippy 指摘を直す

- **対象ファイル:** `crates/pulsen/src/util/atomic.rs`、`crates/pulsen/tests/common/mod.rs`、`crates/pulsen/tests/conformance_task_repository.rs`、`crates/pulsen-conformance/src/lib.rs`(いずれも該当した場合のみ)。4つとも「アダプター / 永続化」の表にある吸収先。
- **変更内容:** 未使用 import・dead_code・cfg 漏れなど、`cfg(not(unix))` 側でだけ出る指摘を、その cfg ブロックの中に閉じた形で解消する。`#[allow(...)]` での握り潰しは使わず、import を cfg で括る・使われていない項目を消す等で対処する。
- **理由:** これらのコードはローカルで一度もコンパイル・lint されていない。`crates/pulsen-conformance/src/lib.rs` の `cfg(not(unix))` 側 `probe_permission_restrictions` は特に、Windows で初めて lint を受ける。修正は既に隔離済みの箇所の中で完結する。なお `conformance_task_repository.rs` の `ensure_dir` は `cfg(unix)` の外でも使われているので、そのファイルの未使用 import は当たらない。

### 7.(条件付き)Windows のテスト失敗を吸収する

**全ケース共通の原則**(adr.md ADR-008): 実測の結果「その環境では前提を作れない」と判明した場合、`cfg(windows)` の決め打ちではなく **probe を足して `SkipBudget` の許容集合に入れ、HOOKS.md に行を足す**形で表す。`continue-on-error` で緑に見せることも、テストを緩めることもしない。CI が赤になった時点で、その修正が下の吸収先に収まるかを**その場で判定する**(判定者は実装者、記録先は PR 本文)。収まらないと判定したら直さずに切り出す — 判定の境界は ADR-008 が定める。

- **対象ファイル:** 失敗した内容に応じて次のいずれか。
  - アトミック置換の共有違反(ユニットテストと `TC-port-task-repository-042` / `044` の両方)→ `crates/pulsen/src/util/atomic.rs`
  - git フィクスチャの崩れ(`NUL`)→ `crates/pulsen/tests/common/git.rs`
  - ロック保持プロセスの合図が期限内に返らない → `crates/pulsen/tests/common/lock.rs`
  - ` .yaml` フィクスチャを作れない → `crates/pulsen/tests/cli_add_error.rs` と probe の追加先(`crates/pulsen/tests/common/mod.rs`)
  - 一時ディレクトリがリポジトリ配下になる → `crates/pulsen/tests/common/git.rs`(`tmpdir_outside_repository` の判定)
  - `fs::rename` の上書き差 → `crates/pulsen/src/util/atomic.rs`
- **変更内容:**
  - 置換の共有違反: `write_atomic` の中で `persist` の一時的な拒否に対する短い再試行を持つ。`persist` を使う置換手順そのものは変えず、`write_atomic` / `rename_atomic` のシグネチャと契約(失敗時に一時ファイルを残さない・対象が変わらない)も変えないので、ADR-008 の停止規則には当たらない。回数を使い切ったら `io::Error` を返す(握り潰さない)。「置換が一時的に拒否されても最終的に置き換わる」「使い切ったら Err になる」をユニットテストで表す。ユニットテストと適合スイート(`TC-port-task-repository-042` / `044`)は同じ原因の別の現れ方なので、片方だけを見て閉じない。
  - `NUL`: 空の一時ファイルを作ってそのパスを `GIT_CONFIG_GLOBAL` / `GIT_CONFIG_SYSTEM` に渡す等、Windows でも確実に「空の設定」になる手段へ差し替える。ADR-033 の意図(開発者のグローバル設定・既定ブランチ名に依存しない)は保つ。
  - 合図が返らない: 吸収の方向は2つあり、どちらを採るかは実測後に決める。(a) `SIGNAL_DEADLINE` を伸ばす — ADR-060 の「フィクスチャのハングはテストの失敗より診断が難しい」を崩さない範囲で。(b) 許容集合の述語を `holder_program().is_some()`(実行ファイルの有無)から「実際に1回保持させてみて成立するか」の probe へ寄せる — ADR-055 に揃うが、probe が本番のケースと同じ資源を奪う。**(a) を先に試す。**
  - ` .yaml`: 「語幹が空白だけのファイルを作れるか」を実際に試す probe を足し、作れない環境ではそのケースを `SkipBudget` の許容集合に入れる。`cfg(windows)` での決め打ちはしない。
- **理由:** OS 差の吸収先を共通ユーティリティとテストフィクスチャに限定することで、CLAUDE.md の隔離方針と ADR-033 の「フィクスチャの固定はテスト側にだけ置く」を両立させる。

### 8.(条件付き)Linux 固有の失敗を直す

- **対象ファイル:** 失敗箇所に応じて `crates/pulsen/src/adapter/` 配下
- **変更内容:** ubuntu ランナーでのみ落ちた箇所を、アダプター層で吸収する。
- **理由:** macOS では緑であることを確認済みなので、Linux 固有の差(ファイルシステムの挙動・git のバージョン差)があればここに現れる。想定は薄いが、想定していないからこそ CI で見る。

### 9.(条件付き)MSRV 失敗を解消する

- **対象ファイル:** 使用箇所(`crates/pulsen/src/**`。`cli/` や `application/` も対象になりうる)、`Cargo.toml`、`.adr/022-std-file-lock-and-lockguard-marker-trait.md`、`.adr/023-dependency-selection.md`。これは AC-4 の管轄で、AC-5 の「OS 差の吸収先」とは別の理由で入る差分である。
- **変更内容:**
  1. まず「該当 API の使用を見直す」を試す。1.89 で足りる書き方があるならそちらへ寄せる。
  2. 回避できない場合のみ `workspace.package.rust-version` を実際に通る版へ引き上げる。あわせて ADR-022 の「Rust 1.89 で `File::try_lock` が安定化しており…」という MSRV の根拠と、ADR-023 の「宣言している MSRV 1.89(ADR-022)はこれより後なので」という記述を新しい値と整合させる。ワークフローは版数を参照するだけなので変更不要。
- **理由:** Issue が明示する条件付き作業。ワークフローに版数をハードコードしていないこと(ステップ 4)により、`Cargo.toml` を直すだけで CI の検証対象も自動的に追随する。

### 10.(条件付き)clippy と rustfmt の、現行 stable との差を解消する

- **対象ファイル:** clippy の指摘箇所。rustfmt は `cargo fmt --all` が触るソースツリー全域。
- **変更内容:**
  - **clippy**(stable ジョブが赤): ランナーの stable が devShell の 1.97.1 より新しいことで出た指摘を解消する。`allow` での握り潰しはしない。
  - **rustfmt**(fmt ジョブが赤): CI の stable で `cargo fmt --all` を掛け直し、その差分をコミットする。`rustfmt.toml` に整形を抑止する設定を足すことも、rustfmt の版を固定することもしない。ローカル(nixpkgs の 1.97.1)で `cargo fmt` を掛け直すと CI の stable と別の整形になりうるので、**掛け直すのは CI と同じ現行 stable の rustfmt**(rustup で入れる)にする。
  - どちらもツールチェーンの版固定はしない。
- **理由:** CLAUDE.md「安定版ツールチェーンを前提とし、`cargo fmt` / `cargo clippy` を通る状態を保つ」は「現行の安定版」を意味する。ここで版を固定すると、CI を入れた目的(1環境依存からの脱却)が薄まる。rustfmt を clippy と同じステップで扱うのは、どちらも「ローカルの版と CI の版が違う」という同一の原因から来る赤で、対処も「現行 stable に合わせる」で同じだから。fmt の差分は `cli/` や `pulsen-domain/` にも及びうるが、これは AC-3 の管轄で、AC-5(ドメイン層に OS 依存の `cfg` を持ち込まない)とは別の基準である — 整形の差分は `cfg` を増やさない。

### 11. HOOKS.md に CI 環境での実測結果を記録する

- **対象ファイル:** `crates/pulsen-conformance/HOOKS.md`
- **変更内容:** 「環境で走らなくなりうる行」の節に、GitHub Actions の3ランナーでの実測を短く追記する。書く内容は次の4点に絞る。
  - ubuntu / macOS(非 root)では `permission_restrictions_effective` が成立し、区分 C の権限8行と受け入れテスト2行がすべて走る。
  - TC-port-clock-005 は環境に関係なく全 OS でスキップされる。`SystemClockHarness` が `rewind` を提供しないためで、`allowed_clock_skips()` はそれを環境非依存の定数として宣言している。
  - Windows では上記に加えて権限系10行がスキップされる(合計10行の一覧)。これは宣言済みで、CI はジョブサマリーに列挙する。
  - `hold_from_other_process` / `try_acquire_from_other_process`(example のビルド)と `non_repo_dir`(一時ディレクトリの位置)は3ランナーとも成立するため、該当行は全 OS で走る。
- **理由:** ADR-055 は「どの行がどの条件で走らないかが、宣言と probe の形でテストファイルに残る」ことを影響として挙げている。HOOKS.md はその対応表であり、実際に運用する環境での判定結果が書かれていなければ「未検証を検証済みにする」という Issue の目的が記録として残らない。ステップ 5 の初回結果ではなく、**緑になった最終実行の結果**を書く。ただし Windows の内容がステップ3 の期待集合と食い違う場合は、観測値をそのまま書かず、まず食い違いの原因を特定する(AC-6(d))。

### 12. 最終確認

- **対象ファイル:** なし(確認と記録)
- **機械的に確認すること:**
  - 7ジョブすべてが緑であること(撤退した場合は adr.md ADR-008 の条件を満たし、残ったジョブがすべて緑であること)。
  - `grep -rnE '(cfg!?|cfg_attr)\([^)]*(unix|windows|target_os|target_family|target_env|target_arch|target_pointer_width)' crates/pulsen-domain/src/` が 0 件(AC-5)。
  - ワークフローに第三者製 Action が無いこと(`uses:` が `actions/checkout@v7` だけ)。
  - `Cargo.toml` の `rust-version` と MSRV ジョブのログに現れる toolchain 名が一致していること。
  - 各 OS のサマリーの SKIP 一覧が、ステップ3 に事前確定した期待集合と一致していること。一致しない場合は AC-6(d) の3点を PR に揃えてから期待集合を更新する。
  - HOOKS.md の記述が、上記の一致した期待集合と揃っていること。
- **人が読んで判断すること**(plan.md「レビューで見る観点」): OS 差の吸収として加えた変更が `util/` / `adapter/` / `tests/` / `pulsen-conformance/src/` に収まっているか、`cfg` の決め打ちでなく probe + 許容集合になっているか、AC-5 の対象外のファイルに AC-2 / AC-3 / AC-4 / AC-8 のいずれの理由が付いているか。いずれも差分の**意味**を読まないと判断できない。
- **成果物に残すこと:**
  - **CI が実測した範囲**: PR 本文・Issue #10 のコメント・`.thread/10/progress.md` に、「CI が実測したのは `origin/main` 時点のコードで、PR #11 が追加するプロセス同定・デタッチ起動の Windows 挙動は含まれない」を1行残す。#10 のクローズが「クロスプラットフォームは検証済み」と読まれないようにするため。
  - **撤退した場合の未達範囲**: ADR-008 の撤退条件を適用したなら、外したジョブ・未達の OS・切り出した Issue 番号を Issue #10 のコメントにも残す(AC-2)。ワークフローの why コメントと PR だけだと、Issue のトラッキング上は要件が満たされたように見える。
  - **期待集合を更新した場合**: AC-6(d) の3点(当初の予測値・観測値・予測が外れた理由)が PR 本文に揃っていること。
  - **PR #11 へ引き継ぐこと**: PR #11 にコメントを1件残し、次の3件を伝える。(1) HOOKS.md の実測は `origin/main` 時点のもので、#11 がスイートと example を足した時点で部分的に古くなるため、その更新は #11 の責務であること。(2) 本 Issue の条件付き修正が入ったファイル(実際に触ったものを列挙。`tests/common/git.rs` / `tests/common/mod.rs` / `util/atomic.rs` は #11 も触っており、ステップ10 の rustfmt 掛け直しが発火した場合はソースツリー全域)とのコンフリクト解消は #11 側で行うこと。(3) #11 の `ProcessController` が Windows で赤になった場合の対応も #11 側であること。**引き継ぎ先が #11 なのに記録先が #10 側にしか無いと、#11 の担当者がこの3件を知る経路が存在しない。**
- **理由:** 受け入れ基準のうち機械で閉じられるものと、人が読まないと閉じられないものを分けて確認する。未達・未検証が残る場合は、それが Issue の外から見える場所に残っていることまでを完了条件にする。
