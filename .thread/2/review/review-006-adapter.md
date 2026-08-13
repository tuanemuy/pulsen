# レビュー 006 — Adapter / Infrastructure

## Adapter / Infrastructure

### Blockers

なし。

OS 依存の隔離・三値の同定情報取得・デタッチ起動・アトミック置換の共通化・`create` の冪等性の境界は、いずれも契約どおりに実装され、実効的なテストで守られている。契約違反・データ破壊・二重起動を招く欠陥は見つからなかった。

### Warnings

- **[W-001]** `run_worktree` の doc コメントが実装と食い違う / `crates/pulsen/src/adapter/worktree.rs:78`(`/// worktree 操作の `git -C <repo> <args...>`。非0終了も失敗として畳む。`)/ この関数が `WorktreeError` に畳むのは**起動自体の失敗**だけで、非0終了は `Output` として呼び出し側に返る。実際 `create` は `worktree list` の `listing.status.success()` と `show-ref` の `.status.success()` を自分で見ており、コメントの記述を信じると「非0はここで弾かれている」と読めてしまう。非0を畳むのは直下の `require_success`(そちらのコメントは正確)。/ 提案: 「起動自体の失敗を `WorktreeError` に写す。非0終了は呼び出し側が判断する」に直す。

### カバレッジ

確認（33）:
`.adr/027-port-conformance-suite-and-harness-hooks.md`,
`.thread/2/plan.md`,
`crates/pulsen-conformance/HOOKS.md`,
`crates/pulsen-conformance/src/doubles/process.rs`,
`crates/pulsen-conformance/src/doubles/run_store.rs`,
`crates/pulsen-conformance/src/lib.rs`,
`crates/pulsen-conformance/src/process_controller.rs`,
`crates/pulsen-conformance/src/run_store.rs`,
`crates/pulsen-conformance/src/worktree_manager.rs`,
`crates/pulsen-domain/src/definition/template.rs`,
`crates/pulsen-domain/src/execution/port.rs`,
`crates/pulsen-domain/src/task/path.rs`,
`crates/pulsen/examples/agent_probe.rs`,
`crates/pulsen/examples/spawn_probe.rs`,
`crates/pulsen/src/adapter/mod.rs`,
`crates/pulsen/src/adapter/process.rs`,
`crates/pulsen/src/adapter/run_store.rs`,
`crates/pulsen/src/adapter/worktree.rs`,
`crates/pulsen/src/application/run_wrapper.rs`,
`crates/pulsen/src/application/tick/launch.rs`,
`crates/pulsen/src/cli/args.rs`,
`crates/pulsen/src/cli/mod.rs`,
`crates/pulsen/src/cli/tick.rs`,
`crates/pulsen/src/cli/wire.rs`,
`crates/pulsen/src/cli/wrapper.rs`,
`crates/pulsen/tests/cli_tick.rs`,
`crates/pulsen/tests/cli_tick_missing_cwd.rs`,
`crates/pulsen/tests/cli_wrapper.rs`,
`crates/pulsen/tests/common/git.rs`,
`crates/pulsen/tests/common/mod.rs`,
`crates/pulsen/tests/conformance_process_controller.rs`,
`crates/pulsen/tests/conformance_run_store.rs`,
`crates/pulsen/tests/conformance_worktree.rs`

（差分に無い `crates/pulsen/src/util/atomic.rs` / `util/fsdir.rs` / `adapter/lock.rs` も、共通ユーティリティが再実装されていないことの確認のため参照した。いずれも本 PR で変更されていない。）

スキップ（62）:

- `.thread/2/review/review-001-adapter.md`, `review-001-architecture.md`, `review-001-domain.md`, `review-001-test.md`, `review-001-usecase.md`, `review-001.md`, `review-002-adapter.md`, `review-002-architecture.md`, `review-002-domain.md`, `review-002-test.md`, `review-002-usecase.md`, `review-003-adapter.md`, `review-003-architecture.md`, `review-003-domain.md`, `review-003-test.md`, `review-003-usecase.md`, `review-003.md`, `review-004-adapter.md`, `review-004-architecture.md`, `review-004-domain.md`, `review-004-test.md`, `review-004-usecase.md`, `review-004.md`, `review-005-adapter.md`, `review-005-architecture.md`, `review-005-domain.md`, `review-005-test.md`, `review-005-usecase.md`, `review-005.md`, `triage.md` — 30件。ゼロベースでレビューする指示のため読まない。
- `.thread/2/adr.md`, `.thread/2/progress.md`, `.thread/2/steps.md`, `.thread/2/testing.md` — 4件。設計判断の記録と進行管理の文書。ADR-067 / 068 / 069 / 070 / 074 / 075 / 077 の内容はコード側のコメントと突き合わせて実装の妥当性を判断したため、文書そのものは対象にしない。
- `crates/pulsen-conformance/src/doubles/clock.rs`, `doubles/mod.rs`, `doubles/task_repository.rs`, `doubles/tests.rs`, `doubles/worktree.rs` — 5件。テストダブルの拡張で、実アダプターの挙動を規定しない（Test 観点）。ただし本スライスで新設された `doubles/process.rs` / `doubles/run_store.rs` は、ポート契約の写しとして確認した。
- `crates/pulsen-domain/src/definition/agent.rs`, `execution/launching.rs`, `execution/mod.rs`, `execution/value.rs`, `task/attempt.rs`, `task/counters.rs`, `task/failure.rs`, `task/mod.rs`, `task/planner.rs`, `task/task.rs`, `task/transition.rs` — 11件。ドメインの遷移・分類ロジック（Domain 観点）。
- `crates/pulsen/src/application/mod.rs`, `application/tick/confirm_spawn.rs`, `application/tick/mod.rs` — 3件。tick の制御フロー（Usecase 観点）。手続きAはアダプター呼び出しの順序（launching記録 → `prepare_attempt` → spawn）の確認のため `tick/launch.rs` のみ確認した。
- `crates/pulsen/src/cli/add.rs`, `cli/render.rs` — 2件。文言層（Usecase / CLI 観点）。
- `crates/pulsen/tests/cli_usage.rs`, `register_task.rs`, `run_wrapper.rs`, `tick_confirm_spawn.rs`, `tick_fixture/mod.rs`, `tick_launch.rs`, `tick_scan.rs` — 7件。ユースケース層のダブルに対するテスト（Usecase / Test 観点）。

