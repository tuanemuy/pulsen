# 指摘台帳 — Issue #1 / PR #8

統合後の指摘は **37件**（Blocker 1 / Warning 36）。5本のレビューの計40件（B 1 / W 39）のうち、同一問題を指す3組を1件に統合した。

| 統合ID | 統合元 |
|---|---|
| W-006 | usecase-cli W-002 + test W-002（PULSEN_HOME 単独が未検証） |
| W-009 | usecase-cli W-005 + test W-004（「全件まとめて」が1件しか検証していない） |
| W-021 | test W-001 + arch W-005（境界値の拒否ケースで config/定義の不変が未検証） |

判定は **fix 34 / wont-fix 3 / defer 0**。

## ラウンド1

| ID | Key（照合キー） | 判定 | 理由 | 再指摘回数 |
|----|----------------|------|------|-----------|
| B-001 | adapter/task_repository.rs:111 list_active/list_archived | fix | `list` は `fs::read` の `NotFound` を握らず `ReadError::Io` に写す。走査中に `archive`（rename）が入ると走査全体が失敗し、spec/domains/task.md:294-295「archive の中間状態は読み手から観測されない」「読み取りはロックなしで常に一貫した内容を返す」に反する。`lookup`（:61）が同じ状況を `Ok(None)` に写しているのと非対称。コード読解で確認、レビュアーの実測（400件並行で19回失敗）と整合 | 0 |
| W-001 | domain/definition/assembler.rs:196-202 重複ステータス名 | fix | `BTreeMap::insert` の戻り値を捨てて後勝ちで畳む。spec は `WorkflowParseError` に重複用の種別を持たず、`YamlSyntax`（重複キー含む）はアダプター生成と定めるため、ドメインで新エラーを作るのは spec 違反。**契約として明文化する方向で直す** | 0 |
| W-002 | domain/assembler.rs:423-455 と adapter/config_store.rs:211-229 説明文の二重定義 | fix | `NameError` / `DurationError` / `CommandError` の日本語説明5文が2層に同一文字列で存在することをコードで確認。片方だけ直すと config.yaml とワークフローYAMLで同じ誤りの案内が食い違う。ただし spec は `InvalidValue { location, message }` を型として宣言しているので、`cause: ValueError` への作り替えは行わない | 0 |
| W-003 | domain/definition/workflow.rs:138-180 effective_* の None 枝 | fix | 不在ステータスが `Wait`/`Cleanup` と同じ枝に落ちるのは事実。ただし **シグネチャ変更（`&StatusDefinition` を取る形）は wont-fix** — spec/domains/definition.md:165-168 が `(&self, status: &StatusName)` を明示しており AC-7 の1:1一致を壊す。**契約の明文化と、未規定の振る舞いを固定しているテストの削除だけを行う** | 0 |
| W-004 | domain/definition/template.rs:196-199 unreachable! | wont-fix | CLAUDE.md が「パニックは不変条件違反にのみ使う」と明示的に許容しており、why コメントも付いている。加えて spec/domains/definition.md:126 が `segments: Vec<Segment>`（`Hole` は `Skill` のみ）とフィールド型を宣言しているため、専用 Segment 型への作り替えは spec 改訂を伴う設計変更。本PRで場当たり的に行うべきでない | 0 |
| W-005 | cli/render.rs:15 アダプター型の import | fix | `cli/mod.rs:3-4` が「アダプターへの依存は `wire` の1箇所に閉じ」と自ら宣言しているのに `render.rs` が `adapter::task_id::IdGeneratorInitError` を import している。doc の主張と実体の食い違いを確認 | 0 |
| W-006 | cli/wire.rs:161 PULSEN_HOME 単独のホーム解決が未検証 | fix | `home_env` を使うのは TC-067 のみで、そこは `--home` も同時に立てるため `env::var_os(HOME_ENV)` の枝が一度も実行されない（`common/mod.rs:303` が既定で `env_remove`）。AC-13 の3段優先順位の中段が無検証 | 0 |
| W-007 | cli/wire.rs:161-163 空の PULSEN_HOME | fix | `!value.is_empty()` に why が無いことを確認。spec にも ADR にも無い挙動上の決定。判断としては妥当なので、規約を残す方向で直す | 0 |
| W-008 | cli/exit.rs:12-23 exit code 2 / --help 0 | fix | `tests/` に `Cli::try_parse` の失敗経路を通す実行が1つも無いことを grep で確認。`exit::USAGE` と `from_clap` の `use_stderr()` 分岐が完全に無検証 | 0 |
| W-009 | tests/register_task.rs:589 「全件まとめて」 | fix | 台本は `UnknownAgent` 1件しか生まない（:589-612 を確認）。1件では「最初の1件で打ち切る」実装と区別できず、テスト名が観測より強い。CLAUDE.md「テストは振る舞いを表す」に反する | 0 |
| W-010 | tests/register_task.rs:526-566 ダブルの台本 | fix | `DetachedHead` / `EmptyRepository` を `with_validate_repo` で流している。ポート契約（execution/port.rs）では HEAD 由来の分類を返すのは `head_branch`。実在しない組み合わせを模した台本で、`head_branch` / `branch_exists` の `Err` がユースケースを通るかは一度も検証されていない | 0 |
| W-011 | cli/wire.rs:65-80 Runtime::home() | fix | `grep '\.home()' crates/` で0件を確認。`pub` のため dead_code にも clippy にも掛からず静かに残る。保持理由の why も無い | 0 |
| W-012 | cli/render.rs:31 「次回の tick で実行されます。」 | wont-fix | spec/pages/index.md の add の完成形として正しい文言。本スライス限りの言い換えは後続スライスで戻す作業を生む。代わりに **意図的な選択であることを Issue コメントに残す**（G6 で対応） | 0 |
| W-013 | adapter/workflow_store.rs:90-97 Io/Parse にパスが乗らない | fix | `WorkflowLoadError::Io { message: error.to_string() }` を確認。`std::io::Error` の Display はパスを含まず、`resolved` はその場にあるのに捨てている。ADR-050「対象ファイルの絶対パスと論理位置で示す」と食い違う。同PR内で `task_repository.rs:218` は `message(&path, &error)` でパスを必ず付けており非対称 | 0 |
| W-014 | util/atomic.rs:39-47 rename_atomic の fsync | fix | 移動先の親しか `sync_dir` しない。`tasks/` → `archive/` は2つのディレクトリエントリを変えるため、クラッシュ時に「両方に在る」= spec:294 が禁じる中間状態が残りうる。`write_atomic` が temp→fsync→rename→fsync dir を守っているのと片手落ち | 0 |
| W-015 | adapter/worktree.rs:51-53 Path::exists() | fix | `exists()` は I/O エラーを `false` に丸めるため、親ディレクトリが読めないリポジトリが `NotFound` として案内される。`try_exists()` + `Err → TargetError::Failed` が正しい。分類は5種のまま増えない | 0 |
| W-016 | util/atomic.rs:26-29 write_atomic の mode | fix | `NamedTempFile::new_in` は Unix で mode 0600 固定、`persist` はそのまま移すため `save` のたびに既存ファイルの権限が 0600 に置き換わる。値としては安全側なので**意図として doc に明記する方向で直す**（挙動は変えない） | 0 |
| W-017 | adapter/task_repository.rs:127-142 create の TOCTOU | wont-fix | 指摘の前提が契約の誤読。spec/domains/task.md:295 は「書き込みメソッドの呼び出し側は排他ロックの取得を前提とする。**ポートは並行書き込みを調停しない**」と明示している。:293 の「呼び出し側の事前確認に依存しない」は「呼び出し側が事前に exists を見る必要がない」の意味で、並行writer に対する原子性の要求ではない。`persist_noclobber` の追加は過度に防御的 | 0 |
| W-018 | adapter/task_repository.rs:128-133 存在確認の失敗経路 | fix | `taken` だけ `error.to_string()` でパスが落ちる。同関数の :139-141 は `message(&path, &error)` を使っており非対称。`try_exists` が失敗する状況（親ディレクトリの権限等）はまさにパスが要る場面 | 0 |
| W-019 | adapter/worktree.rs:12 INHERITED_GIT_ENV | fix | **実測で再現**: `GIT_CEILING_DIRECTORIES=<repo> git -C <repo>/sub rev-parse --show-toplevel` が exit 128（未設定なら exit 0）。正当なリポジトリが `NotARepository` に誤分類される。cron からの無人実行で継承環境を利用者が意識しない運用が requirements の想定にある。ADR-024 の共通項も併せて直す | 0 |
| W-020 | adapter/yaml.rs:235-244 key_text | fix | `agents:` / `statuses:` 直下はキーが自由形式なので、`Yaml::Bool`/`Integer`/`Sequence`/`Mapping` が無言で名前になる（複合キーは全て `"(複合キー)"` に潰れて後勝ちで消える）。ADR-013「typo を無言で捨てない」・ADR-042 のタグ拒否と同じ根拠が非文字列キーにも当てはまる | 0 |
| W-021 | tests/cli_add_boundary.rs:57,118,182 拒否時の不変検証 | fix | `reject_base`（TC-054/055）・TC-053・TC-058 が `has_no_task()` しか見ていないことを確認。AC-15 は「異常系 TC-014〜048 **と境界値の拒否ケース TC-053・054・055・058**」に対して「タスクが作られず、**かつ**ワークフロー定義ファイル・config.yaml が変更されない」を要求。`cli_add_error.rs` は全31件で `assert_unchanged()` を通しており非対称。原因は steps.md ステップ20 が拒否側の要件を「タスクが作られないこと」としか書いていないこと（steps.md 側も直す） | 0 |
| W-022 | conformance/task_repository.rs:687,752 観測回数の下限 | fix | 読み取りスレッドは `while writing.load(...)` で回るだけで観測回数を数えていない。書き手が先に走り切ると読み手0周でも `CaseOutcome::Ran` を返す。spec の主張「すべての読み取りが完全な保存内容のみを観測する」は観測が起きて初めて意味を持つ | 0 |
| W-023 | tests/common/mod.rs:178-206 Untouched | fix | `Untouched::of` は控えたパスの内容比較のみ。拒否経路で `workflows/` やホーム直下に**新規ファイル**が作られても `assert_unchanged` は通る。PAGE-common-006 規則2 の否定的主張として `has_no_task()`（集合を見る）と粒度が揃っていない | 0 |
| W-024 | tests/common/mod.rs:388-406 deny_read | fix | 効き目の確認が `fs::read(path).is_ok()` なので、ディレクトリには root でも常に `Err`（EISDIR）となり `chmod 000` が無効でも `Some` を返す。現在の呼び出し元はファイルのみで実害はないが `pub` で置かれており、後続スライスがディレクトリに使うと ADR-027 が警告する「スキップに落ちずに FAIL」が再発する | 0 |
| W-025 | tests/cli_add_error.rs:197-207 TC-022 重複キー分岐 | fix | 構文エラー分岐は `["YAML 構文エラー", "位置:", "行"]` を見るのに重複キー分岐は `["YAML 構文エラー"]` だけ。位置が落ちても通る。適合テスト TC-port-workflow-store-017 が両方で `location.is_some()` を検証しており、位置は実際に取れる（強化可能であることを確認） | 0 |
| W-026 | conformance/lib.rs:205,238,243,255,261 YAML 生テキストのフック | fix | 主張は正しい（`put_config(&str)` / `put_named(name, text)` が YAML ソースを取る）。ただし **設計自体は不可避** — TC-port-config-store-014（YAML 構文エラー）/ TC-port-workflow-store-017（重複キー）は生テキストの口が無ければ表現できない。AC-8 の括弧書き「生 JSON 文字列を渡す API を持たない」は字義上満たしている。**直すのはコードではなくドキュメント**（HOOKS.md への明記 + ADR 起票）で、AC-8 の「フックを実装するだけで」の適用範囲を TaskRepository / Clock / TaskIdGenerator / ExclusiveLock / WorktreeManager に限定する | 0 |
| W-027 | .adr/ に ADR-038/043/045/046/049/051/052 が未起票 | fix | `ls .adr/` で確認: 019〜037・039・040・042・044・048・050 の25件のみ。ADR-041 / 047 は「既存 ADR に反映済み」と Status に明記があり問題なし。残り7件は説明なし。ADR-046（操作後の観測にスキップ可能フックを使わない）・ADR-049（`--base` の `-` 始まり）・ADR-052（受け入れテストの起動基盤）は**後続スライスを縛る規約**であり、正本に無い状態は放置しない | 0 |
| W-028 | .thread/1/adr.md の Status | fix | 実測: ADR-019/022/023/029/034/035 が `Proposed` のままだが `.adr/` 側の6ファイルはすべて `承認済み`。ADR-050 は `.adr/050-*.md` が存在するのに Status にファイル名がない。「Status 行にファイル名があれば起票済み」という索引が両方向に破れており、W-027 の未起票集合を Status から機械的に導けない | 0 |
| W-029 | conformance/HOOKS.md:200 が .thread/1/adr.md を指す | fix | 出荷物（クレート同梱ドキュメント）からスライス作業用ディレクトリを参照している唯一の箇所。ADR-041 の内容は `.adr/027-*.md:33-35` に反映済みであることを確認。ADR-035 が防ごうとした状態そのもの | 0 |
| W-030 | conformance/lib.rs:150-159 スキップの報告 | fix | `CaseOutcome::report` が `println!` で報告するため libtest に握り潰され、緑と区別できない。Windows では権限系フックが `#[cfg(not(unix))]` で一律 `None` を返し8件が黙って素通りする。「125件 PASS」の意味が環境で変わるのに差が出力に現れない。**なおレビュアーの「最小の手当てとして eprintln!」は誤り** — libtest は stderr も捕捉するので効かない。スキップ件数の宣言と超過時の失敗で直す | 0 |
| W-031 | .thread/1/progress.md:12 と Issue コメント | fix | progress.md:12 の「全件が実行された」は同節 :20 の「TC-port-clock-005 は常にスキップ」と矛盾（実測でもスキップされる）。加えて `gh issue view 1 --comments` を実行し、既存コメントが TC-port-task-repository-022/028 の言い換え提案1件のみであることを確認 — plan.md と Issue 完了条件が求める「スキップ行の理由」「spec 追従の提起（ADR-050 / ADR-051 由来の2件）」が未投稿 | 0 |
| W-032 | domain/definition/name.rs:101 InputText::new | fix | spec/domains/definition.md:38「いずれも `parse(s: String) -> Result<Self, NameError>` でのみ生成する」に対する逸脱で、`InputText` は同表の「制約なし」行。`new` にした判断自体は妥当（コードに why あり）だが、同程度の逸脱である ADR-039/040/048/050 が ADR として残っているのに対し記録が非対称。**spec 追従の提起として記録する**（コードは変えない） | 0 |
| W-033 | cli/args.rs:22-25 PAGE-common-011 の回帰テスト | fix | plan.md が「PAGE-common-007 / 011 は『提供しないことを確認する行』として消化対象」と明記しているのに、現状この行は**不在によってのみ**満たされ、後続スライスが `pulsen init` を足しても何も落ちない。同じ否定的主張の TC-052 は steps.md が明示的に網を張っており扱いが揃っていない | 0 |
| W-034 | tests/register_task.rs のテスト名 | fix | CLI 62件は `tc_task_register_task_NNN_<仕様の言葉>` で台帳行と1:1なのに、ユースケース層で消化する5行（TC-012/018/040/047/048）だけ TC ID を持たない。Issue 完了条件「実装をレビューで確認できた行にのみチェックを付ける」に対し行単位の対応が機械的に取れない。同一プロジェクト内で命名規約が2つに割れている | 0 |
| W-035 | adapter/worktree.rs:104 {error:?} | fix | `format!("HEAD のブランチ名を扱えない: {error:?}")` を確認。`TargetError::Failed` に載り `render.rs:243-245` が「原因: {message}」でそのまま出すため、`ContainsWhitespaceOrControl { char: ' ', position: 3 }` のような Rust の構造体表記が利用者に出る。spec/pages/index.md:15「出力は人間可読なテキストとする」への取りこぼし。`render.rs:346-359` に同じ型の日本語化がすでにあるのと対照的 | 0 |
| W-036 | conformance/HOOKS.md:132,143 対応表 | fix | (a) TC-port-task-repository-043 の spec 行（task-repository.md:88）は前提を「`save` が `Err` を返した（**NotFound / Io**）」とするが実装は NotFound 分岐のみ。原子性の観測面という行の趣旨は、書き込みが始まってから失敗する Io 分岐でこそ意味を持つ。`make_unwritable(Active)` フックが存在することを確認済み。(b) TC-port-clock-003 の spec 行（clock.md:11）は「…テスト中に時刻改変が起きないアダプター環境に限る」で、HOOKS.md:12 が定める C の定義（spec が「アダプター環境に限る」と明示する行）に該当するのに表では B。集計 A 28 / B 86 / C 11 が自前の基準どおりでない | 0 |

