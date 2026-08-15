# 手動確認の結果 — フィクスチャA

**実行日:** 2026-08-14
**環境:** macOS 26.4.1 (Darwin 25.4.0) / 非 root(uid 501) / フィクスチャは `/tmp/pulsen-test`(実体 `/private/tmp/pulsen-test`)。`TMPDIR` は `/var/folders/.../T/` だが手順書どおり `/tmp` 直下に固定した
**バイナリ:** `/Users/hikaru/github.com/tuanemuy/pulsen/target/debug/pulsen`（ブランチ `issue/3/tick-observe-judge-transition`）
**対象:** `.thread/3/testing.md` の確認項目・エッジケースのうち `spec/manual-tests/task-execution.md` 由来のもの（TC-03 / 05 / 06 / 07 / 13 / 14 / 15 / 17 / 19 / 20 / 21 / 22 / 23）

## サマリー

- 実行: 11件（PASS: 11 / FAIL: 0 / 実行不可: 0）
- 内訳: 確認項目 1 / 3 / 4 / 5 / 7 / 8 / 10 / 11、エッジケース 4 / 5 / 6
- FAIL は無し。testing.md の記述の誤り 2件・記載不足 1件を検出（末尾「testing.md の記述の問題」）
- 他フィクスチャ担当の手順（`setup.md` / `intervention.md` 由来）は実行していない

## 確認項目ごとの結果

### 1. exit 0 の観測 → completed → 次 tick で next へ（対応する手順書: task-execution TC-03 手順1〜9・11・12）

- **判定:** PASS
- **実行した手順:**

    ```sh
    export PULSEN_HOME=/tmp/pulsen-test/home
    pulsen add --workflow pipeline --repo /tmp/pulsen-test/repo     # T3=20260814t095503-ooq7xiik
    sed -i.orig 's/prompt: "echo planning .*"/prompt: "echo edited-should-not-appear"/' \
      /tmp/pulsen-test/home/workflows/pipeline.yaml
    pulsen tick                                                     # 1回目(起動)
    sleep 3; pulsen tick                                            # 2回目(spawn確認)
    until [ -f "$PULSEN_HOME/state/runs/$T3/attempt-1/exit" ]; do sleep 1; done
    cat "$PULSEN_HOME/state/runs/$T3/attempt-1/exit"
    pulsen tick                                                     # 3回目(判定)
    sleep 3; pulsen tick                                            # 4回目(遷移)
    cat "$PULSEN_HOME/state/runs/$T3/attempt-1/stdout.log"
    # planned / implemented について同じ4刻みを繰り返す
    git -C /tmp/pulsen-test/repo log --oneline "pulsen/$T3"
    pulsen tick                                                     # done 到達後
    cp /tmp/pulsen-test/pipeline.bak /tmp/pulsen-test/home/workflows/pipeline.yaml   # 手順12(復元)
    ```

- **観測した結果:**
  - 手順3: `起動: 20260814t095503-ooq7xiik`、`.execution` = `{"state":"launching","recorded_at":"2026-08-14T09:55:10Z"}`、`.current_attempt.process` は `null`。スナップショットの `queued.input.prompt` は `echo planning && …`（元の値）で保存されていた。
  - 手順4: `起動確認: …`、`.execution.state` = `running`、`process` = `{"pid":14016,"kill_ident":"-14016","starttime":{"ident":"Fri Aug 14 09:55:10 2026","wall":"2026-08-14T09:55:10Z"}}`。
  - 手順5: `exit` の内容は整形 JSON `{\n  "code": 0\n}`、`jq '.code'` = `0`。
  - 手順6: `判定確定: 20260814t095503-ooq7xiik`（「起動確認」ではない）。`.execution.state` = `completed`、`.task_status` = `queued`、`.counters` = 全0。
  - 手順7: `遷移: 20260814t095503-ooq7xiik`。`.task_status` = `planned`、`.execution.state` = `pending`、`.counters` 全0、`.current_attempt.number` = 1 のまま。`.updated_at` は `09:55:35Z` に更新。
  - 手順8: `stdout.log` に `planning` と `[pulsen/… e62e0b8] plan`。`edited-should-not-appear` は出ていない。
  - 手順9: `planned` → `implemented` → `done` まで各4 tick（起動 / 起動確認 / 判定確定 / 遷移）で進み、run ディレクトリは `attempt-1` `attempt-2` `attempt-3` の3つ。
  - 手順10: `pulsen/20260814t095503-ooq7xiik` に `3871daf review` / `cd84b99 impl` / `e62e0b8 plan` / `4c3c62b init`。worktree には `plan.txt` `impl.txt` `review.txt` が揃っている。
  - 手順11: `処理対象のタスクはありませんでした。` exit code 0。タスクファイルの md5 は tick 前後で `4e0f53622fd9518a5bec9f0c41573ce2` のまま、`.task_status` = `done` / `.execution.state` = `pending`。サマリーにも報告にも現れない。
  - 手順12: 復元後 `grep -n 'echo planning'` が6行目にヒット。`pipeline.yaml.orig` は削除済み。
