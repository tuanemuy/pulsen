### Use Case / CLI

#### 前提と確認方法

- 判定基準は `CLAUDE.md`、`.thread/1/plan.md`（AC-11〜14・17、スコープ）、`spec/usecases/task.md#registertaskadd`、`spec/pages/index.md`（共通事項・縮退状態の共通規則・add）。`.thread/1/review/triage.md` の wont-fix 3件（W-004 / W-012 成功時の tick 案内 / W-017 create の TOCTOU）は本ラウンドでは扱わない。
- 実行して確認した: `cargo fmt --check`（OK）、`cargo clippy --all-targets -- -D warnings`（警告なし）、`cargo test -p pulsen --test cli_add_normal --test cli_add_error --test cli_add_boundary --test cli_usage --test register_task`（12 + 31 + 21 + 5 + 21 = **90件すべて green**。CLI の TC 行は 12 + 31 + 19 = 62 で spec の67件 − ユースケース層の5件と一致）。
- 自動テストに無い経路は `target/debug/pulsen` を直接叩いて観測した（`--home ""` / `--repo ""` / `--version` / 引数なし / 相対 `--home` / config の未知キー / ワークフロー定義の未知キー）。

#### 1周目の修正の検算

| 指摘 | 判定 | 根拠 |
|---|---|---|
| W-005 `render.rs` のアダプター import | 解消 | `grep 'crate::adapter'` の結果は `cli/wire.rs` の7行のみ。`cli/mod.rs:3-4` の「アダプターへの依存は `wire` の1箇所」が実体と一致した |
| W-005 派生: 文言をどこで作るか | 妥当 | `wire::id_generator_cause`(wire.rs:160-165) は合成ルート内にあり、`WireError` は CLI ローカルの型。既に `TargetError::Failed { message }` / `CreateError::Io { message }` / `ConfigLoadError::Io { message }` が「アダプターが書いた文字列を render がそのまま出す」形（spec のポート表が `message: String` を定めている）なので、パターンとして一貫している。why コメントもある |
| W-011 `Runtime::home()` | 解消 | フィールドごと削除。`compose` は `home` をローカルのまま消費し、残りのアクセサ7つはすべて `cli/add.rs` から使われている |
| W-007 空の `PULSEN_HOME` | 解消 | why コメント（wire.rs:172-173）＋境界値テスト（`空の環境変数は未設定として既定のホームに落ちる`）。実ユーザーの `~/.pulsen` を踏まないよう `user_home()` で既定ホームの基点を差し替えている点も適切 |
| W-006 `PULSEN_HOME` 単独 | 解消 | `環境変数だけが指定されていればそのホームに登録される` が `--home` なしで env のみを与え、既定ホームが使われないことも見ている。AC-13 の3段が全段検証された |
| W-008 / W-033 exit code と不在の主張 | 解消 | `tests/cli_usage.rs` 5件（必須引数欠落=2 / 未知フラグ `--json`=2 / `--help`=0 かつ stdout / サブコマンド集合 = `add` + clap の `help` / `add --help` に機械可読フラグが無い）。手動でも引数なし=2（stderr）・`--version`=0 を確認 |
| W-009 全件まとめて | 解消 | 台本が3件（`MissingSkillInput` 1 + `MissingModel` 2）を生み、要素数と内訳を assert している |
| W-010 ダブルの台本 | 解消 | `DetachedHead` / `EmptyRepository` は `with_head_branch` へ移り、`Failed` は `validate_repo` / `head_branch` / `branch_exists` の3経路に割り当てられ、`WorktreeManagerCall` の呼び出し列も assert されている。`ScriptedWorktreeManager` は台本を使い切ると panic するので、列の assert は実効性がある |
| W-035 `{error:?}` | 解消 | `adapter/worktree.rs` の `Failed` メッセージから Debug 表現が消えた。render を通した出力に Rust の構造体表記が出る経路は残っていない |
| W-021 拒否側の不変 | 解消 | `reject_base` / TC-053 / TC-058 に `untouched.assert_unchanged()` が入り、`Untouched` はファイルの顔ぶれも見るようになった |

