# レビュー 001 — PR #8 (Issue #1)

## Architecture / Spec-conformance

### 総評

依存方向・合成ルート・ポート契約・チェックリスト消化のいずれも、この規模のスライスとしては高い水準で揃っている。**コード側にマージを止める欠陥は見つからなかった。** `pulsen-domain` は外部クレート0依存、OS 依存分岐は本番コードでは `crates/pulsen/src/util/atomic.rs` の1箇所のみ、`todo!` / `unimplemented!` / スタブは0件、適合ケースは spec の125行に1:1で対応して125件が実際に実行され PASS する。Issue のチェックリスト346行はすべて `spec/inventory/` の台帳行に実在し、54行をサンプリングした範囲で全行が実装に到達した。

指摘は **(a) 成果物ドキュメント（`.adr/` の起票規則・working log の Status・HOOKS.md の参照先）の自己矛盾**と、**(b) 受け入れ基準の文言と実装がわずかに食い違う箇所（AC-8 / AC-15）** に集中している。いずれも「実装が仕様に反している」のではなく「ドキュメントが実装より先に書かれたまま更新されていない」型の乖離である。

---

### Blockers

なし。

---

### Warnings

- **[W-001]** 適合スイートのフックが YAML の生テキストを受け取っており、AC-8 の「後続スライスの in-memory 実装がフックを実装するだけで同じスイートを通せる」が ConfigStore / WorkflowStore で成立しない
  - 場所: `crates/pulsen-conformance/src/lib.rs:205` (`put_config(&str)`)、`:238` (`put_named(name, text)`)、`:243` (`put_named_with_ext`)、`:255` (`put_at_absolute`)、`:261` (`put_at_relative`)
  - 理由: AC-8 の括弧書きは「生 JSON 文字列を渡す API を持たない」なので字義上は満たしているが、主節「フックは**破損・状況の意味**だけを受け取り」と、同クレートの自称（`lib.rs:15-17`「生の JSON テキストや権限操作の手順をスイート側に持ち込まない」）に対しては YAML ソースが例外になっている。実害は後続スライス側に出る — plan.md「含まれないもの」が予告する**汎用 in-memory アダプター**が同じ125件を通すには、in-memory な `ConfigStore` / `WorkflowStore` が `put_config` / `put_named` を実装するために YAML パーサを抱え込むか、`crates/pulsen` の `adapter::yaml` に依存する必要が出る。
  - なお、この設計自体は不可避に近い。`TC-port-config-store-014`（YAML 構文エラー）や `TC-port-workflow-store-017`（重複キー）は、生テキストを渡す口が無ければ表現できない。したがって直すべきは**コードではなく AC-8 の文言と HOOKS.md の記載**である可能性が高い。
  - 提案: HOOKS.md に「ConfigStore / WorkflowStore の入力系フックは YAML ソースを受け取る。この2ポートのスイートは YAML 表現に結合している」を明記し、AC-8 の「フックを実装するだけで」の適用範囲を TaskRepository / Clock / TaskIdGenerator / ExclusiveLock / WorktreeManager に限定する。判断として残すなら ADR を1件足す。

- **[W-002]** ADR-035 が定めた起票規則が、ADR-037 以降の7件で未適用
  - 場所: `.adr/035-file-slice-adrs-from-019.md:31`（「実装中に生じた新しい決定は、同じ規則で連番を続けて起票する(ADR-037 以降)」）に対し、`.thread/1/adr.md:625` (ADR-038)、`:747` (043)、`:792` (045)、`:817` (046)、`:902` (049)、`:956` (051)、`:976` (052) に対応する `.adr/` ファイルが無い
  - 理由: `.thread/1/adr.md` は ADR-019〜052 の34件を持つが、`.adr/` にあるのは25件。9件のうち ADR-041 と ADR-047 は Status 行に「`.adr/027` に反映済み」「`.adr/036` に反映済み」と明記があり、実際に反映も確認できるので問題ない。残る7件には規則からの逸脱を説明する記述がどこにもない。中身は些末ではない —
    - **ADR-046**（適合ケースは「操作の後の観測」にスキップ可能なフックを使わない）は、後続スライスがケースを足すたびに従う必要があるスイート全体の規約
    - **ADR-049**（`--base` は `-` 始まりも値として受け取る）は、後続の全サブコマンドが揃えるべき CLI フラグ方針
    - **ADR-045**（`TaskFileDto<Snapshot>` の型引数化）は `.adr/025` に含まれない DTO の設計判断
    - **ADR-052**（受け入れテストの起動基盤を `tests/common` に集約）は後続スライスのテスト配置を縛る
  - `.adr/035` の「後続スライスの担当が根拠を辿れない」を防ぐという目的に照らすと、これらが正本に無いのは目的の未達である（`.thread/1/` はコミットされているので参照は切れないが、正本の権威と索引性は失われている）。
  - 提案: 7件を `.adr/` に起票する。あるいは ADR-035 に「本文が既存 ADR に反映される場合と、スライス限りの作業規約は起票しない」といった除外規則を追記し、各エントリの Status 行にその区分を書いて機械的に判別できるようにする。

