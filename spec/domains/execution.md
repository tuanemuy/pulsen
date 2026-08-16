# Execution

実行の観測と判断を担うドメイン。runディレクトリの語彙、launching / running の分類、判定プロトコル、gcの保護規則を、**観測された事実を入力に取り決定を返す純粋なサービス**として定義する。副作用(プロセス・worktree・コマンド実行・runディレクトリの読み書き)はすべてこのドメインのポート越しに行い、tick はどの時点でクラッシュしても次のtickが同じ入力から同じ決定を再導出できる(冪等)。

仕様の根拠: requirements §4.1〜§4.3・§6(全節)・§8・§9.1・§9.2、ADR-008・011・012・016。

## ユビキタス言語

| 英語名 | 日本語名 | 定義 |
|---|---|---|
| RunDirectory | runディレクトリ | attempt単位の実行メタデータ置き場(`state/runs/<task-id>/attempt-<n>/`) |
| Wrapper | ラッパー | ツール自身のバイナリのラッパーモード。同定情報と終了結果をrunディレクトリへ永続化する |
| InvalidationMarker | 無効化マーカー | 遅延起動したラッパーの起動を抑止するファイル。spawn失敗の分類・abort時にツールが書く |
| GracePeriod | 猶予時間 | launching記録からpidファイル出現を待つ時間(30秒・組み込み定数) |
| Judgement | 判定 | 終了した実行を completed / failed / skipped に分類すること |
| JudgeProtocol | 判定プロトコル | 判定コマンドの exit code の4値解釈(0 / 10 / 20 / それ以外) |
| Aliveness | 生存 | PIDの起動時刻照合(§6.2)を通過した「同一プロセスが生存している」判定 |
| Gc | gc | 保持期間を超えたattemptのrunディレクトリの削除(ADR-011) |

## 値オブジェクト

### ExitCode

- フィールド: `i32`
- 意味: エージェント実行の終了結果の符号化値(requirements §4.1)。ラッパーが書く: 正常終了は exit code、シグナル等は POSIX 慣例の非0値(128+シグナル番号)、エージェント起動不能はシェル慣例(コマンド不在 127、実行不能 126)
- 振る舞い: `is_success(&self) -> bool`(0 か否か)
- 等価性: 数値の一致

### PidFileContent

- フィールド: `pid: Pid`、`kill_ident: KillIdent`(型は Task ドメイン)
- 意味: ラッパーが書くpidファイルの内容。tick はこの出現をもって同定情報一式が揃ったとみなす(starttime → pid の書き込み順序をラッパーが守るため)

### JudgeOutcome / JudgeConclusion

```
JudgeOutcome     = Completed | Failed | Skipped
JudgeConclusion  = Outcome(JudgeOutcome) | JudgeFailure { detail: String }
DefaultJudgement = Completed | Failed
```

- `Skipped` は判定コマンドでのみ生じる(ADR-008)
- `DefaultJudgement` は判定コマンド未定義のときの2値。デフォルト判定が `Skipped` を導けないことを返り値型で述べる。この経路の結末は2値のまま写す
- `JudgeOutcome` が3値のまま残るのは、判定コマンドの exit 20(`interpret_judge_completion`)が `Skipped` を生むためである。`From<DefaultJudgement> for JudgeOutcome` は2値が3値に含まれることを型で示す変換として提供する
- `JudgeFailure` は「判定自体が壊れた」(プロトコル外の exit code・判定timeout・判定コマンド起動不能)

### 分類の決定(直和型)

```
LaunchingDecision =
  ConfirmRunning(ProcessIdent)     // pidファイルあり → running へ
| KeepWaiting                      // pidなし・猶予内 → 何もしない
| SuspectSpawnFailure              // pidなし・猶予超過 → マーカー書き込みと再確認へ

LaunchingRecheck =
  ConfirmRunning(ProcessIdent)     // 再確認でpidが現れていた → running へ
| SpawnFailed                      // なお存在しない → spawn失敗確定

RunningDecision =
  Judge(ExitCode)                  // exitファイルあり → 判定を実行
| KeepRunning                      // exitなし・生存・timeout未超過
| KillOnTimeout                    // exitなし・生存・timeout超過 → kill して failed
| DiedWithoutExit                  // exitなし・死亡 → 残存終了ベストエフォート後 failed

AliveDecision =
  KeepRunning | KillOnTimeout | DiedWithoutExit
                                   // `RunningDecision` から `Judge` を除いた3値。
                                   // `From<AliveDecision> for RunningDecision` で合流させる

Aliveness = Alive | Dead           // Dead は「取得不能」と「起動時刻の不一致(PID再利用)」の両方を含む
```

