# ADR — Issue #3: tick による観測・判定・ステータス遷移(リトライ・凍結・通知)

spec に書かれていない実装手段の判断だけを記録する。判定プロトコルの4値解釈(ADR-008)・連続失敗カウンタ(ADR-009)・`NOTIFY_TIMEOUT`(ADR-018)・凍結の受け渡し(ADR-097)など、既に決着している事項は再記録しない。

## ADR-001: `CommandRunner` の timeout は追加依存なしで、`try_wait` のポーリングとして組む

### Status

Proposed

### Context

`CommandRunner::run` は `timeout: Option<&DurationSpec>` を受け取り、超過時に「起動したプロセスを終了させて `TimedOut` を返す」ことを契約とする(`spec/domains/execution.md#commandrunner`、TC-port-command-runner-012)。判定コマンド(`judge_timeout`、既定60秒)と通知コマンド(`NOTIFY_TIMEOUT`、60秒)の両方がこのポートに乗る。

ところが std に「子プロセスを期限つきで待つ」API は無い。`Child::wait` はブロックし、期限を渡せない。ADR-023 が本番依存を6クレートに閉じており `wait-timeout` / `nix` / `tokio` はいずれも入っていない。workspace lints の `unsafe_code = "forbid"` により `waitpid` の直接呼び出しもできない。

選択肢は3つあった。

1. `wait-timeout` 等のクレートを足す
2. `Child` を別スレッドへ move して `wait()` させ、`mpsc::recv_timeout` で期限を測る
3. 呼び出しスレッドで `Child::try_wait()` を短い間隔でポーリングし、期限超過で `Child::kill()` → `wait()`

### Decision

**3(`try_wait` のポーリング)を採る。** 依存を増やさず、`unsafe` も使わない。

2 を採らないのは、`Child` をスレッドへ move すると呼び出し側に終了させる手段が残らないためである。契約は「timeout 後に生存していない」ことまで要求しており、kill するには `Child` の所有権(`kill` は `&mut self`)が要る。pid だけを取り出して `ProcessController::kill` 相当の経路で殺す案は、CommandRunner に実行単位の同定という別の関心を持ち込むうえ、判定コマンドは新しいプロセスグループの長ではないので `KillIdent` の語彙がそもそも合わない。

1 を採らないのは、ADR-023 の依存方針を1つのポートの実装都合で緩めることになるためである。ポーリングで契約を満たせる以上、依存を増やす理由が立たない。

ポーリング間隔は「判定・通知の完了検出が体感の遅延にならない」ことと「1タスクあたりの `try_wait` 呼び出し回数が実用の範囲に収まる」ことの両方から選び、値は1箇所の定数に置く。timeout 未指定(`None`)のときはポーリングせずに `wait()` する — 期限が無いのに繰り返し起こす理由が無い。

### Consequences

- 良い点: 依存も `unsafe` も増えず、`Child` の所有権を保ったまま「超過 → kill → wait」を素直に書ける
- トレードオフ: 完了の検出が最大でポーリング間隔ぶん遅れる。timeout の判定にも同じ粒度の誤差が乗るため、適合ケース(TC-port-command-runner-012 / 013)の timeout は間隔より十分大きく取る
- トレードオフ: 待機中に呼び出しスレッドが定期的に起きる。tick は排他ロックを保持したままこの待機に入るので、間隔を短くしすぎると cron 実行での無駄が積み上がる
- 期限は絶対時刻ではなく開始時刻からの経過で測る。`DurationSpec` が上限のない秒数を受理する以上、`Instant + Duration` は config 由来のパニック経路になるため
- `kill` するのは起動した直接の子だけで、その子が起こした孫は残りうる。契約が要求するのは「起動されたプロセスは終了させられている」ことなので満たすが、判定コマンドが孫を残す設計であることを利用者に要求しない(残存は許容する)

---

## ADR-002: 実行単位の終了は同定子をそのまま渡せる外部コマンドで行い、同定できないときは何も殺さない

### Status

Proposed

### Context

`ProcessController::kill(ident: &KillIdent)` と `try_kill_remnants(ident)` は、プロセスグループ相当の実行単位を一括終了する。ADR-075 が決めたのはデタッチ起動・kill 同定子の**記録**・起動時刻の観測までで、終了操作そのものは未決だった。

`KillIdent` は POSIX で `-<pgid>`、Windows で `<pid>` の文字列として**タスクファイルと pid ファイルに永続化**される(ADR-075)。ツールを再起動した後でも、この値だけで kill できることが契約である(TC-port-process-controller-012)。プロセスハンドルは保持できない。

`try_kill_remnants` の期待結果には `NotIdentifiable` があり、「いかなるプロセスも終了させない(無関係なプロセスの誤殺がない)」ことまでが期待に含まれる(TC-port-process-controller-015)。

### Decision

**終了操作は外部コマンドの起動で行い、`KillIdent` の文字列をそのまま引数に渡す。** OS 依存の分岐は `adapter/process.rs` に閉じたままにする(ADR-075 と同じ隔離)。

- POSIX: `kill` に負の PGID 表記を渡す。`KillIdent` はこの形式で作られているので、アダプターが文字列を組み直さない — 組み直すと、記録した値と実際に殺す対象がずれる経路ができる。`-<pgid>` はオプションと字面で区別できないため、シグナルの後に `--` を置いてオペランドであることを明示する(実体によって解釈が割れ、成否がどちらの向きにも反転する)
- Windows: プロセスツリーを対象とする終了コマンドに `<pid>` を渡す

