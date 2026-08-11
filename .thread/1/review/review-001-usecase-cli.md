### Use Case / CLI

#### 前提と確認方法

- 判定基準は `CLAUDE.md`（ヘキサゴナル・関数型ドメインモデリング）と `.thread/1/plan.md`（AC-11〜14・17、スコープ）、および `spec/usecases/task.md#registertaskadd` / `spec/pages/index.md`（共通事項・縮退状態の共通規則・add の節）。
- コード読解に加えて、実際に `cargo test -p pulsen --test cli_add_normal --test cli_add_error --test cli_add_boundary --test register_task` を実行し、**62 + 20 = 82 件すべて green** であることを確認した（CLI 62件 = spec の 67件 − ユースケース層で消化する5件。plan.md「テスト方針」の配分と一致）。
- exit code とホーム解決は自動テストで埋まっていない経路があるため、`target/debug/pulsen` を手で叩いて確認した（結果は W-002 / W-004 に記載）。

#### spec の処理フローとの照合

`spec/usecases/task.md` の RegisterTask 処理フローと `crates/pulsen/src/application/register_task.rs:127-217` は順序・分岐とも一致している。

| spec の手順 | 実装 | 判定 |
|---|---|---|
| 共通事項: ホーム解決 → `ConfigStore::load` を**実行前に** | `cli/wire.rs:131-154`（`compose`）。ロック取得前 | ○ |
| 1. `ExclusiveLock::try_acquire` | `register_task.rs:128-134`。`Ok(None)`→`LockBusy` / `Err(Failed)`→`LockFailed` | ○ |
| 2. `WorkflowRef::parse` → `WorkflowStore::load` | `:136-143`。`resolved_from` を出力DTOへ | ○ |
| 3. `display_name(declared_name)` | `:145-147` | ○ |
| 4. 対象検証 `validate_repo` → `head_branch` / `branch_exists` | `:187-217` | ○ |
| 5. `RegistrationValidator::validate` | `:151-152`。`Vec<RegistrationError>` を全件そのまま返す | ○ |
| 6. `TaskIdGenerator::generate` → `Task::register` | `:157-163` | ○ |
| 7. `create`（`Conflict` は1回だけ再試行） | `:164-182`。`retried` フラグで2回目の `Conflict` を打ち切り。ID は毎周回 `generate` し直す | ○ |
| 共通事項: parse は入力境界で一度だけ | ADR-048 に従い「その値を最初に使う直前」。`RepoPath::parse` / `BranchName::parse` が手順4の位置にある | ○ |

`_guard` はシャドウされない束縛なので `execute` のスコープ末尾まで生き、全ての `return Err(...)` 経路でドロップ＝解放される。`FileLockGuard` は `File` を保持するだけで（`adapter/lock.rs:55-59`）、ドロップでの解放がポート契約どおり。**失敗経路でロックが漏れる箇所はない。**

規約面も clean:

- `crates/pulsen/src/{application,cli}/` の非テストコードに `unwrap` / `expect` / `panic!` / `todo!` / `unimplemented!` は**1件もない**。
- `match` のワイルドカード `_ =>` も**1件もない**。`CreateError` / `TargetError` / `WorkflowParseError` / `RegistrationError` / `LockError` はすべて明示的に列挙されている（`render.rs` の各関数）。
- `application/` の `use` は `pulsen_domain` と `std` だけ（grep 確認）。依存方向は保たれている。
- `Command` enum は `Add` のみ（`cli/args.rs:22-25`）。`tick` / `ls` / `show` / `abort` / `retry` / `set-status` / `wrapper` の紛れ込みは**なし**。機械可読出力のフラグ（`--json` 等）も設定・定義の生成コマンドも**存在しない**（PAGE-common-007 / 011 は「提供しない」で充足）。

#### 受け入れ基準の判定

