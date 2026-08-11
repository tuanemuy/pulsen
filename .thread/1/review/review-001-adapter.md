# Adapter / Infrastructure

## 前提と検証方法

- 判定基準は CLAUDE.md（技術方針・ヘキサゴナル）と `.thread/1/plan.md` の受け入れ基準、および PR 内の ADR（019/021〜027/030〜033/036/037/042/050）。
- 手元で `cargo test --workspace` / `cargo clippy --workspace --all-targets -- -D warnings` / `cargo fmt --all --check` を実行し、いずれも通ることを確認した（AC-1 の1つ目）。
- OS 依存分岐の隔離（AC-1 の2つ目）を grep で確認した。`#[cfg(unix)]` / `#[cfg(not(unix))]` の出現は `crates/pulsen/src/util/atomic.rs`（2箇所）と `crates/pulsen/tests/`（テスト側）だけで、`crates/pulsen-domain/` には1つも無い。`serde_yaml_ng` を名指しするのも `crates/pulsen/src/adapter/yaml.rs` だけで、ADR-021 / ADR-023 のとおり閉じている。
- 適合ケースの件数を機械的に数えた。config-store 24 / workflow-store 31 / task-repository 44 / clock 5 / task-id-generator 5 / exclusive-lock 7 / worktree-manager 9 = 125 で、AC-9 / AC-10 / AC-11 / AC-12 の件数と一致する。マクロの列挙件数も同数で、宣言だけで実行されないケースは無い。`HOOKS.md` は 125 行の対応表を持ち AC-8 を満たす。
- アトミック置換・排他ロックの再実装が無いことを確認した（AC-19）。`crates/pulsen/src/` 配下で `fs::write` / `fs::rename` / `create_dir_all` / `NamedTempFile` を使うのは `util/atomic.rs` と `util/fsdir.rs` だけで、他はテストコードの1行のみ。
- `crates/pulsen/src/` に `unwrap()` / `expect(` / `panic!` / `todo!` / `unimplemented!` が1つも無いことを確認した（ADR-036）。ドメイン側は `template.rs` の `unreachable!`(不変条件)が1つだけ。
- B-001 は推測ではなく再現で確認した（下記に手順と結果を記載）。

## Blockers

- **[B-001]** `list_active` / `list_archived` が、走査中にアーカイブされたエントリを `ReadError::Io` に写す。ポートの「読み取りはロックなしで常に一貫した内容を返す」「`archive` の中間状態は読み手から観測されない」契約（`crates/pulsen-domain/src/task/port.rs` の `TaskRepository` doc、TC-port-task-repository-044）を破る。
  - 場所: `crates/pulsen/src/adapter/task_repository.rs:111`（`let bytes = fs::read(&path).map_err(|error| ReadError::Io { .. })?;`）
  - 理由: `list` は `read_dir` の結果を `paths` に全件集めてから1件ずつ `fs::read` する。集めた後・読む前に `archive`（= `rename`）が走ると、そのパスは現役側から消えており `fs::read` が `NotFound` を返す。これが `?` でそのまま `ReadError::Io` になり、**走査全体が失敗する**。ファイルが消えたのは破損でも機構異常でもなく「アーカイブされた」という正常な出来事なので、Io エラーは誤分類である。ADR-039 が `ReadError::Io` を「読み取り経路の失敗は入出力エラーだけ」と絞ったことも、この経路をそこへ写す根拠にはならない。

    実際に再現した。`FsTaskRepository` に 400 件作成し、1スレッドで `list_active()` を回しながら別スレッドで 400 件を順に `archive` すると、`list_active()` が **19 回**失敗した（`cargo test` 用の一時プローブで測定・削除済み）。TC-044 が現状で落ちないのは `archive` を1回しか呼ばないため窓が極端に狭いだけで（200 回連続実行で 0 失敗）、契約違反そのものは存在する。後続スライスで `tick` の `archive` と `ls` の走査が同時に走れば、利用者に見える形（`ls` が I/O エラーで落ちる）で顕在化する。
  - 提案: `list` の `fs::read` で `ErrorKind::NotFound` を「この領域にはもう無い」としてスキップする。`lookup` が `NotFound` を `Ok(None)` に写しているのと同じ扱いで、`find` 側とも一貫する。

    ```rust
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        // 走査中にアーカイブされたエントリは、この領域にもう無いだけで失敗ではない。
        Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
        Err(error) => return Err(ReadError::Io { message: message(&path, &error) }),
    };
    ```

    あわせて TC-044 の観測側を「複数回の archive を並行に回す」形に強めておくと、この回帰を捕まえられる。

