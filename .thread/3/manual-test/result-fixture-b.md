# 手動確認の結果 — フィクスチャB

**実行日:** 2026-08-14
**環境:** macOS 26.4.1 (25E253) / 非root（uid=501, hikaru）/ TMPDIR=/var/folders/l_/pptqtzd976329fq6qp57cgh40000gn/T/
**ブランチ:** `issue/3/tick-observe-judge-transition`
**バイナリ:** `/Users/hikaru/github.com/tuanemuy/pulsen/target/debug/pulsen`（メインがビルド済みのものを使用）
**対象:** `.thread/3/testing.md` の確認項目のうち `spec/manual-tests/setup.md` 由来のもの（TC-09 / 10 / 11 / 35 / 37 / 38 / 39 / 47）

## サマリー

- 実行: 8件（PASS: 7 / FAIL: 0 / 実行不可: 1）
- 実行不可の1件は setup TC-35。testing.md が TC-35 をフィクスチャC（確認項目9）で消化する設計になっているため、フィクスチャB では実行対象が存在しない（詳細は当該節）。

| 項目 | 由来 TC | 判定 |
|---|---|---|
| 確認項目2 | setup TC-09 全手順 | PASS |
| 確認項目3 手順7〜9 | setup TC-10 手順1〜4 | PASS |
| 確認項目4 手順12 | setup TC-11 手順1〜4 | PASS |
| 確認項目6 | setup TC-47 手順1〜2 | PASS |
| 確認項目8 手順8 | setup TC-37 手順1〜3 | PASS |
| 確認項目9（setup TC-35 部分） | setup TC-35 | 実行不可（フィクスチャC担当） |
| エッジケース2 | setup TC-38 手順1〜2 | PASS |
| エッジケース3 | setup TC-39 手順1〜3・4の復元 | PASS |

## 事前確認とセットアップ

`$HOME/pulsen-manual-test` / `$HOME/pulsen-test-repo` / `$HOME/pulsen-manual-work` はいずれも実行前に存在しなかった（`ls` が `No such file or directory`）。無関係なデータを消す危険がないことを確認したうえで、testing.md「フィクスチャB」の手順をそのまま実行した。

フィクスチャA（`/tmp/pulsen-test`）とフィクスチャC（`$HOME/pulsen-intervention-test`）には一切触れていない。

タスクIDの対応:

| 変数 | タスクID | ワークフロー |
|---|---|---|
| T9 | `20260814t095517-6876ggnd` | judge-demo |
| T10 | `20260814t095548-gs121eb4` | judge-demo |
| T11 | `20260814t095646-y72j88ty` | judge-demo |
| T47 | `20260814t095717-5mi6gixs` | sigkill |
| T37 | `20260814t095737-fzjj9wsf` | judge-demo |
| T38 | `20260814t095814-h33qrtsl` | judge-missing |
| T39 | `20260814t095834-jzthug19` | judge-hang |

## 確認項目ごとの結果

### 2. judge 定義ありでの exit 0 → completed → 次ステータスへ（対応する手順書: setup TC-09 全手順）

- **判定:** PASS
- **実行した手順:**

    ```sh
    export PULSEN_HOME="$SETUP_HOME"
    echo 0 > "$SETUP_HOME/judge-exit"; : > "$SETUP_HOME/judge.log"
    pulsen add --workflow judge-demo --repo "$SETUP_REPO"   # → T9
    pulsen tick                # 起動
    sleep 3; pulsen tick       # spawn確認
    sleep 3; pulsen tick       # 判定
    cat "$SETUP_HOME/judge.log"
    jq -c '{task_status, execution, counters}' "$SETUP_HOME/state/tasks/$T9.json"
    cat "$SETUP_HOME/state/runs/$T9/attempt-1/exit"
    sleep 3; pulsen tick       # 遷移
    ```

