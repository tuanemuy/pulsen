# 状態の確認と追跡 テスト

## 概要

このドキュメントはタスクの一覧・詳細・実行ログの確認、stopped タスクの原因調査、タスクファイルの直接閲覧・修復に関するマニュアルテストの手順書です。

対応シナリオ: `spec/scenario/monitoring.md`

対象コマンドは読み取り系の `pulsen ls` / `pulsen show` と、runディレクトリ・タスクファイルの直接閲覧・修復操作。関連ユースケースは ListTasks / ShowTask(`spec/usecases/task.md`)。

## 前提条件

### 環境

- POSIX 環境(macOS / Linux)。`sh` / `git` が使えること
- `pulsen` バイナリがビルド済みで PATH から実行できること(例: `cargo build --release` 後に `target/release/pulsen` へ PATH を通す)
- `tick` の外部スケジューラー(cron 等)は登録しないこと。tick はすべて手動で実行する(破損ファイルの編集手順が tick の書き込みと競合しないようにするため)
- テスト用のグローバルホームを使う。すべてのシェルで以下を設定する

```sh
export PULSEN_HOME=$HOME/pulsen-manual-test
```

### テストデータ

事前準備で以下のタスクを作る。`add` が表示したタスクIDをシェル変数に控えて各テストケースで使う。

| 変数 | ワークフロー | 到達させる状態 | 用途 |
|---|---|---|---|
| `TASK_A` | wf-wait | pending(未実行。attempt 0) | 一覧・未実行タスクの詳細・タスクファイル閲覧 |
| `TASK_B` | wf-sleep | running(実行中) | 実行中タスクの詳細・ログ追跡 |
| `TASK_C` | wf-fail | stopped(リトライ上限超過) | stopped 原因調査・複数attemptのログ |
| `TASK_D` | wf-echo | アーカイブ済み(cleanup 到達) | `--all`・アーカイブ済み詳細・終了済みログ |
| `TASK_E` | wf-wait | pending → テスト中に JSON を破損させる | 破損ファイルの報告と修復 |
| `TASK_F` | wf-wait | pending → テスト中にスナップショット部分のみ破損させる | スナップショットのみ破損の縮退表示 |
| `TASK_G` | wf-wait | stopped(abort 経路) | stopped 原因調査(abort の判別) |

このほか TC-09(launching)・TC-16(判定不能)・TC-17(連続spawn失敗)は、テストケース内の手順で追加のタスクを登録する。

### 事前準備

1. 分離ホームとテスト領域を初期化し(既存なら削除して作り直す。過去の実行や他ドキュメントの残留状態を持ち込まないため)、設定を作成する。

    ```sh
    rm -rf $PULSEN_HOME $HOME/pulsen-test-repo $HOME/pulsen-manual-test-empty /tmp/pulsen-notify.log
    mkdir -p $PULSEN_HOME/workflows
    cat > $PULSEN_HOME/config.yaml <<'EOF'
    agents:
      sh:
        cmd: ["sh", "-c", "{input}"]
    notify_cmd: ["sh", "-c", "echo \"stopped: $TASK_ID ($WORKFLOW/$TASK_STATUS)\" >> /tmp/pulsen-notify.log"]
    EOF
    ```

2. ワークフローを4つ作成する。

    ```sh
    cat > $PULSEN_HOME/workflows/wf-wait.yaml <<'EOF'
    workflow: wf-wait
    initial: hold
    statuses:
      hold:
        run: wait
    EOF

    cat > $PULSEN_HOME/workflows/wf-sleep.yaml <<'EOF'
    workflow: wf-sleep
    agent: sh
    initial: work
    statuses:
      work:
        prompt: "sleep 3000"
        next: done
      done:
        run: cleanup
    EOF

    cat > $PULSEN_HOME/workflows/wf-fail.yaml <<'EOF'
    workflow: wf-fail
    agent: sh
    initial: work
    statuses:
      work:
        prompt: "exit 1"
        next: done
      done:
        run: cleanup
    EOF

    cat > $PULSEN_HOME/workflows/wf-echo.yaml <<'EOF'
    workflow: wf-echo
    agent: sh
    initial: work
    statuses:
      work:
        prompt: "echo hello from pulsen"
        next: done
      done:
        run: cleanup
    EOF
    ```

3. 対象リポジトリを作成する(グローバル git 設定のない環境でも初期コミットが成功するよう、identity をリポジトリローカルに設定する)。

    ```sh
    git init $HOME/pulsen-test-repo
    git -C $HOME/pulsen-test-repo config user.name pulsen-test
    git -C $HOME/pulsen-test-repo config user.email pulsen-test@example.com
    git -C $HOME/pulsen-test-repo commit --allow-empty -m init
    ```

4. tick の影響を受けないタスク(wf-wait)を4つ登録し、IDを控える。

    ```sh
    pulsen add --workflow wf-wait --repo $HOME/pulsen-test-repo   # → TASK_A に控える
    pulsen add --workflow wf-wait --repo $HOME/pulsen-test-repo   # → TASK_E
    pulsen add --workflow wf-wait --repo $HOME/pulsen-test-repo   # → TASK_F
    pulsen add --workflow wf-wait --repo $HOME/pulsen-test-repo   # → TASK_G
    ```

