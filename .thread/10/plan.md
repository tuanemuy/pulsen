# 実装計画 — Issue #10: CI を用意して MSRV とクロスプラットフォームを検証する

**Issue:** #10
**作成日:** 2026-08-13
**複雑度:** 中〜大規模
**実装方針:** steps.md

---

## 目的

GitHub Actions に CI を用意し、これまで macOS の1環境・1ツールチェーンでしか回っていなかった `build` / `test` / `clippy` / `fmt` を Linux・macOS・Windows と宣言済み MSRV(`rust-version = "1.89"`)の上で実際に走らせて、CLAUDE.md が掲げる「安定版で通る」「特定の OS に依存しない」を宣言から実測へ変える。落ちた箇所はドメイン層に持ち込まず、共通ユーティリティ・アダプター層・テストフィクスチャのいずれかで吸収して緑にする。

## 受け入れ基準

| # | 基準(検証可能な形で) | 由来 | 対応ステップ |
|---|---|---|---|
| AC-1 | `.github/workflows/ci.yml` が存在し、`push`(main)・`pull_request`・`workflow_dispatch` で起動する。`permissions: contents: read` と、**PR の実行だけを打ち切る** `concurrency`(`cancel-in-progress` が `github.event_name == 'pull_request'` のときだけ真)を持つ。ワークフロー全体に `defaults.run.shell: bash` があり、`run` ステップのシェルが OS で分かれない。ジョブは fmt(1)・stable マトリクス(3)・MSRV マトリクス(3)の計7。**ただし adr.md ADR-008 の撤退条件を適用した場合はこの限りでない** — stable の Windows だけを外したなら6、MSRV の Windows も外したなら5で、外したジョブの数と理由がワークフローの why コメントに残っている | Issue「CI を用意し、少なくとも次を回す」 | 1, 2, 4, 12 |
| AC-2 | stable ジョブが `ubuntu-latest` / `macos-latest` / `windows-latest` の3つで `cargo build --workspace --locked`・`cargo test --workspace --locked`・`cargo clippy --workspace --all-targets --locked -- -D warnings` を実行し、**3つとも緑**。`fail-fast: false` で1 OS の失敗が他を隠さない。Windows を緑にできないと結論した場合は adr.md ADR-008 の撤退条件に従い、`continue-on-error` ではなくマトリクスからの除外+別 Issue への切り出しで閉じ、除外の理由と Issue 番号がワークフローの why コメント・PR・**Issue #10 のコメント**に残っている。Issue #10 へのコメントには「Issue が求めた OS マトリクスのうち何が未達で、どの Issue に引き継いだか」を書く — 記録先がワークフローと PR だけだと、Issue のトラッキング上は要件が満たされたように見える | Issue「`cargo build` / `cargo test` / `cargo clippy --all-targets -- -D warnings`」「OS マトリクス(Linux / macOS / Windows)」 | 2, 5, 6, 7, 8, 10, 12 |
| AC-3 | fmt ジョブが現行 stable の rustfmt(`rustup update stable` を踏んだうえで導入したもの)で `cargo fmt --all --check` を実行して緑。使った版がログに残っている。ローカル(nixpkgs の rustfmt)と整形結果が食い違って赤になった場合は、CI の stable で `cargo fmt --all` を掛け直した差分をコミットして解消する(`rustfmt.toml` での版固定・整形の抑止はしない) | Issue「`cargo fmt --check`」、adr.md ADR-004 | 1, 5, 10 |
| AC-4 | MSRV ジョブが `Cargo.toml` の `workspace.package.rust-version` を **唯一の出典**として(ワークフローに版数をハードコードせず)toolchain を導入し、3 OS で `cargo build --workspace --all-targets --locked` が緑。版数の読み出しは全ワークスペースメンバーの `rust_version` を対象にし、空・`null` のみ・値が割れている場合は失敗する。緑にするために `rust-version` を変えた場合は、`Cargo.toml` の値・`.adr/022` の根拠・`.adr/023` の `home_dir` に関する記述が新しい値と整合している | Issue「MSRV ジョブ(`rust-version` に固定した toolchain でのビルド)」「MSRV ジョブが落ちたら `rust-version` を実際に通る版へ上げるか、該当 API の使用を見直す」 | 4, 5, 9 |
| AC-5 | Windows で初めて build / test / clippy の結果が得られており、**OS 差がドメイン層に漏れていない**。機械確認は `crates/pulsen-domain/` に **target 述語つき `cfg`(属性形 `#[cfg(...)]`・マクロ形 `cfg!(...)`・`cfg_attr` のいずれも)が1件も無い**こと 1点に絞る(`grep -rnE '(cfg!?\|cfg_attr)\([^)]*(unix\|windows\|target_os\|target_family\|target_env\|target_arch\|target_pointer_width)' crates/pulsen-domain/src/` が 0 件。`origin/main` で 0 件、`crates/` 全体では属性形が conformance 2・`adapter/task_repository.rs` 1・`util/atomic.rs` 2・`tests/common/mod.rs` 2・`tests/conformance_task_repository.rs` 5 の計 12 件、マクロ形が `tests/common/git.rs` 1 件であることを実測済み)。**吸収先の層が妥当かは機械確認せず、PR のレビューで人が見る**(下記)。**adr.md ADR-008 の撤退条件で stable の Windows を外した場合**、前半は「赤で終わった Windows 実行から得られた範囲で build / test / clippy の結果が得られており」と読む — 撤退の前提が「Windows で赤が出た」ことなので結果自体は必ず存在する。後半のドメイン層 grep は撤退の有無に関わらず 0 件を要求する | Issue「OS マトリクスが落ちたら、失敗した箇所をアダプター層で吸収する」、CLAUDE.md 技術方針、ADR-037 | 5, 6, 7, 8, 12 |
| AC-6 | スキップが緑に紛れない。(a) unix の stable ジョブは実行ユーザーが root でないことを直接アサートし、root なら失敗する。(b) 全 OS でジョブサマリーに `SKIP ` 行が列挙される。`pulsen-conformance` の lib ユニットテストの区間(`SkipBudget` 自身を検証する架空ケース3件)は除外され、実在の適合ケース・受け入れケースだけが並ぶ。テストが走らずに終わった実行では「なし」ではなく「テストが走っていない」と表示される。列挙は合否判定に使わない。(c) CI は `container:` を使わず(root 実行にしない)、`cargo test --test <名前>` のような単一テストターゲット指定も使わない。(d) 各 OS のサマリーの SKIP 一覧が、**実行前に steps.md ステップ3 に書いた期待集合**と一致する(unix は恒久スキップ `tc_port_clock_005` の1件のみ、Windows はそれに権限系10件を加えた11件)。一致しない場合、期待集合の更新は次の3点が PR 本文に揃って初めて行える: (1) 当初の予測値、(2) 観測値、(3) 予測が外れた理由を **HOOKS.md のどの行・どの probe の見立てを誤ったか**まで遡って示したもの。観測値をそのまま期待値に書き写す更新はしない。ロック系5件・`non_repo_dir` 系2件が現れた場合はフィクスチャ側の欠陥として扱い、期待値の書き換えでは閉じない。**adr.md ADR-008 の撤退条件で stable の Windows を外した場合**、(d) は「残った OS(ubuntu / macOS)のサマリーが期待集合と一致すること」と読む。Windows は最後に赤で終わった実行のサマリーを採り、突き合わせの結果を PR 本文に残すが、一致しないことを理由に Issue を開いたままにはしない | Issue「補足: CI で落ちうる既知の点」、ADR-055、adr.md ADR-005 | 2, 3, 5, 12 |
| AC-7 | CI が使う Action は GitHub 公式の `actions/checkout` だけで、第三者製 Action に依存しない。`actions/checkout` はメジャータグ(`@v7`)で固定され、ブランチ参照や無指定ではない。toolchain の導入はランナー同梱の `rustup` を直接叩く。前提とするランナー同梱ツールは各ジョブ先頭の前提検査ステップで確認し、無ければその場で失敗する。対象は fmt ジョブが `rustup`、stable ジョブが `rustup` と `awk` / `cat` / `grep` / `id` / `tee`(`awk` / `cat` / `grep` / `tee` はスキップ報告が、`id` は非 root アサート — CI が独自に持つ唯一の合否判定 — が依存する)、MSRV ジョブが `rustup` / `cargo` / `jq` / `grep`。`rustup` / `cargo` / `jq` は版も表示し、`awk` / `cat` / `grep` / `id` / `tee` は `command -v` で存在と解決先パスだけを見る。**`sort` はどのジョブも使わない**(adr.md ADR-001) | ADR-023(依存を用途に見合う最小限に保つ)、public リポジトリのサプライチェーン | 1, 2, 4, 12 |
| AC-8 | `crates/pulsen-conformance/HOOKS.md` の「環境で走らなくなりうる行」に、3ランナーで実測した成立/不成立が記録されている(どの OS でどの行が走り、どの行がスキップされたか)。この実測は PR #11 のマージ前のコードに対するもので、マージ後に再測することは本 Issue の完了条件に含めない(実測に使ったコミットを HOOKS.md に明記する)。**adr.md ADR-008 の撤退条件でジョブを外した場合**、その OS については「最後に赤で終わった実行から得られた範囲」と、どこまでが観測でどこからが未観測かを明記して記録する。緑にならなかったことを理由に記録を省くと、HOOKS.md の対応表が「Windows は不明」のまま残り、AC-8 が守ろうとしたものが失われる | Issue「補足: CI で落ちうる既知の点」、ADR-055 の「どの行がどの条件で走らないかがテストファイルに残る」運用 | 11 |

