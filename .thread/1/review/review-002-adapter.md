# Adapter / Infrastructure

## 前提と検証方法

- 2周目。`.thread/1/review/triage.md` の `wont-fix`(W-004 / W-012 / W-017)は蒸し返さない。`create` の TOCTOU は spec/domains/task.md:295「ポートは並行書き込みを調停しない」により指摘しない。
- 手元で `cargo fmt --all --check` / `cargo clippy --workspace --all-targets -- -D warnings` / `cargo test --workspace` を実行し、いずれも通ることを確認した(AC-1)。
- AC-1 の OS 依存分岐の隔離を grep で再確認。`cfg(unix)` / `cfg(not(unix))` / `cfg!(windows)` は `crates/pulsen/src/util/atomic.rs`(2箇所)と `crates/pulsen/tests/`(5ファイル)だけ。`crates/pulsen-domain/` と `crates/pulsen-conformance/` には1つも無い。
- AC-19 を再確認。`crates/pulsen/src/` で `fs::rename` / `create_dir_all` / `NamedTempFile` / `fs::write` を使うのは `util/atomic.rs` と `util/fsdir.rs` だけ、`try_lock` は `adapter/lock.rs` だけ。個別再実装は無い。
- 本番コードの `unwrap()` / `expect(` / `panic!` / `todo!` / `unimplemented!` を全走査。ヒットは `pulsen-domain/src/definition/template.rs:198` の `unreachable!`(不変条件・wont-fix 済み)のみ。
- 1周目の修正点(B-001 / W-013〜W-016 / W-018 / W-019 / W-020 / W-035)は `git show cff5506` の差分で確認し、git 環境変数の追加分は手元の git 2.51.2 で実測した(下記)。

## Blockers

なし

## Warnings

- **[W-001]** `list` の `NotFound` スキップが「本当に消えた」と「エントリはあるが読めない」を取り違え、そのうえ回帰テストが1つも無い。
  - 場所: `crates/pulsen/src/adapter/task_repository.rs:111-122`、契約の記述は `crates/pulsen-domain/src/task/port.rs:106-129`
  - 理由: B-001 の修正方向(走査中の `archive` を失敗にしない)は spec/domains/task.md:294 のとおりで正しい。ただし判定材料が `io::ErrorKind::NotFound` だけなので、**ディレクトリエントリは残っているのに読めない**ケースまで同じ枝に落ちる。典型は宙ぶらりんのシンボリックリンク(`<task-id>.json` という名前でリンク先が無い)で、`read_dir` は列挙し、`fs::read` は `NotFound` を返し、結果としてそのエントリは走査結果から**黙って消える**。spec/pages/index.md:43 は読めないタスクファイルについて「ファイルパスと読めない旨を一覧に含めて報告する」と定めており(`TaskEntry::Corrupt` の存在理由)、`ls` スライスで利用者が修復の入口を失う。「アーカイブされて消えた」と「壊れたエントリが残っている」は `symlink_metadata` で区別できる(前者は `Err(NotFound)`、後者は `Ok`)。
    加えて、この分岐を通す観測が**どこにも無い**。`crates/pulsen/src/adapter/task_repository.rs` にユニットテストのモジュールは1つも無く(`cfg(test)` が0件)、適合ケース TC-044 は `archive` を1回しか呼ばないため窓が極端に狭い(1周目レビューの実測: 200回連続で0失敗)。1周目が併せて提案した「TC-044 を複数回の archive で強める」も入っていない。今の実装は、`continue` を消しても `? `に戻しても全テストが緑のままである。
  - 提案: `NotFound` を受けたら `path.symlink_metadata()` を1回見て、`Err(NotFound)` なら `continue`(走査中に消えた)、`Ok(_)` なら `TaskEntry::Corrupt { path, message }` として報告する。あわせて、宙ぶらりんのリンクを置いて `list_active()` が `Ok` を返しつつそのエントリを落とさないことを確かめる決定的なユニットテスト(unix 限定)を `adapter/task_repository.rs` に足す。走査が消えたエントリを飛ばすという**ポートの契約レベルの決定**は、アダプターのコードコメントだけでなく `task/port.rs` の契約 doc にも書く(W-019 で「実装だけ直すと ADR と食い違う」を守ったのと同じ理由)。

