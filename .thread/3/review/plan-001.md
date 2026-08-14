# 修正の実行計画 — ラウンド001

判定は `triage.md`（fix 21 / wont-fix 0 / defer 0）。単位は担当ファイルが重ならないように割ってあり、6単位すべてを並列に実行できる。

共通の制約:

- `.thread/3/adr.md` を編集するのは**単位1だけ**。ほかの単位で ADR に足すべき判断（新分類の根拠など）が出たら、最終報告で文面案を返す（メインがまとめて追記する）。
- スコープは plan.md の「含まれないもの」（手続きB / 手続きE / gc / abort / retry / set-status / `RunStore::attempt_exists` / CI ワークフロー）を越えない。
- 各単位の完了条件は `cargo fmt --check` / `cargo clippy --all-targets -- -D warnings` / `cargo test` が通ること。AC-7 の隔離（`pulsen-domain` の依存ゼロ・`#[allow(unsafe_code)]` 1箇所・`cfg` の3ファイル）を増やさない。

## 単位1: 実行単位の終了（POSIX / Windows）

- 担当する指摘: B-001, W-008, W-009, W-014
- 触るファイル: `crates/pulsen/src/adapter/process.rs`, `.thread/3/adr.md`
- 方針:
  - **B-001（最優先）**: `terminate::command` の argv を `["-TERM", "--", ident.as_str()]` にする。`--` 以降がオペランドであることを明示し、Linux（procps-ng）で成功時の終了ステータスが 1 になる破れを塞ぐ。不在グループに対して `--` なしだと 0 が返る（何も殺していないのに成功）方向の破れも同時に消える。`:488` のコメント「シグナルを先に置いて、同定子が単独のオプションとして解釈されないようにする」は実測と食い違うので、`--` を置く理由（同定子は `-<pgid>` でありオプションと区別できない）に書き換える。
  - あわせて `terminate` の失敗メッセージが空文字列にならないよう、`stderr` が空なら終了ステータスを添える。現状は Linux で `実行単位 (-12345) を終了できない: ` と原因が読めない。
  - **W-014**: POSIX 版 `identity::unit_is_live` を `identity_command` と同じ扱いに揃える — `LC_ALL` / `TZ` の固定と `LANG` / `LC_TIME` の除去を通し、`!status.success() && !stdout.is_empty()` を `Err(Io::Failed)` に落とす（`observe` と同じ規則）。`Err` は結局 `NotIdentifiable` に畳まれるが、注入した壊れた取得元と本物の失敗が運用で区別できるようになる。
  - **W-008**: `try_kill_remnants`（`:273-284`）と POSIX `unit_is_live`（`:614`）の why を実態に合わせる。「列挙は**実行単位が消滅している場合**を `NotIdentifiable` として分離するためのもので、ポートの入力が `KillIdent` だけである以上、実行単位ID の再利用そのものは検出できない」と書く。`.thread/3/adr.md` ADR-002 の Consequences「良い点: 誤殺しない側に倒す判断が…」も同じ粒度へ落とし、starttime 照合と同等の対策があるかのような書き方をやめる。恒久対策（`try_kill_remnants` に記録済み starttime を渡すポート契約の変更）は本スライス外なので、spec 追従の提起として Issue コメント（AC-8）に回す旨を ADR の Consequences に1行残す。
  - **W-009**: Windows `unit_is_live`（`:1119-1131`）の why を「同定子はラッパーの pid であり、`try_kill_remnants` の呼び出し前提はそのラッパーが死亡した後なので、観測は常に `Ok(None)` → `NotIdentifiable` になる。ツリーを辿り直す手段（ジョブオブジェクト）を持たない現在の設計の帰結であり、`taskkill /T` は根が死んでいれば効かないため誤殺もしない」に置き換える。#10（Windows 実機検証）へ引き継ぐ既知の穴として Issue コメントにも残す。
- 検証: 可能なら Docker `ubuntu:24.04` で `conformance_process_controller` を回し、TC-011 / 012 / 014 が緑になることを確認する（macOS だけでは B-001 の再発を検出できない）。

## 単位2: CommandRunner の待機と後始末

- 担当する指摘: W-011, W-012
- 触るファイル: `crates/pulsen/src/adapter/command_runner.rs`
- 方針:
  - **W-011**: `wait_until` の `let deadline = Instant::now() + limit;` をやめ、`let started = Instant::now();` として `started.elapsed() >= limit` で判定する。`DurationSpec` は上限なしの `u64` 秒を受理する（`definition/duration.rs`）ので、極端な `judge_timeout` を書いた config がアダプターのパニックになる経路を消す。比較の意味は変わらない。
  - **W-012**: `wait` の `child.wait()` が `Err` を返した枝と `wait_until` の `child.try_wait()` が `Err` を返した枝で、`let _ = child.kill(); let _ = child.wait();` を通してから `FailedToStart` を返す。timeout 経路（既に kill → wait を行っている）と扱いを揃える。`Err` を `FailedToStart` へ畳む現在の why コメントに「畳んだうえで子は始末する」を足す。
