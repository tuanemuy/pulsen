# レビュー: PR #8 — Test 観点

## 検証の前提と実測

判定は `CLAUDE.md`（テスト方針）と `.thread/1/plan.md`（AC-6 / 13 / 16 / 17 / 18 と「テスト方針」節）を基準にした。spec のテストケースは表の行を実際に数えて実装と突き合わせている。

| spec | 行数（実測） | 実装 | `#[test]` 数（実測） |
|---|---|---|---|
| `spec/testcases/ports/config-store.md` | 24 | `pulsen-conformance/src/config_store.rs` | 24 |
| `spec/testcases/ports/workflow-store.md` | 31 | `workflow_store.rs` | 31 |
| `spec/testcases/ports/task-repository.md` | 44 | `task_repository.rs` | 44 |
| `spec/testcases/ports/clock.md` | 5 | `clock.rs` | 5 |
| `spec/testcases/ports/task-id-generator.md` | 5 | `task_id_generator.rs` | 5 |
| `spec/testcases/ports/exclusive-lock.md` | 7 | `exclusive_lock.rs` | 7 |
| `spec/testcases/ports/worktree-manager.md`（本スライス該当） | 9 / 21 | `worktree_manager.rs` | 9 |
| `spec/testcases/task/register-task.md` | 67 | `cli_add_{normal,error,boundary}.rs` + `register_task.rs` | 62 + 5 |

`cargo test --workspace` は全件成功（domain 163 / conformance 10 / pulsen unit 38 / cli 62 / conformance suites 125 / usecase 20）。TC ID とケース関数の対応も1行ずつ照合し、**中身が spec の期待とずれている行・期待が緩められている行は見つからなかった**。特に確認した点:

- **スキップが FAIL を隠していないか**: `require!` の使用箇所を全件追った結果、フックはすべて**操作の前**（前提条件の組み立て・期待値の取得）にあり、**操作後の観測に `require!` を使っている箇所は1つも無い**。TC-004 の `record_bytes` / TC-009 の `snapshot_bytes` は事後観測を `assert_eq!(..., Some(before))` で書いており、フックが `None` を返したらスキップではなく失敗する。「材料が消えた」ケースが緑になる経路は塞がっている。
- **実際にスキップされた行**: `cargo test -- --nocapture` で確認した結果、この環境（非 root / macOS）でスキップされたのは **`TC-port-clock-005`（`rewind` 未提供）の1件だけ**。「再現できるアダプター環境に限る」の C 区分11行のうち10行（config-store-023・workflow-store-030・task-repository-005/011/012/019/035/041・exclusive-lock-007・worktree-manager-009）は実際に実行されている。CLI 側の TC-016 / 021 / 036 の条件付きスキップも発火していない。**125行中124行が実走**しており、AC-8 / AC-12 の要求水準を満たす。
- **権限フックの効き目確認**: `deny_dir_read` は `fs::read_dir` の成否、`deny_dir_write` は実際の書き込みプローブ、`deny_read` は `fs::read` の成否で制限が効いたことを確かめてから `Some` を返す（ADR-027 の要求どおり）。root で `chmod` が効かない環境ではスキップに落ちる。

## Test

### Blockers

なし

### Warnings

- **[W-001]** 境界値の拒否ケース（TC-053 / 054 / 055 / 058）で「利用者のリソースが変更されない」が検証されていない
  - 場所: `crates/pulsen/tests/cli_add_boundary.rs:57`（`reject_base`）、`:118`（TC-053）、`:182`（TC-058）
  - 理由: AC-15 は「異常系 TC-014〜048 **と境界値の拒否ケース TC-053・054・055・058**で、タスクが作られず、**かつワークフロー定義ファイル・config.yaml が変更されない**」を要求している。`cli_add_error.rs` は `reject` / `reject_target` / 各テストが漏れなく `Untouched::assert_unchanged()` を呼んでいるのに対し、境界値側は `has_no_task()` しか見ていない。同じ AC の同じ否定的主張が、片方の集合だけ検証されていない非対称がある。
  - 提案: `reject_base` と TC-053 / TC-058 に `let untouched = home.untouched();` … `untouched.assert_unchanged();` を足す（`cli_add_error.rs` の `reject_target` と同じ形）。