5. `TASK_G` を abort して stopped(abort 経路)にする。

    ```sh
    pulsen abort $TASK_G
    ```

6. 実行系のタスクを3つ登録し、IDを控える。

    ```sh
    pulsen add --workflow wf-sleep --repo $HOME/pulsen-test-repo  # → TASK_B
    pulsen add --workflow wf-fail  --repo $HOME/pulsen-test-repo  # → TASK_C
    pulsen add --workflow wf-echo  --repo $HOME/pulsen-test-repo  # → TASK_D
    ```

7. tick を繰り返し実行し、各タスクを目標状態へ進める。

    ```sh
    for i in $(seq 1 15); do pulsen tick; sleep 2; done
    ```

8. `pulsen ls` で以下を確認できたら準備完了(できていなければ tick を追加実行する)。
    - `TASK_B` が `work` / `running`
    - `TASK_C` が `work` / `stopped`(リトライ上限超過)
    - `TASK_D` が一覧に表示されない(アーカイブ済み)
    - `TASK_A` / `TASK_E` / `TASK_F` が `hold` / `pending`、`TASK_G` が `hold` / `stopped`

注意:

- テストケースは番号順の実行を前提とする。ファイルを破壊する操作は、各テストケース内の手順で必ず復元する。
- `wf-sleep` は 3000 秒でエージェントが終了する。テストセッションが 50 分を超える場合は `TASK_B` の状態が変わり得るため、事前準備からやり直す。
- exit code の確認はコマンド直後に `echo $?` で行う。

## TC-01: タスク一覧の全体像

**種別**: 正常系
**目的**: `ls` がタスクステータスと実行状態の両方を区別可能な形で一覧表示することを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | シェルで `pulsen ls` を実行する | 現役タスク(`TASK_A` / `TASK_B` / `TASK_C` / `TASK_E` / `TASK_F` / `TASK_G`)が一覧表示される |
| 2 | 各行の表示項目を確認する | タスクID・ワークフロー名・リポジトリ・ブランチ・タスクステータス・実行状態・attempt_count・更新日時が判別できる |
| 3 | `TASK_A`(hold / pending)と `TASK_G`(hold / stopped)の行を見比べる | 同じタスクステータス `hold` でも実行状態(pending / stopped)が別の列として区別できる |
| 4 | 一覧に `TASK_D` がないことを確認する | アーカイブ済みタスクは既定では表示されない |
| 5 | `echo $?` を実行する | `0` |

**確認ポイント**:

- `TASK_C` の attempt_count がリトライ上限(デフォルト2)を超えた値(3)になっている

## TC-02: `--status` によるタスクステータス絞り込み

**種別**: 正常系
**目的**: ユーザー定義のタスクステータスで絞り込めることを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `pulsen ls --status work` を実行する | タスクステータスが `work` のタスク(`TASK_B` / `TASK_C`)のみ表示される |
| 2 | `hold` のタスク(`TASK_A` 等)が表示されないことを確認する | 表示されない |
| 3 | `echo $?` を実行する | `0` |

## TC-03: `--state` による実行状態絞り込み

**種別**: 正常系
**目的**: 固定6値の実行状態で絞り込めることを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `pulsen ls --state stopped` を実行する | `TASK_C` と `TASK_G` のみ表示される |
| 2 | `pulsen ls --state pending` を実行する | `TASK_A` / `TASK_E` / `TASK_F` のみ表示される |
| 3 | `pulsen ls --state running` を実行する | `TASK_B` のみ表示される |
| 4 | 各コマンド後に `echo $?` を実行する | いずれも `0` |

## TC-04: `--status` と `--state` の併用(AND)

**種別**: 正常系
**目的**: 両方を指定したとき AND で絞り込まれることを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `pulsen ls --status work --state running` を実行する | `TASK_B` のみ表示される(`work` かつ `running`) |
| 2 | `pulsen ls --status work --state stopped` を実行する | `TASK_C` のみ表示される(`TASK_G` は `hold` のため表示されない) |
| 3 | 各コマンド後に `echo $?` を実行する | いずれも `0` |

## TC-05: `--all` によるアーカイブ済みの表示

**種別**: 正常系
**目的**: `--all` で対象集合が現役+アーカイブに拡張され、アーカイブ済みが判別できることを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `pulsen ls --all` を実行する | 現役タスクに加えて `TASK_D` が表示される |
| 2 | `TASK_D` の行を確認する | アーカイブ済みであることを示す印が付き、ブランチ(`pulsen/<TASK_D>`)も表示されている(成果の回収に使うため) |
| 3 | `echo $?` を実行する | `0` |

## TC-06: `--all` と絞り込みの併用

**種別**: 正常系
**目的**: `--all` が絞り込みではなく対象集合の拡張であり、拡張後に絞り込みが適用されることを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `pulsen ls --status done` を実行する | 該当なし。空である旨が表示され exit code 0(`done` は `TASK_D` のみで、現役集合に含まれない) |
| 2 | `pulsen ls --all --status done` を実行する | `TASK_D` のみ表示される(拡張後の集合に絞り込みが適用される) |
| 3 | `echo $?` を実行する | `0` |