- **[W-002]** 適合スイートの許容スキップ件数が `cfg(unix)` で 0 固定のため、root 実行では「スキップされるはずの権限ケース」が失敗になる。
  - 場所: `crates/pulsen/tests/conformance_task_repository.rs:213-216`、`crates/pulsen/tests/conformance_config_store.rs:67-70`、`crates/pulsen/tests/conformance_workflow_store.rs:88-92`、機構は `crates/pulsen-conformance/src/lib.rs:174-205`
  - 理由: W-030 の修正でスキップ超過をケースの失敗にしたのは妥当だが、条件が「unix かどうか」になっている。`deny_read` / `deny_dir_read` / `deny_dir_write` は ADR-027 の規則どおり「制限が実際に効いたか」を確かめて効かなければ `None` を返すので、**root で走らせた unix でもスキップになる**。そこで宣言 0 と食い違い、TC-port-task-repository-005/011/012/019/035/041・TC-port-config-store-023・TC-port-workflow-store-030 の計8件が赤くなる。これは plan.md テスト方針「権限操作でしか再現できない TC-016・TC-021 は POSIX のみで実行し、**root では skip する**」および ADR-027 が避けようとした「スキップに落ちずに FAIL する」と正面から食い違う。同じ状況で `crates/pulsen/tests/cli_add_error.rs:107-110・181-184` は `println!` を出して素通りしており、同一 PR 内で root への態度が2つに割れている。docker の既定ユーザーで CI を回すだけで再現する。
  - 提案: 宣言をプラットフォームではなく**能力**で決める。テストファイル側で一時ファイルに `chmod 000` して読めるかを1回試し、効かない環境なら該当件数を許容件数に加える(効く環境では 0 のまま)。root で緑にしたくないなら、その方針を plan.md / ADR-027 側に反映して「root では実行しない」を明文化する。どちらでもよいが、現状は方針が2箇所で矛盾している。

- **[W-003]** 非文字列キーの拒否が「どのキーか」も位置も示さず、固定スキーマ位置では1周目より診断性が落ちている。
  - 場所: `crates/pulsen/src/adapter/yaml.rs:239-252`
  - 理由: W-020 の修正方向(黙って名前に読み替えない)は正しい。ただし返すのは `キーは文字列である必要があります(実際は数値)` で `location: None`。したがって利用者が見るのは `エラー: ワークフロー定義が不正です。 / YAML 構文エラー: /home/u/.pulsen/workflows/implement.yaml: キーは文字列である必要があります(実際は数値)` だけで、**どのキーか・何行目かがどこにも出ない**。修正前は固定スキーマ位置(`agents.claude` の中身など)なら `unknown_key` が `agents.claude にスキーマ外のキー \`1\` があります。` とキー名も論理位置も出していたので、その経路は退化している。加えて `AgentName` / `StatusName` は非空・前後空白なしだけが制約(`crates/pulsen-domain/src/definition/name.rs:32-40`)なので `1` や `true` は**ドメイン上は正当な名前**であり、`"1":` とクォートすれば通る。その道筋が案内されないまま「構文エラー」と言われる。ADR-050 が「壊れている箇所をキー単位で特定できる」ことを狙いとして掲げているのと合わない。
  - 提案: `key_text` は手元に値を持っているので、キーの表現とクォートで直せる旨をメッセージに入れる(例: `キー \`1\` が文字列ではありません(数値)。名前として使うなら "1" のように引用してください`)。`convert` にキーの経路を渡せるなら論理位置も添える。