- **[W-002]** `PULSEN_HOME` 単独でのホーム解決が自動テストで一度も実行されない
  - 場所: `crates/pulsen/tests/common/mod.rs:303`（`command.env_remove(HOME_ENV)`）、`crates/pulsen/tests/cli_add_boundary.rs:363`（TC-067）、`crates/pulsen/src/cli/wire.rs`（`resolve_home`）
  - 理由: `Add::run` は既定で `PULSEN_HOME` を除去し、`home_env` を与えるのは TC-067 の1ケースだけ。そのケースは `--home` も同時に渡すため、**`resolve_home` が `PULSEN_HOME` を完全に無視して既定の `~/.pulsen/` へ落ちる実装でも緑のまま通る**。AC-13 が要求する3段優先順位のうち中段が、自動テストでは検出できない。`.thread/1/testing.md` の手動確認2（手順2）が唯一の裏付けになっている。
  - 提案: `--home` を渡さず `home_env` だけを与えて、環境変数側のホームに `state/tasks/` が作られる（フラグ側のホームは空のまま）ことを見る CLI ケースを1件足す。`Add` は既に `home_env` 単独を組み立てられるので追加コストは小さい。既定の `~/.pulsen/` は自動テストで触れるべきでないため手動確認のままでよい。

- **[W-003]** 原子性の2ケース（TC-042 / TC-044）は読み取りが0回でも成功する
  - 場所: `crates/pulsen-conformance/src/task_repository.rs:687`（TC-042）、`:752`（TC-044）
  - 理由: 読み取りスレッドは `while writing.load(Ordering::Relaxed)` で回るだけで、**何回観測したかを数えていない**。書き手が先に走り切ってフラグを落とした場合、読み手のループが0周でも `CaseOutcome::Ran` を返し「中間状態を観測しなかった」ことになってしまう。spec の主張（「すべての読み取りが、いずれかの完全な保存内容のみを観測する」）は観測が実際に起きて初めて意味を持つ。実運用ではまず0周にはならないが、テストが「失敗しうる」ことをスケジューラに委ねている。
  - 提案: `AtomicUsize` で観測回数を数え、スコープを抜けた後に `assert!(observations > 0, ...)`（できれば `save` 回数に見合う下限）を置く。後続スライスの in-memory 実装でも同じ保証が要る。

- **[W-004]** テスト名が「全件まとめて」を主張しているが、検証しているのは1件だけ
  - 場所: `crates/pulsen/tests/register_task.rs:589`（`登録時検証のエラーは全件まとめて返り登録は行われない`）
  - 理由: 台本は `UnknownAgent` 1件しか起こさない定義を与えており、`Err(vec![...])` の要素は1つ。全件収集の実質的な検証は `validator.rs` のユニットテスト（`検証エラーは最初の1件で打ち切らず全件返る` / `同一ステータスの複数の不足はまとめて返る`）と CLI の TC-046（3件）が担っているので**カバレッジの穴ではない**が、CLAUDE.md「テストは振る舞いを表す」に照らすと名前と検証内容が食い違っている。読み手はこのテストが AC-5 の裏付けだと誤解する。
  - 提案: 名前を「登録時検証のエラーはそのまま返り登録は行われない」に改めるか、複数エラーになる定義（例: `MissingAgent` + `UnknownAgent`）を与えて名前に合わせる。

- **[W-005]** `Untouched` は「控えた時点のパス集合」しか見ないため、拒否時に新しいファイルが増えても検出しない
  - 場所: `crates/pulsen/tests/common/mod.rs:178`〜`:206`
  - 理由: `Untouched::of` はパスごとの内容を控えて比較するだけなので、拒否経路で `workflows/` やホーム直下に新規ファイルが作られても `assert_unchanged` は通る。PAGE-common-006 規則2「読めないリソースには書き込まない」の否定的主張としては、既存ファイルの改変だけを見ていることになる。
  - 提案: `resources()` が返すパスの**集合そのもの**も控えて比較する（`workflows/` のエントリ一覧の差分を見る）。`has_no_task()` が `state/tasks/` 側を見ているのと同じ粒度に揃う。

