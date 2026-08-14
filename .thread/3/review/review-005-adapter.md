# レビュー 005 — Adapter / Ports

### Adapter / Ports

#### Blockers

なし

#### Warnings

- **[W-001]** 「その行にだけ現れる」が表の実体と食い違う（013 / 015 / 016 は2行に現れる）
  - 場所: `crates/pulsen-conformance/HOOKS.md:28`
  - 理由: 同じ文の前半「005 / 010 は前提を作れない環境が無いため**この表に現れず**」は「この表」を表全体（フィクスチャ行 `:44` / `:45` / `:46` / `:50` を含む）として使っている。同じ読みを後半に適用すると、013 / 015 は `:46` と `:47` の2行に、016 は `:46` と `:48` の2行に現れるので「だけ」が成立しない。一方で「スキップ許容条件の行に限る」と読み替えると、003 が挙がっている `:44` もフィクスチャ行（`:59` が明示的に「前提を作れない環境には当たらない」と定めた側）なので、今度は 003 の側が成立しない。**どちらの読みでも1箇所ずつ破れる。**

    実害は限定的（表本体は正しく、宣言＝`observation_allowed_skips()` の2集合とも一致することを実測で確認済み）だが、`:376` が「フックの一覧はこのファイルが正本」と宣言し、ラウンド2 W-005 / ラウンド3 W-004 で `:49` / `:44` / `:45` / `:46` の正本性を繰り返し直してきた経緯からすると、その修正の結果として前段の散文だけが取り残された形になっている。散文だけを読んだ後続が「TC-013 は `spawn_probe` を要さない」と読み違えうる。
  - 提案: 「だけ」を落として能力の帰属だけを述べる。例: 「…016 はその一部だけを終了させられることを要するため、**それぞれの能力を判定に持つ行として現れる**（フィクスチャの実行ファイルを要する `:46` にも重ねて現れるが、そちらはスキップ許容条件ではない）」。文の主語を切り分ける必要はなく、1節の置き換えで閉じる。

#### 実測で確かめたこと（HOOKS.md の正本性）

ラウンド4 で3箇所が変わったため、記載と実装・スイート・宣言を機械的に突き合わせた。

- **区分の集計**: 表 `:17-22` の A 41 / B 132 / C 23 / 合計 196 は、10ポート各節の行を数え直した結果と完全一致（A 0+0+21+2+4+1+0+10+3+0、B 23+30+17+1+1+5+15+9+16+15、C 1+1+6+2+0+1+1+2+8+1）。各節の見出しの内訳（例: 「ProcessController（27行 / A 3・B 16・C 8）」）も節内の区分列と一致し、`+ 追加ケース1件` を持つ2節（WorktreeManager / RunStore）だけが本文行より1行多い。
- **`:44` の内訳**: `B（001 / 002 / 017〜021 / 024 / 026 / 027）・C（003 / 025）` = B 10・C 2 = 12件。ProcessController 節の区分列（003 と 025 だけが C）と一致。
- **`:46` の内訳**: `B（007 / 011 / 012 / 014）・C（013 / 015 / 016）` = 7件。節の区分列と一致。フックの帰属も実装どおり — `terminated_pid`（TC-007）は `agent_probe` 単独（`conformance_process_controller.rs:308`）、`live_execution_unit` / `detached_execution_unit` / `orphaned_execution_unit`（TC-011〜016）はいずれも `spawn_unit()` 経由で `agent_probe`（`probe_command`）と `spawn_probe`（`spawn_from_other_process`）の**両方**を要する。
- **`:47` / `:48` と宣言の一致**: `:47` の 011 / 012 / 013 / 015 は `EXECUTION_UNIT_CASES`、`:48` の 014 / 016 は `PARTIAL_TERMINATION_CASES` と過不足なく一致（`conformance_process_controller.rs:491-502`）。`ProgramMissing` が空集合を返すことで、`:46` / `:59` の「実行ファイルの不在は許容しない」が実装として成立している。
- **`:50` の範囲**: `001 / 002 / 005〜016` に 003 / 004 が含まれないことは正しい — `missing_command` / `non_executable_command` は `judge_probe` を要さない（`conformance_command_runner.rs:103-119`）。
- **`:59` の列挙**: 「保持プロセス・テスト用エージェント・テスト用コマンド・デタッチ性のフィクスチャ」の4種が、表の該当4行（`:57` / `:44` / `:50` / `:45`）と1:1で対応する。ラウンド4 の追加で漏れは無い。
- **`:74` の但し書き**: 「この run の後に足した TC-port-process-controller-007 / 011〜016 と TC-port-command-runner-001 / 002 / 005〜016 は…まだ測っていない」が、表の該当行が `未測定` であることと整合する。`:72` の TC-port-command-runner-004 についても同じ。
- **フック一覧（`:371` / `:372`）**: `ProcessControllerHarness` の19フックは trait 定義（`controller` アクセサを除く）と順序も含めて完全一致。`CommandRunnerHarness` の7フックも同様。`:374` のアクセサ列に `fn runner` が足されている。
- **`:87` / `lib.rs`**: 「フックが意味だけを受け取る8ポート」の列挙が `crates/pulsen-conformance/src/lib.rs:14-18` の記述と一致。

  ローカル実測（macOS・非 root）で `cargo test --workspace -- --nocapture` を回した結果、`SKIP` 行は `SkipBudget` 自身を検証する lib ユニットテストの架空ケース3件（`:77` が明記）＋ `TC-port-clock-005`（`:71`）のみ。CommandRunner 16行・ProcessController 観測11行はすべて実行され、宣言の外のスキップは1件も出ていない。