### レビューで見る観点(機械確認しない)

機械確認できるのは AC-5 のドメイン層 grep までで、その先は差分の**意味**を読む必要があるため人が判断する。PR レビューで次を見る。

- **吸収先の層が妥当か。** OS 差の吸収として加えた変更が `crates/pulsen/src/util/`・`crates/pulsen/src/adapter/`・`crates/pulsen/tests/`・`crates/pulsen-conformance/src/` のいずれかに収まっているか。
- **吸収の表し方が `cfg` の決め打ちでなく probe + 許容集合になっているか**(adr.md ADR-008)。
- **AC-5 の対象外のファイルに、AC-5 以外の理由が付いているか。** MSRV 回避で `crates/pulsen/src/` を触る(AC-4)・新しい lint の指摘や rustfmt 差分で `cli/` `application/` `pulsen-domain/` を触る(AC-2 / AC-3)・HOOKS.md を追記する(AC-8)は、いずれも Issue が明示的に求めた作業であって AC-5 の管轄ではない。変更理由ごとに、どの基準の下で入った差分かが PR 本文から辿れること。

## スコープ

### 含まれないもの

- **ブランチ保護ルール・required checks の設定** — リポジトリ設定であってコードではない。CI が緑で安定したあとに別途行う。
- **リリース・publish・カバレッジ計測・`cargo audit` / `cargo deny` / dependabot** — Issue が挙げていない。CI の初回導入で足すと、赤の原因が「本 Issue が検証したかったもの」と混ざる。
- **Nix devShell を CI で検証するジョブ** — devShell は開発体験の話で、Issue が求める MSRV・OS マトリクスとは別軸。Windows では成立しない。
- **`target/` のキャッシュ** — 現状クリーンビルドが約1分で、キャッシュの陳腐化・肥大の管理コストが便益を上回る(adr.md ADR-006)。
- **ローカル用の pre-commit フック / Makefile / `cargo xtask`** — CI が回る場所を増やすだけで Issue の目的に効かない。
- **`rust-toolchain.toml` の追加** — rustup 前提のファイルで、Nix devShell(rustup 無し)からは無視される。ツールチェーン指定の出典が2つに割れる(adr.md ADR-003)。
- **`.gitattributes` の追加** — CRLF がテスト結果を変える経路が現状存在しない(`include_str!` / `include_bytes!` / doctest がリポジトリに1件も無く、rustc は文字列リテラル中の CRLF を LF に正規化し、rustfmt の `newline_style` は既定 Auto)。先回りで置くと、検証していない前提を CI に固定することになる。
- **Issue #2(PR #11)が追加したコードへの対応** — 本 Issue のブランチは `origin/main` から切る。#11 が追加する適合スイート・example は #11 のマージ後に CI が自動的に対象へ含める。
- **プロセス同定・デタッチ起動の Windows 検証** — Issue の動機は「実機で確認できているのは macOS のみ」であり、OS 依存が最も濃いのは PR #11 が追加中の `ProcessController` 等である。本 Issue が実測するのは **Issue #1 時点のコード**(アトミック置換・ファイルロック・git シェルアウト・テストフィクスチャ)に限られ、プロセス同定・デタッチ起動の Windows 挙動は #11 が CI に乗って初めて得られる。**本 Issue 完了時点では、その中核が未検証のまま残る。**この事実は PR 本文・Issue #10 のコメント・`.thread/10/progress.md` に残す(ステップ12)。#10 のクローズが「クロスプラットフォームは検証済み」と読まれないようにするため。

