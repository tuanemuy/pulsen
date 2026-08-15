# 実装手順 — Issue #9

## 設計

本 Issue が変更するのは `spec/` が主体で、25件のうち2件（A2 / C5）だけが `crates/` と `.adr/` にも及ぶ。したがって「レイヤーの内側から外側へ」は **spec の層構造の内側から外側へ**（domains → usecases → pages → testcases → manual-tests → inventory）として読み、そのあとに A2 / C5 の実装改修（ステップ17・18）と `.adr/` の更新（ステップ19）を置く。台帳（`spec/inventory/`）は本体の機械的な写しなので spec 側の最後にまとめて追従させ、本体の差分を消化台帳として使う。

**B8（`InvariantViolated` の一掃）の対象は `grep -rn 'InvariantViolated' spec/` の全ヒット**（現時点で7ファイル12箇所 — 本体8箇所 + 台帳4行）。以下のステップは各ファイルの該当箇所を明示するが、grep の結果が手順より広ければ grep のほうを正とする。なお `spec/usecases/execution.md` 手続きD 手順0(95行) は語を含まないため grep には現れないが、B8 の追従先である（ステップ4）。

**A2 / C5 は spec・台帳・`crates/`・`.adr/` を同一 PR で動かす。** どの時点でも spec ≡ 実装が保たれるようにするため、spec だけ先にマージしない。

### ドメイン層の記述（`spec/domains/`）

`spec/domains/` は型・不変条件・ポート契約の正本。ここでの言い換えが最も遠くまで波及する。

- **`definition.md`** — 名前系 newtype の生成規約（A4）、`RegistrationValidator` の重複規則（A5）、`CommandLine` の生成2経路（B1）、`WorkflowLoadError::Parse` への解決先パスの追加（A2。ポート表そのものの改訂）。
- **`task.md`** — `ToolFailureKind` の追加と `record_tool_failure` の引数型（B7）、`TransitionError` の6種化（B8/C1）、`RunDirPath` への逆写像（B2）。
- **`execution.md`** — `InconsistentRunFiles` の種別化（B9/C8）、`RunningClassifier` の返り値型と2段規則の置き場所（C3）、`default_judgement` の返り値型（C4）、`NotificationService` への解釈関数の追加（C5）、`RunStore` の write 系ディレクトリ作成契約（B6）、`RunDirPath` の語彙への逆写像（B2）。

新しく spec に載る型は `ToolFailureKind` / `AliveDecision` / `DefaultJudgement` / `NotifyOutcome` / `NotifyFailureCause` の5つ。いずれも「規則によって取りうる値が狭い」ことを型で述べるための直和型で、既存の広い型（`FailureKind` / `RunningDecision` / `JudgeOutcome`）は**残す**（`.adr/3-narrow-decision-types-embedded-in-ledger-types.md`）。spec からこの関係が読めるように書く。

### ユースケース層の記述（`spec/usecases/execution.md`）

- **出力 DTO（サマリー）** — 9フィールド → 11フィールド（`confirmed_running` / `judged` を追加。B5/C2）、`errors` を `message: String` から分類の直和型へ（B4/B10/C6）。
- **共通手続き notify** — 手順4 を `NotificationService::interpret_notify_completion` 経由に（C5）。失敗は `cause` を持つ分類として報告する。
- **手続きC / D の冒頭検査と、AbortTask のエラーケース表** — `TransitionError::InvariantViolated` への言及を `MissingCurrentAttempt` / `MissingProcessIdent` と tick の報告分類へ（B8）。
- **手続きD 手順2/3** — 2段規則の1段目がユースケース側にあること、`judge` 定義ありの枝の先頭で `workspace` の不在を `MissingWorkspace` として検出すること、`classify_alive` が `AliveDecision` を返し `RunningDecision` へ合流させること、`default_judgement` が `DefaultJudgement` を返すこと（B8/C3/C4/C6）。
- **RunWrapper のエラーケース表** — 終了コードの規約（B3）。

`errors` の分類は**ユースケース層の型**（`crates/pulsen/src/application/tick/mod.rs`）であり、`RunFailureCause` / `RemnantsLeft` も同じ層にある。ドメインには置かない。

### プレゼンテーション層の記述（`spec/pages/index.md`）

- 縮退規則 ※1 の「エラー位置」（A1）。
- `tick` のサマリー表示の見出しの規約（B11/C7）。見出しの軸は「タスクファイルに何を残したか」ではなく「**報告が何を残したか＝運用者が次に取る行動**」。
- `wrapper` の終了コードの規約（B3）。

### テストケース・手動テストの記述

- `spec/testcases/task/register-task.md`（A1 / A3）、`spec/testcases/ports/task-repository.md`（A6・**3箇所**）、`spec/testcases/ports/run-store.md`（B6・**末尾に追加**）、`spec/testcases/ports/workflow-store.md`（A2。パースエラー節13行の記法追従 + 先頭行に `resolved_from` の主張）、`spec/testcases/execution/tick.md`（B8 / C3 の波及）。
- `spec/manual-tests/setup.md` TC-46（A3。手順のファイル名の誤りも直す）、`spec/manual-tests/task-execution.md` の対象外の表（A3）。

**テストケースの表に行を挿入しない。** `spec/inventory/test.md` の TC ID は表内の出現順で振られており、途中に挿入すると以降の ID がすべてずれる。ID の安定性が優先する（`_shared/references/spec-inventory.md`）ため、新しい行は表の末尾に置く。

### 台帳（`spec/inventory/`）

本体の差分に1対1で対応させる。変更は3種類。

1. **既存行の「実装されるべき振る舞いの要点」の書き換え**（ID は変えない）。
2. **新規行の追記**（各グループの最大番号 + 1、表の末尾）。
3. **`最終同期` の日付更新**（5ファイルすべて）。

新規 ID: `DOM-definition-057` / `DOM-task-080` / `DOM-task-081` / `DOM-execution-072`〜`076` / `PAGE-wrapper-006` / `PAGE-tick-010` / `TC-port-run-store-035`。

### 実装（`crates/`）— A2 / C5 のみ。合計9ファイル

- **A2**（5ファイル） — `WorkflowLoadError::Parse` に解決先パスを持たせる。参照は `crates/pulsen-domain/src/definition/port.rs`（enum 定義）/ `crates/pulsen/src/adapter/workflow_store.rs`（構築3箇所）/ `crates/pulsen/src/cli/render.rs`（1アーム）/ `crates/pulsen-conformance/src/workflow_store.rs`（4アーム + ヘルパー `expect_parse_error`）/ **`crates/pulsen-conformance/HOOKS.md`**（`TC-port-workflow-store-017`(140行) の「組み立て手段」に `expected_path_for_name` を足す。HOOKS.md 冒頭が「行を足す・フックを足すときはこの表も更新する」を規約として宣言しており、`expected_path_for_name` を使う 001 / 002 / 003 は既に併記している）。`crates/pulsen/src/application/register_task.rs` は `WorkflowLoadError` を包むだけ、`crates/pulsen/tests/register_task.rs` と `crates/pulsen-conformance/src/doubles/` は `NotFound` しか使わないため変更を要さない（`grep -rn 'WorkflowLoadError::Parse' crates/` で確認）。新しい型は増えないため `definition/mod.rs` の再エクスポートは変わらない。
- **C5**（5ファイル） — `NotifyOutcome::Failed { detail }` を分類にする。参照は `crates/pulsen-domain/src/execution/notification.rs`（enum + `interpret_notify_completion` + ユニットテスト）/ **`crates/pulsen-domain/src/execution/mod.rs`（18行の再エクスポート）** / `crates/pulsen/src/application/tick/mod.rs`（`TickIssue::NotifyFailed` の定義）/ `crates/pulsen/src/application/tick/notify.rs`（2アームと `report_failure`）/ `crates/pulsen/src/cli/render.rs`（文言）。結合テストは `NotifyFailed { .. }` で受けているため変更を要さない。

