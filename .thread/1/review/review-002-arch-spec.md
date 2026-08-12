### Architecture / Spec-conformance

2周目。Blocker 1 / Warning 11。ゼロベースで AC-1〜AC-20 を1つずつ実行検証し、変更ファイル139件を全量走査した。指摘のうち B-001 / W-002 / W-004 / W-005 / W-006 / W-010 の6件は、**1周目の修正そのものが生んだ、または修正が届かなかった**ものである。1周目の Blocker 1 / Warning 34（fix 判定分）はすべて実装に反映されていることを確認した（対応の確認結果は末尾「1周目指摘の消化確認」）。`wont-fix` の3件（W-004 / W-012 / W-017）と W-003 のシグネチャ変更部分は蒸し返していない。

#### Blockers

- **[B-001]** ADR-024 が「固定する」と宣言した `validate_repo` の判定手順と、その「影響」節の主張が、1周目 W-015 の修正で入った分岐と食い違ったまま
  - 場所: `.adr/024-git-cli-shell-out-and-target-classification.md`「## 決定」の `validate_repo(repo)` 1〜4（`1. パスが存在しない → NotFound`）と「## 影響」の「`TargetError::Failed` の到達経路は「git を起動できない」1本に定まる」／ 実装は `crates/pulsen/src/adapter/worktree.rs:63-71`
  - 理由: 1周目 W-015 の修正で `repo.as_path().exists()` が `try_exists()` になり、`Err(error)` が `TargetError::Failed { message: "リポジトリのパスを確認できない: …" }` を返すようになった。これは正しい修正だが、ADR-024 の手順1は「パスが存在しない → `NotFound`」の2値しか書いておらず、I/O エラーの分岐が無い。「影響」節の「`Failed` の到達経路は『git を起動できない』1本に定まる」も成り立たなくなっている（現在の到達経路は少なくとも4本: `run()` の起動失敗 `:50`、`try_exists` の `Err` `:67`、stdout の非 UTF-8 `:117`、`BranchName::parse` の失敗 `:123`。最後の1つは「決定」の表では触れられているが「影響」の記述と矛盾する）。この不整合が Blocker なのは、ADR-024 が「各メソッドの判定は次のとおり固定する」という**後続スライスへの規範**として書かれているためで、`worktree create` / `remove` を足す担当がこの手順に従うと、W-015 が直したばかりの「親ディレクトリを読めないリポジトリを『存在しません』と案内する」欠陥が同型で再導入される。加えて triage.md の `adapter-git-and-cli-wiring` グループは W-019 について「**先に `.adr/024` の共通項を更新してから**実装を合わせる（実装だけ直すと ADR と食い違う）」と明示しており、同じグループの同じファイルに対する W-015 でその手順が踏まれていない（環境変数の共通項は更新済み、判定手順は未更新）。
  - 提案: ADR-024 の `validate_repo` の手順を `1. パスの存在確認が I/O エラー → Failed / 2. 存在しない → NotFound / 3. git の起動自体に失敗 → Failed / 4. 起動できて exit 非0 → NotARepository / 5. exit 0 → Ok` に直し、「影響」節の「1本に定まる」を「対象の分類を確定できない状況（git を起動できない・パスの存在を確認できない・ブランチ名を扱えない）に限る」へ改める。判断基準（分類できたかどうかで分ける）を残すことで、後続スライスの `create` / `remove` にも同じ線引きが効く。

依存方向・合成ルート・スコープ・チェックリストの消化には、実装を止める欠陥は見つからなかった。

- 依存方向: `crates/pulsen/src/application/` からの `crate::adapter` 参照は0件。アダプター型を import するのは `crates/pulsen/src/cli/wire.rs:13-19` の1箇所のみで、`render.rs` からは消えている（1周目 W-005 の修正）。`std::env` / `std::fs` / `std::process` / `Command::new` は `application` / `cli`（`ExitCode` を除く）に1件も現れず、`crates/pulsen-domain/src` には `std::fs` / `std::io` / `std::process` / `std::time` / `SystemTime` が1件もない。
- スコープ「含まれないもの」の混入なし: `RunStore` / `ProcessController` / `CommandRunner` / `LaunchingClassifier` / `RunningClassifier` / `JudgementService` / `NotificationService` / `GcPolicy` / `IdentityCheck` / `WorkspacePlanner`、`WorktreeManager::create|remove`、`Task` の register / rehydrate 以外の遷移関数は、いずれも宣言もスタブも存在しない。`.github/` も無い。
- スタブ・仮実装なし: `todo!` / `unimplemented!` / `FIXME` / `TODO` / `XXX` の grep ヒットは0件。`unwrap()` / `expect(` は全205件が `#[cfg(test)]` 内。`panic!` / `unreachable!` も同様（`template.rs:198` の `unreachable!` のみ本体だが W-004 で wont-fix 判定済み）。

#### Warnings

- **[W-001]** PAGE-common 系6行は台帳の PASS 条件が全コマンド前提で、`add` の列しか満たせない。「部分実装は不可」という Issue 完了条件に対する扱いがどこにも記録されていない
  - 場所: `spec/inventory/frontend.md:8,9,11,12,72`（PAGE-common-002 / 003 / 005 / 006 / 010）、`spec/inventory/usecase.md:27`（UC-flow-007）／ `.thread/1/steps.md:432-437` の対応表
  - 理由: 台帳の PASS 条件を読むと、PAGE-common-002 は「ls / show は取得せず、add / tick / abort / retry / set-status は同一ロックを取得すれば PASS」、PAGE-common-005 は「tick のロックスキップのみ例外的に 0 を返せば PASS」、PAGE-common-006 は「4規則が縮退状態表の**全マス**（7コマンド×9状況）に一貫適用されれば PASS」、PAGE-common-010 は「tick のみ 0、abort/retry/set-status は非0」、PAGE-common-003 は「`notify_cmd` 未定義は stopped 確定処理時に検証」、UC-flow-007 は「作成→更新→アーカイブ移動→参照」の生涯全体。本スライスに存在するコマンドは `add` だけなので（`crates/pulsen/src/cli/args.rs:22-25`、`tests/cli_usage.rs:53` が集合を固定）、いずれも条件の一部しか満たせない。にもかかわらず `.thread/1/steps.md` の対応表はこれらを「ステップ17 で消化」と記す。Issue #1 の完了条件は「全行が実装されていること（スタブ・仮実装・**部分実装は不可**）」「見送る行はチェックせず、理由をこの Issue のコメントに残す」であり、`.thread/1/progress.md` の「環境によってスキップされるケース」表にも Issue の6コメントにも、この6行に関する記述が無い。スキップ運用の対象は `TC-port-clock-005` 1件だけとして扱われている。
  - 提案: 6行の扱いを1つに決めて記録する。(a)「本スライスの範囲（add の列）で満たされたものとしてチェックする」なら、その読み方を `plan.md` か `progress.md` に明記して steps.md の対応表に注記を付ける。(b)「後続スライスまで見送る」なら、`TC-port-clock-005` と同じ運用で Issue にコメントを残しチェックしない。どちらでもよいが、いま記録が無い状態は完了条件の判定を行単位で機械的に取れなくしている。