**`try_kill_remnants` は、実行単位のメンバーを列挙できたときにだけ終了を実行する。** 列挙には ADR-075 / ADR-076 が既に注入している同定情報の取得元を使う。列挙できない(取得機構が失敗する・実行単位が既に存在しない)場合は `NotIdentifiable` を返し、**終了コマンドを1度も起動しない**。列挙が分離するのは「実行単位が既に消滅している」場合であり、そこを `NotIdentifiable` として結末の解像度に載せる。ポートの入力が `KillIdent` だけである以上、PGID が別の実行単位へ再利用されていることはこの列挙では検出できない — `starttime` 照合が PID 再利用に対して持つ強さは、実行単位の側には無い。

`kill` は呼び出し前提が「`IdentityCheck` が `Alive`」であり、照合はユースケースが済ませている。こちらは列挙を挟まずそのまま終了を実行し、失敗は `KillError::Failed { message }` として値で返す(分類には使わない)。

### Consequences

- 良い点: 記録した同定子と終了対象が構成上ずれない。取得元の注入(ADR-076)がそのまま終了側にも効き、適合ケースの `NotIdentifiable` を確定的に作れる
- 良い点: 消滅した実行単位へ終了を投げない判断がコードの分岐として現れるので、レビューで確認できる
- トレードオフ: `try_kill_remnants` が列挙のぶん遅くなり、列挙と終了の間に生まれたプロセスは取り逃す。ベストエフォートの契約(結果は分類に影響しない)の範囲に収まる
- 残る穴: 実行単位ID の再利用は塞げない。恒久対策は `try_kill_remnants` に記録済み `ProcessStartTime` を渡せるようにするポート契約の変更で、本スライスの範囲外のため spec 追従の提起として Issue コメントに回す
- 残る穴: Windows は同定子がラッパーの pid なので、ラッパー死亡後という呼び出し前提の下で列挙は必ず空になり、残存終了は構造上 `NotIdentifiable` にしかならない。ジョブオブジェクトを持たない現在の設計の帰結で、#10(Windows 実機検証)へ引き継ぐ
- トレードオフ: 終了コマンドの実体に依存する。既定は POSIX が絶対パス(`/bin/kill`)、Windows が PATH 解決の固定名(`taskkill`)で固定し(ADR-075 の取得元と同じ扱い — 既定の性質がプラットフォームで揃わない点も同じで、検証していない絶対パスを固定すると終了そのものが不能になる)、構築時に注入できるようにして適合ケースが失敗状況を作れるようにする

---

## ADR-003: 共通手続き notify は通知に必要な3値と保存手段を分けて受け取り、Task と DegradedTask の両方から呼べる形にする

### Status

Proposed

### Context

共通手続き notify は `TASK_ID` / `WORKFLOW` / `TASK_STATUS` を env に組み、`Exited(0)` のときだけ `mark_notified(now)` → `save` する。呼び出し元は5つある — 上限超過3経路(`Tick::commit` の `Freeze::Frozen` の枝に集約済み)、`Branch::Notify` アーム、`TaskRecord::SnapshotUnreadable` かつ未通知 stopped の再通知。

最後の1つだけ対象が `DegradedTask` で、保存は `save_degraded` になる(`spec/domains/task.md#degradedtaskスナップショット破損タスク`、PAGE-tick-007、TC-exec-tick-158)。ところが既存の集約点 `Tick::commit` は `&Task` 専用で、`DegradedTask` を通せない。

`DegradedTask` を通せないまま放置すると、「スナップショット破損タスクを凍結 → notify_cmd 失敗」の後に再通知が永遠に行われず、requirements §8 の at-least-once が破れる。

### Decision

**通知の実行と `mark_notified` の保存を分ける。** 通知の実行は `TaskId` / `WorkflowName` / `StatusName` の3値だけを受け取る関数にし、その3値はどちらの型からも同じ形で取れる(`DegradedTask` はスナップショットを持たないが、この3つはいずれもスナップショット非依存のフィールドである — これが「通知は定義非依存」の実体)。保存は呼び出し側が `save` / `save_degraded` を選ぶ。

`Task` を扱う3経路は引き続き `Tick::commit` を通し(`Freeze` の受け渡しは ADR-097 のまま)、`DegradedTask` の再通知だけが `save_degraded` の経路を持つ。共通化のためにトレイトで抽象化することはしない — 2つの型に共通の遷移は本スライスでは `mark_notified` の1つだけで、抽象を先に置くと #5 が足す `abort` / `retry` の差異(`retry` は `DegradedTask` では警告付きで受理される)を吸収しきれない。

### Consequences

- 良い点: 通知の env 構成と成否の解釈が1箇所に閉じ、保存の違いだけが呼び出し側に残る。破損時にも at-least-once が維持される
- 良い点: `Tick::commit` の役割(保存 + 凍結の集計)が変わらないので、ADR-097 の判断がそのまま生きる
- トレードオフ: 通知アームが `Task` 用と `DegradedTask` 用の2本になる。分岐は「どちらの保存を呼ぶか」だけで、通知の判断は共有される
- #5 が `abort` を足すとき、`AbortTask` ユースケースが同じ関数を呼べる(そちらも `Task` / `DegradedTask` の両方を扱う)

---

## ADR-004: 手続きD 冒頭の不変条件3 の破れは、不変条件2 とは別の報告分類にする

### Status

Proposed

### Context