`render.rs` は A2 / C5 の両方が触るため、9ファイルの内訳は「A2 の5 + C5 の5 − 重複1」。

**`HOOKS.md` の件数（冒頭の196行・`## RunStore` 節）は据え置く。** ステップ9 が `spec/testcases/ports/run-store.md` に1行足すが、`HOOKS.md` が数えるのは「これまでのスライスで扱った行」であり、適合スイート実装を伴わない新規行はまだ数えない（`TC-port-run-store-035` の実装は後続スライス）。数え直すと AC-Z3 が落ちる。

**`NotifyFailureCause` の再エクスポートは省略できない。** `crates/pulsen-domain/src/execution/mod.rs:9` は `mod notification;`（非公開）で、外部に型を出しているのは18行の `pub use notification::{NotificationService, NotifyOutcome};` だけである。`TickIssue::NotifyFailed` のフィールド型（`application/tick/mod.rs`）と網羅 `match`（`cli/render.rs`）の両方がこの型を名指しするため、`pub use` を広げないとコンパイルが通らない。

## 実装ステップ

依存方向の順（spec の内側から外側へ、台帳は最後）。各ステップは1ファイルに閉じる（ステップ12 と14 だけは同じ層の2ファイルをまとめる）。全20ステップ。

### 1. `spec/domains/definition.md` を追従させる

- **対象ファイル:** `spec/domains/definition.md`
- **変更内容:**
  - **A4** —「名前系(文字列 newtype)」の表の直後の1文を「制約のある型は `parse(s: String) -> Result<Self, NameError>` でのみ生成する。制約のない `InputText` は総関数 `new(s: String) -> Self` で生成する(`Err` になる経路のない `Result` を呼び出し側に持ち込まない)。等価性は文字列の完全一致。」に置き換える。
  - **B1** —「CommandLine」の「生成」を2経路にする: 「`CommandTemplate::expand` の結果、またはプロセス境界を越えた復元 `rehydrate(tokens: Vec<String>) -> Result<Self, CommandError>`(トークン0個は `Empty`)。ラッパーが受け取った argv から復元するための経路であり、テンプレートを持たない側でも同じ不変条件(1トークン以上)を通す。」
  - **A5** —「RegistrationValidator」のメソッド説明の末尾（「エラーは全ステータス分をまとめて返す」の行の直後）に「`status` を持たない2種(`UnknownAgent` / `InvalidAgentDefinition`)はステータスではなく**エージェント単位**の誤りであり、複数のステータスが同じエージェントを参照しても値まで同一のエラーになるため、同値の重複は1件にまとめる(直す先が1つしかない案内を参照回数だけ並べない)。`status` を持つ3種は各ステータス分を積む。」を加える。
  - **A2** —「WorkflowStore」のエラー一覧の `Parse(WorkflowParseError)` を `Parse { error: WorkflowParseError, resolved_from: PathBuf }` に改める。あわせて契約リストに「解決先の案内は**構造化フィールドに一本化する**。`NotFound { attempted }` と `Parse { resolved_from }` は対象ファイルを構造として持ち、その内側の `WorkflowParseError` 12種はどれもパスを持たない(`location` は論理位置 `statuses.queued.prompt` そのものを指す)。自由形式のメッセージにパスを前置するのは、構造化フィールドを持てない `Io { message }` だけ — `--workflow` を名前で指定した場合、解決先(`<home>/workflows/<n>.yaml`)は利用者が直接書いていないため、案内に出す責務はポート側にある」を加える。

    **なぜ `Io` だけが前置に残るか（設計判断。adr.md ADR-005）**: `WorkflowLoadError` の3変種のうち `NotFound` / `Parse` は解決先を構造として持つが、`Io { message }` だけは持たない。`WorkflowParseError` の12種はすべて `Parse` の内側にあるため、`Parse { resolved_from }` の1フィールドで全種の解決先が示せる。したがって現在 `YamlSyntax { message }` に前置している解決先は**二重表示になるので外す**（実装はステップ17）。「変種ごとに前置するか決める」のではなく「構造化フィールドで示せる経路は前置しない」という規則にすることで、`WorkflowParseError` に変種が増えても判断が要らない。
- **理由:** 生成規約・検証規約・ポート契約はドメインの正本であり、ここを直さないと台帳もテストケースも根拠を失う。A2 は Issue 本文が「ポート表は spec が確定させており、Issue #1 の受け入れ基準（ポート表との1:1一致）の対象なので実装側では変えていない」として **spec 側が正・実装を直す**と決着させた件なので、ここが起点になる（実装の改修はステップ17）。

### 2. `spec/domains/task.md` を追従させる

- **対象ファイル:** `spec/domains/task.md`
- **変更内容:**
  - **B7** —「FailureNote」の節に `ToolFailureKind = WorktreeCreate | WorktreeRemove | ArchiveMove` を加え、「`FailureKind` のうちツール操作の3種だけを取り出した型。`record_tool_failure` はこれだけを受け取り、記録時に `FailureKind` へ写す(`SpawnFail` / `JudgeFail` を渡すとカウンタと失敗種別が食い違う帳簿になるため、型で排除する)」と説明する。
  - **B7** — 遷移表の `record_tool_failure` の行のシグネチャを `(self, kind: ToolFailureKind, message, retry_limit: u32, now) -> Result<Task>` にする。
  - **B8/C1** —「エラー型」の `TransitionError` を実装の6種に置き換える:
    ```
    TransitionError =
      InvalidState { expected: &'static [ExecutionStateKind], actual: ExecutionStateKind }
    | WorkspaceAlreadySet
    | WorkspaceNotSet
    | NotAgentRunStatus { status: StatusName }
    | MissingCurrentAttempt
    | AlreadyNotified
    ```
    併せて「このエラーは永続化されず表示にしか使われないため、**分類だけを持ち完成文言を持たない**(`expected` は受理される実行状態そのもの。文言は CLI 層が組み立てる)」「`MissingCurrentAttempt` は手動修復で破られた不変条件2〜3の破れ」「`AlreadyNotified` は `Stopped { notified_at: Some }` への `mark_notified`(判別子だけの `InvalidState` では `expected = [stopped]` / `actual = stopped` という自己矛盾した報告になるため専用の変種にする)」を記す。
  - **B8** —「検証の境界」の段落（164行）の「不変条件 2〜4 は … 崩れていれば `TransitionError::InvariantViolated` を返す」を、**破れの検出主体ごとに3文へ書き分ける**。1文のまま `MissingCurrentAttempt` に置換すると、実装と食い違う新しい記述を作ることになる（不変条件4 の破れは `MissingCurrentAttempt` にならない）。
    - 「不変条件 2〜3 は手動修復で破られたままデコードを通り得るため、遷移関数が前提として検査し、崩れていれば `TransitionError::MissingCurrentAttempt` を返す。」
    - 「不変条件 4(workspace)の破れは `record_launching` が `TransitionError::WorkspaceNotSet` で拒否する。」
    - 「判定コマンドへ渡す `WORKSPACE` を組めない形の破れは、遷移関数を呼ぶ前にユースケースが検出し、tick の報告分類 `MissingWorkspace` として報告する(遷移エラーに相乗りさせない — 遷移を呼ばずにスキップする判断であり、帳簿には何も残らない)。」
  - **B8** — `TaskRepository` のデコード節（300行）の「不変条件2〜4(状態間の整合)はデコードでは検証せず、遷移関数の前提検査(`InvariantViolated`)に委ねる」を「…デコードでは検証せず、遷移関数の前提検査(`TransitionError::MissingCurrentAttempt` / `WorkspaceNotSet`)とユースケース側の検査に委ねる」に直す。
  - **B2** —「RunDirPath」の節に「逆写像: `state_root(&self) -> Option<StateRoot>` — パスから `attempt-<n>` と task-id を読み、`derive` で組み直した結果が自身と一致する場合にのみ `StateRoot` を返す。config もホームも読まないラッパーが `RunStore` を組むために使う(`RunDirPath` は起動引数として渡る)。導出の一致を条件にすることで、`derive` と逆写像が食い違う値を返さない。」を加える。