- **観測した結果:**
  - 4つの tick のサマリーはそれぞれ `起動:` / `起動確認:` / `判定確定:` / `遷移:` に T9 が1回ずつ現れた。すべて exit code 0。
  - `judge.log`: `judged: task=20260814t095517-6876ggnd exit=0` が**1行だけ**。
  - 判定 tick 直後: `{"task_status":"checking","execution":{"state":"completed"},"counters":{"attempt_count":0,"judge_attempt_count":0,"spawn_fail_count":0}}`
  - `attempt-1/exit`: `{\n  "code": 0\n}`（整形 JSON。ADR-080 どおり）
  - 遷移 tick 直後: `{"task_status":"finished","execution":{"state":"pending"},"counters":{"attempt_count":0,"judge_attempt_count":0,"spawn_fail_count":0}}`
  - `notify.log` は最後まで作られなかった（通知なし）。
  - 以降の全 tick（本結果の最後まで計20回以上）で T9 はサマリーに一度も現れず、`updated_at` は遷移 tick の `2026-08-14T09:55:38Z` のまま動かなかった（`finished` は `run: wait`）。
- **期待との一致:** 一致。判定の tick が `advance` まで進まないこと（1タスク1tick1ステップ）、`EXIT_CODE` が10進文字列 `0` として渡ること、`judge` が配列形式でも `$HOME` の展開はコマンド側の `sh -c` が行っていること（judge.log が書けている＝パスが解決できている）をすべて確認した。

### 3（setup 側 手順7〜9）. judge の exit 10 が failed 経路に落ちる（対応する手順書: setup TC-10 手順1〜4）

- **判定:** PASS
- **実行した手順:**

    ```sh
    echo 10 > "$SETUP_HOME/judge-exit"
    pulsen add --workflow judge-demo --repo "$SETUP_REPO"   # → T10
    pulsen tick; sleep 3; pulsen tick; sleep 3; pulsen tick   # 起動 → spawn確認 → 判定
    jq -c '...' "$SETUP_HOME/state/tasks/$T10.json"
    sleep 3; pulsen tick                                     # 再起動
    ls "$SETUP_HOME/state/runs/$T10/"
    echo 0 > "$SETUP_HOME/judge-exit"
    sleep 3; pulsen tick; sleep 3; pulsen tick               # spawn確認 → 判定
    ```

- **観測した結果:**
  - 判定 tick のサマリー: `失敗を記録(1件): - 20260814t095548-gs121eb4: 実行の失敗を記録しました(判定コマンドが失敗と判定しました(実行の終了コードは 0))`
  - 手順7 のタスクファイル: `{"task_status":"checking","execution":{"state":"failed"},"counters":{"attempt_count":1,"judge_attempt_count":0,"spawn_fail_count":0},"attempt":1,"last_failure":null}`
  - 手順8: サマリー `起動: 20260814t095548-gs121eb4`、`ls runs/T10` が `attempt-1` / `attempt-2`、タスクファイルは `launching` / `attempt: 2` / `run_dir` が `.../attempt-2`。
  - 手順9: `起動確認:` → `判定確定:` の順に現れ、`{"task_status":"checking","execution":{"state":"completed"},"counters":{"attempt_count":0,...},"attempt":2}`。`attempt_count` が 0 にリセットされた。
  - `notify.log` は未作成のまま（通知なし）。`judge.log` は T10 について2行（attempt-1 判定と attempt-2 判定）。
- **期待との一致:** 一致。`task_status` は終始 `checking`、`spawn_fail_count` は終始 0。
- **付記（testing.md の記述との差）:** 確認項目3 の**確認ポイント**は「`.last_failure` が failed の間だけ残り」としているが、judge exit 10 → `fail_run` の経路では `.last_failure` は `null` のままだった。実装（`crates/pulsen-domain/src/task/task.rs` の `fail_run`）は `last_failure` を意図的に触っておらず、`last_failure` を書くのは `record_spawn_failure` / `record_tool_failure` / `record_judge_failure` のみ。**期待結果**の本文には `last_failure` の記載がなく、観測値は期待結果と矛盾しないため PASS としたが、確認ポイントの文言は実装（＝設計）と食い違っている。テストの誤りではなく testing.md の記述の問題として報告する。

### 4（setup 側 手順12）. judge の exit 20 が skipped になる（対応する手順書: setup TC-11 手順1〜4）