- **期待との一致:** 一致。1タスク1tick1ステップ、スナップショット非依存、`done` 到達後の無反応（ADR-101）まですべて期待どおり。

### 3. 一過性失敗の自動リトライと回復（対応する手順書: task-execution TC-05 手順1〜6、TC-19 手順1〜5 / 本書の手順1〜6・10〜12）

- **判定:** PASS（フィクスチャA の範囲。手順7〜9 は `setup.md` TC-10 = フィクスチャB のため未実行）
- **実行した手順:**

    ```sh
    pulsen add --workflow flaky --repo /tmp/pulsen-test/repo   # T5=20260814t095615-6th0icet
    pulsen tick x3 (2〜3秒間隔)                                # 起動 → spawn確認 → 判定
    pulsen tick                                               # 再起動
    ls -a "$PULSEN_HOME/worktrees/$T5/"
    pulsen tick x2                                            # spawn確認 → 判定
    pulsen tick                                               # 遷移
    cat /tmp/pulsen-test/notify.log
    touch /tmp/pulsen-test/check-broken
    pulsen add --workflow pr-review-watch --repo /tmp/pulsen-test/repo  # T19=20260814t095716-l2uc3zsm
    pulsen tick x3
    rm /tmp/pulsen-test/check-broken
    pulsen tick x3
    grep "$T19" /tmp/pulsen-test/notify.log
    ```

- **観測した結果:**
  - 手順2: サマリー `失敗を記録(1件): - 20260814t095615-6th0icet: 実行の失敗を記録しました(実行が終了コード 1 で終了しました)`。`.execution.state` = `failed`、`.counters.attempt_count` = 1、`.task_status` = `work`。`attempt-1/exit` の `.code` = 1。
  - 手順3: `起動: …`、`.execution` = `{"state":"launching","recorded_at":"…09:56:57Z"}`、`.current_attempt.number` = 2、`run_dir` = `…/attempt-2`。worktree に `.git` と `done.marker`（リトライ間で引き継がれている）。
  - 手順4: attempt-2 の `exit.code` = 0 → `判定確定`、`.execution.state` = `completed`、`.counters.attempt_count` = **0 にリセット**。
  - 手順5: `遷移` → `.task_status` = `done`、`.execution.state` = `pending`。
  - 手順6: `notify.log` は空。`grep -c "$T5"` = 0。
  - 手順10: `失敗を記録(1件): - 20260814t095716-l2uc3zsm: 実行の失敗を記録しました(判定コマンドが失敗と判定しました(実行の終了コードは 1))`。`.execution.state` = `failed`、`.counters.attempt_count` = 1、`.task_status` = `watch`。
  - 手順11: attempt-2 の `exit.code` = 20 → `実行待ちへ復帰: 20260814t095716-l2uc3zsm`。`.execution.state` = `pending`、`.counters.attempt_count` = **0 にリセット**、`.task_status` = `watch`。
  - 手順12: `grep "$T19" notify.log` は非0（該当なし）。
- **期待との一致:** 一致。`.task_status` は全経過を通じて `work` / `watch` から動かず、`.counters.spawn_fail_count` は終始 0。
- **補足（testing.md の確認ポイントとの差）:** 確認ポイントの「`.last_failure` が failed の間だけ残り」は観測できない — エージェント実行の失敗（`fail_run`）では `.last_failure` は `null` のままだった。`spec/domains/task.md` は `FailureNote` を「ツール操作の失敗および判定失敗の記録」と定義しており、`crates/pulsen-domain/src/task/task.rs` の `fail_run` も `last_failure` を書かない。実装は spec どおりで、testing.md の記述のほうが誤り（後述）。

### 4. skipped によるポーリング周回（対応する手順書: task-execution TC-06 全手順、TC-07 手順1〜5 / 本書の手順1〜11）

