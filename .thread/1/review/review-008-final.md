# 最終レビュー（8周目・最終確認） — Issue #1 / PR #8

対象: PR #8（ベース `main`、HEAD `9e871c6`）。重点は7周目の指摘（R6-W-003）を解いた直近コミット `9e871c6`（`git diff HEAD~1...HEAD`）で、あわせて `.thread/1/plan.md` の AC-1〜AC-20 を再検証した。

ラウンド1〜6の台帳（`.thread/1/review/triage.md`、ラウンド6の「訂正」節を含む）を読み、1周目に `wont-fix` と判定された3件（W-004 `unreachable!` / W-012 成功時の tick 案内 / W-017 `create` の TOCTOU）は本ラウンドでも蒸し返していない。

## 結論

**Blocker 0 / Warning 0 — 問題点ゼロ。**

直近コミット `9e871c6`（`attempt_while_held` の共通化）が生んだ**回帰はゼロ**。TC-002 / TC-003 の主張は spec の該当行と一致したままで、`Lock: Sync` の要求範囲は HEAD~1 から**一切広がっていない**（コンパイルで確認）。スレッド・保持プロセスのリークは無く、3回連続実行・各テストバイナリの単体実行・example 退避時のいずれでも FAILED は出ない。適合スイートの残り124ケースを全件走査し、期限のない待ちは**1件も残っていない**。AC-1〜AC-20 は全件合格。

7周目までに `fix` と判定された全指摘が実装に反映され、収束したと判断する。

## Blockers

なし。

## Warnings

なし。

## 直近の変更のレビュー（`9e871c6`）

コード変更は `crates/pulsen-conformance/src/exclusive_lock.rs` の1ファイルのみ（+72 / -27 のうち実質は関数の括り出し）。ほかは `.thread/1/review/triage.md`（R6-W-003 の記録）と過去のレビュー文書。

### (1) TC-002 / TC-003 の主張は変わっていない

`spec/testcases/ports/exclusive-lock.md` の該当2行と突き合わせた。

| spec の行 | 期待結果 | 実装のアサーション |
|---|---|---|
| 別プロセスが同一グローバルホームのロックを保持中 → `try_acquire()` | `Ok(None)`（プロセス間の排他が成立する。エラーではない） | `exclusive_lock.rs:105-111` の `match attempt`。`Attempt::Contended` のみ受理、`Acquired` は「保持中のロックが取得できた」、`Failed` は「競合はエラーではなく取得できなかったこととして返る」で失敗 |
| 別プロセスがロックを保持し続けている → `try_acquire()` | 即座に `Ok(None)` が返る（解放を待ってブロックしない） | `exclusive_lock.rs:128-133`。`!waited`（5秒の期限内に返る）と `Attempt::Contended` |

- TC-002 の主張（`Ok(None)` に写ること）と TC-003 の主張（待たずに返ること）は、共通化の前後でどちらも同じ述語に落ちている。共通化されたのは**観測の仕方**（別スレッドで試み、親が期限を監視する）だけで、判定は各ケースが自前の `match` と失敗メッセージで持つ。
- TC-002 に新たに加わった `assert!(!waited, "競合した取得が期限内に返る")` は、期限を導入した以上必要な判定である。この判定が効くのは「保持中に5秒を超えて待つ実装」に限られ、その実装は TC-003（spec が「即座に」と明記する行）でも必ず落ちるため、スイート全体の合否は spec の行の集合と一致する。TC-002 の主張を狭めても広げてもいない。
- `require!(harness.hold_from_other_process())` は共通ヘルパーの**外**に残っている。フックを提供しないハーネスでは従来どおり TC-002 が `SKIP` になる（後述の example 退避時の実測で確認）。

### (2) `Lock: Sync` は他のケース・他のハーネスへ伝播していない

ワークスペース外にプローブ用クレートを作り、`Cell<u32>` を持つ **!Sync な `ExclusiveLock` 実装**とそのハーネスに対してコンパイルを試した。

