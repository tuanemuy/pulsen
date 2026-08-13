# 型設計・アーキテクチャ整合

3周目。`triage.md` の判定（`wont-fix`: 1周目 test W-005 / type-design W-005 / concurrency W-002 / W-003、2周目 W-038、`defer`: concurrency W-004 / W-037 → Issue #15）は前提として扱い、再指摘しない。

**2周目の方針（型・分岐・後始末の構造を変えず doc とコメントだけを直す）は守られている。** `git diff b344401..0668dcc -- crates/` の実行される行の変化は `conformance_lock.rs` の `use` 統合（`hold` / `holder_capability` をフルパスから import へ、W-036 の手当て）だけで、呼ぶ関数も引数も同一。型・変種・`match` の腕・`kill_and_wait` の呼び出し箇所は1バイトも動いていない。`SIGNAL_DEADLINE` は `Duration::from_secs(10)`（AC-7）、`crates/pulsen-conformance/src/` は無変更、`cargo fmt --all --check` と `cargo clippy --workspace --all-targets --locked -- -D warnings` はローカルで通る（AC-11）。

下記は、2周目に doc / 正本の側だけ動かした結果として**同じ事実についての記述が2箇所で食い違って残った**箇所に絞る。

## Blockers

なし。

## Warnings

- **[W-001]** `HolderCapability::Available` の doc が、2周目に `.adr/073` へ明記した「`Available` を名乗る根拠は経路によって2つに分かれる」を映していない
  - 場所: `crates/pulsen/tests/common/lock.rs:28`（`/// 保持プロセスを起動でき、合図が期限内に返る。`）／`.adr/073-holder-capability-skip-vs-fail.md:39`
  - 理由: 2周目の手当て（W-025）は `probe_holder` の腕のコメント（`lock.rs:97-100`）と `Started::SignalUnreadable` の doc（`:57`）と 073 に入ったが、`HolderCapability` の変種そのものの doc は1周目の綴りのまま「合図が期限内に返る」と断定している。`RecvTimeoutError::Disconnected` 経由でこの変種になった場合、合図は観測していない。宣言側の2箇所（`common/mod.rs:49-54` / `conformance_lock.rs:111-116`）と `spawn_holder:163` が読むのはこの変種の doc であり、`probe_holder` 関数内のコメントはそこからは見えない。073 が「経路によって2つに分かれる」と書いた事実を、コードで最初に当たる場所だけが持っていない。
  - 提案: `:28` に1句足す（例: 「(probe が `Available` を名乗る根拠は、経路によって『合図が期限内に返った』と『期限の超過を観測しなかった』に分かれる。ADR-073)」）。型も分岐も動かさない。

- **[W-002]** `SIGNAL_DEADLINE` の doc が「probe 成立後の期限超過は異常」と断定したままで、2周目に `.adr/073` から外した断定がコード側に残っている
  - 場所: `crates/pulsen/tests/common/lock.rs:14-17`（`probe が成立したあとに超えた場合は異常として失敗になる`）／`.adr/073:41`／`.thread/13/testing.md:245`
  - 理由: W-030 の手当てで 073 は「probe が通った以上、この期限超過は環境の能力の宣言としては読めない … 一度きりの期限超過が異常だと言い切れるわけでもない。繰り返し起きるなら閾値の見直しとして扱う」に改まった。`spawn_holder` のパニック文言（`:178-181`「この環境で繰り返し起きるなら SIGNAL_DEADLINE を見直す」）も既にこの語気である。取り残されたのは定数の doc と、それを逐語で要求している testing.md 確認項目11 手順3（「probe 成立後は異常」）の2箇所だけで、W-030 が避けたかった読み方（赤が出たときに原因を狭く探す）はコードのすぐ横に残っている。
  - 提案: `:15` の「異常として失敗になる」を「能力の宣言としては読めないので失敗になる（繰り返すなら期限の見直し）」の水準へ。testing.md 確認項目11 手順3 の期待結果も同時に合わせる（この2つは同時に動かさないと確認項目が不合格になる）。

- **[W-003]** `.adr/073` の「probe の判定基準」の段落に、同じ ADR の4区分の振り分けと矛盾する一文が2周目に入った
  - 場所: `.adr/073:39`（`能力側へ倒すのは期限の超過を実際に観測したときだけで、それ以外はすべて能力ありとして扱い、異常は後続のケースの失敗に出す`）
  - 理由: 「能力側（＝スキップを許容する側）は期限超過だけ」は正しいが、後半の「それ以外はすべて能力あり」は `ProgramMissing` / `ProgramUnusable` に当てはまらない。この2つは期限超過ではないが `Available` でもなく、同じ ADR の16行上の4区分がまさに失敗側へ振り分けている。段落の実際の射程は「起動して合図待ちに入ったあとの結果」だが、文がその限定を述べていない。073 は「後から区別が増えたときも同じ問いで判断できる」ことを目的に置いた正本なので、5番目の区別を足す人がこの一文を規則として読むと、失敗側へ倒す判断そのものが消える。
  - 提案: 「起動して合図待ちに入ったあとは」の限定を1句加える。決定の内容は変えない。

