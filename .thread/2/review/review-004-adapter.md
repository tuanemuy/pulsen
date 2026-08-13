# レビュー 004 — Adapter / Infrastructure

対象: PR #11(`issue/2/tick-agent-run-launch` ← `main`)/ 契約: `.thread/2/plan.md`

## Adapter / Infrastructure

### Blockers

なし

### Warnings

なし

### 検証したこと

指摘がゼロであることの根拠として、この観点で実際に確かめた事実を残す。

**ビルドと静的検査(AC-1)**

- `cargo fmt --check` / `cargo clippy --workspace --all-targets -- -D warnings` / `cargo test --workspace` がいずれも 0 で終わる。
- `unsafe_code = "forbid"`(workspace lints)は据え置き。`Cargo.toml` / `crates/*/Cargo.toml` / `Cargo.lock` に差分は無く、新規依存は入っていない。
- ターゲット述語つき `cfg`(`unix` / `windows` / `target_os` / `target_family`)は `crates/pulsen-domain/` に 0 件。`crates/pulsen/src/` 側は `util/atomic.rs`・`adapter/process.rs`・`adapter/task_repository.rs`(`#[cfg(all(test, unix))]`)の3ファイルのみで、AC-1 の記述と一致する。`crates/pulsen-conformance/src/lib.rs` にも2件あるが、権限制限が実際に効くかを調べるテストハーネスの能力判定(`permission_restrictions_effective`)であり、AC-1 が数える2つの集合のどちらでもない。
- 本環境(macOS)では `#[cfg(target_os = "linux")]` / `#[cfg(windows)]` の `identity` モジュールが型検査されない。使い捨ての `git worktree`(`/private/tmp` 配下)で cfg を差し替え、両モジュールとも単独で `cargo check` を通ることを確認した(Windows 側の `detach` は `std::os::windows` に触れるため対象外 — Issue #10 の実機確認どおり)。ワークツリーは撤去済み。

**同定情報の取得(ADR-067 / AC-9)**

- 起動時刻と PGID の取得はプラットフォームごとの `identity::observe` **1つ**に閉じており、コンパイルされるのは常に1実装。記録側(`own_identity`)と #3 の照合側が同じ関数を共有できる形になっている。
- 戻り値は三値。`Ok(Some)` / `Ok(None)`(対象不在)/ `Err(Io)`(機構失敗)の写像は ADR-067 の表と一致する — POSIX 非 Linux は「非0終了かつ stdout 空」だけを不在に落とし、Linux は procfs ルートの不在(`Err`)と `<root>/<pid>/stat` の `NotFound`(`Ok(None)`)を分ける。不在を機構失敗に畳む経路は無い。二値への畳み込み(`Ok(None)` → `Err(Io)`)は共有関数ではなく `own_identity` 側の1行に置かれている。
- 三値のうち `Ok(None)` と `Err(Io)` は `adapter/process.rs` のユニットテスト(`存在しないプロセスは不在として返り機構の失敗に畳まれない` / `壊した取得元では不在ではなく機構の失敗になる`)と適合ケース TC-005 の両方で走る。
- `ps` の起動は `LC_ALL=C` / `TZ=UTC` を注入し、`LANG` / `LC_TIME` を `env_remove` している。取得元は `/bin/ps` の絶対パスで固定(PATH 差で別実装の `ps` に解決されない)。環境の固定はユニットテストが `Command::get_envs()` で主張する。
- `/proc/<pid>/stat` のパースは `rsplit_once(')')` で comm を落としてから空白分割しており、`comm` に空白・`)` を含む名前でも位置がずれない。索引(pgrp=2 / starttime=19)は proc(5) のフィールド番号(5 / 22)と整合する。

**デタッチ起動と FD(AC-9 / AC-15)**

- `spawn_wrapper` は stdin/stdout/stderr を `Stdio::null()` にし、POSIX は `process_group(0)`、Windows は `CREATE_NEW_PROCESS_GROUP | DETACHED_PROCESS` を付けて起動し、`Child` を待たずに落とす。
- 「呼び出し側の終了後も完走する」は適合ケース TC-002 が別プロセス(`examples/spawn_probe`)経由で実行し、走ることを確認した。
- 「ラッパー自身がプロセスグループの長である」は `crates/pulsen/tests/cli_tick.rs:141` が `kill_ident == "-<ラッパーpid>"` として主張する。`detach` から `process_group(0)` が落ちればこのテストが落ちるので、ADR-067 が最悪の結末として挙げる「同定子が cron / シェルのプロセスグループを指したまま帳簿に永続化される」は検査で守られている。
- ロックFDの非継承は `crates/pulsen/tests/cli_tick.rs:168`(滞留するエージェントを起動したまま次の tick が競合しない)が観測する。ロックファイル・一時ファイルはいずれも std / tempfile 経由で開かれており `O_CLOEXEC` が付く。

**RunStore(AC-7)**

- read 系3種の分類は「不在(ディレクトリごと不在を含む)= `Ok(None)` / 解釈不能 = `Corrupt` / 機構失敗 = `Io`」で実装され、適合21件+追加1件が本環境で全件走る(スキップ0)。
- write 系3種とマーカーは共通ユーティリティ `util::atomic::write_atomic` 経由で、独自のアトミック置換を再実装していない。`write_atomic` が `ensure_dir` するため「ディレクトリ不在なら作って書く」も同じ経路で満たされる。並行読み取り(TC-016)と失敗時に部分内容を残さないこと(TC-017)が実測で走る。
- 排他ロックは `util::fsdir::ensure_dir` + std の `File::try_lock` で、こちらも共通化された置き場の用意を通る。

**WorktreeManager::create(AC-8 / ADR-077)**

- 冪等の境界は「鍵が一致する登録があり、その登録が `ws.branch` を指し、実体が在る」場合だけ。パスの存在だけでは成功にしない。
- 同定は `physical_key`(親のみ canonicalize して file_name を join)を `ws.path` と git の出力の**両側**に対称適用しており、片側正規化ではない。ハーネスが `worktree_root` をシンボリックリンク経由で組むため、正規化の分岐が実際に実行される。
- `-f` は「鍵一致 + 自ブランチ」を確認した後の張り直しだけに使い、登録なし・ブランチのみ残存は `-f` なしで張り直す(先端が動かないことを TC-013 と追加ケースが主張)。復旧2分岐(prunable / 登録なし)の両方が適合スイートで走ることを確認した(17件全件実行)。
- `worktree でない通常ディレクトリ` / `別ブランチの worktree` / `base 不在` はいずれも `Failed` で、既存内容にもブランチにも触れない。
- git はすべて `Command::new(git_program)` の直接起動でシェルを経由しない。引数は `OsStr` のまま渡り、`INHERITED_GIT_ENV` の除去規則は `output()` の1箇所に閉じている。

**その他**

- 追加された3ポート(`RunStore` 9 / `ProcessController` 3 / `WorktreeManager::create`)は spec/domains/execution.md のポート表と1:1で、未実装メソッドの宣言もスタブも無い(AC-6)。
- 合成ルートは `cli/wire.rs` の1箇所。`current_exe()` は spawn を行う経路(`process_controller()`)だけが読み、ラッパーは `compose_wrapper` でホームも config も読まずに `run_dir` から state root を復元する。アダプター型が漏れているのは合成ルートと `Runtime` のアクセサに限る。
- 本番コードの `expect` はアダプター層で `adapter/process.rs:181`(`CommandLine` が1トークン以上という不変条件)の1つだけ。エラーの握り潰しは `sync_dir`(既存・書き込み自体は成功済み)と `physical_key` の `canonicalize` 失敗(`create` 側で `Failed` に写る)に限られ、いずれも why が添えられている。
- AC-16(worktree の手動削除)は `run_agent` の cwd 事前確認 → 126 → exit ファイルという既存経路に落ち、tick 側に新しい分岐は生じない。`cli_wrapper.rs` の受け入れテストが観測する。
- 新規・変更されたコメントは why / why not に限られており、指摘への弁明や修正経緯の記述は見当たらない。テスト用ヘルパー(`tests/common/git.rs`)の追加関数はすべて利用箇所がある。

### カバレッジ

確認: `.adr/027-port-conformance-suite-and-harness-hooks.md`, `.thread/2/adr.md`, `.thread/2/plan.md`, `crates/pulsen-conformance/HOOKS.md`, `crates/pulsen-conformance/src/doubles/process.rs`, `crates/pulsen-conformance/src/doubles/run_store.rs`, `crates/pulsen-conformance/src/doubles/worktree.rs`, `crates/pulsen-conformance/src/lib.rs`, `crates/pulsen-conformance/src/process_controller.rs`, `crates/pulsen-conformance/src/run_store.rs`, `crates/pulsen-conformance/src/worktree_manager.rs`, `crates/pulsen-domain/src/execution/port.rs`, `crates/pulsen/examples/agent_probe.rs`, `crates/pulsen/examples/spawn_probe.rs`, `crates/pulsen/src/adapter/mod.rs`, `crates/pulsen/src/adapter/process.rs`, `crates/pulsen/src/adapter/run_store.rs`, `crates/pulsen/src/adapter/worktree.rs`, `crates/pulsen/src/cli/args.rs`, `crates/pulsen/src/cli/tick.rs`, `crates/pulsen/src/cli/wire.rs`, `crates/pulsen/src/cli/wrapper.rs`, `crates/pulsen/tests/cli_tick.rs`, `crates/pulsen/tests/cli_wrapper.rs`, `crates/pulsen/tests/common/git.rs`, `crates/pulsen/tests/common/mod.rs`, `crates/pulsen/tests/conformance_process_controller.rs`, `crates/pulsen/tests/conformance_run_store.rs`, `crates/pulsen/tests/conformance_worktree.rs`(29件)

スキップ:

- `.thread/2/progress.md`, `.thread/2/steps.md`, `.thread/2/testing.md`, `.thread/2/review/triage.md`, `.thread/2/review/review-001.md`, `.thread/2/review/review-001-adapter.md`, `.thread/2/review/review-001-architecture.md`, `.thread/2/review/review-001-domain.md`, `.thread/2/review/review-001-test.md`, `.thread/2/review/review-001-usecase.md`, `.thread/2/review/review-002-adapter.md`, `.thread/2/review/review-002-architecture.md`, `.thread/2/review/review-002-domain.md`, `.thread/2/review/review-002-test.md`, `.thread/2/review/review-002-usecase.md`, `.thread/2/review/review-003.md`, `.thread/2/review/review-003-adapter.md`, `.thread/2/review/review-003-architecture.md`, `.thread/2/review/review-003-domain.md`, `.thread/2/review/review-003-test.md`, `.thread/2/review/review-003-usecase.md` — 進行記録と過去ラウンドのレビュー成果物。ゼロベースでのレビューという指示により参照しない(21件)
- `crates/pulsen-conformance/src/doubles/clock.rs`, `crates/pulsen-conformance/src/doubles/mod.rs`, `crates/pulsen-conformance/src/doubles/task_repository.rs`, `crates/pulsen-conformance/src/doubles/tests.rs` — 本スライスで足したアダプター(RunStore / ProcessController / WorktreeManager)以外のダブルとその自己検査。テスト観点の担当(4件)
- `crates/pulsen-domain/src/definition/agent.rs`, `crates/pulsen-domain/src/definition/template.rs`, `crates/pulsen-domain/src/execution/launching.rs`, `crates/pulsen-domain/src/execution/mod.rs`, `crates/pulsen-domain/src/execution/value.rs`, `crates/pulsen-domain/src/task/attempt.rs`, `crates/pulsen-domain/src/task/counters.rs`, `crates/pulsen-domain/src/task/failure.rs`, `crates/pulsen-domain/src/task/mod.rs`, `crates/pulsen-domain/src/task/path.rs`, `crates/pulsen-domain/src/task/planner.rs`, `crates/pulsen-domain/src/task/task.rs`, `crates/pulsen-domain/src/task/transition.rs` — ドメイン層。I/O に触れずアダプターの契約に関わらない(ポート定義 `execution/port.rs` は確認済み)。ドメイン観点の担当(13件)
- `crates/pulsen/src/application/mod.rs`, `crates/pulsen/src/application/run_wrapper.rs`, `crates/pulsen/src/application/tick/confirm_spawn.rs`, `crates/pulsen/src/application/tick/launch.rs`, `crates/pulsen/src/application/tick/mod.rs` — ユースケース層。ポート越しの呼び出し順序と分岐はユースケース観点の担当(5件)
- `crates/pulsen/src/cli/mod.rs`, `crates/pulsen/src/cli/render.rs` — 終了コードの決定とサマリーの文言。アダプターの結線は `wire.rs` / `tick.rs` / `wrapper.rs` で確認済み(2件)
- `crates/pulsen/tests/cli_usage.rs`, `crates/pulsen/tests/register_task.rs`, `crates/pulsen/tests/run_wrapper.rs`, `crates/pulsen/tests/tick_confirm_spawn.rs`, `crates/pulsen/tests/tick_fixture/mod.rs`, `crates/pulsen/tests/tick_launch.rs`, `crates/pulsen/tests/tick_scan.rs` — テストダブルに対するユースケーステストと CLI の使い方の受け入れテスト。実アダプターを触らない(7件)

確認29 + スキップ52 = 81。