- **[W-004]** テストフィクスチャが取り除く `GIT_*` が3つのままで、本番アダプターの7つと食い違う。
  - 場所: `crates/pulsen/tests/common/git.rs:25-37`(`env_remove` は `GIT_DIR` / `GIT_WORK_TREE` / `GIT_INDEX_FILE` の3つ)対 `crates/pulsen/src/adapter/worktree.rs:14-22`(7つ)
  - 理由: W-019 で本番側に `GIT_CEILING_DIRECTORIES` 他を足したが、フィクスチャ側は据え置きになっている。とくに `common::git::is_outside_repository`(:72-76)は「TMPDIR がリポジトリ配下でないこと」を**フィクスチャ側の git 環境**で判定するのに対し、実際にその前提を使う `GitCliWorktreeManager` は ceiling を外して探索する。手元で実測(git 2.51.2): `GIT_CEILING_DIRECTORIES=<repo> git -C <repo>/sub rev-parse --show-toplevel` は exit 128、未設定なら exit 0。したがって「TMPDIR がリポジトリ配下 かつ 呼び出し元に `GIT_CEILING_DIRECTORIES` が設定されている」環境では、フィクスチャは「非リポジトリの前提が成立した」と判断して `Some` を返し、アダプターは親リポジトリを見つけて `Ok` を返すため、TC-port-worktree-manager-003 が**スキップではなく失敗**になる(しかも ADR-033 が用意した説明が効かない失敗になる)。
  - 提案: 除去対象の集合を1箇所(定数)に置き、本番アダプターとフィクスチャの双方が同じ集合を使う。集合を共有できないなら、`common/git.rs` の `env_remove` を `INHERITED_GIT_ENV` と同じ7つに揃え、「前提判定はアダプターと同じ環境で行う」ことを why として残す。

- **[W-005]** パスをメッセージに載せる規約が、同ラウンドで直した範囲の外に2箇所残っている。
  - 場所: `crates/pulsen/src/adapter/worktree.rs:66-70`(`リポジトリのパスを確認できない: {error}`)、`crates/pulsen/src/adapter/task_repository.rs:229-231`(`archive` の `Io` が移動先だけを載せる)
  - 理由: W-013 / W-018 は「`std::io::Error` の Display はパスを含まないので、その場にあるパスを必ず載せる」を規約として確立し、`task_repository.rs` / `workflow_store.rs` はそれに揃った。しかし W-015 の修正で新設した `try_exists` の `Err` 枝は `repo.as_path()` を手元に持ちながら載せておらず、利用者が見るのは `エラー: 対象の検証に失敗しました。 / 原因: リポジトリのパスを確認できない: Permission denied (os error 13)` だけになる。この枝が出るのはまさに「どのパスの話か」が要る状況(親ディレクトリの権限)である。`archive` は `rename_atomic(&from, &to)` の失敗をすべて `to` に帰属させるため、移動元起因の失敗(消えた・別ファイルシステム)でも無関係な移動先パスを指す。
  - 提案: `worktree.rs` は `format!("{}: リポジトリのパスを確認できない: {error}", repo.as_path().display())` 相当にする。`archive` は `from` と `to` の両方をメッセージに載せる。

- **[W-006]** 観測回数の下限が「書き込みと重なった観測」を保証していない。
  - 場所: `crates/pulsen-conformance/src/task_repository.rs:728-737`(TC-042)、`:837-845`(TC-044)、`yield_until_observed` は `:788-799`
  - 理由: W-022 の主張は「観測が起きて初めて『中間状態を観測しなかった』に意味がある」だが、`yield_until_observed` は**書き込みループの後**に置かれている。読み手が最初の1周を回すのが遅れた場合、書き手は 30 回の `save`(TC-044 では `archive`)を終えてから最初の観測を待つので、成立するのは「一度は観測した」だけで「書き込みと重なって観測した」ではない。`observations > 0` も、`while` の1周を数えているだけで並行性は示さない。修正前より良いが、指摘が求めた保証にはなっていない。
  - 提案: `yield_until_observed` を**書き込みを始める前**に1回置き、読み手が回り始めたことを確かめてから書き込む。さらに書き込み前後の観測回数の差を取り、`after - before > 0` を確かめると「重なった観測」が言える。

