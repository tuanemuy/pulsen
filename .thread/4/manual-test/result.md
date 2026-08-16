# 実機検証結果 — Issue #4: 状態の確認と追跡(ls / show)

**PR:** #27
**ブランチ / HEAD:** `issue/4/ls-show-status-inspection` / `ebc4a76`
**実行日:** 2026-08-16
**実行環境:** macOS (Darwin 25.4.0) / 非 root (uid 501) / nix devShell (cargo 1.97.0, git 2.55.0, jq 1.7.1-apple)
**手順書:** `.thread/4/testing.md`（`spec/manual-tests/monitoring.md` の読み替え版）

## 総括

| 区分 | 件数 | PASS | FAIL | SKIP |
|---|---|---|---|---|
| 品質ゲート | 4 | 4 | 0 | 0 |
| 確認項目 | 8 | 8 | 0 | 0 |
| エッジケース・異常系 | 6 | 6 | 0 | 0 |
| 既存機能への影響確認 | 7 | 7 | 0 | 0 |

**実装起因の FAIL は0件。** 期待と実際が食い違った箇所が4件あるが、いずれも **手順書（testing.md）の記述・前提の誤り**であり、実装の欠陥ではない（詳細は「期待と実際の不一致」節）。

## 検証環境の準備

`PULSEN_HOME` は実運用の `~/.pulsen` ではなく専用の一時ホームに向けた。手順書の `$HOME/pulsen-manual-test` 等をすべてスクラッチパッド配下に置き換えている（パス以外は手順書のまま）。

| 手順書の変数 | 実際に使ったパス |
|---|---|
| `$PULSEN_HOME` | `<SCRATCH>/mt/pulsen-manual-test` |
| `$HOME/pulsen-test-repo` | `<SCRATCH>/mt/pulsen-test-repo` |
| `$HOME/pulsen-manual-test-empty` | `<SCRATCH>/mt/pulsen-manual-test-empty` |
| `/tmp/pulsen-notify.log` | `<SCRATCH>/mt/pulsen-notify.log` |

`<SCRATCH>` = `/private/tmp/claude-501/-Users-hikaru-github-com-tuanemuy-pulsen/99215b9c-.../scratchpad`

また、当シェルの `ls` はディレクトリ不在時に `os error 2` を返す Rust 実装に置き換わっていたため、`ls(1)` の exit code を見る手順では `/bin/ls` を使った（不在で非0、存在で0 という判定は変わらない）。

### 品質ゲート（4コマンド） — 全 PASS

| コマンド | exit | 根拠 |
|---|---|---|
| `cargo fmt --all --check` | 0 | 出力なし |
| `cargo build --workspace --locked` | 0 | `Finished dev profile` |
| `cargo test --workspace --locked --no-fail-fast -- --nocapture` | 0 | 全30ターゲット `test result: ok`、`0 failed`（最大 142 tests / ターゲット）。適合スイートの `SKIP` は4行（すべて `tc_port_clock_004/005` 系＝ハーネスが `advance`/`rewind` を提供しないため） |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | 0 | `Finished dev profile`、警告なし |

### AC-8 の隔離 grep — 全 PASS

| grep | 期待 | 実際 | 判定 |
|---|---|---|---|
| `cfg(unix\|windows\|target_os\|target_family)` | `crates/pulsen/src/` は `util/atomic.rs` / `adapter/process.rs` / `adapter/task_repository.rs` の3件のみ、新設ファイルは0件 | その3件 + `crates/pulsen-conformance/src/lib.rs`（想定内）。`application/list_tasks.rs` / `show_task.rs` / `cli/ls.rs` / `cli/show.rs` / `cli/render/*` は1件も無し | PASS |
| `#[allow(unsafe_code)]` | `adapter/process.rs:451` の1件のみ | `crates/pulsen/src/adapter/process.rs:451` の1件のみ | PASS |
| `[dependencies]` | `pulsen-domain` は空、`pulsen` は7行 | `pulsen-domain` は `[dependencies]` の直後が空行→`[lints.rust]`。`pulsen` は `pulsen-domain` / `clap` / `getrandom` / `serde` / `serde_json` / `serde_yaml_ng` / `tempfile` の7行 | PASS |

### フィクスチャ準備 — PASS

読み替え版の手順1〜9をすべて実行。tick 15回で予告どおり **9回目の tick で `TASK_C` が凍結**（`凍結: <TASK_C>` / `通知: <TASK_C>`）、**4回目で `TASK_D` が `done` に到達**（`遷移: <TASK_D>`）、`TASK_B` は `running` のまま滞留。手順9 の確認は4条件とも一致（下記 確認項目1 の出力が根拠）。

