# 調査結果 — Issue #9

## あるべきアーキテクチャ（`CLAUDE.md` / `spec/index.md` から読み取ったもの）

- ヘキサゴナル。ドメイン（`crates/pulsen-domain`）→ ポート（trait）→ アダプター（`crates/pulsen/src/adapter`）→ ユースケース（`crates/pulsen/src/application`）→ CLI（`crates/pulsen/src/cli`）。依存は外→内。
- 関数型ドメインモデリング。不正な状態を型で表現不能にする / OR は enum・AND は struct / parse, don't validate / 遷移は `self` を消費する純粋関数 / `match` にワイルドカードを置かない。
- **spec は設計の正**であり、実装の後追いドキュメントではない。`spec/domains/` `spec/usecases/` `spec/pages/` `spec/testcases/` が正本で、`spec/inventory/*.md` は**その機械的な台帳**（`_shared/references/spec-inventory.md`）。台帳は「spec を変更したら必ず更新する」ものであり、**同じ記述が本体と台帳の2箇所に存在する**。
- 台帳の ID 規則: `{PREFIX}-{group}-{連番3桁}`、**一度振った ID は変えない**（要素が消えたら行を削除し欠番を残す）。連番は「spec 内の出現順」だが、既存 ID の安定性が優先する。

## 本 Issue の性質

25件はすべて「Issue #1 / #2 / #3 の実装で spec の記述と実装の形が食い違い、実装側は ADR に why を残して決着済み」のもの。Issue の完了条件は「6件それぞれについて、spec を言い換えるか『現状の spec が正しく実装側を直す』と判断するかを決め、**決めた側を反映する**。実装を直す判断になったものは、対応する `.adr/` エントリも更新する」であり、**実装変更を明示的に想定している**。

各件について ADR を読んで「spec の元の意図」と「実装が選んだ形」のどちらが設計として正しいかを問い直した結果、**23件は実装側が正しく spec 追従でよい**、**2件（A2 / C5）は spec 側（あるいはより厳密な規約）が正しく実装を直す**と判断した。後者2件は本 Issue の中で spec・台帳・`crates/`・`.adr/` を同時に直す（理由は下記「判断が要る件」）。

## `.adr/` のファイル名スキーム移行 — 旧番号 → 現ファイル名

`4a00abe chore: ADR のファイル名を外部から与えられる番号・日付に移行する` で `.adr/NNN-slug.md` → `.adr/{Issue番号 or yyyy-MM-dd}-{slug}.md` にリネームされた。Issue #9 の本文・コメントは**移行前の番号**で参照しているため、下表で読み替える。

### Issue 本文（#1 由来）が使う番号 = `.thread/1/adr.md` の番号 = 旧 `.adr/` 番号

| 旧 | 現ファイル名 |
|---|---|
| `.adr/021` / ADR-021 | `.adr/1-yaml-value-then-hand-written-schema-walk.md` |
| `.adr/025` / ADR-025 | `.adr/1-task-file-json-and-corrupt-classification.md` |
| `.adr/050` | `.adr/1-schema-error-location-is-logical.md` |
| `.adr/051` | `.adr/1-undisplayable-name-fixture-is-whitespace-stem.md` |
| `.adr/054` | `.adr/1-workflow-error-file-path-goes-into-free-form-messages.md` |

### コメント1（#2 由来）が使う番号

コメントは「ADR 番号は `.thread/2/adr.md` のもの」と書くが、**現在の `.thread/2/adr.md` の番号とは +8 ずれている**（昇格時に採番し直された）。コメントの番号 + 8 = 現 `.thread/2/adr.md` の番号 = 旧 `.adr/` 番号。

| コメントの番号 | `.thread/2/adr.md` / 旧 `.adr/` | 現ファイル名 |
|---|---|---|
| ADR-070 | ADR-078 / `.adr/078` | `.adr/2-wrapper-restores-state-root-from-run-dir.md` |
| ADR-071 | ADR-079 / `.adr/079` | `.adr/2-command-line-rehydrate-across-process-boundary.md` |
| ADR-072 | ADR-080 / `.adr/080` | `.adr/2-run-dir-files-are-json-and-markers-are-empty.md` |
| ADR-073 | ADR-081 / `.adr/081` | `.adr/2-tick-errors-are-structured-values.md` |
| ADR-079 | ADR-087 | （**未昇格**。`RunDirPath::state_root` は `derive` との一致を条件に復元する。内容は `.adr/2-wrapper-restores-state-root-from-run-dir.md` に吸収） |
| ADR-081 | ADR-089 / `.adr/089` | `.adr/2-wrapper-exit-code-reports-its-own-duty.md` |
| ADR-084 | ADR-092 / `.adr/092` | `.adr/2-empty-summary-means-nothing-to-process.md` |
| ADR-086 | ADR-094 / `.adr/094` | `.adr/2-confirmed-running-field-and-recorded-failures-in-errors.md` |
| ADR-087 | ADR-095 / `.adr/095` | `.adr/2-record-tool-failure-takes-tool-failure-kind.md` |
| ADR-088 | ADR-096 / `.adr/096` | `.adr/2-transition-error-holds-classification-only.md` |
| ADR-090 | ADR-098 / `.adr/098` | `.adr/2-spawn-not-observed-classification-and-error-headings.md` |

### コメント2（#3 由来）が使う番号

`.adr/NNN` 表記は**昇格済み・リネーム前**の番号。`ADR-00N` 表記は `.thread/3/adr.md` の番号。

