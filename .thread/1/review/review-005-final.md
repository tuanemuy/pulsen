# 最終レビュー（5周目・収束確認） — Issue #1 / PR #8

対象: `.thread/1/review/changed-files-005.txt` の158件。契約は `.thread/1/plan.md`、過去判定は `.thread/1/review/triage.md`（ラウンド1〜4）。

1周目の `wont-fix` 3件（W-004 `unreachable!` / W-012 成功時の tick 案内 / W-017 `create` の TOCTOU）は対象外とし、本レビューでは一度も再指摘していない。

**問題点ゼロではない。** Blocker は無いが、実行して再現できる欠陥1件と、この PR 自身が定めた規約への違反2件を検出した。いずれも4周目の修正が生んだ回帰ではない。

## 実行結果

| 確認 | 結果 |
|---|---|
| `cargo build --all-targets` | 成功 |
| `cargo clippy --all-targets -- -D warnings` | 警告0 |
| `cargo fmt --check` | 差分なし |
| `cargo test --all` ×3回連続 | 全回合格（14バイナリ・失敗0）。実行時間・件数とも回ごとの揺れなし＝フレーキーなし |
| `cargo test -- --nocapture \| grep -i skip` | 実アダプターのスキップは `tc_port_clock_005` の1件のみ（`conformance_time_id` 単体実行で確認）。他の SKIP 行は `pulsen-conformance` 自身の `SkipBudget` ユニットテストの出力 |
| `grep -rn "todo!\|unimplemented!\|FIXME\|TODO\|XXX" crates/` | 0件 |
| 本番コードの `unwrap` / `expect` / `panic!` / `unreachable!` | `definition/template.rs:198` の `unreachable!`（1周目 W-004 の wont-fix）1件のみ。`unwrap()` は `src/` 配下に0件 |
| 実バイナリ `pulsen add` | 正常系（タスクID・ワークフロー名・解決先の表示、exit 0、タスクファイルの内容）と異常系4種（未初期化ホーム・ワークフロー不在・リポジトリ不在・未知フラグ）を目視。表示はすべて人間可読な日本語、exit code は 0 / 1 / 2 で spec どおり |

## Blockers

なし。

## Warnings

- **[W-001]** ExclusiveLock 適合スイートの許容スキップ宣言が無条件の空集合で、`cargo test --test conformance_lock` が4件 FAILED になる
  - 場所: `crates/pulsen/tests/conformance_lock.rs:99`（前提の主張は `crates/pulsen/tests/common/lock.rs:13-14`）
  - 理由: `common/lock.rs` の doc は「`cargo test` は example もビルドするため、`examples/` に**必ず**置かれる」と書くが、これが成り立つのはパッケージ全体を対象にした実行だけである。`cargo test --test conformance_lock` は example をビルドしないため `holder_program()` が `None` を返し、`hold_from_other_process` / `try_acquire_from_other_process` が `None` になる。宣言が `Vec::new()` なので `SkipBudget::record` が `assert!` で落ち、TC-port-exclusive-lock-002 / 003 / 004 / 005 の4件が失敗する。**実測で再現**（`target/debug/examples/lock_holder` を退避して `cargo test -p pulsen --test conformance_lock` → `FAILED. 3 passed; 4 failed`。確認後に復元し `7 passed` に戻ることも確認）。失敗メッセージは「この環境で前提条件を用意できるようにするか、宣言を実態に合わせる」と案内するが、環境にもアダプターにも問題はなく、読み手を誤誘導する。
    まったく同じフックについて、受け入れテスト側は `crates/pulsen/tests/common/mod.rs:32,45-47` が `lock::holder_program().is_none()` で実行時に判定して `LOCK_HOLDER_CASES` を許容集合に入れており、扱いが2つに割れている。これは3周目 R3-W-001（`conformance_worktree.rs` が `Vec::new()` を宣言していた件）で確定した基準 — ADR-055「宣言はプラットフォームではなく環境の能力に対応させる」— の適用漏れが1箇所だけ残ったもの。`conformance_config_store.rs` / `conformance_task_repository.rs` / `conformance_workflow_store.rs` / `conformance_worktree.rs` は全て `allowed_skips()` で実行時に判定しており、無条件の `Vec::new()` は本ファイルだけである（`conformance_time_id.rs:88` の TaskIdGenerator は唯一の `Option` フック `another_generator` が常に `Some` を返すため妥当）。
    なお AC-1 が定めるゲート（`cargo test`）は常に緑であり、`.adr/032` の「別プロセスの保持は環境非依存に組める」という判断自体は正しい。壊れているのは「example が必ず存在する」という前提のほうである。
  - 提案: `conformance_lock.rs` に `allowed_skips()` を置き、`common::lock::holder_program().is_none()` が真のときだけ TC-002〜005 の4件を許容集合に入れる（`conformance_worktree.rs:119-125` と同じ形）。あわせて `common/lock.rs:13-14` の「必ず置かれる」を「パッケージ全体を対象にした実行ではビルドされる。単一テストターゲット指定では作られないため、不在は前提を作れない環境として扱う」に直す。

