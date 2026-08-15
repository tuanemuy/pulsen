# テストケース: Tick

参照元: [../../usecases/execution.md](../../usecases/execution.md)(Tick)

1回のtickパス。手続きごとにセクションを分け、区分(正常系 / 異常系 / 境界値 / エッジケース)ごとにケースを列挙する。

## 走査と分岐(処理フロー 1〜9)

### 正常系

| 前提条件 | 操作 | 期待結果 | 実装ステータス |
|---|---|---|---|
| AgentRun ステータスの pending タスクが1件ある | tick を実行する | 手続きA(起動)が実行され、サマリーの `launched` に記録される | |
| Wait ステータスの pending タスクがある | tick を実行する | 何もしない(タスクファイル・カウンタ・runディレクトリのいずれも変化しない) | |
| Cleanup ステータスの pending タスクがある | tick を実行する | 手続きB(終端処理)が実行される | |
| AgentRun ステータスの failed タスクがある | tick を実行する | pending と同じく手続きAで再起動される(ADR-012) | |
| Cleanup ステータスの failed タスクがある | tick を実行する | 手続きBが再試行される | |
| Wait ステータスの failed タスクがある | tick を実行する | 何もしない | |
| launching のタスクがある | tick を実行する | 手続きC(spawn確認)が実行される | |
| running のタスクがある | tick を実行する | 手続きD(観測・判定)が実行される | |
| completed のタスク(現ステータスは AgentRun・next 定義あり)がある | tick を実行する | `advance` によりタスクステータスが `next` へ遷移し、実行状態は pending。`transitioned` に記録される | |
| stopped(notified_at あり)のタスクがある | tick を実行する | 何もしない(起動・遷移・通知のいずれも行わない) | |
| stopped(notified_at なし)のタスクがある | tick を実行する | 共通手続き notify が実行される | |
| タスクが1件もない | tick を実行する | 処理対象がない旨を表示して 0 で終了する | |
| 実行状態の異なる複数のタスクがある | tick を実行する | 各タスクがそれぞれの分岐で1ステップずつ処理され、サマリーに集約される。exit は 0 | |
| 実行可能な pending タスクが複数ある | tick を実行する | すべて起動される(並列度制御は行わない) | |

### 異常系

| 前提条件 | 操作 | 期待結果 | 実装ステータス |
|---|---|---|---|
| 別の操作が排他ロックを保持している | tick を実行する | スキップした旨を表示して 0 で終了する。状態は変更しない | |
| ロック機構自体が異常(`LockError::Failed`) | tick を実行する | 非0 で終了する(競合の 0 スキップ例外は適用しない) | |
| config.yaml が存在しない・パース不能・読めない(Io)のいずれか | tick を実行する | 非0 で終了する。状態は変更しない | |
| `list_active` が Io エラー(走査自体の失敗) | tick を実行する | 非0 で終了する。状態は変更しない | |
| パース不能なタスクファイル(`Corrupt`)が混在する | tick を実行する | 当該タスクは報告のみで書き込まない(stopped化もしない)。残りのタスクは処理を続行し、tick は 0 | |
| スナップショットのみ破損(`SnapshotUnreadable`)・stopped 以外のタスクがある | tick を実行する | 定義依存の判断(起動・遷移・終端処理)をすべてスキップして報告する。書き込まない。tick は 0 | |
| completed だが手動修復により遷移の前提が破れている(`TransitionError`) | tick を実行する | 報告してそのタスクをスキップする。tick は 0 | |
| 手動修復で不変条件が破れている(Running なのに `current_attempt` / `process` が None 等) | tick を実行する | 不変条件の破れとして報告してスキップする(検出は手続きC / D 冒頭のユースケース検査、または遷移関数の `TransitionError`(`MissingCurrentAttempt` 等))。修復は人間に委ねる | |
| 1タスクの処理が失敗する(観測の Io 失敗等) | tick を実行する | `errors` に記録して残りのタスクを続行する。tick 全体は 0 | |

### エッジケース

