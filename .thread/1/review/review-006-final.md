# 最終レビュー（6周目・収束確認） — Issue #1 / PR #8

対象: PR #8（ベース `main`、HEAD `70e0c66`）。変更ファイル一覧 `.thread/1/review/changed-files-006.txt` の160件全量。

ラウンド1〜5の台帳（`.thread/1/review/triage.md`）を読み、1周目に `wont-fix` と判定された3件（W-004 `unreachable!` / W-012 成功時の tick 案内 / W-017 `create` の TOCTOU）は本ラウンドでも蒸し返していない。

## 結論

**Blocker 0 / Warning 2**。製品コード（ドメイン・アダプター・アプリケーション・CLI）に残る欠陥はゼロで、AC-1〜AC-20 は全件合格。指摘2件はいずれも**適合スイートを後続スライスの別実装に当てたときにだけ効く層**の問題である。

- W-001: 適合ケース TC-port-exclusive-lock-002 に期限が無く、「解放を待つ」実装に当てるとテストバイナリ全体がハングする（実測300秒で返らず）。ADR-060 が TC-003 で解いた失敗モードの取りこぼし。
- W-002: 出荷ドキュメント `HOOKS.md` の表が5周目の修正に追いついていない。**作業ツリー（未コミット）には既にこの修正が入っている**ため、コミットすれば解消する。

なお、レビュー中に**別セッションが同一リポジトリで並行作業しており**、`crates/pulsen-conformance/HOOKS.md` / `.thread/1/progress.md` / `.thread/1/review/triage.md` の3件が未コミットの変更として作業ツリーに出ている。本レビューの判定は HEAD `70e0c66` に対するもの。

## Blockers

なし。

## Warnings

- **[W-001]** 適合ケース `TC-port-exclusive-lock-002` に期限が無く、「解放を待つ」実装に当てるとテストバイナリ全体がハングする
  - 場所: `/Users/hikaru/github.com/tuanemuy/pulsen/crates/pulsen-conformance/src/exclusive_lock.rs:43-56`（`tc_port_exclusive_lock_002_別プロセスの保持中は取得できない`）／対比: 同ファイル `:59-110`（TC-003）
  - 理由: TC-002 は保持プロセスを起こしたあと、**呼び出しスレッドでそのまま** `harness.lock().try_acquire()` を呼び、`release_holder` はその後ろにある。取得が解放を待つ実装（`std::fs::File::try_lock` の代わりに `lock()` を呼ぶ実装）に当てると、`try_acquire` は保持の解放を待ち、解放は `try_acquire` が返るまで実行されないので**互いを待ってデッドロックする**。すぐ下の TC-003 の doc コメントが `「同じスレッドで経過時間を測ると、解放を待つ実装では try_acquire が返るまで判定に到達せず、ケースが名指しする失敗モードでハングする」` と、この危険を**名指しで説明している**（ADR-060）。TC-003 だけが `thread::scope` + 期限監視で解かれ、同じ形の TC-002 が素のまま残っている。
    - 実測（`crates/pulsen/src/adapter/lock.rs:46` の `file.try_lock()` を `file.lock()` に変異させて計測。検証後に復元済み）: `cargo test --test conformance_lock` は **300秒で返らない**（`conformance_lock-*` と `lock_holder` のプロセスが生存したまま）。TC-002 単体でも **45秒で返らない**。一方 TC-003 単体は **5.65秒で FAILED**（`保持の解放を待たずに返る`）。libtest は同一プロセス内でテストを並列に走らせるため、**TC-003 が正しく赤を出しても TC-002 が返らずバイナリ全体が終わらない**。ADR-060 が掲げた「ハングではなく判定として現れる」が成立していない。
  - 影響の範囲: 現行の `FileExclusiveLock` は `try_lock` を使っており適合しているため、本 PR のテストは緑のまま。効くのは AC-8 が約束する「後続スライスの別実装（in-memory・別プラットフォーム）に同じスイートを当てる」場面で、そこでは診断の無いタイムアウトになる。2周目 R2-W-026（TC-003 のハング）・3周目 R3-W-002（TC-042/044 のハング）と同じ失敗モードの、最後の1箇所。
  - 提案: TC-003 の `thread::scope` + 期限監視を共通ヘルパー（例 `fn attempt_within(lock: &impl ExclusiveLock, limit: Duration) -> Option<Attempt>`）に括り出し、TC-002 も経由させる。`Harness::Lock: Sync` の要求が TC-002 にも広がるが、`FileExclusiveLockHarness` は既に TC-003 のために同じ境界を満たしているのでハーネス側の負担は増えない（ADR-060 の「この1ケースだけの要求」を2ケースへ広げる形になるため、ADR-060 の記述も併せて更新する）。付随して `crates/pulsen/tests/common/lock.rs:36` の `read_line` も期限を持たないので、同じ経路として見ておくとよい。