- **[W-002]** この PR が起票した ADR-038 の見出し規約を、同じ PR の ADR 3件が満たしていない
  - 場所: `.adr/050-schema-error-location-is-logical.md:22`、`.adr/054-workflow-error-file-path-goes-into-free-form-messages.md:23`（いずれも `検討した代替案:` が本文の平文）、`.adr/048-parse-inputs-at-spec-flow-position.md:22`（`### 検討した代替案` と見出しレベルが1段深い）。順序の破れとして `.adr/025-task-file-json-and-corrupt-classification.md`（`## 検討した代替案` が13行目、`## 決定` が17行目で逆順）
  - 理由: `.adr/038-adr-filing-format.md:15` は「見出しは `## ステータス` / `## コンテキスト` / `## 決定` / `## 検討した代替案`(あれば) / `## 影響`」と定め、その効果として「`.adr/` 全体で見出しが1つに揃い、機械的な確認が全件に効く」を掲げている。実測で `grep -l '^## 検討した代替案' .adr/*.md` は 048 / 050 / 054 を取りこぼす（038 自身のヒットは規約本文の引用）。既存の `.adr/001`〜`.adr/018` は18件すべてが規約どおりの見出しと順序であり、破れているのは本 PR が追加した側だけ。1周目 W-027 / W-028、2周目 R2-W-017、3周目 R3-W-004 が「`.adr/` は後続スライスが根拠を辿る正本であり、自ら明文化した規約の破れは放置しない」として扱ってきたのと同じ性質。実装の振る舞いには影響しない。
  - 提案: 048 / 050 / 054 の当該行を `## 検討した代替案` に揃え、025 は `## 検討した代替案` を `## 決定` の後ろへ移す。

- **[W-003]** `.thread/1/adr.md` の ADR-041 に、出荷ドキュメントと食い違う古い集計が残っている
  - 場所: `.thread/1/adr.md:715`
  - 理由: 「125行すべてが『ポートのみ28件 / フック86件 / spec が明示するスキップ可11件』のいずれかに埋まり」とあるが、出荷物 `crates/pulsen-conformance/HOOKS.md:19-24` の集計は A 28 / B 85 / C 12 で、125行の実体もそちらと一致する（1周目 W-036(b) で TC-port-clock-003 を B → C に直した結果）。ADR-041 は `.adr/` に昇格せず内容を `.adr/027` に畳んだため、この記述が当該集計の唯一の記録として残り、修正が届いていない。AC-8 の「125行 × フックの対応表」を `adr.md` から辿る読み手が1行ずれた集計を読む。
  - 提案: `.thread/1/adr.md:715` を「ポートのみ28件 / フック85件 / spec が明示するスキップ可12件」に直す（正は `HOOKS.md` 側）。

## 受け入れ基準の検証（AC-1〜AC-20）

