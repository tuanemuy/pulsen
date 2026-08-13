# 型設計・アーキテクチャ整合

2周目。1周目の指摘（`review-001-type-design.md`）と `triage.md` の判定は前提として扱い、`wont-fix` / `defer`（W-005 / W-011 / W-015 / W-016 / W-017）は再指摘しない。1周目の `fix` 分（`Err(_)` の分割 / `SignalUnreadable { holder, error }` / `Available(HolderProgram)` / `ProgramUnusable(io::Error)` / 文言 / ADR-073 の起票）はいずれも `fix-plan-001.md` の形どおり入っており、下記はその**修正が新たに作った境界**と、修正の射程外に残った箇所についての指摘に限る。

実測: `cargo clippy --workspace --all-targets --locked -- -D warnings` / `cargo fmt --all --check` はローカルで通る（AC-11）。`SIGNAL_DEADLINE` は差分では doc しか変わっておらず、値は `Duration::from_secs(10)` のまま（AC-7）。`crates/pulsen-conformance/src/` は無変更で、スイートが適用先を知らない依存の向きは崩れていない。

## Blockers

なし。

4区別の振り分け、`None` の一意化（AC-6）、`match` の網羅（AC-3）、`holder_program()` と実行ファイルパスの封じ込め（AC-2）、probe の置き場所（ADR-003 / ADR-073）は、いずれも plan.md / `.adr/073` が定めた形と一致している。`HolderProgram` のカプセル化は実効的で、タプルフィールドも `path()` も `lock.rs` の外からは触れず、`HolderCapability::Available` を外部で構築する手段も無い（`HolderProgram` を作れないため）。以下はすべて改善提案の水準。

## Warnings

- **[W-001]** `Disconnected` を失敗側へ寄せた1周目の修正が、probe では `Available`（＝能力を名乗る側）に合流している
  - 場所: `crates/pulsen/tests/common/lock.rs:141-145` と `:96-100`（`Started::Signaled | Started::SignalUnreadable` の腕）
  - 理由: `Disconnected` の直前のコメントは「期限を1ミリ秒も測っていないので、環境の能力(＝スキップを許容する側)には倒さない」と述べるが、寄せ先の `SignalUnreadable` は `probe_holder` で `HolderCapability::Available` に畳まれる。`Available` の doc（`:28`）は「保持プロセスを起動でき、合図が期限内に返る」、`.adr/073`「決定」は「probe の判定基準は『合図が期限内に返ったか』だけに限る」であり、どちらもこの経路では観測されていない。`Started::SignalUnreadable` の doc（`:55`）「期限内に応答はあったが、合図を読み取れなかった」も、送信側スレッドが消えた場合には当てはまらない。`steps.md:177` が `SignalUnreadable` を `Available` に数える根拠として挙げる「合図が期限内に『返った』ことは確か」も、読み取りエラー（子が何かを返した）にしか掛からない。倒れる向きは失敗側（後続ケースは `spawn_holder` でパニックする）なので静かな緑にはならないが、「観測していないことを型が名乗らない」という本Issueの主題からは外れている。
  - 提案: `Disconnected` は「子の応答」ではなく**フィクスチャ自身の不変条件の破れ**（送信側スレッドが `send` に到達せず消えた）なので、`SignalUnreadable` に合成の `io::Error` を載せて流すより、`kill_and_wait` のうえその場でパニックさせるほうが観測に忠実になる（`CLAUDE.md`「パニックは不変条件違反にのみ」に照らしても、ここは正当な使い所）。型を増やさずに済ませるなら、少なくとも `SignalUnreadable` の doc と probe の腕のコメントに「読み取りスレッドが消えた場合を含み、その場合は応答を観測していない」ことを書き、`Available` を名乗る根拠が経路によって違うことを隠さない。

- **[W-002]** 合成した `io::Error` が `ProgramUnusable` に載り、`ErrorKind` を材料として残すという 073 の理由づけを裏側から薄めている
  - 場所: `crates/pulsen/tests/common/lock.rs:119-122`（`io::Error::other("保持プロセスの標準出力を取得できない")` → `SpawnFailed`）、`:102`（`SpawnFailed` → `ProgramUnusable`）、`:34-35`・`:162-165`（doc と文言）
  - 理由: `.adr/073`「決定」は「エラーを早すぎる段階で文字列に畳まないのは、後から『この `ErrorKind` は能力側へ倒す』という述語を書く材料を残すため」と述べ、`ProgramUnusable` が `io::Error` を持つ理由をそこに置いている。しかし `stdout` の取得失敗はその材料を持たない合成エラー（`ErrorKind::Other`）として同じ変種に入り、`ProgramUnusable` の doc「実行ファイルはあるが起動できない」と文言「起動できなかった」を、**子の起動には成功した**経路が名乗ることになる。ADR-007 が「パニック文言は観測に忠実にする」として `!locked` の文言を絞ったのと同じ基準が、この経路にだけ掛かっていない。将来 `ErrorKind` で述語を書くとき、`Other` の中に「起動失敗ではないもの」が混じっている点も効く。
  - 提案: 到達確率は極小（`Stdio::piped()` を指定した直後の `Child::stdout` は `Some`）なので型を増やす価値は薄い。`Started::SpawnFailed` の doc に「`stdout` を取得できなかった場合を含む」と1行足し、`ProgramUnusable` の doc・パニック文言を「起動できない（起動しても合図の経路を用意できない場合を含む）」の水準に合わせるだけでも、名前と観測のずれは消える。ADR-007 の判断（同じ経路に寄せる）自体を覆す必要は無い。

