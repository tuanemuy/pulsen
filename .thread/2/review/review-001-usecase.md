# レビュー 001 — Use Case

## Use Case

### Blockers

なし。

`Tick` の処理フロー(ロック → `list_active` → エントリ分岐 → サマリー)、手続きA の順序(worktree確保 → 展開 → launching記録 → `prepare_attempt` → `spawn_wrapper`)、手続きC の順序(read → classify → マーカー → 再読 → `classify_recheck`)、`RunWrapper` の順序(`own_identity` → `write_starttime` → `write_pid_file` → `marker_exists` → `run_agent` → `write_exit`)はいずれも spec/usecases/execution.md と一致していた。二重起動を防ぐ順序プロトコル(ラッパー「pid の後にマーカー確認」× tick「マーカーの後に pid 再確認」)は両側とも実装され、値に現れない順序をダブルの `calls()` で主張している(`crates/pulsen/tests/run_wrapper.rs:93`、`crates/pulsen/tests/tick_confirm_spawn.rs:158`)。スコープ外の手続き(B / D / E / advance / notify)は宣言もされておらず、未配線アームは `Branch` の網羅 `match` の空アームとして本当に何もしない。

### Warnings

- **[W-001]** `read_starttime` の `RunFileError` 経路がテストで一度も注入されていない
  - 場所: `crates/pulsen/src/application/tick/confirm_spawn.rs:151-160`、`crates/pulsen/tests/tick_confirm_spawn.rs:295-320`
  - 理由: AC-17 は「`RunFileError` の各種」をダブルに対するユースケーステストで消化すると定める。`runファイルを読めなければ報告してスキップする` は `read_pid_file` にだけ `Corrupt` / `Io` を注入しており、`read_run_files` の後半(starttime 側)の分岐は初回・再読とも一度も実行されていない。starttime の読み取り失敗を握り潰して `Ok(None)` に畳む実装(= 猶予経路へ誤って合流し、書き込まないはずの局面で `record_spawn_failure` まで進む)に書き換えても、いまのスイートは全部緑のままになる。手続きC は「読めない run ファイルでは一切書き込まない」ことが滞留の設計そのものなので、pid 側だけで代表させるには分岐の意味が重い。
  - 提案: `runファイルを読めなければ報告してスキップする` を pid / starttime の2軸に広げる(`with_read_pid_file([Ok(None)])` + `with_read_starttime([Err(..)])`)。再読側は `with_read_pid_file([Ok(None), Ok(None)])` + `with_read_starttime([Ok(None), Err(..)])` で1件足せば、マーカー書き込み後に読めなくなった場合も書き込まないことまで主張できる。

- **[W-002]** 「処理対象のタスクはありませんでした」が、走査したが何も起きなかった場合にも出る
  - 場所: `crates/pulsen/src/cli/render.rs`(`tick_summary` の `summary.is_empty()` 分岐)、`crates/pulsen/src/application/tick/mod.rs:157`
  - 理由: spec は「処理対象がなければその旨を表示」(usecases/execution.md 出力DTO)で、これに紐づく TC は `TC-exec-tick-012`(タスク0件)と `TC-exec-tick-027`(`state/tasks/` 未作成)のどちらも**タスクが1件もない**ケース。現在の実装は「サマリーの全フィールドが空」を条件にしているため、猶予時間内の launching タスクや Wait ステータスのタスクだけが並んでいる tick でも同じ文言になる。cron.log を追う運用(monitoring.md)では「タスクが無い」と「全タスクが待ち中」が区別できず、tick が走査対象を見失っている事故を取りこぼす。
  - 提案: `TickSummary` に走査件数(あるいは「1件も走査しなかった」ことを表す値)を持たせ、文言を「走査した結果、記録すべき動きはありませんでした」と「処理対象のタスクはありませんでした」に分ける。#3 / #6 がアームを埋めると空サマリーの頻度はさらに上がるので、いま分けておくほうが安い。

- **[W-003]** ラッパー経路が `current_exe()` に依存している
  - 場所: `crates/pulsen/src/cli/wire.rs`(`compose_wrapper` → `process_controller`)
  - 理由: `compose_wrapper` の doc は「ラッパーが必要とする情報はすべて起動引数で受け取る(ADR-006)」と宣言しているのに、`SystemProcessController::new` が `self_exe` を必須にしているため、ラッパーは使いもしない自バイナリのパス解決に成功しないと起動できない。`self_exe` はラッパー側で参照されるフィールドではない(`spawn_wrapper` は tick だけが呼ぶ)。失敗しても pid が現れないので猶予経路が spawn 失敗として拾い、実害は「原因が run ディレクトリに残らない spawn 失敗が1種類増える」に留まるが、ラッパーの依存を絞った設計判断に穴が空いている。
  - 提案: `SystemProcessController` の `self_exe` を `Option` にして `spawn_wrapper` の中でだけ不在を `SpawnError` に落とすか、`spawn_wrapper` を持つ型と `own_identity` / `run_agent` を持つ型を分けて、ラッパーの合成ルートが後者だけを組むようにする。