- 検証: 既存の `conformance_command_runner` 16件が緑のままであること。

## 単位3: 適合スイートのスキップ宣言と実測列

- 担当する指摘: W-010, W-013
- 触るファイル: `crates/pulsen/tests/conformance_process_controller.rs`, `crates/pulsen-conformance/HOOKS.md`（必要なら `crates/pulsen-conformance/src/process_controller.rs`）
- 方針:
  - **W-010**: `observation_allowed_skips()` の `cfg!(unix)` 決め打ちをやめ、ADR-055 / ADR-073 が定める「環境の能力を実測してから宣言する」形に寄せる。ハーネスのフック（`live_execution_unit` / `detached_execution_unit` / `orphaned_execution_unit`）が実際に `Some` を返せるかを `LazyLock` で1度だけ試す probe を置き、その結果から許容集合を組む。probe の置き場所は ADR-073 の基準に従う — フックの提供有無だけで決まるならスイート側、適用側のフィクスチャに依存するなら適用側（`tests/` 側）。
  - 許容集合を6行まとめてではなく、実際に前提を作れないもの（実行単位の一部だけを終了させられない環境が要る 014 / 016 と、実行単位そのものを作れない 011 / 012 / 013 / 015）で probe の結果に応じて分ける。`cfg` に依存する述語は残さない。
  - **W-013**: HOOKS.md の新規行（TC-port-process-controller-011〜016 / TC-port-command-runner-001〜016 / 004）の3列をいったん `未測定` に戻す（`:32` の規律どおり。実測は CI を回して初めて得られる）。あわせて `:30` の「現在の3列はすべて run 31683845168 の観測である」を「3ランナーでの実測」節の出典（run 31698858400）に揃える。W-010 で判定を probe に変えたら、当該行の「判定」列の文言もフック水準のまま probe の実態を括弧で補う（ADR-073 の「正本の主語はフック水準に保つ」）。
- 注意: 単位1 と論理的に連動する（B-001 が直らないと ubuntu の実測列は埋まらない）が、触るファイルは重ならない。実測列を実際の値で埋めるのは CI を回した後の別作業とし、この単位では `未測定` までにする。

## 単位4: ドメイン遷移の不変条件検査とテスト

- 担当する指摘: W-003, W-004, W-007
- 触るファイル: `crates/pulsen-domain/src/task/task.rs`, `crates/pulsen-domain/src/task/degraded.rs`
- 方針:
  - **W-004**: `spec/domains/task.md` §不変条件の「検証の境界」（不変条件2〜4 は遷移関数が前提として検査する）に実装を合わせる。`ensure_running` を「`Running` かつ `current_attempt` が `Some` かつ `current_attempt.process` が `Some`」まで見る形にし、破れは `TransitionError::MissingCurrentAttempt` を返す（ADR-096 のまま、分類だけを持つ）。`ensure_completed` にも `current_attempt` の存在検査を足す — `advance` が `Completed` かつ `current_attempt = None` を素通しすると `Pending` に戻り、次の `record_launching` が採番 1 で既存の `attempt-1` に衝突する（不変条件5の破れ）。
  - `observe`（ユースケース）冒頭の先回り検査は消さない。報告の解像度を上げるための重複であり ADR-004 の目的がそのまま生きる。ただし単位5 側の分類変更と衝突しないよう、この単位はドメイン側だけを触る。
  - **W-003**: `遷移の前提の破れは5種を区別する`（`task.rs:1943`）の配列に `TransitionError::AlreadyNotified` を足し、テスト名から種類数を外す（例: `遷移の前提の破れは変種ごとに区別される`）。ADR-006 で6変種になった事実にテストを追従させる。
  - **W-007**: `degraded.rs:271-282` の「凍結していない状態からは拒否」を `Task` 側と同じ全状態（`every_execution_state()` 相当）で回すか、「同じ入力に対して `Task` と `DegradedTask` が同じ `Err` を返す」形にして、台帳 DOM-task-059 の「Task と同じ規則」を主張の形にする。
- 注意: 遷移関数の前提が厳しくなるため、`crates/pulsen/tests/` の既存ケース（ダブルで `current_attempt` を欠いた Running を置いているもの）が落ちうる。落ちたら期待を新しい規則側へ揃える。テストの期待を緩めるのではなく、破れが `Transition` として報告されることを主張する形にする。