## 実行計画（ラウンド1）

グループ間でファイルは重複しない。唯一の順序依存は G5 → G1（後述）。

### グループ: `adapter-persistence`

- 担当: B-001, W-013, W-014, W-016, W-018, W-020
- レビューファイル: `.thread/1/review/review-001-adapter.md`
- 方針:
  - B-001: `list` の `fs::read` で `ErrorKind::NotFound` を `continue` でスキップする（`lookup` が `Ok(None)` に写しているのと同じ扱い）。「走査中にアーカイブされたエントリはこの領域にもう無いだけで失敗ではない」を why として残す。
  - W-013: `read_error` の `Io` 分岐と `YamlSyntax` / `Parse` への写像で `resolved` の絶対パスをメッセージに載せる（ADR-050 に合わせる）。
  - W-014: `rename_atomic` で `from.parent()` と `to.parent()` が異なるときは両方 `sync_dir` する。
  - W-016: `write_atomic` が対象ファイルの権限を 0600 に作り直すことを doc コメントに「タスクファイルは所有者限定」という意図として明記する（挙動は変えない）。
  - W-018: `create` の `taken` をパス付きの `message(path, &error)` に変え、現役・アーカイブそれぞれのパスを載せる。
  - W-020: `convert` のマッピング走査で、キーが `Yaml::Text` 以外なら `YamlSyntaxError` にする（タグ拒否と同じ場所）。`key_text` の非文字列分岐を落とし、ユニットテストを足す。
