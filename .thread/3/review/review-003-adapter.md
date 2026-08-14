### Adapter / Ports

#### Blockers

なし。

`terminate` の骨格（境界で `UnitTarget` に parse → 終了操作を起動 → `unit_is_live` を
`TERMINATION_GRACE` / `TERMINATION_POLL` でポーリング → 消滅しなければ `-KILL` へ昇格 →
それでも観測できなければ終了ステータスに委ねる）は、契約・誤殺の両面で成り立っている。
確認した筋を残す。

- **parse の網羅**: `crates/pulsen/src/adapter/process.rs:584` は `-` 接頭辞を要求し、`MIN_PGID = 2`
  で `-1`（送れる全プロセス）・`-0` / `0`（呼び出し側のプロセスグループ）・`-1` 相当の init を
  すべて弾く。`operand()` は読み取った文字列ではなく `u32` から組み直すので、`/bin/kill` へ渡る
  オペランドは常に `-<10進>` であり、`--` と併せてオプション注入もコマンド注入も成立しない。
  弾いた場合に**終了操作を1度も起動しない**ことは、痕跡を残す実体を注入した
  `process.rs:1504` / `1524` の2件が6種の値で押さえている。
- **`Ok` の意味**: 消滅を観測できないまま `Ok` になるのは、終了操作の終了ステータスが成功した
  ときだけ（`process.rs:195`）。`Forced`（SIGKILL）の送出が成功していれば、対象は D 状態に
  留まっていてもユーザー空間へ戻らないため「生きたまま failed → 同一 worktree で並走」には
  至らない。`graceful` だけが成功する経路は、`Forced` が非0（= ESRCH = 実行単位が消えた）の
  ときにしか届かない。ゾンビが列挙に残る構成でこの畳み込みが要ることは
  `process.rs:1547` が固定している。
- **`Err(Io)` を即 false に畳む判断**: 壊れた取得元は待っても直らず、待つ間はロックを保持した
  ままになる（`process.rs:235`）。畳んだ先は「消滅を確かめられなかった」であり、生存への写像
  ではない。結末は昇格と終了ステータスの評価に落ちるだけで、`Ok(true)` と混ざっても状態変更の
  判断は変わらない。`try_kill_remnants` 側で `Ok(false)` と同じ `NotIdentifiable` に畳むのも、
  「いかなるプロセスも終了させない」という期待と一致する。
- **ロック保持時間**: 1回の終了で待ちうるのは 2 段 × `TERMINATION_GRACE` = 4 秒。ポーリングに
  入るのは `unit_is_live` が生存を返す実行単位だけで、再起動後に一斉に `DiedWithoutExit` へ
  落ちるような場面は `Ok(false)` で即抜ける。`judge_timeout`（既定60秒）を承知で組み込んだ
  ADR-018 の下では、桁として過大ではない。
- **既に消滅した実行単位への `kill` が `Ok`**: 目標状態が満たされている以上、冪等な `Ok` が
  正しい。tick 側は `KillOnTimeout` で `Ok` → `fail_run` と進むが、対象は実際に消えている。
- **AC-6 / AC-7**: `observe.rs:59` の2段規則（exit が `Some` なら `starttime_of` を呼ばない）、
  `kill` 失敗で状態を変えない（`observe.rs:167`）、残存終了が分類に影響しない
  （`observe.rs:189`）はいずれも契約どおり。`cfg` の述語つきヒットは
  `util/atomic.rs` / `adapter/process.rs` / `adapter/task_repository.rs` の3ファイル、
  `pulsen-domain` は0件、`#[allow(unsafe_code)]` は `process.rs:435` の1箇所、本番依存は6クレート
  （grep と Cargo.toml で確認）。`cargo fmt --check` / `cargo clippy --workspace --all-targets -D warnings` /
  `cargo test --workspace` は macOS で全て通り、適合スイートは
  ProcessController 27件 / CommandRunner 16件がスキップ0で走った。
- **`CommandRunner` の timeout**: `try_wait` のポーリングで `Child` の所有権を手放さず、超過時は
  `kill` → `wait` で回収する（`command_runner.rs:91`）。`wait` 自体が失敗する経路も同じ後始末を
  通る（`:106`）。`Instant + Duration` ではなく `started.elapsed()` で比較するので、上限のない
  `DurationSpec` でも全域で定義される。シェル非経由・`env_clear` を呼ばない上書き規則も、
  適合ケース TC-006〜011 が exit code で実測している。排他ロックのファイル記述子は std が
  `O_CLOEXEC`（Windows は非継承ハンドル）で開くため、判定・通知コマンドの子へは漏れない。
- **テストダブルの忠実性**: `RecordSeq` を `save` / `save_degraded` / `run` の3点に置いたのは、
  順序の契約がまたぐポートがちょうどこの3つだからで、過不足がない。`tick_notify.rs:99` が1本の
  列へ並べ直し、`Task` 経路（`:213`）と `DegradedTask` 経路（`:414`）の両方で
  「凍結を書く → 通知 → 通知時刻の追記」を主張している。`ScriptedProcessController` に採番が
  無いのは、そこに順序の契約が無く「呼ばれない」ことしか主張しないため。

#### Warnings

