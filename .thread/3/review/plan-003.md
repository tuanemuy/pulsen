# 修正の実行計画 — PR #16 レビュー3周目

**判定内訳:** fix 15 / wont-fix 0 / defer 0 / 要確認 0（B-001 と W-001〜W-014 の全件）

**継承した判定:** W-004 / W-006 / W-009 / W-011 / W-012 は、ラウンド2 の同一 Key（`HOOKS.md / 正本性`、`.thread/3/plan.md / spec 差分`、`.thread/3/testing.md / 影響確認`、`.thread/3/plan.md / 手動確認の表`）を継承して `fix`。いずれも前ラウンドの修正が同じ箇所の一部しか消化していない残り。

## 単位の分け方

担当ファイルが重ならない単位に分けてある。**7単位すべてを並列に実行してよい。** ファイルの衝突は無いが、内容の整合が必要な箇所は各単位に確定した文言・決定として書き込んであるので、単位間で相談する必要は無い。

| 単位 | 指摘 | 触るファイル |
|---|---|---|
| 1. 報告文言 | W-001 | `crates/pulsen/src/cli/render.rs`、`crates/pulsen/tests/tick_notify.rs` |
| 2. 順序の主張と why コメント | W-002, W-003 | `crates/pulsen/src/application/tick/mod.rs`、`crates/pulsen-conformance/src/doubles/process.rs`、`crates/pulsen/tests/tick_observe.rs` |
| 3. Windows の昇格 | W-005 | `crates/pulsen/src/adapter/process.rs` |
| 4. 適合ハーネスの正本 | W-004 | `crates/pulsen-conformance/HOOKS.md` |
| 5. ADR | W-014（+ 単位2・3 の決定の反映） | `.thread/3/adr.md` |
| 6. 契約と手順書 | B-001, W-006, W-007（steps 側）, W-011（plan 側）, W-012（plan 側） | `.thread/3/plan.md`、`.thread/3/steps.md` |
| 7. 手動確認の記録 | W-007（testing 側）, W-008, W-009, W-010, W-011（testing 側）, W-012（testing 側）, W-013 | `.thread/3/testing.md` |

## 単位1 — `MissingCurrentAttempt` の報告文言を状態に依らない形にする

- **担当する指摘:** W-001（Domain W-001）
- **触るファイル:** `crates/pulsen/src/cli/render.rs`、`crates/pulsen/tests/tick_notify.rs`
- **修正の方針:**
  - `render.rs:311` の `"起動記録済みなのに現在 attempt が無い"` を、`transition.rs` の doc（「遷移の前提となる現在 attempt、またはその同定情報が失われている」）と同じ広さの文言へ直す。例: `"遷移の前提となる現在 attempt(または同定情報)が無い"`。launching に限定する語を残さない。
  - `render.rs:1244` のテスト期待（`遷移の前提が成立しません(...)` の末尾一致）を新しい文言に合わせる。
  - `tick_notify.rs` の `advance` 系テスト（`NotAgentRunStatus` だけを見ている `:477` 付近）に、**`current_attempt` を失った Completed タスクを `Branch::Advance` に通す** ケースを1本足す。主張するのは「`TickIssue::TransitionFailed`（`MissingCurrentAttempt`）が積まれ、`save` が起きないこと」と、報告文が実行状態と矛盾しないこと。これでこの経路の報告が初めてテストに載る。

## 単位2 — ポートをまたぐ順序を1本の列で主張し、`Freeze` の why を実在の呼び出し元へ寄せる

