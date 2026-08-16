# 実行ログ — Issue #4 動作確認

ブランチ `issue/4/ls-show-status-inspection` / HEAD `ebc4a76` / 2026-08-16 / macOS Darwin 25.4.0 / uid 501（非 root）。

長い出力は要点に絞る。`<SCRATCH>` = `/private/tmp/claude-501/-Users-hikaru-github-com-tuanemuy-pulsen/99215b9c-.../scratchpad`、`$PULSEN_HOME` = `<SCRATCH>/mt/pulsen-manual-test`、`$TESTREPO` = `<SCRATCH>/mt/pulsen-test-repo`。

## 0. 環境

```sh
git rev-parse --abbrev-ref HEAD   # issue/4/ls-show-status-inspection
git rev-parse --short HEAD        # ebc4a76
cargo --version                   # cargo 1.97.0 (c980f4866 2026-06-30)  ← nix devShell
git --version                     # git version 2.55.0
/usr/bin/jq --version             # jq-1.7.1-apple
ls -a "$HOME/.pulsen"             # No such file or directory (実行前スナップショット)
```

当シェルの `ls` はディレクトリ不在時に `os error 2` を返す Rust 実装だったため、`ls(1)` の exit code を見る手順は `/bin/ls` を使用。

## 1. 品質ゲート

```sh
cargo fmt --all --check                                          # exit 0 (出力なし)
cargo build --workspace --locked                                 # exit 0  Finished `dev` profile
cargo test --workspace --locked --no-fail-fast -- --nocapture    # exit 0
cargo clippy --workspace --all-targets --locked -- -D warnings   # exit 0  Finished `dev` profile
```

テストは30ターゲットすべて `test result: ok`（`0 failed`）。適合スイートの SKIP は4行:

```
SKIP tc_port_clock_004_時刻の前進: ハーネスが advance を提供しないため、この環境では前提条件を用意できない
SKIP tc_port_clock_005_時刻の巻き戻し: ハーネスが rewind を提供しないため、この環境では前提条件を用意できない
SKIP tc_port_clock_005_巻き戻した時刻はそのまま返る: (同上)
SKIP tc_port_clock_0051_別のケース: (同上)
```

## 2. AC-8 の隔離 grep

```sh
grep -rlE 'cfg\([^)]*\b(unix|windows|target_os|target_family)\b' crates/*/src/
# crates/pulsen-conformance/src/lib.rs
# crates/pulsen/src/util/atomic.rs
# crates/pulsen/src/adapter/task_repository.rs
# crates/pulsen/src/adapter/process.rs

grep -rn '#\[allow(unsafe_code)\]' crates/
# crates/pulsen/src/adapter/process.rs:451:#[allow(unsafe_code)]

grep -n -A 10 '^\[dependencies\]' crates/pulsen-domain/Cargo.toml crates/pulsen/Cargo.toml
# pulsen-domain: [dependencies] の直後が空行 → [lints.rust]
# pulsen: pulsen-domain / clap 4.6.6 / getrandom 0.4.3 / serde 1.0.229 / serde_json 1.0.151
#         / serde_yaml_ng 0.10.0 / tempfile 3.27.0 の7行
```

## 3. `--help`

```
$ pulsen --help   # exit 0
Commands:
  add   タスクを登録する(実行はしない)
  tick  1回のtickパスを実行する
  ls    タスクの一覧を表示する
  show  タスクの詳細を表示する
  help  Print this message or the help of the given subcommand(s)
Options:
      --home <DIR>  グローバルホームのディレクトリ(既定: 環境変数 PULSEN_HOME、なければ ~/.pulsen)

$ pulsen ls --help   # exit 0
      --home <DIR>            …
      --status <TASK-STATUS>  タスクステータス(ワークフローごとのユーザー定義)で絞り込む
      --state <EXEC-STATE>    実行状態(pending / launching / running / completed / failed / stopped)で絞り込む
      --all                   アーカイブ済み(state/archive/)も表示する

$ pulsen show --help   # exit 0
Usage: pulsen show [OPTIONS] <TASK-ID>
Arguments:
  <TASK-ID>  表示するタスクのID
Options:
      --home <DIR>  …

$ pulsen wrapper --help   # exit 0 (一覧には出ないが到達できる = 隠しサブコマンド)
```

