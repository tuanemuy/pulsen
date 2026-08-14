### Adapter / Ports

#### Blockers

- **[B-001]** `KillIdent` を実体に渡す前に**プラットフォームの形式として parse していない**。`-1` / `-0` / `0` が来ると、そのユーザーの全プロセス(または tick 自身と呼び出し元シェル)を巻き込んで終了させる
  - 場所: `crates/pulsen/src/adapter/process.rs:155`(`terminate`)、`crates/pulsen/src/adapter/process.rs:503`(`terminate::command`)、`crates/pulsen/src/adapter/process.rs:633` / `937` / `1214`(`unit_is_live`)
  - 理由: `KillIdent::parse`(`crates/pulsen-domain/src/task/process.rs:49`)は**非空しか検査しない**不透明値で、形式を知っているのはアダプターだけである(ADR-075)。ところが `terminate` は永続化された文字列をそのまま `/bin/kill -TERM -- <ident>` のオペランドへ渡す。POSIX で `-1` は「シグナルを送れる全プロセス」、`0` / `-0` は「呼び出し側のプロセスグループ」を意味するので、タスクファイル / pid ファイルが壊れた・手で編集された状態で `kill` または `try_kill_remnants` が走ると、無関係なプロセス群どころかユーザーのセッション全体が落ちる。しかも `try_kill_remnants` の側は `-` 接頭辞や数値としてのパースに失敗する ident を `Ok(false)` → `NotIdentifiable` で弾いている(`unit_is_live`)のに、`-1` / `-0` は「同定できた」として素通りする — `ps -o pid= -g 1` は実際にメンバーを返す。TC-port-process-controller-015 の期待「対象を誤殺なく同定できない状況ではいかなるプロセスも終了させない」は、**実行単位を一意に指さない同定子**に対しても成り立つべき性質であり、現状はその判断が `kill` 経路に一切なく、`try_kill_remnants` 経路にも中途半端にしか無い。plan.md のリスク欄「誤った対象に効くと無関係なプロセス群を殺す」と ADR-002 の「誤殺しない側に倒す」に対して、境界での parse が抜けている(CLAUDE.md「検証は境界で一度だけ行う」)
  - 提案: `terminate` の冒頭で同定子をプラットフォーム形式として parse し、満たさなければ終了操作を1度も起動せずに返す。POSIX は `-<n>`(`n` を `u32` としてパースでき、かつ `n >= 2`)、Windows は `<pid>`(`u32` としてパースでき非0)。`kill` は `KillError::Failed { message }`、`try_kill_remnants` は `NotIdentifiable` に落とす(後者は `unit_is_live` の既存の `Ok(false)` 経路にそのまま合流する)。形式の知識は既に `unit_is_live` にあるので、その parse を共有の関数に括り出して両経路から通せばよい。適合スイート側にも「実行単位を一意に指さない同定子では何も終了させない」を1件足せると、この判断が回帰で消えない

#### Warnings

- **[W-001]** POSIX の `kill` は SIGTERM を送るだけで、成功の判断を `/bin/kill` の終了ステータスに委ねている。SIGTERM を捕まえるエージェントは生き残るのに `Ok(())` が返る
  - 場所: `crates/pulsen/src/adapter/process.rs:503`(`terminate::command` の `["-TERM", "--", ident]`)、`crates/pulsen/src/adapter/process.rs:155`
  - 理由: 契約は「実行単位に属する全プロセスが終了する」(`spec/testcases/ports/process-controller.md` 19〜20行目)で、呼び出し側はこの `Ok` を受けて `fail_run` する(`application/tick/observe.rs:157`)。SIGTERM をハンドルするラッパー・エージェント(graceful shutdown を持つ CLI は珍しくない)が生きたまま `fail_run` されると、次の tick が**同一 worktree で再起動して並走する** — `kill` の失敗で状態を変えない設計が防ごうとしている当のシナリオに、成功側から入る。Windows は `taskkill /T /F` で強制終了なので、同じポートメソッドの保証がプラットフォームで食い違う。適合スイートの `agent_probe` は SIGTERM を捕まえないため、この差はスイートでは検出できない
  - 提案: 終了操作のあとに `unit_is_live` で消滅を確かめ、残っていれば `-KILL` で再送する(同じ `/bin/kill` の実体で足りる)。少なくとも「POSIX は graceful、Windows は forced」という保証差を why として残す