- **判定:** PASS（手順12 は `setup.md` TC-11 = フィクスチャB のため未実行）
- **実行した手順:**

    ```sh
    ls /tmp/pulsen-test/review-flag /tmp/pulsen-test/check-broken 2>/dev/null; echo $?   # 2 (いずれも無い)
    pulsen add --workflow pr-review-watch --repo /tmp/pulsen-test/repo   # T6=20260814t095748-pfeksfby
    pulsen tick x2 → pulsen tick(判定) → pulsen tick(再起動)
    cat /tmp/pulsen-test/notify.log
    touch /tmp/pulsen-test/review-flag; pulsen tick x2
    pulsen tick x4      # attempt3 起動 → spawn確認 → 判定 → 遷移
    pulsen tick x4      # fix の1周
    rm /tmp/pulsen-test/review-flag; pulsen tick x3
    git -C /tmp/pulsen-test/repo log --oneline "pulsen/$T6"
    ```

- **観測した結果:**
  - 手順4: `実行待ちへ復帰: …, 20260814t095748-pfeksfby`。`.task_status` = `watch`、`.execution.state` = `pending`、`.counters.attempt_count` = **0**。
  - 手順5: `ls state/runs/$T6/` = `attempt-1` `attempt-2`。
  - 手順6: `notify.log` 空、`grep -c "$T6"` = 0。
  - 手順7: フラグ作成前に起動済みの attempt-2 は `exit.code` = 20 → 再び `実行待ちへ復帰`、`watch` / `pending` / `attempt_count` 0。
  - 手順8: attempt-3 の `exit.code` = 0 → `判定確定` → 次 tick の `遷移` で `.task_status` = `fix` / `pending`。
  - 手順9: attempt-4（`fix`）が `exit.code` = 0 → `判定確定` → `遷移` で `.task_status` = `watch` に戻る（循環）。
  - 手順10: フラグ削除後 attempt-5 が `exit.code` = 20 → `実行待ちへ復帰`、`.counters.attempt_count` は一貫して 0。
  - 手順11: `git log --oneline pulsen/$T6` = `d24cb65 fix` / `4c3c62b init`。
- **期待との一致:** 一致。skipped の tick は `.task_status` を書き換えず、`attempt_count` / `judge_attempt_count` は周回を通じて 0 のまま。フラグの効き目が「フラグ作成後に起動された attempt から」であることも手順7 と手順8 の対比で確認できた。

### 5. デフォルト判定は2値 — exit 20 も exit 127 も failed（対応する手順書: task-execution TC-23 全手順、TC-17 全手順）

- **判定:** PASS
- **実行した手順:**

    ```sh
    pulsen add --workflow exit20 --repo /tmp/pulsen-test/repo    # T23=20260814t095918-82lcdsk2
    pulsen add --workflow broken --repo /tmp/pulsen-test/repo    # T17=20260814t095918-fojf8rnq
    pulsen tick x3 (2〜3秒間隔)
    cat state/runs/$T23/attempt-1/exit ; cat state/tasks/$T23.json
    cat state/runs/$T17/attempt-1/exit ; cat state/tasks/$T17.json
    grep "$T23" /tmp/pulsen-test/notify.log ; grep "$T17" /tmp/pulsen-test/notify.log
    ```

    ※ TC-23 と TC-17 は独立なので、2件を同じ tick 列で同時に流した（testing.md「tick のサマリーには同時に走っている他タスクの処理も含まれる」に従い、対象タスクIDの行だけで判定した）。

- **観測した結果:**
  - 判定 tick のサマリー:

      ```
      凍結: 20260814t095918-82lcdsk2, 20260814t095918-fojf8rnq
      通知: 20260814t095918-82lcdsk2, 20260814t095918-fojf8rnq
      失敗を記録(2件):
        - 20260814t095918-82lcdsk2: 実行の失敗を記録しました(実行が終了コード 20 で終了しました)
        - 20260814t095918-fojf8rnq: 実行の失敗を記録しました(実行が終了コード 127 で終了しました)
      ```

  - 手順3: T23 の `exit.code` = 20。`.execution` = `{"state":"stopped","reason":"retry_limit_exceeded","notified_at":"2026-08-14T09:59:37Z"}`、`.counters.attempt_count` = 1。pending 復帰（skipped）にはならなかった。
  - 手順4: `stopped 20260814t095918-82lcdsk2 exit20 work` が1行。
  - 手順7: T17 の `exit.code` = 127。`.execution` = `{"state":"stopped","reason":"retry_limit_exceeded","notified_at":"…"}`、`.counters.spawn_fail_count` = **0**、`.last_failure` = `null`（`spawn_fail` ではない）。`state/runs/$T17/attempt-1/` に `exit` `pid` `starttime` `stdout.log` `stderr.log` が作られている（起動不能でも attempt は採番される）。
  - 手順8: `stopped 20260814t095918-fojf8rnq broken work` が1行。
