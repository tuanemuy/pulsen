# レビュー3周目 — Adapter / Infrastructure / Test

対象: PR #8 / ベース `main` / 変更ファイル 149件（`.thread/1/review/changed-files-003.txt`）

2周目の判定台帳（`.thread/1/review/triage.md` ラウンド1・2）を先に読み、`wont-fix` 3件（W-004 `unreachable!` / W-012 tick 案内 / W-017 `create` の TOCTOU）は蒸し返していない。本ラウンドは「実装に残る本当の欠陥」と「2周目の修正が生んだ回帰」だけを見た。

## 実行結果

| 確認 | 結果 |
|---|---|
| `cargo test` | 全通過（456件 / 0 failed）。3回連続実行して差分なし・フレーキーなし |
| `cargo clippy --all-targets -- -D warnings` | 警告0 |
| `cargo fmt --check` | 差分なし |
| `cargo test -- --nocapture \| grep -i skip` | 実スイートのスキップは `tc_port_clock_005`（`rewind` 不提供）の1件のみ。他は `lib.rs` の `SkipBudget` 自体のユニットテスト由来 |

## 受け入れ基準の検証（本観点に関わる分）

| AC | 判定 | 根拠 |
|---|---|---|
| AC-1（`#[cfg(unix)]` の隔離） | 合格 | `grep -rn 'cfg(unix)\|cfg(windows)' crates/*/src/` のヒットは `pulsen-conformance/src/lib.rs` 2件（権限 probe）と `pulsen/src/util/atomic.rs` 2件（`sync_dir`）のみ。`pulsen-domain` は0件。`testing.md:44-48` の手順と実測が一致（R2-W-008 の修正が効いている） |
| AC-8〜AC-12（適合125件） | 合格 | ケース関数の定義数とマクロ列挙数が全ポートで一致し、合計 125（clock 5 / config-store 24 / exclusive-lock 7 / task-id-generator 5 / task-repository 44 / workflow-store 31 / worktree-manager 9）。`spec/testcases/ports/` の行数とも一致。`HOOKS.md` の区分集計 A 28 / B 85 / C 12 = 125 も実測と一致 |
| register-task 67件 | 合格 | `spec/testcases/task/register-task.md` のデータ行は 67（表4本 × 見出し・区切りを除いた実測）。テスト関数の TC 番号は `tc_task_register_task_001..067` が漏れなく存在（normal 12 / error 31 / boundary 19 / usecase 5 = 67）。欠番なし |
| AC-15（拒否側の不変） | 合格 | `cli_add_error.rs` の31件は共通ヘルパ `reject` / `reject_target` 経由を含めて全件が `has_no_task()` + `untouched.assert_unchanged()` を通す。`cli_add_boundary.rs` の TC-053 / 054 / 055 / 058 も同様（1周目 W-021 の修正が入っている） |
| AC-19（アトミック性・排他の集約） | 合格 | アトミック置換は `util/atomic.rs` の `write_atomic` / `rename_atomic` のみ。アダプター側に `fs::rename` / `NamedTempFile` の直接使用なし。排他は `adapter/lock.rs` の `FileExclusiveLock` 1箇所 |
| `Corrupt` / `SnapshotUnreadable` / `save_degraded` | 合格 | `task_file.rs` の分類は JSON として有効かどうかで分かれ、`carried_snapshot` が既存の `snapshot` を `RawValue` のまま引き継ぐ。不変条件2〜4はデコードで検証していない |
| `unwrap` / `expect` / `panic!` | 合格 | 本番経路に0件（`unwrap_or` / `unwrap_or_default` のみ）。`expect` / `panic!` は `#[cfg(test)]` とテストダブル（台本の使い切りは前提違反として意図的にパニック）に閉じている |

## 2周目の変更の確認