## 4. フィクスチャ準備

```sh
export PULSEN_HOME=<SCRATCH>/mt/pulsen-manual-test
mkdir -p $PULSEN_HOME/workflows
# config.yaml (agents.sh + notify_cmd) を作成
# workflows/{wf-wait,wf-sleep,wf-fail,wf-echo}.yaml を作成
git init $TESTREPO && git -C $TESTREPO config user.name/email && git commit --allow-empty -m init
pulsen add --workflow wf-wait --repo $TESTREPO   # ×4 → TASK_A/E/F/G   すべて exit 0
pulsen add --workflow wf-sleep/wf-fail/wf-echo   # → TASK_B/C/D        すべて exit 0
```

ID:

```
TASK_A=20260816t075649-4k7s4dyh   TASK_B=20260816t075654-443d8uut
TASK_E=20260816t075649-8vzq8e1n   TASK_C=20260816t075654-dgzbigrz
TASK_F=20260816t075649-l92nhqbi   TASK_D=20260816t075654-nrawuesl
TASK_G=20260816t075649-vpyjp0qn
```

tick 15回（間2秒）— 各回 exit 0:

```
[t1] 起動: TASK_B, TASK_C, TASK_D
[t2] 起動確認: TASK_B, TASK_C, TASK_D
[t3] 判定確定: TASK_D / 失敗を記録(1件): TASK_C: 実行の失敗を記録しました(実行が終了コード 1 で終了しました)
[t4] 起動: TASK_C / 遷移: TASK_D
[t5] 起動確認: TASK_C
[t6] 失敗を記録(1件): TASK_C
[t7] 起動: TASK_C
[t8] 起動確認: TASK_C
[t9] 凍結: TASK_C / 通知: TASK_C / 失敗を記録(1件): TASK_C
[t10-15] 処理対象のタスクはありませんでした。
```

TASK_G を直編集で stopped に:

```sh
jq --arg at "$(date -u '+%Y-%m-%dT%H:%M:%SZ')" \
  '.execution = {"state":"stopped","reason":"retry_limit_exceeded","notified_at":$at}' ... 
# → "execution": { "state": "stopped", "reason": "retry_limit_exceeded", "notified_at": "2026-08-16T07:57:37Z" }
```

TASK_D を手動アーカイブ:

```sh
git -C $TESTREPO worktree remove $PULSEN_HOME/worktrees/$TASK_D   # exit 0
mkdir -p $PULSEN_HOME/state/archive
mv $PULSEN_HOME/state/tasks/$TASK_D.json $PULSEN_HOME/state/archive/$TASK_D.json
git -C $TESTREPO branch --list "pulsen/*"
# + pulsen/20260816t075654-443d8uut
# + pulsen/20260816t075654-dgzbigrz
#   pulsen/20260816t075654-nrawuesl   ← worktree を外してもブランチは残る
```

## 5. 確認項目1（TC-01）

```
$ pulsen ls   # exit 0
タスクID                  ワークフロー  リポジトリ  ブランチ                         タスクステータス  実行状態  attempt_count  更新日時              備考
20260816t075649-4k7s4dyh  wf-wait       …          (未作成)                         hold              pending   0              2026-08-16T07:56:49Z
20260816t075649-8vzq8e1n  wf-wait       …          (未作成)                         hold              pending   0              2026-08-16T07:56:49Z
20260816t075649-l92nhqbi  wf-wait       …          (未作成)                         hold              pending   0              2026-08-16T07:56:49Z
20260816t075649-vpyjp0qn  wf-wait       …          (未作成)                         hold              stopped   0              2026-08-16T07:56:49Z
20260816t075654-443d8uut  wf-sleep      …          pulsen/20260816t075654-443d8uut  work              running   0              2026-08-16T07:57:00Z
20260816t075654-dgzbigrz  wf-fail       …          pulsen/20260816t075654-dgzbigrz  work              stopped   3              2026-08-16T07:57:14Z

$ pulsen ls --all   # exit 0 — 上記 + 次の行
20260816t075654-nrawuesl  wf-echo       …          pulsen/20260816t075654-nrawuesl  done              pending   0              2026-08-16T07:57:04Z  アーカイブ済み
```