手続きDの冒頭は、`current_attempt` が None(不変条件2の破れ)に加えて `current_attempt.process` が None(不変条件3の破れ)を検査し、報告してスキップする(`spec/usecases/execution.md#手続きd-観測判定running`、TC-exec-tick-022)。

既存の `TickIssue::MissingCurrentAttempt` は #2 が手続きCのために置いたもので、attempt 参照そのものが無い場合だけを指す。不変条件3 の破れをこれに相乗りさせるか、新しい分類を足すかを決める必要があった。

ADR-096 は「遷移エラーの `MissingCurrentAttempt` と tick の報告分類の `MissingCurrentAttempt` は同じ事実の別文脈での報告」として重複を許したが、それは**同じ事実**だからである。

### Decision

**新しい分類を足す。** 不変条件2 と3 は破れの事実が違い、人間に求める修復も違う — 前者は attempt 参照そのものが失われている(run ディレクトリへの導線が無い)、後者は attempt はあるが同定情報が無い(pid ファイルから復元できる可能性がある)。同じ文言に畳むと、`cli::render` が出す案内が修復の入口を示せなくなる。

### Consequences

- 良い点: 表示から修復の手がかりが読み取れる。tick は書き込まずに人間へ委ねるので、報告の解像度がそのまま復旧の速さになる
- トレードオフ: `TickIssue` の変種が1つ増える。分類の列挙は網羅 `match` で表示側が受けるため、足し忘れはコンパイルエラーになる

---

## ADR-005: 判定 completed の確定はサマリーの新しいフィールド `judged` にする

### Status

Proposed

### Context

ADR-092 は「**タスクファイルへの書き込みを行った経路は必ずサマリーのいずれかのフィールドを埋める**」を不変とし、ADR-094 はその不変を満たすために `confirmed_running` を1つ足した。

本スライスが足す書き込み経路のうち、`skip_run` は `skipped_back`、`advance` は `transitioned`、`mark_notified` は `notified`、失敗の記録3種(`fail_run` / `record_judge_failure` と残存終了の報告)は `errors` に収まる。収まらないのは `complete_run` だけである — spec の出力 DTO に「判定が成功として確定した」を受けるフィールドが無い。

`transitioned` は「タスクステータスが遷移したタスク」で、`advance` の結果を指す語として確定している。`complete_run` を混ぜると、1タスク1tick1ステップの2つのステップが同じフィールドに現れ、`advance` を行わない tick と行った tick を表示から区別できなくなる。

放置すると、主経路である「exit 0 の観測」が毎回「処理対象のタスクはありませんでした。」と表示される。

### Decision

**フィールド `judged: Vec<TaskId>` を1つ足す。** 語義は「判定 completed を確定したタスク」で、`confirmed_running` と同じ扱い(spec の9フィールドに載らない正常な前進)にする。

失敗の側を `errors` に載せる ADR-094 の判断はそのまま使う。判定の3値のうち `Skipped` は spec が `skipped_back` を用意しており、`Failed` は「記録した失敗」として `errors` の定義に収まる。フィールドが要るのは `Completed` だけになる。

### Consequences

- 良い点: ADR-092 の不変が本スライスの全経路で成立し、判定が確定した tick が必ず表示に現れる
- 良い点: `transitioned` が spec の語義(ステータスの遷移)のまま残り、`advance` を入れた本スライスでも意味がぶれない
- トレードオフ: 出力 DTO が spec から2フィールド分ずれる(`confirmed_running` と合わせて)。spec 追従の提起に1件足す
- 表示は「判定確定」の行として `起動確認` と `遷移` の間に並ぶ。実行状態の遷移順と同じ並びになる

---

## ADR-006: 通知済みの凍結への再通知は専用の遷移エラーで拒否する

### Status

Proposed

### Context

`mark_notified` の前提は `Stopped { notified_at: None }` であり、状態の判別子(`ExecutionStateKind`)だけでは表せない。`TransitionError::InvalidState { expected, actual }` は判別子の組しか持たないため、通知済みの `Stopped` を拒否すると `expected = [stopped]` / `actual = stopped` という自己矛盾した報告になる。

spec のエラー型は5種で、ADR-096 が「分類だけを持つ」ことを決めている。

### Decision

**変種 `AlreadyNotified` を足す。** `confirm_workspace` の再確定を拒否する `WorkspaceAlreadySet` と同じ形 — 「達成済みの操作を二度行おうとした」ことを、前提状態の不一致とは別の分類にする。

tick の通知アームは `notified_at` が None のときだけ `mark_notified` を呼ぶので、この分類が実際に出るのは帳簿が並行に書き換わった場合に限る。それでも値として持つのは、ドメインの前提を型と値で閉じるため — 呼び出し側の事前検査に依存すると、#5 が足す `abort` / `retry` の経路で同じ検査を書き忘れても気づけない。

### Consequences

- 良い点: 「通知の前提を満たさない」の理由が表示から読み取れる(状態が違うのか、既に通知済みなのか)
- 良い点: `DegradedTask::mark_notified` が同じ規則を共有でき、規則の実体を1つの関数に置ける
- トレードオフ: `TransitionError` が spec の5種から6種になる。ADR-096 の「分類だけを持つ」性質は保たれるが、spec 追従の提起に1件足す

---

## ADR-007: 終了操作の実体も構築時に注入し、観測スイートの異常系を確定的に走らせる

### Status

Proposed

### Context

plan.md は TC-port-process-controller-010 / 013 / 015 / 016 を「実行環境が前提を作れないとスキップで終わる行」に数え、010 は `permission_restrictions_effective` を判定にすると見込んでいた(起動時刻の取得手段への読み取りを塞ぐ)。