| # | 結果 | 検証方法と根拠 |
|---|---|---|
| AC-1 | **合格** | `cargo build` / `cargo test` / `cargo clippy --all-targets -- -D warnings` / `cargo fmt --check` を実行し全て成功。`crates/pulsen-domain/Cargo.toml` の `[dependencies]` は空（コメントのみ）。`grep -rn 'cfg(unix)\|cfg(windows)\|cfg(not(unix))' crates/*/src/` のヒットは `crates/pulsen/src/util/atomic.rs` 2件と `crates/pulsen-conformance/src/lib.rs` 2件のみで、`crates/pulsen-domain/` は0件。AC-1 の文言（テスト側ハーネスの分岐は対象外）と一致 |
| AC-2 | **合格** | `NameError` / `DurationError` / `CommandError` / `TemplateError` / `ExpansionError` / `AgentDefError` の全分岐にユニットテストが対応。生成経路は `parse` のみ（`InputText::new` は制約なし型として ADR/progress に記録済みの既知の逸脱） |
| AC-3 | **合格** | `effective_agent` / `effective_model` / `effective_timeout` / `effective_retry_limit` の優先順位（ステータス上書き > ワークフローデフォルト > 既定）と `Cleanup` の固定値を確認。`display_name` の4規則は区切り文字集合を注入する純関数で、`/` と `\` の双方を明示的に渡すテストがある |
| AC-4 | **合格** | `WorkflowParseError` のアセンブラ側10種すべてがテストで生成される（`YamlSyntax` / `UnknownKey` は spec どおりアダプター生成）。循環・自己参照・到達不能ステータスの受理（ADR-010）は `workflow.rs` と `assembler.rs` の双方で固定 |
| AC-5 | **合格** | `RegistrationValidator::validate` は短絡せず全件収集。複数種を同時に返す台本のテストで要素数と内訳を assert。成功時に `WorkflowSnapshot` を生成。`UnknownAgent` / `InvalidAgentDefinition` の値同一の重複だけを畳む（2周目 R2-W-010）実装で、情報は失われない |
| AC-6 | **合格** | `Task::register` / `rehydrate` / `DegradedTask` が存在し `ExecutionState` の6状態は付随データごと型で表現。`rehydrate` は不変条件1の破れを `RehydrateError::StatusNotInSnapshot` で返す。`Timestamp` は外部クレートなしで RFC3339 往復（うるう年・境界・書式厳密性のテストあり） |
| AC-7 | **合格** | `task/port.rs` / `definition/port.rs` / `execution/port.rs` を spec のポート表と突き合わせて1:1一致を確認（`TaskRepository` 7メソッド、`TargetError` 5種、`ConfigLoadError` 3種、`WorkflowLoadError` 3種、`LockError`）。`todo!` / `unimplemented!` / スタブは0件で、後続スライスのメソッドは宣言自体がない |
| AC-8 | **合格** | `pulsen-conformance` が独立クレートとして存在し、1ケース = 1 `#[test]`。`HOOKS.md` の対応表は spec/testcases/ports/ の125行と1対1（A 28 / B 85 / C 12）。フックは意味だけを受け取り生 JSON の口は無い（YAML ソースの口は `.adr/053` で範囲を限定して明文化済み）。原子性の3ケースは `concurrent_repo` フックに隔離。**ただし W-001 のとおり、宣言の実行時判定が ExclusiveLock で1箇所抜けている** |
| AC-9 | **合格** | `cargo test --test conformance_config_store` = 24 passed。spec の config-store 24行と1対1 |
| AC-10 | **合格** | `cargo test --test conformance_workflow_store` = 31 passed。spec の workflow-store 31行と1対1 |
| AC-11 | **合格** | `cargo test --test conformance_task_repository` = 44 passed。spec の task-repository 44行と1対1 |
| AC-12 | **合格** | Clock 5 / TaskIdGenerator 5（`conformance_time_id` = 10 passed）/ ExclusiveLock 7 / WorktreeManager 9 = 26件。`--nocapture` で実行して、スキップは `tc_port_clock_005` の1件のみ・TC-001〜004 は実行されることを確認。合計は 24+31+44+5+5+7+9 = **125件**で spec の該当行数と一致（worktree-manager の残る12行は `create` / `remove` 用でスコープ外） |
| AC-13 | **合格** | `cli/wire.rs` が `--home` > `PULSEN_HOME`（空文字は未設定扱い）> `env::home_dir()/.pulsen` の順で解決。実バイナリで未初期化ホームを与え、「グローバルホームが未初期化です。」＋解決後のホームパス＋作成すべき config.yaml のパスを表示して exit 1 になることを目視 |
| AC-14 | **合格** | `application/register_task.rs` が spec の順序（ロック → ワークフロー解決 → 表示名 → 対象検証 → 登録時検証 → ID発行 → `create`）で動作。順序点は空の台本を持つダブルのパニックで固定されている。`Conflict` は1回だけ再発行して再試行（3回目が無いことも assert）。実バイナリの成功時出力にタスクID・ワークフロー名・解決先の絶対パスが揃い exit 0 |
| AC-15 | **合格** | 異常系31件・境界値の拒否ケース4件のすべてが `has_no_task()` と `Untouched::assert_unchanged()` の両方を通る。`Untouched` は控えたファイルの内容に加えて `workflows/` とホーム直下のエントリ集合も見るため、新規ファイルの出現も落ちる |
| AC-16 | **合格** | TC-049〜052 / 056 / 057 / 059 / 060〜067 がいずれも存在し、exit code だけでなく永続化されたタスクファイルの内容を assert |
| AC-17 | **合格** | 実バイナリで登録し `state/tasks/<task-id>.json` を目視。人間可読な整形 JSON、検証済みスナップショット埋め込み、`task_status` = `queued`（= `snapshot.initial`）、`execution.state` = `pending`、カウンタ3種すべて0、`workspace` / `current_attempt` / `last_failure` が `null` キーとして存在。`state/` 配下は自動作成される |
| AC-18 | **合格** | TC-012 / 018 / 040 / 047 / 048 が `tests/register_task.rs` に `tc_task_register_task_NNN_*` 命名で存在し、`pulsen_conformance::doubles` のみを相手に走る（実プロセス・実ファイルシステム不使用） |
| AC-19 | **合格** | `write_atomic` / `rename_atomic` は `util/atomic.rs` の1箇所のみ。`fs::rename` / `NamedTempFile` / `persist` は他に無い。`try_lock` は `adapter/lock.rs` の1箇所のみ |
| AC-20 | **合格** | `gh issue view 1` で `- [x]` **345件** / `- [ ]` **1件**（`TC-port-clock-005`）を確認。見送り1件は plan.md の基準どおりで、理由は Issue コメントに投稿済み。register-task の TC ID は `grep` で **001〜067 の67件すべて**がテスト名に現れることを確認（欠番なし） |

