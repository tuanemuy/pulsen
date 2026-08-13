# レビュー 002 — 並行性・プロセス管理・堅牢性

対象: PR #14（base: main） / 契約: `.thread/13/plan.md` / 2周目（既出の `wont-fix` / `defer` は再指摘しない）

### 並行性・プロセス管理・堅牢性

#### Blockers

なし。

1周目の修正で入った変更が新しいリークやハングを作っていないかを、経路ごとに追った結果は次のとおり。

- `Started` の4変種のうち `Child` を持つのは `Signaled` / `SignalUnreadable` の2つで、`probe_holder`（`lock.rs:96-100`）は両方を1つの腕で `kill_and_wait` してから `Available` を返し、`spawn_holder`（`lock.rs:167-180`）は `Signaled` だけを呼び出し側へ渡し `SignalUnreadable` は `kill_and_wait` してからパニックする。`SignalTimedOut` は `start_holder` の中で既に畳まれており（`lock.rs:137-140`）、`SpawnFailed` は `Child` を持たない。`hold` の `!locked`（`lock.rs:189-193`）も `kill_and_wait` が先。**`Started` を受け取るすべての `match` が網羅的で、`Child` を落としたまま抜ける腕は無い。**
- `RecvTimeoutError::Disconnected` を失敗側へ倒したことによる新しい残存子プロセスは無い。`Disconnected` は `SignalUnreadable { holder, .. }` として `Child` を持ち回るので、上の2箇所の後始末に乗る（1周目の `Err(_) => kill + wait; None` と後始末の量は同じで、判定だけが変わっている）。
- 合図読み取りスレッドが `io::Result<String>` を送るようになった影響について。送信先が消えるのは `start_holder` が `receiver` ごと戻った後だけで、`let _ = sender.send(read)`（`lock.rs:129`）が `SendError` を捨てるためスレッドはパニックしない。スレッドが残るのは `read_line` が返らない間だけで、`SignalTimedOut` / `SignalUnreadable` のいずれの経路でも先に `kill_and_wait` が子を回収し、その時点でパイプの書き手が消えて `read_line` が EOF で返る。`lock_holder` は孫プロセスを作らないので、書き手側のハンドルが子より長く生き残ることもない。
- `OnceLock::get_or_init` は「同時に複数スレッドが呼んでも初期化は1回、他は完了までブロック」という契約で、libtest の並列実行でも probe は1回に収まる。`LazyLock<SkipBudget>`（`common/mod.rs:38` / `conformance_cases!` の `$budget`）→ `allowed_skips()` → `holder_capability()` という依存の向きは常に一方向で、`spawn_holder` からの呼び出しも同じ `OnceLock` を内側に見るだけなので、ロック順序の逆転による相互待ちは作られていない。
- probe の子プロセスとテスト本体の子プロセスの資源干渉は無い。probe は `tempfile::tempdir()` 直下の `lock` を使い（`lock.rs:92-93`）、ハーネスは `conformance_cases!` が `$setup` をケースごとに評価するため 1ケース1 `TempDir`、`cli_add_error` の 017 も `Home` ごとの `state/lock` で、`flock` / `LockFileEx` の対象ファイルが重ならない。`TempDir` の解放は `kill_and_wait` の後（`probe_holder` のスコープ末尾）なので、Windows でハンドル保持中の削除に当たる順序にもなっていない。プロセス環境変数を書き換える処理も無い（`detached_home` は `Command` 側にしか触らない）ため、`tempfile` の置き場解決とテストの並列実行が競合しない。
- 実地確認（macOS / stable）: `cargo test --workspace --locked --no-fail-fast -- --nocapture` 緑・ロック系の `SKIP` 行なし・実行後に `lock_holder` の残存プロセスなし。`SIGNAL_DEADLINE` を一時的に `from_nanos(1)` にすると `conformance_lock` の4件と `cli_add_error` の 017 が `SKIP` になって緑、この場合も残存プロセスなし（確認後に `git checkout` で復元済み、`git status` クリーン）。`target/debug/examples/lock_holder` を削除して `--test conformance_lock` を回すと4件が `PROGRAM_MISSING` の案内つきで失敗し、ハングしない。`cargo fmt --all --check` / `cargo clippy --workspace --all-targets --locked -- -D warnings` も通る。

#### Warnings