- **期待との一致:** 一致。exit 20 の扱いが判定コマンドの有無で変わること（確認項目4 の T6 は skipped、ここでは failed）が同一フィクスチャ上で対比できた。TC-16（spawn 失敗）との経路差も `spawn_fail_count` 0 と attempt 採番ありで識別できる。

### 7. リトライ上限の等号と超過 — 凍結と at-least-once 通知（対応する手順書: task-execution TC-13 全手順、TC-22 全手順 / 本書の手順1〜8）

- **判定:** PASS（手順9〜11 は `intervention.md` TC-01 = フィクスチャC のため未実行）
- **実行した手順:**

    ```sh
    pulsen add --workflow fail --repo /tmp/pulsen-test/repo    # T13=20260814t095946-7z1hh29n
    pulsen tick x3 → grep "$T13" notify.log → pulsen tick x3 → grep "$T13" notify.log
    ls -a "$PULSEN_HOME/worktrees/$T13"; git -C /tmp/pulsen-test/repo branch --list "pulsen/$T13"
    md5 -q state/tasks/$T13.json; pulsen tick; md5 -q state/tasks/$T13.json; grep -c "$T13" notify.log
    pulsen add --workflow fail0 --repo /tmp/pulsen-test/repo   # T22=20260814t100024-x8c0a5dz
    pulsen tick x3
    ```

- **観測した結果:**
  - 手順2: `.execution.state` = `failed`、`.counters.attempt_count` = **1（= 上限 1。等号では凍結しない）**、`.task_status` = `work`、`exit.code` = 1。
  - 手順3: `grep "$T13" notify.log` が非0（通知なし）。
  - 手順4: サマリーに `凍結: 20260814t095946-7z1hh29n` と `通知: 20260814t095946-7z1hh29n` の**両方**。`.execution` = `{"state":"stopped","reason":"retry_limit_exceeded","notified_at":"2026-08-14T10:00:11Z"}`、`.counters.attempt_count` = 2、`.task_status` = `work`。
  - 手順5: `stopped 20260814t095946-7z1hh29n fail work` がちょうど1行。
  - 手順6: worktree（`ls -a` で `.git`）もブランチ `+ pulsen/20260814t095946-7z1hh29n` も残存。
  - 手順7: tick の exit code 0。md5 は前後とも `5097c5008e83dc676d8be6ce935c8285`、`grep -c "$T13" notify.log` は 1 のまま。当該 tick のサマリーに T13 は現れなかった。
  - 手順8: T22 は `.execution` = `{"state":"stopped","reason":"retry_limit_exceeded","notified_at":"2026-08-14T10:00:34Z"}`、`.counters.attempt_count` = 1、run ディレクトリは `attempt-1` のみ、`stopped 20260814t100024-x8c0a5dz fail0 work` が1行。
- **期待との一致:** 一致。通知は「凍結を保存した同じ tick 内」で起き、次の tick に持ち越されていない。凍結後の tick は `.updated_at` を動かさない（md5 不変で確認）。
- **手順書上の注意:** 手順6 の `ls "$PULSEN_HOME/worktrees/$T13"` は worktree の中身が `.git` だけなので**何も出力しない**。`ls -a` にしないと「worktree が残っている」ことを確認できない（後述）。

### 8. 判定失敗の上限超過 — エージェントを再実行せずに凍結（対応する手順書: task-execution TC-15 全手順 / 本書の手順1〜7）

- **判定:** PASS（手順8 は `setup.md` TC-37 = フィクスチャB のため未実行）
- **実行した手順:**

    ```sh
    pulsen add --workflow judgefail --repo /tmp/pulsen-test/repo   # T15=20260814t100043-rruipe8b
    pulsen tick x2 → cat state/runs/$T15/attempt-1/exit
    pulsen tick        # 判定1回目
    pulsen tick x2     # 判定2〜3回目
    pulsen tick        # 判定4回目
    ls state/runs/$T15/ ; grep "$T15" /tmp/pulsen-test/notify.log
    ```

