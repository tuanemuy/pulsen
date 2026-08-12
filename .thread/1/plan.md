# 実装計画 — Issue #1: [skeleton] 基盤・グローバル設定・ワークフロー定義とタスク登録(add)

**Issue:** #1
**作成日:** 2026-08-11
**複雑度:** 中〜大規模
**実装方針:** steps.md

---

## 目的

`pulsen add --workflow <name|path> --repo <path> [--base <branch>]` でタスクを登録できるようにし、以降の全スライスが乗る土台 — ドメイン基盤（definition 全体・task の帳簿と再構築・execution の一部ポート）、ファイルベースの永続バックエンド、共通ユーティリティ（アトミック置換・排他ロック）、ポート適合テストの枠組み、CLI の起動導線 — を確定する。

## 受け入れ基準

| # | 基準（検証可能な形で） | 由来 | 対応ステップ |
|---|---|---|---|
| AC-1 | `cargo build` / `cargo test` / `cargo clippy -- -D warnings` / `cargo fmt --check` が通る。ドメインクレートは外部クレートに依存しない（`Cargo.toml` の `[dependencies]` が空であることで機械的に保証される）。OS 依存分岐が隔離されている（`crates/*/src/` を対象に `#[cfg(unix)]` / `#[cfg(windows)]` を grep すると、`crates/pulsen-domain/` に1件も現れず、`crates/pulsen/src/` 側のヒットは `util/atomic.rs` だけであることを確認できる。`crates/pulsen-conformance/src/` と `crates/pulsen/tests/` のヒットは適合ハーネスに権限操作フックを供給するテスト側の分岐で、本番の実行経路には乗らない。確認手順は testing.md） | CLAUDE.md 技術方針・アーキテクチャ、Issue「OS 依存の処理はアダプター層に隔離する」 | 1, 全ステップ |
| AC-2 | definition ドメインの値オブジェクト・テンプレートが spec どおりに実装され、ユニットテストで網羅される（`parse` 経由でのみ生成、`NameError` / `DurationError` / `CommandError` / `TemplateError` / `ExpansionError` / `AgentDefError` の全分岐） | チェックリスト DOM-definition-001〜023, 030, 031 | 2 |
| AC-3 | 実効値の解決規則（`effective_agent` / `effective_model` / `effective_timeout` / `effective_retry_limit` の優先順位: ステータス上書き > ワークフローデフォルト > 既定）と `WorkflowRef::display_name` の4規則が spec どおりであり、区切り文字集合を明示したユニットテストで `/` と `\` の両方が検証される | DOM-definition-032〜044、TC-task-register-task-003/004/005/034/045 | 3 |
| AC-4 | `WorkflowAssembler::assemble` が `WorkflowParseError` の10種（`ForbiddenKey` / `MissingInitial` / `InitialNotFound` / `EmptyStatuses` / `NoAction` / `MultipleActions` / `UnknownRunValue` / `MissingNext` / `NextNotFound` / `InvalidValue`）を spec どおりに返し、循環・自己参照・到達不能ステータスは受理する（ADR-010） | DOM-definition-045、TC-port-workflow-store-013/014 | 4 |
| AC-5 | `RegistrationValidator::validate` が `RegistrationError` の5種を**全件まとめて**返し、成功時に `WorkflowSnapshot` を生成する | DOM-definition-049、TC-task-register-task-046 | 4 |
| AC-6 | `Task::register` / `Task::rehydrate` / `DegradedTask` が実装され、`ExecutionState` の6状態が付随データごと型で表現される。`rehydrate` は不変条件1（`task_status ∈ snapshot.statuses`）の破れを `RehydrateError::StatusNotInSnapshot` で返す。`Timestamp` は RFC3339（`YYYY-MM-DDTHH:MM:SSZ`）との往復をドメイン内で提供する（adr.md ADR-020） | DOM-task-001〜032・056・060・078・079 | 5, 6 |
| AC-7 | ポートのトレイトが spec/domains/{definition,task,execution}.md のポート表と1:1で一致する（メソッド名・引数・戻り値・エラー種。`TaskRepository` 7メソッド、`TargetError` 5種、`ConfigLoadError` 3種、`WorkflowLoadError` 3種、`LockError`）。未実装メソッドの宣言・スタブが1つも無い | DOM-definition-051〜055、DOM-task-062〜077、DOM-execution-059/060/061/065/068/069/070、Issue 完了条件 | 7 |
| AC-8 | ポート適合テストの枠組みが独立クレートとして存在し、1テストケース = 1 `#[test]` として spec の TC ID に対応づけて実行できる。ハーネスのフックは**破損・状況の意味**だけを受け取り（生 JSON 文字列を渡す API を持たない）、後続スライスの in-memory 実装がフックを実装するだけで同じスイートを通せる。**spec/testcases/ports/ の125行 × フックの対応表**が成果物としてあり、全行が「ポートのメソッドだけで組める / 特定のフックで組める / spec が明示するスキップ可」のいずれかで埋まっている。原子性の3ケース（TC-port-task-repository-042〜044）は `Sync` 境界をスイート全体に伝播させないスキップ可能フック経由で書かれている（adr.md ADR-027） | Issue「ポート適合テストの土台」 | 9 |
| AC-9 | ConfigStore アダプターが config-store 適合テスト**24件**を通す（全デフォルト・空ドキュメント `Ok`・未知キー `Invalid`・テンプレート内容は検証しない・キャッシュしない） | ADP-config-001、TC-port-config-store-001〜024 | 10 |
| AC-10 | WorkflowStore アダプターが workflow-store 適合テスト**31件**を通す（`.yml` フォールバックなし、相対パスは注入された基準ディレクトリで解決、`YamlSyntax`（重複キー含む）と `UnknownKey` をアダプターが検出、`resolved_from` は絶対パス） | ADP-workflowstore-001、TC-port-workflow-store-001〜031 | 11 |
| AC-11 | TaskRepository アダプターが task-repository 適合テスト**44件**を通す（tasks→archive の解決順・`Conflict` の現役/アーカイブ横断・`Corrupt` と `SnapshotUnreadable` の区別・命名形式外の非列挙・ディレクトリ自動作成・アトミック置換の観測面・`save_degraded` の破損スナップショット温存） | ADP-taskrepo-001〜007、TC-port-task-repository-001〜044 | 12 |
| AC-12 | Clock **5件** / TaskIdGenerator **5件** / ExclusiveLock **7件** / WorktreeManager（3メソッド分）**9件** = 合計26件の適合ケースが実装され、スキップする1件（clock TC-005）を除く**25件を各アダプターが通す**。ExclusiveLock は別プロセス間で排他し、保持プロセスの強制終了後に取得できる。`head_branch` は「ブランチ / detached / 空リポジトリ / 失敗」の4分岐すべてに到達できる。`TargetError::Failed` は**git を起動できない状況**で3メソッドとも到達でき（存在しないパスで構築した2つ目の manager を `failing_manager` が返す。adr.md ADR-024 / ADR-027）、権限操作にも root 実行の可否にも依存しない。Clock は5件中4件（TC-001〜004）が実行される（`observe_wall_clock` と実時間待ちの `advance` を `SystemClock` のハーネスが提供する）。TC-005（巻き戻し）のみ「時刻を過去に設定できるアダプター環境に限る」ケースとしてスキップし、理由を残す（下記スキップ運用） | ADP-clock-001・ADP-taskid-001・ADP-lock-001・ADP-worktree-001〜003、TC-port-clock-001〜005 他 | 13, 14 |
| AC-13 | グローバルホームが `--home` > `PULSEN_HOME` > `~/.pulsen/` の優先順位で解決され、全コマンドが起動時に `ConfigStore::load` を行い、`NotFound` では「未初期化である旨・解決後のホームパス・config.yaml の作成が必要であること」を表示して非0で終了する | PAGE-common-001/003/008、TC-task-register-task-014/067 | 16, 17, 19, 20 |
| AC-14 | `pulsen add` が spec の順序（ロック → ワークフロー解決 → 表示名決定 → 対象検証 → 登録時検証 → ID発行 → `create`）で動作し、成功時にタスクID・ワークフロー名・解決先パスを表示して 0 で終了する。`Conflict` は1回だけ再発行して再試行する | UC-task-001、PAGE-add-001〜009 | 16, 17, 18 |
| AC-15 | add の**拒否側**（異常系 TC-014〜048 と境界値の拒否ケース TC-053・054・055・058）で、タスクが作られず、かつワークフロー定義ファイル・config.yaml が変更されない（ロック競合、ワークフロー不在、YAML不正、未知キー、登録時検証エラー全種、リポジトリ/ブランチ不在、detached HEAD / 空リポジトリでの `--base` 省略、ID再衝突、I/Oエラー） | PAGE-add-010、PAGE-common-006、TC-task-register-task-014〜048・053〜055・058 | 16, 19, 20 |
| AC-16 | add の**受理側**で登録が成功する — `--workflow` の解釈規則4種（TC-049〜052）、`retries: 0`（TC-056）、`timeout: none`（TC-057）、`statuses` 1件（TC-059）、エッジケース8件（TC-060〜067: `state/` 自動作成、循環・到達不能・終端なしの受理、未参照の壊れたエージェント定義の受理、余分な値の許容、`judge` の `{...}` 非展開、`--home` > `PULSEN_HOME`） | TC-task-register-task-049〜052・056・057・059〜067 | 20 |
| AC-17 | 登録直後のタスクファイルが `state/tasks/<task-id>.json` に人間可読な JSON として作られ、検証済み定義のスナップショットが埋め込まれ、タスクステータス = `initial`・実行状態 = pending・カウンタ全0・workspace/attempt/failure 未設定である。`state/` 配下のディレクトリは自動作成される | PAGE-add-007、PAGE-common-009、TC-task-register-task-009/010/060 | 12, 16, 18, 20 |
| AC-18 | 実アダプターでは外から状況を作れない5行（TC-012 / TC-018 / TC-040 / TC-047 / TC-048）が、差し替えたポート実装（テストダブル）に対するユースケーステストとして実装され、実プロセス・実ファイルシステムを使わずに通る（adr.md ADR-028） | Issue 完了条件（スタブ・仮実装不可）、CLAUDE.md「実アダプターを差し替えられることを設計の健全性の指標とする」 | 15, 16 |
| AC-19 | アトミック置換と排他ロックが**それぞれ1箇所の共通ユーティリティ**として実装され、アダプターはそれを呼ぶだけである（個別再実装がない） | CLAUDE.md 技術方針 | 8, 14 |
| AC-20 | Issue のチェックリスト全行が実装され、各行が steps.md の対応表のとおりどこかのステップで消化されている（スタブ・仮実装なし） | Issue 完了条件 | 全ステップ |