- **担当する指摘:** W-002（UseCase W-001）、W-003（UseCase W-002）
- **触るファイル:** `crates/pulsen/src/application/tick/mod.rs`、`crates/pulsen-conformance/src/doubles/process.rs`、`crates/pulsen/tests/tick_observe.rs`
- **修正の方針:**
  - **W-002:** `mod.rs:518-529` の `Freeze` の doc から Issue 番号による将来形（「#3 の catch-up 通知は…」）を落とし、現在の呼び出し元（`notify.rs` の `mark_notified` 後の保存が `Freeze::NotFrozen` を渡すこと）を指す形に書き換える。制約そのものは正しいので参照先だけを実在のコードへ寄せる。修正の経緯は書かない。
  - **W-003:** `ScriptedProcessController` に `RecordSeq` を足し、`try_kill_remnants` の呼び出しを採番する（`calls_in_order()` を追加。既存の `calls()` は採番を落とした形で据え置く — ADR-014 が定めた既存アクセサの扱いと同じ）。`tick_observe.rs:663-674` のテストを、`ScriptedTaskRepository` の `saved_in_order()` とマージして並べ直し、**`try_kill_remnants` → `save`** の順を1本の列として主張する形に書き換える（`tick_notify.rs` の `notify_steps` と同じ形）。`save` 先行の実装が赤になることが合格条件。
  - 採番を足すのは `try_kill_remnants` だけでよい（順序の契約がまたぐのはこのメソッドだけ。`starttime_of` / `kill` は「呼ばれない」「失敗しても書かない」しか主張しない）。この決定の ADR への反映は単位5 が行うので、この単位では `.thread/3/adr.md` を触らない。

## 単位3 — 昇格を持たないプラットフォームでは2段目を起動しない

- **担当する指摘:** W-005（Adapter W-002）
- **触るファイル:** `crates/pulsen/src/adapter/process.rs`
- **修正の方針:**
  - `terminate` モジュール（cfg で既に POSIX / Windows に割れている）に「このプラットフォームに昇格があるか」を宣言する定数を1つ置く（例: `pub const ESCALATES: bool` — POSIX は `true`、Windows は `false`。Windows の `command` が `Graceful` / `Forced` とも `taskkill /T /F` を返すことが根拠）。
  - `SystemProcessController::terminate`（`:186` 付近）を、`ESCALATES` が偽なら**2段目を起動せず** 1段目の終了ステータスの評価へ進む形にする。最終判定の `forced.status.success() || graceful.status.success()` は、昇格しない経路では `graceful` だけを見る形になる。
  - why として残すのは「同じ操作を2回起動しても得るものが無く、猶予ぶんロックを保持し、pid 再利用による誤殺の窓を2回開く」こと。修正の経緯は書かない。
  - 分岐は `terminate` モジュールの中に閉じるので、AC-7 の隔離（`util/atomic.rs` / `adapter/process.rs` / `adapter/task_repository.rs` の3ファイル）は動かない。`cargo clippy -- -D warnings` は両プラットフォームで通る形にする（`ESCALATES` が定数のため片側で到達不能分岐になる書き方を避け、実行時の `if` で書く）。
  - Windows 実機での確認は #10 の範囲なので行わない。POSIX 側の既存テスト（`-TERM` → `-KILL` の2段が起動されること）が緑のままであることを確認する。
  - ADR-015 への反映は単位5 が行うので、この単位では `.thread/3/adr.md` を触らない。

## 単位4 — HOOKS.md の「実行ファイルが無い」行に本スライスの11行を載せる

- **担当する指摘:** W-004（Adapter W-001）
- **触るファイル:** `crates/pulsen-conformance/HOOKS.md`
- **修正の方針:**
  - `:44`（`examples/agent_probe` を要する行）の対象に `TC-port-process-controller-007` と `011〜016` を足す。根拠: `007` は `terminated_pid` が `example_program("agent_probe")` を直に使い、`011〜016` は `spawn_unit` → `probe_command` 経由で使う。
  - `:45`（`examples/spawn_probe` を要する行）の対象に `011〜016` を足す。根拠: `spawn_unit` が `spawn_from_other_process` を通る。
  - 判定列の文言（「**スキップ許容集合には入れない** — 作り忘れを緑にしないため」）は変えない。適用先の `probe_execution_unit` が両実行ファイルの不在を `ProgramMissing` にし、`observation_allowed_skips` が空集合を返す実装と一致している。
  - 3ランナーの実測列は既存の規律どおり（新しく足す行は無く、既存行の対象IDを広げるだけなので実測列はそのまま）。

## 単位5 — ADR の Consequences と Decision を最終形に揃える

