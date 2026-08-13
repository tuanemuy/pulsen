# レビュー 005 — 最終確認（収束確認）

対象 PR: #14 / ベース: `origin/main` / HEAD: `7627d27`
契約: `.thread/13/plan.md`
既判定（`triage.md`）の `wont-fix` / `defer` は再指摘しない。
4周目以降のコード変更は無い（`git diff cb73567..HEAD --stat` の差分は `.thread/13/adr.md` / `steps.md` / `testing.md` と本ループ自身の成果物のみ。`crates/` / `.adr/` / `ci.yml` / `HOOKS.md` に差分なし）。

## 最終確認

### 4周目の指摘の解消確認

- **code W-001: 解消** — `.thread/13/adr.md:182`（ADR-006 Decision）は「温まったあとのタイムアウトは環境の遅さではなく異常として読める」から、正本 `.adr/073:41` とほぼ同文の「probe が通った以上、この期限超過は環境の能力の宣言としては読めない — 同じ手順が一度は期限内に返っている。…一度きりの期限超過が異常だと言い切れるわけでもない。繰り返し起きるなら閾値の見直しとして扱う」に書き換わった。Consequences の「良い点」も「『環境の能力』と『異常』の線」→「期限超過を『能力の宣言として読める』側と『読めない』側に分ける線」に改まり、同一エントリ内の噛み合わなさ（トレードオフ側の「一度だけ極端な負荷を踏むと赤になる」）も解消している。`.thread/13/steps.md:17` は「そこでのタイムアウトはスキップに逃がさずパニック」になった。`grep -rn "異常" .thread/13/ .adr/073-*.md crates/pulsen/tests/common/lock.rs .github/workflows/ci.yml crates/pulsen-conformance/HOOKS.md`（review/ を除く）の残りヒットは、いずれも別命題（読み取りの異常 / 機構の異常 / 「待ちが入っても異常ではない」）で、`SIGNAL_DEADLINE` の doc（`lock.rs:14-19`）・`spawn_holder` のパニック文言（`lock.rs:181-184`）の述語と揃っている。
- **docs W-001: 解消** — `--no-fail-fast` が `.thread/13/testing.md` の確認項目6 手順2（147行）・確認項目7 手順2（175行）・エッジケース3 手順1（386行）、`.thread/13/steps.md` のステップ8-1（327行）・8-2（328行）・8-3（341行）に入り、testing.md の実行環境節（41-49行）にコマンド表の更新と「複数のテストバイナリにまたがる5件を数える確認には `--no-fail-fast` が要る」という理由の1文が加わった。確認項目7 の確認ポイント（184行）にも打ち切りの帰結が明示されている。
  - **実測で確認。** 確認項目7 手順1 の差し込み（`start_holder` の `CALLS` カウンタ、`recv_timeout` に `deadline`）を入れて2通り回した。
    - `cargo test -p pulsen -- --nocapture` — `Running` 行 **4本**で打ち切り、失敗は `tc_task_register_task_017` の**1件**のみ、「probe は同じ手順で成立している」は**1回**。手順書どおりの期待結果（5件）は観測できない。
    - `cargo test -p pulsen --no-fail-fast -- --nocapture` — `Running` 行 **13本**、失敗は `tc_port_exclusive_lock_002 / 003 / 004 / 005` と `tc_task_register_task_017` の**5件**、当該メッセージ **5回**。本文は「保持プロセスの合図が 10s 以内に返らなかった。probe は同じ手順で成立している(この環境で繰り返し起きるなら SIGNAL_DEADLINE を見直す)」。ロック系5件は `SKIP` として現れず（出た `SKIP` は `tc_port_clock_005_巻き戻した時刻はそのまま返る` の1件のみ）。
    - 確認後 `git checkout crates/pulsen/tests/common/lock.rs` で復元し、`git status --porcelain` が空・`pgrep -fl lock_holder` が0件であることを確認した。
  - 手当ての射程も確認した。`.thread/13/` に残る `--no-fail-fast` 無しの `cargo test` は、いずれも単一テストターゲット指定（`--test conformance_lock` / `--test cli_add_error`）で、打ち切りの単位がターゲットである以上 libtest 内の全ケースは走るため期待結果を落とさない（確認項目8 / 9 / エッジケース2 / 3手順2 / 4、steps.md 8-4 / 8-5）。`plan.md:61-62` の2件は期待結果が**緑**なので打ち切りが起きない。塞ぐべき手順は確認項目7 と steps.md 8-3 のみで、そこは塞がれている。
- **AC-10 の出典** — `gh run view 31688076100` で `headSha` = `cb73567a1d7b631a172482d6ea31bfde1f41e54e`、`conclusion` = `success`、7ジョブ（fmt 1 + test 3 OS + msrv 3 OS）すべて success を確認。`cb73567` は `git log -- crates/ .adr/ .github/` の先頭で、最終のコード変更コミットである（`7627d27` は `.thread/13/` のみ）。run のログから `SKIP` 行を採ると、ubuntu / macOS は4行のうち3行が `SkipBudget` ユニットテストの架空行（`tc_port_clock_004_時刻の前進` / `tc_port_clock_0051_別のケース` / `tc_port_clock_005_時刻の巻き戻し`）で、実在は `tc_port_clock_005_巻き戻した時刻はそのまま返る` の**1件**。Windows はこれに権限系10件（`tc_port_config_store_023` / `tc_port_workflow_store_030` / `tc_port_task_repository_005・011・012・019・035・041` / `tc_task_register_task_016・021`）を加えた**11件**。ロック系5件は3 OS とも `SKIP` に現れず、いずれも `... ok` として実行されている（0件）。記述と一致する。
  - 予測 → 実測 → 突き合わせの順序（`.adr/068`）も保たれている。testing.md 確認項目15 は 手順1（予測、HOOKS.md の述語から導く形）→ 手順4（実測）→ 手順5（突き合わせ）の並びで、実測節は手順1 の予測を書き換えずに引用して「一致」と述べている。「一致しなかった場合、観測値をそのまま期待値に書き写して閉じない」の確認ポイントも残っている。実測が `gh run view --log` から採られた点は本文に明記され、手順5 が定めた「`test.log` を目で見る場合は架空の3行を除いてから数える」の規則を適用した旨も書かれているので、経路と数え方が食い違っていない。