- **[W-002]** `HOOKS.md` の「環境で走らなくなりうる行」の表が、5周目に確定した `TC-port-exclusive-lock-002 / 003 / 004 / 005` を載せていない（**作業ツリーでは修正済み・未コミット**）
  - 場所: `/Users/hikaru/github.com/tuanemuy/pulsen/crates/pulsen-conformance/HOOKS.md:24-38`（節「環境で走らなくなりうる行」）／対比: `/Users/hikaru/github.com/tuanemuy/pulsen/crates/pulsen/tests/conformance_lock.rs:96-118`
  - 理由: この節は「**何が前提を壊すかは行ごとに違う**。…区分 B にも、フックの前提が環境で成立しない行がある。**宣言を組むときはこの表で読む**」と自ら宣言し、区分 B の行としては `TC-port-worktree-manager-003` の1件だけを挙げている。ところが5周目の R5-W-001 で確定したとおり、`TC-port-exclusive-lock-002 / 003 / 004 / 005` も同じ性質の行である — `ExclusiveLockHarness::hold_from_other_process` / `try_acquire_from_other_process` は `common::lock::holder_program()`（`target/debug/examples/lock_holder` の存在）に依存し、**単一テストターゲットを指定した実行では example がビルドされないため `None` を返す**。実際 `conformance_lock.rs:96-118` はこの4件を `holder_program().is_none()` のときだけ許容集合に入れる形で宣言しており、コードは正しい。しかし `HOOKS.md` の当該表は更新されておらず、**表の指示どおりに宣言を組むと4件の許容が漏れ、R5-W-001 が実測した「単体実行で 4 failed」が再発する**。`HOOKS.md` は ADR-055 / AC-8 が「後続スライスが `allowed_skips` を組むときに読む正本」と位置づけた出荷物であり、3周目の R3-W-003 がこの表を新設した趣旨（区分 C の一般化では足りないので行ごとに書く）に照らしても、実装だけが先行して正本が置き去りになっている。
    - 裏取り: `sed -n '24,38p' crates/pulsen-conformance/HOOKS.md` で表の全9行（対象13 ID）を列挙し、`exclusive-lock-002/003/004/005` が1件も無いことを確認。`HOOKS.md:182-192` の ExclusiveLock 節では当該4件はいずれも区分 B・組み立て手段 `hold_from_other_process` として載っているのみで、環境依存の注記は無い。`grep -n holder_program crates/pulsen/tests/` で `conformance_lock.rs:115` と `common/mod.rs:46` の2箇所が同じ述語を使っていることを確認。
  - 提案: 「環境で走らなくなりうる行」の表に1行足す。例: `| TC-port-exclusive-lock-002 / 003 / 004 / 005 | B | 別プロセスにロックを保持させる実行ファイルが無い（単一テストターゲット指定の実行では example がビルドされない） | ハーネスが hold_from_other_process / try_acquire_from_other_process を提供するか |`。あわせて `HOOKS.md:182-192` の ExclusiveLock 節の TC-002〜005 の行から当該表へ導けるようにする（`.thread/1/progress.md` の「環境によってスキップされるケース」の表も CLI 側 `TC-task-register-task-017` しか載せていないので、同じ1行を足すと記録が揃う）。コードの変更は不要。
  - 状況: **本レビューの実施中に、作業ツリーへまさにこの内容の修正が入った**（`git diff -- crates/pulsen-conformance/HOOKS.md` に上記の1行 + ExclusiveLock 節への導線1文、`.thread/1/progress.md` にも1行）。HEAD `70e0c66` には未反映なので指摘としては残すが、コミットすれば解消する。

## 検証の記録

### 受け入れ基準 AC-1〜AC-20