### CommandCompletion

```
CommandCompletion = Exited(ExitCode) | TimedOut | FailedToStart { message: String }
```

- CommandRunner の結果。すべての結末を値で表す(エラーを含めて判定・通知の分類の入力になる)

### GcPlan

- フィールド: `deletions: Vec<(String, AttemptNumber)>`(runディレクトリ名の生文字列とattempt番号)
- `GcPolicy::plan` の出力。ディレクトリ名を `TaskId` にパースできない孤児も削除対象になり得るため生文字列で扱う

## ドメインサービス

すべて純粋(依存ポートなし)。観測はユースケースがポートで行い、値にしてサービスへ渡す。

### IdentityCheck

- 責務: PID再利用対策の起動時刻照合(requirements §6.2)
- メソッド: `check(observed: Option<&ProcessStartTime>, recorded: &ProcessStartTime) -> Aliveness`
  - `None`(取得不能)→ `Dead`。不一致 → `Dead`(別プロセスに再利用された)。一致 → `Alive`
  - kill の実行可否の判定にも使う(照合一致時のみ kill する)

### LaunchingClassifier

- 責務: launching タスクの分類(requirements §4.1)
- 定数: `GRACE_PERIOD = 30秒`(組み込み)
- メソッド:
  - `classify(recorded_at: &Timestamp, now: &Timestamp, pid: Option<PidFileContent>, starttime: Option<StartTimeRecord>) -> Result<LaunchingDecision, InconsistentRunFiles>`
    - pid・starttime とも Some → `ConfirmRunning(ProcessIdent)`
    - pid None・経過(`now - recorded_at`、負なら0)が猶予内 → `KeepWaiting`
    - pid None・猶予超過 → `SuspectSpawnFailure`
    - pid Some・starttime None → `Err(InconsistentRunFiles)`(ラッパーの書き込み順序保証の破れ。当該tickではスキップして報告し、次tickで再観測する)
    - `InconsistentRunFiles` — 破れの**種別だけを持つ列挙**(現在の変種は `MissingStartTime` の1つ)。文言は表示側が組み立てる。どのタスク・どの run_dir かの文脈付与は呼び出し側(報告時)の責務(純粋サービスに報告用のパスを持ち込まない)
  - `classify_recheck(pid: Option<PidFileContent>, starttime: Option<StartTimeRecord>) -> Result<LaunchingRecheck, InconsistentRunFiles>`
    - マーカー書き込み後の再確認。pid・starttime とも Some → `ConfirmRunning`、pid None → `SpawnFailed`(starttime の有無は問わない — starttime のみは書き込み順序 starttime → pid の正常な中間状態)、pid Some・starttime None → `Err`(本体 `classify` と同じ場合分け)

### RunningClassifier

- 責務: running タスクの分類(requirements §6.1)
- 分類は2段で行う。exit ファイルがあれば生存観測(`starttime_of` + IdentityCheck)は**不要かつ行わない**(exit があれば実行は終了しており、観測の一過性失敗で判定を遅延させない):
  - **1段目(exit の有無)はユースケースが値にする** — exit が Some なら生存を観測せず `RunningDecision::Judge(exit)` とする
  - **2段目(生存)だけを `classify_alive` が受け持つ** — exit が None のとき、生存を観測してから呼ぶ
