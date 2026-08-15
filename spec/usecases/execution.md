# ユースケース: Execution

実行の観測・分類・判定を伴うユースケース。CLI の tick / abort / wrapper(内部)に対応する。判断はすべてドメイン(Task の遷移関数・Execution の分類サービス)が行い、ユースケースは「ポートで観測 → ドメインで判断 → ポートで実行」の配線に徹する。

共通事項([task.md](task.md) と同じ): グローバルホーム解決 → `ConfigStore::load`(失敗は非0)。Tick / AbortTask は `ExclusiveLock` を取得する(RunWrapper は取得しない — 書き先が自attemptのrunディレクトリに閉じ、tickと競合しない)。

## 共通手続き: 凍結の確定と通知(notify)

stopped を書いたすべての経路(Tick 内の各上限超過・AbortTask)と、`notified_at` のない stopped の検出時に使う(requirements §8: 書く → 実行 → 追記)。

1. stopped の記録は遷移関数(`fail_run` / `record_spawn_failure` / `record_spawn_failure_in_place` / `record_judge_failure` / `record_tool_failure` / `abort`)が済ませ、`save` 済みであること(`notified_at: None`)
2. `config.notify_cmd` が None → 何もしない(`notified_at` は書かない。後から定義されると catch-up 通知される。execution ドメインの規定)
3. `NotificationService::notify_env(task_id, workflow_name, task_status)` → `CommandRunner::run(notify_cmd, env, NOTIFY_TIMEOUT)`
4. `NotificationService::interpret_notify_completion(completion)` → `Delivered` なら `mark_notified(now)` → `save`(DegradedTask は `save_degraded`)。`Failed { cause }` なら何もしない(次のtickが再通知する。at-least-once)。**失敗の報告の形は呼び出し側が決める**(この手続きが固定するのは成否の判定経路まで — Tick は `errors` に `NotifyFailed { task_id, cause }` を積み、AbortTask は `notify_warning` に載せる)。いずれの経路でも文言は表示層が `cause` から組み立てる

クラッシュ・通知失敗のどの時点でも、「`notified_at` のない stopped」が残る限り次のtickが再実行する(二重通知は許容。requirements §8)。

## Tick(tick)

### 概要

1回のtickパス。全タスクを走査し、実行状態 × タスクステータスの動作種別ごとに定義された処理を行い、`run_retention` 設定時はrunディレクトリのgcを行う(requirements §4・§9.2)。1タスクにつき1tickで1ステップだけ進める(completed → 遷移は次のtick。task-execution.md「以降のtickが遷移させ」)。tick は永続化された事実から毎回同じ判断を再導出する(冪等)。

### 入力DTO

なし(`pulsen tick`)。

### 出力DTO(サマリー)

| フィールド | 型 |
|---|---|
| `launched` / `confirmed_running` / `judged` / `transitioned` / `skipped_back` / `frozen` / `notified` / `archived` | `Vec<TaskId>`(起動 / 起動確認 / 判定確定 / 遷移 / skippedでpending復帰 / 凍結 / 通知 / 終端処理完了) |
| `errors` | `Vec<TickIssue>`(スキップ・破損・観測失敗・kill失敗等の報告) |
| `gc_deleted` | `Vec<(String, AttemptNumber)>`、`gc_errors` 同形 |

- `confirmed_running` は launching → running の取込(`confirm_running`)の受け皿。`transitioned`(タスクステータスを進めた)にも `skipped_back`(pending へ戻した)にも語義が合わない
- `judged` は `complete_run` による判定確定の受け皿。`advance` の結果である `transitioned` に混ぜると、1タスク1tick1ステップの原則の下で異なる2つのステップが同じフィールドに現れる
- **タスクファイルへの書き込みを行った経路は必ずサマリーのいずれかのフィールドを埋める**(これを欠くと、書き込みのあった tick が毎回「処理対象なし」と表示される)