- **AC-13**（ホーム解決 `--home` > `PULSEN_HOME` > `~/.pulsen/`、起動時 config 読み込み、`NotFound` の案内）: 実装は `wire.rs:157-170` / `render.rs:76-85` で満たす。手動実行でも `PULSEN_HOME` 単独・`--home` 優先・未初期化案内（ホームパス + config.yaml のパス + 「作成してください」）を確認した。ただし `PULSEN_HOME` 単独経路に自動テストが無い（W-002）。
- **AC-14**（順序、成功時の表示、`Conflict` 1回再試行）: 満たす。`render::registered` はタスクID・ワークフロー名・解決先の3項目を出す（`render.rs:22-33`、TC-001/002 で検証済み）。
- **AC-17**（登録直後のタスクファイルの中身と `state/` 自動作成）: TC-009 / TC-060 が実ファイルで検証している。`state/` は `FileExclusiveLock::open` が `ensure_dir` するため、ロック取得の時点で作られる。config 読み込みはロックより前なので、**未初期化ホームでは `state/` すら作られない**（TC-014 の観測と整合）。
- **AC-11 / AC-12**: アダプター・適合テスト観点のため本レビューでは扱わない（カバレッジのスキップに記載）。
- **スコープ超過**: なし。

#### Blockers

なし。

#### Warnings

- **[W-001]** アダプターへの具体的依存が合成ルート1箇所に閉じていない
  - 場所: `crates/pulsen/src/cli/render.rs:15`（`use crate::adapter::task_id::IdGeneratorInitError;`）。宣言側は `crates/pulsen/src/cli/wire.rs:61`（`WireError::IdGenerator(IdGeneratorInitError)`）
  - 理由: `cli/mod.rs:3-4` は「アダプターへの依存は `wire` の1箇所に閉じ」と自らの不変条件を宣言しているが、`render.rs` がそれを破っている。依存方向（外→内）としては違反ではないので Blocker にはしないが、**モジュールの doc が主張する性質と実体が食い違っている**のは、後続スライスで `ls` / `show` の render が増えるときに「render は adapter を import してよい」という前例になる。`IdGeneratorInitError` は `getrandom` の失敗という完全にアダプター固有の事情であり、CLI の文言層が知る必要はない。
  - 提案: `WireError::IdGenerator` を `IdGenerator { message: String }` に変え、`wire::compose` で `IdGeneratorInitError` → 文言化する（`render::id_generator_error` の中身をそのまま `wire` 側のヘルパへ移す）。これで `render.rs` の adapter import が消え、doc の主張と一致する。あるいは doc を「アダプター**の構築**は wire の1箇所」と実態に合わせて弱める。

- **[W-002]** `PULSEN_HOME` 単独でのホーム解決に自動テストが1件もない
  - 場所: `crates/pulsen/src/cli/wire.rs:161-165` / `crates/pulsen/tests/cli_add_boundary.rs:363-381`
  - 理由: `Add::home_env` を使っているのは TC-067 だけで、そこでは `--home` フラグも同時に立てている。つまり**フラグが常に勝つため `env::var_os(HOME_ENV)` の分岐は一度も実行されない**。`resolve_home` から環境変数の節を丸ごと削除しても、現在のテストスイートは全件 green のままになる。AC-13 / PAGE-common-001 が求める優先順位は「フラグ > 環境変数 > 既定」の3段であり、2段目が無検証のまま後続の全コマンドがこの結線を再利用する。（実装自体は正しい — 手動実行で `PULSEN_HOME=<home> pulsen add ...` が当該ホームに登録することを確認済み。）
  - 提案: TC-067 に「フラグなしで `PULSEN_HOME` のみを与えると、そのホームに登録される」ケースを1本足す。`Add` は既に `home_env` を持っているので `add(...).home_env(home.path()).run()` で書ける。