- **[W-003]** `.thread/1/adr.md` の Status が実態と乖離しており、「`.adr/` に起票済みか」の索引として機能していない
  - 場所: `.thread/1/adr.md:8` (ADR-019)、`:95` (022)、`:124` (023)、`:403` (029)、`:521` (034)、`:541` (035) がいずれも `Proposed` のまま／`:929` (ADR-050) の Status がファイル名を伴わない
  - 理由: `.adr/035:15-24` の確定表では 019・022・023・029・035 はステップ1、034 はステップ3で承認済みに更新されるはずで、実際に `.adr/` 側の6ファイルはすべて `承認済み` になっている。working log だけが取り残されている。さらに working log 側の暗黙の慣習（Status 行に `.adr/` のファイル名を書けば起票済み）が両方向に破れている — この6件は起票済みなのにファイル名が無く、ADR-050 は `.adr/050-schema-error-location-is-logical.md` が存在するのに Status に書かれていない。W-002 の「未起票7件」を探そうとした読み手が Status 行を索引に使うと誤った集合を得る。
  - 提案: 6件を `Accepted（ステップNで確定。<.adr パス>）` に更新し、ADR-050 の Status にファイル名を足す。

- **[W-004]** クレートに同梱されるドキュメントが、根拠として `.thread/1/adr.md` を指している
  - 場所: `crates/pulsen-conformance/HOOKS.md:200`（「根拠は `.thread/1/adr.md` の ADR-041 にある」）
  - 理由: ADR-041 の内容は `.adr/027-port-conformance-suite-and-harness-hooks.md:33-35` にすでに反映済み（`another_generator` / `absent_branch_name` / `unusable_lock` の3点がそのまま載っている）。出荷物からスライス作業用のディレクトリを指すのは、`.adr/035` が防ごうとした状態そのものである。`crates/` 配下で `.thread/` を参照しているのはこの1箇所だけなので、直せば規則が全体で揃う。
  - 提案: 参照先を `.adr/027-port-conformance-suite-and-harness-hooks.md` に差し替える。

- **[W-005]** AC-15 のうち境界値の拒否4件で「ワークフロー定義ファイル・config.yaml が変更されない」が検証されていない
  - 場所: `crates/pulsen/tests/cli_add_boundary.rs:57` (`reject_base`)、`:118` (TC-053)、`:182` (TC-058)
  - 理由: AC-15 は対象として「異常系 TC-014〜048 **と境界値の拒否ケース TC-053・054・055・058**」を明示し、条件を「タスクが作られず、**かつ**ワークフロー定義ファイル・config.yaml が変更されない」と書いている。異常系31件は `cli_add_error.rs:26` の `reject` / `:45` の `reject_target` ヘルパが `Untouched::assert_unchanged()` を通しているのに、境界値の拒否側は `home.has_no_task()` だけで止まっている。原因は steps.md ステップ20（`.thread/1/steps.md:418`）が拒否側の要件を「タスクが作られないこと」としか書いておらず、plan.md AC-15 と食い違っていること。実装は steps.md に忠実で、plan.md には忠実でない。
  - 性質としては「構造上そもそも書き込み経路が無いので実害はまず出ない」が、AC が明示的に列挙している以上、満たしたと言い切れる状態ではない。
  - 提案: `reject_base` と TC-053 / TC-058 のループにも `home.untouched()` → `assert_unchanged()` を通す（3箇所の追加で済む）。あわせて steps.md ステップ20 の記述を AC-15 に揃える。