## 6. 確認項目2（絞り込み）

```
$ pulsen ls --status work                    # exit 0 → 443d8uut(work/running), dgzbigrz(work/stopped)
$ pulsen ls --state stopped                  # exit 0 → vpyjp0qn(hold/stopped), dgzbigrz(work/stopped)
$ pulsen ls --state pending                  # exit 0 → 4k7s4dyh, 8vzq8e1n, l92nhqbi
$ pulsen ls --state running                  # exit 0 → 443d8uut
$ pulsen ls --status work --state running    # exit 0 → 443d8uut のみ
$ pulsen ls --status work --state stopped    # exit 0 → dgzbigrz のみ (vpyjp0qn は hold なので落ちる)
$ pulsen ls --status done                    # exit 0 → 該当するタスクはありません。
$ pulsen ls --all --status done              # exit 0 → nrawuesl … done pending 0 … アーカイブ済み
$ pulsen ls --status no-such-status          # exit 0 → 該当するタスクはありません。
$ pulsen ls --state launching|completed|failed  # 3つとも exit 0 → 該当するタスクはありません。
```

## 7. 確認項目3（show: running / 未実行 / launching）

```
$ pulsen show $TASK_B   # exit 0
タスク 20260816t075654-443d8uut の詳細
  ワークフロー: wf-sleep
  リポジトリ: <SCRATCH>/mt/pulsen-test-repo
  ベースブランチ: main
  タスクステータス: work
  実行状態: running
  在籍: 現役
  workspace_path: $PULSEN_HOME/worktrees/20260816t075654-443d8uut
  branch: pulsen/20260816t075654-443d8uut
  attempt_count: 0(上限 2)
  judge_attempt_count: 0(上限 3)
  spawn_fail_count: 0(上限 3)
  現在attempt: 1
    runディレクトリ: $PULSEN_HOME/state/runs/20260816t075654-443d8uut/attempt-1
    PID: 40523
    kill同定子: -40523
    starttime: Sun Aug 16 07:56:57 2026(記録時刻 2026-08-16T07:56:57Z)
    stdout.log: …/attempt-1/stdout.log
    stderr.log: …/attempt-1/stderr.log
    exit: …/attempt-1/exit(記録なし)
  直近の失敗要因: なし
  更新日時: 2026-08-16T07:57:00Z
  定義済みステータス: done, work
  スナップショット保存先: $PULSEN_HOME/state/tasks/20260816t075654-443d8uut.json

$ /bin/ls "$PULSEN_HOME/worktrees/$TASK_B"   # exit 0 (中身は .git のみ)

$ pulsen show $TASK_A   # exit 0 — 未実行タスクの差分のみ抜粋
  workspace_path: 未作成
  branch: 未作成
  attempt_count: 0                ← 上限の併記なし (NotApplicable)
  judge_attempt_count: 0(上限 3)
  spawn_fail_count: 0(上限 3)
  現在attempt: なし               ← run dir・PID・exit の行が出ない
  定義済みステータス: hold
$ /bin/ls "$PULSEN_HOME/state/runs/$TASK_A"   # exit 1 (No such file or directory)

$ pulsen add --workflow wf-sleep --repo $TESTREPO   # TASK_H=20260816t075836-o1kyuhjt
$ pulsen tick     # 起動: 20260816t075836-o1kyuhjt   exit 0
$ pulsen show $TASK_H   # exit 0 — 差分のみ
  実行状態: launching
  現在attempt: 1
    runディレクトリ: …/attempt-1
    同定情報: 未取得(PID・kill同定子・starttime)     ← 3値まとめて1行
    exit: …/attempt-1/exit(記録なし)
```

## 8. 確認項目4（アーカイブ済みの詳細 / 手動アーカイブ移動）