- 触るファイル: `crates/pulsen/src/adapter/task_repository.rs`, `crates/pulsen/src/adapter/workflow_store.rs`, `crates/pulsen/src/adapter/yaml.rs`, `crates/pulsen/src/util/atomic.rs`

### グループ: `adapter-git-and-cli-wiring`

- 担当: W-005, W-007, W-011, W-015, W-019, W-035
- レビューファイル: `.thread/1/review/review-001-adapter.md`, `.thread/1/review/review-001-usecase-cli.md`, `.thread/1/review/review-001-arch-spec.md`
- 方針:
  - W-015: `repo.as_path().try_exists()` にし、`Ok(false)` は `NotFound`、`Err(e)` は `TargetError::Failed { message }`（分類は5種のまま）。
  - W-019: `INHERITED_GIT_ENV` に `GIT_CEILING_DIRECTORIES` / `GIT_COMMON_DIR` / `GIT_OBJECT_DIRECTORY` / `GIT_ALTERNATE_OBJECT_DIRECTORIES` を足し、**先に `.adr/024` の共通項を更新してから**実装を合わせる（実装だけ直すと ADR と食い違う）。除去の目的「呼び出し元の環境で `-C` の対象が上書きされるのを防ぐ」を判断基準として ADR に書く。
  - W-035: `branch_name(...)` の `Failed` メッセージから `{error:?}` を外し、扱えなかったブランチ名の文字列だけを載せる（分類は `Failed` で十分。文言の細部は CLI 側が持つ）。
  - W-005: `WireError::IdGenerator(IdGeneratorInitError)` を `IdGenerator { message: String }` に変え、`render::id_generator_error` の中身を `wire::compose` 側のヘルパへ移す。`render.rs` の `use crate::adapter::...` を消して `cli/mod.rs` の doc の主張と一致させる。
  - W-007: `!value.is_empty()` に why を1行残し（「空文字は未設定と同義。空パスはホームとして解決不能なため」）、境界値ケースを G5 のテストで固定する。
  - W-011: `Runtime` の `home` フィールドと `pub fn home` を落とす（`compose` 内で `PulsenHome` を消費する順序だけ整える）。後続で必要になった時点で why 付きで戻す。