- **[W-006]** 適合スイートのスキップが通常の `cargo test` では観測できず、緑と区別がつかない
  - 場所: `crates/pulsen-conformance/src/lib.rs:150-159`（`CaseOutcome::report` が `println!` でスキップを報告する）
  - 理由: libtest は成功したテストの標準出力を握りつぶすため、`--nocapture` を付けない限り「何も表明しなかったケース」が `test result: ok` の中に紛れる。実測（`cargo test -p pulsen --test conformance_* -- --nocapture`）では現環境のスキップは `tc_port_clock_005` の1件だけで、権限系フックは制限が効いたことを確認してから `Some` を返す規則（`tests/common/mod.rs:389-406`、`tests/conformance_task_repository.rs:161-180`）が守られているため C 区分の11行中10行が実際に走っている。問題は環境が変わったときで、Windows では権限系フックが `#[cfg(not(unix))]` で一律 `None` を返す（`tests/conformance_task_repository.rs:198,203`、`tests/common/mod.rs:408`）ため、8件が黙って素通りする。「125件 PASS」という数字が環境によって意味を変えるのに、その差がテスト出力に現れない。
  - 提案: アダプター側のテストファイルで「このアダプターで許容するスキップは N 件」を宣言し、超えたら失敗させる（`conformance_cases!` にスキップ集計を持たせる）。最小の手当てとしては `eprintln!` にするだけでも `--nocapture` 忘れの事故は減る。

- **[W-007]** `.thread/1/progress.md` の「全件が実行された」が、同じ節の表と矛盾している
  - 場所: `.thread/1/progress.md:12`（「現在の開発環境（macOS・非 root・一時ディレクトリはリポジトリ外）では**全件が実行された**」）と `:20`（TC-port-clock-005 のスキップ条件が「システム時計を過去に設定できないため常にスキップ」）
  - 理由: 実測でも clock TC-005 はスキップされている。「全件が実行された」は事実と異なる。残存課題の記述としては、むしろ正確な件数（1件スキップ）が書かれているほうが後続の判断材料になる。
  - あわせて、plan.md「リスクと注意点」と Issue の完了条件が求める「スキップした行の理由を Issue のコメントに残す」「spec 追従の提起を Issue のコメントに残す（ADR-050 / ADR-051 由来の2件）」が未実施である（`gh issue view 1 --comments` に存在するコメントは TC-port-task-repository-022/028 の言い換え提案1件のみ）。progress.md はこれを「提起する点」と書いており隠してはいないが、Issue のチェックを付ける前提条件として残っていることは明記されていない。
  - 提案: `:12` を「TC-port-clock-005 の1件を除き全件が実行された」に直す。「Issue コメント未投稿（スキップ理由1件・spec 追従2件）」を残存課題として1行足す。

- **[W-008]** `InputText` が spec の生成規約から外れているが、ADR にも spec 追従の提起にも記録がない
  - 場所: `crates/pulsen-domain/src/definition/name.rs:101`（`pub fn new(s: String) -> Self`）
  - 理由: `spec/domains/definition.md:38` は名前系 newtype について「いずれも `parse(s: String) -> Result<Self, NameError>` でのみ生成する」と書く。`InputText` は同じ表の「制約なし」行なので `Result` は空虚になり、`new` にした判断自体は妥当（コード上も `SkillInputTemplate::render` が `Result` を扱わずに済む理由が書かれている）。問題は記録の非対称性で、同程度の spec 逸脱である ADR-039（`ReadError` の共有）・ADR-040（`rehydrate` の束）・ADR-048（parse の位置）・ADR-050（エラー位置の粒度）はいずれも ADR として残っているのに、ここだけドキュメンテーションコメント止まりになっている。
  - 提案: progress.md の「spec へ追従を提起する点」に1行足す（spec 側の「いずれも」を「制約のある型は」に言い換える提案）か、ADR を1件起票する。

- **[W-009]** PAGE-common-011（設定・ワークフロー定義の作成コマンドを提供しない）を守るテストがない
  - 場所: `crates/pulsen/src/cli/args.rs:22-25`（`enum Command { Add(AddArgs) }`）
  - 理由: plan.md は「PAGE-common-007 / PAGE-common-011 は『提供しないことを確認する行』としてチェックリストに載っており、消化対象である」と明記している。現状この行は**不在によってのみ**満たされており、後続スライスが `pulsen init` を足しても何のテストも落ちない。同様の否定的主張である TC-052（`.yml` へフォールバックしない）は、steps.md ステップ20 が明示的に「`.yaml` 版の成功と `.yml` のみ版の `NotFound` を対にして検証する」として網を張っているので、扱いが揃っていない。`.thread/1/testing.md` の「既存機能への影響確認」に手動確認項目としては書かれているが、自動化されていない。
  - 提案: `pulsen --help` の出力に対する受け入れテストを1件足し、サブコマンドの集合がちょうど期待どおりであることを表明する（PAGE-common-007 の「機械可読出力オプションが無い」も同じテストで拾える）。