**総評: AC-1〜AC-20 は20件すべて合格。** W-001 は AC-8 が約束する「後続スライスの実装に同じスイートを適用する」運用で効いてくる宣言の抜けだが、AC が要求する `cargo test` は緑であるため AC-8 は合格と判定した。

## 4周目の修正の確認（回帰なし）

| 修正 | 確認結果 |
|---|---|
| `adapter/task_repository.rs` の `unreachable_entry` を唯一の述語にし、`lookup` / `list` の双方が通る形にした | `find` と走査が同じ区分を返すことを unix 限定のユニットテスト3本（検索 / 走査 / 作成）が固定。`symlink_metadata` が `NotFound` 以外の `Err` を返す枝は両呼び出し元とも `?` で伝播しており握り潰しは無い。回帰なし |
| `exists` をリンクを辿らない判定に変更（`create` の一意性判定） | `symlink_metadata` は `try_exists` より**保守的**にしかならない — 宙ぶらりんのリンクもリンクループも `Ok(true)` になり、分岐の差はすべて `Conflict` に倒れる。黙って上書きする経路は生じない。中間パス要素の解決は従来どおりで、`ENOTDIR` / `EACCES` は `CreateError::Io` として出る。**副作用なし** |
| `save` / `archive` の存在確認を `try_exists` のまま残した | 到達できないエントリは読みでは `Corrupt`、書きでは `NotFound` という非対称が生じるが、これは `.adr/064` の決定4番目そのもので、`port.rs` の「破損への書き込み禁止」（呼び出し側の責務）と整合する。`crates/pulsen-domain/src/task/port.rs` の契約 doc も「消失と到達不能」の節で `find` を含む形に書き直されており、doc と実装の食い違いは無い |
| 適合ケース44件が新しい契約と矛盾しないか | 44件すべて緑。フックの集合では「到達できないエントリ」を作れないため、44件のどれもこの状況に期待を置いていない。TC-port-task-repository-029（`find` と走査は同じ区分）が一般の契約側を見ており、個別状況はアダプターのユニットテストが受け持つ、という `.adr/064` の記述どおりの分担になっている |
| `tests/cli_add_normal.rs` TC-010 が2件目の `add` でスナップショットの凍結を検証 | `assert_ne!(edited, WORKFLOW)` で書き換えが実際に効いたことを先に固定し、その後の2件目の登録で「既存タスクは登録時点のまま」「新しいタスクには編集後の内容」の両方を見ている。片方だけでは成立しない主張になっており、テスト名（ADR-015 の独立性）と検証が一致した |
| `tests/cli_add_normal.rs` TC-009 が `Value::get` でキー存在を確認 | `task.get(key) == Some(&Value::Null)` はキー不在で落ちる。`task_status` / `snapshot.initial` もリテラル `"queued"` に対して固定されており、両方が不在でも真になる比較は解消済み |
| `.adr/064` の新設 | 決定4項目すべてが実装と一致。既存 `.adr/001`〜`.adr/018` および 019〜063 と矛盾しない |