| 前提条件 | 操作 | 期待結果 | 実装ステータス |
|---|---|---|---|
| 状態が変化しないタスク群(Wait 滞留・猶予内待機・実行継続中) | tick を連続して複数回実行する | 毎回同じ判断が再導出され、書き込みは発生しない(tick の冪等性) | |
| running のタスクの exit 0 を観測した | tick を実行する | この tick では completed の記録まで(`complete_run`)。next への遷移は行わず、次の tick の `advance` に委ねる(1タスク1tick1ステップ) | |
| `SnapshotUnreadable` かつ stopped(notified_at なし)のタスクがある | tick を実行する | スナップショット破損を理由にスキップせず、共通手続き notify を実行する(通知は定義非依存) | |
| config.yaml は存在するが `state/tasks/` ディレクトリが未作成(初回運用) | tick を実行する | 走査は空結果として扱われ、処理対象がない旨を表示して 0 で終了する(`TaskRepository` 契約: 走査対象ディレクトリの不在は空結果。pages ※3。config.yaml まで無い未初期化ホームは ※1 の非0) | |

## 手続きA: 起動(Pending / Failed × AgentRun)

### 正常系

| 前提条件 | 操作 | 期待結果 | 実装ステータス |
|---|---|---|---|
| workspace 未確定の pending タスク | tick を実行する | タスクIDから決定的に導出されたパス(`worktrees/<task-id>`)・ブランチ(`pulsen/<task-id>`)で worktree が作成され、`confirm_workspace` が保存される | |
| worktree 作成に成功した(同一 tick 内) | (続けて同一 tick の処理) | テンプレート展開 → launching記録 → `prepare_attempt` → spawn の順で進む(ADR-016 の順序)。実行状態は launching になり `recorded_at` が記録される | |
| workspace 確定済みの pending タスク | tick を実行する | worktree 作成を行わず、展開から起動処理が始まる | |
| ステータスに agent / model の上書きがあり、`cmd` が `{input}` `{model}` `{workspace}` を参照するエージェント定義 | tick を実行する | spawn に渡るコマンドラインに、ステータス上書きのエージェント定義・上書きモデル名・当該タスクの worktree パスが展開されている(実効値の解決: ステータス > ワークフローデフォルト。requirements §3.1・§7) | |
| `skill` 指定のステータスで、エージェント定義に `skill_input` がある | tick を実行する | `{input}` には `skill_input` で変換されたスキル入力(例: `/skill <名前>`)が展開されてコマンドラインに渡る | |
| failed のタスク(workspace 確定済み) | tick を実行する | 新しい attempt 番号(現番号+1)で launching が記録される。過去の attempt 番号は再利用されない | |
| launching 記録が成功した | (続けて同一 tick の処理) | run_dir(`state/runs/<task-id>/attempt-<n>`)がタスクファイルに記録され、`prepare_attempt` でディレクトリが作成され、`spawn_wrapper` が run_dir・agent_cmd・workspace を渡してデタッチ起動される | |
| attempt_count > 0 の failed タスクで worktree 作成(再試行)が成功した | tick を実行する | attempt_count・judge_attempt_count はリセット**されない**(途中のツール操作の成功ではリセットしない。requirements §6.4・ADR-009)。起動処理はそのまま進む | |
| attempt_count > 0 のタスクが起動され running へ取り込まれた | 次の tick を実行する | `confirm_running` でリセットされるのは spawn_fail_count のみで、attempt_count・judge_attempt_count は保持される(リセットは completed / skipped の確定と人間の操作のみ) | |

### 異常系

