# 最終レビュー（4周目 / 収束判定） — Issue #1 / PR #8

**前提**: 1周目の `wont-fix` 3件（W-004 `unreachable!` / W-012 成功時の tick 案内 / W-017 `create` の TOCTOU）は蒸し返していない。本ラウンドは「もう直すべきものが残っていないこと」の確認に絞り、好みの提案・スタイル・「あったほうが良い」レベルの改善は出していない。

## 結論

**Blocker 0 / Warning 3。** 3周目の修正（`allowed_skips()` / `StopOnDrop` / HOOKS.md 区分 C / ADR 欠番の明記 / `describe_initial_not_found` 系）はいずれも意図どおり効いており、**回帰は0件**である。AC-1〜AC-20 はすべて合格した。

残った3件はいずれも低い深刻度で、しかも **本スライスの利用者に見える誤動作は起こさない**。ただし1件は後続スライスが継承する契約の非対称（W-001）、2件は「主張を検証していないテスト」（W-002 / W-003）であり、放置すると後から入る回帰を検出できない。1周目 W-004 / W-012 / W-017、2周目 R2-W-027 の判断とも整合する種類の指摘なので記録する。

## Blockers

なし。

## Warnings

- **[W-001]** `find` と `list_active` が「エントリは残っているが内容へ到達できないファイル」を別の区分に写す
  - **場所**: `crates/pulsen/src/adapter/task_repository.rs:57-77`（`lookup`）、`:79-151`（`list`）、`:50-55`（`exists`）
  - **理由**: 2周目 R2-W-012 の修正で `list` は `fs::read` の `NotFound` を `symlink_metadata` で切り分け、エントリが残っていれば `TaskEntry::Corrupt` を報告するようになった（`:113-135`。アダプター自身のユニットテスト `:274-291` が固定している）。一方 `lookup`（`:59-63`）は同じ状況を `fs::read` の `NotFound` だけで判定して `Ok(None)` に写すため、`find` は `TaskLookup::NotFound` を返す。`spec/domains/task.md` の結果型は `TaskLookup::Corrupt` を「ファイル全体が読めない」と定義しており、宙ぶらりんのリンクはその状況に当たる。`spec/testcases/ports/task-repository.md` の `list_active` 行も「`find` と同じ区分で列挙される」と書く。加えて `create` の存在確認（`:53-55` の `try_exists` はリンクを辿るので `Ok(false)`）は同じパスを衝突と見なさず上書きするため、`:50-52` の doc が掲げる「破損したファイルも『その ID は使われている』ことの証拠であり、上書きすると修復材料が消える」がこの1ケースで成り立たない（同一 ID の再発行が要るので実害は理論上）。
  - **失敗シナリオ**: 手動修復の途中で `state/tasks/<task-id>.json` が実体を失ったリンクになる。`list_active()` は `Corrupt { path }` を1件返すのに、`find(<task-id>)` は `NotFound` を返す。`ls` が「このパスのファイルが読めない」と報告した ID を `show` / `abort` / `retry` が「存在しない」と答える。
  - **提案**: `lookup` の `NotFound` 分岐にも `symlink_metadata` の切り分けを入れ、エントリが残っているなら `TaskLookup::Corrupt { path, message }` を返す（`list` と同じ述語）。あわせて `exists` を「エントリの有無」を見る形にすると `create` の doc の主張とも揃う。契約 doc（`crates/pulsen-domain/src/task/port.rs:125-127`）の「走査中の消失」は現在 `list` だけを対象にしているので `find` を含む形に書き直し、spec 追従の提起として Issue コメントに残す。判断は ADR に残す価値がある（続き番号は 064）。
  - **注**: 本スライスに `find` / `list_*` の呼び出し元は無い（`grep` で application / cli とも0件）ため、現時点で利用者に見える影響は無い。