| タスク | ID | ワークフロー | 目標状態 |
|---|---|---|---|
| TASK_A | `20260816t075649-4k7s4dyh` | wf-wait | hold / pending |
| TASK_E | `20260816t075649-8vzq8e1n` | wf-wait | hold / pending |
| TASK_F | `20260816t075649-l92nhqbi` | wf-wait | hold / pending |
| TASK_G | `20260816t075649-vpyjp0qn` | wf-wait | hold / stopped(直編集) |
| TASK_B | `20260816t075654-443d8uut` | wf-sleep | work / running |
| TASK_C | `20260816t075654-dgzbigrz` | wf-fail | work / stopped(attempt_count 3) |
| TASK_D | `20260816t075654-nrawuesl` | wf-echo | done / アーカイブ済み(手動移動) |
| TASK_H | `20260816t075836-o1kyuhjt` | wf-sleep | 確認項目3 手順8 で追加 |
| TASK_J | `20260816t075943-ojo5ys1y` | wf-judge | 確認項目6 手順6 で追加 |
| TASK_I | `20260816t080021-wa6ffhhw` | wf-echo | 確認項目6 手順8 で追加 |
| TASK_K | `20260816t080337-zbqm8c8i` | wf-echo | 影響確認のスポットチェックで追加 |

## 確認項目

### 1. タスク一覧の全体像 — PASS

| 手順 | 期待 | 実際 | 判定 |
|---|---|---|---|
| 1 | 現役6件 + exit 0 | `TASK_A` / `TASK_E` / `TASK_F` / `TASK_G` / `TASK_B` / `TASK_C` の6行、`EXIT=0` | PASS |
| 2 | 8項目が1行に並ぶ | 見出し行が `タスクID / ワークフロー / リポジトリ / ブランチ / タスクステータス / 実行状態 / attempt_count / 更新日時 / 備考`。8項目すべて + 備考列 | PASS |
| 3 | `hold`/`pending` と `hold`/`stopped` が区別できる | `...4k7s4dyh ... hold pending 0` と `...vpyjp0qn ... hold stopped 0`。タスクステータス列と実行状態列が見出しで別列と読める | PASS |
| 4 | `work`/`running` と `work`/`stopped` が区別できる | `...443d8uut ... work running 0` と `...dgzbigrz ... work stopped 3` | PASS |
| 5 | `TASK_D` が現れない | 一覧に `...nrawuesl` の行なし | PASS |
| — | 未実行4件のブランチが未確定と読める | `TASK_A/E/F/G` のブランチ列は `(未作成)` | PASS |
| 確認ポイント | `TASK_C` の attempt_count が 3 | attempt_count 列が `3` | PASS |

### 2. 絞り込みの合成 — PASS

| 手順 | 期待 | 実際 | 判定 |
|---|---|---|---|
| 1 | `--status work` は `TASK_B`/`TASK_C` のみ | 2行（`443d8uut` work running / `dgzbigrz` work stopped）、exit 0 | PASS |
| 2 | stopped=`TASK_C`+`TASK_G`、pending=`A`/`E`/`F`、running=`B`、すべて 0 | そのとおり（stopped 2行 / pending 3行 / running 1行）、3つとも exit 0 | PASS |
| 3 | AND 合成 | `--status work --state running` → `443d8uut` のみ、`--status work --state stopped` → `dgzbigrz` のみ（`TASK_G` は `hold` なので落ちる）。両方 exit 0 | PASS |
| 4 | `--all` で7件、`TASK_D` にアーカイブ印とブランチ | 7行目に `nrawuesl ... pulsen/20260816t075654-nrawuesl done pending 0 ... アーカイブ済み`、exit 0 | PASS |
| 5 | `--status done` は0件で 0、`--all --status done` は `TASK_D` のみ | 前者 `該当するタスクはありません。` exit 0、後者は `nrawuesl ... done pending ... アーカイブ済み` の1行 exit 0 | PASS |
| 6 | 未知ステータスはエラーにならず0件で 0 | `該当するタスクはありません。` exit 0 | PASS |
| 7 | `launching`/`completed`/`failed` が受理され空表示で 0 | 3つとも `該当するタスクはありません。` exit 0 | PASS |

確認ポイント: 手順5 の2結果の差（0件 vs `TASK_D` 1件）が `--all` を絞り込み条件として扱っていない証拠。空のとき無言でなく `該当するタスクはありません。` を出す（PAGE-ls-010）。

### 3. タスク詳細 — running / 未実行 / launching — PASS