## TC-07: 実行中タスクの詳細表示

**種別**: 正常系
**目的**: `show` が running タスクの全属性と実行メタデータを表示し、動いている attempt を特定できることを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `pulsen show $TASK_B` を実行する | 詳細が表示され exit code 0 |
| 2 | 基本属性を確認する | ワークフロー名 `wf-sleep`・対象リポジトリ(`$HOME/pulsen-test-repo`)・ベースブランチ・タスクステータス `work`・実行状態 `running`・更新日時が表示される |
| 3 | ワークスペース情報を確認する | workspace_path(`$PULSEN_HOME/worktrees/<TASK_B>`)とブランチ(`pulsen/<TASK_B>`)が表示される |
| 4 | カウンタを確認する | attempt_count(0)・judge_attempt_count(0)・spawn_fail_count(0)が、それぞれの上限(リトライ 2 / judge 3 / spawn 3)を併記して表示される(カウンタはいずれも連続失敗の数。一度も失敗していない実行中のタスクでは、手順6の attempt 番号が 1 でも attempt_count は 0) |
| 5 | 定義済みステータス一覧を確認する | スナップショットに定義された `work` / `done` が一覧表示され、スナップショット保存先としてタスクファイルのパスが表示される |
| 6 | 実行メタデータを確認する | 現在attemptの番号・runディレクトリパス(`state/runs/<TASK_B>/attempt-1`)・PID・starttime(・プラットフォームの kill 同定子)が表示され、どの attempt が動いているか特定できる |
| 7 | 手順3の workspace_path へ `ls <workspace_path>` で移動・閲覧する | worktree が実在し、成果物を直接確認できる |

## TC-08: 未実行タスク(pending・attempt 0)の詳細表示

**種別**: 正常系
**目的**: 一度も実行されていないタスクで未確定項目がエラーにならず「未確定・未実行」として表示されることを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `pulsen show $TASK_A` を実行する | 詳細が表示され exit code 0 |
| 2 | ワークスペース情報を確認する | workspace は「未作成」(最初の実行時に確定するため)、ブランチも未確定として表示される |
| 3 | attempt 関連の項目を確認する | attempt は「なし」。runディレクトリ・PID・exit への参照も表示されない(またはなしと明示される) |
| 4 | `ls $PULSEN_HOME/state/runs/$TASK_A` を実行する | ディレクトリが存在しない(「実行履歴なし」が自然な状態であり、ツール側もエラーにしていない) |

**確認ポイント**:

- `hold` は `run: wait`(何もしない)ステータスのため、attempt_count にリトライ上限が併記されない(適用対象がない)

## TC-09: launching タスクの詳細表示(同定情報 未取得)

**種別**: 正常系
**目的**: 起動記録済み・spawn確認未了のタスクが「未取得」として表示されることを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `pulsen add --workflow wf-sleep --repo $HOME/pulsen-test-repo` を実行し、表示されたIDを `TASK_H` に控える | タスクIDが表示され exit code 0 |
| 2 | 直後に `pulsen tick && pulsen show $TASK_H` を実行する | tick が起動を記録し(launching)、show は実行状態 `launching` を表示して exit code 0(pidファイル取り込みは次の tick のため、この時点では launching のまま) |
| 3 | 実行メタデータを確認する | attempt 番号と runディレクトリパスは表示されるが、PID・starttime 等の未取り込み項目は「未取得」と表示され、エラーにならない |
| 4 | 後片付けとして `pulsen abort $TASK_H` を実行する | stopped が記録され exit code 0 |

## TC-10: アーカイブ済みタスクの詳細表示

**種別**: 正常系
**目的**: `state/archive/` のタスクが表示でき、アーカイブ済み・worktree 削除済みが明示されることを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `pulsen show $TASK_D` を実行する | 詳細が表示され exit code 0 |
| 2 | 注記を確認する | アーカイブ済みであることが明示される |
| 3 | ワークスペース情報を確認する | workspace_path は「削除済み」であることが分かる形で表示され、ブランチ `pulsen/<TASK_D>` は表示される(ブランチは残っている) |
| 4 | タスクファイルパスを確認する | `state/archive/<TASK_D>.json` が表示される |
| 5 | `git -C $HOME/pulsen-test-repo branch --list "pulsen/*"` を実行する | `pulsen/<TASK_D>` ブランチが実在する |

## TC-11: 終了済み実行のログ確認

**種別**: 正常系
**目的**: runディレクトリの `stdout.log` / `stderr.log` / `exit` からエージェントの出力と終了結果を確認できることを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `pulsen show $TASK_D` を実行し、attempt の runディレクトリパス(`state/runs/<TASK_D>/attempt-1`)と `stdout.log` / `stderr.log` / `exit` のパスを控える | 各パスが表示される。show 自体が exit ファイルの値(`0`)も表示する |
| 2 | `cat` 等で `stdout.log` を閲覧する | `hello from pulsen` が記録されている(ラッパーがリダイレクトした生の出力。ツールによる加工はない) |
| 3 | `stderr.log` を閲覧する | 空である(エージェントが標準エラーに出力していないため。空のログは異常ではない) |
| 4 | `exit` を閲覧する | 数値 `0` が永続化されている |