```
$ pulsen show $TASK_D   # exit 0 — 差分のみ
  タスクステータス: done
  実行状態: pending
  在籍: アーカイブ済み(worktree は削除済み)
  workspace_path: $PULSEN_HOME/worktrees/20260816t075654-nrawuesl(削除済み)
  branch: pulsen/20260816t075654-nrawuesl
    exit: …/attempt-1/exit(値 0)
  スナップショット保存先: $PULSEN_HOME/state/archive/20260816t075654-nrawuesl.json

$ pulsen show $TASK_G | grep -E '在籍|workspace'   # 移動前
  在籍: 現役
  workspace_path: 未作成
  branch: 未作成

$ mkdir -p $PULSEN_HOME/state/archive && mv $PULSEN_HOME/state/tasks/$TASK_G.json $PULSEN_HOME/state/archive/
$ pulsen ls    # exit 0 — vpyjp0qn は消え、4k7s4dyh/8vzq8e1n/l92nhqbi/443d8uut/dgzbigrz/o1kyuhjt の6行
$ pulsen tick  # exit 0 — 「起動確認: 20260816t075836-o1kyuhjt」のみ。vpyjp0qn は現れない
$ pulsen ls --all  # exit 0 — 末尾2行が
#   20260816t075649-vpyjp0qn hold stopped … アーカイブ済み
#   20260816t075654-nrawuesl done pending … アーカイブ済み
$ pulsen show $TASK_G   # exit 0
  在籍: アーカイブ済み(worktree は削除済み)
  workspace_path: 未作成
  スナップショット保存先: $PULSEN_HOME/state/archive/20260816t075649-vpyjp0qn.json

# tick が state/archive/ を書き換えないことの確認（tick 2回の前後で md5 が不変）
G before=dc3f6621948392ef83c2015f0dc0190c after=dc3f6621948392ef83c2015f0dc0190c
D before=4677c95169193443b9803e0691a07704 after=4677c95169193443b9803e0691a07704
```

## 9. 確認項目5（ログの起点）

```
$ cat $PULSEN_HOME/state/runs/$TASK_D/attempt-1/stdout.log
hello from pulsen
$ wc -c < …/attempt-1/stderr.log          # 0
$ cat …/attempt-1/exit
{
  "code": 0
}

$ pulsen show $TASK_C | grep -E '現在attempt|runディレクトリ|exit:'
  現在attempt: 3
    runディレクトリ: …/20260816t075654-dgzbigrz/attempt-3
    exit: …/attempt-3/exit(値 1)
$ /bin/ls $PULSEN_HOME/state/runs/$TASK_C/     # exit 0 → attempt-1 attempt-2 attempt-3
$ for n in 1 2 3; do cat …/attempt-$n/exit; done    # 3つとも {"code": 1}、stderr は3つとも 0 バイト

$ /bin/ls $PULSEN_HOME/state/runs/$TASK_B/attempt-1/   # exit 0 → pid starttime stderr.log stdout.log (exit なし)
$ pulsen show $TASK_B | grep 'exit:'                   # exit: …/attempt-1/exit(記録なし)
$ tail -f …/$TASK_B/attempt-1/stdout.log               # 3秒追跡、出力増加なし、エラーなし

$ rm -rf $PULSEN_HOME/state/runs/$TASK_D
$ pulsen show $TASK_D   # exit 0
    runディレクトリ: …/20260816t075654-nrawuesl/attempt-1(存在しません)
    PID: 40659 / kill同定子: -40659 / starttime: …   ← 他の項目は通常表示
    exit: …/attempt-1/exit(記録なし)
```

## 10. 確認項目6（stopped の3経路）