## スコープ

### 含まれないもの

- `tick` / `wrapper` / `ls` / `show` / `abort` / `retry` / `set-status` の各コマンドとそのユースケース — 後続スライス。
- RunStore / ProcessController / CommandRunner の各ポートと実装、WorktreeManager の `create` / `remove` — 後続スライス。ポートのトレイトは**そのスライスでメソッドを足す**（本スライスで未実装メソッドをトレイトに宣言してスタブを置くことはしない）。
- `Task` の遷移関数のうち `register` / `rehydrate` 以外（`confirm_workspace` / `record_launching` / `complete_run` / `abort` / `retry` / `set_status` 等）と問い合わせメソッド（`execution_kind` / `current_status_def` / `applicable_retry_limit` 等）、`DegradedTask::abort` / `retry` / `mark_notified`、`WorkspacePlanner` — チェックリスト外（DOM-task-033〜055・057〜059・061）。
- execution ドメインの分類・判定・gc サービス（LaunchingClassifier / RunningClassifier / JudgementService / NotificationService / GcPolicy / IdentityCheck）。
- **汎用の in-memory アダプター**（適合テスト125件を通す実装）— 後続スライス。本スライスで置くのは「add の異常系検証に必要なポートに限ったスクリプト式テストダブル」だけ（adr.md ADR-028）。
- CI ワークフロー・リリース設定・パッケージング。