## 単位5: 2段規則の型・報告の構造化・通知アーム

- 担当する指摘: B-002, W-001, W-002, W-005, W-006, W-015, W-016, W-018
- 触るファイル: `crates/pulsen-domain/src/execution/running.rs`, `crates/pulsen-domain/src/execution/notification.rs`, `crates/pulsen/src/application/tick/mod.rs`, `crates/pulsen/src/application/tick/observe.rs`, `crates/pulsen/src/application/tick/notify.rs`, `crates/pulsen/src/cli/render.rs`, `crates/pulsen/tests/tick_observe.rs`, `crates/pulsen/tests/tick_scan.rs`
- 方針:
  - **B-002（型の形。レイヤーをまたぐので単位を割らない）**: `unreachable!` を消し、2段規則の担保を型に戻す。
    - `classify_alive` の返り値を3値の新しい型（`AliveDecision { KeepRunning, KillOnTimeout, DiedWithoutExit }`）にする。これで plan.md のリスク節が言う「`classify_alive` に `Judge` を返させない型の形」が実際の担保になる。
    - `RunningDecision` は **4値のまま残す**（台帳 `DOM-execution-008` の PASS 要件が `Judge(ExitCode)` / `KeepRunning` / `KillOnTimeout` / `DiedWithoutExit` の4値を要求しており、ここを2値に畳むと AC-1 の行が落ちる）。`AliveDecision` から `RunningDecision` への埋め込み（`From` 実装）を置く。
    - `observe.rs` は1段目（exit の有無）を**値にしてから**分岐する — `Some(exit)` なら `RunningDecision::Judge(exit)`、`None` なら観測して `classify_alive(..).into()`。そのうえで4アームを網羅 `match` する。これで `Judge` が本番で構築される経路が生まれ、`unreachable!` が消える。
    - `running.rs` の doc コメント（「だから `classify_alive` は `Judge` を返さない」）は、型がそう言うようになったことに合わせて書き換える。ドメインのユニットテストと `tick_observe.rs` の期待も新しい型に追従させる。
  - **W-015 + W-005**: `TickIssue::RunFailed { message }` / `RemnantsUnhandled { message }` を分類に置き換え、文言は `cli::render` の網羅 `match` で組む（ADR-081）。
    - `RunFailed { task_id, cause: RunFailureCause }` とし、`RunFailureCause` は少なくとも「デフォルト判定が失敗にした（エージェントの exit）」「判定コマンドが失敗と判定した（判定側の exit と実行の exit の両方を持つ）」「timeout 超過で kill した」「exit を残さずに死んだ」を区別する。これで **W-005**（judge exit 10 のとき「実行が終了コード 0 で終了しました」と出る自己矛盾）が構造として解消し、表示から判断の主体が読める。
    - `RemnantsUnhandled { task_id, outcome: RemnantOutcome }` にする（`RemnantOutcome` はドメインの公開型なのでそのまま運べる）。
    - `observe.rs` の `judgement_detail` / `timeout_detail` / `report_remnants` から文言生成を落とし、`cli/render.rs:222-231` 側に網羅 `match` で置く。`JudgeFailed { detail }` は帳簿に永続化される値の再利用なので現状のまま（ADR-090 の例外）。
  - **W-002**: 不変条件4の破れ（judge を持つステータスで `workspace` が未確定）を、ユースケースが `TransitionError::WorkspaceNotSet` を自作する形からやめ、`TickIssue` に分類を足す（`MissingWorkspace { task_id }`）。遷移関数を一度も呼んでいない以上、遷移エラーの語彙に相乗りする根拠がない（ADR-004 の「破れの事実も求める修復も違うなら分類を分ける」に揃える）。`cli/render.rs` に案内を足し、`tick_observe.rs` に「判定コマンドを持つステータスで workspace が未確定なら、判定コマンドを起動せず書き込まずに報告する」ケースを1件足す（`tick_fixture` の `TaskBuilder::running` が必ず `workspace()` を通すので、workspace 未設定の Running を置ける経路が要る）。この分類追加は `.thread/3/adr.md` への追記候補なので、文面案を最終報告で返す。
  - **W-001**: `process()` の `SnapshotUnreadable` の分岐で、`notify_degraded` へ回す経路でも `TickIssue::SnapshotUnreadable` を積む（通知の成否とは独立の「修復が必要である」という報告）。spec の「定義依存の判断はすべてスキップして報告。**ただし** … notify を実行する」は報告の**置換**ではなく**追加**と読む。`tick_scan.rs` に「notify_cmd 未定義の未通知凍結（スナップショット破損）も報告される」ケースを足す。
  - **W-006**: `Branch::Notify { notified: bool }` を割り、分岐を `dispatch` の `match` だけで完結させる（例: `Branch::Notify` と `Branch::AlreadyNotified`）。`branch_of` が `notified_at.is_some()` を見て変種を選び、呼び出し側の `if !notified` を消す。
  - **W-016**: 通知の成否解釈（`Exited(0)` だけが成功、それ以外は失敗の理由つき）を `NotificationService::interpret_notify_completion(&CommandCompletion) -> NotifyOutcome` としてドメインへ寄せる（`JudgementService::interpret_judge_completion` と同型）。`notify.rs` の `Delivery` はユースケースの分岐（`NotConfigured` の有無）だけを持つ形に縮める。これで `.thread/3/adr.md` ADR-003 の Consequences「#5 の `AbortTask` が同じ関数を呼べる」が構造として成立する。ドメイン側にユニットテスト（3〜4値）を足す。
  - **W-018**: `observe.rs:151-159` の `report_remnants` を `Persisted::Saved` の条件から外し、常に報告する。残存プロセスの有無はタスクファイルを書けたかと直交する事実で、保存が失敗した tick でこそ人間の後始末に要る。ADR-092 の不変（書き込んだ経路はサマリーに現れる）は `SaveFailed` が埋めるのでサマリーは空にならない。`tick_observe.rs` に「保存に失敗しても残存の結末は報告される」ケースを足す。