| 確認 | 結果 |
|---|---|
| `Sync` を要求しないケース（TC-004 / 005 / 006 / 007。TC-001 も同型）を !Sync ハーネスへ個別に適用 | **コンパイル成功**。境界はケース関数の `where` に閉じており、ハーネストレイト `ExclusiveLockHarness`（`lib.rs:538-585`）には `Sync` 境界が無いことも確認 |
| `exclusive_lock_conformance!` マクロ（7件全部）を !Sync ハーネスへ適用（HEAD = `9e871c6`） | `E0277` が **2件**（`exclusive_lock.rs:98` = TC-002、`:122` = TC-003） |
| 同じプローブを **HEAD~1（`4ed6866`）の worktree** に対して実行 | `E0277` が **1件**（`exclusive_lock.rs:69` = TC-003） |

結論: マクロ経由でスイート全体を適用する場合の要求は HEAD~1 の時点ですでに `Lock: Sync` であり、**この変更で新たに適用できなくなったハーネスは存在しない**（TC-003 が同じ境界を持っていたため）。エラー件数が1→2になっただけで、受理される型の集合は同一。`.adr/060` の「影響」も「ExclusiveLock の適合スイートを適用する実装には `Sync` が要る。満たさない実装は境界エラーとして即座に分かる」と、すでにスイート単位で書かれている。

他ポートのスイートへの伝播も無い。`pulsen-conformance` 自身が `lib.rs:690-754` に `RefCell` ベースの `NotSyncHarness` を置いて「大多数のケースは `Sync` を要求しない」ことをコンパイルで固定しており（`cargo test -p pulsen-conformance` = 13 passed）、TaskRepository の原子性3件は従来どおり `concurrent_repo` フックに隔離されている（ADR-027）。

### (3) スレッドリーク・保持プロセスのリーク・フレーキー

- **スレッド**: `attempt_while_held` は `thread::scope` の中で完結し、`trying.join()` を明示的に呼ぶ。`thread::scope` はパニックによる巻き戻しの前に子スレッドを合流させるため、どの経路でもスレッドは残らない。
- **保持プロセス**: 期限超過（`waited == true`）の枝では `holder.take()` を `release_holder` に渡してから合流する（`:80-83`。解放しないと待つ実装ではスレッドも返らない）。呼び出し側は `assert!(!waited, …)` を**先に**評価するので、`holder.expect("期限内に返ったので保持は続いている")` が `None` に当たる経路は無い。`attempt` が想定外でパニックする枝では `Option<Child>` が巻き戻しで落ち、`Child` とともに `stdin` パイプが閉じて保持プロセス（`examples/lock_holder.rs` は stdin の EOF で終了する）が自然に終わる。HEAD~1 の TC-002 も同じ扱いだったので変化は無い。
- **期限**: `NON_BLOCKING = 5s` は非ブロッキングな `try_acquire`（1回のファイル操作）に対して十分に広い。3回連続実行・単体実行のいずれでも `conformance_lock` は 0.05 秒未満で終わっており、期限に近づく気配は無い。

## 「期限のない待ち」の全件走査

`crates/pulsen-conformance/src/*.rs` の全ファイル（`clock.rs` / `config_store.rs` / `exclusive_lock.rs` / `lib.rs` / `task_id_generator.rs` / `task_repository.rs` / `workflow_store.rs` / `worktree_manager.rs` と `doubles/`）に対し、`thread::` / `spawn` / `Command::` / `Instant` / `loop {` / `while ` / `join` / `recv` / `wait` / `sleep(` / `yield_now` / `Atomic` を走査した。並行・待ちを含む箇所は**4つだけ**で、すべて期限またはスコープ巻き戻しで打ち切られる。

| 箇所 | 待ちの形 | 打ち切り |
|---|---|---|
| `exclusive_lock.rs:63-88`（TC-002 / TC-003、本ラウンドの対象） | 子スレッドの `try_acquire` を親が監視 | `NON_BLOCKING = 5s` の期限。超過時は先に `release_holder` してから `join` |
| `task_repository.rs:706-745`（TC-042） | 読み手の `while writing`、書き手の周回 | 読み手は `StopOnDrop`（`:801-807`）でスコープの巻き戻しに載る。書き手は `SAVE_ROUND_LIMIT = 1_000` で上限つき。観測待ちは `yield_until_observations_exceed`（`:814-823`）の `OBSERVATION_WAIT = 5s` |
| `task_repository.rs:846-885`（TC-044） | 読み手の `while moving`、`archive` 前後の観測待ち | 同上（`StopOnDrop` + 5秒の期限）。`archive` は1回きりの操作 |
| `lib.rs:726-736`（フレームワーク自身のユニットテスト） | 子スレッドで `list_active` を1回 | ループ無し。`thread::scope` が合流 |