- **[W-002]** TC-010 のテスト名が主張する「元 yaml の編集に影響されない」を、どのアサーションも検証していない
  - **場所**: `crates/pulsen/tests/cli_add_normal.rs:193-216`
  - **理由**: `:204` で読んだタスクファイルと `:215` で読み直すタスクファイルの間で `pulsen` は1回も起動せず、`state/tasks/*.json` を書くものが何も走らない。`:210-213` の YAML 書き換えはタスクファイルに触れないので、`:215` は同じ内容を読み直しているだけで**どの実装でも真になる**。テストが実際に守っているのは `:205-208`（登録時のプロンプトが埋め込まれていること）だけで、ADR-015 が主張する独立性は無検証。
  - **失敗シナリオ**: `common/mod.rs` の `WORKFLOW` が推敲されて `"prompt: 実装して"` の綴りが変わると `replace` が無音の no-op になり、「編集する」手順そのものが消えても緑のまま通る。**実測で確認**: `replace` の検索文字列を存在しない文字列に差し替えて実行すると TC-010 は `ok` で通った（差し替えは復元済み、`git diff` は空）。
  - **提案**: (a) 書き換えの前後で定義ファイルのバイト列が実際に変わったことを `assert_ne!` で確かめる、かつ (b) 書き換え後にもう一度 `pulsen add` を実行し、**新しいタスクには新しいプロンプトが入り、既存のタスクは旧プロンプトのまま**であることを比べる。(b) は本スライスの `add` だけで組めて、「スナップショットは登録時点で固定される」という主張を実際に落とせる形になる。

- **[W-003]** TC-009 の3つの `Value::Null` 比較と `task_status` の比較が、キーが存在しなくても通る
  - **場所**: `crates/pulsen/tests/cli_add_normal.rs:183, 188-190`
  - **理由**: `serde_json::Value` の `Index<&str>` はキーが無いとき `Value::Null` を返す（実測で確認）。したがって `task["workspace"] == Value::Null` は「`workspace` が `null` として書かれている」ではなく「`workspace` が `null` か、または**書かれていない**」しか主張しない。`:183` の `task["task_status"] == task["snapshot"]["initial"]` は**両方が不在でも真**になるため、AC-17 が求める「スナップショットが埋め込まれている」ことをこのテストは落とせない。
  - **失敗シナリオ**: `TaskFileDto`（`adapter/task_file.rs:47-49`）の `workspace` / `current_attempt` / `last_failure` に `skip_serializing_if = "Option::is_none"` が付く、あるいはフィールド名が変わると、タスクファイルからキーが消えても TC-009 は通る。`state/tasks/*.json` は「人間が直接閲覧・修復する」ことが前提（requirements §9）なので、キーの不在は利用者に見える退化になる。
  - **提案**: キーの存在を先に確かめてから値を比べる（`task.get("workspace").expect(...)` 相当）。`:183` は `cli_add_boundary.rs:229`（TC-059）と同じくリテラル（`"queued"`）に対して固定し、あわせて `assert!(task["snapshot"].is_object())` を置く。

## 検討して指摘しなかったもの（記録）

判断の理由を残す。再審議の対象にしないため。

