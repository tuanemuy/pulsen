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

## ラウンド2

5本のレビューの計42件（B 2 / W 40）のうち、同一問題を指す9組を統合して **31件**（B 2 / W 29）にした。判定は **fix 31 / wont-fix 0 / defer 0**。1周目の wont-fix 3件（W-004 / W-012 / W-017）はいずれのレビューも明示的に蒸し返しておらず、**wont-fix の継承は0件**。Key が一致する既出指摘は9件で、いずれも1周目の判定が `fix` なので判定を継承して再指摘回数を +1 した（再審議はしていない）。

| 統合ID | 統合元 |
|---|---|
| R2-B-001 | test B-001 + adapter W-002 + test W-009（`SkipBudget` が root で失敗になる／許容集合を宣言しない） |
| R2-W-001 | test W-001 + arch W-004（CLI 受け入れテストの無言スキップ） |
| R2-W-002 | adapter W-006 + test W-002（TC-042/044 の観測下限が書き手との重なりを保証しない） |
| R2-W-003 | adapter W-007 + arch W-006（`task_file.rs` の `{error:?}` 25箇所） |
| R2-W-004 | adapter W-008 + arch W-003 + usecase-cli W-001（ADR-050 と ADR-054 の矛盾） |
| R2-W-005 | test W-006 + domain W-003（`effective_*` の契約の閉じ方と `Wait` への期待） |
| R2-W-006 | domain W-002 + test W-005（`ForbiddenKey` の平行配列と未検証の3キー） |
| R2-W-007 | usecase-cli W-002 + test W-013（受け入れテストが実ユーザーホームを守らない） |
| R2-W-008 | test W-007 + arch W-007（AC-1 の grep 手順が偽陽性） |

### 判定