- **[W-004]** 2周目に `steps.md` / `testing.md` へ書いた根拠が事実と食い違い、同じ2周目の HOOKS.md の変更とも噛み合っていない（観点越境だが、確かめられる事実の誤りなので挙げる）
  - 場所: `.thread/13/steps.md:308` / `.thread/13/testing.md:280` と `crates/pulsen-conformance/HOOKS.md:58`
  - 理由: 2つある。(a) 新しく書いた理由「この実測は `e524981` 時点の測定で example を含まないため」は成り立たない — `git ls-tree -r e524981` に `crates/pulsen/examples/lock_holder.rs` と `crates/pulsen-conformance/src/exclusive_lock.rs` の両方があり、守ろうとしている当の一文自身が「`--workspace` で example がビルドされ…3 OS で機能した」と述べている。plan.md スコープが HOOKS.md 47行目を「現状に対して古い」と名指ししているその記述を、根拠として書き写している。(b) 単位E がこの bullet を「そのままにする」と書いた一方、単位D は同じ bullet に「後者と同じ前提（保持プロセスの合図が期限内に返るか）を共有する CLI 側の受け入れケース TC-task-register-task-017 も、3 OS すべてで走った」を足した。testing.md 確認項目12 手順5 の期待結果（「合図が期限内に返った」という解釈が足されていない）を手順どおり実行すると、実物と読み合わせる人がどちらとも判定できる。
  - 提案: 据え置きの判断自体は妥当なので、理由を「実測の節には観測した事実だけを置く」に替えて 47行目の古さに寄りかからない形にし、確認項目12 手順5 の期待結果を単位D で実際に足した1文まで含めた形に直す。

## 観点別の確認結果

### 2周目の修正が実装の実態と一致しているか

- **`Started::SignalUnreadable` の doc（`:57-58`）:** 一致。`Ok(Err(error))`（子が読めない出力を返した）と `Disconnected`（読み取りが結果を返さずに終わった）の2経路がこの変種に入る形と、そのまま対応している。
- **`probe_holder` の腕のコメント（`:97-100`）:** 一致。「合図を観測しないまま `Available` に数える」は `Disconnected` の副経路にだけ掛かる形で書かれており、`Ok(Err)` 側（子は応答している）まで巻き込んでいない。「倒れる向きは失敗側で、後続のケースは `spawn_holder` でパニックする」も、同じ異常が続けば `SignalUnreadable` の腕（`:174-177`）でパニック、続かなければ正常に走るので、静かな緑にはならないという主張として成り立つ。
- **`Started::SpawnFailed` / `HolderCapability::ProgramUnusable` の doc（`:62` / `:35-36`）:** 一致。`stdout` の取得失敗（`:124-127`）が `kill_and_wait` を経て `SpawnFailed` に入り、probe が `ProgramUnusable` へ写す経路が、両方の doc から読める。パニック文言を据え置いた判断（`{error}` に「保持プロセスの標準出力を取得できない」が載るので文言全体では取り違えない）も、testing.md の逐語期待値と衝突しない形で成立している。なお 073:26 の「理由は起動が返した `io::Error` にしか無く」はこの合成エラーには厳密には掛からないが、振り分け（失敗側）も材料の残し方も変わらないので指摘には数えない。
- **`HolderCapability::SignalTimedOut` の doc（`:30-31`）:** 一致。`(環境の遅さ)` が落ちて「この観測は原因を決めず…区別しない」になり、073 の新しいトレードオフ（フィクスチャの退行も同じ腕に入る）と同じことを述べている。
- **`common/mod.rs:46-48` の追記:** 一致。基準と `.adr/073` への導線が `match` の直上にあり、複製を残した意図（変種が増えたとき両方の宣言側でコンパイラに判断を迫る）が、こちらのファイルだけを開いた人にも機能する形になった。「緑にせずケースの失敗にする」も実態どおり（`ProgramMissing` / `ProgramUnusable` では `tc_task_register_task_017` が `lock::hold` → `spawn_holder` のパニックで落ちる）。
- **`conformance_lock.rs:67-69` の追記:** 一致。「保持プロセスを起動できた環境では許容集合が空」は `allowed_skips()`（`:111-116`）が `Available` に対して `Vec::new()` を返すことと合っており、`kill_holder` の `None` がスイートの `require!` 経由で `SkipBudget` 違反＝失敗になる筋も正しい。挙動を変えなかった判断（TC-005 の未検証経路を動かさない）も妥当。
- **`conformance_lock.rs:9` の import 統合:** 一致。`HolderCapability` / `hold` / `holder_capability` / `release` / `spawn_holder` はいずれも使用されており、未使用 import も残っていない。

### 型の性質（1周目の修正が3周目時点でも保たれているか）