- **[W-006]** `common::deny_read` はディレクトリに渡すと「制限が効いた」と誤判定する
  - 場所: `crates/pulsen/tests/common/mod.rs:388`〜`:406`
  - 理由: 効き目の確認が `fs::read(path).is_ok()` なので、**ディレクトリに対しては root であっても常に `Err`（EISDIR）**になり、`chmod 000` が実際には無効でも `Some(Restore)` を返す。現在の呼び出し元は `config.yaml` とワークフロー定義**ファイル**だけなので実害は無く、`conformance_task_repository.rs` は正しく `deny_dir_read` / `deny_dir_write` を別に持っている。ただし `common` に `pub` で置かれているため、後続スライスがディレクトリに使うと ADR-027 が警告する「スキップに落ちずに FAIL する」状況が再発する。
  - 提案: doc コメントに「ファイル専用」と明記したうえで `path.is_file()` を前提として確認するか、`conformance_task_repository.rs` のディレクトリ版と同じモジュールに寄せて対で扱う。

- **[W-007]** TC-022 の重複キー分岐だけ位置を検証していない
  - 場所: `crates/pulsen/tests/cli_add_error.rs:197`〜`:207`
  - 理由: spec は「位置・原因を表示して非0で終了する（`YamlSyntax`）」。構文エラーの分岐は `["YAML 構文エラー", "位置:", "行"]` を見ているのに、重複キーの分岐は `["YAML 構文エラー"]` だけで、位置が落ちても通る。ポート適合の TC-port-workflow-store-017 が両方について `location.is_some()` を見ているため穴は塞がっているが、CLI 層の期待だけ緩い。
  - 提案: 重複キー側も `"位置:"` を期待に加える。

## 良かった点（今後のスライスでも維持したいもの）