- **[W-003]** 空の `PULSEN_HOME` を「未設定」として扱う規則が spec にも ADR にもテストにもない
  - 場所: `crates/pulsen/src/cli/wire.rs:161-163`（`if let Some(value) = env::var_os(HOME_ENV) && !value.is_empty()`）
  - 理由: `PULSEN_HOME=` が設定されている状態で `add` を実行すると、既定の `~/.pulsen/` へフォールバックする。手動確認では実ユーザーの `~/.pulsen` を指して「未初期化」を案内した。POSIX の慣行としては妥当な判断だが、**spec には無い挙動上の決定**であり、根拠（why）がコードにもコメントにも残っていない。将来「空文字を明示的なエラーにすべきでは」という議論が起きたときに、意図的な選択だったのか偶然かが読み取れない。
  - 提案: `!value.is_empty()` の直前に why コメントを1行（「空文字は未設定と同義として扱う。空パスはホームとして解決不能なため」）を残し、境界値テストを1件足すか、ADR に落とす。

- **[W-004]** exit code 規約のうち「使い方の誤り = 2」「`--help` = 0」が自動テストで押さえられていない
  - 場所: `crates/pulsen/src/cli/exit.rs:12-23`
  - 理由: `tests/` に `Cli::try_parse` の失敗経路を通す実行が1つも無く、`exit::USAGE` と `from_clap` の `use_stderr()` 分岐は無検証。`reject_base` / TC-053 が `code == Some(1)` を assert しているのは「入力の検証エラーが clap の 2 に紛れない」ことの裏返しの確認であり、**2 側の期待は誰も固定していない**。手動確認では `pulsen add --repo <path>`（`--workflow` 欠落）と未知フラグ `--json` がいずれも 2、`--help` が 0 で正しく動作している。回帰が起きても気付けない状態。
  - 提案: `tests/` に「必須引数の欠落は 2」「未知フラグは 2」「`--help` は 0 かつ標準出力」を確かめる小さなテストを追加する。`--json` を未知フラグとして拒否する行は PAGE-common-007（機械可読形式を提供しない）の観測可能な裏付けにもなる。

- **[W-005]** ユースケーステストの名前が検証内容より広い（AC-5 の「全件」がユースケース層では確かめられていない）
  - 場所: `crates/pulsen/tests/register_task.rs:588-612`（`登録時検証のエラーは全件まとめて返り登録は行われない`）
  - 理由: テスト名は「全件まとめて」と言うが、与えている定義は `UnknownAgent` 1件しか生まない。エラーが1件のとき、`Vec` に1件入る実装と「最初の1件で打ち切る」実装は区別できないため、この名前が主張する振る舞いは検証されていない。CLAUDE.md の「テストは振る舞いを表す。仕様の言葉で名付ける」に照らすと、名前が実際の観測より強い。実質の担保は CLI の TC-046（3件を要求）にあるので機能上の穴ではないが、ユースケース層が「ドメインの返す Vec をそのまま素通しする」ことは層の責務としてここで固定したい。
  - 提案: 定義を TC-046 と同様の複数エラー版（`MissingModel` + `MissingSkillInput` など）に差し替えて `Err(Registration(vec![...]))` の要素数と内訳を assert するか、テスト名を実際の観測（`登録時検証のエラーはそのまま返り登録は行われない`）に合わせる。

- **[W-006]** `resolve_target` の3つの `map_err(Target)` のうち `validate_repo` の1つしかダブルで検証されておらず、その台本がポート契約と矛盾している
  - 場所: `crates/pulsen/tests/register_task.rs:526-566`（`対象の分類はそのまま返り登録は行われない` / `git操作自体の失敗は実行環境のエラーになる`）、対応する実装は `crates/pulsen/src/application/register_task.rs:193-214`
  - 理由: テストは `DetachedHead` / `EmptyRepository` を `with_validate_repo([Err(...)])` で流し込んでいるが、`WorktreeManager` の契約（`crates/pulsen-domain/src/execution/port.rs:32-41`）では HEAD 由来の分類を返すのは `head_branch` であって `validate_repo` ではない。つまり**実在しない組み合わせを模した台本**になっており、`base` 省略時に `head_branch` のエラーが `Target` へ写るかどうか、`branch_exists` の `Err` が握り潰されないかどうかは、ダブルでは1度も確かめられていない。実挙動は CLI の TC-038 / TC-039（実 git の detached / 空リポジトリ）で担保されているので Blocker にはしないが、ADR-028 が「ポートが本当に差し替え可能かをここで検証する」と述べた目的から外れている。
  - 提案: `DetachedHead` / `EmptyRepository` は `with_head_branch([Err(...)])` に、`Failed` は `validate_repo` / `head_branch` / `branch_exists` の3経路それぞれに割り当てる。`WorktreeManagerCall` の記録があるので、呼び出し列の assert も併せて書ける。