### Blockers

なし

### Warnings

- **[W-001]** PR #14 本文の AC-10 の出典が旧 run のままで、`.thread/13/` 側と別の run を指している
  - 場所: PR #14 本文「3 OS CI の実測との突き合わせ」節（`run 31683976608（7ジョブすべて success）`）
  - 理由: `7627d27` は `.thread/13/steps.md:381` と `.thread/13/testing.md:340` の出典を run 31688076100 / コミット `cb73567` へ寄せたが、PR 本文だけ run 31683976608（コミット `b344401`、2周目の修正時点）のまま残っている。`triage.md` の「AC-10 の記録欠落」（1周目 docs W-004、判定 `fix`）は**予測 → 実測 → 突き合わせの記録を PR 本文に残す**ことを手当ての内容としており、PR 本文は AC-10 の記録の一部である。3周目の「AC-10 の出典が親コミットの run」（W-047、判定 `fix`）が「最終のコード変更コミットの run に寄せる」と決めた運用にも、PR 本文だけ従っていない。同じ AC-10 について2つの run が正の出典として並んでいる状態で、`b344401` は `cb73567` の `lock.rs` 変更を含まないため、PR 本文の出典は最終のコード状態を測ったものではない。1〜4周目に `fix` としてきた「正本は直り、追随先だけが旧記述を持つ」（W-042 / W-044 / 4周目 code W-001）と同一クラスの残り。
  - なお実測の結論そのものは変わらない（両 run とも7ジョブ success・予測と一致で、現 HEAD の `gh pr checks 14` も7ジョブ pass）。直すのは PR 本文の run 番号1箇所で足りる。

### 品質ゲート

- `cargo fmt --all --check` — 差分なし（exit 0）
- `cargo clippy --workspace --all-targets --locked -- -D warnings` — 警告0（exit 0）
- `cargo test --workspace --locked --no-fail-fast` — 全18本の `test result:` が `ok`、`failed` 0（exit 0）
- 参考: `gh pr checks 14`（HEAD `7627d27` / run 31689430031）は7ジョブすべて pass
- 検証で入れた一時変更は復元済み。`git status --porcelain` は空、`pgrep -fl lock_holder` は0件

### カバレッジ

- 確認: `/Users/hikaru/github.com/tuanemuy/pulsen_2/.thread/13/adr.md`, `/Users/hikaru/github.com/tuanemuy/pulsen_2/.thread/13/steps.md`, `/Users/hikaru/github.com/tuanemuy/pulsen_2/.thread/13/testing.md`, `/Users/hikaru/github.com/tuanemuy/pulsen_2/.thread/13/plan.md`, `/Users/hikaru/github.com/tuanemuy/pulsen_2/.thread/13/review/triage.md`, `/Users/hikaru/github.com/tuanemuy/pulsen_2/.thread/13/review/review-004-code.md`, `/Users/hikaru/github.com/tuanemuy/pulsen_2/.thread/13/review/review-004-docs.md`, `/Users/hikaru/github.com/tuanemuy/pulsen_2/.adr/073-holder-capability-skip-vs-fail.md`, `/Users/hikaru/github.com/tuanemuy/pulsen_2/crates/pulsen/tests/common/lock.rs`, `/Users/hikaru/github.com/tuanemuy/pulsen_2/.github/workflows/ci.yml`, `/Users/hikaru/github.com/tuanemuy/pulsen_2/crates/pulsen-conformance/HOOKS.md`, PR #14 本文, GitHub Actions run 31688076100 のログ
- 全面再レビューを行わなかった範囲（4周目以降 差分なし）: `/Users/hikaru/github.com/tuanemuy/pulsen_2/crates/pulsen/tests/common/mod.rs`, `/Users/hikaru/github.com/tuanemuy/pulsen_2/crates/pulsen/tests/conformance_lock.rs` — `git diff cb73567..HEAD` に現れず、4周目に確認済み
- スキップ: `/Users/hikaru/github.com/tuanemuy/pulsen_2/.thread/13/review/fix-plan-001.md`, `fix-plan-002.md`, `review-001*.md`, `review-002*.md`, `review-003*.md` — このレビューループ自身の成果物でカバレッジ対象外（`triage.md` のみ全文確認）

## 結論

Blockers 0 / Warnings 1。4周目の2件（code W-001 / docs W-001）はいずれも実物で解消を確認した。W-001 は PR 本文の出典1箇所のみで、コード・`.adr/073`・`ci.yml`・`HOOKS.md`・`.thread/13/` の記述と実物の間に新たな食い違いは見つからなかった。