- 触るファイル: `crates/pulsen/src/adapter/worktree.rs`, `crates/pulsen/src/cli/render.rs`, `crates/pulsen/src/cli/wire.rs`, `.adr/024-git-cli-shell-out-and-target-classification.md`

### グループ: `domain-contracts`

- 担当: W-001, W-002, W-003
- レビューファイル: `.thread/1/review/review-001-domain.md`
- 方針:
  - W-001: `WorkflowParseError` に重複用の種別は足さない（spec が `YamlSyntax`（重複キー含む）をアダプター生成と定めているため）。`WorkflowAssembler::assemble` のドキュメンテーションコメントに「`statuses` の重複はアダプターが `YamlSyntax` として弾く前提」を**前提条件として明記**し、`BTreeMap::insert` の後勝ちがその前提の帰結であることを why で残す。
  - W-002: `NameError` / `DurationError` / `CommandError` に `describe()` をドメイン側で1つ持たせ、`assembler.rs` の `invalid_*` と `adapter/config_store.rs` の `describe_*` の両方がそれを呼ぶ形にして文言の定義箇所を1つにする。`InvalidValue { location, message }` の型は spec どおり変えない。
  - W-003: `effective_*` のシグネチャは spec:165-168 のとおり `&StatusName` のまま保つ。`None` 枝を `Wait` / `Cleanup` と分けたうえで、不変条件1により実運用では到達しないことと `Wait` に対する呼び出しが spec 上未規定であることを doc に書く。`snapshot.rs:120-123` の `Wait` に対する `effective_retry_limit == 2` の期待はテストから外す（spec が「呼び出しを規定しない」と明記する振る舞いを固定しているため）。
