# 修正計画 001 — Issue #13 / PR #14

判定は `.thread/13/review/triage.md`。fix 21件を5つの単位に束ねる。単位1〜4は担当ファイルが重ならないので並列に進められる。単位5は全体の実測なので最後に単独で走らせる。

**共通の前提:** 昇格する ADR のファイル名は `.adr/073-holder-capability-skip-vs-fail.md` に固定する。単位1〜3が同じ番号・同じ綴りを参照するため、起票を待たずに書いてよい。

## 単位1: コード — 能力の型と診断

- **担当する指摘:** W-001 / W-002 / W-008 / W-009 / W-010 / W-013 / W-014 / W-019（`lock.rs` 側）/ W-020 / B-001（コードからの導線）
- **触るファイル:** `crates/pulsen/tests/common/lock.rs`, `crates/pulsen/tests/conformance_lock.rs`

### 型の形（W-009 / W-010）

`HolderCapability` から実行ファイルのパスを取り出せなくし、`ProgramUnusable` は `io::Error` のまま持つ。

```rust
pub enum HolderCapability {
    Available(HolderProgram),
    SignalTimedOut,
    ProgramMissing,
    ProgramUnusable(io::Error),
}

/// 解決済みの実行ファイル。パスは `lock.rs` の外へ出さない。
pub struct HolderProgram(PathBuf);
```

- タプルフィールドを非公開にすることで、他のテストバイナリが `Available` からパスを取り出して `spawn_holder` の判断を迂回する経路が型で塞がる（AC-2 の「コンパイラが保証する」を変種のペイロードにも掛ける）。`Available` を単位変種にして private な `OnceLock<PathBuf>` に持たせる形でもよいが、どちらでもパスが `lock.rs` の外へ出ないことを条件にする。
- `probe_holder` は `Started::SpawnFailed(error)` を `HolderCapability::ProgramUnusable(error)` にそのまま載せる（`to_string()` を挟まない）。パニック文言は `{error}` で同じ内容になる。
- 2つの `allowed_skips()` の `match` は `Available(_)` / `ProgramUnusable(_)` のままコンパイルが通るので、`common/mod.rs` は変更しない（この単位の担当外）。

### 合図の読み取り（W-001 / W-008）

チャネルで `io::Result<String>` を送り、`recv_timeout` の失敗を2つに分ける。

```rust
enum Started {
    Signaled { holder: Child, locked: bool },
    SignalUnreadable { holder: Child, error: io::Error },
    SignalTimedOut,
    SpawnFailed(io::Error),
}
```

```rust
thread::spawn(move || {
    let mut signal = String::new();
    let read = BufReader::new(stdout).read_line(&mut signal).map(|_| signal);
    let _ = sender.send(read);
});
match receiver.recv_timeout(SIGNAL_DEADLINE) {
    Ok(Ok(signal)) => Started::Signaled { holder, locked: signal.trim() == LOCKED },
    Ok(Err(error)) => Started::SignalUnreadable { holder, error },
    Err(mpsc::RecvTimeoutError::Timeout) => { kill_and_wait(holder); Started::SignalTimedOut }
    Err(mpsc::RecvTimeoutError::Disconnected) => Started::SignalUnreadable {
        holder,
        error: io::Error::other("合図の読み取りが結果を返さずに終了した"),
    },
}
```

- `Disconnected` は期限を1ミリ秒も測っていないので、`SignalTimedOut`（＝許容集合側）に倒さない。
- `spawn_holder` の `SignalUnreadable` の腕は `kill_and_wait` のあと `{error}` を載せてパニックする。
- `probe_holder` は `Started::Signaled { holder, locked: _ } | Started::SignalUnreadable { holder, error: _ }` で従来どおり `Available` に畳む（判定基準は変えない）。

### 文言・doc（W-002 / W-013 / W-014 / W-019 / W-020 / B-001）