`Command::` のヒットは全て `spawn_fail_limit`（設定キー名）の誤検出で、適合スイートは外部プロセスを直接起動しない（起動はすべてハーネスのフック経由）。

フィクスチャ側（`crates/pulsen/tests/`）も併せて走査した。`common/lock.rs:33-57` の `spawn_holder` は `SIGNAL_DEADLINE = 10s`（6周目 R6-W-002 の手当て）、`release`（`:69-73`）と `conformance_lock.rs:74` の `kill_holder` の `Child::wait` は「stdin を閉じた／`kill` した後の終了待ち」で、相手は自前の `examples/lock_holder.rs`（stdin の EOF で `ExitCode::SUCCESS`）に限られる。`src/util/atomic.rs:142-163` の並行ユニットテストも `StopOnDrop` を持つ。`src/application/register_task.rs:156` の `loop` は `retried` フラグで最大2周（Conflict の1回再試行、AC-14）。

**期限のない待ちの残りは0件。**

## 実行結果

### 品質ゲート

```
cargo build            → 成功
cargo test             → 全18ターゲット ok / 0 failed（合計 458 tests）
cargo clippy --all-targets -- -D warnings → 警告0
cargo fmt --check      → 差分なし
```

### `cargo test` の3回連続実行

| 回 | 結果 | 所要 |
|---|---|---|
| 1 | 全ターゲット `ok`、0 failed | 7.52s |
| 2 | 全ターゲット `ok`、0 failed | 6.71s |
| 3 | 全ターゲット `ok`、0 failed | 6.68s |

3回とも各ターゲットの passed 件数が完全に一致（62 / 21 / 31 / 12 / 5 / 24 / 7 / 44 / 10 / 31 / 9 / 22 / 13 / 167）。**フレーキーなし。**

### 各テストバイナリの単体実行

| ターゲット | 結果 |
|---|---|
| `cli_add_boundary` | ok. 21 passed; 0 failed |
| `cli_add_error` | ok. 31 passed; 0 failed |
| `cli_add_normal` | ok. 12 passed; 0 failed |
| `cli_usage` | ok. 5 passed; 0 failed |
| `conformance_config_store` | ok. 24 passed; 0 failed |
| `conformance_lock` | ok. 7 passed; 0 failed |
| `conformance_task_repository` | ok. 44 passed; 0 failed |
| `conformance_time_id` | ok. 10 passed; 0 failed |
| `conformance_workflow_store` | ok. 31 passed; 0 failed |
| `conformance_worktree` | ok. 9 passed; 0 failed |
| `register_task` | ok. 22 passed; 0 failed |
| `--lib`（`pulsen`） | ok. 62 passed; 0 failed |
| `-p pulsen-domain` | ok. 167 passed; 0 failed |
| `-p pulsen-conformance` | ok. 13 passed; 0 failed |

**FAILED は1件も無い。**

### `target/debug/examples/lock_holder` を退避した状態

```
cargo test --test conformance_lock → ok. 7 passed; 0 failed
cargo test --test cli_add_error    → ok. 31 passed; 0 failed
```

`--nocapture` で確認した `conformance_lock` の SKIP 行（4件、いずれも `allowed_skips()` の宣言内）:

```
SKIP tc_port_exclusive_lock_002_別プロセスの保持中は取得できない: ハーネスが hold_from_other_process を提供しないため…
SKIP tc_port_exclusive_lock_003_保持中の取得は待たずに返る: ハーネスが hold_from_other_process を提供しないため…
SKIP tc_port_exclusive_lock_004_ガードのドロップで別プロセスが取得できる: ハーネスが try_acquire_from_other_process を提供しないため…
SKIP tc_port_exclusive_lock_005_保持プロセスの強制終了後は取得できる: ハーネスが hold_from_other_process を提供しないため…
```