- 触るファイル: `crates/pulsen-domain/src/definition/assembler.rs`, `crates/pulsen-domain/src/definition/workflow.rs`, `crates/pulsen-domain/src/definition/name.rs`, `crates/pulsen-domain/src/definition/duration.rs`, `crates/pulsen-domain/src/definition/command.rs`, `crates/pulsen-domain/src/definition/snapshot.rs`, `crates/pulsen/src/adapter/config_store.rs`

### グループ: `conformance-suite`

- 担当: W-022, W-026, W-029, W-030, W-036
- レビューファイル: `.thread/1/review/review-001-test.md`, `.thread/1/review/review-001-arch-spec.md`
- 方針:
  - W-022: TC-042 / TC-044 の読み取りスレッドに `AtomicUsize` の観測カウンタを置き、スコープを抜けた後に `assert!(observations > 0, ...)`（可能なら `save` / `archive` 回数に見合う下限）を確認する。
  - W-030: `conformance_cases!` にスキップ集計を持たせ、アダプター側のテストファイルで「このアダプターで許容するスキップは N 件」を宣言して超過したら失敗させる。`eprintln!` への変更は行わない（libtest は stderr も捕捉するため効果がない）。
  - W-036(a): TC-port-task-repository-043 に `make_unwritable(Active)` を使う Io 分岐を足す。NotFound 分岐は無条件のまま残し、Io 分岐だけをフック不在時に飛ばす形にして ADR-046（操作後の観測にスキップ可能フックを使わない）に抵触させない。
  - W-036(b): HOOKS.md の TC-port-clock-003 を C に直し、集計を A 28 / B 85 / C 12 に更新する。
  - W-026: HOOKS.md に「ConfigStore / WorkflowStore の入力系フックは YAML ソースを受け取る。この2ポートのスイートは YAML 表現に結合している」を明記し、`lib.rs` の冒頭 doc（:13-14）にも同じ限定を書く。判断として `.adr/053-*.md` を新規起票し、AC-8 の「フックを実装するだけで同じスイートを通せる」の適用範囲が TaskRepository / Clock / TaskIdGenerator / ExclusiveLock / WorktreeManager であることを残す。**コードの構造は変えない**。
  - W-029: HOOKS.md:200 の参照先を `.adr/027-port-conformance-suite-and-harness-hooks.md` に差し替える。