- **[W-002]** ADR-055 が `.thread/1/adr.md` にのみ存在し `.adr/` に未起票。1周目 W-027 が潰したはずの状態が、その修正で新しく1件生まれている
  - 場所: `.thread/1/adr.md:1039-1055`（`## ADR-055: 適合スイートの適用側は「許容するスキップ件数」を宣言する`。Status は `Accepted（レビュー指摘 W-030 への対応で確定。`.adr/` への昇格は次のスライスで扱う）`）／ `.adr/` は 054 までで 055 が無い
  - 理由: `.adr/035-file-slice-adrs-from-019.md` の決定は「実装中に生じた新しい決定は、同じ規則で連番を続けて起票する（ADR-037 以降）」と例外を設けていない。W-027 が7件を起票対象と判定した基準は「後続スライスを縛る規約であり、正本に無い状態は放置しない」だったが、ADR-055 はまさにその基準に当てはまる — 出荷物である `crates/pulsen-conformance/HOOKS.md:22` が「スイートを適用するテストファイルが『この環境で許容するスキップ件数』を宣言する（`SkipBudget`）」を規約として書いており、後続スライスが in-memory 実装にスイートを適用するときに必ず従う。しかもその HOOKS.md の記述には ADR への参照が無く（HOOKS.md 内の ADR 参照は 027 と 053 の2箇所のみ）、根拠を辿る手段が `.thread/1/` にしかない。これは ADR-035 の「影響」が防ごうとした「後続スライスの担当が根拠を辿れない」状態そのもの。あわせて `.thread/1/adr.md` は ADR-054（:999）が ADR-053（:1019）より前に置かれており、連番順でない。
  - 提案: `.adr/055-conformance-skip-budget.md` として起票し（ADR-038 の書式で）、`.thread/1/adr.md:1042` の Status にファイル名を書いて W-028 が成立させた「Status 行 = 起票済みの索引」を全件で保つ。HOOKS.md:22 からその ADR を参照する。adr.md の 053 / 054 の順序も入れ替える。

- **[W-003]** ADR-050 の決定「スキーマ違反は対象ファイルの絶対パスと論理位置で示す」が、ワークフロー定義では成立していない。ADR-054 が範囲を狭めた事実が ADR-050 側にも Issue コメントにも反映されていない
  - 場所: `.adr/050-schema-error-location-is-logical.md`「## 決定」／ `.adr/054-workflow-error-file-path-goes-into-free-form-messages.md`「## 決定」／ `crates/pulsen/src/cli/render.rs:181-185,214-216`（`UnknownKey` / `InvalidValue` の文言にパスが無い）／ Issue #1 コメント3件目の表／ `.thread/1/progress.md:9`
  - 理由: 実測で確認した。ワークフロー定義に未知キーを置いて `pulsen add --workflow wf` を実行すると出力は `エラー: ワークフロー定義が不正です。 / (トップレベル) にスキーマ外のキー \`typo_key\` があります。` で、解決先のパスが1文字も出ない（`agent: ""` による `InvalidValue` も同様）。同じ種類の違反を config.yaml で起こすと `ファイル: <絶対パス> / 原因: スキーマに無いキーです: typo` とパスが出る（`render.rs:87-96`）。ADR-054 は「パスを載せるのは `Io` と `YamlSyntax` の自由形式メッセージだけ」と決めており、その「影響」節が挙げるトレードオフは「構造の破れ（`NoAction` / `MissingNext` 等）ではパスが出ない」だけで、`UnknownKey` / `InvalidValue`（= ADR-050 が言う「スキーマ違反」）が対象外になることに触れていない。ADR-050 の決定文は無条件のまま残っており、2つの ADR が同じ語で違うことを言っている。さらに Issue #1 に投稿済みのコメント3件目は、表で「スキーマ違反（未知キー・型不一致・値の生成失敗）→ 対象ファイルの絶対パスと論理位置」と書いたうえで「該当するのは register-task.md の **config.yaml / ワークフロー定義**のパース不能の行」と適用先を明示している。この提案どおりに spec が言い換えられると、実装が満たさない要求が spec に入る。`progress.md:9` も同じ文で書かれている。なお `--workflow` を名前で指定した場合、解決先は `<home>/workflows/<name>.yaml` で利用者が直接指定していないため、パスが出ないことの実害は config.yaml より大きい。
  - 提案: 方針を1つに決めて3箇所を揃える。(a) 実装を ADR-050 に合わせるなら、`WorkflowLoadError::Parse` を CLI で描画する際に `RegisterTaskError` が持つ解決先パスを添える（ユースケースは `load` の戻り値からパスを知っているので、`WorkflowLoad` に `resolved` を持たせればポート表は変えずに済む）。(b) ADR-054 の限定を正とするなら、ADR-050 の決定文に「ワークフロー定義のスキーマ違反は例外（ADR-054）」を書き足し、Issue コメント3件目に追補を投稿して提案範囲を config.yaml に限る。あわせて `progress.md:9` を直す。

