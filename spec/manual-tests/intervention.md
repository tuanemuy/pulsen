# 人間の介入と復旧 テスト

## 概要

このドキュメントは、自動進行に任せられなくなったタスクへの介入(stopped 通知の受信・abort・retry・set-status・レビュー待ちステータスからの手動進行)に関するマニュアルテストの手順書です。

対応シナリオ: `spec/scenario/intervention.md`

pulsen は CLI ツールであるため、すべての操作はコマンドラインで行い、期待結果はコマンド出力・exit code・ファイルの内容で検証する。通知の検証には「証跡ファイルへ追記するテスト用 notify_cmd」を使う。

## 前提条件

### 環境

- POSIX 環境(macOS / Linux)。`sh` / `git` が使えること。Windows で実施する場合はシェルスクリプトを同等のコマンドに読み替える
- `pulsen` バイナリがビルド済みで PATH にあること(`cargo build --release` 後にパスを通す。以降のコマンド例は `pulsen` と表記)
- cron 等の外部スケジューラーには登録しない。`tick` はすべて手動で実行し、タイミングを制御する(既存環境で cron に tick を登録済みの場合も、本テストは専用の `PULSEN_HOME` を使うため干渉しない)
- 連続する `tick` の間は 2〜3 秒あける(ラッパーの pid ファイル書き込みを待つため)

### テストデータ

テスト専用ディレクトリ(分離ホーム)を初期化し(既存なら削除して作り直す。過去の実行や他ドキュメントの残留状態を持ち込まないため)、環境変数を設定する。以降のすべてのコマンドはこのシェルで実行する。

```sh
export PMT=$HOME/pulsen-manual-test
export PULSEN_HOME=$PMT/home
rm -rf "$PMT"
mkdir -p "$PMT" "$PULSEN_HOME/workflows"
```

ファイル内の `<PMT>` は `echo $PMT` の実パス(絶対パス)に置き換えて記載する(config.yaml / ワークフローYAML 内では環境変数は展開されない)。

**テスト対象リポジトリ**(worktree 作成元。グローバル git 設定のない環境でも初期コミットが成功するよう、identity をリポジトリローカルに設定する):

```sh
git init -b main "$PMT/repo"
git -C "$PMT/repo" config user.name pulsen-test
git -C "$PMT/repo" config user.email pulsen-test@example.com
git -C "$PMT/repo" commit --allow-empty -m "init"
```

**テスト用 notify_cmd**(`<PMT>/notify.sh`。受け取った環境変数を証跡ファイルへ追記する):

```sh
#!/bin/sh
echo "$(date '+%H:%M:%S') TASK_ID=$TASK_ID WORKFLOW=$WORKFLOW TASK_STATUS=$TASK_STATUS" >> "<PMT>/notify.log"
```

作成後に `chmod +x "$PMT/notify.sh"` を実行する。

**テスト用判定コマンド**(`<PMT>/judge.sh`。初期状態は「判定失敗」を返す):

```sh
#!/bin/sh
exit 5
```

作成後に `chmod +x "$PMT/judge.sh"` を実行する(exit 5 は判定プロトコル上の 0 / 10 / 20 のいずれでもないため「判定失敗」に分類される)。

**グローバル設定**(`$PULSEN_HOME/config.yaml`):

```yaml
agents:
  shell:
    cmd: ["sh", "-c", "{input}"]
  shell2:
    cmd: ["sh", "-c", "{input}"]
notify_cmd: ["sh", "<PMT>/notify.sh"]
```

**ワークフロー定義**(いずれも `$PULSEN_HOME/workflows/` に置く):

`wf-longrun.yaml` — 実行中(running)状態を作る(長時間 sleep するシェルエージェント):

```yaml
workflow: wf-longrun
agent: shell
initial: work
statuses:
  work:
    prompt: "sleep 300"
    next: review_waiting
  fixup:
    prompt: "date >> pulsen-test.txt"
    next: review_waiting
  review_waiting:
    run: wait
  done:
    run: cleanup
```

`wf-fail.yaml` — 意図的に失敗し、リトライ上限超過で stopped に至る:

```yaml
workflow: wf-fail
agent: shell
initial: work
statuses:
  work:
    prompt: "echo boom >&2; exit 1"
    next: review_waiting
  fixup:
    prompt: "date >> pulsen-test.txt"
    next: review_waiting
  review_waiting:
    run: wait
  done:
    run: cleanup
```

`wf-wait.yaml` — 成功してレビュー待ち(「何もしない」ステータス)に滞留する:

```yaml
workflow: wf-wait
agent: shell
initial: work
statuses:
  work:
    prompt: "date >> pulsen-test.txt"
    next: review_waiting
  fixup:
    prompt: "date >> pulsen-test.txt"
    next: review_waiting
  review_waiting:
    run: wait
  done:
    run: cleanup
```

`wf-judge.yaml` — 判定コマンドの不具合で stopped に至る(エージェント自体は成功する):

```yaml
workflow: wf-judge
agent: shell
initial: work
statuses:
  work:
    prompt: "true"
    judge: ["sh", "<PMT>/judge.sh"]
    next: review_waiting
  review_waiting:
    run: wait
```

`wf-spawn.yaml` — 登録後の config 破壊で連続 spawn 失敗を作る(shell2 を参照):

```yaml
workflow: wf-spawn
agent: shell2
initial: work
statuses:
  work:
    prompt: "true"
    next: review_waiting
  review_waiting:
    run: wait
```

`wf-timeout.yaml` — timeout による自動 kill を観測する:

```yaml
workflow: wf-timeout
agent: shell
initial: work
statuses:
  work:
    prompt: "sleep 300"
    timeout: 10s
    next: review_waiting
  review_waiting:
    run: wait
```

### 事前準備: 前提状態の作り方(共通手順)

各テストケースの前提列で参照する。作成したタスクIDは都度控える(以下 `<ID>` と表記)。

