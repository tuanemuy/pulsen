# 型設計・アーキテクチャ整合

## 前提

- 基準は `CLAUDE.md`（関数型ドメインモデリング / ヘキサゴナル）と `.thread/13/plan.md` の AC-1〜AC-13、および `.thread/13/adr.md`。
- 変更はテストのフィクスチャ層に閉じており、ドメイン・ポート・アダプター・ユースケースへの波及は無い（`git diff --stat` の9ファイルが `crates/pulsen/tests/` と文書・CI のみ）。`crates/pulsen-conformance/src/` は無変更で、plan.md「含まれないもの」の線を越えていない。
- `cargo clippy --workspace --all-targets --locked -- -D warnings` と `cargo fmt --all --check` はローカルで通ることを確認した（AC-11）。`SIGNAL_DEADLINE` は `Duration::from_secs(10)` のままで差分に無い（AC-7）。

## Blockers

なし。

4値の振り分け・`None` の一意化・`match` の網羅・probe の置き場所は、いずれも plan.md / adr.md が定めた形と一致しており、依存の向き（スイートが適用先を知らない）も崩れていない。以下はすべて改善提案の水準。

## Warnings

- **[W-001]** `recv_timeout` の `Err(_)` が `Timeout` と `Disconnected` を潰しており、しかも潰した先が**許容集合に入る側**である
  - 場所: `crates/pulsen/tests/common/lock.rs:121`
  - 理由: `RecvTimeoutError` は `Timeout` / `Disconnected` の2値で、`Disconnected`（読み取りスレッドが送信せずに消えた）は「合図が期限内に返らなかった」ではない。ここが `Started::SignalTimedOut` に倒れると `probe_holder` は `HolderCapability::SignalTimedOut` を返し、5件が黙って許容集合に入る。本Issueが直しているのはまさに「環境の能力でないものが宣言済みスキップに化ける」ことであり、新しく作った判断経路に同じ形の潰しが1つだけ残っている。`CLAUDE.md`「分岐は網羅する。`match` でワイルドカード（`_`）を避ける」に対する、この差分で唯一の例外でもある。`Started` の doc（44行）が「合図も終了も期限内に返らなかった」と書いていることとも食い違う。
  - 提案: `Err(RecvTimeoutError::Timeout)` と `Err(RecvTimeoutError::Disconnected)` を分け、後者は `Started::SignalUnreadable`（＝失敗側）へ寄せる。実際に起きる確率は極めて低い（クロージャは `String::new` / `read_line` / `send` しか踏まない）が、倒れる先が許容側である以上、確率ではなく向きで決めるべき箇所。

- **[W-002]** 合図の読み取りエラーの `io::Error` が `.ok()` で捨てられており、`SignalUnreadable` が理由を運ばない
  - 場所: `crates/pulsen/tests/common/lock.rs:112`（`.ok()`）、`:43`（`SignalUnreadable(Child)`）、`:147`（パニック文言）
  - 理由: adr.md ADR-007 は `spawn` について「現行は `io::Error` を `.ok()?` で捨てているため、診断に要る情報がどこにも出ない」ことを欠陥として名指しし、`SpawnFailed(io::Error)` を作って直した。同じ関数の中で、読み取り側は `read_line(...).ok()` のまま `io::Error` を捨てており、パニックは `保持プロセスの合図を読み取れなかった` という固定文言だけになる。`stderr` は `Stdio::null()` なので、この経路でも理由が出る場所は他に無い（ADR-007 が `SpawnFailed` について述べた事情と同一）。原因の違うもの（`InvalidData` = 非UTF-8 の出力 / `BrokenPipe` / `PermissionDenied` 等）が1つの顔で出る。
  - 提案: チャネルで `io::Result<String>` をそのまま送り、`SignalUnreadable { holder: Child, error: io::Error }` として理由をパニック文言に載せる。`SpawnFailed` と同じ形になり、2つの失敗経路の扱いが揃う。

