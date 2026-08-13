# レビュー 005（最終確認ラウンド / 3観点を1体で通し）

**対象:** PR #12 `issue/10/ci-msrv-cross-platform` → `main`（HEAD `e524981`）
**契約:** `.thread/10/plan.md`（AC-1〜AC-8）
**観点:** CI・ビルド基盤 / 共通ユーティリティ・並行性・OS 抽象 / テスト・ドキュメント整合
**判定:** **マージ可**

#### Blockers

なし。

#### Warnings

**[W-001] `.thread/10/progress.md:3,61` / PR #12 本文 — 「`e19c973` = 現在の HEAD / 以降の変更はドキュメントのみ」が HEAD `e524981` と食い違う（実測で確認）**

最終コミット `e524981` は `.thread/` と HOOKS.md に加えて **`crates/pulsen/src/util/atomic.rs` を変更している**（テスト名 `時間で解けない拒否に再試行の予算を使わない` → `…は再試行の予算の対象にならない` とコメント3行の追加）。一方で次の3箇所が `e19c973` を HEAD/最終コード変更点として書いている。

- `progress.md:3` 「run 31665658371、コミット `e19c973` = **現在の HEAD**」
- `progress.md:61` 「実測したのは `e19c973`（**本 PR の変更をすべて適用した時点**）」
- PR 本文「確認結果（run 31665658371 / コミット `e19c973` = **最後にコードとワークフローが変わったコミット**）」「**`e19c973` 以降の変更はドキュメントのみ**」

`progress.md:20` は出典の方針を「**最後にコードまたはワークフローが変わったコミット**に対する実測」と自ら定義しており、その方針に照らしても現在の出典は1コミット古い。

**ブロックしない理由:** 実害は「読み手が出典コミットを HEAD と取り違える」ことに限られ、正しさの主張が緩む向きではない。実際の HEAD `e524981` に対する run **31666626824 は7ジョブすべて success**（実測済み）で、状態は本文の主張より良い。AC-8 の一次記録である `HOOKS.md:47` は「測定したのは `e19c973`」と観測時点の事実だけを書いており、HEAD と同一だとは主張していないので AC-8 自体は無傷。

**最小の直し方:** 出典を run 31666626824 / `e524981` に貼り替えるか、`progress.md:3` の「= 現在の HEAD」と PR 本文の「以降の変更はドキュメントのみ」を「以降の変更はテスト名とコメント・ドキュメントのみ（run 31666626824 で 7/7 緑を再確認）」に書き換える。マージ後の追随でも失われるものは無い。

---

### 受け入れ基準の検証（実測）

| # | 判定 | 根拠（このラウンドで自分で取り直したもの） |
|---|---|---|
| AC-1 | 充足 | `ci.yml` に `push(main)` / `pull_request` / `workflow_dispatch`、`permissions: contents: read`、`cancel-in-progress: ${{ github.event_name == 'pull_request' }}`、`defaults.run.shell: bash` を確認。ジョブは fmt(1) + test(3) + msrv(3) = 7。ADR-008 の撤退は未適用 |
| AC-2 | 充足 | run 31666626824（HEAD `e524981`）で `test (ubuntu/macos/windows-latest)` すべて success。`fail-fast: false` あり。`continue-on-error` は grep 1件だがコメント中の言及のみでキーとしては未使用 |
| AC-3 | 充足 | fmt ジョブは `rustup update stable` → `rustup default stable` → `component add rustfmt` を経て `cargo fmt --version` をログに残し、`cargo fmt --all --check`。手元でも `cargo fmt --all --check` が緑 |
| AC-4 | 充足 | `ci.yml` に `1.89` のハードコードは 0 件。`cargo metadata --no-deps --locked` → `[.packages[].rust_version] \| map(select(. != null)) \| unique` を `grep -c .` の 0/1/複数 で分岐。値は `env:` 越しに渡し `${{ }}` をシェル構文へ混ぜない。3 OS の msrv ジョブ success |
| AC-5 | 充足 | `crates/pulsen-domain/src/` の target 述語つき `cfg`（属性形・マクロ形・`cfg_attr`）**0 件**。`crates/` 全体は 16 件で、内訳は conformance 2 / `adapter/task_repository.rs` 1 / `util/atomic.rs` 5 / `tests/common/git.rs` 1 / `tests/common/mod.rs` 2 / `tests/conformance_task_repository.rs` 5。plan.md 記載の 13 件から +3 は `util/atomic.rs` の `transiently_denied` の cfg 対と分類テストの `cfg!(windows)`（吸収そのもの）で、いずれも `util/` に閉じている |
| AC-6 | 充足 | (a) 非 root アサートは `uid=$(id -u)` を代入で受けてから `case` で 0 / 非数値の両方を失敗にする（fail-open なし）。(b)(c) `container:` 0 件・`--test ` 0 件・`sort` 0 件。(d) run 31665658371 のログから `SKIP` 行を採取した結果、**unix は `tc_port_clock_005` の1件、Windows はそれに権限系10件（`tc_port_config_store_023` / `tc_port_workflow_store_030` / `tc_port_task_repository_005・011・012・019・035・041` / `tc_task_register_task_016・021`）を加えた11件**で、steps.md ステップ3 の期待集合と一致 |
| AC-7 | 充足 | `uses:` は `actions/checkout@v7` の3箇所のみ（すべて `persist-credentials: false`）。第三者製 Action 0。前提検査は fmt=`rustup`、test=`rustup` + `awk`/`cat`/`grep`/`id`/`tee`、msrv=`rustup`/`cargo`/`jq`/`grep` で plan.md の指定どおり。`sort` は不使用 |
| AC-8 | 充足 | HOOKS.md に3ランナー分の列と「3ランナーでの実測」節あり。出典 run・コミット・PR #11 マージ前である旨を明記。列の値はログの `SKIP` 実測と一致 |

