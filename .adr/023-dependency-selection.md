# 023: 依存クレートの選定

## ステータス

承認済み

## コンテキスト

CLI・YAML・JSON・乱数・一時ファイルが必要になる。ドメインは zero-dep(ADR-019)なので、すべて `pulsen` クレートの依存になる。

## 決定

| 用途 | 採用 | 理由 / 却下した選択肢 |
|---|---|---|
| CLI パース | `clap`(derive) | サブコマンドの隠し表示(後続スライスの `wrapper` に必要)・ヘルプ生成・usage エラーの exit code 2 を標準で持つ。`pico-args` / `lexopt` は軽いが、ヘルプと案内の質を自前で持つことになる |
| YAML | `serde_yaml_ng`(`Value` としてのみ使用) | 重複キー検出とエラー位置を実測で確認。`serde_yaml` は deprecated、`serde_yml` は unmaintained 宣言。`saphyr` は 0.0.x で API 未安定。使用箇所を `adapter::yaml` に閉じるため差し替え可能 |
| JSON | `serde` + `serde_json`(`raw_value`) | 人間可読なタスクファイル(requirements §9)。`RawValue` が `save_degraded` の破損スナップショット温存(ADR-025)に必要 |
| 乱数 | `getrandom` | タスクIDのランダム成分にのみ使う。`rand` は分布・アルゴリズムを必要としないため過剰 |
| 一時ファイル | `tempfile` | アトミック置換の一時ファイル(同一ディレクトリ・後始末)とテストの一時ディレクトリ |
| 適合テスト・テストダブル | `pulsen-conformance`(**dev-dependency**) | `[dependencies]` に入れると本番バイナリにテストスイートが載る |

採らないもの:

- `home` — `std::env::home_dir()` は Rust 1.97 で非推奨が解除済みで Windows の挙動も修正されている(実測確認)。std で足りるものを外部クレートにする理由がない。ホーム解決は合成ルート1箇所(ADR-031)
- `time` / `chrono` — RFC3339 変換をドメインに持たせたため不要になった(ADR-020)

## 影響

- いずれも用途が1〜2モジュールに閉じ、差し替え可能。本番依存は5用途・6クレートに収まる(`tempfile` は `util::atomic` のアトミック置換で本番コードが使う)
- トレードオフ: `serde_yaml_ng` はフォーク系クレートであり、長期のメンテナンスは保証されない。`Value` 化にしか使わないことで乗り換えコストを小さく保つ。暦計算を自前で持つ(ADR-020)