1. **走査中の個別ファイルの `EACCES` が走査全体を失敗させる**（`task_repository.rs:137-141`）。spec は「走査対象ディレクトリが読み取り不能 → `Err(Io)`」と「個別の**破損** → `Corrupt`」しか定めておらず、ファイル単位の権限エラーは未規定。機構の失敗を値のエラーとして届ける現在の形は spec/testcases の `find` 行「機構失敗は値のエラーとして呼び出し側に届く」と整合するため、欠陥とは判定しない。
2. **`archive` が同一 ID のアーカイブ済みファイルを黙って上書きする**（`task_repository.rs:242-253`）。この状態は `create` が `Conflict` で防ぐのでポート経由では到達できない（spec が用意する「双方に置く」フィクスチャは手動配置）。`ArchiveError` は spec 上 `NotFound` / `Io` の2種で閉じており、拒否の区分を足すと AC-7 を壊す。
3. **TC-020（`cli_add_error.rs:157-172`）が、定義ファイルを置くはずだった一時ディレクトリを `Untouched` の台帳に入れていない**。AC-15 が名指しするのは「ワークフロー定義ファイルと config.yaml」で、ホーム外の空ディレクトリは対象外。TC-034 が `.with_entries` で外部の定義ファイルを控えているのは、そこに**実在する**定義があるためで、扱いが割れているわけではない。
4. **`crates/pulsen-domain/src/task/port.rs:125-127` の「走査中の消失」に spec の対応行が無い**。2周目 R2-W-012 の方針が明示的に「ポート契約レベルの決定を port.rs の契約 doc にも書く」と決めたもの。spec 追従の提起は W-001 の修正に合わせて行うのが自然で、単独の欠陥ではない。
5. **`WorkflowStructureError::describe_initial_not_found` / `describe_next_not_found` を CLI が呼ぶこと**（3周目 R3-W-005 の修正）。ドメインが持つのは「破れた不変条件の説明」であり、CLI は末尾の句点だけを足す（`render.rs:195-198, 215-218`）。整形・見出し・箇条書きといった表示の都合はすべて `render.rs` にあり、ドメインは CLI を知らない。依存方向の破れではない。
6. **`AttemptNumber::next()` の飽和**・**`RawCommand` が配列形式で空の先頭トークンを許すこと**・**`agent.rs` が prompt のみのワークフローでも `skill_input` を検証すること** — いずれも spec の記述どおり、または spec 側の隙間であり、実装の誤りではない。

## 受け入れ基準の検証

いずれも実際にコマンドを実行して確かめた。実行環境: macOS（Darwin 25.4.0）・非 root・TMPDIR はリポジトリ外。

