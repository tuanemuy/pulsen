# テストケース: AbortTask

参照元: [../../usecases/execution.md](../../usecases/execution.md)(AbortTask)

タスクを停止して stopped を確定する。分岐は実行状態のラベルではなく kill 対象の有無で決まり、CLI 自体が確定させる(次のtickを待たない)。

## 正常系

| 前提条件 | 操作 | 期待結果 | 実装ステータス |
|---|---|---|---|
| running のタスク・プロセス生存(starttime 照合一致) | abort を実行する | 照合付きでプロセスグループ相当が kill され、`abort` で `Stopped { Aborted, notified_at: None }` が保存され、通知が実行される。killed = true で 0 終了 | |
| running のタスク・プロセス死亡(starttime 取得不能) | abort を実行する | kill せずに進み、stopped の記録と通知のみ行う。killed = false で 0 終了 | |
| launching のタスク・pid ファイルあり(starttime あり)・照合一致 | abort を実行する | runディレクトリの pid ファイルの同定情報で照合付き kill を行い、stopped を記録する | |
| launching のタスク・pid ファイルなし | abort を実行する | 無効化マーカーを書き、pid を再確認する。なお存在しなければ kill せず stopped を記録する(遅延起動したラッパーは pid 書き込み後のマーカー確認で終了するため、凍結後にエージェントが走り出さない) | |
| launching のタスク・マーカー書き込み後の再確認で pid が現れていた | abort を実行する | 照合付き kill を行って stopped を記録する | |
| launching のタスク・runディレクトリ自体が不在 | abort を実行する | `write_invalidation_marker` がディレクトリごと作成してマーカーを書き、pid 再確認を経て stopped の記録のみを行う(マーカープロトコルを維持) | |
| pending のタスク | abort を実行する | プロセス操作なしで stopped を記録し、通知する(次のtickによる起動が止まる) | |
| failed のタスク | abort を実行する | プロセス操作なしで stopped を記録する(以降の自動リトライが止まる) | |
| completed のタスク | abort を実行する | プロセス操作なしで stopped を記録する(次ステータスへの遷移が打ち切られる)。`current_attempt.process` が Some でも観測しない | |
| すでに stopped のタスク | abort を実行する | 何も変更せずその旨を表示して 0(冪等成功。already_stopped = true) | |
| `SnapshotUnreadable`(DegradedTask)のタスク | abort を実行する | 通常どおり進む(abort はスナップショット非依存)。`degraded.abort` → `save_degraded` で stopped が記録される | |
| stopped 記録後の通知が成功する | abort を実行する | notified_at が記録され、kill の有無を含む結果が表示されて 0 終了 | |
| notify_cmd 未定義 | abort を実行する | stopped の記録のみ行い、通知せず notified_at も書かない。0 終了 | |

## 異常系