- **[W-002]** `kill` の成否が `/bin/kill` の実体依存のままで、実体が変わると成否が反転する。busybox では**終了させているのに失敗として返る**
  - 場所: `crates/pulsen/src/adapter/process.rs:496-509`
  - 理由: 実測すると、ubuntu 24.04(procps-ng 4.x)は `--` 無しだと対象を終了させたうえで rc=1 を返し、`--` を置くと rc=0 になる — 前ラウンドの修正はこの環境で正しく、不在のグループにはちゃんと rc=1 が返る(`/bin/kill: (-999999): No such process`)。macOS の `/bin/kill` も `-TERM -- -<pgid>` で rc=0・グループ終了で通る。一方 Alpine 3(busybox 1.37)の `/bin/kill` は `--` をオペランドとして読み `kill: invalid number '--'` を出し、**後続の `-<pgid>` は処理して対象を終了させたうえで rc=1** を返す。結果は「状態を変更せず報告のみ → 次 tick が Dead を観測して `DiedWithoutExit` に合流」なので安全側に倒れるが、毎回の凍結判断に偽の `KillFailed` が乗る。CLAUDE.md が「特定の OS に依存しない」を掲げている以上、成否の解釈を外部コマンドの終了ステータス**だけ**に委ねている構造は残しておきたくない
  - 提案: W-001 と同じ手当て(終了後に `unit_is_live` で結果を確かめ、消えていれば `Ok`)で両方閉じられる。実体依存の終了ステータス規則を、契約の観測(「実行単位が消えたか」)に置き換える

- **[W-003]** `unit_is_live` の `Err(Io)` は呼び出し側で `Ok(false)` と同じ `NotIdentifiable` へ畳まれるため、コメントが謳う「報告されない」の解消が実際には起きていない
  - 場所: `crates/pulsen/src/adapter/process.rs:645-647`(コメント「畳むと壊れた取得元が残存終了を恒久的に無効化しても何も報告されない」)、`crates/pulsen/src/adapter/process.rs:292`、`crates/pulsen/src/application/tick/mod.rs:261`(`RemnantsLeft::of`)
  - 理由: `RemnantsLeft::NotIdentifiable` は message を持たないので、「実行単位が既に消滅していた」(正常)と「取得元が壊れて列挙できない」(恒久的な機能不全)がサマリー上まったく区別できない。`Ok(false)` と `Err` の分離が観測できるのはこのモジュールの単体テストの中だけで、コメントが述べる利益は成立していない。ADR-002 が写像そのものを決めている以上、実態を超えているのはコメントの側(triage で `fix` 済みの `adapter/process.rs:273,616,850 / why コメント` と同じ性質の残り)
  - 提案: コメントを実態に合わせる(「区別は取得元の異常を単体テストで固定するためで、呼び出し側の結末は同じ」)か、機構の失敗を報告に載せる口を別に用意する。ポートの `RemnantOutcome` は spec 固定なので、前者が素直

- **[W-004]** `RecordSeq` が `saved` にしか入っておらず、`save_degraded` 経路の順序が主張できていない
  - 場所: `crates/pulsen-conformance/src/doubles/task_repository.rs:23-27`(`saved_degraded: RefCell<Vec<DegradedTask>>`)、`crates/pulsen/tests/tick_notify.rs:274-311`
  - 理由: 採番が入った目的は「通知を先に起動する実装を落とす」こと(`tests/tick_notify.rs:45-49` のコメント)だが、縮退タスクの再通知(`application/tick/notify.rs:71`)は `save_degraded` を使うので同じ列に並べられない。結果として `スナップショットが読めない未通知の凍結にも再通知が行われる` は、`mark_notified` を先に保存してから通知する実装でも通る — at-least-once の破れ方は `Task` 経路とまったく同じ(通知に失敗した凍結が永久に再通知されない)なのに、そちらだけ守られている
  - 提案: `saved_degraded` も `Vec<(RecordSeq, DegradedTask)>` にして `saved_degraded_in_order()` を足し、縮退経路にも「通知 → `notified_at` の追記」の順序主張を1件置く

- **[W-005]** HOOKS.md の CommandRunner の行が、probe を要さない2件を巻き込んでいる
  - 場所: `crates/pulsen-conformance/HOOKS.md:49`
  - 理由: 「TC-port-command-runner-001〜016 | テスト用コマンド(`examples/judge_probe`)がビルドされていない」とあるが、TC-003 は `missing_command`、TC-004 は `non_executable_command` で組み立てられており(`tests/conformance_command_runner.rs:103` / `114`)、どちらも judge_probe を要さない。1つ上の ProcessController の同種の行が `022`(存在しないコマンド名)/ `023`(実行不能)を明示的に外していることと不整合で、この表は「スキップを許容する条件の一覧」を名乗る正本なので、実際より広い集合を書くと読み手の宣言がずれる
  - 提案: `TC-port-command-runner-001 / 002 / 005〜016` に直す(004 は既に権限系の行を別に持っている)