- **判定:** PASS
- **実行した手順:**

    ```sh
    echo 20 > "$SETUP_HOME/judge-exit"
    pulsen add --workflow judge-demo --repo "$SETUP_REPO"   # → T11
    pulsen tick; sleep 3; pulsen tick; sleep 3; pulsen tick
    jq -c '...' "$SETUP_HOME/state/tasks/$T11.json"; cat "$SETUP_HOME/notify.log"
    sleep 3; pulsen tick; ls "$SETUP_HOME/state/runs/$T11/"
    ```

- **観測した結果:**
  - 3回目の tick のサマリー: `実行待ちへ復帰: 20260814t095646-y72j88ty`（「失敗を記録」ではない）
  - タスクファイル: `{"task_status":"checking","execution":{"state":"pending"},"counters":{"attempt_count":0,"judge_attempt_count":0,"spawn_fail_count":0},"attempt":1}`
  - `notify.log` は未作成（`cat` が rc=1）。T11 の通知行は0件。
  - 次の tick で `起動: 20260814t095646-y72j88ty`、`ls runs/T11` が `attempt-1` / `attempt-2`、タスクファイルの `attempt` が 2 かつ `attempt_count` は 0 のまま。
  - 同じ tick で T10 の `遷移:` も現れ、T10 が `finished` / `pending` になった（他タスクの処理が同一 tick で並行して進むこと）。
- **期待との一致:** 一致。skipped の tick が `task_status` を書き換えず、`attempt_count` を消費せず、通知も起こさないことを確認した。

### 6. シグナル死の符号化が EXIT_CODE として判定コマンドへ渡る（対応する手順書: setup TC-47 手順1〜2）

- **判定:** PASS
- **実行した手順:**

    ```sh
    echo 10 > "$SETUP_HOME/judge-exit"
    cat > "$SETUP_WORK/sigkill.yaml" <<'EOF'
    agent: shell
    initial: dying
    statuses:
      dying:
        prompt: "kill -KILL $$"
        judge: ["sh", "-c", "\"$HOME/pulsen-manual-test/judge.sh\""]
        next: waiting
      waiting:
        run: wait
    EOF
    pulsen add --workflow "$SETUP_WORK/sigkill.yaml" --repo "$SETUP_REPO"   # → T47
    pulsen tick; sleep 3; pulsen tick; sleep 3; pulsen tick
    cat "$SETUP_HOME/state/runs/$T47/attempt-1/exit"; tail -n 3 "$SETUP_HOME/judge.log"
    ```

- **観測した結果:**
  - `add` は成功し、ワークフロー表示名がファイル名から `sigkill` に解決された。
  - `attempt-1/exit`: `{\n  "code": 137\n}`（= 128+9）
  - `judge.log` の当該行: `judged: task=20260814t095717-5mi6gixs exit=137`
  - 判定 tick のサマリー: `失敗を記録(1件): - 20260814t095717-5mi6gixs: 実行の失敗を記録しました(判定コマンドが失敗と判定しました(実行の終了コードは 137))`
  - タスクファイル: `{"task_status":"dying","execution":{"state":"failed"},"counters":{"attempt_count":1,"judge_attempt_count":0,"spawn_fail_count":0},"attempt":1}`
- **期待との一致:** 一致。`EXIT_CODE` は `137` の10進文字列としてそのまま渡っており、`-9` や符号ビット表現になっていない。以降 T47 は failed → 再起動を繰り返し、後続項目のサマリーに現れた（`abort` が無いため止められない。想定どおり）。

### 8（setup 側 手順8）. 判定失敗の上限超過 — judge_limit_exceeded（対応する手順書: setup TC-37 手順1〜3）

- **判定:** PASS
- **実行した手順:**

    ```sh
    echo 1 > "$SETUP_HOME/judge-exit"
    pulsen add --workflow judge-demo --repo "$SETUP_REPO"   # → T37
    pulsen tick; sleep 3; pulsen tick; sleep 3              # 起動 → spawn確認
    # 判定 tick を4回（間 3s）
    ```