`errors` の分類は**完成文言を持たない直和型**であり、文言は表示層(pages)が組み立てる。

```
TickIssue =
  CorruptTaskFile      { path, message }        // タスクファイル全体が読めない
| SnapshotUnreadable   { task_id, message }     // スナップショットのみ読めない
| MissingCurrentAttempt{ task_id }              // 不変条件2の破れ
| MissingProcessIdent  { task_id }              // 不変条件3の破れ(pidファイルから復元しうる)
| MissingWorkspace     { task_id }              // 不変条件4の破れ。判定へ渡す WORKSPACE を組めない
| Transition           { task_id, error: TransitionError }
| RunFileUnreadable    { task_id, error: RunFileError }
| InconsistentRunFiles { task_id, kind: InconsistentRunFiles }
| WorktreeCreateFailed { task_id, message }
| CommandExpansionFailed { task_id, message }
| MarkerWriteFailed    { task_id, message }
| PrepareAttemptFailed { task_id, message }
| SpawnFailed          { task_id, message }     // spawn_wrapper の同期エラー
| SpawnNotObserved     { task_id, message }     // 猶予超過で確定した spawn 失敗(カウンタを消費し凍結しうる)
| ObservationFailed    { task_id, message }     // 生存観測の機構自体の失敗。状態を変更しない
| KillFailed           { task_id, message }     // timeout 超過の kill の失敗。状態を変更しない
| RemnantsUnhandled    { task_id, remnants: RemnantsLeft }
| JudgeFailed          { task_id, detail }
| RunFailed            { task_id, cause: RunFailureCause }
| NotifyFailed         { task_id, cause: NotifyFailureCause }
| SaveFailed           { task_id, error: SaveError }

RunFailureCause = DefaultJudgement { exit } | JudgeCommand { exit } | TimedOut { timeout } | DiedWithoutExit
RemnantsLeft    = NotIdentifiable | Failed { message }
```

- 各変種は**必要なフィールドだけを持つ**(`CorruptTaskFile` はタスク ID を持たない — ファイルが読めない以上 ID は確定しない。`MissingCurrentAttempt` はパスを持たない)
- `RunFailureCause` は**判断の主体**を分ける分類(判定コマンドが失敗を返したのか、エージェント自身が非0で終わったのか、timeout kill か、exit を残さず死んだか)
- `RemnantsLeft` は報告を要する2値であり、後始末を残さない `RemnantOutcome::Killed` がこの分類に現れないことを型で表現不能にする
- `MissingProcessIdent` を `MissingCurrentAttempt` と分けるのは修復の手がかりが違うため(前者は pidファイルから復元しうる)
- `SpawnNotObserved`(猶予超過で確定した spawn 失敗)と `SpawnFailed`(`spawn_wrapper` の同期エラー)は**別の変種**にする — 前者はカウンタを消費して凍結しうるが、後者は状態を変更しない

処理対象がなければその旨を表示。tickパスが実行された場合(個別タスクの失敗・gc失敗を含む)の exit は 0。ロック競合も 0 でスキップ(pages)。走査自体ができない場合(下記の `list_active` の Io)のみ非0。

### 処理フロー

1. `ExclusiveLock::try_acquire`。`None` → スキップした旨を表示して 0 で終了
2. `TaskRepository::list_active` → 各エントリを次のとおり分岐処理する。1タスクの処理失敗は `errors` に記録して残りを続行する(requirements §9)
   - `Corrupt` → 報告のみ(書き込まない)
   - `SnapshotUnreadable(degraded)` → 定義依存の判断はすべてスキップして報告。ただし `Stopped { notified_at: None }` なら共通手続き notify を実行する(task ドメインの規定)
   - `Intact(task)` → 実行状態で分岐(3〜7)
3. **Pending / Failed**: `task.current_status_def()` の動作種別で分岐
   - `Wait` → 何もしない
   - `AgentRun` → 手続きA(起動)
   - `Cleanup` → 手続きB(終端処理)
