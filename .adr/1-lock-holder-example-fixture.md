# ADR: ロックの別プロセスフィクスチャは `examples/lock_holder.rs` で供給する

## ステータス

承認済み

## コンテキスト

spec/testcases/ports/exclusive-lock.md は「ロックを取得・保持する別プロセス」「ロックを保持したまま強制終了できるテスト用プロセス」をフィクスチャとして明示的に要求する(TC-002〜005)。本スライスのバイナリは `pulsen`(サブコマンドは `add` のみ)だけで、ロックを保持し続けるモードを持たない。std の `File::try_lock` は同一プロセス内の再取得の挙動が未規定なので、同一プロセスでの代替も取れない(spec も「同一プロセス内の再取得では検証しない」と明記)。

## 決定

- `crates/pulsen/examples/lock_holder.rs` を用意する。引数でロックファイルのパスを受け取り、取得できたら `locked` を stdout に1行書いてフラッシュし、標準入力が閉じるまで保持し続ける
- 統合テストは `env!("CARGO_BIN_EXE_pulsen")` の親ディレクトリ配下 `examples/lock_holder` として実行ファイルを解決する(`cargo test` は example もビルドするため常に存在する)。強制終了は `Child::kill()`
- 実測で確認済み: 保持中の別ハンドルからの `try_lock` は `TryLockError::WouldBlock`、保持プロセスを `kill` した直後は `Ok(())`
- `LockError::Failed` の再現は、ロックファイルのパスにディレクトリを置く(実測: `IsADirectory` で `open` が失敗する)。権限操作と違って root 実行でも Windows でも成立するため、環境依存スキップにしない
- サブコマンドとしてロック保持モードを足すことはしない(利用者に見えるインターフェースを増やさない)

## 影響

- 追加の bin ターゲットを増やさずに済み、`cargo install` の成果物が `pulsen` 1つのままになる。`LockError::Failed` が環境非依存に再現できる
- トレードオフ: example の出力パスがビルドレイアウトに依存する(`target/<profile>/examples/`)。パス解決を1箇所のヘルパーに閉じる