| # | 判定 | 裏付け |
|---|---|---|
| AC-1 | 合格 | `cargo build` / `cargo test`（全18ターゲット 0 failed）/ `cargo clippy --all-targets -- -D warnings`（警告0）/ `cargo fmt --check` すべて通過。`crates/pulsen-domain/Cargo.toml` の `[dependencies]` は空。`grep -rn 'cfg(unix)\|cfg(windows)' crates/*/src/` は2件のみで、`crates/pulsen-domain/` 0件・`crates/pulsen/src/util/atomic.rs`・`crates/pulsen-conformance/src/lib.rs`（適合ハーネスの probe）。`.thread/1/testing.md:44-48` の手順どおりの結果 |
| AC-2 | 合格 | `crates/pulsen-domain` のユニットテスト 167 passed。名前系・`DurationSpec` / `TimeoutSpec` / `RawCommand` / `CommandTemplate` / `AgentDefinition` はすべてフィールド非公開＋`parse` 経由生成で、各エラー型の全分岐にテストがある（ドメイン層の全29ファイルを spec/domains/definition.md と突き合わせて確認） |
| AC-3 | 合格 | `definition/workflow.rs` の `effective_agent` / `effective_model` / `effective_timeout` / `effective_retry_limit` が「ステータス上書き > ワークフローデフォルト > 既定（1h / 2）」の順で解決。`WorkflowRef::display_name` の4規則と区切り文字集合（`/` と `\` の双方）をユニットテストが明示的に渡している（`definition/reference.rs`、`.adr/034` / `.adr/037`） |
| AC-4 | 合格 | `definition/assembler.rs:20-100` の `WorkflowParseError` は12バリアント。うちアダプター生成の `YamlSyntax` / `UnknownKey` を除く10種（`ForbiddenKey` / `MissingInitial` / `InitialNotFound` / `EmptyStatuses` / `NoAction` / `MultipleActions` / `UnknownRunValue` / `MissingNext` / `NextNotFound` / `InvalidValue`）が spec どおり。循環・自己参照・到達不能ステータスは受理（ADR-010、conformance TC-013/014 で確認） |
| AC-5 | 合格 | `definition/validator.rs` の `RegistrationError` は5バリアント（`MissingAgent` / `UnknownAgent` / `InvalidAgentDefinition` / `MissingSkillInput` / `MissingModel`）。`validator.rs:52` の doc が「最初の1件で打ち切らず全件返す」を契約として書き、`検証エラーは最初の1件で打ち切らず全件返る` / `同一ステータスの複数の不足はまとめて返る` / CLI 側 `登録時検証のエラーは全件まとめて返り登録は行われない` が複数エラーの内訳を assert している |
| AC-6 | 合格 | `task/task.rs` の `Task::register` / `Task::rehydrate`、`task/degraded.rs` の `DegradedTask`、`task/state.rs` の `ExecutionState` 6状態が付随データごと enum。`rehydrate` は不変条件1のみを検証し `RehydrateError::StatusNotInSnapshot` を返す（不変条件2〜4は検証しない = ADR-025）。`task/time.rs` の `Timestamp` は RFC3339 往復・うるう年・表現可能範囲の両端をテストで固定 |
| AC-7 | 合格 | `TaskRepository` 7メソッド（`create` / `save` / `save_degraded` / `find` / `list_active` / `list_archived` / `archive`）、`ConfigStore::load`、`WorkflowStore::load`、`TaskIdGenerator::generate`、`Clock::now`、`WorktreeManager` の検証系3メソッド、`ExclusiveLock::try_acquire` が spec/domains の各ポート表とメソッド名・引数・戻り値まで一致。`TargetError` 5種 / `ConfigLoadError` 3種 / `WorkflowLoadError` 3種 / `LockError` 1種も一致。未実装メソッドの宣言・スタブは0件（`grep -rn 'todo!\|unimplemented!\|FIXME\|TODO\|XXX' crates/` が0ヒット） |
| AC-8 | 合格 | `pulsen-conformance` が独立クレートとして存在し、1ケース = 1 `#[test]`。`HOOKS.md` の対応表が **125行**（ConfigStore 24 / WorkflowStore 31 / TaskRepository 44 / Clock 5 / TaskIdGenerator 5 / ExclusiveLock 7 / WorktreeManager 9）を A 28・B 85・C 12 で全行埋めており、集計と各節の見出しの内訳が一致することを再計算で確認。原子性の3ケース（TC-042〜044）はスキップ可能フック `concurrent_repo` に隔離。生 JSON を渡すフックは無い（YAML ソースを取る2ポートの限定は `.adr/053` と `HOOKS.md:40-45` に明記済み）。※ W-001 / W-002 はいずれも本 AC の「後続スライスの別実装に同じスイートを当てる」場面で効く不備で、本スライスの125件の成立自体は損なわない |
| AC-9 | 合格 | `cargo test --test conformance_config_store` = **24 passed** / 0 failed（単体実行・パッケージ全体とも） |
| AC-10 | 合格 | `cargo test --test conformance_workflow_store` = **31 passed** / 0 failed |
| AC-11 | 合格 | `cargo test --test conformance_task_repository` = **44 passed** / 0 failed |
| AC-12 | 合格 | Clock 5 / TaskIdGenerator 5（`conformance_time_id` = 10 passed）+ ExclusiveLock 7（`conformance_lock` = 7 passed）+ WorktreeManager 9（`conformance_worktree` = 9 passed）= **26件**。`--nocapture` で走行を確認したところ SKIP は `tc_port_clock_005_巻き戻した時刻はそのまま返る` の1件のみで、TC-001〜004 は実走（**25件が実行**）。`head_branch` の4分岐・`TargetError::Failed` の3メソッド到達も緑 |
| AC-13 | 合格 | 実バイナリで実測。未初期化ホームに対し `エラー: グローバルホームが未初期化です。` + `グローバルホーム: <解決後パス>` + `グローバル設定 <path>/config.yaml を作成してください。` を出して exit 1。`--home` / `PULSEN_HOME` 単独のいずれでも登録が成立することを実測（`PULSEN_HOME` 単独でタスクが1件増えた） |
| AC-14 | 合格 | 実バイナリの正常系で `タスクを登録しました。` + タスクID + ワークフロー名 + 解決先パスを表示し exit 0。`application/register_task.rs:127-184` が spec/usecases/task.md の順序（ロック → 解決 → 表示名 → 対象検証 → 登録時検証 → ID発行 → `create`）で並び、`Conflict` は `retried` フラグで1回だけ再発行して再試行 |
| AC-15 | 合格 | `cli_add_error` 31 passed（TC-014〜048 の異常系）+ `cli_add_boundary` の拒否ケース（TC-053/054/055/058）。いずれも `has_no_task()` と `Untouched::assert_unchanged()` の双方を通す。実バイナリでもリポジトリ不在・ベースブランチ不在で非0かつ `state/tasks/` に増分が無いことを確認 |
| AC-16 | 合格 | `cli_add_normal` 12 passed + `cli_add_boundary` 21 passed。TC ID の網羅を機械的に確認（下記） |
| AC-17 | 合格 | 実バイナリで登録したタスクファイルを目視。`state/tasks/<task-id>.json` に整形 JSON で `task_status: "queued"`（= `snapshot.initial`）、`execution.state: "pending"`、`counters` が全0、`workspace` / `current_attempt` / `last_failure` が `null`、`snapshot` に検証済み定義が埋め込まれている。`state/` はフィクスチャが作らず書き込み経路が自動作成 |
| AC-18 | 合格 | `register_task.rs` に `tc_task_register_task_012 / 018 / 040 / 047 / 048` の5件が存在し、テストダブルに対して 22 passed（実プロセス・実ファイルシステム不使用） |
| AC-19 | 合格 | `grep -rn 'fs::rename\|NamedTempFile\|persist(' crates/pulsen/src/ \| grep -v util/atomic.rs` が **0ヒット**、`grep -rn 'try_lock\|FileExt' crates/pulsen/src/ \| grep -v adapter/lock.rs` も **0ヒット**。アトミック置換は `util/atomic.rs`、排他ロックは `adapter/lock.rs` にそれぞれ1箇所 |
| AC-20 | 合格 | `gh issue view 1` のチェックリストは **346行**、`- [x]` が **345件**、`- [ ]` が **1件**。未チェックの1行は `TC-port-clock-005`（時刻を過去に巻き戻せないためこの環境では常にスキップ）で、plan.md「チェックを付けない基準」と一致。`.thread/1/steps.md:425-445` の台帳行 → ステップ対応表も全区分が埋まっている |

