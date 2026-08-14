# 手動確認の結果 — フィクスチャC

**実行日:** 2026-08-14
**環境:** macOS 26.4.1 / 非root（uid=501）/ TMPDIR=/var/folders/l_/pptqtzd976329fq6qp57cgh40000gn/T/
**対象:** `.thread/3/testing.md` の確認項目のうち `spec/manual-tests/intervention.md` 由来のもの（TC-01 / TC-15 / TC-24）と、フィクスチャCを前提とするエッジケース1
**バイナリ:** `/Users/hikaru/github.com/tuanemuy/pulsen/target/debug/pulsen`（ブランチ `issue/3/tick-observe-judge-transition`）
**PMT:** `$HOME/pulsen-intervention-test`（testing.md の読み替え規則どおり。手順書の `$HOME/pulsen-manual-test` は使っていない）

## サマリー

- 実行: 3件（PASS: 3 / FAIL: 0 / 実行不可: 0）
  - 確認項目7（フィクスチャC 部分・手順9〜11 のみ）: PASS
  - 確認項目9（全手順1〜10）: PASS
  - エッジケース1（全手順1〜7）: PASS
- 実行しなかった項目（フィクスチャA / B 担当のため）: 確認項目1・2・3・4・5・6・8・10・11、確認項目7 の手順1〜8、エッジケース2・3・4・5・6
- 観測された既定のリトライ上限は **2**（`attempt_count` 3 > 2 で凍結）。凍結までに要した tick は **9回**（手順書の「目安10回」と整合）。

## 確認項目ごとの結果

### 0. フィクスチャC のセットアップ

- **判定:** PASS
- **実行した手順:** testing.md 「フィクスチャC」のブロックをそのまま実行（`PMT="$HOME/pulsen-intervention-test"`）。
- **観測した結果:** `$PMT/{repo,notify.sh,notify.log,config.bak,home/{config.yaml,workflows/wf-fail.yaml}}` が作成された。`config.yaml` の `notify_cmd` は `["sh", "/Users/hikaru/pulsen-intervention-test/notify.sh"]` に絶対パス展開されており、YAML 内の環境変数非展開の注意どおり。
- **期待との一致:** 一致。フィクスチャA（`/tmp/pulsen-test`）およびB（`$HOME/pulsen-manual-test` ほか）には一切触れていない。

### 7. リトライ上限の等号と超過 — 凍結と at-least-once 通知（対応する手順書: `intervention.md` TC-01 手順1〜3・5・7・8）

**注:** 本項目の手順1〜8 は `task-execution.md` TC-13 / TC-22（フィクスチャA）由来のため別エージェント担当。ここではフィクスチャC 部分の**手順9〜11 のみ**を実行した。

- **判定:** PASS
- **実行した手順:**

    ```sh
    export PMT="$HOME/pulsen-intervention-test"
    pulsen --home "$PMT/home" add --workflow wf-fail --repo "$PMT/repo"   # TI1=20260814t095525-1x5enfdn
    : > "$PMT/notify.log"
    # 3秒間隔で tick を繰り返し、各回で .execution / .counters / notify.log の行数を記録
    pulsen --home "$PMT/home" tick    # ×9
    cat "$PMT/notify.log"
    cat "$PMT/home/state/tasks/$TI1.json"
    cat "$PMT/home/state/runs/$TI1/attempt-3/stderr.log"
    ls "$PMT/home/worktrees/$TI1"
    md5 -q "$PMT/home/state/tasks/$TI1.json"   # tick 前後で比較
    pulsen --home "$PMT/home" tick; wc -l < "$PMT/notify.log"
    ```