- **[W-006]** HOOKS.md 末尾の OS 別ユニットテストの内訳が、本 PR の追加で合わなくなっている
  - 場所: `crates/pulsen-conformance/HOOKS.md:78-80`(「ubuntu 102件 / macOS 100件 / Windows 92件」「`ps` 系6件」「macOS だけの1件」)
  - 理由: 本 PR は `#[cfg(all(unix, not(target_os = "linux")))]` の `identity` モジュールにテストを2件足しており(`壊した列挙元では不在ではなく機構の失敗になる` / `列挙元の異常終了は出力の有無で不在と機構の失敗に分かれる`)、`ps` 系は 6 → 8 になる。件数自体は run に紐づく実測なので更新に CI が要るのは分かるが、「内訳は次のとおり」として書かれている構造の記述はこのコミットの形と食い違う。表側は `未測定` の規律で守られているのに、この段落だけ「まだ測っていない」を表す手段が無い
  - 提案: 内訳の側にも「この run 時点の構造」であることを明示するか、構造が変わった箇所(`ps` 系の件数)を `未測定` 相当の扱いにする

#### 確認した挙動(問題なし)

- **AC-7 の隔離は維持されている。** ターゲット述語つき `cfg` は `crates/pulsen/src/` で `util/atomic.rs` / `adapter/process.rs` / `adapter/task_repository.rs` の3ファイルのみ、`crates/pulsen-domain/` は0件。`#[allow(unsafe_code)]` は `adapter/process.rs:360` の1箇所のみ。本番依存は6クレートのまま。`cargo fmt --check` / `cargo clippy --workspace --all-targets -- -D warnings` は通る
- **`CommandRunner` の timeout。** 期限は `started.elapsed() >= limit` の比較で全域(`Instant + Duration` の溢れが無い)。超過時は `kill` → `wait` で回収するのでゾンビを残さない。`try_wait` / `wait` の `Err` 経路も `wait_failed` で kill → wait してから畳む。シェルは経由せず、`env_clear` を呼ばないので継承 + 上書き、`current_dir` は設定しないので呼び出し側の cwd のまま — すべて `spec/domains/execution.md#commandrunner` の契約どおり。stdin を `Stdio::null()` にするのは `run_agent` と同じ扱いで一貫している
- **`kill` / `try_kill_remnants` の呼び出し規約。** exit が Some のときに `starttime_of` を呼ばないこと(`tests/tick_observe.rs:161`)、`kill` の失敗で状態を変更しないこと(`observe.rs:167`)、残存終了の結果が分類に影響しないこと(`observe.rs:184-194`)がテストで守られている。`try_kill_remnants` は `unit_is_live` が真のときだけ終了を実行し、`NotIdentifiable` では終了コマンドを1度も起動しない(B-001 の同定子形式を除く)
- **適合スイートは macOS で 43/43 実行・0スキップ。** `cargo test --test conformance_command_runner --test conformance_process_controller` を実行し、CommandRunner 16件・ProcessController 27件すべてが PASS(スキップ0)。`ExecutionUnitCapability` の probe は「実行単位を起こす → その一部だけを終了させる」という**フィクスチャが本番で踏む手順そのもの**で能力を測っており(ADR-055 の `permission_restrictions_effective` と同じ性質)、`Partitionable` / `WholeOnly` / `Unavailable` / `ProgramMissing` の4区分と許容集合の対応も妥当 — 実行単位を要する4件(011 / 012 / 013 / 015)と一部だけの終了を要する2件(014 / 016)の分割は、各ケースが `require!` するフックと1:1で一致している。実行ファイルの不在(`ProgramMissing`)を許容集合に入れないのも ADR-073 の基準どおり
- **ハーネスのフックは状況の意味だけを受け取っている(ADR-027)。** `ExecutionUnit { kill_ident, members }` / `CommandBehavior` はプラットフォーム固有の機構名を持たず、期待も契約の語彙(「実行単位に属する全プロセスが終了する」= 各PIDで `starttime_of` が `None`)で書かれている(ADR-082)。`non_executable_command` / `deny_dir_write` は制限が実際に効いたことを確かめてから `Some` を返している
- **HOOKS.md の集計は整合している。** 10ポート196行(169 + PC 11 + CR 16)、区分 A 41 / B 132 / C 23、ProcessController 27行(A 3・B 16・C 8)、CommandRunner 16行(A 0・B 15・C 1)がすべて突き合う。新規4行は3列とも `未測定`
- **コード・テストに経緯コメントは無い。** 前ラウンドで大きく手が入った `adapter/process.rs` / `command_runner.rs` / 適合スイート / ダブルを通しで読んだが、指摘への弁明や修正の経緯は残っていない(残っているのは why / why not のみ)。ADR-002 も「starttime 照合が PID 再利用に対して持つ強さは実行単位の側には無い」と実態どおりに書き直されている
- **セキュリティ。** 判定・通知コマンドはシェル非経由で argv をリテラルに渡すため、コマンドインジェクションの経路は無い。環境変数は契約どおり継承 + 追加で、`TASK_ID` / `WORKFLOW` / `TASK_STATUS` / `EXIT_CODE` / `RUN_DIR` 以外を意図的に足してはいない