## Warnings

- **[W-001]** `FsWorkflowStore` のエラーが「どのファイルの話か」を落とす。
  - 場所: `crates/pulsen/src/adapter/workflow_store.rs:90-97`（`read_error`）、同 `68-75`（`YamlSyntax` / `Parse` への写像）
  - 理由: `WorkflowLoadError::Io { message }` に入るのは `error.to_string()` だけで、`std::io::Error` の `Display` はパスを含まない。`resolved` はその場に在るのに捨てている。結果として利用者が見るのは `エラー: ワークフロー定義を読み込めません。 原因: Permission denied (os error 13)` で、どのファイルか分からない。`WorkflowParseError::*` も同様にパスを持たず、`render::workflow_load_error` は `NotFound` 以外でパスを出せない。ADR-050 は「スキーマ違反は対象ファイルの絶対パスと論理位置で示す」と決めており、そこと食い違う。同じ PR 内で `FsTaskRepository` は `message(&path, &error)` でパスを必ず付けており（`task_repository.rs:218`）、`FsConfigStore` は `render::config_error` が `config_path` を補って救っている（`cli_add_error.rs:117` はその表示をアサートしている）のに対し、workflow だけ穴が空いている。実際 `cli_add_error.rs:189` の TC-021 は `"読み込めません"` しかアサートできていない。
  - 提案: `read_error(&error, resolved)` の `Io` 分岐で `message: format!("{}: {error}", resolved.display())` にする（`Parse` 側も同様に文言へパスを混ぜるか、`WorkflowLoadError` にパスを持たせるかは spec と相談）。CLI テストのアサートにもパスを足す。

- **[W-002]** `rename_atomic` が移動先の親ディレクトリしか fsync しない。
  - 場所: `crates/pulsen/src/util/atomic.rs:39-47`
  - 理由: `tasks/x.json` → `archive/x.json` の移動は**2つのディレクトリエントリ**を変える。移動先だけ fsync すると、クラッシュ時に「archive には現れているが tasks からの削除が永続化されていない」= 同一 ID が両方に在る状態が残りうる。`find` が現役優先で解決し TC-018 が両在を許容するので致命傷ではないが、`write_atomic` が丁寧に temp→fsync→rename→fsync dir を守っているのに対して片手落ちである。`ensure_dir` で新規に作ったディレクトリの親を fsync しない点も同じ系統（ファイルは永続化されたのにディレクトリが無い、が起こりうる）。
  - 提案: `rename_atomic` で `from.parent()` と `to.parent()` が異なるときは両方 `sync_dir` する。コストは移動時の1回だけで、`archive` は頻度の低い操作。

- **[W-003]** `validate_repo` の存在判定が I/O エラーを握りつぶして `NotFound` にする。
  - 場所: `crates/pulsen/src/adapter/worktree.rs:51-53`（`if !repo.as_path().exists()`）
  - 理由: `Path::exists()` は `try_exists()` と違い、権限不足などの I/O エラーを `false` に丸める。親ディレクトリが読めないリポジトリを指すと「指定したリポジトリのパスが存在しません」と案内され、利用者は実在するパスを疑うことになる。ADR-024 は分類を「終了ステータスと起動の可否だけで決める」としているが、この分岐は git を起動する前の判定であり、エラーの握りつぶしは意図されていないはず。
  - 提案: `repo.as_path().try_exists()` を使い、`Ok(false)` は `NotFound`、`Err(e)` は `TargetError::Failed { message }` にする。`Failed` は「対象の分類が付かない実行環境のエラー」なので分類は5種のまま増えない。