- **[W-003]** `common/mod.rs` 側の宣言に、振り分けの理由も `.adr/073` への導線も無い
  - 場所: `crates/pulsen/tests/common/mod.rs:37-56`（`conformance_lock.rs:100-113` と対比）
  - 理由: 述語を1本に畳まず2箇所の `match` を残すのは、変種が増えたときに**両方の宣言側でコンパイラに判断を迫るため**という理由で `triage.md` が W-005 を wont-fix にしている。その形が機能するには、迫られた側がその場で判断できる必要があるが、`conformance_lock.rs` の `allowed_skips()` が基準と典拠（HOOKS.md / ADR-068 / ADR-073）を doc で持つのに対し、`common/mod.rs` の `allowed_skips()` の doc は「前提を作れるかどうかを実際に調べて、許容するケースを決める」だけで、`ProgramMissing` / `ProgramUnusable` を失敗側に置く理由がどこからも読めない。5番目の変種を足した人がこちらのファイルだけ開いた場合、複製を残した意図（両側で判断させる）が空振りする。
  - 提案: `common/mod.rs` の `match` の直上に、基準（スキップの宣言だけで次の一手が定まるものだけを許容する）と `.adr/073` への参照を1〜2行置く。`conformance_lock.rs` の doc を複製する必要は無く、参照だけで足りる。

- **[W-004]** 同じ `impl` の中で、`kill_holder` の `None` だけが異常をスキップ経路へ流す旧い形のまま残っている
  - 場所: `crates/pulsen/tests/conformance_lock.rs:66-71`（差分外・既存コード）
  - 理由: このPRは `hold_from_other_process` を `lock::hold` に寄せ、`Option` の `None` を「宣言済みスキップ」1つの意味に絞った。一方 `kill_holder` は `holder.kill().ok()?` / `holder.wait().ok()?` で、**強制終了に失敗した**という異常を `None` に畳み、スイート側の `require!`（`crates/pulsen-conformance/src/exclusive_lock.rs:156`）でスキップとして現れる。`Available` と判定された環境でそれが起きると、`tc_port_exclusive_lock_005` が「ハーネスが提供しない」というフック水準の理由でスキップされ、許容集合に無いので `SkipBudget` 違反として落ちる — 赤にはなるが、原因の表示は本Issueが直したのと同じ取り違え方をする。AC-6 が射程に入れていないので契約違反ではないが、`None` の意味を揃えたこの impl の中で唯一残った例外である。
  - 提案: スコープを広げないなら、`kill_holder` の `None` が「別プロセスを扱えない実装」ではなく `kill` / `wait` の失敗であることをコメントで明示する。揃えるなら `lock.rs` の失敗経路と同じくパニックにし、`Option` は `Some(())` 固定にする（`release_holder` は TC-002 / 003 が `assert!(...is_some())` で受けているのでスキップ経路には流れず、こちらは現状のままでよい）。

- **[W-005]** `.thread/13/adr.md` の ADR-007 が、実装と違う型の綴りのまま残っている
  - 場所: `.thread/13/adr.md:211`（`SignalUnreadable(Child)`）／実装は `crates/pulsen/tests/common/lock.rs:56` の `SignalUnreadable { holder: Child, error: io::Error }`
  - 理由: 1周目 W-002 の修正で読み取りエラーの `io::Error` を運ぶ形に変わり、`fix-plan-001.md` 単位3-5 は `ProgramUnusable(String)` の綴りだけを現在の形に合わせるよう指示していたため、同じ ADR 内のこの1行が取り残されている。ADR-007 は昇格しない作業ログだが、`.adr/073` が `ProgramUnusable` を失敗側に置く理由の出所として名指ししている文書であり、`Status` 行も「波及先は `lock.rs` とそのパニック文言に留まる」と述べている。決定を読んだ人が実装の形を誤って写す余地が残る。
  - 提案: `SignalUnreadable { holder, error }` に直し、「合図の読み取りが返した `io::Error` を運ぶ」ことを1文添える（1周目 W-002 の修正結果と、ADR-007 の「診断に要る情報がどこにも出ない」という問題提起が同じ形で閉じる）。

