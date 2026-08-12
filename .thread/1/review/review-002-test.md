# レビュー2周目 — Test

対象: PR #8 / ベース `main` / 変更ファイル139件（`.thread/1/review/changed-files-002.txt`）
実行環境: macOS 25.4.0・非 root・一時ディレクトリはリポジトリ外・git あり

実行結果:

- `cargo test --workspace`: 447テスト全緑（適合125件 = config 24 / workflow 31 / task-repo 44 / clock 5 / task-id 5 / lock 7 / worktree 9、CLI 62件 + 使い方5件、ユースケース21件、ドメイン163件、pulsen クレート39件、conformance 12件）
- 連続8回実行、および CPU を32〜200倍に過負荷にした状態で TC-042/044 を55回反復 — フレーキーは観測されなかった
- `cargo test -- --nocapture | grep SKIP`: 実スイートのスキップは `tc_port_clock_005` の1件のみ（他4件は枠組み自身のユニットテスト）
- `cargo clippy --workspace --all-targets -- -D warnings` / `cargo fmt --all --check`: 通る
- 突然変異テスト: `wire.rs` の `!value.is_empty()` を外すと `空の環境変数は未設定として既定のホームに落ちる` が FAILED になることを確認（新規テストが実際にバグを検出する）。確認後に元へ戻し、`git status` がクリーンであることを確認済み

spec の件数照合（実測）:

| 対象 | spec の行数 | 実装ケース | 対応 |
|---|---|---|---|
| ports 7ポート（本スライス分） | 24+31+44+5+5+7+9 = **125** | 125 | 1:1・欠番も重複もなし |
| worktree-manager | 21行中の先頭9行（`create`/`remove` の12行はスコープ外） | 9 | 行1〜9と一致 |
| task/register-task | **67** | 67（CLI 62 = normal 12 / error 31 / boundary 19、ユースケース5 = TC-012/018/040/047/048） | 1:1 |
| HOOKS.md 対応表 | 125行・A 28 / B 85 / C 12 | 表の実数と見出しの集計が一致 | 整合 |

## Blockers

- **[B-001]** unix での `ALLOWED_SKIPS = 0` は、plan.md と ADR-027 が「スキップして理由を残す」と定めた root 実行を、8件のテスト失敗に変える
  - 場所: `crates/pulsen/tests/conformance_task_repository.rs:213-216`、`crates/pulsen/tests/conformance_config_store.rs:67-70`、`crates/pulsen/tests/conformance_workflow_store.rs:90-93`（`crates/pulsen-conformance/src/lib.rs:174-206` の `SkipBudget`）
  - 理由: `deny_read` / `deny_dir_read` / `deny_dir_write` は「制限が実際に効いたことを確認してから `Some` を返す」（`tests/common/mod.rs:470-493`、`tests/conformance_task_repository.rs:161-180`）。root では `chmod` が効かないため必ず `None` を返し、TC-port-config-store-023 / TC-port-workflow-store-030 / TC-port-task-repository-005・011・012・019・035・041 の**8件がスキップに落ちる**。ところが unix の宣言は 0 件なので、この8件はすべて `assert!(used <= allowed)` で失敗する。plan.md「リスクと注意点」は「権限操作系のフックは…効かなければ復元して `None` = スキップ」「**スキップで終わった行はチェックせず、スキップした旨と環境上の理由を Issue のコメントに残す**」と明示し、ADR-027 も同じ運用を定めている。契約が「記録して先へ進む」と決めた状況を、実装が「ビルドを赤にする」に変えている。root で走るコンテナ CI は珍しくなく、しかも `static BUDGET: SkipBudget = SkipBudget::new(ALLOWED_SKIPS)` は const 評価なので、利用者は**テストのソースを書き換えない限り回避できない**。W-030 の狙い（スキップを緑に紛れさせない）は正しいが、その手段が「権限が効かない環境 = 失敗」まで巻き込んでいる。非 unix 側（Windows）は `1 / 1 / 6` と実態に合っており、問題は unix の 0 だけである
  - 提案: 許容件数を**実行時に決める**。`SkipBudget` を `static BUDGET: LazyLock<SkipBudget>` で持てるようにするか、`SkipBudget::new` が `fn() -> usize` を受け取る形にして、テストファイル側で「権限制限が実際に効くか」を1度だけ probe（一時ファイルを `chmod 000` して読めるか見る、`deny_read` と同じ判定）し、効かない環境では C 区分の件数を許容値にする。これなら「環境が前提を作れない」= 宣言どおりのスキップ、「フックの実装漏れ・別ケースの想定外スキップ」= 失敗、の両方を保てる。probe の結果は `println!` ではなくスキップ報告行に乗るので出力にも残る