| 前提条件 | 操作 | 期待結果 | 実装ステータス |
|---|---|---|---|
| worktree 作成が失敗する | tick を実行する | `record_tool_failure(WorktreeCreate)`: attempt_count が加算され、実行状態は failed、last_failure に失敗要因が記録される。次tickで再試行される | |
| worktree 作成の失敗で attempt_count が上限(`effective_retry_limit`)を超過する | tick を実行する | `Stopped { RetryLimitExceeded, notified_at: None }` を保存し、直後に共通手続き notify を実行する。`frozen` に記録される | |
| 実効エージェント名が解決できない(手動修復でスナップショットが破れた) | tick を実行する | 展開失敗として `record_spawn_failure_in_place` で処理される(ADR-016 の分類と同じ) | |
| 実効エージェント名が config.agents に存在しない | tick を実行する | 展開失敗(同期spawn失敗): spawn_fail_count 加算・実行状態不変・attempt 採番なし・runディレクトリ作成なし・無効化マーカーなし・last_failure に展開エラーを記録 | |
| エージェント定義のテンプレートが不正(`RawAgentDefinition::parse` 失敗) | tick を実行する | 展開失敗として同期spawn失敗経路で処理される | |
| skill 指定のステータスでエージェント定義に skill_input がない(`MissingSkillInput`) | tick を実行する | 展開失敗として同期spawn失敗経路で処理される | |
| テンプレート展開で値が解決できない(`ExpansionError`。`{model}` への値なし等) | tick を実行する | 展開失敗として同期spawn失敗経路で処理される | |
| failed のタスクでテンプレート展開が失敗する | tick を実行する | spawn_fail_count のみ加算され、実行状態は failed のまま変わらない | |
| 展開失敗の加算で spawn_fail_count が上限(`spawn_fail_limit`)を超過する | tick を実行する | `Stopped { SpawnFailLimitExceeded }` を保存し notify を実行する | |
| `prepare_attempt` が失敗する | tick を実行する | 報告のみ行い、launching のまま(猶予時間経路が次tick以降に分類する) | |
| `spawn_wrapper` が同期エラー(`SpawnError`)を返す | tick を実行する | 状態を変更しない。launching のまま猶予時間経路が分類する | |

### 境界値

| 前提条件 | 操作 | 期待結果 | 実装ステータス |
|---|---|---|---|
| `retries: 0` のステータスで worktree 作成が失敗する | tick を実行する | 加算後 attempt_count = 1 > 0 のため、failed を経由せず即 stopped になる | |
| 展開失敗の加算後 spawn_fail_count = 上限(等号) | tick を実行する | 凍結しない(`count > limit` のみ超過)。実行状態は不変のまま | |
| 展開失敗の加算後 spawn_fail_count = 上限+1 | tick を実行する | stopped になる(デフォルト上限3なら4回目の連続失敗で凍結) | |

### エッジケース

| 前提条件 | 操作 | 期待結果 | 実装ステータス |
|---|---|---|---|
| worktree 作成成功と `confirm_workspace` 保存の間で前回の tick がクラッシュした | 次の tick を実行する | 同じ Workspace が再導出され、`create` が既存の worktree+ブランチを達成済みとして成功させる。同じ判断が再導出され、利用者の修復操作なしで進行が再開する | |
| ブランチ `pulsen/<task-id>` のみ残存し worktree がない(残骸) | tick を実行する | `create` が既存ブランチに worktree を張り直して成功する | |
| launching 記録の保存と `prepare_attempt` の間で前回の tick がクラッシュした(run_dir 不在) | 次の tick を実行する | 手続きC の read 系が `Ok(None)` を返し、猶予時間経路に合流する(猶予超過後 spawn失敗として pending 復帰) | |
| launching 記録・`prepare_attempt` 成功と spawn の間で前回の tick がクラッシュした | 次の tick を実行する | pid が現れないため猶予時間経路が spawn失敗として分類する(同じ判断の再導出) | |
| 展開失敗の原因を config.yaml の編集で修正した | 次の tick を実行する | 展開は起動のたびに行われるため、修正が即座に反映されて起動に成功する(グローバル設定はスナップショットされない) | |

## 手続きB: 終端処理(Pending / Failed × Cleanup)

### 正常系

| 前提条件 | 操作 | 期待結果 | 実装ステータス |
|---|---|---|---|
| Cleanup ステータスの pending タスク(workspace 確定済み・worktree あり) | tick を実行する | worktree が削除され(ブランチは削除しない)、タスクファイルが archive へ移動し、`archived` に記録される。以降走査対象から外れる。runディレクトリは残る | |
| workspace 未確定のままクリーンアップに到達したタスク(worktree 未作成) | tick を実行する | `WorkspacePlanner::derive` の決定的導出パスへの `remove` が `AlreadyAbsent` を返し(達成済み)、アーカイブ処理へ進む | |
| workspace 未確定だが、決定的導出パスに worktree が残っている(作成成功 → 保存前クラッシュの残骸) | tick を実行する | 決定的導出パスへの `remove` が worktree を削除してからアーカイブする(孤児 worktree を残さない。手続きB) | |
| worktree が既に存在しない(手動削除済み。`AlreadyAbsent`) | tick を実行する | 削除は達成済みとして成功扱いになり、アーカイブへ進む | |
| `state/archive/` ディレクトリが存在しない状態での初回の終端処理 | tick を実行する | `archive` が必要なディレクトリを自動作成してアーカイブ移動が成功する(`state/` 配下はツールが管理する領域。pages ※3) | |