- **観測した結果:** T37 の推移（各 tick 後のタスクファイル）

    | 判定 tick | `execution` | `judge_attempt_count` | `attempt_count` | judge.log の T37 行数 |
    |---|---|---|---|---|
    | 1 | `running` | 1 | 0 | 1 |
    | 2 | `running` | 2 | 0 | 2 |
    | 3 | `running` | 3 | 0 | 3 |
    | 4 | `{"state":"stopped","reason":"judge_limit_exceeded","notified_at":"2026-08-14T09:58:03Z"}` | 4 | 0 | 4 |

  - `last_failure`: `{"kind":"judge_fail","message":"判定コマンドがプロトコル外の終了コード 1 を返しました(有効な値は 0 / 10 / 20)","at":...}`
  - 4回目の tick サマリー: `凍結: ..., 20260814t095737-fzjj9wsf` と `通知: ..., 20260814t095737-fzjj9wsf` の両方に T37 が現れた。
  - `notify.log` に `20260814t095737-fzjj9wsf judge-demo checking` の行が1行追記された。
  - `ls "$SETUP_HOME/state/runs/$T37/"` → `attempt-1` のみ。
  - すべての tick が exit code 0。
- **期待との一致:** 一致。上限（3）の**等号では凍結せず**、超過（4>3）で `judge_limit_exceeded`。エージェントの再実行は一度も起きず（attempt は 1 のまま・`attempt_count` は 0 のまま）、判定コマンドだけが tick ごとに再実行された（judge.log が毎 tick +1）。`retry_limit_exceeded` ではないことも確認。
- **付記:** 同じ4回の tick で、既存の T11（judge exit 20 のポーリング周回中）と T47 も `judge-exit` の値を共有するため同じ経路に落ち、いずれも `judge_limit_exceeded` で凍結・通知された。testing.md「実行上の注意」に従い、判定は T37 の行・状態のみで行った。

### 9（setup TC-35 部分）. notify_cmd 未定義でも stopped の確定は正常に動作する（対応する手順書: setup TC-35）

- **判定:** 実行不可（フィクスチャB では実行対象が存在しない）
- **理由:** testing.md の確認項目9 は、**前提を「フィクスチャC」と明記**し、手順1〜10 のすべてを `$PMT/home`（`$HOME/pulsen-intervention-test`）に対して実行する形で書かれている。setup TC-35 は「intervention TC-15 と同じ『notify_cmd 未定義 → 後から定義して catch-up』を問う」ものとして確認項目9 手順7〜10 の1系列に統合されており（testing.md:915 の記帳）、フィクスチャB 側で実行すべき手順は残っていない。フィクスチャC は別エージェントの担当のため、こちらでは実行していない。
- **付随して確認したこと:** フィクスチャB の `config.yaml` は本検証を通じて `notify_cmd` を一度も変更していない。最終状態は `$SETUP_WORK/config.bak` と `diff` して**完全一致**（差分なし）を確認済みで、TC-35 手順4 が要求する「notify_cmd を戻す」状態はフィクスチャB では満たされている。

### エッジケース2. 判定コマンドの実体が見つからない（対応する手順書: setup TC-38 手順1〜2）

- **判定:** PASS
- **実行した手順:**

    ```sh
    cat > "$SETUP_WORK/judge-missing.yaml" <<'EOF'
    agent: shell
    initial: checking
    statuses:
      checking:
        prompt: "echo hi"
        judge: /no/such/judge.sh
        next: waiting
      waiting:
        run: wait
    EOF
    pulsen add --workflow "$SETUP_WORK/judge-missing.yaml" --repo "$SETUP_REPO"; echo $?   # → T38
    pulsen tick; sleep 3; pulsen tick; sleep 3; pulsen tick
    ```

