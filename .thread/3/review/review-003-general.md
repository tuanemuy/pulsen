### General Review（計画ドキュメント）

#### Blockers

- **[B-001]** `steps.md` ステップ8 が報告の見出しを3分類のままにしており、ADR-017 と実装（4分類）と食い違う
  - 場所: `.thread/3/steps.md:180`
  - 理由: 該当箇所は「各分類を『失敗を記録』『起動の結果が未確定』『スキップ』のどの見出しに振り分けるかを網羅 `match` で決める(`RunFailed` と `RemnantsUnhandled` は**「失敗を記録」側**)」と書いている。実装は `crates/pulsen/src/cli/render.rs` の `IssueOutcome` が4分類（`Recorded` / `LaunchUnsettled` / `Skipped` / `CleanupLeft`）で、`RemnantsUnhandled` は**単独で第4の見出し「後始末が残っている」へ**振られている。これは `adr.md` ADR-017 が明示的に決めた形（`.thread/3/adr.md:500-518`）であり、`plan.md`・`testing.md:717,893` は4見出しで揃っている。steps.md だけが round-2 以前の姿で残っている。
  - この差は「どの見出しに出るか = 運用者が次に取る行動」を決める事実で、steps.md は後続 Issue（#4 / #5 / #6）が新しい `TickIssue` 変種を足すときに読む唯一の手順書である。ここが3分類のままだと、#5 が `abort` 由来の分類を足すときに `CleanupLeft` の存在ごと落ちる。
  - 提案: ステップ8 の当該文を「『失敗を記録』『起動の結果が未確定』『スキップ』『後始末が残っている』の4見出し（`RunFailed` は『失敗を記録』、`RemnantsUnhandled` は『後始末が残っている』。adr.md ADR-017）」に直す。

#### Warnings

- **[W-001]** `steps.md` / `testing.md` が `kill` / `try_kill_remnants` の典拠として ADR-002 だけを指しており、ADR-015 に置き換えられた点へ導かない
  - 場所: `.thread/3/steps.md:108`, `.thread/3/steps.md:164`, `.thread/3/testing.md:692`
  - 理由: ADR-015（`.thread/3/adr.md:462`）は「ADR-002 の Decision のうち3点 — 同定子を組み直さずそのまま渡す / `kill` は列挙を挟まない / 失敗は終了操作の失敗として返す — を置き換える」と明記し、ADR-002 の本文は当時の記録として残す方針を採っている。ところが steps.md は「実装手段と `NotIdentifiable` の判定は adr.md **ADR-002 で確定させる**」「`kill` / `try_kill_remnants` は adr.md **ADR-002 の決定に従って**実装し」とだけ書き、ADR-015 に触れていない。steps.md から ADR-002 へ飛んだ読み手は、実装が採っていない形（同定子を組み直さない・終了ステータスを成否に写す）を現行の決定として読む。実装は `crates/pulsen/src/adapter/process.rs` の `terminate::UnitTarget::parse` → `operand()` で**組み直して**おり、成否は `gone_within_grace`（`TERMINATION_GRACE` = 2秒 / `TERMINATION_POLL` = 50ms）の消滅観測で決めている。testing.md:692 の確認ポイントも同じく ADR-002 を引いている。
  - 提案: 3箇所とも参照を「adr.md ADR-002 / ADR-015（ADR-015 が置き換えた点を含む）」に改める。

- **[W-002]** `testing.md` の見出しの説明が ADR-017 の語義の一般化を反映しておらず、直後の文と自己矛盾している
  - 場所: `.thread/3/testing.md:717`
  - 理由: 「見出しは**タスクファイルに何を残したか**で分かれる — 最後の1つは…**タスクファイルには何も書かれていない**」と、同じ文のなかで分類軸と実体が食い違う。ADR-017 は「見出しの語義は『タスクファイルに何を残したか』から『報告が何を残したか(運用者が次に取る行動)』へ広がる」と決めており、`render.rs` の `IssueOutcome` の doc コメントも「報告が何を残したか。運用者が次に取る行動はこれで分かれる。」になっている。
  - 提案: 「見出しは**報告が何を残したか（運用者が次に取る行動）**で分かれる（ADR-017）」に直す。

- **[W-003]** 影響確認の grep の期待件数が実際と合わず、そのまま実行すると不一致になる
  - 場所: `.thread/3/testing.md:891`
  - 理由: 「`grep -rn 'command_runner()' crates/pulsen/src/cli/` のヒットが `cli/tick.rs` の1件だけ」と書いてあるが、実行すると `cli/wire.rs:251`（`pub fn command_runner() -> SystemCommandRunner {`）も一致して**2件**返る（実測）。Phase 4 は testing.md を手順書としてそのまま実行するので、意図（`add` の経路がランナーを要さない）が正しくても実行者は不一致を実装の欠陥かどうか判断する手間を負う。
  - 提案: 期待を「ヒットは `cli/wire.rs` の定義1件と `cli/tick.rs` の呼び出し1件の計2件で、`cli/add.rs` に現れないこと」に直すか、grep を `grep -rn 'wire::command_runner()' crates/pulsen/src/cli/` にして1件に落とす。