- **手順R1(running のタスク)**: `pulsen add --workflow wf-longrun --repo $PMT/repo` → `pulsen tick`(起動)→ 2〜3 秒待って `pulsen tick`(spawn 確認)→ `pulsen ls --state running` で対象が表示されること
- **手順R2(リトライ上限超過で stopped のタスク)**: `pulsen add --workflow wf-fail --repo $PMT/repo` → `pulsen tick` を 2〜3 秒間隔で繰り返す(目安 10 回)→ `pulsen ls --state stopped` で対象が表示されること
- **手順R3(レビュー待ち滞留のタスク)**: `pulsen add --workflow wf-wait --repo $PMT/repo` → `pulsen tick` を 2〜3 秒間隔で繰り返す(目安 4 回)→ `pulsen ls --status review_waiting` で対象が表示されること(実行状態は pending)
- **手順R4(pending のタスク)**: `pulsen add --workflow wf-longrun --repo $PMT/repo` のみ実行し、tick しない
- **手順R5(failed のタスク)**: `pulsen add --workflow wf-fail --repo $PMT/repo` → `pulsen tick` を 3 回(起動 → spawn確認 → 失敗判定)→ `pulsen ls --state failed` で対象が表示されること(stopped まで進めない)
- **手順R6(completed のタスク)**: `pulsen add --workflow wf-wait --repo $PMT/repo` → `pulsen tick` を 3 回 → `pulsen ls --state completed` で対象が表示されること(次の tick で遷移してしまうため、確認後すぐに対象の操作を行う)

### 後片付け

テスト完了後、残存プロセスと一時ディレクトリを掃除する。

```sh
pkill -f "sleep 300"   # 残存していれば
rm -rf "$PMT"
```

## TC-01: リトライ上限超過による stopped 確定と通知の受信

**種別**: 正常系
**目的**: 失敗の継続で凍結したタスクについて、通知(notify_cmd)が発火し、原因調査に必要な情報が揃うことを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `: > $PMT/notify.log` で証跡ファイルを空にする | `notify.log` が空になる |
| 2 | `pulsen add --workflow wf-fail --repo $PMT/repo` を実行し、タスクIDを控える(以下 `<ID>`) | タスクIDが表示され、exit code 0 |
| 3 | `pulsen tick` を 2〜3 秒間隔で繰り返す(目安 10 回)。途中で `pulsen ls` と `cat $PMT/notify.log` を確認する | 実行状態が failed を経由して自動リトライされる。failed の間は `notify.log` に行が増えない(リトライ中は通知されない) |
| 4 | stopped 到達後、`pulsen ls --state stopped` を実行する | `<ID>` が表示され、実行状態が stopped、タスクステータスは work のまま |
| 5 | `cat $PMT/notify.log` を実行する | `TASK_ID=<ID> WORKFLOW=wf-fail TASK_STATUS=work` を含む行がちょうど 1 行ある |
| 6 | `pulsen show <ID>` を実行する | 凍結要因(リトライ上限超過)、attempt_count が上限超過値(上限値の併記あり)、notified_at の記録、runディレクトリの `stdout.log` / `stderr.log` / `exit` のパスと exit の値(1)が表示される |
| 7 | show に表示された `stderr.log` のパスを `cat` する | `boom` が出力されている(凍結原因をログから特定できる) |
| 8 | `ls $PULSEN_HOME/worktrees/<ID>` を実行する | worktree が存在する(凍結後も保持され、作業ツリーを直接確認できる) |

**確認ポイント**:
- 続けて `pulsen tick` を実行しても `<ID>` は起動・遷移されず、`notify.log` に行が増えない(通知済みの stopped は再通知されない)

## TC-02: running タスクの abort(kill を伴う中断)

**種別**: 正常系
**目的**: 実行中のエージェントが直ちに kill され、tick を待たずに stopped が確定して通知されることを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | 手順R1 で running のタスク `<ID>` を用意し、`: > $PMT/notify.log` を実行する | `pulsen ls --state running` に `<ID>` が表示される |
| 2 | `pulsen abort <ID>` を実行し、`echo $?` を確認する | kill を実行したことを含めて stopped を記録した旨が表示され、exit code 0 |
| 3 | `pgrep -f "sleep 300"` を実行する | 該当プロセスがない(プロセスグループ相当の単位で kill されている) |
| 4 | `pulsen show <ID>` を実行する(tick は挟まない) | 実行状態 stopped、凍結要因「人間による abort」、notified_at が記録済み(CLI 操作自体が確定・通知した) |
| 5 | `cat $PMT/notify.log` を実行する | `TASK_ID=<ID>` の行がちょうど 1 行ある(tick なしで通知された) |
| 6 | `ls $PULSEN_HOME/worktrees/<ID>` を実行する | worktree が保持されている(中断時点の作業状態を確認できる) |
| 7 | `pulsen tick` を実行する | `<ID>` は起動されない(凍結が維持される) |

## TC-03: pending タスクの abort(起動前の凍結)

**種別**: 正常系
**目的**: kill 対象のない pending への abort が stopped の記録のみを行い、次の tick による起動を止めることを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | 手順R4 で pending のタスク `<ID>` を用意する | `pulsen ls --state pending` に `<ID>` が表示される |
| 2 | `pulsen abort <ID>` を実行し、`echo $?` を確認する | kill なしで stopped を記録した旨が表示され、exit code 0 |
| 3 | `pulsen show <ID>` を実行する | 実行状態 stopped、凍結要因は abort、attempt 関連は「なし」、workspace は「未作成」(一度も実行されていない) |
| 4 | `pulsen tick` を実行する | `<ID>` は起動されない |

## TC-04: failed タスクの abort(自動リトライの打ち切り)

**種別**: 正常系
**目的**: 放置すれば自動リトライされる failed タスクを、これ以上リトライさせずに凍結できることを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | 手順R5 で failed のタスク `<ID>` を用意し、`pulsen show <ID>` で attempt_count を控える | 実行状態 failed、attempt_count は上限未満(例: 1) |
| 2 | `pulsen abort <ID>` を実行する | kill なしで stopped を記録した旨が表示され、exit code 0 |
| 3 | `pulsen tick` を 2 回実行する | `<ID>` は再起動されない(自動リトライが止まる)。show で attempt_count が増えていない |

## TC-05: completed タスクの abort(次ステータスへの遷移の打ち切り)

