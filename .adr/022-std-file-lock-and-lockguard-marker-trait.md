# 022: 排他ロックは標準ライブラリの `File::try_lock` で実装し、`LockGuard` はドメインのマーカートレイトにする

## ステータス

承認済み

## コンテキスト

requirements §4.3 はファイルの排他ロックを OS 依存操作として挙げ、spec は「ブロックしない」「取得できないのは `Ok(None)`」「機構の異常は `Err(Failed)`」「保持プロセスの異常終了でも OS が解放」を要求する。実現手段として `fs4` / `fd-lock` 等の外部クレートと標準ライブラリがある。

Rust 1.89 で `std::fs::File::try_lock() -> Result<(), std::fs::TryLockError>` が安定化しており(本環境は 1.97)、`TryLockError::WouldBlock` と `TryLockError::Error(io)` が spec の2分岐にそのまま対応する(実測確認済み)。

## 決定

- 標準ライブラリの `File::try_lock` を使い、ロック用の外部クレートを追加しない。ロックファイルは `<home>/state/lock`
- `LockGuard` はドメインが定義するマーカートレイトとし、ポートは `Result<Option<Box<dyn LockGuard>>, LockError>` を返す。実体(`File` を保持し `Drop` で解放する構造体)はアダプターに置く
- workspace の `Cargo.toml` に `rust-version = "1.89"` を明記する(`File::try_lock` が無いツールチェーンを引いたときに原因が即座に分かる)

## 検討した代替案

- ポートに関連型 `type Guard: LockGuard;` を持たせ、`Box` と動的ディスパッチを避ける — spec/domains/execution.md は `try_acquire(&self) -> Result<Option<LockGuard>, LockError>` と書いており、ガードの具体型を利用側に見せない意図がある。関連型にすると型引数が合成ルートまで伝播し、実装ごとに結線先の型が変わる。取得は1コマンド1回で、動的ディスパッチのコストは無視できる

## 影響

- 依存が減り、Windows/POSIX の差異は標準ライブラリが吸収する。ドメインはロックの寿命だけを知り、ハンドルの型を知らない
- トレードオフ: `rust-version` が 1.89 以上に固定される。`Box<dyn>` により動的ディスパッチが1箇所入る