#### `terminate` の最終形

- **境界 parse**（ADR-015）: `terminate::UnitTarget::parse` が POSIX で `-<n>`（`n >= 2`、`MIN_PGID`）、Windows で非0の `<pid>` だけを受理する。`-1` / `-0` / `0` / `-` / 非数値 / 符号なし数値の6値について、`kill` が `Err(Failed)`・`try_kill_remnants` が `NotIdentifiable` になり、**痕跡を残す実体を注入して「終了操作が1度も起動されていない」ことを外から観測**している（`adapter/process.rs:1531-1574`）。「誤殺しないことが型で決まる」という ADR の主張が、実装とテストの両方で成立している。
- **`ESCALATES` の分岐**: POSIX は `-TERM` → `-KILL` の2段、Windows は `taskkill /T /F` の1段。`ESCALATES` が偽の側で2段目を起動しないことにより、ロック保持は最大 `TERMINATION_GRACE`（2秒）に収まり、pid 再利用の窓が二度開かない。ラウンド3 W-005 の主旨が構造として入っている。昇格の実効性は `猶予のうちに消えない実行単位は捕捉できない終了へ昇格させる`（`:1590`）が `trap '' TERM` の実行単位で実証している。
- **`Err(Io)` の扱い**: `gone_within_grace` は観測機構の失敗で即 `false` を返して待たない（`:260`）。壊れた取得元は待っても直らず、待つ間ロックを保持し続けるため、この向きが正しい。`starttime_of` は `Ok(None)` / `Err(Io)` の三値をそのまま返し（`:354-358`）、`observe.rs:135-144` が `Err(Io)` で状態を変更せず報告のみ行う。
- **既消滅への `Ok`**: 成否を終了ステータスではなく消滅の観測で決める帰結として、既に消滅した実行単位への `kill` は `Ok`。`終了ステータスが非0でも実行単位が消えていれば成功になる`（`:1578`）が固定している。ゾンビが列挙に残る場合の逃げ道（終了ステータス成功なら `Ok`）も `:186-189` に why として残っており、Linux の procfs 実装がゾンビを `true` と読むことと整合する。
- **ロック保持時間**: ADR-015 の Consequences（昇格ありで最大4秒 / なしで2秒）と `TERMINATION_GRACE`（2秒）・`TERMINATION_POLL`（50ms）の定義（`:42-49`）が一致。判定 timeout（既定60秒）に対して十分小さい。
- **`unit_is_live` の実測確認**: macOS で `LC_ALL=C TZ=UTC /bin/ps -o pid= -g <消滅した pgid>` が exit 1・stdout 空になることを確認した。`:826` の判定（非0かつ stdout 非空だけを `Err(Io)`）により、これが正しく `Ok(false)` に落ちる。

#### `CommandRunner` の timeout

- **溢れの回避**: `started.elapsed() >= limit` で比較しており、`Instant + Duration` を作らない（`adapter/command_runner.rs:83-91`）。`DurationSpec` が上限のない `u64` 秒を受理する以上この形が全域で定義される。
- **ゾンビ・`Err` 経路の後始末**: timeout 経路（`:92-95`）と `try_wait` / `wait` の `Err` 経路（`wait_failed`、`:106-112`）の**両方**で `kill()` → `wait()` を行っており、扱いが揃っている。ラウンド1 の指摘（cron で回り続ける tick に子が溜まる）が両経路で閉じている。
- **シェル非経由**: `Command::new(program).args(args)` の直接起動で、`sh -c` を経由しない。TC-006（メタ文字）/ TC-007（プレースホルダ）が、期待を**ファイル**で渡す形（`judge_probe check-args <期待ファイル>`）で検証しており、シェルが解釈した場合に期待側も同じく歪んで通ってしまう罠を避けている。
- **`env`**: `env_clear` を呼ばずに継承したうえで `command.env(name, value)` で追加・上書き（`:45-50`）。契約どおり。TC-008（継承）/ 009（追加）/ 010（上書き）が3方向を分けて検証している。
- **符号化の共有**: `super::process::encode` を `run_agent` と共有しており（`:13`、`process.rs:395`）、AC-7 の「OS 依存分岐を自前で持たない」を構造として満たす。`command_runner.rs` にターゲット述語つき `cfg` は1件も無い。