- **[W-007]** `TaskLookup::Corrupt` / `SnapshotUnreadable` の理由に Rust の Debug 表現がそのまま入る。
  - 場所: `crates/pulsen/src/adapter/task_file.rs`(`format!("{error:?}")` が 25 箇所。例 `:304-315`、`:575-577`、`:586`)
  - 理由: W-035 は「エラー値をそのまま埋めると Rust の Debug 表現が利用者に出る」を理由に `worktree.rs` の1箇所を直し、コードに why まで残した。同じパターンが同一 PR の `task_file.rs` に 25 箇所残っている。これらの文字列は診断用の内部値ではなく、spec/pages/index.md:43 が「ファイルパスと読めない旨を一覧に含めて報告する」と定める `ls` の表示に載る値である(`TaskEntry::Corrupt { message }` / `DegradedTask::snapshot_error`)。結果として `task_id: InvalidChar { found: 'U', position: 0 }` のような表記が利用者に出る。しかも同ラウンドで `NameError::describe()` / `DurationError::describe()` / `CommandError::describe()` をドメインに置いたので(W-002)、人間可読な言い換えの置き場は既にある。
  - 提案: 少なくとも `task_file.rs` が触る値オブジェクトのエラーには `describe()` を用意して使う。`ls` スライスまで先送りするなら、その旨と対象箇所を進行メモに残して回収漏れを防ぐ。

- **[W-008]** ADR-050 と ADR-054 が「スキーマ違反にファイルパスを載せるか」で食い違ったまま残っている。
  - 場所: `.adr/050-schema-error-location-is-logical.md`(決定: 「スキーマ違反: **対象ファイルの絶対パス**と論理位置」)、`.adr/054-workflow-error-file-path-goes-into-free-form-messages.md`(決定: パスは自由形式メッセージだけ。`UnknownKey` / `InvalidValue` には載せない)
  - 理由: 実装は ADR-054 のとおりで、`--workflow` の解釈で解決先が決まるワークフロー定義の `UnknownKey` / `InvalidValue` にはパスが出ない(`crates/pulsen/src/cli/render.rs:170-176・212-214`)。つまり ADR-050 の決定文は、正本でありながら実装と食い違う記述を残している。ADR-054 は「トレードオフ」として影響欄に書いているが、ADR-050 側には何の追記も無いため、後続スライスが ADR-050 だけを読むと逆の規約を実装する。W-027 / W-028 が「正本の索引を機械的に成立させる」ことを求めた直後としては、同じ穴が別の形で残っている。
  - 提案: ADR-050 の決定に「ワークフロー定義については ADR-054 で範囲を限定した」旨を1行足す(ADR-038 の起票様式に沿った追記でよい)。

## 個別に確認して問題なしと判断した点