- 触るファイル: `crates/pulsen-conformance/HOOKS.md`, `crates/pulsen-conformance/src/lib.rs`, `crates/pulsen-conformance/src/task_repository.rs`, `crates/pulsen/tests/conformance_config_store.rs`, `crates/pulsen/tests/conformance_lock.rs`, `crates/pulsen/tests/conformance_task_repository.rs`, `crates/pulsen/tests/conformance_time_id.rs`, `crates/pulsen/tests/conformance_workflow_store.rs`, `crates/pulsen/tests/conformance_worktree.rs`, `.adr/053-conformance-yaml-source-hooks.md`（新規）

### グループ: `test-gaps`

- 担当: W-006, W-008, W-009, W-010, W-021, W-023, W-024, W-025, W-033, W-034
- レビューファイル: `.thread/1/review/review-001-test.md`, `.thread/1/review/review-001-usecase-cli.md`, `.thread/1/review/review-001-arch-spec.md`
- 方針:
  - W-021: `reject_base`（TC-054/055）・TC-053・TC-058 に `let untouched = home.untouched();` … `untouched.assert_unchanged();` を足す（`cli_add_error.rs` の `reject_target` と同じ形）。
  - W-023: `Untouched::of` が控えるものに `resources()` が返す**パス集合そのもの**（`workflows/` のエントリ一覧・ホーム直下のエントリ一覧）を加え、`assert_unchanged` で新規ファイルの出現も検出する。
  - W-024: `deny_read` を「ファイル専用」と doc に明記し、`path.is_file()` を前提として確認したうえで、効き目の確認をディレクトリで誤判定しない形にする（`conformance_task_repository.rs` のディレクトリ版と対で扱う）。
  - W-006: `--home` を渡さず `home_env` だけを与え、環境変数側のホームに `state/tasks/` が作られフラグ側のホームは空のままであることを見る CLI ケースを1件足す。W-007 の空文字 `PULSEN_HOME` の境界値ケースも同じ場所に足す（既定の `~/.pulsen/` には触れない）。
  - W-008 / W-033: 新規 `tests/cli_usage.rs` に「必須引数の欠落は 2」「未知フラグ（`--json`）は 2」「`--help` は 0 かつ標準出力」「サブコマンドの集合がちょうど `add` だけ」を確かめるテストを足す。PAGE-common-007（機械可読出力なし）と PAGE-common-011（作成コマンドなし）の観測可能な裏付けを兼ねる。
  - W-009: `登録時検証のエラーは全件まとめて返り登録は行われない` の台本を複数エラー版（`MissingModel` + `MissingSkillInput` 等）に差し替え、`Err(Registration(vec![...]))` の要素数と内訳を assert する。
  - W-010: `DetachedHead` / `EmptyRepository` を `with_head_branch([Err(...)])` に移し、`Failed` は `validate_repo` / `head_branch` / `branch_exists` の3経路それぞれに割り当てる。`WorktreeManagerCall` の記録で呼び出し列も assert する。
  - W-025: TC-022 の重複キー分岐の期待に `"位置:"` を加える（適合テスト TC-port-workflow-store-017 が位置を取れることを確認済み）。
  - W-013 の裏取り: TC-021 の期待に解決先パスを加える。**`adapter-persistence` の `workflow_store.rs` 変更が入ってから**行う（このグループで唯一の順序依存）。
  - W-034: `register_task.rs` の5件（TC-012/018/040/047/048）を CLI テストと同じ `tc_task_register_task_NNN_<仕様の言葉>` の接頭辞に揃える。他の15件は台帳行を持たないため現行の記述的な名前のまま残す。