- **理由:** `TransitionError` は #2 と #3 の両方が指す同一の型で、spec の他ファイルからの参照も多い。ここを正本として直してから波及先を直す。

### 3. `spec/domains/execution.md` を追従させる

- **対象ファイル:** `spec/domains/execution.md`
- **変更内容:**
  - **B9/C8** —「LaunchingClassifier」の `InconsistentRunFiles { message: String }` の行を「`InconsistentRunFiles` — 破れの**種別だけを持つ列挙**(現在の変種は `MissingStartTime` の1つ)。文言は表示側が組み立てる。どのタスク・どの run_dir かの文脈付与は呼び出し側(報告時)の責務」に置き換える。
  - **C3** —「分類の決定(直和型)」のブロックに `AliveDecision = KeepRunning | KillOnTimeout | DiedWithoutExit` を加え、「`RunningDecision` から `Judge` を除いた3値。`From<AliveDecision> for RunningDecision` で合流させる」と注記する。`RunningDecision` は4値のまま残す。
  - **C3** —「RunningClassifier」の節を、2段規則の**1段目がユースケース側にある**ことが読める形にする: 「分類は2段で行う。1段目(exit の有無)はユースケースが値にする — exit が Some なら生存を観測せず `RunningDecision::Judge(exit)` とする。2段目(生存)だけを `classify_alive` が受け持つ。」とし、メソッドのシグネチャを `classify_alive(aliveness, started_wall, timeout, now) -> AliveDecision` にする。「`Judge` は返さない」という doc ではなく**返り値型で担保する**ことを明記する。
  - **C4** —「JudgementService」の `default_judgement` を `(exit: &ExitCode) -> DefaultJudgement` にし、`DefaultJudgement = Completed | Failed` を「JudgeOutcome / JudgeConclusion」の節に加える。「`Skipped` は判定コマンドの exit 20 だけが生むため、デフォルト判定は2値しか返さない。`JudgeOutcome` は3値のまま残り、`From<DefaultJudgement> for JudgeOutcome` で埋め込む」と注記する。
  - **C5** —「NotificationService」の**責務行**（126行）を「stopped 確定通知の環境変数の構成(requirements §8)」から「stopped 確定通知の環境変数の構成と、通知の結末の成否の解釈(requirements §8)」へ広げる。解釈関数を足すだけだと責務行が実態と合わなくなる。
  - **C5** —「NotificationService」の**定数 `NOTIFY_TIMEOUT` の行**（128行）から成否の規則を外す。現在この行の末尾には「超過・起動不能・非0終了はいずれも通知失敗であり、`notified_at` を書かずに終える — 次のtickが再通知する(at-least-once。requirements §8)」があり、次の箇条で足す `interpret_notify_completion` の説明と同じ規則を述べることになる。定数の行には「notify_cmd の実行にはこのtimeoutを必ず適用する(ハングした通知コマンドが排他ロックを保持したまま tick / CLI を塞ぐことを防ぐ)」までを残し、成否の規則は解釈関数の側へ一本化する（「成否の解釈は1関数にしかない」という C5 の主張が spec の構造としても表れる）。
  - **C5** —「NotificationService」の節に次を加える。

    ```
    NotifyOutcome      = Delivered | Failed { cause: NotifyFailureCause }
    NotifyFailureCause = ExitedNonZero { exit: ExitCode } | TimedOut | FailedToStart { message: String }
    ```

    メソッド `interpret_notify_completion(c: &CommandCompletion) -> NotifyOutcome` — 「`Exited(0)` = `Delivered`。非0終了 / `TimedOut` / `FailedToStart` はいずれも `Failed` で、**原因は分類として持ち完成文言は持たない**(文言は CLI 層が組み立てる)。『`Exited(0)` だけが `notified_at` を書く根拠になる』という規則を、stopped を書くすべての経路(tick の各上限超過・DegradedTask の再通知・abort)が共有するためにドメインへ置く。経路ごとに書くと requirements §8 の at-least-once が片方だけで破れる」を記す。`TimedOut` がフィールドを持たないのは、通知の timeout が設定値ではなく組み込み定数 `NOTIFY_TIMEOUT` の1つに定まるため（表示側が定数を読む）。`Failed` を平坦化せず `cause` を内側に持つのは、`Delivered` / `Failed` の2分岐が at-least-once の規則そのものだからであることも記す。隣の `interpret_judge_completion` と解釈の位置が揃うことも記す。
  - **B6** —「RunStore」の契約リストに「**write 系(`write_starttime` / `write_pid_file` / `write_exit` / `write_invalidation_marker`)はいずれも書き込み先のディレクトリを必要に応じて作る。** `prepare_attempt` が失敗した後も spawn は行われる設計であり、ラッパーが自力でディレクトリを作って書けることが自己修復の前提になる」を加える。
  - **B2** —「RunDirPath のファイル配置(語彙)」の表の下に「逆写像 `state_root(&self) -> Option<StateRoot>` も同じ語彙に属する(Task ドメインの `RunDirPath` に定義。`derive` との一致を条件に復元する)」の1行を加える。
- **理由:** 分類サービスの返り値型と NotificationService の語彙は #3 の中核。ユースケース側の記述（ステップ4）はこれを前提に書く。

### 4. `spec/usecases/execution.md` を追従させる