- **[W-004]** AC-7 の grep 節の件数と `-A 3` が実態を確認できる形になっていない
  - 場所: `.thread/3/testing.md:52`, `.thread/3/testing.md:55`
  - 理由: (1) 「現在は 4 / 12 / 1 件」と書かれた `adapter/process.rs` の 12 は、`origin/main` 時点（10件）とも本 PR 実装後（20件）とも一致しない。合否をファイル集合で判定すると断ってはいるが、括弧の数字だけが両方の読みで誤りになる。(2) 3つ目の grep が `-A 3` なので `crates/pulsen/Cargo.toml` は `pulsen-domain` / `clap` / `getrandom` の3行しか出ず、直後に期待として並べている `serde` / `serde_json` / `serde_yaml_ng` / `tempfile` が視野に入らない。「6クレートから増えていないこと」をこのコマンドでは確認できない。
  - 提案: (1) 件数の括弧を落とすか `4 / 20 / 1`（実測値）に更新する。(2) `-A 3` を `-A 8` にする（`pulsen-domain` 側は空判定なので影響しない）。

- **[W-005]** `plan.md` が setup TC-38 に指示する後始末が、その TC のワークフローでは実行不能
  - 場所: `.thread/3/plan.md:138`
  - 理由: 「手順3(`abort` での片付け)は #5 — 代わりにタスクを残したまま次の TC へ進まないよう、`judge-exit` を 0 に戻して completed で進めてから放置する」とあるが、TC-38 のタスクが使う `judge-missing.yaml` の判定コマンドは `/no/such/judge.sh` であり、`judge-exit`（`judge.sh` が読む制御ファイル）は一切効かない。このタスクは判定不能のまま `judge_attempt_count` を積み、上限超過で凍結する以外の終わり方をしない。`testing.md` エッジケース2 の確認ポイントは実際にはそう書いてある（「以降 running のまま毎 tick 再判定され、上限超過で凍結する（放置してよい）」）ので、契約（plan.md）と実行（testing.md）が割れている。
  - 提案: plan.md:138 を「手順3(`abort`)は #5 — 代わりに放置する（判定コマンドが不在のため判定は成立せず、判定上限超過で凍結して止まる）」に直す。

- **[W-006]** setup TC-11 の実行範囲が `plan.md` と `testing.md` で1手順ぶんずれている
  - 場所: `.thread/3/plan.md:135`, `.thread/3/testing.md:915`
  - 理由: plan.md は「手順1〜4。手順5 のうち `set-status` **以降**は #5」と書いており、手順5 の前半（`echo 0 > judge-exit` → tick 2回で completed まで進める回復）は本スライスの実行範囲に入る。testing.md の確認項目4 手順12 は `attempt` 番号が増えるところで終わっており、回復を実行していない。記帳（testing.md:915）も「TC-11(手順1〜4)」となっていて plan.md の範囲より狭い。AC-8 はこの範囲をそのまま Issue コメントに書き写す。
  - 提案: どちらかに揃える — 確認項目4 手順12 に回復（`echo 0 > "$SETUP_HOME/judge-exit"` → tick 2回で completed）を足すか、plan.md:135 を「手順1〜4。手順5 は #5」に直す。

- **[W-007]** フィクスチャC が `PMT` を手順書と別のパスに変えているのに、直前の節は「パスは手順書の記載どおりに保つ」と宣言している
  - 場所: `.thread/3/testing.md:73`, `.thread/3/testing.md:369`
  - 理由: `spec/manual-tests/intervention.md` の `PMT` は `$HOME/pulsen-manual-test`（= フィクスチャB の `SETUP_HOME` と同一）で、testing.md はこの衝突を避けるため `$HOME/pulsen-intervention-test` に変えている。判断としては正しいが、その理由がどこにも書かれていない一方で、73行目は「パスは各手順書の記載どおりに保つ（読み替えによる取り違えを避けるため）」と逆のことを宣言している。フィクスチャC は先頭で `rm -rf "$PMT"` を実行するので、実行者が73行目に従って `PMT` を手順書どおりへ「戻す」と**フィクスチャB のホームごと消える**。
  - 提案: 73行目に例外を明記する（「ただし intervention.md の `PMT` は setup.md の分離ホームと同一パスのため、フィクスチャC では `$HOME/pulsen-intervention-test` に読み替える。手順書どおりに戻すとフィクスチャB を破壊する」）。