- **[W-001]** `kill_and_wait` の `wait()` に期限が無く、しかも probe 経由で `OnceLock::get_or_init` の中で走るため、`kill` が効かない子に当たると `holder_capability()` を待つ全スレッドが無出力のまま止まる。
  - 場所: `crates/pulsen/tests/common/lock.rs:205-208`（呼び出し元は `lock.rs:98` / `120` / `138` / `170` / `190`）
  - 理由: `.adr/073` の「期限の無い待ちは、正常に保持できたと分かっている相手にだけ許す」と、その直後の「probe は…1度きりの初期化の中にあるため、そこで止まれば能力を問うすべてのスレッドが止まる。ADR-060 が期限を置いた対象を、probe で期限の無い待ちに戻さない」は、無条件の性質として書かれている。実装がこれを満たすのは「`kill` が確実に効く」という前提の上でだけで、`kill_and_wait` に入るのは無期限の `wait()` である。前提が崩れる状況は抽象的ではない — `kill_and_wait` を踏む主経路は `SignalTimedOut`（`lock.rs:137-140`）、すなわち子が期限内に合図を返せなかった経路であり、その最有力の原因のひとつがロック取得の syscall で刺さっていることである。ネットワークファイルシステム上での `flock` / `LockFileEx` はまさにその形で止まりうるので、「合図が返らない」と「SIGKILL が届いても即座には終われない」は相関している。ハングした場合の見え方は `.thread/13/review/triage.md` の #15 起票案が述べているとおり（libtest に per-test timeout が無く、CI では出力の無いジョブタイムアウトになる）で、blast radius は `release` より広い — `common/mod.rs:38` の `LazyLock<SkipBudget>` 初期化の中で止まると、ロックと無関係なケースのスキップ記録まで巻き込んで固まる。
  - 提案: このPRのスコープを広げる必要はない。#15（`release` の期限）の起票内容案が「超えたら `kill_and_wait` へ落として `None` を返す」と、`kill_and_wait` を期限つき待ちの安全な終着点として扱っているので、その前提を先に潰す形で **`kill_and_wait` 自体も期限付き子待ちヘルパーの射程に含める**旨を #15 に足す（`try_wait` のポーリングで期限を超えたら諦めて `Child` を捨て、その事実を記録する等）。あわせて `.adr/073` 43-45行と `.thread/13/adr.md` ADR-005 の断定を「`kill` が届く限りにおいて期限の無い待ちにならない」と条件つきに直すと、正本とコードの主張が一致する。

- **[W-002]** `Command::spawn` 成功後に `thread::spawn` がパニックすると、`holder` が `kill` も `wait` もされずに落ち、ロックを掴んだままの子が取り残される。probe の中で起きた場合は `OnceLock` が未初期化のまま残るため、次のスレッドがもう一度 probe を走らせて取り残しを増やしうる。
  - 場所: `crates/pulsen/tests/common/lock.rs:108-130`（`thread::spawn` は `lock.rs:124`）
  - 理由: `std::thread::spawn` は OS がスレッドを作れないときにパニックする（回復するには `thread::Builder::spawn` を使う）。この時点で `holder: Child` はスコープ内にあり、`Child` は Drop で kill も wait もしないので、保持プロセスは走り続け、`probe_holder` の `TempDir` が消えたあともロックファイルを開いたまま zombie として残る。`.thread/13/testing.md` の「1. probe が保持プロセスを取り残さない」は `kill_and_wait` の呼び出し箇所を数える形で確認しているため、この「パニックで `match` に到達しない」経路は検査に掛かっていない。加えて `OnceLock::get_or_init` は初期化関数がパニックするとセルを未初期化のまま残す（`Once::call_once_force` 相当で毒さない）ので、`lock.rs:77` の doc「1度だけ評価して使い回す」と `.adr/073` の「待ちの上限が固定であること(判定は1回きり)」もこの経路では成立しない。スレッド生成の失敗は稀ではあるが、probe が走るのは `.thread/13/adr.md` ADR-001 のトレードオフ欄が言うとおり「そのバイナリの混雑した瞬間」であり、資源が細い状況と重なる側に置かれている。
  - 提案: 後始末を全経路の手書きに依存させない形に寄せる。(a) `thread::Builder::new().spawn(..)` にして `Err` は `kill_and_wait(holder)` してから `Started::SpawnFailed(error)`（または `SignalUnreadable`）へ落とす、あるいは (b) `Child` を Drop で `kill` + `wait` するガード型に包み、正常経路だけが `into_inner()` で取り出す。(b) なら `SignalUnreadable` / `!locked` / `SignalTimedOut` の各腕から `kill_and_wait` の書き忘れが構造的に消え、`.adr/073` の「失敗経路の後始末は `kill` + `wait` に揃える」を型で保てる。