- **[W-006]** 同じモジュールの項目を `use` とフルパスで混在させている（体裁）
  - 場所: `crates/pulsen/tests/conformance_lock.rs:9`（`use common::lock::{HolderCapability, release, spawn_holder};`）と `:63`・`:108`（`common::lock::hold` / `common::lock::holder_capability`）
  - 理由: `hold` と `spawn_holder` は同じフィクスチャの対になる入口で、`holder_capability()` はその判断の源であるにもかかわらず、片方だけ import されているため、読み手には由来が違うものに見える。実害は無いが、このPRが「判断の源は1点」であることを型で示した直後のファイルなので、綴りの水準でも揃えておくほうが読める。
  - 提案: `use common::lock::{HolderCapability, hold, holder_capability, release, spawn_holder};` に寄せるか、逆に全てフルパスにする。どちらでもよいが混在は避ける。

## 観点別の確認結果

### 1周目の修正が型として機能しているか

- **`HolderProgram`（W-003 の修正）:** カプセル化は実効的。`struct HolderProgram(PathBuf)` のフィールドと `path()` はともに `lock` モジュール限りで、`conformance_lock.rs` / `common/mod.rs` からは `Available(_)` としか書けない。`HolderProgram` を構築する手段が外に無いので、`HolderCapability::Available` を偽造して `spawn_holder` の判断を迂回することもできない。AC-2 の「コンパイラが保証する」が変種のペイロードまで掛かっている。
- **`ProgramUnusable(io::Error)`（W-004 の修正）:** `io::Error` は `Send + Sync + 'static` なので `static OnceLock<HolderCapability>` にそのまま置け、`{error}` で `to_string()` と同じ文言が出る。`ErrorKind` が保たれる点も 073 の理由づけどおり — ただし合成エラーの混入について W-002。
- **`SignalUnreadable { holder, error }`（W-002 の修正）:** 名前付きフィールドにしたことで `probe_holder` の or パターン（`Signaled { holder, locked: _ } | SignalUnreadable { holder, error: _ }`）が `..` を使わずに書け、`Signaled` にフィールドが増えたときはこの腕もコンパイルエラーになる。網羅性の性質が落ちていない。
- **`RecvTimeoutError` の2分岐（W-001 の修正）:** `Err(_)` は消え、`Timeout` だけが `SignalTimedOut`（許容側）に届く。境界をワイルドカードが跨ぐ形は解消済み。残る論点は寄せ先の合流（W-001）。
- **`Started` の4変種:** 重複は無い。`Signaled` / `SignalUnreadable` は「子が起動して期限内に何か起きた」の内訳、`SignalTimedOut` は期限超過、`SpawnFailed` は起動段階の失敗で、軸が交差していない。`HolderCapability` との1対1でない対応（`Signaled` / `SignalUnreadable` → `Available`）は ADR-005 の判定基準から導かれる意図的な畳み方。

### `None` の一意性（AC-6）

`lock.rs` で `None` を作るのは `spawn_holder:160`（`SignalTimedOut` の腕）1箇所のみ。`hold` は `?` の伝播だけ、`hold_from_other_process` は `hold` への委譲だけ。`cli_add_error.rs:129` の `let Some(holder) = lock::hold(..) else { skipped(...) }` と `try_acquire_from_other_process:78` の `?` も、同じ1つの意味だけを受けている。`holder_program()` は `lock.rs` 外から参照不能（`grep` で0件）。AC-6 は満たされている。

### パニックの妥当性

パニックは6経路（`ProgramMissing` / `ProgramUnusable` / `SignalUnreadable` / probe 成立後の `SignalTimedOut` / `SpawnFailed` / `hold` の `!locked`）で、いずれもフィクスチャの前提が破れた場合。`CLAUDE.md` の「エラーは値として返す／パニックは不変条件違反にのみ」はドメイン層の規約で、ここはテストのフィクスチャ（`expect("一時ホームを作れる")` が並ぶ層）であり、ADR-004 の判断（`Option` は「宣言済みスキップか」だけを運ぶ）と整合する。`Result` に載せ替えるべきものも、逆に `Result` にすべきものがパニックになっている箇所も無い。

### `match` の網羅性（AC-3）

`allowed_skips()` の2箇所、`spawn_holder` の2つの `match`、`probe_holder`、`start_holder` の `recv_timeout` — 差分の中に変種のワイルドカード（`_`）は1つも無い。5番目の変種を足せば5箇所すべてがコンパイルエラーになる。フィールド束縛の `locked: _` / `error: _` は網羅の放棄ではなく、`..` を使っていないのでフィールド追加も検知できる。