### 異常系

| 前提条件 | 操作 | 期待結果 | 実装ステータス |
|---|---|---|---|
| worktree 削除が失敗する(ファイルを掴むプロセスがいる等) | tick を実行する | `record_tool_failure(WorktreeRemove)`: attempt_count 加算・failed・last_failure 記録。タスクは tasks に残り、次tickで再試行される | |
| worktree 削除の失敗で上限を超過する | tick を実行する | `Stopped { RetryLimitExceeded }` を保存し notify を実行する | |
| アーカイブ移動が失敗する(権限不足等) | tick を実行する | `record_tool_failure(ArchiveMove)`: attempt_count 加算・failed・last_failure 記録 | |
| アーカイブ移動の失敗で上限を超過する | tick を実行する | stopped になり notify が実行される | |

### 境界値

| 前提条件 | 操作 | 期待結果 | 実装ステータス |
|---|---|---|---|
| Cleanup ステータスでの失敗の繰り返し | tick を繰り返し実行する | 適用される上限は常に組み込みデフォルト 2(ADR-014。上書き不可): 加算後 attempt_count = 2 では failed のまま、= 3 で stopped になる | |

### エッジケース

| 前提条件 | 操作 | 期待結果 | 実装ステータス |
|---|---|---|---|
| worktree 削除成功後にアーカイブ移動が失敗した | 次の tick を実行する | `remove` が `AlreadyAbsent` を返すため、実質アーカイブ移動から再開する(冪等) | |
| worktree 削除とアーカイブの間で前回の tick がクラッシュした | 次の tick を実行する | 「Cleanup ステータスのタスクがまだ tasks にある」ことから同じ処理が再導出される。worktree なしの削除は成功扱いで、二重処理は無害 | |

## 手続きC: spawn確認(Launching)

### 正常系

| 前提条件 | 操作 | 期待結果 | 実装ステータス |
|---|---|---|---|
| runディレクトリに pid・starttime の両方がある | tick を実行する | `ConfirmRunning`: 実行状態が running になり、`current_attempt.process` に同定情報一式(pid・kill同定子・starttime)が取り込まれ、spawn_fail_count が 0 にリセットされる | |
| pid がなく、launching 記録からの経過が猶予時間内 | tick を実行する | `KeepWaiting`: 何もしない(ラッパーの書き込み待ち) | |
| pid がなく、猶予時間を超過している | tick を実行する | 無効化マーカーを書き、pid を再読する。なお pid がなければ `record_spawn_failure`: pending 復帰・spawn_fail_count 加算・last_failure = SpawnFail | |
| マーカー書き込み後の再読で pid・starttime が現れていた | tick を実行する | pending に戻さず `confirm_running` で running へ取り込む | |

### 異常系

| 前提条件 | 操作 | 期待結果 | 実装ステータス |
|---|---|---|---|
| `read_pid_file` / `read_starttime` が `RunFileError`(破損・Io)を返す | tick を実行する | 報告してスキップする。launching のまま(書き込まない)。次tickで再観測 | |
| pid はあるが starttime がない(`InconsistentRunFiles`) | tick を実行する | 報告してスキップする。次tickで再観測 | |
| `write_invalidation_marker` が失敗する(`Err(Io)`) | tick を実行する | 状態を変更せず報告してスキップする(マーカーなしで pending に戻すと遅延起動ラッパーが新attemptと並走し得るため)。次tickで再試行 | |
| マーカー書き込み後の再読で pid あり・starttime なし | tick を実行する | `InconsistentRunFiles` として報告してスキップする(本体 classify と同じ場合分け) | |
| `record_spawn_failure` の加算で spawn_fail_count が上限を超過する | tick を実行する | pending へ戻さず `Stopped { SpawnFailLimitExceeded }` を保存し notify を実行する | |

### 境界値