4. **Launching**: 手続きC(spawn確認)
5. **Running**: 手続きD(観測・判定)
6. **Completed**: `task.advance(now)` → `save`(タスクステータスが `next` へ、pending に戻る)。`TransitionError`(手動修復による破れ)は `Transition` として報告してスキップ
7. **Stopped**: `notified_at` が None なら共通手続き notify
8. `config.run_retention` が Some なら手続きE(gc)
9. サマリーを表示して 0 で終了

各手続きで遷移関数が `Stopped` を返した場合は、`save` 直後に共通手続き notify を実行し `frozen` / `notified` に記録する。共通手続き notify が `Failed { cause }` を返した場合は `errors` に `NotifyFailed { task_id, cause }` を積む。遷移関数の `TransitionError`(手動修復による破れを含む)は `errors` に `Transition` として報告してそのタスクをスキップする。

#### 手続きA: 起動(Pending / Failed × AgentRun)

順序は「worktree確保 → テンプレート展開 → launching記録 → spawn」(ADR-016)。

1. worktree確保: `task.workspace` が None なら `WorkspacePlanner::derive(worktree_root, id)` → `WorktreeManager::create(repo, base, ws)`(自タスクの残骸には冪等)→ 成功なら `task.confirm_workspace(ws, now)` → `save`。失敗なら `task.record_tool_failure(WorktreeCreate, message, effective_retry_limit, now)` → `save` → (Stopped なら notify)→ このタスクは終了
2. テンプレート展開(すべて同期。失敗は `task.record_spawn_failure_in_place(message, spawn_fail_limit, now)` → `save` → (Stopped なら notify)→ 終了。ADR-016):
   a. 実効エージェント名 = `snapshot.effective_agent(status)`(登録時検証で解決可能性が保証されるが、手動修復で破られ得る。None なら展開失敗として扱う — ADR-016 の分類と同じ)
   b. `config.agents` から `RawAgentDefinition` を引く(不在 → 展開失敗)
   c. `RawAgentDefinition::parse`(`AgentDefError` → 展開失敗)
   d. `AgentDefinition::render_input(status の AgentInput)`(`MissingSkillInput` → 展開失敗)
   e. `AgentDefinition::build_command_line(input, effective_model, workspace)`(`ExpansionError` → 展開失敗)
3. `task.record_launching(state_root, now)` → `(task', run_dir)` → `save`(launching記録。ここが復旧の起点)
4. `RunStore::prepare_attempt(id, attempt)`(失敗は報告のみ — 猶予経路が分類する)
5. `ProcessController::spawn_wrapper({ run_dir, agent_cmd, workspace })`(同期エラーも状態を変更しない — 猶予経路が分類する。execution ドメインの契約)

#### 手続きB: 終端処理(Pending / Failed × Cleanup)

1. `WorktreeManager::remove(repo, path)` を実行する。パスは `task.workspace` が Some ならその値、**None でも `WorkspacePlanner::derive(worktree_root, id)` の決定的導出パス**を使う(worktree作成成功 → `confirm_workspace` 保存前のクラッシュで workspace 未記録のまま worktree だけが残った場合も、決定的導出により同じパスに到達して削除できる — 孤児 worktree を生まない。未作成なら `AlreadyAbsent`)。`AlreadyAbsent` は成功(cleanup.md)。失敗なら `task.record_tool_failure(WorktreeRemove, message, snapshot.effective_retry_limit(status), now)` → `save` → (Stopped なら notify)→ 終了
2. `TaskRepository::archive(id)`。失敗なら `task.record_tool_failure(ArchiveMove, message, snapshot.effective_retry_limit(status), now)` → `save` → (Stopped なら notify)→ 終了
3. 成功: `archived` に記録。タスクは走査対象から外れる(runディレクトリ・ブランチは残る。requirements §9.1)