- **観測した結果:**

  | tick | サマリー | `.execution` | `attempt_count` | attempt | notify.log 行数 |
  |---|---|---|---|---|---|
  | 1 | 起動 | launching | 0 | 1 | 0 |
  | 2 | 起動確認 | running | 0 | 1 | 0 |
  | 3 | 失敗を記録(1件) | failed | 1 | 1 | 0 |
  | 4 | 起動 | launching | 1 | 2 | 0 |
  | 5 | 起動確認 | running | 1 | 2 | 0 |
  | 6 | 失敗を記録(1件) | failed | 2 | 2 | 0 |
  | 7 | 起動 | launching | 2 | 3 | 0 |
  | 8 | 起動確認 | running | 2 | 3 | 0 |
  | 9 | **凍結 + 通知** + 失敗を記録(1件) | stopped / retry_limit_exceeded / notified_at=2026-08-14T09:56:15Z | 3 | 3 | **1** |

  tick #9 のサマリー全文:

    ```text
    tick を実行しました。
      凍結: 20260814t095525-1x5enfdn
      通知: 20260814t095525-1x5enfdn
      失敗を記録(1件):
        - 20260814t095525-1x5enfdn: 実行の失敗を記録しました(実行が終了コード 1 で終了しました)
    ```

  手順10:

    ```text
    notify.log: 18:56:15 TASK_ID=20260814t095525-1x5enfdn WORKFLOW=wf-fail TASK_STATUS=work   （ちょうど1行）
    .task_status = "work"
    .execution   = {"state":"stopped","reason":"retry_limit_exceeded","notified_at":"2026-08-14T09:56:15Z"}
    .counters    = {"attempt_count":3,"judge_attempt_count":0,"spawn_fail_count":0}
    runs/        = attempt-1 attempt-2 attempt-3
    attempt-3/exit = {"code": 1}（2スペース字下げの整形 JSON）
    attempt-3/stderr.log = boom
    worktrees/<TI1> は存在（wf-fail の work は成果物を書かないため中身は空）
    git branch --list "pulsen/<TI1>" → + pulsen/20260814t095525-1x5enfdn（保持）
    ```

  手順11: `tick を実行しました。 / 処理対象のタスクはありませんでした。`、exit code 0。タスクファイルの md5 は tick 前後とも `4b1d78a5b727053af7b097691cb85d74` で不変、`.updated_at` も `2026-08-14T09:56:15Z` のまま。`notify.log` は 1 行のまま。

- **期待との一致:** 一致。
  - failed でリトライしている間は `notify.log` に行が増えない（tick 1〜8 で 0 行）。
  - stopped 到達の tick で**ちょうど1行**、`TASK_ID` / `WORKFLOW` / `TASK_STATUS` の3値すべて非空、`TASK_STATUS` は凍結時点の `work`。
  - 通知は凍結を保存した**同じ tick 内**で起きており、`notified_at` が `null` のまま残る tick が1つも無い。
  - `stderr.log` に `boom` があり凍結原因を特定できる。worktree・ブランチとも保持。
  - 凍結後の tick は `.updated_at` を更新せず、通知も増えない（ADR-097 の破れなし）。

### 9. 通知の失敗と再通知 / notify_cmd 未定義と catch-up（対応する手順書: `intervention.md` TC-24 / TC-15、`setup.md` TC-35。いずれも `abort` を上限超過での凍結に読み替え）

- **判定:** PASS
- **実行した手順:**

    ```sh
    # 手順1-2
    sed -i.bak 's|^notify_cmd:.*|notify_cmd: ["sh", "-c", "exit 1"]|' "$PMT/home/config.yaml"
    : > "$PMT/notify.log"
    pulsen --home "$PMT/home" add --workflow wf-fail --repo "$PMT/repo"   # TI24=20260814t095640-ftyoot6r
    # 手順3-4: 3秒間隔で tick ×9（stopped まで）
    # 手順5
    cp "$PMT/config.bak" "$PMT/home/config.yaml"; pulsen --home "$PMT/home" tick
    # 手順6
    pulsen --home "$PMT/home" tick; wc -l < "$PMT/notify.log"
    # 手順7-8
    grep -v '^notify_cmd:' "$PMT/config.bak" > "$PMT/home/config.yaml"   # → grep -c notify_cmd = 0
    pulsen --home "$PMT/home" add --workflow wf-fail --repo "$PMT/repo"   # TI35=20260814t095731-5gdo4gvs
    # 3秒間隔で tick ×9（stopped まで）
    # 手順9-10
    cp "$PMT/config.bak" "$PMT/home/config.yaml"; pulsen --home "$PMT/home" tick
    pulsen --home "$PMT/home" tick; wc -l < "$PMT/notify.log"
    ```