**種別**: 正常系
**目的**: 判定済み(completed)のタスクへの abort が、次の tick による次ステータスへの遷移を打ち切ることを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | 手順R6 で completed のタスク `<ID>` を用意する | `pulsen ls --state completed` に `<ID>`(タスクステータス work)が表示される |
| 2 | すぐに `pulsen abort <ID>` を実行する | kill なしで stopped を記録した旨が表示され、exit code 0 |
| 3 | `pulsen tick` を実行し、`pulsen show <ID>` を確認する | review_waiting へ遷移せず、タスクステータス work のまま実行状態 stopped |

## TC-06: abort で凍結したタスクの retry(カウンタリセットと再起動)

**種別**: 正常系
**目的**: retry が 3 つのカウンタをリセットして pending に戻し、次の tick が同じタスクステータスを新しい attempt で再起動することを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | TC-02 の stopped タスク(なければ手順R1 → `pulsen abort`)`<ID>` を用意し、`pulsen show <ID>` で attempt 番号を控える | 実行状態 stopped |
| 2 | `pulsen retry <ID>` を実行し、`echo $?` を確認する | pending に戻した旨と再開されるタスクステータス(work)が表示され、exit code 0 |
| 3 | `pulsen show <ID>` を実行する | attempt_count / judge_attempt_count / spawn_fail_count がすべて 0、実行状態 pending、タスクステータスは work のまま。workspace と attempt 参照(前回の attempt 番号・runディレクトリ)は保持されている |
| 4 | `pulsen tick` → 2〜3 秒後に `pulsen tick` を実行する | `<ID>` が起動され running になる。show の attempt 番号が手順1 より増えている(新しい attempt) |
| 5 | 後片付け: `pulsen abort <ID>` を実行する | stopped に戻る(sleep 300 を放置しない) |

## TC-07: 原因未解消のまま retry した場合の再凍結と再通知

**種別**: 正常系
**目的**: 恒常的な失敗要因のまま retry しても無限ループにならず、再びリトライ上限で凍結して必ず再通知されることを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | TC-01 の stopped タスク(wf-fail。なければ手順R2)`<ID>` を用意し、`: > $PMT/notify.log` を実行する | 実行状態 stopped |
| 2 | `pulsen retry <ID>` を実行する | exit code 0、実行状態 pending(凍結が解ける) |
| 3 | `pulsen tick` を 2〜3 秒間隔で繰り返す(目安 10 回) | 再び失敗を重ね、リトライ上限超過で stopped に戻る(それ以上 tick しても起動されない = 無限ループにならない) |
| 4 | `cat $PMT/notify.log` を実行する | `TASK_ID=<ID>` の行が新たに 1 行ある(retry 後の再凍結でも必ず通知される。過去の通知記録は引き継がれない) |

## TC-08: 判定不能で凍結したタスクの、判定コマンド修正後の retry

**種別**: 正常系
**目的**: 判定コマンドの不具合による凍結を、スナップショットが参照するコマンド実体(スクリプトファイル)側の修正と retry で復旧できることを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `$PMT/judge.sh` の中身が `exit 5` であることを確認し、`pulsen add --workflow wf-judge --repo $PMT/repo` でタスク `<ID>` を登録する | タスクIDが表示される |
| 2 | `pulsen tick` を 2〜3 秒間隔で繰り返す(目安 8 回) | エージェント(`true`)は成功するが判定失敗が繰り返され、再判定上限の超過で stopped になり通知される |
| 3 | `pulsen show <ID>` を実行する | judge_attempt_count が上限超過値(上限値の併記あり)、凍結要因が判定不能であることが表示される |
| 4 | `printf '#!/bin/sh\nexit 0\n' > $PMT/judge.sh` で判定コマンドの実体を修正する | ワークフロー定義(スナップショット)は変更せず、参照先のスクリプトだけが直る |
| 5 | `pulsen retry <ID>` を実行し、`pulsen show <ID>` を確認する | exit code 0。judge_attempt_count を含む全カウンタが 0、実行状態 pending |
| 6 | `pulsen tick` を 2〜3 秒間隔で繰り返す(目安 4 回) | completed 判定を経て review_waiting へ遷移する(復旧成功) |

## TC-09: 連続 spawn 失敗で凍結したタスクの、設定修正後の retry

**種別**: 正常系
**目的**: グローバル設定の不備による連続 spawn 失敗の凍結を、config.yaml の修正(実行時解決のため既存タスクに効く)と retry で復旧できることを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `pulsen add --workflow wf-spawn --repo $PMT/repo` でタスク `<ID>` を登録する | タスクIDが表示される(登録時点の config は正常なため検証を通過する) |
| 2 | `$PULSEN_HOME/config.yaml` の `shell2` の cmd を `["sh", "-c", "{bogus}"]` に書き換える(未知プレースホルダ) | 登録済みタスクには add の検証が働かず、起動時の展開失敗として現れる |
| 3 | `pulsen tick` を数回実行し、各回の後に `pulsen show <ID>` を確認する | 実行状態は pending のまま spawn_fail_count が 1 ずつ増え、上限(デフォルト 3)超過で stopped になり通知される |
| 4 | config.yaml の `shell2` を `["sh", "-c", "{input}"]` に戻す | グローバル設定は各実行時に解決されるため、修正がそのまま既存タスクに効く |
| 5 | `pulsen retry <ID>` を実行し、`pulsen show <ID>` を確認する | exit code 0。spawn_fail_count を含む全カウンタが 0、実行状態 pending |
| 6 | `pulsen tick` を 2〜3 秒間隔で繰り返す(目安 4 回) | 正常に起動・完了し review_waiting へ遷移する(復旧成功) |

## TC-10: stopped タスクの set-status による別ステータスからのやり直し

**種別**: 正常系
**目的**: 凍結したタスクを別ステータスへ手動遷移させると凍結が解け、worktree を引き継いで遷移先の定義で再実行されることを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | 手順R2 で stopped のタスク `<ID>`(wf-fail)を用意する | 実行状態 stopped、タスクステータス work |
| 2 | `pulsen show <ID>` を実行する | スナップショットの定義済みステータス一覧に work / fixup / review_waiting / done が表示される(遷移先の確認) |
| 3 | `pulsen set-status <ID> fixup` を実行し、`echo $?` を確認する | 遷移元(work)と遷移先(fixup)が表示され、exit code 0 |
| 4 | `pulsen show <ID>` を実行する | タスクステータス fixup、実行状態 pending(凍結が解けた)、カウンタすべて 0 |
| 5 | `pulsen tick` を 2〜3 秒間隔で繰り返す(目安 4 回) | fixup のエージェントが実行され、completed を経て review_waiting へ遷移する |
| 6 | `cat $PULSEN_HOME/worktrees/<ID>/pulsen-test.txt` を実行する | 日付が 1 行ある(同一 worktree の上で遷移先ステータスが実行された) |