| ID | Key（照合キー） | 判定 | 理由 | 再指摘回数 |
|----|----------------|------|------|-----------|
| R2-B-001 | conformance/lib.rs SkipBudget + tests/conformance_*.rs ALLOWED_SKIPS | fix | 実コードで確認: `deny_read`（tests/common/mod.rs:461-491）・`deny_dir_read` / `deny_dir_write`（conformance_task_repository.rs:161-180）は ADR-027 どおり「制限が効いたことを確認してから `Some`」なので、root では必ず `None` を返す。一方 unix の宣言は `ALLOWED_SKIPS = 0`（task_repository:214 / config_store:68 / workflow_store:91）で、`SkipBudget::record` が `assert!(used <= allowed)` で落とすため、TC-port-config-store-023 / workflow-store-030 / task-repository-005・011・012・019・035・041 の**8件が失敗**になる。非 unix 側の宣言 1/1/6 = 8 と一致するので件数も裏付けられる。plan.md「権限操作でしか再現できない…root では skip する」「スキップで終わった行はチェックせず理由を Issue コメントに残す」と ADR-027 に正面から反する、1周目 W-030 の修正が生んだ回帰。`SkipBudget::new` が `const fn` のため利用者はソースを書き換えない限り回避できない | 1（1周目 W-030） |
| R2-B-002 | .adr/024 validate_repo の判定手順と「影響」 | fix | `.adr/024` の `validate_repo` 手順1が「パスが存在しない → NotFound」の2値のみで、1周目 W-015 で入れた `try_exists()` の `Err → Failed`（worktree.rs:63-71）に対応する分岐が無い。「影響」の「`Failed` の到達経路は『git を起動できない』1本に定まる」も成り立たない（起動失敗・`try_exists` の `Err`・stdout 非 UTF-8・`BranchName::parse` 失敗の4本）。ADR は後続スライスへの規範なので、実装に合わせて ADR を更新する | 0 |
| R2-W-001 | tests/cli_add_error.rs の println! スキップ4件 | fix | :108 / :130 / :182 / :411 に `println!("スキップ: …")` + `return` を確認。libtest が標準出力を握り潰すため緑と区別できない。同一 PR 内で `conformance_worktree.rs` は同じ前提（TMPDIR がリポジトリ配下）を失敗として見せるのに `cli_add_error.rs:411` は黙って通る。TC-016/017/021/036 は Issue のチェックリスト行なので、走らなかった行にチェックが付く | 0 |
| R2-W-002 | conformance/task_repository.rs 観測回数の下限 | fix | `yield_until_observed`（:788-798）が TC-042 の書き込みループ（:731-734）と TC-044 の `archive`（:838）の**後**に置かれていることを確認。成立するのは「一度は観測した」だけで「書き込みと重なって観測した」ではない。1周目 W-022 の方針「可能なら `save` / `archive` 回数に見合う下限」に届いていない | 1（1周目 W-022） |
| R2-W-003 | adapter/task_file.rs の {error:?} | fix | `grep -c 'error:?'` で25箇所を確認。これらは `TaskLookup::Corrupt { message }` / `TaskEntry::Corrupt` / `TaskRecord::SnapshotUnreadable` の理由になり、spec/pages/index.md:118 が定める `ls` / `show` の**修復の入口**として利用者に見せる値。1周目 W-035 が `worktree.rs` の1箇所で同じ理由により fix 判定された。`render.rs:337-351` に同じ型の日本語化が既にある | 0 |
| R2-W-004 | .adr/050 と .adr/054 の矛盾 | fix | `.adr/050` の決定「スキーマ違反: 対象ファイルの絶対パスと論理位置」が無条件のまま残り、`.adr/054` が範囲を狭めた事実がどこにも書かれていない。**実装を ADR-050 に寄せる案（arch W-003 の案 a）は採らない** — `load` の失敗時にユースケースが解決先パスを持たないことを確認した（`register_task.rs:140-142` は `Ok` のときだけ `LoadedWorkflow::into_parts()` から `resolved_from` を得る）ため、パスを届けるにはポート表の変更が要り AC-7 を壊す。ADR-054 を正として ADR-050 に例外を明記し、Issue コメント3件目と `progress.md:9` を追補する（案 b） | 0 |
| R2-W-005 | definition/workflow.rs effective_* の None 枝と snapshot.rs の Wait 期待 | fix | (a) `workflow.rs:137-139` ほかの doc が definition ドメインの契約を task ドメインの不変条件1で説明している。spec/domains/definition.md:91「Definition は他ドメインに依存しない（ADR-017）」に対する文書上の逆向き依存で、実在する呼び出し2経路のうち `RegistrationValidator`（`definition.statuses()` のキーを渡す）を説明できない。(b) `snapshot.rs:114-118` に `effective_timeout(&status("waiting")) == DEFAULT_TIMEOUT` と `effective_agent` / `effective_model` の `Wait` に対する期待が残っている（1周目 W-003 は `effective_retry_limit` の行しか外していない）。同じ契約の両面なので1件で扱う | 1（1周目 W-003） |
| R2-W-006 | definition/assembler.rs ForbiddenKey の平行配列 | fix | `AGENT_RUN_KEYS`（:226）と `present` 配列（:332-339）を `zip`（:340）していることを確認。キー名と述語の対応が型に現れず、片方を並べ替えると「`model` を書いたのに `timeout` が禁止キーとして案内される」が無警告で通る。テスト（:574-610）は `judge` / `next` / `agent` の3つしか見ない。CLAUDE.md「不正な状態を型で表現不能にする」を追加コストほぼ0で満たせる箇所 | 0 |
| R2-W-007 | tests/common/mod.rs Add::run のホーム環境 | fix | `Add::run`（:355-390）は `env_remove(HOME_ENV)` で `PULSEN_HOME` を必ず断つのに、`HOME` / `USERPROFILE` は `user_home()` を明示したテストでしか差し替わらない。既定へ落ちる経路を踏むテストが1周目の修正で入った（`cli_add_boundary.rs:410-430`）ため、書き忘れが実 `~/.pulsen/` に漏れる余地が今後増える。`.thread/1/testing.md:176-181` の手動手順3も実 `$HOME/.pulsen` を対象にしており同じ危うさがある | 0 |
| R2-W-008 | .thread/1/testing.md:44-48 と plan.md AC-1 の grep 手順 | fix | 実行すると `crates/pulsen/tests/` 配下が13件ヒットし、`crates/pulsen/src/adapter/` は逆に0件。手順どおりなら AC-1 が不合格に見える。テスト側の `#[cfg(unix)]` は権限操作フックの正当な分岐なので、直すのは確認手順と AC-1 の文言のほう | 0 |
| R2-W-009 | definition/name.rs describe() と cli/render.rs name_error | fix | `name.rs:12-22` の `describe()` は doc に「説明の定義箇所をドメインに1つ置く。層ごとに書くと同じ誤りの案内が食い違う」と明記しているのに、`render.rs:330-335` の `name_error` が「空文字列です」/「前後に空白を含みます」という別の文言を持ち続けている。`adapter/config_store.rs` 側では解消済みで扱いが非対称。**doc が主張する一元化が成立していない状態**が一番まずい | 1（1周目 W-002） |
| R2-W-010 | definition/validator.rs UnknownAgent の重複 | fix | `validate_agent_run`（:75-105）がステータスごとに `UnknownAgent` / `InvalidAgentDefinition` を push することを確認。この2種は spec（definition.md:274-280）で `status` を持たないエージェント単位の誤りなので、3ステータスが同じ未定義エージェントを参照すると値まで同一のエラーが3件返り、`render.rs` が「(3件)」として同じ2行を3回出す。`status` を持つ他3種と扱いが非対称 | 0 |
| R2-W-011 | application/home.rs worktree_root() | fix | `grep -rn worktree_root crates/pulsen*/src crates/pulsen/tests` で、`pulsen-domain` を除く参照が `home.rs` 内の定義・構築・自分のユニットテスト（:116）だけであることを確認。1周目 W-011 が `Runtime::home()` を削除して決着した基準（未使用の `pub` は why が無ければ落とす）との割れを解消する | 0 |
| R2-W-012 | adapter/task_repository.rs list の NotFound スキップ | fix | :111-121 の `continue` が `fs::read` の `NotFound` だけを判定材料にしていることを確認。宙ぶらりんのシンボリックリンク（`read_dir` は列挙し `fs::read` は `NotFound`）が走査結果から黙って消え、spec/pages/index.md:43 が定める `TaskEntry::Corrupt` による修復の入口を失う。`symlink_metadata` で「消えた」と「読めないエントリが残っている」を区別できる。加えて `adapter/task_repository.rs` に `cfg(test)` が0件で、この分岐を通す観測がどこにも無い（`continue` を消しても全テストが緑） | 1（1周目 B-001） |
| R2-W-013 | adapter/yaml.rs key_text の非文字列キー | fix | :239-251 が `location: None` の「キーは文字列である必要があります(実際は数値)」しか返さないことを確認。1周目 W-020 の修正前は固定スキーマ位置（`agents.claude` の中身等）なら文字列化 → スキーマ走査 → `UnknownKey { location, key }` でキー名と論理位置が出ていたので、その経路は診断性が退化している。ADR-050 の狙い「壊れている箇所をキー単位で特定できる」と合わない | 1（1周目 W-020） |
| R2-W-014 | tests/common/git.rs の env_remove とアダプターの INHERITED_GIT_ENV | fix | フィクスチャ（git.rs:31-33）は `GIT_DIR` / `GIT_WORK_TREE` / `GIT_INDEX_FILE` の3つ、本番（worktree.rs:15-22）は7つであることを確認。`is_outside_repository` はフィクスチャ側の git 環境で前提を判定するのに、その前提を使うアダプターは ceiling を外して探索するため、「TMPDIR がリポジトリ配下 かつ `GIT_CEILING_DIRECTORIES` が設定済み」の環境で TC-port-worktree-manager-003 がスキップではなく失敗になる | 0 |
| R2-W-015 | パスをメッセージに載せる規約の残り | fix | (a) `worktree.rs:66-70` は `repo.as_path()` を手元に持ちながら `リポジトリのパスを確認できない: {error}` にパスを載せない。この枝が出るのはまさに「どのパスか」が要る状況。(b) `task_repository.rs:229-231` は `rename_atomic(&from, &to)` の失敗をすべて `to` に帰属させるため、移動元起因の失敗でも無関係な移動先パスを指す。1周目 W-013 / W-018 が確立した規約の適用漏れ | 0 |
| R2-W-016 | spec/inventory の PAGE-common 系6行の扱い | fix | PASS 条件を実際に読んで確認した（frontend.md:8,9,11,12,72 の PAGE-common-002/003/005/006/010、usecase.md:27 の UC-flow-007）。いずれも全コマンド前提で `add` の列しか満たせない。**さらに `gh issue view` で6行が Issue #1 にしか現れないことを確認した**ため、「後続スライスまで見送る（案 b）」を採るとこの6行はどの Issue でもチェックされない孤児になる。一方 `PAGE-tick-006` は Issue #2 に配分済みで、コマンド別の縮退規則は各スライスの行が受け持つ構造になっている。したがって**案 a（本スライスの範囲で満たされたものとしてチェックする）を採り、基準を記録する** | 0 |
| R2-W-017 | .adr/ に ADR-055 が未起票 | fix | `ls .adr/` は 054 までで、`.thread/1/adr.md:1039` の ADR-055 だけ Status にファイル名が無いことを確認。1周目 W-028 が成立させた「Status 行 = 起票済みの索引」が1件で破れている。出荷物 `HOOKS.md:22` が `SkipBudget` を規約として書きながら ADR を参照しておらず、ADR-035 が防ごうとした「後続スライスの担当が根拠を辿れない」状態。あわせて `adr.md` は ADR-054（:999）が ADR-053（:1019）より前で連番順でない | 1（1周目 W-027 / W-028） |
| R2-W-018 | .thread/1/steps.md:57 の InputText | fix | steps.md:57 が「名前系 newtype 7種（… `InputText`）は `parse(String) -> Result<Self, NameError>` のみで生成する」と書いたまま、実装（`name.rs` の `InputText::new`）と食い違うことを確認。1周目 W-032 の対応は `progress.md` と Issue コメントに届いたが、実装手順の正本である steps.md に届いていない | 0 |
| R2-W-019 | Issue #1 のチェックリスト346行 | fix | `gh issue view 1` で `- [ ]` が346件・`- [x]` が0件を確認。完了条件「実装をレビューで確認できた行にのみチェックを付ける。見送る行はチェックせず理由をコメントに残す」に対し、見送り側（TC-port-clock-005）だけ運用が回っていて確認できた側が記帳されていない。AC-20 の判定を Issue 側から追えない | 1（1周目 W-031） |
| R2-W-020 | Cargo.toml の rust-version と .adr/023 の版数 | fix | `Cargo.toml:8` の `rust-version = "1.89"` に対し devShell の rustc は 1.97.1 の1つだけで、CI も無いため 1.89 でのビルドは一度も行われていない。`.adr/023:24`「`std::env::home_dir()` は Rust 1.97 で非推奨が解除済み」は「1.97 時点で解除済み」とも「1.97 で解除された」とも読め、後者なら 1.89 で deprecated 警告が出て AC-1 の `clippy -D warnings` が通らない。**版数の確定は文書側で完結する**（CI 追加はスコープ外なので、検証手段が無いこと自体は残作業として記録する） | 0 |
| R2-W-021 | conformance/HOOKS.md:208-212 の「変えた点」 | fix | `grep -rn break_lock_location` で、この識別子が `crates/` と `.adr/` に1件も無く `.thread/1/{adr.md,steps.md}` の作業ログにしか残らないこと、`.adr/027:25-28` のフック表がすでに変更後の名前になっていることを確認。読み手が指示どおり `.adr/027` を開いても「変えた点」の3項目を裏取りできない。1周目 W-029 で参照先だけ差し替えた結果、注記が宙に浮いた。加えてこれは出荷物に残った修正の経緯である | 1（1周目 W-029） |
| R2-W-022 | .adr/030 の base_dir と std::path::absolute | fix | `workflow_store.rs:87` の `std::path::absolute` は引数が相対なら内部で cwd を読む。`absolutize` は先に `base_dir.join(path)` するので `base_dir` が絶対なら cwd は読まれないが、`FsWorkflowStore::new` は絶対性を型でも実行時検査でも要求せず、ADR-030 の決定にもその前提が書かれていない。ADR の不変条件「cwd を読むのは合成ルートの1箇所だけ」が明文化されていない前提の上でしか成立していない | 0 |
| R2-W-023 | conformance/workflow_store.rs が AgentInput::Prompt を検証しない | fix | `grep -rn 'AgentInput::Prompt' crates/pulsen-conformance/src/` のヒットは `task_repository.rs:1098` と `doubles/tests.rs:152` だけで、workflow-store スイート31件のどこにも `prompt:` の本文の等値比較が無いことを確認。`prompt` の本文をステータス名で埋める・`judge` と取り違えるといったアダプターのバグが31件すべてを通過する。実際に守っているのは CLI 受け入れテスト1箇所だけで、ポートの契約としては無防備 | 0 |
| R2-W-024 | cli/render.rs に単体テストが無い | fix | `grep -c 'cfg(test)' crates/pulsen/src/cli/render.rs` = 0 を確認。375行のほぼ全体が「エラー値 → 表示テキスト」の純関数の `match` なのにテストが1つも無い。実アダプターでは作れない `WireError` 5分岐・`Target::Failed`・`LockFailed`・`Create` 2分岐・`InvalidRepoPath` はユースケーステストが `render` を通らないため文言を誰も見ていない。1周目 W-035 の劣化はまさにこの無テスト領域で起きた | 0 |
| R2-W-025 | tests/common/mod.rs listed_files と TC-034 | fix | `listed_files`（:243-254）が `path.is_file()` で絞ることを確認。拒否経路がホーム直下や `workflows/` に新規ディレクトリを作ってその中に書いても検出されない。除外の理由として doc に挙がっているのは `state/` の自動作成1点だけなので、名前で `state` だけを除けば粒度を落とさずに済む。また `cli_add_error.rs:390`（TC-034）だけが `Untouched::of` で listings を持たず、異常系31件の中でここだけ新規ファイルの出現を見ない | 1（1周目 W-023） |
| R2-W-026 | conformance/exclusive_lock.rs TC-003 の非ブロッキング判定 | fix | :43-56 を確認。`elapsed < NON_BLOCKING` の判定は `try_acquire` が返った**後**にしか評価されず、保持の解放（`release_holder`）はさらに後。`try_acquire` が解放を待つ実装だと互いを待って失敗ではなく無限にハングする。ケースが名指しする失敗モード（待ってしまう実装）を、宣言したアサーションでは検出できない。AC-8 が約束する「後続スライスの実装に同じスイートを適用する」場面で効かない | 0 |
| R2-W-027 | adapter/task_id.rs:169-178 と task/path.rs:245-255 | fix | (a) `TaskId::parse(id.as_str().to_owned()) == Ok(id)` は、`TaskId` がフィールド非公開で `parse` 以外の生成経路を持たない以上どの値でも常に成立する型の不変条件の言い換え。しかもテスト名が主張する「組み立て規則が制約を満たす」は `generate` の `self.verified` フォールバック（task_id.rs:66-77）のせいで壊れていても検出できない。(b) `TaskFilePath::active(root, id) == active_dir(root).join(file_name(id))` は実装（path.rs:110-112）の本体そのもので、隣接する :219-243 が実際の配置を文字列で固定しているため何も足していない。CLAUDE.md「テストは振る舞いを表す。実装の内部構造に依存させない」 | 0 |
| R2-W-028 | tests/register_task.rs 対象検証 → 登録時検証の順序 | fix | `リポジトリの検証が返した分類はそのまま返り登録は行われない`（:534-556）が既定の正しい config を使うため、対象検証と登録時検証を入れ替えても両方通ることを確認。他の順序点はすべて空の台本を持つダブルのパニックで固定されているのに、この1点だけが無防備。利用者から見ると「リポジトリも定義も不正なとき、どちらの案内が出るか」が変わる（AC-14 の処理順） | 0 |
| R2-W-029 | 期待を半分しか見ていない適合ケース・ユニットテスト3件 | fix | (a) `conformance/config_store.rs:182-197`（TC-010）は `parse().is_err()` しか見ず、隣接する TC-009 / TC-011 が行っている「生トークンが `load` を通っても変わらない」検証が無い（spec 行の主張は「壊れたテンプレートもそのまま保持される」）。(b) `adapter/yaml.rs:270-277` の重複キーのテストは `error.message.contains("duplicate")` で、同モジュールが「YAML クレートを差し替えても影響はこのモジュールに閉じる」と宣言しているのにサードパーティの英語メッセージに依存している。(c) `adapter/task_file.rs:769-774` の未知キーのテストは `is_err()` のみで、テスト名が主張する「未知キーが原因」を確かめていない | 0 |