- **対象ファイル:** `spec/usecases/execution.md`
- **変更内容:**
  - **B5/C2** —「出力DTO(サマリー)」の表を11フィールドにする。`launched` / `confirmed_running` / `judged` / `transitioned` / `skipped_back` / `frozen` / `notified` / `archived` はいずれも `Vec<TaskId>`。`confirmed_running` は launching → running の取込（`transitioned` にも `skipped_back` にも語義が合わない）、`judged` は `complete_run` による判定確定（`advance` の結果である `transitioned` に混ぜると1タスク1tick1ステップの2つのステップが同じフィールドに現れる）であることを注記する。加えて「タスクファイルへの書き込みを行った経路は必ずサマリーのいずれかのフィールドを埋める」という規則を明記する（これを欠くと主経路が毎回「処理対象なし」と表示される）。
  - **B4/B10/C6** — `errors` の行を `Vec<TickIssue>` にし、直後に分類の直和型を置く。文言はここでは組み立てず、CLI 層（pages）が組み立てることを明記する。
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
    `RunFailureCause` は**判断の主体**を分けるための分類（判定コマンドが失敗を返したのか、エージェント自身が非0で終わったのか）。`RemnantsLeft` は報告を要する2値であり、後始末を残さない `RemnantOutcome::Killed` がこの分類に現れないことを型で表現不能にする。`MissingProcessIdent` を `MissingCurrentAttempt` と分けるのは修復の手がかりが違うため。
  - **B8** — 処理フロー7 の直後の段落（56行）「遷移関数の `TransitionError::InvariantViolated`(手動修復による破れ)は報告してそのタスクをスキップする」を「遷移関数の `TransitionError`(手動修復による破れを含む)は `errors` に `Transition` として報告してそのタスクをスキップする」に直す。手続きC 手順0（83行）/ 手続きD 手順0（95行）の「不変条件2の破れ」「不変条件2〜3の破れ」の記述に、報告分類が `MissingCurrentAttempt` / `MissingProcessIdent` であることを添える。
  - **B8** — **AbortTask のエラーケース表（178行）**の「Tick の InvariantViolated スキップと同じ原則」を「Tick が `MissingCurrentAttempt` / `MissingProcessIdent` を報告してスキップするのと同じ原則」に直す。台帳（`UC-execution-008`）だけが新しくなり本体が古いまま残る組み合わせを避ける。
  - **C3** — 手続きD 手順2 の「(生存観測は行わない。RunningClassifier の2段規則)」を「(生存観測は行わない。2段規則の1段目はここで `RunningDecision::Judge(exit)` として値にする)」に直し、手順3 の `classify_alive` の結果が `AliveDecision` であり `RunningDecision` へ合流させてから網羅 `match` で分岐することを記す。
  - **C4** — 手続きD 手順2 の `default_judgement(exit)` の結果を `DefaultJudgement`(Completed / Failed) と書く。
  - **C6** — 手続きD 手順2 の「`judge` 定義あり」の枝（99〜100行）の**先頭**に検査を1つ挿入する: 「`task.workspace` が None なら不変条件4の破れとして `MissingWorkspace` を報告しスキップする(判定コマンドは起動せず、書き込みも行わない)」。現在は `judge_env(id, workspace, exit, run_dir)` を無条件に呼ぶ形になっており、この検査が無いまま `MissingWorkspace` を分類表に足すと「どの手順が積むのか読めない分類」が1つ生まれる。
  - **C5** — 共通手続き notify の手順4（15行）を「`NotificationService::interpret_notify_completion(completion)` → `Delivered` なら `mark_notified(now)` → `save`(DegradedTask は `save_degraded`)。`Failed { cause }` なら何もしない(次のtickが再通知する。at-least-once)。**失敗の報告の形は呼び出し側が決める**（この手続きが固定するのは成否の判定経路まで）。いずれの経路でも文言は表示層が `cause` から組み立てる」に直す。
  - **C5** — **経路ごとの報告先は呼び出し側の記述に置く。** 共通手続き notify は冒頭（9行）が宣言するとおり Tick と AbortTask が共有しており、報告先は同じではない。共通手順に tick 専用の `errors` を書くと、AbortTask の経路に存在しないフィールドを共通手順が要求する形になり、新しい食い違いを spec に作る（AbortTask は未実装なので `cargo test` では検出されない）。
    - Tick 側: 処理フロー7 の直後の段落（56行。B8 で書き換える段落）に「共通手続き notify が `Failed { cause }` を返した場合は `errors` に `NotifyFailed { task_id, cause }` を積む」を足す。
    - AbortTask 側: 出力DTO の `notify_warning | Option<String>`（152行）と処理フロー 5（163行「共通手続き notify を実行(…`notify_warning` を表示)」）は現行のまま成立する。`notify_warning` の説明に「文言は表示層が `cause` から組み立てる」を添えるにとどめ、`errors` を持ち込まない。
  - **B3** — RunWrapper のエラーケース表と処理フローに終了コードの規約を反映する（詳細な規約本体は pages に置き、ここからは参照する）。
- **理由:** サマリー DTO と `errors` の分類はユースケース層の契約であり、pages の表示規約（ステップ6）とテストケース（ステップ11）の前提になる。

### 5. `spec/usecases/task.md` を追従させる

- **対象ファイル:** `spec/usecases/task.md`
- **変更内容（A2）:** RegisterTask のエラーケース表（54行）の「ワークフローのパースエラー(`WorkflowParseError` 全種) | 入力(位置・原因を表示)」を「入力(位置・原因・**解決先パス**を表示)」に直す。台帳 `UC-task-001`（ステップ14）にも同じ語を反映する。
- **理由:** A2 の動機は「名前で指定した場合、利用者が直接書いていない解決先が案内に出ない」ことの解消だが、ポート表と適合ケースだけを直すと「型は変わったが、表示がどう変わるかは spec のどこにも書かれていない」状態になる。実装（`cli/render.rs` の `Parse` アーム。ステップ17）は必ず解決先を出すので、この1語を足さないと `spec/usecases/task.md:54` が実態より狭い記述として残る。A2 の効果を spec 側から検証可能にするのはここだけ。

### 6. `spec/pages/index.md` を追従させる

- **対象ファイル:** `spec/pages/index.md`
- **変更内容:**
  - **A1** — 縮退規則 ※1 の「パース不能ならエラー位置を表示して非0」を「パース不能なら**構文エラー・重複キーは行・列、スキーマ違反はキーのパス(論理位置。`agents.claude.cmd` 等)**を表示して非0」に直す。
  - **B11/C7** — `tick` の「機能」の「処理結果のサマリー表示」を、見出しの規約として展開する。
    - ID を並べる見出しは**サマリーの10フィールドと1対1で対応**し、空の見出しは出さない（規約はこの対応と非表示のほうで、日本語の文言は表示層の裁量に残す。例: 起動 / 起動確認 / 判定確定 / 遷移 / 実行待ちへ復帰 / 凍結 / 通知 / 終端処理 / gcで削除 / gcで削除できず）。
    - 報告(`errors`)の見出しは4つで、**文言まで規約として固定する**: **失敗を記録**(カウンタを消費して帳簿に失敗を残した) / **起動の結果が未確定**(起動したかどうかが確定しておらず猶予経路が分類する) / **スキップ**(何も記録せず次のtickが同じ判断を再導出して再試行する) / **後始末が残っている**(OS 側に残ったものがあり、後始末の主体は tick ではなく人間)。Issue が求めているのは「3分類から4分類へ増えた」という**分類の規約**なので、この4つは軸・振り分け規則とともに固定する。
    - 見出しの軸は「タスクファイルに何を残したか」ではなく「**報告が何を残したか＝運用者が次に取る行動**」。
    - 記録すべきことが1つも起きなかった tick は「処理対象のタスクはありませんでした」と表示する。
  - **B3** — `wrapper` の「状態」に終了コードの規約を加える: 「終了コードは**ラッパー自身が責務を果たせたか**を表す。エージェントを実行した場合とマーカーにより起動しなかった場合は 0(pages の『エージェントを起動せず正常終了する』に従う)、同定情報を何も残せずに終えた場合と起動引数が不正な場合は非0。**エージェントの終了コードは伝播しない** — エージェントの結末は run ディレクトリの `exit` ファイルだけが持つ記録であり、2箇所に分けると『exit なし = プロセス死亡として分類する』という tick 側の規則と食い違う。」