- メソッド: `classify_alive(aliveness: Aliveness, started_wall: &Timestamp, timeout: &TimeoutSpec, now: &Timestamp) -> AliveDecision`
  - 「`Judge` は返さない」を doc ではなく**返り値型で担保する**(`AliveDecision` に `Judge` が無い)。呼び出し側は `RunningDecision` へ合流させてから網羅 `match` で分岐する
  - `Alive`・timeout 未超過(または `Unlimited`)→ `KeepRunning`
  - `Alive`・timeout 超過 → `KillOnTimeout`
  - `Dead` → `DiedWithoutExit`
  - timeout の経過は `started_wall`(StartTimeRecord.wall。記録済みstarttimeの壁時計成分)を起点に測る(launching の猶予時間は含まれない。§6.1)。経過が負なら 0 として扱う

### JudgementService

- 責務: 終了した実行の判定(requirements §6.3)
- メソッド:
  - `default_judgement(exit: &ExitCode) -> DefaultJudgement` — 0 = `Completed`、非0 = `Failed`。「`Skipped` は返さない」を返り値型で担保する
  - `interpret_judge_completion(c: &CommandCompletion) -> JudgeConclusion` — `Exited(0)` = Completed、`Exited(10)` = Failed、`Exited(20)` = Skipped、`Exited(その他)` / `TimedOut` / `FailedToStart` = `JudgeFailure`(detail に原因を記す)
  - `judge_env(task_id: &TaskId, workspace: &WorktreePath, exit: &ExitCode, run_dir: &RunDirPath) -> Vec<(String, String)>` — `TASK_ID` / `WORKSPACE` / `EXIT_CODE`(10進文字列)/ `RUN_DIR`。引数は使わない(requirements §6.3)
- 判定は冪等: 同じ exit・同じ定義に対して常に同じ結論を導く(判定コマンド自体の冪等性は利用者の責務。setup.md シナリオ5)

### NotificationService

- 責務: stopped 確定通知の環境変数の構成と、通知の結末の成否の解釈(requirements §8)
- メソッド:
  - `notify_env(task_id: &TaskId, workflow: &WorkflowName, task_status: &StatusName) -> Vec<(String, String)>` — `TASK_ID` / `WORKFLOW` / `TASK_STATUS`
  - `interpret_notify_completion(c: &CommandCompletion) -> NotifyOutcome` — `Exited(0)` = `Delivered`。非0終了 / `TimedOut` / `FailedToStart` はいずれも `Failed` で、**原因は分類として持ち完成文言は持たない**(文言は CLI 層が組み立てる)。`Failed` は `notified_at` を書かずに終える — 次のtickが再通知する(at-least-once。requirements §8)。「`Exited(0)` だけが `notified_at` を書く根拠になる」という規則を、stopped を書くすべての経路(tick の各上限超過・DegradedTask の再通知・abort)が共有するためにドメインへ置く(経路ごとに書くと requirements §8 の at-least-once が片方だけで破れる)。隣の `interpret_judge_completion` と解釈の位置が揃う

  ```
  NotifyOutcome      = Delivered | Failed { cause: NotifyFailureCause }
  NotifyFailureCause = ExitedNonZero { exit: ExitCode } | TimedOut | FailedToStart { message: String }
  ```

  - `TimedOut` がフィールドを持たないのは、通知の timeout が設定値ではなく組み込み定数 `NOTIFY_TIMEOUT` の1つに定まるため(表示側が定数を読む)
  - `Failed` を平坦化せず `cause` を内側に持つのは、`Delivered` / `Failed` の2分岐が at-least-once の規則そのものだから(4変種にすると、`notified_at` を書く根拠を呼び出し側が3変種の列挙で書くことになる)
- 定数: `NOTIFY_TIMEOUT = 60秒`(組み込み。ADR-018)。notify_cmd の実行にはこのtimeoutを必ず適用する(ハングした通知コマンドが排他ロックを保持したまま tick / CLI を塞ぐことを防ぐ)
- notify_cmd が未定義(`GlobalConfig.notify_cmd = None`)の場合は通知を行わず、**`notified_at` も書かない**(「通知した」という虚偽の記録を作らない)。後から notify_cmd を定義すると、次のtickが `notified_at` のない stopped タスクを検出して catch-up 通知する(凍結中で対応が必要なタスクの通知であり、有用な挙動として意図する)

### GcPolicy