- **[W-004]** CLI 受け入れテストの4件が `println!` + `return` で無言スキップする。1周目 W-030 で適合スイートには `SkipBudget` を入れたのに、同じ欠陥が受け入れテスト側にそのまま残っている
  - 場所: `crates/pulsen/tests/cli_add_error.rs:108`（TC-016）、`:130`（TC-017）、`:182`（TC-021）、`:411`（TC-036）
  - 理由: 4件とも「前提を作れなければ `println!("スキップ: …")` して `return`」で、libtest は成功したテストの標準出力を握り潰すため緑と区別できない。これは W-030 の指摘文（「`CaseOutcome::report` が `println!` で報告するため libtest に握り潰され、緑と区別できない」）とまったく同じ構造で、W-030 は fix 判定され `SkipBudget` で解決された（`crates/pulsen-conformance/src/lib.rs:174-206`、unix では `ALLOWED_SKIPS = 0` 宣言なのでスキップが1件でも出れば失敗する）。受け入れテスト側だけが手当てされていないため、`.thread/1/progress.md:16` の「TC-port-clock-005 の1件を除き全件が実行された」という主張のうち、CLI 62件分は出力から確認できない。とくに TC-036 のスキップ条件（`common::git::is_outside_repository`）はリポジトリの置き場所に依存し、`/tmp` が git 管理下にある開発機では黙って落ちる。
  - 提案: `SkipBudget` と同じ形の宣言を `tests/common` に置き、4件をそこ経由にする（unix・非 root では許容0）。少なくとも TC-036 は前提の確認結果をテストファイル冒頭で1度だけ評価し、想定外にスキップしたら失敗させる。

- **[W-005]** `.thread/1/steps.md` の設計節が `InputText` の生成規約を実装と違う形で書いたまま残っている
  - 場所: `.thread/1/steps.md:57`（「名前系 newtype 7種（`AgentName` / `ModelName` / `SkillName` / `Prompt` / `StatusName` / `WorkflowName` / `InputText`）は `parse(String) -> Result<Self, NameError>` のみで生成する」）／ 実装は `crates/pulsen-domain/src/definition/name.rs:105-113` の `InputText::new(String) -> Self`
  - 理由: 1周目 W-032 の対応で `.thread/1/progress.md:11` と Issue コメント5件目には `InputText::new` の判断が書かれたが、steps.md は更新されていない。steps.md は「実装手順」の正本として後続スライスが参照する文書で、同じ表の同じ行が実装と食い違っている。triage.md の実行計画（`docs-adr-issue` グループ）も触るファイルに `steps.md` を挙げているが、対象は W-021 のステップ20だけだった。
  - 提案: steps.md:57 を「制約のある名前系 newtype 6種は `parse` のみで生成し、制約を持たない `InputText` は総関数（`new`）で生成する」に直す。

- **[W-006]** `adapter/task_file.rs` の復号エラー25箇所が `{error:?}` で Rust の Debug 表現をメッセージに載せる。1周目 W-035 と同じ欠陥が、同じ PR 内の別ファイルに残っている
  - 場所: `crates/pulsen/src/adapter/task_file.rs:305,307,310,312,315,522,524,533,539,553,555,572,576,586,596,601,603,606,627,631,638,643,648,654,656`
  - 理由: W-035 は `worktree.rs:104` の `format!("HEAD のブランチ名を扱えない: {error:?}")` を「`ContainsWhitespaceOrControl { char: ' ', position: 3 }` のような Rust の構造体表記が利用者に出る」として fix 判定し、実際に `worktree.rs:123-125` は扱えなかった名前だけを載せる形に直った。ところが `task_file.rs` は同じ形のまま25箇所ある。ここで作られる文字列は `TaskLookup::Corrupt { message }` / `TaskEntry::Corrupt { message }` / `TaskRecord::SnapshotUnreadable` の理由になり、`spec/pages/index.md:118`「パース不能なタスクファイル、およびスナップショットが読み取り不能なタスクの検出・報告（**修復の入口**）」として `ls` / `show` が利用者に見せる値である。本スライスでは `add` が `find` を呼ばないため表面化しないが、修復材料を人間に読ませることがこの経路の目的なので、`BranchNameError` や `TimestampError` の Debug 表現が出るのは spec の意図に反する。`render.rs:338-351` にはこれらの型の日本語化がすでにある。
  - 提案: 少なくとも `render.rs` が持つ日本語化と定義箇所を1つにする（1周目 W-002 で `NameError` / `DurationError` / `CommandError` に `describe()` をドメインへ置いた形をそのまま広げる）。本スライスで全部やらないなら、`ls` / `show` を入れるスライスまでの残作業として `progress.md` に記録し、`{error:?}` を1箇所に集約しておく。

- **[W-007]** `.thread/1/testing.md` の AC-1 確認手順が、書かれたとおりに実行すると期待と食い違う
  - 場所: `.thread/1/testing.md:44-48`（`grep -rn 'cfg(unix)\|cfg(windows)' crates/` に対して「`crates/pulsen/src/adapter/` と `crates/pulsen/src/util/` 配下だけがヒットし、`crates/pulsen-domain/` が1件もヒットしないこと」）
  - 理由: 実際に実行すると `crates/pulsen/src/util/atomic.rs`（2件）に加えて `crates/pulsen/tests/common/mod.rs`（2件）・`crates/pulsen/tests/conformance_config_store.rs`（2件）・`crates/pulsen/tests/conformance_task_repository.rs`（7件）・`crates/pulsen/tests/conformance_workflow_store.rs`（2件）がヒットする。`crates/pulsen/src/adapter/` は逆に**1件もヒットしない**。テスト側の `#[cfg(unix)]` は権限操作フックの提供有無を分ける正当なもので実装の問題ではないが、手順書の合格条件がそのままでは満たされないため、手動確認をなぞる人が判断に迷う（AC-1 の文言「`crates/pulsen/src/{adapter,util}/` 配下に限られ」も同じ書き方）。
  - 提案: 期待を「`crates/pulsen-domain/` に1件も現れず、`crates/pulsen/src/` 側のヒットは `util/atomic.rs` だけであること（`tests/` 配下は適合ハーネスの権限操作フックで、アダプター層の隔離とは別の話）」に直す。