- **理由:** 表示規約は CLI（pages）の責務。`errors` の分類（ステップ4）を運用者が読む見出しへ振り分ける規則をここに置く。

### 7. `spec/testcases/task/register-task.md` を追従させる

- **対象ファイル:** `spec/testcases/task/register-task.md`
- **変更内容:**
  - **A1** — 異常系の「config.yaml がパース不能(構文エラー・未知キー)」の期待結果を「構文エラー・重複キーは行・列を、スキーマ違反(未知キー・型不一致)は問題のキーのパスを表示して非0で終了する」に直す。
  - **A3** — 異常系の「`workflow:` キーがなく、ファイル名由来の表示名が不正になるパス(拡張子を除くと空になるファイル名等)」を「`workflow:` キーがなく、ファイル名由来の表示名が不正になるパス(語幹が空白のみになる ` .yaml` 等。`Path::file_stem` は `.yaml` を語幹として返すため『拡張子を除くと空』は作れない)」に直す。
- **理由:** テストケースの前提条件・期待結果は受け入れテストの契約であり、到達不能な例示を残すと実装できない行になる。

### 8. `spec/testcases/ports/task-repository.md` を追従させる

- **対象ファイル:** `spec/testcases/ports/task-repository.md`
- **変更内容（A6。対象は同じ語を使う**3箇所**）:**
  - **現役側（52行。「Corrupt と SnapshotUnreadable の区別」）** —「スナップショットフィールドのみを構文不正な内容に置き換える(タスク側フィールドは有効なまま)」を「スナップショットフィールドのみを**有効な JSON だがスナップショットとして解釈できない内容**に置き換える(タスク側フィールドは有効なまま)」に直す（台帳 `TC-port-task-repository-022`）。
  - **アーカイブ側（58行。同じ節）** —「`state/archive/` にスナップショットフィールドのみ構文不正なタスクファイルを置く(タスク側フィールドは有効なまま。現役側に同 ID なし)」も同じ語に直す（台帳 `TC-port-task-repository-028`）。**52行と完全に同じ性質**（ファイル全体を1回の JSON パースで読む限り `Corrupt` になり `Archived(SnapshotUnreadable)` へ到達できない）で、片方だけ直すと実装できないテストケースが1件残る。なおこの行は助詞が無い（「のみ構文不正」）ため、`スナップショットフィールドのみを構文不正` の grep では拾えない（ステップ20 の語は `(を)?` を含む）。
  - **「save / save_degraded」の前提条件（24行）** —「スナップショットフィールドのみを不正な内容に書き換えたタスクファイル」も同じ語（有効な JSON だがスナップショットとして解釈できない）にそろえる（台帳 `TC-port-task-repository-009`）。
  - 併せて、ファイル全体を1回の JSON パースで読む実装では `Corrupt` と `SnapshotUnreadable` を同時に満たす内容が作れないことを、区別の節の導入文に1文で添える（`.adr/1-task-file-json-and-corrupt-classification.md` の判断と一致させる）。
- **変更内容（B8 の波及）:** 同じ節（56行）の「不変条件2〜4はデコードでは検証しない。遷移関数の前提検査 `InvariantViolated` に委ねる」を「…遷移関数の前提検査(`TransitionError::MissingCurrentAttempt` 等)に委ねる」に直す（台帳 `TC-port-task-repository-026`。A6 とは別件）。
- **理由:** 「構文不正」のままではファイル全体のパースが落ち `Corrupt` になるため、そのケースを満たす実装が存在しない。

### 9. `spec/testcases/ports/run-store.md` に適合ケースを追加する

- **対象ファイル:** `spec/testcases/ports/run-store.md`
- **変更内容（B6）:** 表の**末尾**に1行追加する。前提条件「`prepare_attempt` を経ずに attempt ディレクトリが不在」／ 操作「`write_starttime` / `write_pid_file` / `write_exit` のいずれか」／ 期待結果「`Ok`。書き込み先のディレクトリが作られ、対応する read 系が書いた値を返す(`prepare_attempt` の失敗後も spawn は行われるため、ラッパーが自力で置き場を作って書けることが自己修復の前提)」。
- **理由:** 契約（ステップ3）に対応する適合ケースが spec に無いと、台帳から検証できない。表の途中に挿入すると既存の TC ID がずれるため末尾に置く。

### 10. `spec/testcases/ports/workflow-store.md` を追従させる

- **対象ファイル:** `spec/testcases/ports/workflow-store.md`
- **変更内容（A2）:**
  - 「パースエラー」節の表**13行（37〜49行）**の期待結果を、ステップ1 で改めたポート表と同じ記法にそろえる: `Err(Parse(YamlSyntax))` → `Err(Parse { error: YamlSyntax, .. })`、以下 `UnknownKey` / `ForbiddenKey` / `MissingInitial` / `InitialNotFound` / `EmptyStatuses` / `NoAction` / `MultipleActions` / `UnknownRunValue` / `MissingNext` / `NextNotFound` / `InvalidValue` も同様（`..` は `resolved_from` を明示的に見ない行であることを示す）。行の挿入・削除はしない（TC ID がずれる）。
  - **先頭行（37行）の期待結果にだけ `resolved_from` の主張を足す**: 「`Err(Parse { error: YamlSyntax, resolved_from })`。message・location を含み、`resolved_from` は名前解決した絶対パス(`<workflows_dir>/wf.yaml`)」。**全13行に同じ主張は足さない** — 1行で契約は固定でき、13行に重複させると skip 予算の趣旨（`.adr/1-conformance-skip-budget.md`）にも反する。
- **理由:** ポート契約にフィールドを増やすのに適合ケースだけ旧いタプル記法で残ると、同じ型が spec 内に2つの形で存在することになる。これは本 Issue が閉じようとしている食い違いそのもので、しかも `Parse(` は横断確認（ステップ20）でしか捕まらない。加えて、B6 では「契約を足したら適合ケースで主張する」形を採っているので、A2 だけ契約に増えたフィールドを適合スイートが1件も検証しない状態にすると非対称になる（実装側の対応はステップ17）。

### 11. `spec/testcases/execution/tick.md` を追従させる

- **対象ファイル:** `spec/testcases/execution/tick.md`
- **変更内容:**
  - **B8** — 「走査と分岐」異常系（39行）の「手動修復で不変条件が破れている」の期待結果の「または遷移関数の `InvariantViolated`」を「または遷移関数の `TransitionError`(`MissingCurrentAttempt` 等)」に直す。対応する台帳行は `TC-exec-tick-022`（ステップ14）。
  - **C3** — 手続きD 異常系（203行）の「exit ファイルあり・`starttime_of` が失敗する環境」の期待結果「(exit が Some なら判定 — RunningClassifier の2段規則。…)」を「(exit が Some なら判定 — 2段規則の1段目はユースケース側にあり、`classify_alive` は生存の分類だけを返す。…)」に直す。対応する台帳行は `TC-exec-tick-103`（365行。ステップ14）。この2行がステップ11 で書き換える全てで、他の `TC-exec-tick-*` は対象外。
  - 正常系「exit なし・プロセス生存(照合一致)・timeout 未超過」の注記「(exit があれば生存観測は行わない、の対偶として生存観測を経由する)」はそのまま成立するので触らない。
- **理由:** テストケースが古い分類器の責務分割を述べていると、実装がどちらを担保すべきか読めなくなる。