## TC-12: 複数 attempt にまたがるログの遡り

**種別**: 正常系
**目的**: attempt 番号がパスに含まれ、リトライを跨いだ試行同士が混ざらずに遡れることを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `pulsen show $TASK_C` を実行し、現在attemptの runディレクトリパスを控える | `state/runs/<TASK_C>/attempt-3`(最終試行)が表示される |
| 2 | `ls $PULSEN_HOME/state/runs/$TASK_C/` を実行する | `attempt-1` / `attempt-2` / `attempt-3` が別ディレクトリとして存在する |
| 3 | attempt-1 から順に各ディレクトリの `exit` / `stderr.log` を閲覧する | すべての attempt で `exit` が `1`。毎回同じ失敗であることが試行ごとに分離されたログから判別できる |

**確認ポイント**:

- タスクファイル(show)が指す現在attemptのパスを起点にすれば、過去の試行の残骸と取り違えない

## TC-13: 実行中タスクのログ追跡

**種別**: 正常系
**目的**: 実行中の実行が「exit ファイルなし = 未終了」として判別でき、ログを追跡できることを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `pulsen show $TASK_B` を実行し、現在attemptの runディレクトリパスを控える | `state/runs/<TASK_B>/attempt-1` が表示される |
| 2 | `ls <runディレクトリ>` を実行する | `stdout.log` / `stderr.log` は存在するが `exit` が存在しない(実行は未終了) |
| 3 | `tail -f <runディレクトリ>/stdout.log` を数秒実行して Ctrl-C で抜ける | エラーなく追跡できる(`sleep` のため出力は増えないが、ファイルとして追跡可能。リアルタイム性は見る側の手段に依存する) |

## TC-14: stopped 原因調査(リトライ上限超過)

**種別**: 正常系
**目的**: リトライ上限超過で凍結したタスクの原因を show とログから特定できることを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `pulsen show $TASK_C` を実行する | 実行状態 `stopped` と凍結要因(直前実行の終了情報・最終出力への参照)が表示され exit code 0 |
| 2 | カウンタと上限を見比べる | attempt_count(3)がリトライ上限(2)を超過している → 4経路のうち「実行失敗の繰り返し」と判別できる。judge_attempt_count / spawn_fail_count は上限未満 |
| 3 | 最終attemptの runディレクトリの `exit` / `stderr.log` / `stdout.log` を閲覧する | `exit` が `1`(エージェント自体のエラー)。失敗の中身をログで確認できる |
| 4 | notified_at を確認する | 通知済みの日時が記録されている |
| 5 | `cat /tmp/pulsen-notify.log` を実行する | `stopped: <TASK_C> (wf-fail/work)` の行がある(notify_cmd が実行された) |
| 6 | show の workspace_path を `ls` で確認する | worktree が残っている(stopped の worktree は自動削除されない。現場がそのまま残る) |

## TC-15: stopped 原因調査(abort 経路)

**種別**: 正常系
**目的**: 人間による abort で凍結したタスクが、上限超過の経路と判別できることを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `pulsen show $TASK_G` を実行する | 実行状態 `stopped` が表示され exit code 0 |
| 2 | 凍結要因を確認する | 「人間による abort」であることが記録されており、TC-14 の上限超過と判別できる |
| 3 | カウンタを確認する | attempt_count / judge_attempt_count / spawn_fail_count はいずれも上限未満(0)。上限超過の3経路ではないことが裏付けられる |
| 4 | notified_at と `/tmp/pulsen-notify.log` を確認する | abort でも stopped 確定の通知が行われ、notified_at が記録されている |

## TC-16: stopped 原因調査(判定不能・再判定上限超過)

**種別**: 正常系
**目的**: 判定コマンドの失敗継続による凍結を判別し、runディレクトリで判定入力を再現確認できることを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | 判定コマンドが常に判定失敗(exit 5)を返すワークフロー `$PULSEN_HOME/workflows/wf-judge.yaml` を以下の内容で作成する。<br><br><pre>workflow: wf-judge<br>agent: sh<br>initial: work<br>statuses:<br>  work:<br>    prompt: "echo checked"<br>    judge: ["sh", "-c", "exit 5"]<br>    next: done<br>  done:<br>    run: cleanup</pre> | ファイルが作成される |
| 2 | `pulsen add --workflow wf-judge --repo $HOME/pulsen-test-repo` を実行し、IDを `TASK_J` に控える | タスクIDが表示され exit code 0 |
| 3 | `for i in $(seq 1 10); do pulsen tick; sleep 2; done` を実行する | tick のサマリーに判定の失敗・凍結が現れ、`TASK_J` が stopped になる(`pulsen ls --state stopped` に現れるまで tick を追加実行) |
| 4 | `pulsen show $TASK_J` を実行する | judge_attempt_count が上限(3)を超過している → 「判定自体の不能」と判別できる。attempt_count は上限未満 |
| 5 | show が表示するスナップショット保存先(タスクファイル)を開き、`work` の `judge` 定義を確認する | `["sh", "-c", "exit 5"]` がスナップショットに固定されており、判定コマンド側の不具合(エージェント再実行では解決しない)と特定できる |
| 6 | 現在attemptの runディレクトリの `exit` / ログを閲覧する | エージェント自体は `exit` = `0` で成功しており、失敗しているのは判定であることが裏付けられる(判定コマンドには同じ RUN_DIR が渡っていた) |