### PR #11 とのマージ順

**本 Issue(CI)を先にマージする。** CI の PR は `.github/workflows/ci.yml` の追加だけで小さく、#11 は大きい。順序を決めておかないと AC-8 の実測値が何時点のものか定まらない。

- #11 は次の push で3 OS の CI を背負い、`ProcessController` のデタッチ起動・プロセス同定が Windows で初めて回る。そこで赤が出るのは CI が仕事をしている状態であり、**#11 側で解消する**(必要なら別 Issue へ切り出す)。本 Issue の完了条件には含めない。
- AC-8 で HOOKS.md に書く実測は #11 のマージ前のもので、#11 がスイートと example を足した時点で部分的に古くなる。その更新は #11 の責務とする。
- 本 Issue が #11 の責務とした3件(HOOKS.md の実測更新・条件付き修正とのコンフリクト解消・Windows で `ProcessController` が赤になったときの対応)は、**PR #11 へのコメントとして実際に伝える**(steps.md ステップ12)。記録先が Issue #10 と PR #10 だけだと、引き受ける側にそれが届く経路が無い。

## リスクと注意点

想定される失敗と対処方針。「何が落ちるか」は CI を回すまで確定しないため、緑にするまでを本 Issue の射程とし、想定ごとに修正先の層をあらかじめ決めておく。射程の上限(層を越える吸収の停止規則・Windows を緑にできない場合の撤退条件)は adr.md ADR-008 が定める。

