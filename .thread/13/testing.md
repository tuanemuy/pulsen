# 動作確認計画 — Issue #13: ロック保持フィクスチャの合図タイムアウトが、スキップではなく失敗として現れる

**Issue:** #13
**作成日:** 2026-08-13

---

## 確認環境

このIssueの変更を確認するために必要な手順のみ記載（プロジェクト全体のセットアップは省略）。

本Issueの変更対象はテストのフィクスチャ層（`crates/pulsen/tests/`）とその表（`crates/pulsen-conformance/HOOKS.md`）・CI の why コメント・`.adr/` であり、CLI の振る舞いも `ExclusiveLock` ポートの契約も変わらない。**Web UI も dev サーバも存在しない**（ワークスペースは CLI バイナリ1本とライブラリ2本のみ）ため、確認はすべてターミナル上でのテスト実行・コード確認・CI 実行結果の読み取りで行う。

確認の中心は「5つの経路（`Available` / `SignalTimedOut` / probe 成立後のタイムアウト / `ProgramMissing` / `ProgramUnusable`）をそれぞれ1回ずつ実地で踏み、宣言（`allowed_skips()`）と実態が一致することを見る」ことにある。最後の1つは unix 限定で、Windows 固有の事情（Defender の隔離・実行形式の不一致）だけがコードレビューによる。経路を作るには一時的なコード差し替えやビルド成果物の操作が要るものがあり、その戻し忘れ自体が受け入れ基準（AC-7）に関わるので、各項目の手順に戻し方まで書いてある。

### 検証環境の起動

`.envrc` は `use flake` のみ。direnv がこのリポジトリで許可済みであれば、リポジトリに `cd` するだけで `flake.nix` の devShell（cargo / rustc / clippy / rustfmt / git）に入る。direnv を使わない場合は各コマンドを `nix develop -c <command>` で包む。

```sh
cd /Users/hikaru/github.com/tuanemuy/pulsen_2
cargo --version    # devShell が効いていることの確認
git --version
```

確認を始める前の基準状態（この時点で緑であること）。

```sh
cargo fmt --all --check
cargo build --workspace --locked
cargo test --workspace --locked --no-fail-fast -- --nocapture
cargo clippy --workspace --all-targets --locked -- -D warnings
```

`cargo fmt` には `--locked` を付けない（受け付けない）。ローカルの rustfmt は nixpkgs 同梱で CI の現行 stable とは版が違うため、ここが緑でも CI の fmt ジョブが赤になりうる。

本Issueの確認で繰り返し使う実行の形は3つある。使い分けの意味が経路の分かれ目そのものなので、混同しないこと。

| コマンド | example をビルドするか | この計画での用途 |
|---|---|---|
| `cargo test --workspace --locked --no-fail-fast -- --nocapture` | する | `Available` 経路（CI と同じ形） |
| `cargo test -p pulsen -- --nocapture` | する | `SignalTimedOut` / probe 成立後のタイムアウト経路 |
| `cargo test -p pulsen --test conformance_lock -- --nocapture` / `cargo test -p pulsen --test cli_add_error -- --nocapture` | **しない**（ただし既存の成果物を消しもしない） | `ProgramMissing` 経路（成果物の削除とセット）/ `ProgramUnusable` 経路（成果物の `chmod 000` とセット）。どちらも成果物への操作と組んで初めて意味を持つ |

保持プロセスの実行ファイルは `target/debug/examples/lock_holder`（Unix）/ `target/debug/examples/lock_holder.exe`（Windows）にある。

`SKIP` 行の読み取りには `--nocapture` が要る。libtest は成功したテストの標準出力を握り潰すため、これが無いと `SKIP` 行そのものがログに出ない。

### デプロイ方法

なし。変更対象はテストコードとドキュメントとワークフローファイルで、配布物にも実行時の振る舞いにも影響しない。CI の why コメント（確認項目13）はコミットした時点でその PR 上のファイルとして有効になり、別途のデプロイ手順は無い。

---

## 確認項目

### 1. 能力の型と probe が `lock.rs` に1つだけ置かれている

- **対応する受け入れ基準:** AC-1
- **目的:** 4つの区別（`Available` / `SignalTimedOut` / `ProgramMissing` / `ProgramUnusable`）を持つ能力の型と、それを1度だけ評価する probe が、`crates/pulsen/tests/common/lock.rs` に**1組だけ**存在することを確認する。同じ判断を別の場所で作り直していれば、宣言と挙動が割れる余地が残る。
- **手順:**
  1. `grep -n "enum HolderCapability" -A 10 crates/pulsen/tests/common/lock.rs`
  2. `grep -n "OnceLock" crates/pulsen/tests/common/lock.rs`
  3. `grep -rn "OnceLock\|LazyLock" crates/pulsen/tests/`
  4. `grep -n "enum Started" -A 10 crates/pulsen/tests/common/lock.rs`
- **期待結果:**
  1. `HolderCapability` が1件ヒットし、変種が `Available(HolderProgram)` / `SignalTimedOut` / `ProgramMissing` / `ProgramUnusable(io::Error)` の**4つちょうど**。`Available` は解決済みの実行ファイルを、`ProgramUnusable` は probe の起動時に `spawn` が返したエラーをそのまま持つ。`HolderProgram` のフィールドが非公開で、実行ファイルのパスが `lock.rs` の外へ出ない。
  2. `OnceLock` の宣言が `holder_capability()` の中の1件だけ（`static CAPABILITY: OnceLock<HolderCapability>`）。
  3. ヒットは**6行**で、3つの `use` 行（`common/git.rs` / `common/lock.rs` / `common/mod.rs`）と3つの `static` 宣言だけ。`static` の内訳は `common/lock.rs` の `CAPABILITY: OnceLock<HolderCapability>`（ロック能力。`crates/pulsen/tests/` 全体でこの1件だけ）、`common/mod.rs` の `SKIPS: LazyLock<SkipBudget>`（許容集合のキャッシュ。この中から `holder_capability()` を呼ぶ）、`common/git.rs` の `OUTSIDE: OnceLock<bool>`（`tmpdir_outside_repository()` が使う別の probe のキャッシュ）で、説明の付かないヒットは無い。
  4. `Started` が private（`pub` が付いていない）で、変種が `Signaled { holder, locked }` / `SignalUnreadable { holder, error }` / `SignalTimedOut` / `SpawnFailed(io::Error)` の4つ。`SignalUnreadable` は読み取りが失敗した理由（`io::Error`）を運ぶ。