- **[W-008]** Issue #1 のチェックリスト346行が全行未チェックのままで、完了条件の記帳が終わっていない
  - 場所: GitHub Issue #1（`- [ ]` が346件、`- [x]` が0件）／ `.thread/1/progress.md:26`
  - 理由: Issue の完了条件は「実装をレビューで確認できた行にのみチェックを付ける。見送る行はチェックせず、理由をこの Issue のコメントに残す」。見送り側（`TC-port-clock-005`）はコメント2件目で理由が残っているのに、確認できた側のチェックが1件も付いていない。`progress.md:26` の「TC-port-clock-005 は…チェックリスト行にチェックを付けない」という書き方は、他の行にはチェックが付くことを前提にしている。AC-20 の「Issue のチェックリスト全行が実装され」の判定を、Issue 側からは追えない状態になっている。
  - 提案: 2周目のレビュー完了後にチェックを付ける運用なら、その順序を `progress.md` の残作業に1行書く。W-001 の6行はチェックの可否がまだ決まっていないので、そこを決めてから一括で付ける。

- **[W-009]** 宣言している MSRV（`rust-version = "1.89"`）を検証する手段が PR 内に無く、ADR-022 が期待する効果が宣言だけでは得られない
  - 場所: `Cargo.toml:8`（`rust-version = "1.89"`）／ `.adr/022-std-file-lock-and-lockguard-marker-trait.md`「決定」／ `.adr/023-dependency-selection.md`「採らないもの」（`std::env::home_dir()` は Rust 1.97 で非推奨が解除済み）
  - 理由: ADR-022 は MSRV を明記する理由を「`File::try_lock` が無いツールチェーンを引いたときに原因が即座に分かる」としているが、devShell の rustc は 1.97.1 の1つだけで、CI も無いため 1.89 でのビルドは一度も行われていない。加えて ADR-023 は `std::env::home_dir()`（`crates/pulsen/src/cli/wire.rs:179` で使用）の非推奨解除を「Rust 1.97 で」と書いており、この記述をそのまま読むと 1.89 では deprecated 警告が出て `cargo clippy -- -D warnings`（AC-1）が通らない。ADR-023 の版数が「1.97 時点で解除済み」の意味なのか「1.97 で解除された」の意味なのかが文面から確定できず、どちらでも MSRV 宣言との整合が検証されていない点は変わらない。
  - 提案: ADR-023 の記述を「解除された版」に確定させ（`env::home_dir` の非推奨が解除された実際のリリースを明記）、それが 1.89 より後なら `rust-version` を引き上げる。検証手段が無いこと自体は、CI をスコープ外とした本スライスの帰結として `progress.md` の残作業に残す。

- **[W-010]** 出荷物 `HOOKS.md` の「ADR-027 の一覧から変えた点」が、参照先を見ても確認できない差分注記になっている。1周目 W-029 の修正で参照先を差し替えた結果
  - 場所: `crates/pulsen-conformance/HOOKS.md:208-212`（「ADR-027 の一覧から変えた点は次の3つで、根拠は `.adr/027-port-conformance-suite-and-harness-hooks.md` にある」＋ 3項目。最後の項目が「ExclusiveLock の `break_lock_location` を `unusable_lock`（別ハンドルを返す形）にした」）
  - 理由: `.adr/027-port-conformance-suite-and-harness-hooks.md:25-28` のフック表は**すでに変更後の名前**（`unusable_lock` / `absent_branch_name` / TaskIdGenerator の `another_generator` 行）になっており、変更前の状態も変更の経緯も残っていない。読み手が指示どおり `.adr/027` を開いても「変えた点」の3つを裏取りできない。`break_lock_location` という識別子は `crates/` にも `.adr/` にも1件も存在せず（grep で確認）、残っているのは `.thread/1/adr.md:315,704,711` と `.thread/1/steps.md:232` のスライス作業ログだけ。1周目 W-029 の修正前は参照先が `.thread/1/adr.md` で、そこには変更前の一覧（`break_lock_location` を含む）があったため注記が成立していた。参照先だけ差し替えたことで注記が宙に浮いた。あわせて、この注記は CLAUDE.md の「コードにもテストにも、指摘への弁明や修正の経緯を残さない。残すのは現在の形が成り立つ理由（why / why not）だけ」に照らしても経緯側の記述である。
  - 提案: 出荷物からは「変えた点」の3項目を落とし、`.adr/027` のフック表が正であることだけを示す（`ADR-041` の内容として `.adr/027` に「TaskIdGenerator の行を後から足した」等の理由を残したいなら、そちらに why として書く）。少なくとも `break_lock_location` という現存しない識別子は出荷物から消す。

- **[W-011]** ADR-030 の「cwd を読むのは合成ルートの1箇所だけにする」が、明文化されていない前提の上でしか成立していない（低）
  - 場所: `crates/pulsen/src/adapter/workflow_store.rs:87`（`std::path::absolute(&joined).unwrap_or(joined)`）／ `.adr/030-workflow-store-base-dir-injection.md`「## 決定」
  - 理由: `std::env::current_dir()` の明示呼び出しは `cli/wire.rs:130` の1箇所だけだが、`std::path::absolute` は引数が相対パスのとき内部で cwd を参照する。`absolutize`（`:81-88`）は先に `base_dir.join(path)` するため、`base_dir` が絶対である限り cwd は読まれない。しかし `FsWorkflowStore::new(workflows_dir, base_dir)` は `base_dir` が絶対であることを型でも実行時検査でも要求しておらず、ADR-030 の本文にもこの前提が書かれていない。適合テストのハーネスや後続スライスが相対の `base_dir` を渡すと、ADR の不変条件が静かに破れる（現在の合成ルートは `env::current_dir()` を渡すので実害はない）。
  - 提案: ADR-030 の決定に「`base_dir` は絶対パスであることを呼び出し側の前提とする」を1行足すか、`FsWorkflowStore::new` の doc コメントに前提として書く（`PulsenHome::new` が `AbsolutePathError` で絶対性を要求しているのと扱いを揃えるなら、型で受けるのが一貫する）。