### 各テストバイナリの単体実行

**example あり**（`cargo build` 済み・`target/debug/examples/lock_holder` が存在）:

| ターゲット | 結果 |
|---|---|
| `cli_add_boundary` | ok. 21 passed / 0 failed |
| `cli_add_error` | ok. 31 passed / 0 failed |
| `cli_add_normal` | ok. 12 passed / 0 failed |
| `cli_usage` | ok. 5 passed / 0 failed |
| `conformance_config_store` | ok. 24 passed / 0 failed |
| `conformance_lock` | ok. 7 passed / 0 failed |
| `conformance_task_repository` | ok. 44 passed / 0 failed |
| `conformance_time_id` | ok. 10 passed / 0 failed |
| `conformance_workflow_store` | ok. 31 passed / 0 failed |
| `conformance_worktree` | ok. 9 passed / 0 failed |
| `register_task` | ok. 22 passed / 0 failed |

**example なし**（`mv target/debug/examples/lock_holder /tmp/lock_holder.bak` の状態で同じループ）:

11ターゲットすべて上と同一の件数で `0 failed`。FAILED は1件も出ず、`conformance_lock` の TC-002〜005 と `cli_add_error` の TC-017 は許容集合に入ってスキップに落ちる。**5周目の修正（`conformance_lock.rs` の `allowed_skips()`）は正しく効いており、同種の適用漏れは他のターゲットにも無い**。検証後に example を復元済み。