| # | 判定 | 根拠 |
|---|---|---|
| AC-1 | 合格 | `cargo build` / `cargo test` / `cargo clippy --all-targets -- -D warnings` / `cargo fmt --check` がすべて成功。`crates/pulsen-domain/Cargo.toml` の `[dependencies]` は空。`grep -rn 'cfg(unix)\|cfg(windows)' crates/*/src/` は2件で、`pulsen-domain` に0件、`crates/pulsen/src/` 側は `util/atomic.rs` の `sync_dir` だけ（もう1件は `pulsen-conformance/src/lib.rs` の probe） |
| AC-2 | 合格 | ドメインのユニットテスト167件が通る。`parse` 経由の生成のみ（フィールドは全て非公開）、`NameError` / `DurationError` / `CommandError` / `TemplateError` / `ExpansionError` / `AgentDefError` の全分岐にテストがある |
| AC-3 | 合格 | `definition/workflow.rs:175-227` の `effective_*` が「ステータス上書き > ワークフローデフォルト > 既定」。`retries: Some(0)` が 0 のまま保たれる。`reference.rs` の `POSIX_SEPARATORS` / `WINDOWS_SEPARATORS` が定数として公開され、`/` と `\` を明示的に渡すテストがある |
| AC-4 | 合格 | `assembler.rs:176-424` が10種を返す（`YamlSyntax` / `UnknownKey` はアダプター生成という spec の分担どおり）。循環・自己参照・到達不能は `workflow.rs:121-133` が `next ∈ statuses` しか見ないため受理される |
| AC-5 | 合格 | `validator.rs:53-133` は短絡せず全件を積む。`tests/register_task.rs:694` が2ステータス3件（`MissingSkillInput` + `MissingModel`×2）を要素まで固定 |
| AC-6 | 合格 | `Task::register` / `rehydrate` / `DegradedTask` があり、`rehydrate`（`task.rs:103-124`）が `StatusNotInSnapshot` を返す。`ExecutionState` 6値。`Timestamp` の RFC3339 往復・閏年・不正日付・表現可能範囲の両端がテスト済み |
| AC-7 | 合格 | ポート表と1:1（`TaskRepository` 7メソッド / `TargetError` 5種 / `ConfigLoadError` 3種 / `WorkflowLoadError` 3種 / `LockError`）。`RunStore` / `ProcessController` / `CommandRunner` と `WorktreeManager::create` / `remove` は存在しない。`todo!` / `unimplemented!` / スタブは `crates/` 全体で0件 |
| AC-8 | 合格 | `pulsen-conformance` が独立クレート。125ケース = 125 `#[test]`（config-store 24 / workflow-store 31 / task-repository 44 / clock 5 / task-id-generator 5 / exclusive-lock 7 / worktree-manager 9）。`HOOKS.md` に125行×フックの対応表（A 28 / B 85 / C 12、表の行数と一致）。原子性の3件は `concurrent_repo` フック経由 |
| AC-9 | 合格 | `conformance_config_store.rs` 24件が通る |
| AC-10 | 合格 | `conformance_workflow_store.rs` 31件が通る |
| AC-11 | 合格 | `conformance_task_repository.rs` 44件が通る |
| AC-12 | 合格 | Clock 5 + TaskIdGenerator 5 + ExclusiveLock 7 + WorktreeManager 9 = 26件が実装され、`tc_port_clock_005` の1件だけがスキップ（`SKIP` 行として出力される）。残り25件が通る |
| AC-13 | 合格 | 実バイナリで確認: `--home` 指定で登録成功、`--home` を外し `PULSEN_HOME` だけで登録成功、未初期化ホームでは「未初期化・解決後のホームパス・config.yaml の作成が必要」を表示して exit 1、かつホームのディレクトリを作らない |
| AC-14 | 合格 | `application/register_task.rs:127-184` が spec の順序（ロック → 解決 → 表示名 → 対象検証 → 登録時検証 → ID発行 → `create`）。`Conflict` は `retried` フラグで1回だけ再試行。成功時にタスクID・ワークフロー名・解決先を表示して 0 |
| AC-15 | 合格 | `cli_add_error.rs` の31件は `reject` / `reject_definition` / `reject_target` か各ケース内で `assert_unchanged()` を通る。`cli_add_boundary.rs` の拒否4件（TC-053 / 054 / 055 / 058）も同様。`Untouched` は内容とディレクトリ直下のエントリの顔ぶれ（`state` を除く）の両方を見る |
| AC-16 | 合格 | 受理側 TC-049〜052 / 056 / 057 / 059〜067 がすべて実装され通る（`cli_add_normal.rs` 12件 + `cli_add_boundary.rs` の受理ケース） |
| AC-17 | 合格 | 実バイナリの出力を確認。`state/tasks/<task-id>.json` が人間可読な JSON で、`snapshot` を埋め込み、`task_status: "queued"`（= initial）・`execution.state: "pending"`・カウンタ全0・`workspace` / `current_attempt` / `last_failure` が `null`。`state/` は自動作成される |
| AC-18 | 合格 | `tests/register_task.rs` に `tc_task_register_task_012 / 018 / 040 / 047 / 048` の5件があり、ダブル（`pulsen-conformance::doubles`）だけで通る。実プロセス・実ファイルシステムを使わない |
| AC-19 | 合格 | `NamedTempFile` / `persist` / `fs::rename` の出現は `crates/pulsen/src/util/atomic.rs` のみ（`grep` で確認）。`File::try_lock` は `adapter/lock.rs:46` の1箇所のみ |
| AC-20 | 合格 | Issue #1 のチェックリスト346行のうち `- [x]` が345行、`- [ ]` が1行（`TC-port-clock-005`）。理由は Issue のコメント（全10件）に投稿済み。サンプル確認（`DOM-execution-059〜061 / 065 / 068〜070`、`ADP-worktree-001〜003`、`ADP-taskrepo-001〜007`、`PAGE-add-001〜010`、`PAGE-common-002 / 003 / 005 / 007 / 010 / 011`、`UC-flow-007`、`DOM-definition-030 / 031 / 049`、`DOM-task-078 / 079`）はいずれも実装とテストが実在する |