#### spec の処理フローとの照合

`spec/usecases/task.md` の RegisterTask と `application/register_task.rs:127-217` は順序・分岐とも一致している（1周目から変更なし）。ロックは `_guard`（シャドウされない束縛）が `execute` の末尾まで生き、すべての `return Err(...)` 経路でドロップ＝解放される。`FileLockGuard` は `File` を持つだけ（`adapter/lock.rs:54-59`）でドロップが解放になる。**失敗経路でロックが漏れる箇所はない。**

`Conflict` の再試行は `retried` フラグで2回目を打ち切り、周回ごとに `ids.generate()` をやり直す（`:155-183`）。TC-012 / TC-047 が発行列と `create` の呼び出し回数（2回、3回目なし）を固定している。`create` は成功したときだけ `Ok` を返し、`Io` は即座に返すので、**エラー時にタスクが作られない**保証は「`create` が唯一の書き込み」であることから型で読める。

規約面:

- `crates/pulsen/src/{application,cli}/` の非テストコードに `unwrap` / `expect` / `panic!` / `todo!` / `unimplemented!` は**1件もない**（`home.rs:94` はテストモジュール内）。
- `match` のワイルドカード `_ =>` も**1件もない**。
- `application/` の `use` は `pulsen_domain` と `std` のみ。`#[cfg(unix)]` / `#[cfg(windows)]` は `crates/pulsen/src/{util,adapter}/` と `tests/` にしか現れない（AC-1）。
- `Command` enum は `Add` のみ。`tick` / `ls` / `show` / `abort` / `retry` / `set-status` / `wrapper` の紛れ込みも機械可読出力のフラグも**なし**（`cli_usage.rs` が不在を観測している）。

#### 受け入れ基準の判定

- **AC-13**（`--home` > `PULSEN_HOME` > `~/.pulsen/`、起動時 config 読み込み、`NotFound` の案内）: 満たす。3段すべてに自動テストが付いた。案内はホームパスと `config.yaml` の絶対パスと作成の要請を出す（TC-014・空env のテスト）。
- **AC-14**（順序・成功表示・`Conflict` 1回再試行）: 満たす。`render::registered` はタスクID・ワークフロー名・解決先の3項目（TC-001/002）。
- **AC-15**（拒否側でタスクが作られず利用者のリソースが変わらない）: 満たす。異常系31件・境界値の拒否4種すべてが `has_no_task()` と `assert_unchanged()` を通る。
- **AC-17**（登録直後のタスクファイルと `state/` 自動作成）: 満たす（TC-009 / TC-060）。config 読み込みがロックより前なので、未初期化ホームでは `state/` すら作られない（TC-014 と整合）。
- **AC-11 / AC-12**: アダプター・適合テストの観点のため本レビューでは扱わない（カバレッジのスキップに記載）。
- **スコープ超過**: なし。

#### Blockers

なし。

#### Warnings