## Warnings

- **[W-001]** CLI 受け入れテストの4件は `println!` + `return` で黙ってスキップする。libtest が標準出力を握り潰すため成功と区別できない
  - 場所: `crates/pulsen/tests/cli_add_error.rs:107-110`（TC-016）、`:129-132`（TC-017）、`:181-184`（TC-021）、`:410-413`（TC-036）
  - 理由: W-030 で「`println!` による報告はスキップと成功を区別できない」と結論し、適合スイートには `SkipBudget` を入れた。同じ欠陥が CLI 側にはそのまま残っており、しかも**同じ前提条件の扱いが真逆**になっている — 一時ディレクトリがリポジトリ配下にある状況は、`conformance_worktree.rs:108-111` では宣言0件により失敗として現れるのに、`cli_add_error.rs:410` では黙って緑になる。TC-016/021/036/017 はいずれも Issue のチェックリスト行であり、Issue 完了条件は「見送る行はチェックせず理由をコメントに残す」を求めている。root や Windows で走らせた人は、走らなかった2行にチェックを付けてしまう。`.thread/1/progress.md` はこの4件を表に挙げているが、それは「この環境では走った」という**実行時の観測ではなく静的な注記**にすぎない
  - 提案: `tests/common/mod.rs` に CLI 用のスキップ集計（`SkipBudget` と同じ考え方でよい。テストファイル単位の `static`）を置き、`deny_read` / `lock::hold` / `is_outside_repository` が前提を作れなかったときはそこへ記録する。unix・非 root・リポジトリ外 TMPDIR での宣言は 0 件になり、B-001 と同じ probe を共有できる

- **[W-002]** TC-042 / TC-044 の観測回数の下限は「読み手が走った」ことしか保証せず、**書き手と重なったこと**を保証しない
  - 場所: `crates/pulsen-conformance/src/task_repository.rs:728-739`（TC-042）、`:837-845`（TC-044）、`:788-798`（`yield_until_observed`）
  - 理由: TC-042 は `for _ in 0..30 { save(large); save(small); }` を**すべて終えてから** `yield_until_observed` を呼び、その後に `observations > 0` を確かめる。読み手が一度も走らないまま書き手が完走した場合でも、待機の後に読み手が静止状態を1周観測すれば `observations == 1` になり緑になる。つまり「反復する保存の途中経過は読み手に観測されない」という行の主張は、**書き込み中に一度も読まれていなくても成立しうる**まま。W-022 が問題にしたのは「観測が起きて初めて意味を持つ」ことで、triage の方針も「可能なら `save` / `archive` 回数に見合う下限」と書いている。TC-044 はさらに厳しく、`archive` は1回の操作なので、待機に入る時点で移動はすでに終わっている — 現在の形では重なりの保証がまったく無い。過負荷（CPU 200倍）で55回反復してもフレーキーは出なかったので、実際には重なっている。問題は**テストがそれを要求していない**こと
  - 提案: TC-042 は書き手を「30周を終え、かつ `observations >= N`（例: 5）になるまで」回す（上限付き）。TC-044 は `archive` の**前**に読み手の最初の観測を待ち（`yield_until_observed`）、`archive` 後にさらに観測が増えたことを確かめる。どちらも実時間の待機を増やさずに重なりを要求できる