- **確認ポイント:** `Started` の変種名が `enum` 名 `Started` で終わっていないこと（`clippy::enum_variant_names` は既定 warn で、CI の `-D warnings` で赤になる。確認項目11 で機械的にも見る）。`HolderCapability` に `Child` が乗っていないこと（`'static` に置く型なのでプロセスハンドルは持てず、それが `Started` を別に立てている理由）。`ProgramUnusable` が起動失敗の理由を文字列に畳んでいないこと（`io::Error` は `Send + Sync + 'static` なので `'static` の型にそのまま置け、畳むと `ErrorKind` が失われる）。

### 2. 判断の源が probe の1点に絞られ、古い述語が残っていない

- **対応する受け入れ基準:** AC-2
- **目的:** `spawn_holder` / `hold` / `try_acquire_from_other_process` と、`conformance_lock.rs` / `common/mod.rs` の `allowed_skips()` が、いずれも probe の結果だけを見ていることを確認する。`holder_program()` を private にしたことで、古い述語（実行ファイルの有無）を使う呼び出し側が残らないことをコンパイラが保証する。
- **手順:**
  1. `grep -n "fn holder_program" crates/pulsen/tests/common/lock.rs`
  2. `grep -rn "holder_program" crates/pulsen/tests/`
  3. `grep -rn "holder_capability" crates/pulsen/tests/`
  4. `grep -n "fn try_acquire_from_other_process" -A 6 crates/pulsen/tests/conformance_lock.rs`
  5. `git diff origin/main -- crates/pulsen/tests/conformance_lock.rs` で `try_acquire_from_other_process` の本文に差分が無いことを見る
  6. `cargo build --workspace --locked`
- **期待結果:**
  1. `fn holder_program(` に `pub` が付いていない。
  2. ヒットが `crates/pulsen/tests/common/lock.rs` の中だけ（定義と `probe_holder` からの1呼び出し）。`conformance_lock.rs` / `common/mod.rs` からは消えている。
  3. `common/lock.rs`（定義）・`conformance_lock.rs` の `allowed_skips()`・`common/mod.rs` の `allowed_skips()` の3箇所。`spawn_holder` からの呼び出しを加えて、判断の源はこの1関数に集まっている。
  4. 本文が `spawn_holder` を呼ぶ形のまま。`locked == false` を失敗に倒していない（TC-port-exclusive-lock-004 が「ドロップで解放され、別プロセスが取得できる」を判定する経路で、`bool` は正当な観測結果）。
  5. 差分なし。**変更不要であることの確認でこの基準の一部を満たす**（probe の結果は `spawn_holder` 経由で同じ1点を見ている）。
  6. コンパイルが通る（private 化で壊れる呼び出し側が残っていない）。
- **確認ポイント:** 手順2 が `tests/` の外（例: `crates/pulsen/src/`）にヒットしないこと。手順4 で `hold` を呼ぶ形に「揃えて」しまっていないこと — ここで `!locked` を失敗に倒すと、フィクスチャがケースの判定を先取りする。

### 3. `None` の意味が1つに絞られている

- **対応する受け入れ基準:** AC-6
- **目的:** `spawn_holder` / `hold` / `hold_from_other_process` の `None` が「この環境は合図を期限内に返せない」だけを意味し、ロックを取得できなかった場合と合図を読み取れなかった場合が混ざらないことを確認する。
- **手順:**
  1. `grep -n "fn spawn_holder" -A 30 crates/pulsen/tests/common/lock.rs` で `None` を返す箇所を数える
  2. `grep -n "fn hold" -A 12 crates/pulsen/tests/common/lock.rs`
  3. `grep -n "fn hold_from_other_process" -A 6 crates/pulsen/tests/conformance_lock.rs`
  4. `grep -n "None" crates/pulsen/tests/common/lock.rs`
  5. `grep -n "recv_timeout" -A 12 crates/pulsen/tests/common/lock.rs`
- **期待結果:**
  1. `spawn_holder` の中で `None` を返すのは `HolderCapability::SignalTimedOut` の腕**1箇所だけ**。残る3つの腕（`Available` / `ProgramMissing` / `ProgramUnusable`）は、それぞれ処理の続行と2種のパニックになっている。`Started::SignalUnreadable` / `SignalTimedOut` / `SpawnFailed` はいずれもパニックで、`None` に落ちない。
  2. `hold` は `spawn_holder(lock_path)?` の伝播のみで `None` を作り、`!locked` は `kill_and_wait` + パニックになっている（`return None` が無い）。
  3. `hold_from_other_process` は `common::lock::hold(&self.lock_path())` の1行に委ねられている（`!locked` の判断が `hold` の中の1箇所だけになる）。
  4. `?` 演算子と `SignalTimedOut` 腕以外に `None` を生む箇所が無い。
  5. `recv_timeout` の失敗が `RecvTimeoutError::Timeout` と `Disconnected` に分かれており、`Err(_)` の形が無い。`Disconnected`（読み取りスレッドが結果を返さずに消えた）は `Started::SignalUnreadable`＝失敗側に寄っていて、許容集合に入る `SignalTimedOut` に落ちない。
- **確認ポイント:** `spawn_holder` の doc コメントが `None` の意味を1つに明記し、実行ファイルの不在・起動の失敗・合図の読み取り失敗・probe 成立後のタイムアウトを**スキップに逃がさない**と書いてあること。`hold` の `!locked` のメッセージが「取得の合図が返らなかった」であって「取得できなかった」と断定していないこと（`stderr(Stdio::null())` で子の標準エラーを捨てている以上、原因を断定する材料をフィクスチャは持たない）。

### 4. 許容集合が `SignalTimedOut` のときだけ広がる（宣言のコード）

- **対応する受け入れ基準:** AC-3
- **目的:** 5件（`tc_port_exclusive_lock_002/003/004/005` と `tc_task_register_task_017`）が許容集合に入るのは probe が `SignalTimedOut` を返したときだけであり、その `match` がワイルドカードを使わず4つの区別を網羅していることを確認する。
- **手順:**
  1. `grep -n "fn allowed_skips" -A 16 crates/pulsen/tests/conformance_lock.rs`
  2. `grep -n "fn allowed_skips" -A 22 crates/pulsen/tests/common/mod.rs`
  3. `grep -n "LOCK_HOLDER_CASES" -B 4 crates/pulsen/tests/conformance_lock.rs crates/pulsen/tests/common/mod.rs`
- **期待結果:**
  1. `match common::lock::holder_capability()` の腕が `SignalTimedOut => LOCK_HOLDER_CASES.to_vec()` と、`Available(_) | ProgramMissing | ProgramUnusable(_) => Vec::new()` の形になっており、`_` が無い。
  2. 同じ形のロック分岐になっている（`SignalTimedOut` のときだけ `allowed.extend(LOCK_HOLDER_CASES)`、残る3つは何もしない）。権限系（`permission_restrictions_effective`）と git 系（`tmpdir_outside_repository`）の分岐は変わっていない。
  3. 両ファイルの `LOCK_HOLDER_CASES` の doc コメントが「別プロセスにロックを保持させられない環境」ではなく「保持プロセスの合図が期限内に返らない環境」と同じ言葉で揃っている。件数は `conformance_lock.rs` が4件（`tc_port_exclusive_lock_002` / `003` / `004` / `005`）、`common/mod.rs` が1件（`tc_task_register_task_017`）。