## 3周目の修正の確認

| ID | 確認内容 | 判定 |
|---|---|---|
| R3-W-001 | `tests/conformance_worktree.rs:119-125` の `allowed_skips()` が `common::git::tmpdir_outside_repository()`（`common/git.rs:90-94` の `OnceLock` probe）で分岐し、偽のときだけ `tc_port_worktree_manager_003` を許容集合に入れる。CLI 側 TC-036（`common/mod.rs:35, 49-51`）と同一の述語 | 効いている |
| R3-W-002 | `StopOnDrop` の効き目を実測で確認した。`write_atomic` を一時的に「4回目以降 `Err`」に差し替えると、TC-042 は **0.09 秒で FAILED**（ハングしない）。`write_atomic` を非アトミックな逐次書き込みに差し替えると、TC-042 は **`Corrupt` を観測して FAILED** — 従来検出できていた欠陥を今も検出できる。差し替えは検証後に復元し、`git diff` が空であることを確認済み。`util/atomic.rs:169-175` にも同型のガードがある | 効いている（ハングせず、検出力も保たれる） |
| R3-W-003 | `HOOKS.md:24-38` が「環境で走らなくなりうる行」の節になり、12行 + 区分 B の worktree-manager-003 を「前提を作れない環境」「判定」の表にしている。:37 の worktree-manager-009 に `repo_with_commit` / `head_branch_name` が入っている。区分の集計 A 28 / B 85 / C 12 は表の行数（`awk` で数えて 28 / 85 / 12）と一致 | 効いている |
| R3-W-004 | `.thread/1/adr.md:5` に ADR-056〜059 が欠番である旨と理由がある。`.adr/035-file-slice-adrs-from-019.md` の「決定」にも同じ1行がある | 効いている |
| R3-W-005 | `definition/workflow.rs:53-85` に `describe()` / `describe_initial_not_found` / `describe_next_not_found` があり、`cli/render.rs:195-198, 215-218` と `adapter/task_file.rs:603` の両方がそれを経由する。文言の定義箇所は1つ | 効いている |
| — | `.adr/063-concurrent-observation-stops-the-reader-on-unwind.md` が `承認済み` で存在し、`.thread/1/adr.md:1133` の ADR-063 と対応する | 効いている |

## 横断の確認