- **[W-008]** ADR-014 の Consequences が、同じ ADR の Decision と実装に反する件数を単独では述べている
  - 場所: `.thread/3/adr.md:424`
  - 理由: 最終行のトレードオフは「今回付けるのは `TaskRepository::save` と `CommandRunner::run` の**2つに限り**」のままで、Decision（`:417`）が「この1つを加えた3つ(`TaskRepository::save` / `TaskRepository::save_degraded` / `CommandRunner::run`)になる」と後から打ち消す形になっている。実装は3つ（`crates/pulsen-conformance/src/doubles/task_repository.rs` の `saved_in_order` / `saved_degraded_in_order`、`command_runner.rs` の `calls_in_order`）。ステップ15 でこの ADR を `.adr/3-*.md` へ昇格させると、Consequences だけを読む後続の読み手に誤った件数が残る。CLAUDE.md の「残すのは現在の形が成り立つ理由だけ」からも、決定の内部に改訂の跡を残す形は外れる。
  - 提案: `:424` の箇条書きを3つに直し、`:417` の「下の『今回付けるのは2つ』は…3つになる」という打ち消し文を落とす。

#### 確認したこと（問題なし）

以下は実装と突き合わせて一致を確認した。

- サマリー項目の並び（起動 / 起動確認 / 判定確定 / 遷移 / 実行待ちへ復帰 / 凍結 / 通知 / 終端処理 / gcで削除 / gcで削除できず）と、報告の4見出しの文言 — `cli/render.rs`
- `TickSummary` 11フィールド（spec 9 + `confirmed_running` + `judged`）、`TickIssue` の追加8変種、`RunFailureCause` 4変種、`RemnantsLeft` 2変種、`IssueOutcome::CleanupLeft`
- ドメインの型名・定数名 — `DefaultJudgement` / `AliveDecision` / `NotifyOutcome` / `NotificationService::NOTIFY_TIMEOUT`(60s) / `TransitionError::AlreadyNotified` / `Delivery::{NotConfigured, Attempted}`
- アダプター — `terminate::UnitTarget`（POSIX は `-<n>`・`n >= 2`、Windows は非0 pid）/ `TERMINATION_GRACE` 2秒 / `TERMINATION_POLL` 50ms / `TerminatorSource`（既定は `/bin/kill` と `taskkill`）/ `SystemCommandRunner::new() -> Self` / `POLL_INTERVAL` 50ms / `Instant::elapsed` 方式の期限測定
- AC-7 の3つの grep の合否条件 — `pulsen-domain` の `[dependencies]` は空、`#[allow(unsafe_code)]` は `adapter/process.rs` の1件、`cfg` のヒットは `util/atomic.rs` / `adapter/process.rs` / `adapter/task_repository.rs` の3ファイル（`adapter/command_runner.rs` は0件）
- スキップ許容集合の表（plan.md:63-75）と `conformance_command_runner.rs` の `allowed_skips` / `conformance_process_controller.rs` の `observation_allowed_skips`・`EXECUTION_UNIT_CASES`・`PARTIAL_TERMINATION_CASES`・4値の `ExecutionUnitCapability`
- `HOOKS.md` の節（ProcessController 27行 / CommandRunner 16行・A 0・B 15・C 1）と steps.md の記述
- testing.md の JSON パス全件 — `.execution.state` / `.execution.reason` / `.execution.notified_at` / `.task_status` / `.counters.*` / `.current_attempt.number` / `.run_dir` / `.process.pid` / `.kill_ident` / `.last_failure.kind` / `.snapshot.statuses`（`adapter/task_file.rs` の DTO）と、`exit` が `.code` を持つ整形 JSON（`adapter/run_store.rs` の `to_vec_pretty`）
- `spec/manual-tests/` の TC 番号と手順番号 — task-execution TC-03/05/06/07/13/14/15/17/19/20/21/22/23、setup TC-09/10/11/35/37/38/39/47、intervention TC-01/15/24 の実在と手順数、plan.md の実行範囲との対応（W-005 / W-006 を除き一致）
- testing.md エッジケース3 が setup TC-39 の `judge` を `["sh","-c","sleep 120"]` から `["sleep","180"]` へ変えた理由（ADR-001 の孫残存の許容）が明記されていること
- ADR-001〜017 の Context / Decision と `.adr/` 既存89件の重複 — 既存 ADR を置き換える3件（ADR-013→ADR-007、ADR-015→ADR-002、ADR-017→`.adr/098`）はいずれも置き換えた点を明記しており、丸ごと重複するエントリは無い
- 見出し階層・表・コードフェンスの対応（testing.md 46・steps.md 2、いずれも偶数）・リンク切れなし

#### カバレッジ

- 確認: `.thread/3/plan.md`, `.thread/3/steps.md`, `.thread/3/testing.md`, `.thread/3/adr.md`
- スキップ: なし（担当外の53ファイルはコードレビューの3観点が確認）
