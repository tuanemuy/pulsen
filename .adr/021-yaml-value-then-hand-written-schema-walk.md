# 021: YAML は Value 化してから手書きでスキーマ走査する

## ステータス

承認済み

## コンテキスト

ADR-013 はワークフローYAML・config.yaml の未知キーを読み込み時エラーにすることを要求し、spec は `WorkflowParseError` として `YamlSyntax`(重複キー含む)/ `UnknownKey` / `InvalidValue` を**区別**することを要求する。serde の `deny_unknown_fields` を使うと、未知キー・型不一致・値エラーがすべて `serde::de::Error` になり、エラー種の区別が文字列マッチ頼みになる。

## 決定

`serde_yaml_ng`(実測で重複キー検出・エラー位置取得を確認済み)で YAML を `Value` に落とすところまでを外部クレートに任せ、`Value` → `RawWorkflowDoc` / `GlobalConfig` のスキーマ走査は手書きにする。

- 構文エラー・重複キー → `YamlSyntax { message, location }`(config では `Invalid`)
- スキーマに無いキー → `UnknownKey { location, key }`(config では `Invalid`)。`location` は論理パス(例: `statuses.queued`)で表現する
- 型不一致・値の生成失敗 → `InvalidValue { location, message }`
- 空ファイル・null ドキュメント → 「全キー省略」として `Ok`(`Err` にしない)

## 影響

- エラー種の区別が構造的に決まり、文字列マッチが要らない。エラーメッセージの文言を spec の案内に合わせて自由に組める
- トレードオフ: 走査コードを書く量が増える。YAML クレートを差し替えても影響が `adapter::yaml` に閉じる点は利点でもある
- 注意: `serde_yaml_ng` のスカラー解決は YAML 1.2 core schema 相当で、`no` / `off` / `yes` / `n` は `Value::String` になり bool には変換されない(実測確認)。bool になるのは `true` / `false` のみ。「YAML 1.1 の暗黙 bool 変換」を前提としたテスト期待を書かない