### 5周目の修正の確認（回帰なし）

| 修正 | 確認 |
|---|---|
| `conformance_lock.rs` の `allowed_skips()` | `common::lock::holder_program()` を probe する形で `LOCK_HOLDER_CASES`（TC-002〜005）を宣言。`crates/pulsen/tests/common/mod.rs:36-53` の CLI 側（`PERMISSION_CASES` / `LOCK_HOLDER_CASES` / `OUTSIDE_REPOSITORY_CASES`）と同一の述語になっており、扱いの割れは解消。example の有無どちらでも `0 failed`（上記） |
| `common/lock.rs` の doc | `holder_program()` の doc から「必ず置かれる」の主張が消え、「単一のテストターゲットを指定した実行ではビルドされないため、不在は『前提を作れない環境』として扱う」に置き換わっている。実測どおりの記述 |
| `.adr/025 / 048 / 050 / 054` の見出し | `.adr/001`〜`064` の全64件で見出しを走査。すべて `# NNN: …` / `## ステータス` / `## コンテキスト` /（`## 前提`）/ `## 決定` /（`## 検討した代替案`）/ `## 影響` の並びで、`### 検討した代替案` や平文の `検討した代替案:` は0件。`.adr/025` は `## 検討した代替案` が `## 決定` の後ろに移り、内容の筋も通っている |
| `.thread/1/adr.md` のフック集計 | `:715` が「ポートのみ28件 / フック85件 / spec が明示するスキップ可12件」となり、`HOOKS.md:16-24` の A 28 / B 85 / C 12（合計125）と一致。各ポート節の見出しの内訳（0+0+21+2+4+1+0 = 28、23+30+17+1+1+5+8 = 85、1+1+6+2+0+1+1 = 12）を再計算しても合う |

### spec との適合・網羅

- **適合テスト125件**: `spec/testcases/ports/` の対象7ポートの表を数えると 5 + 24 + 7 + 5 + 44 + 31 + 21 = 137行で、WorktreeManager の `create` / `remove` 12行（本スライス対象外）を引くと **125行**。実装側の `pub fn tc_port_*` はポートごとに 5 / 24 / 7 / 5 / 44 / 31 / 9 = **125件**（`lib.rs:52` のヒットは doc コメント内の例）。テストバイナリの実行件数（24+7+44+10+31+9 = 125）とも一致。
- **register-task 67件**: `spec/testcases/task/register-task.md` の4節（正常系13 / 異常系35 / 境界値11 / エッジケース8 のヘッダを除く実データ行）は 12+34+10+7 … を含めて **67行**。実装側の `tc_task_register_task_NNN` を抽出して `001`〜`067` と突き合わせたところ **欠番0・余剰0・重複0の67件**。うち `register_task.rs`（ユースケース層）が 012 / 018 / 040 / 047 / 048 の5件、残る62件が CLI 受け入れテスト。
- **ドメイン / ポート契約**: `spec/domains/{definition,task,execution}.md` の値オブジェクト・エラー列挙・ポート表と実装が1:1（AC-2〜AC-7 の欄を参照）。
- **`spec/pages/index.md` / `spec/usecases/task.md`**: add の処理順・出力・exit code・ホーム解決・`state/` の自動作成・ロック競合時の「タスクは作られない」がいずれも実装と一致。