#### カバレッジ

一覧139件と1対1で対応させる。

**確認（ADR / 正本）** — 全35件について「決定」節を抽出し、対応する実装コードを読んで照合した。不整合は B-001 / W-002 / W-003 / W-009 / W-011。ADR-027 のフック42個は実装（`conformance/src/lib.rs`）と過不足なく一致（既定実装の `None` も42個）。既存の `.adr/001`〜`018` とは、011（run dir gc）・013（未知キー拒否）・015（スナップショット埋め込み）・017（ドメイン境界）・010（循環許容）を突き合わせて矛盾なし（013 のキー集合は `adapter/config_store.rs:16-26` と `adapter/workflow_store.rs:18-23` に一致、010 は `definition/workflow.rs` のユニットテスト「自己参照と循環と到達不能なステータスは受理される」で担保、015/017 は `adapter/task_file.rs` とモジュール構成に一致）。1周目に新規起票された9件（038 / 043 / 045 / 046 / 049 / 051 / 052 / 053 / 054）は ADR-038 が定める書式（`## ステータス` / `## コンテキスト` / `## 決定` / `## 検討した代替案` / `## 影響`・語は「承認済み」）を満たし、いずれも実装に対応がある（038→`.adr/` 全ファイルの書式、043→`adapter/config_store.rs` `FsConfigStore::new(config_path, home)` と `adapter/workflow_store.rs:43`、045→`adapter/task_file.rs:42` の `TaskFileDto<Snapshot>`、046→`conformance/src/task_repository.rs` の観測フックの扱い、049→`cli/args.rs:42` の `allow_hyphen_values`、051→` .yaml` フィクスチャ、052→`tests/common/mod.rs` のビルダー、053→`HOOKS.md:26-28` と `conformance/src/lib.rs:16-20`、054→`adapter/workflow_store.rs:99-105` の `at()`）。寿命テスト（後続スライスを縛るか）・波及テスト（他の判断に影響するか）も9件すべて満たす。
`.adr/019-domain-crate-workspace.md`, `.adr/020-no-serde-in-domain-timestamp-conversion-in-domain.md`, `.adr/021-yaml-value-then-hand-written-schema-walk.md`, `.adr/022-std-file-lock-and-lockguard-marker-trait.md`, `.adr/023-dependency-selection.md`, `.adr/024-git-cli-shell-out-and-target-classification.md`, `.adr/025-task-file-json-and-corrupt-classification.md`, `.adr/026-task-id-format.md`, `.adr/027-port-conformance-suite-and-harness-hooks.md`, `.adr/028-usecase-error-paths-via-test-doubles.md`, `.adr/029-wildcard-enum-match-arm-lint-domain-only.md`, `.adr/030-workflow-store-base-dir-injection.md`, `.adr/031-pulsen-home-layout-in-application-layer.md`, `.adr/032-lock-holder-example-fixture.md`, `.adr/033-git-fixture-reproducibility.md`, `.adr/034-workflow-ref-separator-set-as-constant.md`, `.adr/035-file-slice-adrs-from-019.md`, `.adr/036-infallible-ports-absorb-failure-at-construction.md`, `.adr/037-platform-separator-set-without-cfg.md`, `.adr/038-adr-filing-format.md`, `.adr/039-read-error-shared-by-find-and-list.md`, `.adr/040-rehydrate-takes-field-bundle.md`, `.adr/042-absent-yaml-value-is-omission.md`, `.adr/043-store-adapters-receive-injected-paths.md`, `.adr/044-task-file-layout-in-domain.md`, `.adr/045-task-file-dto-generic-over-snapshot.md`, `.adr/046-no-skippable-hooks-for-post-operation-observation.md`, `.adr/048-parse-inputs-at-spec-flow-position.md`, `.adr/049-base-flag-allows-hyphen-values.md`, `.adr/050-schema-error-location-is-logical.md`, `.adr/051-undisplayable-name-fixture-is-whitespace-stem.md`, `.adr/052-acceptance-test-harness-in-tests-common.md`, `.adr/053-conformance-yaml-source-hooks.md`, `.adr/054-workflow-error-file-path-goes-into-free-form-messages.md`

**確認（スライスの記録）** — `.thread/1/adr.md`（Status 行と `.adr/` の実ファイル集合を機械照合。019〜054 は双方向に一致、041 / 047 は「反映済み」で正しく欠番、055 のみ未起票 = W-002）, `.thread/1/plan.md`（AC-1〜20 の検証基準）, `.thread/1/progress.md`（W-003 / W-008 で言及）, `.thread/1/steps.md`（対応表がチェックリスト全群を漏れなく覆うことを確認。ステップ20 は AC-15 に揃えて更新済み。:57 は W-005）, `.thread/1/testing.md`（W-007）, `.thread/1/review/triage.md`（wont-fix 判定の把握）

**確認（ビルド・全体検証）** — `Cargo.toml`（3クレート構成・workspace lints・MSRV は W-009）, `Cargo.lock`, `rustfmt.toml`, `flake.nix`（devShell への `git` 追加が ADR-024 の理由つきで入っている）, `crates/pulsen-domain/Cargo.toml`（`[dependencies]` が空 = AC-1 の機械的保証、`wildcard_enum_match_arm` は ADR-029 どおりドメインのみ）, `crates/pulsen/Cargo.toml`（`pulsen-conformance` が dev-dependencies に限られる）, `crates/pulsen-conformance/Cargo.toml`（依存は `pulsen-domain` のみ）