| 手順 | 期待 | 実際 | 判定 |
|---|---|---|---|
| 1・2 | 詳細表示 + exit 0 | `ワークフロー: wf-sleep` / リポジトリ / `ベースブランチ: main` / `タスクステータス: work` / `実行状態: running` / `更新日時: 2026-08-16T07:57:00Z`、`EXIT=0` | PASS |
| 3 | workspace_path とブランチ、実在 | `workspace_path: .../worktrees/20260816t075654-443d8uut` / `branch: pulsen/20260816t075654-443d8uut`、`/bin/ls` exit 0 | PASS |
| 4 | 3カウンタと上限を併記 | `attempt_count: 0(上限 2)` / `judge_attempt_count: 0(上限 3)` / `spawn_fail_count: 0(上限 3)` | PASS |
| 5 | `work`/`done` の一覧と `state/tasks/<B>.json` | `定義済みステータス: done, work` / `スナップショット保存先: .../state/tasks/20260816t075654-443d8uut.json` | PASS |
| 6 | 番号1・run dir・PID・kill同定子・starttime | `現在attempt: 1` / `runディレクトリ: .../attempt-1` / `PID: 40523` / `kill同定子: -40523` / `starttime: Sun Aug 16 07:56:57 2026(記録時刻 2026-08-16T07:56:57Z)` | PASS |
| 7 | 未実行タスクの表示 + `ls` 非0 | `workspace_path: 未作成` / `branch: 未作成` / `現在attempt: なし`（run dir・PID・exit の行が出ない）/ `attempt_count: 0`（**上限の併記なし**）/ judge・spawn は上限併記あり。`EXIT=0`。`/bin/ls state/runs/<A>` は `EXIT=1` | PASS |
| 8 | launching で同定情報3値がまとめて未取得 | `実行状態: launching` / `現在attempt: 1` / `runディレクトリ: .../attempt-1` / **`同定情報: 未取得(PID・kill同定子・starttime)`** の1行、`EXIT=0` | PASS |

確認ポイント: 手順7 の併記なし（`attempt_count: 0`）と、確認項目7 手順11 の不明（`attempt_count: 0(上限 導出不能: スナップショットを読めません)`）が別の文言であること → PASS。3項目がまとめて未取得になること → PASS。

### 4. アーカイブ済みタスクの詳細と手動アーカイブ移動 — PASS

| 手順 | 期待 | 実際 | 判定 |
|---|---|---|---|
| 1〜3 | アーカイブ明示 / workspace「削除済み」/ ブランチ表示 / `state/archive/<D>.json` | `在籍: アーカイブ済み(worktree は削除済み)` / `workspace_path: .../worktrees/...nrawuesl(削除済み)` / `branch: pulsen/20260816t075654-nrawuesl` / `スナップショット保存先: .../state/archive/20260816t075654-nrawuesl.json`、`EXIT=0` | PASS |
| 4 | `pulsen/<TASK_D>` が実在 | `git branch --list "pulsen/*"` に `pulsen/20260816t075654-nrawuesl` | PASS |
| 5 | `TASK_G` の workspace が「未作成」 | `在籍: 現役` / `workspace_path: 未作成` / `branch: 未作成`、exit 0 | PASS |
| 7 | `ls` にも tick にも `TASK_G` が出ない | `ls` 6行に `vpyjp0qn` なし（exit 0）、`tick` は `起動確認: 20260816t075836-o1kyuhjt` のみ（exit 0） | PASS |
| 8 | `--all` でアーカイブ印付きで出る | `vpyjp0qn hold stopped アーカイブ済み` の行、exit 0 | PASS |
| 9 | アーカイブ注記付きで表示・exit 0 | `在籍: アーカイブ済み(worktree は削除済み)` / `スナップショット保存先: .../state/archive/20260816t075649-vpyjp0qn.json`、`EXIT=0` | PASS |
| 確認ポイント | tick が `state/archive/` を書き換えない | tick 2回の前後で `<G>.json` の md5 `dc3f6621...` / `<D>.json` の md5 `4677c951...` が不変 | PASS |

観察（欠陥ではない）: 一度も worktree を作っていない `TASK_G` でも 在籍行は `アーカイブ済み(worktree は削除済み)` と出る。`workspace_path` は正しく `未作成` なので矛盾は生じないが、在籍行の注記はアーカイブ済みという事実からのみ導かれる（pages ※4 の設計どおり）。

### 5. エージェント実行ログの確認の起点 — PASS

| 手順 | 期待 | 実際 | 判定 |
|---|---|---|---|
| 1 | 3パス + exit の値 0 | `stdout.log` / `stderr.log` / `exit: .../attempt-1/exit(値 0)` | PASS |
| 2 | `hello from pulsen` | `hello from pulsen`（加工なし） | PASS |
| 3 | 空 | 0 バイト | PASS |
| 4 | `{"code": 0}` の整形 JSON | `{\n  "code": 0\n}`。show の `(値 0)` と一致 | PASS |
| 5 | 現在attempt が `attempt-3`、3ディレクトリが別々に存在 | `現在attempt: 3` / `runディレクトリ: .../attempt-3`、`/bin/ls` が `attempt-1` `attempt-2` `attempt-3` | PASS |
| 6 | 全 attempt の `.code` が 1 | attempt-1/2/3 とも `{"code": 1}`、stderr は3つとも 0 バイト | PASS |
| 7 | log 2つはあり `exit` は無い、show も「なし」 | `/bin/ls attempt-1` は `pid` `starttime` `stderr.log` `stdout.log`（`exit` なし）、show は `exit: ...(記録なし)` | PASS |
| 8 | `tail -f` がエラーなく追える | 3秒追跡して出力増加なし、終了 exit 0 | PASS |
| 9 | exit **0**、run dir を「存在しない」と明示、他は通常表示 | `runディレクトリ: .../attempt-1(存在しません)`、PID・starttime・log パス・カウンタ等は通常表示、`EXIT=0` | PASS |

