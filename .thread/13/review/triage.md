# 指摘台帳 — Issue #13 実装レビュー

判定は `fix`（このPRで直す）/ `wont-fix`（直さない）/ `defer`（別Issueへ送る）の3値。`元ID` はレイヤー別ファイル内の ID。

| Key | 初出 | 元ID | 判定 | 理由 | 再指摘 |
| --- | --- | --- | --- | --- | --- |
| adr-073-未起票 | 001 | docs B-001 | fix | steps.md ステップ10 / AC-12 の対象。コードと HOOKS.md が指す正本が実在しない状態を残さない | 0 |
| adr-status-未更新 | 001 | docs B-002 | fix | AC-12 が Status 行からの判別を明示的に要求している | 0 |
| 読み取りエラーを捨てる | 001 | test W-001 / type-design W-002 | fix | 同じ関数内で `spawn` の `io::Error` だけ保存し読み取り側は捨てる形になっており、基準が割れている | 2（002: W-027 / W-028） |
| probe コメントの断定 | 001 | test W-002 | fix | コードが保証しない帰結でこの分岐を正当化している（CLAUDE.md「成り立つ理由だけを残す」） | 0 |
| HOOKS.md の典拠誤り | 001 | test W-003 | fix | `.adr/068` は「起動できない場合」を扱っていない。073 起票と同時に併記へ直す | 0 |
| 前提列の原因推定 | 001 | test W-004 | fix | 測っていない原因（初回起動のスキャン・高負荷）が正本に事実の形で載っている | 1（002: W-027） |
| probe の1回きり | 001 | test W-005 | wont-fix | 再試行しても偽陽性は消えず確率が下がるだけで、代償として「遅い」と判定済みの環境の待ちが倍になる。トレードオフは `.adr/068` と 073 に記録され `SKIP` 一覧にも現れる（1回でよい理由は 073 に書く） | 0 |
| ProgramUnusable の実地検証 | 001 | test W-006 | fix | unix の `chmod 000` で確定的に踏めることが実測済み。plan / steps / testing の「実地では踏まない」を事実に合わせる | 1（002: W-028） |
| SKIP 行の文言 | 001 | test W-007 | fix | ログがフック水準で書かれることを HOOKS.md に1文添える。ラベル自体は変えない（`deny_read` 等の呼び出し側の綴りと揃える約束を崩さず、文面生成はスコープ外） | 0 |
| Err(\_) が Disconnected を潰す | 001 | type-design W-001 / concurrency W-001 | fix | 潰した先が許容集合側で、このPRが引いた境界をワイルドカードが跨いでいる | 0 |
| Available がパスを公開 | 001 | type-design W-003 | fix | AC-2 の「コンパイラが保証する」が変種のペイロードで迂回できる | 0 |
| ProgramUnusable の String 化 | 001 | type-design W-004 | fix | `io::Error` は `'static` に置けるので文字列化の理由が成り立たず、`ErrorKind` を捨てている | 0 |
| 許容方針の複製 | 001 | type-design W-005 | wont-fix | AC-3 が2つの `allowed_skips()` に4区別の網羅を要求しているのは、変種が増えたとき両方の宣言側でコンパイラに判断を迫るため。述語1本に畳むとその問いが消える | 0 |
| try\_acquire の release | 001 | type-design W-006 | fix | 実装は妥当なので、`.adr/073` 側で「期限の無い待ちを許す相手」の射程を probe と `lock.rs` の失敗経路に明示して文言と実装を一致させる | 1（002: W-027） |
| 起動失敗の文言が同一 | 001 | type-design W-007 | fix | probe 時の観測か今回の起動かを読み分けられない。文言の追加だけで済む | 0 |
| Signaled の doc | 001 | type-design W-008 | fix | `locked == false` の実態（合図を書かずに終了）を doc が述べていない | 0 |
| probe の expect が毒す | 001 | concurrency W-002 | wont-fix | 一時ディレクトリを作れない環境は他のフィクスチャの `expect` でも同じ形で落ち、真の原因は1件目に出る。`ProgramUnusable` へ落とすと能力の型が誤った原因を名乗る | 0 |
| stderr を捨てる | 001 | concurrency W-003 | wont-fix | ADR-007 が piped を退けた判断を、未観測の環境クラスのために覆すだけの材料が無い。同 ADR が再検討の条件（区別が要る場面が出たとき）を既に記録している | 0 |
| release に期限が無い | 001 | concurrency W-004 | defer (#15) | 本PRが持ち込んだ構造ではなく、期限付きの子待ちヘルパーの新設と3 OS の検証が要る。`lock_holder` を触れない以上この経路の実地検証手段も無い | 0 |
| ADR-001 の前提誤り | 001 | concurrency W-005 | fix | probe は無負荷ではなくバイナリの混雑時に走る。W-005 を wont-fix にする根拠になる記述なので、正確でなければならない | 0 |
| 失敗側に倒す理由 | 001 | docs W-001 | fix | 正本とコードが、ADR-002 が明示的に否定した理由づけを述べている | 0 |
| SIGNAL\_DEADLINE の doc | 001 | docs W-002 | fix | 同一ファイル内で同じ状態に2つの述語が付き、testing.md が禁じた文言そのものが定数の doc に入っている | 0 |
| 実測に解釈を足した | 001 | docs W-003 | fix | example を含まない実測に「合図が期限内に返り」を重ねている。本PRが足した節だけ落とす | 1（002: W-027） |
| AC-10 の記録欠落 | 001 | docs W-004 | fix | 実測は一致している。予測 → 実測 → 突き合わせの順序（`.adr/068`）の記録を PR 本文に残す | 0 |
| grep の期待結果不足 | 001 | docs W-005 | fix | 手順どおり実行すると説明の付かないヒットが2件出る | 0 |
| 確認項目14 が実行不能 | 001 | docs W-006 | fix | B-001 / B-002 の解消と同時に実際に通す | 0 |
| probe が Disconnected を Available に合流させる | 002 | W-025（type-design W-001 / concurrency W-003 / docs W-003） | fix | `Disconnected` を失敗側へ倒した理由（期限を測っていない）が、寄せ先 `SignalUnreadable` の probe での扱い（`Available`）と噛み合っていない。倒れる向きは失敗側なので直すのは型ではなく doc と 073 の射程 | 0 |
| AC-10 の出典が旧 run | 002 | W-026（test W-005 / docs W-004） | fix | 出典 run 31681471522 は修正前 `aacc9af`。`start_holder` の腕を書き換えた後の HEAD `b344401` の run 31683976608（7ジョブ success・予測と一致）へ差し替える | 0 |
| 073 の成立条件が無条件 | 002 | W-029（docs W-001） | fix | AC-13 が ci.yml に書かせた成立条件（`target/` に成果物が残っていないこと）が、コードの doc が指す正本の側だけ無条件のまま | 0 |
| 073 内で理由が噛み合わない | 002 | W-030（test W-002） | fix | 41行「温まったあとの期限超過は異常」が、70行「相殺できるのは cold-start 由来だけ」に掘り崩されている。赤が出たときに読み手が誤って狭く探す | 0 |
| SignalTimedOut にフィクスチャの退行が入る | 002 | W-031（test W-001） | fix | 構造的なトレードオフで実装では塞げない（probe は本番と同じ手順を踏む）。正本が原因を「環境の遅さ」に決め打っている点だけを直す | 0 |
| 017 の読み替え先が無い | 002 | W-032（test W-006 / docs W-005） | fix | 1周目 W-007 の手当てで入れた1文の射程が実物より広く、名指しの対象だった `tc_task_register_task_017` だけ意味を引けない | 0 |
| 合成 io::Error が ProgramUnusable を名乗る | 002 | W-033（type-design W-002） | fix | `stdout` 取得失敗は `ErrorKind` を材料に残すという 073 の理由づけを持たない。到達確率は極小なので型は増やさず doc だけ揃える | 0 |
| common/mod.rs の宣言に導線が無い | 002 | W-034（type-design W-003） | fix | 複製を残した意図（両側の宣言でコンパイラに判断を迫る）は、迫られた側がその場で判断できて初めて機能する | 0 |
| kill\_holder の None が異常を畳む | 002 | W-035（type-design W-004） | fix | `None` の意味を1つに絞ったこの impl で唯一残った例外。挙動は変えずコメントで明示する（変えると本PRが実地検証していない TC-005 の経路が動く） | 0 |
| use とフルパスの混在 | 002 | W-036（type-design W-006） | fix | 判断の源が1点であることを型で示した直後のファイルで、由来が違って見える。1行で済む | 0 |
| kill\_and\_wait に期限が無い | 002 | W-037（concurrency W-001） | defer (#15) | 期限付き子待ちヘルパーの新設と3 OS 検証が要り、#15 の起票案が `kill_and_wait` を安全な終着点として前提にしているので同じ Issue で扱う。正本側の断定を条件つきに直す分だけ本PRで直す | 0 |
| thread::spawn のパニックで Child が残る | 002 | W-038（concurrency W-002） | wont-fix | OS がスレッドを作れない状況では libtest の並列実行自体が成立しておらず、この1経路を塞いでも診断可能性は上がらない。再現手段が無く、`SpawnFailed` に寄せれば W-033 と同じ名前と観測のずれを増やす | 0 |

2周目の W-027（手順書が1周目の fix に追随していない）と W-028（作業ログの綴りとトレードオフが現状と食い違う）は、いずれも1周目に `fix` と判定した Key の修正が正本にだけ入り、`.thread/13/` 側へ波及していなかったもの。新しい行を立てず、該当 Key の再指摘に数えた（「前提列の原因推定」「実測に解釈を足した」「try\_acquire の release」「読み取りエラーを捨てる」「ProgramUnusable の実地検証」）。直す箇所の一覧は `fix-plan-002.md` の単位E / 単位F にある。

## defer の起票内容案

### release の期限（concurrency W-004 / W-017）

- **タイトル案:** ロック保持フィクスチャの `release` に期限が無く、保持プロセスが stdin の EOF で終了しない環境ではテストがハングする
- **本文の骨子:**
  - `crates/pulsen/tests/common/lock.rs` の `release` は stdin を閉じたあと期限なしに `wait()` する。呼び出し元は `conformance_lock.rs` の `release_holder` / `try_acquire_from_other_process` と `cli_add_error.rs` の正常系。
  - probe（`.adr/073`）が測るのは「合図が期限内に返るか」だけで、「stdin を閉じたら終了するか」は測っていない。起動も合図も速いが EOF 検出が壊れている環境は probe を通過し、`release` で止まる。libtest に per-test timeout が無いため、CI では出力の無いジョブタイムアウトになり、どのケースが原因かも分からない。
  - `.adr/060` の「フィクスチャのハングはテストの失敗より診断が難しい」と、Issue #13 で probe / 失敗経路に掛けた同じ基準の自然な延長。
  - 案: `try_wait` のポーリングか読み取りスレッドと同じ形で期限付きの終了待ちを置き、超えたら `kill_and_wait` へ落として `None` を返す。`release_holder` の `None` は TC-002 / 003 では `assert!` の失敗になり、`try_acquire_from_other_process` では結果に影響しないので、許容集合は広がらない。
  - 実地検証の手段は `crates/pulsen/examples/lock_holder.rs` 側（stdin の EOF を無視する保持プロセス）が要るので、その扱いも合わせて決める。

### `kill_and_wait` の期限（W-037 / concurrency W-001）— Issue #15 へのコメント追記案

新規 Issue は立てない。#15 の本文が「超えたら `kill_and_wait` へ落として `None` を返す」と `kill_and_wait` を期限つき待ちの安全な終着点として前提にしており、その前提自体がここで崩れるため、同じ Issue の射程に含めるのが正しい。

- **コメントの骨子:**
  - `kill_and_wait`（`crates/pulsen/tests/common/lock.rs`）の `wait()` にも期限が無い。この Issue が新設する期限付き子待ちヘルパーの射程に、`release` だけでなく `kill_and_wait` も含める。本文の案「超えたら `kill_and_wait` へ落として `None` を返す」は、`kill_and_wait` 自体が返ることを前提にしている。
  - `kill` が効かない子に当たると止まる。`kill_and_wait` を踏む主経路は `SignalTimedOut`、すなわち子が期限内に合図を返せなかった経路であり、その原因の1つがロック取得の syscall で刺さっていること（ネットワークファイルシステム上の `flock` / `LockFileEx`）である。「合図が返らない」と「SIGKILL が届いても即座には終われない」は相関する。
  - blast radius は `release` より広い。`kill_and_wait` は probe から `OnceLock::get_or_init` の中で走るため、そこで止まると `holder_capability()` を待つ全スレッドが無出力で停止する。`common/mod.rs` の `LazyLock<SkipBudget>` 初期化の中で止まった場合は、ロックと無関係なケースのスキップ記録まで巻き込む。
  - 案: `try_wait` のポーリングで期限を超えたら `Child` を諦めて捨て、その事実を記録する（残った子は一時ディレクトリのロックだけを掴んでいる）。
- **本PRで直す分:** `.adr/073` の「期限の無い待ちは、正常に保持できたと分かっている相手にだけ許す」と `.thread/13/adr.md` ADR-005 の同じ断定を、「`kill` が届く限りにおいて期限の無い待ちにならない」と条件つきにする（fix-plan-002 の単位A / 単位F）。正本が無条件の性質として述べている状態を残さない。

## 3周目

| Key | 初出 | 元ID | 判定 | 理由 | 再指摘 |
| --- | --- | --- | --- | --- | --- |
| Available の doc が断定のまま | 003 | W-039（type-design W-001 / docs W-001） | fix | 宣言側と `spawn_holder` が読むのはこの doc で、probe 内のコメントはそこから見えない | 0 |
| 073 の「それ以外はすべて能力あり」に射程が無い | 003 | W-040（type-design W-003 / docs W-002） | fix | 文字どおりには `ProgramMissing` / `ProgramUnusable` も `Available` に数える形になり、同じ「決定」の4区分と逆になる | 0 |
| 手順書が use 統一に追随していない | 003 | W-041（test W-001 / docs W-004 / docs W-006） | fix | 手順どおり実行すると不合格になる。1・2周目と同じ再発形 | 0 |
| 作業ログの旧断定 | 003 | W-042（test W-002） | fix | `.adr/073` とコードは直っており作業ログだけ `Disconnected` の扱いと食い違う | 0 |
| kill_and_wait の doc が逆に読める | 003 | W-043（test W-003） | fix | 「待たない」が本文の無期限 `wait()` とも 073 の条件つきの記述とも逆。実装は #15 の射程 | 0 |
| SIGNAL_DEADLINE の doc に旧断定が残る | 003 | W-044（type-design W-002） | fix | 2周目に 073 から外した断定が定数の doc に残り、パニック文言の語気とも割れている | 0 |
| 確認項目13 の期待が実物と不一致 | 003 | W-045（docs W-003） | fix | 手順どおり AC-13 を検証すると未達に見える | 0 |
| e524981 の根拠が事実誤り | 003 | W-046（type-design W-004） | fix | `git ls-tree e524981` に example がある。2周目に足した根拠が成り立たない | 0 |
| AC-10 の出典が親コミットの run | 003 | W-047（test / docs W-005） | fix | 最終のコード変更コミットの run に寄せる（`.thread/10` の運用に揃える） | 0 |