**スコープ:** 除外項目（キャッシュ・`rust-toolchain.toml`・`.gitattributes`・cargo audit / deny・dependabot・カバレッジ・devShell ジョブ）はいずれも差分に無い。差分は CI 1本＋ Windows 吸収3ファイル＋ HOOKS.md ＋ `.thread/` に収まっている。

### 観点1: CI・ビルド基盤

偽陰性・シェルの落とし穴を一通り当てたが、実害のあるものは見つからなかった。

- **偽陰性:** `cargo test … | tee test.log` は `shell: bash`（`-eo pipefail`）で cargo の終了コードが伝播する。`--no-fail-fast` は打ち切られたバイナリを「未観測」にしないためで、終了コードは非ゼロのまま。clippy の `if: success() || failure()` は失敗時も走らせるだけで、赤は赤のまま残る（`continue-on-error` ではない）。SKIP 報告ステップは `$GITHUB_STEP_SUMMARY` に書くだけで合否に一切関与しないため、拾いすぎ/取りこぼしがあっても判定を汚さない。
- **`-e` が効かない位置の扱いが正しい:** `[ "$(grep -c … )" = "0" ]` と `case $(… | grep -c .) in` は、いずれも該当なしの `grep` の exit 1 がステップ失敗にならない位置。コメントの主張と実際のシェル挙動が一致している。逆に「失敗を通してはいけない」`uid=$(id -u)` は代入形で受けており、向きの使い分けが揃っている。
- **OS 差:** `defaults.run.shell: bash` で Windows の pwsh 混在を排除。`awk`/`cat`/`grep`/`id`/`tee` は `--version` の移植性が無いため `command -v` にとどめ、解決先パスを出して Windows の System32 掴みを検知可能にしている。ANSI 除去を `awk -v esc=$(printf '\033')` で渡すのは、`\033` の正規表現定数解釈が実装依存であることへの正しい対処。
- **サプライチェーン:** Action は公式 checkout のみ・メジャータグ固定・`persist-credentials: false`・`permissions: contents: read`。`${{ }}` をシェルへ直接展開する箇所は無い（MSRV は `env:` 経由）。fork PR でも読み取りトークンのみで、シークレット参照は 0。
- **why コメントと振る舞いの一致:** 「`RUST_BACKTRACE` を置かない」「単一テストターゲット指定にしない（example がビルドされずロック系5件が化ける）」「msrv ジョブだけ `cargo` を前提検査する（`rustup default` を呼ばないため）」はいずれも実際の記述と整合。msrv ジョブが `rustup default` を呼んでいないことも確認した。

### 観点2: 共通ユーティリティ・並行性・OS 抽象（`util/atomic.rs`）