- **確認ポイント:** 2箇所の `match` が同じ4区別を同じ側へ振り分けていること。片方だけ `ProgramUnusable` をスキップ側に倒すと、同じフィクスチャを使う適合スイートと CLI 側で扱いが割れる。`conformance_lock.rs` の `allowed_skips()` の doc コメントから `HOOKS.md` / ADR-068 / ADR-073 が辿れること（確認項目14 で新番号の併記を見る）。

### 5. `Available` 経路 — 通常の実行が緑で、5件が `SKIP` に出ない

- **対応する受け入れ基準:** AC-3, AC-9（対照）
- **目的:** probe が成立する環境（手元の開発機）で、5件が実際に走ることを確認する。これが以降3つの経路の基準線になる。
- **手順:**
  1. `cargo test --workspace --locked --no-fail-fast -- --nocapture`
  2. 出力から `SKIP ` を含む行を拾い、`tc_port_exclusive_lock_002` / `003` / `004` / `005` / `tc_task_register_task_017` が現れないことを見る
  3. `ls target/debug/examples/`
- **期待結果:**
  1. 全テストバイナリが緑（`test result: ok`）。
  2. ロック系5件の `SKIP` 行が**0件**。手元が unix であれば `SKIP` 行は `tc_port_clock_005`（実時計を巻き戻せない恒久スキップ）と、`pulsen-conformance` の lib ユニットテストが `SkipBudget` 自身の検証で出す架空の3行（`tc_port_clock_004_…` / `tc_port_clock_0051_…` / `tc_port_clock_005_…`）だけ。架空の3行は走らなかった適合ケースではないので数えない。
  3. `lock_holder`（Windows では `lock_holder.exe`）が存在する。
- **確認ポイント:** probe が実行時間を目に見えて伸ばしていないこと（コストは1プロセスぶん。Windows の初回起動スキャンを probe が先に払う副次効果は見込めるが、本番5件がどれだけ温まった状態で走るかは測っていない）。ロック系5件が `SKIP` に出た場合、この時点で以降の経路確認に進まない — 基準線が崩れている。

### 6. `SignalTimedOut` 経路 — 5件が `SKIP` として現れ、かつ緑になる

- **対応する受け入れ基準:** AC-9, AC-3
- **目的:** 合図が期限内に返らない環境で、5件が `SkipBudget` 違反の失敗ではなく宣言済みスキップとして現れることを確認する。これが Issue の主目的（現状はここが赤になる）。
- **手順:**
  1. `crates/pulsen/tests/common/lock.rs` の `SIGNAL_DEADLINE` を一時的に `Duration::from_nanos(1)` に書き換える。
  2. `cargo test -p pulsen -- --nocapture`
  3. 出力の `SKIP ` 行を拾う。
  4. `git checkout crates/pulsen/tests/common/lock.rs` で戻す（または手で `Duration::from_secs(10)` に戻す）。
  5. `git diff crates/pulsen/tests/common/lock.rs` で差分が無いことを確認する。
- **期待結果:**
  - 手順2: **緑**（`test result: ok`）。probe が `SignalTimedOut` に倒れ、許容集合が5件ぶん広がるため、`SkipBudget` 違反にならない。
  - 手順3: `tc_port_exclusive_lock_002` / `003` / `004` / `005` と `tc_task_register_task_017` の**5件**が `SKIP` 行として現れる。
  - 手順5: 差分なし。
- **確認ポイント:** `-p pulsen` は example もビルドするので、probe は起動まで進んで**期限だけが原因で** `SignalTimedOut` に倒れる（確認項目8 との違いはここ）。実行が `SIGNAL_DEADLINE` ぶん待たされないこと — probe の1回だけが待ち、残る5件は起動を試みずに即座に `None` を返す設計になっている。5件が「失敗」として出たら宣言が広がっておらず、`allowed_skips()` の `match` を疑う。

### 7. probe 成立後のタイムアウト経路 — スキップではなく失敗になる

- **対応する受け入れ基準:** AC-5
- **目的:** probe が `Available` と判定したあとに合図がタイムアウトした場合、それが環境の能力ではなく異常として失敗し、メッセージが「probe は同じ手順で成立している」ことを述べることを確認する。
- **手順:**
  1. `crates/pulsen/tests/common/lock.rs` の `start_holder` の先頭に、呼び出し回数で期限を切り替える差し込みを一時的に入れる。probe が1回目、本番のケースが2回目以降になるので、能力は `Available` のまま5件が `Started::SignalTimedOut` を踏む。

     ```rust
     // 一時的な差し込み(確認後に必ず戻す)
     static CALLS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
     let deadline = if CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed) == 0 {
         SIGNAL_DEADLINE
     } else {
         Duration::from_nanos(1)
     };
     ```

     `recv_timeout` には `SIGNAL_DEADLINE` ではなくこの `deadline` を渡す。`SIGNAL_DEADLINE` の**定数の値そのものは変えない**。
  2. `cargo test -p pulsen -- --nocapture`
  3. 失敗したケース名と、パニックメッセージの本文を読む。
  4. `git checkout crates/pulsen/tests/common/lock.rs` で戻す。
  5. `git diff crates/pulsen/tests/common/lock.rs` で差分が無いことを確認する。
- **期待結果:**
  - 手順2: **赤**。`tc_port_exclusive_lock_002` / `003` / `004` / `005` と `tc_task_register_task_017` が失敗する。
  - 手順3: パニックメッセージに「保持プロセスの合図が … 以内に返らなかった」と「**probe は同じ手順で成立している**」の両方が含まれ、この環境で繰り返し起きるなら `SIGNAL_DEADLINE` を見直す旨が読める。
  - 手順3: 5件が `SKIP` 行としては現れない（能力は `Available` なので許容集合は空）。
  - 手順5: 差分なし。
- **確認ポイント:** メッセージが「この環境では保持プロセスを使えない」と読める文言になっていないこと — probe が同じ手順で成立している以上、能力の問題ではないと述べるのがこのメッセージの役目。`crates/pulsen/examples/lock_holder.rs` に `sleep` を入れる形は取らない（スコープ外。保持プロセス側の挙動は本Issueで変えない）。

### 8. `ProgramMissing` 経路 — 失敗し、不在と回避方法を案内する