観察: 手順9 で `exit` 行は `(記録なし)` になり、これは未終了時（`TASK_B`）と同じ文言。ただし直上の runディレクトリ行が `(存在しません)` を持つので不在は読み取れる。エラー扱い（非0）への昇格は起きていない。

### 6. stopped タスクの原因調査 — 3経路の判別 — PASS

| 手順 | 期待 | 実際 | 判定 |
|---|---|---|---|
| 1・2 | 凍結要因表示、attempt_count 3 が上限 2 を超過、judge/spawn は上限未満、notified_at あり | `凍結要因: リトライ上限の超過` / `notified_at: 2026-08-16T07:57:14Z` / `attempt_count: 3(上限 2)` / `judge_attempt_count: 0(上限 3)` / `spawn_fail_count: 0(上限 3)`、`EXIT=0` | PASS |
| 3 | 通知ログに1行 | `stopped: 20260816t075654-dgzbigrz (wf-fail/work)` | PASS |
| 4 | worktree が残っている（中身は `.git` だけ） | `/bin/ls -a` が `.` `..` `.git`、exit 0 | PASS |
| 7 | judge が上限超過、attempt は上限未満、スナップショットに judge 固定、`.code` は 0 | `凍結要因: 判定失敗の上限の超過` / `judge_attempt_count: 4(上限 3)` / `attempt_count: 0(上限 2)` / `spawn_fail_count: 0(上限 3)` / `exit: ...(値 0)` / `直近の失敗要因: 判定の実行(...): 判定コマンドがプロトコル外の終了コード 5 を返しました(有効な値は 0 / 10 / 20)`。タスクファイルの `snapshot.statuses.work.judge` は `["sh","-c","exit 5"]`、`EXIT=0` | PASS |
| 11 | spawn が上限超過、展開エラーの内容、**attempt なし**、`ls` 非0、通知ログに行 | `凍結要因: spawn 失敗の上限の超過` / `spawn_fail_count: 4(上限 3)` / `現在attempt: なし` / `直近の失敗要因: エージェントの起動(...): エージェント \`sh\` の定義が不正です: cmd の\`{bogus}\` は使えないプレースホルダ \`bogus\` を参照しています`。`/bin/ls state/runs/<I>` は `EXIT=1`、通知ログに `stopped: ...wa6ffhhw (wf-echo/work)`、`EXIT=0` | PASS |
| 12 | config 復元 | `cp config.yaml.bak config.yaml` 実行済み、`cmd: ["sh", "-c", "{input}"]` に戻っている | PASS（実行済み） |

確認ポイント: 3経路が「どのカウンタがどの上限を超えたか」だけで判別できる（`3(上限 2)` / `4(上限 3)` judge / `4(上限 3)` spawn）→ PASS。同期 spawn 失敗（現在attempt: なし＝採番なし）と猶予超過（run dir はあるが exit なし）が別物として読める → PASS。

### 7. タスクファイルの直接閲覧・修復 — PASS

| 手順 | 期待 | 実際 | 判定 |
|---|---|---|---|
| 1・2 | 人間可読 JSON、show は 0 | `task_id` / `workflow_name` / `target` / `task_status` / `execution` / `counters` / `updated_at` / `snapshot` が読める。`show` exit 0、ファイルの md5 不変 | PASS |
| 5 | exit **0**、`TASK_E` のパスと読み取り不能の旨、残りは通常表示 | `EXIT=0`。末尾に `読み取れなかったタスクファイル(1件):` → `- .../state/tasks/20260816t075649-8vzq8e1n.json: unknown field \`broken\`, ... at line 1 column 10` / `内容を直接確認して修復してください。`。残り7行（`A`/`F`/`B`/`C`/`H`/`J`/`I`）は通常表示 | PASS（※`TASK_G` の扱いは「期待と実際の不一致」M3 参照） |
| 6 | パースエラー内容 + パス、**非0**、書き込みなし | `エラー: タスクファイルを読めません。` / `ファイル: .../8vzq8e1n.json` / `原因: unknown field \`broken\` ...` / `内容は変更していません。...`、`EXIT=1`、破損ファイルの md5 不変 | PASS |
| 7 | 修復後 show 0、`ls` から報告が消え通常行に戻る | show exit 0、`ls` の `読み取れなかった` 件数 0、8行すべて通常表示、exit 0 | PASS |
| 9 | `task_status`/`execution` は読め、`snapshot.initial` が空文字 | jq が `"hold"` / `{"state":"pending"}` / `""` | PASS |
| 10 | 行として表示 + 印、exit 0、`--state pending` にも出る | `l92nhqbi ... hold pending 0 ... スナップショット読み取り不能`（exit 0）。`--state pending` の3行目に同じ行（exit 0） | PASS |
| 11 | exit 0、タスク側は表示、理由注記、**一覧は非表示**、リトライ上限は「不明」、judge/spawn は通常 | `attempt_count: 0(上限 導出不能: スナップショットを読めません)` / `judge_attempt_count: 0(上限 3)` / `spawn_fail_count: 0(上限 3)` / `定義済みステータス: 読み取れません(initial: 空文字列は指定できません)`、`EXIT=0`、ファイルの md5 不変 | PASS |
| 12 | 通常表示に戻り 0 | `attempt_count: 0`（併記なしに戻る）/ `定義済みステータス: hold`、exit 0、`ls` の備考列が空 | PASS |