- **[W-003]** `HolderCapability::Available(PathBuf)` が実行ファイルのパスを公開しており、AC-2 の「コンパイラが保証する」が型のレベルでは成り立っていない
  - 場所: `crates/pulsen/tests/common/lock.rs:29`（`pub enum` の変種）
  - 理由: AC-2 は `holder_program()` を private にすることで「古い述語を直接見る呼び出し側が残らないことをコンパイラに保証させる」としている。関数は確かに private になった（`grep` で `lock.rs` 外の参照は0件）が、`pub fn holder_capability() -> &'static HolderCapability` が `Available(PathBuf)` としてパスをそのまま公開しているため、別のテストバイナリが `if let Available(program) = holder_capability()` で実行ファイルを取り出し、`spawn_holder` のパニック判断（`ProgramMissing` / `ProgramUnusable` / probe 後タイムアウト）を迂回して起動できる。塞いだ穴と同じ形の穴が、変種のペイロードとして残っている。
  - 提案: 能力の型は「宣言のための答え」だけを持たせ（`Available` を単位変種にする）、解決済みパスは `lock.rs` 内の別の private な `OnceLock<PathBuf>` か、`probe_holder` が返す private な内部型に持たせる。`allowed_skips()` 側の `match` も `Available` と書けて読みやすくなる。

- **[W-004]** `ProgramUnusable(String)` の早すぎる文字列化。根拠として挙がっている理由が、この型には当てはまらない
  - 場所: `crates/pulsen/tests/common/lock.rs:35`（定義）、`:88`（`error.to_string()`）
  - 理由: `.thread/13/testing.md:215` は文字列化の理由を「`Child` を持てない `'static` の型に、理由だけを文字列で移す」と説明しているが、持てないのは `Child` であって `io::Error` ではない。`std::io::Error` は `Send + Sync + 'static` なので `static OnceLock<HolderCapability>` にそのまま置ける。文字列にした時点で `ErrorKind` が失われ、後から「`PermissionDenied` は環境の能力側へ倒す」といった述語を書く材料が無くなる（ADR-002 が「区別が増えたときも同じ問いで判断できる」と言っている、その材料）。`CLAUDE.md`「プリミティブは意味のある型でラップする」「エラーは値として返す」の向きとも逆。
  - 提案: `ProgramUnusable(io::Error)` にする。パニック文言は `{error}` でそのまま同じ内容になり、`Display` は `to_string()` と一致するので観測できる文言は変わらない。

- **[W-005]** 「適合スイートと CLI 側で扱いが揃う」を保証しているのは probe の共有までで、振り分けの方針は2箇所に複製されている
  - 場所: `crates/pulsen/tests/conformance_lock.rs:106-113` と `crates/pulsen/tests/common/mod.rs:46-51`
  - 理由: `conformance_lock.rs:104` の doc は「同じ判定を CLI 側の受け入れテスト（TC-task-register-task-017）も使うため、両者で扱いが揃う（ADR-055）」と書くが、共有されているのは `holder_capability()`（観測）だけで、「`SignalTimedOut` だけを許容側に置く」という**方針**は同じ形の `match` が2つある。片方だけ `ProgramUnusable` を許容側に倒しても両方コンパイルが通り、`.thread/13/testing.md:121` が「片方だけ倒すと扱いが割れる」と警告しているとおりの割れ方が構造的には防げていない。ADR-004 の「判断が `lock.rs` の1箇所に集まる」も、`None`/パニックの判断については成立しているが、宣言の判断については成立していない。
  - 提案: `lock.rs` に `pub fn holder_signal_times_out() -> bool` のような述語を1つ置き（その中に4区別の網羅 `match` を1つだけ持つ）、2つの `allowed_skips()` はケース一覧の違いだけを持つ形にする。新しい変種が増えたときにコンパイルエラーで判断を要求される点は変わらず（`match` は1箇所に残る）、方針が割れる余地だけが消える。