- **対応する受け入れ基準:** AC-4
- **目的:** 実行ファイルが無い場合に5件がスキップではなく失敗として現れ、メッセージが原因（フィクスチャがビルドされていない）と回避方法（`cargo test --workspace` のように example をビルドする形で回す）を述べることを確認する。
- **手順:**
  1. `rm -f target/debug/examples/lock_holder target/debug/examples/lock_holder.exe`
  2. `ls target/debug/examples/` で `lock_holder` が消えていることを確認する（消えていないと probe が `Available` に倒れ、この確認は空振りする）。
  3. `cargo test -p pulsen --test conformance_lock -- --nocapture`
  4. `cargo test -p pulsen --test cli_add_error -- --nocapture`
  5. 各失敗のパニックメッセージを読む。
  6. `cargo test --workspace --locked --no-fail-fast -- --nocapture` で成果物を作り直し、緑に戻ることを確認する。
- **期待結果:**
  - 手順3: **赤**。`tc_port_exclusive_lock_002` / `003` / `004` / `005` の**4件**が失敗する。
  - 手順4: **赤**。`tc_task_register_task_017` の**1件**が失敗する。
  - 手順5: メッセージに「ロック保持フィクスチャ(examples/lock_holder)の実行ファイルが無い」旨と、「単一のテストターゲットを指定した実行では example がビルドされない」旨、「`cargo test --workspace` のように example をビルドする形で実行する」旨の3点が含まれる。
  - 手順3・4 とも、5件が `SKIP` 行としては現れない。
  - 手順6: 緑に戻り、ロック系5件の `SKIP` が0件（確認項目5 と同じ状態）。
- **確認ポイント:** **手順1 の削除を省略しないこと。** `--test` 指定は example をビルドしないが、過去にビルドされた成果物を消しもしないため、確認項目5 の直後は成果物が必ず残っていて 5件が普通に通ってしまう（=何も確かめられない）。`CARGO_TARGET_DIR` を別のディレクトリに向けて `--test` 指定で1回だけ回す形でも同じ状態を作れるが、依存の再ビルドが要るぶん遅い。
  手順4 では、**ロックを使わないケースのスキップが probe を起こす**。`common/mod.rs` の `allowed_skips()` は `SKIPS`（`LazyLock`）の初期化時に一度だけ評価され、その中で `holder_capability()` を呼ぶため、最初に `common::skipped` を通るのが権限系（`tc_task_register_task_016` / `021`）や git 系（`036`）であれば、そこで probe が走る。この確認では成果物を消してあるので probe は `ProgramMissing` を即座に返し、待ちは入らない。

### 9. `ProgramUnusable` 経路 — 失敗し、`spawn` の `io::Error` が案内に載る

- **対応する受け入れ基準:** AC-4（実行ファイルはあるが起動できない側）
- **目的:** 起動が失敗した場合に、その理由（権限不足・`noexec` マウント・Windows Defender による隔離・実行形式の不一致のどれなのか）がパニックメッセージに載り、スキップではなく失敗として現れることを確認する。unix ではビルド済みの実行ファイルから実行ビットを落とすことで確定的に再現できる。Windows には同じ手段が無いので、そちら側だけコードレビューによる。
- **手順（unix — 実地）:**
  1. `cargo test --workspace --locked --no-fail-fast -- --nocapture` で成果物がある状態にする（確認項目8 の直後はここから始める）。
  2. `chmod 000 target/debug/examples/lock_holder`
  3. `ls -l target/debug/examples/lock_holder` で実行ビットが落ちていることを確認する。
  4. `cargo test -p pulsen --test conformance_lock -- --nocapture`
  5. 失敗したケース名とパニックメッセージの本文を読む。
  6. `chmod 755 target/debug/examples/lock_holder` で戻し、`ls -l` で実行ビットが戻っていることを確認する。
  7. `cargo test --workspace --locked --no-fail-fast -- --nocapture` で緑に戻ることを確認する。
- **手順（Windows — コードレビュー）:**
  8. `grep -n "ProgramUnusable" -B 2 -A 4 crates/pulsen/tests/common/lock.rs`
  9. `grep -n "SpawnFailed" -B 2 -A 4 crates/pulsen/tests/common/lock.rs`
  10. `grep -n "fn start_holder" -A 25 crates/pulsen/tests/common/lock.rs` で `spawn()` の `Err` と `stdout` 取得失敗が `Started::SpawnFailed` に振り分けられていることを読む
  11. `grep -n "fn probe_holder" -A 16 crates/pulsen/tests/common/lock.rs`
- **期待結果:**
  - 手順4: **赤**。`tc_port_exclusive_lock_002` / `003` / `004` / `005` の**4件**が失敗し、`SKIP` 行としては現れない。
  - 手順5: メッセージが `ロック保持フィクスチャ(examples/lock_holder)を起動できなかった(probe の起動時に観測した理由): Permission denied (os error 13)` の形で、`spawn` が返したエラーの内容がそのまま載っている。probe の時点で観測した理由であることが読め、`Started::SpawnFailed` の腕（今この場の起動の失敗）と読み分けられる。
  - 手順7: 緑に戻り、ロック系5件の `SKIP` が0件（確認項目5 と同じ状態）。
  - 手順8: `HolderCapability::ProgramUnusable(error)` の腕のパニックメッセージに `{error}` が埋め込まれている。理由は `io::Error` のまま保持されていて、文字列に畳まれていない（`ErrorKind` が残るので、後から「この種類は環境の能力側へ倒す」といった述語を書く材料がある）。
  - 手順9: `Started::SpawnFailed(error)` の腕のパニックメッセージにも `{error}` が埋め込まれている。
  - 手順10: `spawn()` が `Err` を返した場合はそのまま `Started::SpawnFailed(error)`、`stdout` の取得に失敗した場合は `kill_and_wait` してから `io::Error::other(..)` を包んだ `Started::SpawnFailed` になる。エラーが `.ok()?` で捨てられていない。
  - 手順11: `probe_holder` が `Started::SpawnFailed(error)` を `HolderCapability::ProgramUnusable(error)` にそのまま載せている。
- **確認ポイント:** 元の `io::Error` が「起動できませんでした」のような固定文言に置き換わっていないこと。理由がここ以外に出る場所は無い（`stderr` は `Stdio::null()` に捨てている）。`ProgramUnusable` がスキップ側に倒れていないこと（確認項目4 で見た `match` と併せて確認する）。**手順6 を飛ばさないこと** — 実行ビットが落ちたままだと以降の確認項目が同じ失敗を出し続ける（`git diff` には現れないので、確認項目10 の手順5 で実行ビットも見る）。

### 10. 一時的な差し替えが差分に残っていない

