# 037: プラットフォーム既定のパス区切り集合は `#[cfg]` ではなく `MAIN_SEPARATOR` から選ぶ

## ステータス

承認済み

## コンテキスト

ADR-034 は `WorkflowRef::parse` の区切り文字集合を定数に切り出し、「プラットフォーム既定の集合は `#[cfg]` で選ぶ」としていた。一方 Issue #1 の受け入れ基準は「OS 依存分岐の隔離を機械的に確認できること」——`#[cfg(unix)]` / `#[cfg(windows)]` の出現箇所が `crates/pulsen/src/{adapter,util}/` 配下に限られ、`crates/pulsen-domain/` には1つも現れないこと——を求めている。ドメインに `#[cfg(windows)]` を1つでも置くと、この確認が grep で成立しなくなる。

## 決定

既定集合の選択を `std::path::MAIN_SEPARATOR` の比較で行う。

```rust
pub const fn platform_separators() -> &'static [char] {
    if std::path::MAIN_SEPARATOR == '\\' { WINDOWS_SEPARATORS } else { POSIX_SEPARATORS }
}
```

`MAIN_SEPARATOR` は std が提供する const であり、条件分岐はコンパイル時に畳まれる。ドメインからは条件付きコンパイルが消え、判定ロジックは引き続き集合を引数に取る純粋関数(`parse_with_separators`)に閉じる。

## 影響

- ドメインクレートに `#[cfg]` が1つも現れず、OS 依存分岐の隔離を grep で機械的に確認できる
- ADR-034 の「集合はデータとして選ぶ / 判定は純粋関数」という性質はそのまま保たれる
- トレードオフ: プラットフォーム既定の集合が「区切り文字が `\` かどうか」という間接的な条件で決まる。両方の集合を明示的に渡すユニットテストがあるため、既定の選択誤りは集合そのもののテストでは検出できない点は ADR-034 と変わらない