## TC-11: 実行中タスクの abort → set-status(複合操作)

**種別**: 正常系
**目的**: kill + 遷移の複合コマンドは存在せず、実行中タスクの別ステータスへの移動が abort → set-status の順で表現できることを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | 手順R1 で running のタスク `<ID>` を用意する | `pulsen ls --state running` に表示される |
| 2 | `pulsen abort <ID>` を実行する | kill を伴って stopped が記録され、exit code 0。`pgrep -f "sleep 300"` で該当プロセスがない |
| 3 | 続けて `pulsen set-status <ID> fixup` を実行する | exit code 0。タスクステータス fixup、実行状態 pending |
| 4 | `pulsen tick` を 2〜3 秒間隔で繰り返す(目安 4 回) | fixup が実行され review_waiting へ遷移する |

## TC-12: レビュー待ちステータスへの到達と手動進行(承認 → クリーンアップ)

**種別**: 正常系
**目的**: 「何もしない」ステータスでの滞留は通知されず tick も何もしないこと、確認後の set-status で次(クリーンアップ)へ進められることを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `: > $PMT/notify.log` を実行し、手順R3 で review_waiting 滞留のタスク `<ID>`(wf-wait)を用意する | `pulsen ls --status review_waiting` に `<ID>` が表示され、実行状態は pending(stopped ではない) |
| 2 | `cat $PMT/notify.log` を実行する | 空のまま(「何もしない」ステータスへの到達は stopped ではないため通知されない) |
| 3 | `pulsen tick` を実行する | `<ID>` に対して何も起きない(滞留。tick サマリーに当該タスクのアクションがない) |
| 4 | `pulsen show <ID>` で worktree パスを確認し、`cat $PULSEN_HOME/worktrees/<ID>/pulsen-test.txt` で成果物をレビューする | 日付が 1 行ある(成果物をツール外で確認できる) |
| 5 | `pulsen set-status <ID> done` を実行する | exit code 0(launching / running ではないため abort を挟まず受理される) |
| 6 | `pulsen tick` を実行する | 終端処理が行われる: `ls $PULSEN_HOME/worktrees/` に `<ID>` がない(worktree 削除)、`pulsen ls` に表示されず、`pulsen ls --all` にアーカイブ済みの印付きで表示される |

**確認ポイント**:
- このタスクは TC-17 の「アーカイブ済みタスク」前提として流用できる

## TC-13: レビュー待ちからの差し戻し(worktree 引き継ぎの確認)

**種別**: 正常系
**目的**: レビュー待ちから修正用ステータスへ差し戻すと、前回までの成果が残った worktree の上で再実行されることを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | 手順R3 で review_waiting 滞留のタスク `<ID>`(wf-wait)を用意する | `pulsen-test.txt` に日付が 1 行ある |
| 2 | `pulsen set-status <ID> fixup` を実行する | exit code 0(差し戻し) |
| 3 | `pulsen tick` を 2〜3 秒間隔で繰り返す(目安 4 回) | fixup が実行され、再び review_waiting に滞留する |
| 4 | `cat $PULSEN_HOME/worktrees/<ID>/pulsen-test.txt` を実行する | 日付が 2 行ある(前回の成果の上に追記された = worktree・ブランチが引き継がれている) |

## TC-14: timeout による自動 kill と abort の区別

**種別**: 正常系
**目的**: timeout kill は failed → 自動リトライ(通知なし)であり、凍結(stopped)には至らないこと、リトライさせずに止めるのは abort であることを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `: > $PMT/notify.log` を実行し、`pulsen add --workflow wf-timeout --repo $PMT/repo` でタスク `<ID>` を登録する | タスクIDが表示される |
| 2 | `pulsen tick` → 2〜3 秒後に `pulsen tick` で running にする | `pulsen ls --state running` に表示される |
| 3 | 起動から 10 秒以上待って `pulsen tick` を実行する | timeout 超過で kill され、実行状態が failed になる(stopped ではない)。`pgrep -f "sleep 300"` で該当プロセスがない |
| 4 | `cat $PMT/notify.log` を実行する | 空のまま(timeout kill では通知されない) |
| 5 | `pulsen tick` を実行する | 自動リトライで再起動される(launching / running)。timeout を待っても凍結にはならない |
| 6 | `pulsen abort <ID>` を実行する | こちらは stopped が確定し、`notify.log` に通知の行が追加される(abort との違い) |

## TC-15: notify_cmd 未設定時の凍結と、後から定義した場合の catch-up 通知

**種別**: 正常系
**目的**: notify_cmd 未設定なら stopped は通知されず notified_at も書かれないこと、後から定義すると次の tick が catch-up 通知することを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `$PULSEN_HOME/config.yaml` の `notify_cmd` 行をコメントアウトし、`: > $PMT/notify.log` を実行する | notify_cmd が未定義になる |
| 2 | 手順R4 で pending のタスク `<ID>` を用意し、`pulsen abort <ID>` を実行する | exit code 0、stopped が記録される |
| 3 | `cat $PMT/notify.log` と `pulsen show <ID>` を確認する | `notify.log` は空のまま。notified_at は記録されていない |
| 4 | `pulsen tick` を実行する | 通知されない(`ls` での定期確認が代替手段となる) |
| 5 | config.yaml の `notify_cmd` を元に戻し、`pulsen tick` を実行する | 「notified_at のない stopped」が検出され、`notify.log` に `TASK_ID=<ID>` の行が追加される(catch-up 通知)。show で notified_at が記録される |

## TC-16: retry 直後・tick 前の abort(起動前の再凍結)

