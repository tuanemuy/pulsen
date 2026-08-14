### Adapter / Ports

#### 検証の前提

- 規約の基準は `CLAUDE.md`(技術方針・ヘキサゴナル)、`spec/domains/execution.md`(ProcessController / CommandRunner)、`spec/testcases/ports/{command-runner,process-controller}.md`、`crates/pulsen-conformance/HOOKS.md`、`.thread/3/plan.md`(AC-1 / AC-6 / AC-7)、`.thread/3/adr.md`(ADR-001 / ADR-002 / ADR-007)。
- 機械的な確認は実際に走らせた。
  - `cargo test -p pulsen --test conformance_command_runner --test conformance_process_controller` → **43件すべて pass(macOS / 非 root)**。CommandRunner 16件・ProcessController 27件でスキップ0。
  - AC-7 の隔離: ターゲット述語つき `cfg` のヒットは `crates/pulsen-domain/` に **0件**、`crates/pulsen/src/` は `util/atomic.rs` / `adapter/process.rs` / `adapter/task_repository.rs` の **3ファイルのまま**(`pulsen-conformance/src/lib.rs` の2件は main 時点から不変)。`#[allow(unsafe_code)]` は `adapter/process.rs:350` の **1箇所のまま**。本番依存は `clap` / `getrandom` / `serde` / `serde_json` / `serde_yaml_ng` / `tempfile` の **6クレートのまま**、`pulsen-domain` の `[dependencies]` は空のまま。
  - 新規の OS 依存コードは `adapter/process.rs` の `terminate` / `identity` モジュールにのみ現れ、`adapter/command_runner.rs` は `cfg` を1つも持たず `encode` を共有している(AC-7 の但し書きどおり)。
- **Linux 上の挙動は Docker(`ubuntu:24.04`)で実測した**。B-001 はその実測に基づく。

#### Blockers

- **[B-001]** `kill` / `try_kill_remnants` が **Linux では常に失敗を返す**(実行単位は実際には終了しているのに `KillError::Failed` / `RemnantOutcome::Failed` になる)
  - 場所: `crates/pulsen/src/adapter/process.rs:491`(`command.args(["-TERM", ident.as_str()])`)、影響は `crates/pulsen/src/adapter/process.rs:155`(`terminate`)・`:267`(`kill`)・`:273`(`try_kill_remnants`)
  - 理由: `KillIdent` は `-<pgid>` 形式なので、実際の argv は `/bin/kill -TERM -12345` になる。`ubuntu:24.04` の `/bin/kill`(**procps-ng 4.0.4**、GitHub Actions の `ubuntu-latest` と同じ系統)でこれを実測すると、**対象のプロセスグループは終了するが終了ステータスは 1、標準エラーは空**になる。`--` を挟んだ `/bin/kill -TERM -- -12345` では終了ステータス 0 になる。

    実測(いずれも `docker run --rm --init ubuntu:24.04`、呼び出し側とは別のプロセスグループを対象):

    | 呼び出し | 終了ステータス | stderr | 対象グループ |
    |---|---|---|---|
    | `/bin/kill -TERM -<pgid>` | **1** | 空 | 終了する |
    | `/bin/kill -TERM -- -<pgid>` | 0 | 空 | 終了する |
    | `/bin/kill -TERM -99999`(不在) | **0** | 空 | — |
    | `/bin/kill -TERM -- -99999`(不在) | 1 | `(-99999): No such process` | — |

    `terminate` は `output.status.success()` だけで成否を分けるため、Linux では成功した終了が毎回 `KillError::Failed { message: "実行単位 (-12345) を終了できない: " }`(原因は空文字列)になる。結果は次のとおり。
    - 適合ケースが落ちる: `TC-port-process-controller-011` / `012` は `assert_eq!(controller.kill(..), Ok(()))`、`014` は `assert_eq!(.., RemnantOutcome::Killed)` を主張しており、**ubuntu の CI で失敗する**。AC-7 の「`cargo test` が通る」は macOS でしか成立していない(HOOKS.md の実測列もこの3行が `未測定` のままで、この破れを検出できていない)。
    - 運用上の破れ: `KillOnTimeout` は `kill` の失敗で状態を変更しない設計なので、Linux では timeout kill が毎回「終了させたのに `KillFailed` を報告して failed を記録しない」になる。次 tick が `DiedWithoutExit` 経由で拾うため最終的には failed に落ちるが、**AC-6 が要求する「`kill` してから `fail_run` する」経路は Linux で1度も成立しない**。報告される原因は空文字列で、運用者は何が起きたか読めない。
    - 3行目の実測(不在グループに対して `--` なしだと**終了ステータス 0**)も別方向に危険で、「何も殺していないのに成功」を返す。ADR-002 が「記録した値をそのまま渡す」と決めた狙い(帳簿と対象がずれない)は、`--` を挟んでも損なわれない。
  - 提案: 引数を `["-TERM", "--", ident.as_str()]` にする(`--` 以降がオペランドであることを明示する)。macOS の `/bin/kill` でも `-- -<pgid>` は受理され、実グループに対して終了ステータス 0 で終了させられることを実測済み。合わせて `terminate` の失敗メッセージが空にならないよう、`stderr` が空なら終了ステータスを添える。コメント「シグナルを先に置いて、同定子が単独のオプションとして解釈されないようにする」(`process.rs:488`)は**この実測と食い違う**ので、`--` を使う理由に置き換える。