共通化後も TC-002 の `require!` が `attempt_while_held` の手前に残っているため、フックを用意できない環境では**失敗ではなくスキップ**になる（5周目 R5-W-001 で確定した扱いのまま）。退避したファイルは復元し、`git status --porcelain` が空であること、復元後は SKIP 行が0件（4件が実走）になることを確認した。

### スキップの走査（`cargo test -- --nocapture | grep -i skip`）

```
SKIP tc_port_clock_005_巻き戻した時刻はそのまま返る: ハーネスが rewind を提供しないため、この環境では前提条件を用意できない
SKIP tc_port_clock_0051_別のケース: …（pulsen-conformance 自身のユニットテスト）
SKIP tc_port_clock_005_時刻の巻き戻し: …（同上）
SKIP tc_port_clock_004_時刻の前進: …（同上）
```

適合スイートの実スキップは `TC-port-clock-005` の**1件のみ**で、plan.md が宣言した唯一のスキップと一致する。残る3行は `SkipBudget` の挙動を固定する `pulsen-conformance` 内のユニットテストの出力であり、適合ケースではない。

## AC-1〜AC-20 の再検証

| AC | 判定 | 根拠 |
|---|---|---|
| AC-1 | 合格 | `cargo build` / `cargo test` / `cargo clippy --all-targets -- -D warnings` / `cargo fmt --check` がすべて成功。`crates/pulsen-domain/Cargo.toml` の `[dependencies]` は空。`crates/*/src/` の `cfg(unix)` / `cfg(windows)` / `cfg(not(unix))` は `crates/pulsen/src/util/atomic.rs`（2件）と `crates/pulsen-conformance/src/lib.rs`（2件）のみで、`crates/pulsen-domain/` は0件。`crates/pulsen/tests/` 側のヒットは `common/mod.rs` と `conformance_task_repository.rs` の権限操作フックだけで、AC-1 の文言どおり |
| AC-2 | 合格 | `pulsen-domain` の167ユニットテストが緑。値オブジェクト・テンプレートは `parse` 経由でのみ生成され、`NameError` / `DurationError` / `CommandError` / `TemplateError` / `ExpansionError` / `AgentDefError` の分岐を網羅 |
| AC-3 | 合格 | `snapshot.rs:59-74` と `workflow.rs:175-219` の `effective_agent` / `effective_model` / `effective_timeout` / `effective_retry_limit`、`reference.rs:58` の `display_name`。区切り文字の `/` と `\` を両方渡すテストは3周目までに確定済みで変更なし |
| AC-4 | 合格 | `assembler.rs` の `WorkflowParseError` に `YamlSyntax` / `UnknownKey` / `ForbiddenKey` / `MissingInitial` / `InitialNotFound` / `EmptyStatuses` / `NoAction` / `MultipleActions` / `UnknownRunValue` / `MissingNext` / `NextNotFound` / `InvalidValue` が揃う。循環・自己参照・到達不能の受理（ADR-010）はユニットテストと `conformance_workflow_store`（31件）で緑 |
| AC-5 | 合格 | `validator.rs:12` の `RegistrationError` 5種と全件収集。`register_task.rs` の「登録時検証のエラーは全件まとめて返り登録は行われない」が複数エラーの台本で緑（1周目 W-009 の手当て） |
| AC-6 | 合格 | `task/task.rs:76` `register` / `:103` `rehydrate`（`RehydrateError::StatusNotInSnapshot`）、`task/degraded.rs:46` `DegradedTask`、`task/state.rs:92` `ExecutionState`（6状態）。`Timestamp` の RFC3339 往復はドメイン内 |
| AC-7 | 合格 | ポートのトレイトは `definition/port.rs` の `ConfigStore` / `WorkflowStore`、`task/port.rs` の `TaskRepository`（7メソッド） / `TaskIdGenerator` / `Clock`、`execution/port.rs` の `WorktreeManager` / `ExclusiveLock` / `LockGuard` の8つ。`todo!()` / `unimplemented!()` は `crates/` 全体で0件。直近の変更はポートに触れていない |
| AC-8 | 合格 | `pulsen-conformance` が独立クレートとして存在し、1ケース = 1 `#[test]`。`HOOKS.md` のユニーク TC ID が **125**、`crates/pulsen-conformance/src/*.rs` の `pub fn tc_port_*` が **125** で完全一致。ハーネスのフックは意味レベル（生 JSON を渡す API は無い）。原子性3件は `concurrent_repo` に隔離され、`lib.rs` の `NotSyncHarness` が非 `Sync` 実装への適用可能性をコンパイルで固定している。**直近の変更で `Sync` 境界の到達範囲は変わっていない**（上記(2)の HEAD~1 比較） |
| AC-9 | 合格 | `conformance_config_store` = 24 passed / 0 failed |
| AC-10 | 合格 | `conformance_workflow_store` = 31 passed / 0 failed |
| AC-11 | 合格 | `conformance_task_repository` = 44 passed / 0 failed |
| AC-12 | 合格 | Clock 5 + TaskIdGenerator 5（`conformance_time_id` = 10 passed）/ ExclusiveLock 7（`conformance_lock` = 7 passed）/ WorktreeManager 9（`conformance_worktree` = 9 passed）= **26件**。実スキップは `TC-port-clock-005` の1件だけで、**25件が実走**。ExclusiveLock は別プロセス間の排他（TC-002 / 003）と強制終了後の取得（TC-005）を通す。`head_branch` の4分岐・`TargetError::Failed` の3メソッド分は `conformance_worktree` が緑 |
| AC-13 | 合格 | 下記「AC-13〜AC-18 / AC-20 の裏取り」 |
| AC-14 | 合格 | 同上 |
| AC-15 | 合格 | 同上 |
| AC-16 | 合格 | 同上 |
| AC-17 | 合格 | 同上 |
| AC-18 | 合格 | 同上 |
| AC-19 | 合格 | アトミック置換は `crates/pulsen/src/util/atomic.rs` の1箇所（`write_atomic` / `rename_atomic`）で、呼び出しは `adapter/task_repository.rs` の4箇所のみ。排他ロックは `adapter/lock.rs` の `FileExclusiveLock` 1箇所で、`try_lock` を使う実装はほかに無い（`crates/pulsen/src/` 全体を grep して0件） |
| AC-20 | 合格 | `todo!()` / `unimplemented!()` / `FIXME` / `XXX` / `HACK` は `crates/` 配下で0件。`unreachable!` は `definition/template.rs:198` の1件のみで、1周目 W-004 の `wont-fix` 判定（不変条件違反への使用として CLAUDE.md が許容、why コメントあり）どおり |

