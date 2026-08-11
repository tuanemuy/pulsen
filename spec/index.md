# 設計インデックス: pulsen

AIエージェントタスクの汎用スケジューラー。要件は [requirements.md](requirements.md) を参照。

## 進捗

| フェーズ | 状態 | 成果物 |
|---|---|---|
| Phase 0: 準備 | 完了 | [requirements.md](requirements.md) |
| Phase 1: シナリオ設計 | 完了 | [scenario/index.md](scenario/index.md) ほか5カテゴリ |
| Phase 2: ページ設計 | 完了 | [pages/index.md](pages/index.md) |
| Phase 3: 技術設計 | 完了(クロスフェーズ検証済み) | [domains/index.md](domains/index.md) / [usecases/task.md](usecases/task.md)・[usecases/execution.md](usecases/execution.md) / [flows/index.md](flows/index.md) / testcases/(task 5・execution 3・ports 10) |
| Phase 4: マニュアルテスト | 完了(レビュー5ラウンドで収束) | [manual-tests/index.md](manual-tests/index.md) ほか5カテゴリ・162件(正常 61 / 異常 80 / 境界 21) |

## Phase 0 での決定事項

- tickは `tick` サブコマンド。1呼び出し = 1パス。定期性は外部スケジューラーが担う
- 設定・状態はグローバルホーム `~/.pulsen/`(`PULSEN_HOME` / `--home` で上書き可)に集約
- タスク登録は `add`(登録のみ)。旧 `run` コマンドは廃止。即実行は `add && tick` のシェル合成で行い、`--now` フラグは設けない(完全等価のため。ADR-007)
- ワークフローYAMLは `workflows/` から名前解決。ファイルパス直接指定も可

## Phase 2 後の改訂での決定事項

ポーリング型ワークフロー(チェック → なければ待機 → あれば処理)を成立させるための改訂。

- 判定の分類は completed / failed / skipped の3値。判定コマンドのexit codeプロトコルは 0 / 10 / 20 / それ以外=判定失敗 の4値で、skippedは判定コマンドでのみ表現できる(ADR-008)
- リトライ系カウンタは連続失敗を数える。completed / skipped の確定で attempt_count・judge_attempt_count をリセット(ADR-009)
- ワークフロー定義の循環(自己参照含む)は正当な表現として明示的に許容(ADR-010)
- `agents:` にはAIエージェントに限らず任意のコマンドを登録できる(既存設計の明文化)
- runディレクトリのgcを保持期間設定 `run_retention` としてスコープ内化。未設定なら無効(ADR-011)

## Phase 3(技術設計)着手時の決定事項

Phase 2 完了時の申し送り5件は、Phase 3 着手時にすべて確定し ADR に記録した:

- ツール操作(worktree作成・削除、アーカイブ移動)の失敗時は実行状態を failed に書き換えて観測させる(ADR-012)
- ワークフローYAML・config.yaml の余剰キー(未知キー・動作種別に無関係なキー)は読み込み時エラー。検証は二層(構造は読み込み時、内容は参照時)(ADR-013)
- リトライ上限は常に現在のタスクステータスの定義から取る。クリーンアップステータスは上書き不可のため常に組み込みデフォルト2。リトライ上限・timeout のデフォルトは組み込み定数とし config.yaml キーを設けない(ADR-014)
- ワークフロー定義のスナップショットはタスクファイルに正規化された構造として埋め込む(ADR-015)
- テンプレート展開失敗は launching 記録前の同期spawn失敗経路として処理する(spawn_fail_count 加算・失敗要因記録・無効化マーカー不要・attempt採番なし。ADR-016)

設計中に追加で下した決定:

- ドメイン境界は definition / task / execution の3分割(依存方向は Definition ← Task ← Execution。ADR-017)
- notify_cmd に組み込み60秒のtimeoutを適用(requirements §8 に反映済み。ADR-018)
- worktree のパス・ブランチ規約(`worktrees/<task-id>`・`pulsen/<task-id>`)を requirements §5・§9 に明文化
- UnitOfWork ポートは定義しない(全書き込みが単一タスクファイルに閉じ、複数リソース処理は冪等な再導出と at-least-once で回復。domains/index.md)