- **B-001 の修正が別の失敗を生んでいないこと**: `list` の `continue` は `read_dir` の `NotFound`(`:85`)・`lookup` の `NotFound`(`:61`)と同じ扱いで、`Vec::with_capacity` のずれ以外に副作用は無い。TC-044 の読み手は `list_active` → `list_archived` の順で見るため、rename と重なっても「どちらにも現れない」窓は生じない。
- **`rename_atomic` の両親 fsync**(`util/atomic.rs:43-57`): `source != dir` の比較は字句比較なので、同一ディレクトリを別表記で渡しても最悪 fsync が1回増えるだけで、落とすことはない。`archive` は低頻度の操作でコストも問題にならない。
- **`try_exists()` への置き換えが分類を保つこと**: `FsTaskRepository::exists`(`:53-55`)・`save`(`:163`)・`archive`(`:222`)・`validate_repo`(`:63-71`)のいずれも `Ok(false)` を不在、`Err` を `Io` / `Failed` に写しており、`NotFound` を握り潰す経路は残っていない。
- **git 環境変数の追加が除去しすぎでないこと**: 追加した4変数はすべてリポジトリスコープの解決を上書きするもので、本アダプターが使う3コマンドの結果を「正しい方向へ」戻すだけであることを実測で確認した。`GIT_OBJECT_DIRECTORY` / `GIT_ALTERNATE_OBJECT_DIRECTORIES` を外しても `rev-parse --verify --quiet HEAD` は ref の解決だけで exit 0 を返すため、`(true,false)` = `EmptyRepository` への誤分類は起きない(オブジェクトを退避したリポジトリで確認)。`GIT_NAMESPACE` は `show-ref --verify` の結果を変えないため除去対象に加えなくてよい。`safe.directory` 等のユーザー設定を尊重する方針も維持されている。
- **パス付きメッセージの二重化が無いこと**: `workflow_store::at` が前置するのは `Io` / `YamlSyntax` の自由形式メッセージだけで、`render::workflow_load_error` はそこにパスを重ねていない(`NotFound` だけが `attempted` を出す)。`config_store` は `Io` / `Invalid` にパスを持たせず `render::config_error` が `config_path` を1回だけ出す形で、こちらも重複しない。
- **`Corrupt` / `SnapshotUnreadable` の分類**(`adapter/task_file.rs:1-17・246-281`)は ADR-025 の表と一致。JSON 全体不正・タスク側フィールドの型/値/未知キー(`deny_unknown_fields`)は `Err` = `Corrupt`、`snapshot` 不在・解釈不能・`WorkflowDefinition::new` の構造不変条件破れ・`task_status ∉ statuses` は `SnapshotUnreadable`。不変条件2〜4はデコードで一切見ていない。
- **`save_degraded` の温存**: `carried_snapshot` が `Box<RawValue>` を素通しし、`skip_serializing_if = "Option::is_none"` でキーの不在を不在のまま書き戻す。タスク側フィールドの妥当性を問わない点も正しい(修復材料を落とさない)。
- **`FsConfigStore` / `FsWorkflowStore`**: 未知キー拒否(`TOP_LEVEL_KEYS` / `AGENT_KEYS` / `STATUS_KEYS`)、重複キーはパーサ由来の構文エラー、二層検証(テンプレート内容に触れない)、`.yml` フォールバック無し、`base_dir` 注入で `current_dir()` を読まない(読むのは `cli::wire::compose` の1箇所)、`resolved_from` は `std::path::absolute`、キャッシュ無し。ADR-030 / ADR-043 / AC-9 / AC-10 のとおり。
- **`GitCliWorktreeManager`** の分類は ADR-024 の判定表と1対1(`symbolic-ref` × `rev-parse --verify --quiet HEAD` の4象限、`show-ref` の 0 / 1 / その他 / シグナル)。`git_program` 注入により本番インスタンスはイミュータブル。
- **`FileExclusiveLock`**: `WouldBlock` → `Ok(None)`、`TryLockError::Error` → `Err(Failed)`、`FileLockGuard` が `File` を所有し drop で解放。置き場の自動作成は `util::fsdir::ensure_dir` 経由。ADR-022 / ADR-032 のとおりで、`lock_holder` example もロック取得の失敗を握り潰さない。
- **`SystemClock` / `DefaultTaskIdGenerator`**: `unix_secs` は epoch 前後と飽和を総関数で処理、`getrandom` は構築時1回、`generate` は無謬の契約を守るため検証済み ID へ畳む。ADR-026 / ADR-036 のとおり。
- **依存の閉じ込め**: `serde_yaml_ng` は `adapter/yaml.rs` のみ、`tempfile` は `util/atomic.rs`(+テスト)、`getrandom` は `adapter/task_id.rs` のみ、`serde` / `serde_json` は `adapter/task_file.rs` のみ。`pulsen-conformance` は `[dependencies]` が `pulsen-domain` だけで、本番バイナリには載らない。
- **スコープ逸脱なし**: `WorktreeManager` に `create` / `remove` の宣言は無く、RunStore / ProcessController / CommandRunner も存在しない。未実装メソッドのスタブは1つも無い(AC-7)。
- **`flake.nix`** の変更は devShell への `git` 追加1行のみで ADR-024 と整合。`rustfmt.toml` は `edition = "2024"` の1行。

## カバレッジ

一覧 139 件に対し、確認 71 件 / スキップ 68 件。

### 確認