### 12. `spec/manual-tests/` を追従させる

- **対象ファイル:** `spec/manual-tests/setup.md`、`spec/manual-tests/task-execution.md`
- **変更内容（A3）:**
  - `setup.md` TC-46 の目的文「ファイル名から拡張子を除くと空になるパス指定」を「ファイル名の語幹が空白のみになるパス指定」に直す。
  - **TC-46 手順1 のファイル名 `$WORK/.yaml` を ` .yaml`(先頭が空白)に直す。** `Path::file_stem(".yaml")` は `.yaml` を返すため、現在の手順では登録が成功してしまい期待結果と食い違う。コマンド例もクォートを含む形（例: `pulsen add --workflow "$WORK/ .yaml" --repo "$REPO"`）に直す。
  - `task-execution.md` の手動テスト対象外の表の「拡張子を除くと空になるファイル名という特殊入力のみ」を「語幹が空白のみになるファイル名という特殊入力のみ」に直す。
- **理由:** 手動テストは人間がそのまま実行する手順書であり、到達しない手順を残すと検証したつもりの空振りになる。

### 13. `spec/inventory/domain.md` を追従させる

- **対象ファイル:** `spec/inventory/domain.md`
- **変更内容:**
  - 既存行の要点を書き換える: `DOM-definition-007`（A4）、`DOM-definition-023`（B1）、`DOM-definition-049`（A5）、`DOM-definition-052`（A2・`load` の契約が返すエラーが解決先を伴うこと）、`DOM-definition-055`（A2・`Parse { error, resolved_from }`）、`DOM-task-013`（B2）、`DOM-task-024`（B7・`ToolFailureKind` との関係）、`DOM-task-042`（B7）、`DOM-task-053`（B8/C1・6種）、`DOM-execution-004`（C4・`DefaultJudgement` からの埋め込み）、`DOM-execution-008`（C3・`AliveDecision` からの埋め込み）、`DOM-execution-016`（B9/C8）、`DOM-execution-017`（C3・2段目に限定）、`DOM-execution-019`（C4）、`DOM-execution-022`（C5）、`DOM-execution-071`（C5・**サービスの要点を「通知の構成と、結末の成否の解釈」へ広げる**。現在は `NOTIFY_TIMEOUT` の話に留まっている）、`DOM-execution-041` / `042` / `043`（B6）。
  - 表の末尾に新規行を追記: `DOM-definition-057`（`CommandLine.rehydrate` ドメイン関数）、`DOM-task-080`（`ToolFailureKind` 値オブジェクト）、`DOM-task-081`（`RunDirPath.state_root` ドメイン関数）、`DOM-execution-072`（`AliveDecision` 値オブジェクト）、`DOM-execution-073`（`DefaultJudgement` 値オブジェクト）、`DOM-execution-074`（`NotifyOutcome` 値オブジェクト）、`DOM-execution-075`（`NotificationService.interpret_notify_completion` ドメイン関数）、`DOM-execution-076`（`NotifyFailureCause` 値オブジェクト）。
  - `DOM-definition-050`（`RegistrationError` 5種）は**列挙が変わらないため更新しない**。research.md の波及表にある行なので、消化確認（ステップ20）で宙に浮かせないよう「対応なし」で確定させる。
  - `最終同期` を更新する。
- **理由:** 台帳は完全性ゲート・監査・Issue 化がすべて基準にする列挙であり、ここに載らない要素は下流で永久に検出されない。

### 14. `spec/inventory/usecase.md` と `spec/inventory/test.md` を追従させる

- **対象ファイル:** `spec/inventory/usecase.md`、`spec/inventory/test.md`
- **変更内容:**
  - `usecase.md`: `UC-execution-001`（C5・notify の解釈と `NotifyFailureCause`。**報告先は経路ごとに異なることも要点に残す** — Tick は `errors`、AbortTask は `notify_warning`）、`UC-execution-002`（B4/B5/B8/B10/C2/C5/C6・サマリー11フィールドと `errors` の分類）、`UC-execution-003` / `UC-execution-004`（B7・`record_tool_failure` の引数型）、`UC-execution-005`（B8/B10）、`UC-execution-006`（B8/C3/C4 + **C6・`judge` 定義ありの枝の `MissingWorkspace` 検査**）、`UC-execution-008`（B8・AbortTask のエラーケース表の言い換えに対応。`notify_warning` の記述は変えない）、`UC-execution-009`（B3・終了コード）、`UC-task-001`（A5 + **A2・パースエラーの案内に解決先パスが出ること**）。`UC-execution-007`（手続きE の gc）は報告先が `gc_errors` であり `errors` の分類とは無関係なので**更新しない**。`最終同期` を更新する。
  - `test.md`: `TC-task-register-task-015`（A1）、`TC-task-register-task-034`（A3）、`TC-port-task-repository-009` / `TC-port-task-repository-022` / **`TC-port-task-repository-028`（A6。ステップ8 で書き換えるアーカイブ側(58行)に対応する行。567行）**、**`TC-port-task-repository-026`（B8。ステップ8 で書き換える `spec/testcases/ports/task-repository.md:56` に対応する行で、A6 の 009 / 022 / 028 とは別件）**、`TC-exec-tick-022`（B8。ステップ11 で書き換える `spec/testcases/execution/tick.md:39` に対応）、**`TC-exec-tick-103`（C3。ステップ11 で書き換える `spec/testcases/execution/tick.md:203` に対応する行。365行。要点欄にも「RunningClassifier の2段規則」が写っている）**、**`TC-port-workflow-store-017`〜`029`（A2。ステップ10 で書き換える適合ケース13行に対応する 600〜612行。`Err(Parse(<変種>))` の記法を本体と同じ形にそろえ、017 には `resolved_from` の主張を加える）** の要点を書き換える。末尾に `TC-port-run-store-035`（B6）を追記する。`最終同期` を更新する。
- **理由:** テストケースの台帳は spec/testcases の写しであり、本体だけ直すと2つの記述が残る。

### 15. `spec/inventory/adapter.md` を追従させる

- **対象ファイル:** `spec/inventory/adapter.md`
- **変更内容:** `ADP-runstore-008` / `009` / `010` に「書き込み先のディレクトリを必要に応じて作る」を加える（B6）。`ADP-workflowstore-001` に「パースの失敗は `Parse { error, resolved_from }` として解決先を伴って返す」を加える（A2）。`ADP-config-001` の要点にエラー位置の2層（構文・重複キーは行・列、スキーマ違反はキーのパス）を加える（A1）。`最終同期` を更新する。
- **理由:** アダプター台帳は「その契約を満たす実装が存在すること」を検証する基準であり、契約の追加はここにも現れる必要がある。

### 16. `spec/inventory/frontend.md` を追従させる

- **対象ファイル:** `spec/inventory/frontend.md`
- **変更内容:** `PAGE-common-008` の要点にエラー位置の2層を反映する（A1）。`PAGE-tick-004` の要点をサマリーの見出しの規約に合わせる（B11/C7）。末尾に `PAGE-tick-010`（tick サマリーの報告4見出しと見出しの軸）と `PAGE-wrapper-006`（wrapper の終了コード規約）を追記する。`最終同期` を更新する。
- **理由:** 表示規約（ステップ6）に対応する検証基準を台帳に置く。

### 17. A2 の実装を `spec` に合わせる（`WorkflowLoadError::Parse`）

