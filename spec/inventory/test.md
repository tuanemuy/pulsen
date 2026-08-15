# Inventory — test

生成元: spec/testcases/（最終同期: 2026-08-16）

| ID | 要素 | 定義場所 | 実装されるべき振る舞いの要点 |
|----|------|---------|------------------------------|
| TC-task-list-tasks-001 | ListTasks 正常系: 絞り込みなしで一覧する(異なるワークフロー・タスクステータス・実行状態の現役タスクが複数ある) | spec/testcases/task/list-tasks.md#正常系 | 各タスクのID・ワークフロー名・リポジトリ・ブランチ・タスクステータス・実行状態・attempt_count・更新日時が表示されて 0 で終了する |
| TC-task-list-tasks-002 | ListTasks 正常系: 一覧する(タスクステータスと実行状態が同名になり得るタスクがある) | spec/testcases/task/list-tasks.md#正常系 | タスクステータスと実行状態の両方が常に表示され、区別できる |
| TC-task-list-tasks-003 | ListTasks 正常系: `--status` で絞り込む(特定のタスクステータスを持つタスクと持たないタスクが混在する) | spec/testcases/task/list-tasks.md#正常系 | 一致するタスクのみが表示される |
| TC-task-list-tasks-004 | ListTasks 正常系: `--state` に有効値を指定して絞り込む(異なる実行状態のタスクが混在する) | spec/testcases/task/list-tasks.md#正常系 | 一致する実行状態のタスクのみが表示される |
| TC-task-list-tasks-005 | ListTasks 正常系: 両方を指定して一覧する(`--status` と `--state` の両条件に部分的に一致するタスクが混在する) | spec/testcases/task/list-tasks.md#正常系 | 両条件を AND で満たすタスクのみが表示される |
| TC-task-list-tasks-006 | ListTasks 正常系: 絞り込みなしで一覧する(アーカイブ済みタスクが存在する) | spec/testcases/task/list-tasks.md#正常系 | アーカイブ済みタスクは表示されない(既定は現役のみ) |
| TC-task-list-tasks-007 | ListTasks 正常系: `--all` で一覧する(アーカイブ済みタスクが存在する) | spec/testcases/task/list-tasks.md#正常系 | アーカイブ済みタスクも表示され、行にその旨の印が付き、ブランチも表示される |
| TC-task-list-tasks-008 | ListTasks 正常系: `--all` と絞り込みを併用する(現役・アーカイブ両方に条件一致タスクがある) | spec/testcases/task/list-tasks.md#正常系 | 対象集合の拡張(現役+アーカイブ)後に絞り込みが適用される |
| TC-task-list-tasks-009 | ListTasks 正常系: 一覧する(タスクが1件もない) | spec/testcases/task/list-tasks.md#正常系 | 空である旨を表示して 0 で終了する |
| TC-task-list-tasks-010 | ListTasks 正常系: 絞り込み付きで一覧する(絞り込み条件に一致するタスクがない) | spec/testcases/task/list-tasks.md#正常系 | 空である旨を表示して 0 で終了する |
| TC-task-list-tasks-011 | ListTasks 異常系: 一覧する(config.yaml が存在しない・パース不能・読めないのいずれか) | spec/testcases/task/list-tasks.md#異常系 | 非0で終了する(pages ※1) |
| TC-task-list-tasks-012 | ListTasks 異常系: 一覧する(`--state` に固定6値以外の値を指定する) | spec/testcases/task/list-tasks.md#異常系 | 有効な値の一覧(pending / launching / running / completed / failed / stopped)を添えて非0で終了する |
| TC-task-list-tasks-013 | ListTasks 異常系: 一覧する(`state/tasks/` の走査自体が I/O エラーで失敗する(権限不足等)) | spec/testcases/task/list-tasks.md#異常系 | 実行環境エラーとして非0で終了する |
| TC-task-list-tasks-014 | ListTasks 異常系: `--all` で一覧する(`state/archive/` の走査自体が I/O エラーで失敗する) | spec/testcases/task/list-tasks.md#異常系 | 実行環境エラーとして非0で終了する |
| TC-task-list-tasks-015 | ListTasks 境界値: `--state` に `pending` /…(6つの実行状態それぞれのタスクがある) | spec/testcases/task/list-tasks.md#境界値 | 6値すべてが受理され、それぞれ該当タスクのみが表示される |
| TC-task-list-tasks-016 | ListTasks 境界値: `--state` に大文字混じりの値(`Pending` 等)を指定する | spec/testcases/task/list-tasks.md#境界値 | 小文字の6値のみ受理のため、有効値一覧を添えて非0で終了する |
| TC-task-list-tasks-017 | ListTasks 境界値: `--state` に空文字列を指定する | spec/testcases/task/list-tasks.md#境界値 | 有効値一覧を添えて非0で終了する |
| TC-task-list-tasks-018 | ListTasks 境界値: `--status` にその値を指定する(どのタスクにも存在しないタスクステータス名がある) | spec/testcases/task/list-tasks.md#境界値 | 値は検証されず(ユーザー定義語彙)、該当0件として空の一覧を 0 で返す |
| TC-task-list-tasks-019 | ListTasks エッジケース: 一覧する(パース不能なタスクファイル(`Corrupt`)と正常なタスクが混在する) | spec/testcases/task/list-tasks.md#エッジケース | 破損ファイルはパスと読めない旨として報告され、残りのタスクは表示され、0 で終了する(修復の入口。pages ※5) |
| TC-task-list-tasks-020 | ListTasks エッジケース: 一覧する(スナップショットのみ読めないタスク(`SnapshotUnreadable`)がある) | spec/testcases/task/list-tasks.md#エッジケース | 行として表示され、スナップショット読み取り不能の印が付き、0 で終了する |
| TC-task-list-tasks-021 | ListTasks エッジケース: `--status` / `--state` で絞り込む(`SnapshotUnreadable` のタスクが絞り込み条件に一致する) | spec/testcases/task/list-tasks.md#エッジケース | 実行状態・タスクステータスは読めているため、絞り込みの対象になる |
| TC-task-list-tasks-022 | ListTasks エッジケース: 一覧する(`state/tasks/` ディレクトリが存在しない) | spec/testcases/task/list-tasks.md#エッジケース | 空の一覧として 0 で終了する(pages 縮退表) |
| TC-task-list-tasks-023 | ListTasks エッジケース: `--all` で一覧する(`state/archive/` ディレクトリが存在しない) | spec/testcases/task/list-tasks.md#エッジケース | アーカイブ分は空として扱われ、現役のみ表示して 0 で終了する |
| TC-task-list-tasks-024 | ListTasks エッジケース: `--all` で一覧する(アーカイブ側にパース不能なタスクファイルがある) | spec/testcases/task/list-tasks.md#エッジケース | 一覧全体を失敗させず、読み取り不能として報告される |
| TC-task-list-tasks-025 | ListTasks エッジケース: 一覧する(別の操作が排他ロックを保持している) | spec/testcases/task/list-tasks.md#エッジケース | ロックを取得しないため通常どおり一覧が表示され 0 で終了する |
| TC-task-list-tasks-026 | ListTasks エッジケース: 一覧する(tick による更新と同時に読み取る) | spec/testcases/task/list-tasks.md#エッジケース | 更新はアトミック置換のため、書きかけの内容が観測されることはない |
| TC-task-register-task-001 | RegisterTask 正常系: ワークフロー名 `implement` とリポジトリ…(有効な config.yaml と `workflows/implement.yaml`…) | spec/testcases/task/register-task.md#正常系 | タスクが pending で作成され、タスクIDが表示されて 0 で終了する |
| TC-task-register-task-002 | RegisterTask 正常系: 登録結果を確認する(名前指定で登録が成功する) | spec/testcases/task/register-task.md#正常系 | ワークフロー名は指定した名前になり、解決先パス(`workflows/implement.yaml` の絶対パス)が表示される |
| TC-task-register-task-003 | RegisterTask 正常系: ワークフロー名 `implement` を指定して登録する(`workflows/implement.yaml` に異なる値の `workflow:…) | spec/testcases/task/register-task.md#正常系 | タスクのワークフロー名は指定した `implement` になる(名前指定では `workflow:` キーは使われない) |
| TC-task-register-task-004 | RegisterTask 正常系: ファイルパス指定で登録する(YAML に `workflow: my-flow` キーを持つ定義ファイルがある) | spec/testcases/task/register-task.md#正常系 | タスクのワークフロー名は `workflow:` キーの値(`my-flow`)になる |
| TC-task-register-task-005 | RegisterTask 正常系: ファイルパス指定で登録する(YAML に `workflow:` キーのない定義ファイル `custom.yaml`…) | spec/testcases/task/register-task.md#正常系 | タスクのワークフロー名はファイル名から拡張子を除いた `custom` になる |
| TC-task-register-task-006 | RegisterTask 正常系: ベースブランチを省略して登録する(リポジトリの HEAD がブランチ `main` を指している) | spec/testcases/task/register-task.md#正常系 | ベースブランチが `main` に解決されてタスクが作成される |
| TC-task-register-task-007 | RegisterTask 正常系: `--base develop` を指定して登録する(リポジトリにブランチ `develop` が存在する) | spec/testcases/task/register-task.md#正常系 | 対象のベースブランチが `develop` として記録される |
| TC-task-register-task-008 | RegisterTask 正常系: 相対パスの `--repo` で登録する(カレントディレクトリからの相対パスでリポジトリを指定できる状態) | spec/testcases/task/register-task.md#正常系 | 絶対パスへ正規化された値が対象として記録される |
| TC-task-register-task-009 | RegisterTask 正常系: 作成直後のタスクを照会する(登録が成功する) | spec/testcases/task/register-task.md#正常系 | タスクステータスは snapshot の `initial`、実行状態は pending、カウンタは全0、workspace・attempt・失敗要因は未設定である |
| TC-task-register-task-010 | RegisterTask 正常系: 登録後の状態を観測する(登録が成功する) | spec/testcases/task/register-task.md#正常系 | 検証済みワークフロー定義のスナップショットがタスクファイルに埋め込まれ、以降の元YAML編集はこのタスクに影響しない(ADR-015) |
| TC-task-register-task-011 | RegisterTask 正常系: 登録直後のプロセス・実行状態を観測する(登録が成功する) | spec/testcases/task/register-task.md#正常系 | エージェント実行は開始されない(実行は次の tick に委ねられる。ADR-007) |
| TC-task-register-task-012 | RegisterTask 正常系: 登録する(ID発行が既存タスクと衝突する) | spec/testcases/task/register-task.md#正常系 | IDが再発行されて1回だけ再試行され、登録が成功して 0 で終了する |
| TC-task-register-task-013 | RegisterTask 正常系: 同じ指定でもう一度登録する(同一リポジトリ・同一ワークフローのタスクが既に登録済み) | spec/testcases/task/register-task.md#正常系 | 重複排除されず、独立した別IDのタスクとして登録される |
| TC-task-register-task-014 | RegisterTask 異常系: 登録する(config.yaml が存在しない) | spec/testcases/task/register-task.md#異常系 | 「グローバルホームが未初期化」である旨と解決後のホームパス・作成が必要であることを表示して非0で終了する |
| TC-task-register-task-015 | RegisterTask 異常系: 登録する(config.yaml がパース不能(構文エラー・未知キー)) | spec/testcases/task/register-task.md#異常系 | 構文エラー・重複キーは行・列を、スキーマ違反(未知キー・型不一致)は問題のキーのパスを表示して非0で終了する |
| TC-task-register-task-016 | RegisterTask 異常系: 登録する(config.yaml が存在するが読めない(権限不足等)) | spec/testcases/task/register-task.md#異常系 | 実行環境エラーとして非0で終了する |
| TC-task-register-task-017 | RegisterTask 異常系: 登録する(別の操作が排他ロックを保持している) | spec/testcases/task/register-task.md#異常系 | 「別の操作が実行中」として非0で終了する(登録前のためタスクは作られない) |
| TC-task-register-task-018 | RegisterTask 異常系: 登録する(ロック機構自体が異常(`LockError::Failed`)) | spec/testcases/task/register-task.md#異常系 | 実行環境エラーとして非0で終了する |
| TC-task-register-task-019 | RegisterTask 異常系: 名前指定で登録する(指定した名前に対応する `workflows/<name>.yaml` が存在しない) | spec/testcases/task/register-task.md#異常系 | 解決を試みた絶対パスを添えて非0で終了する |
| TC-task-register-task-020 | RegisterTask 異常系: パス指定で登録する(指定したファイルパスのワークフロー定義が存在しない) | spec/testcases/task/register-task.md#異常系 | 解決を試みたパスを添えて非0で終了する |
| TC-task-register-task-021 | RegisterTask 異常系: 登録する(ワークフロー定義ファイルが存在するが読めない(I/O障害)) | spec/testcases/task/register-task.md#異常系 | 非0で終了する |
| TC-task-register-task-022 | RegisterTask 異常系: 登録する(ワークフローYAMLが構文として不正(重複キー含む)) | spec/testcases/task/register-task.md#異常系 | 位置・原因に加えて解決先の絶対パスを表示して非0で終了する(`YamlSyntax`) |
| TC-task-register-task-023 | RegisterTask 異常系: 登録する(ワークフローYAMLにスキーマ外のキーがある) | spec/testcases/task/register-task.md#異常系 | 非0で終了する(`UnknownKey`。ADR-013) |
| TC-task-register-task-024 | RegisterTask 異常系: 登録する(`run: wait` のステータスに `judge` 等のエージェント実行用キーがある) | spec/testcases/task/register-task.md#異常系 | 非0で終了する(`ForbiddenKey`。ADR-013) |
| TC-task-register-task-025 | RegisterTask 異常系: 登録する(`initial` が欠落している) | spec/testcases/task/register-task.md#異常系 | 非0で終了する(`MissingInitial`) |
| TC-task-register-task-026 | RegisterTask 異常系: 登録する(`initial` の参照先ステータスが存在しない) | spec/testcases/task/register-task.md#異常系 | 非0で終了する(`InitialNotFound`) |
| TC-task-register-task-027 | RegisterTask 異常系: 登録する(`statuses` が空または欠落している) | spec/testcases/task/register-task.md#異常系 | 非0で終了する(`EmptyStatuses`) |
| TC-task-register-task-028 | RegisterTask 異常系: 登録する(動作宣言(`prompt` / `skill` / `run`)のないステータスがある) | spec/testcases/task/register-task.md#異常系 | 非0で終了する(`NoAction`) |
| TC-task-register-task-029 | RegisterTask 異常系: 登録する(1ステータスに `prompt` と `skill` など複数の動作宣言がある) | spec/testcases/task/register-task.md#異常系 | 非0で終了する(`MultipleActions`) |
| TC-task-register-task-030 | RegisterTask 異常系: 登録する(`run` の値が `cleanup` / `wait` 以外) | spec/testcases/task/register-task.md#異常系 | 非0で終了する(`UnknownRunValue`) |
| TC-task-register-task-031 | RegisterTask 異常系: 登録する(エージェント実行のステータスに `next` がない) | spec/testcases/task/register-task.md#異常系 | 非0で終了する(`MissingNext`) |
| TC-task-register-task-032 | RegisterTask 異常系: 登録する(`next` の参照先ステータスが存在しない) | spec/testcases/task/register-task.md#異常系 | 非0で終了する(`NextNotFound`) |
| TC-task-register-task-033 | RegisterTask 異常系: 登録する(ステータス名・期間・コマンドの値が不正(空文字のプロンプト、`timeout: 0s`…) | spec/testcases/task/register-task.md#異常系 | 非0で終了する(`InvalidValue`) |
| TC-task-register-task-034 | RegisterTask 異常系: パス指定で登録する(`workflow:` キーがなく、ファイル名由来の表示名が不正になるパス…) | spec/testcases/task/register-task.md#異常系 | 表示名の決定失敗として非0で終了する(例示は語幹が空白のみになる ` .yaml`。`Path::file_stem` は `.yaml` を語幹として返すため「拡張子を除くと空」は作れない) |
| TC-task-register-task-035 | RegisterTask 異常系: 登録する(指定したリポジトリパスが存在しない) | spec/testcases/task/register-task.md#異常系 | 非0で終了する |
| TC-task-register-task-036 | RegisterTask 異常系: 登録する(指定したパスが git リポジトリでない) | spec/testcases/task/register-task.md#異常系 | 非0で終了する |
| TC-task-register-task-037 | RegisterTask 異常系: 登録する(指定したベースブランチがリポジトリに存在しない) | spec/testcases/task/register-task.md#異常系 | 非0で終了する |
| TC-task-register-task-038 | RegisterTask 異常系: 登録する(リポジトリが detached HEAD で `--base` 省略) | spec/testcases/task/register-task.md#異常系 | `--base` の明示指定を案内して非0で終了する |
| TC-task-register-task-039 | RegisterTask 異常系: 登録する(リポジトリがコミットのない空リポジトリで `--base` 省略) | spec/testcases/task/register-task.md#異常系 | `--base` の明示指定を案内して非0で終了する |
| TC-task-register-task-040 | RegisterTask 異常系: 登録する(対象検証の git 操作自体が失敗する(`TargetError::Failed`)) | spec/testcases/task/register-task.md#異常系 | 実行環境エラーとして非0で終了する |
| TC-task-register-task-041 | RegisterTask 異常系: 登録する(ステータスにもワークフローにもエージェント指定がない) | spec/testcases/task/register-task.md#異常系 | 非0で終了する(`MissingAgent`) |
| TC-task-register-task-042 | RegisterTask 異常系: 登録する(実効エージェント名が config の `agents` に存在しない) | spec/testcases/task/register-task.md#異常系 | config.yaml に定義済みのエージェント名一覧を添えて非0で終了する(`UnknownAgent`) |
| TC-task-register-task-043 | RegisterTask 異常系: 登録する(参照エージェントの `cmd` に未知プレースホルダ・波括弧不正がある) | spec/testcases/task/register-task.md#異常系 | 非0で終了する(`InvalidAgentDefinition`) |
| TC-task-register-task-044 | RegisterTask 異常系: 登録する(`skill` 指定のステータスがあるがエージェント定義に `skill_input`…) | spec/testcases/task/register-task.md#異常系 | 非0で終了する(`MissingSkillInput`) |
| TC-task-register-task-045 | RegisterTask 異常系: 登録する(`cmd` が `{model}` を参照するがワークフロー・ステータスのどちらにも…) | spec/testcases/task/register-task.md#異常系 | 非0で終了する(`MissingModel`) |
| TC-task-register-task-046 | RegisterTask 異常系: 登録する(複数のステータスにまたがって登録時検証エラーが複数ある) | spec/testcases/task/register-task.md#異常系 | エラーが最初の1件で打ち切られず全件まとめて表示され、非0で終了する |
| TC-task-register-task-047 | RegisterTask 異常系: 登録する(ID再発行後もIDが衝突する) | spec/testcases/task/register-task.md#異常系 | 実行環境エラーとして非0で終了する(2回目の再試行はしない) |
| TC-task-register-task-048 | RegisterTask 異常系: 登録する(タスクファイルの作成が I/O エラーで失敗する) | spec/testcases/task/register-task.md#異常系 | 実行環境エラーとして非0で終了する |
| TC-task-register-task-049 | RegisterTask 境界値: 登録する(ワークフロー指定値がパス区切り文字を含む(`./flows/x`)) | spec/testcases/task/register-task.md#境界値 | ファイルパスとして解決される(ファイルの存在有無に依存しない決定的規則) |
| TC-task-register-task-050 | RegisterTask 境界値: 登録する(ワークフロー指定値が `.yaml` で終わる) | spec/testcases/task/register-task.md#境界値 | ファイルパスとして解決される |
| TC-task-register-task-051 | RegisterTask 境界値: 登録する(ワークフロー指定値が `.yml` で終わる) | spec/testcases/task/register-task.md#境界値 | ファイルパスとして解決される |
| TC-task-register-task-052 | RegisterTask 境界値: 登録する(ワークフロー指定値がパス区切りも拡張子も含まない) | spec/testcases/task/register-task.md#境界値 | 名前として `workflows/<name>.yaml` に解決される(`.yml` へのフォールバックはしない) |
| TC-task-register-task-053 | RegisterTask 境界値: 登録する(ワークフロー指定値が空文字列) | spec/testcases/task/register-task.md#境界値 | 入力境界の検証エラー(`NameError::Empty`)として非0で終了する。タスクは作られない |
| TC-task-register-task-054 | RegisterTask 境界値: 登録する(`--base` に空文字列を指定する) | spec/testcases/task/register-task.md#境界値 | ブランチ名の検証エラーとして非0で終了する |
| TC-task-register-task-055 | RegisterTask 境界値: 登録する(`--base` の先頭が `-`、`..` を含む、`/` 始まり・終わり…) | spec/testcases/task/register-task.md#境界値 | ブランチ名の検証エラーとして非0で終了する |
| TC-task-register-task-056 | RegisterTask 境界値: 登録する(ステータスに `retries: 0` が指定されている) | spec/testcases/task/register-task.md#境界値 | 受理されて登録が成功する(0 = 初回失敗で即 stopped は実行時の意味) |
| TC-task-register-task-057 | RegisterTask 境界値: 登録する(ステータスに `timeout: none` が指定されている) | spec/testcases/task/register-task.md#境界値 | 無制限として受理され登録が成功する |
| TC-task-register-task-058 | RegisterTask 境界値: 登録する(ステータスの `timeout` が `0s`・単位なし・負値のいずれか) | spec/testcases/task/register-task.md#境界値 | 期間の検証エラーとして非0で終了する |
| TC-task-register-task-059 | RegisterTask 境界値: 登録する(statuses が1件だけの定義) | spec/testcases/task/register-task.md#境界値 | 受理されて登録が成功する |
| TC-task-register-task-060 | RegisterTask エッジケース: 登録する(`state/` 配下のディレクトリ(tasks 等)が存在しない) | spec/testcases/task/register-task.md#エッジケース | 必要なディレクトリが自動作成され、登録が成功する(pages ※3) |
| TC-task-register-task-061 | RegisterTask エッジケース: 登録する(`next` に自己参照・循環があるワークフロー定義) | spec/testcases/task/register-task.md#エッジケース | 正当な表現として受理され登録が成功する(ADR-010) |
| TC-task-register-task-062 | RegisterTask エッジケース: 登録する(遷移経路のない到達不能ステータス(set-status 用のクリーンアップ等)を含む定義) | spec/testcases/task/register-task.md#エッジケース | 受理され登録が成功する |
| TC-task-register-task-063 | RegisterTask エッジケース: 登録する(クリーンアップ動作のステータス(終端)を持たない定義) | spec/testcases/task/register-task.md#エッジケース | 受理され登録が成功する(循環の出口は人間の操作・stopped) |
| TC-task-register-task-064 | RegisterTask エッジケース: 登録する(config.yaml にテンプレートが壊れたエージェント定義があるが…) | spec/testcases/task/register-task.md#エッジケース | 参照されないエージェント定義は検証されず、登録が成功する(内容の検証は参照時) |
| TC-task-register-task-065 | RegisterTask エッジケース: 登録する(エージェント定義に値の供給されるプレースホルダ(`{model}` 等)への参照がなく…) | spec/testcases/task/register-task.md#エッジケース | 許容されて登録が成功する(値は単に渡らない) |
| TC-task-register-task-066 | RegisterTask エッジケース: 登録する(ステータスの `judge` コマンドのトークンに `{...}` 形の文字列が含まれる) | spec/testcases/task/register-task.md#エッジケース | プレースホルダとして検査されず文字どおり受理される(判定コマンドに展開は行われない) |
| TC-task-register-task-067 | RegisterTask エッジケース: 登録する(`PULSEN_HOME` と `--home` フラグの両方が指定されている) | spec/testcases/task/register-task.md#エッジケース | フラグのホームが優先して解決される(フラグ > 環境変数 > 既定) |
| TC-task-retry-task-001 | RetryTask 正常系: retry する(リトライ上限超過(`RetryLimitExceeded`)で stopped…) | spec/testcases/task/retry-task.md#正常系 | attempt_count・judge_attempt_count・spawn_fail_count が全て 0 にリセットされ、実行状態が pending に戻り 0 で終了する |
| TC-task-retry-task-002 | RetryTask 正常系: retry する(判定不能の上限超過(`JudgeLimitExceeded`)で stopped…) | spec/testcases/task/retry-task.md#正常系 | 受理されて pending に戻る |
| TC-task-retry-task-003 | RetryTask 正常系: retry する(連続spawn失敗の上限超過(`SpawnFailLimitExceeded`)で…) | spec/testcases/task/retry-task.md#正常系 | 受理されて pending に戻る |
| TC-task-retry-task-004 | RetryTask 正常系: retry する(人間の abort(`Aborted`)で stopped のタスクがある) | spec/testcases/task/retry-task.md#正常系 | 受理されて pending に戻る |
| TC-task-retry-task-005 | RetryTask 正常系: retry 後の状態を照会する(stopped のタスクを retry する) | spec/testcases/task/retry-task.md#正常系 | タスクステータスは変更されず、再開されるタスクステータスが結果に表示される |
| TC-task-retry-task-006 | RetryTask 正常系: retry 後の状態を照会する(実行履歴のある stopped タスクを retry する) | spec/testcases/task/retry-task.md#正常系 | 現在attempt参照(attempt番号・runディレクトリパス)と workspace は保持されたまま pending になる |
| TC-task-retry-task-007 | RetryTask 異常系: retry する(config.yaml が存在しない・パース不能・読めないのいずれか) | spec/testcases/task/retry-task.md#異常系 | 非0で終了する(状態は変更しない。pages ※1) |
| TC-task-retry-task-008 | RetryTask 異常系: retry する(別の操作が排他ロックを保持している) | spec/testcases/task/retry-task.md#異常系 | 「別の操作が実行中」として非0で終了する |
| TC-task-retry-task-009 | RetryTask 異常系: retry する(ロック機構自体が異常(`LockError::Failed`)) | spec/testcases/task/retry-task.md#異常系 | 実行環境エラーとして非0で終了する |
| TC-task-retry-task-010 | RetryTask 異常系: retry する(指定IDのタスクが現役にもアーカイブにも存在しない) | spec/testcases/task/retry-task.md#異常系 | タスク不在として非0で終了する |
| TC-task-retry-task-011 | RetryTask 異常系: retry する(指定IDのタスクがアーカイブ済み) | spec/testcases/task/retry-task.md#異常系 | アーカイブ済みは操作不可として非0で終了する |
| TC-task-retry-task-012 | RetryTask 異常系: retry する(指定IDのタスクファイルがパース不能(`Corrupt`)) | spec/testcases/task/retry-task.md#異常系 | 非0で終了し、破損ファイルへの書き込みは行わない |
| TC-task-retry-task-013 | RetryTask 異常系: retry する(タスクが failed) | spec/testcases/task/retry-task.md#異常系 | 拒否され「放置すれば自動リトライされる」と案内して非0で終了する |
| TC-task-retry-task-014 | RetryTask 異常系: retry する(タスクが pending) | spec/testcases/task/retry-task.md#異常系 | 拒否され「既に実行待ち」と案内して非0で終了する |
| TC-task-retry-task-015 | RetryTask 異常系: retry する(タスクが completed) | spec/testcases/task/retry-task.md#異常系 | 拒否され「判定済み。次のtickが遷移させる」と案内して非0で終了する |
| TC-task-retry-task-016 | RetryTask 異常系: retry する(タスクが launching) | spec/testcases/task/retry-task.md#異常系 | 拒否され「先に abort」と案内して非0で終了する |
| TC-task-retry-task-017 | RetryTask 異常系: retry する(タスクが running) | spec/testcases/task/retry-task.md#異常系 | 拒否され「先に abort」と案内して非0で終了する |
| TC-task-retry-task-018 | RetryTask 異常系: retry する(タスクIDとして不正な文字列を指定する) | spec/testcases/task/retry-task.md#異常系 | 入力境界の検証エラーとして非0で終了する |
| TC-task-retry-task-019 | RetryTask 境界値: retry する(タスクIDに空文字列を指定する) | spec/testcases/task/retry-task.md#境界値 | 検証エラー(`Empty`)として非0で終了する |
| TC-task-retry-task-020 | RetryTask 境界値: retry する(タスクIDに65文字の文字列を指定する) | spec/testcases/task/retry-task.md#境界値 | 検証エラー(`TooLong`)として非0で終了する |
| TC-task-retry-task-021 | RetryTask 境界値: retry する(64文字の有効なIDを持つ stopped タスクがある) | spec/testcases/task/retry-task.md#境界値 | 受理されて pending に戻る |
| TC-task-retry-task-022 | RetryTask 境界値: retry する(1文字の有効なID(英数字)を持つ stopped タスクがある) | spec/testcases/task/retry-task.md#境界値 | 受理されて pending に戻る |
| TC-task-retry-task-023 | RetryTask 境界値: retry する(タスクIDに `[a-z0-9-]` 以外の文字(大文字・`_`…) | spec/testcases/task/retry-task.md#境界値 | 検証エラー(`InvalidChar`)として非0で終了する |
| TC-task-retry-task-024 | RetryTask 境界値: retry する(タスクIDの先頭が `-` の文字列を指定する) | spec/testcases/task/retry-task.md#境界値 | 検証エラー(`InvalidLeadingChar`)として非0で終了する |
| TC-task-retry-task-025 | RetryTask エッジケース: retry する(スナップショットのみ読めない stopped タスク(`SnapshotUnreadabl…) | spec/testcases/task/retry-task.md#エッジケース | 受理されて pending に戻り 0 で終了するが、tick に拾われないためスナップショット修復が必要である旨の警告が表示される(pages ※7) |
| TC-task-retry-task-026 | RetryTask エッジケース: 保存後のタスクファイルを確認する(DegradedTask を retry する) | spec/testcases/task/retry-task.md#エッジケース | 読めないスナップショットフィールドは元の内容のまま温存される(修復材料を消さない) |
| TC-task-retry-task-027 | RetryTask エッジケース: retry する(`state/tasks/`・`state/archive/` ディレクトリが存在しない) | spec/testcases/task/retry-task.md#エッジケース | タスク不在として非0で終了する(pages 縮退表) |
| TC-task-retry-task-028 | RetryTask エッジケース: retry する(未通知(`notified_at` なし)の stopped タスクがある) | spec/testcases/task/retry-task.md#エッジケース | 通知の有無によらず受理される(人間が操作した = 気づいている。flows F3) |
| TC-task-retry-task-029 | RetryTask エッジケース: 同じタスクをもう一度 retry する(stopped タスクを retry 済みで pending になっている) | spec/testcases/task/retry-task.md#エッジケース | stopped ではないため「既に実行待ち」と案内して非0で終了する |
| TC-task-set-task-status-001 | SetTaskStatus 正常系: 遷移先を指定して set-status する(stopped のタスクと、スナップショットに定義済みの遷移先ステータスがある) | spec/testcases/task/set-task-status.md#正常系 | タスクステータスが遷移し、カウンタが全0にリセットされ、実行状態が pending になり(凍結が解ける)、0 で終了する |
| TC-task-set-task-status-002 | SetTaskStatus 正常系: 定義済みステータスへ set-status する(pending のタスク(「何もしない」ステータス滞留中)がある) | spec/testcases/task/set-task-status.md#正常系 | 受理されて遷移し、pending のまま 0 で終了する(abort を挟む必要はない) |
| TC-task-set-task-status-003 | SetTaskStatus 正常系: 定義済みステータスへ set-status する(failed のタスクがある) | spec/testcases/task/set-task-status.md#正常系 | 受理されて遷移し、カウンタ全0・pending になる |
| TC-task-set-task-status-004 | SetTaskStatus 正常系: 定義済みステータスへ set-status する(completed のタスクがある) | spec/testcases/task/set-task-status.md#正常系 | 受理されて遷移し、次ステータスへの自動遷移は行われなくなる(手動遷移が上書きする) |
| TC-task-set-task-status-005 | SetTaskStatus 正常系: set-status する(遷移先がエージェント実行のステータス) | spec/testcases/task/set-task-status.md#正常系 | 受理される(次のtickの動作は遷移先の定義に従う) |
| TC-task-set-task-status-006 | SetTaskStatus 正常系: set-status する(遷移先が「何もしない」(wait)ステータス) | spec/testcases/task/set-task-status.md#正常系 | 受理され、動作種別によらず一律にカウンタリセット・pending になる |
| TC-task-set-task-status-007 | SetTaskStatus 正常系: set-status する(遷移先がクリーンアップステータス) | spec/testcases/task/set-task-status.md#正常系 | 受理され、一律にカウンタリセット・pending になる(終端処理は次のtick) |
| TC-task-set-task-status-008 | SetTaskStatus 正常系: 結果を確認する(set-status が成功する) | spec/testcases/task/set-task-status.md#正常系 | 遷移元(`from`)と遷移先(`to`)のステータス名が表示される |
| TC-task-set-task-status-009 | SetTaskStatus 異常系: set-status する(config.yaml が存在しない・パース不能・読めないのいずれか) | spec/testcases/task/set-task-status.md#異常系 | 非0で終了する(状態は変更しない。pages ※1) |
| TC-task-set-task-status-010 | SetTaskStatus 異常系: set-status する(別の操作が排他ロックを保持している) | spec/testcases/task/set-task-status.md#異常系 | 「別の操作が実行中」として非0で終了する |
| TC-task-set-task-status-011 | SetTaskStatus 異常系: set-status する(ロック機構自体が異常(`LockError::Failed`)) | spec/testcases/task/set-task-status.md#異常系 | 実行環境エラーとして非0で終了する |
| TC-task-set-task-status-012 | SetTaskStatus 異常系: set-status する(指定IDのタスクが現役にもアーカイブにも存在しない) | spec/testcases/task/set-task-status.md#異常系 | タスク不在として非0で終了する |
| TC-task-set-task-status-013 | SetTaskStatus 異常系: set-status する(指定IDのタスクがアーカイブ済み) | spec/testcases/task/set-task-status.md#異常系 | アーカイブ済みは操作不可として非0で終了する |
| TC-task-set-task-status-014 | SetTaskStatus 異常系: set-status する(指定IDのタスクファイルがパース不能(`Corrupt`)) | spec/testcases/task/set-task-status.md#異常系 | 非0で終了し、破損ファイルへの書き込みは行わない |
| TC-task-set-task-status-015 | SetTaskStatus 異常系: set-status する(スナップショットのみ読めないタスク(`SnapshotUnreadable`)がある) | spec/testcases/task/set-task-status.md#異常系 | 拒否して非0で終了する(遷移先の検証にスナップショットが必要。pages ※7) |
| TC-task-set-task-status-016 | SetTaskStatus 異常系: set-status する(タスクが launching) | spec/testcases/task/set-task-status.md#異常系 | 拒否され「先に abort せよ」と案内して非0で終了する(`Active`) |
| TC-task-set-task-status-017 | SetTaskStatus 異常系: set-status する(タスクが running) | spec/testcases/task/set-task-status.md#異常系 | 拒否され「先に abort せよ」と案内して非0で終了する(`Active`) |
| TC-task-set-task-status-018 | SetTaskStatus 異常系: set-status する(遷移先がスナップショット定義に存在しないステータス名) | spec/testcases/task/set-task-status.md#異常系 | 定義済みステータスの一覧を添えて非0で終了する(`UnknownStatus`) |
| TC-task-set-task-status-019 | SetTaskStatus 異常系: set-status する(タスクIDとして不正な文字列を指定する) | spec/testcases/task/set-task-status.md#異常系 | 入力境界の検証エラーとして非0で終了する |
| TC-task-set-task-status-020 | SetTaskStatus 異常系: set-status する(ステータス名として不正な文字列を指定する) | spec/testcases/task/set-task-status.md#異常系 | 入力境界の検証エラーとして非0で終了する |
| TC-task-set-task-status-021 | SetTaskStatus 境界値: set-status する(タスクIDに空文字列を指定する) | spec/testcases/task/set-task-status.md#境界値 | 検証エラー(`Empty`)として非0で終了する |
| TC-task-set-task-status-022 | SetTaskStatus 境界値: set-status する(タスクIDに65文字の文字列を指定する) | spec/testcases/task/set-task-status.md#境界値 | 検証エラー(`TooLong`)として非0で終了する |
| TC-task-set-task-status-023 | SetTaskStatus 境界値: set-status する(タスクIDに `[a-z0-9-]` 以外の文字を含む・先頭が `-` の文字列を指定する) | spec/testcases/task/set-task-status.md#境界値 | 検証エラー(`InvalidChar` / `InvalidLeadingChar`)として非0で終了する |
| TC-task-set-task-status-024 | SetTaskStatus 境界値: set-status する(ステータス名に空文字列を指定する) | spec/testcases/task/set-task-status.md#境界値 | 検証エラー(`Empty`)として非0で終了する |
| TC-task-set-task-status-025 | SetTaskStatus 境界値: set-status する(ステータス名の前後に空白がある) | spec/testcases/task/set-task-status.md#境界値 | 検証エラー(`SurroundingWhitespace`)として非0で終了する |
| TC-task-set-task-status-026 | SetTaskStatus エッジケース: set-status する(`state/tasks/`・`state/archive/` ディレクトリが存在しない) | spec/testcases/task/set-task-status.md#エッジケース | タスク不在として非0で終了する(pages 縮退表) |
| TC-task-set-task-status-027 | SetTaskStatus エッジケース: set-status する(登録後に元YAMLへ追加されたステータス名を指定する) | spec/testcases/task/set-task-status.md#エッジケース | スナップショットが検証の基準のため `UnknownStatus` として拒否される(定義更新は既存タスクに適用されない) |
| TC-task-set-task-status-028 | SetTaskStatus エッジケース: set-status する(現在と同じステータス名を指定する) | spec/testcases/task/set-task-status.md#エッジケース | 遷移経路の制約はないため受理され、カウンタリセット・pending になる |
| TC-task-set-task-status-029 | SetTaskStatus エッジケース: 続けて同じタスクを retry する(stopped のタスクを set-status で動かした) | spec/testcases/task/set-task-status.md#エッジケース | 既に stopped ではないため retry は拒否される |
| TC-task-set-task-status-030 | SetTaskStatus エッジケース: set-status する(到達不能ステータス(遷移経路のないクリーンアップ等)を遷移先に指定する) | spec/testcases/task/set-task-status.md#エッジケース | スナップショットに定義済みであれば受理される |
| TC-task-set-task-status-031 | SetTaskStatus エッジケース: set-status する(kill 対象の残る可能性のあるタスク(abort 済みで孤児プロセス残存疑い)) | spec/testcases/task/set-task-status.md#エッジケース | プロセス操作・worktree操作は一切行われず、タスクステータスの遷移のみが行われる |
| TC-task-show-task-001 | ShowTask 正常系: 詳細を表示する(登録直後で一度も実行されていない現役タスクがある) | spec/testcases/task/show-task.md#正常系 | ワークフロー名・対象・タスクステータス・実行状態・カウンタ・更新日時が表示され、workspace は「未作成」、attempt は「なし」として 0 で終了する |
| TC-task-show-task-002 | ShowTask 正常系: 詳細を表示する(実行履歴のある現役タスクがある) | spec/testcases/task/show-task.md#正常系 | 現在attemptの番号・runディレクトリパス・stdout / stderr ログのパスが表示される(ログ確認の起点) |
| TC-task-show-task-003 | ShowTask 正常系: 詳細を表示する(running のタスクがある) | spec/testcases/task/show-task.md#正常系 | PID・kill同定子・starttime が表示され、どのattemptが動いているかが特定できる |
| TC-task-show-task-004 | ShowTask 正常系: 詳細を表示する(launching で同定情報が未取り込みのタスクがある) | spec/testcases/task/show-task.md#正常系 | PID・starttime 等の該当項目が「未取得」として表示され、エラーにしない |
| TC-task-show-task-005 | ShowTask 正常系: 詳細を表示する(現在attemptの exit ファイルが存在する) | spec/testcases/task/show-task.md#正常系 | exit の値が読み取られて表示される |
| TC-task-show-task-006 | ShowTask 正常系: 詳細を表示する(ツール操作の失敗(worktree 作成失敗等)で凍結した stopped タスクがある) | spec/testcases/task/show-task.md#正常系 | 凍結要因(`StopReason`)と notified_at、直近の失敗要因(`last_failure` の FailureNote)が表示される |
| TC-task-show-task-007 | ShowTask 正常系: 詳細を表示する(エージェント実行の失敗で凍結した stopped タスクがある) | spec/testcases/task/show-task.md#正常系 | 凍結要因(`StopReason`)と notified_at、直前実行の exit・runディレクトリ(ログ)への参照が表示される(FailureNote はエージェント実行自体の失敗を記録しないため、`last_failure` の表示は要求しない) |
| TC-task-show-task-008 | ShowTask 正常系: 詳細を表示する(未通知(`notified_at` なし)の stopped タスクがある) | spec/testcases/task/show-task.md#正常系 | notified_at が未記録であることが確認できる(at-least-once の検証に使える) |
| TC-task-show-task-009 | ShowTask 正常系: 詳細を表示する(スナップショットが読めるタスクがある) | spec/testcases/task/show-task.md#正常系 | スナップショットの定義済みステータス一覧が表示される(set-status の遷移先確認に使う) |
| TC-task-show-task-010 | ShowTask 正常系: 詳細を表示する(タスクがある) | spec/testcases/task/show-task.md#正常系 | スナップショット保存先としてタスクファイル自体のパス(`state/tasks/<task-id>.json`)が表示される(ADR-015) |
| TC-task-show-task-011 | ShowTask 正常系: 詳細を表示する(`retries` 上書きのあるエージェント実行ステータスのタスクがある) | spec/testcases/task/show-task.md#正常系 | attempt_count に上書き値のリトライ上限が併記される |
| TC-task-show-task-012 | ShowTask 正常系: 詳細を表示する(`retries` 上書きのないエージェント実行ステータスのタスクがある) | spec/testcases/task/show-task.md#正常系 | 組み込みデフォルト 2 がリトライ上限として併記される |
| TC-task-show-task-013 | ShowTask 正常系: 詳細を表示する(クリーンアップステータスのタスクがある) | spec/testcases/task/show-task.md#正常系 | リトライ上限は常に 2 として併記される(ADR-014) |
| TC-task-show-task-014 | ShowTask 正常系: 詳細を表示する(「何もしない」ステータスのタスクがある) | spec/testcases/task/show-task.md#正常系 | リトライ上限は併記されない(`NotApplicable`。適用対象がない) |
| TC-task-show-task-015 | ShowTask 正常系: 詳細を表示する(config に `judge_attempt_limit` /…) | spec/testcases/task/show-task.md#正常系 | judge・spawn の上限は動作種別・スナップショットによらず常に config の値で表示される |
| TC-task-show-task-016 | ShowTask 正常系: 詳細を表示する(アーカイブ済みタスクのIDを指定する) | spec/testcases/task/show-task.md#正常系 | 現役 → アーカイブの順で解決され、アーカイブ済みであること・worktree は削除済みであることを明示して 0 で終了する(pages ※4) |
| TC-task-show-task-017 | ShowTask 正常系: 保存先パスを確認する(アーカイブ済みタスクを表示する) | spec/testcases/task/show-task.md#正常系 | `state/archive/<task-id>.json` が表示される |
| TC-task-show-task-018 | ShowTask 異常系: 詳細を表示する(config.yaml が存在しない・パース不能・読めないのいずれか) | spec/testcases/task/show-task.md#異常系 | 非0で終了する(pages ※1) |
| TC-task-show-task-019 | ShowTask 異常系: 詳細を表示する(指定IDのタスクが現役にもアーカイブにも存在しない) | spec/testcases/task/show-task.md#異常系 | タスク不在として非0で終了する(無言で空を返さない) |
| TC-task-show-task-020 | ShowTask 異常系: 詳細を表示する(指定IDのタスクファイルがパース不能(`Corrupt`)) | spec/testcases/task/show-task.md#異常系 | パースエラーの内容とファイルパスを表示して非0で終了する(直接修復の導線) |
| TC-task-show-task-021 | ShowTask 異常系: 詳細を表示する(現在attemptの exit ファイルの読み取りが I/O エラー・内容不正で失敗する) | spec/testcases/task/show-task.md#異常系 | 当該項目を読めない旨の注記付きで表示は継続し、0 で終了する |
| TC-task-show-task-022 | ShowTask 異常系: 詳細を表示する(runディレクトリの存在確認が I/O エラーで失敗する) | spec/testcases/task/show-task.md#異常系 | 当該項目を読めない旨の注記付きで表示は継続し、0 で終了する |
| TC-task-show-task-023 | ShowTask 異常系: 詳細を表示する(タスクIDとして不正な文字列を指定する) | spec/testcases/task/show-task.md#異常系 | 入力境界の検証エラーとして非0で終了する |
| TC-task-show-task-024 | ShowTask 境界値: 詳細を表示する(タスクIDに空文字列を指定する) | spec/testcases/task/show-task.md#境界値 | 検証エラー(`Empty`)として非0で終了する |
| TC-task-show-task-025 | ShowTask 境界値: 詳細を表示する(タスクIDに65文字の文字列を指定する) | spec/testcases/task/show-task.md#境界値 | 検証エラー(`TooLong`)として非0で終了する |
| TC-task-show-task-026 | ShowTask 境界値: 詳細を表示する(64文字・1文字の有効なIDを持つタスクがある) | spec/testcases/task/show-task.md#境界値 | 受理されて詳細が表示される |
| TC-task-show-task-027 | ShowTask 境界値: 詳細を表示する(タスクIDに `[a-z0-9-]` 以外の文字を含む・先頭が `-` の文字列を指定する) | spec/testcases/task/show-task.md#境界値 | 検証エラー(`InvalidChar` / `InvalidLeadingChar`)として非0で終了する |
| TC-task-show-task-028 | ShowTask エッジケース: 詳細を表示する(スナップショットのみ読めないタスク(`SnapshotUnreadable` /…) | spec/testcases/task/show-task.md#エッジケース | 読める項目はすべて表示し、スナップショットが読めない理由を注記し、定義済みステータス一覧は表示せず、0 で終了する(pages ※6) |
| TC-task-show-task-029 | ShowTask エッジケース: リトライ上限の併記を確認する(DegradedTask を表示する) | spec/testcases/task/show-task.md#エッジケース | `Unknown`(スナップショット破損で導出不能)として表示され、Wait の「併記なし」(`NotApplicable`)と区別できる |
| TC-task-show-task-030 | ShowTask エッジケース: judge・spawn の上限を確認する(DegradedTask を表示する) | spec/testcases/task/show-task.md#エッジケース | スナップショット非依存のため config の値で通常どおり表示される |
| TC-task-show-task-031 | ShowTask エッジケース: 詳細を表示する(現在attemptのrunディレクトリが gc で削除済み) | spec/testcases/task/show-task.md#エッジケース | runディレクトリが「存在しない」ことを明示して 0 で終了する(エラーにしない) |
| TC-task-show-task-032 | ShowTask エッジケース: 詳細を表示する(launching 記録直後のクラッシュ等で現在attemptのrunディレクトリが未作成) | spec/testcases/task/show-task.md#エッジケース | 「存在しない」表示で 0 で終了する |
| TC-task-show-task-033 | ShowTask エッジケース: 詳細を表示する(worktree が手動削除された現役タスクがある) | spec/testcases/task/show-task.md#エッジケース | workspace_path をそのまま表示し、存在検証は行わない(pages ※9) |
| TC-task-show-task-034 | ShowTask エッジケース: 詳細を表示する(`state/tasks/`・`state/archive/` ディレクトリが存在しない) | spec/testcases/task/show-task.md#エッジケース | タスク不在として非0で終了する(pages 縮退表) |
| TC-task-show-task-035 | ShowTask エッジケース: 詳細を表示する(別の操作が排他ロックを保持している) | spec/testcases/task/show-task.md#エッジケース | ロックを取得しないため通常どおり表示され 0 で終了する |
| TC-task-show-task-036 | ShowTask エッジケース: 詳細を表示する(tick による更新と同時に読み取る) | spec/testcases/task/show-task.md#エッジケース | 更新はアトミック置換のため、書きかけの内容が観測されることはない |
| TC-task-show-task-037 | ShowTask エッジケース: 詳細を表示する(spawn失敗で pending へ戻った痕跡(無効化マーカーのみのattempt)を持つ…) | spec/testcases/task/show-task.md#エッジケース | タスクファイルが指す現在attemptのパスが表示され、過去の残骸と取り違えない起点になる |
| TC-task-show-task-038 | ShowTask エッジケース: 詳細を表示する(登録後の config.yaml 編集による展開失敗の連続で凍結したタスク…) | spec/testcases/task/show-task.md#エッジケース | 凍結要因 `SpawnFailLimitExceeded`・直近の失敗要因(`SpawnFail`。展開エラーの内容)が表示され、attempt は「なし」(採番されない)。runディレクトリに痕跡が残る猶予経路と判別できる(ADR-016) |
| TC-exec-abort-task-001 | AbortTask 正常系: abort を実行する(running のタスク・プロセス生存(starttime 照合一致)) | spec/testcases/execution/abort-task.md#正常系 | 照合付きでプロセスグループ相当が kill され、`abort` で `Stopped { Aborted, notified_at: None }` が保存され、通知が実行される。killed = true で 0 終了 |
| TC-exec-abort-task-002 | AbortTask 正常系: abort を実行する(running のタスク・プロセス死亡(starttime 取得不能)) | spec/testcases/execution/abort-task.md#正常系 | kill せずに進み、stopped の記録と通知のみ行う。killed = false で 0 終了 |
| TC-exec-abort-task-003 | AbortTask 正常系: abort を実行する(launching のタスク・pid ファイルあり(starttime あり)・照合一致) | spec/testcases/execution/abort-task.md#正常系 | runディレクトリの pid ファイルの同定情報で照合付き kill を行い、stopped を記録する |
| TC-exec-abort-task-004 | AbortTask 正常系: abort を実行する(launching のタスク・pid ファイルなし) | spec/testcases/execution/abort-task.md#正常系 | 無効化マーカーを書き、pid を再確認する。なお存在しなければ kill せず stopped を記録する(遅延起動したラッパーは pid 書き込み後のマーカー確認で終了するため、凍結後にエージェントが走り出さない) |
| TC-exec-abort-task-005 | AbortTask 正常系: abort を実行する(launching のタスク・マーカー書き込み後の再確認で pid が現れていた) | spec/testcases/execution/abort-task.md#正常系 | 照合付き kill を行って stopped を記録する |
| TC-exec-abort-task-006 | AbortTask 正常系: abort を実行する(launching のタスク・runディレクトリ自体が不在) | spec/testcases/execution/abort-task.md#正常系 | `write_invalidation_marker` がディレクトリごと作成してマーカーを書き、pid 再確認を経て stopped の記録のみを行う(マーカープロトコルを維持) |
| TC-exec-abort-task-007 | AbortTask 正常系: abort を実行する(pending のタスク) | spec/testcases/execution/abort-task.md#正常系 | プロセス操作なしで stopped を記録し、通知する(次のtickによる起動が止まる) |
| TC-exec-abort-task-008 | AbortTask 正常系: abort を実行する(failed のタスク) | spec/testcases/execution/abort-task.md#正常系 | プロセス操作なしで stopped を記録する(以降の自動リトライが止まる) |
| TC-exec-abort-task-009 | AbortTask 正常系: abort を実行する(completed のタスク) | spec/testcases/execution/abort-task.md#正常系 | プロセス操作なしで stopped を記録する(次ステータスへの遷移が打ち切られる)。`current_attempt.process` が Some でも観測しない |
| TC-exec-abort-task-010 | AbortTask 正常系: abort を実行する(すでに stopped のタスク) | spec/testcases/execution/abort-task.md#正常系 | 何も変更せずその旨を表示して 0(冪等成功。already_stopped = true) |
| TC-exec-abort-task-011 | AbortTask 正常系: abort を実行する(`SnapshotUnreadable`(DegradedTask)のタスク) | spec/testcases/execution/abort-task.md#正常系 | 通常どおり進む(abort はスナップショット非依存)。`degraded.abort` → `save_degraded` で stopped が記録される |
| TC-exec-abort-task-012 | AbortTask 正常系: abort を実行する(stopped 記録後の通知が成功する) | spec/testcases/execution/abort-task.md#正常系 | notified_at が記録され、kill の有無を含む結果が表示されて 0 終了 |
| TC-exec-abort-task-013 | AbortTask 正常系: abort を実行する(notify_cmd 未定義) | spec/testcases/execution/abort-task.md#正常系 | stopped の記録のみ行い、通知せず notified_at も書かない。0 終了 |
| TC-exec-abort-task-014 | AbortTask 異常系: abort を実行する(task_id が `TaskId::parse` を通らない(不正な形式)) | spec/testcases/execution/abort-task.md#異常系 | 非0 で終了する。何も変更しない |
| TC-exec-abort-task-015 | AbortTask 異常系: abort を実行する(config.yaml が存在しない・パース不能・読めない(Io)のいずれか) | spec/testcases/execution/abort-task.md#異常系 | 非0 で終了する(共通事項の `ConfigStore::load` 失敗)。何も変更しない |
| TC-exec-abort-task-016 | AbortTask 異常系: abort を実行する(別の操作が排他ロックを保持している) | spec/testcases/execution/abort-task.md#異常系 | 非0 で終了する。何も変更しない |
| TC-exec-abort-task-017 | AbortTask 異常系: abort を実行する(ロック機構自体が異常(`LockError::Failed`)) | spec/testcases/execution/abort-task.md#異常系 | 実行環境エラーとして非0 で終了する。何も変更しない |
| TC-exec-abort-task-018 | AbortTask 異常系: abort を実行する(タスクが存在しない(`NotFound`)) | spec/testcases/execution/abort-task.md#異常系 | 非0 で終了する。何も変更しない |
| TC-exec-abort-task-019 | AbortTask 異常系: abort を実行する(アーカイブ済みのタスク) | spec/testcases/execution/abort-task.md#異常系 | 非0 で終了する(操作不可)。書き込まない |
| TC-exec-abort-task-020 | AbortTask 異常系: abort を実行する(タスクファイルがパース不能(`Corrupt`)) | spec/testcases/execution/abort-task.md#異常系 | 非0 で終了する。破損ファイルへは書き込まない |
| TC-exec-abort-task-021 | AbortTask 異常系: abort を実行する(running のタスク・照合一致後の kill が失敗する(`KillError`)) | spec/testcases/execution/abort-task.md#異常系 | 状態を変更せず(stopped を記録せず)非0 で終了し、再実行を案内する(プロセスが生きたまま凍結扱いになることを防ぐ) |
| TC-exec-abort-task-022 | AbortTask 異常系: abort を実行する(running のタスク・`starttime_of` が `Err(Io)`…) | spec/testcases/execution/abort-task.md#異常系 | 状態を変更せず非0 で終了し、再実行を案内する |
| TC-exec-abort-task-023 | AbortTask 異常系: abort を実行する(`save` / `save_degraded` が失敗する) | spec/testcases/execution/abort-task.md#異常系 | 非0 で終了し、再実行を案内する |
| TC-exec-abort-task-024 | AbortTask 異常系: abort を実行する(running なのに `current_attempt` / `process` が…) | spec/testcases/execution/abort-task.md#異常系 | 状態を変更せず非0 で終了し、タスクファイルの修復を案内する(照合できない対象を kill せず、kill 対象が残り得るまま stopped も記録しない) |
| TC-exec-abort-task-025 | AbortTask 異常系: abort を実行する(launching のタスク・runファイルが破損(`RunFileError::Corr…) | spec/testcases/execution/abort-task.md#異常系 | 状態を変更せず非0 で終了し、破損した runファイルの削除による復旧を案内する |
| TC-exec-abort-task-026 | AbortTask 異常系: abort を実行する(launching のタスク・pid あり・starttime なし(照合材料が揃わない)) | spec/testcases/execution/abort-task.md#異常系 | 状態を変更せず非0 で終了する(照合なしの kill も、マーカーなしの stopped 化も行わない) |
| TC-exec-abort-task-027 | AbortTask 異常系: abort を実行する(launching のタスク・`write_invalidation_marker`…) | spec/testcases/execution/abort-task.md#異常系 | 状態を変更せず非0 で終了し、再実行を案内する |
| TC-exec-abort-task-028 | AbortTask 異常系: abort を実行する(stopped の記録後、notify_cmd の実行が失敗する(非0 /…) | spec/testcases/execution/abort-task.md#異常系 | stopped の記録は完了しているため 0 で終了する。notified_at は書かれず、通知失敗と次のtickが再通知する旨の警告(notify_warning)が表示される |
| TC-exec-abort-task-029 | AbortTask 境界値: それぞれに abort を実行する(stopped・アーカイブ済み以外の全実行状態(pending / launching…) | spec/testcases/execution/abort-task.md#境界値 | いずれも受理され stopped に至る。kill を伴うのは kill 対象(生存プロセス)が同定できた場合のみで、実行状態ごとの例外分岐はない(requirements §6.5) |
| TC-exec-abort-task-030 | AbortTask 境界値: abort を実行する(どの経路の stopped 記録も) | spec/testcases/execution/abort-task.md#境界値 | 常に `notified_at: None` で記録される(過去の凍結の通知記録を引き継がず、必ず通知対象になる) |
| TC-exec-abort-task-031 | AbortTask エッジケース: abort を実行する(running のタスク・PID が別プロセスに再利用されている(starttime…) | spec/testcases/execution/abort-task.md#エッジケース | Dead(死亡)と判定され、無関係なプロセスを kill せずに stopped の記録のみ行う(誤殺しない。requirements §6.2) |
| TC-exec-abort-task-032 | AbortTask エッジケース: abort を実行する(launching のタスク・pid ファイルあり(starttime あり)…) | spec/testcases/execution/abort-task.md#エッジケース | Dead と判定され、無関係なプロセスを kill せず stopped の記録と通知のみ行う(「照合一致時のみ kill」の規則は runディレクトリ由来の同定情報でも成立する。マーカー書き込み後の再確認で現れた pid の照合不一致も同じ。requirements §6.2) |
| TC-exec-abort-task-033 | AbortTask エッジケース: abort を再実行する(kill 成功後に `save` が失敗した(running のまま・プロセスは死亡)) | spec/testcases/execution/abort-task.md#エッジケース | 生存観測が Dead を返し、kill なしで stopped の記録のみで完了する(初回は非0 で再実行が案内されている) |
| TC-exec-abort-task-034 | AbortTask エッジケース: 次の tick を実行する(kill 成功後 `save` 失敗のまま放置した) | spec/testcases/execution/abort-task.md#エッジケース | 「exitなし・プロセス死亡」として failed → 再起動へ進み得る(このため abort は非0 で再実行を案内する) |
| TC-exec-abort-task-035 | AbortTask エッジケース: abort を再実行する(runファイル破損で abort が拒否され続けた後、人間が破損 runファイルを削除した) | spec/testcases/execution/abort-task.md#エッジケース | 「不在」としてマーカープロトコルに合流し、通常どおり stopped を確定できる |
| TC-exec-abort-task-036 | AbortTask エッジケース: (ラッパーの動作を観測する)(launching・pid なしで abort により stopped 確定後…) | spec/testcases/execution/abort-task.md#エッジケース | ラッパーは pid 書き込み後のマーカー確認で終了し、エージェントは起動されない(凍結後に実行が走り出さない) |
| TC-exec-abort-task-037 | AbortTask エッジケース: 次の tick を実行する(未通知(notified_at なし)のまま abort が 0 終了した(通知失敗)) | spec/testcases/execution/abort-task.md#エッジケース | tick が「notified_at のない stopped」を検出して再通知する(at-least-once) |
| TC-exec-abort-task-038 | AbortTask エッジケース: abort を実行する(`state/` 配下ディレクトリ(tasks / archive)が存在しない) | spec/testcases/execution/abort-task.md#エッジケース | 「タスク不在」として非0 で終了し、何も変更しない(pages 縮退表「state/ 配下ディレクトリ不在」) |
| TC-exec-run-wrapper-001 | RunWrapper 正常系: ラッパーを実行する(正しい起動引数(run_dir / workspace / agent_cmd)で起動され…) | spec/testcases/execution/run-wrapper.md#正常系 | `own_identity` で自身の pid・kill同定子・starttime を取得し、starttime → pid の順で runディレクトリへ書き込む |
| TC-exec-run-wrapper-002 | RunWrapper 正常系: (続けて同一実行)(starttime・pid の書き込みに成功し、無効化マーカーが存在しない) | spec/testcases/execution/run-wrapper.md#正常系 | エージェントが workspace(worktree)を作業ディレクトリとして起動され、標準出力・標準エラーが `stdout.log` / `stderr.log` へリダイレクトされる |
| TC-exec-run-wrapper-003 | RunWrapper 正常系: (続けて同一実行)(エージェントが exit 0 で正常終了する) | spec/testcases/execution/run-wrapper.md#正常系 | exit ファイルに 0 が書き込まれ、ラッパーが終了する |
| TC-exec-run-wrapper-004 | RunWrapper 正常系: (続けて同一実行)(エージェントが非0(例: 1)で終了する) | spec/testcases/execution/run-wrapper.md#正常系 | exit ファイルにその値が書き込まれる(判定は tick 側の責務。ラッパーは符号化と永続化のみ) |
| TC-exec-run-wrapper-005 | RunWrapper 正常系: ラッパーを実行する(pid 書き込み後のマーカー確認で無効化マーカーが存在する) | spec/testcases/execution/run-wrapper.md#正常系 | エージェントを起動せず正常終了する(遅延起動の排除。その後の分類は実行状態に依る — tick が再確認で pid を取り込み running へ進めたケースは tick 手続きD のエッジケースが定める) |
| TC-exec-run-wrapper-006 | RunWrapper 正常系: ラッパーを実行する(マーカー確認自体が失敗する(`marker_exists` が `Err(Io)`)) | spec/testcases/execution/run-wrapper.md#正常系 | エージェントを起動せず終了する(無効化されていないことを確認できない以上、起動は安全側に倒す)。次tickが「exitなし・プロセス死亡」として failed に分類する |
| TC-exec-run-wrapper-007 | RunWrapper 正常系: ラッパーを実行する(config.yaml が不在・破損した環境) | spec/testcases/execution/run-wrapper.md#正常系 | 動作に影響しない(config は読まない。必要な情報はすべて起動引数で受け取る) |
| TC-exec-run-wrapper-008 | RunWrapper 正常系: ラッパーを実行する(tick が並行して動作している) | spec/testcases/execution/run-wrapper.md#正常系 | ラッパーはロックを取得せず動作する(書き先は自attemptのrunディレクトリに閉じ、tickと競合しない) |
| TC-exec-run-wrapper-009 | RunWrapper 異常系: ラッパーを実行する(起動引数が不正(`WrapperLaunchSpec` の直列化の破れ)) | spec/testcases/execution/run-wrapper.md#異常系 | runディレクトリに何も書かず非0で終了する(pid が現れないため、猶予時間経路が spawn失敗として分類する) |
| TC-exec-run-wrapper-010 | RunWrapper 異常系: ラッパーを実行する(`own_identity` が失敗する(Io)) | spec/testcases/execution/run-wrapper.md#異常系 | 何も書き残さず終了する(自前のエラー報告経路を持たない。観測されないことで tick が分類する) |
| TC-exec-run-wrapper-011 | RunWrapper 異常系: ラッパーを実行する(starttime の書き込みが失敗する) | spec/testcases/execution/run-wrapper.md#異常系 | 何も書き残さず終了する(猶予時間経路が spawn失敗として分類する) |
| TC-exec-run-wrapper-012 | RunWrapper 異常系: ラッパーを実行する(pid の書き込みが失敗する(starttime は書き込み済み)) | spec/testcases/execution/run-wrapper.md#異常系 | 以降を書かずに終了する。starttime のみの状態は書き込み順序の正常な中間状態であり、pid 不在として猶予時間経路が spawn失敗に分類する |
| TC-exec-run-wrapper-013 | RunWrapper 異常系: ラッパーを実行する(エージェントのコマンドが存在しない(起動不能)) | spec/testcases/execution/run-wrapper.md#異常系 | exit ファイルに 127 を書いて終了する(spawn失敗ではなく通常の実行失敗(failed)として分類させる) |
| TC-exec-run-wrapper-014 | RunWrapper 異常系: ラッパーを実行する(エージェントのコマンドが実行不能(権限なし等)) | spec/testcases/execution/run-wrapper.md#異常系 | exit ファイルに 126 を書いて終了する |
| TC-exec-run-wrapper-015 | RunWrapper 異常系: ラッパーを実行する(エージェントがシグナルで死んだ(exit code を持たない終了)) | spec/testcases/execution/run-wrapper.md#異常系 | exit ファイルに 128+シグナル番号 を書いて終了する(デフォルト判定では非0 = failed で足りる) |
| TC-exec-run-wrapper-016 | RunWrapper 異常系: ラッパーを実行する(ログのリダイレクト先を開けない(権限・ディスク満杯等)) | spec/testcases/execution/run-wrapper.md#異常系 | エージェントを起動せず exit ファイルに 126 を書いて終了する(通常の実行失敗として failed 経路に合流させる) |
| TC-exec-run-wrapper-017 | RunWrapper 異常系: ラッパーを実行する(workspace(worktree)が手動削除等で存在しない状態で起動された) | spec/testcases/execution/run-wrapper.md#異常系 | エージェント起動不能として非0(126 等)が exit ファイルに符号化され、通常の実行失敗(failed 経路)に合流する(失敗の現れ方はエージェント・コマンドテンプレートに依るため、期待は failed 経路への合流を軸とする。pages ※9) |
| TC-exec-run-wrapper-018 | RunWrapper 異常系: ラッパーを実行する(exit の書き込みが失敗する) | spec/testcases/execution/run-wrapper.md#異常系 | exit を書けないまま終了する(次tickが「exitファイルなし・プロセス死亡」として failed に分類する) |
| TC-exec-run-wrapper-019 | RunWrapper 境界値: ラッパーを実行する(エージェントの exit code が 0 / 非0 / 126 / 127 /…) | spec/testcases/execution/run-wrapper.md#境界値 | いずれもそのままの数値が exit ファイルに符号化されて永続化される(区別・解釈はしない) |
| TC-exec-run-wrapper-020 | RunWrapper 境界値: ラッパーを実行する(テンプレートが `{workspace}` を参照しない agent_cmd) | spec/testcases/execution/run-wrapper.md#境界値 | 参照の有無によらず、カレントディレクトリは常に worktree(workspace)でエージェントが起動される |
| TC-exec-run-wrapper-021 | RunWrapper エッジケース: (書き込み順序を検証する)(tick が pid の存在を観測した時点) | spec/testcases/execution/run-wrapper.md#エッジケース | starttime は必ず先に書かれているため、pid が存在すれば同定情報一式が揃っている(starttime → pid の順序保証。requirements §4.1) |
| TC-exec-run-wrapper-022 | RunWrapper エッジケース: (順序を検証する)(マーカー確認は pid 書き込みの後に行われる) | spec/testcases/execution/run-wrapper.md#エッジケース | ラッパー側「pid 書き込み後にマーカー確認」とツール側「マーカー書き込み後に pid 再確認」の順序の組により、どちらの観測が先でも遅延起動ラッパーと新attemptの並走が排除される |
| TC-exec-run-wrapper-023 | RunWrapper エッジケース: ラッパーの動作を観測する(ラッパーが pid を書いた直後に tick / abort がマーカーを書いた…) | spec/testcases/execution/run-wrapper.md#エッジケース | マーカー確認で存在を検出し、エージェントを起動せず終了する(二重起動防止) |
| TC-exec-run-wrapper-024 | RunWrapper エッジケース: ツール側の動作を観測する(ラッパーのマーカー確認(不在)後にツール側がマーカーを書いた) | spec/testcases/execution/run-wrapper.md#エッジケース | ツール側はマーカー書き込み後の pid 再確認で pid を検出し、pending へ戻さず running に取り込む(並走しない) |
| TC-exec-run-wrapper-025 | RunWrapper エッジケース: tick 側から読み取る(tick と並行して starttime / pid / exit が書き込まれている) | spec/testcases/execution/run-wrapper.md#エッジケース | 各ファイルはアトミック置換で書かれるため、読み手は常に「不在」か「完全な内容」のどちらかのみを観測する(書きかけは観測されない) |
| TC-exec-run-wrapper-026 | RunWrapper エッジケース: ラッパーを実行する(エージェントが何も出力せず終了した) | spec/testcases/execution/run-wrapper.md#エッジケース | ログファイルは空(または最小限)のまま exit が書かれる。異常ではない(判定は exit による) |
| TC-exec-run-wrapper-027 | RunWrapper エッジケース: 次の tick を観測する(エージェント実行中(exit 書き込み前)にラッパーごと kill された) | spec/testcases/execution/run-wrapper.md#エッジケース | exit は書かれず、tick が「exitなし・プロセス死亡」として failed に分類する(残存プロセスは try_kill_remnants のベストエフォート対象) |
| TC-exec-tick-001 | Tick 走査と分岐(処理フロー 1〜9) > 正常系: tick を実行する(AgentRun ステータスの pending タスクが1件ある) | spec/testcases/execution/tick.md#正常系 | 手続きA(起動)が実行され、サマリーの `launched` に記録される |
| TC-exec-tick-002 | Tick 走査と分岐(処理フロー 1〜9) > 正常系: tick を実行する(Wait ステータスの pending タスクがある) | spec/testcases/execution/tick.md#正常系 | 何もしない(タスクファイル・カウンタ・runディレクトリのいずれも変化しない) |
| TC-exec-tick-003 | Tick 走査と分岐(処理フロー 1〜9) > 正常系: tick を実行する(Cleanup ステータスの pending タスクがある) | spec/testcases/execution/tick.md#正常系 | 手続きB(終端処理)が実行される |
| TC-exec-tick-004 | Tick 走査と分岐(処理フロー 1〜9) > 正常系: tick を実行する(AgentRun ステータスの failed タスクがある) | spec/testcases/execution/tick.md#正常系 | pending と同じく手続きAで再起動される(ADR-012) |
| TC-exec-tick-005 | Tick 走査と分岐(処理フロー 1〜9) > 正常系: tick を実行する(Cleanup ステータスの failed タスクがある) | spec/testcases/execution/tick.md#正常系 | 手続きBが再試行される |
| TC-exec-tick-006 | Tick 走査と分岐(処理フロー 1〜9) > 正常系: tick を実行する(Wait ステータスの failed タスクがある) | spec/testcases/execution/tick.md#正常系 | 何もしない |
| TC-exec-tick-007 | Tick 走査と分岐(処理フロー 1〜9) > 正常系: tick を実行する(launching のタスクがある) | spec/testcases/execution/tick.md#正常系 | 手続きC(spawn確認)が実行される |
| TC-exec-tick-008 | Tick 走査と分岐(処理フロー 1〜9) > 正常系: tick を実行する(running のタスクがある) | spec/testcases/execution/tick.md#正常系 | 手続きD(観測・判定)が実行される |
| TC-exec-tick-009 | Tick 走査と分岐(処理フロー 1〜9) > 正常系: tick を実行する(completed のタスク(現ステータスは AgentRun・next 定義あり)がある) | spec/testcases/execution/tick.md#正常系 | `advance` によりタスクステータスが `next` へ遷移し、実行状態は pending。`transitioned` に記録される |
| TC-exec-tick-010 | Tick 走査と分岐(処理フロー 1〜9) > 正常系: tick を実行する(stopped(notified_at あり)のタスクがある) | spec/testcases/execution/tick.md#正常系 | 何もしない(起動・遷移・通知のいずれも行わない) |
| TC-exec-tick-011 | Tick 走査と分岐(処理フロー 1〜9) > 正常系: tick を実行する(stopped(notified_at なし)のタスクがある) | spec/testcases/execution/tick.md#正常系 | 共通手続き notify が実行される |
| TC-exec-tick-012 | Tick 走査と分岐(処理フロー 1〜9) > 正常系: tick を実行する(タスクが1件もない) | spec/testcases/execution/tick.md#正常系 | 処理対象がない旨を表示して 0 で終了する |
| TC-exec-tick-013 | Tick 走査と分岐(処理フロー 1〜9) > 正常系: tick を実行する(実行状態の異なる複数のタスクがある) | spec/testcases/execution/tick.md#正常系 | 各タスクがそれぞれの分岐で1ステップずつ処理され、サマリーに集約される。exit は 0 |
| TC-exec-tick-014 | Tick 走査と分岐(処理フロー 1〜9) > 正常系: tick を実行する(実行可能な pending タスクが複数ある) | spec/testcases/execution/tick.md#正常系 | すべて起動される(並列度制御は行わない) |
| TC-exec-tick-015 | Tick 走査と分岐(処理フロー 1〜9) > 異常系: tick を実行する(別の操作が排他ロックを保持している) | spec/testcases/execution/tick.md#異常系 | スキップした旨を表示して 0 で終了する。状態は変更しない |
| TC-exec-tick-016 | Tick 走査と分岐(処理フロー 1〜9) > 異常系: tick を実行する(ロック機構自体が異常(`LockError::Failed`)) | spec/testcases/execution/tick.md#異常系 | 非0 で終了する(競合の 0 スキップ例外は適用しない) |
| TC-exec-tick-017 | Tick 走査と分岐(処理フロー 1〜9) > 異常系: tick を実行する(config.yaml が存在しない・パース不能・読めない(Io)のいずれか) | spec/testcases/execution/tick.md#異常系 | 非0 で終了する。状態は変更しない |
| TC-exec-tick-018 | Tick 走査と分岐(処理フロー 1〜9) > 異常系: tick を実行する(`list_active` が Io エラー(走査自体の失敗)) | spec/testcases/execution/tick.md#異常系 | 非0 で終了する。状態は変更しない |
| TC-exec-tick-019 | Tick 走査と分岐(処理フロー 1〜9) > 異常系: tick を実行する(パース不能なタスクファイル(`Corrupt`)が混在する) | spec/testcases/execution/tick.md#異常系 | 当該タスクは報告のみで書き込まない(stopped化もしない)。残りのタスクは処理を続行し、tick は 0 |
| TC-exec-tick-020 | Tick 走査と分岐(処理フロー 1〜9) > 異常系: tick を実行する(スナップショットのみ破損(`SnapshotUnreadable`)・stopped…) | spec/testcases/execution/tick.md#異常系 | 定義依存の判断(起動・遷移・終端処理)をすべてスキップして報告する。書き込まない。tick は 0 |
| TC-exec-tick-021 | Tick 走査と分岐(処理フロー 1〜9) > 異常系: tick を実行する(completed だが手動修復により遷移の前提が破れている(`TransitionErr…) | spec/testcases/execution/tick.md#異常系 | 報告してそのタスクをスキップする。tick は 0 |
| TC-exec-tick-022 | Tick 走査と分岐(処理フロー 1〜9) > 異常系: tick を実行する(手動修復で不変条件が破れている(Running なのに `current_attempt`…) | spec/testcases/execution/tick.md#異常系 | 不変条件の破れとして報告してスキップする(検出は手続きC / D 冒頭のユースケース検査、または遷移関数の `TransitionError`(`MissingCurrentAttempt` 等))。修復は人間に委ねる |
| TC-exec-tick-023 | Tick 走査と分岐(処理フロー 1〜9) > 異常系: tick を実行する(1タスクの処理が失敗する(観測の Io 失敗等)) | spec/testcases/execution/tick.md#異常系 | `errors` に記録して残りのタスクを続行する。tick 全体は 0 |
| TC-exec-tick-024 | Tick 走査と分岐(処理フロー 1〜9) > エッジケース: tick を連続して複数回実行する(状態が変化しないタスク群(Wait 滞留・猶予内待機・実行継続中)) | spec/testcases/execution/tick.md#エッジケース | 毎回同じ判断が再導出され、書き込みは発生しない(tick の冪等性) |
| TC-exec-tick-025 | Tick 走査と分岐(処理フロー 1〜9) > エッジケース: tick を実行する(running のタスクの exit 0 を観測した) | spec/testcases/execution/tick.md#エッジケース | この tick では completed の記録まで(`complete_run`)。next への遷移は行わず、次の tick の `advance` に委ねる(1タスク1tick1ステップ) |
| TC-exec-tick-026 | Tick 走査と分岐(処理フロー 1〜9) > エッジケース: tick を実行する(`SnapshotUnreadable` かつ stopped(notified_at…) | spec/testcases/execution/tick.md#エッジケース | スナップショット破損を理由にスキップせず、共通手続き notify を実行する(通知は定義非依存) |
| TC-exec-tick-027 | Tick 走査と分岐(処理フロー 1〜9) > エッジケース: tick を実行する(config.yaml は存在するが `state/tasks/` ディレクトリが未作成…) | spec/testcases/execution/tick.md#エッジケース | 走査は空結果として扱われ、処理対象がない旨を表示して 0 で終了する(`TaskRepository` 契約: 走査対象ディレクトリの不在は空結果。pages ※3。config.yaml まで無い未初期化ホームは ※1 の非0) |
| TC-exec-tick-028 | Tick 手続きA: 起動(Pending / Failed × AgentRun) > 正常系: tick を実行する(workspace 未確定の pending タスク) | spec/testcases/execution/tick.md#正常系-1 | タスクIDから決定的に導出されたパス(`worktrees/<task-id>`)・ブランチ(`pulsen/<task-id>`)で worktree が作成され、`confirm_workspace` が保存される |
| TC-exec-tick-029 | Tick 手続きA: 起動(Pending / Failed × AgentRun) > 正常系: (続けて同一 tick の処理)(worktree 作成に成功した(同一 tick 内)) | spec/testcases/execution/tick.md#正常系-1 | テンプレート展開 → launching記録 → `prepare_attempt` → spawn の順で進む(ADR-016 の順序)。実行状態は launching になり `recorded_at` が記録される |
| TC-exec-tick-030 | Tick 手続きA: 起動(Pending / Failed × AgentRun) > 正常系: tick を実行する(workspace 確定済みの pending タスク) | spec/testcases/execution/tick.md#正常系-1 | worktree 作成を行わず、展開から起動処理が始まる |
| TC-exec-tick-031 | Tick 手続きA: 起動(Pending / Failed × AgentRun) > 正常系: tick を実行する(ステータスに agent / model の上書きがあり、`cmd` が…) | spec/testcases/execution/tick.md#正常系-1 | spawn に渡るコマンドラインに、ステータス上書きのエージェント定義・上書きモデル名・当該タスクの worktree パスが展開されている(実効値の解決: ステータス > ワークフローデフォルト。requirements §3.1・§7) |
| TC-exec-tick-032 | Tick 手続きA: 起動(Pending / Failed × AgentRun) > 正常系: tick を実行する(`skill` 指定のステータスで、エージェント定義に `skill_input` がある) | spec/testcases/execution/tick.md#正常系-1 | `{input}` には `skill_input` で変換されたスキル入力(例: `/skill <名前>`)が展開されてコマンドラインに渡る |
| TC-exec-tick-033 | Tick 手続きA: 起動(Pending / Failed × AgentRun) > 正常系: tick を実行する(failed のタスク(workspace 確定済み)) | spec/testcases/execution/tick.md#正常系-1 | 新しい attempt 番号(現番号+1)で launching が記録される。過去の attempt 番号は再利用されない |
| TC-exec-tick-034 | Tick 手続きA: 起動(Pending / Failed × AgentRun) > 正常系: (続けて同一 tick の処理)(launching 記録が成功した) | spec/testcases/execution/tick.md#正常系-1 | run_dir(`state/runs/<task-id>/attempt-<n>`)がタスクファイルに記録され、`prepare_attempt` でディレクトリが作成され、`spawn_wrapper` が run_dir・agent_cmd・workspace を渡してデタッチ起動される |
| TC-exec-tick-035 | Tick 手続きA: 起動(Pending / Failed × AgentRun) > 正常系: tick を実行する(attempt_count > 0 の failed タスクで worktree 作成…) | spec/testcases/execution/tick.md#正常系-1 | attempt_count・judge_attempt_count はリセット**されない**(途中のツール操作の成功ではリセットしない。requirements §6.4・ADR-009)。起動処理はそのまま進む |
| TC-exec-tick-036 | Tick 手続きA: 起動(Pending / Failed × AgentRun) > 正常系: 次の tick を実行する(attempt_count > 0 のタスクが起動され running へ取り込まれた) | spec/testcases/execution/tick.md#正常系-1 | `confirm_running` でリセットされるのは spawn_fail_count のみで、attempt_count・judge_attempt_count は保持される(リセットは completed / skipped の確定と人間の操作のみ) |
| TC-exec-tick-037 | Tick 手続きA: 起動(Pending / Failed × AgentRun) > 異常系: tick を実行する(worktree 作成が失敗する) | spec/testcases/execution/tick.md#異常系-1 | `record_tool_failure(WorktreeCreate)`: attempt_count が加算され、実行状態は failed、last_failure に失敗要因が記録される。次tickで再試行される |
| TC-exec-tick-038 | Tick 手続きA: 起動(Pending / Failed × AgentRun) > 異常系: tick を実行する(worktree 作成の失敗で attempt_count が上限…) | spec/testcases/execution/tick.md#異常系-1 | `Stopped { RetryLimitExceeded, notified_at: None }` を保存し、直後に共通手続き notify を実行する。`frozen` に記録される |
| TC-exec-tick-039 | Tick 手続きA: 起動(Pending / Failed × AgentRun) > 異常系: tick を実行する(実効エージェント名が解決できない(手動修復でスナップショットが破れた)) | spec/testcases/execution/tick.md#異常系-1 | 展開失敗として `record_spawn_failure_in_place` で処理される(ADR-016 の分類と同じ) |
| TC-exec-tick-040 | Tick 手続きA: 起動(Pending / Failed × AgentRun) > 異常系: tick を実行する(実効エージェント名が config.agents に存在しない) | spec/testcases/execution/tick.md#異常系-1 | 展開失敗(同期spawn失敗): spawn_fail_count 加算・実行状態不変・attempt 採番なし・runディレクトリ作成なし・無効化マーカーなし・last_failure に展開エラーを記録 |
| TC-exec-tick-041 | Tick 手続きA: 起動(Pending / Failed × AgentRun) > 異常系: tick を実行する(エージェント定義のテンプレートが不正(`RawAgentDefinition::parse…) | spec/testcases/execution/tick.md#異常系-1 | 展開失敗として同期spawn失敗経路で処理される |
| TC-exec-tick-042 | Tick 手続きA: 起動(Pending / Failed × AgentRun) > 異常系: tick を実行する(skill 指定のステータスでエージェント定義に skill_input がない…) | spec/testcases/execution/tick.md#異常系-1 | 展開失敗として同期spawn失敗経路で処理される |
| TC-exec-tick-043 | Tick 手続きA: 起動(Pending / Failed × AgentRun) > 異常系: tick を実行する(テンプレート展開で値が解決できない(`ExpansionError`。`{model}`…) | spec/testcases/execution/tick.md#異常系-1 | 展開失敗として同期spawn失敗経路で処理される |
| TC-exec-tick-044 | Tick 手続きA: 起動(Pending / Failed × AgentRun) > 異常系: tick を実行する(failed のタスクでテンプレート展開が失敗する) | spec/testcases/execution/tick.md#異常系-1 | spawn_fail_count のみ加算され、実行状態は failed のまま変わらない |
| TC-exec-tick-045 | Tick 手続きA: 起動(Pending / Failed × AgentRun) > 異常系: tick を実行する(展開失敗の加算で spawn_fail_count が上限(`spawn_fail_lim…) | spec/testcases/execution/tick.md#異常系-1 | `Stopped { SpawnFailLimitExceeded }` を保存し notify を実行する |
| TC-exec-tick-046 | Tick 手続きA: 起動(Pending / Failed × AgentRun) > 異常系: tick を実行する(`prepare_attempt` が失敗する) | spec/testcases/execution/tick.md#異常系-1 | 報告のみ行い、launching のまま(猶予時間経路が次tick以降に分類する) |
| TC-exec-tick-047 | Tick 手続きA: 起動(Pending / Failed × AgentRun) > 異常系: tick を実行する(`spawn_wrapper` が同期エラー(`SpawnError`)を返す) | spec/testcases/execution/tick.md#異常系-1 | 状態を変更しない。launching のまま猶予時間経路が分類する |
| TC-exec-tick-048 | Tick 手続きA: 起動(Pending / Failed × AgentRun) > 境界値: tick を実行する(`retries: 0` のステータスで worktree 作成が失敗する) | spec/testcases/execution/tick.md#境界値 | 加算後 attempt_count = 1 > 0 のため、failed を経由せず即 stopped になる |
| TC-exec-tick-049 | Tick 手続きA: 起動(Pending / Failed × AgentRun) > 境界値: tick を実行する(展開失敗の加算後 spawn_fail_count = 上限(等号)) | spec/testcases/execution/tick.md#境界値 | 凍結しない(`count > limit` のみ超過)。実行状態は不変のまま |
| TC-exec-tick-050 | Tick 手続きA: 起動(Pending / Failed × AgentRun) > 境界値: tick を実行する(展開失敗の加算後 spawn_fail_count = 上限+1) | spec/testcases/execution/tick.md#境界値 | stopped になる(デフォルト上限3なら4回目の連続失敗で凍結) |
| TC-exec-tick-051 | Tick 手続きA: 起動(Pending / Failed × AgentRun) > エッジケース: 次の tick を実行する(worktree 作成成功と `confirm_workspace` 保存の間で前回の…) | spec/testcases/execution/tick.md#エッジケース-1 | 同じ Workspace が再導出され、`create` が既存の worktree+ブランチを達成済みとして成功させる。同じ判断が再導出され、利用者の修復操作なしで進行が再開する |
| TC-exec-tick-052 | Tick 手続きA: 起動(Pending / Failed × AgentRun) > エッジケース: tick を実行する(ブランチ `pulsen/<task-id>` のみ残存し worktree がない…) | spec/testcases/execution/tick.md#エッジケース-1 | `create` が既存ブランチに worktree を張り直して成功する |
| TC-exec-tick-053 | Tick 手続きA: 起動(Pending / Failed × AgentRun) > エッジケース: 次の tick を実行する(launching 記録の保存と `prepare_attempt` の間で前回の…) | spec/testcases/execution/tick.md#エッジケース-1 | 手続きC の read 系が `Ok(None)` を返し、猶予時間経路に合流する(猶予超過後 spawn失敗として pending 復帰) |
| TC-exec-tick-054 | Tick 手続きA: 起動(Pending / Failed × AgentRun) > エッジケース: 次の tick を実行する(launching 記録・`prepare_attempt` 成功と spawn…) | spec/testcases/execution/tick.md#エッジケース-1 | pid が現れないため猶予時間経路が spawn失敗として分類する(同じ判断の再導出) |
| TC-exec-tick-055 | Tick 手続きA: 起動(Pending / Failed × AgentRun) > エッジケース: 次の tick を実行する(展開失敗の原因を config.yaml の編集で修正した) | spec/testcases/execution/tick.md#エッジケース-1 | 展開は起動のたびに行われるため、修正が即座に反映されて起動に成功する(グローバル設定はスナップショットされない) |
| TC-exec-tick-056 | Tick 手続きB: 終端処理(Pending / Failed × Cleanup) > 正常系: tick を実行する(Cleanup ステータスの pending タスク(workspace 確定済み…) | spec/testcases/execution/tick.md#正常系-2 | worktree が削除され(ブランチは削除しない)、タスクファイルが archive へ移動し、`archived` に記録される。以降走査対象から外れる。runディレクトリは残る |
| TC-exec-tick-057 | Tick 手続きB: 終端処理(Pending / Failed × Cleanup) > 正常系: tick を実行する(workspace 未確定のままクリーンアップに到達したタスク(worktree 未作成)) | spec/testcases/execution/tick.md#正常系-2 | `WorkspacePlanner::derive` の決定的導出パスへの `remove` が `AlreadyAbsent` を返し(達成済み)、アーカイブ処理へ進む |
| TC-exec-tick-058 | Tick 手続きB: 終端処理(Pending / Failed × Cleanup) > 正常系: tick を実行する(workspace 未確定だが、決定的導出パスに worktree が残っている…) | spec/testcases/execution/tick.md#正常系-2 | 決定的導出パスへの `remove` が worktree を削除してからアーカイブする(孤児 worktree を残さない。手続きB) |
| TC-exec-tick-059 | Tick 手続きB: 終端処理(Pending / Failed × Cleanup) > 正常系: tick を実行する(worktree が既に存在しない(手動削除済み。`AlreadyAbsent`)) | spec/testcases/execution/tick.md#正常系-2 | 削除は達成済みとして成功扱いになり、アーカイブへ進む |
| TC-exec-tick-060 | Tick 手続きB: 終端処理(Pending / Failed × Cleanup) > 正常系: tick を実行する(`state/archive/` ディレクトリが存在しない状態での初回の終端処理) | spec/testcases/execution/tick.md#正常系-2 | `archive` が必要なディレクトリを自動作成してアーカイブ移動が成功する(`state/` 配下はツールが管理する領域。pages ※3) |
| TC-exec-tick-061 | Tick 手続きB: 終端処理(Pending / Failed × Cleanup) > 異常系: tick を実行する(worktree 削除が失敗する(ファイルを掴むプロセスがいる等)) | spec/testcases/execution/tick.md#異常系-2 | `record_tool_failure(WorktreeRemove)`: attempt_count 加算・failed・last_failure 記録。タスクは tasks に残り、次tickで再試行される |
| TC-exec-tick-062 | Tick 手続きB: 終端処理(Pending / Failed × Cleanup) > 異常系: tick を実行する(worktree 削除の失敗で上限を超過する) | spec/testcases/execution/tick.md#異常系-2 | `Stopped { RetryLimitExceeded }` を保存し notify を実行する |
| TC-exec-tick-063 | Tick 手続きB: 終端処理(Pending / Failed × Cleanup) > 異常系: tick を実行する(アーカイブ移動が失敗する(権限不足等)) | spec/testcases/execution/tick.md#異常系-2 | `record_tool_failure(ArchiveMove)`: attempt_count 加算・failed・last_failure 記録 |
| TC-exec-tick-064 | Tick 手続きB: 終端処理(Pending / Failed × Cleanup) > 異常系: tick を実行する(アーカイブ移動の失敗で上限を超過する) | spec/testcases/execution/tick.md#異常系-2 | stopped になり notify が実行される |
| TC-exec-tick-065 | Tick 手続きB: 終端処理(Pending / Failed × Cleanup) > 境界値: tick を繰り返し実行する(Cleanup ステータスでの失敗の繰り返し) | spec/testcases/execution/tick.md#境界値-1 | 適用される上限は常に組み込みデフォルト 2(ADR-014。上書き不可): 加算後 attempt_count = 2 では failed のまま、= 3 で stopped になる |
| TC-exec-tick-066 | Tick 手続きB: 終端処理(Pending / Failed × Cleanup) > エッジケース: 次の tick を実行する(worktree 削除成功後にアーカイブ移動が失敗した) | spec/testcases/execution/tick.md#エッジケース-2 | `remove` が `AlreadyAbsent` を返すため、実質アーカイブ移動から再開する(冪等) |
| TC-exec-tick-067 | Tick 手続きB: 終端処理(Pending / Failed × Cleanup) > エッジケース: 次の tick を実行する(worktree 削除とアーカイブの間で前回の tick がクラッシュした) | spec/testcases/execution/tick.md#エッジケース-2 | 「Cleanup ステータスのタスクがまだ tasks にある」ことから同じ処理が再導出される。worktree なしの削除は成功扱いで、二重処理は無害 |
| TC-exec-tick-068 | Tick 手続きC: spawn確認(Launching) > 正常系: tick を実行する(runディレクトリに pid・starttime の両方がある) | spec/testcases/execution/tick.md#正常系-3 | `ConfirmRunning`: 実行状態が running になり、`current_attempt.process` に同定情報一式(pid・kill同定子・starttime)が取り込まれ、spawn_fail_count が 0 にリセットされる。サマリーの `confirmed_running` に記録される |
| TC-exec-tick-069 | Tick 手続きC: spawn確認(Launching) > 正常系: tick を実行する(pid がなく、launching 記録からの経過が猶予時間内) | spec/testcases/execution/tick.md#正常系-3 | `KeepWaiting`: 何もしない(ラッパーの書き込み待ち) |
| TC-exec-tick-070 | Tick 手続きC: spawn確認(Launching) > 正常系: tick を実行する(pid がなく、猶予時間を超過している) | spec/testcases/execution/tick.md#正常系-3 | 無効化マーカーを書き、pid を再読する。なお pid がなければ `record_spawn_failure`: pending 復帰・spawn_fail_count 加算・last_failure = SpawnFail |
| TC-exec-tick-071 | Tick 手続きC: spawn確認(Launching) > 正常系: tick を実行する(マーカー書き込み後の再読で pid・starttime が現れていた) | spec/testcases/execution/tick.md#正常系-3 | pending に戻さず `confirm_running` で running へ取り込む |
| TC-exec-tick-072 | Tick 手続きC: spawn確認(Launching) > 異常系: tick を実行する(`read_pid_file` / `read_starttime` が…) | spec/testcases/execution/tick.md#異常系-3 | 報告してスキップする。launching のまま(書き込まない)。次tickで再観測 |
| TC-exec-tick-073 | Tick 手続きC: spawn確認(Launching) > 異常系: tick を実行する(pid はあるが starttime がない(`InconsistentRunFiles`…) | spec/testcases/execution/tick.md#異常系-3 | 報告してスキップする。次tickで再観測 |
| TC-exec-tick-074 | Tick 手続きC: spawn確認(Launching) > 異常系: tick を実行する(`write_invalidation_marker` が失敗する(`Err(Io)`)) | spec/testcases/execution/tick.md#異常系-3 | 状態を変更せず報告してスキップする(マーカーなしで pending に戻すと遅延起動ラッパーが新attemptと並走し得るため)。次tickで再試行 |
| TC-exec-tick-075 | Tick 手続きC: spawn確認(Launching) > 異常系: tick を実行する(マーカー書き込み後の再読で pid あり・starttime なし) | spec/testcases/execution/tick.md#異常系-3 | `InconsistentRunFiles` として報告してスキップする(本体 classify と同じ場合分け) |
| TC-exec-tick-076 | Tick 手続きC: spawn確認(Launching) > 異常系: tick を実行する(`record_spawn_failure` の加算で spawn_fail_count…) | spec/testcases/execution/tick.md#異常系-3 | pending へ戻さず `Stopped { SpawnFailLimitExceeded }` を保存し notify を実行する |
| TC-exec-tick-077 | Tick 手続きC: spawn確認(Launching) > 境界値: tick を実行する(pid なし・経過がちょうど猶予時間(30秒)に等しい) | spec/testcases/execution/tick.md#境界値-2 | 超過していないため `KeepWaiting`(超過は経過 > 30秒のみ) |
| TC-exec-tick-078 | Tick 手続きC: spawn確認(Launching) > 境界値: tick を実行する(pid なし・経過が猶予時間を超えている(30秒+1秒)) | spec/testcases/execution/tick.md#境界値-2 | `SuspectSpawnFailure` としてマーカー書き込みと再確認に進む |
| TC-exec-tick-079 | Tick 手続きC: spawn確認(Launching) > 境界値: tick を実行する(時計の巻き戻りで now が recorded_at より前(経過が負)) | spec/testcases/execution/tick.md#境界値-2 | 経過を 0 として扱い `KeepWaiting`(巻き戻りで過大評価しない) |
| TC-exec-tick-080 | Tick 手続きC: spawn確認(Launching) > エッジケース: tick を実行する(runディレクトリ自体が存在しない(launching 記録と…) | spec/testcases/execution/tick.md#エッジケース-3 | read 系は不在を `Ok(None)` として返し、猶予時間経路に合流する(正常な復旧経路) |
| TC-exec-tick-081 | Tick 手続きC: spawn確認(Launching) > エッジケース: tick を実行する(starttime のみあり・pid なし) | spec/testcases/execution/tick.md#エッジケース-3 | 書き込み順序(starttime → pid)の正常な中間状態として pid なしの猶予判定に従う |
| TC-exec-tick-082 | Tick 手続きC: spawn確認(Launching) > エッジケース: tick を実行する(tick がマーカーを書いた後に遅延起動したラッパーが pid を書いていた(再読で検出)) | spec/testcases/execution/tick.md#エッジケース-3 | running へ取り込む(新attemptとの並走は起きない)。ラッパーは pid 書き込み後のマーカー確認で未起動終了するため、次tickが「exitなし・プロセス死亡」として failed に分類する |
| TC-exec-tick-083 | Tick 手続きC: spawn確認(Launching) > エッジケース: tick を実行する(ラッパーが pid を書いた直後に tick がマーカーを書いた(ラッパー先行)) | spec/testcases/execution/tick.md#エッジケース-3 | 再読が pid を検出して running へ取り込む。pending 復帰しないため二重起動は起きない(マーカープロトコルの両順序で並走排除) |
| TC-exec-tick-084 | Tick 手続きC: spawn確認(Launching) > エッジケース: tick を繰り返し実行する(runファイルの破損(`Corrupt` / `InconsistentRunFiles`…) | spec/testcases/execution/tick.md#エッジケース-3 | スキップと報告が続き、タスクは launching のまま滞留する(stopped に至らないため通知されない) |
| TC-exec-tick-085 | Tick 手続きC: spawn確認(Launching) > エッジケース: 次の tick を実行する(人間が破損した runファイルを削除した) | spec/testcases/execution/tick.md#エッジケース-3 | 「不在」として無効化マーカープロトコルに合流し、通常の spawn失敗分類で決着する |
| TC-exec-tick-086 | Tick 手続きC: spawn確認(Launching) > エッジケース: 次の tick を実行する(spawn失敗で pending 復帰したタスクが再起動される) | spec/testcases/execution/tick.md#エッジケース-3 | 新しい attempt 番号が採番され、runディレクトリも新しいパスになる(過去の試行の残骸と混同しない) |
| TC-exec-tick-087 | Tick 手続きD: 観測・判定(Running) > 正常系: tick を実行する(exit ファイルに 0・judge 未定義) | spec/testcases/execution/tick.md#正常系-4 | デフォルト判定で Completed → `complete_run`: completed になり attempt_count・judge_attempt_count が 0 にリセットされる(next への遷移は次tick)。サマリーの `judged` に記録される |
| TC-exec-tick-088 | Tick 手続きD: 観測・判定(Running) > 正常系: tick を実行する(exit ファイルに非0・judge 未定義) | spec/testcases/execution/tick.md#正常系-4 | デフォルト判定で Failed → `fail_run`: failed になり attempt_count 加算・judge_attempt_count リセット |
| TC-exec-tick-089 | Tick 手続きD: 観測・判定(Running) > 正常系: tick を実行する(exit ファイルあり・judge 定義あり) | spec/testcases/execution/tick.md#正常系-4 | 判定コマンドが `TASK_ID` / `WORKSPACE` / `EXIT_CODE`(10進文字列)/ `RUN_DIR` の環境変数と `config.judge_timeout` で、シェルを介さず直接起動される(引数なし・プレースホルダ展開なし) |
| TC-exec-tick-090 | Tick 手続きD: 観測・判定(Running) > 正常系: tick を実行する(判定コマンドが exit 0 で終了する) | spec/testcases/execution/tick.md#正常系-4 | Completed → `complete_run` |
| TC-exec-tick-091 | Tick 手続きD: 観測・判定(Running) > 正常系: tick を実行する(判定コマンドが exit 10 で終了する) | spec/testcases/execution/tick.md#正常系-4 | Failed → `fail_run`(通常の失敗として自動リトライ対象) |
| TC-exec-tick-092 | Tick 手続きD: 観測・判定(Running) > 正常系: tick を実行する(判定コマンドが exit 20 で終了する) | spec/testcases/execution/tick.md#正常系-4 | Skipped → `skip_run`: タスクステータス不変のまま pending に復帰し、attempt_count・judge_attempt_count が 0 にリセットされる。通知は行わず、サマリーの `skipped_back` に記録される(ADR-008) |
| TC-exec-tick-093 | Tick 手続きD: 観測・判定(Running) > 正常系: tick を実行する(exit なし・プロセス生存(照合一致)・timeout 未超過) | spec/testcases/execution/tick.md#正常系-4 | `KeepRunning`: 何もしない(exit があれば生存観測は行わない、の対偶として生存観測を経由する) |
| TC-exec-tick-094 | Tick 手続きD: 観測・判定(Running) > 正常系: tick を実行する(exit なし・プロセス生存・timeout 超過) | spec/testcases/execution/tick.md#正常系-4 | kill(照合一致時のみ)が成功したら `fail_run` で failed になる |
| TC-exec-tick-095 | Tick 手続きD: 観測・判定(Running) > 正常系: tick を実行する(exit なし・プロセス死亡(starttime 取得不能)) | spec/testcases/execution/tick.md#正常系-4 | `try_kill_remnants` で残存終了をベストエフォートで試みたうえで `fail_run` → failed(exit code 不明の一過性死は自動リトライで回復させる) |
| TC-exec-tick-096 | Tick 手続きD: 観測・判定(Running) > 異常系: tick を実行する(判定コマンドがプロトコル外の exit code(例: 1)で終了する) | spec/testcases/execution/tick.md#異常系-4 | `JudgeFailure` → `record_judge_failure`: judge_attempt_count 加算・running のまま・last_failure = JudgeFail(detail に原因)。次tickで再判定される |
| TC-exec-tick-097 | Tick 手続きD: 観測・判定(Running) > 異常系: tick を実行する(判定コマンドが judge_timeout を超過する(`TimedOut`)) | spec/testcases/execution/tick.md#異常系-4 | 判定失敗として `record_judge_failure` で処理される |
| TC-exec-tick-098 | Tick 手続きD: 観測・判定(Running) > 異常系: tick を実行する(判定コマンドが起動できない(`FailedToStart`)) | spec/testcases/execution/tick.md#異常系-4 | 判定失敗として `record_judge_failure` で処理される |
| TC-exec-tick-099 | Tick 手続きD: 観測・判定(Running) > 異常系: tick を実行する(判定失敗の加算で judge_attempt_count が上限…) | spec/testcases/execution/tick.md#異常系-4 | `Stopped { JudgeLimitExceeded }` を保存し notify を実行する(エージェントの再実行では解決しないためリトライせず凍結) |
| TC-exec-tick-100 | Tick 手続きD: 観測・判定(Running) > 異常系: tick を実行する(`fail_run` の加算で attempt_count が上限を超過する) | spec/testcases/execution/tick.md#異常系-4 | `Stopped { RetryLimitExceeded }` を保存し notify を実行する |
| TC-exec-tick-101 | Tick 手続きD: 観測・判定(Running) > 異常系: tick を実行する(timeout kill が失敗する(`KillError`)) | spec/testcases/execution/tick.md#異常系-4 | `fail_run` を呼ばず状態を変更せず報告のみ行う。次tickが同じ決定を再導出して再試行する(プロセス生存のまま failed → 再起動 → 同一worktree並走を防ぐ) |
| TC-exec-tick-102 | Tick 手続きD: 観測・判定(Running) > 異常系: tick を実行する(`starttime_of` が `Err(Io)` を返す(取得機構自体の失敗)) | spec/testcases/execution/tick.md#異常系-4 | 状態を変更せず報告してスキップする。次tickで再観測 |
| TC-exec-tick-103 | Tick 手続きD: 観測・判定(Running) > 異常系: tick を実行する(exit ファイルあり・`starttime_of` が失敗する環境) | spec/testcases/execution/tick.md#異常系-4 | 生存観測に依存せず判定が遅延なく実行され、分類が確定する(exit が Some なら判定 — 2段規則の1段目はユースケース側にあり、`classify_alive` は生存の分類だけを返す。観測の一過性失敗で判定を遅延させない) |
| TC-exec-tick-104 | Tick 手続きD: 観測・判定(Running) > 異常系: tick を実行する(`read_exit` が `RunFileError` を返す) | spec/testcases/execution/tick.md#異常系-4 | 当該タスクをスキップして報告する(書き込まない)。tick は 0 |
| TC-exec-tick-105 | Tick 手続きD: 観測・判定(Running) > 異常系: tick を実行する(`try_kill_remnants` が `NotIdentifiable` /…) | spec/testcases/execution/tick.md#異常系-4 | 結果は報告のみで、分類(failed)には影響しない(孤児の残存は許容) |
| TC-exec-tick-106 | Tick 手続きD: 観測・判定(Running) > 境界値: tick を実行する(launching の猶予に時間を要したタスクが running 中) | spec/testcases/execution/tick.md#境界値-3 | timeout の経過は記録済み starttime の壁時計成分(`starttime.wall`)を起点に測る(launching 記録から pid 出現までの猶予は timeout に含まれない) |
| TC-exec-tick-107 | Tick 手続きD: 観測・判定(Running) > 境界値: tick を実行する(starttime.wall からの経過がちょうど timeout に等しい) | spec/testcases/execution/tick.md#境界値-3 | 未超過として `KeepRunning` |
| TC-exec-tick-108 | Tick 手続きD: 観測・判定(Running) > 境界値: tick を実行する(starttime.wall からの経過が timeout を超えている) | spec/testcases/execution/tick.md#境界値-3 | `KillOnTimeout` |
| TC-exec-tick-109 | Tick 手続きD: 観測・判定(Running) > 境界値: tick を実行する(時計の巻き戻りで starttime.wall からの経過が負) | spec/testcases/execution/tick.md#境界値-3 | 経過を 0 として扱い `KeepRunning` |
| TC-exec-tick-110 | Tick 手続きD: 観測・判定(Running) > 境界値: tick を実行する(`timeout: none`(Unlimited)のステータス) | spec/testcases/execution/tick.md#境界値-3 | どれだけ経過しても `KeepRunning`(kill されない) |
| TC-exec-tick-111 | Tick 手続きD: 観測・判定(Running) > 境界値: tick を実行する(timeout 未指定のステータス・starttime.wall からの経過が 1h…) | spec/testcases/execution/tick.md#境界値-3 | 組み込みデフォルト 1h の超過として `KillOnTimeout` になる(無指定を Unlimited と扱わない。requirements §7.2) |
| TC-exec-tick-112 | Tick 手続きD: 観測・判定(Running) > 境界値: tick を実行する(`fail_run` の加算後 attempt_count = retry_limit…) | spec/testcases/execution/tick.md#境界値-3 | failed のまま(凍結しない)。デフォルト2なら初回+2回のリトライまで許容 |
| TC-exec-tick-113 | Tick 手続きD: 観測・判定(Running) > 境界値: tick を実行する(`fail_run` の加算後 attempt_count = retry_limit…) | spec/testcases/execution/tick.md#境界値-3 | stopped になる(デフォルト2なら3連続失敗で凍結) |
| TC-exec-tick-114 | Tick 手続きD: 観測・判定(Running) > 境界値: tick を実行する(`retries: 0` のステータスで実行が失敗する) | spec/testcases/execution/tick.md#境界値-3 | 初回の失敗で即 stopped になる |
| TC-exec-tick-115 | Tick 手続きD: 観測・判定(Running) > 境界値: tick を実行する(判定失敗の加算後 judge_attempt_count = judge_attempt_…) | spec/testcases/execution/tick.md#境界値-3 | running のまま次tickで再判定される(デフォルト3) |
| TC-exec-tick-116 | Tick 手続きD: 観測・判定(Running) > 境界値: tick を実行する(判定失敗の加算後 judge_attempt_count = judge_attempt_…) | spec/testcases/execution/tick.md#境界値-3 | stopped になる |
| TC-exec-tick-117 | Tick 手続きD: 観測・判定(Running) > エッジケース: tick を実行する(PID が別プロセスに再利用されている(starttime は取得できるが記録済み…) | spec/testcases/execution/tick.md#エッジケース-4 | `Dead` と判定され `DiedWithoutExit` → failed。無関係なプロセスを kill しない(kill は照合一致時のみ) |
| TC-exec-tick-118 | Tick 手続きD: 観測・判定(Running) > エッジケース: tick を実行する(判定失敗を記録した後の次tick(同じ exit・同じ定義)) | spec/testcases/execution/tick.md#エッジケース-4 | 再判定が同じ結論を導く(判定の冪等性) |
| TC-exec-tick-119 | Tick 手続きD: 観測・判定(Running) > エッジケース: 次の tick を実行する(判定確定後の `save` が失敗した(completed が永続化されなかった)) | spec/testcases/execution/tick.md#エッジケース-4 | 「running かつ exit あり」を再検出して再判定し、同じ結論に至る(永続化された事実からの再導出で復旧) |
| TC-exec-tick-120 | Tick 手続きD: 観測・判定(Running) > エッジケース: tick を実行する(judge 未定義のステータスでエージェントが exit 20 で終了する) | spec/testcases/execution/tick.md#エッジケース-4 | failed に分類される(デフォルト判定は 0 / 非0 の2値。skipped は判定コマンドでのみ生じる。ADR-008) |
| TC-exec-tick-121 | Tick 手続きD: 観測・判定(Running) > エッジケース: tick を順に実行する(failed → 再実行 → completed(または skipped)が確定する) | spec/testcases/execution/tick.md#エッジケース-4 | attempt_count・judge_attempt_count が 0 にリセットされる(連続失敗のみを数える。散発的な一過性失敗の蓄積で凍結しない。ADR-009) |
| TC-exec-tick-122 | Tick 手続きD: 観測・判定(Running) > エッジケース: tick を実行する(判定失敗の後に failed が確定する) | spec/testcases/execution/tick.md#エッジケース-4 | judge_attempt_count はリセットされる(判定自体は成立しているため) |
| TC-exec-tick-123 | Tick 手続きD: 観測・判定(Running) > エッジケース: 次の tick を実行する(skipped が確定した(pending 復帰済み)) | spec/testcases/execution/tick.md#エッジケース-4 | 同じ exit ファイルが再判定されることはなく、同じタスクステータスの実行が新しい attempt で起動される(周回。通知なし) |
| TC-exec-tick-124 | Tick 手続きD: 観測・判定(Running) > エッジケース: tick を実行する(無効化マーカーを見て未起動終了したラッパー(running 取込済み・exit なし…) | spec/testcases/execution/tick.md#エッジケース-4 | 「exitなし・プロセス死亡」として failed に分類される |
| TC-exec-tick-125 | Tick 手続きD: 観測・判定(Running) > エッジケース: tick を実行する(エージェントがシグナル死し exit に 128+n が記録されている) | spec/testcases/execution/tick.md#エッジケース-4 | デフォルト判定で failed(非0)に分類される |
| TC-exec-tick-126 | Tick 手続きE: runディレクトリの gc(`run_retention` 設定時のみ) > 正常系: tick を実行する(`run_retention` 未設定・保持期間相当を超えた attempt がある) | spec/testcases/execution/tick.md#正常系-5 | gc は行われない(明示オプトイン。ADR-011) |
| TC-exec-tick-127 | Tick 手続きE: runディレクトリの gc(`run_retention` 設定時のみ) > 正常系: tick を実行する(`run_retention` 設定済み・非保護の attempt の…) | spec/testcases/execution/tick.md#正常系-5 | 当該 attempt が削除され、`gc_deleted` に記録される |
| TC-exec-tick-128 | Tick 手続きE: runディレクトリの gc(`run_retention` 設定時のみ) > 正常系: tick を実行する(現役タスク(stopped 以外)が現在参照している attempt が期間超過) | spec/testcases/execution/tick.md#正常系-5 | 削除されない(`ActiveCurrent` 保護) |
| TC-exec-tick-129 | Tick 手続きE: runディレクトリの gc(`run_retention` 設定時のみ) > 正常系: tick を実行する(現役タスクの過去 attempt(現在参照外)が期間超過) | spec/testcases/execution/tick.md#正常系-5 | 削除される |
| TC-exec-tick-130 | Tick 手続きE: runディレクトリの gc(`run_retention` 設定時のみ) > 正常系: tick を実行する(stopped タスクの attempt(現在参照外を含む)が期間超過) | spec/testcases/execution/tick.md#正常系-5 | 全 attempt が削除されない(`AllProtected`。調査材料の保護) |
| TC-exec-tick-131 | Tick 手続きE: runディレクトリの gc(`run_retention` 設定時のみ) > 正常系: tick を実行する(アーカイブ済みタスクの attempt が期間超過) | spec/testcases/execution/tick.md#正常系-5 | 保護されず削除される |
| TC-exec-tick-132 | Tick 手続きE: runディレクトリの gc(`run_retention` 設定時のみ) > 正常系: tick を実行する(タスクファイルの存在しない孤児の runディレクトリが期間超過) | spec/testcases/execution/tick.md#正常系-5 | `Unprotected` として削除される(TaskId にパースできないディレクトリ名も対象) |
| TC-exec-tick-133 | Tick 手続きE: runディレクトリの gc(`run_retention` 設定時のみ) > 正常系: tick を実行する(あるタスクの attempt がすべて削除された) | spec/testcases/execution/tick.md#正常系-5 | 空になった `state/runs/<task-id>/` も削除される |
| TC-exec-tick-134 | Tick 手続きE: runディレクトリの gc(`run_retention` 設定時のみ) > 異常系: tick を実行する(`list_runs` が Io エラーを返す) | spec/testcases/execution/tick.md#異常系-5 | gc のみ中止して `gc_errors` に報告する。タスク処理は完了しているため tick は 0 |
| TC-exec-tick-135 | Tick 手続きE: runディレクトリの gc(`run_retention` 設定時のみ) > 異常系: tick を実行する(`delete_attempt` が失敗する(ログを開いているプロセスがいる等)) | spec/testcases/execution/tick.md#異常系-5 | 当該 attempt をスキップして `gc_errors` に報告する。どのタスクのカウンタも消費せず stopped も発生しない。次tickが再試行する |
| TC-exec-tick-136 | Tick 手続きE: runディレクトリの gc(`run_retention` 設定時のみ) > 境界値: tick を実行する(now - last_activity がちょうど retention に等しい) | spec/testcases/execution/tick.md#境界値-4 | 削除されない(`now - last_activity > retention` のみ削除対象) |
| TC-exec-tick-137 | Tick 手続きE: runディレクトリの gc(`run_retention` 設定時のみ) > 境界値: tick を実行する(now - last_activity が retention をわずかに超える) | spec/testcases/execution/tick.md#境界値-4 | 削除される |
| TC-exec-tick-138 | Tick 手続きE: runディレクトリの gc(`run_retention` 設定時のみ) > 境界値: tick を実行する(ファイルが1つもない attempt ディレクトリ) | spec/testcases/execution/tick.md#境界値-4 | ディレクトリ自体の最終更新時刻で経過を判定する |
| TC-exec-tick-139 | Tick 手続きE: runディレクトリの gc(`run_retention` 設定時のみ) > エッジケース: tick を実行する(パース不能(`Corrupt`)なタスクファイルが tasks に存在する) | spec/testcases/execution/tick.md#エッジケース-5 | ファイル名主部(`<task-id>.json` の `<task-id>`)をキーに `AllProtected` となり、全 attempt が保護される(読めない帳簿に対して何もしない) |
| TC-exec-tick-140 | Tick 手続きE: runディレクトリの gc(`run_retention` 設定時のみ) > エッジケース: tick を実行する(`SnapshotUnreadable`(DegradedTask)・stopped…) | spec/testcases/execution/tick.md#エッジケース-5 | 通常規則(`ActiveCurrent`)が適用される(実行状態と現在attempt参照は読めるため) |
| TC-exec-tick-141 | Tick 手続きE: runディレクトリの gc(`run_retention` 設定時のみ) > エッジケース: tick を実行する(`SnapshotUnreadable` かつ stopped のタスク) | spec/testcases/execution/tick.md#エッジケース-5 | `AllProtected` |
| TC-exec-tick-142 | Tick 手続きE: runディレクトリの gc(`run_retention` 設定時のみ) > エッジケース: tick を実行する(このtick(同一パス)で stopped に凍結されたタスク) | spec/testcases/execution/tick.md#エッジケース-5 | 保護マップは各タスクの処理後の状態から構築されるため `AllProtected` になる(凍結した tick 自身の gc が調査材料を消さない) |
| TC-exec-tick-143 | Tick 手続きE: runディレクトリの gc(`run_retention` 設定時のみ) > エッジケース: tick を実行する(このtickでアーカイブ完了したタスク) | spec/testcases/execution/tick.md#エッジケース-5 | `Unprotected` として扱われる |
| TC-exec-tick-144 | Tick 手続きE: runディレクトリの gc(`run_retention` 設定時のみ) > エッジケース: tick を実行する(このtickで `save` に失敗したタスク) | spec/testcases/execution/tick.md#エッジケース-5 | `AllProtected` として扱う(メモリ上と永続上のどちらが真か確定できないため保守側に倒す) |
| TC-exec-tick-145 | Tick 手続きE: runディレクトリの gc(`run_retention` 設定時のみ) > エッジケース: tick を実行する(`attempt-<n>` 形式に合致しないエントリ(手動配置ファイル…) | spec/testcases/execution/tick.md#エッジケース-5 | 列挙対象外で触れない。その残存により親ディレクトリが削除できないことは許容される |
| TC-exec-tick-146 | Tick 手続きE: runディレクトリの gc(`run_retention` 設定時のみ) > エッジケース: tick を実行する(skipped・spawn失敗で pending 復帰したタスク) | spec/testcases/execution/tick.md#エッジケース-5 | `current_attempt` の参照は保持されており(launching 記録でのみ置き換わる)、`ActiveCurrent` の保護対象が決定的に定まる |
| TC-exec-tick-147 | Tick 共通手続き: 凍結の確定と通知(notify) > 正常系: tick を実行する(stopped(notified_at なし)・notify_cmd 定義あり…) | spec/testcases/execution/tick.md#正常系-6 | `TASK_ID` / `WORKFLOW` / `TASK_STATUS` の環境変数と NOTIFY_TIMEOUT(組み込み60秒)で notify_cmd が直接起動され、成功後に `mark_notified` → `save` で notified_at が記録される。`notified` に記録 |
| TC-exec-tick-148 | Tick 共通手続き: 凍結の確定と通知(notify) > 正常系: tick を実行する(notify_cmd 未定義(None)の stopped(notified_at なし)) | spec/testcases/execution/tick.md#正常系-6 | 通知を行わず、notified_at も書かない(「通知した」という虚偽の記録を作らない) |
| TC-exec-tick-149 | Tick 共通手続き: 凍結の確定と通知(notify) > 正常系: tick を実行する(notified_at 記録済みの stopped) | spec/testcases/execution/tick.md#正常系-6 | 通知しない(再通知は未通知のもののみ) |
| TC-exec-tick-150 | Tick 共通手続き: 凍結の確定と通知(notify) > 正常系: tick を実行する(このtickの手続き中で上限超過により stopped になった) | spec/testcases/execution/tick.md#正常系-6 | `save` 直後に同一 tick 内で notify が実行される(次tickを待たない)。`frozen` / `notified` に記録される |
| TC-exec-tick-151 | Tick 共通手続き: 凍結の確定と通知(notify) > 異常系: tick を実行する(notify_cmd が非0で終了する) | spec/testcases/execution/tick.md#異常系-6 | notified_at を書かずに終える。次のtickが再通知する(at-least-once) |
| TC-exec-tick-152 | Tick 共通手続き: 凍結の確定と通知(notify) > 異常系: tick を実行する(notify_cmd が NOTIFY_TIMEOUT(60秒)を超過する) | spec/testcases/execution/tick.md#異常系-6 | 通知失敗として notified_at を書かない。次のtickが再通知する(ADR-018) |
| TC-exec-tick-153 | Tick 共通手続き: 凍結の確定と通知(notify) > 異常系: tick を実行する(notify_cmd が起動できない(`FailedToStart`)) | spec/testcases/execution/tick.md#異常系-6 | 通知失敗として notified_at を書かない。次のtickが再通知する |
| TC-exec-tick-154 | Tick 共通手続き: 凍結の確定と通知(notify) > 異常系: tick を実行する(`mark_notified` 後の `save` が失敗する) | spec/testcases/execution/tick.md#異常系-6 | notified_at が永続化されず、次のtickが再通知する(二重通知は許容) |
| TC-exec-tick-155 | Tick 共通手続き: 凍結の確定と通知(notify) > エッジケース: 次の tick を実行する(stopped の記録と通知の間で前回の処理がクラッシュした) | spec/testcases/execution/tick.md#エッジケース-6 | 「notified_at のない stopped」が検出され、同じ判断が再導出されて通知される |
| TC-exec-tick-156 | Tick 共通手続き: 凍結の確定と通知(notify) > エッジケース: 次の tick を実行する(notify_cmd 実行成功と notified_at 追記の間でクラッシュした) | spec/testcases/execution/tick.md#エッジケース-6 | 再通知される(二重通知は許容。欠落を許さない) |
| TC-exec-tick-157 | Tick 共通手続き: 凍結の確定と通知(notify) > エッジケース: 次の tick を実行する(notify_cmd 未定義のまま凍結したタスクがあり、後から notify_cmd…) | spec/testcases/execution/tick.md#エッジケース-6 | notified_at のない stopped が検出され、catch-up 通知される |
| TC-exec-tick-158 | Tick 共通手続き: 凍結の確定と通知(notify) > エッジケース: tick を実行する(DegradedTask(スナップショット破損)の stopped…) | spec/testcases/execution/tick.md#エッジケース-6 | 再通知が行われ、成功時は `save_degraded` で notified_at が永続化される(通知は定義非依存。at-least-once を破損時も維持) |
| TC-exec-tick-159 | Tick 共通手続き: 凍結の確定と通知(notify) > エッジケース: tick を実行する(未通知のまま retry / set-status で stopped を離脱したタスク) | spec/testcases/execution/tick.md#エッジケース-6 | 通知されない(stopped でないため対象外。人間が操作した = 気づいている) |
| TC-port-clock-001 | Clock: `now` | spec/testcases/ports/clock.md | `Timestamp` が返る。秒精度の UTC 日時であり、サブ秒成分を持たない |
| TC-port-clock-002 | Clock: `now` の返値を RFC3339 文字列に直列化して再パースする | spec/testcases/ports/clock.md | 元の `Timestamp` と等価(タスクファイルの直列化表現との往復) |
| TC-port-clock-003 | Clock: 呼び出しの直前・直後に外部で観測した実時刻と比較する(呼び出しの前後で外部の実時刻を観測できる状態(システム時計を参照し…) | spec/testcases/ports/clock.md | `now` の値が前後の観測時刻の範囲に収まる(秒精度での整合。呼び出し時点の壁時計を返す) |
| TC-port-clock-004 | Clock: 先の `now` の値を保持したまま再度 `now`(時刻が進んだ状態(実時間の経過、または時刻を制御できるアダプターでの前進)) | spec/testcases/ports/clock.md | 後の値は先の値より後の時刻になり、`elapsed_since` で経過秒数が導出できる |
| TC-port-clock-005 | Clock: `now`(時刻を先の `now` より過去に巻き戻した状態(時刻を過去に設定できるアダプター環境に限…) | spec/testcases/ports/clock.md | 設定どおりの(先の値より過去の)時刻が返る(契約は単調性を要求しないため、巻き戻った値を返すことは契約違反ではない) |
| TC-port-command-runner-001 | CommandRunner: `run(cmd, [], None)`(exit 0 で終了するテスト用コマンド) | spec/testcases/ports/command-runner.md | `Exited(ExitCode(0))` |
| TC-port-command-runner-002 | CommandRunner: `run(cmd, [], None)`(非0(例: 5)で終了するテスト用コマンド) | spec/testcases/ports/command-runner.md | `Exited(ExitCode(5))` |
| TC-port-command-runner-003 | CommandRunner: `run(cmd, [], None)`(存在しないコマンド名) | spec/testcases/ports/command-runner.md | `FailedToStart`(`Exited` の非0 とは区別される) |
| TC-port-command-runner-004 | CommandRunner: `run(cmd, [], None)`(実行できない実体(実行権限がないファイル等)) | spec/testcases/ports/command-runner.md | `FailedToStart` |
| TC-port-command-runner-005 | CommandRunner: `run(cmd, [], None)`(実行中に外部から強制終了される(exit code を持たない終了をする)コマンド) | spec/testcases/ports/command-runner.md | 非0 の符号化値(POSIX慣例では 128+シグナル番号)の `Exited(ExitCode)` を返す(`run_agent` と同じ符号化規則。`TimedOut` / `FailedToStart` にならない) |
| TC-port-command-runner-006 | CommandRunner: `run(cmd, [], None)`(シェルのメタ文字(`*`・`$VAR`・`&&`・リダイレクト記号等)を引数トークンに含め…) | spec/testcases/ports/command-runner.md | `Exited(0)`(引数が展開・連結・解釈されずそのまま渡る = シェル機能が効かない直接起動) |
| TC-port-command-runner-007 | CommandRunner: `run(cmd, [], None)`(`{input}` 等のプレースホルダ文字列を引数トークンに含めた検査コマンド) | spec/testcases/ports/command-runner.md | `Exited(0)`(プレースホルダ展開は行われず文字どおり渡る) |
| TC-port-command-runner-008 | CommandRunner: `run(cmd, [], None)`(呼び出しプロセスに環境変数を設定し、その値を検査するコマンド。`env` 引数は空) | spec/testcases/ports/command-runner.md | `Exited(0)`(呼び出しプロセスの環境が継承される) |
| TC-port-command-runner-009 | CommandRunner: `run(cmd, env, None)`(`env` 引数で新しい変数を与え、その値を検査するコマンド) | spec/testcases/ports/command-runner.md | `Exited(0)`(`env` の変数が追加される) |
| TC-port-command-runner-010 | CommandRunner: `run(cmd, env, None)`(呼び出しプロセスと `env` 引数で同名の変数に異なる値を設定し、`env`…) | spec/testcases/ports/command-runner.md | `Exited(0)`(`env` の値が継承環境を上書きする) |
| TC-port-command-runner-011 | CommandRunner: `run(cmd, [], None)`(呼び出しプロセスの作業ディレクトリと一致するか検査するコマンド) | spec/testcases/ports/command-runner.md | `Exited(0)`(作業ディレクトリは呼び出しプロセスの cwd のまま。ポートは変更しない) |
| TC-port-command-runner-012 | CommandRunner: `run(cmd, [], Some…(timeout 指定より長く実行し続けるコマンド) | spec/testcases/ports/command-runner.md | `TimedOut`。起動されたプロセスは終了させられている(timeout 後に生存していない) |
| TC-port-command-runner-013 | CommandRunner: `run(cmd, [], Some(timeout))`(timeout 内に exit 0 で終了するコマンド) | spec/testcases/ports/command-runner.md | `Exited(0)`(`TimedOut` にならない) |
| TC-port-command-runner-014 | CommandRunner: `run(cmd, [], None)`(一定時間実行してから終了するコマンド・timeout 未指定) | spec/testcases/ports/command-runner.md | 終了まで待って `Exited`(timeout による打ち切りは発生しない) |
| TC-port-command-runner-015 | CommandRunner: `run(cmd, [], None)`(終了直前に完了の証跡(ファイル等)を残すコマンド) | spec/testcases/ports/command-runner.md | `run` の呼び出しはコマンド終了まで戻らず、戻った時点で証跡が観測できる(同期実行) |
| TC-port-command-runner-016 | CommandRunner: `run(cmd, [], None)`(標準出力・標準エラーへ既知の文字列を出力するコマンド) | spec/testcases/ports/command-runner.md | `CommandCompletion` に出力は含まれず、出力は呼び出しプロセスの標準出力・標準エラーへそのまま流れる(捕捉しない) |
| TC-port-config-store-001 | ConfigStore 正常系とデフォルト値: `load`(全キー(`agents` / `notify_cmd` / `judge_attempt_…) | spec/testcases/ports/config-store.md#正常系とデフォルト値 | `Ok(GlobalConfig)`。記述した各値がフィールドに反映される |
| TC-port-config-store-002 | ConfigStore 正常系とデフォルト値: `load`(キーを1つも持たない(空マッピングの)config.yaml を置く) | spec/testcases/ports/config-store.md#正常系とデフォルト値 | `Ok`。デフォルト値が適用される: `agents` = 空マップ、`notify_cmd` = None、`judge_attempt_limit` = 3、`judge_timeout` = 60s、`spawn_fail_limit` = 3、`run_retention` = None |
| TC-port-config-store-003 | ConfigStore 正常系とデフォルト値: `load`(完全に空の config.yaml(空ファイル・null ドキュメント)を置く) | spec/testcases/ports/config-store.md#正常系とデフォルト値 | `Ok`。「全キー省略」として空マッピングと同じ全デフォルトが適用される(`Err(Invalid)` にしない) |
| TC-port-config-store-004 | ConfigStore 正常系とデフォルト値: `load`(一部のキーのみ(例: `spawn_fail_limit: 5`)を記述した…) | spec/testcases/ports/config-store.md#正常系とデフォルト値 | `Ok`。記述したキーはその値、未記述のキーはデフォルト値 |
| TC-port-config-store-005 | ConfigStore 正常系とデフォルト値: `load`(期間キーに単位の異なる等価値を記述する(例: `judge_timeout: 1m`)) | spec/testcases/ports/config-store.md#正常系とデフォルト値 | `Ok`。`DurationSpec` は秒数に正規化され、`1m` は `60s` と等価 |
| TC-port-config-store-006 | ConfigStore 正常系とデフォルト値: `load`(`agents` の `cmd` を文字列形式(連続空白・クォートを含む。例: `sh…) | spec/testcases/ports/config-store.md#正常系とデフォルト値 | `Ok`。単純な空白分割(連続空白は1区切り)でトークン化され、クォートはグルーピングとして解釈されない(`"echo` と `hi"` は別トークン) |
| TC-port-config-store-007 | ConfigStore 正常系とデフォルト値: `load`(`agents` の `cmd` を配列形式(空文字列トークンを含む)で記述する) | spec/testcases/ports/config-store.md#正常系とデフォルト値 | `Ok`。各要素がそのままトークンになり、空文字列トークンは配列形式でのみ許容される |
| TC-port-config-store-008 | ConfigStore 正常系とデフォルト値: それぞれ `load`(`notify_cmd` を文字列形式で記述した config と配列形式で記述した…) | spec/testcases/ports/config-store.md#正常系とデフォルト値 | いずれも `Ok`。`RawCommand` と同じ生成規則でトークン化され `Some(PlainCommand)` になる |
| TC-port-config-store-009 | ConfigStore 参照時検証の原則(内容は load で検証しない): `load`(`cmd` に未知プレースホルダ(例: `{foo}`)を含むエージェント定義を記述する) | spec/testcases/ports/config-store.md#参照時検証の原則内容は-load-で検証しない | `Ok`。テンプレート内容は load では検証されず、`RawAgentDefinition` として保持される(検証は参照時の `RawAgentDefinition::parse`) |
| TC-port-config-store-010 | ConfigStore 参照時検証の原則(内容は load で検証しない): `load`(`cmd` に波括弧不正(対応する `}` のない `{`、空の…) | spec/testcases/ports/config-store.md#参照時検証の原則内容は-load-で検証しない | `Ok`(同上。壊れたテンプレートを含む config でも load は通る) |
| TC-port-config-store-011 | ConfigStore 参照時検証の原則(内容は load で検証しない): `load`(`skill_input` に `{skill}` 以外のプレースホルダ(例:…) | spec/testcases/ports/config-store.md#参照時検証の原則内容は-load-で検証しない | `Ok`(同上) |
| TC-port-config-store-012 | ConfigStore 参照時検証の原則(内容は load で検証しない): `load`(`notify_cmd` のトークンに波括弧(例: `{task}`)を含める) | spec/testcases/ports/config-store.md#参照時検証の原則内容は-load-で検証しない | `Ok`。`PlainCommand` はプレースホルダ検査を行わず、文字どおり保持される |
| TC-port-config-store-013 | ConfigStore エラー: `load`(config.yaml が存在しない) | spec/testcases/ports/config-store.md#エラー | `Err(NotFound)`。解決後のグローバルホームパスを含む |
| TC-port-config-store-014 | ConfigStore エラー: `load`(YAML として不正な内容(構文エラー)の config.yaml を置く) | spec/testcases/ports/config-store.md#エラー | `Err(Invalid)`。message と location を含む |
| TC-port-config-store-015 | ConfigStore エラー: `load`(スキーマに無いトップレベルキー(例: `run_retension` のような…) | spec/testcases/ports/config-store.md#エラー | `Err(Invalid)`(未知キーは構造エラー。ADR-013) |
| TC-port-config-store-016 | ConfigStore エラー: `load`(組み込み定数に相当するキー(`retries` / `timeout`…) | spec/testcases/ports/config-store.md#エラー | `Err(Invalid)`(リトライ上限・timeout のデフォルトは config.yaml のキーではない。ADR-014) |
| TC-port-config-store-017 | ConfigStore エラー: `load`(`agents` のエントリ内に `cmd` / `skill_input`…) | spec/testcases/ports/config-store.md#エラー | `Err(Invalid)` |
| TC-port-config-store-018 | ConfigStore エラー: `load`(`agents` のエントリに `cmd` キーが無い(例: `skill_input`…) | spec/testcases/ports/config-store.md#エラー | `Err(Invalid)`(`cmd` は必須。キーの有無は構造であり読み込み時に検証する — 参照時まで遅延しない) |
| TC-port-config-store-019 | ConfigStore エラー: `load`(型不一致(例: `judge_attempt_limit` に文字列、`agents`…) | spec/testcases/ports/config-store.md#エラー | `Err(Invalid)` |
| TC-port-config-store-020 | ConfigStore エラー: `load`(`judge_attempt_limit: 0` または `spawn_fail_limi…) | spec/testcases/ports/config-store.md#エラー | `Err(Invalid)`(1 以上の制約) |
| TC-port-config-store-021 | ConfigStore エラー: `load`(期間形式の不正(`0s`・未知の単位・単位なし・空白混入)を `judge_timeout…) | spec/testcases/ports/config-store.md#エラー | `Err(Invalid)`(`DurationError` 相当の構造エラー) |
| TC-port-config-store-022 | ConfigStore エラー: `load`(`agents` の `cmd` に空文字列または空配列、あるいは…) | spec/testcases/ports/config-store.md#エラー | `Err(Invalid)`(トークン0個。`CommandError::Empty` 相当) |
| TC-port-config-store-023 | ConfigStore エラー: `load`(config.yaml が存在するが読み取れない(権限不足等。再現できるアダプター環境に限…) | spec/testcases/ports/config-store.md#エラー | `Err(Io)`。message を含む |
| TC-port-config-store-024 | ConfigStore 可視性: 再度 `load`(有効な config.yaml で `load` に成功した後…) | spec/testcases/ports/config-store.md#可視性 | 置き換え後の内容が返る(キャッシュしない。呼び出し時点のファイル内容。グローバル設定は各実行時に解決される) |
| TC-port-exclusive-lock-001 | ExclusiveLock: `try_acquire()`(誰もロックを保持していない) | spec/testcases/ports/exclusive-lock.md | `Ok(Some(LockGuard))` |
| TC-port-exclusive-lock-002 | ExclusiveLock: `try_acquire()`(別プロセスが同一グローバルホームのロックを保持中) | spec/testcases/ports/exclusive-lock.md | `Ok(None)`(プロセス間の排他が成立する。エラーではない) |
| TC-port-exclusive-lock-003 | ExclusiveLock: `try_acquire()`(別プロセスがロックを保持し続けている) | spec/testcases/ports/exclusive-lock.md | 即座に `Ok(None)` が返る(解放を待ってブロックしない) |
| TC-port-exclusive-lock-004 | ExclusiveLock: 別プロセスから `try_acquire()`(`LockGuard` を取得後にドロップ済み) | spec/testcases/ports/exclusive-lock.md | `Ok(Some(LockGuard))`(ドロップで解放され、別プロセスが取得できる。同一プロセス内の再取得では検証しない — 対象外参照) |
| TC-port-exclusive-lock-005 | ExclusiveLock: 別プロセスから `try_acquire()`(ロックを保持したままのテスト用プロセスを強制終了(解放処理を実行させない)) | spec/testcases/ports/exclusive-lock.md | `Ok(Some(LockGuard))`(保持プロセスの異常終了でも解放される) |
| TC-port-exclusive-lock-006 | ExclusiveLock: `try_acquire()`(異なるグローバルホームのロックを別ハンドルが保持中) | spec/testcases/ports/exclusive-lock.md | `Ok(Some(LockGuard))`(ロックはグローバルホームごとに1つで、他のホームの保持に影響されない) |
| TC-port-exclusive-lock-007 | ExclusiveLock: `try_acquire()`(ロック機構自体が利用不能な状況(ロックの置き場を用意できない等。当該状況を再現できるアダプ…) | spec/testcases/ports/exclusive-lock.md | `Err(LockError::Failed { message })`(「取得できなかった」の `Ok(None)` と区別される) |
| TC-port-process-controller-001 | ProcessController: `spawn_wrapper(spec)`(`prepare_attempt` 済みの run_dir・実在するworktree…) | spec/testcases/ports/process-controller.md | `Ok(())`。起動後の成否は戻り値に現れない(観測はrunディレクトリ経由) |
| TC-port-process-controller-002 | ProcessController: 呼び出し側プロセスを終了させ、別プロセスからrunディレクト…(一定時間実行し続けるテスト用コマンドを spec に与えて `spawn_wrapper`…) | spec/testcases/ports/process-controller.md | ラッパーは呼び出し側の終了後も生存して実行を完了する(runディレクトリに starttime・pid・exit が揃う)(デタッチ性) |
| TC-port-process-controller-003 | ProcessController: `spawn_wrapper(spec)`(ラッパーの起動自体が不可能な状況(起動対象を実行できない等。当該状況を再現できるアダプター…) | spec/testcases/ports/process-controller.md | `Err(SpawnError::Failed { message })`。runディレクトリ・プロセスに副作用を残さない |
| TC-port-process-controller-004 | ProcessController: `own_identity()`(テストプロセス自身) | spec/testcases/ports/process-controller.md | `Ok(WrapperIdentity)`。`pid` は自プロセスのPID、`kill_ident` は非空、`starttime.wall` は呼び出し前後に取得した時刻の範囲内 |
| TC-port-process-controller-005 | ProcessController: `own_identity()`(自プロセスの同定情報の取得機構自体が失敗する状況(当該状況を再現できるアダプター環境…) | spec/testcases/ports/process-controller.md | `Err(Io)` を値として返す(パニックしない。不正な同定情報で `Ok` を装わない) |
| TC-port-process-controller-006 | ProcessController: `starttime_of(自プロセスのPID)`(生存中の自プロセス) | spec/testcases/ports/process-controller.md | `Ok(Some(ProcessStartTime))` |
| TC-port-process-controller-007 | ProcessController: `starttime_of(pid)`(終了を確認済みのプロセスのPID) | spec/testcases/ports/process-controller.md | `Ok(None)`(不在 = 死亡) |
| TC-port-process-controller-008 | ProcessController: `starttime_of(pid)` を2回呼ぶ(生存中の同一プロセス) | spec/testcases/ports/process-controller.md | 両方 `Ok(Some)` で値が等価(等価比較に使える安定値) |
| TC-port-process-controller-009 | ProcessController: `starttime_of(own_identity の…(`own_identity()` の結果を保持) | spec/testcases/ports/process-controller.md | `Ok(Some)` で、`own_identity` の `starttime.ident` と等価(記録と照合が同一の取得手段で行われる) |
| TC-port-process-controller-010 | ProcessController: `starttime_of(pid)`(生存中のプロセスに対して起動時刻の取得機構自体が失敗する状況(取得手段への読み取り権限がな…) | spec/testcases/ports/process-controller.md | `Err(Io)`。プロセス不在の `Ok(None)`(= 死亡)と区別される(機構失敗を死亡に写像しない。写像すると生存プロセスの Dead 誤判定から failed → 再起動 → 同一worktreeの並走を招くため、呼び出し側は状態を変更せず再観測する) |
| TC-port-process-controller-011 | ProcessController: `kill(kill_ident)`(子プロセスを起動するテスト用コマンドを `spawn_wrapper` で起動し…) | spec/testcases/ports/process-controller.md | `Ok`。実行単位に属する全プロセス(ラッパー・エージェント・子プロセス)が終了する(`starttime_of` が各PIDで `None` になる)。呼び出し側のテストプロセスは影響を受けない(実行単位が分離されている) |
| TC-port-process-controller-012 | ProcessController: `kill(kill_ident)`(子プロセスを起動するテスト用コマンドを `spawn_wrapper` で起動した…) | spec/testcases/ports/process-controller.md | `Ok`。実行単位に属する全プロセスが終了する(`starttime_of` が各PIDで `None` になる)。プロセス内に保持したハンドルに依存せず、タスクファイルの情報だけで kill を実行できる(ツールの再起動後の kill) |
| TC-port-process-controller-013 | ProcessController: `kill(kill_ident)`(実行単位への終了操作自体が失敗する状況(当該状況を再現できるアダプター環境…) | spec/testcases/ports/process-controller.md | `Err(KillError::Failed { message })` を値として返す(パニックしない。分類・状態変更は呼び出し側が行わない前提の報告用エラー) |
| TC-port-process-controller-014 | ProcessController: `try_kill_remnants…(ラッパーのみ死亡し、エージェント(および子)が実行単位に属したまま生存) | spec/testcases/ports/process-controller.md | `Killed`。残存プロセスが終了する |
| TC-port-process-controller-015 | ProcessController: `try_kill_remnants…(対象を誤殺なく同定できない状況(実行単位の同定手段が失われている等。当該状況を再現できるア…) | spec/testcases/ports/process-controller.md | `NotIdentifiable`。いかなるプロセスも終了させない(無関係なプロセスの誤殺がない) |
| TC-port-process-controller-016 | ProcessController: `try_kill_remnants…(対象は同定できるが終了操作自体が失敗する状況(当該状況を再現できるアダプター環境…) | spec/testcases/ports/process-controller.md | `Failed { message }` を値として返す(パニックしない。呼び出し側の failed 分類には影響しない報告用) |
| TC-port-process-controller-017 | ProcessController: `run_agent(cmd, cwd, stdout,…(exit 0 で終了するテスト用コマンド・実在するworktree・書き込み可能なログパス) | spec/testcases/ports/process-controller.md | `ExitCode(0)` |
| TC-port-process-controller-018 | ProcessController: `run_agent(...)`(非0(例: 7)で終了するテスト用コマンド) | spec/testcases/ports/process-controller.md | `ExitCode(7)`(exit code をそのまま返す) |
| TC-port-process-controller-019 | ProcessController: `run_agent(cmd, cwd=worktree,…(自身の作業ディレクトリが worktree なら 0 を返す検査コマンド) | spec/testcases/ports/process-controller.md | `ExitCode(0)`(カレントディレクトリは常にworktree) |
| TC-port-process-controller-020 | ProcessController: `run_agent(...)`(標準出力・標準エラーへそれぞれ既知の文字列を出力するコマンド) | spec/testcases/ports/process-controller.md | 指定した stdout パスに標準出力の内容、stderr パスに標準エラーの内容が書かれている |
| TC-port-process-controller-021 | ProcessController: `run_agent(...)`(シェルのメタ文字(`*`・`$VAR`・`&&`・リダイレクト記号等)や空白を含む引数トー…) | spec/testcases/ports/process-controller.md | `ExitCode(0)`(引数がシェルに解釈されずそのまま渡る = シェルを介さない直接起動。requirements §3.1) |
| TC-port-process-controller-022 | ProcessController: `run_agent(...)`(存在しないコマンド名) | spec/testcases/ports/process-controller.md | `ExitCode(127)`(コマンド不在の符号化。エラー・パニックにならない) |
| TC-port-process-controller-023 | ProcessController: `run_agent(...)`(実行不能なファイル(実行権限がない等、起動できない実体)) | spec/testcases/ports/process-controller.md | `ExitCode(126)`(実行不能の符号化) |
| TC-port-process-controller-024 | ProcessController: `run_agent(...)`(実行中に外部から強制終了される(exit code を持たない終了をする)コマンド) | spec/testcases/ports/process-controller.md | 非0 の符号化値(POSIX慣例では 128+シグナル番号)の `ExitCode` を返す(常に値を返し、失敗しない) |
| TC-port-process-controller-025 | ProcessController: `run_agent(...)`(stdout のリダイレクト先が開けない(書き込み不能なパス等)) | spec/testcases/ports/process-controller.md | エージェントを起動せず `ExitCode(126)` を返す(エージェントの副作用が生じない) |
| TC-port-process-controller-026 | ProcessController: `run_agent(...)`(`cwd`(worktree)が存在しない(手動削除等。pages ※9)) | spec/testcases/ports/process-controller.md | 非0 の符号化値(起動不能 126 相当)の `ExitCode` を返す(常に値を返し、失敗しない — failed 経路への合流) |
| TC-port-process-controller-027 | ProcessController: `run_agent(...)`(一定時間実行してから終了するコマンド) | spec/testcases/ports/process-controller.md | 呼び出しはコマンド終了まで戻らず、終了後に `ExitCode` を返す(同期実行) |
| TC-port-run-store-001 | RunStore: `prepare_attempt(id, 1)`(runディレクトリ階層が未作成) | spec/testcases/ports/run-store.md | `Ok(RunDirPath)`。親を含めてattemptディレクトリが作成され、`attempt_exists` が true になる。返るパスは `RunDirPath::derive(state_root, id, 1)` の導出結果と一致する |
| TC-port-run-store-002 | RunStore: 同じ引数で再度 `prepare_attempt`(`prepare_attempt` 済みで、write系でファイルを書き込み済み) | spec/testcases/ports/run-store.md | `Ok`(冪等)。既存の書き込み済みファイルの内容に影響しない |
| TC-port-run-store-003 | RunStore: `read_pid_file(run_dir)`(`prepare_attempt` 済み・pidファイル未書き込み) | spec/testcases/ports/run-store.md | `Ok(None)`(ファイル不在) |
| TC-port-run-store-004 | RunStore: `read_pid_file(run_dir)`(attemptディレクトリ自体が不在) | spec/testcases/ports/run-store.md | `Ok(None)`(ディレクトリ不在もファイル不在と同様に扱う) |
| TC-port-run-store-005 | RunStore: `read_pid_file(run_dir)`(`write_pid_file(run_dir, {pid, kill_ident})`…) | spec/testcases/ports/run-store.md | `Ok(Some)`。pid・kill同定子とも書いた値と等しい(往復可能) |
| TC-port-run-store-006 | RunStore: `read_pid_file(run_dir)`(pidファイルの位置に解釈不能な内容を直接置いた状態) | spec/testcases/ports/run-store.md | `Err(RunFileError::Corrupt { path, message })`。不在(`Ok(None)`)と区別される |
| TC-port-run-store-007 | RunStore: `read_pid_file(run_dir)`(pidファイルは存在するが読み取り自体が失敗する状況(読み取り権限がない等。再現できるアダ…) | spec/testcases/ports/run-store.md | `Err(RunFileError::Io { message })`。内容不正の `Corrupt`・不在の `Ok(None)` と区別される |
| TC-port-run-store-008 | RunStore: `read_starttime(run_dir)`(`prepare_attempt` 済み・starttimeファイル未書き込み) | spec/testcases/ports/run-store.md | `Ok(None)` |
| TC-port-run-store-009 | RunStore: `read_starttime(run_dir)`(attemptディレクトリ自体が不在) | spec/testcases/ports/run-store.md | `Ok(None)` |
| TC-port-run-store-010 | RunStore: `read_starttime(run_dir)`(`write_starttime(run_dir, {ident, wall})` 済み) | spec/testcases/ports/run-store.md | `Ok(Some)`。ident・wall とも書いた値と等しい |
| TC-port-run-store-011 | RunStore: `read_starttime(run_dir)`(starttimeファイルの位置に解釈不能な内容を直接置いた状態) | spec/testcases/ports/run-store.md | `Err(RunFileError::Corrupt)`。不在と区別される |
| TC-port-run-store-012 | RunStore: `read_exit(run_dir)`(`prepare_attempt` 済み・exitファイル未書き込み) | spec/testcases/ports/run-store.md | `Ok(None)` |
| TC-port-run-store-013 | RunStore: `read_exit(run_dir)`(attemptディレクトリ自体が不在) | spec/testcases/ports/run-store.md | `Ok(None)` |
| TC-port-run-store-014 | RunStore: `read_exit(run_dir)`(`write_exit(run_dir, code)` 済み(0 と非0 の両方で確認)) | spec/testcases/ports/run-store.md | `Ok(Some)`。書いた `ExitCode` と等しい |
| TC-port-run-store-015 | RunStore: `read_exit(run_dir)`(exitファイルの位置に解釈不能な内容を直接置いた状態) | spec/testcases/ports/run-store.md | `Err(RunFileError::Corrupt)`。不在と区別される |
| TC-port-run-store-016 | RunStore: 対応する read系を並行して繰り返し呼ぶ(write系(`write_starttime` / `write_pid_file`…) | spec/testcases/ports/run-store.md | すべての読み取りが「不在」または「書き込まれたいずれかの完全な値」のみを観測する。書きかけ・新旧混合の内容は観測されない(アトミック置換。観測可能な範囲での検証) |
| TC-port-run-store-017 | RunStore: `Err(Io)` を観測した後、対応する read系を呼ぶ(write系の書き込みが途中で失敗する状況(当該状況を再現できるアダプター環境に限る)) | spec/testcases/ports/run-store.md | 「不在(`Ok(None)`)」または「従前の完全な値」のみを観測し、`Corrupt`(部分的な書きかけ)にはならない(失敗時もアトミック置換の非観測性が保たれる — 部分的な pid 等が残ると tick が `Corrupt` としてスキップし続け、通知されない launching 滞留を生むため) |
| TC-port-run-store-018 | RunStore: `write_invalidation_marker…(attemptディレクトリ自体が不在) | spec/testcases/ports/run-store.md | `Ok`。ディレクトリごと作成してマーカーを書く。`marker_exists` が true になる |
| TC-port-run-store-019 | RunStore: `write_invalidation_marker…(`write_invalidation_marker` 済み) | spec/testcases/ports/run-store.md | `Ok`(冪等)。`marker_exists` は true のまま |
| TC-port-run-store-020 | RunStore: `marker_exists(run_dir)`(`prepare_attempt` 済み・マーカー未書き込み) | spec/testcases/ports/run-store.md | `Ok(false)` |
| TC-port-run-store-021 | RunStore: `marker_exists(run_dir)`(`write_invalidation_marker` 済み) | spec/testcases/ports/run-store.md | `Ok(true)` |
| TC-port-run-store-022 | RunStore: `attempt_exists(run_dir)`(`prepare_attempt` 済みでファイルを1つも書いていない…) | spec/testcases/ports/run-store.md | `Ok(true)`(read系の `Ok(None)` では区別できない「空ディレクトリ」を「ディレクトリごと不在」と区別できる) |
| TC-port-run-store-023 | RunStore: `attempt_exists(run_dir)`(attemptディレクトリ自体が不在) | spec/testcases/ports/run-store.md | `Ok(false)` |
| TC-port-run-store-024 | RunStore: `list_runs()`(runディレクトリの格納領域(`state/runs/` 相当)自体が未作成) | spec/testcases/ports/run-store.md | `Ok(空の RunListing)` |
| TC-port-run-store-025 | RunStore: `list_runs()`(2タスク分のattempt(それぞれ attempt-1・attempt-2)を…) | spec/testcases/ports/run-store.md | 各タスクの `dir_name` と、attempt番号 1・2 の `AttemptInfo` がすべて列挙される |
| TC-port-run-store-026 | RunStore: `list_runs()`(attempt内に write系でファイルを時間差をおいて複数書き込み済み) | spec/testcases/ports/run-store.md | 当該attemptの `last_activity` がディレクトリ内ファイルの最終更新時刻の最大値(最後に書いたファイルの時刻)になる |
| TC-port-run-store-027 | RunStore: `list_runs()`(ファイルが1つもない空のattemptディレクトリ) | spec/testcases/ports/run-store.md | 当該attemptの `last_activity` がディレクトリ自体の最終更新時刻になる |
| TC-port-run-store-028 | RunStore: `list_runs()`(タスクディレクトリ配下に `attempt-<n>` 形式に合致しないエントリ…) | spec/testcases/ports/run-store.md | 形式外エントリは列挙されない(`attempt-<n>` 形式のattemptのみが列挙される) |
| TC-port-run-store-029 | RunStore: `list_runs()`(`TaskId` としてパースできない名前のタスクディレクトリ(孤児)配下に…) | spec/testcases/ports/run-store.md | `dir_name` が生文字列のまま列挙される(gcの孤児削除対象にできる) |
| TC-port-run-store-030 | RunStore: `delete_attempt(dir_name, n)`(複数attemptが存在し、うち1つにファイルがある) | spec/testcases/ports/run-store.md | `Ok`。当該attemptは `attempt_exists` が false になり `list_runs` から消える。他のattemptには影響しない |
| TC-port-run-store-031 | RunStore: `delete_attempt(dir_name, n)`(対象attemptの削除自体が失敗する状況(削除権限がない等。再現できるアダプター環境に限…) | spec/testcases/ports/run-store.md | `Err(Io)` を値として返す(パニックしない。失敗は呼び出し側がスキップ・報告する前提の報告用エラー) |
| TC-port-run-store-032 | RunStore: `remove_task_dir_if_empty…(attemptがすべて削除され空になったタスクディレクトリ) | spec/testcases/ports/run-store.md | `Ok`。タスクディレクトリが削除され、`list_runs` に `dir_name` が現れない |
| TC-port-run-store-033 | RunStore: `remove_task_dir_if_empty…(attemptが1つ以上残っているタスクディレクトリ) | spec/testcases/ports/run-store.md | 削除せず `Ok` を返す(非空はエラーではない)。残っているattemptに影響しない |
| TC-port-run-store-034 | RunStore: `remove_task_dir_if_empty…(タスクディレクトリに `attempt-<n>` 形式外のエントリのみが残存) | spec/testcases/ports/run-store.md | 親ディレクトリを削除せず `Ok` を返し、残存エントリにも触れない(ユーザーが置いたものを黙って消さない。非空はエラーではなく、残存が毎tick `gc_errors` に報告され続けない) |
| TC-port-task-id-generator-001 | TaskIdGenerator: `generate` | spec/testcases/ports/task-id-generator.md | 返値が `TaskId` の制約を満たす: 1〜64文字、使用文字は `[a-z0-9-]` のみ、先頭は英数字(文字列表現が `TaskId::parse` を通る) |
| TC-port-task-id-generator-002 | TaskIdGenerator: `generate` を多数回(例: 10,000回)呼び出す | spec/testcases/ports/task-id-generator.md | すべて制約を満たし、互いに重複しない(実用上の一意性) |
| TC-port-task-id-generator-003 | TaskIdGenerator: 時間間隔を置かず連続して `generate` する | spec/testcases/ports/task-id-generator.md | 重複しない(時刻成分のみに依存しない。同一時刻内の連続発行でも区別される) |
| TC-port-task-id-generator-004 | TaskIdGenerator: それぞれから `generate` する(同じ構成のジェネレーターを複数用意する) | spec/testcases/ports/task-id-generator.md | 互いに重複しない(インスタンスを跨いでも実用上の一意性が成り立つ) |
| TC-port-task-id-generator-005 | TaskIdGenerator: `generate` した ID からパス・ブランチ名を導出する | spec/testcases/ports/task-id-generator.md | `<worktree_root>/<id>`(worktreeパス)・`pulsen/<id>`(`BranchName::parse` を通る)・`<id>.json`(ファイル名主部)がいずれも有効になる(文字集合制約の帰結として常に安全) |
| TC-port-task-repository-001 | TaskRepository create: 新規 Task で `create`(状態ディレクトリに何も無い(`state/tasks/` 不在)) | spec/testcases/ports/task-repository.md#create | `Ok`。必要なディレクトリが自動作成され、`find` が `Active(Intact)` で同じ内容を返す |
| TC-port-task-repository-002 | TaskRepository create: 同じ ID の Task で `create`(`create` 済みの ID がある) | spec/testcases/ports/task-repository.md#create | `Err(Conflict)`(現役に存在。一意性はポートが担保し、呼び出し側の事前確認に依存しない) |
| TC-port-task-repository-003 | TaskRepository create: 同じ ID の Task で `create`(`create` → `archive` 済みの ID がある) | spec/testcases/ports/task-repository.md#create | `Err(Conflict)`(アーカイブに存在。一意性は現役・アーカイブ横断) |
| TC-port-task-repository-004 | TaskRepository create: 同じ ID の Task で `create`(`state/tasks/<task-id>.json` に JSON…) | spec/testcases/ports/task-repository.md#create | `Err(Conflict)`。既存ファイルの内容は変更されない(存在判定はデコード可否によらない。破損ファイルを上書きせず修復の材料を消さない) |
| TC-port-task-repository-005 | TaskRepository create: `create`(書き込み先を用意できない(`state/` が書き込み不能等。再現できるアダプター環境に限…) | spec/testcases/ports/task-repository.md#create | `Err(Io)`。message を含む |
| TC-port-task-repository-006 | TaskRepository save / save_degraded: 遷移後の値(実行状態・カウンタ・`updated_at`…(`create` 済みのタスク) | spec/testcases/ports/task-repository.md#save--save_degraded | `Ok`。直後の `find` が更新後の内容を返す(read-your-writes) |
| TC-port-task-repository-007 | TaskRepository save / save_degraded: `save`(`create` していない ID のタスク) | spec/testcases/ports/task-repository.md#save--save_degraded | `Err(NotFound)`(現役に存在しない) |
| TC-port-task-repository-008 | TaskRepository save / save_degraded: `save`(`create` → `archive` 済みのタスク) | spec/testcases/ports/task-repository.md#save--save_degraded | `Err(NotFound)`(アーカイブ側は `save` の対象外) |
| TC-port-task-repository-009 | TaskRepository save / save_degraded: タスク側フィールドを変更(`abort` による…(スナップショットフィールドのみを有効な JSON だがスナップショットとして解釈できない内容に書き換えたタスクファイルを `find` し…) | spec/testcases/ports/task-repository.md#save--save_degraded | `Ok`。直後の `find` は変更後のタスク側フィールドを持つ `SnapshotUnreadable` を返し、スナップショットフィールドは元の(破損した)内容のままファイルに温存される(往復。修復の材料を消さない) |
| TC-port-task-repository-010 | TaskRepository save / save_degraded: `save_degraded`(現役に存在しない ID の DegradedTask) | spec/testcases/ports/task-repository.md#save--save_degraded | `Err(NotFound)` |
| TC-port-task-repository-011 | TaskRepository save / save_degraded: `save`(`create` 済みのタスク。書き込み先へ書き込めない(`state/tasks/`…) | spec/testcases/ports/task-repository.md#save--save_degraded | `Err(Io)`。message を含む(部分的な書き込み結果を残さないことは「原子性の観測面」で検証) |
| TC-port-task-repository-012 | TaskRepository save / save_degraded: `save_degraded`(`find` で `SnapshotUnreadable(DegradedTask)`…) | spec/testcases/ports/task-repository.md#save--save_degraded | `Err(Io)`。message を含む |
| TC-port-task-repository-013 | TaskRepository 往復可能性(デコード): `find`(全 Optional フィールドを持つタスク(workspace 確定…) | spec/testcases/ports/task-repository.md#往復可能性デコード | `Active(Intact)`。スナップショットを含む全フィールドが元の値と等価(往復可能な保存) |
| TC-port-task-repository-014 | TaskRepository 往復可能性(デコード): それぞれ `find`(各実行状態(Pending / Launching / Running /…) | spec/testcases/ports/task-repository.md#往復可能性デコード | 各状態が付随データごと復元される(`Launching.recorded_at`、`Stopped.reason` / `notified_at` を含む) |
| TC-port-task-repository-015 | TaskRepository find と解決順: `find`(何も作成していない(`state/tasks/` / `state/archive/`…) | spec/testcases/ports/task-repository.md#find-と解決順 | `Ok(NotFound)`(ディレクトリ不在は空結果として扱う) |
| TC-port-task-repository-016 | TaskRepository find と解決順: `find`(`create` 済みのタスク) | spec/testcases/ports/task-repository.md#find-と解決順 | `Active(Intact(task))` |
| TC-port-task-repository-017 | TaskRepository find と解決順: `find`(`create` → `archive` 済みのタスク) | spec/testcases/ports/task-repository.md#find-と解決順 | `Archived(TaskRecord)` |
| TC-port-task-repository-018 | TaskRepository find と解決順: `find`(同一 ID のタスクファイルを `state/tasks/` と…) | spec/testcases/ports/task-repository.md#find-と解決順 | `Active` として返す(解決順は tasks → archive) |
| TC-port-task-repository-019 | TaskRepository find と解決順: `find`(走査対象を読み取れない(`state/tasks/` が読み取り不能等。再現できるアダプタ…) | spec/testcases/ports/task-repository.md#find-と解決順 | `Err(Io)`。message を含む(`Ok(NotFound)` / `Corrupt` に写像しない。機構失敗は値のエラーとして呼び出し側に届く) |
| TC-port-task-repository-020 | TaskRepository Corrupt と SnapshotUnreadable の区別: `find`(タスクファイル全体を JSON として不正な内容に置き換える) | spec/testcases/ports/task-repository.md#corrupt-と-snapshotunreadable-の区別 | `Corrupt { path, message }`(path は当該ファイル) |
| TC-port-task-repository-021 | TaskRepository Corrupt と SnapshotUnreadable の区別: `find`(タスク側フィールドの構文・値制約を破る(実行状態に未知の値、`task_id`…) | spec/testcases/ports/task-repository.md#corrupt-と-snapshotunreadable-の区別 | `Corrupt`(タスク側フィールドの破れはファイル全体の破損として扱う) |
| TC-port-task-repository-022 | TaskRepository Corrupt と SnapshotUnreadable の区別: `find`(スナップショットフィールドのみを有効な JSON だがスナップショットとして解釈できない内容に置き換える(タスク側フィールドは有効なまま…) | spec/testcases/ports/task-repository.md#corrupt-と-snapshotunreadable-の区別 | `Active(SnapshotUnreadable(DegradedTask))`。message に理由を含み、タスク側フィールド(実行状態・カウンタ・attempt 参照等)はすべて読める |
| TC-port-task-repository-023 | TaskRepository Corrupt と SnapshotUnreadable の区別: `find`(スナップショットフィールドを**削除**する(不在。タスク側フィールドは有効なまま)) | spec/testcases/ports/task-repository.md#corrupt-と-snapshotunreadable-の区別 | `Active(SnapshotUnreadable(DegradedTask))`(欠落も「スナップショットのみ読めない」に分類する。`Corrupt` に落とさない — pages 縮退表「スナップショット 不在・パース不能」) |
| TC-port-task-repository-024 | TaskRepository Corrupt と SnapshotUnreadable の区別: `find`(`task_status` を snapshot の statuses…) | spec/testcases/ports/task-repository.md#corrupt-と-snapshotunreadable-の区別 | `SnapshotUnreadable`(不変条件1の照合破れ。`RehydrateError::StatusNotInSnapshot` の写像) |
| TC-port-task-repository-025 | TaskRepository Corrupt と SnapshotUnreadable の区別: `find`(スナップショットの構造不変条件を破る(`initial ∉ statuses`、または…) | spec/testcases/ports/task-repository.md#corrupt-と-snapshotunreadable-の区別 | `SnapshotUnreadable` |
| TC-port-task-repository-026 | TaskRepository Corrupt と SnapshotUnreadable の区別: `find`(状態間整合の不変条件2〜4を破る内容(例: Running なのに…) | spec/testcases/ports/task-repository.md#corrupt-と-snapshotunreadable-の区別 | `Active(Intact)`(不変条件2〜4はデコードでは検証しない。遷移関数の前提検査(`TransitionError::MissingCurrentAttempt` 等)に委ねる) |
| TC-port-task-repository-027 | TaskRepository Corrupt と SnapshotUnreadable の区別: `find`(`state/archive/` に JSON として不正な内容のタスクファイルを置く…) | spec/testcases/ports/task-repository.md#corrupt-と-snapshotunreadable-の区別 | `Corrupt { path, message }`(path はアーカイブ側の当該ファイル。破損の区分は tasks / archive で変わらない) |
| TC-port-task-repository-028 | TaskRepository Corrupt と SnapshotUnreadable の区別: `find`(`state/archive/` にスナップショットフィールドのみ有効な JSON だがスナップショットとして解釈できない内容のタスクファイル…) | spec/testcases/ports/task-repository.md#corrupt-と-snapshotunreadable-の区別 | `Archived(SnapshotUnreadable(DegradedTask))`。message に理由を含み、タスク側フィールドはすべて読める |
| TC-port-task-repository-029 | TaskRepository Corrupt と SnapshotUnreadable の区別: `list_active`(上記の各破損フィクスチャのうち**現役側(`state/tasks/`)に置いたもの**) | spec/testcases/ports/task-repository.md#corrupt-と-snapshotunreadable-の区別 | `find` と同じ区分で列挙される(`Corrupt` は `TaskEntry::Corrupt`、スナップショット破損は `Record(SnapshotUnreadable)`)。アーカイブ側のフィクスチャは現れない |
| TC-port-task-repository-030 | TaskRepository Corrupt と SnapshotUnreadable の区別: `list_active`(`state/tasks/` に命名形式(`<task-id>.json`)に合致しないエ…) | spec/testcases/ports/task-repository.md#corrupt-と-snapshotunreadable-の区別 | 形式外エントリは列挙されない(`Corrupt` としても現れない)。既存タスクの走査には影響しない(RunStore の `attempt-<n>` 形式外と同じ規則) |
| TC-port-task-repository-031 | TaskRepository archive: `archive`(`create` 済みのタスク(`state/archive/` 不在)) | spec/testcases/ports/task-repository.md#archive | `Ok`。移動先ディレクトリが自動作成され、`find` は `Archived` を返す |
| TC-port-task-repository-032 | TaskRepository archive: `list_active` / `list_archived…(`archive` 直後) | spec/testcases/ports/task-repository.md#archive | 現役側(`list_active` と `find` の現役扱い)から即座に消え、アーカイブ側に現れる(read-your-writes)。内容は移動前と等価 |
| TC-port-task-repository-033 | TaskRepository archive: `archive`(`create` していない ID) | spec/testcases/ports/task-repository.md#archive | `Err(NotFound)` |
| TC-port-task-repository-034 | TaskRepository archive: 再度 `archive`(`archive` 済みの ID) | spec/testcases/ports/task-repository.md#archive | `Err(NotFound)`(現役に存在しない) |
| TC-port-task-repository-035 | TaskRepository archive: `archive`(移動先を用意できない(再現できるアダプター環境に限る)) | spec/testcases/ports/task-repository.md#archive | `Err(Io)`。タスクは現役側に完全な内容のまま残る(部分的な移動を残さない) |
| TC-port-task-repository-036 | TaskRepository list_active / list_archived: `list_active` / `list_archived…(走査対象ディレクトリが存在しない) | spec/testcases/ports/task-repository.md#list_active--list_archived | `Ok(空リスト)` |
| TC-port-task-repository-037 | TaskRepository list_active / list_archived: `list_active`(複数タスクを `create` し、うち1つを `archive` する) | spec/testcases/ports/task-repository.md#list_active--list_archived | archive していないタスクのみが `Record(Intact)` で全件列挙される |
| TC-port-task-repository-038 | TaskRepository list_active / list_archived: `list_archived`(同上) | spec/testcases/ports/task-repository.md#list_active--list_archived | archive したタスクのみが列挙される |
| TC-port-task-repository-039 | TaskRepository list_active / list_archived: `list_active`(現役に正常タスク・全体破損ファイル・スナップショットのみ破損のタスクを混在させる) | spec/testcases/ports/task-repository.md#list_active--list_archived | `Ok`。正常は `Record(Intact)`、全体破損は `Corrupt { path, message }`、スナップショット破損は `Record(SnapshotUnreadable)` としてすべて返り、個別の破損が走査全体を失敗させない |
| TC-port-task-repository-040 | TaskRepository list_active / list_archived: `list_archived`(アーカイブ側に正常タスク・全体破損ファイル・スナップショットのみ破損のタスクを混在させる…) | spec/testcases/ports/task-repository.md#list_active--list_archived | `Ok`。`list_active` と同じ区分ですべて返り、個別の破損が走査全体を失敗させない |
| TC-port-task-repository-041 | TaskRepository list_active / list_archived: `list_active` / `list_archived…(走査対象ディレクトリが存在するが読み取り不能(再現できるアダプター環境に限る)) | spec/testcases/ports/task-repository.md#list_active--list_archived | `Err(Io)`。message を含む(走査自体の失敗はエラー。`Ok(空リスト)` に写像しない — 写像すると tick の無言の停滞を招く) |
| TC-port-task-repository-042 | TaskRepository 原子性の観測面: 内容(スナップショットを含む全体)を大きく変える…(`create` 済みのタスク。別スレッド/プロセスから `find` /…) | spec/testcases/ports/task-repository.md#原子性の観測面 | すべての読み取りが、いずれかの完全な保存内容のみを観測する(フィールドの新旧混在・書きかけの内容が現れない。読み取りはロックなしで常に一貫した内容を返す) |
| TC-port-task-repository-043 | TaskRepository 原子性の観測面: `find` / `list_active`(`save` が `Err` を返した(NotFound / Io)) | spec/testcases/ports/task-repository.md#原子性の観測面 | 部分的な書き込み結果が残らない(対象は操作前の状態のまま、または NotFound のまま) |
| TC-port-task-repository-044 | TaskRepository 原子性の観測面: `archive` を実行する(`create` 済みのタスク。別スレッド/プロセスから `find` /…) | spec/testcases/ports/task-repository.md#原子性の観測面 | 移動中の反復読み取りが「現役とアーカイブの両方に現れる」「どちらにも完全体が無い」という中間状態を観測しない(常にどちらか一方の完全な内容のみ)。完了後は Archived のみ、失敗後は Active のみが観測される |
| TC-port-workflow-store-001 | WorkflowStore 名前解決: `load(Name("impl"))`(`workflows/impl.yaml` として有効な定義を置く) | spec/testcases/ports/workflow-store.md#名前解決 | `Ok(LoadedWorkflow)`。`resolved_from` = `<home>/workflows/impl.yaml` の絶対パス |
| TC-port-workflow-store-002 | WorkflowStore 名前解決: `load(Name("impl"))`(`workflows/impl.yml`(拡張子 `.yml`)のみを置く) | spec/testcases/ports/workflow-store.md#名前解決 | `Err(NotFound)`。`attempted` = `<home>/workflows/impl.yaml`(`.yml` へのフォールバックはしない) |
| TC-port-workflow-store-003 | WorkflowStore 名前解決: `load(Name("missing"))`(`workflows/` に該当ファイルが無い(ディレクトリ自体の不在を含む)) | spec/testcases/ports/workflow-store.md#名前解決 | `Err(NotFound)`。`attempted` = `<home>/workflows/missing.yaml` の絶対パス(add の案内用) |
| TC-port-workflow-store-004 | WorkflowStore 名前解決: `load(Path(絶対パス))`(任意の場所に有効な定義ファイルを置く) | spec/testcases/ports/workflow-store.md#名前解決 | `Ok`。`resolved_from` = そのパス |
| TC-port-workflow-store-005 | WorkflowStore 名前解決: `load(Path(相対パス))`(プロセスのカレントディレクトリからの相対位置に有効な定義ファイルを置く) | spec/testcases/ports/workflow-store.md#名前解決 | `Ok`。相対パスはカレントディレクトリから解決され、`resolved_from` は実際に読み込んだ絶対パス |
| TC-port-workflow-store-006 | WorkflowStore 名前解決: `load(Path(不在のパス))`(指定パスにファイルが無い) | spec/testcases/ports/workflow-store.md#名前解決 | `Err(NotFound)`。`attempted` = 解決を試みた絶対パス |
| TC-port-workflow-store-007 | WorkflowStore 正常系パース: `load`(`workflow:` キーを持つ有効な定義(`initial`・AgentRun…) | spec/testcases/ports/workflow-store.md#正常系パース | `Ok`。`parsed.declared_name` = `workflow:` の値、`definition` に initial・statuses が正規化されて入る。表示名の決定は行われない(呼び出し側の `WorkflowRef::display_name`) |
| TC-port-workflow-store-008 | WorkflowStore 正常系パース: `load`(トップレベルに `agent` / `model`(ワークフローデフォルト)を持つ有効な定…) | spec/testcases/ports/workflow-store.md#正常系パース | `Ok`。`definition.default_agent` / `default_model` に値が取り込まれる(黙って落とさない — 実効値解決(ステータス > ワークフローデフォルト)の入力になる) |
| TC-port-workflow-store-009 | WorkflowStore 正常系パース: `load`(`workflow:` キーの無い有効な定義を置く) | spec/testcases/ports/workflow-store.md#正常系パース | `Ok`。`declared_name` = None |
| TC-port-workflow-store-010 | WorkflowStore 正常系パース: `load`(`skill` 指定のステータスと、`agent` / `model` /…) | spec/testcases/ports/workflow-store.md#正常系パース | `Ok`。各値が対応するドメイン型(`TimeoutSpec`・`PlainCommand` 等)に落ちる |
| TC-port-workflow-store-011 | WorkflowStore 正常系パース: `load`(`timeout: none` を指定したステータスを含む定義を置く) | spec/testcases/ports/workflow-store.md#正常系パース | `Ok`。当該ステータスの timeout は `Unlimited` |
| TC-port-workflow-store-012 | WorkflowStore 正常系パース: `load`(`retries: 0` を指定したステータスを含む定義を置く) | spec/testcases/ports/workflow-store.md#正常系パース | `Ok`。0 は正当な値(0 = 初回失敗で即 stopped。config.yaml の「1 以上」の規則とは異なる) |
| TC-port-workflow-store-013 | WorkflowStore 正常系パース: `load`(`next` が自ステータスを指す(自己参照)定義・複数ステータスで循環を成す定義を置く) | spec/testcases/ports/workflow-store.md#正常系パース | `Ok`(循環・自己参照は正当な表現。ADR-010) |
| TC-port-workflow-store-014 | WorkflowStore 正常系パース: `load`(遷移経路のない到達不能ステータスを含む定義を置く) | spec/testcases/ports/workflow-store.md#正常系パース | `Ok`(到達不能ステータスは許容) |
| TC-port-workflow-store-015 | WorkflowStore 正常系パース: `load`(`judge` のトークンに波括弧(`{...}`)を含む定義を置く) | spec/testcases/ports/workflow-store.md#正常系パース | `Ok`。`PlainCommand` はプレースホルダ展開・検査をせず文字どおり保持される |
| TC-port-workflow-store-016 | WorkflowStore 正常系パース: `load`(グローバル設定に存在しないエージェント名を参照する定義を置く) | spec/testcases/ports/workflow-store.md#正常系パース | `Ok`(グローバル設定との突き合わせは本ポートの責務外。`RegistrationValidator` が担う) |
| TC-port-workflow-store-017 | WorkflowStore パースエラー(ADR-013 の全エラー種): `load`(YAML として不正な内容(構文エラー・重複キー)のファイルを置く) | spec/testcases/ports/workflow-store.md#パースエラーadr-013-の全エラー種 | `Err(Parse { error: YamlSyntax, resolved_from })`。message・location を含み、`resolved_from` は名前解決した絶対パス(`<workflows_dir>/wf.yaml`)。message に解決先を前置しない — パスを持たないのは `WorkflowParseError` 12種すべての契約であり、この行が固定するのは `YamlSyntax` の1経路 |
| TC-port-workflow-store-018 | WorkflowStore パースエラー(ADR-013 の全エラー種): `load`(トップレベルに許容外のキー(`workflow` / `agent` / `model`…) | spec/testcases/ports/workflow-store.md#パースエラーadr-013-の全エラー種 | `Err(Parse { error: UnknownKey, .. })` |
| TC-port-workflow-store-019 | WorkflowStore パースエラー(ADR-013 の全エラー種): `load`(ステータス内にスキーマ外のキー(`prmopt` 等の typo)を含む定義を置く) | spec/testcases/ports/workflow-store.md#パースエラーadr-013-の全エラー種 | `Err(Parse { error: UnknownKey, .. })` |
| TC-port-workflow-store-020 | WorkflowStore パースエラー(ADR-013 の全エラー種): `load`(`run: wait` / `run: cleanup` のステータスにエージェント実行系…) | spec/testcases/ports/workflow-store.md#パースエラーadr-013-の全エラー種 | `Err(Parse { error: ForbiddenKey, .. })`(`Wait` / `Cleanup` に許されるキーは `run` のみ) |
| TC-port-workflow-store-021 | WorkflowStore パースエラー(ADR-013 の全エラー種): `load`(`initial` キーの無い定義を置く) | spec/testcases/ports/workflow-store.md#パースエラーadr-013-の全エラー種 | `Err(Parse { error: MissingInitial, .. })` |
| TC-port-workflow-store-022 | WorkflowStore パースエラー(ADR-013 の全エラー種): `load`(`initial` が `statuses` に無い名前を指す定義を置く) | spec/testcases/ports/workflow-store.md#パースエラーadr-013-の全エラー種 | `Err(Parse { error: InitialNotFound, .. })` |
| TC-port-workflow-store-023 | WorkflowStore パースエラー(ADR-013 の全エラー種): `load`(`statuses` が空・欠落した定義を置く) | spec/testcases/ports/workflow-store.md#パースエラーadr-013-の全エラー種 | `Err(Parse { error: EmptyStatuses, .. })` |
| TC-port-workflow-store-024 | WorkflowStore パースエラー(ADR-013 の全エラー種): `load`(動作宣言(`prompt` / `skill` / `run`)の無いステータスを含む定義…) | spec/testcases/ports/workflow-store.md#パースエラーadr-013-の全エラー種 | `Err(Parse { error: NoAction, .. })` |
| TC-port-workflow-store-025 | WorkflowStore パースエラー(ADR-013 の全エラー種): `load`(動作宣言が複数あるステータス(`prompt` と `skill`、`prompt` と…) | spec/testcases/ports/workflow-store.md#パースエラーadr-013-の全エラー種 | `Err(Parse { error: MultipleActions, .. })` |
| TC-port-workflow-store-026 | WorkflowStore パースエラー(ADR-013 の全エラー種): `load`(`run` の値が `cleanup` / `wait` 以外の定義を置く) | spec/testcases/ports/workflow-store.md#パースエラーadr-013-の全エラー種 | `Err(Parse { error: UnknownRunValue, .. })` |
| TC-port-workflow-store-027 | WorkflowStore パースエラー(ADR-013 の全エラー種): `load`(AgentRun ステータスに `next` の無い定義を置く) | spec/testcases/ports/workflow-store.md#パースエラーadr-013-の全エラー種 | `Err(Parse { error: MissingNext, .. })` |
| TC-port-workflow-store-028 | WorkflowStore パースエラー(ADR-013 の全エラー種): `load`(`next` が `statuses` に無い名前を指す定義を置く) | spec/testcases/ports/workflow-store.md#パースエラーadr-013-の全エラー種 | `Err(Parse { error: NextNotFound, .. })` |
| TC-port-workflow-store-029 | WorkflowStore パースエラー(ADR-013 の全エラー種): `load`(値の生成エラーを含む定義(空の `prompt`、`timeout: 0s`、空文字列の…) | spec/testcases/ports/workflow-store.md#パースエラーadr-013-の全エラー種 | `Err(Parse { error: InvalidValue, .. })`(`NameError` / `DurationError` / `CommandError` を包む) |
| TC-port-workflow-store-030 | WorkflowStore エラー・可視性: `load`(ファイルは存在するが読み取れない(権限不足等。再現できるアダプター環境に限る)) | spec/testcases/ports/workflow-store.md#エラー可視性 | `Err(Io)`。message を含む |
| TC-port-workflow-store-031 | WorkflowStore エラー・可視性: 再度 `load`(`load` に成功した後、同じファイルを別の有効な定義に書き換える) | spec/testcases/ports/workflow-store.md#エラー可視性 | 書き換え後の定義が返る(呼び出し時点のファイル内容。スナップショットは呼び出し側の責務) |
| TC-port-worktree-manager-001 | WorktreeManager: `validate_repo(repo)`(コミットのある実在するgitリポジトリ) | spec/testcases/ports/worktree-manager.md | `Ok(())` |
| TC-port-worktree-manager-002 | WorktreeManager: `validate_repo(repo)`(存在しないパス) | spec/testcases/ports/worktree-manager.md | `Err(TargetError::NotFound)` |
| TC-port-worktree-manager-003 | WorktreeManager: `validate_repo(repo)`(実在するがgitリポジトリでないディレクトリ) | spec/testcases/ports/worktree-manager.md | `Err(TargetError::NotARepository)` |
| TC-port-worktree-manager-004 | WorktreeManager: `head_branch(repo)`(HEADがブランチを指すリポジトリ) | spec/testcases/ports/worktree-manager.md | `Ok(そのブランチ名)` |
| TC-port-worktree-manager-005 | WorktreeManager: `head_branch(repo)`(detached HEAD 状態のリポジトリ) | spec/testcases/ports/worktree-manager.md | `Err(TargetError::DetachedHead)` |
| TC-port-worktree-manager-006 | WorktreeManager: `head_branch(repo)`(コミットのない空リポジトリ) | spec/testcases/ports/worktree-manager.md | `Err(TargetError::EmptyRepository)` |
| TC-port-worktree-manager-007 | WorktreeManager: `branch_exists(repo, branch)`(指定ブランチが存在するリポジトリ) | spec/testcases/ports/worktree-manager.md | `Ok(true)` |
| TC-port-worktree-manager-008 | WorktreeManager: `branch_exists(repo, branch)`(指定ブランチが存在しないリポジトリ) | spec/testcases/ports/worktree-manager.md | `Ok(false)` |
| TC-port-worktree-manager-009 | WorktreeManager: `validate_repo(repo)` /…(検証対象の git 操作自体が失敗する状況(リポジトリメタデータの読み取りが失敗する等。当…) | spec/testcases/ports/worktree-manager.md | `Err(TargetError::Failed { message })` を値として返す(パニックしない)。対象の分類(`NotFound` / `NotARepository` / `DetachedHead` / `EmptyRepository`)とは区別される |
| TC-port-worktree-manager-010 | WorktreeManager: `create(repo, base, ws)`(base ブランチが存在し、`ws.branch`・`ws.path` とも未使用) | spec/testcases/ports/worktree-manager.md | `Ok(())`。`ws.path` に worktree が用意され、その HEAD は base の先端から作成された新ブランチ `ws.branch` を指す |
| TC-port-worktree-manager-011 | WorktreeManager: `create(repo, base, ws)`(worktree_root(`ws.path` の親ディレクトリ)自体がまだ存在しない…) | spec/testcases/ports/worktree-manager.md | `Ok(())`。親ディレクトリを作成したうえで worktree が用意される(ツール管理領域の自動作成) |
| TC-port-worktree-manager-012 | WorktreeManager: 同じ引数で再度 `create`(`create` 成功済みで、`ws.path` に `ws.branch` の…) | spec/testcases/ports/worktree-manager.md | `Ok`(達成済みとして成功)。worktree の内容(既存のファイル変更)に一切触れない(自タスク残骸への冪等性) |
| TC-port-worktree-manager-013 | WorktreeManager: `create(repo, base, ws)`(ブランチ `ws.branch` のみ存在し(コミットが積まれている)…) | spec/testcases/ports/worktree-manager.md | `Ok`。既存ブランチ `ws.branch` に worktree を張り直す。ブランチの先端は変更されない(積まれたコミットが保持され、base から作り直されない) |
| TC-port-worktree-manager-014 | WorktreeManager: `create(repo, base, ws)`(`ws.path` に worktree でない通常のディレクトリ(ファイルを含む)が存在) | spec/testcases/ports/worktree-manager.md | `Err(WorktreeError::Failed { message })`。既存ディレクトリの内容に触れず、自動修復も行わない |
| TC-port-worktree-manager-015 | WorktreeManager: `create(repo, base, ws)`(`ws.path` に `ws.branch` **以外**のブランチの…) | spec/testcases/ports/worktree-manager.md | `Err(WorktreeError::Failed { message })`。既存 worktree(実体・登録・そのブランチ)に触れず、自動修復も行わない(冪等成功は `ws.branch` の worktree として存在する場合に限る。パスの存在のみでは達成済みとみなさない) |
| TC-port-worktree-manager-016 | WorktreeManager: `create(repo, base, ws)`(base に指定したブランチがリポジトリに存在しない) | spec/testcases/ports/worktree-manager.md | `Err(WorktreeError::Failed { message })`。ブランチ・worktree とも作られない |
| TC-port-worktree-manager-017 | WorktreeManager: `remove(repo, ws.path)`(`create` 成功済みの worktree が存在) | spec/testcases/ports/worktree-manager.md | `Ok(Removed)`。worktree の実体と登録が消える。ブランチ `ws.branch` は残る(ブランチには一切触れない) |
| TC-port-worktree-manager-018 | WorktreeManager: `remove(repo, ws.path)`(worktree 内に未コミット変更・未追跡ファイル・`.git` 配下の残骸…) | spec/testcases/ports/worktree-manager.md | `Ok(Removed)`(内容の状態によらず削除する — git worktree remove --force 相当。クリーンアップの主経路)。ブランチは残る |
| TC-port-worktree-manager-019 | WorktreeManager: `remove(repo, path)`(worktree が既に存在しない(手動削除・前回削除済み等)) | spec/testcases/ports/worktree-manager.md | `Ok(AlreadyAbsent)`(達成済みとして成功) |
| TC-port-worktree-manager-020 | WorktreeManager: 同じ引数で再度 `remove`(`remove` で `Removed` を得た直後) | spec/testcases/ports/worktree-manager.md | `Ok(AlreadyAbsent)`(冪等)。ブランチは引き続き残る |
| TC-port-worktree-manager-021 | WorktreeManager: `remove(repo, ws.path)`(`create` 成功済みの worktree が存在し、その削除操作自体が失敗する状況…) | spec/testcases/ports/worktree-manager.md | `Err(WorktreeError::Failed { message })` を値として返す(パニックしない。呼び出し側が `record_tool_failure(WorktreeRemove)` の入力にする報告用エラー)。worktree(実体・登録)とブランチの既存状態には触れない(次回の `remove` が同じ前提から再試行できる) |
| TC-port-run-store-035 | RunStore: `write_starttime` / `write_pid_file` / `write_exit` のいずれか(`prepare_attempt` を経ずに attempt ディレクトリが不在) | spec/testcases/ports/run-store.md | `Ok`。書き込み先のディレクトリが作られ、対応する read 系が書いた値を返す(`prepare_attempt` の失敗後も spawn は行われるため、ラッパーが自力で置き場を作って書けることが自己修復の前提) |
| TC-exec-tick-160 | Tick 手続きD: 観測・判定(Running) > 異常系: tick を実行する(exit ファイルあり・judge 定義あり・`task.workspace` が None(手動修復による不変条件4の破れ)) | spec/testcases/execution/tick.md#異常系-4 | 判定コマンドを起動せず書き込みも行わず、`MissingWorkspace` として報告してスキップする。tick は 0 |
| TC-exec-run-wrapper-028 | RunWrapper 異常系: ラッパー自身の終了コードを観測する(エージェントが非0で終了する / 同定情報一式を残せずに終える) | spec/testcases/execution/run-wrapper.md#異常系 | 前者は 0(エージェントは実行できており、その終了コードは伝播しない — 非0の値は `exit` ファイルだけが持つ)、後者は非0(起動引数が不正な場合も同じく非0)。ラッパー自身の終了コードが表すのはラッパーが責務を果たせたかであって、エージェントの成否ではない |