- **フックが「意味」だけを受け取る設計が守られている**: `TaskRepositoryHarness` は `corrupt_whole_record` / `break_task_field` / `corrupt_snapshot` / `drop_snapshot_field` / `set_task_status_outside_snapshot` / `break_snapshot_invariant` と、**破損の種類ごとに別のフック**を持ち、生 JSON をスイートに持ち込んでいない。ケース関数は永続化技術に一切触れておらず、後続スライスの in-memory 実装がフックを実装するだけで同じ125行を通せる形になっている（AC-8）。
- **`Sync` 境界の隔離が効いている**: `concurrent_repo` にだけ `dyn TaskRepository + Sync` を閉じ込め、`lib.rs` の自前テスト（`ToyRepo<RefCell<u32>>` / `ToyRepo<()>`）で「`RefCell` ベースの実装でも残り42ケースが適用でき、並行ケースだけがスキップされる」ことを**枠組みのテストとして**証明している。設計意図がテストで守られている珍しく良い例。
- **エラーの種類を必ず見ている**: `expect_invalid` / `expect_not_found` / `expect_parse_error` / `assert_failed` はいずれも `match` で全分岐を書き、期待と違う分岐は「なぜそれではないか」を添えて `panic!` する。「エラーになること」だけを見て種類を見ない緩いアサーションは1つも無かった。`assert_reports` の期待語も、`render.rs` の文言と突き合わせた結果 `InitialNotFound` / `NextNotFound` などの近接するエラー種を取り違えたら落ちるだけの識別力がある。
- **異常系の否定的主張**: `cli_add_error.rs` は全31ケースで「非0終了 + 案内の内容 + タスクが作られない + 利用者のリソースが変更されない」の4点を揃えて確認している（W-001 の境界値側を除く）。ユースケース側も `assert_eq!(doubles.created(), vec![])` で「`create` が呼ばれていない」ことを直接見ている。
- **環境非依存な再現手段の選択**: `LockError::Failed` をロックパスにディレクトリを置いて作る（`unusable_lock`）、`TargetError::Failed` を存在しない git 実行ファイルで構築した別 manager で作る（`failing_manager`）という選択により、権限操作にも root 実行の可否にも依存せずに C 区分を実走させている。結果として125行中124行が実際に走った。
- **フィクスチャの再現性**: `common/git.rs` が `GIT_CONFIG_GLOBAL` / `GIT_CONFIG_SYSTEM` を null device に固定し、`GIT_DIR` / `GIT_WORK_TREE` / `GIT_INDEX_FILE` を除去し、`init -b main` で既定ブランチを固定し、コミットのたびに author を明示している。さらに **TMPDIR 自体がリポジトリ配下にある場合**を `is_outside_repository` で事前に検出してフィクスチャを諦める（TC-036 / `non_repo_dir`）。ADR-033 が実装として守られている。
- **並列実行での干渉**: すべてのホーム・リポジトリが `tempfile::tempdir()` 単位で分離され、`Add::run` は既定で `PULSEN_HOME` を除去する。相対パス解決はカレントディレクトリを書き換えず、`FsWorkflowStore` への基準ディレクトリ注入（ADR-030）と子プロセスの `current_dir` で表現しており、**プロセス全体の状態を触るテストが1つも無い**。
- **フレーキー要素の抑制**: 時刻依存はユースケース・ドメインとも `FixedClock` / 固定 `Timestamp` に寄せてあり、実時間待ちは Clock 適合の TC-004（1.1秒）1箇所だけ。乱数依存は `ScriptedTaskIdGenerator` で排除されている。
- **テストダブルは最小構成**: 6種すべてが「結果列を順に返す」「呼び出しを記録する」以外を持たず、扱わない操作は値を返さず `panic!` にしている（誤った前提のまま緑になるのを防ぐ）。`ScriptedTaskRepository` が `create` 以外の6メソッドをすべてパニックにしているのは、汎用 in-memory 実装をここに作らないという ADR-028 の線引きが守られている証拠。
- コードにもテストにも修正の経緯・弁明のコメントは残っていない（`TODO` / `FIXME` / 「以前は」「指摘」等の grep で0件）。テスト名はすべて仕様の言葉（`空文字列のプロンプトは受理されない` 等）で書かれている。

## 観察（指摘ではないもの）

- **相対パス解決の適合ケースと spec の文言**: `spec/testcases/ports/workflow-store.md` の TC-005 は「プロセスのカレントディレクトリから解決」と書いているが、実装・ハーネスは注入された基準ディレクトリで解決する（ADR-030、AC-10 が明示的に追認）。ポート適合テストは「注入した基準からの解決」しか検証しないが、**基準ディレクトリが実際のカレントディレクトリに束ねられていること**は CLI の TC-049〜051（`.cwd(dir)` + 相対 `--workflow`）と TC-008（相対 `--repo`）が端から端まで押さえているので、経路としての穴はない。
- **TC-055 のブランチ名の網羅**: spec は「空白・**制御文字**を含む」を挙げているが CLI 側は空白（`ma in`）のみ。制御文字は `task/branch.rs` のユニットテスト（`空白と制御文字を含むブランチ名は受理されない`）が押さえており、CLI は同じ `BranchName::parse` を通すだけなので実害はない。
- **TC-port-task-repository-043 の Io 分岐**: spec は「`save` が `Err`（NotFound / Io）を返した」だが、実装は NotFound 分岐だけで観測している。HOOKS.md がその判断（「NotFound 分岐だけで観測できる」）を明示しており、Io 分岐は TC-011 が別に押さえているので妥当。
- 適合テストのスキップ1件（`TC-port-clock-005`）は plan.md の運用（「スキップで終わった行はチェックせず、理由を Issue のコメントに残す」）に従って処理する必要がある。レビュー時点では未確認。

## カバレッジ

一覧121行との対応（確認 68 + スキップ 53 = 121）。

### 確認（68）