- **[W-003]** WorkflowStore の適合スイートは `prompt:` が `AgentInput::Prompt(本文)` になることを一度も検証していない
  - 場所: `crates/pulsen-conformance/src/workflow_store.rs:104-141`（TC-007）、`:191-244`（`AgentRun` を分解する唯一のケースだが `skill:` を使う）
  - 理由: spec/testcases/ports/workflow-store.md の該当行は前提に `AgentRun(prompt)` を含み `statuses` の正規化を期待するが、TC-007 が確かめるのは `declared_name` / `initial` / `statuses().len() == 3` と `Wait` / `Cleanup` の2件だけで、`queued` の中身を見ていない。スイート31件のどこにも `AgentInput::Prompt` の等値比較が無い（`grep` で確認）。したがって、`prompt` の本文をステータス名で埋める・`judge` と取り違える・切り詰める、といったアダプターのバグは**31件すべてを通過する**。実際に守っているのは CLI 受け入れテスト（`cli_add_normal.rs:204-208` のスナップショット比較）1箇所だけで、ポートの契約としては無防備。TC-029 の `statuses.queued.prompt` の `InvalidValue` は「値が `Prompt::parse` に届く」ことしか示さない
  - 提案: TC-007 に `definition.status(&status("queued")) == Some(&StatusDefinition::AgentRun { input: AgentInput::Prompt(Prompt::parse("実装して")…), … })` 相当の検証を足す（既存の TC-010 と同じ形が使える）

- **[W-004]** `cli/render.rs` は単体テストが1つも無く、W-035 で直した `TargetError::Failed` の文言を含む十数の分岐がテストから到達不能
  - 場所: `crates/pulsen/src/cli/render.rs:43-73`（`WireError` 5分岐）、`:137`→`:221-238`（`TargetError::Failed`）、`:114-116`（`LockFailed`）、`:129-132`（`InvalidRepoPath`）、`:143-152`（`Create` の2分岐）、`:310-312`（`AgentDefError::InvalidSkillInput`）、`:323-325`（`TemplateError::MalformedBrace`）
  - 理由: `render.rs` は「エラー値 → 表示テキスト」の純関数で、375行のほぼ全体が `match` の分岐だが `#[cfg(test)]` が無い。実アダプターでは作れない状況（`LockFailed` / `Target::Failed` / `Create::Conflict` / `Create::Io`）はユースケーステストで消化されているものの、そちらは `render` を通らないため**文言は誰も見ていない**。W-035 の指摘（`{error:?}` が利用者に出る）はまさにこの無テスト領域で起きたのに、修正後も回帰テストが無く、同じ種類の劣化が再発しても誰も気づかない。`WireError` の5分岐と `RegisterTaskError::InvalidRepoPath` に至っては、値を作る経路すら全テストに存在しない
  - 提案: `render.rs` に `#[cfg(test)] mod tests` を置き、少なくとも「実アダプターでは作れないエラー」（`WireError` 全5種・`Target::Failed`・`LockFailed`・`Create` 2種・`InvalidRepoPath`）について、Rust の `Debug` 表記（`{ ... }` や `::`）が出力に現れないことと、原因の語が含まれることを固定する。純関数なので I/O は要らない