- 責務: runディレクトリgcの削除対象決定(requirements §9.2、ADR-011)
- 入力型:

  ```
  RunListing    = Vec<RunDirListing { dir_name: String, attempts: Vec<AttemptInfo> }>
  AttemptInfo   = { number: AttemptNumber, last_activity: Timestamp }
  TaskProtection =
    ActiveCurrent(Option<AttemptNumber>)  // 現役(stopped以外): 現在参照attemptのみ保護
  | AllProtected                          // 現役のstopped、またはパース不能なタスクファイル: 全attempt保護
  | Unprotected                           // アーカイブ済み・タスクファイル不在の孤児
  ```

- メソッド: `plan(listing: &RunListing, protection: &Map<String, TaskProtection>, retention: &DurationSpec, now: &Timestamp) -> GcPlan`
  - 削除対象: 保護されておらず、`now - last_activity > retention` の attempt
  - `protection` に無い `dir_name` は `Unprotected` として扱う(孤児)
  - スナップショットのみ破損した現役タスク(DegradedTask)は、実行状態と現在attempt参照が読めるため通常規則を適用する(stopped なら `AllProtected`、それ以外は `ActiveCurrent`)。「パース不能 = 全attempt保護」(requirements §9.2)はファイル全体が読めない `Corrupt` のみを指す
  - `last_activity` の算出(ディレクトリ内ファイルのmtime最大値、ファイルがなければディレクトリ自体のmtime)は RunStore アダプターの契約
- 削除の失敗はどのカウンタも消費せず stopped も発生させない(タスクの帳簿と無関係。requirements §9.2)。空になったタスクディレクトリの削除はユースケースが `GcPlan` 適用後に行う

## RunDirPath のファイル配置(語彙)

Task ドメインの `RunDirPath` に対する純粋な導出関数としてファイル配置を固定する。

| 関数 | パス | 内容 |
|---|---|---|
| `pid_file()` | `<run_dir>/pid` | `PidFileContent`(pid・kill同定子) |
| `starttime_file()` | `<run_dir>/starttime` | `StartTimeRecord`(同定用の不透明値 + 壁時計時刻) |
| `exit_file()` | `<run_dir>/exit` | `ExitCode` |
| `stdout_log()` | `<run_dir>/stdout.log` | エージェントの標準出力(ラッパーがリダイレクト) |
| `stderr_log()` | `<run_dir>/stderr.log` | エージェントの標準エラー |
| `marker_file()` | `<run_dir>/invalidated` | 無効化マーカー(存在のみが意味を持つ空ファイル) |

逆写像 `state_root(&self) -> Option<StateRoot>` も同じ語彙に属する(Task ドメインの `RunDirPath` に定義。`derive` との一致を条件に復元する)。

## ポート

### RunStore

- 目的: runディレクトリの読み書きとgcの実行
- メソッドとエラー:

| メソッド | シグネチャ | 備考 |
|---|---|---|
| `prepare_attempt` | `(id: &TaskId, n: AttemptNumber) -> Result<RunDirPath, Io>` | ディレクトリ作成(親含む)。冪等 |
| `read_pid_file` | `(run_dir) -> Result<Option<PidFileContent>, RunFileError>` | 不在 = None |
| `read_starttime` | `(run_dir) -> Result<Option<StartTimeRecord>, RunFileError>` | 同上 |
| `read_exit` | `(run_dir) -> Result<Option<ExitCode>, RunFileError>` | 同上 |
| `attempt_exists` | `(run_dir) -> Result<bool, Io>` | attempt ディレクトリ自体の存在確認(show の「runディレクトリは存在しない(gc済み等)」表示用。read系の `Ok(None)` では「ディレクトリごと不在」と「空ディレクトリ」を区別できないため) |
| `write_invalidation_marker` | `(run_dir) -> Result<(), Io>` | ディレクトリ不在なら作成して書く(pages ※8b)。冪等 |
| `marker_exists` | `(run_dir) -> Result<bool, Io>` | ラッパーが使用 |
| `write_starttime` | `(run_dir, rec: &StartTimeRecord) -> Result<(), Io>` | ラッパーが使用 |
| `write_pid_file` | `(run_dir, c: &PidFileContent) -> Result<(), Io>` | ラッパーが使用 |
| `write_exit` | `(run_dir, code: &ExitCode) -> Result<(), Io>` | ラッパーが使用 |
| `list_runs` | `() -> Result<RunListing, Io>` | gc用。`last_activity` の算出規則は下記 |
| `delete_attempt` | `(dir_name: &str, n: AttemptNumber) -> Result<(), Io>` | 失敗は呼び出し側がスキップ・報告 |
| `remove_task_dir_if_empty` | `(dir_name: &str) -> Result<(), Io>` | attempt が全て消えた後の親削除。**空でない(attempt・形式外エントリの残存を含む)場合は削除せず `Ok` を返す**(エラーではない — 形式外エントリが残る限り毎tick `gc_errors` に報告され続けることを防ぐ) |