- **[W-001]** ワークフロー定義の**スキーマ違反**では「どのファイルの話か」が出力に一切現れない。ADR-054 は判断として記録されているが、spec 追従の提起としては記録されていない
  - 場所: `crates/pulsen/src/cli/render.rs:165-167`（`WorkflowLoadError::Parse` の分岐）、`crates/pulsen/src/adapter/workflow_store.rs:88-104`（`at()` は `Io` と `YamlSyntax` にしか前置しない）、`.adr/054-workflow-error-file-path-goes-into-free-form-messages.md`、`.thread/1/progress.md:5-12`
  - 理由: 実測（`pulsen --home <H> add --workflow implement --repo /tmp`、定義に未知キー）の出力は

    ```
    エラー: ワークフロー定義が不正です。
      statuses.a にスキーマ外のキー `typo` があります。
    ```

    でファイル名もパスも無い。同じ「スキーマ違反」でも config.yaml 側は `render::config_error` が必ず `ファイル: <絶対パス>` を添える（実測で確認）ので、利用者から見て**同種のエラーの案内が非対称**になっている。名前指定（`--workflow implement`）では解決先は `<home>/workflows/implement.yaml` であり、home 自体が `PULSEN_HOME` や既定に落ちている場合、利用者は入力からファイルを特定できない。`NotFound` が「解決を試みたパス」を出し、成功時も「解決先」を出す（spec の「名前指定時の確認表示用」）のと比べても一貫しない。
    ADR-054 の**決定内容そのものは現契約では妥当**である（`WorkflowLoadError` は spec/domains/definition.md:305-308 で `NotFound { attempted }` / `Parse(WorkflowParseError)` / `Io { message }` と確定しており、AC-7 が1:1一致を要求する以上フィールドは足せない。`location` にパスを混ぜる案も棄却済み）。問題は**記録の非対称**で、ADR-050 が「スキーマ違反は対象ファイルの絶対パスと論理位置で示す」と決めたものを ADR-054 が狭めているのに、`progress.md` の「spec へ追従を提起した点」には載っていない。同じ性質の逸脱（ADR-050 由来のエラー位置の粒度、ADR-051 由来のファイル名の例示、`InputText` の生成規約）はすべて Issue コメント＋progress.md に記録されており、扱いが揃っていない。
  - 提案: `progress.md`「spec へ追従を提起した点」と Issue #1 のコメントに1件足す — 「`WorkflowLoadError::Parse` は解決先パスを持たないため、名前指定のワークフローがスキーマ違反のとき対象ファイルを案内できない。`Parse { error, resolved_from }` へポート表を改める提案」。コードは本スライスでは変えない（AC-7 を壊すため）。

- **[W-002]** 受け入れテストのフィクスチャが、環境変数 `PULSEN_HOME` は守るのに**実ユーザーのホーム**は守っていない
  - 場所: `crates/pulsen/tests/common/mod.rs:355-390`（`Add::run`）、`:396-408`（`run_cli`）
  - 理由: `Add::run` は `command.env_remove(HOME_ENV)` で実行環境の `PULSEN_HOME` の影響を必ず断つ（why コメントもある）が、`HOME` / `USERPROFILE` は `user_home()` を明示的に呼んだテストでしか差し替えられない。`--home` も `home_env` も `user_home` も付けずに `add(...).run()` と書いたテストは、**開発者の実 `~/.pulsen/` を解決先にして起動する**。config.yaml が存在すればそこに `state/tasks/<id>.json` を書き、無ければ「未初期化」を返す — どちらも一時ディレクトリの外に出る。現状の呼び出し元はすべて安全（`grep` で確認: ホーム未指定は新規2件のみで、両方 `user_home()` を付けている）だが、安全であることが**規約でしか担保されていない**。1周目の修正で「既定へ落ちる経路」を初めて踏むテストが入った（`空の環境変数は未設定として既定のホームに落ちる`）ので、この経路は今後も増える。`PULSEN_HOME` の混入は防ぐのにユーザーホームの混入は防がない、という非対称は `deny_read` の防御的な作り（ADR-027 に沿って「効いたことを確認してから `Some`」）とも揃わない。
  - 提案: `Add::run` / `run_cli` の既定で `HOME` / `USERPROFILE` を毎回作る一時ディレクトリに向け、`user_home()` はその上書きにする。既定ホーム経路の観測はそのまま書けて、書き忘れが実ホームに漏れる余地が消える。

