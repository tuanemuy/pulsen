# テストケース: RetryTask(retry)

参照元: [ユースケース: Task — RetryTask](../../usecases/task.md)

関連仕様: [requirements §10](../../requirements.md)、[pages retry・縮退表](../../pages/index.md)、[domains/task](../../domains/task.md)、[scenario/intervention](../../scenario/intervention.md)

## 正常系

| 前提条件 | 操作 | 期待結果 | 実装ステータス |
|---|---|---|---|
| リトライ上限超過(`RetryLimitExceeded`)で stopped のタスクがある | retry する | attempt_count・judge_attempt_count・spawn_fail_count が全て 0 にリセットされ、実行状態が pending に戻り 0 で終了する | |
| 判定不能の上限超過(`JudgeLimitExceeded`)で stopped のタスクがある | retry する | 受理されて pending に戻る | |
| 連続spawn失敗の上限超過(`SpawnFailLimitExceeded`)で stopped のタスクがある | retry する | 受理されて pending に戻る | |
| 人間の abort(`Aborted`)で stopped のタスクがある | retry する | 受理されて pending に戻る | |
| stopped のタスクを retry する | retry 後の状態を照会する | タスクステータスは変更されず、再開されるタスクステータスが結果に表示される | |
| 実行履歴のある stopped タスクを retry する | retry 後の状態を照会する | 現在attempt参照(attempt番号・runディレクトリパス)と workspace は保持されたまま pending になる | |

## 異常系

いずれの拒否・エラーでもタスクファイルには書き込まない。

| 前提条件 | 操作 | 期待結果 | 実装ステータス |
|---|---|---|---|
| config.yaml が存在しない・パース不能・読めないのいずれか | retry する | 非0で終了する(状態は変更しない。pages ※1) | |
| 別の操作が排他ロックを保持している | retry する | 「別の操作が実行中」として非0で終了する | |
| ロック機構自体が異常(`LockError::Failed`) | retry する | 実行環境エラーとして非0で終了する | |
| 指定IDのタスクが現役にもアーカイブにも存在しない | retry する | タスク不在として非0で終了する | |
| 指定IDのタスクがアーカイブ済み | retry する | アーカイブ済みは操作不可として非0で終了する | |
| 指定IDのタスクファイルがパース不能(`Corrupt`) | retry する | 非0で終了し、破損ファイルへの書き込みは行わない | |
| タスクが failed | retry する | 拒否され「放置すれば自動リトライされる」と案内して非0で終了する | |
| タスクが pending | retry する | 拒否され「既に実行待ち」と案内して非0で終了する | |
| タスクが completed | retry する | 拒否され「判定済み。次のtickが遷移させる」と案内して非0で終了する | |
| タスクが launching | retry する | 拒否され「先に abort」と案内して非0で終了する | |
| タスクが running | retry する | 拒否され「先に abort」と案内して非0で終了する | |
| タスクIDとして不正な文字列を指定する | retry する | 入力境界の検証エラーとして非0で終了する | |

## 境界値

| 前提条件 | 操作 | 期待結果 | 実装ステータス |
|---|---|---|---|
| タスクIDに空文字列を指定する | retry する | 検証エラー(`Empty`)として非0で終了する | |
| タスクIDに65文字の文字列を指定する | retry する | 検証エラー(`TooLong`)として非0で終了する | |
| 64文字の有効なIDを持つ stopped タスクがある | retry する | 受理されて pending に戻る | |
| 1文字の有効なID(英数字)を持つ stopped タスクがある | retry する | 受理されて pending に戻る | |
| タスクIDに `[a-z0-9-]` 以外の文字(大文字・`_` 等)を含む文字列を指定する | retry する | 検証エラー(`InvalidChar`)として非0で終了する | |
| タスクIDの先頭が `-` の文字列を指定する | retry する | 検証エラー(`InvalidLeadingChar`)として非0で終了する | |

## エッジケース

| 前提条件 | 操作 | 期待結果 | 実装ステータス |
|---|---|---|---|
| スナップショットのみ読めない stopped タスク(`SnapshotUnreadable` / DegradedTask)がある | retry する | 受理されて pending に戻り 0 で終了するが、tick に拾われないためスナップショット修復が必要である旨の警告が表示される(pages ※7) | |
| DegradedTask を retry する | 保存後のタスクファイルを確認する | 読めないスナップショットフィールドは元の内容のまま温存される(修復材料を消さない) | |
| `state/tasks/`・`state/archive/` ディレクトリが存在しない | retry する | タスク不在として非0で終了する(pages 縮退表) | |
| 未通知(`notified_at` なし)の stopped タスクがある | retry する | 通知の有無によらず受理される(人間が操作した = 気づいている。flows F3) | |
| stopped タスクを retry 済みで pending になっている | 同じタスクをもう一度 retry する | stopped ではないため「既に実行待ち」と案内して非0で終了する | |