- `RunFileError = Corrupt { path, message } | Io { message }` — 内容が不正なファイルは「不在」と区別する(呼び出し側は当該タスクをその tick でスキップして報告し、次 tick で再観測する)
- 契約:
  - **ディレクトリ自体の不在もファイル不在と同様に `Ok(None)` とする**(read系)。launching記録と `prepare_attempt` の間のクラッシュで run_dir が存在しないまま観測されるのは正常な復旧経路であり、猶予時間経路の分類(pending 復帰)に合流させる(pages ※8)。`list_runs` も `state/runs/` 不在時は空の `RunListing` を返す
  - **write 系(`write_starttime` / `write_pid_file` / `write_exit` / `write_invalidation_marker`)はいずれも書き込み先のディレクトリを必要に応じて作る。** `prepare_attempt` が失敗した後も spawn は行われる設計であり、ラッパーが自力でディレクトリを作って書けることが自己修復の前提になる
  - `write_starttime` / `write_pid_file` / `write_exit` はアトミック置換で行い、書きかけの内容が観測されない(requirements §9.2)
  - 読み取りは書き込みと並行しても常に「不在」か「完全な内容」のどちらかを観測する
  - `list_runs` の `last_activity`: ディレクトリ内ファイルの最終更新時刻の最大値。ファイルが1つもない場合はディレクトリ自体の最終更新時刻(requirements §9.2)
  - `list_runs` は `attempt-<n>` 形式に合致しないエントリ(手動で置かれたファイル・不正な名前のディレクトリ)を列挙対象外とする(gc はそれらに触れない)。その残存により `remove_task_dir_if_empty` が親を削除できないことは許容する(ユーザーが置いたものを黙って消さない)
  - 一意性・参照整合性: 関与しない(パスは呼び出し側が `RunDirPath::derive` で決定的に導出する)
  - 並行性: ラッパーとtickが同一ファイルへ同時書き込みしない設計(書き手はファイル種別ごとに一意)を前提とし、ポートは調停しない

### ProcessController

- 目的: プラットフォーム抽象(requirements §4.3)のうちプロセスに関わる操作
- メソッドとエラー:

| メソッド | シグネチャ | 備考 |
|---|---|---|
| `spawn_wrapper` | `(spec: &WrapperLaunchSpec) -> Result<(), SpawnError>` | ツール自身のバイナリをラッパーモードで、新しいプロセスグループ相当の単位でデタッチ起動する。起動後の成否は関知しない(観測はrunディレクトリ経由)。**このメソッドの同期エラーに対してユースケースは状態を変更しない**(launching記録は済んでおり、猶予時間経路が分類する) |
| `starttime_of` | `(pid: Pid) -> Result<Option<ProcessStartTime>, Io>` | None = プロセス不在(死亡)。生存確認はこの取得と `IdentityCheck` への還元(§4.3)。`Err(Io)`(取得機構自体の失敗)は Alive / Dead のどちらにも写像せず、呼び出し側は状態を変更しない(abort は非0エラーで再実行を案内、tick は当該タスクをスキップして報告し次tickで再観測) |
| `kill` | `(ident: &KillIdent) -> Result<(), KillError>` | プロセスグループ相当の一括終了。呼び出し前提: `IdentityCheck` が `Alive`。**失敗時は呼び出し側が状態を変更しない** — abort は stopped を記録せずエラー(requirements §6.5)、tick の `KillOnTimeout` は `fail_run` を呼ばず報告のみ行い、次tickが同じ決定を再導出して再試行する(プロセス生存のまま failed → 再起動 → 同一worktreeでの並走、を防ぐ) |
| `try_kill_remnants` | `(ident: &KillIdent) -> RemnantOutcome` | ラッパー死亡後の残存プロセスの終了。`Killed \| NotIdentifiable \| Failed { message }`。誤殺なく同定できる場合に限り実行するベストエフォート(§6.2)。結果は分類(failed)に影響しない(tickサマリーの報告のみ) |
| `own_identity` | `() -> Result<WrapperIdentity, Io>` | ラッパーモードが自身の pid・kill同定子・`StartTimeRecord` を取得する |
| `run_agent` | `(cmd: &CommandLine, cwd: &WorktreePath, stdout: &Path, stderr: &Path) -> ExitCode` | ラッパーモードがエージェントを同期実行する。cwd は常に worktree(§4.1)。標準出力・標準エラーを指定パスへリダイレクト。起動不能は 127 / 126、シグナル死は 128+n に符号化して返す(常に ExitCode を返し、失敗しない)。リダイレクト先を開けない場合(権限・ディスク満杯等)はエージェントを起動せず 126 に符号化して返す(通常の実行失敗として failed 経路に合流させる) |

- `WrapperLaunchSpec { run_dir: RunDirPath, agent_cmd: CommandLine, workspace: WorktreePath }` — ラッパーモードの引数へ直列化される
- `WrapperIdentity { pid: Pid, kill_ident: KillIdent, starttime: StartTimeRecord }`
- エラー型(いずれも分類には使わず、報告・失敗要因の記録にのみ使うため、不透明な message で足りる):

  ```
  SpawnError = Failed { message }   // OSレベルの起動失敗。状態は変更しない(猶予経路が分類する)
  KillError  = Failed { message }   // シグナル送出・ジョブ終了自体のエラー
  ```
- 契約: `starttime_of` の値は記録時と**同一の取得手段**で取得する(照合の前提。§4.3)。kill同定子はツールの再起動後でもタスクファイルの情報だけで kill を実行できる形式とする

### WorktreeManager

- 目的: 対象(リポジトリ・ブランチ)の検証と worktree の作成・削除
- メソッドとエラー:

| メソッド | シグネチャ | エラー |
|---|---|---|
| `validate_repo` | `(repo: &RepoPath) -> Result<(), TargetError>` | `NotFound` / `NotARepository` |
| `head_branch` | `(repo: &RepoPath) -> Result<BranchName, TargetError>` | `DetachedHead` / `EmptyRepository`(`--base` の明示を案内。pages add) |
| `branch_exists` | `(repo: &RepoPath, branch: &BranchName) -> Result<bool, TargetError>` | |
| `create` | `(repo: &RepoPath, base: &BranchName, ws: &Workspace) -> Result<(), WorktreeError>` | `Failed { message }` |
| `remove` | `(repo: &RepoPath, path: &WorktreePath) -> Result<RemoveOutcome, WorktreeError>` | `Failed { message }` |

- `RemoveOutcome = Removed | AlreadyAbsent` — 既に存在しない場合は成功(達成済み。cleanup.md)。ブランチには一切触れない(requirements §9.1)
- `remove` は worktree の内容の状態(未コミット変更・未追跡ファイル・`index.lock` 等の残骸)によらず削除する(git worktree remove --force 相当)。クリーンアップはエージェント実行後のほぼ常に dirty な worktree を対象とするため、クリーンな worktree しか消せない実装は主経路で破綻する
- `create` は `ws.path` の親ディレクトリ(worktree_root)が存在しなければ作成する(ツールが管理する領域のため。初回タスクの起動で必ず通る経路)
- エラー型:

  ```
  TargetError   = NotFound | NotARepository | DetachedHead | EmptyRepository | Failed { message }
  WorktreeError = Failed { message }   // git 操作の失敗。内訳は不透明な message でよい(分類に使わない)
  ```