- **[W-010]** ユースケース層で消化する5行（TC-012 / 018 / 040 / 047 / 048）だけ、テスト名から台帳行への対応が読み取れない
  - 場所: `crates/pulsen/tests/register_task.rs:343`（TC-012）、`:427`（TC-018）、`:550`（TC-040）、`:369`（TC-047）、`:390`（TC-048）
  - 理由: CLI 受け入れテスト62件は `tc_task_register_task_NNN_<仕様の言葉>` という命名で台帳行と1:1に対応づけられているのに、ユースケーステストだけ TC ID を持たない記述的な名前になっている。AC-18 は「実アダプターでは外から状況を作れない5行が…ユースケーステストとして実装され」ていることを求めており、Issue の完了条件は「実装をレビューで確認できた行にのみチェックを付ける」としている。5行がどのテストに対応するかはファイル冒頭のコメント（`register_task.rs:3-5`、`cli_add_error.rs:7-9`）から推測はできるが、行単位の対応が機械的に取れない。同一プロジェクト内で命名規約が2つに割れている点も一貫性を欠く。
  - 提案: CLI テストと同じ接頭辞に揃えるか、各テストのドキュメンテーションコメントに対応する TC ID を書く。

- **[W-011]** 対象検証のエラーメッセージに Rust の Debug 表現が混入し、利用者にそのまま表示されうる
  - 場所: `crates/pulsen/src/adapter/worktree.rs:104`（`format!("HEAD のブランチ名を扱えない: {error:?}")`）
  - 理由: この `message` は `TargetError::Failed` に載り、`cli/render.rs:243-245` が「原因: {message}」としてそのまま標準エラー出力に流す。`error` は `BranchNameError` なので、表示は `ContainsWhitespaceOrControl { char: ' ', position: 3 }` のような構造体表記になる。`spec/pages/index.md:14`「出力は人間可読なテキストとする」に対する取りこぼしで、`render.rs:346-359` に同じエラー型の日本語化（`branch_name_error`）がすでにあるのと対照的。到達経路は「git 側では有効だがドメインの実用サブセットに乗らないブランチ名」に限られるため実害の頻度は低いが、レイヤーの責務としては「アダプターが利用者向け文言を組み立てている」ことになる。
  - 提案: メッセージにブランチ名の文字列だけを載せ（分類は `Failed` で十分）、文言の組み立ては `render.rs` 側に寄せる。

- **[W-012]** HOOKS.md の対応表に、spec 行の一部しか検証していない行と、区分基準に合っていない行がある
  - 場所: `crates/pulsen-conformance/HOOKS.md:132`（TC-port-task-repository-043）、`:143`（TC-port-clock-003）
  - 理由:
    - TC-port-task-repository-043 の spec 行（`spec/testcases/ports/task-repository.md:88`）は前提を「`save` が `Err` を返した（**NotFound / Io**）」としているが、実装（`crates/pulsen-conformance/src/task_repository.rs:735-751`）は NotFound 分岐しか通していない。HOOKS.md 自身が「Io 分岐は `make_unwritable(Active)` を使えるが、NotFound 分岐だけで観測できる」と正直に書いているものの、この行は125行の対応表では「埋まった」扱いになっている。原子性の観測面という行の趣旨（`Err` の後に部分的な結果が残っていない）は、書き込みが実際に始まってから失敗する Io 分岐でこそ意味を持つ。
    - TC-port-clock-003 の spec 行（`spec/testcases/ports/clock.md:11`）には「…テスト中に時刻改変が起きないアダプター環境に限る」という但し書きがあり、HOOKS.md:12 が定める C の定義（spec が「再現できるアダプター環境に限る」と明示する行）に該当する。表では B になっている。挙動は同じ（`require!` で同様にスキップする）だが、A 28 / B 86 / C 11 の集計が基準どおりでなくなる。
  - 提案: 043 に `make_unwritable(Active)` を使う Io 分岐を足す。003 を C に直し、集計を A 28 / B 85 / C 12 に更新する。

---

### 受け入れ基準の検証結果（AC-1〜AC-20）