### AC-13〜AC-18 / AC-20 の裏取り

証拠の所在と件数を機械的に突き合わせた（いずれも直近の変更の影響を受けない領域だが、収束確認として全件を確認した）。

**AC-13**: `cli/wire.rs:167-183` の `resolve_home()` が `--home` → `PULSEN_HOME`（空文字は未設定扱い）→ `env::home_dir().join(".pulsen")` の順。`compose()`（`:128-154`）が起動時に `FsConfigStore::load()` を呼び、`WireError::Config { config_path, error }` を作る。唯一のサブコマンド `add`（`cli/add.rs::execute`）は必ず `compose` を通る。3段すべてに検証テストがある — `cli_add_boundary.rs:370`（フラグ > 環境変数、TC-067）/ `:391`（環境変数だけ。既定ホーム側に `.pulsen` が作られないことも確認）/ `:411`（空の環境変数は既定へ落ちる）。未初期化の案内と非0終了は `cli_add_error.rs:63`（TC-014）が `assert_reports(&["未初期化", home_path, config_path])` で3要素を固定。

**AC-14**: `application/register_task.rs::execute`（`:127-184`）が spec の順どおり — `lock.try_acquire()`(:128) → `WorkflowRef::parse` + `workflows.load`(:136-143) → `display_name`(:145) → `resolve_target`(:149) → `RegistrationValidator::validate`(:151) → `ids.generate()`(:157) → `tasks.create`(:164)。Conflict の1回再試行は `:155` の `retried` フラグと `:173-178`。順序はダブルの呼び出し記録で固定（`register_task.rs:417` ロック失敗時は `workflows.requested() == []`、`:535/:560` 対象検証 → 登録時検証）。成功時の表示3要素は `cli_add_normal.rs:55/:77`。