- **依存方向**: `crates/pulsen/src/application/` に `crate::adapter` の参照は0件。`crate::adapter` を import するのは `crates/pulsen/src/cli/wire.rs` の1箇所だけ。`std::env::var_os` / `env::current_dir` / `env::home_dir` の呼び出しも `wire.rs` に閉じている（合成ルートは1箇所）。`pulsen-domain` の `[dependencies]` は空
- **関数型ドメインモデリング**: フィールドは非公開、生成は `parse`（制約のない `InputText` のみ総関数 `new`。ADR 相当の記録は `progress.md` と Issue コメント）。ドメイン enum に対する `match` のワイルドカードは0件（`_ =>` の出現は `char` / `&str` / `i64` に対するもののみ）。パニックは `template.rs:198` の `unreachable!` 1件で、1周目に `wont-fix` と判定済み
- **spec 適合**: `spec/usecases/task.md:35-42` の8段と `register_task.rs:127-184` が1対1。`spec/pages/index.md:13` の exit code 規約と `cli/exit.rs` が一致（`--help` は 0、使い方の誤りは 2）
- **テストケースの網羅**: 適合125件（実装125 = spec の対象7ポート分。`worktree-manager` の `create` / `remove` 12行と `command-runner` / `process-controller` / `run-store` は plan.md「含まれないもの」に該当）、`register-task` 67件（TC-001〜067 に欠番・重複なし。うち5件がユースケース層、62件が CLI）
- **`unwrap` / `expect` / `panic!`**: `crates/pulsen/src` と `crates/pulsen-domain/src` の本番経路に0件（`grep` のヒットはすべて `#[cfg(test)]` 配下、または `unwrap_or` / `unwrap_or_else`）。`todo!` / `unimplemented!` / `FIXME` / `TODO` / `XXX` は `crates/` 全体で0件
- **スコープ**: `.thread/1/plan.md`「含まれないもの」の混入なし（`tick` / `wrapper` / `ls` / `show` / `abort` / `retry` / `set-status` のサブコマンド、`RunStore` / `ProcessController` / `CommandRunner`、`WorktreeManager::create` / `remove`、execution のドメインサービス、汎用 in-memory アダプター、CI 設定はいずれも不在）
- **コメントの質**: `crates/` 全体を「指摘 / レビュー / 以前は / もともと / 修正した / 変えた点 / N周目」で grep して0件。修正の経緯・弁明は出荷物に漏れていない
- **フレーキー / ハング / 並列干渉**: `cargo test` を3回連続・`--test-threads=1`・`RUST_TEST_THREADS=32` で実行し、いずれも全件成功で結果が一致。`conformance_task_repository`（並行観測2件を含む）を8回連続実行しても44件成功。受け入れテストは環境変数を `Command` に設定してプロセスグローバルには触らず、ホームは毎回一時ディレクトリ（`detached_home`。ADR-062）なので並列でも干渉しない
- **ADR の整合**: `.adr/` は 001〜063（041 / 047 は既存エントリへ反映済み、056〜059 は欠番と明記）。`crates/` から参照される ADR 番号26種はすべて `.adr/` に実在する。`.thread/1/adr.md` の Status 行はすべて `Accepted` でファイル名を持ち、`.adr/` の実体と双方向に一致する。既存 `.adr/001`〜`018` との矛盾は見当たらない（ADR-010 の循環許容、ADR-013 の未知キー拒否、ADR-014 の既定値、ADR-015 のスナップショット埋め込み、ADR-017 のドメイン境界はいずれも実装と一致）

## カバレッジ

変更ファイル一覧155件と1対1で対応する。

### 確認（155件）

**`.adr/`（39件）** — 全件の `## ステータス` が `承認済み` であることと、`crates/` から参照される番号の実在を機械的に確認。`035` / `063` は本文まで通読、`024` / `030` / `032` は実装との照合のため該当節を通読:
`.adr/019-domain-crate-workspace.md`, `.adr/020-no-serde-in-domain-timestamp-conversion-in-domain.md`, `.adr/021-yaml-value-then-hand-written-schema-walk.md`, `.adr/022-std-file-lock-and-lockguard-marker-trait.md`, `.adr/023-dependency-selection.md`, `.adr/024-git-cli-shell-out-and-target-classification.md`, `.adr/025-task-file-json-and-corrupt-classification.md`, `.adr/026-task-id-format.md`, `.adr/027-port-conformance-suite-and-harness-hooks.md`, `.adr/028-usecase-error-paths-via-test-doubles.md`, `.adr/029-wildcard-enum-match-arm-lint-domain-only.md`, `.adr/030-workflow-store-base-dir-injection.md`, `.adr/031-pulsen-home-layout-in-application-layer.md`, `.adr/032-lock-holder-example-fixture.md`, `.adr/033-git-fixture-reproducibility.md`, `.adr/034-workflow-ref-separator-set-as-constant.md`, `.adr/035-file-slice-adrs-from-019.md`, `.adr/036-infallible-ports-absorb-failure-at-construction.md`, `.adr/037-platform-separator-set-without-cfg.md`, `.adr/038-adr-filing-format.md`, `.adr/039-read-error-shared-by-find-and-list.md`, `.adr/040-rehydrate-takes-field-bundle.md`, `.adr/042-absent-yaml-value-is-omission.md`, `.adr/043-store-adapters-receive-injected-paths.md`, `.adr/044-task-file-layout-in-domain.md`, `.adr/045-task-file-dto-generic-over-snapshot.md`, `.adr/046-no-skippable-hooks-for-post-operation-observation.md`, `.adr/048-parse-inputs-at-spec-flow-position.md`, `.adr/049-base-flag-allows-hyphen-values.md`, `.adr/050-schema-error-location-is-logical.md`, `.adr/051-undisplayable-name-fixture-is-whitespace-stem.md`, `.adr/052-acceptance-test-harness-in-tests-common.md`, `.adr/053-conformance-yaml-source-hooks.md`, `.adr/054-workflow-error-file-path-goes-into-free-form-messages.md`, `.adr/055-conformance-skip-budget.md`, `.adr/060-non-blocking-lock-case-observes-from-a-second-thread.md`, `.adr/061-unused-public-accessors-are-kept-only-for-verified-layout.md`, `.adr/062-acceptance-tests-detach-the-user-home.md`, `.adr/063-concurrent-observation-stops-the-reader-on-unwind.md`