確認ポイント: `Corrupt`（行にならずパス付きで別枠報告）と `SnapshotUnreadable`（行として出て `--state` の絞り込み対象になる）の2系統が出力から読み分けられる → PASS。`ls`/`show` が破損ファイルに書き込み・正規化を行わない（md5 不変）→ PASS。

### 8. 読み取り専用であることの外形確認 — PASS

| 手順 | 期待 | 実際 | 判定 |
|---|---|---|---|
| 2 | worktree 削除後も記録どおり表示、exit 0 | 削除前後で `workspace_path: .../worktrees/20260816t075654-dgzbigrz` が同一（`(削除済み)` も付かない）、`branch` も表示、`EXIT=0` | PASS |
| 4 | ロック保持中も3つとも通常結果 + exit 0 | holder が `locked` を出力した状態で `ls`（9行）exit 0 / `ls --all`（11行）exit 0 / `show <B>` 詳細表示 exit 0。**「スキップ」メッセージは一切出ない** | PASS |
| 5 | tick はスキップして exit 0 | `別の操作が実行中のため、今回の tick はスキップしました。` `EXIT=0` | PASS |
| 6 | 解放後も手順4 と同じ | `ls` 9行 exit 0（手順4 と同一） | PASS |
| 7 | tick 同時実行でも毎回 0・破損報告なし | 3回とも `ls_exit=0 lines=9 corrupt_report=0` | PASS |
| 8 | grep による型の担保 | 「期待と実際の不一致」M1・M2 参照。実体は別 grep で確認済み | PASS（実体） |

実体の確認（手順8 の代替）:

- `crates/pulsen/src/cli/ls.rs` / `show.rs` に `lock()` の**呼び出しは無い**（ヒットは行21 のドキュメントコメント `**runtime.lock() を渡さない。**` のみ）。実呼び出しは `cli/add.rs:37` と `cli/tick.rs:36` の2箇所だけ。
- `grep -rnE '\.(exists|try_exists)\(\)' application/show_task.rs cli/render/show.rs application/list_tasks.rs cli/render/ls.rs` → **0件**（exit 1）。`Path::exists()` は使っていない。

## エッジケース・異常系

### 1. 存在しないタスクID・不正な書式・長さの境界 — PASS

| 手順 | 入力 | 実際の出力 | exit | 判定 |
|---|---|---|---|---|
| 1 | `no-such-task-0000` | `エラー: 指定されたタスクが見つかりません。` / `タスクID: no-such-task-0000` / `現役にもアーカイブにも存在しません。` | 1 | PASS |
| 2 | `TASK_A!` | `エラー: タスクIDが不正です。` / `原因: 1文字目に使えない文字('T')があります。使えるのは英小文字・数字・\`-\` です` | 1 | PASS |
| 3 | `-- -abc` | `エラー: タスクIDが不正です。` / `原因: 先頭は英小文字か数字である必要があります`（clap のオプション解釈に食われず値として届いた） | 1 | PASS |
| 4 | 65文字 | `エラー: タスクIDが不正です。` / `原因: 64文字を超えられません` | 1 | PASS |
| 5 | 64文字 | `エラー: 指定されたタスクが見つかりません。` / `タスクID: aaa…(64)` | 1 | PASS |
| 6 | 空文字 | `エラー: タスクIDが不正です。` / `原因: 空文字列は指定できません` | 1 | PASS |

確認ポイント: 手順4（検証エラー）と手順5（不在エラー）が同じ非0でも別の見出しで、64文字が受理されることで長さ上限の境界が確認できる → PASS。空表示にはならない → PASS。

### 2. `--state` への不正値・表記揺れ・空文字 — PASS

| 手順 | 入力 | 実際 | exit | 判定 |
|---|---|---|---|---|
| 1 | `--state stoped` | `エラー: --state の値が不正です。` / `指定: \`stoped\`` / `有効な値: pending / launching / running / completed / failed / stopped` | 1 | PASS |
| 2 | `--state Pending` | 同上（`指定: \`Pending\``） | 1 | PASS |
| 3 | `--state ""` | 同上（`指定: \`\``） | 1 | PASS |
| 4 | `--status ""` | `該当するタスクはありません。` | **0** | PASS |

確認ポイント: 有効値一覧が clap の整形（exit 2 の `error: invalid value ...`）ではなく本ツールの拒否文言（exit 1・日本語・`エラー:` 見出し）で出る → `value_parser` で先に弾いていない証拠。手順3 と手順4 の扱いが逆になっており、検証する語彙／しない語彙の線引きが見える → PASS。

### 3. config.yaml 不在・パース不能 — PASS