```
$ pulsen show $TASK_C   # exit 0 — リトライ上限超過
  実行状態: stopped
  凍結要因: リトライ上限の超過
  notified_at: 2026-08-16T07:57:14Z
  attempt_count: 3(上限 2)
  judge_attempt_count: 0(上限 3)
  spawn_fail_count: 0(上限 3)
  現在attempt: 3 / exit: …/attempt-3/exit(値 1)
  直近の失敗要因: なし
$ cat $NOTIFYLOG
stopped: 20260816t075654-dgzbigrz (wf-fail/work)
$ /bin/ls -a $PULSEN_HOME/worktrees/$TASK_C   # exit 0 → . .. .git

# TC-16: wf-judge.yaml を作成 → TASK_J=20260816t075943-ojo5ys1y → tick ×10
[t1] 起動 / [t2] 起動確認
[t3-t6] 失敗を記録(1件): 判定できず判定失敗として記録しました(判定コマンドがプロトコル外の終了コード 5 を返しました(有効な値は 0 / 10 / 20))
[t6] 凍結: TASK_J / 通知: TASK_J
$ pulsen show $TASK_J   # exit 0
  凍結要因: 判定失敗の上限の超過
  notified_at: 2026-08-16T07:59:54Z
  attempt_count: 0(上限 2)
  judge_attempt_count: 4(上限 3)
  spawn_fail_count: 0(上限 3)
  現在attempt: 1 / exit: …/attempt-1/exit(値 0)
  直近の失敗要因: 判定の実行(2026-08-16T07:59:54Z): 判定コマンドがプロトコル外の終了コード 5 を返しました(有効な値は 0 / 10 / 20)
$ jq '.snapshot.statuses.work' $PULSEN_HOME/state/tasks/$TASK_J.json
  "judge": [ "sh", "-c", "exit 5" ]      ← スナップショットに固定されている

# TC-17: TASK_I=20260816t080021-wa6ffhhw を登録 → config.yaml の cmd を ["sh","-c","{input}","{bogus}"] に
[t1-t4] 失敗を記録(1件): 起動コマンドを組み立てられません(エージェント `sh` の定義が不正です: cmd の`{bogus}` は使えないプレースホルダ `bogus` を参照しています)
[t4] 凍結: TASK_I / 通知: TASK_I
$ pulsen show $TASK_I   # exit 0
  凍結要因: spawn 失敗の上限の超過
  attempt_count: 0(上限 2) / judge_attempt_count: 0(上限 3) / spawn_fail_count: 4(上限 3)
  現在attempt: なし                      ← 採番されない
  直近の失敗要因: エージェントの起動(2026-08-16T08:00:33Z): エージェント `sh` の定義が不正です: cmd の`{bogus}` は…
$ /bin/ls $PULSEN_HOME/state/runs/$TASK_I   # exit 1 (No such file or directory)
$ grep "$TASK_I" $NOTIFYLOG
stopped: 20260816t080021-wa6ffhhw (wf-echo/work)
$ cp $PULSEN_HOME/config.yaml.bak $PULSEN_HOME/config.yaml   # 復元済み (cmd: ["sh", "-c", "{input}"])
```

## 11. 確認項目7（タスクファイルの直接閲覧・修復）

```
$ cat $PULSEN_HOME/state/tasks/$TASK_A.json
{ "task_id", "workflow_name", "target"{repo, base_branch}, "task_status":"hold",
  "execution":{"state":"pending"}, "workspace":null, "current_attempt":null,
  "counters":{attempt_count:0, judge_attempt_count:0, spawn_fail_count:0},
  "last_failure":null, "updated_at":"2026-08-16T07:56:49Z",
  "snapshot":{... "initial":"hold", "statuses":{"hold":{"action":"wait"}}} }
$ pulsen show $TASK_A   # exit 0、ファイルの md5 不変

# Corrupt
$ cp …/$TASK_E.json $PULSEN_HOME/backup-taskE.json
$ printf '{ "broken":' > $PULSEN_HOME/state/tasks/$TASK_E.json
$ pulsen ls   # exit 0
… 7行の通常表示 (4k7s4dyh / l92nhqbi / 443d8uut / dgzbigrz / o1kyuhjt / ojo5ys1y / wa6ffhhw) …

読み取れなかったタスクファイル(1件):
  - $PULSEN_HOME/state/tasks/20260816t075649-8vzq8e1n.json: unknown field `broken`, expected one of `task_id`, … at line 1 column 10
  内容を直接確認して修復してください。

$ pulsen show $TASK_E   # exit 1
エラー: タスクファイルを読めません。
  ファイル: $PULSEN_HOME/state/tasks/20260816t075649-8vzq8e1n.json
  原因: unknown field `broken`, expected one of `task_id`, … at line 1 column 10
  内容は変更していません。ファイルを直接確認して修復してください。
# 破損ファイルの md5 は show の前後で不変

$ cp $PULSEN_HOME/backup-taskE.json …/$TASK_E.json
$ pulsen show $TASK_E   # exit 0 (通常表示)
$ pulsen ls             # exit 0、「読み取れなかった」の報告は0件、8行すべて通常表示

# SnapshotUnreadable
$ jq '.snapshot.initial = ""' …/$TASK_F.json > /tmp/x && mv /tmp/x …/$TASK_F.json
$ jq '.task_status, .execution, .snapshot.initial' …/$TASK_F.json
"hold"
{ "state": "pending" }
""
$ pulsen ls   # exit 0
20260816t075649-l92nhqbi  wf-wait  …  (未作成)  hold  pending  0  2026-08-16T07:56:49Z  スナップショット読み取り不能
$ pulsen ls --state pending   # exit 0 — 3行目に同じ行 (絞り込みの対象になる)
$ pulsen show $TASK_F   # exit 0
  attempt_count: 0(上限 導出不能: スナップショットを読めません)
  judge_attempt_count: 0(上限 3)
  spawn_fail_count: 0(上限 3)
  定義済みステータス: 読み取れません(initial: 空文字列は指定できません)   ← 一覧は出ない
# ファイルの md5 は show の前後で不変
$ cp $PULSEN_HOME/backup-taskF.json …/$TASK_F.json
$ pulsen show $TASK_F   # exit 0 → attempt_count: 0 (併記なしに戻る) / 定義済みステータス: hold
$ pulsen ls | grep l92nhqbi   # 備考列が空に戻る
```

