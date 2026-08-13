# レビュー 001 — 並行性・プロセス管理・堅牢性

対象: PR #14 / ベース `origin/main` / 契約 `.thread/13/plan.md`

## 並行性・プロセス管理・堅牢性

### Blockers

なし。

`OnceLock` の使い方・子プロセスの後始末・ハング経路のいずれにも、この PR が新しく持ち込んだ破綻は見つからなかった。根拠は「実地で確かめたこと」の節に置く。

### Warnings

- **[W-001]** `recv_timeout` の `Err(_)` が `Disconnected` を `Timeout` と同じ「環境の能力」に丸める
  - 場所: `crates/pulsen/tests/common/lock.rs:121`
  - 理由: `recv_timeout` の失敗は `RecvTimeoutError::Timeout` と `RecvTimeoutError::Disconnected` の2つで、後者は**合図読み取りスレッドが送信せずに消えた**ことを意味する(スレッド本体のパニック)。現状はどちらも `Started::SignalTimedOut` になり、probe から見ると `HolderCapability::SignalTimedOut` — すなわち**許容集合が広がる側**へ倒れる。しかも `Disconnected` は期限を待たずに即座に返るので、「10秒待った末の判断」に見えて実際には1ミリ秒も測っていない判定が5件のスキップを黙って正当化する。この PR の主題は「環境の能力(スキップ可)」と「異常(失敗)」を型で分けることなので、残った1つのワイルドカードがちょうどその境界を跨いでいる。実際に起きる確率は低い(読み取りスレッドの本体は `read_line().ok()` と `send` だけで、`read_line` は不正な UTF-8 でも `Err` を返してパニックしない)が、倒れる向きが安全側ではない。
  - 提案: `Err(RecvTimeoutError::Timeout) => Started::SignalTimedOut` / `Err(RecvTimeoutError::Disconnected) => Started::SignalUnreadable(holder)` に分ける。`SignalUnreadable` は既に「期限内に応答はあったが読み取れなかった」＝失敗側の変種として存在するので、新しい変種は要らない。ワイルドカード禁止(`CLAUDE.md` 関数型ドメインモデリング)はドメイン層に掛かる規約でありテストには直接及ばないので、根拠は規約ではなく上記の分類の向きに置いている。

- **[W-002]** probe が `expect` でパニックしうる — 既存の2つの probe と壊れ方が揃っておらず、`SKIPS`(`LazyLock`)を毒して以降のスキップ記録が原因不明のメッセージに化ける
  - 場所: `crates/pulsen/tests/common/lock.rs:79`(`tempfile::tempdir().expect("一時ディレクトリを作れる")`)、`crates/pulsen/tests/common/lock.rs:110`(`thread::spawn` のスレッド生成失敗)
  - 理由: `common/mod.rs::allowed_skips()` は `static SKIPS: LazyLock<SkipBudget>` の初期化子の中で `holder_capability()` を呼ぶ。`std::sync::LazyLock` は初期化子がパニックすると **poison** され、以降の `SKIPS.record(...)` はすべて `Once instance has previously been poisoned` で落ちる — フィクスチャ名もケース名も原因も出ない。つまり一時ディレクトリを作れない/スレッドを作れない環境では、本当の原因を述べたパニックが1件、無関係な毒されたパニックが N 件出る。既存の probe は2つともこの形を避けている: `permission_restrictions_effective()` は `fs::write` の失敗を `false` に落とし(`crates/pulsen-conformance/src/lib.rs:250`)、`tmpdir_outside_repository()` は `tempfile::tempdir().is_ok_and(..)` で吸収する(`crates/pulsen/tests/common/git.rs:93`)。新しい probe だけがこの不変を破っている。なお `holder_capability()` 側の `OnceLock::get_or_init` は `call_once_force` 相当なので毒されず**再実行**される。結果として「OnceLock は毎回 probe をやり直し、LazyLock は毒されたまま」という非対称な壊れ方になる。
  - 提案: probe を「パニックしない」側に揃える。`tempfile::tempdir()` の失敗は `HolderCapability::ProgramUnusable(error.to_string())`(＝失敗側だが、パニックする場所は実際に使うケースの中)へ落とす。スレッド生成の失敗まで拾うなら `thread::Builder::new().spawn(..)` を使って `Err` を `Started::SpawnFailed` に載せる。いずれも「判定は値で返し、パニックは使う場所で起こす」という本 PR の骨(ADR-004)とそのまま整合する。