### wont-fix / defer

なし。31件すべて fix。

- 2周目の5本はいずれも「1周目の wont-fix 3件は蒸し返さない」と冒頭で宣言しており、実際に W-004（`unreachable!`）・W-012（成功時の tick 案内）・W-017（`create` の TOCTOU）は1件も再指摘されていない。
- 誤りと判定できる指摘は無かった。該当コードを1件ずつ読み、`grep -c 'error:?' … = 25`、`ALLOWED_SKIPS = 0` の3ファイル、`cfg(test)` が render.rs で0件、`break_lock_location` の不在、Issue #1 の `- [ ]` 346 / `- [x]` 0、PAGE-common 系6行が Issue #1 にしか無いこと、`AgentInput::Prompt` が workflow-store スイートに無いことなどを実測で裏付けた。
- **arch W-003 の案 a（実装を ADR-050 に寄せる）だけは採らない**。`load` の失敗時にユースケースが解決先パスを持たないことをコードで確認したため、実現にはポート表の変更が必要で AC-7 を壊す。指摘そのものは有効なので、案 b（ADR-054 を正として ADR-050・Issue コメント・progress.md を揃える）で fix する。
- スコープ外に出す（defer する）ものは無い。CI が無いために実行できない「MSRV 1.89 での実ビルド」（R2-W-020）も、版数の確定と残作業の記録という形で本 PR 内で完結させる（CI 追加は plan.md がスコープ外と定めている）。