- **[W-005]** `ForbiddenKey` のキー名は位置合わせの `zip` で決まるのに、6キー中3キーしかテストされていない
  - 場所: `crates/pulsen-domain/src/definition/assembler.rs:226`（`AGENT_RUN_KEYS`）と `:332-345`（`present` 配列 + `zip`）、テストは `:574-610`、適合側は `crates/pulsen-conformance/src/workflow_store.rs:480-511`
  - 理由: `AGENT_RUN_KEYS = ["agent","model","timeout","retries","judge","next"]` と、同じ順で組んだ `bool` 配列を `zip` して報告するキーを決める。2つの配列の対応が崩れると「`model` を書いたのに `timeout` が禁止キーとして案内される」という利用者に見える誤りになるが、テストが押さえているのは `judge` / `next` / `agent` の3つだけで、**`model` / `timeout` / `retries` は一度も検証されていない**。配列の途中に1つ挿入する種類の変更が無警告で通る。同様に `InvalidValue` の `location` は11箇所の生成のうち4箇所（`statuses.<name>` / `.prompt` / `.timeout` / `.judge`）しか固定されておらず、`workflow` / `agent` / `model` / `.skill` / `.agent` / `.model` / `.next` の論理位置は誤っていても検出されない
  - 提案: `ForbiddenKey` は6キーをループで回す1テストにする（`run: wait` に各キーを1つずつ足す）。`InvalidValue` の `location` も生成箇所ごとに1件ずつ足す

- **[W-006]** spec が規定しない `Wait` に対する実効値の問い合わせが、まだテストで固定されている
  - 場所: `crates/pulsen-domain/src/definition/snapshot.rs:114-119`
  - 理由: triage の W-003 は「spec が『呼び出しを規定しない』と明記する振る舞いを固定しているため」`Wait` に対する `effective_retry_limit == 2` の期待をテストから外す、と決めた。しかし同じテストの中に `effective_timeout(&status("waiting")) == DEFAULT_TIMEOUT`（および `effective_agent` / `effective_model` が `None`）が残っている。`workflow.rs` 側には「`Wait` に対する呼び出しは spec 上未規定」という doc が追加され、`workflow.rs` のテストは `Wait` を避けているのに、`snapshot.rs` だけ扱いが揃っていない。修正が片側にしか及んでいない
  - 提案: `snapshot.rs` の当該テストから `Wait` に対する `effective_*` の3行を落とし、委譲の検証（`initial` / `statuses` / `status`）だけ残す。実効値の優先順位は `workflow.rs` のテストが担っている

- **[W-007]** AC-1 の機械的確認（`cfg(unix)` の出現箇所）が、今回の修正で偽陽性になる
  - 場所: `.thread/1/plan.md:18`（AC-1）、`.thread/1/testing.md:44-48`
  - 理由: testing.md は `grep -rn 'cfg(unix)\|cfg(windows)' crates/` を実行して「`crates/pulsen/src/adapter/` と `crates/pulsen/src/util/` 配下だけがヒットする」ことを確認せよ、と書く。実測すると15件のうち**13件が `crates/pulsen/tests/` 配下**（`common/mod.rs` 2件、`conformance_task_repository.rs` 7件、`conformance_config_store.rs` 2件、`conformance_workflow_store.rs` 2件）で、うち4件は今回の `ALLOWED_SKIPS` の `#[cfg(unix)]` 分岐で増えたもの。手順どおりに実行すると AC-1 が不合格に見える。テスト側の OS 分岐（権限操作）は正当なので、直すべきは確認手順のほう
  - 提案: 手順の grep を `crates/*/src/` に限定し、AC-1 の文言も「本体コードの `#[cfg]` は adapter / util に限る（テストの権限操作は対象外）」と明示する

- **[W-008]** `Untouched::with_listings` はディレクトリを一切数えないため、`state/` 以外の新規ディレクトリ経由の書き込みを見逃す。TC-034 はそもそも listings を持たない
  - 場所: `crates/pulsen/tests/common/mod.rs:237-251`（`listed_files`）、`:171-175`（`untouched`）、`crates/pulsen/tests/cli_add_error.rs:390`（TC-034 は `Untouched::of` のみ）
  - 理由: `listed_files` は `path.is_file()` で絞るので、拒否経路がホーム直下や `workflows/` に**新しいディレクトリを作ってその中に書いた**場合は検出されない。除外の理由として doc に挙がっているのは `state/` の自動作成1点だけなので、名前で `state` だけを除けば粒度を落とさずに済む。また TC-034 だけが `Untouched::of(...)`（listings なし）で、W-023 の修正が届いていない — 31件の異常系の中でここだけ「新規ファイルの出現」を見ない
  - 提案: `listed_files` を「直下のエントリ名（ファイル・ディレクトリとも）から `state` だけを除いた集合」にする。TC-034 は `home.untouched()` を使い、外部の定義ファイルだけを追加で控える形に揃える