ところが ADR-076 が既に、同定情報の**取得元を構築時に注入**する形を作っている。存在しないパスを取得元にしたコントローラは、権限にも root の可否にも依存せず「取得機構そのものの失敗」を返す。

### Decision

**終了操作の実体にも同じ注入口(`TerminatorSource`)を置き、4行とも別ハンドルの注入で確定的に走らせる。**

- 010(取得機構の失敗)/ 015(同定できない)→ 取得元を壊したコントローラ
- 013 / 016(終了操作の失敗)→ 終了操作の実体を壊したコントローラ

015 は「いかなるプロセスも終了させない」ことまでが期待なので、生存中の実行単位を作ってメンバーが生き残ることで観測する。列挙が失敗した時点で終了操作を1度も起動しない実装だけがこれを通る。

スキップの宣言に残すのは、実行単位(プロセスグループ相当)そのものを作れない・その一部だけを終了させられないプラットフォームだけになる(011〜016 の6行)。

### Consequences

- 良い点: 権限操作にも root の可否にも依存せず、誤殺しないことの主張が常に走る
- 良い点: 既定の実体は絶対パス(または PATH 解決の固定名)のままで、本番の構成は変わらない
- トレードオフ: `SystemProcessController` に注入口が1つ増える。既定を持つ後付けのメソッド(`with_terminator_source`)にして、合成ルートの呼び出しは変えない

---

## ADR-008: 不変条件4の破れは遷移エラーに相乗りさせず、tick の報告分類にする

### Status

Proposed

### Context

手続きDの判定アームは、判定コマンドを持つステータスで `task.workspace()` が None のとき、判定コマンドの起動も書き込みも行わずに報告する。実装はこのとき `TransitionError::WorkspaceNotSet` を組み立てて `report_transition` に渡していた。

ところがこの経路は遷移関数を一度も呼んでいない。`WorkspaceNotSet` は spec 上「workspace 未確定での `record_launching`」を指す前提の破れであり(`spec/domains/task.md#エラー型`)、遷移関数だけが返す値である。ユースケースが自作すると、表示は「遷移の前提が成立しません(ワークスペースが未確定)」になり、遷移を試みてすらいないという事実と食い違う。

### Decision

**`TickIssue::MissingWorkspace { task_id }` を足し、`WorkspaceNotSet` の自作をやめる。** 根拠は ADR-004 と同じ — 破れの事実も人間に求める修復も違うなら分類を分ける。ここで起きたのは「判定コマンドへ渡す `WORKSPACE` を組めなかった」であり、修復は帳簿の `workspace` を埋めることになる。

`TransitionError` 側は触らない。`WorkspaceNotSet` は `record_launching` の前提として引き続き値で返る — 遷移関数を全域に保つ判断(ADR-096)はそのまま生きる。

### Consequences

- 良い点: ドメインのエラー型を、それを返す遷移関数を呼ばずに作る経路が消える。値の出所と意味が構成上一致する
- 良い点: 表示が修復の入口(帳簿の `workspace`)を示せる。書き込みが無いので `cli::render` の振り分けでは「スキップ」の見出しに入る(ADR-098)
- トレードオフ: `TickIssue` の変種がもう1つ増える。網羅 `match` が表示側で受けるため足し忘れはコンパイルエラーになる

---

## ADR-009: 2段規則は `classify_alive` の返り値型で担保する

### Status

Proposed

### Context

running の分類は2段で行う。1段目(exit の有無)は観測を行うユースケース側にあり、`RunningClassifier::classify_alive` が受け持つのは2段目(生存)だけである — exit があれば実行は終了しており、生存観測の一過性の失敗で判定を遅延させない。

ところが `classify_alive` の返り値は4値の `RunningDecision` のままだった。ユースケース側の網羅 `match` に `RunningDecision::Judge(_) => unreachable!(...)` が現れ、2段規則の担保が doc コメントとパニック経路になっていた。規則が守られていることを型は何も述べていない。

### Decision

**`AliveDecision`(`KeepRunning` / `KillOnTimeout` / `DiedWithoutExit`)を足し、`classify_alive` の返り値をこれに絞る。**

`RunningDecision` は4値のまま残し、`From<AliveDecision>` で埋め込む。3値に畳んで `RunningDecision` を落とさないのは、台帳 `DOM-execution-008` の PASS 要件が `Judge(ExitCode)` を含む4値を要求しており(`spec/inventory/domain.md`)、落とすと AC-1 の1行が満たせなくなるためである。

ユースケースは1段目を `RunningDecision::Judge(exit)` として値にし、2段目を `.into()` で合流させてから1つの網羅 `match` で分岐する。分類を値にしてから分岐する形は ADR-091 と同じ。

### Consequences

- 良い点: `unreachable!` が消え、「生存の観測からは判定が導かれない」という規則の担保がコメントから型へ戻る
- 良い点: ドメインのユニットテストが2段目の3値だけを主張でき、`Judge` を作れない事実を型が述べる
- トレードオフ: 分類の型が2つになる。対応は `From` の1箇所に閉じ、3値がそのまま写ることをユニットテストで主張する

---

## ADR-010: 実行の失敗の根拠と残存の後始末も分類として持ち、文言は `cli::render` が組む

### Status

Proposed

### Context

ADR-081 は tick の `errors` を分類として持ち、文言は `cli::render` が組むと決めている。`TickIssue` のうち `RunFailed { message: String }` と `RemnantsUnhandled { message: String }` の2つだけが、この規約の外でユースケースが完成文言を組んでいた。