#### Warnings

- **[W-001]** `try_kill_remnants` の「列挙してから殺す」は、ADR-002 が挙げた**実行単位ID再利用の誤殺を防いでいない**
  - 場所: `crates/pulsen/src/adapter/process.rs:273-284`、`:612-630`(ps 版 `unit_is_live`)、`:846-884`(procfs 版)
  - 理由: `unit_is_live` が確かめるのは「その PGID を持つプロセスが1つでも居るか」だけで、それが**記録した実行単位と同じものか**は確かめていない。PGID が別の実行単位に再利用されていれば列挙は `Ok(true)` を返し、そのまま無関係なプロセス群を終了させる。一方、実行単位が消滅している場合は `kill(2)` 自体が ESRCH で何も殺さないため、列挙を挟んで得られる安全性の差分は実質ゼロで、得られるのは「`NotIdentifiable` という分類の解像度」だけである。それにもかかわらずコメント(`:614`「列挙できたことが『誤殺なく同定できた』の実質になる」・`:274-276`「同定子へ無条件に投げる実装は…無関係なプロセス群を殺す」)と ADR-002 は、starttime 照合と同等の再利用対策が入ったかのように書いている。`starttime_of` 側は記録済み `ProcessStartTime` との照合で再利用を実際に排除しているので、同列に並べると読み手を誤らせる。加えて列挙と終了の間には TOCTOU がある(ベストエフォート契約の範囲内ではある)。
  - 提案: 実装は現状(誤殺しない側に倒す形)のままでよいが、why を実態に合わせる — 「列挙は**実行単位が消滅している場合**を `NotIdentifiable` として分離するためのもので、ポートの入力が `KillIdent` だけである以上、PGID 再利用そのものは検出できない」と書く。ADR-002 の Consequences の「良い点」も同じ粒度に落とす。恒久的に塞ぐなら、`try_kill_remnants` に記録済み starttime を渡せるようポート契約側の変更が要る(本スライスの範囲外だが、spec 追従の提起に足す価値がある)。

- **[W-002]** Windows の `try_kill_remnants` は、その唯一の用途で**必ず `NotIdentifiable` になる**
  - 場所: `crates/pulsen/src/adapter/process.rs:1119-1131`
  - 理由: Windows の `KillIdent` はラッパー自身の `<pid>` であり、`unit_is_live` は `observe(source, pid).is_some()`、つまり**ラッパーが生きているか**を見ている。ところが `try_kill_remnants` の呼び出し前提は「ラッパーが死亡した後の残存プロセス」(`RunningDecision::DiedWithoutExit`)なので、Windows では観測が常に `Ok(None)` → `NotIdentifiable` になり、残存終了が1度も実行されない。`taskkill /T` はツリーの根が死んでいれば効かないので結果として正しい(誤殺しない)が、コメント「同定子は pid そのものなので、観測できれば実行単位は生きている」は、この前提の下では成り立たない説明になっている。TC-014 / 016 は非 unix では丸ごとスキップ許容(W-003)なので、この穴はテストでも現れない。
  - 提案: why を「Windows では実行単位の同定子がラッパーの pid であり、ラッパー死亡後にツリーを辿り直す手段が無いため、残存終了は構造上 `NotIdentifiable` にしかならない(ジョブオブジェクトを持たない現在の設計の帰結)」と明示する。#10(Windows 実機検証)に引き継ぐ既知の穴として Issue コメントにも残す。