- **[W-009]** `SkipBudget` は件数だけを宣言し、**どのケースがスキップしてよいか**を宣言しない
  - 場所: `crates/pulsen-conformance/src/lib.rs:174-206`
  - 理由: 宣言が「N件まで」なので、想定した C 区分のケースが走った代わりに別のケースがフックを得られずスキップしても、合計が N 以内なら緑のまま通る。本スライスの実アダプターでは C 区分のフックが無条件に `None` を返す（Windows）か無条件に効く（unix 非 root）ため実害は出ないが、AC-8 が約束する「後続スライスの in-memory 実装が同じスイートを通す」場面では、宣言と実態のズレが集計で相殺されうる。ケース名は `record` に渡っているので、集合として宣言するほうが同じ手間で強い
  - 提案: `SkipBudget::new(&["tc_port_config_store_023"])` のように**許容するケース名の集合**を宣言し、集合外のスキップを失敗にする（件数の上限は集合の要素数から導ける）。B-001 の実行時 probe とも組み合わせやすい

- **[W-010]** 「待たずに返る」ロックのケースは、ブロックする実装に対して失敗せずハングする
  - 場所: `crates/pulsen-conformance/src/exclusive_lock.rs:43-56`（TC-003）、`:28-41`（TC-002）
  - 理由: `NON_BLOCKING` の判定（`elapsed < 5s`）は `try_acquire` が**返ってきた後**にしか評価されない。保持プロセスの解放はその判定より後の `release_holder(holder)` で行われるので、`try_acquire` が解放を待つ実装だと両者が互いを待ち、ケースは失敗ではなく無限にハングする。つまりこのケースが名指しする失敗モードを、宣言したアサーションでは検出できない（CI のジョブタイムアウトで気づくことになる）
  - 提案: `try_acquire` を `thread::scope` の子スレッドで走らせ、親は期限まで完了フラグを監視して、期限超過なら先に `release_holder` してから失敗させる。あるいは監視スレッド側で期限到来時に保持プロセスを解放し、`elapsed` の判定を有効にする

- **[W-011]** ID とパス導出のテストに、実装の同じ式を書き写しただけのものがある
  - 場所: `crates/pulsen/src/adapter/task_id.rs:173-179`、`crates/pulsen-domain/src/task/path.rs:245-255`
  - 理由: 前者は `TaskId::parse(id.as_str().to_owned()) == Ok(id)` を確かめるが、`TaskId` はフィールド非公開で `parse` 以外に生成経路が無いため、`generate()` が返すあらゆる値でこれは常に成り立つ（型の不変条件の言い換え）。しかもテスト名が主張する「組み立て規則が制約を満たす」は、`generate` が失敗時に構築済みの `self.verified` を返すフォールバック（`task_id.rs:66-77`）のせいで**壊れていても検出できない**。後者は `TaskFilePath::active(root, id) == active_dir(root).join(file_name(id))` で、実装（`path.rs:110-112`）の本体そのもの。隣接する `:219-243` が実際の配置を文字列で固定しているので、こちらは何も足していない
  - 提案: `task_id.rs` は「`yyyymmddThhmmss-<8桁>` の形」を文字列として固定するテスト（`:157` にある構成のテスト）に統合するか、`compose` を直接検証する。`path.rs:245` のケースは削るか、`archived` 側だけ文字列で固定する形に変える