## 12. 確認項目8（読み取り専用）

```
$ pulsen show $TASK_C | grep workspace_path
  workspace_path: $PULSEN_HOME/worktrees/20260816t075654-dgzbigrz
$ rm -rf $PULSEN_HOME/worktrees/$TASK_C
$ pulsen show $TASK_C   # exit 0 — workspace_path は同じ文字列のまま、branch も表示（存在検証なし）

# ロック保持中
$ ( sleep 25 | target/debug/examples/lock_holder "$PULSEN_HOME/state/lock" ) &   # → "locked"
$ pulsen ls        # exit 0  9行
$ pulsen ls --all  # exit 0  11行
$ pulsen show $TASK_B   # exit 0  詳細表示
   ↑ 3つとも「スキップしました」等のロック関連メッセージは一切出ない
$ pulsen tick      # exit 0
別の操作が実行中のため、今回の tick はスキップしました。
# 解放後
$ pulsen ls        # exit 0  9行 (保持中と同一)

# tick と同時の読み取り
$ for i in 1 2 3; do pulsen tick & pulsen ls; wait; sleep 2; done
iter1 ls_exit=0 lines=9 corrupt_report=0
iter2 ls_exit=0 lines=9 corrupt_report=0
iter3 ls_exit=0 lines=9 corrupt_report=0

# 型としての担保
$ grep -rn 'lock()' crates/pulsen/src/cli/
crates/pulsen/src/cli/ls.rs:21:/// **`runtime.lock()` を渡さない。** …        ← コメント
crates/pulsen/src/cli/show.rs:21:/// **`runtime.lock()` を渡さない。** …      ← コメント
crates/pulsen/src/cli/add.rs:35,37 / crates/pulsen/src/cli/tick.rs:36,37     ← 実呼び出し
# 手順書の期待 (wire.rs + tick.rs のみ) とは一致しない → result.md M1
$ grep -rn 'exists\|try_exists' application/show_task.rs cli/render/show.rs
crates/pulsen/src/application/show_task.rs:326: let presence = match self.runs.attempt_exists(run_dir) {
# 手順書の期待 (0件) とは一致しない → result.md M2
$ grep -rnE '\.(exists|try_exists)\(\)' application/show_task.rs cli/render/show.rs application/list_tasks.rs cli/render/ls.rs
# 0件 (exit 1) — Path::exists() は使っていない
```

## 13. エッジケース1（タスクIDの境界）