## 横断確認

- **ヘキサゴナルの依存方向**: `crate::adapter::` を import するのは `crates/pulsen/src/cli/wire.rs` の1ファイルのみ（`grep` で確認）。合成ルートは1箇所。`crates/pulsen/src/application/` は `pulsen_domain` と `std` 以外を import しない。`cli/render.rs` にアダプター型は現れない（1周目 W-005 の状態に戻っていない）
- **`pulsen-domain` の外部クレート非依存**: `Cargo.toml` の `[dependencies]` は空
- **関数型ドメインモデリング**: newtype とフィールド非公開 + `parse`、エラーは値、`match` のドメイン enum に対するワイルドカードなし（`wildcard_enum_match_arm = warn` がドメインクレートに掛かっており clippy が緑）、更新は `self` を消費する形
- **spec 適合**: `spec/domains/*.md` のポート表・値制約、`spec/usecases/task.md` の処理順、`spec/pages/index.md` の出力規則（人間可読・exit code・案内に添える情報）と一致
- **テストケースの網羅**: 適合 **125件**（spec の該当行数と一致）、register-task **67件**（TC ID 001〜067 に欠番なし）
- **Issue #1 のチェックリスト346行**: 345チェック / 1見送り（`TC-port-clock-005`）。plan.md の記帳基準どおり
- **`.adr/019`〜`.adr/064` と実装の整合**: 40件すべての「決定」を実装と突き合わせて矛盾なし。既存 `.adr/001`〜`.adr/018` とも矛盾しない。欠番 041 / 047（既存 ADR に畳んだ）・056〜059（採番のみ）は `.thread/1/adr.md:5` と `.adr/035` に明記されている。**書式のみ W-002**
- **plan.md「含まれないもの」の混入**: なし。コマンドは `add` のみ、ポートは7種のみ（`WorktreeManager` は3メソッド）、`Task` の遷移は `register` / `rehydrate` のみ、execution ドメインにサービスは無く、汎用 in-memory アダプターも CI / リリース設定も存在しない
- **コメントの質**: `crates/**` と `.adr/**` に「修正した」「指摘により」「以前は」「変更点」等の経緯・弁明は0件（`grep` で確認）。残っているのは why / why not と doc コメントのみ
- **フレーキー・ハング・並列干渉**: `cargo test --all` を3回連続実行して全回同一の結果。`std::env::set_var` / プロセス全体の `set_current_dir` は0件（環境と cwd はすべて `Command` 単位）。受け入れテストは毎回 `HOME` / `USERPROFILE` を一時ディレクトリへ向けるため実 `~/.pulsen` に漏れない。並行観測のスピンは `StopOnDrop` で巻き戻し時に必ず停止し、ロック保持プロセスのハンドシェイクは stdout の EOF で必ず戻る
- **`unwrap` / `expect` / `panic!` / `match` のワイルドカード / `todo!` / スタブ**: 本番コードに残るのは `definition/template.rs:198` の `unreachable!`（1周目 W-004 の wont-fix）1件のみ

## カバレッジ

一覧158件と1対1で対応する。

### 確認（137件）