**確認（ドメイン）** — AC-2 / AC-3 / AC-4 / AC-5 / AC-6 / AC-7 の検証で読んだ。ポートのトレイトは spec のポート表と1:1（`TaskRepository` 7メソッド、`TargetError` 5種、`ConfigLoadError` 3種、`WorkflowLoadError` 3種、`WorkflowParseError` 12種のうち assembler 生成が AC-4 の10種、`RegistrationError` 5種、`LockError` 1種）。未実装メソッドの宣言なし。`effective_*` の `None` 枝は `Wait` / `Cleanup` と分離済み（1周目 W-003）。`WorkflowAssembler::assemble` に重複キーの前提条件が明記済み（1周目 W-001）。`describe()` がドメイン側に1つ（1周目 W-002）。
`crates/pulsen-domain/src/lib.rs`, `crates/pulsen-domain/src/definition/mod.rs`, `crates/pulsen-domain/src/definition/name.rs`, `crates/pulsen-domain/src/definition/duration.rs`, `crates/pulsen-domain/src/definition/command.rs`, `crates/pulsen-domain/src/definition/template.rs`, `crates/pulsen-domain/src/definition/agent.rs`, `crates/pulsen-domain/src/definition/config.rs`, `crates/pulsen-domain/src/definition/workflow.rs`, `crates/pulsen-domain/src/definition/reference.rs`, `crates/pulsen-domain/src/definition/snapshot.rs`, `crates/pulsen-domain/src/definition/assembler.rs`, `crates/pulsen-domain/src/definition/validator.rs`, `crates/pulsen-domain/src/definition/port.rs`, `crates/pulsen-domain/src/execution/mod.rs`, `crates/pulsen-domain/src/execution/port.rs`, `crates/pulsen-domain/src/task/mod.rs`, `crates/pulsen-domain/src/task/id.rs`, `crates/pulsen-domain/src/task/path.rs`, `crates/pulsen-domain/src/task/branch.rs`, `crates/pulsen-domain/src/task/time.rs`, `crates/pulsen-domain/src/task/process.rs`, `crates/pulsen-domain/src/task/attempt.rs`, `crates/pulsen-domain/src/task/counters.rs`, `crates/pulsen-domain/src/task/failure.rs`, `crates/pulsen-domain/src/task/state.rs`, `crates/pulsen-domain/src/task/task.rs`, `crates/pulsen-domain/src/task/degraded.rs`, `crates/pulsen-domain/src/task/port.rs`（29件）

**確認（アダプター・アプリケーション・CLI・ユーティリティ）** — 依存方向・合成ルートの単一性・AC-9〜AC-19 の検証で読んだ。1周目の B-001（`list` の NotFound スキップ）・W-013（`at()` でパス前置）・W-014（`rename_atomic` の両ディレクトリ `sync_dir`）・W-015（`try_exists`）・W-016（権限の意図を doc 化）・W-018（`taken` のパス付き）・W-019（`GIT_CEILING_DIRECTORIES` 等の追加）・W-020（非文字列キーの拒否）・W-035（`{error:?}` の除去）・W-005（`id_generator_cause` を wire へ）・W-007（空 `PULSEN_HOME` の why）・W-011（`Runtime::home()` の削除）はすべて反映を確認。
`crates/pulsen/src/lib.rs`, `crates/pulsen/src/main.rs`, `crates/pulsen/src/adapter/mod.rs`, `crates/pulsen/src/adapter/clock.rs`, `crates/pulsen/src/adapter/config_store.rs`, `crates/pulsen/src/adapter/lock.rs`, `crates/pulsen/src/adapter/task_file.rs`, `crates/pulsen/src/adapter/task_id.rs`, `crates/pulsen/src/adapter/task_repository.rs`, `crates/pulsen/src/adapter/workflow_store.rs`, `crates/pulsen/src/adapter/worktree.rs`, `crates/pulsen/src/adapter/yaml.rs`, `crates/pulsen/src/application/mod.rs`, `crates/pulsen/src/application/home.rs`, `crates/pulsen/src/application/register_task.rs`, `crates/pulsen/src/cli/mod.rs`, `crates/pulsen/src/cli/args.rs`, `crates/pulsen/src/cli/add.rs`, `crates/pulsen/src/cli/exit.rs`, `crates/pulsen/src/cli/render.rs`, `crates/pulsen/src/cli/wire.rs`, `crates/pulsen/src/util/mod.rs`, `crates/pulsen/src/util/atomic.rs`, `crates/pulsen/src/util/fsdir.rs`, `crates/pulsen/examples/lock_holder.rs`（25件）

**確認（適合スイート・テスト）** — AC-8 / AC-11 / AC-12 / AC-15 / AC-16 / AC-18 の検証で読んだ。HOOKS.md の125行が Issue のチェックリストの `TC-port-*` 125行、および `conformance/src/*.rs` のケース関数125個と完全一致（`diff` が空）。区分の集計 A 28 / B 85 / C 12 は7つの節見出しの内訳と一致し、実表の列を数えた結果とも一致。生 JSON を渡す API（`put_raw` / `read_raw`）は0件。1周目の W-022（観測カウンタ）・W-030（`SkipBudget`）・W-036(a)（TC-043 の Io 分岐）・W-036(b)（clock-003 を C に）・W-029（HOOKS.md の参照先を `.adr/027` へ）・W-026（適用範囲の明記）・W-006（`PULSEN_HOME` 単独）・W-008/W-033（`cli_usage.rs`）・W-009（複数エラー）・W-010（`with_head_branch`）・W-021（境界値の `assert_unchanged`）・W-023（`with_listings`）・W-024（`deny_read` の限定）・W-025（重複キーに「位置:」）・W-034（TC ID 接頭辞）はすべて反映を確認。残る問題は W-004。
`crates/pulsen-conformance/HOOKS.md`, `crates/pulsen-conformance/src/lib.rs`, `crates/pulsen-conformance/src/clock.rs`, `crates/pulsen-conformance/src/config_store.rs`, `crates/pulsen-conformance/src/exclusive_lock.rs`, `crates/pulsen-conformance/src/task_id_generator.rs`, `crates/pulsen-conformance/src/task_repository.rs`, `crates/pulsen-conformance/src/workflow_store.rs`, `crates/pulsen-conformance/src/worktree_manager.rs`, `crates/pulsen-conformance/src/doubles/mod.rs`, `crates/pulsen-conformance/src/doubles/clock.rs`, `crates/pulsen-conformance/src/doubles/lock.rs`, `crates/pulsen-conformance/src/doubles/stores.rs`, `crates/pulsen-conformance/src/doubles/task_id.rs`, `crates/pulsen-conformance/src/doubles/task_repository.rs`, `crates/pulsen-conformance/src/doubles/tests.rs`, `crates/pulsen-conformance/src/doubles/worktree.rs`, `crates/pulsen/tests/common/mod.rs`, `crates/pulsen/tests/common/git.rs`, `crates/pulsen/tests/common/lock.rs`, `crates/pulsen/tests/cli_add_normal.rs`, `crates/pulsen/tests/cli_add_error.rs`, `crates/pulsen/tests/cli_add_boundary.rs`, `crates/pulsen/tests/cli_usage.rs`, `crates/pulsen/tests/register_task.rs`, `crates/pulsen/tests/conformance_config_store.rs`, `crates/pulsen/tests/conformance_lock.rs`, `crates/pulsen/tests/conformance_task_repository.rs`, `crates/pulsen/tests/conformance_time_id.rs`, `crates/pulsen/tests/conformance_workflow_store.rs`, `crates/pulsen/tests/conformance_worktree.rs`（31件）

