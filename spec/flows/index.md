# 動的フロー台帳

複数のユースケース・tickをまたいで生きるライフサイクルの一覧。クロスフェーズ検証(動的フロー検証)の入力となる。実行主体の凡例: **tick** = Tick ユースケース、**CLI** = 各CLIユースケース、**wrapper** = RunWrapper、**人間** = ツール外の操作。

## F1: タスクの実行状態ライフサイクル

- 対象: `Task.execution`(ExecutionState)
- トリガー: RegisterTask(add)による pending での生成
- 経路:

| 遷移 | 主体 | 契機 |
|---|---|---|
| (生成) → Pending | CLI(RegisterTask) | `Task::register` |
| Pending / Failed → Launching | tick(手続きA) | AgentRun ステータスの起動。`record_launching` |
| Pending / Failed → Failed | tick(手続きA/B) | worktree作成・削除、アーカイブ移動の失敗。`record_tool_failure`(ADR-012) |
| Pending / Failed → 状態不変(カウンタのみ) | tick(手続きA) | テンプレート展開失敗。`record_spawn_failure_in_place`(ADR-016) |
| Launching → Running | tick(手続きC) | pid取込。`confirm_running`(spawn_fail_count リセット) |
| Launching → Pending | tick(手続きC) | 猶予超過・マーカー・再確認。`record_spawn_failure` |
| Running → Completed | tick(手続きD) | 判定 completed。`complete_run`(attempt / judge リセット) |
| Running → Pending | tick(手続きD) | 判定 skipped。`skip_run`(タスクステータス不変。ADR-008) |
| Running → Failed | tick(手続きD) | 判定 failed / timeout kill / プロセス死亡。`fail_run` |
| Running → Running(カウンタのみ) | tick(手続きD) | 判定失敗。`record_judge_failure` |
| Completed → Pending(タスクステータスが next へ) | tick(手順6) | `advance`。1タスク1tick1ステップ |
| 各状態 → Stopped | tick / CLI(AbortTask) | 上限超過3経路(リトライ / 判定不能 / 連続spawn失敗)+ abort(計4経路) |
| Stopped → Pending | CLI(RetryTask / SetTaskStatus) | カウンタ全リセット |
| Pending / Failed / Completed / Stopped → Pending(任意ステータスへ) | CLI(SetTaskStatus) | 手動遷移 |

- 終端: Cleanup ステータスでのアーカイブ完了(F7)。Stopped は人間の介入(retry / set-status)まで滞留する準終端(通知で可視化)。Wait ステータスの Pending は set-status までの滞留(通知されない設計。intervention.md)
- 失敗時の扱い: すべての失敗は連続カウンタ(ADR-009)と stopped への収束 + 通知(F3)で人間へエスカレーションする。到達不能・行き止まり状態はない(全状態に脱出遷移または明示された滞留の意味がある)。例外的な滞留は2つ — `Corrupt` / `SnapshotUnreadable` の修復待ち(F7)と、Launching × runファイル破損(tick スキップ・abort 拒否が続く。復旧は破損 runファイルの手動削除 → マーカープロトコル合流。usecases/execution.md 手続きC) — で、いずれも人間の直接修復が文書化された導線を持つ

## F2: attempt(2フェーズ起動 → 観測 → 判定)

- 対象: 1回のエージェント実行(attempt番号・runディレクトリ・プロセス)
- トリガー: tick 手続きA の `record_launching`
- 経路:

| 遷移 | 主体 |
|---|---|
| launching記録(タスクファイルに attempt 採番) | tick |
| runディレクトリ作成 → デタッチspawn | tick |
| starttime → pid の順で書き込み | wrapper |
| マーカー確認(あれば未起動終了) | wrapper |
| エージェント実行(cwd = worktree、ログリダイレクト) | wrapper |
| exit 書き込み | wrapper |
| pid観測 → running 取込 | tick(次tick以降) |
| exit観測 → 判定(デフォルト / 判定コマンド) | tick |

- 終端: completed / skipped / failed の確定(F1 へ)。または spawn失敗(pid が現れず pending へ)・判定不能の上限超過(stopped)・abort による中断(Running は照合付き kill、Launching はマーカー無効化 → stopped。AbortTask)
- 失敗時の扱い: どの時点のクラッシュも次のtickが永続化された事実(タスクファイル・runディレクトリ・プロセス生存)から再分類する。runファイル自体の破損(`Corrupt`・書き込み順序の破れ)はスキップ・報告が続く滞留となり、人間による破損ファイルの削除で猶予経路・マーカープロトコルに合流させる(F1 の例外的滞留)。二重起動は attempt別ディレクトリ + マーカーの順序プロトコル(wrapper「pid後にマーカー確認」× tick/abort「マーカー後にpid再確認」)で排除。PID再利用は starttime 照合(§6.2)で誤kill・誤生存判定を防ぐ。ラッパーのみ死亡した孤児は `try_kill_remnants`(同定できる場合のみ)、残存は許容し人間がOSツールで後始末(monitoring.md)

## F3: stopped 通知(at-least-once)

- 対象: `Stopped.notified_at`
- トリガー: stopped の確定(上限超過3経路 + abort の計4経路)
- 経路: stopped 記録(`notified_at: None`)→ notify_cmd 実行 → 成功時のみ `mark_notified`(主体: 確定させた tick / AbortTask)。`notified_at` のない stopped は以降の全tickが検出して再通知(スナップショット破損タスクも対象 — 通知は定義非依存)
- 終端: `notified_at` の記録。または retry / set-status による stopped 離脱(未通知のまま離脱してよい — 人間が操作した = 気づいている)。notify_cmd 未定義なら未通知のまま保持(catch-up 通知の意図。execution ドメイン)
- 失敗時の扱い: 実行失敗・timeout(NOTIFY_TIMEOUT。ADR-018)・クラッシュのいずれも「`notified_at` なし」が残り再通知される。二重通知は許容(欠落より重複)