| コメントの表記 | 現ファイル名 |
|---|---|
| `.adr/008` | `.adr/2026-08-11-skipped-judgement-outcome.md` |
| `.adr/090` | `.adr/2-persisted-explanations-come-from-domain-describe.md` |
| `.adr/092` | `.adr/2-empty-summary-means-nothing-to-process.md` |
| `.adr/096` | `.adr/2-transition-error-holds-classification-only.md` |
| `.adr/098` | `.adr/2-spawn-not-observed-classification-and-error-headings.md` |
| ADR-003 / ADR-011 | `.adr/3-notification-procedure-layering.md` |
| ADR-004 / ADR-008 | `.adr/3-tick-issue-classification-by-repair-hint.md` |
| ADR-005（`judged`） | **未昇格**（`.thread/3/adr.md` のみ） |
| ADR-006（`AlreadyNotified`） | **未昇格**（`.thread/3/adr.md` のみ） |
| ADR-009 / ADR-016 | `.adr/3-narrow-decision-types-embedded-in-ledger-types.md` |
| ADR-010 | `.adr/3-run-failure-cause-and-remnants-as-classifications.md` |
| ADR-017 | `.adr/3-cleanup-left-as-fourth-error-heading.md` |

## 25件の対応表

件番号は本文6件を A1〜A6、コメント1（#2 由来）11件を B1〜B11、コメント2（#3 由来）8件を C1〜C8 とする。