**スキップ** — 7件。
- `.thread/1/review/changed-files-001.txt` — 1周目のレビュー対象一覧。作業記録で、Architecture / Spec-conformance の判断材料にならない。
- `.thread/1/review/review-001.md`, `.thread/1/review/review-001-adapter.md`, `.thread/1/review/review-001-arch-spec.md`, `.thread/1/review/review-001-domain.md`, `.thread/1/review/review-001-test.md`, `.thread/1/review/review-001-usecase-cli.md` — 1周目のレビュー本文6件。指摘の内容と判定は `triage.md`（確認済み）が統合済みで、そちらで wont-fix の把握と修正確認の突き合わせを行った。2周目はゼロベースで見る方針のため、先入観を持たないよう本文は読まずに台帳だけを参照した。

合計: 確認 132件（ADR 35 + スライスの記録 6 + ビルド 6 + ドメイン 29 + アダプター等 25 + 適合スイート・テスト 31）+ スキップ 7件 = **139件**。

#### AC-1〜AC-20 の検証結果

| AC | 結果 | 根拠 |
|---|---|---|
| AC-1 | PASS | `cargo build` / `cargo test`（429件、失敗0）/ `cargo clippy --all-targets -- -D warnings` / `cargo fmt --check` すべて exit 0。`pulsen-domain/Cargo.toml` の `[dependencies]` は空。`cfg(unix)` / `cfg(windows)` は `crates/pulsen/src/util/atomic.rs` と `crates/pulsen/tests/` のみ、`crates/pulsen-domain/` は0件（手順書の文言は W-007） |
| AC-2 | PASS | `pulsen-domain` のユニットテスト163件が全通過。`NameError` 2 / `DurationError` 2 / `CommandError` 1 / `TemplateError` 2 / `ExpansionError` 1 / `AgentDefError` 3 の全分岐に生成箇所とテストがある |
| AC-3 | PASS | `definition/workflow.rs:135-193` の `effective_*` 4種、`definition/reference.rs:58-66` の `display_name` 4規則。区切り集合は `POSIX_SEPARATORS` / `WINDOWS_SEPARATORS` を `parse_with_separators` に明示的に渡すテストがある（ADR-034 / 037） |
| AC-4 | PASS | `definition/assembler.rs:25` の `WorkflowParseError` 12種のうち assembler が返す10種を確認。循環・自己参照・到達不能はユニットテストで受理を検証（ADR-010） |
| AC-5 | PASS | `definition/validator.rs:53-73` が `errors` に集約して返す。`tests/register_task.rs:674` が `MissingSkillInput` + `MissingModel`×2 の複数件を要素と順序で assert（1周目 W-009 の修正） |
| AC-6 | PASS | `task/task.rs:76,103`・`task/degraded.rs:46`・`task/state.rs:92`（6状態）。`RehydrateError::StatusNotInSnapshot` はユニットテスト「スナップショットにないタスクステータスは再構築されない」で検証。`Timestamp` の RFC3339 往復は `task/time.rs` の12テスト |
| AC-7 | PASS | ポート表と1:1。`TaskRepository` 7メソッド（`task/port.rs:130-151`）、`TargetError` 5 / `ConfigLoadError` 3 / `WorkflowLoadError` 3 / `LockError` 1。`WorktreeManager` は本スライスの3メソッドのみ宣言、`create` / `remove` の宣言もスタブも無い |
| AC-8 | PASS | `pulsen-conformance` が独立クレート（依存は `pulsen-domain` のみ、`pulsen` の dev-dependencies）。1ケース = 1 `#[test]`。HOOKS.md 125行 = Issue の `TC-port-*` 125行 = ケース関数125個（`diff` 空）。生 JSON を渡す API なし。TC-042〜044 は `concurrent_repo` フックに隔離 |
| AC-9 | PASS | `tests/conformance_config_store.rs` 24件通過 |
| AC-10 | PASS | `tests/conformance_workflow_store.rs` 31件通過。`resolved_from` が絶対パスであることは手動実行でも確認 |
| AC-11 | PASS | `tests/conformance_task_repository.rs` 44件通過 |
| AC-12 | PASS | clock 5 + task-id 5 = `conformance_time_id.rs` 10件、lock 7件、worktree 9件 = 26件。`cargo test --test 'conformance_*' -- --nocapture` のスキップ報告は `tc_port_clock_005` の1件のみ（残り25件が実走）。unix では `ALLOWED_SKIPS = 0` 宣言のため権限操作系8件は必ず実行される |
| AC-13 | PASS | `cli/wire.rs:168-183` が `--home` > `PULSEN_HOME` > `~/.pulsen/`。手動実行で `PULSEN_HOME` 単独指定時に未初期化案内（ホームパス + config.yaml の作成要求）と exit 1 を確認。`cli_add_boundary.rs:378,398,416` が3段全部を CLI から検証（1周目 W-006 / W-007） |
| AC-14 | PASS | `application/register_task.rs:127-184` が spec の処理フロー1〜8 の順どおり。`Conflict` は `retried` フラグで1回だけ再発行。手動実行で成功時にタスクID・ワークフロー名・解決先を表示して exit 0 |
| AC-15 | PASS | `cli_add_error.rs` 31件・`cli_add_boundary.rs` の拒否4ケース（TC-053/054/055/058）がすべて `has_no_task()` と `untouched.assert_unchanged()` を通す。`Untouched` は内容に加えディレクトリのファイル一覧も控える（1周目 W-021 / W-023） |
| AC-16 | PASS | `cli_add_boundary.rs` 21件。TC-049〜067 すべてに対応する関数がある |
| AC-17 | PASS | 手動実行で `state/tasks/<task-id>.json` を生成。整形済み JSON・`snapshot` 埋め込み・`task_status = initial`・`execution.state = pending`・カウンタ全0・`workspace` / `current_attempt` / `last_failure` が `null` を確認。`state/tasks/` は自動作成された |
| AC-18 | PASS | `tests/register_task.rs` の `tc_task_register_task_012 / 018 / 040 / 047 / 048` がテストダブルに対して実行される（1周目 W-034 で TC ID 接頭辞に統一）。実ファイルシステム・実プロセスを使わない |
| AC-19 | PASS | アトミック置換は `util/atomic.rs` の `write_atomic` / `rename_atomic` の1箇所、排他ロックは `adapter/lock.rs` の1箇所。`task_repository.rs` は両方を呼ぶだけで再実装が無い |
| AC-20 | 条件付き PASS | チェックリスト346行はすべて `spec/inventory/*.md` の台帳行と1:1（機械照合。差分0）。実装が存在しない行・スタブは0件。steps.md の対応表は346行の全群を漏れなく覆う。ただし PAGE-common 系6行は PASS 条件が全コマンド前提で `add` の列しか満たせず（W-001）、Issue のチェックも未着手（W-008） |