- **対象ファイル:** `crates/pulsen-domain/src/definition/port.rs`、`crates/pulsen/src/adapter/workflow_store.rs`、`crates/pulsen/src/cli/render.rs`、`crates/pulsen-conformance/src/workflow_store.rs`、`crates/pulsen-conformance/HOOKS.md`
- **変更内容:**
  - `port.rs`: `Parse(WorkflowParseError)`（82行付近）を `Parse { error: WorkflowParseError, resolved_from: PathBuf }` に改める。doc に「解決先を構造として持つ — `--workflow` を名前で指定した場合、利用者が直接書いていないパスを案内できるのはポート側だけ」を書く。`PathBuf` は3行目で import 済み。
  - `adapter/workflow_store.rs`: 構築3箇所（72〜79行の `yaml::parse_document` の失敗、`decode`、`WorkflowAssembler::assemble`）を新しい形にする。`map_err(WorkflowLoadError::Parse)` の関数参照（78 / 79行）は `resolved` を添えるクロージャになる。
  - `adapter/workflow_store.rs`: **`parse_document` 失敗アーム（74行）の `at(&resolved, &error.message)` をやめ、`message` を原因のみにする。** `Parse` が `resolved_from` を持ち `render.rs` が必ずそれを出すようになるため、前置を残すと同じパスが1つの案内に2回現れる（ステップ1 の設計判断・adr.md ADR-005）。あわせて `at()` の doc（105〜106行）から `WorkflowParseError::YamlSyntax` の記述を外し、利用者を `read_error` の `WorkflowLoadError::Io` だけにする。
    - 前置を外しても既存テストは通る: 受け入れテスト `crates/pulsen/tests/cli_add_error.rs`（197 / 207行）は `["YAML 構文エラー", "位置:", "行"]` の3語しか見ず、適合スイート（`crates/pulsen-conformance/src/workflow_store.rs:435-437`）も `message` の非空と `location` の存在しか主張していない。
  - `cli/render.rs`: `workflow_load_error`（525行）の `Parse` アームで `resolved_from` を案内に出す（`NotFound { attempted }`（529行）と同じ「解決を試みたパス」の見せ方に揃える）。
  - `pulsen-conformance/src/workflow_store.rs`: ヘルパー `expect_parse_error`（846行）を `(WorkflowParseError, PathBuf)` を返す形に改め、4つの match アーム（756〜856行付近）を新しい変種の形に追従させる。**`tc_port_workflow_store_017`（434行付近）で `resolved_from == harness.expected_path_for_name("wf")` を1回だけ主張する**（`require!` で skip 予算に載せる。既定実装は `None`）。適合ケースの本数は増やさない。
  - `HOOKS.md`: `TC-port-workflow-store-017` の行（140行）の「組み立て手段」を `put_named` から `put_named + expected_path_for_name（resolved_from の期待値）` に改める。冒頭が「フックを足すときはこの表も更新する」を規約として宣言しており（`.adr/1-port-conformance-suite-and-harness-hooks.md`）、放置すると表が嘘になる。区分は B のままで件数（`## WorkflowStore（31行 / A 0・B 30・C 1）`・冒頭196行）は変わらない。
- **理由:** Issue 本文が「ポート表は spec が確定させており、実装側では変えていない」と述べているとおり、実装が現在の形なのは Issue #1 の受け入れ基準（ポート表との1:1一致）を守った結果にすぎない。ポート表を直したステップ1 のあと、この追従を欠くと spec と実装の乖離が1件残る。適合スイートに1主張を足すのは、契約にフィールドが増えたのに適合ケースが1件もそれを検証しない状態を避けるため（B6 で採った扱いと対称）。

### 18. C5 の実装を `spec` に合わせる（`NotifyOutcome::Failed` の分類化）

- **対象ファイル:** `crates/pulsen-domain/src/execution/notification.rs`、`crates/pulsen-domain/src/execution/mod.rs`、`crates/pulsen/src/application/tick/mod.rs`、`crates/pulsen/src/application/tick/notify.rs`、`crates/pulsen/src/cli/render.rs`
- **変更内容:**
  - `notification.rs`: `NotifyOutcome::Failed { detail: String }` を `Failed { cause: NotifyFailureCause }` にし、`NotifyFailureCause = ExitedNonZero { exit: ExitCode } | TimedOut | FailedToStart { message: String }` を加える。`interpret_notify_completion` の3つの `format!`（43 / 46 / 52行）を分類の構築に置き換える（完成文言はドメインから消える）。ユニットテスト `通知の失敗の3つの原因は説明から判別できる`（122行）を、文言の差ではなく変種の判別で主張する形に書き換え、テストヘルパー `detail_of`（76行）を落とす。`NOTIFY_TIMEOUT` の doc コメント（32行）は残す（timeout を置く理由であり C5 の対象ではない）。
  - `execution/mod.rs`: 18行の再エクスポートを `pub use notification::{NotificationService, NotifyFailureCause, NotifyOutcome};` に改める。`mod notification;`（9行）は非公開のままなので、これを広げないと `application` / `cli` から `NotifyFailureCause` を名指しできずコンパイルが通らない。
  - `tick/mod.rs`: `TickIssue::NotifyFailed { task_id, message: String }`（203行）を `NotifyFailed { task_id, cause: NotifyFailureCause }` にする。
  - `tick/notify.rs`: `Delivery::Attempted(NotifyOutcome::Failed { detail })` の2アームと `report_failure` を `cause` を運ぶ形にする。
  - `cli/render.rs`: `TickIssue::NotifyFailed` の文言（246行付近）を `cause` の網羅 `match` から組み立てる。`TimedOut` の秒数は `NotificationService::NOTIFY_TIMEOUT` を読む。見出しの振り分け（`issue_outcome`、132行付近）は `NotifyFailed { .. }` で受けているため変更しない。
  - 結合テスト（`crates/pulsen/tests/tick_notify.rs:356`、`tick_scan.rs:546`）は `NotifyFailed { .. }` で受けているため変更を要さない。変更が要る場合は「文言に依存したテスト」なので、その依存自体を外す。
- **理由:** `.adr/2-transition-error-holds-classification-only.md` が「表示専用のエラーは分類だけを持つ」を一般規則として宣言しており、`NotifyOutcome::Failed.detail` は帳簿に残らず `cli::render` にしか流れないので、この規則が効く側である（対称に見える `JudgeConclusion::JudgeFailure { detail }` は `last_failure` として永続化されるため `.adr/2-persisted-explanations-come-from-domain-describe.md` が効く側で、性質が違う）。`.adr/3-notification-procedure-layering.md` の代替案節が「分類化は spec 追従の提起に回す」と本 Issue を反映先に名指ししているため、ここで反映しないと同じ判断が3回目の Issue へ持ち越される。

### 19. A2 / C5 に対応する `.adr/` エントリを更新する