リトライ上限の出所は手続きAと同じく `effective_retry_limit`(Cleanup は常に組み込みデフォルト 2 を返す。ADR-014)。worktree削除成功 → archive 失敗 → 次tickは remove が `AlreadyAbsent` を返すため実質 archive から再開する(冪等)。

#### 手続きC: spawn確認(Launching)

0. 冒頭で `current_attempt` が None なら不変条件2の破れとして `MissingCurrentAttempt` を報告しスキップする(遷移関数の `TransitionError::MissingCurrentAttempt` と同じ扱い。AbortTask の観測前検出と対称 — 観測は遷移関数より先に attempt 参照へアクセスするため、ユースケース側でも検査する)
1. `run_dir = task.current_attempt.run_dir` から `RunStore::read_pid_file` / `read_starttime`(ディレクトリ不在は None。`RunFileError` は報告してスキップ)。破損した runファイル(`Corrupt`)・`InconsistentRunFiles` が続く場合、tick はスキップし続け、abort も照合材料が揃わず拒否するため、タスクは launching のまま滞留する(stopped に至らないため通知されない)。復旧は人間による当該 runファイルの削除(タスクファイルの直接修復と同じ位置づけ。monitoring.md) — 削除後は「不在」として無効化マーカープロトコルに合流し、通常の spawn失敗分類で決着する
2. `LaunchingClassifier::classify(recorded_at, now, pid, starttime)`:
   - `ConfirmRunning(ident)` → `task.confirm_running(ident, now)` → `save`
   - `KeepWaiting` → 何もしない
   - `SuspectSpawnFailure` → `RunStore::write_invalidation_marker(run_dir)`(**失敗(`Err(Io)`)なら状態を変更せず報告してスキップ** — マーカーなしで pending に戻すと遅延起動したラッパーが新attemptと並走し得るため、次tickで再試行する)→ 成功なら `read_pid_file` / `read_starttime` を再読 → `classify_recheck`:
     - `ConfirmRunning(ident)` → `confirm_running` → `save`
     - `SpawnFailed` → `task.record_spawn_failure(message, spawn_fail_limit, now)` → `save` → (Stopped なら notify)
   - `Err(InconsistentRunFiles)` → 報告してスキップ(次tickで再観測)

#### 手続きD: 観測・判定(Running)

0. 冒頭で `current_attempt` / `current_attempt.process` が None なら不変条件2〜3の破れとして報告しスキップする(手続きC の冒頭検査と同じ)。報告分類は前者が `MissingCurrentAttempt`、後者が `MissingProcessIdent`(修復の手がかりが違う — 後者は pidファイルから復元しうる)
1. `RunStore::read_exit(run_dir)`
2. exit が Some → 判定(生存観測は行わない。2段規則の1段目はここで `RunningDecision::Judge(exit)` として値にする):
   - `judge` 未定義 → `JudgementService::default_judgement(exit)` → `DefaultJudgement`(Completed / Failed)を `JudgeOutcome` へ埋め込む
   - `judge` 定義あり → `task.workspace` が None なら不変条件4の破れとして `MissingWorkspace` を報告しスキップする(判定コマンドは起動せず、書き込みも行わない)。Some なら `judge_env(id, workspace, exit, run_dir)` → `CommandRunner::run(judge, env, config.judge_timeout)` → `interpret_judge_completion`:
     - `Outcome(o)` → o で分岐、`JudgeFailure { detail }` → `task.record_judge_failure(detail, config.judge_attempt_limit, now)` → `save` → (Stopped なら notify)→ 終了
   - Completed → `task.complete_run(now)` → `save`(遷移は次tickの Completed 処理)
   - Skipped → `task.skip_run(now)` → `save`(タスクステータス不変・pending 復帰。ADR-008)
   - Failed → `task.fail_run(effective_retry_limit, now)` → `save` → (Stopped なら notify)