**`.thread/1/`（5件）**: `.thread/1/adr.md`, `.thread/1/plan.md`, `.thread/1/progress.md`, `.thread/1/steps.md`, `.thread/1/testing.md`

**`.thread/1/review/`（19件）** — 過去3ラウンドの入力と台帳。`triage.md` は全文、他は判定の照合に使用:
`.thread/1/review/changed-files-001.txt`, `.thread/1/review/changed-files-002.txt`, `.thread/1/review/changed-files-003.txt`, `.thread/1/review/review-001.md`, `.thread/1/review/review-001-adapter.md`, `.thread/1/review/review-001-arch-spec.md`, `.thread/1/review/review-001-domain.md`, `.thread/1/review/review-001-test.md`, `.thread/1/review/review-001-usecase-cli.md`, `.thread/1/review/review-002-adapter.md`, `.thread/1/review/review-002-arch-spec.md`, `.thread/1/review/review-002-domain.md`, `.thread/1/review/review-002-test.md`, `.thread/1/review/review-002-usecase-cli.md`, `.thread/1/review/review-003.md`, `.thread/1/review/review-003-adapter-test.md`, `.thread/1/review/review-003-arch-spec.md`, `.thread/1/review/review-003-domain-usecase-cli.md`, `.thread/1/review/triage.md`

**ワークスペース設定（4件）**: `Cargo.toml`, `Cargo.lock`, `flake.nix`, `rustfmt.toml`

**`crates/pulsen-domain/`（30件）**: `crates/pulsen-domain/Cargo.toml`, `crates/pulsen-domain/src/lib.rs`, `crates/pulsen-domain/src/definition/mod.rs`, `crates/pulsen-domain/src/definition/agent.rs`, `crates/pulsen-domain/src/definition/assembler.rs`, `crates/pulsen-domain/src/definition/command.rs`, `crates/pulsen-domain/src/definition/config.rs`, `crates/pulsen-domain/src/definition/duration.rs`, `crates/pulsen-domain/src/definition/name.rs`, `crates/pulsen-domain/src/definition/port.rs`, `crates/pulsen-domain/src/definition/reference.rs`, `crates/pulsen-domain/src/definition/snapshot.rs`, `crates/pulsen-domain/src/definition/template.rs`, `crates/pulsen-domain/src/definition/validator.rs`, `crates/pulsen-domain/src/definition/workflow.rs`, `crates/pulsen-domain/src/execution/mod.rs`, `crates/pulsen-domain/src/execution/port.rs`, `crates/pulsen-domain/src/task/mod.rs`, `crates/pulsen-domain/src/task/attempt.rs`, `crates/pulsen-domain/src/task/branch.rs`, `crates/pulsen-domain/src/task/counters.rs`, `crates/pulsen-domain/src/task/degraded.rs`, `crates/pulsen-domain/src/task/failure.rs`, `crates/pulsen-domain/src/task/id.rs`, `crates/pulsen-domain/src/task/path.rs`, `crates/pulsen-domain/src/task/port.rs`, `crates/pulsen-domain/src/task/process.rs`, `crates/pulsen-domain/src/task/state.rs`, `crates/pulsen-domain/src/task/task.rs`, `crates/pulsen-domain/src/task/time.rs`