- ADR（40件）: `.adr/019-domain-crate-workspace.md`, `.adr/020-no-serde-in-domain-timestamp-conversion-in-domain.md`, `.adr/021-yaml-value-then-hand-written-schema-walk.md`, `.adr/022-std-file-lock-and-lockguard-marker-trait.md`, `.adr/023-dependency-selection.md`, `.adr/024-git-cli-shell-out-and-target-classification.md`, `.adr/025-task-file-json-and-corrupt-classification.md`, `.adr/026-task-id-format.md`, `.adr/027-port-conformance-suite-and-harness-hooks.md`, `.adr/028-usecase-error-paths-via-test-doubles.md`, `.adr/029-wildcard-enum-match-arm-lint-domain-only.md`, `.adr/030-workflow-store-base-dir-injection.md`, `.adr/031-pulsen-home-layout-in-application-layer.md`, `.adr/032-lock-holder-example-fixture.md`, `.adr/033-git-fixture-reproducibility.md`, `.adr/034-workflow-ref-separator-set-as-constant.md`, `.adr/035-file-slice-adrs-from-019.md`, `.adr/036-infallible-ports-absorb-failure-at-construction.md`, `.adr/037-platform-separator-set-without-cfg.md`, `.adr/038-adr-filing-format.md`, `.adr/039-read-error-shared-by-find-and-list.md`, `.adr/040-rehydrate-takes-field-bundle.md`, `.adr/042-absent-yaml-value-is-omission.md`, `.adr/043-store-adapters-receive-injected-paths.md`, `.adr/044-task-file-layout-in-domain.md`, `.adr/045-task-file-dto-generic-over-snapshot.md`, `.adr/046-no-skippable-hooks-for-post-operation-observation.md`, `.adr/048-parse-inputs-at-spec-flow-position.md`, `.adr/049-base-flag-allows-hyphen-values.md`, `.adr/050-schema-error-location-is-logical.md`, `.adr/051-undisplayable-name-fixture-is-whitespace-stem.md`, `.adr/052-acceptance-test-harness-in-tests-common.md`, `.adr/053-conformance-yaml-source-hooks.md`, `.adr/054-workflow-error-file-path-goes-into-free-form-messages.md`, `.adr/055-conformance-skip-budget.md`, `.adr/060-non-blocking-lock-case-observes-from-a-second-thread.md`, `.adr/061-unused-public-accessors-are-kept-only-for-verified-layout.md`, `.adr/062-acceptance-tests-detach-the-user-home.md`, `.adr/063-concurrent-observation-stops-the-reader-on-unwind.md`, `.adr/064-unreachable-entry-is-corrupt-in-find-and-create.md`
- スライス作業記録（6件）: `.thread/1/adr.md`, `.thread/1/plan.md`, `.thread/1/progress.md`, `.thread/1/review/triage.md`, `.thread/1/steps.md`, `.thread/1/testing.md`
- ワークスペース設定（3件）: `Cargo.toml`, `flake.nix`, `rustfmt.toml`
- `pulsen-conformance`（18件）: `crates/pulsen-conformance/Cargo.toml`, `crates/pulsen-conformance/HOOKS.md`, `crates/pulsen-conformance/src/clock.rs`, `crates/pulsen-conformance/src/config_store.rs`, `crates/pulsen-conformance/src/doubles/clock.rs`, `crates/pulsen-conformance/src/doubles/lock.rs`, `crates/pulsen-conformance/src/doubles/mod.rs`, `crates/pulsen-conformance/src/doubles/stores.rs`, `crates/pulsen-conformance/src/doubles/task_id.rs`, `crates/pulsen-conformance/src/doubles/task_repository.rs`, `crates/pulsen-conformance/src/doubles/tests.rs`, `crates/pulsen-conformance/src/doubles/worktree.rs`, `crates/pulsen-conformance/src/exclusive_lock.rs`, `crates/pulsen-conformance/src/lib.rs`, `crates/pulsen-conformance/src/task_id_generator.rs`, `crates/pulsen-conformance/src/task_repository.rs`, `crates/pulsen-conformance/src/workflow_store.rs`, `crates/pulsen-conformance/src/worktree_manager.rs`
- `pulsen-domain`（30件）: `crates/pulsen-domain/Cargo.toml`, `crates/pulsen-domain/src/definition/agent.rs`, `crates/pulsen-domain/src/definition/assembler.rs`, `crates/pulsen-domain/src/definition/command.rs`, `crates/pulsen-domain/src/definition/config.rs`, `crates/pulsen-domain/src/definition/duration.rs`, `crates/pulsen-domain/src/definition/mod.rs`, `crates/pulsen-domain/src/definition/name.rs`, `crates/pulsen-domain/src/definition/port.rs`, `crates/pulsen-domain/src/definition/reference.rs`, `crates/pulsen-domain/src/definition/snapshot.rs`, `crates/pulsen-domain/src/definition/template.rs`, `crates/pulsen-domain/src/definition/validator.rs`, `crates/pulsen-domain/src/definition/workflow.rs`, `crates/pulsen-domain/src/execution/mod.rs`, `crates/pulsen-domain/src/execution/port.rs`, `crates/pulsen-domain/src/lib.rs`, `crates/pulsen-domain/src/task/attempt.rs`, `crates/pulsen-domain/src/task/branch.rs`, `crates/pulsen-domain/src/task/counters.rs`, `crates/pulsen-domain/src/task/degraded.rs`, `crates/pulsen-domain/src/task/failure.rs`, `crates/pulsen-domain/src/task/id.rs`, `crates/pulsen-domain/src/task/mod.rs`, `crates/pulsen-domain/src/task/path.rs`, `crates/pulsen-domain/src/task/port.rs`, `crates/pulsen-domain/src/task/process.rs`, `crates/pulsen-domain/src/task/state.rs`, `crates/pulsen-domain/src/task/task.rs`, `crates/pulsen-domain/src/task/time.rs`
- `pulsen` 本体（26件）: `crates/pulsen/Cargo.toml`, `crates/pulsen/examples/lock_holder.rs`, `crates/pulsen/src/adapter/clock.rs`, `crates/pulsen/src/adapter/config_store.rs`, `crates/pulsen/src/adapter/lock.rs`, `crates/pulsen/src/adapter/mod.rs`, `crates/pulsen/src/adapter/task_file.rs`, `crates/pulsen/src/adapter/task_id.rs`, `crates/pulsen/src/adapter/task_repository.rs`, `crates/pulsen/src/adapter/workflow_store.rs`, `crates/pulsen/src/adapter/worktree.rs`, `crates/pulsen/src/adapter/yaml.rs`, `crates/pulsen/src/application/home.rs`, `crates/pulsen/src/application/mod.rs`, `crates/pulsen/src/application/register_task.rs`, `crates/pulsen/src/cli/add.rs`, `crates/pulsen/src/cli/args.rs`, `crates/pulsen/src/cli/exit.rs`, `crates/pulsen/src/cli/mod.rs`, `crates/pulsen/src/cli/render.rs`, `crates/pulsen/src/cli/wire.rs`, `crates/pulsen/src/lib.rs`, `crates/pulsen/src/main.rs`, `crates/pulsen/src/util/atomic.rs`, `crates/pulsen/src/util/fsdir.rs`, `crates/pulsen/src/util/mod.rs`
- `pulsen` テスト（14件）: `crates/pulsen/tests/cli_add_boundary.rs`, `crates/pulsen/tests/cli_add_error.rs`, `crates/pulsen/tests/cli_add_normal.rs`, `crates/pulsen/tests/cli_usage.rs`, `crates/pulsen/tests/common/git.rs`, `crates/pulsen/tests/common/lock.rs`, `crates/pulsen/tests/common/mod.rs`, `crates/pulsen/tests/conformance_config_store.rs`, `crates/pulsen/tests/conformance_lock.rs`, `crates/pulsen/tests/conformance_task_repository.rs`, `crates/pulsen/tests/conformance_time_id.rs`, `crates/pulsen/tests/conformance_workflow_store.rs`, `crates/pulsen/tests/conformance_worktree.rs`, `crates/pulsen/tests/register_task.rs`