- **観測した結果:**
  - 手順2: `exit.code` = 0（エージェント自体は成功）。
  - 手順3: サマリー `失敗を記録(1件): - 20260814t100043-rruipe8b: 判定できず判定失敗として記録しました(判定コマンドがプロトコル外の終了コード 1 を返しました(有効な値は 0 / 10 / 20))`。`.counters.judge_attempt_count` = 1、`.execution.state` = **`running` のまま**、`.counters.attempt_count` = 0、`.last_failure` = `{"kind":"judge_fail","message":"判定コマンドがプロトコル外の終了コード 1 を返しました(有効な値は 0 / 10 / 20)","at":"2026-08-14T10:00:53Z"}`。
  - 手順4: `judge_attempt_count` が 2 → 3 と増え、`.execution.state` は両方とも `running`。
  - 手順5: 4回目で `judge_attempt_count` = 4、`.execution` = `{"state":"stopped","reason":"judge_limit_exceeded","notified_at":"2026-08-14T10:01:10Z"}`。サマリーに `凍結` と `通知` の両方。
  - 手順6: run ディレクトリは `attempt-1` のみ、`.counters.attempt_count` は 0 のまま。
  - 手順7: `stopped 20260814t100043-rruipe8b judgefail work` が1行。
- **期待との一致:** 一致。判定は tick ごとにやり直され（`judge_attempt_count` が毎 tick +1、`last_failure.at` も毎回更新）、エージェントの再実行は一度も起きていない。`reason` は `judge_limit_exceeded` で `retry_limit_exceeded` ではない。

### 10. timeout 超過での kill と failed、実行中の連続 tick の冪等性（対応する手順書: task-execution TC-14 全手順）

- **判定:** PASS
- **実行した手順:**

    ```sh
    pgrep -fl 'sleep '                                         # 事前に他の sleep が無いことを確認
    pulsen add --workflow sleeper --repo /tmp/pulsen-test/repo  # T14=20260814t100120-1hm7bp1m
    pulsen tick x2 → タスクファイルから P14=19701 を取得
    md5 -q state/tasks/$T14.json; pulsen tick; pulsen tick; md5 -q state/tasks/$T14.json   # 間を置かず
    sleep して起動から10秒超 → pulsen tick
    ps -p "$P14"; pgrep -fl 'sleep 120'
    ls state/runs/$T14/attempt-1/
    pulsen tick x2 → 10秒以上待って pulsen tick
    grep "$T14" notify.log; ls -a worktrees/$T14
    ```

- **観測した結果:**
  - 手順2: `.execution.state` = `running`、`process` = `{"pid":19701,"kill_ident":"-19701","starttime":{"ident":"Fri Aug 14 10:01:26 2026","wall":"2026-08-14T10:01:26Z"}}`。`pgrep -fl 'sleep 120'` はラッパー(19701)と `sleep 120`(19727)の2件のみ。
  - 手順3: 起動から約9秒の時点で2回連続 tick。両方 exit code 0（1回目は他タスクの行のみ、2回目は `処理対象のタスクはありませんでした。`）、md5 は前後とも `37faac6013d9090837d5f398bfd1ca62`。**T14 はサマリーに現れなかった**。
  - 手順4: 起動から21秒後の tick で `失敗を記録(1件): - 20260814t100120-1hm7bp1m: 実行の失敗を記録しました(実行が timeout(10秒)を超えたため終了させました)`。`.execution.state` = `failed`、`.counters.attempt_count` = 1、`.task_status` = `work`。
  - 手順5: `ps -p 19701` が exit 1（該当なし）、`pgrep -f 'sleep 120'` も exit 1。ラッパーも子の `sleep 120` も残っていない。
  - 手順6: `attempt-1/` の中身は `pid` `starttime` `stdout.log` `stderr.log` — **`exit` は無い**。
  - 手順7: attempt-2 が `starttime.wall` = `10:01:58Z` で起動し、12秒後の tick で `凍結` + `通知`、`.execution` = `{"state":"stopped","reason":"retry_limit_exceeded","notified_at":"2026-08-14T10:02:13Z"}`、`.counters.attempt_count` = 2。
  - 手順8: `stopped 20260814t100120-1hm7bp1m sleeper work` が1行。worktree（`.git`）は保持。`pgrep -f 'sleep 120'` は該当なし。
- **期待との一致:** 一致。経過の起点が `starttime.wall` であること（手順3 の連続 tick で早期 kill が起きない／手順4 で確実に超過判定される）と、kill が `kill_ident`（`-19701`）で実行単位ごと消滅させていることが確認できた。他の `sleep` を巻き込んだ形跡はない。

### 11. exit 記録なしのプロセス死亡の検出と自動リトライ（対応する手順書: task-execution TC-21 手順1〜5 / 本書の手順1〜6）