| 手順 | 実際 | exit | 判定 |
|---|---|---|---|
| 2 (`ls`) | `エラー: グローバルホームが未初期化です。` / `グローバルホーム: <PULSEN_HOME>` / `グローバル設定 <PULSEN_HOME>/config.yaml を作成してください。` | 1 | PASS |
| 3 (`show`) | 同一の3行 | 1 | PASS |
| 5 (壊れた YAML) | `エラー: グローバル設定を解釈できません。` / `ファイル: <PULSEN_HOME>/config.yaml` / `原因: did not find expected node content at line 2 column 1, while parsing a flow node` / `位置: 2行1列` | 1 | PASS |
| 6 (復元) | 通常の一覧（9行） | 0 | PASS |

確認ポイント: 表示されるホームパスが `PULSEN_HOME` の解決結果になっており、ホーム解決は成功して config 読み込みで拒否されている → PASS。拒否の過程で `state/tasks/` 配下のファイル集合の md5 が不変 → PASS。

### 4. 状態ディレクトリの走査不能（権限エラー） — PASS

非 root（uid 501 / macOS Darwin 25.4.0）のため `chmod 000` が有効に働き、**スキップせずに実施できた**。

| 手順 | 実際 | exit | 判定 |
|---|---|---|---|
| 1 (`state/tasks` を 000) | `エラー: タスクを走査できません。` / `原因: <PULSEN_HOME>/state/tasks: Permission denied (os error 13)` | 1 | PASS |
| 3 (`state/archive` を 000) | `ls --all` は同じ形式のエラーで **1**、`ls`（`--all` なし）は **0** | 1 / 0 | PASS |
| 4 (復元) | `ls --all` が11行 | 0 | PASS |

確認ポイント: 走査の失敗が「破損ファイル1件の報告」に化けず、一覧を一切出さずに非0で終わる → PASS。

### 5. exit / runディレクトリが読めない場合の表示継続 — PASS

| 手順 | 期待 | 実際 | exit | 判定 |
|---|---|---|---|---|
| 3 (`exit` に `abc`) | exit 0、exit 項目にのみ注記、他は通常 | `exit: .../attempt-3/exit(読み取れません: 内容を解釈できない: expected value at line 1 column 1)`。`凍結要因` `attempt_count: 3(上限 2)` `runディレクトリ` は通常表示。ファイル内容は `abc` のまま（書き換えなし） | 0 | PASS |
| 5 (`attempt-3` を 000) | exit 0、run dir の確認失敗の注記付きで継続 | `runディレクトリ: .../attempt-3`（注記なし）/ `exit: 読み取れません: .../exit: 読み取れない: Permission denied (os error 13)` | 0 | PASS（注記の位置は「不一致 M4」参照） |
| 6 (復元) | exit の値 1 を含む通常表示 | `exit: .../exit(値 1)` | 0 | PASS |

補助手順（手順5 の意図を実際に成立させるため追加実行）: 親の `state/runs/<TASK_C>` を `chmod 000` にすると
`runディレクトリ: .../attempt-3 → 存在を確認できません: ...: attempt ディレクトリの有無を確認できない: Permission denied (os error 13)` /
`exit: ...(runディレクトリの有無を確認できないため読んでいません)` となり **exit 0**。
これで「存在しません（不在）」「存在を確認できません（確認失敗）」「読み取れません（内容破損）」「記録なし（未終了）」の4つが別文言であることを確認した → 確認ポイント PASS。

### 6. タスク0件・`state/` 不在 — PASS

| 手順 | 実際 | exit | 判定 |
|---|---|---|---|
| 2 (`ls`) | `該当するタスクはありません。` | 0 | PASS |
| 3 (`ls --all`) | `該当するタスクはありません。` | 0 | PASS |
| 4 (`show <TASK_A>`) | `エラー: 指定されたタスクが見つかりません。` / `現役にもアーカイブにも存在しません。` | **1** | PASS |
| 5 (元ホーム・未知ステータス) | `該当するタスクはありません。` | 0 | PASS |
| 確認ポイント | 読み取りが `state/` を作らない → 手順2・3 の後の `/bin/ls -a <EMPTYHOME>` が `.` `..` `config.yaml` のみ | — | PASS |

## 既存機能への影響確認