- **[W-006]** `try_acquire_from_other_process` だけ、取得できていないプロセスに対して期限の無い `release` を掛けている
  - 場所: `crates/pulsen/tests/conformance_lock.rs:79`
  - 理由: adr.md ADR-005 は「期限の無い待ちは、正常に保持できたと分かっている相手にだけ許す」を**1本の基準**として掲げ、`release` を使うのは正常に保持できたプロセスを畳むときだけ、と決めた。実装は `probe_holder` / `start_holder` / `spawn_holder` / `hold` ではその基準どおりだが、`try_acquire_from_other_process` は `locked` の値によらず `release(holder)`（stdin を閉じて期限なしに `wait`）を呼ぶ。`locked == false` は「保持できなかったプロセス」であり、基準の適用外にいる唯一の呼び出し。ADR-005 の本文は適用範囲を `spawn_holder` / `hold` と書いているので契約違反ではないが、「1本の基準」という言い方と実装が食い違う。
  - 提案: 実害は小さい（`lock_holder` は取得に失敗すれば即終了するので `wait` は返る）ため、(a) `kill_and_wait` を `pub(crate)` に上げて `if locked { release } else { kill_and_wait }` に揃えるか、(b) ADR-005 の「1本の基準」の射程を `spawn_holder` / `hold` に明示的に限る、のどちらかで文言と実装を一致させる。

- **[W-007]** 「起動できなかった」の2つのパニック文言が同一で、probe 時に観測した理由か今回の起動の理由かを読み分けられない
  - 場所: `crates/pulsen/tests/common/lock.rs:140` と `:154`
  - 理由: 前者は probe が別パス（`tempfile` の一時ディレクトリ）に対して観測し、`OnceLock` に凍結した理由の再生であり、後者は本番のロックパスに対する今この場の失敗。文言が一字一句同じなので、失敗を読む人はどちらか判別できない。AC-5 が probe 成立後のタイムアウトに「probe は同じ手順で成立している」という一句を要求したのと同じ理由が、この対にも当てはまる（区別を型で作った以上、その区別が出力にも現れてほしい）。
  - 提案: `ProgramUnusable` 側に「（probe の起動時に観測した理由）」の一句を足す。文言の追加だけで、型の形は変えなくてよい。

- **[W-008]** `Started::Signaled` の doc「合図が期限内に返った」が、`locked: false` の実態（合図を書かずに終了した＝EOF）を述べていない
  - 場所: `crates/pulsen/tests/common/lock.rs:40-41`
  - 理由: `examples/lock_holder.rs` は取得に成功したときだけ標準出力に `locked` を書き、失敗時は標準エラーに書いて即終了する。したがって `read_line` が `Ok(0)`（EOF）を返す経路が `signal == ""` として `Signaled { locked: false }` になり、到達しうる `locked == false` はすべて「何も返らずに子が終了した」である。ADR-007 は `!locked` の**パニック文言**については観測に忠実であること（「取得した合図を返さなかった」）を強く求め、実装もそのとおりになっている（`lock.rs:168`）のに、型のドキュメント側は「合図が期限内に返った」「ロックを取得できたかを添える」と、観測していないことを述べている。名前の付いた区別を増やした差分なので、名前と doc が観測とずれると次の読者が同じ潰し方を繰り返す。
  - 提案: 変種の doc を「期限内に子が応答した（合図を書いたか、書かずに終了したか）。`locked` は合図が `LOCKED` だったか」に改める。型を割る必要は無い — `locked == false` は TC-004 が自分で判定すべき観測であり、フィクスチャが先取りしない現在の形（steps.md 236行）は正しい。

## 観点別の確認結果

### `HolderCapability` / `Started` の4値