- **[W-007]** `Runtime::home()` とそれが公開するフィールドが未使用
  - 場所: `crates/pulsen/src/cli/wire.rs:65-80`（`home: PulsenHome` フィールドと `pub fn home`）
  - 理由: `compose` の後に `home` を読む呼び出し元はリポジトリ内に存在しない（`grep '\.home()'` で 0 件）。`pub` なのでコンパイラの dead_code にも clippy にも掛からず、静かに残る。`PulsenHome` は各アダプターの構築時に消費済みで、`Runtime` が保持する理由がコードから読めない。
  - 提案: 使う予定があるなら why コメントで「後続スライスの `show` がタスクファイルのパス表示に使う」等を明記し、無いならフィールドごと落とす。

- **[W-008]** 成功時の案内が本スライスに存在しないコマンドを指す
  - 場所: `crates/pulsen/src/cli/render.rs:31`（`"  次回の tick で実行されます。"`）
  - 理由: `spec/pages/index.md` の add は「実行はしない（次回tickを待たず開始したい場合は `add && tick`）」なので完成形としては正しい文言だが、本スライスのバイナリで `pulsen tick` を叩くと clap の usage エラー（exit 2）になる。walking skeleton を触る人間には「案内どおりに操作したのに失敗する」導線になる。
  - 提案: 意図的に完成形へ寄せているならこのままで良い（その旨を Issue のコメントに残す）。気になるなら「実行は次の tick に委ねられます（tick は後続スライス）」のように、現時点で不可能な操作を指示しない言い回しにする。

#### 参考: Blocker にしなかった論点