| # | 件名 | spec の該当箇所（本体） | 台帳の該当行 | 実装の現在の形 | ADR（現ファイル名） | 対応方針 |
|---|---|---|---|---|---|---|
| A1 | エラー位置の粒度 | `spec/testcases/task/register-task.md` 異常系 表2行目「config.yaml がパース不能(構文エラー・未知キー)→ エラー位置を表示」／ `spec/pages/index.md` 縮退規則 ※1 | `spec/inventory/test.md` TC-task-register-task-015 ／ `spec/inventory/frontend.md` PAGE-common-008 | `ConfigLoadError::Invalid { message, location }`。構文エラー・重複キーは行・列、スキーマ違反はキーのパス（`agents.claude.cmd`）。`crates/pulsen/src/adapter/yaml.rs` / `config_store.rs` | `.adr/1-schema-error-location-is-logical.md`（+ `.adr/1-yaml-value-then-hand-written-schema-walk.md`） | spec 追従。「エラー位置」を「構文エラー・重複キーは行・列、スキーマ違反はキーのパス」へ言い換える |
| A2 | ワークフロー定義のスキーマ違反で対象ファイルを案内できない | `spec/domains/definition.md#workflowstore` エラー一覧 `Parse(WorkflowParseError)` | `spec/inventory/domain.md` DOM-definition-052 / DOM-definition-055 ／ `spec/inventory/adapter.md` ADP-workflowstore-001 | **spec どおり** `Parse(WorkflowParseError)`。解決先の絶対パスは `WorkflowLoadError::Io { message }` と `WorkflowParseError::YamlSyntax { message }` の自由形式メッセージに前置 | `.adr/1-workflow-error-file-path-goes-into-free-form-messages.md` ／ `.adr/1-schema-error-location-is-logical.md` | **実装を直す側。** Issue 本文が「ポート表を `Parse { error, resolved_from }` へ改める」と結論まで書いており、実装が現在の形なのは #1 の受け入れ基準（ポート表との1:1一致）を守った結果にすぎない。本 Issue で spec のポート表・適合ケース・台帳・`crates/`（4ファイル）・`.adr/`（2ファイル）を同時に直す。解決先の案内は構造化フィールドに一本化し、`YamlSyntax` への前置は外す |
| A3 | 表示名を決められないファイル名の例示 | `spec/testcases/task/register-task.md` 異常系「拡張子を除くと空になるファイル名等」／ `spec/manual-tests/setup.md` TC-46（目的文 + 手順1が `$WORK/.yaml` を使っている）／ `spec/manual-tests/task-execution.md` 手動テスト対象外の表 | `spec/inventory/test.md` TC-task-register-task-034 | 受け入れテストは語幹が空白のみのファイル名 ` .yaml` で再現。到達するのは `NameError::SurroundingWhitespace`。`Path::file_stem(".yaml")` は `.yaml` を返すため **TC-46 手順1 の `$WORK/.yaml` は成功してしまう（手順の誤り）** | `.adr/1-undisplayable-name-fixture-is-whitespace-stem.md` | spec 追従。例示に「語幹が空白のみになるファイル名(` .yaml`)」を含め、TC-46 の手順のファイル名も直す |
| A4 | 制約を持たない `InputText` の生成規約 | `spec/domains/definition.md#名前系(文字列 newtype)` の表直後「いずれも `parse(s: String) -> Result<Self, NameError>` でのみ生成する」 | `spec/inventory/domain.md` DOM-definition-007 | `InputText::new(s: String) -> Self`（総関数、`parse` なし）。`crates/pulsen-domain/src/definition/name.rs:110-127`。他6型は `parse`（`Prompt` は空のみ拒否） | なし（規約の適用） | spec 追従。「制約のある型は `parse` でのみ生成する。制約のない `InputText` は総関数 `new` で生成する」へ限定 |
| A5 | エージェント単位の登録時検証エラーの重複 | `spec/domains/definition.md#registrationvalidator`「エラーは全ステータス分をまとめて返す(最初の1件で打ち切らない)」 | `spec/inventory/domain.md` DOM-definition-049 / DOM-definition-050 ／ `spec/inventory/usecase.md` UC-task-001 | `crates/pulsen-domain/src/definition/validator.rs:135-144` の `push_once`。`UnknownAgent` / `InvalidAgentDefinition` は同値なら積まない。`MissingSkillInput` / `MissingModel` は `status` を持つので常に積む | なし | spec 追従。「`status` を持たない2種はエージェント単位の誤りであり、同値の重複は1件にまとめる」を明記 |
| A6 | スナップショットフィールドの破損の作り方 | `spec/testcases/ports/task-repository.md` 「Corrupt と SnapshotUnreadable の区別」の「スナップショットフィールドのみを構文不正な内容に置き換える」／ 同 save_degraded 節の前提条件 | `spec/inventory/test.md` TC-port-task-repository-022 / TC-port-task-repository-009 | ファイル全体を1回の JSON パースで読むため、「有効な JSON だがスナップショットとして解釈できない」でのみ `SnapshotUnreadable` に到達する | `.adr/1-task-file-json-and-corrupt-classification.md` | spec 追従。「有効な JSON だがスナップショットとして解釈できない内容に置き換える」へ |
| B1 | `CommandLine::rehydrate` の追加 | `spec/domains/definition.md#commandline`「生成: `CommandTemplate::expand` の結果としてのみ生成する」 | `spec/inventory/domain.md` DOM-definition-023 | `crates/pulsen-domain/src/definition/template.rs:136-159`。`rehydrate(tokens: Vec<String>) -> Result<Self, CommandError>`（空は `Empty`）+ `CommandTemplate::expand` の2経路 | `.adr/2-command-line-rehydrate-across-process-boundary.md` | spec 追従。生成2経路（展開・プロセス境界からの復元）に改める。台帳に `CommandLine.rehydrate` 行を追加 |
| B2 | `RunDirPath::state_root` の追加 | `spec/domains/task.md#rundirpath`（生成は `derive` のみ）／ `spec/domains/execution.md#rundirpath-のファイル配置(語彙)` | `spec/inventory/domain.md` DOM-task-013 / DOM-execution-028〜033 | `crates/pulsen-domain/src/task/path.rs:137` `pub fn state_root(&self) -> Option<StateRoot>`。`attempt-<n>` と task-id を parse し、祖父ディレクトリ名が `runs` であること + `derive` との一致を条件に返す | `.adr/2-wrapper-restores-state-root-from-run-dir.md`（`derive` 一致条件の thread ADR-087 は未昇格） | spec 追従。`RunDirPath` の語彙に逆写像を追加。台帳に行を追加 |
| B3 | `wrapper` の終了コードの規約 | `spec/pages/index.md#wrapper(内部コマンド)`「状態: 利用者は直接観測しない」／ `spec/usecases/execution.md#runwrapper` エラーケース表「引数の不正 → 何も書かず非0終了」 | `spec/inventory/frontend.md` PAGE-wrapper-005 ／ `spec/inventory/usecase.md` UC-execution-009 | `crates/pulsen/src/cli/mod.rs:57-64`。`Ran` / `Suppressed` = 0、`WrapperError`（`NothingRecorded` 含む）= 1、clap の使い方の誤り = 2。エージェントの exit code は伝播しない（`exit` ファイルだけが持つ） | `.adr/2-wrapper-exit-code-reports-its-own-duty.md` | spec 追従（追記）。「終了コードはラッパー自身が責務を果たせたかを表す」を pages に明記。台帳に行を追加 |
| B4 | tick の `errors` の型 | `spec/usecases/execution.md#出力DTO(サマリー)` の `errors` 行（`message: String`） | `spec/inventory/usecase.md` UC-execution-002 | `TickIssue`（21変種、`crates/pulsen/src/application/tick/mod.rs:62-216`）。文言は `crates/pulsen/src/cli/render.rs` | `.adr/2-tick-errors-are-structured-values.md` | spec 追従。B10 / C6 と合わせて `errors` の分類表を1つ作る |
| B5 | サマリー DTO に `confirmed_running` | `spec/usecases/execution.md#出力DTO(サマリー)`（9フィールド） | `spec/inventory/usecase.md` UC-execution-002 | `TickSummary` は11フィールド（`launched` / `confirmed_running` / `judged` / `transitioned` / `skipped_back` / `frozen` / `notified` / `archived` / `errors` / `gc_deleted` / `gc_errors`） | `.adr/2-confirmed-running-field-and-recorded-failures-in-errors.md` / `.adr/2-empty-summary-means-nothing-to-process.md` | spec 追従。**C2 と同じ表なので一括で直す** |
| B6 | `RunStore` の write 系がディレクトリを作る契約 | `spec/domains/execution.md#runstore` の契約リスト（`write_invalidation_marker` にのみ作成契約がある） | `spec/inventory/domain.md` DOM-execution-041/042/043 ／ `spec/inventory/adapter.md` ADP-runstore-008/009/010 ／ `spec/testcases/ports/run-store.md`（該当行なし） | `crates/pulsen-domain/src/execution/port.rs:66-68` の trait doc で宣言。適合スイートに `write_準備を経ない書き込みも置き場ごと作って残る`（`crates/pulsen-conformance/src/run_store.rs:381`）があるが**spec の適合表には無い** | `.adr/2-run-dir-files-are-json-and-markers-are-empty.md` | spec 追従（追記）。契約に1行、`spec/testcases/ports/run-store.md` の表**末尾**に1行追加。台帳（domain / adapter / test）を追従 |
| B7 | `record_tool_failure` の `kind` の型 | `spec/domains/task.md#振る舞い(遷移関数)` の `record_tool_failure` 行 ／ `FailureKind` の定義 | `spec/inventory/domain.md` DOM-task-042 / DOM-task-024 | `record_tool_failure(kind: ToolFailureKind, ...)`。`ToolFailureKind = WorktreeCreate \| WorktreeRemove \| ArchiveMove`（`crates/pulsen-domain/src/task/failure.rs:25-33`）。`FailureKind` へは crate 内の `recorded()` で写す（`From` ではない） | `.adr/2-record-tool-failure-takes-tool-failure-kind.md` | spec 追従。`ToolFailureKind` を値オブジェクトとして追加し遷移表の引数型を絞る。台帳に行を追加 |
| B8 | `TransitionError` の形 | `spec/domains/task.md#エラー型`（5種、`InvalidState { expected: &'static str }` / `InvariantViolated { message: String }`） | `spec/inventory/domain.md` DOM-task-053 | 6種: `InvalidState { expected: &'static [ExecutionStateKind], actual: ExecutionStateKind }` / `WorkspaceAlreadySet` / `WorkspaceNotSet` / `NotAgentRunStatus { status }` / `MissingCurrentAttempt` / `AlreadyNotified`（`crates/pulsen-domain/src/task/transition.rs:14-37`） | `.adr/2-transition-error-holds-classification-only.md`（`AlreadyNotified` は `.thread/3/adr.md` ADR-006・未昇格） | spec 追従。**C1 と同一の型なので一括で直す**。波及先は下記「波及の洗い出し」参照 |
| B9 | `InconsistentRunFiles` の形 | `spec/domains/execution.md#launchingclassifier` の `InconsistentRunFiles { message: String }` | `spec/inventory/domain.md` DOM-execution-016 | `pub enum InconsistentRunFiles { MissingStartTime }`（`crates/pulsen-domain/src/execution/launching.rs:32-36`） | `.adr/2-tick-errors-are-structured-values.md` / `.adr/2-transition-error-holds-classification-only.md` の規則 | spec 追従。**C8 と同一**（C8 は #3 で変更していないことの確認のみ） |
| B10 | tick の `errors` に `SpawnNotObserved` | `spec/usecases/execution.md` 手続きC・エラーケース表（猶予超過の spawn 失敗と同期エラーの区別がない） | `spec/inventory/usecase.md` UC-execution-002 / UC-execution-005 | `TickIssue::SpawnNotObserved`（猶予超過で確定）と `TickIssue::SpawnFailed`（`spawn_wrapper` の同期エラー）が別変種 | `.adr/2-spawn-not-observed-classification-and-error-headings.md` | spec 追従（B4 の分類表に含める） |
| B11 | tick サマリーの表示の見出し | `spec/pages/index.md#tick`「処理結果のサマリー表示(…)」 | `spec/inventory/frontend.md` PAGE-tick-004 | `crates/pulsen/src/cli/render.rs:54-135`。ID系10見出し（起動 / 起動確認 / 判定確定 / 遷移 / 実行待ちへ復帰 / 凍結 / 通知 / 終端処理 / gcで削除 / gcで削除できず）+ 報告4見出し（失敗を記録 / 起動の結果が未確定 / スキップ / 後始末が残っている）。空なら「処理対象のタスクはありませんでした。」 | `.adr/2-spawn-not-observed-classification-and-error-headings.md` / `.adr/3-cleanup-left-as-fourth-error-heading.md` / `.adr/2-empty-summary-means-nothing-to-process.md` | spec 追従（追記）。**C7 の第4見出しと語義の一般化を織り込んで1回で書く** |
| C1 | `TransitionError` の変種と種類数 | （B8 と同一） | （B8 と同一） | （B8 と同一） | （B8 と同一） | **B8 に統合** |
| C2 | tick 出力 DTO に `judged` | `spec/usecases/execution.md#出力DTO(サマリー)` | `spec/inventory/usecase.md` UC-execution-002 | `TickSummary.judged: Vec<TaskId>`（`complete_run` の受け皿） | `.thread/3/adr.md` ADR-005（**未昇格**）／ `.adr/2-empty-summary-means-nothing-to-process.md` | **B5 に統合**（同じ表） |
| C3 | `classify_alive` の返り値型と2段規則の1段目の置き場所 | `spec/domains/execution.md#runningclassifier`（`-> RunningDecision`。2段規則の1段目を分類器の節で述べている）／ `spec/usecases/execution.md` 手続きD 手順2 の「RunningClassifier の2段規則」／ `spec/testcases/execution/tick.md` 手続きD 異常系「exit ファイルあり・`starttime_of` が失敗する環境」 | `spec/inventory/domain.md` DOM-execution-017（1段目を `classify_alive` に含めている）/ DOM-execution-008 | `classify_alive(...) -> AliveDecision`（`KeepRunning` / `KillOnTimeout` / `DiedWithoutExit` の3値）。`From<AliveDecision> for RunningDecision` あり。1段目は `crates/pulsen/src/application/tick/observe.rs:57-66`。`RunningDecision` は4値のまま | `.adr/3-narrow-decision-types-embedded-in-ledger-types.md` | spec 追従。`AliveDecision` を値オブジェクトとして追加し、DOM-execution-017 の要点を2段目に限定。1段目の置き場所をユースケース側と明記 |
| C4 | `default_judgement` の返り値型 | `spec/domains/execution.md#judgementservice` の `default_judgement` 行 | `spec/inventory/domain.md` DOM-execution-019 / DOM-execution-004 | `default_judgement(&ExitCode) -> DefaultJudgement`（`Completed` / `Failed`）。`From<DefaultJudgement> for JudgeOutcome` あり。`JudgeOutcome` は3値のまま | `.adr/3-narrow-decision-types-embedded-in-ledger-types.md` | spec 追従。`DefaultJudgement` を値オブジェクトとして追加 |
| C5 | `interpret_notify_completion` / `NotifyOutcome` の追加 | `spec/domains/execution.md#notificationservice`（`notify_env` と定数のみ）／ `spec/usecases/execution.md#共通手続き` 手順4 | `spec/inventory/domain.md` DOM-execution-022 / DOM-execution-071 ／ `spec/inventory/usecase.md` UC-execution-001 | `interpret_notify_completion(&CommandCompletion) -> NotifyOutcome`（`Delivered` / `Failed { detail }`）。`crates/pulsen-domain/src/execution/notification.rs:12-56`。呼び出しは `crates/pulsen/src/application/tick/notify.rs` の `Delivery::{NotConfigured, Attempted(NotifyOutcome)}` | `.adr/3-notification-procedure-layering.md` | **実装を直す側（下記「判断が要る件」参照）。** 本 Issue で `Failed { detail }` を `Failed { cause: NotifyFailureCause }`（`ExitedNonZero` / `TimedOut` / `FailedToStart`）へ改め、spec・台帳・`crates/`（5ファイル。`execution/mod.rs` の再エクスポートを含む）・`.adr/` を同時に直す。文言の組み立ては `cli::render` へ寄せる |
| C6 | tick の `errors` の分類が6つ増えた | `spec/usecases/execution.md`（`errors` に分類の列挙がない） | `spec/inventory/usecase.md` UC-execution-002 / 004〜008 | `MissingProcessIdent` / `MissingWorkspace` / `ObservationFailed` / `KillFailed` / `RunFailed { cause: RunFailureCause }` / `RemnantsUnhandled { remnants: RemnantsLeft }`。`RunFailureCause = DefaultJudgement { exit } \| JudgeCommand { exit } \| TimedOut { timeout } \| DiedWithoutExit`、`RemnantsLeft = NotIdentifiable \| Failed { message }` | `.adr/3-tick-issue-classification-by-repair-hint.md` / `.adr/3-run-failure-cause-and-remnants-as-classifications.md` | **B4 / B10 に統合**（`errors` の分類表を1回で書く） |
| C7 | サマリーの報告の見出しが3分類から4分類に | （B11 と同一） | （B11 と同一） | （B11 と同一） | `.adr/3-cleanup-left-as-fourth-error-heading.md` | **B11 に統合** |
| C8 | `LaunchingClassifier` の `InconsistentRunFiles` | （B9 と同一。#3 でも変更していないことの確認のみ） | （B9 と同一） | （B9 と同一） | （B9 と同一） | **B9 に統合** |