**種別**: 正常系
**目的**: retry で pending に戻した直後でも、tick 前に abort すれば起動されずに再凍結できることを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | stopped のタスク(TC-15 の `<ID>` を流用可)に `pulsen retry <ID>` を実行する | exit code 0、実行状態 pending |
| 2 | tick を挟まず `pulsen abort <ID>` を実行する | kill なしで stopped が記録され、exit code 0。通知も発火する(`notify.log` に新しい行) |
| 3 | `pulsen tick` を実行する | `<ID>` は起動されない(起動前に凍結し直せた) |

## TC-17: 存在しないタスクIDへの abort / retry / set-status

**種別**: 異常系
**目的**: 未登録のタスクIDを指定した介入操作がエラーを返し、何も変更しないことを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `pulsen ls` で現在の一覧を控える | 一覧が表示される |
| 2 | `pulsen abort no-such-task` を実行し、`echo $?` を確認する | タスク不在の旨が表示され、exit code 非0 |
| 3 | `pulsen retry no-such-task` を実行し、`echo $?` を確認する | タスク不在の旨が表示され、exit code 非0 |
| 4 | `pulsen set-status no-such-task work` を実行し、`echo $?` を確認する | タスク不在の旨が表示され、exit code 非0 |
| 5 | `pulsen ls` を再実行する | 手順1 と同じ(どのタスクも変更されていない) |

## TC-18: アーカイブ済みタスクへの abort / retry / set-status

**種別**: 異常系
**目的**: アーカイブ済み(走査対象外)のタスクへの介入操作が「操作不可」としてエラーになることを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | TC-12 でアーカイブ済みになったタスク `<ID>` を用意する(なければ手順R3 → `set-status <ID> done` → `pulsen tick`) | `pulsen ls --all` にアーカイブ済みの印付きで表示される |
| 2 | `pulsen abort <ID>` を実行し、`echo $?` を確認する | アーカイブ済みは操作不可の旨が表示され、exit code 非0 |
| 3 | `pulsen retry <ID>` を実行し、`echo $?` を確認する | 同様に exit code 非0 |
| 4 | `pulsen set-status <ID> fixup` を実行し、`echo $?` を確認する | 同様に exit code 非0 |
| 5 | `pulsen show <ID>` を実行する | アーカイブ済みの注記付きで表示され(exit code 0)、状態は変化していない |

## TC-19: stopped 以外への retry の状態別案内

**種別**: 異常系
**目的**: retry が stopped のタスクのみを受理し、それ以外の実行状態には状態に応じた案内文言付きで拒否することを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | 手順R4 で pending のタスク `<ID1>` を用意し、`pulsen retry <ID1>` を実行する | 拒否され「既に実行待ち」の旨が表示され、exit code 非0 |
| 2 | `pulsen tick` を実行し、`pulsen ls --state launching` で `<ID1>` が launching であることを確認して `pulsen retry <ID1>` を実行する | 拒否され「実行中。止めたい場合は先に abort」の旨が表示され、exit code 非0 |
| 3 | 2〜3 秒後に `pulsen tick` で running にし、`pulsen retry <ID1>` を実行する | 拒否され「先に abort」の旨が表示され、exit code 非0 |
| 4 | 手順R5 で failed のタスク `<ID2>` を用意し、`pulsen show <ID2>` で attempt_count を控えて `pulsen retry <ID2>` を実行する | 拒否され「放置すれば次のtickで自動リトライされる」の旨が表示され、exit code 非0 |
| 5 | 直後に `pulsen show <ID2>` を確認してから `pulsen abort <ID2>` を実行する | show で実行状態 failed・attempt_count が手順4 の値と同一(拒否時は何も書き込まれない)。abort で stopped になる(failed を放置すると以降の tick の自動リトライで状態が変わるため、確認は retry の直前後で行い、確認後は abort で影響を断つ) |
| 6 | 手順R6 で completed のタスク `<ID3>` を用意し、すぐに `pulsen retry <ID3>` を実行する | 拒否され「判定済み。次のtickが次ステータスへ遷移させる」の旨が表示され、exit code 非0 |
| 7 | tick を挟まず `pulsen show <ID1>` と `pulsen show <ID3>` を確認する | `<ID1>` は実行状態 running、`<ID3>` は実行状態 completed のまま、いずれもカウンタが変化していない(拒否時は何も書き込まれない) |
| 8 | 後片付け: `pulsen abort <ID1>` を実行する | running のタスクを残さない |

## TC-20: launching / running への set-status 拒否と正規手順への誘導

**種別**: 異常系
**目的**: 実行中プロセスを残したままステータスだけが変わる中途半端な状態を作らないため、launching / running への set-status が拒否され、案内どおり abort を挟めば成功することを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | 手順R4 → `pulsen tick` で launching のタスク `<ID>` を用意し(`pulsen ls --state launching` で確認)、`pulsen set-status <ID> fixup` を実行する | 拒否され「先に abort せよ」の旨が表示され、exit code 非0 |
| 2 | 2〜3 秒後に `pulsen tick` で running にし、`pulsen set-status <ID> fixup` を実行する | 同様に拒否され、exit code 非0 |
| 3 | `pulsen show <ID>` を実行する | タスクステータス work・実行状態 running のまま(変更されていない) |
| 4 | 案内どおり `pulsen abort <ID>` → `pulsen set-status <ID> fixup` の順で実行する | いずれも exit code 0。タスクステータス fixup・実行状態 pending になる(回復手順が機能する) |

## TC-21: スナップショットに存在しないステータス名の set-status

**種別**: 異常系
**目的**: 未定義のステータス名を指定した場合、定義済みステータスの一覧を添えて拒否され、何も変更されないことを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | 手順R3 で review_waiting 滞留のタスク `<ID>` を用意する | `pulsen ls --status review_waiting` に表示される |
| 2 | `pulsen set-status <ID> nonexistent-status` を実行し、`echo $?` を確認する | 拒否され、定義済みステータスの一覧(work / fixup / review_waiting / done)が表示され、exit code 非0 |
| 3 | `pulsen show <ID>` を実行する | タスクステータス review_waiting のまま、カウンタも変化なし |

## TC-22: 登録後に元YAMLへ追加したステータスは既存タスクに指定できない