- **`HolderProgram` のカプセル化:** 保たれている。タプルフィールドも `path()` も `lock` モジュール限りで、外部から `HolderProgram` を構築する手段が無い以上 `HolderCapability::Available` を偽造して `spawn_holder` の判断を迂回することもできない（AC-2）。
- **`None` の一意性（AC-6）:** 保たれている。`lock.rs` で `None` を作るのは `spawn_holder:165`（`SignalTimedOut` の腕）だけで、`hold` は `?` の伝播、`hold_from_other_process` は委譲のみ。
- **`match` の網羅（AC-3）:** 差分に変種のワイルドカードは無い。`allowed_skips()` 2箇所・`probe_holder`・`spawn_holder`・`start_holder` の `recv_timeout` のいずれも4値ないし2値を明示的に列挙しており、`locked: _` / `error: _` はフィールド束縛であって `..` ではないため、フィールドが増えた場合も検知される。
- **パニックの使用:** 6経路すべてがフィクスチャの前提の破れ（不変条件違反）で、`Result` に載せ替えるべきものは無い。ドメイン層ではなくテストのフィクスチャであり、ADR-004 の「`Option` は宣言済みスキップかだけを運ぶ」と整合する。
- **後始末:** `kill_and_wait` の呼び出しは4経路5箇所（`:103` / `:125` / `:143` / `:175` / `:195`）、`release` の呼び出しは `lock.rs` 内に無し。073 が定めた射程（probe と `lock.rs` の失敗経路）と一致し、`try_acquire_from_other_process` が射程外であることも 073:49 に明記されている。
- **依存の向き:** `crates/pulsen-conformance/src/` は無変更で、能力の区別は適用側（`crates/pulsen/tests/`）に閉じている（073「probe の置き場所の基準」／HOOKS.md の判定列・前提列に適用側の名前が無いことも確認）。

### `.adr/073` / `.thread/13/adr.md` とコードの一致（2周目の追記分）

| 2周目に入った記述 | コードの実態 |
|---|---|
| 073「`Available` を名乗る根拠は経路によって2つに分かれる」 | 実装は一致。変種の doc だけが追随していない（W-001） |
| 073「probe が成立したあとのタイムアウトは失敗（一度きりが異常だと言い切れるわけでもない）」 | パニック文言は一致。定数の doc が旧い断定のまま（W-002） |
| 073「`SignalTimedOut` の腕にはフィクスチャの退行も入る」 | 一致（`HolderCapability::SignalTimedOut` の doc も同じことを述べている） |
| 073「期限の無い待ちが消えるのは `kill` が届く限りにおいて」 | 一致（`kill_and_wait` の `wait()` に期限は無い）。実装分は #15 の射程（`defer`） |
| 073「影響」に足した成立条件（`target/` に成果物が残っていないこと） | `ci.yml:137-145` の why コメントと同文の趣旨で一致（AC-13） |
| adr.md ADR-005 の条件づけ / ADR-007 の綴り（`SignalUnreadable { holder, error }`）とトレードオフ | いずれも実装と一致 |
| adr.md 各エントリの Status 行 | 昇格済み4件・作業ログ限り2件が判別できる（AC-12） |

## カバレッジ

- 確認: `crates/pulsen/tests/common/lock.rs`, `crates/pulsen/tests/common/mod.rs`, `crates/pulsen/tests/conformance_lock.rs`, `crates/pulsen-conformance/HOOKS.md`, `.github/workflows/ci.yml`, `.adr/073-holder-capability-skip-vs-fail.md`, `.thread/13/adr.md`, `.thread/13/plan.md`, `.thread/13/review/triage.md`, `.thread/13/review/fix-plan-002.md`, `.thread/13/review/review-002-type-design.md`
- 確認（部分）: `.thread/13/steps.md` — ステップ1〜3・6・9 を型の形と正本の実物に照合。手順の運用そのものは検証観点に委ねる
- 確認（部分）: `.thread/13/testing.md` — 確認項目5 / 11 / 12 / 15 とエッジケース1（型・後始末・doc の逐語期待値）だけを型設計の観点で照合
- スキップ: `.thread/13/review/fix-plan-001.md` — このレビューループ自身の成果物。1周目の修正結果はコードと `triage.md` で確認済み
- スキップ: `.thread/13/review/review-001.md` — 同上（1周目の統合サマリー）
- スキップ: `.thread/13/review/review-001-type-design.md` — 同上（1周目の同観点。指摘と判定は `triage.md` の元ID列で確認済み）
- スキップ: `.thread/13/review/review-001-concurrency.md` — 同上（1周目の並行性観点）
- スキップ: `.thread/13/review/review-001-docs.md` — 同上（1周目のドキュメント観点）
- スキップ: `.thread/13/review/review-001-test.md` — 同上（1周目のテスト観点）
- スキップ: `.thread/13/review/review-002.md` — 同上（2周目の統合サマリー）
- スキップ: `.thread/13/review/review-002-concurrency.md` — 同上（2周目の並行性観点）
- スキップ: `.thread/13/review/review-002-docs.md` — 同上（2周目のドキュメント観点）
- スキップ: `.thread/13/review/review-002-test.md` — 同上（2周目のテスト観点）