3. exit が None → 生存観測: `ProcessController::starttime_of(pid)`(`Err(Io)` → 報告してスキップ)→ `IdentityCheck::check(observed, recorded.ident)` → `RunningClassifier::classify_alive(aliveness, recorded.wall, effective_timeout, now)` → 結果は `AliveDecision` であり、`RunningDecision` へ合流させてから網羅 `match` で分岐する:
   - `KeepRunning` → 何もしない
   - `KillOnTimeout` → `ProcessController::kill(kill_ident)`。成功 → `task.fail_run(...)` → `save` → (Stopped なら notify)。失敗(`KillError`)→ 状態を変更せず報告(次tickが再導出。execution ドメインの契約)
   - `DiedWithoutExit` → `ProcessController::try_kill_remnants(kill_ident)`(結果は報告のみ)→ `task.fail_run(...)` → `save` → (Stopped なら notify)

#### 手続きE: runディレクトリのgc(`run_retention` 設定時のみ)

1. `RunStore::list_runs` → 保護マップを**各タスクの処理後の状態**(手順3〜7適用後)から構築する(処理前の読み取りから作ると、このtickで凍結したタスクの調査材料を同じtickのgcが削除し得る — requirements §9.2(b) の破れ): `Corrupt` → path のファイル名主部(`<task-id>.json` の `<task-id>`)をキーに `AllProtected`、stopped(Degraded 含む。このtickで凍結したものを含む)→ `AllProtected`、その他の現役 → `ActiveCurrent(attempt番号)`、このtickでアーカイブ完了したタスク・runsにあるが現役にないもの → `Unprotected`。**このtickで `save` に失敗したタスクは `AllProtected`** とする(メモリ上と永続上のどちらの状態が真かをこのtickでは確定できないため、保守側に倒す — requirements §9.2(a) の参照保護を破らない)
2. `GcPolicy::plan(listing, protection, retention, now)` → 各 `(dir, attempt)` を `RunStore::delete_attempt`(失敗はスキップして `gc_errors` に報告。カウンタ消費・stopped化なし)→ 空になった親を `remove_task_dir_if_empty`
3. 結果をサマリーに含める

### トランザクション境界

- UnitOfWork: 不要。書き込みはすべて単一タスクファイルの `save` / `archive` に閉じる。複数リソースにまたがる列(launching記録 → prepare_attempt → spawn、worktree削除 → archive、stopped記録 → notify → mark_notified)は原子化せず、途中失敗・クラッシュ時に残る状態を各手続きに明記のとおり次のtickの冪等な再導出と at-least-once で回復する(domains/index.md)

### エラーケース

| 条件 | 扱い |
|---|---|
| ロック競合 | 0 でスキップ(cron 運用でアラートにしない) |
| `Err(LockError::Failed)`(ロック機構自体の異常) | 実行環境(非0。競合の 0 スキップ例外は適用しない — 共通事項のとおり) |
| config 読み込み失敗 | 非0(※1) |
| `list_active` の Io(走査自体の失敗) | 実行環境(非0。状態は変更しない。ListTasks と同じ扱い) |
| `list_runs` の Io | gc のみ中止して `gc_errors` に報告(タスク処理は完了しているため tick は 0) |
| タスクファイル破損 / スナップショット破損 / 不変条件の破れ / runファイル破損 / 観測の Io 失敗 | 当該タスクをスキップして報告(書き込まない)。tick 全体は 0 |
| ツール操作・spawn・判定の失敗 | 各手続きの遷移関数で記録(failed / pending維持 / stopped) |
| kill 失敗 | 状態を変更せず報告(次tickが再試行) |
| gc の削除失敗 | スキップして報告(カウンタ消費なし) |

## AbortTask(abort)

### 概要

タスクを停止して stopped を確定する。分岐は実行状態のラベルではなく kill 対象の有無で決まる(requirements §6.5)。CLI 自体が確定させる(次のtickを待たない)。