- **対応する受け入れ基準:** AC-7
- **目的:** 確認項目6・7・8・9 で入れた一時的な変更（`SIGNAL_DEADLINE` の書き換え・呼び出し回数カウンタ・成果物の削除・実行ビットの落とし）が、コミットする差分にも `target/` にも残っていないことを確認する。`SIGNAL_DEADLINE` を延ばすだけの対処は Issue 本文が明示的に否定しており、値が動いていれば区別を作った意味が薄れる。
- **手順:**
  1. `git diff`
  2. `git diff origin/main -- crates/pulsen/tests/common/lock.rs` を読み、`SIGNAL_DEADLINE` の**値**（`Duration::from_secs(10)`）が変わっていないことを見る
  3. `grep -n "SIGNAL_DEADLINE" -B 6 crates/pulsen/tests/common/lock.rs`
  4. `grep -rn "AtomicUsize\|from_nanos" crates/pulsen/tests/`
  5. `ls -l target/debug/examples/`
  6. `cargo test --workspace --locked --no-fail-fast -- --nocapture`
- **期待結果:**
  1. 一時的な差し込み（カウンタ・`from_nanos`）が1件も残っていない。
  2. `SIGNAL_DEADLINE` の値は変わらず、変わっているのは doc コメントだけ。
  3. doc コメントが、期限超過の意味が2つに分かれたこと（probe では能力の判定、probe 成立後は異常）を述べており、待ち続けない理由（フィクスチャのハングはテストの失敗より診断が難しい。ADR-060）が残っている。
  4. ヒット0件。
  5. `lock_holder` が存在し（確認項目8 で消したものが `--workspace` の実行で作り直されている）、実行ビットが戻っている（確認項目9 の `chmod 000` が残っていない）。
  6. 緑。
- **確認ポイント:** 保持プロセスが残っていないこと（`lock_holder` のプロセスが取り残されると、以降のケースがロックを取れない）。probe は自分が作った一時ディレクトリのロックを使い、判定後は成否によらず `kill` + `wait` する設計なので、正常なら残らない。

### 11. `cargo fmt` と CI と同じ形の `cargo clippy` が通る

- **対応する受け入れ基準:** AC-11
- **目的:** 新しく置いた型・バリアントの名前が既定の warn 級 lint に掛からないことを含めて、CI の fmt / clippy が緑になることを確認する。
- **手順:**
  1. `cargo fmt --all`
  2. `cargo fmt --all --check`
  3. `cargo clippy --workspace --all-targets --locked -- -D warnings`
  4. CI の clippy ステップと msrv ジョブのログを確認項目15 で併せて見る。
- **期待結果:**
  - 手順2: 差分なしで exit code 0。
  - 手順3: 警告0で exit code 0。特に `clippy::enum_variant_names` が `Started` の変種名（`SpawnFailed` を `NotStarted` にしていない）で発火しないこと。
- **確認ポイント:** `--all-targets` がテストターゲットも見るので、`crates/pulsen/tests/` に置いた新しい enum がここで初めて lint に掛かる。`-D warnings` が付いた形でしか発火しない差なので、`--all-targets` を省いた実行で代替しない。ローカルの rustfmt は nixpkgs 同梱で CI の現行 stable と版が違うため、手順2 が緑でも CI の fmt ジョブが赤になりうる。その場合の解消は CI の stable で `cargo fmt --all` を掛け直した差分をコミットすることであり、`rustfmt.toml` での抑止ではない。

### 12. HOOKS.md が新しい述語で揃っている

- **対応する受け入れ基準:** AC-8
- **目的:** 行 × フックの正本である `crates/pulsen-conformance/HOOKS.md` が、宣言の述語の変更に追随していることを確認する。表・前書き・失敗側の明記の3点が揃い、反対のことを述べる文が残っていないこと。
- **手順:**
  1. `grep -n "TC-port-exclusive-lock-002" crates/pulsen-conformance/HOOKS.md` で該当行を読む
  2. `grep -n "TC-002" crates/pulsen-conformance/HOOKS.md` で ExclusiveLock 節の前書きを読む
  3. `grep -n "実行ファイル" crates/pulsen-conformance/HOOKS.md`
  4. `grep -n "SIGNAL_DEADLINE\|holder_capability\|holder_program\|HolderCapability" crates/pulsen-conformance/HOOKS.md`
  5. `grep -n "example がビルドされ" crates/pulsen-conformance/HOOKS.md` で「3ランナーでの実測」の該当箇所を読む
- **期待結果:**
  1. 「前提を作れない環境」列が「保持プロセスの合図が期限内に返らない」の趣旨になっており、測っていない原因（初回起動のスキャン・高負荷など）の推定が添えられていない。「判定」列が「ハーネスが `hold_from_other_process` / `try_acquire_from_other_process` を提供するか（この適用先では、保持プロセスを1回起動して合図が期限内に返るかで決まる）」の形で、フック水準の書き方を保ったまま適用先での実態を括弧で補っている。3 OS の列は `実行` のまま。
  2. 前書きが「実行ファイルを要するため」ではなく「保持プロセスの合図が期限内に返らない環境では走らなくなりうる」の趣旨に改まり、実行ファイルが無い場合はスキップではなくケースの失敗になる旨が続いている。
  3. 実行ファイルの不在・起動の失敗が「前提を作れない環境」ではなく**ケースの失敗**であることが、表の直後か該当行の近くに1文で明記されている。旧い述語（不在 → 前提を作れない環境 → スキップ）を述べる文が文書内に1つも残っていない。
  4. **ヒット0件。** 適用側の定数名・関数名・型名が正本の表に入っていない（`crates/pulsen/tests/` の名前を変えるだけで表が古くなる形にしない。「判定」列だけでなく「前提を作れない環境」列にも同じ縛りが掛かる）。
  5. 区分 B の5件が3 OS で走った理由が「`--workspace` で example がビルドされ、ロックを保持する別プロセスが3 OS で機能した」のままで、この実測（`e524981` 時点の測定で、example を含まない）に「合図が期限内に返った」という解釈が足されていない。実測の値そのもの（run 番号・件数・OS 別の内訳）も書き換わっていない。
- **確認ポイント:** 手順4 が0件であることが、この文書を「別実装（in-memory 等）にも適用できるスイートの正本」として保つ条件。`permission_restrictions_effective` が判定列に書かれているのはスイート側（`crates/pulsen-conformance/src/`）にある関数だからで、事情が違う。同節の「測定したのは `e524981`…」と「その更新は #11 の責務とする」の古さは本Issueでは直さない（スコープ外）。

### 13. CI の why コメントが事実に合わせて改まっている