- **観測した結果:**
  - `add` は exit code 0 で成功（`judge` のコマンド実体は登録時に検証されない）。
  - 判定 tick のサマリー: `失敗を記録(1件): - 20260814t095814-h33qrtsl: 判定できず判定失敗として記録しました(判定コマンドを起動できませんでした: /no/such/judge.sh を起動できない: No such file or directory (os error 2))`
  - タスクファイル: `execution` = `{"state":"running"}`、`counters` = `{"attempt_count":0,"judge_attempt_count":1,"spawn_fail_count":0}`、`attempt` = 1、`last_failure` = `{"kind":"judge_fail","message":"判定コマンドを起動できませんでした: /no/such/judge.sh を起動できない: No such file or directory (os error 2)","at":"2026-08-14T09:58:24Z"}`
  - tick の exit code は 0。
- **期待との一致:** 一致。起動不能が `Failed`（エージェントの失敗）に誤分類されず、`attempt_count` を消費していない。その後 T38 は放置され、`judge_attempt_count` 4 で `judge_limit_exceeded` に到達して通知された（testing.md の想定どおり）。

### エッジケース3. 判定 timeout と tick のブロック（対応する手順書: setup TC-39 手順1〜3・4の復元）

- **判定:** PASS
- **実行した手順:**

    ```sh
    sed -i.bak 's/^judge_timeout: 60s$/judge_timeout: 5s/' "$SETUP_HOME/config.yaml"
    cat > "$SETUP_WORK/judge-hang.yaml" <<'EOF'
    agent: shell
    initial: checking
    statuses:
      checking:
        prompt: "echo hi"
        judge: ["sleep", "180"]
        next: waiting
      waiting:
        run: wait
    EOF
    pgrep -f 'sleep 180'                                        # 事前に該当なしを確認（rc=1）
    pulsen add --workflow "$SETUP_WORK/judge-hang.yaml" --repo "$SETUP_REPO"   # → T39
    pulsen tick; sleep 4; pulsen tick; sleep 3
    S=$(date +%s); pulsen tick; RC=$?; E=$(date +%s)            # 判定（ブロックする）
    pgrep -f 'sleep 180'
    cp "$SETUP_WORK/config.bak" "$SETUP_HOME/config.yaml"       # 復元
    rm -f "$SETUP_HOME/config.yaml.bak"
    ```

- **観測した結果:**
  - `judge_timeout: 5s` への変更を `grep` で確認（10行目）。
  - 判定 tick は **elapsed=5s**（実測）でブロックしてから exit code 0 で返った。
  - サマリー: `失敗を記録(2件): ... - 20260814t095834-jzthug19: 判定できず判定失敗として記録しました(判定コマンドが timeout までに終了しませんでした)`
  - タスクファイル: `execution` = `{"state":"running"}`、`counters.judge_attempt_count` = 1、`attempt_count` = 0、`last_failure` = `{"kind":"judge_fail","message":"判定コマンドが timeout までに終了しませんでした","at":"2026-08-14T09:58:51Z"}`
  - `pgrep -f 'sleep 180'` は rc=1（該当なし）。tick が返った時点で判定コマンドのプロセスは残っていない。
  - 復元: `grep -n judge_timeout` が `10:judge_timeout: 60s`。`diff "$SETUP_WORK/config.bak" "$SETUP_HOME/config.yaml"` が差分なし（完全復元）。`config.yaml.bak` も削除済み。
- **期待との一致:** 一致。ブロック時間が `judge_timeout`（5s）に対して桁違いに大きくなく、直接の子プロセスが timeout 超過で終了させられている。

## フィクスチャB の最終状態（引き継ぎ用）

| タスクID | ワークフロー | task_status | execution | counters |
|---|---|---|---|---|
| `20260814t095517-6876ggnd` | judge-demo | finished | pending | 0/0/0 |
| `20260814t095548-gs121eb4` | judge-demo | finished | pending | 0/0/0 |
| `20260814t095646-y72j88ty` | judge-demo | checking | stopped / judge_limit_exceeded / 通知済み | 1/4/0 |
| `20260814t095717-5mi6gixs` | sigkill | dying | stopped / judge_limit_exceeded / 通知済み | 1/4/0 |
| `20260814t095737-fzjj9wsf` | judge-demo | checking | stopped / judge_limit_exceeded / 通知済み | 0/4/0 |
| `20260814t095814-h33qrtsl` | judge-missing | checking | stopped / judge_limit_exceeded / 通知済み | 0/4/0 |
| `20260814t095834-jzthug19` | judge-hang | checking | running | 0/1/0 |