## TC-17: stopped 原因調査(連続 spawn 失敗・テンプレート展開エラー)

**種別**: 正常系
**目的**: 登録後の config 編集による展開失敗が spawn 失敗経路で凍結し、attempt が採番されない(runディレクトリに痕跡がない)ことを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `pulsen add --workflow wf-echo --repo $HOME/pulsen-test-repo` を実行し、IDを `TASK_I` に控える | タスクIDが表示され exit code 0 |
| 2 | `cp $PULSEN_HOME/config.yaml $PULSEN_HOME/config.yaml.bak` でバックアップし、config.yaml の `sh` の `cmd` を `["sh", "-c", "{input}", "{bogus}"]` に書き換える(未知プレースホルダを混入させる) | 編集完了(add 済みタスクには登録時検証が効かず、実行時の展開失敗となる) |
| 3 | `pulsen tick` を `TASK_I` が stopped になるまで繰り返す(4回程度。`pulsen ls --state stopped` で確認) | 各 tick が展開失敗を同期検出して spawn_fail_count を加算し、上限(3)超過で stopped になる。tick のサマリーに報告される |
| 4 | `pulsen show $TASK_I` を実行する | spawn_fail_count が上限を超過 → 「エージェント起動自体の失敗」と判別できる。直近の失敗要因に展開エラーの内容(未知プレースホルダ)が記録されている。attempt は「なし」(採番されない) |
| 5 | `ls $PULSEN_HOME/state/runs/$TASK_I` を実行する | ディレクトリが存在しない(同期検出の展開失敗では runディレクトリに痕跡がない。猶予時間超過の経路との判別点) |
| 6 | `cp $PULSEN_HOME/config.yaml.bak $PULSEN_HOME/config.yaml` で設定を復元する | 復元完了(設定を直してから retry で再開できる状態) |

**確認ポイント**:

- `/tmp/pulsen-notify.log` に `TASK_I` の通知行が追加されている

## TC-18: タスクファイルの直接閲覧

**種別**: 正常系
**目的**: タスクファイルが人間可読であり、閲覧だけなら tick と競合せず安全に行えることを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `cat $PULSEN_HOME/state/tasks/$TASK_A.json` を実行する(閲覧のみなのでスケジューラー停止は不要) | 人間可読な JSON が表示される |
| 2 | 内容を確認する | ワークフロー名・対象・タスクステータス・実行状態・各カウンタ・更新日時と、スナップショットされたワークフロー定義(`hold` の定義)が読み取れる |
| 3 | そのまま閉じる(編集しない) | 記録に問題がなければ閲覧だけで終わってよい |
| 4 | `pulsen show $TASK_A` を実行する | 変わらず表示でき exit code 0(閲覧は状態に影響しない) |

## TC-19: 存在しないタスクIDの詳細表示

**種別**: 異常系
**目的**: 不在のIDに対して無言で空を返さず、明確なエラーで終了することを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `pulsen show no-such-task-0000` を実行する | タスクが見つからないことを明確に示すエラーが表示される(空表示にならない) |
| 2 | `echo $?` を実行する | 非0 |

## TC-20: 不正な書式のタスクID

**種別**: 異常系
**目的**: ID の入力境界の検証エラーが非0で報告されることを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `pulsen show 'TASK_A!'` を実行する(`[a-z0-9-]` 以外の文字を含む) | 検証エラーとして非0で終了する |
| 2 | `pulsen show -- -abc` を実行する(先頭が `-`) | 検証エラーとして非0で終了する |

## TC-21: `--state` への不正値

**種別**: 異常系
**目的**: 固定6値以外の実行状態指定が有効値一覧付きで拒否されることを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `pulsen ls --state stoped` を実行する(typo) | 有効な値の一覧(pending / launching / running / completed / failed / stopped)を添えたエラーが表示される |
| 2 | `echo $?` を実行する | 非0 |

## TC-22: 破損タスクファイル混在時の一覧

**種別**: 異常系
**目的**: パース不能なタスクファイルが混在しても `ls` が一覧全体を失敗させず、修復の入口として報告することを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | バックアップを取る: `cp $PULSEN_HOME/state/tasks/$TASK_E.json $PULSEN_HOME/backup-taskE.json` | バックアップ作成(`state/tasks/` の外に置く) |
| 2 | JSON を破壊する: `printf '{ "broken":' > $PULSEN_HOME/state/tasks/$TASK_E.json` | パース不能なファイルになる |
| 3 | `pulsen ls` を実行する | `TASK_E` のファイルパスと読み取り不能である旨が報告され、残りのタスク(`TASK_A` / `TASK_B` / `TASK_C` / `TASK_F` / `TASK_G`)は通常どおり表示される |
| 4 | `echo $?` を実行する | `0`(一覧全体は失敗しない) |