- **判定:** PASS
- **実行した手順:**

    ```sh
    pulsen add --workflow longrun --repo /tmp/pulsen-test/repo   # T21 =20260814t100221-knobsoef
    pulsen add --workflow longrun --repo /tmp/pulsen-test/repo   # T21B=20260814t100221-ms28juhf(誤殺の対照用)
    pulsen tick x2 → P21=20262 / kill_ident="-20262" を取得
    pgrep -fl 'sleep 600'
    kill -9 -- "-20262"
    ls state/runs/$T21/attempt-1/
    pulsen tick   # 手順5
    pulsen tick   # 手順6
    ```

    ※ testing.md 手順1 は longrun を1件しか登録しないが、確認ポイントが「tick 後に**他タスク由来の sleep が残っている**ことを確認する」と要求しているため、対照用に2件目を登録した。

- **観測した結果:**
  - 手順2: T21 `pid=20262` / `kill_ident="-20262"`（`-<pgid>` 形式）、T21B `pid=20326`。`pgrep -fl 'sleep 600'` は 20262 / 20323 / 20326 / 20352 の4件。
  - 手順3: `kill -9 -- "-20262"` が exit 0。直後の `pgrep -fl 'sleep 600'` は T21B 由来の 20326 / 20352 のみ。
  - 手順4: `attempt-1/` は `pid` `starttime` `stdout.log` `stderr.log` — `exit` は無い。
  - 手順5: tick exit code 0。

      ```
      失敗を記録(1件):
        - 20260814t100221-knobsoef: 実行の失敗を記録しました(実行が終了コードを残さずに終わりました)
      後始末が残っている(1件):
        - 20260814t100221-knobsoef: 残存プロセスを誤殺なく同定できませんでした(終了操作は行っていません)
      ```

      `.execution.state` = `failed`、`.counters.attempt_count` = 1、`.task_status` = `work`。`attempt-1/exit` は依然として存在しない。T21B は `running` のまま無傷で、その `sleep 600`（20326 / 20352）も残っていた。
  - 手順6: `起動: 20260814t100221-knobsoef`。`.execution` = `{"state":"launching","recorded_at":"2026-08-14T10:02:57Z"}`、`.current_attempt.number` = 2、`run_dir` = `…/attempt-2`。`state/runs/$T21/` は `attempt-1` `attempt-2`。
- **期待との一致:** 一致。`後始末が残っている` の報告は testing.md が「残存終了の結果は報告として現れうるが、状態の分類には影響しない」と明記している範囲内（プロセスグループごと消滅済みで誤殺なく同定できないため）。`try_kill_remnants` が無関係なプロセス（T21B の `sleep 600`）を巻き込んでいないことも確認できた。

## エッジケース・異常系

### 4. 手動修復で不変条件が破れたタスクの報告とスキップ（前提: フィクスチャA。対応する手順書の記載なし）

- **判定:** PASS
- **実行した手順:**

    ```sh
    pulsen add --workflow pipeline --repo /tmp/pulsen-test/repo   # TX=20260814t100303-6cr4924l
    pulsen tick x2  → cp state/tasks/$TX.json /tmp/pulsen-test/tx.bak
    jq '.execution = {"state":"running"} | .current_attempt = null' tx.bak > … && mv …
    md5 -q …; pulsen tick; md5 -q …
    jq '.execution = {"state":"running"} | .current_attempt.process = null' tx.bak > … && mv …
    md5 -q …; pulsen tick; md5 -q …
    cp /tmp/pulsen-test/tx.bak "$PULSEN_HOME/state/tasks/$TX.json"    # 復元
    ```

- **観測した結果:**
  - 手順3（不変条件2 の破れ）: tick exit code 0、`スキップ(1件): - 20260814t100303-6cr4924l: 観測の前提となる現在 attempt がありません(タスクファイルの修復が必要です)`。md5 は前後とも `4dded426543ab8751596500e333cb399`。
  - 手順4（不変条件3 の破れ）: tick exit code 0、`スキップ(1件): - 20260814t100303-6cr4924l: 起動確認済みですが同定情報がありません(pid ファイルからの修復が必要です)`。md5 は前後とも `dfd716eb037be2d38165084ebd4290ca`、内容も `.execution.state` = `running` / `.current_attempt.process` = `null` のまま。
  - どちらの tick でも他タスク（T6 / T19）の処理は同一 tick で継続していた。
- **期待との一致:** 一致。2つの破れは「現在 attempt がありません」と「同定情報がありません」という**区別できる文言**で報告され、`stopped` 化もパニックも起きなかった。

