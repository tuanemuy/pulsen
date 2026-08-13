# レビュー 009 — Adapter / Infrastructure

## Adapter / Infrastructure

### Blockers

なし

### Warnings

なし

### 所見

ゼロベースで観点ごとに確認した結果、マージを妨げる欠陥は見つからなかった。確認した内容を根拠として残す。

- **OS 依存の隔離と `cfg` の分布**: ターゲット述語つき `cfg`(`unix` / `windows` / `target_os` / `target_family`)は `crates/pulsen-domain/` に1件も無く、`crates/pulsen/src/` 側は `util/atomic.rs`・`adapter/process.rs`・`adapter/task_repository.rs`(`#[cfg(all(test, unix))]`)の3ファイルだけ。AC-1 の期待どおり。プラットフォーム分岐は `adapter/process.rs` 内の `identity` モジュール3つと `detach` / `signal_code` に閉じ、合成ルート(`cli/wire.rs`)は `IdentitySource::platform_default()` を呼ぶだけで cfg を持たない。
- **`unsafe` 禁止・新規依存なし**: `unsafe` の出現はコメント2箇所のみ。`Cargo.toml` / `Cargo.lock` に差分は無く、`pulsen-domain` の `[dependencies]` は空のまま、workspace の `unsafe_code = "forbid"` も維持されている。
- **起動時刻・PGID の三値と各プラットフォームの対称性**: 取得は `identity::observe(&IdentitySource, Pid) -> Result<Option<ObservedProcess>, Io>` の1関数に閉じ、POSIX 非 Linux / Linux / Windows のいずれも ADR-067 の写像表どおりに「不在」と「機構の失敗」を分けている。畳み込み(`Ok(None)` → `Err(Io)`)は `own_identity` の呼び出し側1行だけ。Linux は procfs ルートの不在を `<root>/<pid>/stat` の `NotFound` と区別し、pgrp / ticks は「最後の `)` より後ろ」の位置で読む。Windows は `$ErrorActionPreference = 'Stop'` + `catch` で非終端エラーを非0終了に落とし、完走した空出力だけを不在に写している。
- **`ps` のロケール・TZ 注入**: `LC_ALL=C` / `TZ=UTC` を設定し `LANG` / `LC_TIME` を `env_remove` する。取得元は `/bin/ps` の絶対パスで固定。`取得はロケールとタイムゾーンを固定した環境で行われる` が `get_envs` で固定を主張しており、後から環境の固定を外すと落ちる。
- **デタッチ起動と FD の非継承**: `process_group(0)` / `CREATE_NEW_PROCESS_GROUP|DETACHED_PROCESS`、stdio は3つとも `Stdio::null()`、`Child` は待たずに落とす。ロックFDの非継承は `cli_tick.rs::滞留するエージェントを起動したままでも次のtickは競合しない` が、ラッパー生存中(`exit` 未出現)であることを主張したうえで次の tick がスキップしないことで観測している。デタッチ性そのものは `examples/spawn_probe` 経由の TC-002 が呼び出し側プロセスの終了後に検証する。
- **`create` の冪等性の境界と移植性**: 同定の鍵は `physical_key`(親のみ `canonicalize`)で、`ws.path` と `worktree list --porcelain` の**両側**を同じ関数に通してから比較している。達成済みの条件は「鍵一致 + 自ブランチ + 実体あり + `prunable` でない」で、実体の有無は `try_exists` で直接観測する。復旧の2分岐(`prunable` からの `add -f` / 登録なし・ブランチのみからの `-f` なし `add`)は台帳行 TC-013 と追加ケース `create_prunable` の双方で実行される。適合ハーネスは置き場をシンボリックリンク経由で組み、張れない環境はスキップに載せている。
- **git のシェル非経由**: `Command` に `-C` と引数配列を渡す形のみで、`INHERITED_GIT_ENV` の除去も1箇所(`output`)に閉じている。パスは `OsStr` のまま渡す。
- **read 系の3分類とアトミック置換の共通化**: `FsRunStore` の read 系は「不在(ディレクトリごとを含む)= `Ok(None)` / JSON 破れ・値制約の破れ = `Corrupt` / それ以外 = `Io`」、write 系は `util::atomic::write_atomic` を呼ぶだけで、置換ロジックの再実装は無い。`marker_exists` は `try_exists` を使い、I/O エラーを「無い」に丸めない。
- **ポート宣言の1:1**: `RunStore` 9・`ProcessController` 3・`WorktreeManager` に `create` 1つ。`spec/domains/execution.md` のポート表と一致し、未実装メソッドの宣言・スタブは無い(AC-6)。
- **エラーの握り潰し**: `run_agent` の符号化(cwd 不在 126 / ログ不能 126 / `NotFound` 127 / その他 126 / シグナル死 128+n)以外に、失敗を成功や既定値へ落としている箇所は見当たらない。`sync_dir` の失敗を伝えない扱いだけが意図的で、doc に理由が書かれている。