- **対応する受け入れ基準:** AC-13
- **目的:** `.github/workflows/ci.yml` の why コメントが述べる帰結（単一テストターゲット指定で何が起きるか）が、本Issueの変更後の事実に一致していることを確認する。あわせて、綴りを残さない縛りが守られていることを機械的に確かめる。
- **手順:**
  1. `grep -n "宣言済みスキップ" .github/workflows/ci.yml`
  2. `grep -n "workspace のまま" -B 8 -A 4 .github/workflows/ci.yml` で該当のコメント塊を読む
  3. `grep -n -- "--test " .github/workflows/ci.yml`
  4. `git diff origin/main -- .github/workflows/ci.yml`
- **期待結果:**
  1. **ヒット0件。** 「4件＋1件が『宣言済みスキップ』に化ける」が「4件＋1件が**失敗する**（実行ファイルの不在は環境の能力ではないので、`SkipBudget` の許容集合には入れない）」の趣旨に改まっている。
  2. 成立条件が1文添えられている — 単一のテストターゲットを指定した実行は example をビルドしないだけで過去の成果物を消しはしないため、失敗するのは `target/` にその成果物が残っていない場合である旨。`--workspace` のままにする理由は残っている。
  3. **ヒット0件。** 書き加えた文に単一ターゲット指定のコマンドの綴りが入っていない（日本語の言い換え「単一のテストターゲットを指定した実行」で通している）。
  4. 差分がこのコメント塊だけで、`run:` の実行内容（`cargo test --workspace --locked --no-fail-fast -- --nocapture 2>&1 | tee test.log` 等）が変わっていない。
- **確認ポイント:** 手順3 が0件であることが、「その道具を使っていないこと」を grep で確認できる状態を保つ条件。綴りが1語でもコメントに混ざると確認手段が壊れ、以後は目視で除外する運用になって、実際に使い始めたときの検出力が落ちる。この6行はまさに本Issueが語る帰結（スキップ→失敗）を述べる場所なので、書き手が綴りへ手を伸ばしやすい。

### 14. `.adr/073` が起票され、区別の理由が正本に残っている

- **対応する受け入れ基準:** AC-12
- **目的:** 「合図タイムアウト＝環境の能力・実行ファイル不在＝失敗」という区別の理由が、作業ログ（`.thread/13/adr.md`）ではなく正本（`.adr/`）に残り、コードから辿れることを確認する。
- **手順:**
  1. `ls .adr/ | tail -5`
  2. `grep -n "^## " .adr/073-holder-capability-skip-vs-fail.md`
  3. `grep -n "ProgramUnusable\|068" .adr/073-holder-capability-skip-vs-fail.md`
  4. `grep -n "073" crates/pulsen/tests/common/lock.rs crates/pulsen/tests/conformance_lock.rs`
  5. `grep -n "^### Status" -A 2 .thread/13/adr.md`
- **期待結果:**
  1. `073-holder-capability-skip-vs-fail.md` が1件あり、既存の最大採番 072 の次になっている（手順2・3 はこのファイル名を直接指す。glob で書くと、該当ファイルが無いときは展開されずにコマンド自体が失敗し、中身について何も読めないまま空振りする）。
  2. 見出しが `## ステータス` / `## コンテキスト` / `## 決定` / `## 検討した代替案` / `## 影響`（`.adr/038` の書式）で、ステータスが **承認済み**。
  3. `## 決定` に、能力側と失敗側を分ける基準（スキップにしたときに「なぜ走らなかったか」と「次に何をすればよいか」が宣言だけから定まるか）が書かれており、そこから `ProgramUnusable` を失敗側に置く理由が読める。`## 影響` に、`.adr/068` が挙げた帰結（単一テストターゲット指定でロック系が「宣言済みスキップ」に化ける）が「4件＋1件が失敗する」へ改まったことが、068 だけを読んだ人が辿れる形で書かれている。
  4. `lock.rs` の `PROGRAM_MISSING` の doc コメントと `conformance_lock.rs` の `allowed_skips()` の doc コメントの両方から 073 が参照されている。068 の参照も残っている。
  5. `.thread/13/adr.md` の各エントリの Status 行から、昇格済み（`→ .adr/073-... に昇格`）か作業ログ限りかが判別できる。ADR-001 / 002 / 003 / 005 / 006 が1本に畳まれて昇格し、ADR-004 / 007 は作業ログ限りである旨と理由が書かれている。
- **確認ポイント:** `.adr/068` そのものが書き換わっていないこと（`git diff origin/main -- .adr/068-*.md` が空。`.adr/` は判断が下された時点の記録で、現在どの述語が許容集合を決めているかは正本の `HOOKS.md` とコードが持つ）。073 が4本の別立てではなく1本になっていること（Issue コメントが求めているのは区別の理由1本で、別立てにすると正本で理由が分散する）。

### 15. 3 OS の CI で、`SKIP` 集合が実行前の予測と一致する

- **対応する受け入れ基準:** AC-10
- **目的:** 許容集合の述語そのものを動かした変更なので、3 OS の実測と実行前の予測を突き合わせて、宣言が正しいことを確かめる。突き合わせを飛ばすと、宣言が正しいかを誰も確かめていない状態になる。
- **手順:**
  1. **PR を上げる前に**、3 OS それぞれで期待する `SKIP` 集合を書き出す。3 OS の列（＝前回の実測）を写すのではなく、HOOKS.md の「環境で走らなくなりうる行」の判定列の述語が CI の環境で成立するかを1行ずつ見て導く。
     - `permission_restrictions_effective` — CI は非 root（unix ジョブは `id -u` を直接アサート）・コンテナ指定なしなので unix では成立（実行）、Windows は POSIX の権限操作が効かないので不成立（スキップ）。該当は適合8行（config-store-023 / workflow-store-030 / task-repository-005・011・012・019・035・041）と CLI 側2件（`tc_task_register_task_016` / `021`）の**計10件**。
     - ハーネスが `rewind` を提供するか（`tc_port_clock_005`）— `SystemClockHarness` は実時計を巻き戻せないので提供しない。3 OS とも**1件**。
     - ハーネスが `hold_from_other_process` / `try_acquire_from_other_process` を提供するか（本Issueで述語が変わる5件）— CI は `--workspace` なので example がビルドされ、probe は起動して合図を待つところまで進む。3ランナーとも保持プロセスが機能してきた実績があるので `Available` に倒れると見込む。3 OS とも**0件**（出ない）。
     - 残るフック提供系（`observe_wall_clock` / `unusable_lock` / `failing_manager` / `non_repo_dir`）と `tmpdir_outside_repository` — いずれも実行（0件）。
     - **合計の見込み:** unix（ubuntu / macOS）は `tc_port_clock_005` の**1件**、Windows はそれに権限系10件を加えた**11件**。
     - **数えないもの:** `pulsen-conformance` の lib ユニットテストが `SkipBudget` 自身の検証で出す架空の3行（`SKIP tc_port_clock_004_…` / `tc_port_clock_0051_…` / `tc_port_clock_005_…`）は全 OS の `test.log` に出るが、走らなかった適合ケースではないので集合に入れない。ジョブサマリー側は ci.yml がその区間を落とすので現れない。`crates/pulsen/src/adapter/task_repository.rs` の `#[cfg(all(test, unix))]` の3件は Windows でコンパイルされず `SKIP` としても現れないので差分にならない。
  2. PR を作成して CI を起動する。ワークフローは `push`（main）・`pull_request`・`workflow_dispatch` で起動するので、マージ前の実行手段は PR 一択。
  3. `gh pr checks <PR番号>` で7ジョブ（fmt 1 + test 3 OS + msrv 3 OS）の結果を取る。
  4. 各 OS の run のページを**ブラウザで開き**、ジョブサマリーの `SKIP` 行一覧を読む（サマリーは `gh run view` のログには出ない）。3 OS 分を並べて読む。
  5. 手順1 の予測と突き合わせる。`test.log` を目で見る場合は架空の3行を除いてから数える。
  6. msrv ジョブのログで `cargo "+$MSRV" build --workspace --all-targets --locked` が緑であることを確認する。