### R2-B-001 の解き方（plan.md / ADR-027 / 1周目 W-030 の3つを同時に満たす形）

許容件数を**プラットフォーム**ではなく**環境の能力**で決め、かつ「何件まで」ではなく「どのケースなら許すか」を宣言する。

1. `SkipBudget` の宣言を `&[&str]`（許容するケース名の集合）に変える。集合外のスキップは即失敗、集合内でもスキップは `SKIP` 行として必ず出力する（W-030 の狙い＝スキップを緑に紛れさせない、を保つ）。
2. 集合を実行時に決める。テストファイル側で一時ファイルを `chmod 000` して読めるかを1度だけ probe し（`deny_read` と同じ述語）、制限が効かない環境（root・非 POSIX・権限を持たないファイルシステム）なら C 区分のケース名を集合に入れ、効く環境なら空集合にする。`static BUDGET: LazyLock<SkipBudget>` にすれば `const fn` の制約は外れる。
3. これで「環境が前提を作れない → 宣言どおりスキップして記録」（plan.md / ADR-027）と「フックの実装漏れ・想定外のスキップ → 失敗」（W-030）が両立し、後続スライスの in-memory 実装にスイートを適用する場面（AC-8）でも集合と実態のズレが集計で相殺されない。
4. `.adr/055`（R2-W-017 で新規起票）と `HOOKS.md:22` の記述をこの形に合わせる。