- `HolderCapability` の4値は許容側1（`SignalTimedOut`）／失敗側2（`ProgramMissing` / `ProgramUnusable`）／該当なし1（`Available`）に分かれ、ADR-002 の基準（スキップの宣言だけで「なぜ走らなかったか」と「次に何をすればよいか」が定まるか）で一意に振り分けられる。重複は無い。
- `ProgramMissing` を返すのは `holder_program()` が `None` のときだけで（`lock.rs:75-77`）、ADR-007 の「起動の失敗を `ProgramMissing` に寄せない」が守られている。
- `Started` の4値と `HolderCapability` の4値は1対1ではない（`Signaled` / `SignalUnreadable` → `Available`）。これは ADR-005 の判定基準（合図が期限内に返ったかだけを見る）から導かれる意図的な畳み方で、畳んだ先が失敗側でないことは probe が測っている範囲と一致する。
- 欠落として指摘できるのは W-001（`Disconnected`）と W-008（EOF）の2点で、いずれも新しい変種を足すべきかは別の判断。前者は許容側に落ちるので向きを直す価値がある。
- 変種名は `clippy::enum_variant_names` に掛からない（`Signaled` / `Signal*` / `Spawn*` で先頭語が全一致しないため）。`cargo clippy --all-targets -- -D warnings` が通ることで実証済み（AC-11）。

### `None` の一意性（AC-6）

`spawn_holder` で `None` を作るのは `HolderCapability::SignalTimedOut` の腕1箇所だけ（`lock.rs:137`）。`hold` は `?` の伝播だけで `None` を作り、`!locked` はパニックに移った。`hold_from_other_process` は `hold` に委ねられ、`!locked` の判断は `lock.rs` の1箇所に集まっている。`grep` の限りでも `lock.rs` に他の `None` 生成は無い。AC-6 は満たされている。

### パニックの妥当性

`CLAUDE.md` の「エラーは値として返す／パニックは不変条件違反にのみ」はドメイン層の規約で、ここはテストのフィクスチャ（`expect("一時ホームを作れる")` が既に並ぶ層）。ADR-004 の判断（`Option` は「宣言済みスキップか」だけを運び、失敗はパニック）は妥当で、フックの `Option` 契約（`crates/pulsen-conformance/src/lib.rs`）を変えずに区別を成立させる唯一の形になっている。パニックの5経路（`ProgramMissing` / `ProgramUnusable` / `SignalUnreadable` / probe 後 `SignalTimedOut` / `!locked`）はいずれも「フィクスチャの前提が破れた」であり、`Result` に載せ替えるべきものは無い。逆に `Result` にすべきものがパニックになっている箇所も無い。

`probe_holder` の `tempfile::tempdir().expect(...)`（`lock.rs:79`）は `OnceLock::get_or_init` の中にあるため、パニックすると `common/mod.rs` の `SKIPS`（`LazyLock`）が毒され、以降の `skipped()` が別の理由で落ちる。ただし一時ディレクトリを作れない環境では他の大半のフィクスチャも同じ `expect` で落ちるので、ここだけ扱いを変える理由は無い。指摘としては挙げない。

### `match` の網羅性

`allowed_skips()` は2箇所とも `_` を使わず4変種を列挙している（AC-3 を満たす）。`spawn_holder` の2つの `match` も同様。差分の中で `_` が現れるのは `Started::Signaled { holder, locked: _ }`（`lock.rs:83`、ADR-005 が明示的に捨てると決めた値のフィールド束縛）と、W-001 の `Err(_)` の2箇所。前者は網羅の放棄ではないので問題無い。

### `.thread/13/adr.md` とコードの一致