### 5. run ファイル(exit)の破損での滞留（前提: フィクスチャA。対応する手順書の記載なし）

- **判定:** PASS
- **実行した手順:**

    ```sh
    pulsen add --workflow pipeline --repo /tmp/pulsen-test/repo   # TR=20260814t100331-wf644tuk
    pulsen tick x2 → until [ -f state/runs/$TR/attempt-1/exit ]; do sleep 1; done
    cp state/runs/$TR/attempt-1/exit /tmp/pulsen-test/tr-exit.bak
    echo 'broken' > state/runs/$TR/attempt-1/exit
    md5 -q state/tasks/$TR.json; pulsen tick; md5 -q state/tasks/$TR.json
    cp /tmp/pulsen-test/tr-exit.bak state/runs/$TR/attempt-1/exit; pulsen tick
    ```

- **観測した結果:**
  - 手順4: tick exit code 0、`スキップ(1件): - 20260814t100331-wf644tuk: runディレクトリのファイルを読めません(/tmp/pulsen-test/home/state/runs/20260814t100331-wf644tuk/attempt-1/exit を解釈できない: 内容を解釈できない: expected value at line 1 column 1)`。md5 は前後とも `ffc03044afa913bfc89042561df75201`。
  - 手順5: `.execution.state` = `running` のまま、`.counters` 全0。壊した `exit` ファイルの内容は `broken` のまま（tick が書き換えも削除もしていない）。
  - 手順6: `exit` を戻した次の tick で `判定確定: 20260814t100331-wf644tuk`、`.execution.state` = `completed`。
- **期待との一致:** 一致。`Corrupt` を「exit なし」と取り違えて生存観測へ進んでいない（失敗記録も再起動も起きなかった）。

### 6. 1タスクの失敗が他タスクを止めない・冪等な連続 tick（対応する手順書: task-execution TC-20 手順1〜4・6）

- **判定:** PASS
- **実行した手順:**

    ```sh
    pulsen add --workflow pipeline --repo /tmp/pulsen-test/repo  # T20 =20260814t100401-r3jhqtbb
    pulsen add --workflow pipeline --repo /tmp/pulsen-test/repo  # T20P=20260814t100401-67wzpa62（draft.yaml の代替）
    cp state/tasks/$T20.json /tmp/pulsen-test/t20.bak && echo broken > state/tasks/$T20.json
    pulsen tick
    cat state/tasks/$T20.json ; cat state/tasks/$T20P.json ; grep -c "$T20" notify.log
    md5 -q state/tasks/*.json > before.md5 ; pulsen tick; sleep 3; pulsen tick ; md5 -q state/tasks/*.json > after.md5 ; diff
    cp /tmp/pulsen-test/t20.bak state/tasks/$T20.json    # 復元
    ```

- **観測した結果:**
  - 手順3: tick exit code **0**。

      ```
      起動: 20260814t100401-67wzpa62
      判定確定: 20260814t100303-6cr4924l
      遷移: 20260814t100331-wf644tuk
      実行待ちへ復帰: 20260814t095716-l2uc3zsm, 20260814t095748-pfeksfby
      スキップ(1件):
        - /tmp/pulsen-test/home/state/tasks/20260814t100401-r3jhqtbb.json: タスクファイルを読めません(expected value at line 1 column 1)
      ```

      報告は**タスクIDではなくファイルパス**で出る（読めないのでIDが取れないため。妥当）。
  - 手順4: T20 の内容は `broken` のまま。T20P は同一 tick で `launching` に入り、以後 `running` → `completed` と通常どおり進んだ。
  - 手順5: `grep -c "$T20" notify.log` = 0。
  - 手順6: 全16タスクファイルの md5 を2 tick 前後で比較。差分が出たのは5件のみで、いずれもその2 tick でちょうど2ステップ進んだタスクだった:

      | タスク | before | after |
      | --- | --- | --- |
      | 20260814t095716-l2uc3zsm (T19) | watch/pending/attempt20 | watch/running/attempt21 |
      | 20260814t095748-pfeksfby (T6) | watch/pending/attempt18 | watch/running/attempt19 |
      | 20260814t100303-6cr4924l (TX) | planned/completed/attempt2 | implemented/launching/attempt3 |
      | 20260814t100331-wf644tuk (TR) | planned/pending/attempt1 | planned/running/attempt2 |
      | 20260814t100401-67wzpa62 (T20P) | queued/launching/attempt1 | queued/completed/attempt1 |

      差分が出なかったのは `done` 滞留2件（T3 / T5）、`stopped` 6件（T23 / T17 / T13 / T22 / T15 / T14）、`timeout: none` で `running` 滞留の longrun 2件、破損した T20 の計11件。