### R2-W-016 の決定（PAGE-common 系6行にチェックを付けるか）

**付ける（案 a）**。根拠と基準を `plan.md` に1節として書き、`steps.md` の対応表に注記を付け、Issue #1 にコメントで残す。

- 根拠: PAGE-common-002 / 003 / 005 / 006 / 010 と UC-flow-007 は Issue #1 にしか現れず、コマンド別の縮退規則（PAGE-tick-006 等）は Issue #2〜#6 に配分済み。見送ると孤児になり、どのスライスでも検証されなくなる。
- **チェックを付ける基準**: 台帳行の PASS 条件のうち、本スライスに存在するコマンド（`add`）の列がすべて満たされ、規則そのもの（ホーム解決・ロック取得・exit code・縮退4規則・タスクファイルの生涯）が実装として確定していること。後続スライスがコマンドを足したときの適用は、そのコマンドの台帳行が受け持つ。
- **チェックを付けない基準**: 環境が前提を作れずケースが走らなかった行（現時点では TC-port-clock-005 の1件）。理由を Issue コメントに残す（既存運用のまま）。
- 1周目 W-030 / 本ラウンド R2-W-001 の手当てが入るまで、CLI の TC-016 / 017 / 021 / 036 が「走った」ことは出力から確認できない。**チェックの記帳は全グループの修正が入った後に行い**、確認した実行環境（OS・root か否か・TMPDIR の位置）をコメントに明記する。

## 実行計画（ラウンド2）

グループ間でファイルは重複しない。順序依存は2本のみ — **G1 → G6**（`SkipBudget` の API と probe を G1 が確定し、G6 がそれを再利用する）と、**全グループ → G5 の記帳部分**（Issue のチェック付けは全修正が入った後）。

### グループ: `conformance-suite`（G1）

- 担当: R2-B-001, R2-W-002, R2-W-017, R2-W-021, R2-W-023, R2-W-026, R2-W-029(a)
- レビューファイル: `.thread/1/review/review-002-test.md`, `.thread/1/review/review-002-adapter.md`, `.thread/1/review/review-002-arch-spec.md`
- 方針:
  - R2-B-001: 上記「R2-B-001 の解き方」のとおり、`SkipBudget` を許容ケース名の集合 + 実行時 probe に変える。`.adr/055-conformance-skip-budget.md` を新規起票し（ADR-038 の書式・`承認済み`）、`.thread/1/adr.md:1042` の Status にファイル名を書き、`HOOKS.md:22` からその ADR を参照する。`adr.md` の ADR-053 / 054 の順序も入れ替える（R2-W-017 と同じ作業）。
  - R2-W-002: `yield_until_observed` を書き込みの**前**に1回置いて読み手が回り始めたことを確かめてから書く。書き込み前後の観測回数の差を取り `after - before > 0` を確かめる。TC-042 は「30周を終え、かつ観測が N 件以上（上限付き）」まで回す。
  - R2-W-021: `HOOKS.md:208-212` の「ADR-027 の一覧から変えた点」3項目を落とし、`.adr/027` のフック表が正であることだけを示す。残したい理由があれば `.adr/027` 側に why として書く（現存しない `break_lock_location` を出荷物から消すことが最低条件）。
  - R2-W-023: TC-007 に `status(&status("queued"))` が `AgentRun { input: AgentInput::Prompt(…) }` であることの等値比較を足す（TC-010 と同じ形）。
  - R2-W-026: TC-003 の `try_acquire` を `thread::scope` の子スレッドで走らせ、親は期限まで完了を監視する。期限超過なら先に `release_holder` してから失敗させる。
  - R2-W-029(a): TC-010 に「生トークンが `load` を通っても変わらない」等値比較を足す（TC-009 / TC-011 と同じ形）。