- **[W-004]** `write_atomic` が置換のたびに対象ファイルの権限を作り直す。
  - 場所: `crates/pulsen/src/util/atomic.rs:26-29`
  - 理由: `NamedTempFile::new_in` は Unix で mode 0600 固定のファイルを作り、`persist` はそれをそのまま対象パスに移す。したがって `save` するたびに既存タスクファイルの mode / ACL は 0600 に置き換わり、利用者が付けた権限（共有ホームでのグループ読み等）や umask の意図は失われる。値としては 0600 のほうが安全側なので即座の実害は無いが、「アトミック置換」の唯一の実装が権限も暗黙に書き換える点は文書化されていない。
  - 提案: 意図（タスクファイルは所有者限定）なら doc コメントに明記する。既存ファイルの権限を保つ意図なら、対象が既存のときだけ `fs::metadata(path).permissions()` を temp に転写してから `persist` する。

- **[W-005]** `create` の一意性判定が TOCTOU で、書き込み自体は無条件の上書きになる。
  - 場所: `crates/pulsen/src/adapter/task_repository.rs:127-142`
  - 理由: `exists(Active) || exists(Archived)` で確認した後に `write_atomic` が無条件に置換する。ポートの契約は「`create` はID衝突を**ポートが**担保し、呼び出し側の事前確認に依存しない」と書いており、排他ロックを取らない呼び出し側が2つ居れば、後勝ちで既存タスクファイルを丸ごと潰す。今は `add` が必ずロックを取るので観測できないが、`create` 単体の契約としては担保できていない。
  - 提案: 少なくとも doc コメントに「衝突判定は排他ロック下での呼び出しを前提とする」と書く。より強く担保するなら `write_atomic` に「対象が存在しなければ作る」版（`persist_noclobber` 相当）を1つ足し、`create` だけがそれを使う。

- **[W-006]** `create` の存在確認で失敗したときだけパスがメッセージから落ちる。
  - 場所: `crates/pulsen/src/adapter/task_repository.rs:128-133`（`let taken = |error: io::Error| CreateError::Io { message: error.to_string() }`）
  - 理由: 同じ関数の直下（`139-141`）では `message(&path, &error)` でパスを付けているのに、存在確認の失敗経路だけ `error.to_string()` になっている。`try_exists` が失敗する状況（親ディレクトリの権限等）はまさにパスが知りたい場面で、W-001 と同じ「`?` / `map_err` で文脈が落ちる」パターン。
  - 提案: `taken` を `|path: &Path| move |error| CreateError::Io { message: message(path, &error) }` 相当にして、現役・アーカイブそれぞれのパスを載せる。

- **[W-007]** git に渡す環境変数の除去リストに `GIT_CEILING_DIRECTORIES` 系が無い。
  - 場所: `crates/pulsen/src/adapter/worktree.rs:12`（`INHERITED_GIT_ENV`）
  - 理由: ADR-024 の文面（`GIT_DIR` / `GIT_WORK_TREE` / `GIT_INDEX_FILE`）には忠実だが、除去の目的「呼び出し元の環境で `-C` の対象が上書きされるのを防ぐ」は `GIT_CEILING_DIRECTORIES` でも破れる。これが設定されていると `rev-parse --show-toplevel` が上位探索を打ち切り、正当なリポジトリが `NotARepository` に落ちる。`GIT_COMMON_DIR` / `GIT_OBJECT_DIRECTORY` / `GIT_ALTERNATE_OBJECT_DIRECTORIES` も同種。`cron` から `add` を無人実行する運用（requirements 想定）では、継承された環境の内容を利用者が意識していない可能性が高い。
  - 提案: 実装だけ直すと ADR と食い違うので、ADR-024 の共通項に上記を追記したうえで `INHERITED_GIT_ENV` を拡張する。逆に「ユーザー環境は尊重する」判断なら、なぜ3つだけなのかを ADR に明記する。