### 入力DTO

| フィールド | 型 | 必須 | バリデーション |
|---|---|---|---|
| `task_id` | `String` | 必須 | `TaskId::parse` |

### 出力DTO

| フィールド | 型 |
|---|---|
| `task_id` | `TaskId` |
| `killed` | `bool`(kill を実行したか) |
| `already_stopped` | `bool`(冪等成功) |
| `notify_warning` | `Option<String>`(通知失敗時: 次のtickが再通知する旨。文言は表示層が `NotifyFailureCause` から組み立てる) |

### 処理フロー

1. ロック取得 → `TaskRepository::find`。`NotFound` / `Archived` / `Corrupt` はエラー(書き込まない)。`SnapshotUnreadable` は通常どおり進む(abort はスナップショット非依存。pages ※7)
2. すでに `Stopped` → 何も変更せずその旨を表示して 0(冪等成功)
3. kill 対象の同定(生存プロセスが設計上あり得るのは Launching / Running のみ。実行状態から静的に分岐する — pages abort と一致):
   - `Running` → `current_attempt.process`(不変条件3により Some)から `starttime_of(pid)`(`Err(Io)` → 状態を変更せず非0・再実行を案内)→ `IdentityCheck::check`。`Alive` → `kill(kill_ident)`。失敗(`KillError`)→ **状態を変更せず**非0(再実行を案内。stopped は記録しない)。成功 → killed = true。`Dead`(照合不一致・死亡)→ kill せず進む
   - `Launching` → `read_pid_file` / `read_starttime`(runディレクトリ不在なら `write_invalidation_marker` がディレクトリごと作成する。pages ※8b)。pid あり(starttime あり)→ 上記の照合付き kill。pid なし → `write_invalidation_marker` → 再読。なお pid なし → kill せず進む。再読で pid あり → 照合付き kill。**読み取りの `RunFileError`・「pid あり・starttime なし」(照合材料が揃わない)・`write_invalidation_marker` の失敗は、いずれも状態を変更せず非0(再実行を案内)**(照合なしの kill も、マーカーなしの stopped 化も行わない)
   - それ以外(Pending / Failed / Completed)→ プロセス操作なしで進む(`current_attempt.process` が Some でも観測しない — Running を離脱した時点で終了確認・kill・死亡確認のいずれかを経ており、生存プロセスは残っていない。requirements §6.5)
4. `task.abort(now)`(Degraded は `degraded.abort(now)`)→ `save` / `save_degraded`
5. 共通手続き notify を実行(失敗しても stopped の記録は完了しているため 0。`notify_warning` を表示)
6. kill の有無を含む結果を表示して 0

### トランザクション境界

- UnitOfWork: 不要(単一タスクの `save`。kill・通知は原子化せず、kill失敗時は書き込み自体を行わない / 通知失敗は at-least-once で回復)
- kill 成功 → `save` 失敗の残存状態: Running のまま・プロセスは死亡。放置すると次のtickが `DiedWithoutExit` → failed → 再起動へ進み得るため、非0で abort の再実行を案内する(再実行時は観測が `Dead` を返し、記録のみで完了する)

### エラーケース

| 条件 | 種類 |
|---|---|
| ロック競合 / タスク不在 / アーカイブ済み / `Corrupt` | 入力・状態(書き込まない) |
| kill 操作自体の失敗(照合一致後のエラー)/ 生存観測の Io 失敗 | 外部(状態を変更せず非0。再実行を案内) |
| `save` / `save_degraded` の失敗 | 実行環境(非0。再実行を案内。残存状態はトランザクション境界の節を参照) |
| 不変条件の破れ(Running で `current_attempt` / `process` が None 等の手動修復による破れ) | 状態(状態を変更せず非0。タスクファイルの修復を案内 — 照合できない対象を kill せず、kill 対象が残り得るまま stopped も記録しない。Tick が `MissingCurrentAttempt` / `MissingProcessIdent` を報告してスキップするのと同じ原則) |
| Launching の runファイル破損(`RunFileError::Corrupt`・pid あり starttime なし)の継続 | 状態(非0。破損した runファイルの削除による復旧を案内する — 手続きC の復旧経路と同じ。削除後の abort はマーカープロトコルで通常どおり確定できる) |
| すでに stopped | 正常(0。冪等成功) |