- 触るファイル: `crates/pulsen-conformance/src/lib.rs`, `crates/pulsen-conformance/src/task_repository.rs`, `crates/pulsen-conformance/src/workflow_store.rs`, `crates/pulsen-conformance/src/exclusive_lock.rs`, `crates/pulsen-conformance/src/config_store.rs`, `crates/pulsen-conformance/HOOKS.md`, `crates/pulsen/tests/conformance_config_store.rs`, `crates/pulsen/tests/conformance_lock.rs`, `crates/pulsen/tests/conformance_task_repository.rs`, `crates/pulsen/tests/conformance_time_id.rs`, `crates/pulsen/tests/conformance_workflow_store.rs`, `crates/pulsen/tests/conformance_worktree.rs`, `.adr/055-conformance-skip-budget.md`（新規）, `.adr/027-port-conformance-suite-and-harness-hooks.md`, `.thread/1/adr.md`

### グループ: `adapter-contracts`（G2）

- 担当: R2-B-002, R2-W-012, R2-W-013, R2-W-014, R2-W-015, R2-W-022, R2-W-029(b)
- レビューファイル: `.thread/1/review/review-002-adapter.md`, `.thread/1/review/review-002-arch-spec.md`, `.thread/1/review/review-002-test.md`
- 方針:
  - R2-B-002: `.adr/024` の `validate_repo` を5段（1. パスの存在確認が I/O エラー → `Failed` / 2. 存在しない → `NotFound` / 3. git の起動に失敗 → `Failed` / 4. 起動できて exit 非0 → `NotARepository` / 5. exit 0 → `Ok`）に直す。「影響」の「`Failed` の到達経路は『git を起動できない』1本に定まる」を「**対象の分類を確定できない状況**に限る（git を起動できない・パスの存在を確認できない・ブランチ名を扱えない）」に改め、判断基準（分類できたかどうかで分ける）を後続の `create` / `remove` にも効く形で残す。
  - R2-W-012: `fs::read` が `NotFound` を返したら `path.symlink_metadata()` を1回見て、`Err(NotFound)` なら `continue`（走査中に消えた）、`Ok(_)` なら `TaskEntry::Corrupt { path, message }` として報告する。「走査中に消えたエントリは飛ばす」というポート契約レベルの決定を `crates/pulsen-domain/src/task/port.rs` の契約 doc にも書く。宙ぶらりんのリンクを置いて `list_active()` がエントリを落とさないことを確かめる決定的なユニットテスト（unix 限定）を `adapter/task_repository.rs` に足す。
  - R2-W-013: `key_text` のエラーメッセージに**どのキーだったか**（値の表現と種別）を載せる。キーの経路を `convert` に渡せるなら論理位置も添える。`location` が `None` のままになるのは `Yaml` が位置を持たないためなので、その理由を why として残す。
  - R2-W-014: 除去対象の `GIT_*` 集合を1箇所の定数に置き、本番アダプターとフィクスチャの双方が同じ集合を使う。共有できないなら `tests/common/git.rs` の `env_remove` を7つに揃え、「前提判定はアダプターと同じ環境で行う」を why として残す。
  - R2-W-015: `worktree.rs` の `try_exists` の `Err` 枝を `format!("{}: リポジトリのパスを確認できない: {error}", repo.as_path().display())` 相当にする。`archive` の `Io` は `from` と `to` の両方をメッセージに載せる。
  - R2-W-022: `.adr/030` の決定に「`base_dir` は絶対パスであることを呼び出し側の前提とする」を1行足し、同じ前提を `FsWorkflowStore::new` の doc コメントにも書く（`std::path::absolute` が相対引数で cwd を読むため、ADR の「cwd を読むのは合成ルートの1箇所だけ」がこの前提に乗っていることを明示する）。
  - R2-W-029(b): `yaml.rs` の重複キーのテストを `contains("duplicate")` から「`YamlSyntaxError` になること + 位置が付くこと」の検証に変える（サードパーティの英語メッセージへの依存を切る）。
- 触るファイル: `crates/pulsen/src/adapter/task_repository.rs`, `crates/pulsen/src/adapter/yaml.rs`, `crates/pulsen/src/adapter/worktree.rs`, `crates/pulsen/src/adapter/workflow_store.rs`, `crates/pulsen-domain/src/task/port.rs`, `crates/pulsen/tests/common/git.rs`, `.adr/024-git-cli-shell-out-and-target-classification.md`, `.adr/030-workflow-store-base-dir-injection.md`

### グループ: `error-messages`（G3）