規約から外れた結果が実害に出ていた。失敗の文言は `judgement_detail(&exit)` が組んでいたが、この `exit` はエージェントの終了コードである。判定コマンドが exit 10(失敗)を返し、エージェント自身は 0 で終わっていた場合、報告は「実行が終了コード 0 で終了しました」になる。失敗と判断した主体が判定コマンドであることも、その終了コードが 10 だったことも読めない。

`RemnantOutcome` には `Killed` があり、これは後始末を残さない。報告の分類が文字列だったため、「後始末が残った」報告に `Killed` を載せる経路を型が禁じていなかった。

### Decision

**どちらも分類にする。`RunFailed { cause: RunFailureCause }` と `RemnantsUnhandled { remnants: RemnantsLeft }`。**

- `RunFailureCause` は**判断の主体**で分ける(`DefaultJudgement { exit }` / `JudgeCommand { exit }` / `TimedOut { timeout }` / `DiedWithoutExit`)。`JudgeCommand` も exit を運ぶが、それは判定コマンドが受け取った材料であって失敗の根拠ではない。`cli::render` は「判定コマンドが失敗と判定しました(実行の終了コードは N)」と組み、主体と材料を書き分ける
- 判定の結末はユースケース内の `Settled` に一度写し、失敗のときだけ根拠を伴わせる。「誰が失敗と判断したか」は結論と同時にしか分からず、結末だけを見て後から復元できない
- `RemnantsLeft` は `RemnantOutcome` をそのまま運ばず、報告を要する2値(`NotIdentifiable` / `Failed { message }`)へ写す。写像は `Option` を返す関数1つに閉じ、`Killed` がこの分類に現れる状態を型で表現不能にする

**あわせて、残存の報告を保存の成否から切り離す。** プロセスが残っているという事実はタスクファイルを書けたかと直交する — 後始末は人間が OS のツールで行うので、保存に失敗した tick でも報告する。

### Consequences

- 良い点: `errors` の全変種が ADR-081 の規約に揃い、規約の外にある変種が1つも無くなる
- 良い点: exit の出所が表示から読み分けられ、判定コマンドの失敗をエージェントの終了コードのせいに読む余地が消える
- 良い点: ユースケース層のテストが文言ではなく根拠の分類で主張でき、`TimedOut` と `DiedWithoutExit` の取り違えを検出できる
- トレードオフ: `RemnantsLeft::Failed` は原因の説明を文字列で運ぶ。ポート機構の失敗は単一の `Io` 文字列で表す(ADR-086)ので、ここでこれ以上は分類できない
- トレードオフ: 保存に失敗した tick で、1つのタスクが `SaveFailed` と `RemnantsUnhandled` の両方に現れうる。どちらも別の事実の報告なので重複ではない

---

## ADR-011: 通知の成否の解釈はドメインに置く

### Status

Proposed

### Context

`deliver` は `CommandCompletion` の4通りをユースケースの中で直接 `Sent` / `Failed(String)` に落としていた。この規則(`Exited(0)` だけが `notified_at` を書く根拠になり、非0終了・timeout・起動不能はいずれも書かずに終える)は、stopped を書いたすべての経路が共有しなければ requirements §8 の at-least-once が経路ごとに破れる。

ADR-003 は通知の実行と保存を分け、Consequences に「#5 が `abort` を足すとき `AbortTask` ユースケースが同じ関数を呼べる」と書いたが、成否の解釈がユースケース側にある限りそれは成立していなかった — 呼べるのは env の組み立てだけで、解釈は呼び出し側ごとに書き直しになる。

ドメインには対称の関数が既にある。`JudgementService::interpret_judge_completion` は同じ `CommandCompletion` を判定の結論へ解釈している。

### Decision

**`NotificationService::interpret_notify_completion(&CommandCompletion) -> NotifyOutcome`(`Delivered` / `Failed { detail }`)をドメインに置く。**

ユースケースの `Delivery` は `NotConfigured` / `Attempted(NotifyOutcome)` に縮小する。ここに残るのは「そもそも通知を実行する構成か」という配線の分岐だけになり、通知が成功したと言える条件はドメインの1関数にしか無くなる。

### Consequences

- 良い点: ADR-003 の Consequences が構造として成立する。`Task` / `DegradedTask` の2本の通知アームも、#5 が足す `AbortTask` も、同じ関数を呼ぶだけで規則を共有する
- 良い点: 通知と判定で `CommandCompletion` の解釈位置が揃う。ポートの結末を解釈する場所がドメインに一本化される
- トレードオフ: `NotifyOutcome::Failed { detail }` は**帳簿に永続化されない**完成文言をドメインが持つ。ADR-096 の「表示専用のエラーは分類だけを持つ」に厳密に沿うなら3変種(非0終了 / timeout / 起動不能)の分類にすべきだが、その形は隣の `JudgeConclusion::JudgeFailure { detail }`(こちらは帳簿に残るため ADR-090 で文言をドメインに置く側)と非対称になる。判定と通知で形を揃えるほうを採り、分類化は spec 追従の提起に回す

---

## ADR-012: スナップショット破損の報告は通知の実行と独立に積む

### Status

Proposed

### Context

`spec/usecases/execution.md` の走査は「`SnapshotUnreadable(degraded)` → 定義依存の判断はすべてスキップして報告。ただし `Stopped { notified_at: None }` なら共通手続き notify を実行する」と書かれている。実装はこの「ただし」を報告の**置換**と読み、未通知の凍結の枝では報告を積まずに notify だけを行っていた。