## RunWrapper(wrapper 内部コマンド)

### 概要

デタッチ起動されたラッパーモード。自身の同定情報と終了結果をrunディレクトリへ永続化し、エージェントを worktree で実行する(requirements §4.1)。利用者向けではなくヘルプにも出さない。config は読まない(必要な情報はすべて起動引数で受け取る)。

### 入力DTO(起動引数。`WrapperLaunchSpec` の直列化)

| フィールド | 型 | 必須 |
|---|---|---|
| `run_dir` | `RunDirPath` | 必須 |
| `workspace` | `WorktreePath` | 必須 |
| `agent_cmd` | `CommandLine` | 必須 |

### 出力DTO

なし(結果はすべてrunディレクトリのファイルとして現れる。pages)。

### 処理フロー

1. `ProcessController::own_identity()` → `WrapperIdentity { pid, kill_ident, starttime }`
2. `RunStore::write_starttime(run_dir, starttime)` — **必ず pid より先に書く**(tick は pid の存在で同定情報一式が揃ったとみなすため。requirements §4.1)
3. `RunStore::write_pid_file(run_dir, { pid, kill_ident })`
4. `RunStore::marker_exists(run_dir)` → true ならエージェントを起動せず終了(遅延起動の排除。「pid書き込み後にマーカー確認」の順序がツール側の「マーカー書き込み後にpid再確認」と対になる)。**`Err(Io)`(確認自体の失敗)も起動せず終了する**(無効化されていないことを確認できない以上、起動は安全側に倒す。次tickが「exitなし・プロセス死亡」として failed に分類する)
5. `ProcessController::run_agent(agent_cmd, workspace, run_dir.stdout_log(), run_dir.stderr_log())` → `ExitCode`(起動不能は 127 / 126、シグナル死は 128+n、リダイレクト不能は 126 に符号化)
6. `RunStore::write_exit(run_dir, exit)` → 終了

ステップ1〜3・6 の書き込みが失敗した場合は何も書き残さず終了する(観測されない = 猶予経路またはプロセス死亡としてtickが分類する。ラッパーは自前のエラー報告経路を持たない)。

終了コードは**ラッパー自身が責務を果たせたか**を表す(規約は pages `wrapper` の節。エージェントの終了コードは伝播せず、run ディレクトリの `exit` ファイルだけが持つ)。エージェントを実行した場合とマーカーにより起動しなかった場合は 0、同定情報を何も残せずに終えた場合と起動引数が不正な場合は非0。

### トランザクション境界

- UnitOfWork: 不要(書き先は自attemptのrunディレクトリのみ。各ファイルはアトミック置換)

### エラーケース

| 条件 | 扱い |
|---|---|
| 引数の不正(直列化の破れ) | 何も書かず**非0**終了(猶予経路が spawn失敗として分類) |
| starttime / pid の書き込み失敗 | 何も書かず**非0**終了(同上。同定情報を何も残せていない) |
| 無効化マーカーあり | **0** で終了(エージェント未起動。次tickが「プロセス死亡」として failed に分類 — requirements §4.1) |
| エージェント起動不能・シグナル死 | exit に符号化(127 / 126 / 128+n)して通常の failed 経路へ。ラッパー自身は **0** で終了する(エージェントの終了コードは伝播しない) |
| exit の書き込み失敗 | 書けないまま終了(tick が「exitなし・プロセス死亡」として failed に分類)。エージェントは実行できているためラッパーは **0** |