- 担当: R2-W-003, R2-W-009, R2-W-024, R2-W-029(c)
- レビューファイル: `.thread/1/review/review-002-adapter.md`, `.thread/1/review/review-002-arch-spec.md`, `.thread/1/review/review-002-domain.md`, `.thread/1/review/review-002-test.md`
- 方針:
  - R2-W-003: `task_file.rs` が触る値オブジェクトのエラー型（`BranchNameError` / `TimestampError` / `TaskIdError` 等）に、1周目 W-002 で `NameError` に置いたのと同じ `describe()` をドメイン側へ用意し、`task_file.rs` の `{error:?}` 25箇所をそれに置き換える。`render.rs:337-351` の日本語化も同じ `describe()` を呼ぶ形にして定義箇所を1つにする。
  - R2-W-009: `describe()` の doc が主張する「定義箇所は1つ」を実体と一致させる。`render.rs:125` は「ファイル名から決めた名前が{}」という**文中に埋める**用途なので、(a) `describe()` を「定義ファイルの値制約の説明」に狭めて CLI 引数向けの述語形が別に要る理由を why として残すか、(b) 文中に埋めない箇所（`render.rs:117-120`）だけ `describe()` を呼び `name_error` を「文中に埋める述語形」専用と doc に書くか、どちらかに決める。**「一元化したと書いてあるが一元化されていない」状態を残さないことが要件**。
  - R2-W-024: `render.rs` に `#[cfg(test)] mod tests` を置く。実アダプターでは作れないエラー（`WireError` 全5種・`Target::Failed`・`LockFailed`・`Create` 2種・`InvalidRepoPath`）について、期待する日本語の文言を明示的に固定する（`::` の不在を見る形ではなく、原因の語と構成要素を pin する）。純関数なので I/O は要らない。
  - R2-W-029(c): `task_file.rs` の未知キーのテストで、メッセージにキー名が含まれることを確かめる（`is_err()` だけをやめる）。
- 触るファイル: `crates/pulsen/src/cli/render.rs`, `crates/pulsen/src/adapter/task_file.rs`, `crates/pulsen-domain/src/definition/name.rs`, `crates/pulsen-domain/src/task/branch.rs`, `crates/pulsen-domain/src/task/time.rs`, `crates/pulsen-domain/src/task/id.rs`

### グループ: `domain-contracts`（G4）

- 担当: R2-W-005, R2-W-006, R2-W-010, R2-W-011
- レビューファイル: `.thread/1/review/review-002-domain.md`, `.thread/1/review/review-002-test.md`, `.thread/1/review/review-002-usecase-cli.md`
- 方針:
  - R2-W-005: `effective_*` の doc の根拠を definition ドメイン内で閉じる形に書き換える（「引数のステータス名がこの定義に属することは呼び出し側の責務。属さない名前には適用対象がないため既定値を返す」）。task ドメインの不変条件1への言及は「タスク経由の呼び出しではこの責務が自動的に満たされる」という補足に留める。`snapshot.rs:114-118` から `Wait` に対する `effective_timeout` / `effective_agent` / `effective_model` の3行を落とし、委譲の検証（`initial` / `statuses` / `status`）だけ残す。シグネチャは1周目 W-003 のとおり `&StatusName` のまま（spec:165-168 / AC-7）。
  - R2-W-006: 平行配列をやめ、`for (key, is_present) in [("agent", options.agent.is_some()), …]` のようにキー名と述語を組で書く。`AGENT_RUN_KEYS` を別に公開する必要がなければ畳む。テストは6キーをループで回す1本にする（`run: wait` に各キーを1つずつ足す）。あわせて `InvalidValue` の `location` を生成箇所ごとに1件ずつ固定する（現状4/11箇所）。
  - R2-W-010: `UnknownAgent` / `InvalidAgentDefinition` の2種だけ、同じ値をすでに積んでいれば push しない（`errors.contains(&error)`。両バリアントとも `PartialEq` を導出済み）。`同じ未定義エージェントを参照する複数ステータスは1件にまとまる` を仕様の言葉で1本足す。既存テストは全ステータスが別々の誤りを持つ台本なので影響しない。
  - R2-W-011: `worktree_root()` は**アクセサを残し why を1行付ける**。構築（`WorktreeRoot::parse` による絶対性検証）が ADR-031 のレイアウトの一部であり `create` を足すスライスが使うためで、参照が0だった `Runtime::home()`（1周目 W-011 で削除）とは条件が違う。「未使用の `pub` は、ADR がレイアウトとして列挙し構築時検証の対象であるものに限り why 付きで残す。それ以外は落とす」という基準を doc に書き、後続スライスに伝わる形にする。
- 触るファイル: `crates/pulsen-domain/src/definition/workflow.rs`, `crates/pulsen-domain/src/definition/snapshot.rs`, `crates/pulsen-domain/src/definition/assembler.rs`, `crates/pulsen-domain/src/definition/validator.rs`, `crates/pulsen/src/application/home.rs`

### グループ: `docs-adr-issue`（G5）