その結果、notify_cmd 未定義(既定構成)で未通知の凍結かつスナップショット破損のタスクは、`errors` にも `notified` にも現れない。tick は毎回このタスクを走査しながらサマリーは空のままで、cron 運用の唯一の窓に「処理対象のタスクはありませんでした。」と出る。破損したタスクが黙って消え続ける。

書き込みが無い経路なので ADR-092 の不変(書き込んだ経路は必ずいずれかのフィールドを埋める)そのものは破れていない。破れるのは、その不変を支えた根拠 — cron 運用ではこの出力が唯一の窓である — のほうである。

### Decision

**「ただし」を報告の追加と読む。** `SnapshotUnreadable` の報告は実行状態によらず積み、そのうえで未通知の凍結だけが notify へ進む。修復が要るという事実は通知の成否と独立であり、片方が起きたことでもう片方の報告が消える理由が無い。

### Consequences

- 良い点: 破損したタスクは構成によらず毎 tick 表示に現れる。修復の必要が notify_cmd の有無で黙って消えない
- 良い点: 通知に成功した tick では `errors` と `notified` の両方に現れ、「破損しているが凍結の通知は済んだ」と読める
- トレードオフ: 修復されるまで毎 tick 同じ報告が出る。破損の報告は他の実行状態では既に毎 tick 出ているので、扱いが揃うほうへ寄せる

---

## ADR-013: 実行単位フィクスチャの能力も probe で1度だけ判定し、実行ファイルの不在は失敗側に置く

### Status

Proposed

### Context

ProcessController 観測スイート(TC-011〜016)の許容集合は `cfg!(unix)` で6行をまとめて宣言していた。ADR-055 は許容の宣言を「プラットフォームではなく環境の能力から実行時に決める」と定め、`cfg(unix)` での許容件数決めを明示的に却下している。ADR-071 はそれを OS 差の吸収全体に掛かる原則にした。ADR-073 はロック保持フィクスチャに対して、能力を probe で1度だけ測り、区別を「スキップの宣言だけで『なぜ走らなかったか』と『次に何をすればよいか』が定まるか」で能力側と失敗側に振り分ける基準を確立している。

実行単位フィクスチャだけがその外に残っていた。実行ファイル(`examples/agent_probe` / `examples/spawn_probe`)の不在は unix では宣言の外なので失敗するが、`cfg!(unix)` が偽の環境では6件まとめて静かな緑になる。逆に、実行単位は起こせるがその一部だけを終了させられない環境では、011/012/013/015 まで一緒に落ちる。

### Decision

**基準は ADR-073 のものをそのまま使い、実行単位フィクスチャに掛ける。** この場で決めたのは適用の形だけで、基準そのものは決め直さない。

- 能力を4つに分ける — `Partitionable` / `WholeOnly` / `Unavailable` / `ProgramMissing`。前3つは能力側、`ProgramMissing` は失敗側(原因も回避方法も一意で、緑にすると作り忘れとビルド構成の誤りを隠す)
- 許容集合を2つに割る — 実行単位そのものを要する4件(011/012/013/015)と、その一部だけの終了を要する2件(014/016)。`WholeOnly` の環境では後者だけを許容する。取得機構の失敗(TC-010)は取得元の注入だけで作れるため(ADR-076)、どちらの集合にも現れない
- probe は実行単位を1度だけ実際に起こし、その一部を終了させられるかまでを**本番の手順そのもの**で測る。判定と実際のスキップが食い違わない形は ADR-055 の `permission_restrictions_effective` と同じ
- probe の置き場所は適用側(`crates/pulsen/tests/conformance_process_controller.rs`)。フィクスチャの実行ファイルは適用側のテストターゲットからしか解決できない(ADR-073 の置き場所の基準)

**あわせて `SIGNAL_CASES`(TC-024)の `cfg!(unix)` 宣言を削除する。** spec に但し書きが無く、要求するフックは `agent_command` だけで、期待も「非0の符号化値」までなので(ADR-082)、前提を作れない環境が存在しない — 測るべき能力が無い以上、宣言そのものが不要である。

### Consequences

- 良い点: 実行単位は起こせるが一部だけを終了させられない環境で、011/012/013/015 が走る。`cfg!(unix)` の宣言では6件まとめて落ちていた
- 良い点: 実行ファイルの不在が、どのプラットフォームでも緑にならない
- ADR-007 が「スキップの宣言に残るのは 011〜016 の6行」と述べた帰結が、本決定で能力ごとの2集合に分かれる。ADR-007 の本文は判断が下された時点の記録として書き換えない
- トレードオフ: ADR-073 のトレードオフをそのまま引き継ぐ。probe は無負荷の測定ではなく、偽陽性で能力側に倒れると許容集合が黙って広がる(スキップの一覧には現れる)
- トレードオフ: probe が本番のケースと同じ資源(実行単位1つ分のプロセス起動)を使う
- トレードオフ: `Unavailable` は原因を1つに定めず、実行単位を作れない環境とフィクスチャ側の退行を区別しない(ADR-073 の `SignalTimedOut` と同じ性質)

---

## ADR-014: ポートをまたぐ順序は、ダブル共通の採番で主張する

### Status

Proposed

### Context

通知の共通手続きの順序(凍結を書く → notify_cmd を実行する → `notified_at` を追記する)は、逆にすると失敗した通知が永久に再送されない — at-least-once を支える契約そのものである。