## F4: worktree の生涯

- 対象: `Task.workspace` が指す worktree ディレクトリ
- トリガー: 最初の AgentRun 起動(tick 手続きA)での `WorkspacePlanner::derive` + `WorktreeManager::create`
- 経路: 作成(tick)→ 全ステータスで同一 worktree を実行対象に使用(wrapper が cwd に。リトライ間で内容は引き継がれ、ツールはリセットしない)→ 削除(tick 手続きB。Cleanup ステータスのみ)
- 終端: `WorktreeManager::remove`(`AlreadyAbsent` = 達成済み)。stopped・進行中のタスクの worktree は削除されない(調査材料の保護)
- 失敗時の扱い: 作成失敗 = failed 経路(リトライ → stopped)。作成成功直後のクラッシュは `create` の冪等契約で次tickが回復。削除失敗 = failed 経路(人間が原因解消 → retry で再試行)。人間による手動削除は「達成済み」(削除時)または実行失敗の表面化(進行中。task-execution.md)。ツールが再作成することはない。孤児 worktree は生じない: アーカイブは worktree 削除の成功(または既不在)の後にのみ行われ、削除対象のパスは workspace 未記録(作成成功 → 保存前クラッシュ)でも決定的導出で再特定される(usecases/execution.md 手続きB)ため、worktree を残したままタスクだけがアーカイブされることはない。逆(worktree 削除済み・アーカイブ失敗で stopped 滞留)は正規の残存状態であり、F7・cleanup.md のとおり retry で決着させる

## F5: runディレクトリの生涯(attempt 蓄積と gc)

- 対象: `state/runs/<task-id>/attempt-<n>/`
- トリガー: tick 手続きA の `prepare_attempt`(および tick(手続きC)/ abort の `write_invalidation_marker` による不在時作成)
- 経路: 作成(tick)→ pid / starttime / exit / ログの書き込み(wrapper)→ 参照(tick の観測・判定、show、判定コマンドの `RUN_DIR`)→ gc 削除(tick 手続きE。`run_retention` 設定時のみ)
- 終端: gc による削除(attempt 全削除後に親ディレクトリも削除)。`run_retention` 未設定なら永続(クリーンアップでも削除しない。requirements §9.2)
- 失敗時の扱い: 削除失敗はスキップ・報告・次tick再試行(カウンタ消費なし)。保護規則 — 現役タスクの現在attempt(参照は launching記録以外で不変)・stopped の全attempt・`Corrupt` タスクの全attempt — は期間によらず削除しない。アーカイブ済み・孤児は保護しない。`attempt-<n>` 形式外のエントリには触れない

## F6: ブランチの生涯

- 対象: `pulsen/<task-id>` ブランチ
- トリガー: `WorktreeManager::create`(base から分岐)
- 経路: 作成(tick)→ エージェントのコミット蓄積(ツール関知外)→ worktree 削除後も残存(成果物。requirements §9.1)→ 回収(マージ・PR)・削除(人間)
- 終端: 人間による削除(ツールはブランチのライフサイクルに関与しない)
- 失敗時の扱い: コミットのないブランチも異常ではない(cleanup.md)。回収前のユーザーによる削除は成果の喪失(ツールに復元手段なし — ログのみ残る)

## F7: タスクファイルの生涯

- 対象: `state/tasks/<task-id>.json` → `state/archive/<task-id>.json`
- トリガー: RegisterTask の `create`
- 経路: 作成(CLI)→ アトミック置換による更新(tick / CLI。全遷移)→ アーカイブ移動(tick 手続きB)→ 参照のみ(`ls --all` / show)
- 終端: アーカイブ(以降 tick の走査対象外。abort / retry / set-status は拒否)。アーカイブ済みファイルの整理は将来機能(スコープ外)
- 失敗時の扱い: 移動失敗は failed 経路(再試行はアーカイブ移動から実質再開)。破損(`Corrupt`)は全コマンドが書き込みを拒否し、修復は人間の直接編集(ロック外のため tick 停止中に行う。monitoring.md)。スナップショットのみ破損(`SnapshotUnreadable`)は abort / retry / show / ls と再通知のみ可能で、tick の進行からは除外(修復までの滞留)

## F8: ポーリング循環(skipped ループ)

- 対象: 循環ワークフロー(ADR-010)上のタスク
- トリガー: 循環 `next` を持つワークフローでの登録(例: watch → fix → watch)
- 経路: watch 実行 → 判定コマンドが skipped → pending 復帰(同一ステータス・新attempt。周期は tick 間隔に律速)→ … → completed → fix へ遷移 → completed → watch へ循環(すべて tick)
- 終端: ワークフロー上の終端には到達しない。出口は人間の abort(→ stopped → set-status で Cleanup へ)または失敗経路の stopped のみ
- 失敗時の扱い: 一過性失敗は failed → リトライ、completed / skipped 確定で連続カウンタがリセットされ蓄積しない(ADR-009)。周回ごとの attempt 蓄積は F5 の gc で対処(運用上 `run_retention` 設定を推奨)。止め忘れはチェック実行と蓄積が続く(ツールは自動停止しない)