- **[W-012]** AC-14 の処理順のうち「対象検証 → 登録時検証」だけが、どのテストでも固定されていない
  - 場所: `crates/pulsen/tests/register_task.rs:535-557`（対象検証の分類）、`:674-737`（登録時検証の全件）
  - 理由: 他の順序点は台本を空にしたダブルが呼び出しでパニックすることで固定されている — ロックが先（`:417-433` は `workflows.requested() == []`）、ワークフロー解決が対象検証より先（`:475-494` は `ScriptedWorktreeManager::new()`）、表示名決定が対象検証より先（`:497-517`）、登録時検証が ID 発行より先（`:674` は `ids: ScriptedTaskIdGenerator::new([])`）。残る「対象検証が登録時検証より先」だけは、リポジトリ不正のケースが正しい config を使うため入れ替えても両方通る。利用者から見ると「リポジトリも定義も不正なとき、どちらの案内が出るか」が変わる
  - 提案: 対象検証が失敗する台本に、登録時検証も失敗するワークフロー（`{model}` を要求する config など）を組み合わせたケースを1件足し、`Target` が返ることと ID が発行されないことを確かめる

- **[W-013]** 既定ホーム（`~/.pulsen/`）へ落ちる経路のテストが、実ユーザーのホームを触らない保証をテスト自身の中に持っていない
  - 場所: `crates/pulsen/tests/cli_add_boundary.rs:410-430`、`crates/pulsen/tests/common/mod.rs:334-340`（`Add::user_home`）、`.thread/1/testing.md:176-181`（手動手順2の手順3）
  - 理由: 子プロセスの `HOME` / `USERPROFILE` を差し替える方法自体は妥当で、`std::env::home_dir` は unix で `HOME`、Windows で `USERPROFILE` を先に見るため現行プラットフォームでは実ホームに落ちない。ただしテストはそれを**前提にしているだけで検証していない**。仮にホーム解決が環境変数を無視する経路に変わると、この1件は「実 `~/.pulsen/config.yaml` を読み、`workflows/implement.yaml` があれば実ホームにタスクを1件作ってから」assert で落ちる（落ちるので気づけるが、副作用は残る）。さらに `assert_rejected` だけで「タスクが作られない」を確かめていない点も、他の拒否ケース全件と扱いが揃っていない。手動テスト手順のほうは意図的に実 `~/.pulsen/` を対象にしており（確認ポイントで事後確認する形）、同じ危うさがある
  - 提案: このケースだけワークフロー名を実運用と衝突しない一意な値にし、`user_home` 配下に `.pulsen` が作られないことに加えて、実行後に `--home` 側・環境変数側の双方でタスクが作られていないことを確かめる。手動手順のほうは `HOME=$(mktemp -d)` を前置きする形に変える

- **[W-014]** 適合ケース・アダプターテストに、期待を半分しか見ていないものが残っている
  - 場所: `crates/pulsen-conformance/src/config_store.rs:182-197`（TC-010）、`crates/pulsen/src/adapter/yaml.rs:269-277`、`crates/pulsen/src/adapter/task_file.rs:768-774`
  - 理由: (a) TC-010 の spec 行は「壊れたテンプレートも `RawAgentDefinition` としてそのまま保持される」だが、ケースは `parse().is_err()` しか見ておらず、隣接する TC-009 / TC-011 が行っている「生トークンが `load` を通っても変わらない」検証が無い。読み込み時にトークンを壊す実装でも通る。(b) `yaml.rs` の重複キーのテストは `error.message.contains("duplicate")` で、同モジュール冒頭が「YAML クレートを差し替えても影響はこのモジュールに閉じる」と宣言しているのに、サードパーティの英語メッセージに依存している。(c) `task_file.rs` の未知キーのテストは `is_err()` のみで、テスト名が主張する「未知キーが原因」を確かめていない
  - 提案: (a) は生トークンの等値比較を足す。(b) は `WorkflowParseError::YamlSyntax` になることと位置が付くことだけを見る。(c) はメッセージにキー名が含まれることを確かめる

## カバレッジ

### 確認