なお、機械可読出力（JSON 等）と config.yaml / ワークフローYAML の生成コマンドは**提供しない**が、PAGE-common-007 / PAGE-common-011 は「提供しないことを確認する行」としてチェックリストに載っており、消化対象である（steps.md ステップ17）。除外リストではない。

## チェックリスト行にチェックを付ける基準

Issue の完了条件は「実装をレビューで確認できた行にのみチェックを付ける。見送る行はチェックせず理由をコメントに残す」。判定を行単位で機械的に取れるよう、基準を次の2つに固定する。

**チェックを付ける**: 台帳行の「実装されるべき振る舞いの要点」に対応する実装とテストが存在し、そのテストが実際に走って通っていること。全コマンドを前提に書かれた台帳行（PAGE-common-002 / 003 / 005 / 006 / 010、UC-flow-007）は、**本スライスに存在するコマンド（`add`）の列がすべて満たされ、規則そのもの（ホーム解決・ロック取得・exit code・縮退4規則・タスクファイルの生涯）が実装として確定していれば付ける**。後続スライスがコマンドを足したときの適用は、そのコマンドの台帳行（`PAGE-tick-006` など。Issue #2〜#6 に配分済み）が受け持つ。この6行は Issue #1 にしか現れないため、見送るとどのスライスでも検証されない孤児になる。