一括で直す組: `B8 = C1`（`TransitionError`）／ `B9 = C8`（`InconsistentRunFiles`）／ `B5 + C2`（サマリー DTO）／ `B11 + C7`（表示の見出し）／ `B4 + B10 + C6`（`errors` の分類表）。**独立した改訂対象は 21 箇所**。

## 実装の現在の形（実際にコードを読んで確認したもの）

Issue 本文・コメントの記述は執筆時点のものだが、下記はいずれも現ブランチのコードで確認済み。Issue の記述と食い違う点は次の2つだけ。

- `CommandLine` は `crates/pulsen-domain/src/definition/command.rs` ではなく `definition/template.rs:136-159` にある。
- `InconsistentRunFiles` は「種別だけを持つ列挙」ではあるが、変種は `MissingStartTime` の**1つだけ**（多変種の列挙ではない）。

参考: `RunStore` trait の現在のメソッドは9つ（`prepare_attempt` / `read_pid_file` / `read_starttime` / `read_exit` / `write_invalidation_marker` / `marker_exists` / `write_starttime` / `write_pid_file` / `write_exit`）で、spec のポート表にある `attempt_exists` / `list_runs` / `delete_attempt` / `remove_task_dir_if_empty` は後続スライス（show / gc）の未実装分であり、**本 Issue の乖離ではない**（spec を削ってはならない）。