## TC-23: 破損タスクファイルの詳細表示と直接修復

**種別**: 異常系
**目的**: 破損ファイルへの `show` がパースエラーとパスを示して失敗し、直接修復で通常の進行へ復帰できることを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | (TC-22 の破損状態のまま)`pulsen show $TASK_E` を実行する | パースエラーの内容と対象ファイルパス(`state/tasks/<TASK_E>.json`)が表示され、非0で終了する |
| 2 | tick の書き込みが起きないことを確認する(本手順書ではスケジューラー未登録のため追加操作は不要。cron 登録済み環境では一時停止する) | 手動編集がロック外の書き込みと競合しない状態になる |
| 3 | 修復する: `cp $PULSEN_HOME/backup-taskE.json $PULSEN_HOME/state/tasks/$TASK_E.json`(エディタでの修正に相当) | ファイルが正しい JSON に戻る |
| 4 | `pulsen show $TASK_E` を実行する | 詳細が表示され exit code 0(以降は次の tick に委ねれば通常進行に復帰する) |
| 5 | `pulsen ls` を実行する | 読み取り不能の報告が消え、`TASK_E` が通常の行として表示される |

## TC-24: スナップショット部分のみ破損したタスク

**種別**: 異常系
**目的**: タスクファイル自体は読めるがスナップショットが読めない縮退状態で、ls / show が読める範囲の表示を継続することを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | バックアップを取る: `cp $PULSEN_HOME/state/tasks/$TASK_F.json $PULSEN_HOME/backup-taskF.json` | バックアップ作成 |
| 2 | エディタで `state/tasks/$TASK_F.json` を開き、埋め込まれたワークフロー定義(スナップショット)部分の値だけを不正な構造に置き換えて保存する(例: スナップショットのフィールドの値を `null` にする。フィールド名は TC-18 で確認した実ファイルの構造に合わせる)。ファイル全体は JSON として妥当なまま保つ | タスク属性は読めるがスナップショットが読めない状態になる |
| 3 | `pulsen ls` を実行する | `TASK_F` は行として表示され、スナップショット読み取り不能の印が付く。exit code 0 |
| 4 | `pulsen ls --state pending` を実行する | 実行状態は読めているため、`TASK_F` は絞り込みの対象になり表示される |
| 5 | `pulsen show $TASK_F` を実行する | タスクファイル由来の項目(ステータス・実行状態・カウンタ等)は表示され、スナップショットが読めない理由が注記される。定義済みステータス一覧は表示されない。exit code 0 |
| 6 | リトライ上限の併記を確認する | 「不明」(スナップショット破損で導出不能)として表示され、TC-08 の wait ステータスの「併記なし」と区別できる。judge / spawn の上限は config 由来のため通常どおり表示される |
| 7 | 修復する: `cp $PULSEN_HOME/backup-taskF.json $PULSEN_HOME/state/tasks/$TASK_F.json` を実行し、`pulsen show $TASK_F` で確認する | 定義済みステータス一覧を含む通常表示に戻り exit code 0 |

## TC-25: runディレクトリ不在(gc 後相当)の詳細表示

**種別**: 異常系
**目的**: 参照先の runディレクトリが存在しなくても show がエラーにせず「存在しない」と明示することを検証する

TC-11 の後に実行する(`TASK_D` のログ確認を先に済ませる)。

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `rm -rf $PULSEN_HOME/state/runs/$TASK_D` を実行する(アーカイブ済みタスクの runディレクトリは gc の保護対象外。gc による削除を再現する) | ディレクトリが消える |
| 2 | `pulsen show $TASK_D` を実行する | attempt の runディレクトリが「存在しない」ことを明示して詳細が表示される |
| 3 | `echo $?` を実行する | `0`(エラーにしない) |

## TC-26: exit ファイルが読めない場合の詳細表示

**種別**: 異常系
**目的**: 実行メタデータの一部が読めなくても show が注記付きで表示を継続することを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | バックアップを取る: `cp $PULSEN_HOME/state/runs/$TASK_C/attempt-3/exit /tmp/pulsen-exit.bak` | バックアップ作成 |
| 2 | 内容を不正にする: `printf 'abc' > $PULSEN_HOME/state/runs/$TASK_C/attempt-3/exit` | 数値として読めない exit ファイルになる |
| 3 | `pulsen show $TASK_C` を実行する | exit の項目が読めない旨の注記付きで、他の項目の表示は継続される。exit code 0 |
| 4 | 復元する: `cp /tmp/pulsen-exit.bak $PULSEN_HOME/state/runs/$TASK_C/attempt-3/exit` | 復元完了 |
| 5 | `chmod 000 $PULSEN_HOME/state/runs/$TASK_C/attempt-3` を実行し、`pulsen show $TASK_C` を実行する | runディレクトリ側の確認が失敗しても、当該項目を読めない旨の注記付きで表示は継続され exit code 0 |
| 6 | `chmod 755 $PULSEN_HOME/state/runs/$TASK_C/attempt-3` で復元し、`pulsen show $TASK_C` で通常表示を確認する | exit の値(1)を含む通常表示に戻る |