- **[W-008]** 文字列でない YAML キーが、自由形式のキー位置で無言で名前になる。
  - 場所: `crates/pulsen/src/adapter/yaml.rs:235-244`（`key_text`）
  - 理由: 固定スキーマの位置（トップレベル・`agents.<name>` の中身・ステータスの中身）では `unknown_key` が拾うので問題ない（テストもある）。しかし `agents:` 直下と `statuses:` 直下は**キー自体が自由形式**なので、`agents: { true: {...} }` はエージェント名 `"true"` に、複合キー（`[a, b]: {...}`）は `"(複合キー)"` という名前になり、そのまま受理される。複数の複合キーがあれば同じ `"(複合キー)"` に潰れて後勝ちで消える。ADR-042 がタグを構文エラーにした理由「スキーマに無い記法を黙って剥がすと ADR-013 の『typo を無言で捨てない』が破れる」は、非文字列キーにも同じく当てはまる。
  - 提案: `convert` のマッピング走査で、キーが `Yaml::Text` 以外なら `YamlSyntaxError`（またはスキーマ層の `InvalidValue`）にする。タグの拒否と同じ場所で1行増えるだけで済む。

## 個別に確認して問題なしと判断した点

観点として明示されていた項目のうち、指摘に至らなかったものを記録する。

- **`Corrupt` / `SnapshotUnreadable` の分類**（`adapter/task_file.rs`）は ADR-025 の表どおり。JSON 全体不正・タスク側フィールドの型/値/未知キー（`deny_unknown_fields`）は `Err` = `Corrupt`、`snapshot` 不在・解釈不能・`WorkflowDefinition::new` の構造不変条件破れ・`task_status ∉ statuses` はすべて `SnapshotUnreadable`。
- **不変条件2〜4をデコードで検証していない**ことを確認した。`Task::rehydrate`（`crates/pulsen-domain/src/task/task.rs:103-124`）は不変条件1しか見ず、`Running` なのに `current_attempt` が無いといった状態は `Intact` で返る。過剰検証は無い。
- **`save_degraded` の温存**は `carried_snapshot` が既存ファイルから `Box<RawValue>` を取り出し、`skip_serializing_if = "Option::is_none"` で「キーの不在を不在のまま」書き戻す。タスク側フィールドの妥当性を問わずに引き継ぐ点も正しい（修復材料を落とさない）。
- **`write_atomic` の順序**（同一ディレクトリの temp → `write_all` → `sync_all` → `persist` → dir の fsync）と、失敗時に残骸を残さないこと（`PersistError` が `NamedTempFile` を保持したまま drop される）は正しい。一時ファイル名 `.tmpXXXXXX` が `TaskFilePath::parse_file_name` の「先頭は英数字」で確実に走査から外れる点も、レイアウト側と辻褄が合っている。
- **`FsWorkflowStore`** は `.yml` フォールバック無し、`base_dir` 注入で `current_dir()` を読まない（読むのは合成ルート `cli::wire` の1箇所）、`resolved_from` は `std::path::absolute` で絶対パス、キャッシュ無し。ADR-030 / AC-10 のとおり。
- **`FsConfigStore`** は未知キー拒否（`TOP_LEVEL_KEYS` / `AGENT_KEYS`）、二層検証（構造だけを見てテンプレート内容には触らない）、キャッシュ無し、空ドキュメント `Ok`。エラー位置は構文エラーのみ行列を持ち、スキーマ違反は論理パス（`agents.claude.cmd:` 前置）で ADR-050 に合致。
- **`GitCliWorktreeManager`** の分類は ADR-024 の判定表と1対1（`symbolic-ref` × `rev-parse --verify --quiet HEAD` の4象限、`show-ref` の exit 0/1/その他）。空リポジトリと detached HEAD が区別できることを適合テスト（TC-006 / TC-005）が実行時に確認している。`git_program` 注入で `Failed` を作る設計により、本番アダプターはイミュータブルなまま。
- **`FileExclusiveLock`** は `WouldBlock` → `Ok(None)`、`TryLockError::Error` → `Err(Failed)` で機構異常と区別。`FileLockGuard` が `File` を所有し drop で解放。`LockError::Failed` の再現をロックパスにディレクトリを置く方式（ADR-032）にしたのは Windows でも root でも成立するので妥当。プロセス強制終了後の解放は TC-005 が実プロセスで確認している。
- **`SystemClock` / `DefaultTaskIdGenerator`** に `unwrap` / `expect` / `panic!` は無い。飽和は `Timestamp::saturating_from_unix_secs`（ドメインの総関数）に委ね、`getrandom` は構築時1回のみ、`generate` の `parse` 失敗は構築時に検証済みの値へ畳む。base36 8桁の根拠は ADR-026 にある。
- **依存の閉じ込め**: `serde_yaml_ng` は `adapter/yaml.rs` のみ、`tempfile` は `util/atomic.rs`（+テスト）、`getrandom` は `adapter/task_id.rs` のみ。`pulsen-conformance` は `[dev-dependencies]` で、本番バイナリに載らない（ADR-019 / ADR-023）。
- **スコープ逸脱なし**: `WorktreeManager` は `create` / `remove` を宣言しておらず、RunStore / ProcessController / CommandRunner も存在しない。未実装メソッドの宣言・スタブは1つも無い（AC-7 / plan.md「含まれないもの」）。
- **`flake.nix`** の変更は devShell への `git` 追加1行のみで、ADR-024 の実行時依存と整合。