| 前提条件 | 操作 | 期待結果 | 実装ステータス |
|---|---|---|---|
| task_id が `TaskId::parse` を通らない(不正な形式) | abort を実行する | 非0 で終了する。何も変更しない | |
| config.yaml が存在しない・パース不能・読めない(Io)のいずれか | abort を実行する | 非0 で終了する(共通事項の `ConfigStore::load` 失敗)。何も変更しない | |
| 別の操作が排他ロックを保持している | abort を実行する | 非0 で終了する。何も変更しない | |
| ロック機構自体が異常(`LockError::Failed`) | abort を実行する | 実行環境エラーとして非0 で終了する。何も変更しない | |
| タスクが存在しない(`NotFound`) | abort を実行する | 非0 で終了する。何も変更しない | |
| アーカイブ済みのタスク | abort を実行する | 非0 で終了する(操作不可)。書き込まない | |
| タスクファイルがパース不能(`Corrupt`) | abort を実行する | 非0 で終了する。破損ファイルへは書き込まない | |
| running のタスク・照合一致後の kill が失敗する(`KillError`) | abort を実行する | 状態を変更せず(stopped を記録せず)非0 で終了し、再実行を案内する(プロセスが生きたまま凍結扱いになることを防ぐ) | |
| running のタスク・`starttime_of` が `Err(Io)`(生存観測の失敗) | abort を実行する | 状態を変更せず非0 で終了し、再実行を案内する | |
| `save` / `save_degraded` が失敗する | abort を実行する | 非0 で終了し、再実行を案内する | |
| running なのに `current_attempt` / `process` が None(手動修復による不変条件の破れ) | abort を実行する | 状態を変更せず非0 で終了し、タスクファイルの修復を案内する(照合できない対象を kill せず、kill 対象が残り得るまま stopped も記録しない) | |
| launching のタスク・runファイルが破損(`RunFileError::Corrupt`) | abort を実行する | 状態を変更せず非0 で終了し、破損した runファイルの削除による復旧を案内する | |
| launching のタスク・pid あり・starttime なし(照合材料が揃わない) | abort を実行する | 状態を変更せず非0 で終了する(照合なしの kill も、マーカーなしの stopped 化も行わない) | |
| launching のタスク・`write_invalidation_marker` が失敗する | abort を実行する | 状態を変更せず非0 で終了し、再実行を案内する | |
| stopped の記録後、notify_cmd の実行が失敗する(非0 / TimedOut / FailedToStart) | abort を実行する | stopped の記録は完了しているため 0 で終了する。notified_at は書かれず、通知失敗と次のtickが再通知する旨の警告(notify_warning)が表示される | |

## 境界値

| 前提条件 | 操作 | 期待結果 | 実装ステータス |
|---|---|---|---|
| stopped・アーカイブ済み以外の全実行状態(pending / launching / running / failed / completed) | それぞれに abort を実行する | いずれも受理され stopped に至る。kill を伴うのは kill 対象(生存プロセス)が同定できた場合のみで、実行状態ごとの例外分岐はない(requirements §6.5) | |
| どの経路の stopped 記録も | abort を実行する | 常に `notified_at: None` で記録される(過去の凍結の通知記録を引き継がず、必ず通知対象になる) | |

## エッジケース

| 前提条件 | 操作 | 期待結果 | 実装ステータス |
|---|---|---|---|
| running のタスク・PID が別プロセスに再利用されている(starttime 照合不一致) | abort を実行する | Dead(死亡)と判定され、無関係なプロセスを kill せずに stopped の記録のみ行う(誤殺しない。requirements §6.2) | |
| launching のタスク・pid ファイルあり(starttime あり)・照合不一致(PID 再利用) | abort を実行する | Dead と判定され、無関係なプロセスを kill せず stopped の記録と通知のみ行う(「照合一致時のみ kill」の規則は runディレクトリ由来の同定情報でも成立する。マーカー書き込み後の再確認で現れた pid の照合不一致も同じ。requirements §6.2) | |
| kill 成功後に `save` が失敗した(running のまま・プロセスは死亡) | abort を再実行する | 生存観測が Dead を返し、kill なしで stopped の記録のみで完了する(初回は非0 で再実行が案内されている) | |
| kill 成功後 `save` 失敗のまま放置した | 次の tick を実行する | 「exitなし・プロセス死亡」として failed → 再起動へ進み得る(このため abort は非0 で再実行を案内する) | |
| runファイル破損で abort が拒否され続けた後、人間が破損 runファイルを削除した | abort を再実行する | 「不在」としてマーカープロトコルに合流し、通常どおり stopped を確定できる | |
| launching・pid なしで abort により stopped 確定後、遅延起動したラッパーが走った | (ラッパーの動作を観測する) | ラッパーは pid 書き込み後のマーカー確認で終了し、エージェントは起動されない(凍結後に実行が走り出さない) | |
| 未通知(notified_at なし)のまま abort が 0 終了した(通知失敗) | 次の tick を実行する | tick が「notified_at のない stopped」を検出して再通知する(at-least-once) | |
| `state/` 配下ディレクトリ(tasks / archive)が存在しない | abort を実行する | 「タスク不在」として非0 で終了し、何も変更しない(pages 縮退表「state/ 配下ディレクトリ不在」) | |
