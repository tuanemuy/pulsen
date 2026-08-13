# レビュー4周目 — Test（フィクスチャ設計・スキップ判定）／型設計・アーキテクチャ整合

対象: PR #14 / base `origin/main` / HEAD `cb73567`
契約: `.thread/13/plan.md`
既判定（`triage.md`）の `wont-fix` / `defer` は再指摘しない。

## Test / 型設計

#### Blockers

なし。

#### Warnings

- **[W-001]** 2周目に `.adr/073` から、3周目に `SIGNAL_DEADLINE` の doc から落とした「温まったあとの期限超過は異常」という断定が、作業ログと手順書に残っている
  - 場所: `.thread/13/adr.md:182`（ADR-006 の Decision「失敗にする。温まったあとのタイムアウトは環境の遅さではなく異常として読める。」）／ `.thread/13/steps.md:17`（「`Available` なら起動して合図を待ち、そこでのタイムアウトは異常としてパニック」）
  - 理由: 正本 `.adr/073:41` は同じ命題を「probe が測るのは1プロセスの単発の起動で、本番の負荷そのものは測定条件に入っていない…**一度きりの期限超過が異常だと言い切れるわけでもない**。繰り返し起きるなら閾値の見直しとして扱う」と明示的に否定側へ書き換えている。コードもそれに揃っており、`SIGNAL_DEADLINE` の doc（3周目に修正）は「同じ手順が一度は期限内に返っている以上その超過を能力の宣言としては読めないので失敗になる（繰り返し起きるなら期限そのものの見直しとして扱う）」、`spawn_holder` のパニック文言も「probe は同じ手順で成立している（この環境で繰り返し起きるなら SIGNAL_DEADLINE を見直す）」で、いずれも「異常」と断じていない。`grep -n "異常として" .thread/13/ .adr/073-*.md crates/pulsen/tests/common/lock.rs` のヒットは上記2件だけで、この断定を残しているのは作業ログと手順書のみ。しかも ADR-006 は自身の Consequences（`.thread/13/adr.md:189`）で「probe が通ったあとに一度だけ極端な負荷を踏むと赤になる…この余地は残る」と述べており、同一エントリ内で 2周目の W-030（073 の 41行 vs 70行）と同じ噛み合わなさを再現している。ADR-006 の Status 行は読者を 073 へ送るが、送られる前に読む Decision 本文が正本と逆を述べている状態。判定の実装は正しいので実装を誤らせるものではないが、`triage.md` が 1〜3周目に `fix` としてきた「正本とコードは直り、作業ログだけが旧断定を持つ」（W-042 / W-044）と同一クラスの残りである
  - 提案: `.thread/13/adr.md:182` の1文を、`.adr/073:41` と同じ射程（probe が成立した以上その超過を能力の宣言としては読めない／一度きりで異常と断じるわけではない／繰り返すなら閾値の見直し）に合わせる。`steps.md:17` は「そこでのタイムアウトはスキップに逃がさずパニック」で足りる（同ファイルは 33行で「文面の最終形はコードを見る」と断っているので、断定語だけ落とせばよい）

## 検証の記録（指摘に至らなかった確認）

