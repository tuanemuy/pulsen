# 適合テスト: TaskRepository

対象の契約: [Task ドメイン: TaskRepository](../../domains/task.md#taskrepository)(関連: [DegradedTask](../../domains/task.md#degradedtaskスナップショット破損タスク)、ADR-015)

TaskRepository のすべてのアダプター実装が共通で通す適合テストスイート。前提条件はポートのメソッド呼び出しと、契約に書かれたファイル形式のフィクスチャで組み立てる: タスクファイルのパス(`state/tasks/<task-id>.json` / `state/archive/<task-id>.json`)・人間可読な JSON・スナップショット埋め込み(ADR-015)は契約の一部であるため、破損系の前提条件、およびポートのメソッドだけでは到達できない配置状態(同一 ID の双方配置等)の構成は、この範囲でファイルを直接配置・加工してよい。それ以外の永続化技術の実装詳細には依存しない。

## create

| 前提条件 | 操作 | 期待結果 | 実装ステータス |
|---|---|---|---|
| 状態ディレクトリに何も無い(`state/tasks/` 不在) | 新規 Task で `create` | `Ok`。必要なディレクトリが自動作成され、`find` が `Active(Intact)` で同じ内容を返す | |
| `create` 済みの ID がある | 同じ ID の Task で `create` | `Err(Conflict)`(現役に存在。一意性はポートが担保し、呼び出し側の事前確認に依存しない) | |
| `create` → `archive` 済みの ID がある | 同じ ID の Task で `create` | `Err(Conflict)`(アーカイブに存在。一意性は現役・アーカイブ横断) | |
| `state/tasks/<task-id>.json` に JSON として不正な内容を置く(ファイルフィクスチャ) | 同じ ID の Task で `create` | `Err(Conflict)`。既存ファイルの内容は変更されない(存在判定はデコード可否によらない。破損ファイルを上書きせず修復の材料を消さない) | |
| 書き込み先を用意できない(`state/` が書き込み不能等。再現できるアダプター環境に限る) | `create` | `Err(Io)`。message を含む | |

## save / save_degraded

| 前提条件 | 操作 | 期待結果 | 実装ステータス |
|---|---|---|---|
| `create` 済みのタスク | 遷移後の値(実行状態・カウンタ・`updated_at` を変更)で `save` | `Ok`。直後の `find` が更新後の内容を返す(read-your-writes) | |
| `create` していない ID のタスク | `save` | `Err(NotFound)`(現役に存在しない) | |
| `create` → `archive` 済みのタスク | `save` | `Err(NotFound)`(アーカイブ側は `save` の対象外) | |
| スナップショットフィールドのみを不正な内容に書き換えたタスクファイルを `find` し、`SnapshotUnreadable(DegradedTask)` を得る | タスク側フィールドを変更(`abort` による Stopped 化等)して `save_degraded` | `Ok`。直後の `find` は変更後のタスク側フィールドを持つ `SnapshotUnreadable` を返し、スナップショットフィールドは元の(破損した)内容のままファイルに温存される(往復。修復の材料を消さない) | |
| 現役に存在しない ID の DegradedTask | `save_degraded` | `Err(NotFound)` | |
| `create` 済みのタスク。書き込み先へ書き込めない(`state/tasks/` が書き込み不能等。再現できるアダプター環境に限る) | `save` | `Err(Io)`。message を含む(部分的な書き込み結果を残さないことは「原子性の観測面」で検証) | |
| `find` で `SnapshotUnreadable(DegradedTask)` を得た状態で、書き込み先へ書き込めない(再現できるアダプター環境に限る) | `save_degraded` | `Err(Io)`。message を含む | |

## 往復可能性(デコード)

| 前提条件 | 操作 | 期待結果 | 実装ステータス |
|---|---|---|---|
| 全 Optional フィールドを持つタスク(workspace 確定、`current_attempt` に process 取込済み、`last_failure` あり、`Stopped { reason, notified_at: Some }`)を `create` する | `find` | `Active(Intact)`。スナップショットを含む全フィールドが元の値と等価(往復可能な保存) | |
| 各実行状態(Pending / Launching / Running / Completed / Failed / Stopped)のタスクをそれぞれ `create` / `save` する | それぞれ `find` | 各状態が付随データごと復元される(`Launching.recorded_at`、`Stopped.reason` / `notified_at` を含む) | |

## find と解決順

| 前提条件 | 操作 | 期待結果 | 実装ステータス |
|---|---|---|---|
| 何も作成していない(`state/tasks/` / `state/archive/` 不在) | `find` | `Ok(NotFound)`(ディレクトリ不在は空結果として扱う) | |
| `create` 済みのタスク | `find` | `Active(Intact(task))` | |
| `create` → `archive` 済みのタスク | `find` | `Archived(TaskRecord)` | |
| 同一 ID のタスクファイルを `state/tasks/` と `state/archive/` の双方に置く(ファイルフィクスチャ) | `find` | `Active` として返す(解決順は tasks → archive) | |
| 走査対象を読み取れない(`state/tasks/` が読み取り不能等。再現できるアダプター環境に限る) | `find` | `Err(Io)`。message を含む(`Ok(NotFound)` / `Corrupt` に写像しない。機構失敗は値のエラーとして呼び出し側に届く) | |

## Corrupt と SnapshotUnreadable の区別

| 前提条件 | 操作 | 期待結果 | 実装ステータス |
|---|---|---|---|
| タスクファイル全体を JSON として不正な内容に置き換える | `find` | `Corrupt { path, message }`(path は当該ファイル) | |
| タスク側フィールドの構文・値制約を破る(実行状態に未知の値、`task_id` の文字集合違反等)よう書き換える | `find` | `Corrupt`(タスク側フィールドの破れはファイル全体の破損として扱う) | |
| スナップショットフィールドのみを構文不正な内容に置き換える(タスク側フィールドは有効なまま) | `find` | `Active(SnapshotUnreadable(DegradedTask))`。message に理由を含み、タスク側フィールド(実行状態・カウンタ・attempt 参照等)はすべて読める | |
| スナップショットフィールドを**削除**する(不在。タスク側フィールドは有効なまま) | `find` | `Active(SnapshotUnreadable(DegradedTask))`(欠落も「スナップショットのみ読めない」に分類する。`Corrupt` に落とさない — pages 縮退表「スナップショット 不在・パース不能」) | |
| `task_status` を snapshot の statuses に無い名前に書き換える | `find` | `SnapshotUnreadable`(不変条件1の照合破れ。`RehydrateError::StatusNotInSnapshot` の写像) | |
| スナップショットの構造不変条件を破る(`initial ∉ statuses`、または AgentRun の `next ∉ statuses`)よう書き換える | `find` | `SnapshotUnreadable` | |
| 状態間整合の不変条件2〜4を破る内容(例: Running なのに `current_attempt.process` が無い)に書き換える(構文・値制約とスナップショットは有効なまま) | `find` | `Active(Intact)`(不変条件2〜4はデコードでは検証しない。遷移関数の前提検査 `InvariantViolated` に委ねる) | |
| `state/archive/` に JSON として不正な内容のタスクファイルを置く(ファイルフィクスチャ。現役側に同 ID なし) | `find` | `Corrupt { path, message }`(path はアーカイブ側の当該ファイル。破損の区分は tasks / archive で変わらない) | |
| `state/archive/` にスナップショットフィールドのみ構文不正なタスクファイルを置く(タスク側フィールドは有効なまま。現役側に同 ID なし) | `find` | `Archived(SnapshotUnreadable(DegradedTask))`。message に理由を含み、タスク側フィールドはすべて読める | |
| 上記の各破損フィクスチャのうち**現役側(`state/tasks/`)に置いたもの** | `list_active` | `find` と同じ区分で列挙される(`Corrupt` は `TaskEntry::Corrupt`、スナップショット破損は `Record(SnapshotUnreadable)`)。アーカイブ側のフィクスチャは現れない | |
| `state/tasks/` に命名形式(`<task-id>.json`)に合致しないエントリ(一時ファイル残骸・手動配置のファイル等)を置く | `list_active` | 形式外エントリは列挙されない(`Corrupt` としても現れない)。既存タスクの走査には影響しない(RunStore の `attempt-<n>` 形式外と同じ規則) | |

## archive

| 前提条件 | 操作 | 期待結果 | 実装ステータス |
|---|---|---|---|
| `create` 済みのタスク(`state/archive/` 不在) | `archive` | `Ok`。移動先ディレクトリが自動作成され、`find` は `Archived` を返す | |
| `archive` 直後 | `list_active` / `list_archived` / `find` | 現役側(`list_active` と `find` の現役扱い)から即座に消え、アーカイブ側に現れる(read-your-writes)。内容は移動前と等価 | |
| `create` していない ID | `archive` | `Err(NotFound)` | |
| `archive` 済みの ID | 再度 `archive` | `Err(NotFound)`(現役に存在しない) | |
| 移動先を用意できない(再現できるアダプター環境に限る) | `archive` | `Err(Io)`。タスクは現役側に完全な内容のまま残る(部分的な移動を残さない) | |

## list_active / list_archived

| 前提条件 | 操作 | 期待結果 | 実装ステータス |
|---|---|---|---|
| 走査対象ディレクトリが存在しない | `list_active` / `list_archived` | `Ok(空リスト)` | |
| 複数タスクを `create` し、うち1つを `archive` する | `list_active` | archive していないタスクのみが `Record(Intact)` で全件列挙される | |
| 同上 | `list_archived` | archive したタスクのみが列挙される | |
| 現役に正常タスク・全体破損ファイル・スナップショットのみ破損のタスクを混在させる | `list_active` | `Ok`。正常は `Record(Intact)`、全体破損は `Corrupt { path, message }`、スナップショット破損は `Record(SnapshotUnreadable)` としてすべて返り、個別の破損が走査全体を失敗させない | |
| アーカイブ側に正常タスク・全体破損ファイル・スナップショットのみ破損のタスクを混在させる(破損はファイルフィクスチャ) | `list_archived` | `Ok`。`list_active` と同じ区分ですべて返り、個別の破損が走査全体を失敗させない | |
| 走査対象ディレクトリが存在するが読み取り不能(再現できるアダプター環境に限る) | `list_active` / `list_archived` | `Err(Io)`。message を含む(走査自体の失敗はエラー。`Ok(空リスト)` に写像しない — 写像すると tick の無言の停滞を招く) | |

## 原子性の観測面

| 前提条件 | 操作 | 期待結果 | 実装ステータス |
|---|---|---|---|
| `create` 済みのタスク。別スレッド/プロセスから `find` / `list_active` を繰り返し呼び続けている | 内容(スナップショットを含む全体)を大きく変える `save` を反復する | すべての読み取りが、いずれかの完全な保存内容のみを観測する(フィールドの新旧混在・書きかけの内容が現れない。読み取りはロックなしで常に一貫した内容を返す) | |
| `save` が `Err` を返した(NotFound / Io) | `find` / `list_active` | 部分的な書き込み結果が残らない(対象は操作前の状態のまま、または NotFound のまま) | |
| `create` 済みのタスク。別スレッド/プロセスから `find` / `list_active` / `list_archived` を繰り返し呼び続けている | `archive` を実行する | 移動中の反復読み取りが「現役とアーカイブの両方に現れる」「どちらにも完全体が無い」という中間状態を観測しない(常にどちらか一方の完全な内容のみ)。完了後は Archived のみ、失敗後は Active のみが観測される | |

## 対象外

- 並行書き込みの調停: 契約どおりポートは調停しない(呼び出し側が ExclusiveLock を取得する前提)
- `Corrupt` と報告したファイルへの書き込み禁止: 呼び出し側の責務であり、ポートが拒否を担保する契約ではない
- 絞り込み・並び順・ページング: ユースケース側の責務(走査は全件返却のみ)