適合テストの枠組みとケース（18件）:

- `crates/pulsen-conformance/Cargo.toml`, `crates/pulsen-conformance/HOOKS.md`, `crates/pulsen-conformance/src/lib.rs`, `crates/pulsen-conformance/src/clock.rs`, `crates/pulsen-conformance/src/config_store.rs`, `crates/pulsen-conformance/src/exclusive_lock.rs`, `crates/pulsen-conformance/src/task_id_generator.rs`, `crates/pulsen-conformance/src/task_repository.rs`, `crates/pulsen-conformance/src/workflow_store.rs`, `crates/pulsen-conformance/src/worktree_manager.rs`, `crates/pulsen-conformance/src/doubles/mod.rs`, `crates/pulsen-conformance/src/doubles/clock.rs`, `crates/pulsen-conformance/src/doubles/lock.rs`, `crates/pulsen-conformance/src/doubles/stores.rs`, `crates/pulsen-conformance/src/doubles/task_id.rs`, `crates/pulsen-conformance/src/doubles/task_repository.rs`, `crates/pulsen-conformance/src/doubles/tests.rs`, `crates/pulsen-conformance/src/doubles/worktree.rs`

統合テストとフィクスチャ（15件）:

- `crates/pulsen/tests/cli_add_normal.rs`, `crates/pulsen/tests/cli_add_error.rs`, `crates/pulsen/tests/cli_add_boundary.rs`, `crates/pulsen/tests/cli_usage.rs`, `crates/pulsen/tests/register_task.rs`, `crates/pulsen/tests/common/mod.rs`, `crates/pulsen/tests/common/git.rs`, `crates/pulsen/tests/common/lock.rs`, `crates/pulsen/tests/conformance_config_store.rs`, `crates/pulsen/tests/conformance_lock.rs`, `crates/pulsen/tests/conformance_task_repository.rs`, `crates/pulsen/tests/conformance_time_id.rs`, `crates/pulsen/tests/conformance_workflow_store.rs`, `crates/pulsen/tests/conformance_worktree.rs`, `crates/pulsen/examples/lock_holder.rs`

実装コード — **`#[cfg(test)] mod tests` とテストからの到達可能性の観点で確認**（32件）:

- `crates/pulsen-domain/src/lib.rs`, `crates/pulsen-domain/src/definition/mod.rs`, `agent.rs`, `assembler.rs`, `command.rs`, `config.rs`, `duration.rs`, `name.rs`, `port.rs`, `reference.rs`, `snapshot.rs`, `template.rs`, `validator.rs`, `workflow.rs`
- `crates/pulsen-domain/src/task/mod.rs`, `attempt.rs`, `branch.rs`, `counters.rs`, `degraded.rs`, `failure.rs`, `id.rs`, `path.rs`, `port.rs`, `process.rs`, `state.rs`, `task.rs`, `time.rs`
- `crates/pulsen-domain/src/execution/mod.rs`, `crates/pulsen-domain/src/execution/port.rs`
- `crates/pulsen/src/util/mod.rs`, `crates/pulsen/src/util/atomic.rs`, `crates/pulsen/src/util/fsdir.rs`

実装コード — テストとの対応（期待値・到達可能性）を追うために読んだもの（16件）:

- `crates/pulsen/src/adapter/mod.rs`, `clock.rs`, `config_store.rs`, `lock.rs`, `task_file.rs`, `task_id.rs`, `task_repository.rs`, `workflow_store.rs`, `worktree.rs`, `yaml.rs`
- `crates/pulsen/src/application/mod.rs`, `home.rs`, `register_task.rs`
- `crates/pulsen/src/cli/wire.rs`, `render.rs`, `exit.rs`

ビルド・設定（5件）:

- `Cargo.toml`（ワークスペース・lints）, `crates/pulsen/Cargo.toml`（dev-dependencies と tempfile の位置づけ）, `crates/pulsen-domain/Cargo.toml`（依存が空であること）, `flake.nix`（devShell への `git` 追加＝ worktree 適合テストの前提）, `rustfmt.toml`