#### カバレッジ

- 確認: `crates/pulsen/src/adapter/command_runner.rs`, `crates/pulsen/src/adapter/process.rs`, `crates/pulsen/src/adapter/mod.rs`, `crates/pulsen-domain/src/execution/port.rs`, `crates/pulsen-domain/src/execution/mod.rs`, `crates/pulsen-conformance/src/command_runner.rs`, `crates/pulsen-conformance/src/process_controller.rs`, `crates/pulsen-conformance/src/lib.rs`, `crates/pulsen-conformance/src/doubles/command_runner.rs`, `crates/pulsen-conformance/src/doubles/mod.rs`, `crates/pulsen-conformance/src/doubles/process.rs`, `crates/pulsen-conformance/src/doubles/run_store.rs`, `crates/pulsen-conformance/src/doubles/task_repository.rs`, `crates/pulsen-conformance/HOOKS.md`, `crates/pulsen/tests/conformance_command_runner.rs`, `crates/pulsen/tests/conformance_process_controller.rs`, `crates/pulsen/tests/common/mod.rs`, `crates/pulsen/tests/tick_fixture/mod.rs`, `crates/pulsen/examples/judge_probe.rs`, `crates/pulsen/examples/agent_probe.rs`, `crates/pulsen/src/cli/wire.rs`, `crates/pulsen/src/cli/tick.rs`, `crates/pulsen/src/application/tick/observe.rs`, `crates/pulsen/src/application/tick/notify.rs`, `crates/pulsen/src/application/tick/mod.rs`, `crates/pulsen/src/application/tick/launch.rs`, `crates/pulsen/src/application/tick/confirm_spawn.rs`, `.thread/3/plan.md`, `.thread/3/adr.md`, `.thread/3/review/triage.md`
- 確認(部分): `crates/pulsen/tests/tick_observe.rs` — ポート呼び出しの記録に対する主張だけを見た（分岐の網羅はユースケース観点）
- 確認(部分): `crates/pulsen/tests/tick_notify.rs` — ダブルの使い方と順序主張だけを見た（同上）
- スキップ: `.thread/3/review/plan-001.md`, `.thread/3/review/review-001.md`, `.thread/3/review/review-001-adapter.md`, `.thread/3/review/review-001-domain.md`, `.thread/3/review/review-001-general.md`, `.thread/3/review/review-001-usecase.md` — 前ラウンドの中間成果物（Phase 8 で削除）。既決の判定は triage.md で把握した
- スキップ: `.thread/3/steps.md` — 実装手順の記録で、ポート契約に関する判断は plan.md / adr.md に集約されている
- スキップ: `.thread/3/testing.md` — 手動確認の手順書（本観点の対象外）
- スキップ: `crates/pulsen-domain/src/execution/judgement.rs` — 純粋サービス（ドメイン観点）。ポート越しの利用は observe.rs で確認した
- スキップ: `crates/pulsen-domain/src/execution/notification.rs` — 同上。`NOTIFY_TIMEOUT` が必ず適用されることは notify.rs 側で確認した
- スキップ: `crates/pulsen-domain/src/execution/running.rs` — 分類の純粋サービス（ドメイン観点）
- スキップ: `crates/pulsen-domain/src/execution/value.rs` — 値オブジェクト（ドメイン観点）。`CommandCompletion` の変種はポート契約として port.rs 側で確認した
- スキップ: `crates/pulsen-domain/src/task/task.rs`, `crates/pulsen-domain/src/task/degraded.rs`, `crates/pulsen-domain/src/task/counters.rs`, `crates/pulsen-domain/src/task/transition.rs` — 遷移ロジック（ドメイン観点）
- スキップ: `crates/pulsen/src/cli/render.rs` — 報告の文言組み立て（一般 / ユースケース観点）
- スキップ: `crates/pulsen/tests/cli_tick.rs` — 実バイナリの受け入れテスト（一般観点）。judge_probe の利用のみ examples 側で確認した
- スキップ: `crates/pulsen/tests/tick_scan.rs` — アームの振り分けの主張（ユースケース観点）