- 触るファイル: `crates/pulsen/tests/cli_add_boundary.rs`, `crates/pulsen/tests/cli_add_error.rs`, `crates/pulsen/tests/common/mod.rs`, `crates/pulsen/tests/register_task.rs`, `crates/pulsen/tests/cli_usage.rs`（新規）

### グループ: `docs-adr-issue`

- 担当: W-027, W-028, W-031, W-032（+ W-012 / W-021 の記録面）
- レビューファイル: `.thread/1/review/review-001-arch-spec.md`, `.thread/1/review/review-001-test.md`
- 方針:
  - W-027: ADR-038 / 043 / 045 / 046 / 049 / 051 / 052 の7件を `.adr/` に起票する（ADR-035 の連番規則どおり、Status は `承認済み`）。特に ADR-046（適合ケースは操作後の観測にスキップ可能フックを使わない）・ADR-049（`--base` は `-` 始まりも値として受け取る）・ADR-052（受け入れテストの起動基盤を `tests/common` に集約）は後続スライスを縛る規約なので正本に置く。
  - W-028: `.thread/1/adr.md` の ADR-019/022/023/029/034/035 の Status を `Accepted（ステップNで確定。<.adr パス>）` に更新し、ADR-050 の Status にファイル名を足す。W-027 で起票した7件の Status にもファイル名を書き、「Status 行 = 起票済みの索引」を機械的に成立させる。
  - W-031: `progress.md:12` を「TC-port-clock-005 の1件を除き全件が実行された」に直す。
  - W-021（記録面）: `steps.md` ステップ20 の拒否側の要件を AC-15 に揃える（「タスクが作られないこと」に加えて「ワークフロー定義ファイルと config.yaml が変更されないこと」）。
  - W-032: `progress.md` の「spec へ追従を提起する点」に `InputText` の行を足す（spec/domains/definition.md:38 の「いずれも `parse` でのみ生成する」を「制約のある型は」へ言い換える提案）。
  - Issue #1 へのコメント投稿（plan.md と Issue 完了条件が求める運用。現在は TC-port-task-repository-022/028 の1件のみ）:
    1. **スキップ行の理由**: TC-port-clock-005（時刻を過去に設定できないためこの環境では常にスキップ。チェックは付けない）。
    2. **spec 追従の提起（ADR-050 由来）**: エラー位置の粒度を「構文エラーは行・列、スキーマ違反はキーのパス」へ言い換える提案。
    3. **spec 追従の提起（ADR-051 由来）**: 表示名を決められないファイル名の例示に「語幹が空白のみ」を含める提案。
    4. **spec 追従の提起（W-032 由来）**: `InputText` を `parse` 規約の対象外とする言い換えの提案。
    5. **W-012 の記録**: 成功時の「次回の tick で実行されます。」は spec の完成形の文言として意図的に残す（本スライスに `tick` は無い）。
  - `progress.md` の残存課題に「Issue コメント投稿済み（スキップ理由1件・spec 追従3件・文言の意図1件）」を反映する。
- 触るファイル: `.adr/038-*.md`, `.adr/043-*.md`, `.adr/045-*.md`, `.adr/046-*.md`, `.adr/049-*.md`, `.adr/051-*.md`, `.adr/052-*.md`（いずれも新規）, `.thread/1/adr.md`, `.thread/1/progress.md`, `.thread/1/steps.md`, GitHub Issue #1（コメント）