**AC-15**: `cli_add_error.rs` が31件（TC-014〜046 のうち TC-018 / TC-040 を除く全件。この2件は AC-18 でダブルへ委譲）、`cli_add_boundary.rs` が TC-053(:121) / 054・055（`reject_base` ヘルパー :58-75）/ 058(:187)。**31件すべてで `has_no_task()` と `assert_unchanged()` の両方が通ることを確認**（7件はインライン、残りは `reject` / `reject_definition` / `reject_target` の各ヘルパー経由で、いずれも両方を含む）。抜けゼロ。TC-047 / 048 は実 FS で作れないためダブル側。

**AC-16**: `cli_add_boundary.rs` に15件すべて — TC-049(:83) / 050(:88) / 051(:93) / 052(:98) / 056(:149) / 057(:168) / 059(:219) / 060(:240) / 061(:256) / 062(:275) / 063(:295) / 064(:313) / 065(:329) / 066(:348) / 067(:370)。

**AC-17**: `cli_add_normal.rs:180`（TC-009）が `task_status` / `execution.state = pending` / カウンタ全0 / `workspace`・`current_attempt`・`last_failure` が `null`（4周目 R4-W-003 でキー不在を検出できる形に修正済み）を固定。スナップショット埋め込みは `:209`（TC-010）。`state/` 自動作成は `cli_add_boundary.rs:240`（TC-060）。配置 `state/tasks/<id>.json` は `pulsen-domain/src/task/path.rs:234` が実パス文字列で固定。人間可読 JSON は `adapter/task_file.rs:715` が整形済み文字列との全文一致で固定。

**AC-18**: `register_task.rs` に5件すべてが `tc_task_register_task_NNN_` 命名で存在 — TC-012(:352) / TC-018(:436) / TC-040(:612) / TC-047(:378) / TC-048(:399)。同ファイルの `use` は `std::collections::BTreeMap` / `std::path::PathBuf` / `pulsen::application` / `pulsen_conformance::doubles` / `pulsen_domain` のみで、`std::fs` / `std::process` / `tempfile` を一切取り込んでいない（実プロセス・実 FS 不使用）。

**AC-20**: `crates/` 全体で `todo!` / `unimplemented!` / `FIXME` / `XXX` / `HACK` / `#[ignore]` / 「仮実装」「スタブ」「暫定」「未実装」を走査。ヒットは `definition/template.rs:198` の `unreachable!`（1周目 W-004 の `wont-fix`）と `execution/port.rs:5` の「未実装メソッドの宣言とスタブは置かない」という方針を述べた doc コメントの2件のみで、いずれも違反ではない。`#![allow(dead_code)]` は `tests/common/mod.rs:4`（統合テストの共有フィクスチャ）の1箇所だけで、製品コードには無い。

## 確認したが指摘に値しないと判断したもの

- **`.adr/060` の題名と決定文が TC-003 単独の書き方のまま**: 見出しは「そのケースだけ `Lock: Sync` を要求する」、決定文も「TC-003 は…」で、TC-002 が同じ経路を通るようになったことに追いついていない。ただし同 ADR の「影響」節はすでに「ExclusiveLock の適合スイートを適用する実装には `Sync` が要る。満たさない実装は境界エラーとして即座に分かる」と**スイート単位**で書かれており、読み手が誤った宣言を組んで失敗する経路が無い（`HOOKS.md:190` が挙げる TC-002 のフック `hold_from_other_process` + `release_holder` も正しいまま）。5周目 R5-W-001・6周目 R6-W-001 が指摘した「表どおりに宣言を組むと実際に失敗する」性質の食い違いとは違い、実害のある不整合ではないため指摘としては出さない。コード側の doc（`exclusive_lock.rs:47-48`）は「この経路を通るケースだけの要求」と正しく一般化されている。
- **TC-002 が期限超過時に TC-003 と同じ理由で落ちること**: 「保持中に5秒を超えて待って `Ok(None)` を返す実装」は TC-002 の spec 行だけを見れば適合と読めるが、その実装は「即座に返る」を明記する TC-003 の行で必ず落ちる。スイート全体の合否は spec の行の集合と一致するため、判定の過剰・過小は生じない。ハングを失敗に変えるための必要最小限の判定であり、ADR-060 の決定に沿う。