- **[W-003]** 観測スイートのスキップ許容集合が `cfg!(unix)` で決まっており、HOOKS.md が宣言した判定(フックの提供有無)と食い違う
  - 場所: `crates/pulsen/tests/conformance_process_controller.rs:503-509`、`crates/pulsen-conformance/HOOKS.md:46`
  - 理由: HOOKS.md はこの6行の判定を「ハーネスが `live_execution_unit` / `detached_execution_unit` / `orphaned_execution_unit` を提供するか」と書いているのに、実装は**コンパイル時のターゲット述語**で6行まとめて許容している。ADR-055 / ADR-073 が求めるのは「制限が実際に効いたことを確認してから」スキップにすることで、`non_executable_command`(起動が拒否されることを確かめてから `Some`)や `permission_restrictions_effective` はその形になっている。`cfg!(unix)` は前提が作れるかを一切試さないため、非 unix では `live_execution_unit` が `Some` を返せる(`spawn_unit` 自体はプラットフォーム非依存)にもかかわらず 011 / 012 / 013 / 015 まで無条件に緑にできてしまう。ADR-082 の「プラットフォーム固有の機構名に踏み込まない」とも方向が逆。
  - 提案: 宣言も `let can_build_unit = harness_probe();` のような**能力の実測**に寄せる(例: `orphaned_execution_unit` が `Some` を返せるかを1回だけ試す、あるいは `terminate_one` が成功するかを確かめる述語 `execution_unit_partially_terminable()` を `pulsen-conformance` に置く)。少なくとも許容集合を `EXECUTION_UNIT_CASES` の全6行ではなく、実際に前提を作れないもの(単体終了を要する 014 / 016)に絞る。

- **[W-004]** `CommandRunner` の期限計算が `Instant::now() + limit` で、極端な `timeout` 値でパニックしうる
  - 場所: `crates/pulsen/src/adapter/command_runner.rs:86`
  - 理由: `DurationSpec` は `u64` 秒を上限なく受理する(`crates/pulsen-domain/src/definition/duration.rs:56-60`。`checked_mul` は溢れを弾くが `18446744073709551615s` は通る)。`Instant + Duration` は溢れでパニックするため、`judge_timeout` に極端な値を書いた config が**アダプターのパニック**になる。設定値は境界で parse 済みでもここは全域ではなく、CLAUDE.md の「パニックは不変条件違反にのみ」から外れる。
  - 提案: 期限を絶対時刻で持たず、`let started = Instant::now();` として `started.elapsed() >= limit` で判定する(溢れが起きず、比較の意味も変わらない)。`Instant::checked_add` でも可。

- **[W-005]** `try_wait` / `wait` が `Err` を返した経路で子プロセスを回収も終了もしない
  - 場所: `crates/pulsen/src/adapter/command_runner.rs:71-78`(`wait`)、`:88-94`(`wait_until`)
  - 理由: どちらも `FailedToStart` を返して `Child` をそのままドロップする。Rust の `Child` はドロップで待たないため、この経路では**判定・通知コマンドが生きたまま放置され、ゾンビも残りうる**。timeout 経路では `kill` → `wait` を丁寧に行っている(`:99-101`、コメントもある)のに、こちらだけ扱いが揃っていない。発生頻度は低い(`waitpid` 自体の失敗)が、tick は cron で回り続けるので溜まると効いてくる。
  - 提案: `Err` の枝でも `let _ = child.kill(); let _ = child.wait();` を通してから返す。合わせて「起動には成功しているので `FailedToStart` に畳む」という現在の why に、「畳んだうえで子は始末する」ことを足す。