**チェックを付けない**: 環境が前提を作れずケースが走らなかった行。適合スイートは走らなかったケースを `SKIP` として報告し、宣言した許容集合の外なら失敗させるので、「走っていないのに緑」と区別がつく。該当は `TC-port-clock-005`（時刻を過去に設定できない）の1件で、理由は Issue のコメントに残す。

記帳はレビュー指摘の修正がすべて入った後に行い、確認した実行環境（OS・root か否か・TMPDIR の位置）をコメントに明記する。走ったかどうかが出力から確認できる状態で数えるため。

## リスクと注意点

- **YAML パーサ選定が仕様適合の急所**: 未知キー拒否（ADR-013）・重複キー検出・エラー位置の3つを同時に満たす必要がある。`serde_yaml_ng 0.10` で実測確認済み（research.md）。serde の `deny_unknown_fields` に頼ると「未知キー」と「型不一致」がどちらも serde エラーになり `UnknownKey` と `InvalidValue` を区別できないため、**Value 化してから手書きでスキーマ走査する**方針を取る（adr.md ADR-021）。
- **`save_degraded` の往復可能性**: 壊れたスナップショットを消さずに書き戻す契約（TC-port-task-repository-009）は、ドメインの `DegradedTask` がスナップショットを持たないため「保存時に既存ファイルの当該フィールドをそのまま引き継ぐ」実装でしか満たせない。ここを取り違えると修復材料を消す。加えて TC-009 の前提「タスク側フィールドを変更（`abort` による Stopped 化等）して `save_degraded`」の `DegradedTask::abort` は本スライスのスコープ外なので、適合ケースは `DegradedTask` の再構築コンストラクタで変更後の値を直接組み立てる。
- **`Corrupt` と `SnapshotUnreadable` の境界**: JSON として有効かどうかが分岐点になる（adr.md ADR-025）。不変条件2〜4（Running なのに process が無い等）は**デコードで検証してはいけない**（`Intact` で返す。TC-port-task-repository-026）。過剰検証は tick スライスの `InvariantViolated` 経路を壊す。
- **クロスプラットフォーム**: ロック（`File::try_lock`）・アトミック置換（同一ディレクトリ内 `rename`）・パス区切り（`WorkflowRef::parse`）・行末。Windows 実機検証も Windows ターゲットへのクロスチェックも本スライスでは行えない（nix の rustc に rustup が無く `x86_64-pc-windows-msvc` の std が入らないことを実測確認）。代わりに AC-1 の grep で OS 依存分岐の隔離を機械的に確認する。
- **git への依存**: WorktreeManager アダプターは `git` CLI へシェルアウトする（adr.md ADR-024）。`flake.nix` の devShell に `git` が無いため、テストが環境依存で落ちる。devShell への追加が必要。フィクスチャ側の環境固定は adr.md ADR-033。
- **`head_branch` の判定**: `symbolic-ref --short HEAD` は**空リポジトリでも exit 0 でブランチ名を返す**（実測）。単独では `EmptyRepository` を検出できず「空リポジトリで `--base` 省略 → 誤った登録成功」になる。`rev-parse --verify --quiet HEAD` との組み合わせで判定する（adr.md ADR-024）。
- **`TargetError::Failed` はメタデータ破損では作れない**: `.git/HEAD` の破壊・`objects` の削除・`.git` の権限剥奪・gitdir ポインタの不在・`config` の構文不正・`repositoryformatversion` の不正の6パターンすべてで `rev-parse --show-toplevel` が exit 128 になり、`NotARepository` と区別できない（実測）。git 実行ファイルのパスを構築時に注入し、**存在しないパスで構築した2つ目の manager**（ハーネスの `failing_manager`）で `Failed` を作る（adr.md ADR-024 / ADR-027）。本番アダプターはイミュータブルなまま保つ。
- **適合テストの環境依存ケース**: 「権限不足等。再現できるアダプター環境に限る」と spec が明示するケースは、ハーネスのフックが `None` を返したらスキップする。root で走らせると `chmod 000` が効かないため、**権限操作系のフックは制限が実際に効いたことを確認してから `Some` を返す**（効かなければ復元して `None` = スキップ。確認を省くと期待した `Err(Io)` の代わりに `Ok` を観測して FAIL する。adr.md ADR-027）。対象は `TC-port-config-store-023` / `TC-port-workflow-store-030` / `TC-port-task-repository-005・011・012・019・035・041` / `TC-port-clock-005` の9件で、いずれも Issue のチェックリスト行である。**スキップで終わった行はチェックせず、スキップした旨と環境上の理由を Issue のコメントに残す**（手動確認で落とす手順と同じ運用。Issue の完了条件「見送る行はチェックせず、理由をこの Issue のコメントに残す」に従う）。`LockError::Failed`（TC-port-exclusive-lock-007）はロックパスにディレクトリを置く手段（adr.md ADR-032）、`TargetError::Failed`（TC-port-worktree-manager-009）は存在しないパスで構築した別 manager（adr.md ADR-024 / ADR-027）、`Clock` の TC-003・004 は `SystemTime` の観測と実時間待ち（adr.md ADR-027）で、いずれも環境非依存に再現できるためこの9件には含まれない。
- **原子性の観測面と `Sync`**: TC-port-task-repository-042〜044 はポートの並行利用を前提とするため、スイートに `Sync` 境界が要る（実測: 境界なしでは `E0277`）。境界を無条件に置くと後続スライスの `RefCell` ベース in-memory 実装がスイート全体を適用できなくなるので、当該3ケースだけをスキップ可能フック（`concurrent_repo`）に隔離する（adr.md ADR-027）。
- **後続スライスとの整合**: トレイトのメソッドをスライスごとに足していく方針のため、本スライスで確定するトレイトの**引数・エラー型**が spec と食い違うと後続で破壊的変更になる。AC-7 でポート表との1:1一致を明示的に検証する。
- **マニュアルテストの部分実行**: spec/manual-tests/setup.md・task-execution.md は `ls` / `tick` / `show` 前提の手順を含み、本スライスでは手順の途中までしか実行できない ID がある。下記「手動確認」に ID 単位で範囲を書く。