- **[W-003]** 子の標準エラーを捨てているため、「この環境ではファイルロックが効かない」が「フィクスチャの実装ミス」に見えるメッセージになる
  - 場所: `crates/pulsen/tests/common/lock.rs:98`(`stderr(Stdio::null())`)、同 `:167-168`(`hold` の `!locked` 腕)
  - 理由: ADR-005 の決定により probe は `locked` を捨てるので、**ロック機構そのものが機能しない環境**(`try_lock` が常に `WouldBlock`/`Error` を返すファイルシステム上に一時ディレクトリがある、など)は `HolderCapability::Available` を通過する。そのあと5件は `hold` の「保持プロセスが、誰も保持していないロックを取得した合図を返さなかった」で落ちるが、このメッセージは子が握っていた唯一の材料(`lock_holder` が `eprintln!` する `ロックを扱えない: {message}` — `examples/lock_holder.rs:27-30`)を捨てたあとに出る。AC-4 が求める「失敗メッセージが原因を取り違えない」を、`ProgramUnusable` については満たしている一方、この環境クラスについては満たしていない。コード中の why コメントが「名指しする材料は無い」と自認しているとおりで、その材料は捨てているだけで取れる。
  - 提案: `start_holder` の `stderr` を `Stdio::piped()` にし、`kill_and_wait` の直前に(あるいは `wait_with_output` で)読み出して `Started::Signaled` の失敗側パニックに載せる。読み出しにも期限が要る点が気になるなら、probe の1回だけ `piped()` にして `HolderCapability::Available` の判断材料に加えず**メッセージ用にだけ**保持する形でもよい。`examples/lock_holder.rs` は触らずに済むのでスコープ外にも当たらない。

- **[W-004]** 期限の無い `wait()` は `release()` に残っており、probe はそこを測っていない — 保持プロセスが stdin の EOF で終了しない環境では、失敗ではなくハングになる
  - 場所: `crates/pulsen/tests/common/lock.rs:174-178`(`release`)、呼び出し元は `crates/pulsen/tests/conformance_lock.rs:74`(`release_holder`) / 同 `:79`(`try_acquire_from_other_process`) / `crates/pulsen/tests/cli_add_error.rs:135`
  - 理由: この PR は「フィクスチャのハングはテストの失敗より診断が難しい」(ADR-060)という基準を probe と失敗経路に一貫して掛けた(`kill_and_wait` の導入は正しい判断で、ADR-005 の理由づけも妥当)。残る `release()` は**正常に保持できたと分かっている相手にだけ使う**という線引きだが、probe が測っているのは「合図が期限内に返るか」だけで、「stdin を閉じたら終了するか」は一度も測っていない。したがって「起動も合図も速いが、パイプの EOF 検出が壊れている/遅い」環境は probe を通過し、`attempt_while_held` の期限超過パス(`crates/pulsen-conformance/src/exclusive_lock.rs:82`)まで含めて**期限の無い `wait()` で止まる**。libtest には per-test timeout が無いので、CI では出力の無いジョブタイムアウトとして現れ、5件のどれが原因かも分からない。ADR-060 が「期限超過時も保持を解放してから合流するため、テスト実行が止まらない」と述べた保証は、解放そのものが返ることを前提にしている。
  - 提案: この PR の変更ではなく既存構造なので Blocker には数えないが、`release()` にも期限を入れるのが本 PR の基準の自然な延長になる。案: stdin を閉じたあと `wait` を短い期限(例: `SIGNAL_DEADLINE`)付きで待ち、超えたら `kill_and_wait` へ落として `None` を返す。`release_holder` の `None` は TC-002/003 では `assert!` で失敗になり(スキップにはならない)、`try_acquire_from_other_process` では結果に影響しないので、許容集合は広がらない。子の終了待ちに期限を持たせる汎用ヘルパーが要る点だけ、スコープ判断(`.adr/071` の停止規則 — `crates/pulsen/tests/` に収まる)を PR 本文に残しておくとよい。