- `.adr/021-yaml-value-then-hand-written-schema-walk.md`, `.adr/022-std-file-lock-and-lockguard-marker-trait.md`, `.adr/023-dependency-selection.md`, `.adr/024-git-cli-shell-out-and-target-classification.md`, `.adr/025-task-file-json-and-corrupt-classification.md`, `.adr/026-task-id-format.md`, `.adr/027-port-conformance-suite-and-harness-hooks.md`, `.adr/030-workflow-store-base-dir-injection.md`, `.adr/031-pulsen-home-layout-in-application-layer.md`, `.adr/032-lock-holder-example-fixture.md`, `.adr/033-git-fixture-reproducibility.md`, `.adr/036-infallible-ports-absorb-failure-at-construction.md`, `.adr/037-platform-separator-set-without-cfg.md`, `.adr/039-read-error-shared-by-find-and-list.md`, `.adr/042-absent-yaml-value-is-omission.md`, `.adr/043-store-adapters-receive-injected-paths.md`, `.adr/044-task-file-layout-in-domain.md`, `.adr/045-task-file-dto-generic-over-snapshot.md`, `.adr/046-no-skippable-hooks-for-post-operation-observation.md`, `.adr/050-schema-error-location-is-logical.md`, `.adr/053-conformance-yaml-source-hooks.md`, `.adr/054-workflow-error-file-path-goes-into-free-form-messages.md`(22件)
- `.thread/1/plan.md`, `.thread/1/review/triage.md`, `.thread/1/review/review-001-adapter.md`(3件)
- `Cargo.toml`, `crates/pulsen/Cargo.toml`, `crates/pulsen-conformance/Cargo.toml`(3件)
- `crates/pulsen-conformance/HOOKS.md`, `crates/pulsen-conformance/src/lib.rs`, `crates/pulsen-conformance/src/task_repository.rs`(3件)
- `crates/pulsen-conformance/src/clock.rs`, `crates/pulsen-conformance/src/config_store.rs`, `crates/pulsen-conformance/src/exclusive_lock.rs`, `crates/pulsen-conformance/src/task_id_generator.rs`, `crates/pulsen-conformance/src/workflow_store.rs`, `crates/pulsen-conformance/src/worktree_manager.rs` — 今ラウンドの変更点(マクロのスキップ宣言引数)とハーネス側インタフェースを確認(6件)
- `crates/pulsen-domain/src/task/port.rs`, `crates/pulsen-domain/src/task/path.rs`, `crates/pulsen-domain/src/definition/name.rs` — アダプターが依存する契約・レイアウト・名前制約として確認(3件)
- `crates/pulsen/examples/lock_holder.rs`(1件)
- `crates/pulsen/src/adapter/mod.rs`, `clock.rs`, `config_store.rs`, `lock.rs`, `task_file.rs`, `task_id.rs`, `task_repository.rs`, `workflow_store.rs`, `worktree.rs`, `yaml.rs`(10件)
- `crates/pulsen/src/application/home.rs`, `crates/pulsen/src/application/mod.rs`(2件)
- `crates/pulsen/src/cli/render.rs`, `crates/pulsen/src/cli/wire.rs`(2件)
- `crates/pulsen/src/lib.rs`, `crates/pulsen/src/util/mod.rs`, `crates/pulsen/src/util/atomic.rs`, `crates/pulsen/src/util/fsdir.rs`(4件)
- `crates/pulsen/tests/common/mod.rs`, `crates/pulsen/tests/common/git.rs`, `crates/pulsen/tests/common/lock.rs`(3件)
- `crates/pulsen/tests/conformance_config_store.rs`, `conformance_lock.rs`, `conformance_task_repository.rs`, `conformance_time_id.rs`, `conformance_workflow_store.rs`, `conformance_worktree.rs`(6件)
- `crates/pulsen/tests/cli_add_error.rs` — TC-016 / TC-021 / TC-022 の権限操作とパス表示の裏取りとして確認(1件)
- `flake.nix`, `rustfmt.toml`(2件)

### スキップ