### `.adr/073` / `.thread/13/adr.md` とコードの一致

| 決定 | 実装 |
|---|---|
| 073「能力側と失敗側を分ける基準」 | 一致（許容側は `SignalTimedOut` のみ、2箇所の宣言で同じ振り分け） |
| 073「能力は probe で1度だけ判定し、宣言と挙動の双方がその1点を見る」 | 一致（`OnceLock`、`allowed_skips()` 2箇所と `spawn_holder`） |
| 073「probe の判定基準は『合図が期限内に返ったか』だけ」 | `locked` を捨てる点は一致。`Disconnected` 経由の合流だけ基準の外（W-001） |
| 073「probe が成立したあとのタイムアウトは失敗」 | 一致（`lock.rs:173-176`、文言も「probe は同じ手順で成立している」を含む） |
| 073「期限の無い待ちは正常に保持できた相手にだけ許す。射程は probe と `lock.rs` の失敗経路」 | 一致（失敗経路はすべて `kill_and_wait`。`try_acquire_from_other_process` の `release` は射程外と 073:47 が明記しており、実装もそのまま） |
| 073「失敗側の2つは失敗メッセージに次の一手の材料を載せる」 | 一致（`PROGRAM_MISSING` に回避方法、`ProgramUnusable` に `{error}`、probe 由来であることの一句付き） |
| 073「宣言側の網羅はワイルドカードで畳まない」 | 一致 |
| 073「probe の置き場所の基準」「HOOKS.md の主語はフック水準」 | 一致（HOOKS.md 43行の判定列・前提列に適用側の関数名／定数名が無い） |
| 073「コードの `PROGRAM_MISSING` と `conformance_lock.rs` の `allowed_skips()` から 073 を辿れる」（AC-12） | 一致（`lock.rs:21` / `conformance_lock.rs:105`）。`common/mod.rs` 側は導線が無い（W-003） |
| adr.md ADR-007「`SignalUnreadable` を独立させる」 | 実装は名前付きフィールド。ADR 本文の綴りが旧いまま（W-005） |
| ci.yml の why コメント | 一致（`ProgramMissing` が失敗側になった帰結と、成立条件・`--workspace` を保つ理由が書かれている。AC-13） |

### Rust の慣用

- `OnceLock<HolderCapability>` に置くのは `PathBuf` / `io::Error` だけで `Send + Sync + 'static`、`&'static` 返しも自然。`static` を関数内に閉じているのも良い。
- `start_holder` を probe と本番で共有する構造により、「probe が本番の手順そのもので判定する」（ADR-001 / 073）がコードの形で担保されている。
- `kill_and_wait(holder: Child)` が値で取って結果を捨てるのは、目的が「残さないこと」だけである以上妥当。失敗経路4箇所すべてがこれを通る。
- タイムアウト経路で読み取りスレッドが残るが、`kill` で `stdout` が EOF になり `send` の失敗を無視して終わるので、リークは有界。
- `common/mod.rs:4` の `#![allow(dead_code)]` は `lock` にも掛かるため、将来 `HolderProgram::path()` が未使用になっても警告は出ない。AC-2 の保証（private 化による**コンパイルエラー**）はこれとは独立に成立しているので、指摘としては挙げない。

## カバレッジ

- 確認: `crates/pulsen/tests/common/lock.rs`, `crates/pulsen/tests/common/mod.rs`, `crates/pulsen/tests/conformance_lock.rs`, `crates/pulsen-conformance/HOOKS.md`, `.github/workflows/ci.yml`, `.adr/073-holder-capability-skip-vs-fail.md`, `.thread/13/adr.md`, `.thread/13/plan.md`, `.thread/13/review/triage.md`, `.thread/13/review/review-001-type-design.md`, `.thread/13/review/fix-plan-001.md`
- 確認（部分）: `.thread/13/steps.md` — ステップ1〜3（型の形・probe・フィクスチャ関数）を実装と照合。手順の運用そのものは検証観点に委ねる
- 確認（部分）: `.thread/13/testing.md` — 確認項目1 / 3 / 4 / 9 / 12（型の形・`None` の一意性・`match` の網羅・`ProgramUnusable` の保持・`kill_and_wait` の呼び出し元）だけを型設計の観点で照合
- スキップ: `.thread/13/review/review-001.md` — このレビューループ自身の成果物（統合サマリー）
- スキップ: `.thread/13/review/review-001-concurrency.md` — 同上（並行性観点の1周目レビュー。判定は `triage.md` で確認済み）
- スキップ: `.thread/13/review/review-001-docs.md` — 同上（ドキュメント観点の1周目レビュー）
- スキップ: `.thread/13/review/review-001-test.md` — 同上（テスト観点の1周目レビュー）