| 項目 | 期待 | 実際 | 判定 |
|---|---|---|---|
| `pulsen --help` | サブコマンドが `add`/`tick`/`ls`/`show`/`help`、`wrapper` は現れない、`abort`/`retry`/`set-status` なし | `Commands: add / tick / ls / show / help` のみ。`wrapper` は一覧に出ない（`pulsen wrapper --help` では到達できるので隠しサブコマンドとして機能）。`abort`/`retry`/`set-status` なし | PASS |
| `ls --help` / `show --help` | `--status`/`--state`/`--all`、位置引数 `<TASK-ID>`、両方に `--home` | `ls`: `--home` `--status <TASK-STATUS>` `--state <EXEC-STATE>` `--all`。`show`: `Arguments: <TASK-ID>` + `--home` | PASS |
| `add` / `tick` 経路 | `.thread/3/testing.md` 確認項目1 のスポットチェック、サマリー・報告の見出しと刻みが #3 時点と同じ | `TASK_K`(wf-echo): `起動` → `起動確認` → `判定確定` → `遷移` → `処理対象のタスクはありませんでした。`（毎回 exit 0）。show で `タスクステータス: done` / `exit: ...(値 0)` | PASS |
| `attempt_exists` の配置 | `adapter/run_store.rs` の実装と `application/show_task.rs` の呼び出しに限られ、`application/tick*` に現れない | ヒットは `adapter/run_store.rs:122`(impl) / `:267`(同ファイル内テスト) / `application/show_task.rs:326`(呼び出し) の3件のみ | PASS |
| `cli/render.rs` の分割 | 既存の文言が巻き添えで変わっていない | 分割前 `render.rs`（merge-base `f575b4a`）の日本語文字列リテラル135件が、分割後の `cli/render/*.rs` に**1件も欠けずに**存在（`comm -23` が0行）。新規は78件（ls/show 用）。tick サマリーの刻み順は `render/tick.rs` のテストが `起動 / 起動確認 / 判定確定 / 遷移 / 実行待ちへ復帰 / 凍結 / 通知 / 終端処理 / gcで削除 / gcで削除できず` の順で固定、報告の4見出しは `失敗を記録` / `起動の結果が未確定` / `スキップ` / `後始末が残っている`。実機の tick 出力もこの語で一致 | PASS |
| `archived`/`gc_deleted`/`gc_errors` | 値の入る経路を持たない（#6） | `application/tick/mod.rs:444` が `Branch::Cleanup => {}`。3フィールドへの push 箇所は0件 | PASS |
| 時刻の表示形式 | RFC3339 UTC でタスクファイルと同形式 | `ls` の更新日時 `2026-08-16T07:56:49Z` = タスクファイルの `"updated_at": "2026-08-16T07:56:49Z"`。`show` の `notified_at: 2026-08-16T07:57:14Z`、starttime は `Sun Aug 16 07:56:57 2026(記録時刻 2026-08-16T07:56:57Z)` で記録時刻側が RFC3339 UTC | PASS |
| 実運用ホームの非汚染 | 実行前と変わらない | 実行前・実行後とも `/Users/hikaru/.pulsen: No such file or directory`（生成されていない） | PASS |
| 後片付け | 残留プロセス終了・一時領域削除 | 実行済み（下記「後片付け」節） | PASS |

## 期待と実際の不一致（すべて手順書・環境起因。実装起因は0件）

### M1. 確認項目8 手順8 の1つ目の grep

- **期待（testing.md）:** 「1つ目のヒットが `cli/wire.rs` の定義と `cli/tick.rs` の呼び出しだけで、`cli/ls.rs` / `cli/show.rs` に現れないこと」
- **実際:** `grep -rn 'lock()' crates/pulsen/src/cli/` のヒットは
  `cli/ls.rs:21` と `cli/show.rs:21`（いずれもドキュメントコメント `**runtime.lock() を渡さない。**`）、`cli/add.rs:35,37`、`cli/tick.rs:36,37`。`cli/wire.rs` は**ヒットしない**。
- **切り分け:** 実装ではなく **手順書の grep 期待が不正確**。(1) `wire.rs:146` の定義は `pub fn lock(&self)` なので `lock()` というパターンでは一致しない。(2) `add.rs` は書き込み系なのでロックを取るのが正しい（手順書の期待に `add.rs` が抜けている）。(3) `ls.rs`/`show.rs` のヒットは「ロックを渡さない」と明言したコメントであり、呼び出しではない。
- **実体の担保:** `ls.rs`/`show.rs` に `lock()` の呼び出しは存在せず、確認項目8 手順4・5 の実機挙動（ロック保持中でも `ls`/`show` は 0 で完走、`tick` だけがスキップ）が同じことを外形から示している。

### M2. 確認項目8 手順8 の2つ目の grep

- **期待（testing.md）:** 「2つ目のヒットが0件であること」
- **実際:** `grep -rn 'exists\|try_exists' application/show_task.rs cli/render/show.rs` が1件ヒット（`show_task.rs:326: let presence = match self.runs.attempt_exists(run_dir) {`）。
- **切り分け:** **手順書の自己矛盾**。同じ testing.md の「既存機能への影響確認」で「本スライスが既存経路に足すのは `RunStore::attempt_exists`（…）だけで」「`application/show_task.rs` の呼び出し」と明記しており、`exists` を部分文字列で引けば必ず当たる。
- **実体の担保:** `grep -rnE '\.(exists|try_exists)\(\)'` は0件。`Path::exists()` による存在検証は行っていない（確認項目8 手順2 の実機挙動＝worktree 削除後も表示が変わらない、とも一致）。

### M3. 確認項目7 手順5 の期待に並ぶタスクの列挙