## テスト方針

- **ドメイン**（`cargo test` のユニットテスト、I/O なし）: 値オブジェクトの `parse` 全分岐、テンプレート解析・展開、`effective_*` の優先順位、`display_name` の4規則（区切り文字集合は `/` のみ・`/` と `\` の両方をそれぞれ明示的に渡す）、`Timestamp` の RFC3339 往復とうるう年・不正日付の拒否、`WorkflowAssembler` の全エラー種、`RegistrationValidator` の全件収集、`Task::register` の事後条件、`rehydrate` の不変条件1検証。テスト名は仕様の言葉（例: `空文字列のプロンプトは受理されない`）で付ける。
- **ポート適合テスト**（`pulsen-conformance` クレート）: spec/testcases/ports/ の表を1行 = 1テストとして実装し、マクロでアダプターに適用する。ハーネスのフックは意図レベル（adr.md ADR-027）にして、後続スライスの in-memory 実装にも同じスイートをそのまま適用できる形にする。本スライスでは fs / システム実装に適用する。
- **ユースケース**: `RegisterTask` はポートをジェネリック引数で受け取り、テストは**すべてテストダブル**（`pulsen-conformance::doubles`）に対して書く。実プロセス・実ファイルシステムは使わない。実アダプターでは作れない TC-012 / 018 / 040 / 047 / 048 はここで消化する（adr.md ADR-028）。
- **CLI**: `tests/` の統合テストでバイナリを起動し、spec/testcases/task/register-task.md の67件のうちユースケース層で消化する5件を除く62件を、実アダプター（一時ホーム + `git init` した一時リポジトリ）に対して exit code と出力内容で検証する。権限操作でしか再現できない TC-016（config.yaml が読めない）・TC-021（ワークフロー定義が読めない）は POSIX のみで実行し、root では skip する。
- **手動確認**: Issue の「検証 / 手順書」の一覧に揃える。範囲は**手順書を実際に読んで ID 単位で**書く。本スライスに無いコマンド（`ls` / `tick` / `show` / `abort` / `set-status`）を使う手順は実行しない。読み替えの型は2つ — (a) 確認手段の `pulsen ls` / `pulsen show` は `state/tasks/` の直接確認に、(b) 起動時 config 読み込みの確認としての `pulsen ls` / `pulsen tick` は `pulsen add` に読み替える。

  | 手順書 | ID | 本スライスでの実行範囲 |
  |---|---|---|
  | setup.md | TC-01 | 手順1のみ。手順2の `pulsen ls` は、TC-03 の定義配置後に `pulsen add` を実行して「パースエラー・未初期化の案内が出ない」ことで代替する（読み替え b） |
  | setup.md | TC-03 | 手順1〜2。手順3（`ls`）・手順4（`show`）は実行せず、`state/tasks/<task-id>.json` の直接確認に読み替える（読み替え a） |
  | setup.md | TC-04 | 手順1のみ。手順2（`ls`）は `state/tasks/` に別IDのタスクファイルが2件あることの確認に読み替える（読み替え a） |
  | setup.md | TC-07 | 手順1〜2。手順3以降は tick / show |
  | setup.md | TC-08 | 手順1〜2。手順3以降は tick / show（スナップショット独立の確認自体は本スライスでは行えない） |
  | setup.md | TC-12 | 手順2のみ。手順1・3（`ls` / `tick`）と手順4の回復確認（`ls`）は実行しない。未初期化ホームで `add` が案内付きで非0終了しタスクが作られないことを確認する（AC-13 の直接の裏付け） |
  | setup.md | TC-13 | 手順1・2・4・5。手順3（`ls`）は実行しない。手順5の回復確認は `add` の再実行に読み替える（読み替え b） |
  | setup.md | TC-14 | **`add` の手順が無いため全面的に読み替える**。手順1で typo キーを追記し、`pulsen add` が未知キーのエラーで非0終了することを確認、手順3の回復後に `add` が成功することを確認する（読み替え b） |
  | setup.md | TC-15 | 手順3・4（ワークフロー定義の権限）。手順1・2（config.yaml の権限）は `ls` を `add` に読み替えて実行する（読み替え b）。POSIX のみ・root では実行しない |
  | setup.md | TC-16〜TC-28 | 全手順（片付けの `abort` / `set-status` / `tick` を除く）。TC-16 手順2の「`pulsen ls` にタスクは現れない」は `state/tasks/` の直接確認に読み替える（読み替え a） |
  | setup.md | TC-29〜TC-33 | 全手順（片付けの `set-status` / `tick` を除く） |
  | setup.md | TC-40・TC-41 | 全手順（片付けを除く） |
  | setup.md | TC-42 | **`add` の手順が無いため全面的に読み替える**。手順1で空の config.yaml を置き、手順2・3（`ls` / `tick`）を `pulsen add --home` に読み替えて、全デフォルトで config が受理されることを確認する（読み替え b）。AC-9（config-store の全デフォルト）の唯一の手動裏付け |
  | setup.md | TC-43〜TC-46 | 全手順（片付けの `abort` / `set-status` / `tick` を除く） |
  | setup.md | TC-48 | 全手順（`ls` を `add` に読み替え。読み替え b。AC-13 の直接の裏付け） |
  | task-execution.md | TC-01 | 手順3・6・7（手順1・2・4・5・8 は ls / tick / show / set-status。手順6・7 はシェルの `ls` によるディレクトリ確認なので実行できる） |
  | task-execution.md | TC-02 | 手順1のみ（手順2以降は ls / tick / show） |
  | task-execution.md | TC-08〜TC-11 | 全手順（確認手段の `pulsen ls` は `state/tasks/` の直接確認に読み替え。読み替え a） |

  上表で落とす手順（tick / ls / show / abort / set-status を要する部分）は、Issue の完了条件に従い「見送る行はチェックせず理由を Issue のコメントに残す」運用に合わせる。適合テストがスキップで終わった行も同じ運用にする（上記「リスクと注意点」）。
