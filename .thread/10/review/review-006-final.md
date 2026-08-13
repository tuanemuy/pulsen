# レビュー 006（最終確認 / 3観点通し）

**対象:** PR #12 `issue/10/ci-msrv-cross-platform` → `main`
**HEAD:** `714d79f`
**観点:** CI・ビルド基盤 / 共通ユーティリティ・並行性・OS 抽象 / テスト・ドキュメント整合
**判定:** マージ可（Blocker なし）

## 検証した事実

- **実測の出典。** `gh run view 31666626824` の `headSha` は `e5249819daca808038a42db1bfdc6ec07d348ae8`、`conclusion: success`、7ジョブすべて success。`git log origin/main..HEAD -- .github/ crates/` で `e524981` より後に現れるのは `714d79f` だけで、その中身は `.thread/**` と `crates/pulsen-conformance/HOOKS.md`（ドキュメント）のみ。**`e524981` は実際に「最後にコードまたはワークフローが変わったコミット」である。**
- **現 HEAD の CI。** run 31667058756 が走行中で、`fmt` / `test`(ubuntu, macOS) / `msrv`(3 OS) が pass、`test (windows-latest)` が pending。docs-only 差分なので結果は `e524981` と同じになる見込みだが、マージ前に緑を確認すること。
- **AC-1:** `on` は push(main) / pull_request / workflow_dispatch。`permissions: contents: read`。`cancel-in-progress: ${{ github.event_name == 'pull_request' }}`。`defaults.run.shell: bash`。ジョブは fmt 1 + test 3 + msrv 3 = 7。撤退条件（ADR-008）は適用されていない。
- **AC-2:** 3 OS で `cargo build --workspace --locked` / `cargo test --workspace --locked --no-fail-fast -- --nocapture` / `cargo clippy --workspace --all-targets --locked -- -D warnings`、`fail-fast: false`、全緑。`continue-on-error` は不使用。`clippy` の `if: success() || failure()` は「テストが落ちても指摘を揃える」ためで、赤は赤のまま。
- **AC-3:** `rustup update stable --no-self-update` → `rustup default stable` → `rustup component add rustfmt` を踏んだうえで `cargo fmt --all --check`。`rustc --version` / `cargo fmt --version` をログに残す。`rustfmt.toml` は追加されていない。ローカル（nixpkgs 1.97.1）でも `cargo fmt --all --check` が exit 0。
- **AC-4:** 版数のハードコードなし。`cargo metadata --no-deps --locked` から `[.packages[].rust_version] | map(select(. != null)) | unique` を取り、`grep -c .` が 0 なら失敗・複数なら失敗。値は `env: MSRV` 経由で渡し（`${{ }}` をシェル構文へ直接展開しない）、`rustc "+$MSRV" --version` で名乗らせてから `cargo "+$MSRV" build --workspace --all-targets --locked`。`Cargo.toml` は `rust-version = "1.89"`、3メンバーとも `rust-version.workspace = true` で割れなし。`.adr/022` / `.adr/023` は MSRV 据え置きにつき更新不要。
- **AC-5:** `grep -rnE '(cfg!?|cfg_attr)\([^)]*(unix|windows|target_os|target_family|target_env|target_arch|target_pointer_width)' crates/pulsen-domain/src/` が 0 件（実行して確認）。
- **AC-6:** (a) `id -u` を代入で受けてから `case` で判定し、`0` と非数値の両方を失敗にする（fail-open なし）。(b) サマリーは `if: always()`、`test.log` の有無とテスト結果行の有無で3状態に分岐。`pulsen_conformance-` のバイナリ区間を除外し、除外件数を表示。(c) `container:` / 単一テストターゲット指定なし。(d) **期待集合は初回コミット `09e282c` の steps.md ステップ3 に書かれており（ubuntu/macOS 1件・Windows 11件）、その後のコミットで書き換えられていないことを `git diff 09e282c..HEAD -- .thread/10/steps.md` で確認した。** 観測と一致しており、事後更新は発生していない。
- **AC-7:** Action は `actions/checkout@v7` のみ（`persist-credentials: false`）。fmt は `rustup`、test は `rustup` + `awk`/`cat`/`grep`/`id`/`tee`、msrv は `rustup`/`cargo`/`jq`/`grep` を検査。`sort` はワークフロー全体で 0 件。ワークフローで実際に使われている外部コマンドは検査対象に収まっている（`cat` はサマリーの `summary()`、`tee` はテストステップ、`awk`/`grep` は報告ステップ、`id` は非 root アサート）。
- **AC-8:** HOOKS.md の表に ubuntu / macOS / Windows の3列が入り、「3ランナーでの実測」節に出典 run・コミット・実行コマンド・テストバイナリ本数・自己テスト3件の扱い・Windows でのみ存在しない `#[cfg(all(test, unix))]` 3件のカバレッジ差まで書かれている。行を追加するときの `未測定` 運用も明記。
- **吸収先の層（機械確認しない観点）:** 変更は `crates/pulsen/src/util/atomic.rs` / `crates/pulsen/src/adapter/task_file.rs`（テストフィクスチャ）/ `crates/pulsen/src/adapter/task_repository.rs` の3ファイルのみ。ドメイン層・ポート定義は無変更。`cfg` の決め打ちで分岐を切っているのは `transiently_denied` の分類関数だけで、再試行の有無は「OS が返したコードが一時的な拒否か」という値の判定に還元されている。
- **再試行ループ:** `MAX_ATTEMPTS = 10`、`retry_waits()` が 1,2,…,256ms の9本を返し、和は 511ms。ループは列が尽きた時点で元のエラーを返すので試行回数は 10 に一致し、公称値と doc / ADR-010 / ADR-012 / ADR-013 の記述が揃っている。`persist_with_retry` は失敗時に `NamedTempFile` を持ち回り、打ち切り時は Drop で消えるため「失敗しても一時ファイルを残さない」契約が保たれる。
- **`read_atomic` の適用範囲:** `fs::read` を置き換えたのは `FsTaskRepository` の3経路（`lookup` / `list` / `save_degraded`）。`config_store` / `workflow_store` は `load` のみの読み取り専用ポートで、pulsen 側がアトミック置換する経路を持たないため対象外なのは妥当。