## 波及の洗い出し（本体と台帳の両方）

同じ型・DTO が複数ファイルに現れるため、1件の言い換えが波及する先を列挙する。

| 改訂対象 | spec 本体 | 台帳 |
|---|---|---|
| `TransitionError`（B8/C1） | `spec/domains/task.md#エラー型`(230) ／ 同「検証の境界」(164) ／ 同 `TaskRepository` のデコード節(300) ／ `spec/usecases/execution.md` 処理フロー7(56)・手続きC 手順0(83)・手続きD 手順0(95)・**AbortTask のエラーケース表(178)** ／ `spec/testcases/execution/tick.md` 走査と分岐 異常系(39) ／ `spec/testcases/ports/task-repository.md`「Corrupt と SnapshotUnreadable の区別」(56) | `spec/inventory/domain.md` DOM-task-053(111) ／ `spec/inventory/usecase.md` UC-execution-002 / 005 / 006 / 008(19) ／ `spec/inventory/test.md` TC-exec-tick-022(284) / **TC-port-task-repository-026(565)**（注1・注2） |
| サマリー DTO（B5/C2） | `spec/usecases/execution.md#出力DTO(サマリー)` | `spec/inventory/usecase.md` UC-execution-002 |
| `errors` の分類（B4/B10/C6） | `spec/usecases/execution.md#出力DTO(サマリー)` + エラーケース表 + 手続きA〜D の報告記述（**手続きD 手順2 の `judge` 定義ありの枝には `MissingWorkspace` を積む検査そのものが無いので、分類の追加と同時に手順を1つ挿入する**） | `spec/inventory/usecase.md` UC-execution-002 / 005 / 006 / 008（注3） |
| 表示の見出し（B11/C7） | `spec/pages/index.md#tick` | `spec/inventory/frontend.md` PAGE-tick-004（+ 新規行） |
| `RunningClassifier` 2段規則（C3） | `spec/domains/execution.md#runningclassifier`(109) ／ `spec/usecases/execution.md` 手続きD 手順2(97)・3 ／ `spec/testcases/execution/tick.md` 手続きD 異常系(203) | `spec/inventory/domain.md` DOM-execution-008 / 017（+ `AliveDecision` の新規行）／ `spec/inventory/usecase.md` UC-execution-006 ／ **`spec/inventory/test.md` TC-exec-tick-103(365)**（要点欄に「RunningClassifier の2段規則」が写っている。`grep -rn 'RunningClassifier の2段規則' spec/` の3ヒットがこの行の全体） |
| `default_judgement`（C4） | `spec/domains/execution.md#judgementservice` ／ `spec/usecases/execution.md` 手続きD 手順2 | `spec/inventory/domain.md` DOM-execution-004 / 019（+ `DefaultJudgement` の新規行） |
| `NotificationService`（C5） | `spec/domains/execution.md#notificationservice` の**責務行(126)** + **定数行(128。成否の規則を解釈関数側へ一本化する)** + メソッド ／ `spec/usecases/execution.md#共通手続き` 手順4(15) + Tick 側の報告(56 付近の `errors` の `NotifyFailed`) | `spec/inventory/domain.md` DOM-execution-022(157) / 071(212)（+ 新規3行）／ `spec/inventory/usecase.md` UC-execution-001 / 002（注4） |
| `record_tool_failure`（B7） | `spec/domains/task.md` 遷移表 + `FailureNote` の節 ／ `spec/usecases/execution.md` 手続きA・B | `spec/inventory/domain.md` DOM-task-024 / 042（+ `ToolFailureKind` の新規行）／ `spec/inventory/usecase.md` UC-execution-003 / 004 |
| `RunStore` write 系（B6） | `spec/domains/execution.md#runstore` 契約 ／ `spec/testcases/ports/run-store.md`（末尾に1行） | `spec/inventory/domain.md` DOM-execution-041/042/043 ／ `spec/inventory/adapter.md` ADP-runstore-008/009/010 ／ `spec/inventory/test.md`（末尾に TC-port-run-store-035） |
| `CommandLine`（B1） | `spec/domains/definition.md#commandline` ／ `spec/domains/execution.md` の `WrapperLaunchSpec` 説明（必要なら） | `spec/inventory/domain.md` DOM-definition-023（+ 新規行） |
| `RunDirPath`（B2） | `spec/domains/task.md#rundirpath` ／ `spec/domains/execution.md#rundirpath-のファイル配置(語彙)` | `spec/inventory/domain.md` DOM-task-013（+ 新規行） |
| wrapper 終了コード（B3） | `spec/pages/index.md#wrapper(内部コマンド)` ／ `spec/usecases/execution.md#runwrapper` エラーケース表 | `spec/inventory/frontend.md`（新規行）／ `spec/inventory/usecase.md` UC-execution-009 |
| 表示名の例示（A3） | `spec/testcases/task/register-task.md` ／ `spec/manual-tests/setup.md` TC-46（目的 + 手順1）／ `spec/manual-tests/task-execution.md` 対象外の表 | `spec/inventory/test.md` TC-task-register-task-034 |
| エラー位置（A1） | `spec/testcases/task/register-task.md` ／ `spec/pages/index.md` ※1 | `spec/inventory/test.md` TC-task-register-task-015 ／ `spec/inventory/frontend.md` PAGE-common-008 |
| スナップショット破損（A6） | `spec/testcases/ports/task-repository.md` **3箇所** — 「Corrupt と SnapshotUnreadable の区別」の現役側(52)と**アーカイブ側(58)**、「save / save_degraded」の前提条件(24) | `spec/inventory/test.md` TC-port-task-repository-009(548) / 022(561) / **028(567)** |
| `InputText`（A4） | `spec/domains/definition.md#名前系(文字列 newtype)` | `spec/inventory/domain.md` DOM-definition-007 |
| `RegistrationValidator`（A5） | `spec/domains/definition.md#registrationvalidator` | `spec/inventory/domain.md` DOM-definition-049。**DOM-definition-050（`RegistrationError` 5種）は列挙が変わらないため対応なし**（重複集約は validator の振る舞いであり、エラー型の列挙には現れない） |
| `WorkflowLoadError::Parse`（A2） | `spec/domains/definition.md#workflowstore` のエラー一覧(305) + 契約リスト ／ **`spec/testcases/ports/workflow-store.md`「パースエラー」節の13行(37〜49。タプル記法 `Err(Parse(...))` を構造体記法へ。37行には `resolved_from` の主張を足す)** ／ **`spec/usecases/task.md` RegisterTask エラーケース表(54。「位置・原因を表示」→「位置・原因・解決先パスを表示」)** | `spec/inventory/domain.md` DOM-definition-052(58) / DOM-definition-055(208) ／ `spec/inventory/adapter.md` ADP-workflowstore-001(8) ／ **`spec/inventory/test.md` TC-port-workflow-store-017〜029(600〜612)** ／ **`spec/inventory/usecase.md` UC-task-001(7)**（注5） |
| エラー位置（A1・アダプター側） | （本体は上の A1 行） | `spec/inventory/adapter.md` ADP-config-001(7) |