- **対象ファイル:** `.adr/1-workflow-error-file-path-goes-into-free-form-messages.md`、`.adr/1-schema-error-location-is-logical.md`、`.adr/3-notification-procedure-layering.md`
- **変更内容:**
  - `1-workflow-error-file-path-goes-into-free-form-messages.md`（A2）: この ADR が回避策を採った唯一の理由は「`WorkflowLoadError` の12種は spec のポート表で確定しており、フィールドを増やすとポート表との1:1一致が壊れる」だった。ポート表を直した本 Issue でその前提が消えたため、ステータスを「置き換え済み」相当に改め、決定・影響の各節に「`Parse` は `resolved_from` を構造として持つ形に改めた（Issue #9）。自由形式のメッセージへの前置に残るのは `Io { message }` のみ」と、`location` に論理位置だけを載せる部分は依然として有効であることを書く。
  - **`1-schema-error-location-is-logical.md`（A2）**: A2 でこの ADR の**決定理由と影響の一部が偽になる**ため更新する。ADR を無効化するのではなく、前提の変化を書き足す形にする（`location` を論理位置に限る決定本体は有効なまま）。
    - 決定節の但し書き（18行）「ただしワークフロー定義のスキーマ違反(`UnknownKey` / `InvalidValue`)は絶対パスを伴わず、論理位置だけで示す。**解決先を知っているのはストアのアダプターだけで、パスを載せられる場所が自由形式のメッセージに限られるため**」— 後半の理由が成立しなくなる（`Parse { resolved_from }` という構造上の置き場ができる）。「`location` は論理位置のみを指す。対象ファイルは `WorkflowLoadError::Parse { resolved_from }` が構造として持つ（Issue #9）」へ改める。
    - 影響節のトレードオフ2つ目（31行）「ワークフロー定義を名前で指定した場合、…スキーマ違反の案内にそのパスが出ない。ポート表に解決先を持たせる改訂は spec 側の追従として提起する」— 提起先が本 Issue であり、ここで解消されたため、解消済みとして書き換える。
  - `3-notification-procedure-layering.md`（C5）: **決定節（29行）の `interpret_notify_completion(&CommandCompletion) -> NotifyOutcome`(`Delivered` / `Failed { detail }`) を `Failed { cause: NotifyFailureCause }` に直す。** ここを直さないと、同じ節の中に `Failed { detail }` と `Failed { cause }` が並ぶ ADR が残る。加えて、検討した代替案節の最終項「`NotifyOutcome::Failed` を分類(非0終了 / timeout / 起動不能)にする … 分類化は spec 追従の提起に回す」を代替案から外し、決定節へ「成否の解釈はドメインに置き、**失敗の原因は分類として持つ**（`Failed { cause: NotifyFailureCause }`）」として移す。影響節のトレードオフ「`NotifyOutcome::Failed { detail }` は帳簿に永続化されない完成文言をドメインが持つ」を削り、判定との非対称（`JudgeFailure` は帳簿に残るため文言をドメインが持つ）が**性質の違いに基づく意図された非対称**であることを書く。
- **理由:** Issue #9 の完了条件が「実装を直す判断になったものは、対応する `.adr/` エントリも更新する」と明示している。更新しないと、`.adr/` を根拠に読む後続の実装者が「現行の形（回避策・`detail`）が正しい」と読み続ける。

### 20. 25件の消化と横断確認

- **対象:** `spec/` 全体
- **変更内容（確認のみ）:**
  - `.thread/9/research.md` の25件対応表と波及表の全行について、spec 本体と台帳の両方に差分があることを確認する。
  - **言い換え前の語が残っていないことを `grep` で確認する。** 各語は「正しく実装すれば0件になり、無関係な行（doc コメント等）を拾わない」ことを実測で確認したもので、括弧内は現時点のヒット数。

    | # | コマンド | 現在 | 由来 |
    |---|---|---|---|
    | 1 | `grep -rn 'InvariantViolated' spec/` | 12 | B8/C1 |
    | 2 | `grep -rn "expected: &'static str" spec/` | 1 | B8 |
    | 3 | `grep -rn 'InconsistentRunFiles { message' spec/` | 1 | B9/C8 |
    | 4 | `grep -n 'message: String' spec/usecases/execution.md` | 1 | B4 |
    | 5 | `grep -rn 'の結果としてのみ' spec/` | 2 | B1（`CommandTemplate::expand` 自体は2経路の一方として残るため、語は「の結果としてのみ」を使う） |
    | 6 | ``grep -rn 'いずれも `parse' spec/`` | 1 | A4（`parse` 単体・`でのみ生成` 単体は他の行を拾う） |
    | 7 | `grep -rn '拡張子を除くと空になる' spec/` | 3 | A3（`拡張子を除くと空` 単体では0件にならない — ステップ7 の置換後テキスト自身が「『拡張子を除くと空』は作れない」という why としてこの語を含む） |
    | 8 | `grep -rnE 'スナップショットフィールドのみ(を)?構文不正' spec/` | 4 | A6（アーカイブ側の2行は「のみ構文不正」で助詞が無いため `(を)?` が要る） |
    | 9 | `grep -rn -- '-> RunningDecision' spec/` | 1 | C3 |
    | 10 | `grep -rn 'RunningClassifier の2段規則' spec/` | 3 | C3（本体・台帳の両方を拾う） |
    | 11 | `grep -rn -- '-> JudgeOutcome' spec/` | 1 | C4 |
    | 12 | `grep -rn 'Parse(' spec/` | 28 | A2（本体1 + 台帳1 + 適合ケース13 + 台帳13） |

  - `spec/inventory/*.md` の新規行 ID がグループ内で一意かつ最大番号 + 1 であること、既存 ID が1つも変わっていないこと、5ファイルすべての `最終同期` が更新されていることを確認する。
  - `git diff --name-only crates/` に現れるのが AC-Z3 の9ファイルだけであること（A2 / C5 以外の実装変更が混ざっていないこと）を確認する。`crates/pulsen-conformance/HOOKS.md` の件数（冒頭196行・各節の見出し）は据え置きで、変わるのは `TC-port-workflow-store-017` の「組み立て手段」の1セルだけであることも確認する。
  - **C5 の完成文言がドメインから消えたことを確認する。** `grep -rnE '通知コマンドが終了コード|秒のうちに終了しませんでした|通知コマンドを起動できませんでした' crates/pulsen-domain/`（現在3件）と `grep -rn 'detail' crates/pulsen-domain/src/execution/notification.rs`（現在9件）がいずれも0件になること。`grep -rn '通知コマンドが' crates/pulsen-domain/` は使わない — `notification.rs:32` の `NOTIFY_TIMEOUT` の存在理由を述べる doc コメントを拾い、正しく実装しても1件残る（この doc は `.adr/2026-08-11-notify-cmd-timeout.md` 由来の why であり C5 が消す対象ではない）。
  - **A2 の前置が二重表示になっていないことを確認する。** `grep -n 'at(' crates/pulsen/src/adapter/workflow_store.rs` の呼び出し元が `read_error`（`Io`）の1箇所だけになること（AC-A2b）。
  - `cargo fmt --check` / `cargo clippy` / `cargo test` を実行して通ることを確認する。
  - `.adr/` の3ファイル（`1-workflow-error-file-path-goes-into-free-form-messages.md` / `1-schema-error-location-is-logical.md` / `3-notification-procedure-layering.md`）が更新されていることを確認する（Issue の完了条件）。
  - `.thread/9/adr.md` に ADR-001〜005 が記録済みで、いずれも現在の方針を述べていることを確認する（ADR-002 が「本 Issue で spec と実装を同時に変える」であること、ADR-004 が `Failed { cause }` であること、ADR-005 が解決先の前置の規則であること）。実装中に新たな判断が出た場合のみここへ追記する。
- **理由:** 台帳の追従漏れは下流のすべての検証を古い基準で走らせるが、そのこと自体は検出されない。ここが唯一の安全網になる。実装側は `cargo` が安全網になるが、`.adr/` の更新漏れはどちらの網にもかからないので明示的に確認する。