- **期待（testing.md）:** 「残り（`TASK_A` / `TASK_B` / `TASK_C` / `TASK_F` / **`TASK_G`** と TC-16・TC-17 で追加した `TASK_H` / `TASK_I` / `TASK_J`）は通常どおり表示される」
- **実際:** `TASK_G` は現れず、残りは `TASK_A` / `TASK_F` / `TASK_B` / `TASK_C` / `TASK_H` / `TASK_J` / `TASK_I` の7件。
- **切り分け:** **手順書の列挙誤り**。同じ testing.md の確認項目4 手順6 で `TASK_G` を `state/archive/` へ移しているため、`--all` なしの `ls` に出ないのが正しい（確認項目4 手順7 の期待とも整合）。実装の挙動は一貫している。

### M4. エッジケース5 手順5 の注記の位置

- **期待（testing.md）:** 「手順5 も exit code 0 で、**runディレクトリの存在確認が失敗した旨**の注記付きで表示が続く」
- **実際:** exit code 0 で表示は続くが、`runディレクトリ` 行に注記は付かず（通常表示）、`exit` 行に `読み取れません: …: Permission denied (os error 13)` が付いた。
- **切り分け:** **手順の前提が当該環境で成立しない**。macOS では対象ディレクトリ自身を `chmod 000` にしても、そのパスの stat（存在確認）は親ディレクトリの権限で成功するため `attempt_exists` は失敗せず、失敗するのは中の `exit` の読み取りだけになる。手順書が想定した「存在確認の失敗」は作れていない。
- **補助検証:** 親の `state/runs/<TASK_C>` を `chmod 000` にすると `runディレクトリ: … → 存在を確認できません: …: attempt ディレクトリの有無を確認できない: Permission denied` / `exit: …(runディレクトリの有無を確認できないため読んでいません)` が exit 0 で出る。確認ポイントである「不在（存在しません）と確認失敗（存在を確認できません）が別の結末」は成立している。

## 実行しなかった項目（testing.md が実行範囲外と定めたもの・AC-9 の記帳材料）

| 項目 | 理由 |
|---|---|
| `monitoring.md` TC-09 手順4 | 後片付けの `pulsen abort` が #5 で未実装 |
| `monitoring.md` TC-15 | abort 経路で凍結したタスクを CLI で作れない（#5） |
| `monitoring.md` TC-34 手順1 | `pulsen set-status` が #5 で未実装 |
| `cleanup.md` TC-13 / TC-14 / TC-15 | 前提のアーカイブ済みタスクが tick の終端処理（#6）を要する |
| `cleanup.md` TC-17 | 手順1 は TC-19 と重複、手順2 の `pulsen retry` は #5 |
| `cleanup.md` TC-23 | 前提の gc 済み run ディレクトリが #6 を要する |

部分消化: TC-05 / TC-06 / TC-10 / TC-11 / TC-25 は**アーカイブ済みの前提を手動移動で作った読み替え**で消化しており、tick の終端処理がアーカイブを生む経路そのものは確認していない（`PAGE-ls-004` / `PAGE-show-008` / `TC-task-show-task-031` は部分消化）。

権限制限によるスキップ: **なし**（非 root の macOS Darwin 25.4.0 で `chmod 000` が有効に働いたため、エッジケース4 とエッジケース5 手順5 はいずれも実施できた）。

## 後片付け

- `ps -ef` で確認した残留は `pulsen wrapper`（`TASK_B` / `TASK_H`）とその子 `sleep 3000` の4プロセス。`pkill -f 'sleep 3000'` と wrapper の終了で解消。
- `<SCRATCH>/mt/`（`PULSEN_HOME` / テストリポジトリ / 空ホーム / 通知ログ / `exit` のバックアップ）を削除。
- `~/.pulsen` は最初から最後まで存在しない（作られていない）。
- リポジトリの作業ツリーは無変更（コードは修正していない）。

## 手順書への改善提案

1. 確認項目8 手順8 の grep 2つを実態に合わせる（M1 / M2）。`grep -rn 'lock()' crates/pulsen/src/cli/` は期待を「`add.rs` / `tick.rs` の呼び出しのみ。`ls.rs` / `show.rs` のヒットはコメント」に直すか、`grep -rn 'runtime\.lock()' ` に絞る。2つ目は `grep -rnE '\.(exists|try_exists)\(\)'` にすれば意図どおり0件になる。
2. 確認項目7 手順5 の期待から `TASK_G` を外す（M3）。
3. エッジケース5 手順5 の `chmod 000` の対象を `state/runs/<TASK_C>`（親）にする（M4）。現行の `attempt-3` 自身では「存在確認の失敗」を作れない。
4. 確認項目5 手順9 の確認ポイントは「`exit` の項目が『存在しない』に吸収され」とあるが、実際は `exit` 行が `(記録なし)` のままで、不在は `runディレクトリ` 行の `(存在しません)` が担う。文言を実装に合わせるとよい。
5. 手順書の `PULSEN_HOME` を `$HOME/pulsen-manual-test` 固定にせず、任意の一時ディレクトリを指定できる書き方にすると、ホームディレクトリを汚さずに実行できる。