- `notify.log`（4行）:

    ```
    20260814t095646-y72j88ty judge-demo checking
    20260814t095717-5mi6gixs sigkill dying
    20260814t095737-fzjj9wsf judge-demo checking
    20260814t095814-h33qrtsl judge-missing checking
    ```

- `judge-exit` は最後に `1` のまま（TC-39 は judge-exit を読まない `sleep 180` を使うため影響なし）。
- `config.yaml` は `$SETUP_WORK/config.bak` と完全一致（`judge_timeout: 60s` / `notify_cmd` 定義あり）。
- T39（judge-hang）は running のまま残っており、以降の tick でも毎回5秒…ではなく **60秒**（復元後の judge_timeout）ブロックする点に注意。フィクスチャB でさらに tick を打つ場合は、先に T39 を止めるか judge_timeout を下げること。
- 残留プロセスなし（`ps -ef | grep 'sleep 180'` が該当なし）。実運用ホーム `$HOME/.pulsen` は未作成のまま（非汚染）。

## 実行しなかった項目（他フィクスチャ担当）

- 確認項目1（task-execution TC-03）— フィクスチャA
- 確認項目3 手順1〜6・10〜12（task-execution TC-05 / TC-19）— フィクスチャA
- 確認項目4 手順1〜11（task-execution TC-06 / TC-07）— フィクスチャA
- 確認項目5（task-execution TC-23 / TC-17）— フィクスチャA
- 確認項目7（task-execution TC-13 / TC-22、intervention TC-01）— フィクスチャA / C
- 確認項目8 手順1〜7（task-execution TC-15）— フィクスチャA
- 確認項目9 手順1〜10（intervention TC-24 / TC-15、setup TC-35 の読み替え）— フィクスチャC
- 確認項目10（task-execution TC-14）— フィクスチャA
- 確認項目11（task-execution TC-21）— フィクスチャA
- エッジケース1（intervention 由来の直編集）— フィクスチャC
- エッジケース4・5・6 — フィクスチャA

## 検証中に気づいた testing.md の記述の問題

1. **確認項目3 の確認ポイントの `last_failure` の記述が実装と食い違う。** 「`.last_failure` が failed の間だけ残り」とあるが、判定 failed（`fail_run`）の経路では `last_failure` は書かれない（`crates/pulsen-domain/src/task/task.rs` の `fail_run` は `last_failure` を触らない）。実際、setup TC-10 の failed 状態で `.last_failure` は `null` だった。`last_failure` が入るのは `record_spawn_failure` / `record_tool_failure` / `record_judge_failure` の3経路。期待結果の本文には影響しないため PASS としたが、確認ポイントの文言は修正が必要。

2. **setup TC-35 の割り当てが二重になっている。** testing.md の「フィクスチャB」の見出しは対象 TC に TC-35 を含める（`spec/manual-tests/setup.md`（TC-09 / 10 / 11 / 35 / 37 / 38 / 39 / 47 用））が、TC-35 を消化する確認項目9 の前提は「フィクスチャC」で、手順はすべて `$PMT/home` に対するもの。フィクスチャB 側では TC-35 に対応する実行手順が存在しない。フィクスチャB の見出しから TC-35 を外すか、確認項目9 が TC-35 をフィクスチャC で消化する旨を見出し側にも書くのが正しい。

3. **エッジケース3 の復元後に T39 が残す副作用が書かれていない。** `judge_timeout` を 60s に戻すと、running のまま残る judge-hang タスク（`judge: ["sleep", "180"]`）が以降の tick を毎回**60秒**ブロックする。TC-39 の後にフィクスチャB で tick を打つ手順があるなら、復元手順に「T39 の始末（本スライスでは `abort` が無いため tick を打たない／judge-hang の定義を消す等）」への言及が要る。今回はエッジケース3 をフィクスチャB の最後に実行したため実害はなかった。
