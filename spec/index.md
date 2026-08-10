# 設計インデックス: pulsen

AIエージェントタスクの汎用スケジューラー。要件は [requirements.md](requirements.md) を参照。

## 進捗

| フェーズ | 状態 | 成果物 |
|---|---|---|
| Phase 0: 準備 | 完了 | [requirements.md](requirements.md) |
| Phase 1: シナリオ設計 | 完了 | [scenario/index.md](scenario/index.md) ほか5カテゴリ |
| Phase 2: ページ設計 | 完了 | [pages/index.md](pages/index.md) |
| Phase 3: 技術設計 | 未着手 | spec/domains/ ほか |
| Phase 4: マニュアルテスト | 未着手 | spec/manual-tests/ |

## Phase 0 での決定事項

- tickは `tick` サブコマンド。1呼び出し = 1パス。定期性は外部スケジューラーが担う
- 設定・状態はグローバルホーム `~/.pulsen/`(`PULSEN_HOME` / `--home` で上書き可)に集約
- タスク登録は `add`(登録のみ)。旧 `run` コマンドは廃止。即実行は `add && tick` のシェル合成で行い、`--now` フラグは設けない(完全等価のため。ADR-007)
- ワークフローYAMLは `workflows/` から名前解決。ファイルパス直接指定も可