**種別**: 異常系
**目的**: set-status の検証基準が登録時のスナップショットであり、元YAMLの事後編集が既存タスクに影響しないことを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | 手順R3 で review_waiting 滞留のタスク `<ID>` を用意する | 表示を確認する |
| 2 | `$PULSEN_HOME/workflows/wf-wait.yaml` の `statuses:` に `extra:` / `run: wait` のステータスを追記する | ファイルが保存される |
| 3 | `pulsen set-status <ID> extra` を実行し、`echo $?` を確認する | 拒否され exit code 非0。表示される定義済み一覧に `extra` は含まれない(スナップショットが基準) |
| 4 | `pulsen add --workflow wf-wait --repo $PMT/repo` で新規タスク `<ID2>` を登録し、`pulsen set-status <ID2> extra` を実行する | 新規タスクのスナップショットには `extra` が含まれるため受理される(exit code 0) |
| 5 | 後片付け: wf-wait.yaml から `extra` を削除し、`pulsen abort <ID2>` を実行する | 元の定義に戻る |

## TC-23: 二重 abort の冪等性

**種別**: 異常系
**目的**: すでに stopped のタスクへの abort が失敗扱いにならず(exit code 0)、状態変更も再通知も行われないことを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | stopped のタスク `<ID>`(手順R4 → `pulsen abort` で作成)を用意し、`pulsen show <ID>` で notified_at を控え、`: > $PMT/notify.log` を実行する | 実行状態 stopped、notified_at 記録済み |
| 2 | `pulsen abort <ID>` をもう一度実行し、`echo $?` を確認する | すでに凍結済みである旨が表示され、exit code 0(失敗扱いにしない) |
| 3 | `cat $PMT/notify.log` と `pulsen show <ID>` を確認する | `notify.log` は空のまま(再通知されない)。notified_at・実行状態・カウンタは手順1 と同一(何も変更されていない) |

## TC-24: 通知失敗時の再通知(at-least-once)

**種別**: 異常系
**目的**: notify_cmd の実行失敗時も stopped の記録は完了(exit code 0)し、notified_at が残らないため次の tick が自動で再通知することを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `$PULSEN_HOME/config.yaml` の notify_cmd を `["sh", "-c", "exit 1"]`(必ず失敗する通知)に変更し、`: > $PMT/notify.log` を実行する | 設定が保存される |
| 2 | 手順R4 で pending のタスク `<ID>` を用意し、`pulsen abort <ID>` を実行して `echo $?` を確認する | stopped の記録は完了して exit code 0。通知に失敗した旨と「次のtickが再通知する」旨の警告が表示される |
| 3 | `pulsen show <ID>` を実行する | 実行状態 stopped だが notified_at は記録されていない |
| 4 | notify_cmd を `["sh", "<PMT>/notify.sh"]` に戻し、`pulsen tick` を実行する | `notify.log` に `TASK_ID=<ID>` の行が追加され(再通知)、show で notified_at が記録される。利用者側の特別な操作は不要 |
| 5 | さらに `pulsen tick` を実行する | `notify.log` に行が増えない(通知済みは再通知されない) |

## TC-25: 排他ロック競合時の拒否

**種別**: 異常系
**目的**: 状態を変更する CLI 操作が tick と同一の排他ロックを取得し、競合時は「別の操作が実行中」として何も変更せず終了することを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `$PULSEN_HOME/config.yaml` の notify_cmd を `["sh", "-c", "sleep 30"]` に変更する(通知はロック保持中に同期実行されるため、約 30 秒のロック保持時間を作れる) | 設定が保存される |
| 2 | 手順R4 で pending のタスク `<ID>` を用意し、端末Aで `pulsen abort <ID>` を実行する | 約 30 秒間、コマンドが完了しない(ロック保持中) |
| 3 | その間に端末B(`PMT` / `PULSEN_HOME` を同様に設定)で `pulsen retry <ID>` を実行し、`echo $?` を確認する | 「別の操作が実行中」の旨が表示され、待たされずに exit code 非0 |
| 4 | 同じく端末Bで `pulsen ls` を実行する | ロックを取得しないため待たされず一覧が表示される |
| 5 | 端末Aの完了後、notify_cmd を `["sh", "<PMT>/notify.sh"]` に戻し、端末Bで `pulsen retry <ID>` を実行する | 今度は受理され exit code 0(競合時に部分的な変更が残っていない) |

## TC-26: パース不能なタスクファイルへの操作拒否

**種別**: 異常系
**目的**: 破損したタスクファイルに対する介入操作が非0で終了し、破損ファイルへ一切書き込まない(修復材料を壊さない)ことを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | 手順R4 で pending のタスク `<ID>` を用意し、`pulsen show <ID>` でタスクファイルのパス(以下 `<FILE>`)を確認する | タスクファイルパスが表示される |
| 2 | `cp <FILE> $PMT/backup.json` でバックアップし、`echo 'garbage' > <FILE>` で破損させる | ファイルが `garbage` のみになる |
| 3 | `pulsen abort <ID>`、`pulsen retry <ID>`、`pulsen set-status <ID> fixup` をそれぞれ実行し、`echo $?` を確認する | いずれもパース不能の旨が表示され、exit code 非0 |
| 4 | `cat <FILE>` を実行する | 内容が `garbage` のまま(破損ファイルへ書き込まれていない) |
| 5 | `cp $PMT/backup.json <FILE>` で復旧し、`pulsen show <ID>` を実行する | 元どおり表示できる(exit code 0) |

## TC-27: config.yaml 不在時の操作拒否

**種別**: 異常系
**目的**: グローバル設定が読めない場合、介入操作が状態を変更せずに非0で終了し、未初期化の案内が表示されることを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `mv $PULSEN_HOME/config.yaml $PMT/config.bak` で設定を退避する | config.yaml が不在になる |
| 2 | 現役タスク `<ID>` に対して `pulsen abort <ID>`、`pulsen retry <ID>`、`pulsen set-status <ID> fixup` をそれぞれ実行し、`echo $?` を確認する | いずれも「グローバルホームが未初期化」である旨・解決後のホームパスが表示され、exit code 非0 |
| 3 | `mv $PMT/config.bak $PULSEN_HOME/config.yaml` で復旧し、`pulsen show <ID>` を実行する | タスクの状態は手順1 の前と変わっていない |

## TC-28: タスクIDの形式検証(空・不正文字・長さの境界)