### 補足（表の注）

- **注1（`InvariantViolated` の全ヒット）**: `grep -rn 'InvariantViolated' spec/` の結果は**7ファイル12箇所**（表の spec 本体8箇所 = `task.md` 164 / 230 / 300・`usecases/execution.md` 56 / 83 / 178・`tick.md` 39・`task-repository.md` 56、+ 台帳4行 = `domain.md` DOM-task-053(111) / `usecase.md` UC-execution-008(19) / `test.md` TC-exec-tick-022(284) / TC-port-task-repository-026(565)）。**B8 の対象はこの grep の全ヒット**とし、ステップの箇条は grep の結果より狭くならないことを条件にする。このうち `task.md:230` は `TransitionError` の定義そのもの（6種化で消える）なので、追従先として残るのは本体7箇所。加えて **`spec/usecases/execution.md` 手続きD 手順0(95行)** は「不変条件2〜3の破れ」と書くだけで `InvariantViolated` の語を含まないため grep には現れないが、報告分類（`MissingCurrentAttempt` / `MissingProcessIdent`）を添える追従先である。受け入れ基準は「残り7箇所 + 手続きD 手順0(95行)」と書き分ける（gate の0件と列挙が一致する）。
- **注2（「検証の境界」の書き換え方）**: (164) の書き換えは1文の置換ではない。実装では不変条件4 の破れは `MissingCurrentAttempt` にならず、`record_launching` が `TransitionError::WorkspaceNotSet` を返し（`crates/pulsen-domain/src/task/task.rs:290`）、判定経路では**遷移関数を呼ばずに**ユースケースが `TickIssue::MissingWorkspace` を積む（`crates/pulsen/src/application/tick/observe.rs` の `judge` 内、`workspace()` が None の枝）。根拠は `.thread/3/adr.md` ADR-008「不変条件4の破れは遷移エラーに相乗りさせず、tick の報告分類にする」。3つに書き分ける（steps.md ステップ2）。
- **注3（`UC-execution-007` は対応なし）**: 手続きE の gc は範囲表記に含めない。報告先が `errors` ではなく `gc_errors`（`Vec<(String, AttemptNumber)>`）であり、`TickIssue` の分類とは無関係のため**対応なし**。
- **注4（共通手続き notify の報告先は経路ごとに違う）**: 冒頭(9行)が宣言するとおりこの手続きは Tick と **AbortTask** が共有しており、AbortTask 側の報告先は出力DTO の `notify_warning: Option<String>`(152行)、手順は「共通手続き notify を実行(…`notify_warning` を表示)」(163行)である。共通手順に tick 専用の `errors` を書くと AbortTask に存在しないフィールドを要求する形になり、新しい食い違いを作る（AbortTask は未実装なので `cargo test` では検出されない）。共通手順が固定するのは `interpret_notify_completion` の経由と「`Delivered` のときだけ `mark_notified` → `save`」まで。報告の形は呼び出し側の記述に置く（steps.md ステップ4）。
- **注5（A2 の消化確認）**: `grep -rn 'Parse(' spec/`（現在28箇所 = 本体1 + 台帳1 + 適合ケース13 + 台帳13）が0件になることで行う。適合ケースを落とすと、ポート契約を変えたのにその契約の適合ケースだけ旧い形で残る（本 Issue が閉じようとしている食い違いそのもの）。`resolved_from` の主張は先頭行1行に置けば契約は固定できる（`.adr/1-conformance-skip-budget.md`）。実装側は `crates/pulsen-conformance/src/workflow_store.rs` の4アーム + `expect_parse_error` に加え、`crates/pulsen-conformance/HOOKS.md` の `TC-port-workflow-store-017` の組み立て手段も追従する（`expected_path_for_name` を新たに使うため）。