- 契約:
  - `create` は「`base` から新しいブランチ `ws.branch` を作り、`ws.path` に worktree を用意する」(git worktree add 相当)
  - **`create` は自タスクの残骸に対して冪等**: `ws.path` に `ws.branch` の worktree として既に存在する場合は成功(達成済み)とみなす。ブランチ `ws.branch` だけが存在し worktree がない場合は、既存ブランチに worktree を張り直して成功させる(ブランチ名はタスクIDから決定的に導出され、タスクIDは現役・アーカイブ横断で一意なため、残骸は自タスクのものと同定してよい)。これにより「worktree作成成功 → タスクファイル保存前のクラッシュ」から次のtickが自動復旧する(task-execution.md「利用者による修復操作は不要」)
  - 上記以外の予期しない状態(`ws.path` に worktree でないディレクトリがある等)の自動修復は行わない(§2 の責務境界。再試行が同じエラーで失敗し続ければリトライ上限超過 → stopped → 人間の対応に委ねる)
  - worktree の中身の読み書き・リセットは行わない(作成と削除のみ。§2)
  - 依存インストール等の初期化は関知しない(requirements §5)

### CommandRunner

- 目的: 判定コマンド・通知コマンドの直接起動(シェル非経由)
- メソッド: `run(cmd: &PlainCommand, env: &[(String, String)], timeout: Option<&DurationSpec>) -> CommandCompletion`
- 契約:
  - シェルを介さず直接起動する(requirements §3.1・§6.3)。プレースホルダ展開は行わない
  - 環境変数は呼び出しプロセスの環境を継承し、`env` の変数を追加・上書きする
  - 作業ディレクトリは規定しない(呼び出しプロセスの cwd のまま。requirements §6.3)
  - `timeout` 指定時、超過したら起動したプロセスを終了させ `TimedOut` を返す
  - コマンドの実体が見つからない・起動できない場合は `FailedToStart`
  - exit code を持たない終了(シグナル死等)は、`run_agent` と同じ規則で非0の符号化値(POSIX では 128+シグナル番号)の `Exited` として返す(判定コマンドのシグナル死はプロトコル外の値として `JudgeFailure` に合流する)
  - 同期実行(終了まで待つ)
  - 標準出力・標準エラーは捕捉しない(呼び出しプロセスの出力へそのまま流れる。cron 運用ではスケジューラーのログに残る)。判定失敗の調査は `JudgeFailure.detail`(プロトコル外の exit code・timeout・起動不能の別)と利用者側の出力確認で行う

### ExclusiveLock

- 目的: tick の二重起動防止と、tick・状態変更CLI(add / abort / retry / set-status)の相互排他(requirements §4.3・§10)
- メソッド: `try_acquire(&self) -> Result<Option<LockGuard>, LockError>`
- エラー型: `LockError = Failed { message }`(ロック機構自体の異常。「取得できなかった」は `Ok(None)` でありエラーではない)
- 契約:
  - 単一のグローバルロック(グローバルホームに1つ)。すべての状態変更操作が同じロックを取る
  - ブロックしない: 取得できなければ即座に `None`(tick は 0 でスキップ、CLI は非0 で「別の操作が実行中」。pages 共通事項)
  - `LockGuard` のドロップで解放される。保持プロセスの異常終了でもOSにより解放される(アドバイザリロック相当)
  - 排他の単位は**プロセス間**である。同一プロセス内での再取得の挙動は規定しない(tick と状態変更CLIは常に別プロセスであり、運用上発生しない。fcntl 系のプロセス単位ロック実装も適合する)
  - 読み取り専用操作(ls / show)は取得しない

## ユースケース(概要)

- tick: LaunchingClassifier / RunningClassifier / JudgementService / IdentityCheck による分類 → Task の遷移関数 → ポートで実行。GcPolicy による gc(`run_retention` 設定時)
- wrapper(内部コマンド): own_identity → starttime / pid の書き込み(この順)→ マーカー確認 → run_agent → exit の書き込み
- abort: IdentityCheck + kill(または無効化マーカーの書き込みと再確認)
- add: WorktreeManager の検証系(validate_repo / head_branch / branch_exists)
- 通知(tick / abort): NotificationService + CommandRunner