## カバレッジ

一覧 121 件に対し、確認 72 件 / スキップ 49 件。

### 確認

- `.adr/021-yaml-value-then-hand-written-schema-walk.md`, `.adr/022-std-file-lock-and-lockguard-marker-trait.md`, `.adr/023-dependency-selection.md`, `.adr/024-git-cli-shell-out-and-target-classification.md`, `.adr/025-task-file-json-and-corrupt-classification.md`, `.adr/026-task-id-format.md`, `.adr/027-port-conformance-suite-and-harness-hooks.md`, `.adr/030-workflow-store-base-dir-injection.md`, `.adr/031-pulsen-home-layout-in-application-layer.md`, `.adr/032-lock-holder-example-fixture.md`, `.adr/033-git-fixture-reproducibility.md`, `.adr/036-infallible-ports-absorb-failure-at-construction.md`, `.adr/037-platform-separator-set-without-cfg.md`, `.adr/039-read-error-shared-by-find-and-list.md`, `.adr/042-absent-yaml-value-is-omission.md`, `.adr/044-task-file-layout-in-domain.md`, `.adr/050-schema-error-location-is-logical.md`
- `.thread/1/plan.md`
- `Cargo.toml`
- `crates/pulsen-conformance/Cargo.toml`, `crates/pulsen-conformance/HOOKS.md`, `crates/pulsen-conformance/src/lib.rs`, `crates/pulsen-conformance/src/clock.rs`, `crates/pulsen-conformance/src/config_store.rs`, `crates/pulsen-conformance/src/exclusive_lock.rs`, `crates/pulsen-conformance/src/task_id_generator.rs`, `crates/pulsen-conformance/src/task_repository.rs`, `crates/pulsen-conformance/src/workflow_store.rs`, `crates/pulsen-conformance/src/worktree_manager.rs`
- `crates/pulsen-domain/Cargo.toml`, `crates/pulsen-domain/src/execution/port.rs`, `crates/pulsen-domain/src/task/path.rs`, `crates/pulsen-domain/src/task/port.rs`, `crates/pulsen-domain/src/task/task.rs`
- `crates/pulsen/Cargo.toml`, `crates/pulsen/examples/lock_holder.rs`
- `crates/pulsen/src/adapter/mod.rs`, `crates/pulsen/src/adapter/clock.rs`, `crates/pulsen/src/adapter/config_store.rs`, `crates/pulsen/src/adapter/lock.rs`, `crates/pulsen/src/adapter/task_file.rs`, `crates/pulsen/src/adapter/task_id.rs`, `crates/pulsen/src/adapter/task_repository.rs`, `crates/pulsen/src/adapter/workflow_store.rs`, `crates/pulsen/src/adapter/worktree.rs`, `crates/pulsen/src/adapter/yaml.rs`
- `crates/pulsen/src/application/mod.rs`, `crates/pulsen/src/application/home.rs`, `crates/pulsen/src/application/register_task.rs`
- `crates/pulsen/src/cli/mod.rs`, `crates/pulsen/src/cli/add.rs`, `crates/pulsen/src/cli/args.rs`, `crates/pulsen/src/cli/exit.rs`, `crates/pulsen/src/cli/render.rs`, `crates/pulsen/src/cli/wire.rs`
- `crates/pulsen/src/lib.rs`, `crates/pulsen/src/main.rs`
- `crates/pulsen/src/util/mod.rs`, `crates/pulsen/src/util/atomic.rs`, `crates/pulsen/src/util/fsdir.rs`
- `crates/pulsen/tests/common/mod.rs`, `crates/pulsen/tests/common/git.rs`, `crates/pulsen/tests/common/lock.rs`
- `crates/pulsen/tests/cli_add_error.rs`
- `crates/pulsen/tests/conformance_config_store.rs`, `crates/pulsen/tests/conformance_lock.rs`, `crates/pulsen/tests/conformance_task_repository.rs`, `crates/pulsen/tests/conformance_time_id.rs`, `crates/pulsen/tests/conformance_workflow_store.rs`, `crates/pulsen/tests/conformance_worktree.rs`
- `flake.nix`, `rustfmt.toml`