| 前提条件 | 操作 | 期待結果 | 実装ステータス |
|---|---|---|---|
| pid なし・経過がちょうど猶予時間(30秒)に等しい | tick を実行する | 超過していないため `KeepWaiting`(超過は経過 > 30秒のみ) | |
| pid なし・経過が猶予時間を超えている(30秒+1秒) | tick を実行する | `SuspectSpawnFailure` としてマーカー書き込みと再確認に進む | |
| 時計の巻き戻りで now が recorded_at より前(経過が負) | tick を実行する | 経過を 0 として扱い `KeepWaiting`(巻き戻りで過大評価しない) | |

### エッジケース

| 前提条件 | 操作 | 期待結果 | 実装ステータス |
|---|---|---|---|
| runディレクトリ自体が存在しない(launching 記録と `prepare_attempt` の間のクラッシュ後) | tick を実行する | read 系は不在を `Ok(None)` として返し、猶予時間経路に合流する(正常な復旧経路) | |
| starttime のみあり・pid なし | tick を実行する | 書き込み順序(starttime → pid)の正常な中間状態として pid なしの猶予判定に従う | |
| tick がマーカーを書いた後に遅延起動したラッパーが pid を書いていた(再読で検出) | tick を実行する | running へ取り込む(新attemptとの並走は起きない)。ラッパーは pid 書き込み後のマーカー確認で未起動終了するため、次tickが「exitなし・プロセス死亡」として failed に分類する | |
| ラッパーが pid を書いた直後に tick がマーカーを書いた(ラッパー先行) | tick を実行する | 再読が pid を検出して running へ取り込む。pending 復帰しないため二重起動は起きない(マーカープロトコルの両順序で並走排除) | |
| runファイルの破損(`Corrupt` / `InconsistentRunFiles`)が続いている | tick を繰り返し実行する | スキップと報告が続き、タスクは launching のまま滞留する(stopped に至らないため通知されない) | |
| 人間が破損した runファイルを削除した | 次の tick を実行する | 「不在」として無効化マーカープロトコルに合流し、通常の spawn失敗分類で決着する | |
| spawn失敗で pending 復帰したタスクが再起動される | 次の tick を実行する | 新しい attempt 番号が採番され、runディレクトリも新しいパスになる(過去の試行の残骸と混同しない) | |

## 手続きD: 観測・判定(Running)

### 正常系

| 前提条件 | 操作 | 期待結果 | 実装ステータス |
|---|---|---|---|
| exit ファイルに 0・judge 未定義 | tick を実行する | デフォルト判定で Completed → `complete_run`: completed になり attempt_count・judge_attempt_count が 0 にリセットされる(next への遷移は次tick) | |
| exit ファイルに非0・judge 未定義 | tick を実行する | デフォルト判定で Failed → `fail_run`: failed になり attempt_count 加算・judge_attempt_count リセット | |
| exit ファイルあり・judge 定義あり | tick を実行する | 判定コマンドが `TASK_ID` / `WORKSPACE` / `EXIT_CODE`(10進文字列)/ `RUN_DIR` の環境変数と `config.judge_timeout` で、シェルを介さず直接起動される(引数なし・プレースホルダ展開なし) | |
| 判定コマンドが exit 0 で終了する | tick を実行する | Completed → `complete_run` | |
| 判定コマンドが exit 10 で終了する | tick を実行する | Failed → `fail_run`(通常の失敗として自動リトライ対象) | |
| 判定コマンドが exit 20 で終了する | tick を実行する | Skipped → `skip_run`: タスクステータス不変のまま pending に復帰し、attempt_count・judge_attempt_count が 0 にリセットされる。通知は行わず、サマリーの `skipped_back` に記録される(ADR-008) | |
| exit なし・プロセス生存(照合一致)・timeout 未超過 | tick を実行する | `KeepRunning`: 何もしない(exit があれば生存観測は行わない、の対偶として生存観測を経由する) | |
| exit なし・プロセス生存・timeout 超過 | tick を実行する | kill(照合一致時のみ)が成功したら `fail_run` で failed になる | |
| exit なし・プロセス死亡(starttime 取得不能) | tick を実行する | `try_kill_remnants` で残存終了をベストエフォートで試みたうえで `fail_run` → failed(exit code 不明の一過性死は自動リトライで回復させる) | |