- **[W-003]** `PulsenHome::worktree_root()` が未使用のまま残っており、`Runtime::home()` に適用した規則と扱いが揃っていない
  - 場所: `crates/pulsen/src/application/home.rs:65-67`
  - 理由: `grep -rn worktree_root crates/` の結果、`pulsen-domain` を除く参照は `home.rs` 内の定義・構築・**自分のユニットテスト（:116）だけ**である。他のアクセサ（`config_path` / `workflows_dir` / `state_root` / `lock_path`）はすべて `cli/wire.rs` から使われている。1周目の W-011 は「`pub` なので dead_code にも clippy にも掛からず静かに残る」「保持理由の why が無い」を根拠に `Runtime::home()` を**削除**して決着したが、同じ条件のこのアクセサは残った。ADR-031 が「`config_path` / `workflows_dir` / `state_root` / `worktree_root` / `lock_path` の導出」をレイアウトとして列挙しているので構築自体（`WorktreeRoot::parse` による絶対パス検証）を残す理由はあるが、**アクセサを公開し続ける理由**はそこには書かれていない。W-011 と本件で判断が割れると、「未使用の `pub` をいつ残してよいか」の基準が後続スライスに伝わらない。
  - 提案: `worktree_root()` に「WorktreeManager の `create` を足すスライスが使う」旨の why を1行付けるか、W-011 と同じくアクセサだけ落として必要になった時点で戻す。どちらでもよいが、W-011 と同じ基準で明示的に決めること。

#### 参考: Blocker にも Warning にもしなかった論点

- **縮退表の add 行の「—」が無検証**: `spec/pages/index.md` の縮退表は、add が「タスクファイル パース不能」「スナップショット 不在・パース不能」に**関与しない**（規則1）と定めるが、`state/tasks/` に壊れたファイルを置いて `add` が成功することを見るテストは無い。ただし `FsTaskRepository::create`(`adapter/task_repository.rs:134-157`) は衝突判定を `exists` だけで行い、走査も復号もしないため独立性は構造から読める。`steps.md` の消化表も PAGE-common-006 を規則2 のためだけに割り当てており、チェックリスト行としての要求も無い。テストを足すなら「後続で `create` の衝突判定を `list_active` 経由に変えても落ちない」ことへの保険という位置づけになる。
- **`println!` / `eprintln!` は書き込み失敗でパニックする**: `pulsen add | head` のような EPIPE でパニック経路に入る。CLAUDE.md の「パニックは不変条件違反にのみ」に厳密には反するが、Rust の CLI としては一般的な慣行であり、本スライスの出力は数行なので実害はほぼない。指摘するなら CLI 全体の方針として後続スライスで扱うべき論点。
- **`--repo ""` は clap の exit 2、`--workflow ""` は入力エラーの exit 1**: 実測で確認。`PathBuf` の value parser が空値を「値なし」として弾くためで、spec は `--repo` の空文字列を規定していない（境界値表にあるのは `--workflow` と `--base`）。どちらも非0で、`exit code 規約`（成功=0 / 入力・状態起因=非0）は満たす。
- **入力の parse 位置（ADR-048）**: `--base` の `BranchName::parse` は `validate_repo` の後にあるため、リポジトリ不在と `--base` 不正が同時なら前者だけが出る。1周目で ADR-048 として決着済みで、spec のどの TC の期待とも矛盾しない。
- **`--base` の `allow_hyphen_values`**: `--base --home X` が `--home` を値として飲む副作用は ADR-049 の「影響」に明記済み。実測でも非0（2）で止まる。

#### カバレッジ