```
$ pulsen show no-such-task-0000   # exit 1
エラー: 指定されたタスクが見つかりません。
  タスクID: no-such-task-0000
  現役にもアーカイブにも存在しません。
$ pulsen show 'TASK_A!'           # exit 1  エラー: タスクIDが不正です。/ 原因: 1文字目に使えない文字('T')があります。使えるのは英小文字・数字・`-` です
$ pulsen show -- -abc             # exit 1  エラー: タスクIDが不正です。/ 原因: 先頭は英小文字か数字である必要があります
$ pulsen show $(printf 'a%.0s' $(seq 1 65))   # exit 1  原因: 64文字を超えられません
$ pulsen show $(printf 'a%.0s' $(seq 1 64))   # exit 1  エラー: 指定されたタスクが見つかりません。(不在エラー)
$ pulsen show ""                  # exit 1  原因: 空文字列は指定できません
```

## 14. エッジケース2（`--state` の値）

```
$ pulsen ls --state stoped    # exit 1
エラー: --state の値が不正です。
  指定: `stoped`
  有効な値: pending / launching / running / completed / failed / stopped
$ pulsen ls --state Pending   # exit 1  同じ形式 (指定: `Pending`)
$ pulsen ls --state ""        # exit 1  同じ形式 (指定: ``)
$ pulsen ls --status ""       # exit 0  該当するタスクはありません。
```

## 15. エッジケース3（config.yaml）

```
$ mv $PULSEN_HOME/config.yaml $PULSEN_HOME/config.yaml.bak2
$ pulsen ls     # exit 1
エラー: グローバルホームが未初期化です。
  グローバルホーム: $PULSEN_HOME
  グローバル設定 $PULSEN_HOME/config.yaml を作成してください。
$ pulsen show $TASK_A   # exit 1  同一の3行
$ printf 'agents: [\n' > $PULSEN_HOME/config.yaml
$ pulsen ls     # exit 1
エラー: グローバル設定を解釈できません。
  ファイル: $PULSEN_HOME/config.yaml
  原因: did not find expected node content at line 2 column 1, while parsing a flow node
  位置: 2行1列
# state/tasks/ 配下の md5 集合は前後で不変
$ mv $PULSEN_HOME/config.yaml.bak2 $PULSEN_HOME/config.yaml
$ pulsen ls     # exit 0  9行
```

## 16. エッジケース4（走査不能）

```
$ id -u   # 501 (非 root) / uname -sr → Darwin 25.4.0  ⇒ chmod 000 が有効
$ chmod 000 $PULSEN_HOME/state/tasks; pulsen ls     # exit 1
エラー: タスクを走査できません。
  原因: $PULSEN_HOME/state/tasks: Permission denied (os error 13)
$ chmod 755 $PULSEN_HOME/state/tasks
$ chmod 000 $PULSEN_HOME/state/archive
$ pulsen ls --all   # exit 1  エラー: タスクを走査できません。/ 原因: …/state/archive: Permission denied (os error 13)
$ pulsen ls         # exit 0  (アーカイブ側に依存しない)
$ chmod 755 $PULSEN_HOME/state/archive; pulsen ls --all   # exit 0  11行
```

## 17. エッジケース5（exit / run ディレクトリが読めない）

```
$ cp …/$TASK_C/attempt-3/exit /tmp/pulsen-exit.bak
$ printf 'abc' > …/attempt-3/exit
$ pulsen show $TASK_C   # exit 0
  凍結要因: リトライ上限の超過 / attempt_count: 3(上限 2) …            ← 通常表示
    runディレクトリ: …/attempt-3                                     ← 通常表示
    exit: …/attempt-3/exit(読み取れません: 内容を解釈できない: expected value at line 1 column 1)
$ cat …/attempt-3/exit   # abc のまま (書き換えていない)
$ cp /tmp/pulsen-exit.bak …/attempt-3/exit

$ chmod 000 …/$TASK_C/attempt-3; pulsen show $TASK_C   # exit 0
    runディレクトリ: …/attempt-3                                     ← 注記なし
    exit: 読み取れません: …/attempt-3/exit: 読み取れない: Permission denied (os error 13)
$ chmod 755 …/attempt-3; pulsen show $TASK_C   # exit 0 → exit: …(値 1)

# 補助: 親を 000 にして「存在確認の失敗」を作る
$ chmod 000 $PULSEN_HOME/state/runs/$TASK_C; pulsen show $TASK_C   # exit 0
    runディレクトリ: 存在を確認できません: …/attempt-3: attempt ディレクトリの有無を確認できない: Permission denied (os error 13)
    exit: …/attempt-3/exit(runディレクトリの有無を確認できないため読んでいません)
$ chmod 755 $PULSEN_HOME/state/runs/$TASK_C
```