### 異常系

| 前提条件 | 操作 | 期待結果 | 実装ステータス |
|---|---|---|---|
| 判定コマンドがプロトコル外の exit code(例: 1)で終了する | tick を実行する | `JudgeFailure` → `record_judge_failure`: judge_attempt_count 加算・running のまま・last_failure = JudgeFail(detail に原因)。次tickで再判定される | |
| 判定コマンドが judge_timeout を超過する(`TimedOut`) | tick を実行する | 判定失敗として `record_judge_failure` で処理される | |
| 判定コマンドが起動できない(`FailedToStart`) | tick を実行する | 判定失敗として `record_judge_failure` で処理される | |
| 判定失敗の加算で judge_attempt_count が上限(`judge_attempt_limit`)を超過する | tick を実行する | `Stopped { JudgeLimitExceeded }` を保存し notify を実行する(エージェントの再実行では解決しないためリトライせず凍結) | |
| `fail_run` の加算で attempt_count が上限を超過する | tick を実行する | `Stopped { RetryLimitExceeded }` を保存し notify を実行する | |
| timeout kill が失敗する(`KillError`) | tick を実行する | `fail_run` を呼ばず状態を変更せず報告のみ行う。次tickが同じ決定を再導出して再試行する(プロセス生存のまま failed → 再起動 → 同一worktree並走を防ぐ) | |
| `starttime_of` が `Err(Io)` を返す(取得機構自体の失敗) | tick を実行する | 状態を変更せず報告してスキップする。次tickで再観測 | |
| exit ファイルあり・`starttime_of` が失敗する環境 | tick を実行する | 生存観測に依存せず判定が遅延なく実行され、分類が確定する(exit が Some なら判定 — 2段規則の1段目はユースケース側にあり、`classify_alive` は生存の分類だけを返す。観測の一過性失敗で判定を遅延させない) | |
| `read_exit` が `RunFileError` を返す | tick を実行する | 当該タスクをスキップして報告する(書き込まない)。tick は 0 | |
| `try_kill_remnants` が `NotIdentifiable` / `Failed` を返す | tick を実行する | 結果は報告のみで、分類(failed)には影響しない(孤児の残存は許容) | |

### 境界値

| 前提条件 | 操作 | 期待結果 | 実装ステータス |
|---|---|---|---|
| launching の猶予に時間を要したタスクが running 中 | tick を実行する | timeout の経過は記録済み starttime の壁時計成分(`starttime.wall`)を起点に測る(launching 記録から pid 出現までの猶予は timeout に含まれない) | |
| starttime.wall からの経過がちょうど timeout に等しい | tick を実行する | 未超過として `KeepRunning` | |
| starttime.wall からの経過が timeout を超えている | tick を実行する | `KillOnTimeout` | |
| 時計の巻き戻りで starttime.wall からの経過が負 | tick を実行する | 経過を 0 として扱い `KeepRunning` | |
| `timeout: none`(Unlimited)のステータス | tick を実行する | どれだけ経過しても `KeepRunning`(kill されない) | |
| timeout 未指定のステータス・starttime.wall からの経過が 1h を超えている | tick を実行する | 組み込みデフォルト 1h の超過として `KillOnTimeout` になる(無指定を Unlimited と扱わない。requirements §7.2) | |
| `fail_run` の加算後 attempt_count = retry_limit(等号) | tick を実行する | failed のまま(凍結しない)。デフォルト2なら初回+2回のリトライまで許容 | |
| `fail_run` の加算後 attempt_count = retry_limit + 1 | tick を実行する | stopped になる(デフォルト2なら3連続失敗で凍結) | |
| `retries: 0` のステータスで実行が失敗する | tick を実行する | 初回の失敗で即 stopped になる | |
| 判定失敗の加算後 judge_attempt_count = judge_attempt_limit(等号) | tick を実行する | running のまま次tickで再判定される(デフォルト3) | |
| 判定失敗の加算後 judge_attempt_count = judge_attempt_limit + 1 | tick を実行する | stopped になる | |

### エッジケース