### スキップ（21件）

- `.thread/1/review/changed-files-001.txt`, `.thread/1/review/changed-files-002.txt`, `.thread/1/review/changed-files-003.txt`, `.thread/1/review/changed-files-004.txt` — 過去ラウンドのレビュー対象一覧。本ラウンドの対象は `changed-files-005.txt` で、その158件を上記で全量見ている
- `.thread/1/review/review-001.md`, `.thread/1/review/review-001-adapter.md`, `.thread/1/review/review-001-arch-spec.md`, `.thread/1/review/review-001-domain.md`, `.thread/1/review/review-001-test.md`, `.thread/1/review/review-001-usecase-cli.md`, `.thread/1/review/review-002-adapter.md`, `.thread/1/review/review-002-arch-spec.md`, `.thread/1/review/review-002-domain.md`, `.thread/1/review/review-002-test.md`, `.thread/1/review/review-002-usecase-cli.md`, `.thread/1/review/review-003.md`, `.thread/1/review/review-003-adapter-test.md`, `.thread/1/review/review-003-arch-spec.md`, `.thread/1/review/review-003-domain-usecase-cli.md`, `.thread/1/review/review-004-final.md` — 過去1〜4周目のレビュー本文。判定と反映内容は `triage.md` に統合済みで、そちらを正本として読んだ（本ラウンドは `wont-fix` 3件の継承と再指摘回数の確認にのみ使用）
- `Cargo.lock` — cargo の生成物。手で編集する対象ではなく、`cargo build` / `cargo test` / `cargo clippy` が成功することで依存解決の整合を確認した