### スコープ「含まれないもの」の混入

- `grep -rn 'RunStore\|ProcessController\|CommandRunner\|WorkspacePlanner\|LaunchingClassifier\|RunningClassifier\|JudgementService\|NotificationService\|GcPolicy\|IdentityCheck' crates/*/src/` は **0ヒット**。
- `WorktreeManager` に `create` / `remove` の宣言なし（`execution/port.rs` は3メソッドのみ）。
- サブコマンドは `cli/args.rs:22-25` の `Add` 1つだけ（`cli_usage.rs` が「集合がちょうど `add` だけ」を回帰テストとして固定）。
- CI ワークフロー・リリース設定・パッケージングの追加なし。`flake.nix` の差分は devShell への `git` 追加2行のみ。

### テストの実効性（意図的な変異での実測）

「主張しているが実際には検証していない」テストが残っていないかを、変異させて赤になるかで確かめた。いずれも検証後に復元し、`git diff -- crates/ ` が空であることと `cargo test` 全件緑（458 tests / 0 failed）を再確認済み。

| 変異 | 期待 | 実測 |
|---|---|---|
| `conformance_lock.rs` の `hold_from_other_process` を `None` にする | 宣言外のスキップが失敗として現れる | TC-port-exclusive-lock-002 / 003 / 005 が `SkipBudget` により FAILED |
| `tests/common/mod.rs` の `deny_read` を `None` にする | 受け入れテスト側の宣言も効く | `tc_task_register_task_016` / `021` が FAILED |
| `util/atomic.rs::write_atomic` を非アトミックな分割書き込みにする | 原子性の観測面が落ちる | TC-port-task-repository-011 / 012 / 042 / 043 が FAILED（042 は書きかけの内容を実際に観測） |
| `util/atomic.rs::rename_atomic` を copy + delay + remove にする | 中間状態が観測される | TC-port-task-repository-044 が FAILED（`双方に在る状態を観測した`） |
| `adapter/lock.rs::try_acquire` を解放を待つ `lock()` にする | 「待たずに返る」が落ちる | TC-003 単体は 5.65 秒で FAILED。**ただし TC-002 は返らない**（W-001） |

### フレーキー・ハング・並列干渉

- `cargo test` を3回連続で実行し、いずれも18ターゲット全部が `ok`・`FAILED` 0件。
- 実バイナリの手動実行は `HOME` を一時ディレクトリへ向けて行い、実 `~/.pulsen/` に副作用が無いことを確認（`.adr/062` の方針どおり、受け入れテストも既定でユーザーホームを切り離している）。
- 並行観測のケース（TC-042/044・`util/atomic.rs` のユニットテスト）は `StopOnDrop` で巻き戻し時にも読み手が止まる（`.adr/063`）。非ブロッキングのロック TC-003 は期限監視付き（`.adr/060`）。
- **期限を持たない待ちが1箇所だけ残っている**: TC-port-exclusive-lock-002（W-001）。現行アダプターに対してはハングしないが、スイートを別実装に当てる場面では診断の無いタイムアウトになる。`tests/common/lock.rs:36` の `read_line` も同じ性質（保持プロセスが合図を出さないと返らない）。

### `unwrap` / `expect` / `panic!` / ワイルドカード / スタブ

- 非テストコードのパニック経路は `crates/pulsen-domain/src/definition/template.rs:198` の `unreachable!` **1件のみ**（1周目 W-004 で `wont-fix` 判定済み。CLAUDE.md が不変条件違反へのパニックを許容し、why コメントもある）。`crates/pulsen/src/` の本番経路には `unwrap` / `expect` / `panic!` が0件。
- `match` のワイルドカードは、ドメイン enum に対しては0件。残る `_ =>` はすべて `&str` / `char` / `i64` 等の非 enum に対するもので、`clippy::wildcard_enum_match_arm`（ドメインのみ・ADR-029）を含む `clippy -D warnings` が警告0で通っている。
- `todo!` / `unimplemented!` / `FIXME` / `TODO` / `XXX` は `crates/` 全体で0ヒット。