この順序は `TaskRepository` と `CommandRunner` の2ポートにまたがる。ダブルはそれぞれ独立した列に記録するため、`saved()` と `calls()` を別々に見るテストが主張できるのは同じポート内の順序だけになる。通知を先に起動して、成功したら stopped と `notified_at` を一度に書く実装でも、`saved` の列も `calls` の列もそれぞれの中では期待どおりに見えるので緑で通る。

### Decision

**`pulsen-conformance` のダブルに `RecordSeq` を持たせ、記録に添える。** 採番はプロセス内の1つの単調増加カウンタ(`static AtomicU64`)から採る。テストは複数ダブルの `*_in_order()` をマージし、採番で並べ直して1本の列として主張する。

既存アクセサ(`saved()` / `calls()`)は採番を落とした形で維持する — 順序を要さないテストを書き換えない。

`Rc<Cell<u64>>` をハーネスが明示的に共有する形を採らないのは、共有し忘れたダブルどうしでも比較がコンパイルを通り、無意味な順序を静かに主張できるためである。採番元がプロセスに1つなら、その失敗のしかたが存在しない。

**採番は `save_degraded` にも掛ける。** 通知の順序は書き戻し先の型で変わらないので、`Task` 経路だけを1本の列に並べても、縮退タスクの再通知は「`mark_notified` を先に保存する」実装を素通しにする。ダブル側は `saved_degraded_in_order()` を足して既存アクセサ(`saved_degraded()`)は据え置き、テスト側は保存先(`save` / `save_degraded`)と起動したコマンド(通知 / 判定)を区別したまま1本の列へ並べる — 契約は経路ごとに同じ順序を求めるので、取り違えを列の形として見せる。下の「今回付けるのは2つ」はこの1つを加えた3つ(`TaskRepository::save` / `TaskRepository::save_degraded` / `CommandRunner::run`)になる。なお縮退タスクは tick 内で凍結する経路を持たない(定義依存の判断をすべてスキップする)ため、この経路で主張できるのは「通知 → `notified_at` の追記」の2ステップである。

### Consequences

- 良い点: ポートをまたぐ順序の契約を値として主張でき、通知を先に起動する実装が赤になる
- 良い点: `RecordSeq` は比較だけができる不透明な値で、テストが具体的な番号に依存しない
- トレードオフ: 並行して走る別テストの記録が番号を飛ばす。主張に使えるのは前後関係だけで、差や連続性は使えない
- トレードオフ: 採番を要するポートごとにアクセサが2本になる。今回付けるのは `TaskRepository::save` と `CommandRunner::run` の2つに限り、順序の契約が無いメソッドには付けない

---

## ADR-015: 実行単位の終了は境界で parse し、成否は終了ステータスではなく消滅の観測で決める

### Status

Proposed

### Context

ADR-002 は「`KillIdent` の文字列をそのまま終了コマンドへ渡し、失敗は終了操作の失敗として値で返す」形を採った。実装するとこの2点がどちらも成立しない。

同定子の側。`KillIdent` は非空しか検査しない**永続化された不透明値**で、形式を知っているのはアダプターだけである(ADR-075)。手で編集された・破損した帳簿からも到達する。POSIX の `kill` で `-1` は「シグナルを送れる全プロセス」、`-0` / `0` は呼び出し側のプロセスグループ(= tick 自身と呼び出し元シェル)を指すため、そのまま渡す実装は帳簿の破損を最悪の誤殺に変換する。

終了ステータスの側。`kill` の実体でステータスの規則が割れている(Docker で実測)。

| 実体 | `-TERM -- -<pgid>` | `-TERM -<pgid>` | 不在のグループ(`--` あり / なし) |
|---|---|---|---|
| procps-ng 4.0.4 | rc=0・終了する | rc=0・**何も終了しない** | rc=1 / rc=0 |
| busybox 1.37 | rc=1(`invalid number '--'`)・**終了する** | rc=0・終了する | rc=2 / rc=1 |
| macOS `/bin/kill` | rc=0・終了する | — | — |

同じ引数に対して、終了させた実体が非0を返し、何も終了させなかった実体が0を返す。ステータスを成否に写すと、結末が実体によってどちらの向きにも反転する。

契約が `kill` の `Ok` に求めるのは「実行単位に属する全プロセスが終了する」ことであり(TC-port-process-controller-011 / 012)、これは**結果**であって終了操作の自己申告ではない。終了を捕まえるエージェント(`trap`)が生き残ったまま `Ok` を返す実装も同じ理由で契約を満たさない。

### Decision

**同定子はアダプターの境界で1度だけ parse する。** 終了操作を向けられる形だけを表す型(`terminate::UnitTarget`)を置き、POSIX は `-<n>`(`n >= 2`)、Windows は非0の `<pid>` からだけ作る。満たさない値では `kill` は `KillError::Failed`、`try_kill_remnants` は `NotIdentifiable` に写し、**終了操作を1度も起動しない**。同じ parse をメンバーの列挙(`unit_is_live`)と共有し、終了を向ける対象と観測する対象が構成上ずれないようにする。

**成否は消滅の観測で決める。** 終了操作の後、`unit_is_live` を `TERMINATION_GRACE`(2秒)まで `TERMINATION_POLL`(50ms)間隔でポーリングし、消滅を観測できたら終了ステータスによらず `Ok` を返す。