### テストの実効性(使い捨てコピーでのミューテーション)

作業ツリーは変更せず、スクラッチのコピーに対して確認した(後始末済み)。

- `physical_key` の正規化を両側から外す(生パス比較にする)と `TC-port-worktree-manager-012` と追加ケース `create_prunable` が落ちる。ハーネスがシンボリックリンク経由の置き場を組む前提が実際に効いている。
- `write_atomic` を `std::fs::write` に置き換えると `TC-port-run-store-016 / 017 / 018` と追加ケースが落ちる。アトミック性と「write 系が置き場ごと作る」契約は決定的に検証されている。
- git 出力側の正規化だけを外す片側正規化は macOS では全件緑のままだった。これは ADR-077 が「片側だけの正規化は macOS / Linux では緑のまま Windows でだけ壊れる」と明記している既知の性質で、実装は両側対称なため欠陥ではない。Windows 実機の裏づけは Issue #10 の範囲。

`cargo test --workspace` / `cargo clippy --workspace --all-targets -- -D warnings` / `cargo fmt --check` はいずれも緑(適合スイートの許容外スキップも無し)。

### カバレッジ

確認: `.adr/027-port-conformance-suite-and-harness-hooks.md`, `.thread/2/adr.md`, `.thread/2/plan.md`, `.thread/2/progress.md`, `crates/pulsen-conformance/HOOKS.md`, `crates/pulsen-conformance/src/doubles/clock.rs`, `crates/pulsen-conformance/src/doubles/mod.rs`, `crates/pulsen-conformance/src/doubles/process.rs`, `crates/pulsen-conformance/src/doubles/run_store.rs`, `crates/pulsen-conformance/src/doubles/worktree.rs`, `crates/pulsen-conformance/src/lib.rs`, `crates/pulsen-conformance/src/process_controller.rs`, `crates/pulsen-conformance/src/run_store.rs`, `crates/pulsen-conformance/src/worktree_manager.rs`, `crates/pulsen-domain/src/execution/port.rs`, `crates/pulsen/examples/agent_probe.rs`, `crates/pulsen/examples/spawn_probe.rs`, `crates/pulsen/src/adapter/mod.rs`, `crates/pulsen/src/adapter/process.rs`, `crates/pulsen/src/adapter/run_store.rs`, `crates/pulsen/src/adapter/worktree.rs`, `crates/pulsen/src/application/run_wrapper.rs`, `crates/pulsen/src/cli/args.rs`, `crates/pulsen/src/cli/mod.rs`, `crates/pulsen/src/cli/tick.rs`, `crates/pulsen/src/cli/wire.rs`, `crates/pulsen/src/cli/wrapper.rs`, `crates/pulsen/tests/cli_tick.rs`, `crates/pulsen/tests/cli_tick_missing_cwd.rs`, `crates/pulsen/tests/cli_wrapper.rs`, `crates/pulsen/tests/common/git.rs`, `crates/pulsen/tests/common/mod.rs`, `crates/pulsen/tests/conformance_process_controller.rs`, `crates/pulsen/tests/conformance_run_store.rs`, `crates/pulsen/tests/conformance_worktree.rs`(35件)

スキップ:

- `.thread/2/review/review-001-adapter.md`, `.thread/2/review/review-001-architecture.md`, `.thread/2/review/review-001-domain.md`, `.thread/2/review/review-001-test.md`, `.thread/2/review/review-001-usecase.md`, `.thread/2/review/review-001.md`, `.thread/2/review/review-002-adapter.md`, `.thread/2/review/review-002-architecture.md`, `.thread/2/review/review-002-domain.md`, `.thread/2/review/review-002-test.md`, `.thread/2/review/review-002-usecase.md`, `.thread/2/review/review-003-adapter.md`, `.thread/2/review/review-003-architecture.md`, `.thread/2/review/review-003-domain.md`, `.thread/2/review/review-003-test.md`, `.thread/2/review/review-003-usecase.md`, `.thread/2/review/review-003.md`, `.thread/2/review/review-004-adapter.md`, `.thread/2/review/review-004-architecture.md`, `.thread/2/review/review-004-domain.md`, `.thread/2/review/review-004-test.md`, `.thread/2/review/review-004-usecase.md`, `.thread/2/review/review-004.md`, `.thread/2/review/review-005-adapter.md`, `.thread/2/review/review-005-architecture.md`, `.thread/2/review/review-005-domain.md`, `.thread/2/review/review-005-test.md`, `.thread/2/review/review-005-usecase.md`, `.thread/2/review/review-005.md`, `.thread/2/review/review-006-adapter.md`, `.thread/2/review/review-006-architecture.md`, `.thread/2/review/review-006-domain.md`, `.thread/2/review/review-006-test.md`, `.thread/2/review/review-006-usecase.md`, `.thread/2/review/review-006.md`, `.thread/2/review/review-007-adapter.md`, `.thread/2/review/review-007-architecture.md`, `.thread/2/review/review-007-domain.md`, `.thread/2/review/review-007-test.md`, `.thread/2/review/review-007-usecase.md`, `.thread/2/review/review-007.md`, `.thread/2/review/review-008-adapter.md`, `.thread/2/review/review-008-architecture.md`, `.thread/2/review/review-008-domain.md`, `.thread/2/review/review-008-test.md`, `.thread/2/review/review-008-usecase.md`, `.thread/2/review/review-008.md`, `.thread/2/review/triage.md` — 過去ラウンドのレビュー成果物。本ラウンドはゼロベースで行うため読まない(48件)
- `.thread/2/steps.md`, `.thread/2/testing.md` — 実装手順・検証手順の記録。契約は plan.md / adr.md で足りる(2件)
- `crates/pulsen-domain/src/definition/agent.rs`, `crates/pulsen-domain/src/definition/template.rs`, `crates/pulsen-domain/src/execution/launching.rs`, `crates/pulsen-domain/src/execution/mod.rs`, `crates/pulsen-domain/src/execution/value.rs`, `crates/pulsen-domain/src/task/attempt.rs`, `crates/pulsen-domain/src/task/counters.rs`, `crates/pulsen-domain/src/task/failure.rs`, `crates/pulsen-domain/src/task/mod.rs`, `crates/pulsen-domain/src/task/path.rs`, `crates/pulsen-domain/src/task/planner.rs`, `crates/pulsen-domain/src/task/task.rs`, `crates/pulsen-domain/src/task/transition.rs` — ドメインの値・遷移。アダプターからは `port.rs` の契約越しにしか触れないため、Domain 観点に委ねる(13件)
- `crates/pulsen/src/application/mod.rs`, `crates/pulsen/src/application/tick/confirm_spawn.rs`, `crates/pulsen/src/application/tick/launch.rs`, `crates/pulsen/src/application/tick/mod.rs` — ポート越しの手続き。Usecase 観点に委ねる(4件)
- `crates/pulsen/src/cli/add.rs`, `crates/pulsen/src/cli/render.rs` — 既存コマンドと文言層。アダプターの結線を含まない(2件)
- `crates/pulsen-conformance/src/doubles/task_repository.rs`, `crates/pulsen-conformance/src/doubles/tests.rs` — 本スライスで足したポートのダブル以外。Test 観点に委ねる(2件)
- `crates/pulsen/tests/cli_usage.rs`, `crates/pulsen/tests/register_task.rs`, `crates/pulsen/tests/run_wrapper.rs`, `crates/pulsen/tests/tick_confirm_spawn.rs`, `crates/pulsen/tests/tick_fixture/mod.rs`, `crates/pulsen/tests/tick_launch.rs`, `crates/pulsen/tests/tick_scan.rs` — ダブルに対するユースケーステストと CLI の使い方の検証。実 I/O を持たないため Adapter 観点の対象外(7件)

確認35 + スキップ78 = 113