- **[W-006]** HOOKS.md の新規行が、同じファイルが定めた「行を足すときは3列を `未測定` で埋める」規律から外れている
  - 場所: `crates/pulsen-conformance/HOOKS.md:46-48`(macOS 列が `実行`)、規律は `:32`
  - 理由: 3ランナー実測の節(`:63`)は出典を run 31698858400 と書いており、新規11行+16行はその run に含まれていない。macOS 列だけローカル実行の結果で埋まっているため、「表の実測列 = CI の観測」という不変が崩れ、B-001 のような**ubuntu でだけ落ちる破れ**が表からは読み取れなくなる。実際 011〜016 の ubuntu 列は `未測定` のままで、B-001 はこの空白の中に隠れている。
  - 提案: 新規行の3列をいったん `未測定` に戻し、CI を回してから実測で置き換える(`:32` の規律どおり)。加えて `:30` の「現在の3列はすべて run 31683845168 の観測である」は `:63` の出典(31698858400)と食い違ったままなので、この機会に揃える。

- **[W-007]** POSIX(非 Linux)の `unit_is_live` が取得コマンドの環境固定と終了ステータス確認を共有していない
  - 場所: `crates/pulsen/src/adapter/process.rs:616-630`(`identity_command` は `:633-643`)
  - 理由: `observe` は `LC_ALL` / `TZ` の固定と `LANG` / `LC_TIME` の除去、さらに「終了ステータス非0 かつ stdout 非空なら `Err`」という区別を持つのに、`unit_is_live` は素の `Command::new` で `-o pid= -g <pgid>` を叩き、**終了ステータスを一切見ずに stdout の空・非空だけ**で判断する。取得元が壊れて usage エラーを吐いた場合(引数体系の違う `ps` に解決された等)も静かに `Ok(false)` → `NotIdentifiable` になり、残存終了が恒久的に無効化されても報告されない。`identity_command` と同じ扱いにしておけば、注入した壊れた取得元との区別も一貫する。
  - 提案: `unit_is_live` も `identity_command` と同じ環境固定を通し、`!output.status.success() && !output.stdout.is_empty()` を `Err(Io)` に落とす(`observe` と同じ規則)。`Err` は結局 `NotIdentifiable` に畳まれるが、`Io::Failed` のメッセージが残るぶん運用で追える。

#### 良かった点(意図的に残すべき形)

- `SystemCommandRunner` が OS 依存分岐を1つも持たず、シグナル死の符号化を `run_agent` と同じ `encode` で共有している(`adapter/command_runner.rs:1-5` / `adapter/process.rs:291`)。AC-7 の但し書きどおりで、判定コマンドとエージェントで同じ結末が別の値に見える経路が構造上できない。
- `TerminatorSource` を `with_terminator_source` の後付けメソッドにしたこと(ADR-007)。合成ルート(`cli/wire.rs`)の呼び出しが変わらず、既定は絶対パス固定のまま、適合ケースの異常系だけが確定的に走る。TC-013 / 015 / 016 が権限にも root の可否にも依存せず macOS で実際に走ることを確認した。
- `TC-port-process-controller-015` が「同定できないコントローラで `try_kill_remnants` → **別のコントローラでメンバーの生存を観測**」という二重の観測になっていること(`process_controller.rs`「同定できなければ何も終了させない」)。同定子へ無条件に終了を投げる実装はここで確実に落ちる — 期待結果が `NotIdentifiable` だけなら検出できない破れを、生存観測が拾っている。
- `judge_probe` が期待値を**ファイルで**受け取る設計(`examples/judge_probe.rs:58-73`、HOOKS.md TC-006)。引数で渡すとシェル経由の場合に期待側も同じく歪んで照合が通ってしまう、という穴を正しく塞いでいる。
- ハーネスのフックが `CommandBehavior` / `ExecutionUnit`(同定子とメンバー PID)という**契約の語彙**だけを受け取り、`taskkill` / `kill` / プロセスグループといった機構名をケースに持ち込んでいない(ADR-027 / ADR-082)。`non_executable_command` が「起動が拒否されることを確かめてから `Some`」を返す形も ADR-055 / ADR-073 どおり。
- ダブル(`ScriptedCommandRunner` / `ScriptedProcessController` / `ScriptedRunStore`)が既存の規律(台本を使い切ったらパニック・呼び出しを順序つきで記録)を崩さずに拡張されており、`CommandRunnerCall` が cmd / env / timeout の3つを残すので「判定と通知を取り違えた実装」がユースケーステストで検出できる。`tick_notify.rs` が実際に `NOTIFY_TIMEOUT` と env の3値、保存 → 通知 → 追記の順序を主張している。