## Blockers

なし。

## Warnings

1. **`.thread/10/progress.md:3` の「コミット `e524981` = 現在の HEAD」が事実と食い違う。**
   HEAD は `714d79f` であり、この行を書いた commit `714d79f` の時点で既に偽だった。同じファイルの L20 が「出典は最後にコードまたはワークフローが変わったコミットに対する実測とする」という方針を明記しており、L3 だけがその方針と別の（かつ誤った）言い方をしている。PR 本文と testing.md は「最後にコードとワークフローが変わったコミット」「PR #11 のマージ前」と正しく書けているので、揃っていないのは L3 のみ。
   **マージをブロックしない理由:** 誤りの範囲が `.thread/` の作業メモ1行に閉じ、実測値・AC の充足・成果物（ワークフロー・コード・HOOKS.md）のいずれにも影響しない。修正するなら `= 最後にコードとワークフローが変わったコミット` に置き換えるだけで足りる。

## カバレッジ

| ファイル | 状態 | 見たこと |
|---|---|---|
| `.github/workflows/ci.yml` | A | 全文精読。AC-1 / 2 / 3 / 4 / 6 / 7 を条文と突き合わせ。`sort` / `container:` / 単一ターゲット指定 / 版数ハードコードが 0 件、Action は checkout のみ、前提検査の対象が実使用コマンドを網羅していることを確認 |
| `.thread/10/adr.md` | A | ADR-008（撤退条件・停止規則）/ ADR-010・012・013（再試行の分類・上限・読み手側の吸収）を実装と照合。511ms・`MAX_ATTEMPTS` 10・`retry_while_transient` への集約が記述どおり |
| `.thread/10/plan.md` | A | AC-1〜8 とスコープ・リスク欄を精読。契約として使用 |
| `.thread/10/progress.md` | A | 精読。run 表・スキップ実測・未検証事項を CI の実測結果と照合。L3 に Warning 1 |
| `.thread/10/steps.md` | A | ステップ1〜4・12 と、ステップ3 の期待スキップ集合を精読。集合が `09e282c` 以降変更されていないことを diff で確認 |
| `.thread/10/testing.md` | A | 確認手順と PR #11 への影響欄を確認。出典コミットの記述は正しい |
| `.thread/10/review/review-001〜005*.md`（18件） | A | 過去ラウンドの台帳。**ゼロベース方針のため判定は引き継がず、Phase 8 で削除予定の既知ファイルとして内容レビューはスキップ** |
| `.thread/10/review/triage.md` | A | 同上（レビュー台帳。スキップ） |
| `crates/pulsen-conformance/HOOKS.md` | M | 差分全文を精読。3列の実測・出典 run・自己テスト3件の除外・Windows のみ存在しない3件のカバレッジ差・`未測定` 運用を確認。AC-8 充足 |
| `crates/pulsen/src/adapter/task_file.rs` | M | 差分精読。`absolute()` が `MAIN_SEPARATOR` からプラットフォーム固有の絶対パスを組み、期待 JSON から綴りを追い出す形。アサーションの緩和なし、ドメイン無変更 |
| `crates/pulsen/src/adapter/task_repository.rs` | M | 差分精読。`fs::read` → `read_atomic` の3箇所と、遅延の合成（最大2倍 / N 倍）を明記したモジュール doc |
| `crates/pulsen/src/util/atomic.rs` | M | 差分全文を精読。`retry_while_transient` の試行回数・打ち切り・状態持ち回り、`transiently_denied` の cfg 分岐、`read_atomic` の追加、ユニットテスト9件（分類・打ち切り・予算・3経路の吸収・一時ファイル非残留）を確認 |