| 場所 | 直し方 |
|---|---|
| `SIGNAL_DEADLINE` の doc | 「この環境は保持プロセスを使えない」→「この環境は合図を期限内に返せない」。`SignalTimedOut` の doc と述語を揃える（W-020） |
| `PROGRAM_MISSING` の doc | 参照を `HOOKS.md / ADR-068 / ADR-073` に。理由の記述（環境の能力ではなくビルド構成の誤り）は `ProgramMissing` だけを指しているので正しく、変えない（B-001） |
| `Started::Signaled` の doc | 「期限内に子が応答した（合図を書いたか、書かずに終了したか）。`locked` は合図が `LOCKED` だったか」（W-014） |
| `probe_holder` の `Signaled \| SignalUnreadable` 腕のコメント | 「最初のケースで失敗として現れる」を落とす。probe が測るのは合図が期限内に返るかだけで、読めたかどうかは能力の判定に入れない（読み取りの異常はケース側で失敗になる）ことだけを述べる（W-002） |
| `spawn_holder` の doc | 「いずれも環境の能力ではないので」→ 基準の言葉に寄せる。実行ファイルが無い場合は原因も回避方法も一意なので、起動できない場合は理由が起動時のエラーにしかなく次の一手が宣言から定まらないので、いずれもスキップに逃がさず失敗させる（W-019） |
| `ProgramUnusable` の腕のパニック文言 | 「（probe の起動時に観測した理由）」の一句を足し、`SpawnFailed` の腕（今この場の失敗）と読み分けられるようにする（W-013） |
| `conformance_lock.rs` の `allowed_skips()` doc | 参照に ADR-073 を併記（068 は残す）。失敗側に倒す理由も上と同じ基準の言葉に揃える（B-001 / W-019） |

### 確認

`cargo fmt --all` / `cargo clippy --workspace --all-targets --locked -- -D warnings` / `cargo test --workspace --locked --no-fail-fast -- --nocapture` が緑で、ロック系5件の `SKIP` が0件であること。

## 単位2: 正本 — HOOKS.md

- **担当する指摘:** W-003 / W-004 / W-007 / W-019（HOOKS.md 側）/ W-021
- **触るファイル:** `crates/pulsen-conformance/HOOKS.md`

1. **43行「前提を作れない環境」列（W-004）:** 括弧の「（初回起動のスキャン・高負荷）」を落とすか、「（環境の遅さ。原因の内訳は測っていない）」に改める。この PR も過去の実測も原因を測っていない。判定列は現状のままでよい。
2. **45行の段落（W-003 / W-019）:** 理由を ADR-002 が立てた基準の言葉に寄せ、典拠を `.adr/068` と `.adr/073` の併記にする。文案:

   > 保持プロセスの実行ファイルが無い場合と、実行ファイルはあるが起動できない場合は、ここでいう「前提を作れない環境」には当たらない。前者は原因も回避方法も一意で、後者は理由が起動時のエラーにしか無く、いずれもスキップの宣言だけからは次の一手が定まらない。どちらもスキップにせずケースの失敗にする（`.adr/068-*.md` / `.adr/073-*.md`）。

3. **同じ段落に1文追加（W-007）:** ログの `SKIP` 行はスイートが書くためフック水準の文言（`ハーネスが … を提供しない`）になり、この適用先で実際に成立しなかった条件は表の「判定」列の括弧で読む、という趣旨を書く。`cli_add_error.rs` が渡すラベル（`lock::hold`）は変えない — `deny_read` / `git::is_outside_repository` と綴りの約束が揃っているため。
4. **56行（W-021）:** 本 PR が足した「起動した保持プロセスの合図が期限内に返り、」を落とし、`e524981` の実測が支える範囲に戻す。47行・59行の古さは plan.md のスコープ外なので触らない。

**確認:** `grep -n "SIGNAL_DEADLINE\|holder_capability\|holder_program\|HolderCapability" crates/pulsen-conformance/HOOKS.md` が0件のまま（AC-8 / ADR-003）。

## 単位3: `.adr/073` の起票と作業ログの片付け

- **担当する指摘:** B-001（起票）/ B-002 / W-012 / W-018（＋ W-005 を wont-fix にする根拠の記録）
- **触るファイル:** `.adr/073-holder-capability-skip-vs-fail.md`（新規）, `.thread/13/adr.md`