- `.adr/019-domain-crate-workspace.md`, `.adr/020-no-serde-in-domain-timestamp-conversion-in-domain.md`, `.adr/028-usecase-error-paths-via-test-doubles.md`, `.adr/029-wildcard-enum-match-arm-lint-domain-only.md`, `.adr/034-workflow-ref-separator-set-as-constant.md`, `.adr/035-file-slice-adrs-from-019.md`, `.adr/038-adr-filing-format.md`, `.adr/040-rehydrate-takes-field-bundle.md`, `.adr/048-parse-inputs-at-spec-flow-position.md`, `.adr/049-base-flag-allows-hyphen-values.md`, `.adr/051-undisplayable-name-fixture-is-whitespace-stem.md`, `.adr/052-acceptance-test-harness-in-tests-common.md` — クレート構成・ドメインモデリング・lint 方針・CLI 引数解釈・ADR 起票様式・受け入れテスト基盤の決定で、アダプター実装の是非を左右しない(domain / usecase-cli / arch-spec 観点の担当)(12件)
- `.thread/1/adr.md`, `.thread/1/progress.md`, `.thread/1/steps.md`, `.thread/1/testing.md`, `.thread/1/review/changed-files-001.txt`, `.thread/1/review/review-001.md`, `.thread/1/review/review-001-arch-spec.md`, `.thread/1/review/review-001-domain.md`, `.thread/1/review/review-001-test.md`, `.thread/1/review/review-001-usecase-cli.md` — 進行管理・他観点のレビュー記録で、設計判断の正本は `.adr/` 側(10件)
- `Cargo.lock` — 生成物。依存選定は `Cargo.toml` と ADR-023 で確認済み(1件)
- `crates/pulsen-conformance/src/doubles/clock.rs`, `doubles/lock.rs`, `doubles/mod.rs`, `doubles/stores.rs`, `doubles/task_id.rs`, `doubles/task_repository.rs`, `doubles/tests.rs`, `doubles/worktree.rs` — ユースケース層のテストダブル(ADR-028)で、実アダプターの契約適合とは別系統(usecase / test 観点の担当)(8件)
- `crates/pulsen-domain/Cargo.toml`, `crates/pulsen-domain/src/lib.rs`, `definition/agent.rs`, `definition/assembler.rs`, `definition/command.rs`, `definition/config.rs`, `definition/duration.rs`, `definition/mod.rs`, `definition/port.rs`, `definition/reference.rs`, `definition/snapshot.rs`, `definition/template.rs`, `definition/validator.rs`, `definition/workflow.rs`, `execution/mod.rs`, `execution/port.rs`, `task/attempt.rs`, `task/branch.rs`, `task/counters.rs`, `task/degraded.rs`, `task/failure.rs`, `task/id.rs`, `task/mod.rs`, `task/process.rs`, `task/state.rs`, `task/task.rs`, `task/time.rs` — 値オブジェクトと遷移ロジックで domain 観点の担当。アダプターから呼ぶ `parse` / `rehydrate` のシグネチャは呼び出し側で整合を確認済み(27件)
- `crates/pulsen/src/application/register_task.rs`, `crates/pulsen/src/cli/add.rs`, `crates/pulsen/src/cli/args.rs`, `crates/pulsen/src/cli/exit.rs`, `crates/pulsen/src/cli/mod.rs`, `crates/pulsen/src/main.rs` — ユースケースの手順と CLI の引数・終了コードで usecase-cli 観点の担当(6件)
- `crates/pulsen/tests/cli_add_boundary.rs`, `crates/pulsen/tests/cli_add_normal.rs`, `crates/pulsen/tests/cli_usage.rs`, `crates/pulsen/tests/register_task.rs` — 受理系・使い方・ユースケーステストで、アダプターの契約ではなく体験と分岐網羅の検証(test 観点の担当)(4件)

なお「確認」に挙げた `crates/pulsen-conformance/src/{clock,config_store,exclusive_lock,task_id_generator,workflow_store,worktree_manager}.rs` は今ラウンドの変更点と接続部までで、ケース本体と spec 125行との1:1対応の検証は test 観点の担当。