- **`SkipBudget` の集合 + probe 化**: `permission_restrictions_effective()` は `chmod 000` したファイルを読めるかを `OnceLock` で1度だけ調べる。`deny_read`（file）・`deny_dir_read`（dir 0o000）・`deny_dir_write`（dir 0o555）はいずれも「制限が効いたことを確認してから `Some`」なので、root / 非 POSIX / 権限を持たないファイルシステムでは probe とフックが同時に落ち、宣言と実態がずれない。宣言は task-repository 6件・config-store 1件・workflow-store 1件で、`HOOKS.md` の区分 C のうち権限系8件と一致。`allows()` の接頭辞照合は `tc_port_clock_005` が `tc_port_clock_0051` に一致しないことをテストで固定済み。偽陽性・偽陰性は見つからなかった。
- **TC-042 / TC-044 の観測**: `yield_until_observations_exceed` が書き込みの**前**に置かれ、`before` / `after` の差で「重なった観測」を数える形になっている。TC-042 は `SAVE_ROUNDS` 30 を下限、`SAVE_ROUND_LIMIT` 1000 を上限にした二重条件で、観測の下限 `CONCURRENT_OBSERVATIONS` 5 を満たすまで回す。5秒期限は「読み手が落ちたときに待ち続けない」ための保険で、期限切れは戻り値が `floor` のままになって最終アサーションに現れる。3回連続実行で 1.16 秒前後・失敗0。フレーキーは観測されなかった（ただし W-002）。
- **ロック TC-003**: `try_acquire` を子スレッドに移し、親が `NON_BLOCKING`（5秒）まで `returned` を監視、期限超過なら**先に保持を解放してから** `join` する。待つ実装でもハングしない形になっている。`Harness::Lock: Sync` の要求はこの1ケースだけに閉じている。
- **`list` の `symlink_metadata` 分岐**: 「消えた」= `continue`、「残っているが読めない」= `TaskEntry::Corrupt` に分かれ、宙ぶらりんの symlink を置く unix 限定のユニットテストが分岐を実際に通す。
- **`yaml.rs` の非文字列キー**: キーの値・種別・論理位置がメッセージに載り、スカラー4種・複合キー2種・入れ子の論理位置をテストが固定。重複キーのテストは英語メッセージ依存をやめて位置の等値比較（`line: 3, column: 5`）になっている。
- **`tests/common/mod.rs` の `detached_home()`**: `Add::run` / `run_cli` の**両方**が `HOME` / `USERPROFILE` を毎回作る一時ディレクトリへ向け、`PULSEN_HOME` は `env_remove` する。`user_home()` はその後に上書きされる順序で、実ホームへ落ちる経路が残っていない。`pulsen` バイナリを起動する箇所は `CARGO_BIN_EXE_pulsen` の grep で `Add::run` と `run_cli` の2箇所だけであることを確認した。サンドボックスの `TempDir` は `command.output()` の直後に drop され、リークしない。
- **`listed_entries`**: `is_file()` をやめ、名前で `state` だけを除く形になり、新規ディレクトリを作ってその中に書く実装も検出できる。
- **`cli_add_error.rs` の無言スキップ4件**: `println!` + `return` が全廃され、TC-016 / 017 / 021 / 036 が `common::skipped(case, fixture)` を通る。宣言は `permission_restrictions_effective()` / `lock::holder_program()` / `git::tmpdir_outside_repository()` の3つの述語から実行時に決まる。
- **`task_id.rs` / `task/path.rs` のトートロジー**: `TaskId::parse(id) == Ok(id)` は `compose` を1000回直接検証する形（`generate` の `verified` フォールバックを迂回する）に置き換わり、`path.rs` の重複テストは削除されている。

## Blockers

なし。

## Warnings

- **[W-001]** 同じ環境前提（TMPDIR が git リポジトリの外にあるか）が、適合スイートでは失敗、受け入れテストでは許容スキップと、2つの扱いに割れている
  - 場所: `crates/pulsen/tests/conformance_worktree.rs:108-111`（`Vec::new()`）／ 対比対象は `crates/pulsen/tests/common/mod.rs:34-35,49-51`（`OUTSIDE_REPOSITORY_CASES` を `git::tmpdir_outside_repository()` で宣言）
  - 理由: `TC-port-worktree-manager-003` の前提は `non_repo_dir()` → `common::git::is_outside_repository(&dir).then_some(())?` で、TMPDIR 自体がリポジトリ配下にある環境では `None` を返してスキップになる。ところが同ファイルの宣言は `Vec::new()` なので、そのスキップは `SkipBudget::record` の `assert!` でケースの失敗として現れる。一方、まったく同じ前提を使う CLI 側の `TC-036` は `git::tmpdir_outside_repository()` で許容集合に入れており、スキップで済む。plan.md の記帳基準（:58「環境が前提を作れずケースが走らなかった行」はスキップして理由を残す）と ADR-055 の原則（宣言はプラットフォームではなく**環境の能力**に対応させる）に対して、この1ケースだけが例外になっている。`conformance_worktree.rs` は既に `mod common;` を持ち `common::git` を使っているので、述語はその場にある。TMPDIR をワークスペース配下に置く CI・開発環境では `cargo test` が「スキップを許容するのは `[]`」という、実装の不備を指すように読めるメッセージで落ちる。
  - 提案: `allowed_skips()` を置き、`if common::git::tmpdir_outside_repository() { Vec::new() } else { vec!["tc_port_worktree_manager_003"] }` にする（`conformance_task_repository.rs:222-228` と同じ形）。意図的に失敗させ続けるなら、なぜ CLI 側の TC-036 と扱いを変えるのかを why として残し、plan.md の記帳基準にも例外として書く。