- 確認（34件）: `.adr/024-git-cli-shell-out-and-target-classification.md`, `.adr/030-workflow-store-base-dir-injection.md`, `.adr/031-pulsen-home-layout-in-application-layer.md`, `.adr/049-base-flag-allows-hyphen-values.md`, `.adr/050-schema-error-location-is-logical.md`, `.adr/054-workflow-error-file-path-goes-into-free-form-messages.md`, `.thread/1/plan.md`, `.thread/1/progress.md`, `.thread/1/steps.md`, `.thread/1/review/triage.md`, `.thread/1/review/review-001-usecase-cli.md`, `crates/pulsen-conformance/src/doubles/worktree.rs`, `crates/pulsen/src/adapter/lock.rs`, `crates/pulsen/src/adapter/task_id.rs`, `crates/pulsen/src/adapter/worktree.rs`, `crates/pulsen/src/adapter/workflow_store.rs`（CLI に文言が届く経路のみ）, `crates/pulsen/src/adapter/task_repository.rs`（`create` と `list` の走査のみ）, `crates/pulsen/src/application/home.rs`, `crates/pulsen/src/application/mod.rs`, `crates/pulsen/src/application/register_task.rs`, `crates/pulsen/src/cli/add.rs`, `crates/pulsen/src/cli/args.rs`, `crates/pulsen/src/cli/exit.rs`, `crates/pulsen/src/cli/mod.rs`, `crates/pulsen/src/cli/render.rs`, `crates/pulsen/src/cli/wire.rs`, `crates/pulsen/src/lib.rs`, `crates/pulsen/src/main.rs`, `crates/pulsen/tests/cli_add_boundary.rs`, `crates/pulsen/tests/cli_add_error.rs`, `crates/pulsen/tests/cli_add_normal.rs`, `crates/pulsen/tests/cli_usage.rs`, `crates/pulsen/tests/common/mod.rs`, `crates/pulsen/tests/register_task.rs`
- スキップ: `.adr/` の残り28件（019・020・021・022・023・025・026・027・028・029・032・033・034・035・036・037・038・039・040・042・043・044・045・046・048・051・052・053）— ドメイン・アダプター・適合テスト・記録運用の設計判断で、ユースケースの順序と CLI の文言・終了コードに影響しない
- スキップ: `.thread/1/adr.md`, `.thread/1/testing.md`, `.thread/1/review/changed-files-001.txt`, `.thread/1/review/review-001.md`, `.thread/1/review/review-001-adapter.md`, `.thread/1/review/review-001-arch-spec.md`, `.thread/1/review/review-001-domain.md`, `.thread/1/review/review-001-test.md`（8件）— 他観点のレビュー記録・作業ログ。契約は plan.md と triage.md で読んだ
- スキップ: `Cargo.lock`, `Cargo.toml`（2件）— ワークスペース構成と依存選定（ADR-019 / 023、アダプター観点）
- スキップ: `crates/pulsen-conformance/` の残り17件（`Cargo.toml`, `HOOKS.md`, `src/lib.rs`, `src/{clock,config_store,exclusive_lock,task_id_generator,task_repository,workflow_store,worktree_manager}.rs`, `src/doubles/{mod,clock,lock,stores,task_id,task_repository,tests}.rs`）— ポート適合スイートの枠組みで AC-8〜12 の担当観点。ユースケーステストが使うダブルのうち呼び出し列の実効性に関わる `doubles/worktree.rs` だけ確認した
- スキップ: `crates/pulsen-domain/` の30件（`Cargo.toml` と `src/` 29件）— ドメイン層（AC-2〜7 の担当観点）。ポート表との1:1一致は AC-7 の担当
- スキップ: `crates/pulsen/Cargo.toml`, `crates/pulsen/examples/lock_holder.rs`（2件）— 依存宣言とロック適合テストのフィクスチャ（ADR-032）
- スキップ: `crates/pulsen/src/adapter/{clock,config_store,mod,task_file,yaml}.rs`（5件）— アダプター実装の観点
- スキップ: `crates/pulsen/src/util/{atomic,fsdir,mod}.rs`（3件）— 共通ユーティリティ（AC-19 の担当観点）
- スキップ: `crates/pulsen/tests/common/git.rs`, `crates/pulsen/tests/common/lock.rs`, `crates/pulsen/tests/conformance_{config_store,lock,task_repository,time_id,workflow_store,worktree}.rs`（8件）— git / ロックのフィクスチャと適合スイートの適用（AC-9〜12 の担当観点）
- スキップ: `flake.nix`, `rustfmt.toml`（2件）— devShell への `git` 追加と整形設定

合計 34 + 105 = 139 件（一覧と1対1）。