| ADR | 決定 | 実装 |
|---|---|---|
| 001 | `OnceLock` で1度だけ評価する probe を骨に、宣言と挙動の双方がそれを見る | 一致（`lock.rs:69-72`、`allowed_skips()` 2箇所） |
| 002 | 許容集合に入れるのは `SignalTimedOut` だけ、`ProgramMissing` / `ProgramUnusable` は失敗側 | 一致 |
| 003 | probe は `tests/common/lock.rs`、HOOKS.md の判定列はフック水準の主語を保つ | 一致（HOOKS.md 43行は `hold_from_other_process` / `try_acquire_from_other_process` を主語に保ち、適用先の実態は括弧の補足。`SIGNAL_DEADLINE` や `holder_capability` の名前は表に出ていない） |
| 004 | `None` は「宣言済みスキップか」だけ、失敗はパニック、`hold` の `!locked` もパニック、`holder_program()` は private | 一致 |
| 005 | probe の判定は合図の期限内到達だけ、後始末は `kill_and_wait` | `lock.rs` 内は一致。`try_acquire_from_other_process` だけ基準の外（W-006） |
| 006 | probe 成立後のタイムアウトは失敗 | 一致（`lock.rs:149-152`、文言も「probe は同じ手順で成立している」を含む） |
| 007 | `ProgramUnusable` / `SignalUnreadable` を独立させる、`stdout` 取得失敗は `SpawnFailed`、文言は観測に忠実 | 概ね一致。`spawn` の `io::Error` は保存されるが（AC-4 のコードレビュー分は満たされている: `lock.rs:88` → `:140` と `:153-155` で `{reason}` / `{error}` が文言に載る）、読み取りエラーの `io::Error` は同じ理由が当てはまるのに捨てられている（W-002） |

### 型の公開範囲

`HolderCapability` だけが `pub`、`Started` / `holder_program` / `probe_holder` / `start_holder` / `kill_and_wait` は private。steps.md 24行の方針（公開するのは能力の型だけ）どおりで、`Child` を持つ型が `'static` の外へ漏れていない。唯一の緩みが `Available` のペイロード（W-003）。

### Rust の慣用

- `OnceLock<HolderCapability>` に置く値は `PathBuf` / `String` のみで `Send + Sync`、`&'static` を返す形も自然。`static` を関数の中に閉じているのも良い。
- `PROGRAM_MISSING` の `\` による行継続は次行の先頭空白ごと畳まれるので、余計な空白は混ざらない。
- `Started` を返す `start_holder` を `probe_holder` と `spawn_holder` が共有する形で、起動の手順が1箇所に集まっている。probe と本番で手順が食い違わないという ADR-001 の主張が、コードの構造で担保されている。
- `kill_and_wait` が `Child` を値で取って結果を捨てるのは、「残さないこと」だけが目的である以上妥当。
- 改善余地は W-004（早すぎる `String` 化）とW-001（`Err(_)`）。

## AC の充足（この観点に関わるもの）

- AC-1 充足 / AC-2 充足（`holder_program` は `lock.rs` 外から参照不能。ただし W-003）/ AC-3 充足 / AC-4 コードレビュー分は充足（`io::Error` が文言に載る。ただし W-007）/ AC-5 充足 / AC-6 充足 / AC-7 充足 / AC-8 のうち ADR-003 由来の縛り（適用先固有の名前を表の主語にしない）は充足 / AC-11 ローカルで充足 / AC-13 コードの実挙動と一致（`--test` の綴りは ci.yml に0件のまま）。
- AC-9 / AC-10 / AC-12 は実地検証と片付けフェーズの担当（steps.md ステップ8〜10）。`.adr/073-*.md` は未作成で、`lock.rs:21` と `conformance_lock.rs:104` の doc コメントも現時点では ADR-068 止まりだが、これは steps.md がレビュー後の最終ステップに置いているためで、この時点での欠落として数えない。**マージ前にステップ10（`.adr/073` の起票、doc コメントからの導線、`.thread/13/adr.md` の Status 更新）が残っていることだけ、完了判定の際に落とさないこと。**

## カバレッジ

- 確認: `crates/pulsen/tests/common/lock.rs`, `crates/pulsen/tests/common/mod.rs`, `crates/pulsen/tests/conformance_lock.rs`, `crates/pulsen-conformance/HOOKS.md`, `.thread/13/plan.md`, `.thread/13/adr.md`, `.thread/13/steps.md`, `.github/workflows/ci.yml`
- 確認（部分）: `.thread/13/testing.md` — 確認項目3・4・9（`None` の一意性 / 許容集合の `match` / `ProgramUnusable` のコードレビュー分）だけを型設計の観点で照合した。実地手順の妥当性そのものは検証観点のレビューに委ねる。
- スキップ: なし