- 注意: `TickSummary` / `TickIssue` の変種が増えるので `cli/render.rs` の網羅 `match` がコンパイルエラーで足し忘れを教える。表示文言は既存の語彙（`ADR-098` の見出し規則）に揃える。

## 単位6: 通知テストの実効性

- 担当する指摘: W-017, W-019
- 触るファイル: `crates/pulsen-conformance/src/doubles/command_runner.rs`, `crates/pulsen-conformance/src/doubles/task_repository.rs`, `crates/pulsen-conformance/src/doubles/mod.rs`, `crates/pulsen/tests/tick_notify.rs`
- 方針:
  - **W-017**: 「stopped の `save` → notify_cmd 実行」の順序を実効的に検証できるようにする。`ScriptedTaskRepository` と `ScriptedCommandRunner` が独立したベクタに記録しているため、現在のアサーション（`saved[0].notified_at == None` / `saved[1] == Some(_)`）は2回の `save` の順序しか見ておらず、「notify_cmd を先に起動してから stopped を保存する」実装でも緑になる。両ダブルが共有する単調増加のシーケンス番号（`Rc<Cell<u64>>` などの共有カウンタ）を `CommandRunnerCall` と保存記録の双方に持たせ、`tick_notify.rs:70-119` の主張を1本の列（保存 → 通知 → 保存）で書く。安価な代替として、`ScriptedCommandRunner` が結末を返す直前に共有した保存記録を観測する形でもよい。plan.md のテスト方針「呼び出し記録の並びで主張する」がこれで実際に成立する。
  - **W-019**: `tick_notify.rs:276-285` のコメント「起動へ進まないよう、エージェント定義を持たない設定にしていないため、…」は二重否定で字義が反転している。テストの組み立て事情の説明ごと落とし、「凍結でない実行状態は通知アームへ入らない」という規則そのものの説明に置き換える（CLAUDE.md「残すのは現在の形が成り立つ理由だけ」）。あわせて `let _ = harness.run();` で結果を捨てている点を改め、`commands.calls().is_empty()` だけでなく `tasks.saved()` に `Stopped` が現れないことも主張する。`Branch::Notify` に入らない状態を複数回して、実装が状態判別を誤ったときに落ちる形にする。
- 注意: ダブルの記録構造を変えるので、`tick_observe.rs` / `tick_scan.rs` が同じアクセサを使っていれば追従が要る。既存アクセサ（`calls()` / `saved()`）のシグネチャは変えず、並びを見る手段を**足す**形にすれば単位5 とぶつからない。

## 並列実行できる組み合わせ

担当ファイルはどの2単位のあいだでも重ならないため、**単位1〜6 をすべて同時に実行できる**。ただし論理的な連動が2組ある。

- 単位1 → 単位3: B-001 が直るまで ubuntu の実測列は埋まらない。単位3 は `未測定` に戻すところまでで完結させ、実測での置き換えは CI を回した後に行う。
- 単位4 → 単位5: 遷移関数の前提が厳しくなると `tick_observe.rs` / `tick_scan.rs` の一部が落ちうる。両方が入った状態で `cargo test` を一度回し、期待を新しい規則側へ揃える。

単位6 は単位5 と同じテスト観点を扱うが、触るファイル（`tick_notify.rs` と `doubles/`）は単位5 の担当（`tick_observe.rs` / `tick_scan.rs`）と分かれている。