- **[W-002]** TC-042 / TC-044 は、対象の `save` / `archive` が `Err` を返す実装に対して**失敗ではなくハング**する
  - 場所: `crates/pulsen-conformance/src/task_repository.rs:735-736`（`repo.save(&large).expect("保存できる")`）、`:855`（`repo.archive(task.id()).expect("アーカイブできる")`）。同型が `crates/pulsen/src/util/atomic.rs:157-160` にもある
  - 理由: 読み手スレッドの停止条件は `while writing.load(..)` / `while moving.load(..)` だけで、その `store(false)` は `thread::scope` のクロージャ本体の**末尾**にある。クロージャが `expect` でパニックすると `store(false)` に到達しないまま巻き戻り、`thread::scope` は巻き戻しの前に子スレッドを合流させるため、読み手が無限ループしてプロセスが返らない。`save` / `archive` が失敗するのはまさにアダプターの欠陥・ディスク満杯といった、スイートが検出すべき状況であり、そこで判定が出ずに固まる。AC-8 が約束する「後続スライスの in-memory 実装に同じスイートを適用する」場面では、`RefCell` 由来の失敗などがそのまま CI のタイムアウトになる。ロック TC-003 が2周目に期限監視へ作り替えられたのと同じ失敗モードが、この3箇所に残っている。
  - 提案: `writing` / `moving` の停止を巻き戻しに載せる（`struct Stop<'a>(&'a AtomicBool)` の `Drop` で `store(false)` する、あるいは `scope.spawn` に渡す読み手ループへ `Instant` の期限を足す）。最小の手当ては、書き込みループを `expect` ではなく `Result` として受けて、`writing.store(false)` の後にアサーションする形にすること。

## カバレッジ

一覧149件との対応（行番号は `changed-files-003.txt` の行）。

### 確認（51件）

- 適合スイート（16件）: `crates/pulsen-conformance/src/clock.rs`, `crates/pulsen-conformance/src/config_store.rs`, `crates/pulsen-conformance/src/doubles/clock.rs`, `crates/pulsen-conformance/src/doubles/lock.rs`, `crates/pulsen-conformance/src/doubles/mod.rs`, `crates/pulsen-conformance/src/doubles/stores.rs`, `crates/pulsen-conformance/src/doubles/task_id.rs`, `crates/pulsen-conformance/src/doubles/task_repository.rs`, `crates/pulsen-conformance/src/doubles/tests.rs`, `crates/pulsen-conformance/src/doubles/worktree.rs`, `crates/pulsen-conformance/src/exclusive_lock.rs`, `crates/pulsen-conformance/src/lib.rs`, `crates/pulsen-conformance/src/task_id_generator.rs`, `crates/pulsen-conformance/src/task_repository.rs`, `crates/pulsen-conformance/src/workflow_store.rs`, `crates/pulsen-conformance/src/worktree_manager.rs`
- 適合スイートの出荷ドキュメント（1件）: `crates/pulsen-conformance/HOOKS.md`
- アダプター（10件）: `crates/pulsen/src/adapter/clock.rs`, `crates/pulsen/src/adapter/config_store.rs`, `crates/pulsen/src/adapter/lock.rs`, `crates/pulsen/src/adapter/mod.rs`, `crates/pulsen/src/adapter/task_file.rs`, `crates/pulsen/src/adapter/task_id.rs`, `crates/pulsen/src/adapter/task_repository.rs`, `crates/pulsen/src/adapter/workflow_store.rs`, `crates/pulsen/src/adapter/worktree.rs`, `crates/pulsen/src/adapter/yaml.rs`
- 共通ユーティリティ（3件）: `crates/pulsen/src/util/atomic.rs`, `crates/pulsen/src/util/fsdir.rs`, `crates/pulsen/src/util/mod.rs`
- テスト（14件）: `crates/pulsen/tests/cli_add_boundary.rs`, `crates/pulsen/tests/cli_add_error.rs`, `crates/pulsen/tests/cli_add_normal.rs`, `crates/pulsen/tests/cli_usage.rs`, `crates/pulsen/tests/common/git.rs`, `crates/pulsen/tests/common/lock.rs`, `crates/pulsen/tests/common/mod.rs`, `crates/pulsen/tests/conformance_config_store.rs`, `crates/pulsen/tests/conformance_lock.rs`, `crates/pulsen/tests/conformance_task_repository.rs`, `crates/pulsen/tests/conformance_time_id.rs`, `crates/pulsen/tests/conformance_workflow_store.rs`, `crates/pulsen/tests/conformance_worktree.rs`, `crates/pulsen/tests/register_task.rs`
- テスト用フィクスチャ（1件）: `crates/pulsen/examples/lock_holder.rs`
- ビルド・依存・環境（6件）: `Cargo.toml`, `crates/pulsen/Cargo.toml`, `crates/pulsen-conformance/Cargo.toml`, `crates/pulsen-domain/Cargo.toml`（`[dependencies]` が無いこと＝AC-1 の機械的保証を確認）, `flake.nix`（devShell に `git` が入っていることを確認）, `rustfmt.toml`