#### 適合スイートの実効性と probe

- `probe_execution_unit`（`conformance_process_controller.rs:534-555`）が**本番のケースと同じ手順**（実行単位を起こす → その一部だけを終了させる）で能力を測るため、判定と実際のスキップが食い違わない。`OnceLock` で1度だけ。判定後は `kill` で実行単位を畳み、滞留の解放は `Drop` が書く。
- `ProgramMissing` を能力側ではなく失敗側（空の許容集合）に置く区別が、ADR-073 の基準（宣言だけで「なぜ走らなかったか」と「次の一手」が定まるか）に沿っている。
- 権限系フック（`deny_dir_write` / `deny_execute`）は制限が実際に効いたことを確かめてから `Some` を返す（ADR-027）。`deny_execute` は起動して `PermissionDenied` を確認するところまでやっている。
- TC-015 は同定不能なコントローラで `try_kill_remnants` を呼び、**別の（正常な）コントローラ**でメンバーの生存を確認する形になっており、「いかなるプロセスも終了させない」を誤殺の不在として観測できている。
- `evidence_path` が呼び出しごとに別パスを返す契約（`serial` による連番）を守っており、前のケースの証跡が TC-012 の観測を壊さない。

#### テストダブルの忠実性（`RecordSeq`）

- 採番は4メソッド — `TaskRepository::save` / `save_degraded`、`CommandRunner::run`、`ProcessController::try_kill_remnants`。順序の契約がポートをまたぐのはこの4つだけで、`ProcessController` の他4メソッドは `None` で採番せず `calls_in_order()` にも現れない（その理由が `doubles/process.rs` の doc に書かれている）。
- `tick_notify.rs:100-144` が `save` / `save_degraded` / `run` を1本の列に並べ直して「凍結を書く → 通知 → `notified_at` を追記」を主張し、`tick_observe.rs:107-142` が `try_kill_remnants` → `save(Failed)` を主張する。ラウンド2 W-004（`save_degraded` 経路）とラウンド3 W-003（`ProcessController` 側）の穴が両方塞がっている。`ProcessControllerCall` の網羅 `match`（ワイルドカードなし）により、変種を足したときにこの分類が破れない。
- `ScriptedRunStore::read_exit` はパニックから台本つきの記録へ置き換わり、`RunStoreCall::ReadExit` も足されている。

#### OS 依存の隔離（AC-7）— 実測

- `cargo fmt --check` / `cargo clippy --workspace --all-targets -- -D warnings` / `cargo test --workspace` すべて通過（0 failed）。
- `#[allow(unsafe_code)]` は `crates/pulsen/src/adapter/process.rs:454`（Windows ハンドル抑止）の**1箇所のみ**。`unsafe_code = "forbid"`（workspace / pulsen-domain）と `"deny"`（pulsen）は変更なし。
- `crates/pulsen-domain/src/` のターゲット述語つき `cfg` は**0件**。`crates/pulsen/src/` 側のヒットは `util/atomic.rs` / `adapter/task_repository.rs` / `adapter/process.rs` の**3ファイル**で、`adapter/command_runner.rs` は含まれない。
- `crates/pulsen-domain/Cargo.toml` の `[dependencies]` は空。`crates/pulsen` の本番依存は `pulsen-domain` + clap / getrandom / serde / serde_json / serde_yaml_ng / tempfile の6クレートから増えていない。

#### セキュリティ・パフォーマンス