| AC | 判定 | 根拠 |
|---|---|---|
| AC-1 | ✅ | `cargo build` / `cargo test`（418件・全 PASS）/ `cargo clippy --all-targets -- -D warnings` / `cargo fmt --check` すべて通過。`crates/pulsen-domain/Cargo.toml:8` の `[dependencies]` は空で、domain に外部クレート名の出現も0件。OS 依存分岐は**本番コードでは `crates/pulsen/src/util/atomic.rs:62,69` の1箇所のみ**、`crates/pulsen-domain/` は0件。※ AC-1 の grep 式（`cfg(unix)\|cfg(windows)`）は `cfg(not(unix))` と `cfg!(windows)` を取りこぼす。全形を拾っても結論は変わらないが（追加ヒットは `tests/` 配下のみ）、判定式としては不完全 |
| AC-2 | ✅ | `definition/{name,duration,command,template,agent}.rs`。全エラー分岐のユニットテストあり |
| AC-3 | ✅ | `definition/workflow.rs:138,148,158,171` の `effective_*`（内蔵既定 1h / 2）、`definition/reference.rs:58-66` の `display_name` 4規則。区切り文字集合は `reference.rs:8,10` の定数で、`:102-111` が `/` と `\` の両方を明示的に渡して検証 |
| AC-4 | ✅ | `definition/assembler.rs`。`WorkflowParseError` は12種で、うち `YamlSyntax` / `UnknownKey` はアダプター生成（spec/domains/definition.md:195 と一致）、`assemble` が返すのは AC-4 が列挙する10種ちょうど。循環・自己参照・到達不能は受理（`workflow.rs` のテスト） |
| AC-5 | ✅ | `definition/validator.rs:53-71`。ステータス横断でエラーを蓄積し、空なら `WorkflowSnapshot::from_validated`（`pub(crate)`）。5種すべて実装 |
| AC-6 | ✅ | `task/task.rs:74,103`、`task/degraded.rs:65`、`task/state.rs:92`（6状態が付随データ付き）、`task/task.rs:104-109`（不変条件1）、`task/time.rs`（RFC3339 往復・閏年・範囲飽和を自前実装、13テスト） |
| AC-7 | ✅ | `TaskRepository` 7メソッド（`task/port.rs:130-151`）、`TargetError` 5種（`execution/port.rs:11-25`）、`ConfigLoadError` 3種（`definition/port.rs:11-29`）、`WorkflowLoadError` 3種（`:78-91`）、`LockError`（`execution/port.rs:47-53`）。spec のポート表と突合して差異なし。未実装メソッドの宣言・スタブは0件（`execution/port.rs:3-5` が方針を明記） |
| AC-8 | ⚠️ | 125行の対応表（HOOKS.md）は行数・ID・組み立て手段のすべてが実装と一致。`Sync` は `lib.rs:370` の `concurrent_repo` にのみ現れ、`lib.rs:562-637` の `NotSyncHarness` テストが伝播しないことをコンパイル時に保証している。**ただし ConfigStore / WorkflowStore のフックが YAML 生テキストを取る（W-001）** ほか、区分と網羅の細部に不整合（W-012） |
| AC-9 | ✅ | 24件、実行して全 PASS |
| AC-10 | ✅ | 31件、実行して全 PASS。`.yml` フォールバックなし（`adapter/workflow_store.rs:15`）、相対は注入 `base_dir` 基準（`:43,81`） |
| AC-11 | ✅ | 44件、実行して全 PASS。tasks→archive の解決順（`adapter/task_repository.rs:185-193`）、`Conflict` の横断確認（`:131-135`）、破損分類（`adapter/task_file.rs:7-17` の表）、命名形式フィルタ（`:102`）、`save_degraded` の温存（`:172-179`） |
| AC-12 | ✅ | Clock 5 / TaskIdGenerator 5 / ExclusiveLock 7 / WorktreeManager 9 = 26件が実装され、TC-port-clock-005 のみスキップ、残り25件が PASS。`failing_manager` は存在しないパスで構築した2つ目の manager（`adapter/worktree.rs:26-31` の注入設計）で、権限操作にも root にも依存しない |
| AC-13 | ✅ | `cli/wire.rs:157-170`（`--home` > `PULSEN_HOME` > `~/.pulsen`）、`:137-140`（全サブコマンド共通の起動時 `ConfigStore::load`）、`cli/render.rs:76-85`（未初期化の案内・ホームパス・作成指示）。TC-014 / TC-067 で検証 |
| AC-14 | ✅ | `application/register_task.rs:127-184` が `spec/usecases/task.md` の処理フロー1〜8と同順。`Conflict` は `retried` フラグで1回だけ再発行（`:172-178`）、再衝突は `Create(Conflict)`。成功表示は `cli/render.rs:22-33` |
| AC-15 | ⚠️ | 異常系31件は `cli_add_error.rs:26,45` のヘルパ経由で「タスク不作成」「利用者リソース不変」の両方を検証。**境界値の拒否4件は前者のみ（W-005）** |
| AC-16 | ✅ | `cli_add_boundary.rs` の受理側15件。TC-052 は `.yaml` の成功と `.yml` のみの `NotFound` を対で検証 |
| AC-17 | ✅ | `cli_add_normal.rs` の TC-009/010、`cli_add_boundary.rs:233` の TC-060。`adapter/task_file.rs:363` が `to_vec_pretty` で人間可読な JSON を書く |
| AC-18 | ✅ | `tests/register_task.rs` の20件がすべてテストダブル（`pulsen_conformance::doubles`）に対して書かれ、実プロセス・実ファイルシステムを使わない。5行の消化はコメントで示されるのみ（W-010） |
| AC-19 | ✅ | アトミック置換は `util/atomic.rs` の1箇所のみ（`fs::rename` / `NamedTempFile` の出現も同ファイルに限られる）。`adapter/task_repository.rs` は `write_atomic` / `rename_atomic` を呼ぶだけ。排他ロックの原始的操作（`File::try_lock`）も `adapter/lock.rs:46` の1箇所のみ |
| AC-20 | ✅ | Issue の346行はすべて `spec/inventory/` の台帳行に実在（差分0）。54行のサンプリングで全行が実装に到達。スコープ外項目（RunStore / ProcessController / CommandRunner / `WorktreeManager::create` / `remove` / `Task` の他遷移関数 / `WorkspacePlanner` / execution のドメインサービス）は grep で0件、逆にスコープ内で欠けている行もなし |

### 依存方向・合成ルートの検証

- **クレート境界**: `pulsen-domain` は依存0（`Cargo.toml:8`）。`pulsen-conformance` は `pulsen-domain` のみに依存し、`pulsen` からは `[dev-dependencies]` でしか参照されない（`crates/pulsen/Cargo.toml:16-17`）ので本番の依存グラフに漏れない。
- **クレート内**: `application/` の `use` は `std` と `pulsen_domain` だけ（`home.rs:7,9`、`register_task.rs:7,9-17`）。アダプター型はアプリケーション層にもドメインにも1つも現れない。逆流はなし。ADR-031 がこの性質を明示的に設計判断として残している。
- **合成ルート**: `cli::wire::compose`（`cli/wire.rs:131-154`）の1箇所のみ。`env::current_dir()` / `env::var_os` / `env::home_dir()` の読み取りもここに閉じている。`cli/add.rs:23-34` は `Runtime` からポートを取り出して `RegisterTask` に渡すだけ。
- **ドメイン内の方向**: `definition` は他ドメインを import せず、`task` は `definition` のみ、`execution` は `task` のみ。ADR-017 のポート所有と一致（`definition/port.rs` = ConfigStore/WorkflowStore、`task/port.rs` = TaskRepository/TaskIdGenerator/Clock、`execution/port.rs` = WorktreeManager/ExclusiveLock）。

### `.adr/` の整合

`.adr/019`〜`050` の25件はすべて `承認済み` で、25件すべてについて実装側の対応物が特定でき、決定内容と一致した（不一致0件・検証不能0件）。すべて `提案中` で起票してから確定ステップで昇格しており、ADR-035 の運用は `.adr/` 側では守られている。既存の `.adr/001`〜`018` との矛盾も見つからなかった — 特に ADR-013（未知キー拒否）は `adapter/{config_store,workflow_store}.rs` の許容キー集合で両ファイルとも守られており、ADR-042（値の書かれていないキー = 省略）とも衝突しない（誤記されたキーは `yaml.rs:180-186` の `unknown_key` が生の `entries()` を見るので依然として弾かれる）。ADR-015（スナップショットの埋め込み）は ADR-025 が `Option<Box<RawValue>>` + `skip_serializing_if` で温存性ごと引き継いでいる。ADR-016 は本スライスに実行経路が無く、抵触しうる実装も存在しない。ADR-017 は上記のとおり。

問題は `.adr/` の**中身**ではなく**起票の網羅性と working log の同期**にある（W-002 / W-003 / W-004）。

### スコープ（plan.md「含まれないもの」）の検証

除外項目はいずれも混入していない。`RunStore` / `ProcessController` / `CommandRunner` / `WorktreeError` / `RemoveOutcome` / `WorkspacePlanner` は `crates/` に1件も出現せず、`WorktreeManager` は `validate_repo` / `head_branch` / `branch_exists` の3メソッドのみ（`execution/port.rs:32-41`）。`Task` の遷移関数は `register` / `rehydrate` のみで、`confirm_workspace` / `record_launching` / `complete_run` / `abort` / `retry` / `set_status` / `current_status_def` / `applicable_retry_limit` も、`TransitionError` / `RetryError` / `SetStatusError` も存在しない。`DegradedTask::execution_kind`（`task/degraded.rs:107`）はスコープ外の遷移関数ではなく、チェックリストにある DOM-task-060（`spec/inventory/domain.md:118`）そのもの。汎用 in-memory アダプターは無く、`pulsen-conformance/src/doubles/` にあるのは add の分岐網羅に必要な6つのスクリプト式ダブルだけ（`ScriptedConfigStore` は意図的に不在で、`doubles/stores.rs:3-5` に理由が書かれている）。逆に、スコープ内で抜けている項目も見つからなかった。

### コメントの質

`crates/` 全体を「修正の経緯・弁明・自明な言い換え」の語彙で走査したが、該当は0件だった。残っているコメントはドキュメンテーションと why / why not に限られており、しかも why の質が高い — 例として `adapter/task_repository.rs:51-52`（破損ファイルを上書きしない理由）、`util/atomic.rs:58-61`（`sync_dir` の失敗を伝えない理由）、`adapter/task_id.rs:67-68`（到達しない分岐でパニックさせない理由）、`task/mod.rs:16-17`（`module_inception` を許す理由）、`definition/template.rs:198`（`unreachable!` を不変条件で裏づける理由）。CLAUDE.md の基準に照らして指摘なし。

### モジュール構成・公開面

ドメインの各モジュールは `mod` を非公開にして `pub use` で再輸出する形に統一されており、公開される型が一望できる。`WorkflowSnapshot::from_validated` が `pub(crate)`（`definition/snapshot.rs:24`）で `rehydrate` だけが公開されているのは、生成経路を型で制御する良い例。`crates/pulsen` 側は `lib.rs` で `adapter` / `application` / `cli` / `util` をすべて `pub` にしているが、これは `tests/` と `examples/lock_holder.rs` が実体を必要とするためで、バイナリクレートの lib 面としては許容範囲。指摘なし。

---

### カバレッジ

- 確認: `.adr/019-domain-crate-workspace.md`, `.adr/020-no-serde-in-domain-timestamp-conversion-in-domain.md`, `.adr/021-yaml-value-then-hand-written-schema-walk.md`, `.adr/022-std-file-lock-and-lockguard-marker-trait.md`, `.adr/023-dependency-selection.md`, `.adr/024-git-cli-shell-out-and-target-classification.md`, `.adr/025-task-file-json-and-corrupt-classification.md`, `.adr/026-task-id-format.md`, `.adr/027-port-conformance-suite-and-harness-hooks.md`, `.adr/028-usecase-error-paths-via-test-doubles.md`, `.adr/029-wildcard-enum-match-arm-lint-domain-only.md`, `.adr/030-workflow-store-base-dir-injection.md`, `.adr/031-pulsen-home-layout-in-application-layer.md`, `.adr/032-lock-holder-example-fixture.md`, `.adr/033-git-fixture-reproducibility.md`, `.adr/034-workflow-ref-separator-set-as-constant.md`, `.adr/035-file-slice-adrs-from-019.md`, `.adr/036-infallible-ports-absorb-failure-at-construction.md`, `.adr/037-platform-separator-set-without-cfg.md`, `.adr/039-read-error-shared-by-find-and-list.md`, `.adr/040-rehydrate-takes-field-bundle.md`, `.adr/042-absent-yaml-value-is-omission.md`, `.adr/044-task-file-layout-in-domain.md`, `.adr/048-parse-inputs-at-spec-flow-position.md`, `.adr/050-schema-error-location-is-logical.md`, `.thread/1/adr.md`, `.thread/1/plan.md`, `.thread/1/progress.md`, `.thread/1/steps.md`, `.thread/1/testing.md`, `Cargo.toml`, `crates/pulsen-conformance/Cargo.toml`, `crates/pulsen-conformance/HOOKS.md`, `crates/pulsen-conformance/src/clock.rs`, `crates/pulsen-conformance/src/config_store.rs`, `crates/pulsen-conformance/src/doubles/clock.rs`, `crates/pulsen-conformance/src/doubles/lock.rs`, `crates/pulsen-conformance/src/doubles/mod.rs`, `crates/pulsen-conformance/src/doubles/stores.rs`, `crates/pulsen-conformance/src/doubles/task_id.rs`, `crates/pulsen-conformance/src/doubles/task_repository.rs`, `crates/pulsen-conformance/src/doubles/tests.rs`, `crates/pulsen-conformance/src/doubles/worktree.rs`, `crates/pulsen-conformance/src/exclusive_lock.rs`, `crates/pulsen-conformance/src/lib.rs`, `crates/pulsen-conformance/src/task_id_generator.rs`, `crates/pulsen-conformance/src/task_repository.rs`, `crates/pulsen-conformance/src/workflow_store.rs`, `crates/pulsen-conformance/src/worktree_manager.rs`, `crates/pulsen-domain/Cargo.toml`, `crates/pulsen-domain/src/definition/agent.rs`, `crates/pulsen-domain/src/definition/assembler.rs`, `crates/pulsen-domain/src/definition/command.rs`, `crates/pulsen-domain/src/definition/config.rs`, `crates/pulsen-domain/src/definition/duration.rs`, `crates/pulsen-domain/src/definition/mod.rs`, `crates/pulsen-domain/src/definition/name.rs`, `crates/pulsen-domain/src/definition/port.rs`, `crates/pulsen-domain/src/definition/reference.rs`, `crates/pulsen-domain/src/definition/snapshot.rs`, `crates/pulsen-domain/src/definition/template.rs`, `crates/pulsen-domain/src/definition/validator.rs`, `crates/pulsen-domain/src/definition/workflow.rs`, `crates/pulsen-domain/src/execution/mod.rs`, `crates/pulsen-domain/src/execution/port.rs`, `crates/pulsen-domain/src/lib.rs`, `crates/pulsen-domain/src/task/attempt.rs`, `crates/pulsen-domain/src/task/branch.rs`, `crates/pulsen-domain/src/task/counters.rs`, `crates/pulsen-domain/src/task/degraded.rs`, `crates/pulsen-domain/src/task/failure.rs`, `crates/pulsen-domain/src/task/id.rs`, `crates/pulsen-domain/src/task/mod.rs`, `crates/pulsen-domain/src/task/path.rs`, `crates/pulsen-domain/src/task/port.rs`, `crates/pulsen-domain/src/task/process.rs`, `crates/pulsen-domain/src/task/state.rs`, `crates/pulsen-domain/src/task/task.rs`, `crates/pulsen-domain/src/task/time.rs`, `crates/pulsen/Cargo.toml`, `crates/pulsen/examples/lock_holder.rs`, `crates/pulsen/src/adapter/clock.rs`, `crates/pulsen/src/adapter/config_store.rs`, `crates/pulsen/src/adapter/lock.rs`, `crates/pulsen/src/adapter/mod.rs`, `crates/pulsen/src/adapter/task_file.rs`, `crates/pulsen/src/adapter/task_id.rs`, `crates/pulsen/src/adapter/task_repository.rs`, `crates/pulsen/src/adapter/workflow_store.rs`, `crates/pulsen/src/adapter/worktree.rs`, `crates/pulsen/src/adapter/yaml.rs`, `crates/pulsen/src/application/home.rs`, `crates/pulsen/src/application/mod.rs`, `crates/pulsen/src/application/register_task.rs`, `crates/pulsen/src/cli/add.rs`, `crates/pulsen/src/cli/args.rs`, `crates/pulsen/src/cli/exit.rs`, `crates/pulsen/src/cli/mod.rs`, `crates/pulsen/src/cli/render.rs`, `crates/pulsen/src/cli/wire.rs`, `crates/pulsen/src/lib.rs`, `crates/pulsen/src/main.rs`, `crates/pulsen/src/util/atomic.rs`, `crates/pulsen/src/util/fsdir.rs`, `crates/pulsen/src/util/mod.rs`, `crates/pulsen/tests/cli_add_boundary.rs`, `crates/pulsen/tests/cli_add_error.rs`, `crates/pulsen/tests/cli_add_normal.rs`, `crates/pulsen/tests/common/git.rs`, `crates/pulsen/tests/common/lock.rs`, `crates/pulsen/tests/common/mod.rs`, `crates/pulsen/tests/conformance_config_store.rs`, `crates/pulsen/tests/conformance_lock.rs`, `crates/pulsen/tests/conformance_task_repository.rs`, `crates/pulsen/tests/conformance_time_id.rs`, `crates/pulsen/tests/conformance_workflow_store.rs`, `crates/pulsen/tests/conformance_worktree.rs`, `crates/pulsen/tests/register_task.rs`, `flake.nix`, `rustfmt.toml`（120件）
- スキップ: `Cargo.lock` — cargo が生成するロックファイルで、選定の判断そのものは `Cargo.toml` と `.adr/023` 側で確認済みのため（1件）

合計 121件（一覧と一致）。