| 前提条件 | 操作 | 期待結果 | 実装ステータス |
|---|---|---|---|
| PID が別プロセスに再利用されている(starttime は取得できるが記録済み `starttime.ident` と不一致) | tick を実行する | `Dead` と判定され `DiedWithoutExit` → failed。無関係なプロセスを kill しない(kill は照合一致時のみ) | |
| 判定失敗を記録した後の次tick(同じ exit・同じ定義) | tick を実行する | 再判定が同じ結論を導く(判定の冪等性) | |
| 判定確定後の `save` が失敗した(completed が永続化されなかった) | 次の tick を実行する | 「running かつ exit あり」を再検出して再判定し、同じ結論に至る(永続化された事実からの再導出で復旧) | |
| judge 未定義のステータスでエージェントが exit 20 で終了する | tick を実行する | failed に分類される(デフォルト判定は 0 / 非0 の2値。skipped は判定コマンドでのみ生じる。ADR-008) | |
| failed → 再実行 → completed(または skipped)が確定する | tick を順に実行する | attempt_count・judge_attempt_count が 0 にリセットされる(連続失敗のみを数える。散発的な一過性失敗の蓄積で凍結しない。ADR-009) | |
| 判定失敗の後に failed が確定する | tick を実行する | judge_attempt_count はリセットされる(判定自体は成立しているため) | |
| skipped が確定した(pending 復帰済み) | 次の tick を実行する | 同じ exit ファイルが再判定されることはなく、同じタスクステータスの実行が新しい attempt で起動される(周回。通知なし) | |
| 無効化マーカーを見て未起動終了したラッパー(running 取込済み・exit なし・プロセス死亡) | tick を実行する | 「exitなし・プロセス死亡」として failed に分類される | |
| エージェントがシグナル死し exit に 128+n が記録されている | tick を実行する | デフォルト判定で failed(非0)に分類される | |

## 手続きE: runディレクトリの gc(`run_retention` 設定時のみ)

### 正常系

| 前提条件 | 操作 | 期待結果 | 実装ステータス |
|---|---|---|---|
| `run_retention` 未設定・保持期間相当を超えた attempt がある | tick を実行する | gc は行われない(明示オプトイン。ADR-011) | |
| `run_retention` 設定済み・非保護の attempt の last_activity が保持期間を超過 | tick を実行する | 当該 attempt が削除され、`gc_deleted` に記録される | |
| 現役タスク(stopped 以外)が現在参照している attempt が期間超過 | tick を実行する | 削除されない(`ActiveCurrent` 保護) | |
| 現役タスクの過去 attempt(現在参照外)が期間超過 | tick を実行する | 削除される | |
| stopped タスクの attempt(現在参照外を含む)が期間超過 | tick を実行する | 全 attempt が削除されない(`AllProtected`。調査材料の保護) | |
| アーカイブ済みタスクの attempt が期間超過 | tick を実行する | 保護されず削除される | |
| タスクファイルの存在しない孤児の runディレクトリが期間超過 | tick を実行する | `Unprotected` として削除される(TaskId にパースできないディレクトリ名も対象) | |
| あるタスクの attempt がすべて削除された | tick を実行する | 空になった `state/runs/<task-id>/` も削除される | |

### 異常系

| 前提条件 | 操作 | 期待結果 | 実装ステータス |
|---|---|---|---|
| `list_runs` が Io エラーを返す | tick を実行する | gc のみ中止して `gc_errors` に報告する。タスク処理は完了しているため tick は 0 | |
| `delete_attempt` が失敗する(ログを開いているプロセスがいる等) | tick を実行する | 当該 attempt をスキップして `gc_errors` に報告する。どのタスクのカウンタも消費せず stopped も発生しない。次tickが再試行する | |

### 境界値

| 前提条件 | 操作 | 期待結果 | 実装ステータス |
|---|---|---|---|
| now - last_activity がちょうど retention に等しい | tick を実行する | 削除されない(`now - last_activity > retention` のみ削除対象) | |
| now - last_activity が retention をわずかに超える | tick を実行する | 削除される | |
| ファイルが1つもない attempt ディレクトリ | tick を実行する | ディレクトリ自体の最終更新時刻で経過を判定する | |

### エッジケース

