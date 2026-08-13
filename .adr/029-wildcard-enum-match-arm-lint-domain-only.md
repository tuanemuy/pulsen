# 029: `clippy::wildcard_enum_match_arm` はドメインクレートにのみ適用する

## ステータス

承認済み

## コンテキスト

CLAUDE.md は「`match` でワイルドカード(`_`)を避ける」と定め、当初は workspace lints に `clippy::wildcard_enum_match_arm` を warn として設定する方針だった。しかし受け入れ基準が `cargo clippy -- -D warnings` の通過を求めるため、warn 設定は実質 deny になる。

実測で、この lint は `#[non_exhaustive]` な外部 enum の `_` アームにも発火することを確認した(`std::io::ErrorKind` の `_` に対して既知の全バリアント列挙を提案する。網羅 match はそもそも書けない)。同じことが `std::fs::TryLockError` / `serde_yaml_ng::Value` / `serde_json::Value` にも当てはまる。

## 決定

`crates/pulsen-domain/Cargo.toml` の `[lints.clippy]` にだけ設定する。workspace lints には全クレート共通のもの(`unsafe_code = "forbid"` 等)だけを置く。`pulsen` クレートには掛けない。

（`pulsen` はのちに `unsafe_code` を `deny` にするため workspace lints の継承をやめた。ADR-100）

## 影響

- CLAUDE.md の規約が本来の適用対象(ドメインの網羅 match)で強制され、外部 enum を扱うアダプターに `#[allow]` を撒かずに済む
- トレードオフ: `pulsen` クレート内のドメイン enum への `_` は lint で捕まらない。レビューで見る