**`crates/pulsen-conformance/`（18件）**: `crates/pulsen-conformance/Cargo.toml`, `crates/pulsen-conformance/HOOKS.md`, `crates/pulsen-conformance/src/lib.rs`, `crates/pulsen-conformance/src/clock.rs`, `crates/pulsen-conformance/src/config_store.rs`, `crates/pulsen-conformance/src/exclusive_lock.rs`, `crates/pulsen-conformance/src/task_id_generator.rs`, `crates/pulsen-conformance/src/task_repository.rs`, `crates/pulsen-conformance/src/workflow_store.rs`, `crates/pulsen-conformance/src/worktree_manager.rs`, `crates/pulsen-conformance/src/doubles/mod.rs`, `crates/pulsen-conformance/src/doubles/clock.rs`, `crates/pulsen-conformance/src/doubles/lock.rs`, `crates/pulsen-conformance/src/doubles/stores.rs`, `crates/pulsen-conformance/src/doubles/task_id.rs`, `crates/pulsen-conformance/src/doubles/task_repository.rs`, `crates/pulsen-conformance/src/doubles/tests.rs`, `crates/pulsen-conformance/src/doubles/worktree.rs`

**`crates/pulsen/`（40件）**: `crates/pulsen/Cargo.toml`, `crates/pulsen/src/lib.rs`, `crates/pulsen/src/main.rs`, `crates/pulsen/examples/lock_holder.rs`, `crates/pulsen/src/adapter/mod.rs`, `crates/pulsen/src/adapter/clock.rs`, `crates/pulsen/src/adapter/config_store.rs`, `crates/pulsen/src/adapter/lock.rs`, `crates/pulsen/src/adapter/task_file.rs`, `crates/pulsen/src/adapter/task_id.rs`, `crates/pulsen/src/adapter/task_repository.rs`, `crates/pulsen/src/adapter/workflow_store.rs`, `crates/pulsen/src/adapter/worktree.rs`, `crates/pulsen/src/adapter/yaml.rs`, `crates/pulsen/src/application/mod.rs`, `crates/pulsen/src/application/home.rs`, `crates/pulsen/src/application/register_task.rs`, `crates/pulsen/src/cli/mod.rs`, `crates/pulsen/src/cli/add.rs`, `crates/pulsen/src/cli/args.rs`, `crates/pulsen/src/cli/exit.rs`, `crates/pulsen/src/cli/render.rs`, `crates/pulsen/src/cli/wire.rs`, `crates/pulsen/src/util/mod.rs`, `crates/pulsen/src/util/atomic.rs`, `crates/pulsen/src/util/fsdir.rs`, `crates/pulsen/tests/common/mod.rs`, `crates/pulsen/tests/common/git.rs`, `crates/pulsen/tests/common/lock.rs`, `crates/pulsen/tests/cli_add_boundary.rs`, `crates/pulsen/tests/cli_add_error.rs`, `crates/pulsen/tests/cli_add_normal.rs`, `crates/pulsen/tests/cli_usage.rs`, `crates/pulsen/tests/conformance_config_store.rs`, `crates/pulsen/tests/conformance_lock.rs`, `crates/pulsen/tests/conformance_task_repository.rs`, `crates/pulsen/tests/conformance_time_id.rs`, `crates/pulsen/tests/conformance_workflow_store.rs`, `crates/pulsen/tests/conformance_worktree.rs`, `crates/pulsen/tests/register_task.rs`

### スキップ

なし（155件すべてに目を通した）。`Cargo.lock` は生成物のため、依存の集合が `Cargo.toml` の宣言（`clap` / `getrandom` / `serde` / `serde_json` / `serde_yaml_ng` / `tempfile`）と一致することの確認に留めた。