#### Issue #1 のコメント6件の正確性

1件目（TC-port-task-repository-022/028 の言い換え提起）— 正確。ADR-025 の分類表と `adapter/task_file.rs:9-14` の doc が一致し、44件が通っている。
2件目（TC-port-clock-005 のスキップ理由）— 正確。`--nocapture` でスキップが `tc_port_clock_005` 1件だけであること、他8件が実走することを実測で確認した。末尾の「スキップ件数を宣言して超過したら失敗させる形へ手当てします」は未来形だが、実装済み（`conformance_time_id.rs:81` ほか）。追補があるとなお良い。
3件目（エラー位置の粒度）— **不正確**。表の「スキーマ違反 → 対象ファイルの絶対パスと論理位置」が、ワークフロー定義では成立しない（W-003）。
4件目（表示名を決められないファイル名の例示）— 正確。`Path::new(".yaml").file_stem()` が `Some(".yaml")` を返すこと、実装が ` .yaml` で再現していることを確認。
5件目（`InputText` の生成規約）— 正確。`definition/name.rs:105-113` と一致。ただし steps.md が追随していない（W-005）。
6件目（「次回の tick で実行されます。」を残す判断）— 正確。triage.md の W-012 wont-fix 判定の記録として妥当。

#### コメントの質（CLAUDE.md）

コード・テストは問題なし。`grep -rn "レビュー\|指摘\|W-0[0-9][0-9]\|修正\|以前は\|もともと\|かつては" crates/` は0件で、修正の経緯・弁明は残っていない。読んだ範囲のコメントはすべて why / why not（例: `adapter/task_repository.rs:113-115` の「走査中にアーカイブされたエントリは、この領域にもう無いだけで失敗ではない」、`util/atomic.rs:51-52` の「別ディレクトリへの移動は2つのディレクトリエントリを変える」、`adapter/worktree.rs:11-13` の「ADR-024 の判断基準はこの目的であって変数名の列挙ではない」）で、自明な言い換えは見当たらなかった。唯一の例外は出荷物のドキュメント側で、`HOOKS.md:208-212` が「ADR-027 の一覧から変えた点」という経緯の記述を持つ（W-010）。

#### 1周目指摘の消化確認

fix 判定34件（B-001 + W-001〜003, 005〜011, 013〜016, 018〜036）はすべて実装・記録に反映されていることを個別に確認した。ただし次の3件は、直した先が狭く同種の欠陥が別の場所に残っている（それぞれ W-004 / W-005 / W-006 として起票）。

| 1周目 | 直った場所 | 残っている場所 |
|---|---|---|
| W-030（無言スキップ） | `pulsen-conformance` の `SkipBudget` | `tests/cli_add_error.rs` の4件 |
| W-032（`InputText` の記録） | `progress.md` / Issue コメント | `steps.md:57` |
| W-035（`{error:?}` が利用者に出る） | `adapter/worktree.rs:123-125` | `adapter/task_file.rs` の25箇所 |

さらに、次の3件は修正そのものが新しい不整合を生んでいる。

| 1周目 | 修正内容 | 生じた不整合 |
|---|---|---|
| W-015（`exists()` → `try_exists()`） | `worktree.rs:63-71` に `Failed` の分岐を追加 | ADR-024 の判定手順と「影響」が未更新（B-001） |
| W-029（HOOKS.md の参照先） | `.thread/1/adr.md` → `.adr/027` | 参照先に変更前の一覧が無く「変えた点」を裏取りできない（W-010） |
| W-030（`SkipBudget` の導入） | 新しい決定を `.thread/1/adr.md` に ADR-055 として記録 | `.adr/` に未起票のまま（W-002） |

wont-fix 3件（W-004 `unreachable!` / W-012「次回の tick で実行されます。」/ W-017 `create` の TOCTOU）と、W-003 のうち wont-fix とされたシグネチャ変更は、2周目でも再指摘していない。