- **期待結果:**
  - 手順3: 7ジョブすべて `success`。
  - 手順4・5: unix は `tc_port_clock_005` の1件、Windows は11件。ロック系5件（`tc_port_exclusive_lock_002` / `003` / `004` / `005` と `tc_task_register_task_017`）が**3 OS のいずれにも現れない**。
  - 手順6: MSRV（`1.89`）で新しい enum を含むテストターゲットがコンパイル・リンクできる（`--all-targets` がテストと example も見る）。
- **実測（run 31683976608 / コミット `b344401`）:**
  - 手順3: 7ジョブ（fmt 1 + test 3 OS + msrv 3 OS）がすべて `success`。
  - 手順4: ジョブサマリーの `SKIP` 行は、ubuntu / macOS が `tc_port_clock_005` の**1件**、Windows がそれに権限系10件（`tc_port_config_store_023` / `tc_port_workflow_store_030` / `tc_port_task_repository_005・011・012・019・035・041` / `tc_task_register_task_016・021`）を加えた**11件**。ロック系5件（`tc_port_exclusive_lock_002` / `003` / `004` / `005` と `tc_task_register_task_017`）は3 OS とも現れない。架空の3行は手順1 の宣言どおり数えていない。
  - 手順5: 手順1 の予測（unix 1件 / Windows 11件 / ロック系5件は3 OS とも0件）と**一致**。述語が変わった唯一の行が3 OS とも `Available` に倒れており、宣言と実態が割れていない。一致したので HOOKS.md の3 OS 列は変更なし。
- **確認ポイント:**
  - **一致しなかった場合、観測値をそのまま期待値に書き写して閉じない。** 予測が誤っていた理由を先に特定してから HOOKS.md の3 OS 列を更新し、出典の run を「3ランナーでの実測」に書き足す。
  - ロック系5件が `SKIP` として現れた場合、本Issueで述語が変わった唯一の行なので**真っ先に疑う先**。probe が偽陽性で `SignalTimedOut` に倒れると許容集合が黙って広がり、5件が走らなくても緑になる（`.adr/068` が記録済みのトレードオフ。`SKIP` 一覧には現れる）。
  - ロック系5件が**失敗**として現れた場合は、CI で example がビルドされていない（`ProgramMissing`）か、probe 成立後にタイムアウトしている。パニックメッセージの本文で経路を切り分ける — 確認項目7・8 で見た2つのメッセージは文言が違う。
  - 手順1 の書き出しを PR 本文に残しておく（予測 → 実測 → 突き合わせの順序が守られたことの記録になる）。
  - CI の所要時間が probe の追加で目に見えて伸びていないこと（probe が走るのは `conformance_lock` と `cli_add_error` の2バイナリ）。

---

## エッジケース・異常系

### 1. probe が保持プロセスを取り残さない

- **目的:** probe が起動した保持プロセスが残ると、以降のケースがロックを取れず、本Issueとは無関係な失敗が出る。probe が判定後に成否によらず片付けていることを確認する。
- **手順:**
  1. `grep -n "fn kill_and_wait" -A 5 crates/pulsen/tests/common/lock.rs`
  2. `grep -n "kill_and_wait\|release(" crates/pulsen/tests/common/lock.rs`
  3. `grep -n "fn probe_holder" -A 16 crates/pulsen/tests/common/lock.rs`
  4. 確認項目5〜9 の各実行後に、`lock_holder` のプロセスが残っていないことを確認する。
- **期待結果:**
  - 手順1: `kill()` と `wait()` の結果をいずれも捨てる小さなヘルパーが1つある（既に終了している子への `kill()` はエラーになりうるが、目的は「残さないこと」だけで、どちらの結果もこの先の判断に使わない）。
  - 手順2: 呼び出し元が4経路5箇所 — `start_holder`（`stdout` の取得に失敗した経路・合図が期限内に返らなかった経路の2箇所）、`probe_holder`（判定後の後始末）、`spawn_holder` の `SignalUnreadable` 腕、`hold` の `!locked` 腕（後ろ2つはパニック直前の後始末）。どの経路も `release` のまま残っていない。
  - 手順2: `lock.rs` の中に `release`（stdin を閉じて正常終了を待つ）の呼び出しは無く、ヒットするのは定義行だけ。「`release` を使うのは正常に保持できたプロセスを畳むときだけ」という基準が掛かるのは probe と `lock.rs` の失敗経路の2つで、適合ハーネスの `try_acquire_from_other_process` が `locked` の値によらず `release` を呼ぶ経路は `.adr/073` が射程外と明示している。
  - 手順3: `Signaled` と `SignalUnreadable` の両方で `kill_and_wait` してから `HolderCapability::Available` を返している。
- **確認ポイント:** `release` の `wait()` には期限が無く、しかも probe は `OnceLock::get_or_init` の中なので、子プロセスが終了しない環境では `holder_capability()` を呼ぶ全スレッドがそこで止まる。probe が測っているのは「合図が期限内に返るか」だけで、正常終了できるかは測っていない — 測っていない性質に後始末を依存させないのがこの分けの理由。

### 2. ロックを使わないケースのスキップが probe を起こす

- **目的:** `cli_add_error` では、ロックと無関係なスキップ（権限系・git 系）が保持プロセスの起動を引き起こす。実害が無いこと（`ProgramMissing` でも probe はパニックせず能力を返す）と、待ちが入りうる条件を把握しておく。
- **手順:**
  1. `grep -n "fn allowed_skips" -A 22 crates/pulsen/tests/common/mod.rs` で、`holder_capability()` の呼び出しが `SKIPS`（`LazyLock`）の初期化経路にあることを読む。
  2. `grep -rn "common::skipped\|skipped(" crates/pulsen/tests/cli_add_error.rs`
  3. `cargo test -p pulsen --test cli_add_error -- --nocapture`（成果物がある状態＝確認項目5 の直後）