## 台帳の新規 ID（各グループの最大番号 + 1、末尾に追記）

現在の最大: `DOM-definition-056` / `DOM-task-079` / `DOM-execution-071` / `TC-port-run-store-034`。

| 新規 ID | 要素 | 由来 |
|---|---|---|
| DOM-definition-057 | `CommandLine.rehydrate` ドメイン関数 | B1 |
| DOM-task-080 | `ToolFailureKind` 値オブジェクト | B7 |
| DOM-task-081 | `RunDirPath.state_root` ドメイン関数 | B2 |
| DOM-execution-072 | `AliveDecision` 値オブジェクト | C3 |
| DOM-execution-073 | `DefaultJudgement` 値オブジェクト | C4 |
| DOM-execution-074 | `NotifyOutcome` 値オブジェクト | C5 |
| DOM-execution-075 | `NotificationService.interpret_notify_completion` ドメイン関数 | C5 |
| DOM-execution-076 | `NotifyFailureCause` 値オブジェクト | C5 |
| PAGE-wrapper-006 | wrapper の終了コード規約 | B3 |
| PAGE-tick-010 | tick サマリーの見出しの規約 | B11/C7 |
| TC-port-run-store-035 | write 系が置き場ごと作る適合ケース | B6 |

`spec/inventory/*.md` の `最終同期` はいずれも `2026-08-11` のまま。**全ファイルの日付を更新する**。

## 判断が要る件（いずれも「実装を直す」で決着。本 Issue で反映する）

### C5: `NotifyOutcome::Failed { detail }` を3変種の分類にすべきか

Issue が「あわせて判断が要る」とした件。

- **設計としては分類化が正しい。** `.adr/2-transition-error-holds-classification-only.md` は「表示専用のエラーは分類だけを持つ」を一般規則として宣言しており、`NotifyOutcome::Failed.detail` は帳簿に残らず `TickIssue::NotifyFailed { message }` を経て `cli::render` にしか流れない（`crates/pulsen/src/application/tick/notify.rs` の `report_failure` → `crates/pulsen/src/cli/render.rs:246`）。対称に見える `JudgeConclusion::JudgeFailure { detail }` は `last_failure = JudgeFail { detail }` として**帳簿に残る**ため `.adr/2-persisted-explanations-come-from-domain-describe.md` が効く側で、性質が違う。`.adr/3-notification-procedure-layering.md` が採った「隣の `interpret_judge_completion` と形を揃える」は見た目の対称性であり、規約の適用より弱い。
- **反映も本 Issue で行う。** `.adr/3-notification-procedure-layering.md` の代替案節は「分類化は spec 追従の提起に回す」と書いており、その「spec 追従の提起」が本 Issue（#9）である。ここでさらに別 Issue へ回すと ADR が指定した反映先が空振りし、同じ判断が3回目の Issue へ持ち越される。判断を下しながら現行の形を spec の正本に書き込むと、「書いてすぐ消す規約」を spec の履歴に残すことにもなる。
- **変更コストは小さい。** 参照は `crates/pulsen-domain/src/execution/notification.rs`（enum + `interpret_notify_completion` + ユニットテスト）/ **`crates/pulsen-domain/src/execution/mod.rs`（18行の再エクスポート。`mod notification;` は非公開なので `NotifyFailureCause` を `pub use` に足さないとコンパイルが通らない）** / `crates/pulsen/src/application/tick/mod.rs`（`TickIssue::NotifyFailed` の定義。203行）/ `crates/pulsen/src/application/tick/notify.rs`（2アーム + `report_failure`。115行）/ `crates/pulsen/src/cli/render.rs`（文言。246行）の**5ファイル**。結合テスト（`crates/pulsen/tests/tick_notify.rs:356` / `tick_scan.rs:546`）は `NotifyFailed { .. }` で受けているため変更を要さない。`cli/render.rs` の見出しの振り分け `issue_outcome`（132行）も `NotifyFailed { .. }` で受けているため変更を要さない。
- **決定**: `NotifyOutcome = Delivered | Failed { cause: NotifyFailureCause }`、`NotifyFailureCause = ExitedNonZero { exit } | TimedOut | FailedToStart { message }` に改める。`Failed` を平坦化して `NotifyOutcome` を4変種にはしない（`Delivered` / `Failed` の2分岐が at-least-once の規則そのものであり、`.adr/3-run-failure-cause-and-remnants-as-classifications.md` の `RunFailed { cause: RunFailureCause }` と同じ形に揃う）。`TimedOut` はフィールドを持たない（通知の timeout は設定値ではなく組み込み定数 `NOTIFY_TIMEOUT` の1つに定まるため、秒数は表示側が定数を読む）。あわせて `.adr/3-notification-procedure-layering.md` を更新する。

### A2: `WorkflowLoadError::Parse` に解決先を持たせるか