- **誤殺**: 境界 parse で `-1` / `-0` / `0` を排除し、`try_kill_remnants` は列挙できたときにだけ終了を実行する。PID 再利用に対する強さが無いことは `process.rs:372-377` に why として明記され、ADR-002 との関係も正確（「starttime 照合が PID 再利用に対して持つ強さは無い」）。実態を超えた記述は見当たらない。
- **コマンドインジェクション**: `CommandRunner` / `run_agent` / `terminate::command` / `identity_command` のいずれもシェルを経由せず、トークンを引数として直接渡す。`UnitTarget::operand()` は読み取った `u32` から `format!("-{}", ...)` で組み直すため、同定子に混ざった表記が終了操作の引数へ流れ込まない。`--` はオペランドの明示として残っている。
- **環境変数**: `identity::fixed_env_command` が `LC_ALL` / `TZ` を固定し `LANG` / `LC_TIME` を除去する規律を、`observe` と `unit_is_live` の**両方**が共有している（取得と列挙で表現がずれない）。`CommandRunner` は継承 + 上書きのみで、`env_clear` による意図しない剥奪も無い。
- **パフォーマンス**: ポーリング間隔（`POLL_INTERVAL` 50ms / `TERMINATION_POLL` 50ms）と待ち上限（`TERMINATION_GRACE` 2秒）が定数として1箇所に集まり、それぞれ why を持つ。tick がロックを保持する最大時間は ADR-018 / ADR-015 が承知した範囲に収まる。

#### 弁明・経緯の残存

`crates/**/*.rs` の追加行から「ラウンド」「指摘」「レビュー」「以前は」「かつては」「修正」「変更した」「もともと」「従来」を機械的に走査して0件。コメントは仕様・契約・why / why not だけで構成されている。

#### カバレッジ

- 確認: `crates/pulsen/src/adapter/command_runner.rs`, `crates/pulsen/src/adapter/process.rs`, `crates/pulsen/src/adapter/mod.rs`, `crates/pulsen/src/cli/wire.rs`, `crates/pulsen/src/cli/tick.rs`, `crates/pulsen-domain/src/execution/port.rs`, `crates/pulsen-conformance/HOOKS.md`, `crates/pulsen-conformance/src/lib.rs`, `crates/pulsen-conformance/src/command_runner.rs`, `crates/pulsen-conformance/src/process_controller.rs`, `crates/pulsen-conformance/src/doubles/mod.rs`, `crates/pulsen-conformance/src/doubles/command_runner.rs`, `crates/pulsen-conformance/src/doubles/process.rs`, `crates/pulsen-conformance/src/doubles/run_store.rs`, `crates/pulsen-conformance/src/doubles/task_repository.rs`, `crates/pulsen/tests/conformance_command_runner.rs`, `crates/pulsen/tests/conformance_process_controller.rs`, `crates/pulsen/tests/common/mod.rs`, `crates/pulsen/examples/agent_probe.rs`, `crates/pulsen/examples/judge_probe.rs`, `crates/pulsen/src/application/tick/observe.rs`, `crates/pulsen/src/application/tick/notify.rs`, `.thread/3/plan.md`, `.thread/3/review/triage.md`
- 確認（自観点に関わる部分に限定）: `.thread/3/adr.md` — ADR-001 / 002 / 007 / 013 / 015 と実装の一致だけを照合、`crates/pulsen/tests/tick_notify.rs` / `crates/pulsen/tests/tick_observe.rs` — `RecordSeq` による順序の主張だけを確認、`crates/pulsen/tests/cli_tick.rs` — フィクスチャ実行ファイルの解決が `expect`（スキップにしない）であることだけを確認
- スキップ: `.thread/3/review/` 配下24ファイル（plan-001〜003・review-001〜004 の各観点・統合） — レビューの中間成果物で Phase 8 で削除される
- スキップ: `.thread/3/steps.md`, `.thread/3/testing.md` — 実装手順書と手動確認の台本。General / Usecase の担当範囲で、自観点からの新規の主張は無い
- スキップ: `crates/pulsen-domain/src/execution/judgement.rs`, `.../notification.rs`, `.../running.rs`, `.../value.rs`, `.../mod.rs` — ドメインの型設計と遷移。Domain の担当範囲（`NOTIFY_TIMEOUT = 60秒` がポート経由で渡ることだけは notify.rs 側で確認済み）
- スキップ: `crates/pulsen-domain/src/task/counters.rs`, `.../degraded.rs`, `.../task.rs`, `.../transition.rs` — 同上
- スキップ: `crates/pulsen/src/application/tick/mod.rs`, `.../launch.rs`, `.../confirm_spawn.rs` — tick の合成と既存アーム。Usecase の担当範囲
- スキップ: `crates/pulsen/src/cli/render.rs` — 報告の文言組み立て。General の担当範囲
- スキップ: `crates/pulsen/tests/tick_fixture/mod.rs`, `crates/pulsen/tests/tick_scan.rs` — ダブルに対するユースケース層テストの土台と走査レベルの主張。Usecase の担当範囲