### スキップ

- `.adr/019-domain-crate-workspace.md`, `.adr/020-no-serde-in-domain-timestamp-conversion-in-domain.md`, `.adr/028-usecase-error-paths-via-test-doubles.md`, `.adr/029-wildcard-enum-match-arm-lint-domain-only.md`, `.adr/034-workflow-ref-separator-set-as-constant.md`, `.adr/035-file-slice-adrs-from-019.md`, `.adr/040-rehydrate-takes-field-bundle.md`, `.adr/048-parse-inputs-at-spec-flow-position.md` — 019 は本文を読んで前提（ドメイン zero-dep とクレート境界）として使ったが決定対象はクレート構成でドメイン観点の担当。残りはドメインモデリング・テストダブル・lint 方針の決定で、アダプター実装の是非を左右しない（8件）
- `.thread/1/adr.md`, `.thread/1/progress.md`, `.thread/1/steps.md`, `.thread/1/testing.md` — 進行管理・手順のプロセス文書で、実装の設計判断は `.adr/` 側が正本（4件）
- `Cargo.lock` — 生成物。依存の選定自体は `Cargo.toml` と ADR-023 で確認済み（1件）
- `crates/pulsen-conformance/src/doubles/` 配下 8件（`clock.rs` / `lock.rs` / `mod.rs` / `stores.rs` / `task_id.rs` / `task_repository.rs` / `tests.rs` / `worktree.rs`）— ユースケース層のテストダブル（ADR-028）で、実アダプターの契約適合とは別系統。ユースケース/テスト観点の担当
- `crates/pulsen-domain/src/definition/` 配下 13件、`crates/pulsen-domain/src/execution/mod.rs`、`crates/pulsen-domain/src/lib.rs`、`crates/pulsen-domain/src/task/` のうち `attempt.rs` / `branch.rs` / `counters.rs` / `degraded.rs` / `failure.rs` / `id.rs` / `mod.rs` / `process.rs` / `state.rs` / `time.rs` の10件 — 値オブジェクトと遷移ロジックでドメイン観点の担当。アダプターから使う `parse` / `rehydrate` のシグネチャは呼び出し側で整合を確認済み（25件）
- `crates/pulsen/tests/cli_add_boundary.rs`, `crates/pulsen/tests/cli_add_normal.rs`, `crates/pulsen/tests/register_task.rs` — CLI 受理系の受け入れテストとユースケーステストで、アダプターの契約ではなく体験・分岐網羅の検証（3件）