計画・記録（5件）:

- `.thread/1/plan.md`（受け入れ基準・テスト方針の照合）, `.thread/1/review/triage.md`（wont-fix の把握）, `.thread/1/progress.md`（スキップ運用の記録）, `.thread/1/testing.md`（手動確認手順）, `.adr/046-no-skippable-hooks-for-post-operation-observation.md`

### スキップ

- `crates/pulsen/src/cli/mod.rs`, `crates/pulsen/src/cli/add.rs`, `crates/pulsen/src/cli/args.rs`, `crates/pulsen/src/lib.rs`, `crates/pulsen/src/main.rs` — 引数定義と結線の薄い層。振る舞いは `tests/cli_usage.rs` / `cli_add_*.rs` 側から観測しており、実装の構造は usecase-cli 観点の担当
- `Cargo.lock` — 生成物。依存の選定は adapter / arch 観点の担当
- `.adr/019-domain-crate-workspace.md`, `.adr/020-no-serde-in-domain-timestamp-conversion-in-domain.md`, `.adr/021-yaml-value-then-hand-written-schema-walk.md`, `.adr/022-std-file-lock-and-lockguard-marker-trait.md`, `.adr/023-dependency-selection.md`, `.adr/024-git-cli-shell-out-and-target-classification.md`, `.adr/025-task-file-json-and-corrupt-classification.md`, `.adr/026-task-id-format.md`, `.adr/027-port-conformance-suite-and-harness-hooks.md`, `.adr/028-usecase-error-paths-via-test-doubles.md`, `.adr/029-wildcard-enum-match-arm-lint-domain-only.md`, `.adr/030-workflow-store-base-dir-injection.md`, `.adr/031-pulsen-home-layout-in-application-layer.md`, `.adr/032-lock-holder-example-fixture.md`, `.adr/033-git-fixture-reproducibility.md`, `.adr/034-workflow-ref-separator-set-as-constant.md`, `.adr/035-file-slice-adrs-from-019.md`, `.adr/036-infallible-ports-absorb-failure-at-construction.md`, `.adr/037-platform-separator-set-without-cfg.md`, `.adr/038-adr-filing-format.md`, `.adr/039-read-error-shared-by-find-and-list.md`, `.adr/040-rehydrate-takes-field-bundle.md`, `.adr/042-absent-yaml-value-is-omission.md`, `.adr/043-store-adapters-receive-injected-paths.md`, `.adr/044-task-file-layout-in-domain.md`, `.adr/045-task-file-dto-generic-over-snapshot.md`, `.adr/048-parse-inputs-at-spec-flow-position.md`, `.adr/049-base-flag-allows-hyphen-values.md`, `.adr/050-schema-error-location-is-logical.md`, `.adr/051-undisplayable-name-fixture-is-whitespace-stem.md`, `.adr/052-acceptance-test-harness-in-tests-common.md`, `.adr/053-conformance-yaml-source-hooks.md`, `.adr/054-workflow-error-file-path-goes-into-free-form-messages.md` — ADR の起票と内容の妥当性は arch-spec 観点の担当。テストの前提に直接効く 027 / 032 / 033 / 046 / 053 は本文中で参照しつつ、記録としての評価は行っていない（046 のみ全文を確認）
- `.thread/1/adr.md`, `.thread/1/steps.md` — 設計判断とステップ分割の記録。テストの合否に影響しない
- `.thread/1/review/changed-files-001.txt`, `.thread/1/review/review-001.md`, `.thread/1/review/review-001-test.md`, `.thread/1/review/review-001-adapter.md`, `.thread/1/review/review-001-arch-spec.md`, `.thread/1/review/review-001-domain.md`, `.thread/1/review/review-001-usecase-cli.md` — 1周目の成果物。指示に従い判定は `triage.md` で把握し、原文は参照していない