- **[W-004]** attempt 番号の導出源が手続きAの中で二重になっている
  - 場所: `crates/pulsen/src/application/tick/launch.rs:41-60`
  - 理由: `prepare_attempt` に渡す番号は `task.next_attempt_number()` から、run ディレクトリは `task.record_launching(...)` の戻り値から取っている。両者はいまは同じ規則(`record_launching` が内部で `next_attempt_number` を呼ぶ)で一致するが、帳簿に載る run ディレクトリと実際に作られるディレクトリが別々の呼び出しに依存する形になっており、`record_launching` の採番規則が変わったときに片側だけがずれる。ずれると帳簿の `run_dir` は attempt N なのにディレクトリは N-1 に作られ、猶予経路が毎回 spawn 失敗を積む縮退になる。`起動記録は次の番号を採番し導出したrunディレクトリを渡す` が現状のずれは検出するが、依存関係そのものは暗黙のまま。
  - 提案: `record_launching` の戻り値から番号を取る(`recorded.current_attempt()` の `number()`)か、`prepare_attempt` の引数を `RunDirPath` にして導出点を `record_launching` 1つに寄せる。後者はポート表(AC-6)を変えるので、本スライスでは前者。

### 確認した点(記録)

- **Tick の順序と exit code**: ロック競合 → `TickOutcome::Skipped` → 0、`LockError::Failed` → `TickError::LockFailed` → 非0、`list_active` の Io → `TickError::Scan` → 非0で状態不変。いずれもダブルで直接主張されている(`tick_scan.rs:25/37/55`)。config 読み込みがロックより前なのは spec 共通事項どおり(タスク状態を触らないため排他の外でよい)。
- **1タスクの失敗で tick を落とさない**: `process` / `dispatch` は全経路が `summary.errors` に積んで戻るだけで、`?` も `panic` も持たない。残る `expect` は `workspace_path` / `retry_limit` の2箇所で、どちらも `Branch::Launch` が構成として保証する不変条件(ADR-017)。タスクファイル側の不変条件1は `Task::rehydrate` が値で弾き `TaskEntry::Corrupt` になるため、走査からパニックには到達しない。
- **launching 記録より後は状態を変更しない**: `prepare_attempt` 失敗・`spawn_wrapper` の同期エラーのどちらも `errors` に積むだけで、保存されたタスクは `Launching` のまま(`tick_launch.rs:269/299`)。遅延起動ラッパーと次 attempt の並走を招く「pending へ戻す」実装は入っていない。
- **`write_invalidation_marker` の失敗**: 状態を変更せず報告してスキップし、`saved()` が空であることまで主張されている(`tick_confirm_spawn.rs:236`)。
- **RunWrapper の安全側**: `marker_exists` の `Ok(true)` と `Err(Io)` を同じアームで受け、どちらも `run_agent` を呼ばない。`processes.calls()` が `OwnIdentity` だけであることで主張されている。`write_pid_file` 失敗時に starttime だけが残るのは、`LaunchingClassifier::classify` が `(None, Some)` を猶予経路に合流させる設計と対になっており spec どおり。
- **冪等性**: 同じ走査結果で tick を2回回して書き込みが発生しないこと(`tick_scan.rs:247`)、worktree 作成後の保存前クラッシュから同じワークスペースが再導出されること(`tick_launch.rs:448`)、失敗確定からの再起動が新 attempt・同一 worktree になること(`cli_tick.rs:187`)が実効的に主張されている。
- **スコープ**: `advance` / notify / gc / `list_runs` / `delete_attempt` / `starttime_of` / `kill` / `try_kill_remnants` / `attempt_exists` はソースに1件も現れない。`RunStore` の宣言は spec の9メソッドちょうど。
- **ダブルの誠実さ**: `ScriptedRunStore` / `ScriptedProcessController` は台本を使い切ると `panic!` する。想定外の呼び出しが暗黙に成功へ落ちないので、呼び出し列のアサーションが形骸化していない。
- **コメント**: 弁明・修正の経緯は見当たらない。未配線アームの `// 終端処理(手続きB)は Issue #6 が入れる。` 系は、空アームが空である理由(why not)の説明として残ってよい範囲。