- **期待との一致:** 一致。走査は1件の失敗で打ち切られない。`stopped` かつ通知済みのタスクは毎 tick 無反応（md5 不変・通知行不変）、`done` 到達済みのタスクは報告にも現れなかった。差分の出た5件は testing.md が明示する「判定待ちのタスクが混じっていると差分が出る」ケースで、いずれも1 tick 1ステップの範囲内。

## 実行しなかった項目（他フィクスチャ担当）

| 項目 | 対応する手順書 | 理由 |
| --- | --- | --- |
| 確認項目2 | `setup.md` TC-09 | フィクスチャB |
| 確認項目3 手順7〜9 | `setup.md` TC-10 | フィクスチャB |
| 確認項目4 手順12 | `setup.md` TC-11 | フィクスチャB |
| 確認項目6 | `setup.md` TC-47 | フィクスチャB |
| 確認項目7 手順9〜11 | `intervention.md` TC-01 | フィクスチャC |
| 確認項目8 手順8 | `setup.md` TC-37 | フィクスチャB |
| 確認項目9 | `intervention.md` TC-24 / TC-15、`setup.md` TC-35 | フィクスチャC |
| エッジケース1 | （手順書なし。前提がフィクスチャC） | フィクスチャC |
| エッジケース2 | `setup.md` TC-38 | フィクスチャB |
| エッジケース3 | `setup.md` TC-39 | フィクスチャB |

「既存機能への影響確認」節（`--help` 表示・grep による隔離確認・ロックFDの非継承など）も確認項目ではないため実行していない。

## testing.md の記述の問題

1. **確認項目3 の確認ポイント「`.last_failure` が failed の間だけ残り」は誤り。**
   エージェント実行の失敗（`fail_run`）では `.last_failure` は `null` のままだった（確認項目3 手順2、確認項目5 手順7、確認項目10 手順4、確認項目11 手順5 のいずれでも `null`）。`spec/domains/task.md` は `FailureNote` を「ツール操作の失敗(worktree作成・削除、アーカイブ移動、spawn失敗)および判定失敗の記録」と定義し、`crates/pulsen-domain/src/task/task.rs:446` の `fail_run` も `last_failure` を書かない。実装が spec どおりで、testing.md の確認ポイントが実態と合っていない。実行失敗の要因は run ディレクトリの `exit` / `stderr.log` と tick の報告文から読む。
   （判定失敗の `.last_failure.kind = judge_fail` は確認項目8 手順3 で期待どおり記録されることを確認済み。確認項目5 手順7 の「`.last_failure.kind` は `spawn_fail` ではない」も `null` で満たされる。）

2. **確認項目7 手順6 の `ls "$PULSEN_HOME/worktrees/$T13"` は何も出力しない。**
   `fail` ワークフローの worktree にはコミット成果物が無く、中身が `.git` だけになるため。「worktree が残っている」ことを確認するには `ls -a` が必要。同じ問題が確認項目10 手順8 の `ls "$PULSEN_HOME/worktrees/$T14"` にもある。

3. **確認項目11 の確認ポイントが要求する対照が、手順1 だけでは作れない。**
   確認ポイントは「tick 後に**他タスク由来の sleep が残っている**ことを確認する」としているが、手順1 は `longrun` を1件しか登録しない。本確認では2件目の `longrun` を登録して対照を作った。手順1 に「`longrun` を2件登録する」と明記するのが妥当。

4. **エッジケース4・5 に「対応する手順書」の行が無い。**
   どちらも前提がフィクスチャA なので実質フィクスチャA 担当だが、担当分けを「対応する手順書に挙がっている TC」で決めると宙に浮く。本確認では実行した（結果は上記のとおり PASS）。

## 後片付け（フィクスチャA 分）

```sh
pkill -f 'sleep 600'                                   # T21 / T21B の残存プロセス
ps -ef | grep -E 'pulsen wrapper|sleep (120|180|600)' | grep -v grep   # 該当なし
ls -a "$HOME/.pulsen"                                  # 存在しない（実運用ホームは非汚染）
```

`sleep 120` / `sleep 180` の残存は無かった（`sleep 600` は確認項目11 の `timeout: none` タスクが再実行を続けるためのもので、testing.md が織り込み済み）。`/tmp/pulsen-test` はメインが結果を追検証できるよう残してある。