- **これは「判断が要る件」ではなく、Issue が既に決着させた件。** Issue 本文は「→ ポート表を `Parse { error, resolved_from }` へ改める。**ポート表は spec が確定させており**、Issue #1 の受け入れ基準（ポート表との1:1一致）の対象なので実装側では変えていない」と書いており、`spec` が正・実装を直す側だと明示している。実装が現在の形なのは #1 の受け入れ基準を守った結果にすぎない。
- **回避策の根拠は本 Issue で消える。** `.adr/1-workflow-error-file-path-goes-into-free-form-messages.md` が回避策を採った唯一の理由は「`WorkflowLoadError` … の12種は **spec のポート表で確定しており**、フィールドを増やすとポート表との1:1一致が壊れる」である。ポート表を直す Issue においてこの理由は成立しない。回避策を spec の契約節へ昇格させると、次の Issue で削除する規約を1つ増やすことになる。`.adr/1-schema-error-location-is-logical.md` の影響節も「ポート表に解決先を持たせる改訂（`WorkflowLoadError::Parse` にパスを添える）は **spec 側の追従として提起する**」と本 Issue を反映先に名指ししている。
- **変更コストは小さい。** `Parse` を参照するのは `crates/pulsen-domain/src/definition/port.rs`（enum 定義）/ `crates/pulsen/src/adapter/workflow_store.rs`（構築3箇所。73 / 78 / 79行）/ `crates/pulsen/src/cli/render.rs`（1アーム。535行）/ `crates/pulsen-conformance/src/workflow_store.rs`（4アーム。760 / 827 / 841 / 852行）の4ファイル8箇所（`grep -rn 'WorkflowLoadError::Parse' crates/` で確認）。`crates/pulsen/src/application/register_task.rs` は `WorkflowLoadError` を包むだけ、`crates/pulsen/tests/register_task.rs` と `crates/pulsen-conformance/src/doubles/` は `NotFound` しか使わないため変更を要さない。
- **`YamlSyntax { message }` へのパス前置は外す（決着済み）。** `WorkflowLoadError` の3変種のうち `NotFound { attempted }` と `Parse { resolved_from }` は解決先を構造として持ち、`Io { message }` だけが持たない。`WorkflowParseError` の12種はすべて `Parse` の内側にあるため、`Parse` の1フィールドで全種の解決先が示せる。現在パスを前置しているのは `crates/pulsen/src/adapter/workflow_store.rs:74` の `at(&resolved, &error.message)`（`YamlSyntax` のみ）で、`render.rs` が `resolved_from` を必ず出すようになると同じパスが1つの案内に2回現れる。したがって **前置に残るのは `Io { message }` の1変種だけ**とする（規則: 構造化フィールドで示せる経路は自由形式へ前置しない）。外しても既存テストは通る（受け入れテスト `crates/pulsen/tests/cli_add_error.rs:197 / 207` は `["YAML 構文エラー", "位置:", "行"]` の3語しか見ず、適合スイート `crates/pulsen-conformance/src/workflow_store.rs:435-437` も `message` の非空しか主張していない）。判断は adr.md ADR-005 に記録した。
- **適合スイートにも1主張を足す。** `crates/pulsen-conformance/src/workflow_store.rs:846` の `expect_parse_error` は `WorkflowParseError` だけを返して解決先を捨てるため、このままだと契約にフィールドが増えるのに適合ケースが1件もそれを検証しない。`(WorkflowParseError, PathBuf)` を返す形に改め、`tc_port_workflow_store_017` で `resolved_from == harness.expected_path_for_name("wf")` を主張する（`expected_path_for_name` は `crates/pulsen-conformance/src/lib.rs:394` に既にあり、`tc_port_workflow_store_001 / 003` が同じ形で使う。既定は `None` = skip 予算）。B6（`RunStore` の write 系の作成契約）で「契約を足したら適合ケースで主張する」形を採ったのと対称にする。
- **決定**: `spec/domains/definition.md#workflowstore` のエラー一覧を `Parse { error: WorkflowParseError, resolved_from: PathBuf }` に改め、spec 本体（上記の波及表 A2 行 = 定義・適合ケース13行・`spec/usecases/task.md:54`）・台帳（`DOM-definition-052` / `DOM-definition-055` / `ADP-workflowstore-001` / `UC-task-001` / `TC-port-workflow-store-017`〜`029`）と上記4ファイルを同時に直す。`location` が論理位置を指す契約と `WorkflowParseError` の12種の**形**は変えない。あわせて `.adr/1-workflow-error-file-path-goes-into-free-form-messages.md` と **`.adr/1-schema-error-location-is-logical.md`** を更新する（後者は決定節18行の理由「パスを載せられる場所が自由形式のメッセージに限られるため」と影響節31行のトレードオフ「スキーマ違反の案内にそのパスが出ない」が A2 で偽になるため。`location` を論理位置に限る決定本体は有効なまま）。

### 「実装側が正しい」と再確認した件（ADR の根拠を読んで妥当と判断）

A1 / A3 / A4 / A6 / B1 / B2 / B4 / B5 / B6 / B7 / B8 / B9 / B10 / B11 / C1 / C2 / C3 / C4 / C6 / C7 / C8 — いずれも ADR に「なぜ spec のままでは成立しないか」の根拠があり、CLAUDE.md の規約（型で不正な状態を排除する / 判断と表示の分離 / パニックを不変条件違反に限る）に沿う。A5 は ADR がないが、「同じ案内を参照回数だけ並べても直す先は1つ」という理由がコードの doc に残っており、spec の「全件まとめて返す」の意図（打ち切らない）とも矛盾しない。

## 依存関係

- 本 Issue は spec を主に変更し、`crates/` に触れるのは A2 / C5 の2件だけ（**合計9ファイル** = A2 の5 + C5 の5 − 重複1。`crates/pulsen/src/cli/render.rs` のみ両方が触る。A2 の5ファイル目は `crates/pulsen-conformance/HOOKS.md` — `TC-port-workflow-store-017` が新たに `expected_path_for_name` を使うため、フック対応表の1セルが動く）。それ以外の23件は spec の言い換えに閉じる。
- A2 / C5 の実装変更は spec・台帳・`.adr/`（3ファイル）と同一 PR で動かす。spec だけ先に変えると、その時点で新しい乖離を作る。
- 後続の完全性ゲート・`implement-audit`・`spec-to-issues` は台帳を基準に走るため、**台帳の追従漏れがそのまま下流の検証漏れになる**。本 Issue の最大のリスク。
- Issue #17（`try_kill_remnants` のポート契約変更）は #3 のコメントで明示的に本 Issue の対象外とされている。