**猶予のうちに消えなければ強い終了へ昇格する。** POSIX は `-TERM` → `-KILL` の2段。Windows の `taskkill /T /F` は捕捉できる終了を持たないので段が1つしかなく、昇格しても同じ操作になる。

`--` はオペランドの明示として残す。外すと procps-ng が `-<pgid>` をシグナル指定として読み、**何も終了させずに rc=0** を返す。busybox が `--` に対して出す非0は、成否を観測で決める以上は結果に現れない。

本 ADR は ADR-002 の Decision のうち3点 — 同定子を組み直さずそのまま渡す / `kill` は列挙を挟まない / 失敗は終了操作の失敗として返す — を置き換える。`try_kill_remnants` が列挙できたときにだけ終了を実行する判断と、誤殺しない側に倒す方針そのものは変わらない。ADR-002 の本文は判断が下された時点の記録として書き換えない(ADR-013 が ADR-007 に採った扱いと同じ)。

### Consequences

- 良い点: 誤殺しないことが実体の挙動ではなく型で決まる。`-1` / `-0` / `0` や数値でない同定子は終了操作へ到達せず、「1度も起動しない」ことを痕跡を残す実体の注入で外から観測できる
- 良い点: 契約の語彙(「実行単位に属する全プロセスが終了する」)と成否の判定が一致し、`kill` の実体が入れ替わっても結末が反転しない
- トレードオフ: 観測機構の失敗(`Err(Io)`)は失敗に写さず、最終判定を終了ステータスに委ねる。壊れた取得元は待っても直らず、待つあいだ tick は排他ロックを保持したままになる
- トレードオフ: `Ok(true)`(生存の観測)だけでは失敗にしない。呼び出し側がメンバーの親のままだと終了したプロセスがゾンビとして列挙に残り、生存の観測を失敗に写すと実際には終了しているのに毎 tick 偽の失敗が積まれる。帰結として、既に消滅している実行単位への `kill` は `Ok` になる(目標とする状態が満たされている)
- トレードオフ: 終了1回あたり最大 `TERMINATION_GRACE` × 2 = 4秒、tick が排他ロックを保持したまま待つ。判定の timeout(既定60秒)に対して十分小さく取る

---

## ADR-016: デフォルト判定の返り値は2値の専用型に絞る

### Status

Proposed

### Context

見送り(`Skipped`)は判定コマンドの exit 20 だけが生む(`.adr/008-skipped-judgement-outcome.md`)。`JudgementService::default_judgement` はこの規則により2値しか返さないが、返り値は3値の `JudgeOutcome` のままで、規則の担保が doc コメントにしか無かった。結果、ユースケース側の `Settled::by_default` に到達不能な `JudgeOutcome::Skipped` アームが生きていた。

ADR-009 が `classify_alive` に当てた手当ての、判定側の残りである。

### Decision

**`DefaultJudgement`(`Completed` / `Failed`)をドメインに足し、`default_judgement` の返り値をこれに絞る。**

`JudgeOutcome` は3値のまま残し、`From<DefaultJudgement>` で埋め込む。2値に畳んで `JudgeOutcome` を落とさないのは、台帳 `DOM-execution-004` の PASS 要件が `Skipped` を含む3値を要求しているためで、ADR-009 が `RunningDecision` に採ったのと同じ扱いになる。

### Consequences

- 良い点: 「デフォルト判定は見送りを導かない」がコメントではなく型の主張になる。判定側とデフォルト側で規則の担保のしかたが揃う
- 良い点: `Settled::by_default` が2アームの網羅 `match` になり、到達不能アームが消える
- トレードオフ: 判定の型が2つになる。対応は `From` の1箇所に閉じ、2値がそのまま写ることをユニットテストで主張する

---

## ADR-017: 残存の後始末は表示の第4の見出しにする

### Status

Proposed

### Context

`.adr/098-spawn-not-observed-classification-and-error-headings.md` は `errors` を「タスクファイルに何を残したか」で3つの見出し(失敗を記録 / 起動の結果が未確定 / スキップ)に分けた。

ADR-010 で残存の報告を保存の成否と独立に積むようにした結果、この3分類のどれにも収まらなくなった。実行の失敗を保存できなかった tick でも `RemnantsUnhandled` は積まれるので、`attempt_count` が動いていないのに「失敗を記録」に現れる。「スキップ」(次の tick がそのまま再試行する)も当てはまらない — 残存終了は実行の失敗を確定させる tick でだけ試み、tick はこれを再試行しない。

### Decision

**`IssueOutcome` に第4分類 `CleanupLeft`(見出し「後始末が残っている」)を足し、`RemnantsUnhandled` だけをそこへ振る。** 見出しの語義は「タスクファイルに何を残したか」から「報告が何を残したか(運用者が次に取る行動)」へ広がり、OS 側に残ったものもこの軸で読める。

変種名を `RemnantsLeft` にしないのは、`cli::render` が `application::tick::RemnantsLeft` を型として使っており、同一ファイル内で同名衝突するためである。

ADR-098 の本文は書き換えず、置き換えた点(3分類 → 4分類、語義の一般化)を本 ADR に書く。振り分けは引き続き `cli::render` の網羅 `match` に置く。

### Consequences

- 良い点: カウンタを消費していない tick が「失敗を記録」に現れなくなり、見出しと帳簿の状態が食い違わない
- 良い点: 後始末の主体が tick ではなく人間であることが見出しから読める(tick は残存終了を再試行しない)
- トレードオフ: 見出しが4つになる。どれも空なら見出しごと出さないので、通常運用の行数は変わらない(ADR-098 と同じ)