1. **昇格判定を `adr-guide.md` の記録基準にかける。** steps.md ステップ10 手順1 の出発点（ADR-001 / 002 / 003 / 005 / 006 を1本に畳んで昇格、ADR-004 / 007 は波及テストを満たさず作業ログ限り）を、実際に寿命テスト・波及テストで確かめてから確定する。
2. **`.adr/073-holder-capability-skip-vs-fail.md` を起票する。** 書式は `.adr/038`（`## ステータス` / `## コンテキスト` / `## 決定` / `## 検討した代替案` / `## 影響`、ステータスは **承認済み**）。含める内容:
   - 能力側と失敗側を分ける**基準**（スキップの宣言だけで「なぜ走らなかったか」と「次に何をすればよいか」が定まるか）と、その基準から `ProgramUnusable` が失敗側に落ちること。
   - probe を1度だけ評価し、宣言（`allowed_skips()`）と挙動（`spawn_holder`）の双方がそれを見る形。probe の判定基準は「合図が期限内に返ったか」に限り、probe 成立後のタイムアウトは失敗にすること。
   - probe の置き場所の基準（スイート側が判定できる能力か、適用側の具体的フィクスチャに依存する能力か）。
   - **期限の無い待ちを許す相手の射程（W-012）:** 「正常に保持できたと分かっている相手にだけ許す」を掛ける対象は probe と `lock.rs` の失敗経路であることを明示する。`try_acquire_from_other_process` の `release` はこの射程の外にあり、実装は変えない。
   - **トレードオフ（W-018 / W-005）:** probe が走るのは `OnceLock` の遅延評価によりテストバイナリが並列に走っている最中であり、無負荷の測定ではないこと。それでも1回きりにする理由（再試行は偽陽性を消さず確率を下げるだけで、代償として「遅い」と判定済みの環境の待ちが倍になる。退行は `SKIP` 一覧に現れ、機械的な歯止めが無いことは `.adr/068` が記録済み）。
   - **`## 影響`:** `.adr/068` が「決定」で挙げた帰結（単一テストターゲット指定でロック系が「宣言済みスキップ」に化ける）が「4件＋1件が失敗する」に改まったこと。068 だけを読んだ人が辿れる形で書く。`.adr/068` 自体は書き換えない。
3. **`.thread/13/adr.md` の Status 行を更新する（B-002）。** 昇格分は `→ .adr/073-holder-capability-skip-vs-fail.md に昇格`、残りは作業ログ限りである旨と理由（波及テストを満たさない）。
4. **ADR-001 の Consequences（W-018）** の「probe は無負荷・単発で1プロセスを起動して測る」を実態（バイナリ内で最も混んでいる瞬間に走る）に直す。ADR-006 のトレードオフ欄にある同趣旨の記述も揃える。
5. 単位1 が `ProgramUnusable(io::Error)` に変えるので、ADR-002 / ADR-007 の本文で `ProgramUnusable(String)` と綴っている箇所を現在の形に合わせる。

## 単位4: 計画・手順・検証ドキュメント

- **担当する指摘:** W-006 / W-010（testing.md の理由）/ W-023 / W-024（手順側）
- **触るファイル:** `.thread/13/plan.md`, `.thread/13/steps.md`, `.thread/13/testing.md`

1. **`ProgramUnusable` は実地で踏める（W-006）。** unix ではビルド済み `target/debug/examples/lock_holder` を `chmod 000` するだけで確定的に再現し、`Permission denied (os error 13)` がパニック文言に載ることをレビューが実測している。
   - `plan.md` テスト方針の該当行 — 「3 OS で安定して再現する手段が無いため実地では踏まない」を「unix では `chmod 000` で踏む。Windows は手段が無いのでコードレビューによる」に改める。
   - `steps.md` ステップ8-5 — 実地手順（`chmod 000` → 単一テストターゲット指定で実行 → `chmod 755` で戻す）に書き換える。
   - `testing.md` 確認項目9 — 表題と目的から「コードレビュー」限定を外し、unix 限定の実地手順と期待結果（メッセージに `spawn` が返したエラーの内容が載る）を足す。Windows 側だけコードレビューに限定する。