契約・手順書:
`.thread/1/plan.md`, `.thread/1/testing.md`（AC-13 / AC-15 関連の手動確認範囲を照合）

適合テストの枠組み（17）:
`crates/pulsen-conformance/HOOKS.md`, `crates/pulsen-conformance/src/lib.rs`, `src/clock.rs`, `src/config_store.rs`, `src/exclusive_lock.rs`, `src/task_id_generator.rs`, `src/task_repository.rs`, `src/workflow_store.rs`, `src/worktree_manager.rs`, `src/doubles/mod.rs`, `src/doubles/clock.rs`, `src/doubles/lock.rs`, `src/doubles/stores.rs`, `src/doubles/task_id.rs`, `src/doubles/task_repository.rs`, `src/doubles/tests.rs`, `src/doubles/worktree.rs`

ドメインのユニットテスト（22）:
`crates/pulsen-domain/src/definition/agent.rs`, `assembler.rs`, `command.rs`, `config.rs`, `duration.rs`, `name.rs`, `reference.rs`, `snapshot.rs`, `template.rs`, `validator.rs`, `workflow.rs`, `crates/pulsen-domain/src/task/attempt.rs`, `branch.rs`, `counters.rs`, `degraded.rs`, `failure.rs`, `id.rs`, `path.rs`, `process.rs`, `state.rs`, `task.rs`, `time.rs`

アダプター・ユーティリティのユニットテストと、アサーションの識別力の突き合わせ先（13）:
`crates/pulsen/src/adapter/clock.rs`, `adapter/lock.rs`, `adapter/task_file.rs`, `adapter/task_id.rs`, `adapter/worktree.rs`, `adapter/yaml.rs`, `application/home.rs`, `cli/exit.rs`, `cli/render.rs`, `cli/wire.rs`, `util/atomic.rs`, `util/fsdir.rs`, `crates/pulsen/examples/lock_holder.rs`

統合テスト（13）:
`crates/pulsen/tests/cli_add_boundary.rs`, `cli_add_error.rs`, `cli_add_normal.rs`, `common/git.rs`, `common/lock.rs`, `common/mod.rs`, `conformance_config_store.rs`, `conformance_lock.rs`, `conformance_task_repository.rs`, `conformance_time_id.rs`, `conformance_workflow_store.rs`, `conformance_worktree.rs`, `register_task.rs`

環境（1）:
`flake.nix` — devShell への `git` 追加（git フィクスチャの前提）を確認

### スキップ（53）

- `.adr/019`〜`.adr/050` の25ファイル — 設計判断の記録。Test 観点の判定基準は plan.md / CLAUDE.md / spec に置いたため、ADR 本文は読んでいない（アーキ観点の担当範囲）
- `.thread/1/adr.md`, `.thread/1/progress.md`, `.thread/1/steps.md` — 計画・進行の記録でテスト成果物ではない
- `Cargo.lock`, `Cargo.toml`, `crates/pulsen-conformance/Cargo.toml`, `crates/pulsen-domain/Cargo.toml`, `crates/pulsen/Cargo.toml`, `rustfmt.toml` — 依存・整形の設定（AC-1 の依存空チェックは他観点）
- `crates/pulsen-domain/src/{lib.rs, definition/mod.rs, definition/port.rs, execution/mod.rs, execution/port.rs, task/mod.rs, task/port.rs}` — 再エクスポートとトレイト宣言。テストを持たず、契約の1:1一致は AC-7（アーキ観点）の担当
- `crates/pulsen/src/adapter/{config_store.rs, task_repository.rs, workflow_store.rs, mod.rs}` — テストを持たない実装本体。適合スイート（24 / 44 / 31件）経由で振る舞いを検証済み
- `crates/pulsen/src/application/{mod.rs, register_task.rs}`, `crates/pulsen/src/cli/{add.rs, args.rs, mod.rs}`, `crates/pulsen/src/{lib.rs, main.rs}`, `crates/pulsen/src/util/mod.rs` — テストを持たない実装本体・配線。`register_task.rs`（テスト）と CLI 統合テスト経由で検証済み