## 18. エッジケース6（0件 / `state/` 不在）

```
$ mkdir -p $EMPTYHOME && printf 'agents: {}\n' > $EMPTYHOME/config.yaml
$ PULSEN_HOME=$EMPTYHOME pulsen ls          # exit 0  該当するタスクはありません。
$ PULSEN_HOME=$EMPTYHOME pulsen ls --all    # exit 0  該当するタスクはありません。
$ PULSEN_HOME=$EMPTYHOME pulsen show $TASK_A   # exit 1  エラー: 指定されたタスクが見つかりません。
$ /bin/ls -a $EMPTYHOME     # . .. config.yaml   ← state/ は生えていない
$ pulsen ls --status no-such-status   # exit 0 (元ホーム)  該当するタスクはありません。
```

## 19. 既存機能への影響確認

```
# add / tick スポットチェック (TASK_K=20260816t080337-zbqm8c8i, wf-echo)
$ pulsen add --workflow wf-echo --repo $TESTREPO   # exit 0
タスクを登録しました。
  タスクID: 20260816t080337-zbqm8c8i
  ワークフロー: wf-echo
  解決先: $PULSEN_HOME/workflows/wf-echo.yaml
  次回の tick で実行されます。
[t1] 起動: TASK_K        exit 0
[t2] 起動確認: TASK_K    exit 0
[t3] 判定確定: TASK_K    exit 0
[t4] 遷移: TASK_K        exit 0
[t5] 処理対象のタスクはありませんでした。  exit 0
$ pulsen show $TASK_K | grep -E 'タスクステータス|実行状態|exit:'
  タスクステータス: done / 実行状態: pending / exit: …/attempt-1/exit(値 0)

$ grep -rn 'attempt_exists' crates/pulsen/src/
crates/pulsen/src/adapter/run_store.rs:122:    fn attempt_exists(&self, run_dir: &RunDirPath) -> Result<bool, Io> {
crates/pulsen/src/adapter/run_store.rs:267:        assert_eq!(store.attempt_exists(&run_dir), Ok(false));
crates/pulsen/src/application/show_task.rs:326:        let presence = match self.runs.attempt_exists(run_dir) {
# application/tick* には現れない

# render 分割で既存文言が変わっていないことの確認
$ git show f575b4a:crates/pulsen/src/cli/render.rs > /tmp/render_base.rs   # 1456行
$ 日本語を含む文字列リテラルを抽出して比較
base literals: 135, head literals: 213
comm -23 (base にあって head に無い) → 0行

$ grep -rn 'Branch::Cleanup' crates/pulsen/src/
crates/pulsen/src/application/tick/mod.rs:444:            Branch::Cleanup => {}
crates/pulsen/src/application/tick/mod.rs:588:            StatusDefinition::Cleanup => Branch::Cleanup,
$ grep -rn 'archived\.push\|gc_deleted\.push\|gc_errors\.push' crates/pulsen/src/application/tick/   # 0件

# 実運用ホーム
$ /bin/ls -a "$HOME/.pulsen"   # exit 1  No such file or directory (実行前と同じ)
```

## 20. 後片付け

```
$ ps -ef | grep -E 'pulsen wrapper|sleep 3000' | grep -v grep
501 40523 1 … pulsen wrapper --run-dir …/443d8uut/attempt-1 … -- sh -c sleep 3000   (TASK_B)
501 40588 40523 … sleep 3000
501 41109 1 … pulsen wrapper --run-dir …/o1kyuhjt/attempt-1 … -- sh -c sleep 3000   (TASK_H)
501 41136 41109 … sleep 3000
$ pkill -f 'sleep 3000'
$ git -C $TESTREPO worktree list
$ rm -rf <SCRATCH>/mt /tmp/pulsen-exit.bak
```