2. **testing.md 確認項目1・9 の期待結果を型の現在の形に合わせる（W-010）。** `Available(PathBuf)` → `Available(HolderProgram)`、`ProgramUnusable(String)` → `ProgramUnusable(io::Error)`。確認項目9 の「`Child` を持てない `'static` の型に、理由だけを文字列で移す」という説明は成り立たない（`io::Error` は `Send + Sync + 'static`）ので落とし、`ErrorKind` を保つ形になっていることを見る手順にする。確認項目1 の `Started` の変種も `SignalUnreadable { holder, error }` に更新する。
3. **確認項目1 手順3 の期待結果（W-023）:** `grep -rn "OnceLock\|LazyLock" crates/pulsen/tests/` は `common/git.rs` の `OUTSIDE`（`tmpdir_outside_repository` のキャッシュ）にもヒットする。期待結果に1行足して、説明の付かないヒットが残らないようにする。
4. 単位1 が `Err(_)` を2つに分けるので、`match` の網羅について触れている記述があれば現在の形に合わせる（`recv_timeout` の失敗が `Timeout` / `Disconnected` に分かれる）。

## 単位5: 実測と PR 本文

- **担当する指摘:** W-022 / W-024（実行）/ W-006（実測の記録）
- **触るファイル:** PR #14 本文（`gh pr edit`）
- **前提:** 単位1〜4 がすべて入ったあとに実施する。

1. **AC-10 の突き合わせを記録する（W-022）。** `.thread/13/steps.md` ステップ9 手順1 の予測（unix 1件 = `tc_port_clock_005` / Windows 11件 / ロック系5件は3 OS とも0件 / 架空3行は数えない）と、run 31681471522（7ジョブ success）の実測が一致したことを、出典の run 番号つきで PR 本文に書く。**予測を先に、実測を後に、突き合わせの結論を最後に**という順序（`.adr/068`）を本文でも保つ。「残り」節から AC-10 を外す。
2. **`ProgramUnusable` の実地検証を記録する（W-006）。** PR 本文の確認項目から「実地検証は持たず、コードレビューで確認」を外し、unix で `chmod 000` により `Permission denied (os error 13)` を確認した旨に改める（Windows は手段が無いのでコードレビュー、と限定を残す）。
3. **修正後の再検証。** 単位1〜4 の変更後に `cargo fmt --all --check` / `cargo clippy --workspace --all-targets --locked -- -D warnings` / `cargo test --workspace --locked --no-fail-fast -- --nocapture` と、4経路（`Available` / `SignalTimedOut` / probe 成立後のタイムアウト / `ProgramMissing`）＋ `ProgramUnusable`（unix）を踏み直す。一時的な差し替えが `git diff` に残っていないこと（AC-7）。
4. **testing.md 確認項目14 を実際に通す（W-024）。** 単位3 の起票後に手順1〜5 をそのまま実行し、`.adr/073` の見出し・ステータス・コードからの導線・`.thread/13/adr.md` の Status 行がすべて期待どおりであることを確認する。
5. CI を回し直し、3 OS の `SKIP` 集合が上記の予測と変わらないことを確認する（判定の述語は今回の修正で動かないので、集合も動かない見込み）。

## 単位をまたぐ注意

- 単位1〜4 は担当ファイルが重ならない。`.adr/073` の綴りだけが共有の前提で、これは本計画の冒頭で固定してある。
- 単位1 が型の形を変えるので、単位4 の testing.md 更新は本計画に書いた形（`Available(HolderProgram)` / `ProgramUnusable(io::Error)` / `SignalUnreadable { holder, error }`）を前提に書いてよい。単位1 が別の形（`Available` を単位変種にする）を選んだ場合だけ、単位5 の再検証時に突き合わせる。
- wont-fix にした W-005 / W-011 / W-015 / W-016 と defer にした W-017 は、どの単位でもコードを触らない。W-005 の「1回でよい理由」だけは単位3 が `.adr/073` に書く。