- **[W-005]** 「probe は無負荷・単発で測る」という ADR-001 の前提が実態と合わない — probe が走るのはテストバイナリ内で**最も混んでいる瞬間**
  - 場所: `.thread/13/adr.md:37`(ADR-001 の Consequences)、`crates/pulsen/tests/common/lock.rs:69-72`
  - 理由: `holder_capability()` は `OnceLock` で遅延評価されるので、probe が走るのは「最初にロック能力を必要としたテストスレッドの中」であり、そのとき同じバイナリの他のテストは**並列に走っている最中**である。`conformance_lock` では7ケースが同時に立ち上がり4件がほぼ同時に `spawn_holder` へ到達する。`cli_add_error` はさらに悪く、31ケースが `Home::new()` / `Repo::with_commit()`(= `git` の子プロセス起動)や `pulsen` 本体の起動を並行して行っている最中に、権限系のスキップが probe を起こす。probe の測定条件は「無負荷・単発」ではなく「そのバイナリのピーク負荷」に近い。この誤差は安全側ではなく**危険側**に効く: probe が偽陽性で `SignalTimedOut` に倒れると許容集合が黙って広がり、5件が走らなくても CI は緑になる(`.adr/068` が「機械的な歯止めが無い」と記録した経路そのもの)。ADR-001 のトレードオフ欄は「1回きり」であることは書いているが、その1回が無負荷だという前提で書かれているので、リスクの見積もりが1段甘い。
  - 提案: (a) ADR-001 のトレードオフ記述を実態に合わせる(probe の測定は並列実行の最中に行われる)。(b) 判定を非対称にする — `SignalTimedOut` に倒れるときだけ1回だけ再試行し、2回とも期限を超えたら `SignalTimedOut` とする。コストを払うのは実際に能力が無い環境だけ(最悪 `SIGNAL_DEADLINE` ×2 が1回)で、`Available` 環境の所要時間は変わらない。偽陽性で5件が黙って消える確率だけを下げられる。(b) を採らないなら、採らない理由(1回きりを保つこと自体に価値がある/待ちの上限を固定したい)を ADR に残しておきたい。

## 実地で確かめたこと

レビュー中に3つの経路を実際に踏んだ。作業ツリーは各実行後に `git checkout` で戻してあり、`git status` はクリーン。

| 経路 | 手順 | 結果 |
|---|---|---|
| `Available` | `cargo test -p pulsen --test conformance_lock -- --nocapture` | 7件 PASS / `SKIP` 行なし。所要 0.01s(probe のプロセス起動1回ぶんは計測誤差に埋もれる) |
| `ProgramMissing` | `target/debug/examples/lock_holder` を退避して同上 | 3件 PASS・**4件 FAILED**。メッセージは `lock.rs:138` の `PROGRAM_MISSING` で、不在と `cargo test --workspace` という回避方法の両方が出る(AC-4 の前半を確認) |
| `SignalTimedOut` | `SIGNAL_DEADLINE` を `Duration::from_nanos(1)` にして `cargo test -p pulsen -- --nocapture` | **全件緑**で、`tc_port_exclusive_lock_002/003/004/005` と `tc_task_register_task_017` の5件が `SKIP` 行として出る(AC-3 / AC-9 を確認)。全体の所要は 17.5s で、期限ぶんの待ちが5件に重ならないことも見えている |

- 上記3回の実行後に `ps aux | grep lock_holder` で**保持プロセスの取り残しはゼロ**。`SIGNAL_DEADLINE` を極小にした実行は probe を含めて多数の子を起動直後に `kill` するので、`kill_and_wait` の後始末が最も疑わしい条件だが、残骸は出なかった。
- `cargo clippy --workspace --all-targets --locked -- -D warnings` は無警告で通る(AC-11 の clippy 側)。`HolderCapability` / `Started` の変種名は `clippy::enum_variant_names` に掛かっていない。
- `SIGNAL_DEADLINE` は `Duration::from_secs(10)` のまま(AC-7)。

## 観点ごとの確認

### `OnceLock::get_or_init` と libtest の並列実行

問題なし。根拠を4点。

1. **再入が無い。** `probe_holder` が呼ぶのは `holder_program` / `tempfile::tempdir` / `start_holder` / `kill_and_wait` だけで、この中から `holder_capability()` へ戻る経路は無い。`get_or_init` の自己再入によるデッドロックは構造的に起きない。
2. **二重起動が無い。** `OnceLock` は初期化を1スレッドに直列化するので、probe の保持プロセスが同時に2つ立つことはない。
3. **初期化中の他スレッドは待つだけ。** 待ち時間の上限は `SIGNAL_DEADLINE`(10秒)で、libtest には per-test timeout が無いため待ち自体が失敗を作ることはない。
4. **`LazyLock` との入れ子にも循環が無い。** `SKIPS`(`LazyLock`) → `allowed_skips()` → `holder_capability()`(`OnceLock`) という一方向だけで、逆向き(probe から `common::skipped`)は存在しない。ロック獲得順序が1通りなのでデッドロックしない。