### スキップ（98件）

- `.adr/019-*.md` 〜 `.adr/062-*.md`（38件） — ADR は arch-spec 観点の担当。本レビューは実装との整合が問題になった箇所（ADR-024 の `INHERITED_GIT_ENV` / ADR-027 の権限フック規則 / ADR-055 の `SkipBudget` / ADR-060 のロック TC-003 / ADR-062 のホーム分離）だけを参照した
- `.thread/1/adr.md`, `.thread/1/progress.md`, `.thread/1/steps.md`, `.thread/1/review/changed-files-001.txt`, `.thread/1/review/changed-files-002.txt`, `.thread/1/review/review-001-adapter.md`, `.thread/1/review/review-001-arch-spec.md`, `.thread/1/review/review-001-domain.md`, `.thread/1/review/review-001-test.md`, `.thread/1/review/review-001-usecase-cli.md`, `.thread/1/review/review-001.md`, `.thread/1/review/review-002-adapter.md`, `.thread/1/review/review-002-arch-spec.md`, `.thread/1/review/review-002-domain.md`, `.thread/1/review/review-002-test.md`, `.thread/1/review/review-002-usecase-cli.md`（16件） — 作業ログ・過去のレビュー成果物。arch-spec 観点の担当
- `.thread/1/plan.md`, `.thread/1/review/triage.md`, `.thread/1/testing.md`（3件） — 本レビューの入力として読んだ（AC の検証・既決着判定の確認・AC-1 の grep 手順の照合）。指摘対象としてはレビューしていない
- `Cargo.lock`（1件） — 生成物
- `crates/pulsen-domain/src/**`（29件: `definition/` 13件, `execution/` 2件, `lib.rs`, `task/` 13件） — ドメイン層。domain 観点の担当。ポート表との1:1（AC-7）はアダプター実装がトレイトを満たすことでのみ確認した
- `crates/pulsen/src/application/home.rs`, `crates/pulsen/src/application/mod.rs`, `crates/pulsen/src/application/register_task.rs`, `crates/pulsen/src/cli/add.rs`, `crates/pulsen/src/cli/args.rs`, `crates/pulsen/src/cli/exit.rs`, `crates/pulsen/src/cli/mod.rs`, `crates/pulsen/src/cli/render.rs`, `crates/pulsen/src/cli/wire.rs`, `crates/pulsen/src/lib.rs`, `crates/pulsen/src/main.rs`（11件） — ユースケース層・CLI 層。usecase-cli 観点の担当。`render.rs` は `ConfigLoadError::Io` にファイルパスが載ることの確認（ADR-050 / ADR-054 の整合）でのみ参照した

## 総括

**Blocker は0件**。2周目の修正はいずれも意図どおり効いており、回帰は見つからなかった（`SkipBudget` の probe は root / 非 POSIX で正しく偽になり、TC-042 / 044 の観測は書き手と重なり、ロック TC-003 はハングしない）。残る2件はどちらも「別の何かが壊れたときの見え方」の問題で、正常系の判定を変えるものではない。