- **観測した結果:**

  手順3（TI24 の凍結 tick = 9回目）は **exit code 0**。サマリー:

    ```text
    tick を実行しました。
      凍結: 20260814t095640-ftyoot6r
      失敗を記録(1件):
        - 20260814t095640-ftyoot6r: 実行の失敗を記録しました(実行が終了コード 1 で終了しました)
      スキップ(1件):
        - 20260814t095640-ftyoot6r: 凍結を通知できません(通知コマンドが終了コード 1 で終了しました)。次の tick が再通知します
    ```

  「凍結」に TI24 が現れ、「通知」の行は出ていない。`.execution` = `{"state":"stopped","reason":"retry_limit_exceeded","notified_at":null}`。

  手順4: `notify.log` は 0 行（空）。

  手順5: config 復元後の tick は `通知: 20260814t095640-ftyoot6r` の1行のみ（「凍結」には現れない）、exit code 0。`notify.log` に `18:57:19 TASK_ID=20260814t095640-ftyoot6r WORKFLOW=wf-fail TASK_STATUS=work` が1行追加され計 **1行**。`.execution.notified_at` = `2026-08-14T09:57:19Z`、`.updated_at` も同時刻。

  手順6: `処理対象のタスクはありませんでした。` exit code 0、`notify.log` は **1行**のまま。

  手順7: `grep -c notify_cmd` = **0**（未定義）。TI35 は 9 tick で stopped に到達。凍結 tick のサマリーは

    ```text
    tick を実行しました。
      凍結: 20260814t095731-5gdo4gvs
      失敗を記録(1件):
        - 20260814t095731-5gdo4gvs: 実行の失敗を記録しました(実行が終了コード 1 で終了しました)
    ```

  「通知」も通知失敗の報告も出ない（未定義なので通知経路自体が走らない）。

  手順8: `.execution` = `{"state":"stopped","reason":"retry_limit_exceeded","notified_at":null}`、`notify.log` は **1行**のまま（TI24 の行のみ）。

  手順9: config 復元後の tick は `通知: 20260814t095731-5gdo4gvs` の1行のみ（「凍結」には現れない）、exit code 0。`notify.log` は **2行**になり、追加行は `18:58:09 TASK_ID=20260814t095731-5gdo4gvs WORKFLOW=wf-fail TASK_STATUS=work`。`.execution.notified_at` = `2026-08-14T09:58:09Z`。

  手順10: `処理対象のタスクはありませんでした。` exit code 0、`notify.log` は **2行**のまま。

- **期待との一致:** 一致。「stopped を書く → notify_cmd → 成功時のみ `mark_notified`」の順序が保たれており、通知失敗時に `notified_at` が入ってしまう欠落パターンは起きていない。catch-up の tick が「凍結」として再計上されることもない。フィクスチャA / B の config には一切触れていない（本項目は `$PMT/home` に閉じている）。

### エッジケース1. スナップショットのみ破損した未通知 stopped への再通知（前提: フィクスチャC）

- **判定:** PASS
- **実行した手順:**

    ```sh
    pulsen --home "$PMT/home" add --workflow wf-fail --repo "$PMT/repo"   # TD=20260814t095822-ayddohad
    cp "$PMT/home/state/tasks/$TD.json" "$PMT/td.bak"
    jq '.execution = {"state":"stopped","reason":"retry_limit_exceeded","notified_at":null}
        | .snapshot.statuses = "broken"' "$PMT/td.bak" > "$PMT/td.new" && mv "$PMT/td.new" "$PMT/home/state/tasks/$TD.json"
    : > "$PMT/notify.log"
    pulsen --home "$PMT/home" tick; echo $?
    cat "$PMT/notify.log"; cat "$PMT/home/state/tasks/$TD.json"
    pulsen --home "$PMT/home" tick; wc -l < "$PMT/notify.log"
    jq '.execution = {"state":"pending"}' "$PMT/home/state/tasks/$TD.json" > "$PMT/td.pending" && mv "$PMT/td.pending" "$PMT/home/state/tasks/$TD.json"
    md5 -q ...; pulsen --home "$PMT/home" tick; md5 -q ...; wc -l < "$PMT/notify.log"
    ```