- **[高] Windows の clippy `-D warnings` が新規指摘を出す。** `cfg(not(unix))` 側のコード(`util/atomic.rs` の no-op `sync_dir`、`tests/common/mod.rs` と `conformance_task_repository.rs` の `None` を返すスタブ、`permission_restrictions_effective` の非 unix 版)は一度も lint されていない。未使用 import / dead_code が最有力。ただし `conformance_task_repository.rs` の `ensure_dir` は `cfg(unix)` の外でも使われているので、そのファイルの未使用 import は当たらない。修正はその cfg ブロック内に閉じる。
- **[中] Windows のアトミック置換が共有違反で落ちる。** 読み手が開いている最中の `NamedTempFile::persist` は、Windows では `MoveFileEx` が `ERROR_ACCESS_DENIED` / `ERROR_SHARING_VIOLATION` を返しうる。踏む経路は**2つ**ある。(1) `util/atomic.rs` のユニットテスト `読み手は旧内容か新内容のどちらかだけを観測する`(64KB × 100 回)。(2) 適合スイートの `TC-port-task-repository-042`(別スレッドが読み続ける中で `save` を反復)/ `TC-port-task-repository-044`(同 `archive`)。後者は `concurrent_repo` フックが `Some` を返すので許容集合に入らず、共有違反が起きればケースの失敗として現れる。ユニットテストだけを直しても適合スイート側の赤は残るので、両方を同じ原因として扱う。吸収先はどちらも **`crates/pulsen/src/util/atomic.rs`**(CLAUDE.md「アトミック性が要る操作は共通ユーティリティに集約」)であって、テストを緩めることではない。
- **[中] Windows でロック保持プロセスの合図が期限内に返らず、ロック適合4件+受け入れ1件が「スキップ」ではなく「失敗」になる。** `conformance_lock.rs` の許容集合は `holder_program().is_some()`(実行ファイルの有無)だけで決まるが、`spawn_holder` は `SIGNAL_DEADLINE`(10秒)以内に保持プロセスが `locked` を返さないときにも `None` を返す。この経路では実行ファイルは在るので許容集合は空のまま、ケースだけがスキップされ、`SkipBudget` が `tc_port_exclusive_lock_002/003/004/005` と `tc_task_register_task_017` を失敗させる。Windows は初回起動する実行ファイルにウイルス対策のスキャンが入り、`cargo test` の並列実行で負荷も高いため、原因の見えにくい不安定な赤として現れうる。吸収先は `crates/pulsen/tests/common/lock.rs`。
- **[中] git フィクスチャが Windows で崩れる。** `tests/common/git.rs` は `GIT_CONFIG_GLOBAL` / `GIT_CONFIG_SYSTEM` に `NUL` を渡す(ADR-033)。git for Windows がこれを空の設定として読めなければ、ランナーのグローバル設定が漏れて既定ブランチ名や identity が変わり、worktree 適合テストと `cli_add_*` の多数が連鎖的に落ちる。吸収先は `tests/common/git.rs`(空の一時ファイルを指す等)。
- **[中] MSRV ジョブが落ちる。** 依存グラフ側の最大 `rust-version` は 1.85 で 1.89 と矛盾しないことは確認済み。自コード側も clippy の `incompatible_msrv` が 1.89 を基準に緑。残る穴は **Windows 固有コードの std API** — ローカルでコンパイルされないため `incompatible_msrv` の検査を受けていない。落ちた場合は「該当 API を回避する」を第一手にし、回避できないときだけ `rust-version` を実測値へ引き上げ、`.adr/022`(MSRV の根拠)と `.adr/023`(`home_dir` の非推奨解除に関する記述)を同時に更新する。
- **[中] Windows で ` .yaml` のフィクスチャが作れない。** `cli_add_error.rs` の `tc_task_register_task_034`(ADR-051)は語幹が空白のみのファイルを実際に作る。Win32 が落とすのは末尾の空白・ドットなので先頭空白は残る見込みだが、失敗するとこのテストはスキップ経路を持たず `expect` で落ちる。吸収するなら、ADR-055 に倣って「そのファイル名を作れるか」を実際に試す probe を足し、その結果で `SkipBudget` に加える(件数の決め打ちや `cfg` 分岐にはしない)。
- **[中] `Cargo.lock` が `--locked` で弾かれる。** ロックファイルが最新でなければ全ジョブが即赤になる。これは検出したい事象であり、直し方は `cargo update -w` ではなく「なぜズレたか」を見てからコミットする。
- **[低] stable の更新で CI が突然赤くなる。** ツールチェーンを固定せず現行 stable を使うため、新しい clippy lint が追加された日に赤になりうる(adr.md ADR-004)。同じ理由で **fmt ジョブも赤になりうる** — ローカルは nixpkgs の rustfmt 1.97.1、CI は現行 stable の rustfmt で、整形結果は版によって変わる。対処はどちらも「現行 stable に合わせて直す」ことで(clippy は指摘の解消、rustfmt は `cargo fmt --all` の掛け直し)、版の固定・`allow`・`rustfmt.toml` での抑止はしない(ステップ10)。
- **[低] Windows の SKIP が「静かな緑」に見える。** 権限系10件は Windows では宣言済みスキップになる(ADR-055 の設計どおり)。ジョブサマリーへの列挙で可視化するが、これは検出ではなく記録である。**CI で `container:` を使うとランナーが root になり、unix でも権限系10件が宣言済みスキップに化けて静かに緑になる** — 使わないことと、非 root であることをジョブ側で直接アサートすることが前提条件。なお非 root unix でも SKIP 行はゼロにならない(恒久スキップ `tc_port_clock_005` と `SkipBudget` 自己テスト3件で計4件。実測済み)ので、SKIP の件数を合否の指標には使わない。
- **[低] PR #11 とのコンフリクト。** 条件付き修正が `tests/common/git.rs` / `tests/common/mod.rs` / `util/atomic.rs` に入ると #11 と衝突しうる。マージ順は本 Issue 先行と決めてあるので(スコープ「PR #11 とのマージ順」)、衝突の解消は #11 側で行う。**ただしステップ10 の rustfmt 掛け直しが発火した場合、衝突面はこの3ファイルではなく `cargo fmt --all` が触るソースツリー全域になる。** 発火確率は低い(ローカルの nixpkgs 1.97.1 と現行 stable の整形差)が、起きた場合は「#11 側で解消する」という合意が吸収できる規模かを、掛け直した差分の大きさを見て #11 と確認し直す。確認の結果に関わらず本 Issue が整形を先送りすることはしない — AC-3 を満たさなくなる。
- **[低] Windows ランナーの実行時間。** ウイルス対策スキャンでビルドが遅い。`CARGO_INCREMENTAL=0` を設定して余計な中間生成を避ける。それでも遅ければジョブ単位のタイムアウト(`timeout-minutes`)で暴走だけ止める。