- **スキップ許容集合の導出。** `conformance_lock.rs:110-117` / `common/mod.rs:41-59` とも `SignalTimedOut` の腕だけが `LOCK_HOLDER_CASES` を足し、`Available` / `ProgramMissing` / `ProgramUnusable` はワイルドカードなしで明示的に空。フィクスチャの用意漏れ（example 未ビルド）は `holder_program()` が `None` → `ProgramMissing` → `spawn_holder` のパニックに落ち、緑にならない。example が壊れて合図を書かずに即終了する退行は `Started::Signaled { locked: false }` → `Available` → `hold` の `!locked` パニックで、やはり失敗側へ倒れる（合図を書かず生き続ける退行だけが `SignalTimedOut` に入り、これは `.adr/073:74` がトレードオフとして明記済み）。
- **probe と本番の経路一致。** 両者とも同じ `start_holder` を通り、probe は `HolderProgram` が保持する解決済みパスをそのまま本番へ渡す。`HolderProgram` のフィールドは非公開、`path()` も `common::lock` に private なので、能力の判定を迂回して起動する呼び出し側はコンパイラが弾く（AC-2）。`holder_program` の可視性を `grep -rn holder_program crates/pulsen/tests/` で確認、ヒットは定義と `probe_holder` の1呼び出しのみ。
- **型の過不足。** `HolderCapability` は4区分ちょうど、`Started` は private で4変種、`match` は `probe_holder` / `spawn_holder` / 2つの `allowed_skips()` のいずれもワイルドカードなしで網羅。`recv_timeout` の `Timeout` / `Disconnected` も分けられており `Err(_)` は無い。`Option` の `None` は `spawn_holder` の `SignalTimedOut` 腕1箇所と `hold` の `?` 伝播だけ（`grep -n None crates/pulsen/tests/common/lock.rs` のコード行は1件）。パニックはフィクスチャ層の前提破れに限られ、`ADR-004` の判断と一致。
- **3周目の doc 修正と実装の一致。**
  - `SIGNAL_DEADLINE`（14-19行）— `.adr/073:41` の言い回しと一致。実装は probe / 本番の双方で同じ定数を使い、双方とも待ち続けない。
  - `Available`（29-32行）— 「合図の期限の超過を観測しなかった」は `probe_holder` の2腕（`Signaled` / `SignalUnreadable`）を過不足なく覆う。挙げた2つの根拠の和集合が、合図あり／EOF で終了／読み取りエラー／`Disconnected` の全経路を覆っており、抜けは無い。`.adr/073:39` の言い方（「期限の超過を観測しなかった」）はより広い同値の言い換えで、矛盾ではない。
  - `kill_and_wait`（213-218行）— 「回収そのものには期限が無いので、`kill` を受けても即座には終われない相手に当たればここで止まる」は実装（`kill()` 後に無期限 `wait()`）と `.adr/073:47` の条件付き記述の双方に一致。#15 へ送った射程とも整合。
  - `.adr/073` の射程限定（39行「起動して合図待ちに入った子については…（起動そのものが成立しなかった場合の振り分けは上の4区分による）」）— `probe_holder` の分岐と一致。`ProgramMissing` / `ProgramUnusable` が `Available` に数えられる読みは消えている。
  - `.thread/13/adr.md` ADR-005 の Decision — `Disconnected` を `Available` に入れつつ「倒れる向きは失敗側」と述べる形で、コードと 073 に一致。
- **手順書の期待値と現物。** 3周目に直した grep 期待をすべて機械的に再実行し、一致を確認した — `OnceLock|LazyLock` 6行 / `holder_capability` 5行 / HOOKS.md の適用側名称 0件 / ci.yml の「宣言済みスキップ」1件（非 root ステップのみ）/ ci.yml の `--test ` 0件 / HOOKS.md の「example がビルドされ」1行 / `.thread/13/adr.md` の `### Status` 7件 / `lock.rs` と `conformance_lock.rs` からの 073 参照。
- **スコープ。** `crates/pulsen-conformance/src/` と `crates/pulsen/examples/lock_holder.rs` と `.adr/068` に差分なし。`SIGNAL_DEADLINE` の値は `Duration::from_secs(10)` のまま（AC-7）。`release` / `try_acquire_from_other_process` / `release_holder` は据え置きで、`.adr/073:49` が射程外と明示した範囲と一致。

## カバレッジ

- 確認: `crates/pulsen/tests/common/lock.rs`, `crates/pulsen/tests/common/mod.rs`, `crates/pulsen/tests/conformance_lock.rs`, `crates/pulsen-conformance/HOOKS.md`, `.github/workflows/ci.yml`, `.adr/073-holder-capability-skip-vs-fail.md`, `.thread/13/plan.md`, `.thread/13/steps.md`, `.thread/13/testing.md`, `.thread/13/adr.md`, `.thread/13/review/triage.md`
- 参照（差分外・整合の裏取りに使用）: `.adr/055`, `.adr/060`, `.adr/068`, `.adr/071`, `crates/pulsen/tests/cli_add_error.rs`, `CLAUDE.md`
- スキップ: `.thread/13/review/fix-plan-001.md`, `fix-plan-002.md`, `review-001*.md`, `review-002*.md`, `review-003*.md` — このレビューループ自身の成果物で、指示によりカバレッジ対象外（`triage.md` のみ全文確認）

## 結論

Blockers 0 / Warnings 1。W-001 は作業ログと手順書の記述が正本・コードと逆を述べている点のみで、実装・宣言・許容集合の導出には問題を見つけなかった。