**種別**: 境界値
**目的**: 介入操作の入力境界でタスクIDが検証され、形式不正は状態解決より前に拒否されることを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `pulsen retry ""` を実行し、`echo $?` を確認する | 空のIDとして検証エラーが表示され、exit code 非0 |
| 2 | `pulsen abort INVALID_ID` を実行し、`echo $?` を確認する | 不正な文字(大文字・`_`)として検証エラーが表示され、exit code 非0 |
| 3 | `pulsen retry $(printf 'a%.0s' $(seq 1 65))` を実行し、`echo $?` を確認する | 長さ上限(64 文字)超過として検証エラーが表示され、exit code 非0 |
| 4 | `pulsen retry $(printf 'a%.0s' $(seq 1 64))` を実行し、`echo $?` を確認する | 64 文字は形式として有効なため検証は通過し、「タスク不在」として exit code 非0(エラー種別が手順3 と異なる) |

## TC-29: ステータス名の形式検証(空文字・前後空白)

**種別**: 境界値
**目的**: set-status のステータス名が入力境界で検証され、空文字・前後空白付きが拒否されることを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | 手順R3 で review_waiting 滞留のタスク `<ID>` を用意する | launching / running 以外の状態にある |
| 2 | `pulsen set-status <ID> ""` を実行し、`echo $?` を確認する | 空のステータス名として検証エラーが表示され、exit code 非0 |
| 3 | `pulsen set-status <ID> " fixup "` を実行し、`echo $?` を確認する | 前後空白付きとして検証エラーが表示され、exit code 非0 |
| 4 | `pulsen show <ID>` を実行する | タスクステータス review_waiting のまま変化なし |

## TC-30: 現在と同じステータス名への set-status

**種別**: 境界値
**目的**: 遷移経路の制約がないため同一ステータスの指定も受理され、一律規則どおりカウンタリセット・pending 化されることを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | 手順R3 で review_waiting 滞留のタスク `<ID>` を用意する | タスクステータス review_waiting、実行状態 pending |
| 2 | `pulsen set-status <ID> review_waiting` を実行し、`echo $?` を確認する | 受理され exit code 0。遷移元・遷移先とも review_waiting と表示される |
| 3 | `pulsen show <ID>` を実行する | カウンタすべて 0、実行状態 pending のまま滞留を継続する |

**確認ポイント**:
- stopped のタスクを同じステータスのまま再実行したいだけなら retry を使う(set-status で同一ステータスを指定し直す必要はない)

## TC-31: スナップショットのみ破損したタスクへの retry(受理+警告)と set-status(拒否)

**種別**: 異常系
**目的**: タスクファイル自体は読めるがスナップショットが読めない縮退状態(`SnapshotUnreadable`)で、スナップショットに依存しない retry は受理されて修復が必要な旨が警告され、遷移先の検証にスナップショットが必要な set-status は拒否されることを検証する。破損状態は monitoring.md TC-24 と同じ手順で作る

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | 手順R4 で pending のタスク `<ID>` を用意して `pulsen abort <ID>` を実行し(stopped 化)、`pulsen show <ID>` でタスクファイルのパス(以下 `<FILE>`)を確認する | 実行状態 stopped。タスクファイルパスが表示される |
| 2 | `cp <FILE> $PMT/backup-snap.json` でバックアップし、エディタで `<FILE>` の埋め込まれたワークフロー定義(スナップショット)部分の値だけを不正な構造に置き換えて保存する(monitoring.md TC-24 手順2 と同じ方法。ファイル全体は JSON として妥当なまま保つ) | タスク属性は読めるがスナップショットが読めない状態になる |
| 3 | `pulsen set-status <ID> fixup` を実行し、`echo $?` を確認する | スナップショットが読めず遷移先を検証できない旨が表示されて拒否され、exit code 非0。タスクは変更されない |
| 4 | `pulsen retry <ID>` を実行し、`echo $?` を確認する | 受理されて exit code **0**。スナップショットの修復が必要である旨の警告が表示される。`pulsen show <ID>` で実行状態 pending・カウンタすべて 0(縮退の注記付き表示) |
| 5 | `pulsen tick` を実行する | `<ID>` は起動されず、スナップショット読み取り不能によるスキップが報告される(pending に戻しても修復するまで tick に拾われない = 手順4の警告のとおり) |
| 6 | 復元: `cp $PMT/backup-snap.json <FILE>` を実行し、`pulsen show <ID>` を確認する | 通常表示に戻る(内容は retry 前の stopped。以後は retry で再開するか、stopped のまま残してよい) |

## TC-32: launching タスクの abort(pid 出現後の照合付き kill)

**種別**: 異常系
**目的**: launching(起動記録済み・spawn 確認前)のタスクへの abort が、タスクファイルではなく runディレクトリの pid ファイルから同定情報を取り、照合付き kill を伴って stopped を確定することを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `: > $PMT/notify.log` を実行し、手順R4 で pending のタスク `<ID>`(wf-longrun)を用意して `pulsen tick` を1回だけ実行し、2〜3 秒待つ | `pulsen ls --state launching` に `<ID>` が表示される(spawn 確認の tick は挟んでいないため launching のまま。待機によりラッパーは pid ファイルを書き終えている) |
| 2 | `pulsen abort <ID>` を実行し、`echo $?` を確認する | kill を実行したことを含めて stopped を記録した旨が表示され、exit code 0(同定情報はタスクファイルに未取り込みのため、runディレクトリの pid ファイルの参照と starttime 照合を経て kill される) |
| 3 | `pgrep -f "sleep 300"` を実行する | 該当プロセスがない(launching のうちに起動していたエージェントも kill されている) |
| 4 | `pulsen show <ID>` と `cat $PMT/notify.log` を確認する | 実行状態 stopped、凍結要因「人間による abort」、notified_at 記録済み。`notify.log` に `TASK_ID=<ID>` の行がちょうど 1 行ある |
| 5 | `pulsen tick` を実行する | `<ID>` は起動・spawn 確認されない(凍結が維持される) |

**確認ポイント**:
- pid ファイル未出現(無効化マーカーを書いて再確認し、記録のみで stopped とする)側の分岐は、ラッパーの pid 書き込みとのタイミング競合になるため本書では扱わない(カバレッジ表の対象外理由を参照)

