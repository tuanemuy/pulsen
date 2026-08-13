# 指摘台帳 — Issue #13 実装レビュー

判定は `fix`（このPRで直す）/ `wont-fix`（直さない）/ `defer`（別Issueへ送る）の3値。`元ID` はレイヤー別ファイル内の ID。

| Key | 初出 | 元ID | 判定 | 理由 | 再指摘 |
| --- | --- | --- | --- | --- | --- |
| adr-073-未起票 | 001 | docs B-001 | fix | steps.md ステップ10 / AC-12 の対象。コードと HOOKS.md が指す正本が実在しない状態を残さない | 0 |
| adr-status-未更新 | 001 | docs B-002 | fix | AC-12 が Status 行からの判別を明示的に要求している | 0 |
| 読み取りエラーを捨てる | 001 | test W-001 / type-design W-002 | fix | 同じ関数内で `spawn` の `io::Error` だけ保存し読み取り側は捨てる形になっており、基準が割れている | 0 |
| probe コメントの断定 | 001 | test W-002 | fix | コードが保証しない帰結でこの分岐を正当化している（CLAUDE.md「成り立つ理由だけを残す」） | 0 |
| HOOKS.md の典拠誤り | 001 | test W-003 | fix | `.adr/068` は「起動できない場合」を扱っていない。073 起票と同時に併記へ直す | 0 |
| 前提列の原因推定 | 001 | test W-004 | fix | 測っていない原因（初回起動のスキャン・高負荷）が正本に事実の形で載っている | 0 |
| probe の1回きり | 001 | test W-005 | wont-fix | 再試行しても偽陽性は消えず確率が下がるだけで、代償として「遅い」と判定済みの環境の待ちが倍になる。トレードオフは `.adr/068` と 073 に記録され `SKIP` 一覧にも現れる（1回でよい理由は 073 に書く） | 0 |
| ProgramUnusable の実地検証 | 001 | test W-006 | fix | unix の `chmod 000` で確定的に踏めることが実測済み。plan / steps / testing の「実地では踏まない」を事実に合わせる | 0 |
| SKIP 行の文言 | 001 | test W-007 | fix | ログがフック水準で書かれることを HOOKS.md に1文添える。ラベル自体は変えない（`deny_read` 等の呼び出し側の綴りと揃える約束を崩さず、文面生成はスコープ外） | 0 |
| Err(\_) が Disconnected を潰す | 001 | type-design W-001 / concurrency W-001 | fix | 潰した先が許容集合側で、このPRが引いた境界をワイルドカードが跨いでいる | 0 |
| Available がパスを公開 | 001 | type-design W-003 | fix | AC-2 の「コンパイラが保証する」が変種のペイロードで迂回できる | 0 |
| ProgramUnusable の String 化 | 001 | type-design W-004 | fix | `io::Error` は `'static` に置けるので文字列化の理由が成り立たず、`ErrorKind` を捨てている | 0 |
| 許容方針の複製 | 001 | type-design W-005 | wont-fix | AC-3 が2つの `allowed_skips()` に4区別の網羅を要求しているのは、変種が増えたとき両方の宣言側でコンパイラに判断を迫るため。述語1本に畳むとその問いが消える | 0 |
| try\_acquire の release | 001 | type-design W-006 | fix | 実装は妥当なので、`.adr/073` 側で「期限の無い待ちを許す相手」の射程を probe と `lock.rs` の失敗経路に明示して文言と実装を一致させる | 0 |
| 起動失敗の文言が同一 | 001 | type-design W-007 | fix | probe 時の観測か今回の起動かを読み分けられない。文言の追加だけで済む | 0 |
| Signaled の doc | 001 | type-design W-008 | fix | `locked == false` の実態（合図を書かずに終了）を doc が述べていない | 0 |
| probe の expect が毒す | 001 | concurrency W-002 | wont-fix | 一時ディレクトリを作れない環境は他のフィクスチャの `expect` でも同じ形で落ち、真の原因は1件目に出る。`ProgramUnusable` へ落とすと能力の型が誤った原因を名乗る | 0 |
| stderr を捨てる | 001 | concurrency W-003 | wont-fix | ADR-007 が piped を退けた判断を、未観測の環境クラスのために覆すだけの材料が無い。同 ADR が再検討の条件（区別が要る場面が出たとき）を既に記録している | 0 |
| release に期限が無い | 001 | concurrency W-004 | defer (#15) | 本PRが持ち込んだ構造ではなく、期限付きの子待ちヘルパーの新設と3 OS の検証が要る。`lock_holder` を触れない以上この経路の実地検証手段も無い | 0 |
| ADR-001 の前提誤り | 001 | concurrency W-005 | fix | probe は無負荷ではなくバイナリの混雑時に走る。W-005 を wont-fix にする根拠になる記述なので、正確でなければならない | 0 |
| 失敗側に倒す理由 | 001 | docs W-001 | fix | 正本とコードが、ADR-002 が明示的に否定した理由づけを述べている | 0 |
| SIGNAL\_DEADLINE の doc | 001 | docs W-002 | fix | 同一ファイル内で同じ状態に2つの述語が付き、testing.md が禁じた文言そのものが定数の doc に入っている | 0 |
| 実測に解釈を足した | 001 | docs W-003 | fix | example を含まない実測に「合図が期限内に返り」を重ねている。本PRが足した節だけ落とす | 0 |
| AC-10 の記録欠落 | 001 | docs W-004 | fix | 実測は一致している。予測 → 実測 → 突き合わせの順序（`.adr/068`）の記録を PR 本文に残す | 0 |
| grep の期待結果不足 | 001 | docs W-005 | fix | 手順どおり実行すると説明の付かないヒットが2件出る | 0 |
| 確認項目14 が実行不能 | 001 | docs W-006 | fix | B-001 / B-002 の解消と同時に実際に通す | 0 |

## defer の起票内容案

### release の期限（concurrency W-004 / W-017）

- **タイトル案:** ロック保持フィクスチャの `release` に期限が無く、保持プロセスが stdin の EOF で終了しない環境ではテストがハングする
- **本文の骨子:**
  - `crates/pulsen/tests/common/lock.rs` の `release` は stdin を閉じたあと期限なしに `wait()` する。呼び出し元は `conformance_lock.rs` の `release_holder` / `try_acquire_from_other_process` と `cli_add_error.rs` の正常系。
  - probe（`.adr/073`）が測るのは「合図が期限内に返るか」だけで、「stdin を閉じたら終了するか」は測っていない。起動も合図も速いが EOF 検出が壊れている環境は probe を通過し、`release` で止まる。libtest に per-test timeout が無いため、CI では出力の無いジョブタイムアウトになり、どのケースが原因かも分からない。
  - `.adr/060` の「フィクスチャのハングはテストの失敗より診断が難しい」と、Issue #13 で probe / 失敗経路に掛けた同じ基準の自然な延長。
  - 案: `try_wait` のポーリングか読み取りスレッドと同じ形で期限付きの終了待ちを置き、超えたら `kill_and_wait` へ落として `None` を返す。`release_holder` の `None` は TC-002 / 003 では `assert!` の失敗になり、`try_acquire_from_other_process` では結果に影響しないので、許容集合は広がらない。
  - 実地検証の手段は `crates/pulsen/examples/lock_holder.rs` 側（stdin の EOF を無視する保持プロセス）が要るので、その扱いも合わせて決める。