- **[W-001]** HOOKS.md の「フィクスチャの実行ファイルが無い」行が、本スライスで足した11行を
  拾っていない
  - 場所: `crates/pulsen-conformance/HOOKS.md:44`, `:45`
  - 理由: 同じ表の CommandRunner 側（`:49`）は `judge_probe` を要する行を
    `001 / 002 / 005〜016` と漏れなく列挙しているのに、ProcessController 側は
    `001 / 002 / 003 / 017〜021 / 024〜027` のままで、`agent_probe` を要する
    TC-007（`terminated_pid`）と TC-011〜016（`spawn_unit` 経由）が落ちている。`spawn_probe` の
    行（`:45`）も同様に TC-011〜016 を含まない。実装側には
    `ExecutionUnitCapability::ProgramMissing`（`tests/conformance_process_controller.rs:519`）という
    専用の区分まであり、実行ファイルが無い環境ではこれらの行が「許容されないスキップ = 失敗」に
    なる。冒頭（`:3`）が「行を足すときはこの表も更新する」と定めた正本の側だけが追随していない。
  - 提案: `:44` の対象行に `007 / 011〜016` を、`:45` に `011〜016` を足す（判定列の文言はそのままで
    よい。どちらも「スキップ許容集合には入れない」側）。
- **[W-002]** Windows では昇格の段が構造上の no-op なのに、2段目を実際に起動する
  - 場所: `crates/pulsen/src/adapter/process.rs:191`, `:674`
  - 理由: Windows の `terminate::command` は `Graceful` と `Forced` で同じ `taskkill /T /F` を返す
    （`:671` のコメントがそう述べている）。`terminate` はそれを知らずに2段目を起動するため、
    1段目で消滅を観測できなかった場合に、**同じ操作**をもう一度実行したうえでさらに
    `TERMINATION_GRACE` ぶん待つ。得られるものは無く、コストは (a) 排他ロックの保持が最大4秒
    （観測が PowerShell の起動を伴うぶん実測はさらに伸びる）、(b) `KillIdent` が pid そのもので
    ある Windows で、pid 再利用に対する誤殺の窓が2回開くこと。POSIX 側は SIGTERM → SIGKILL で
    段に意味があるので、この問題は Windows 固有。
  - 提案: `terminate` モジュールに「このプラットフォームに昇格があるか」を1つ置き
    （例: `pub const ESCALATES: bool`）、無ければ `terminate` が2段目をスキップして
    1段目の終了ステータスの評価へ進む。分岐は既に `cfg` で割れている `terminate` の中に閉じるので、
    AC-7 の隔離（3ファイル）は動かない。

#### カバレッジ

- 確認: `crates/pulsen/src/adapter/process.rs`, `crates/pulsen/src/adapter/command_runner.rs`,
  `crates/pulsen/src/adapter/mod.rs`, `crates/pulsen-domain/src/execution/port.rs`,
  `crates/pulsen-domain/src/execution/mod.rs`, `crates/pulsen-domain/src/execution/value.rs`,
  `crates/pulsen-domain/src/execution/notification.rs`, `crates/pulsen-conformance/HOOKS.md`,
  `crates/pulsen-conformance/src/lib.rs`, `crates/pulsen-conformance/src/command_runner.rs`,
  `crates/pulsen-conformance/src/process_controller.rs`,
  `crates/pulsen-conformance/src/doubles/mod.rs`,
  `crates/pulsen-conformance/src/doubles/command_runner.rs`,
  `crates/pulsen-conformance/src/doubles/process.rs`,
  `crates/pulsen-conformance/src/doubles/run_store.rs`,
  `crates/pulsen-conformance/src/doubles/task_repository.rs`,
  `crates/pulsen/examples/agent_probe.rs`, `crates/pulsen/examples/judge_probe.rs`,
  `crates/pulsen/src/cli/wire.rs`, `crates/pulsen/src/cli/tick.rs`,
  `crates/pulsen/src/application/tick/mod.rs`, `crates/pulsen/src/application/tick/observe.rs`,
  `crates/pulsen/src/application/tick/notify.rs`,
  `crates/pulsen/tests/conformance_process_controller.rs`,
  `crates/pulsen/tests/conformance_command_runner.rs`, `crates/pulsen/tests/common/mod.rs`,
  `crates/pulsen/tests/tick_fixture/mod.rs`, `crates/pulsen/tests/tick_notify.rs`,
  `crates/pulsen/tests/tick_observe.rs`, `crates/pulsen/tests/cli_tick.rs`
- スキップ: `.thread/3/adr.md`, `.thread/3/plan.md`, `.thread/3/steps.md`, `.thread/3/testing.md` —
  契約・手順書として読んだが、記載の当否は General 観点の担当
- スキップ: `.thread/3/review/`（13ファイル） — レビューの中間成果物（Phase 8 で削除）
- スキップ: `crates/pulsen-domain/src/execution/judgement.rs` — 判定規則は Domain 観点。ポート境界に
  関わる `judge_env` の4変数だけ確認した
- スキップ: `crates/pulsen-domain/src/execution/running.rs`,
  `crates/pulsen-domain/src/task/task.rs`, `crates/pulsen-domain/src/task/degraded.rs`,
  `crates/pulsen-domain/src/task/counters.rs`, `crates/pulsen-domain/src/task/transition.rs` —
  ドメインの遷移・分類ロジックで、Domain 観点の担当
- スキップ: `crates/pulsen/src/application/tick/confirm_spawn.rs`,
  `crates/pulsen/src/application/tick/launch.rs` — 変更はジェネリック引数 `C: CommandRunner` の
  追加のみ（差分で確認）
- スキップ: `crates/pulsen/src/cli/render.rs`, `crates/pulsen/tests/tick_scan.rs` — 表示と走査レベルの
  主張で、UseCase / General 観点の担当