## TC-27: config.yaml 不在・パース不能

**種別**: 異常系
**目的**: グローバル設定が読めないとき、ls / show が状態を変更せず非0で終了することを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `mv $PULSEN_HOME/config.yaml $PULSEN_HOME/config.yaml.bak` を実行する | config.yaml が不在になる |
| 2 | `pulsen ls` を実行する | グローバルホームが未初期化である旨・解決後のホームパス(`$HOME/pulsen-manual-test`)・作成が必要であることが表示され、非0で終了する |
| 3 | `pulsen show $TASK_A` を実行する | 同様に非0で終了する |
| 4 | `printf 'agents: [\n' > $PULSEN_HOME/config.yaml` を実行する(パース不能な YAML) | 作成完了 |
| 5 | `pulsen ls` を実行する | パースエラーの位置を示して非0で終了する |
| 6 | `mv $PULSEN_HOME/config.yaml.bak $PULSEN_HOME/config.yaml` で復元し、`pulsen ls` を実行する | 通常の一覧に戻り exit code 0 |

## TC-28: 状態ディレクトリの走査不能(権限エラー)

**種別**: 異常系
**目的**: 走査自体ができない I/O エラーが実行環境エラーとして非0で報告されることを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `chmod 000 $PULSEN_HOME/state/tasks` を実行する | 読み取り権限が失われる |
| 2 | `pulsen ls` を実行する | 走査の失敗が実行環境エラーとして表示され、非0で終了する |
| 3 | `chmod 755 $PULSEN_HOME/state/tasks` で復元する | 復元完了 |
| 4 | `chmod 000 $PULSEN_HOME/state/archive` を実行し、`pulsen ls --all` を実行する | アーカイブ側の走査失敗として非0で終了する(`--all` なしの `pulsen ls` は archive に依存せず exit code 0) |
| 5 | `chmod 755 $PULSEN_HOME/state/archive` で復元し、`pulsen ls --all` で通常表示を確認する | 通常の一覧に戻り exit code 0 |

## TC-29: worktree が手動削除されたタスクの詳細表示

**種別**: 異常系
**目的**: show が workspace_path の存在検証を行わず、パス表示のみで正常終了することを検証する

TC-14 の後に実行する(worktree 残存の確認を先に済ませる)。

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `pulsen show $TASK_C` で workspace_path を控え、`rm -rf $PULSEN_HOME/worktrees/$TASK_C` を実行する | worktree が消える |
| 2 | `pulsen show $TASK_C` を実行する | workspace_path は記録どおりそのまま表示される(存在検証は行われない) |
| 3 | `echo $?` を実行する | `0` |

## TC-30: タスクが0件の一覧

**種別**: 境界値
**目的**: タスク0件・`state/` ディレクトリ不在でも空の一覧として正常終了することを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | 空のホームを作る: `mkdir -p $HOME/pulsen-manual-test-empty && printf 'agents: {}\n' > $HOME/pulsen-manual-test-empty/config.yaml` | 作成完了(config のみ。`state/` は存在しない) |
| 2 | `PULSEN_HOME=$HOME/pulsen-manual-test-empty pulsen ls` を実行する | 空である旨が表示され exit code 0(`state/` 不在でもエラーにしない) |
| 3 | `PULSEN_HOME=$HOME/pulsen-manual-test-empty pulsen ls --all` を実行する | 同様に空・exit code 0(`state/archive/` 不在も空として扱う) |
| 4 | `PULSEN_HOME=$HOME/pulsen-manual-test-empty pulsen show $TASK_A` を実行する | タスク不在として非0で終了する(一覧の空と単一照会の不在は扱いが異なる) |

## TC-31: 存在しないタスクステータス名での絞り込み

**種別**: 境界値
**目的**: ユーザー定義語彙のため値は検証されず、該当0件が正常終了になることを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `pulsen ls --status no-such-status` を実行する | エラーにならず、空である旨(該当0件)が表示される(typo かどうかの判断は利用者に委ねられる) |
| 2 | `echo $?` を実行する | `0` |

## TC-32: タスクIDの長さ境界

**種別**: 境界値
**目的**: ID の長さ・空文字の境界で検証エラーとタスク不在エラーが区別されることを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `pulsen show $(printf 'a%.0s' $(seq 1 65))` を実行する(65文字) | 長すぎる旨の検証エラーとして非0で終了する |
| 2 | `pulsen show $(printf 'a%.0s' $(seq 1 64))` を実行する(64文字。書式は有効) | 書式としては受理され、タスク不在のエラーとして非0で終了する(手順1とはエラーの種類が異なる) |
| 3 | `pulsen show ""` を実行する(空文字) | 検証エラーとして非0で終了する |

## TC-33: `--state` の表記揺れ・空文字

**種別**: 境界値
**目的**: 実行状態の値が小文字6値のみ受理されることを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `pulsen ls --state Pending` を実行する(大文字混じり) | 有効値一覧を添えて非0で終了する |
| 2 | `pulsen ls --state ""` を実行する(空文字) | 有効値一覧を添えて非0で終了する |
| 3 | `pulsen ls --state pending` / `--state launching` / `--state running` / `--state completed` / `--state failed` / `--state stopped` を順に実行する | 6値すべて受理され、それぞれ該当タスクのみ(該当なしなら空)を表示して exit code 0 |