- **契約保持:** `write_atomic` / `rename_atomic` のシグネチャと3契約（一時ファイルを残さない・失敗時に対象が変わらない・読み手はロックなしで一貫した内容）は不変。`persist_with_retry` の打ち切り時、`NamedTempFile` は `retry_while_transient` の `state` として保持され関数の戻りで drop されるため、再試行を挟んでも掃除は保たれる（`置換の一時的な拒否が続けば打ち切られ一時ファイルも残らない` が押さえている）。
- **エラー分類:** `transiently_denied` は Windows で `ERROR_ACCESS_DENIED`(5) / `ERROR_SHARING_VIOLATION`(32) のみ。`NotFound`(2) と `DISK_FULL`(112) を除外し、`raw_os_error()` が `None` のエラーも偽。エラーの型・意味を変えず OS のエラーをそのまま返す。
- **上限:** `MAX_ATTEMPTS: NonZeroU32 = 10`、待ちは 1ms から倍々の9本で和 511ms。`retry_while_transient` は列を先に引いてから分類を問うので、最終試行の後に無駄な分類呼び出しが起きない。試行回数は最大10回でちょうど列の本数 +1。
- **`cfg` の網羅:** `#[cfg(windows)]` / `#[cfg(not(windows))]` の対で、第三の分岐は生じない。unix では分類が恒常的に偽なので、`read_atomic` / `rename_atomic` / `write_atomic` の unix 挙動は `fs::read` / `fs::rename` / `persist` と完全に一致する（＝既存挙動の回帰なし）。
- **抽象化の置き場:** 吸収は `crates/pulsen/src/util/atomic.rs` に集約され、`adapter/task_repository.rs` は `read_atomic` を呼ぶだけ。分類・上限・バックオフの出典が1箇所。CLAUDE.md「アトミック性が要る操作は共通ユーティリティに集約し、個別に再実装しない」に沿う。`config_store` / `workflow_store` が `fs::read_to_string` のままなのは、`write_atomic` で書く経路が無く置換の窓が生じないためで、ADR-013 の「対象外」記述と実際のコードが一致している（grep で確認）。
- **セマンティクスの回帰確認:** `lookup` / `list` は読み取りエラーの `NotFound` を `unreachable_entry` に渡して「消えたエントリ」と「読めないエントリ」を判別する。Windows の delete-pending は 5 を返して再試行に入り、窓が閉じた後は `NotFound` が返って **非一時的として即返る**ため、判別ロジックは変わらない。unix の宙ぶらりん symlink（`#[cfg(all(test, unix))] mod tests` の3件）も分類が常に偽なので経路が変わらず、手元で緑を確認。
- **テストの実効性:** 打ち切り・回数・元エラーの伝播は分類器を注入して全 OS で走る。予算の公称値 511ms は壁時計ではなく待ちの列の和として固定されているので、`*2` → `*4` のような改変を遅いランナーに依存せず検出できる。`置換/移動/読み取りが一時的に拒まれても上限内に解ければ…` の3件は本番の `*_with_retry` を通り、阻害要因の寿命 20ms に対し累積待ちは5回目で 31ms に達するため余裕がある。
- **既知の穴が正しく記録されている:** 「公開関数が `transiently_denied` を渡している配線」はテストで殺せない（緩い分類に差し替えても `NotFound` は `NotFound` のまま）。この穴はテスト内コメントと ADR-012 の Consequences に、**被害が有界な遅延に限られる**という評価つきで残っており、テスト名も `時間で解けない拒否は再試行の予算の対象にならない` と実際に検証している内容に一致するよう直っている。実測で確認（`git show e524981 -- crates/pulsen/src/util/atomic.rs`）。

### 観点3: テスト・ドキュメント整合

- **テストの緩和なし:** `#[ignore]`・`#[cfg(windows)]` による除外・アサーション削除はいずれも 0 件。むしろ `読み手は旧内容か新内容のどちらかだけを観測する` は `if let Ok(observed)` の「読めたときだけ見る」形から `read_atomic(...).expect("読み手は常に読める")` へ**強められて**おり、ポート水準の契約と主張の強さが揃った。
- **フィクスチャの可搬化が正しい向き:** `task_file.rs` は `MAIN_SEPARATOR` から絶対パスを組み、JSON 整形の期待値はリテラル1通りのまま `<repo>` だけを `serde_json::to_string` の結果で差し替える。整形（インデント・キー順・末尾改行）の検査は緩んでいない。ドメインの `RepoPath` は無変更。
- **ドキュメントの事実性（実測で照合）:**
  - HOOKS.md の「Windows 69件・unix 72件」は run 31665658371 のログで **windows `69 passed` / ubuntu・macOS `72 passed`** を確認。手元の `cargo test -p pulsen --lib` も 72 で一致。`68/71 → 69/72` の訂正は主張どおり。
  - 「テストバイナリ15本、`test result:` 行は Doc-tests 3本を含めて18」はログの結果行数（各 OS 18行）と一致。
  - Windows のスキップ11件の内訳（適合8 + CLI 受け入れ2 + clock 1）はログの `SKIP` 行と完全一致。
  - PR 本文の主張（7ジョブ緑・第三者 Action ゼロ・テスト緩和なし・ドメイン層無変更・`sort`/`container:`/単一ターゲット指定/版数ハードコードが 0 件）はすべて実測で裏が取れた。