- **`branch_exists` を `--base` 明示時にしか呼ばない**: spec 手順4は「`base` 省略時は `head_branch` で解決 → `branch_exists` が false ならエラー」と書き、素直に読むと HEAD 由来のブランチにも存在確認が要るようにも取れる。ただし `head_branch` は定義上「HEAD が指す実在のブランチ」を返すので確認は冗長であり、実装（`register_task.rs:197-214`）と TC-006 の呼び出し列 assert が意図を明示している。設計判断として妥当と判断した。
- **ロック取得の副作用として `state/` と `state/lock` が作られる**: ワークフロー不在などで `add` が失敗しても `state/` は残る。ただし PAGE-common-009（※3）が「状態を書き込むコマンドが必要に応じて自動作成する」を明示しており、規則4「部分的な変更を残さない」が禁じているのは**タスクの部分登録**であって管理領域の作成ではない。テストの `Untouched` が利用者の用意したリソース（config.yaml / workflows/*.yaml）だけを見ているのも、この線引きと整合している。
- **`RegisterTaskError::Registration(Vec<RegistrationError>)` が空 Vec を型で禁じていない**: 空になれば `render` が「(0件)」を出す。ただし `Vec` を返すのはドメインの `RegistrationValidator` の戻り値型であり、非空の表現はドメイン層の課題。本観点では扱わない。
- **`RegisterTaskError::InvalidRepoPath` が CLI からは到達不能**: `wire::absolute_repo` が `std::path::absolute` を通すため、`RepoPath::parse` が `NotAbsolute` を返す入力は CLI 経由では作れない。ユースケースを直接使う消費者（テストダブル・後続スライス）のための正当な分岐であり、削るべきではない。

#### カバレッジ

- 確認: `.adr/028-usecase-error-paths-via-test-doubles.md`, `.adr/030-workflow-store-base-dir-injection.md`, `.adr/031-pulsen-home-layout-in-application-layer.md`, `.adr/048-parse-inputs-at-spec-flow-position.md`, `.thread/1/plan.md`, `.thread/1/progress.md`, `crates/pulsen-domain/src/definition/reference.rs`, `crates/pulsen-domain/src/execution/port.rs`, `crates/pulsen-domain/src/task/port.rs`, `crates/pulsen-conformance/src/doubles/stores.rs`, `crates/pulsen-conformance/src/doubles/task_repository.rs`, `crates/pulsen/src/adapter/lock.rs`, `crates/pulsen/src/adapter/task_id.rs`, `crates/pulsen/src/application/home.rs`, `crates/pulsen/src/application/mod.rs`, `crates/pulsen/src/application/register_task.rs`, `crates/pulsen/src/cli/add.rs`, `crates/pulsen/src/cli/args.rs`, `crates/pulsen/src/cli/exit.rs`, `crates/pulsen/src/cli/mod.rs`, `crates/pulsen/src/cli/render.rs`, `crates/pulsen/src/cli/wire.rs`, `crates/pulsen/src/lib.rs`, `crates/pulsen/src/main.rs`, `crates/pulsen/tests/cli_add_boundary.rs`, `crates/pulsen/tests/cli_add_error.rs`, `crates/pulsen/tests/cli_add_normal.rs`, `crates/pulsen/tests/common/git.rs`, `crates/pulsen/tests/common/lock.rs`, `crates/pulsen/tests/common/mod.rs`, `crates/pulsen/tests/register_task.rs`（31件）
- スキップ: `.adr/` の残り21件（019・020・021・022・023・024・025・026・027・029・032・033・034・035・036・037・039・040・042・044・050）— ドメイン / アダプター / 適合テストの設計判断で、ユースケース・CLI の順序と文言に影響しない
- スキップ: `.thread/1/adr.md`, `.thread/1/steps.md`, `.thread/1/testing.md` — 計画・作業記録であり実装の判定対象ではない（契約は plan.md で読んだ）
- スキップ: `Cargo.lock`, `Cargo.toml` — ワークスペース構成・依存選定は ADR-019 / 023 の範囲でアダプター観点
- スキップ: `crates/pulsen-conformance/` の残り16件（`Cargo.toml`, `HOOKS.md`, `src/lib.rs`, `src/{clock,config_store,exclusive_lock,task_id_generator,task_repository,workflow_store,worktree_manager}.rs`, `src/doubles/{mod,clock,lock,task_id,worktree,tests}.rs`）— ポート適合テストの枠組みで、AC-8〜12 の担当観点
- スキップ: `crates/pulsen-domain/` の残り27件 — ドメイン層の値オブジェクト・遷移・スナップショット（AC-2〜7 の担当観点）
- スキップ: `crates/pulsen/Cargo.toml` — 依存宣言
- スキップ: `crates/pulsen/examples/lock_holder.rs` — ロック適合テストのフィクスチャ（ADR-032）
- スキップ: `crates/pulsen/src/adapter/` の残り8件（`clock.rs`, `config_store.rs`, `mod.rs`, `task_file.rs`, `task_repository.rs`, `workflow_store.rs`, `worktree.rs`, `yaml.rs`）— アダプター実装の観点
- スキップ: `crates/pulsen/src/util/{atomic,fsdir,mod}.rs` — 共通ユーティリティ（AC-19 の担当観点）
- スキップ: `crates/pulsen/tests/conformance_{config_store,lock,task_repository,time_id,workflow_store,worktree}.rs` — ポート適合スイートの適用（AC-9〜12 の担当観点）
- スキップ: `flake.nix` — devShell への `git` 追加（環境構築）
- スキップ: `rustfmt.toml` — 整形設定

合計 31 + 90 = 121 件（一覧と1対1）。