| 前提条件 | 操作 | 期待結果 | 実装ステータス |
|---|---|---|---|
| パース不能(`Corrupt`)なタスクファイルが tasks に存在する | tick を実行する | ファイル名主部(`<task-id>.json` の `<task-id>`)をキーに `AllProtected` となり、全 attempt が保護される(読めない帳簿に対して何もしない) | |
| `SnapshotUnreadable`(DegradedTask)・stopped 以外のタスク | tick を実行する | 通常規則(`ActiveCurrent`)が適用される(実行状態と現在attempt参照は読めるため) | |
| `SnapshotUnreadable` かつ stopped のタスク | tick を実行する | `AllProtected` | |
| このtick(同一パス)で stopped に凍結されたタスク | tick を実行する | 保護マップは各タスクの処理後の状態から構築されるため `AllProtected` になる(凍結した tick 自身の gc が調査材料を消さない) | |
| このtickでアーカイブ完了したタスク | tick を実行する | `Unprotected` として扱われる | |
| このtickで `save` に失敗したタスク | tick を実行する | `AllProtected` として扱う(メモリ上と永続上のどちらが真か確定できないため保守側に倒す) | |
| `attempt-<n>` 形式に合致しないエントリ(手動配置ファイル・不正名ディレクトリ)がある | tick を実行する | 列挙対象外で触れない。その残存により親ディレクトリが削除できないことは許容される | |
| skipped・spawn失敗で pending 復帰したタスク | tick を実行する | `current_attempt` の参照は保持されており(launching 記録でのみ置き換わる)、`ActiveCurrent` の保護対象が決定的に定まる | |

## 共通手続き: 凍結の確定と通知(notify)

### 正常系

| 前提条件 | 操作 | 期待結果 | 実装ステータス |
|---|---|---|---|
| stopped(notified_at なし)・notify_cmd 定義あり・コマンドが exit 0 | tick を実行する | `TASK_ID` / `WORKFLOW` / `TASK_STATUS` の環境変数と NOTIFY_TIMEOUT(組み込み60秒)で notify_cmd が直接起動され、成功後に `mark_notified` → `save` で notified_at が記録される。`notified` に記録 | |
| notify_cmd 未定義(None)の stopped(notified_at なし) | tick を実行する | 通知を行わず、notified_at も書かない(「通知した」という虚偽の記録を作らない) | |
| notified_at 記録済みの stopped | tick を実行する | 通知しない(再通知は未通知のもののみ) | |
| このtickの手続き中で上限超過により stopped になった | tick を実行する | `save` 直後に同一 tick 内で notify が実行される(次tickを待たない)。`frozen` / `notified` に記録される | |

### 異常系

| 前提条件 | 操作 | 期待結果 | 実装ステータス |
|---|---|---|---|
| notify_cmd が非0で終了する | tick を実行する | notified_at を書かずに終える。次のtickが再通知する(at-least-once) | |
| notify_cmd が NOTIFY_TIMEOUT(60秒)を超過する | tick を実行する | 通知失敗として notified_at を書かない。次のtickが再通知する(ADR-018) | |
| notify_cmd が起動できない(`FailedToStart`) | tick を実行する | 通知失敗として notified_at を書かない。次のtickが再通知する | |
| `mark_notified` 後の `save` が失敗する | tick を実行する | notified_at が永続化されず、次のtickが再通知する(二重通知は許容) | |

### エッジケース

| 前提条件 | 操作 | 期待結果 | 実装ステータス |
|---|---|---|---|
| stopped の記録と通知の間で前回の処理がクラッシュした | 次の tick を実行する | 「notified_at のない stopped」が検出され、同じ判断が再導出されて通知される | |
| notify_cmd 実行成功と notified_at 追記の間でクラッシュした | 次の tick を実行する | 再通知される(二重通知は許容。欠落を許さない) | |
| notify_cmd 未定義のまま凍結したタスクがあり、後から notify_cmd が定義された | 次の tick を実行する | notified_at のない stopped が検出され、catch-up 通知される | |
| DegradedTask(スナップショット破損)の stopped(notified_at なし) | tick を実行する | 再通知が行われ、成功時は `save_degraded` で notified_at が永続化される(通知は定義非依存。at-least-once を破損時も維持) | |
| 未通知のまま retry / set-status で stopped を離脱したタスク | tick を実行する | 通知されない(stopped でないため対象外。人間が操作した = 気づいている) | |