### 記録・ドキュメントの整合

- `.thread/1/adr.md` の Status 行から抽出した `.adr/NNN-*.md` の集合と、`.adr/` の実ファイル（019〜064）が**双方向に一致**（欠落・余剰0）。001〜018 は本スライス以前の正本。
- `.adr/` の欠番 041 / 047 / 056〜059 は `.thread/1/adr.md:5` と `.adr/035` に理由が明記済み。
- 出荷物（`crates/`）から `.thread/` への参照は**0件**（ADR-035 の要件）。
- `.thread/1/progress.md` は「TC-port-clock-005 の1件を除き全件が実行された」と書き、スキップ運用・Issue コメント10件・残作業（MSRV 1.89 の未検証）まで記録されている。

## カバレッジ

一覧160件との1対1対応。

### 確認（137件）

- ADR 正本 40件: `.adr/019-domain-crate-workspace.md`, `.adr/020-no-serde-in-domain-timestamp-conversion-in-domain.md`, `.adr/021-yaml-value-then-hand-written-schema-walk.md`, `.adr/022-std-file-lock-and-lockguard-marker-trait.md`, `.adr/023-dependency-selection.md`, `.adr/024-git-cli-shell-out-and-target-classification.md`, `.adr/025-task-file-json-and-corrupt-classification.md`, `.adr/026-task-id-format.md`, `.adr/027-port-conformance-suite-and-harness-hooks.md`, `.adr/028-usecase-error-paths-via-test-doubles.md`, `.adr/029-wildcard-enum-match-arm-lint-domain-only.md`, `.adr/030-workflow-store-base-dir-injection.md`, `.adr/031-pulsen-home-layout-in-application-layer.md`, `.adr/032-lock-holder-example-fixture.md`, `.adr/033-git-fixture-reproducibility.md`, `.adr/034-workflow-ref-separator-set-as-constant.md`, `.adr/035-file-slice-adrs-from-019.md`, `.adr/036-infallible-ports-absorb-failure-at-construction.md`, `.adr/037-platform-separator-set-without-cfg.md`, `.adr/038-adr-filing-format.md`, `.adr/039-read-error-shared-by-find-and-list.md`, `.adr/040-rehydrate-takes-field-bundle.md`, `.adr/042-absent-yaml-value-is-omission.md`, `.adr/043-store-adapters-receive-injected-paths.md`, `.adr/044-task-file-layout-in-domain.md`, `.adr/045-task-file-dto-generic-over-snapshot.md`, `.adr/046-no-skippable-hooks-for-post-operation-observation.md`, `.adr/048-parse-inputs-at-spec-flow-position.md`, `.adr/049-base-flag-allows-hyphen-values.md`, `.adr/050-schema-error-location-is-logical.md`, `.adr/051-undisplayable-name-fixture-is-whitespace-stem.md`, `.adr/052-acceptance-test-harness-in-tests-common.md`, `.adr/053-conformance-yaml-source-hooks.md`, `.adr/054-workflow-error-file-path-goes-into-free-form-messages.md`, `.adr/055-conformance-skip-budget.md`, `.adr/060-non-blocking-lock-case-observes-from-a-second-thread.md`, `.adr/061-unused-public-accessors-are-kept-only-for-verified-layout.md`, `.adr/062-acceptance-tests-detach-the-user-home.md`, `.adr/063-concurrent-observation-stops-the-reader-on-unwind.md`, `.adr/064-unreachable-entry-is-corrupt-in-find-and-create.md`（全件の見出し規約・Status 索引・欠番の説明を走査。5周目に直した 025 / 048 / 050 / 054 は内容も読んだ）
- 作業ドキュメント 6件: `.thread/1/adr.md`, `.thread/1/plan.md`, `.thread/1/progress.md`, `.thread/1/review/triage.md`, `.thread/1/steps.md`, `.thread/1/testing.md`
- ビルド設定 3件: `Cargo.toml`, `flake.nix`, `rustfmt.toml`
- `pulsen-conformance` 18件: `crates/pulsen-conformance/Cargo.toml`, `crates/pulsen-conformance/HOOKS.md`, `crates/pulsen-conformance/src/clock.rs`, `crates/pulsen-conformance/src/config_store.rs`, `crates/pulsen-conformance/src/doubles/clock.rs`, `crates/pulsen-conformance/src/doubles/lock.rs`, `crates/pulsen-conformance/src/doubles/mod.rs`, `crates/pulsen-conformance/src/doubles/stores.rs`, `crates/pulsen-conformance/src/doubles/task_id.rs`, `crates/pulsen-conformance/src/doubles/task_repository.rs`, `crates/pulsen-conformance/src/doubles/tests.rs`, `crates/pulsen-conformance/src/doubles/worktree.rs`, `crates/pulsen-conformance/src/exclusive_lock.rs`, `crates/pulsen-conformance/src/lib.rs`, `crates/pulsen-conformance/src/task_id_generator.rs`, `crates/pulsen-conformance/src/task_repository.rs`, `crates/pulsen-conformance/src/workflow_store.rs`, `crates/pulsen-conformance/src/worktree_manager.rs`
- `pulsen-domain` 30件: `crates/pulsen-domain/Cargo.toml`, `crates/pulsen-domain/src/lib.rs`, `crates/pulsen-domain/src/definition/{agent,assembler,command,config,duration,mod,name,port,reference,snapshot,template,validator,workflow}.rs`, `crates/pulsen-domain/src/execution/{mod,port}.rs`, `crates/pulsen-domain/src/task/{attempt,branch,counters,degraded,failure,id,mod,path,port,process,state,task,time}.rs`
- `pulsen` 本体 26件: `crates/pulsen/Cargo.toml`, `crates/pulsen/examples/lock_holder.rs`, `crates/pulsen/src/adapter/{clock,config_store,lock,mod,task_file,task_id,task_repository,workflow_store,worktree,yaml}.rs`, `crates/pulsen/src/application/{home,mod,register_task}.rs`, `crates/pulsen/src/cli/{add,args,exit,mod,render,wire}.rs`, `crates/pulsen/src/{lib,main}.rs`, `crates/pulsen/src/util/{atomic,fsdir,mod}.rs`
- `pulsen` テスト 14件: `crates/pulsen/tests/cli_add_boundary.rs`, `crates/pulsen/tests/cli_add_error.rs`, `crates/pulsen/tests/cli_add_normal.rs`, `crates/pulsen/tests/cli_usage.rs`, `crates/pulsen/tests/common/git.rs`, `crates/pulsen/tests/common/lock.rs`, `crates/pulsen/tests/common/mod.rs`, `crates/pulsen/tests/conformance_config_store.rs`, `crates/pulsen/tests/conformance_lock.rs`, `crates/pulsen/tests/conformance_task_repository.rs`, `crates/pulsen/tests/conformance_time_id.rs`, `crates/pulsen/tests/conformance_workflow_store.rs`, `crates/pulsen/tests/conformance_worktree.rs`, `crates/pulsen/tests/register_task.rs`