パニック時の挙動だけが非対称で、そこは W-002 に書いた。

### 子プロセスとスレッドのリーク

`Child` を kill も wait もせずに落とす経路は残っていない。

| 経路 | 後始末 |
|---|---|
| `probe_holder` の `Signaled` / `SignalUnreadable` | `kill_and_wait` |
| `probe_holder` の `SignalTimedOut` | `start_holder` 内で `kill_and_wait` 済み |
| `probe_holder` の `SpawnFailed` | 子が存在しない |
| `start_holder` の `stdout` 取得失敗 | `kill_and_wait` |
| `spawn_holder` の `SignalUnreadable` | `kill_and_wait` → パニック |
| `hold` の `!locked` | `kill_and_wait` → パニック |
| `spawn_holder` の `Signaled` | 呼び出し側へ移譲(`release` / `kill_holder`) |

合図読み取りスレッドは join されないが、リークしない。`kill_and_wait` が子を終了させると、パイプの書き込み端は**子だけ**が持っている(親側の複製は `spawn` の時点で std が閉じる)ため EOF が立ち、`read_line` が `Ok(0)` で返ってスレッドは終わる。3 OS ともこの性質は同じ。送信先(`Receiver`)が既に落ちている場合の `send` は `let _ =` で捨てているので、スレッドがパニックすることもない — ただしその逆向き(送信側が消えた場合)の扱いが W-001。

`kill_and_wait` の戻り値を両方捨てるのは妥当。既に終了した子への `kill()` は OS によってエラーになりうるが、直後の `wait()` が回収するので目的(残さないこと)は達成される。

### 資源の干渉

probe は `tempfile::tempdir()` で自前の置き場を作り、その下の `lock` を使う。本番のケースが使うのは `FileExclusiveLockHarness` の一時ホーム配下 `state/lock` と `Home` 配下 `state/lock` で、いずれも別の一時ディレクトリ。**ロックファイルの衝突は無い。** probe の一時ディレクトリは `kill_and_wait` のあとに `TempDir` の drop で消えるので、Windows でも「子がまだハンドルを握っている最中に削除しようとする」形にならない(順序は `probe_holder:80-89` で保証されている)。

プロセス数の観点では probe が1つ増えるだけ。`cargo test` はテストバイナリを逐次実行するので、バイナリ間で probe が重なることもない。

### ハングしうる経路

期限の無い `wait()` は `release()` の1箇所だけで、そこから辿れる呼び出し元は3つ(W-004 に列挙)。probe と失敗経路からは意図的に外してあり、この分離の理由づけ(ADR-005)は妥当。残った露出も既存構造の範囲だが、probe が測っていない性質に依存し続けている点は記録しておく価値がある。

`attempt_while_held`(ADR-060)は本 PR で触っていない。`hold_from_other_process` が `hold` 経由になったことで、期限超過時の `release_holder` の相手が「合図を返した＝生きていると分かっている子」であることは変わらない。

### 3 OS での成立

- **`EXE_SUFFIX`**: `holder_program()` が `env::consts::EXE_SUFFIX` を使う形は変更されていない(`lock.rs:60`)。`cfg(windows)` の決め打ちは入っていない(`.adr/071` の禁止に適合)。
- **kill の意味**: Unix は SIGKILL、Windows は `TerminateProcess`。どちらも「解放処理を実行しないままプロセスが消える」を作り、アドバイザリロックは OS が解放する。TC-005 が依拠する性質で、この PR では変えていない。`kill_and_wait` が新たに使う場所も同じ意味で十分。
- **パイプの EOF**: 上記のとおり子の終了で確実に立つ。Windows の匿名パイプでも書き込みハンドルが全て閉じれば `read` は 0 を返す。
- **一時ディレクトリの削除タイミング**: probe は `wait()` で子の終了を待ってから `TempDir` を落とすので、Windows の「使用中のファイルは消せない」に当たらない。`TempDir::drop` が失敗しても黙って無視される点は既存の慣習どおり。