- **担当する指摘:** W-014（General W-008）。加えて単位2・単位3 が確定させた決定を同じファイルに反映する
- **触るファイル:** `.thread/3/adr.md`
- **修正の方針:**
  - **ADR-014（`:417` / `:424`）:** Consequences の「今回付けるのは `TaskRepository::save` と `CommandRunner::run` の2つに限り」を、**4つ**（`TaskRepository::save` / `TaskRepository::save_degraded` / `CommandRunner::run` / `ProcessController::try_kill_remnants`）に直す。`:417` の「下の『今回付けるのは2つ』はこの1つを加えた3つになる」という打ち消し文は落とし、Decision 本文が最初から `save_degraded` と `try_kill_remnants` を含む形に書き直す。`try_kill_remnants` を含める理由は「spec 手続きD 3.`DiedWithoutExit` が `try_kill_remnants` → `fail_run` → `save` を契約として定めており、順序の契約が `ProcessController` と `TaskRepository` をまたぐ」こと。判断基準「順序の契約が無いメソッドには付けない」はそのまま残す（`starttime_of` / `kill` には付けない）。
  - **ADR-015（`:458` 付近の Decision）:** 「猶予のうちに消えなければ強い終了へ昇格する」に、**昇格を持たないプラットフォームでは2段目を起動しない**ことを足す（Windows の `taskkill /T /F` は捕捉できる終了を持たず、同じ操作の再実行は猶予ぶんのロック保持と pid 再利用の窓を増やすだけ）。Consequences の「終了1回あたり最大 `TERMINATION_GRACE` × 2 = 4秒」も、昇格を持つプラットフォームに限る形に直す。
  - どちらも改訂の経緯（「ラウンド3 で直した」等）は書かず、現在の形が成り立つ理由だけを残す。

## 単位6 — 契約（plan.md）と手順書（steps.md）を実装の最終形へ追随させる

- **担当する指摘:** B-001、W-006、W-007（steps 側）、W-011（plan 側）、W-012（plan 側）
- **触るファイル:** `.thread/3/plan.md`、`.thread/3/steps.md`
- **修正の方針:**
  - **B-001（`steps.md:180`）:** ステップ8 の「各分類を『失敗を記録』『起動の結果が未確定』『スキップ』のどの見出しに振り分けるかを網羅 `match` で決める(`RunFailed` と `RemnantsUnhandled` は『失敗を記録』側)」を4見出しに直す — 「『失敗を記録』『起動の結果が未確定』『スキップ』『後始末が残っている』の4見出し（`RunFailed` は『失敗を記録』、`RemnantsUnhandled` は『後始末が残っている』。adr.md ADR-017）」。
  - **W-006（`plan.md:95-101` と `steps.md:259`）:** 「spec との差分として提起するもの」と、ステップ15 の提起内容に2件を足す。(a) `spec/domains/execution.md:109` は `classify_alive(...) -> RunningDecision` だが実装は `AliveDecision`（3値）を返し、2段規則の1段目（exit が Some なら即 `Judge`）はユースケース（`application/tick/observe.rs`）にある — `DOM-execution-017` の PASS 要件の字義とは配置が異なる（ADR-009）。(b) `spec/domains/execution.md:119` は `default_judgement(exit) -> JudgeOutcome` だが実装は2値の `DefaultJudgement` を返す（ADR-016）。いずれも `DOM-execution-004` / `008` / `019` の値数要件は満たすので、提起は spec 追従の依頼として書く。plan.md と steps.md の両方で同じ内容にする（AC-8 が名指しする正本が割れないこと）。
  - **W-007（`steps.md:108`, `:164`）:** `kill` / `try_kill_remnants` の典拠を「adr.md ADR-002 / ADR-015（ADR-015 が置き換えた3点 — 同定子は境界で parse して組み直す / 成否は消滅の観測で決める / 昇格の段 — を含む）」に改める。ADR-002 の本文は当時の記録として書き換えない方針なので、参照側に置き換えを明示する。
  - **W-011（`plan.md:138`）:** setup TC-38 の行を「手順1〜2。手順3(`abort` での片付け)は #5 — 代わりに放置する（判定コマンドの実体が不在で判定が成立しないため、判定上限超過で凍結して止まる）」に直す。`judge-exit` を 0 に戻す指示は落とす（`judge-missing.yaml` の `judge` は `/no/such/judge.sh` で `judge-exit` を読まない）。
  - **W-012（`plan.md:135`）:** setup TC-11 の行を「手順1〜4。手順5 は実行しない — 片付けの `set-status` が #5 で、回復（`judge-exit` を 0 に戻して completed へ進む筋）は setup TC-09 と同じ内容を確認項目1 で消化済み」に直す。実行と記帳（testing.md:915 の「TC-11(手順1〜4)」）に契約を揃える側で解く。