- **期待結果:**
  - 手順1: `allowed_skips()` は権限系・ロック系・git 系の3つの述語を評価し、そのうちロック系が `holder_capability()`（＝probe）を呼ぶ。最初に `common::skipped` を通るのが権限系（`tc_task_register_task_016` / `021`）や git 系（`036`）であっても、そこで probe が走る。
  - 手順3: 緑。probe が走っても結果は変わらない。
- **期待結果（挙動の理解として）:** Windows では権限系が必ずスキップされるため、`cli_add_error` は毎回 probe を走らせる。`tc_task_register_task_017` を含まない絞り込みで回した場合も probe は走るので、合図が期限内に返らない環境では `SIGNAL_DEADLINE` ぶん待つ — 待ちが入っても異常ではない。従来この経路は `holder_program()` のファイル存在確認だけで、I/O もプロセス起動も無かった。

### 3. `SkipBudget` の評価順が結果を変えない

- **目的:** `SkipBudget` は `LazyLock` で最初の `record` 時に許容集合を確定する。probe を先に置く形でこの順序問題を回避しているので、「タイムアウトを記録してから許容集合へ反映する」実装に後退していないことを確認する。
- **手順:**
  1. 確認項目6 の状態（`SIGNAL_DEADLINE` を `Duration::from_nanos(1)` にする）で `cargo test -p pulsen -- --nocapture` を**続けて2回**回す。
  2. `cargo test -p pulsen --test conformance_lock -- --nocapture` だけを回す（対象4件が並列に走る）。
  3. 一時変更を戻す（`git checkout crates/pulsen/tests/common/lock.rs`）。
- **期待結果:** 手順1 の2回とも同じ結果（緑・5件が `SKIP`）。手順2 も緑で、4件が `SKIP`。ケースの実行順や並列度で結果が変わらない。
- **確認ポイント:** 2件目以降のタイムアウトが「評価済みの集合に間に合わない」形になっていないこと（宣言を観測から導く実装では、そこが非決定的な赤になる）。1回でも赤が混ざったら、`allowed_skips()` が probe ではなく実行時の記録を見ていないかを疑う。

### 4. 成果物を消さずに `--test` 指定で回すと、確認自体が空振りする

- **目的:** 確認項目8 の手順1 を省いたときに何が起きるかを1回見ておく。以後この確認を再実行する人が同じ落とし穴を踏まないため。
- **手順:**
  1. `cargo test --workspace --locked --no-fail-fast -- --nocapture`（成果物を確実に作る）
  2. `cargo test -p pulsen --test conformance_lock -- --nocapture`（削除せずに実行）
- **期待結果:** 手順2 が**緑**で、4件が普通に通る。`ProgramMissing` の失敗も `SKIP` も出ない。
- **確認ポイント:** これが「単一テストターゲット指定は example をビルドしないが、既存の成果物を消しもしない」ことの実地の裏付けになる。CI の why コメントに書き加える成立条件（確認項目13 手順2）は、まさにこの事実を述べている。

---

## 既存機能への影響確認

- **CLI・ドメイン・アダプター・ユースケースへの影響:** なし。変更はテストのフィクスチャ層と表とワークフローのコメントに閉じており、`ExclusiveLock` ポートの契約も `FileExclusiveLock` の実装も変わらない。`git diff --name-only origin/main...HEAD` が `crates/pulsen/tests/` / `crates/pulsen-conformance/HOOKS.md` / `.github/workflows/ci.yml` / `.adr/` / `.thread/13/` の範囲に収まっていることで確認する。`crates/pulsen-conformance/src/` と `crates/pulsen/examples/lock_holder.rs` に差分が無いこと（`ExclusiveLockHarness` の trait 定義もフックのシグネチャも `SkipBudget` も変えない — フックが `Option` を返す契約は、別実装が同じスイートを適用できることの前提）。

- **開発者の実行習慣への影響（既知の非互換）:** `cargo test -p pulsen --test conformance_lock -- --nocapture` / `cargo test -p pulsen --test cli_add_error -- --nocapture` の単体実行が、`target/` に example の成果物が無い状態では「緑（スキップ）」から**5件の失敗**に変わる。意図した変更だが、単一テストターゲットで回す習慣を持つ開発者は初見で驚く。確認項目8 の手順5 で、失敗メッセージだけを読んで `cargo test --workspace` に切り替えればよいと分かることを見る。この非互換は PR 本文にも書き出す。

- **他の probe への影響:** `pulsen_conformance::permission_restrictions_effective()` と `common::git::tmpdir_outside_repository()`、およびそれらが判定する行（権限系10件・`tc_task_register_task_036` / `TC-port-worktree-manager-003`）の扱いは変えない。確認項目5 の実行で、これらの `SKIP` の出方が変更前と同じであることを見る（unix では権限系が0件、Windows では10件）。

- **`TC-port-exclusive-lock-004` の判定:** `try_acquire_from_other_process` を `hold` に寄せていないことが、このケースの判定を守る条件になる（確認項目2 手順4・5）。`hold` に寄せると `locked == false` がフィクスチャのパニックになり、「ドロップで解放され、別プロセスが取得できる」の判定をフィクスチャが先取りする。確認項目5 でこのケースが緑に走ることを見る。

- **`TC-port-exclusive-lock-006 / 007` への影響:** `separate_home` / `unusable_lock` は保持プロセスを使わないフックなので、能力の判定に影響されない。確認項目6（`SignalTimedOut` に倒した状態）でもこの2件が `SKIP` にならず走ることを見る — 5件だけがスキップになる形が保たれていることの裏付けになる。

- **`release()` の残存利用:** `conformance_lock.rs` の `release_holder` と `cli_add_error.rs` の正常系は、正常に保持できたプロセスを畳む経路なので `release` のままでよい。`try_acquire_from_other_process` は `locked` の値によらず `release` を呼ぶが、`.adr/073` が射程外と明示した経路である（取得に失敗した保持プロセスは即座に終了するので待ちは返る）。`grep -rn "release" crates/pulsen/tests/` で、`lock.rs` に `release` の呼び出しが無く、その失敗経路がすべて `kill_and_wait` になっていることを確認する。

- **後片付け:** 全項目の実行後に `git diff` が空で（`git status` に意図した変更ファイルだけが並び）、`cargo test --workspace --locked --no-fail-fast -- --nocapture` / `cargo fmt --all --check` / `cargo clippy --workspace --all-targets --locked -- -D warnings` の3つが緑に戻っていること。`target/debug/examples/lock_holder` が存在して実行ビットが戻っており、そのプロセスが残っていないこと。