- **観測した結果:**

  手順4（exit code 0）:

    ```text
    tick を実行しました。
      通知: 20260814t095822-ayddohad
      スキップ(1件):
        - 20260814t095822-ayddohad: 埋め込まれたワークフロー定義を読めません(invalid type: string "broken", expected a map at line 5 column 24)
    ```

  手順5: `notify.log` に `18:58:29 TASK_ID=20260814t095822-ayddohad WORKFLOW=wf-fail TASK_STATUS=work` が1行。タスクファイルは `.execution.notified_at` = `2026-08-14T09:58:29Z` が入り、`.snapshot.statuses` は **`"broken"` のまま温存**。`.execution.state` は `stopped` / `retry_limit_exceeded` のまま、`.counters` も全0で不変。

  手順6: 同じ「スキップ」報告のみが出て「通知」は出ず、`notify.log` は1行のまま。exit code 0。

  手順7（`pending` に戻した対照）: `grep -c '"broken"'` = 1、tick は exit code 0 で「スキップ(1件): … 埋め込まれたワークフロー定義を読めません」のみ。md5 は tick 前後とも `b86e61440a5eb68ece7f199ead951efc` で不変、`notify.log` も1行のまま。

- **期待との一致:** 一致。通知と報告が両立しており（報告が通知に置き換わっていない。ADR-012）、`save_degraded` によって破損スナップショットが正規化・上書きされていない。pending の縮退タスクには再通知経路が無く、tick が縮退タスクを `stopped` にし直すこともない。

## 実行しなかった項目

他フィクスチャ担当（`/tmp/pulsen-test` = フィクスチャA、`$HOME/pulsen-manual-test` ほか = フィクスチャB）のため未実行:

- 確認項目1（TC-03）／2（setup TC-09）／3（TC-05・TC-19・setup TC-10）／4（TC-06・TC-07・setup TC-11）／5（TC-23・TC-17）／6（setup TC-47）／8（TC-15・setup TC-37）／10（TC-14）／11（TC-21）
- 確認項目7 の手順1〜8（TC-13 / TC-22。フィクスチャA）
- エッジケース2（setup TC-38）／3（setup TC-39）／4（フィクスチャA）／5（フィクスチャA）／6（TC-20）

## 気づいた testing.md の記述の問題

いずれも判定に影響しない軽微な点。

1. **確認項目9 手順3 の「通知の失敗が報告として読める形で出る」の見出し名が未記載** — 実際には報告4見出しのうち「スキップ」に `凍結を通知できません(通知コマンドが終了コード 1 で終了しました)。次の tick が再通知します` として出る。「失敗を記録」と読み違えやすいので、見出し名を明記すると判定がぶれない。
2. **確認項目7 手順10 の `ls "$PMT/home/worktrees/$TI1"` は出力が空になる** — `wf-fail` の `work` は worktree に成果物を書かないため、worktree は存在するが `ls` は何も表示しない（`.git` は隠しファイル）。存在確認としては `ls -a` か `test -d` が適切。期待結果の文言（「worktree が存在する」）と出力の見た目が食い違うため、誤って FAIL 判定されうる。
3. **フィクスチャC の「目安10回の tick」は実測9回** — 既定のリトライ上限は 2 で、`attempt_count` 3 > 2 の tick（起動 → 起動確認 → 判定 の3刻み × 3 attempt = 9 tick 目）で凍結する。目安として矛盾はないが、実測値を書くなら 9。
4. **確認項目9 手順1 の `sed -i.bak` が `$PMT/home/config.yaml.bak` を残す** — 後続で `cp "$PMT/config.bak"` により復元するので実害は無いが、`home` 直下にゴミが残る。手順の最後で消すか、`sed -i ''` ではなく別ファイル経由にすると綺麗。

## 環境の状態（実行後）

- `$HOME/.pulsen` は存在しない（実運用ホームは非汚染）。
- `ps -ef | grep 'pulsen wrapper'` の残留プロセスは 0 件。
- `$PMT/home/config.yaml` は `$PMT/config.bak` と完全一致（復元済み）。
- フィクスチャC のタスク4件（TI1 / TI24 / TI35 / TD）はすべて `stopped`（TD のみ手順7 で `pending` + スナップショット破損のまま）。後片付け（`rm -rf "$HOME/pulsen-intervention-test"`）は他フィクスチャの検証完了後にメインが実施する想定で、本エージェントでは実行していない。