- **[W-003]** `Started::SignalUnreadable` が「期限内に返ったが読めなかった」と「そもそも何も返ってこなかった」を1つの変種に畳んでおり、probe が後者まで `Available`（＝能力あり）と判定する。`Disconnected` を許容集合側に倒さなかった理由と、probe の腕が食い違っている。
  - 場所: `crates/pulsen/tests/common/lock.rs:141-145` と `crates/pulsen/tests/common/lock.rs:94-100`（変種の定義は `lock.rs:55-56`）
  - 理由: `lock.rs:141` のコメントは「期限を1ミリ秒も測っていないので、環境の能力(＝スキップを許容する側)には倒さない」と述べるが、`Disconnected` が入る `SignalUnreadable` は `probe_holder` で `Available` に畳まれる。`Available` も「合図が期限内に返る」という能力の主張なので、測っていない期限について probe が肯定側の断定をする形になっている。`lock.rs:94-95` の「probe が測るのは合図が期限内に返るかだけで、読み取れたかどうかは能力の判定に入れない」という説明が成り立つのは `Ok(Err(_))`（何かが期限内に返り、その中身が読めなかった）の側だけで、`Disconnected` には掛からない。`.adr/073` の「probe の判定基準は『合図が期限内に返ったか』だけに限る」に照らすと、`Disconnected` は `Available` でも `SignalTimedOut` でもない第3の観測であり、いまは前者に寄せられている。実害は「読み取りスレッドが確定的に死ぬ環境で、probe が `Available` を名乗ったうえで5件が『合図の読み取りが結果を返さずに終了した』で失敗する」という形に留まり、静かな緑にはならない（＝Blocker ではない）が、能力の型が持つ値としては偽になる。
  - 提案: `Started` の変種を `SignalUnreadable`（`Ok(Err)`）と `SignalLost`（`Disconnected`）に割り、probe では前者だけを `Available` に数え、後者は `Available` を名乗らずに落とす（フィクスチャ側の異常なのでその場でパニックさせるのが素直）。変種を増やしたくない場合は、最低限 `lock.rs:94-95` のコメントから「読み取れたかどうかは能力の判定に入れない」の断定を外し、`Disconnected` も `Available` に数えていることと、その割り切りの理由を書く。

#### カバレッジ

- 確認: `crates/pulsen/tests/common/lock.rs`, `crates/pulsen/tests/common/mod.rs`, `crates/pulsen/tests/conformance_lock.rs`, `.github/workflows/ci.yml`, `crates/pulsen-conformance/HOOKS.md`, `.adr/073-holder-capability-skip-vs-fail.md`, `.thread/13/plan.md`, `.thread/13/steps.md`, `.thread/13/testing.md`, `.thread/13/adr.md`, `.thread/13/review/triage.md`
- 参照（差分外・前提の確認用）: `crates/pulsen-conformance/src/exclusive_lock.rs`, `crates/pulsen-conformance/src/lib.rs`, `crates/pulsen/examples/lock_holder.rs`, `crates/pulsen/src/adapter/lock.rs`, `crates/pulsen/tests/cli_add_error.rs`, `.adr/032` / `.adr/060` / `.adr/068` / `.adr/071`
- スキップ: `.thread/13/review/review-001.md` — レビューループ自身の成果物（1周目の統合レビュー）
- スキップ: `.thread/13/review/review-001-concurrency.md` — 同上。既出指摘の判定は `triage.md` で把握
- スキップ: `.thread/13/review/review-001-docs.md` — 同上
- スキップ: `.thread/13/review/review-001-test.md` — 同上
- スキップ: `.thread/13/review/review-001-type-design.md` — 同上
- スキップ: `.thread/13/review/fix-plan-001.md` — 同上（修正計画。結果は成果物のコードで確認した）

一覧17件の内訳: 上記の「確認」10件（`triage.md` を含む）＋「スキップ」6件＋本ファイル未満の残り1件 = `.thread/13/plan.md`（確認済み・受け入れ基準とスコープの検証に使用）。

##### 契約（plan.md）との突き合わせ — 本観点に掛かる分

- AC-6（`None` の意味が1つに絞られている）: 成立。`spawn_holder` の `None` は `SignalTimedOut` の1経路だけで、`hold` は `?` の伝播しか `None` を作らない。
- AC-7（`SIGNAL_DEADLINE` が変わっていない）: 成立。`lock.rs:18` は `from_secs(10)` のまま。
- AC-11（fmt / clippy）: ローカルで再現し、いずれも通ることを確認。
- リスク欄「[低] probe が起動した保持プロセスを取り残すと、以降のケースがロックを取れない」: 正常系では成立しているが、W-002 のパニック経路だけがこの前提の外にある。
- スコープ「`crates/pulsen-conformance/src/` の変更」を含まないこと: 差分に無く、ハーネスの `Option` 契約も維持されている（`hold_from_other_process` は `SignalTimedOut` のときだけ `None`）。