### スキップ（23件）

- `Cargo.lock` — cargo が生成するロックファイル。依存の選定自体は `.adr/023` と `crates/*/Cargo.toml` で確認済み。
- `.thread/1/review/changed-files-001.txt`, `.thread/1/review/changed-files-002.txt`, `.thread/1/review/changed-files-003.txt`, `.thread/1/review/changed-files-004.txt`, `.thread/1/review/changed-files-005.txt` — 過去ラウンドのレビュー対象一覧。本ラウンドの対象は `changed-files-006.txt` で、その全量を確認済み。
- `.thread/1/review/review-001.md`, `.thread/1/review/review-001-adapter.md`, `.thread/1/review/review-001-arch-spec.md`, `.thread/1/review/review-001-domain.md`, `.thread/1/review/review-001-test.md`, `.thread/1/review/review-001-usecase-cli.md`, `.thread/1/review/review-002-adapter.md`, `.thread/1/review/review-002-arch-spec.md`, `.thread/1/review/review-002-domain.md`, `.thread/1/review/review-002-test.md`, `.thread/1/review/review-002-usecase-cli.md`, `.thread/1/review/review-003.md`, `.thread/1/review/review-003-adapter-test.md`, `.thread/1/review/review-003-arch-spec.md`, `.thread/1/review/review-003-domain-usecase-cli.md`, `.thread/1/review/review-004-final.md`, `.thread/1/review/review-005-final.md` — 過去ラウンドのレビュー本文。判定・反映内容は正本である `triage.md`（ラウンド1〜5の台帳）を通読して確認しており、個々のレビュー本文を再読しても新たな判断材料は出ない。