## カバレッジ

### ユースケースエラーケース対応表

| ユースケース | エラーケース | 対応TC | 備考 |
|---|---|---|---|
| AbortTask | タスク不在(`NotFound`) | TC-17 | |
| AbortTask | アーカイブ済み | TC-18 | |
| AbortTask | タスクファイル破損(`Corrupt`) | TC-26 | |
| AbortTask | ロック競合 | TC-25 | |
| AbortTask | ロック機構自体の異常(`LockError::Failed`) | 対象外 | ロック機構の内部故障は手動で誘発できない |
| AbortTask | config 読み込み失敗 | TC-27 | |
| AbortTask | task_id の形式不正 | TC-28 | |
| AbortTask | kill 操作自体の失敗(`KillError`) | 対象外 | 照合一致後のシグナル送出・ジョブ終了のエラーは OS 障害であり手動で再現できない |
| AbortTask | 生存観測の Io 失敗(`starttime_of` の `Err`) | 対象外 | 同上(OS 側の観測障害を意図的に起こせない) |
| AbortTask | `save` / `save_degraded` の失敗 | 対象外 | ファイルシステム障害を安全に再現できない |
| AbortTask | 不変条件の破れ(running で attempt / process 欠落) | 対象外 | タスクファイル内部形式の手動編集に依存し、形式は実装詳細のため本書では手順化しない |
| AbortTask | launching への abort(pid 出現後: pid ファイル参照 → 照合付き kill) | TC-32 | tick 直後に 2〜3 秒待てば pid ファイルの出現が保証され、pid あり側の分岐を決定的に確認できる |
| AbortTask | launching への abort(pid 未出現: 無効化マーカー書き込み → 再確認 → 記録のみ) | 対象外 | 「pid 未出現」の瞬間はラッパーの pid 書き込みとのタイミング競合になり、手動で決定的に再現できない(マーカープロトコルはユニットテスト側でカバーされる想定) |
| AbortTask | launching の runファイル破損・pid あり starttime なし | 対象外 | ラッパーの書き込みとのタイミング競合が必要で、安定した手動再現ができない |
| AbortTask | `write_invalidation_marker` の失敗 | 対象外 | I/O 障害の再現不能 |
| AbortTask | notify_cmd 実行失敗(警告扱い・0 終了) | TC-24 | |
| AbortTask | すでに stopped(冪等成功) | TC-23 | |
| AbortTask | PID 再利用(starttime 照合不一致) | 対象外 | PID の再利用を意図したタイミングで起こせない(ユニットテスト側でカバーされる想定) |
| RetryTask | タスク不在 | TC-17 | |
| RetryTask | アーカイブ済み | TC-18 | |
| RetryTask | タスクファイル破損(`Corrupt`) | TC-26 | |
| RetryTask | ロック競合 | TC-25 | |
| RetryTask | ロック機構自体の異常(`LockError::Failed`) | 対象外 | 手動で誘発できない |
| RetryTask | config 読み込み失敗 | TC-27 | |
| RetryTask | stopped 以外への retry(`NotStopped`: pending / launching / running / failed / completed の状態別案内) | TC-19 | |
| RetryTask | task_id の形式不正・長さ境界 | TC-28 | |
| RetryTask | スナップショットのみ破損(`SnapshotUnreadable`)の受理+警告 | TC-31 | monitoring.md TC-24 と同じ手順で破損状態を作る |
| SetTaskStatus | タスク不在 | TC-17 | |
| SetTaskStatus | アーカイブ済み | TC-18 | |
| SetTaskStatus | タスクファイル破損(`Corrupt`) | TC-26 | |
| SetTaskStatus | スナップショットのみ破損(`SnapshotUnreadable`)の拒否 | TC-31 | 同上 |
| SetTaskStatus | ロック競合 | TC-25 | |
| SetTaskStatus | ロック機構自体の異常(`LockError::Failed`) | 対象外 | 手動で誘発できない |
| SetTaskStatus | config 読み込み失敗 | TC-27 | |
| SetTaskStatus | launching / running への遷移(`Active`) | TC-20 | |
| SetTaskStatus | スナップショットに無いステータス(`UnknownStatus`) | TC-21, TC-22 | |
| SetTaskStatus | task_id の形式不正 | TC-28 | |
| SetTaskStatus | ステータス名の形式不正 | TC-29 | |
| notify(共通手続き) | notify_cmd の実行失敗 → 次 tick の再通知 | TC-24 | |
| notify(共通手続き) | notify_cmd 未定義 → 通知なし・後から定義で catch-up | TC-15 | |
| notify(共通手続き) | notify_cmd 実行後・notified_at 追記前のクラッシュによる二重通知 | 対象外 | クラッシュのタイミングを手動で制御できない。重複の許容(欠落よりも重複)は仕様であり、欠落しないことは TC-24 で検証する |

### 観点チェックリスト

| 観点 | 対応TC | 対象外の理由 |
|---|---|---|
| 入力バリデーション | TC-28, TC-29 | |
| 境界値 | TC-28(ID 64 / 65 文字), TC-30(同一ステータス指定) | |
| 認証・権限 | 対象外 | 単一利用者のローカル CLI であり、認証・権限機構を持たない |
| 空状態・初期状態 | TC-27(config 不在 = 未初期化ホーム), TC-03(一度も実行されていないタスクへの介入) | タスク 0 件の一覧表示は「状態の確認と追跡」カテゴリの対象 |
| 重複・競合 | TC-23(二重 abort), TC-25(ロック競合) | |
| 削除・変更の影響 | TC-18(アーカイブ済みへの操作), TC-22(登録後の定義変更が既存タスクへ波及しない) | |
| 操作の中断・逸脱 | TC-16(retry 直後の翻意 → abort), TC-11(実行中タスクの正規手順 abort → set-status) | |
| 特殊入力 | TC-28(不正文字), TC-29(前後空白) | ID・ステータス名以外の自由入力欄は本カテゴリの操作にない |
| UIの状態(CLI 読み替え: 拒否後の回復・失敗後の再試行) | TC-19, TC-20(案内文言に従った再操作で回復), TC-24(通知失敗後の自動再試行) | |