## TC-34: クリーンアップステータスのないタスクの手動アーカイブ移動(最終処分)

**種別**: 異常系
**目的**: クリーンアップステータスを持たない定義で登録したタスク(set-status で終端処理に乗せられない)を、タスクファイルの手動 `state/archive/` 移動で tick の走査対象から外せること、移動後は通常のアーカイブ済みタスクとして参照できることを検証する(setup.md シナリオ4 異常系の最終処分手順)

**前提**: `TASK_G` が stopped(wf-wait には `run: cleanup` のステータスがない)。外部スケジューラー(cron 等)が動いていないこと(手動移動は tick と同一ロックの外での操作のため、スケジューラーが動いていないタイミングで行う)

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `pulsen set-status $TASK_G done` を実行し、`echo $?` を確認する | スナップショットに `done` が存在しないため拒否され、exit code 非0(このタスクはツールの終端処理に乗せられない = 手動移動が最終処分の手段になる) |
| 2 | `pulsen show $TASK_G` で workspace を確認する | workspace は「未作成」(一度も実行されていない)。worktree 作成済みのタスクを片付ける場合は、先に `git worktree remove <workspace_path>` を対象リポジトリで実行して手動で削除する |
| 3 | `mv $PULSEN_HOME/state/tasks/$TASK_G.json $PULSEN_HOME/state/archive/$TASK_G.json` を実行する | タスクファイルが `state/archive/` へ移動する |
| 4 | `pulsen ls` と `pulsen tick` を実行する | `ls` に `TASK_G` は表示されず、tick のサマリーにも `TASK_G` に関する処理・報告が現れない(走査対象から外れた) |
| 5 | `pulsen ls --all` を実行する | `TASK_G` がアーカイブ済みの印付きで表示される |
| 6 | `pulsen show $TASK_G; echo $?` を実行する | アーカイブ済みの注記付きで詳細が表示され、exit code 0(移動後は通常のアーカイブ済みタスクと同じ扱いになる) |

## 後片付け

1. `pulsen abort $TASK_B`(実行中の sleep を停止する)
2. `rm -rf $PULSEN_HOME $HOME/pulsen-manual-test-empty $HOME/pulsen-test-repo /tmp/pulsen-notify.log /tmp/pulsen-exit.bak`

## カバレッジ

### ユースケースエラーケース対応表

| ユースケース | エラーケース | 対応TC | 備考 |
|---|---|---|---|
| ListTasks | `--state` が固定6値以外 | TC-21, TC-33 | typo・大文字・空文字 |
| ListTasks | `list_active` / `list_archived` の Io(走査不能) | TC-28 | 権限剥奪で再現 |
| ListTasks | config 読み込み失敗(※1) | TC-27 | 不在・パース不能の両方 |
| ShowTask | タスク不在 | TC-19, TC-30 | 無言で空を返さない |
| ShowTask | タスクファイル破損(`Corrupt`) | TC-23 | パースエラー内容とパスを表示。修復導線まで確認 |
| ShowTask | `attempt_exists` / `read_exit` の Io・`RunFileError` | TC-26 | 注記付きで表示継続(exit 0) |
| ShowTask | config 読み込み失敗(※1) | TC-27 | |
| ShowTask | タスクIDの入力境界(`TaskId::parse`) | TC-20, TC-32 | 不正文字・先頭 `-`・空・長さ超過 |

### 観点チェックリスト

| 観点 | 対応TC | 対象外の理由 |
|---|---|---|
| 入力バリデーション | TC-20, TC-21, TC-32, TC-33 | |
| 境界値 | TC-30, TC-31, TC-32, TC-33 | |
| 認証・権限 | TC-28 | 認証機構はない。CLI ではファイルパーミッションに読み替えて検証 |
| 空状態・初期状態 | TC-08, TC-30 | 空のログは TC-11 手順3 で確認(異常ではない) |
| 重複・競合 | 対象外 | ls / show はロックを取得せず、tick との同時実行はアトミック置換で安全とされる。読み取りと書き込みのタイミング競合は手動で決定的に再現できないため自動テスト(spec/testcases/task/)に委ねる(ロック競合自体の決定的再現は task-execution.md TC-24 / intervention.md TC-25 で確認する) |
| 削除・変更の影響 | TC-10, TC-25, TC-29, TC-34 | アーカイブ後の worktree 削除済み表示・gc 後の runディレクトリ不在・worktree 手動削除・タスクファイルの手動アーカイブ移動 |
| 操作の中断・逸脱 | TC-18, TC-23 | 閲覧のみで終える・スケジューラー停止を確保してから修復する |
| 特殊入力 | TC-20, TC-32 | 記号・先頭ハイフン・空文字・長大なID |
| UIの状態(エラー後のリトライ) | TC-23, TC-24, TC-26, TC-27 | 破損の報告 → 修復・復元 → 再実行で通常表示へ復帰することまで確認 |
