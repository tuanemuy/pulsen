# 指摘台帳 — Issue #10 実装レビュー

| Key | 初出 | 判定 | 理由（一行） | 再指摘 |
|---|---|---|---|---|
| `HOOKS.md:実測/テストバイナリ数の誤り` | R1 | fix | 19本は集計ミス（サマリーがエコーしたスクリプト行を数えた） | 0 |
| `progress.md:Issue #10 コメント/未実施を実施済みと記載` | R1 | fix | コメント0件。steps.md 12 の記録先が未達 | 0 |
| `progress.md:PR #11 引き継ぎ/未実施を完了と記載` | R1 | fix | #11 へのコメント0件。「ステップ1〜12 すべて完了」も偽 | 0 |
| `PR#12:本文/CI 実行前のまま` | R1 | fix | コード変更3ファイルの理由が本文から辿れず、参照 ADR も古い | 0 |
| `ci.yml:cargo test/--no-fail-fast 欠落` | R1 | fix | 最初の失敗ターゲットで打ち切られ、残りが未観測のまま緑扱いの余地を生む | 0 |
| `ci.yml:非rootアサート/fail-open` | R1 | fix | `if` 条件内では errexit が効かず、`id` が壊れると無言で通る | 0 |
| `ci.yml:grep \|\| true/コメントが不正確` | R1 | fix | その位置では成り立たない理由を述べている | 0 |
| `ci.yml:msrv版数/run への直接展開` | R1 | fix | 値の出所がフォークの Cargo.toml。env 経由に寄せる | 0 |
| `ci.yml:SKIPサマリー/除外の不可視` | R1 | fix | 区間まるごと除外なので、将来実在ケースが混ざると黙って消える | 0 |
| `task_repository.rs:lookup/list/読み手側の共有違反が未処理` | R1 | fix | 書き手だけ塞いでも delete-pending 窓で読み手が ACCESS_DENIED を受ける。メイン裁定を参照 | 0 |
| `util/atomic.rs:テスト/上限の未検証` | R1 | fix | 10回・511ms を観測するアサーションが無く、seam を置いた目的が未達 | 0 |
| `util/atomic.rs:is_transient/最終試行で呼ばれない` | R1 | fix | 分類器が副作用フックとして使われ短絡する | 0 |
| `util/atomic.rs:retry_while_transient/未使用の一般化` | R1 | fix | 型引数 T が両呼び出し元で `()` 固定 | 0 |
| `util/atomic.rs:MAX_ATTEMPTS-1/減算オーバーフロー` | R1 | fix | 定数値にのみ依存した安全性 | 0 |
| `util/atomic.rs:doc/遅延特性の未記載` | R1 | fix | 最大511msブロックしうることと、伸びたクラッシュ窓が契約に書かれていない | 0 |
| `HOOKS.md:実測/測定コミットの誤り` | R1 | fix | 実測は af24360。9d54376 の Windows は赤だった | 0 |
| `HOOKS.md:実測/OS差なしの過剰主張` | R1 | fix | `#[cfg(all(test, unix))]` 3件が Windows では非コンパイルで SKIP にも出ない | 0 |
| `HOOKS.md:実測3列/更新運用と噛み合わない` | R1 | fix | 新規行の3列を埋める手段・空欄の意味が未定義 | 0 |
| `.thread/10:ADR参照/裸の連番が .adr と衝突` | R1 | fix | スレッド内連番と `.adr/NNN` が区別できない | 0 |
| `adr.md:ADR-012/存在しない定数を参照` | R1 | fix | `PERSIST_ATTEMPTS` は `MAX_ATTEMPTS` に改名済み | 0 |

R1: 新規 20 / fix 20 / fix-editorial 0 / wont-fix 0 / defer 0 / 継承 0（方針フェーズ: 省略 — 見送り判定ゼロ。唯一の設計判断を要する指摘（読み手側の共有違反）はメインが裁定。観点別 fix: ci 5 / util 6 / test-docs 9）

## メインの裁定（読み手側の共有違反 — util W-001）

書き手側（`write_atomic` / `rename_atomic`）だけを塞いだ状態は非対称で、**読み手側も同じ窓で `ERROR_ACCESS_DENIED` を受ける**（`MoveFileExW` の delete-pending 窓）。CI は緑だったが、ADR-012 で確立した「未観測 ≠ 赤でない」をここでも適用する。CI が間欠的に赤くなる状態を残すことは、宣言を実測に変えるという本 Issue の目的を損なう。

- **`crates/pulsen/src/adapter/task_repository.rs` の読み取り経路に、書き手側と同じ分類（`transiently_denied`）と上限を適用する。** アダプター層に閉じるので ADR-008 の「本 Issue で扱う」側。
- **エラーの意味は変えない。** 上限に達したら従来どおり `ReadError::Io` を元のエラーで返す。読み取りが失敗しなくなるのではなく、一時的な拒否だけを吸収する。
- **適合スイート・ユニットテストの期待は緩めない。** 現状 `atomic.rs` のユニットテストは読み取り失敗を許容し、適合スイートは許容しないという非対称がある。これは**適合スイート側が正**（読み手はロックなしで常に一貫した内容を見る、が契約）なので、ユニットテスト側の許容を緩和として残さず、契約に合わせる方向で揃える。
- 分類・上限の定数は書き手側と**同一の出典**を使う。二重化しない。
