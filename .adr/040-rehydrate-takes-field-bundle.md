# 040: 永続化からの再構築は「フィールド束の struct」を入力に取る

## ステータス

承認済み

## コンテキスト

`Task::rehydrate` は spec 上「全フィールド」を受け取る唯一の再構築経路であり、フィールドは11個ある。位置引数で並べると、呼び出し側(TaskRepository アダプターの復号、適合テストのフィクスチャ)が型の同じ `Option` を取り違えても気づけず、フィールドが増えるたびに全呼び出しの引数順が壊れる。

また `AttemptRef` は spec で「`record_launching` の内部でのみ生成される」と定められており、そのままでは永続化されたタスクを組み直せない。

## 決定

- `Task::rehydrate(fields: TaskFields)` / `DegradedTask::rehydrate(fields: DegradedTaskFields)` の形にする。`TaskFields` / `DegradedTaskFields` は公開フィールドの struct で、definition ドメインの `GlobalConfigInput` と同じ「境界で一度だけ検証する入力の束」である。検証(不変条件1 = `task_status ∈ snapshot.statuses`)は `Task::rehydrate` が行い、束の型自体は不変条件を持たない。
- `AttemptRef::rehydrate` / `RetryCounters::rehydrate` を公開の再構築コンストラクタとして置く。新規採番(`record_launching` が `RunDirPath::derive` で番号とパスの整合を構成で保証する)は再構築とは別の口にする。

## 検討した代替案

- 位置引数のまま `Task::rehydrate(id, workflow_name, target, snapshot, task_status, execution, workspace, current_attempt, counters, last_failure, updated_at)` にする — 引数の取り違えがコンパイルで捕まらず、フィールド追加が全呼び出しに波及する
- ビルダーを持たせる — 「必須フィールドを与え忘れた」状態を型で排除できず、`parse, don't validate` の境界が1回にならない

## 影響

- フィールドの追加が呼び出し側の引数順に波及しない
- 適合テストのフィクスチャが名前つきで組め、実行状態6種と全 Optional フィールドの組み合わせを網羅できる
- トレードオフ: 束の struct が2つ増え、エンティティのフィールドと二重に並ぶ。生成経路が `register` / `rehydrate` の2つだけである点は変わらない