- 担当: R2-W-004, R2-W-008, R2-W-016, R2-W-018, R2-W-019, R2-W-020（+ R2-W-007 / R2-W-010 の記録面）
- レビューファイル: `.thread/1/review/review-002-arch-spec.md`, `.thread/1/review/review-002-adapter.md`, `.thread/1/review/review-002-usecase-cli.md`, `.thread/1/review/review-002-test.md`
- 方針:
  - R2-W-004: `.adr/050` の決定に「ワークフロー定義のスキーマ違反（`UnknownKey` / `InvalidValue`）は ADR-054 で範囲を限定した」を1行足す。`progress.md:9` を同じ範囲に直し、Issue #1 のコメント3件目に追補を投稿して提案範囲を config.yaml に限る。あわせて「`WorkflowLoadError::Parse` は解決先パスを持たないため、名前指定のワークフローがスキーマ違反のとき対象ファイルを案内できない。`Parse { error, resolved_from }` へポート表を改める提案」を spec 追従の提起として `progress.md` と Issue コメントに足す。**コードは変えない**（AC-7 を壊すため）。
  - R2-W-008: `.thread/1/testing.md:44-48` の grep を `crates/*/src/` に限定し、期待を「`crates/pulsen-domain/` に1件も現れず、`crates/pulsen/src/` 側のヒットは `util/atomic.rs` だけ（`tests/` 配下は適合ハーネスの権限操作フックでアダプター層の隔離とは別の話）」に直す。`plan.md` の AC-1 の文言も同じ趣旨に揃える。
  - R2-W-016: 上記「R2-W-016 の決定」の基準を `plan.md` に1節として書き、`steps.md` の対応表（:432-437 付近）に注記を付け、Issue #1 に「PAGE-common 系6行にチェックを付ける読み方と根拠（6行が Issue #1 にしか無いこと、コマンド別の行は Issue #2〜#6 に配分済みであること）」をコメントする。
  - R2-W-018: `steps.md:57` を「制約のある名前系 newtype 6種は `parse` のみで生成し、制約を持たない `InputText` は総関数（`new`）で生成する」に直す。
  - R2-W-019: **全グループの修正が入った後に** Issue #1 のチェックリスト346行を記帳する。TC-port-clock-005 はチェックせず（理由は投稿済み）、それ以外は R2-W-016 の基準で付ける。確認した実行環境（OS・root か否か・TMPDIR がリポジトリ外であること）をコメントに明記する。`progress.md:26` の残作業にこの順序を1行書く。
  - R2-W-020: `.adr/023:24` の記述を確定させる — `std::env::home_dir()` の非推奨が解除された実際のリリースを調べて版数を明記し、それが 1.89 より後なら `Cargo.toml` の `rust-version` を引き上げる。オフラインで確定できない場合は、版数の主張をやめて実測できた事実（「本環境の 1.97 では非推奨ではない」）だけを書く。いずれにせよ「MSRV 1.89 でのビルドは CI が無いため未検証」を `progress.md` の残作業に残す（CI 追加は plan.md がスコープ外）。
  - R2-W-007（記録面）: `.thread/1/testing.md:176-181` の手動手順2 の手順3 を `HOME=$(mktemp -d)` を前置きする形に変える（既定ホーム解決の確認という目的は保ったまま、実 `~/.pulsen/` に登録が起きる余地を消す）。
  - R2-W-010（記録面）: `progress.md` の「spec へ追従を提起する点」に「`UnknownAgent` / `InvalidAgentDefinition` はエージェント単位の誤りなので、複数ステータスが同じエージェントを参照しても1件にまとめる」を足し、Issue コメントにも同じ提起を投稿する。
- 触るファイル: `.adr/050-schema-error-location-is-logical.md`, `.adr/023-dependency-selection.md`, `Cargo.toml`（`rust-version` を上げる場合のみ）, `.thread/1/plan.md`, `.thread/1/steps.md`, `.thread/1/testing.md`, `.thread/1/progress.md`, GitHub Issue #1（チェックリストとコメント）

### グループ: `acceptance-tests`（G6）

- 担当: R2-W-001, R2-W-007, R2-W-025, R2-W-027, R2-W-028
- レビューファイル: `.thread/1/review/review-002-test.md`, `.thread/1/review/review-002-arch-spec.md`, `.thread/1/review/review-002-usecase-cli.md`
- 方針（**G1 の後**に着手する — `SkipBudget` の集合宣言と probe を再利用するため）:
  - R2-W-001: `tests/common/mod.rs` に CLI 用のスキップ集計を置き（`pulsen_conformance::SkipBudget` をそのまま使う。`pulsen-conformance` は `pulsen` の dev-dependency）、`cli_add_error.rs` の4件（TC-016 / 017 / 021 / 036）の `println!` + `return` をそこ経由にする。unix・非 root・リポジトリ外 TMPDIR での宣言は空集合。TC-036 の前提（`is_outside_repository`）はテストファイル冒頭で1度だけ評価する。
  - R2-W-007: `Add::run` / `run_cli` の既定で `HOME` / `USERPROFILE` を毎回作る一時ディレクトリに向け、`user_home()` はその上書きにする（`PULSEN_HOME` の `env_remove` と同じ扱いに揃える）。`cli_add_boundary.rs:410-430` の既定ホーム経路のケースに、`--home` 側・環境変数側の双方でタスクが作られていないことの確認を足す。
  - R2-W-025: `listed_files` を「直下のエントリ名（ファイル・ディレクトリとも）から `state` だけを除いた集合」にする。`cli_add_error.rs:390`（TC-034）を `home.untouched()` + 外部の定義ファイルの追加控えに揃える。
  - R2-W-027: `task_id.rs:169-178` を `yyyymmddThhmmss-<8桁>` の形を文字列として固定するテスト（:150 付近の構成のテスト）に統合するか、`compose` を直接検証する形に変える。`path.rs:245-255` は削る（隣接の :219-243 が実際の配置を文字列で固定済み）。
  - R2-W-028: 対象検証が失敗する台本に、登録時検証も失敗するワークフロー（`{model}` を要求する config 等）を組み合わせたケースを1件足し、`Target` が返ることと ID が発行されないことを確かめる。
- 触るファイル: `crates/pulsen/tests/common/mod.rs`, `crates/pulsen/tests/cli_add_error.rs`, `crates/pulsen/tests/cli_add_boundary.rs`, `crates/pulsen/tests/register_task.rs`, `crates/pulsen/src/adapter/task_id.rs`, `crates/pulsen-domain/src/task/path.rs`