#### カバレッジ

- 確認: `.thread/3/plan.md`, `.thread/3/adr.md`, `crates/pulsen-conformance/HOOKS.md`, `crates/pulsen-conformance/src/command_runner.rs`, `crates/pulsen-conformance/src/doubles/command_runner.rs`, `crates/pulsen-conformance/src/doubles/mod.rs`, `crates/pulsen-conformance/src/doubles/process.rs`, `crates/pulsen-conformance/src/doubles/run_store.rs`, `crates/pulsen-conformance/src/doubles/task_repository.rs`, `crates/pulsen-conformance/src/lib.rs`, `crates/pulsen-conformance/src/process_controller.rs`, `crates/pulsen-domain/src/execution/judgement.rs`, `crates/pulsen-domain/src/execution/mod.rs`, `crates/pulsen-domain/src/execution/notification.rs`, `crates/pulsen-domain/src/execution/port.rs`, `crates/pulsen-domain/src/execution/value.rs`, `crates/pulsen/examples/agent_probe.rs`, `crates/pulsen/examples/judge_probe.rs`, `crates/pulsen/src/adapter/command_runner.rs`, `crates/pulsen/src/adapter/mod.rs`, `crates/pulsen/src/adapter/process.rs`, `crates/pulsen/src/application/tick/mod.rs`, `crates/pulsen/src/application/tick/notify.rs`, `crates/pulsen/src/application/tick/observe.rs`, `crates/pulsen/src/cli/tick.rs`, `crates/pulsen/src/cli/wire.rs`, `crates/pulsen/tests/cli_tick.rs`, `crates/pulsen/tests/common/mod.rs`, `crates/pulsen/tests/conformance_command_runner.rs`, `crates/pulsen/tests/conformance_process_controller.rs`, `crates/pulsen/tests/tick_fixture/mod.rs`, `crates/pulsen/tests/tick_notify.rs`
- スキップ: `.thread/3/steps.md`, `.thread/3/testing.md` — 実装手順書・手動確認手順であり、ポート契約の判定材料にならない(契約の基準は plan.md と adr.md で取った)
- スキップ: `crates/pulsen-domain/src/execution/running.rs` — ポート非依存の純粋サービス(`IdentityCheck` / `RunningClassifier`)でドメイン観点の担当。ポート側からは `starttime_of` の三値を畳む位置だけを `application/tick/observe.rs` で確認した
- スキップ: `crates/pulsen-domain/src/task/counters.rs`, `crates/pulsen-domain/src/task/degraded.rs`, `crates/pulsen-domain/src/task/task.rs`, `crates/pulsen-domain/src/task/transition.rs` — 遷移ロジックとカウンタでポートに触れない(ドメイン観点)
- スキップ: `crates/pulsen/src/application/tick/confirm_spawn.rs`, `crates/pulsen/src/application/tick/launch.rs` — ジェネリック引数追加の追従のみで、ポートの使い方は変わっていない(ユースケース観点)
- スキップ: `crates/pulsen/src/cli/render.rs` — 報告の表示のみ(CLI 観点)
- スキップ: `crates/pulsen/tests/tick_observe.rs`, `crates/pulsen/tests/tick_scan.rs` — ダブルに対するユースケース分岐の網羅で、ポート契約そのものは検証していない(テスト観点)。ダブルの忠実性は `doubles/` 側で確認した