## 確認した観点と根拠

- **`cfg` の分布（AC-1）**: `grep -rEn 'cfg\([^)]*\b(unix|windows|target_os|target_family)\b' crates/*/src/` のヒットは `crates/pulsen/src/adapter/process.rs`(8) / `util/atomic.rs`(2) / `adapter/task_repository.rs`(1, `#[cfg(all(test, unix))]`) と `crates/pulsen-conformance/src/lib.rs`(2, main 既存) のみ。`crates/pulsen-domain/` は 0 件。
- **依存と lint**: `Cargo.toml` / `Cargo.lock` に差分なし（新規依存なし）。`unsafe_code = "forbid"` は workspace とドメインの両方で維持され、`crates/*/src/` に `unsafe` の使用はない。
- **同定情報の取得（要件 §4.3）**: プラットフォームごとに private な `identity::observe` 1つに閉じ、戻り値は `Result<Option<ObservedProcess>, Io>` の三値。macOS は `ps` の exit 1・stdout 空を `Ok(None)`（実測で一致）、Linux は `/proc/<pid>/stat` の `NotFound` を `Ok(None)`、取得元の不在は先に `Err(Io)` として分離されている。`own_identity` 側でのみ二値へ畳む。`LC_ALL=C` / `TZ=UTC` の固定と `LANG` / `LC_TIME` の除去はユニットテストで主張されている。`/proc` のフィールド位置（pgrp = 2、starttime = 19、最後の `)` からの相対）も正しい。
- **デタッチ起動と FD**: `process_group(0)`（Windows は `DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP`）＋ stdin/stdout/stderr を `Stdio::null()`。適合ケース TC-002 は `examples/spawn_probe` で呼び出し側プロセスの終了後の完走を観測し、`cli_tick.rs` の「滞留するエージェントを起動したままでも次の tick は競合しない」がロック FD を継承していないことを観測する。`agent_probe wait-for` を使っているため、環境の速さに依存しない。
- **アトミック置換・排他ロック**: `FsRunStore` の write 系はすべて `util::atomic::write_atomic` 経由、ディレクトリ作成は `util::fsdir::ensure_dir` 経由。ロックは既存の `FileExclusiveLock` のまま。個別の再実装はない。
- **read 系の3分類**: `read_json` が `NotFound`（ディレクトリごと不在を含む）→ `Ok(None)`、JSON 不正・値制約の破れ → `Corrupt { path, message }`、その他 → `Io`。`marker_exists` は `exists()` ではなく `try_exists()` で、機構の失敗を「無い」に丸めない。
- **`create` の冪等性の境界**: 同定は `physical_key`（親の `canonicalize` + ファイル名）で git 側の出力にも**対称に**適用。達成済みは「鍵一致 かつ 登録が `ws.branch` を指す かつ 実体がある かつ `prunable` でない」に限る。実体の消えた登録は `add -f`、登録なし・ブランチのみは `-f` なしの `add`（先端を変えない）、通常ディレクトリ・別ブランチ・base 不在はいずれも `Failed`。`prunable` の注記に依存せず `try_exists` でも実体を見るため、注記を出さない git でも達成済みに倒れない。置き場の `ensure_dir` は鍵を作る前に行われる（親が無いと `canonicalize` が失敗するため順序が必要）。適合スイートは worktree_root をシンボリックリンク経由で組み、正規化の分岐を必ず通す。
- **シェル非経由**: git・`ps`・エージェント・ラッパーのいずれも `Command::new(program).arg(..)` で直接起動。TC-021 と `cli_wrapper.rs` が `*` / `$HOME` / `&&` / `>out.txt` / `--model` / 空文字列トークンのリテラル通過を実バイナリ経由で主張する。
- **エラーの扱い**: アダプター・合成ルートに `unwrap()` は無く、`expect` は `CommandLine` の「1トークン以上」不変条件の1箇所のみ（`rehydrate` が 0 トークンを拒否することで担保）。`sync_dir` の失敗のみ意図的に無視（理由がコメントにある）。
- **ポートの1:1（AC-6）**: `RunStore` 9・`ProcessController` 3・`WorktreeManager` に `create` 1つ。宣言だけのメソッド・スタブは無い。
- **ビルドとテスト**: `cargo fmt --check` / `cargo clippy --all-targets --all-features` / `cargo test --workspace` がいずれも緑。適合スイートは `conformance_process_controller` 16件・`conformance_run_store` 22件（21 + 追加1）・`conformance_worktree` 17件（16 + 追加1）が**スキップ0件**で走った（`--nocapture` でスキップ出力なしを確認）。
- **コメント**: 指摘への弁明・修正経緯・TODO / FIXME・仮実装の記述は無い（W-001 の1件を除き、残っているのは why / why not）。