- **ドキュメントどうしの矛盾:** W-001 の1点のみ。`plan.md` / `steps.md` / `testing.md` / `adr.md` / HOOKS.md の間に他の食い違いは見つからなかった。`progress.md:31` の「緑になった5つの run」は表の6行に対する記述で、その後 2 run 増えたが表の範囲では正しい。
- **ADR と実装のズレ:** ADR-010（分類つき上限再試行）・ADR-011（`MAIN_SEPARATOR` + プレースホルダー）・ADR-012（`retry_while_transient` への集約、`MAX_ATTEMPTS: NonZeroU32`）・ADR-013（読み手側の吸収、対象外の明示）はいずれもコードと一致。ADR-010 の撤回項目には「ADR-012 が置き換えた」旨が書かれており、決定の系譜が追える。
- **弁明・経緯の混入:** コードとテストのコメントを `以前は|かつて|当初は|レビュー|指摘|修正した|変更した|直した|元は|もともと|TODO|FIXME|戻した` で走査した。ヒットは `ci.yml` の2箇所（clippy の「1回の実行で**指摘**を揃える」＝ lint の指摘の意）と `task_file.rs`（`parse` の言及）だけで、いずれも語の別義。修正の経緯・レビューへの弁明・作業履歴はコード・テストのどちらにも残っていない。時系列の記述は `.thread/`（ADR と progress）に閉じており、CLAUDE.md「残すのは現在の形が成り立つ理由（why / why not）だけ」に沿っている。

### このラウンドで実行した検証

```
git fetch origin && git diff --no-renames --name-status origin/main...HEAD
gh pr view 12 --json body,title,state,headRefName,baseRefName
gh run list --branch issue/10/ci-msrv-cross-platform          # 31666626824(HEAD e524981) = 7/7 success
gh run view 31665658371 --log                                 # test result 行 / SKIP 行の採取
cargo test -p pulsen --lib --locked                           # 72 passed
cargo fmt --all --check                                       # 緑
cargo clippy --workspace --all-targets --locked -- -D warnings # 緑
grep -rnE '(cfg!?|cfg_attr)\([^)]*(unix|windows|target_os|…)' crates/pulsen-domain/src/  # 0 件
git show e524981 -- crates/pulsen/src/util/atomic.rs crates/pulsen-conformance/HOOKS.md .thread/10/adr.md
```

#### カバレッジ

| ファイル | 状態 | 見たこと |
|---|---|---|
| `.github/workflows/ci.yml` | 精読 | 全299行。トリガー / permissions / concurrency / defaults / 7ジョブ / 前提検査 / 非 root アサート / SKIP 報告の awk / clippy の `if:` / MSRV 読み出しと env 経由の受け渡し。偽陰性・シェルの落とし穴・OS 差・サプライチェーンを個別に照合 |
| `crates/pulsen/src/util/atomic.rs` | 精読 | 差分381行と現行全体。契約保持・分類・上限・`cfg` 網羅・状態を持つ再試行ループ・追加テスト10件の実効性 |
| `crates/pulsen/src/adapter/task_repository.rs` | 精読 | `read_atomic` への差し替え3箇所と `unreachable_entry` / `lookup` / `list` / `save_degraded` の判別ロジックへの影響。モジュール doc の遅延の積み上がり記述 |
| `crates/pulsen/src/adapter/task_file.rs` | 精読 | `absolute()` / `repo()` / `encoded_repo()` と整形テストのプレースホルダー化 |
| `crates/pulsen-conformance/HOOKS.md` | 精読 | 追加3列・`未測定` の運用規則・「3ランナーでの実測」節。件数と内訳を CI ログで実測照合 |
| `.thread/10/plan.md` | 精読 | AC-1〜AC-8 とスコープ・リスク欄。全 AC を実測で突き合わせ |
| `.thread/10/adr.md` | 精読 | ADR-010 / 011 / 012 / 013 を実装と照合。他の ADR は該当箇所を通読 |
| `.thread/10/progress.md` | 精読 | 実測結果表・run 一覧・出典方針。**W-001 を検出** |
| `.thread/10/steps.md` | 通読 | ステップ構成と条件付きステップの発火状況、ステップ3 の期待集合 |
| `.thread/10/testing.md` | 通読 | 確認手順と期待結果。実施済みの記述と CI の実測に矛盾なし |
| `.thread/10/review/review-001〜004*.md`（16件） | 参照のみ | 判定継承のための台帳。Phase 8 で削除予定と共有済みのため指摘対象外。ゼロベース方針に従い、指摘内容は本レビューの結論に流用していない |
| `.thread/10/review/triage.md` | 参照のみ | 同上 |

（変更ファイル 27 件すべてを列挙。`review-001-ci.md` / `review-001-test-docs.md` / `review-001-util.md` / `review-001.md` / `review-002-*` 4件 / `review-003-*` 4件 / `review-004-*` 4件 = 16件を1行にまとめている）