## テスト方針

本 Issue の「テスト」は CI の実行そのもの。既存テストを増やすことは目的ではない。

- **CI が緑であること**が主たる検証。ブランチを push して Actions の7ジョブ(adr.md ADR-008 の撤退を適用した場合はその後のジョブ数)すべてが成功することを確認する。1回目で赤なら、赤の内容を steps.md の条件付きステップで解消して再実行する。
- **環境前提の検査**: unix の stable ジョブが「非 root で走っている」ことを直接アサートし、そのステップが成功していることを確認する。スキップの合否判定は `SkipBudget` に委ねるため、CI 側では SKIP の件数を数えない。
- **スキップの可視化**: 各 OS のサマリーに列挙された SKIP 行を、steps.md ステップ3 に**実行前に**書いた期待集合と突き合わせる(unix は1件、Windows は11件)。一致しない場合は、観測値で期待値を上書きせず、予測が誤っていた理由を先に特定する(AC-6(d))。テストが走らずに終わった実行では、サマリーが「なし」ではなく「テストが走っていない」と表示されることも合わせて確認する。
- **MSRV の実測**: MSRV ジョブのログに、`Cargo.toml` から読み取った版数がそのまま toolchain 名として現れ、`cargo +<版数> build --workspace --all-targets --locked` が成功していることを確認する。
- **隔離の維持**: OS 差の吸収を入れた場合、`crates/pulsen-domain/` に target 述語つき `cfg`(属性形・マクロ形・`cfg_attr`)が 0 件のままであることを grep で確認する(AC-5)。マクロ形 `cfg!(windows)` はこのコードベースの既存の作法(`tests/common/git.rs`)なので、属性形だけを見る grep では歯止めにならない。吸収先の層が妥当かは grep では見えないので、「レビューで見る観点」として人が判断する。
- **振る舞いを足した場合のみユニットテストを足す**: 例えば `util/atomic.rs` に置換の再試行を入れたなら、その振る舞い(「置換が一時的に拒否されても最終的に置き換わる」「回数を使い切ったら Err を返す」)を仕様の言葉で名付けたユニットテストにする。CI を通すためだけの `#[cfg(windows)]` による検証省略はしない。