### 5件への待ち時間の影響

- `Available` 環境: probe のプロセス起動1回ぶん(実測で計測誤差の範囲)。Windows の初回起動スキャンを probe が先払いする副次効果は ADR-001 の見立てどおりだが、その射程は cold-start に限られる(ADR-001 自身が正しく限定している)。
- `SignalTimedOut` 環境: **改善**。従来は4〜5件がそれぞれ `SIGNAL_DEADLINE` を待っていたのが、probe の1回だけになる。`spawn_holder` が `SignalTimedOut` で起動を試みずに `return None` する形(`lock.rs:137`)がこれを担保している。
- `ProgramMissing` 環境: probe はファイル存在確認だけで返るので待ちは増えない。
- `cli_add_error` は、ロックと無関係なスキップ(権限系・git 系)でも probe を起こす。plan のリスク欄と testing.md 補足2が正しく捉えている。Windows では権限系が必ずスキップされるので毎回 probe が走るが、`Available` なら1プロセスぶん、`SignalTimedOut` なら10秒が1回。実害は無い。

### `.github/workflows/ci.yml`

変更は why コメント5行の書き換えのみで、**実行内容は1バイトも変わっていない**。`cargo test --workspace --locked --no-fail-fast -- --nocapture 2>&1 | tee test.log` も clippy の `-D warnings` も `--nocapture` 前提の SKIP 抽出も、非 root アサートもそのまま。新しい記述「単一のテストターゲットを指定した実行は example をビルドしないだけで、過去にビルドされた成果物を消しはしない。したがって失敗するのは、target/ にその成果物が残っていない場合である」は、上で実測した `ProgramMissing` 経路の再現手順(成果物の退避が必要だった)と一致しており、正確。AC-13 は満たされている。

`--no-fail-fast` の前提への影響もない。むしろ5件が「スキップ」から「失敗」に変わる環境では、`--no-fail-fast` が無ければ後続バイナリが未観測になるので、この変更は既存のフラグ選択の価値を高める方向に効く。

## スコープの逸脱

見つからなかった。

- `crates/pulsen-conformance/src/` は変更なし(`HOOKS.md` のみ)。`ExclusiveLockHarness` のシグネチャも `SkipBudget` も `Option` を返すフック契約も無傷。
- `crates/pulsen/examples/lock_holder.rs` は変更なし。
- `SIGNAL_DEADLINE` は不変。
- `cfg(windows)` の分岐は増えていない。
- 他の probe(`permission_restrictions_effective` / `tmpdir_outside_repository`)には触れていない。

## 観点外だが記録(担当レビューアーへ)

- `.adr/073-*.md` が存在しない(`.adr/` の最大番号は 072)。plan の AC-12 は 073 の起票と、`lock.rs` の `PROGRAM_MISSING` および `conformance_lock.rs` の `allowed_skips()` の doc コメントから 073 を辿れることを求めているが、現状これらのコメントは `HOOKS.md / ADR-068` を指している。`.thread/13/adr.md` の各エントリの Status も全て `Proposed` のままで、昇格済みかどうかの判別が付かない。steps.md のステップ10(片付けフェーズ)が未実施と読める。AC-10(3 OS の CI)も同様に未実施。ドキュメント担当の観点で確認されたい。

## カバレッジ

- 確認: `crates/pulsen/tests/common/lock.rs`, `crates/pulsen/tests/common/mod.rs`, `crates/pulsen/tests/conformance_lock.rs`, `.github/workflows/ci.yml`, `crates/pulsen-conformance/HOOKS.md`, `.thread/13/plan.md`, `.thread/13/adr.md`, `.thread/13/testing.md`
- スキップ: `.thread/13/steps.md` — 実装手順の作業ログで、並行性・プロセス管理の判断そのものは adr.md と lock.rs に現れており、手順書側に独立した判断が無いため

差分外で判断材料として読んだもの: `crates/pulsen/examples/lock_holder.rs`, `crates/pulsen/src/adapter/lock.rs`, `crates/pulsen-conformance/src/exclusive_lock.rs`, `crates/pulsen-conformance/src/lib.rs`, `crates/pulsen/tests/common/git.rs`, `crates/pulsen/tests/cli_add_error.rs`, `.adr/032` `.adr/060` `.adr/068` `.adr/071`