## カバレッジ

一覧は62行(`changed-files.txt` の非空行数。依頼文の「61ファイル」と1件ずれるため実ファイルの行数に合わせた)。確認30件 + スキップ32件 = 62件。

### 確認

- `.thread/2/plan.md`
- `.thread/2/adr.md`(ADR-001 / 002 / 016 / 017 の該当箇所)
- `crates/pulsen/src/application/tick/mod.rs`
- `crates/pulsen/src/application/tick/launch.rs`
- `crates/pulsen/src/application/tick/confirm_spawn.rs`
- `crates/pulsen/src/application/run_wrapper.rs`
- `crates/pulsen/src/application/mod.rs`
- `crates/pulsen/src/cli/tick.rs`
- `crates/pulsen/src/cli/wrapper.rs`
- `crates/pulsen/src/cli/mod.rs`
- `crates/pulsen/src/cli/args.rs`
- `crates/pulsen/src/cli/wire.rs`
- `crates/pulsen/src/cli/render.rs`
- `crates/pulsen/src/adapter/process.rs`(`spawn_wrapper` のデタッチ・`run_agent` の副作用順序のみ)
- `crates/pulsen-domain/src/execution/port.rs`
- `crates/pulsen-domain/src/execution/launching.rs`
- `crates/pulsen-domain/src/task/planner.rs`
- `crates/pulsen-domain/src/task/transition.rs`
- `crates/pulsen-domain/src/task/task.rs`
- `crates/pulsen-domain/src/task/attempt.rs`
- `crates/pulsen-conformance/src/doubles/run_store.rs`
- `crates/pulsen-conformance/src/doubles/process.rs`
- `crates/pulsen/tests/tick_fixture/mod.rs`
- `crates/pulsen/tests/tick_scan.rs`
- `crates/pulsen/tests/tick_launch.rs`
- `crates/pulsen/tests/tick_confirm_spawn.rs`
- `crates/pulsen/tests/run_wrapper.rs`
- `crates/pulsen/tests/cli_tick.rs`
- `crates/pulsen/tests/cli_wrapper.rs`
- `crates/pulsen/tests/common/mod.rs`

差分外の参照として `spec/usecases/execution.md` / `spec/flows/index.md` / `spec/pages/index.md` / `spec/testcases/execution/tick.md` / `spec/inventory/*.md` / `CLAUDE.md` / `crates/pulsen/src/adapter/task_repository.rs` も読んでいる。

### スキップ

- `.thread/2/progress.md`, `.thread/2/steps.md`, `.thread/2/testing.md` — 進行管理と手動テストの記録で、ユースケースの振る舞いを定めない。
- `crates/pulsen-conformance/HOOKS.md`, `crates/pulsen-conformance/src/lib.rs`, `crates/pulsen-conformance/src/process_controller.rs`, `crates/pulsen-conformance/src/run_store.rs`, `crates/pulsen-conformance/src/worktree_manager.rs` — ポート適合スイートの本体で、アダプター/ポート観点の担当。
- `crates/pulsen-conformance/src/doubles/clock.rs`, `doubles/mod.rs`, `doubles/task_repository.rs`, `doubles/tests.rs`, `doubles/worktree.rs` — ダブルの整備。ユースケーステスト経由の利用可否だけ確認し、実装自体はテスト観点の担当。
- `crates/pulsen-domain/src/definition/agent.rs`, `definition/template.rs`, `execution/mod.rs`, `execution/value.rs`, `task/counters.rs`, `task/failure.rs`, `task/mod.rs`, `task/path.rs` — 値型・エラー説明・再輸出で、ドメイン観点の担当。
- `crates/pulsen/src/adapter/mod.rs`, `adapter/run_store.rs`, `adapter/worktree.rs` — アダプター実装。ポート契約の充足はアダプター観点の担当。
- `crates/pulsen/examples/agent_probe.rs`, `examples/spawn_probe.rs` — テスト用プローブ。
- `crates/pulsen/tests/cli_usage.rs`, `tests/common/git.rs`, `tests/register_task.rs` — 既存コマンドのヘルプ・git ヘルパー・登録の受け入れで、tick / wrapper の振る舞いを扱わない。
- `crates/pulsen/tests/conformance_process_controller.rs`, `tests/conformance_run_store.rs`, `tests/conformance_worktree.rs` — 適合スイートの適用点。