## 単位7 — 手動確認の記録を実測値と ADR に合わせる

- **担当する指摘:** W-007（testing 側）、W-008、W-009、W-010、W-011（testing 側）、W-012（testing 側）、W-013
- **触るファイル:** `.thread/3/testing.md`
- **修正の方針:**
  - **W-010（`:52`, `:55`）:** (1) `:55` の「現在は 4 / 12 / 1 件」を実測値に直す。**実測: baseline（`origin/main`）が `util/atomic.rs` 4 / `adapter/process.rs` 10 / `adapter/task_repository.rs` 1、本 PR 実装後が 4 / 20 / 1。** 「本スライスの実装で 10 件 → 20 件に増える」と書くか、括弧の件数を落として合否をファイル集合だけで述べる。(2) `:52` の `grep -n -A 3` を `-A 8` にする（実測で `crates/pulsen/Cargo.toml` の依存7行がすべて視野に入り、`pulsen-domain` 側は空判定のままで影響しない）。
  - **W-009（`:891`）:** 「`grep -rn 'command_runner()' crates/pulsen/src/cli/` のヒットが `cli/tick.rs` の1件だけ」を実測に合わせる。**実測は2件**（`cli/wire.rs:251` の定義と `cli/tick.rs:30` の呼び出し）。「ヒットは定義1件と呼び出し1件の計2件で、`cli/add.rs` に現れないこと」に直すか、grep を `grep -rn 'wire::command_runner()' crates/pulsen/src/cli/`（実測1件）に変えて期待を1件に落とす。意図（`add` の経路がランナーを要さない）は変えない。
  - **W-008（`:717`）:** 「見出しはタスクファイルに何を残したかで分かれる」を「見出しは**報告が何を残したか（運用者が次に取る行動）**で分かれる（ADR-017）」に直す。直後の「タスクファイルには何も書かれていない」との自己矛盾が消えることを確認する。
  - **W-007（`:692`）:** 確認ポイントの典拠 `ADR-002` を「ADR-002 / ADR-015」に改める（steps.md 側と同じ扱い。単位6 と文言を揃える必要は無く、置き換えた点へ導けばよい）。
  - **W-011（`:759`）:** 「手順3 の `abort` 片付けは #5 — 代わりに `judge-exit` を 0 に戻して completed で進めてから放置する」を「手順3 の `abort` 片付けは #5 — 代わりに放置する（判定コマンドの実体が不在なので `judge-exit` は効かず、判定上限超過で凍結して止まる）」に直す。同項目の確認ポイント（`:786` 付近の「以降 running のまま毎 tick 再判定され、上限超過で凍結する（放置してよい）」）と一致させる。
  - **W-012（`:510`）:** 「`setup.md` TC-11 手順1〜4（手順5 の `set-status` 以降は #5）」を「`setup.md` TC-11 手順1〜4（手順5 は実行しない。片付けの `set-status` が #5 で、回復は確認項目1 と同じ筋）」に直す。確認項目4 手順12 と記帳（`:915`）は現状のままでよい（手動確認は実施済みで、新しい手順は足さない）。
  - **W-013（`:73`）:** 「パスは各手順書の記載どおりに保つ」に例外を明記する — 「ただし `intervention.md` の `PMT`（`$HOME/pulsen-manual-test`）はフィクスチャB の `SETUP_HOME` と同一パスのため、フィクスチャC では `$HOME/pulsen-intervention-test` に読み替える。手順書どおりに戻すと、フィクスチャC 冒頭の `rm -rf "$PMT"` がフィクスチャB のホームを破壊する」。`:369` 側は現在の値（`$HOME/pulsen-intervention-test`）のままでよい。

## 完了の確認

- 単位1〜4 の完了後に `cargo build --workspace --locked` / `cargo test --workspace --locked` / `cargo clippy --workspace --all-targets --locked -- -D warnings` / `cargo fmt --all --check` を通す（AC-7）。
- 単位7 の W-009 / W-010 は、直した grep をその場で実行して記載と一致することを確かめる。
- どの単位でも、コードにもドキュメントにも指摘への弁明や修正の経緯を残さない（CLAUDE.md）。
