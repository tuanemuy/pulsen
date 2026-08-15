# セットアップとワークフロー定義 テスト

## 概要

このドキュメントは、pulsen の初期設定(グローバル設定の作成)・ワークフローYAMLの定義・外部スケジューラーへの tick 登録・ファイルパス指定での試行・判定コマンドの定義に関するマニュアルテストの手順書です。

pulsen は CLI ツールであり、「画面」はコマンドとその出力に対応します。操作はすべて具体的なコマンドラインとファイル内容で記述します。

対応シナリオ: `spec/scenario/setup.md`(シナリオ1〜5)

## 前提条件

### 環境

- POSIX 環境(macOS または Linux)。ファイル権限のテスト(TC-15)とシグナル死のテスト(TC-47)は POSIX を前提とする
- `pulsen` バイナリがビルド済みで PATH が通っている(`pulsen --help` が表示できる)
- `git` がインストール済み
- TC-06 のみ cron が利用できること(macOS では cron にフルディスクアクセスの許可が必要な場合がある)
- 実エージェントCLI(Claude Code 等)は不要。テストは決定的に動作させるため `sh` をエージェントとして登録して行う

### テストデータ

- テスト専用グローバルホーム: `$HOME/pulsen-manual-test`(実運用の `~/.pulsen/` を汚染しないため、環境変数 `PULSEN_HOME` で切り替える)
- テスト対象リポジトリ: `$HOME/pulsen-test-repo`(ブランチ `main` にコミットが1つある git リポジトリ)
- 作業ディレクトリ: `$HOME/pulsen-manual-work`(ドラフトYAML・壊れたYAMLの置き場)
- 判定スクリプト: `$HOME/pulsen-manual-test/judge.sh`(制御ファイル `judge-exit` の内容を exit code として返す)

### 事前準備

1. テストセッションのシェルで環境変数を設定する。

    ```sh
    export PULSEN_HOME="$HOME/pulsen-manual-test"
    export REPO="$HOME/pulsen-test-repo"
    export WORK="$HOME/pulsen-manual-work"
    ```

2. 分離ホームとテスト領域を初期化し(既存なら削除して作り直す。過去の実行や他ドキュメントの残留状態を持ち込まないため)、テスト対象リポジトリを作成する(グローバル git 設定のない環境でも初期コミットが成功するよう、identity をリポジトリローカルに設定する)。

    ```sh
    rm -rf "$PULSEN_HOME" "$WORK" "$REPO" "$HOME/pulsen-empty-home" "$HOME/pulsen-default-home"
    mkdir -p "$PULSEN_HOME" "$WORK"
    git init -b main "$REPO"
    git -C "$REPO" config user.name pulsen-test
    git -C "$REPO" config user.email pulsen-test@example.com
    git -C "$REPO" commit --allow-empty -m "init"
    ```

3. 判定スクリプトを作成する(TC-09〜11・37 で使用)。

    ```sh
    cat > "$PULSEN_HOME/judge.sh" <<'EOF'
    #!/bin/sh
    echo "judged: task=$TASK_ID exit=$EXIT_CODE" >> "$HOME/pulsen-manual-test/judge.log"
    exit "$(cat "$HOME/pulsen-manual-test/judge-exit")"
    EOF
    chmod +x "$PULSEN_HOME/judge.sh"
    ```

4. TC-01 で config.yaml を作成するまでは、`$PULSEN_HOME` に config.yaml を置かない(TC-12 が未初期化状態を使うため、TC-12 は独立したホームで実行できるようにしてある)。

各テストケースの操作は、明記がない限り上記の環境変数が設定されたシェルの任意のカレントディレクトリから開始する。TC-03 以降は TC-01 の config.yaml 作成済みを前提とする。exit code は直後に `echo $?` で確認する。

tick は1タスクにつき1回で1ステップだけ進める。1回のエージェント実行の消化には「起動 → spawn確認 → 判定 → 遷移」の約4回の tick を要する(`task-execution.md` と同じ刻み)。判定が観測されるのは起動の tick から数えて3回目であり、起動直後の tick は spawn確認(running への取り込み)しか行わない。連続する tick の間は 2〜3 秒あける(ラッパーの pid ファイル書き込みを待つため)。

## 正常系

## TC-01: グローバル設定を作成し読み込みを確認する

**種別**: 正常系
**目的**: シナリオ1のフロー全体。エージェント定義(文字列形式・配列形式)・通知コマンド・各種デフォルトを config.yaml に定義し、任意のコマンドで設定が読み込めることを確認する。

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `$PULSEN_HOME/config.yaml` を以下の内容で作成する。<br><br><pre>agents:<br>  shell:<br>    cmd: ["sh", "-c", "{input}"]<br>    skill_input: "{skill}"<br>  claude:<br>    cmd: claude -p {input} --model {model}<br>    skill_input: "/skill {skill}"<br>notify_cmd: ["sh", "-c", "echo \"$TASK_ID $WORKFLOW $TASK_STATUS\" >> \"$HOME/pulsen-manual-test/notify.log\""]<br>judge_attempt_limit: 3<br>judge_timeout: 60s<br>spawn_fail_limit: 3</pre> | ファイルが作成される |
| 2 | `pulsen ls` を実行する | 設定が読み込まれ、タスクが空である旨が表示されて exit code 0 |

**確認ポイント**:
- エラー(パースエラー・未初期化の案内)が表示されないこと
- `shell` は配列形式(シェル機能が必要な場合の書き方)、`claude` は文字列形式の定義例になっている

## TC-02: タスク0件で手動 tick を実行する

**種別**: 正常系
**目的**: シナリオ3のフロー1。登録タスクが無い状態で tick が何もせず正常終了することを確認する(定期実行登録前の動作確認手順)。

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | TC-01 完了直後(タスク未登録)の状態で `pulsen tick` を実行する | 処理対象がない旨が表示されて exit code 0。`$PULSEN_HOME/state/` 配下のタスクは作られない |

## TC-03: ワークフローYAMLを作成し名前指定でタスクを登録する

**種別**: 正常系
**目的**: シナリオ2のフロー。ワークフローYAMLを `workflows/` に配置し、`add` の名前指定で登録が受理される(= 定義の受理確認になる)ことを確認する。

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `$PULSEN_HOME/workflows/implement.yaml` を以下の内容で作成する。<br><br><pre>workflow: implement<br>agent: shell<br>initial: queued<br>statuses:<br>  queued:<br>    prompt: "echo planned > plan.txt"<br>    next: implemented<br>  implemented:<br>    prompt: "echo implemented >> plan.txt"<br>    timeout: 30m<br>    next: review_waiting<br>  review_waiting:<br>    run: wait<br>  done:<br>    run: cleanup</pre> | ファイルが作成される |
| 2 | `pulsen add --workflow implement --repo "$REPO"` を実行する | タスクIDと、解決したワークフロー名 `implement`・解決先パス(`$PULSEN_HOME/workflows/implement.yaml` の絶対パス)が表示されて exit code 0 |
| 3 | `pulsen ls` を実行する | 登録したタスクが1行表示される。ワークフロー名 `implement`、タスクステータス `queued`(initial)、実行状態 `pending` |
| 4 | 表示されたタスクIDで `pulsen show <task-id>` を実行する | タスクステータス `queued`・実行状態 `pending`・attempt_count 0・workspace「未作成」・attempt「なし」が表示され、定義済みステータス一覧に `queued` / `implemented` / `review_waiting` / `done` が含まれる。exit code 0 |

**確認ポイント**:
- `--base` を省略したため、ベースブランチがリポジトリの HEAD(`main`)に解決されて表示されること
- 登録だけではエージェント実行が始まらないこと(実行状態は `pending` のまま)

## TC-04: 同一指定の二重登録が独立したタスクとして許容される

**種別**: 正常系
**目的**: 重複排除が行われないこと(観点: 重複・競合)。同一ワークフロー・同一対象のタスクが独立して複数登録できることを確認する。

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | TC-03 完了状態で、同じコマンド `pulsen add --workflow implement --repo "$REPO"` をもう一度実行する | TC-03 とは異なる新しいタスクIDが表示されて exit code 0 |
| 2 | `pulsen ls` を実行する | 2つのタスクが別IDで表示され、どちらも `queued` / `pending` |

## TC-05: 確認用タスクを set-status でクリーンアップして片付ける

**種別**: 正常系
**目的**: シナリオ2のフロー6。定義の受理確認に使ったタスクを `set-status` でクリーンアップステータスへ遷移させ、次の tick が終端処理(アーカイブ)を行うことを確認する。

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | TC-04 で作成した2つのタスクそれぞれに `pulsen set-status <task-id> done` を実行する | 遷移した旨(`queued` → `done`)が表示されて exit code 0 |
| 2 | `pulsen tick` を実行する | サマリーに終端処理(アーカイブ)されたタスクとして2件が表示されて exit code 0 |
| 3 | `pulsen ls` を実行する | タスクが空である旨が表示される(現役タスク0件) |
| 4 | `pulsen ls --all` を実行する | 2つのタスクがアーカイブ済みの印付きで表示される |
| 5 | `pulsen show <task-id>` をアーカイブ済みIDで実行する | 詳細が表示され、アーカイブ済みであることが注記され、workspace は「未作成」と表示される(一度も実行されておらず worktree が作られていないため。cleanup.md TC-09 と同じ表示)。exit code 0 |

**確認ポイント**:
- worktree 未作成のまま片付けたため、`$PULSEN_HOME/worktrees/` に残骸がないこと

## TC-06: 外部スケジューラー(cron)に tick を登録して自動進行を確認する

**種別**: 正常系
**目的**: シナリオ3のフロー2〜5。cron に tick を登録し、登録済みタスクが人手なしでステータスを進むことを確認する。

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `which pulsen` でバイナリの絶対パスを確認する | 絶対パスが表示される |
| 2 | `crontab -e` で以下の行を登録する(`<pulsen>` は手順1の絶対パス、`<home>` は `$HOME/pulsen-manual-test` の絶対パス)。<br><br><pre>* * * * * <pulsen> tick --home <home> >> <home>/cron.log 2>&1</pre> | crontab が保存される |
| 3 | `pulsen add --workflow implement --repo "$REPO"` でテスト用タスクを登録する | タスクIDが表示されて exit code 0 |
| 4 | 10〜15分待ち、`pulsen ls` を実行する | タスクが `queued` → `implemented` → `review_waiting` と進行し、`review_waiting` / `pending` で滞留している(wait ステータスは手動遷移待ち)。各エージェント実行ステータスの消化には約4 tick(起動 → spawn確認 → 判定 → 遷移)を要するため、2ステータスの消化には約8回の tick = 1分間隔で約8分かかる |
| 5 | `pulsen show <task-id>` を実行する | workspace_path(`$PULSEN_HOME/worktrees/<task-id>`)とブランチ(`pulsen/<task-id>`)が確定しており、attempt の runディレクトリ・exit(0)が表示される |
| 6 | worktree 内の成果物を確認する: `cat "$PULSEN_HOME/worktrees/<task-id>/plan.txt"` | `planned` と `implemented` の2行が出力されている |
| 7 | crontab から手順2の行を削除する | 以後 tick は自動実行されない |
| 8 | `pulsen set-status <task-id> done` の後、手動で `pulsen tick` を実行する | スケジューラー停止後も状態は壊れておらず、手動 tick で終端処理(アーカイブ)が行われて exit code 0 |

**確認ポイント**:
- cron 環境はインタラクティブシェルと環境変数が異なるため、バイナリの絶対パスと `--home` フラグを必ずコマンドラインに含めていること(フロー4)
- `cron.log` に tick のサマリーが定期的に追記されていること

## TC-07: ワークフローをファイルパス指定で登録して試行する

**種別**: 正常系
**目的**: シナリオ4のフロー1〜4。`workflows/` に配置する前のドラフトYAMLをファイルパス指定で登録し、手動 tick とログで挙動を確認する。

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `$WORK/draft.yaml` を以下の内容で作成する。<br><br><pre>workflow: my-flow<br>agent: shell<br>initial: greet<br>statuses:<br>  greet:<br>    prompt: "echo hello from draft"<br>    next: waiting<br>  waiting:<br>    run: wait<br>  done:<br>    run: cleanup</pre> | ファイルが作成される |
| 2 | `cd "$WORK"` して `pulsen add --workflow ./draft.yaml --repo "$REPO"` を実行する | タスクIDが表示されて exit code 0。タスクのワークフロー名は YAML の `workflow:` キーの値 `my-flow` になる |
| 3 | `pulsen tick` を実行する | サマリーに起動したタスクとして表示されて exit code 0(このタスク以外に実行可能なタスクがあればそれも処理される) |
| 4 | 数秒待って `pulsen tick` を 2〜3 秒間隔で2回実行し(spawn確認 → 判定)、`pulsen show <task-id>` を実行する | 実行が観測・判定され、タスクが `waiting` へ向けて進行する。show に最新 attempt の runディレクトリと `stdout.log` / `stderr.log` のパスが表示される |
| 5 | `cat "$PULSEN_HOME/state/runs/<task-id>/attempt-1/stdout.log"` を実行する | `hello from draft` が記録されている |

## TC-08: スナップショットにより元ファイルの編集・削除が既存タスクに影響しない

**種別**: 正常系
**目的**: シナリオ2・4の異常系(登録済みタスクへの影響 / 元ファイルの削除・移動)。定義は登録時にスナップショットされ、以降元ファイルに依存しないことを確認する。

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | TC-07 の手順1〜2 と同様に `$WORK/draft.yaml` から新しいタスクを登録する(tick はまだ実行しない) | タスクIDが表示されて exit code 0 |
| 2 | 元ファイルを削除する: `rm "$WORK/draft.yaml"` | ファイルが消える |
| 3 | `pulsen tick` を実行し(起動)、2〜3 秒間隔でさらに2回 `pulsen tick` を実行する(spawn確認 → 判定) | タスクは元ファイルなしで正常に起動・判定され、進行する(スナップショットのみを参照) |
| 4 | `pulsen show <task-id>` を実行する | 定義済みステータス一覧(`greet` / `waiting` / `done`)がスナップショットから表示され、スナップショットの保存先パスが表示される |
| 5 | 片付け: `pulsen set-status <task-id> done` → `pulsen tick`。TC-07 のタスクも同様に片付ける | 終端処理されてアーカイブされる |

**確認ポイント**:
- 「このタスクが何の定義で動いたか」が show の表示するスナップショットから事後に確認できること

## TC-09: 判定コマンドが exit 0 を返すと completed として次ステータスへ遷移する

**種別**: 正常系
**目的**: シナリオ5のフロー1〜4。`judge` キーで定義した判定コマンドの exit 0 が completed と解釈されることを確認する。

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `$PULSEN_HOME/workflows/judge-demo.yaml` を以下の内容で作成する。<br><br><pre>workflow: judge-demo<br>agent: shell<br>initial: checking<br>statuses:<br>  checking:<br>    prompt: "echo checking"<br>    judge: ["sh", "-c", "\"$HOME/pulsen-manual-test/judge.sh\""]<br>    next: finished<br>  finished:<br>    run: wait<br>  done:<br>    run: cleanup</pre> | ファイルが作成される |
| 2 | 判定結果を completed に設定する: `echo 0 > "$PULSEN_HOME/judge-exit"` | 制御ファイルが作られる |
| 3 | `pulsen add --workflow judge-demo --repo "$REPO"` → `pulsen tick` を実行する | タスクが登録され、tick で起動される |
| 4 | 数秒待って `pulsen tick` を 2〜3 秒間隔で2回実行し(spawn確認 → 判定)、`pulsen show <task-id>` で確認する | 判定コマンドが実行され(`$PULSEN_HOME/judge.log` に `exit=0` の行が追記される)、実行状態が `completed` になる |
| 5 | `pulsen tick` をもう一度実行し、`pulsen ls` で確認する | タスクステータスが `finished` へ遷移し、実行状態 `pending` で滞留する(wait) |

## TC-10: 判定コマンドが exit 10 を返すと failed としてリトライされる

**種別**: 正常系
**目的**: シナリオ5のフロー4。判定コマンドの exit 10 が failed と解釈され、エージェントが再実行されることを確認する。

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `echo 10 > "$PULSEN_HOME/judge-exit"` を実行し、`pulsen add --workflow judge-demo --repo "$REPO"` → `pulsen tick` で新しいタスクを起動する | タスクが起動される |
| 2 | 数秒待って `pulsen tick` を 2〜3 秒間隔で2回実行し(spawn確認 → 判定)、`pulsen show <task-id>` で確認する | 実行状態が `failed` になり、attempt_count が 1 になる(リトライ上限が併記される)。タスクステータスは `checking` のまま |
| 3 | `pulsen tick` を実行する | failed のタスクが新しい attempt(attempt-2)で再実行される |
| 4 | 回復: `echo 0 > "$PULSEN_HOME/judge-exit"` にしてから、数秒待って `pulsen tick` を 2〜3 秒間隔で2回実行する(spawn確認 → 判定) | 判定が completed になり、attempt_count がリセットされる(次の tick が `finished` へ遷移させる) |
| 5 | 片付け: `pulsen set-status <task-id> done` → `pulsen tick` | アーカイブされる |

## TC-11: 判定コマンドが exit 20 を返すと skipped として同じステータスのまま再実行される

**種別**: 正常系
**目的**: シナリオ5のフロー2・4。skipped(現状維持)がタスクステータスを変えず pending へ戻し、次の tick で再実行されること(ポーリング型)を確認する。

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `echo 20 > "$PULSEN_HOME/judge-exit"` を実行し、`pulsen add --workflow judge-demo --repo "$REPO"` → `pulsen tick` で新しいタスクを起動する | タスクが起動される |
| 2 | 数秒待って `pulsen tick` を 2〜3 秒間隔で2回実行する(spawn確認 → 判定) | サマリーに「skipped で実行待ちに戻したタスク」として表示される |
| 3 | `pulsen show <task-id>` で確認する | タスクステータスは `checking` のまま、実行状態は `pending`、attempt_count は 0(リセット)。通知は行われない(`notify.log` に追記なし) |
| 4 | `pulsen tick` を実行する | 同じタスクステータスが新しい attempt で再実行される(attempt 番号が増える) |
| 5 | 回復と片付け: `echo 0 > "$PULSEN_HOME/judge-exit"` → 数秒待って `pulsen tick` を 2〜3 秒間隔で2回(spawn確認 → 判定で completed)→ `pulsen set-status <task-id> done` → `pulsen tick` | 次ステータスへ進み、アーカイブされる |

## 異常系

## TC-12: config.yaml が存在しない(未初期化ホーム)

**種別**: 異常系
**目的**: シナリオ1の異常系。グローバルホーム未初期化のエラーが、解決後のホームパスと作成の案内を含むことを確認する(観点: 空状態・初期状態)。

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `pulsen ls --home "$HOME/pulsen-empty-home"` を実行する(存在しないホーム) | グローバルホームが未初期化である旨・解決後のホームパス(`$HOME/pulsen-empty-home`)・config.yaml の作成が必要であることが表示されて exit code 非0 |
| 2 | `pulsen add --workflow implement --repo "$REPO" --home "$HOME/pulsen-empty-home"` を実行する | 同様のエラーで exit code 非0。タスクは作られない |
| 3 | `pulsen tick --home "$HOME/pulsen-empty-home"` を実行する | 同様のエラーで exit code 非0(スケジューラー環境で「設定がない」となった場合はこの表示のホームパスで誤解決に気づける) |
| 4 | 回復: `mkdir -p "$HOME/pulsen-empty-home"` して `config.yaml`(TC-01 と同内容)を置き、`pulsen ls --home "$HOME/pulsen-empty-home"` を再実行する | 空一覧が表示されて exit code 0 |

## TC-13: config.yaml が不正なYAML

**種別**: 異常系
**目的**: シナリオ1の異常系。パースエラーの位置とともに読み込みが失敗し、壊れた設定で部分的に動作しないことを確認する。

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `$PULSEN_HOME/config.yaml` を退避する: `cp "$PULSEN_HOME/config.yaml" "$WORK/config.bak"` | バックアップが作られる |
| 2 | config.yaml の任意の行のインデントを崩して保存する(例: `agents:` の次行の先頭スペースを削除) | 構文が壊れる |
| 3 | `pulsen ls` を実行する | パースエラーの位置(行等)が表示されて exit code 非0 |
| 4 | `pulsen add --workflow implement --repo "$REPO"` を実行する | 同様に非0。タスクは作られない(状態は変更されない) |
| 5 | 回復: `cp "$WORK/config.bak" "$PULSEN_HOME/config.yaml"` で戻し、`pulsen ls` を再実行する | exit code 0 に復帰する |

## TC-14: config.yaml にスキーマ外のキーがある

**種別**: 異常系
**目的**: config の未知キー(typo)が構造エラーとして拒否され、typo の黙殺で設定が効かないまま動く事故を防ぐことを確認する。

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `$PULSEN_HOME/config.yaml` の末尾に `run_retension: 30d`(typo)を追記する | ファイルが更新される |
| 2 | `pulsen ls` を実行する | 未知キーのエラーが表示されて exit code 非0 |
| 3 | 回復: 追記した行を削除して `pulsen ls` を再実行する | exit code 0 に復帰する |

## TC-15: ファイル権限で設定・定義が読めない

**種別**: 異常系
**目的**: 読み取り権限のないファイルが実行環境エラーとして扱われることを確認する(観点: 認証・権限。ログイン概念のない CLI ではファイル権限が権限系の境界になる)。

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `chmod 000 "$PULSEN_HOME/config.yaml"` を実行してから `pulsen ls` を実行する | 読み取りエラー(実行環境エラー)が表示されて exit code 非0 |
| 2 | `chmod 644 "$PULSEN_HOME/config.yaml"` で戻す | — |
| 3 | `chmod 000 "$PULSEN_HOME/workflows/implement.yaml"` を実行してから `pulsen add --workflow implement --repo "$REPO"` を実行する | ワークフロー定義の読み取り失敗として exit code 非0。タスクは作られない |
| 4 | 回復: `chmod 644 "$PULSEN_HOME/workflows/implement.yaml"` で戻し、手順3のコマンドを再実行する | 登録が成功して exit code 0(登録したタスクは `pulsen set-status <task-id> done` → `pulsen tick` で片付ける) |

## TC-16: エージェントテンプレートに未知プレースホルダが含まれる

**種別**: 異常系
**目的**: シナリオ1の異常系。ワークフローが参照するエージェント定義の不備(未知プレースホルダ)が add の登録時検証で弾かれ、タスクが作られないことを確認する。

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `$PULSEN_HOME/config.yaml` の `shell` の定義を `cmd: ["sh", "-c", "{inptu}"]`(typo)に書き換える | ファイルが更新される |
| 2 | `pulsen add --workflow implement --repo "$REPO"` を実行する | 未知プレースホルダによるテンプレート不備のエラーが表示されて exit code 非0。`pulsen ls` にタスクは現れない |
| 3 | 回復: `cmd: ["sh", "-c", "{input}"]` に戻して手順2のコマンドを再実行する | 登録が成功して exit code 0(タスクは TC-05 の手順で片付ける) |

## TC-17: skill 指定のステータスがあるのに skill_input が未定義

**種別**: 異常系
**目的**: シナリオ1の異常系。スキル→入力文字列の変換ができないテンプレート不備が登録時検証で弾かれることを確認する。

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `$WORK/skill-flow.yaml` を以下の内容で作成する。<br><br><pre>agent: shell<br>initial: start<br>statuses:<br>  start:<br>    skill: plan<br>    next: waiting<br>  waiting:<br>    run: wait</pre> | ファイルが作成される |
| 2 | `$PULSEN_HOME/config.yaml` の `shell` から `skill_input:` の行を削除する | ファイルが更新される |
| 3 | `pulsen add --workflow "$WORK/skill-flow.yaml" --repo "$REPO"` を実行する | `skill_input` 欠落のエラーが表示されて exit code 非0。タスクは作られない |
| 4 | 回復: `skill_input: "{skill}"` を戻して手順3のコマンドを再実行し、直後に `pulsen abort <task-id>` で止める | 登録が成功して exit code 0。登録したタスクは `start`(skill 実行)の pending であり、放置すると以後のTCの tick で実行されて失敗を重ね、凍結・通知のノイズになるため直後に abort する(stopped として残留する。無害) |

## TC-18: テンプレートが {model} を参照するのに model 指定がない

**種別**: 異常系
**目的**: プレースホルダへの値の供給不足(`{model}` に対し model 未指定)が登録時検証で弾かれることを確認する。

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `$WORK/no-model.yaml` を以下の内容で作成する(`claude` のテンプレートは `{model}` を参照するが、ワークフロー・ステータスのどちらにも `model` がない)。<br><br><pre>agent: claude<br>initial: start<br>statuses:<br>  start:<br>    prompt: "hi"<br>    next: waiting<br>  waiting:<br>    run: wait</pre> | ファイルが作成される |
| 2 | `pulsen add --workflow "$WORK/no-model.yaml" --repo "$REPO"` を実行する | model の値が供給できない旨のエラーが表示されて exit code 非0。タスクは作られない |
| 3 | 回復: `agent: claude` の次の行に `model: claude-opus-4-8` を追記して再実行し、直後に `pulsen abort <task-id>` で止める | 登録が成功して exit code 0。登録したタスクは `start`(agent `claude`)の pending であり、放置すると以後のTCの tick で実行されて失敗を重ね(実エージェント CLI は未インストールのため exit 127)、凍結・通知のノイズになるため直後に abort する(stopped として残留する。無害) |

## TC-19: agents に定義されていないエージェント名を参照する

**種別**: 異常系
**目的**: シナリオ2の異常系。未定義エージェントの参照が登録時にエラーとなり、定義済みエージェント名の一覧が示されることを確認する。

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `$WORK/bad-agent.yaml` を以下の内容で作成する。<br><br><pre>agent: cladue<br>initial: start<br>statuses:<br>  start:<br>    prompt: "hi"<br>    next: waiting<br>  waiting:<br>    run: wait</pre> | ファイルが作成される |
| 2 | `pulsen add --workflow "$WORK/bad-agent.yaml" --repo "$REPO"` を実行する | 未定義エージェント `cladue` のエラーが、config.yaml に定義済みのエージェント名一覧(`shell` / `claude`)とともに表示されて exit code 非0。タスクは作られない |
| 3 | 回復: `agent: shell` に修正して再実行し、直後に `pulsen abort <task-id>` で止める | 登録が成功して exit code 0。登録したタスクは `start`(prompt 実行)の pending であり、放置すると以後のTCの tick で実行されて失敗を重ねるため直後に abort する(stopped として残留する。無害) |

## TC-20: エージェント指定がどこにもない

**種別**: 異常系
**目的**: エージェント実行ステータスに対し、ステータスにもワークフローにもエージェント指定がない定義が登録時に弾かれることを確認する。

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `$WORK/missing-agent.yaml` を以下の内容(トップレベル `agent:` なし)で作成する。<br><br><pre>initial: start<br>statuses:<br>  start:<br>    prompt: "hi"<br>    next: waiting<br>  waiting:<br>    run: wait</pre> | ファイルが作成される |
| 2 | `pulsen add --workflow "$WORK/missing-agent.yaml" --repo "$REPO"` を実行する | エージェント指定の欠落エラーが表示されて exit code 非0。タスクは作られない |

## TC-21: initial・statuses の欠落

**種別**: 異常系
**目的**: シナリオ2の異常系。必須トップレベル構造(`initial:`・`statuses:`)の欠落が読み込み時(登録時のパース)エラーになることを確認する。

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `$WORK/no-initial.yaml` を以下の内容で作成し、`pulsen add --workflow "$WORK/no-initial.yaml" --repo "$REPO"` を実行する。<br><br><pre>agent: shell<br>statuses:<br>  start:<br>    prompt: "hi"<br>    next: waiting<br>  waiting:<br>    run: wait</pre> | `initial` 欠落のパースエラーが表示されて exit code 非0。タスクは作られない |
| 2 | `$WORK/no-statuses.yaml` を以下の内容で作成し、同様に add する。<br><br><pre>agent: shell<br>initial: start</pre> | `statuses` の欠落(空)のパースエラーで exit code 非0。タスクは作られない |

## TC-22: initial・next が存在しないステータスを参照している

**種別**: 異常系
**目的**: シナリオ2の異常系。参照先の不在が到達時ではなく読み込み時に検出されることを確認する。

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `$WORK/bad-initial-ref.yaml` を以下の内容で作成し、`pulsen add --workflow "$WORK/bad-initial-ref.yaml" --repo "$REPO"` を実行する。<br><br><pre>agent: shell<br>initial: nosuch<br>statuses:<br>  start:<br>    prompt: "hi"<br>    next: waiting<br>  waiting:<br>    run: wait</pre> | `initial` の参照先不在のパースエラーで exit code 非0。タスクは作られない |
| 2 | `$WORK/bad-next-ref.yaml` を以下の内容で作成し、同様に add する。<br><br><pre>agent: shell<br>initial: start<br>statuses:<br>  start:<br>    prompt: "hi"<br>    next: nosuch</pre> | `next` の参照先不在のパースエラーで exit code 非0。タスクは作られない |

## TC-23: エージェント実行ステータスに next がない

**種別**: 異常系
**目的**: シナリオ2の異常系。completed 時の遷移先が決まらない定義が読み込み時に弾かれることを確認する。

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `$WORK/no-next.yaml` を以下の内容で作成し、`pulsen add --workflow "$WORK/no-next.yaml" --repo "$REPO"` を実行する。<br><br><pre>agent: shell<br>initial: start<br>statuses:<br>  start:<br>    prompt: "hi"</pre> | `next` 欠落のパースエラーで exit code 非0。タスクは作られない(滞留させたい場合は `run: wait` で表現する旨の仕様) |

## TC-24: 動作宣言が無い・複数ある・run の値が不正

**種別**: 異常系
**目的**: シナリオ2の異常系。動作(`prompt` / `skill` / `run`)が一意に決まらないステータス定義が読み込み時に弾かれることを確認する。

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `$WORK/no-action.yaml` を以下の内容で作成し、`pulsen add --workflow "$WORK/no-action.yaml" --repo "$REPO"` を実行する。<br><br><pre>agent: shell<br>initial: start<br>statuses:<br>  start:<br>    next: waiting<br>  waiting:<br>    run: wait</pre> | 動作宣言なしのパースエラーで exit code 非0 |
| 2 | `$WORK/two-actions.yaml` を以下の内容で作成し、同様に add する。<br><br><pre>agent: shell<br>initial: start<br>statuses:<br>  start:<br>    prompt: "hi"<br>    skill: plan<br>    next: waiting<br>  waiting:<br>    run: wait</pre> | 動作宣言が複数あるパースエラーで exit code 非0 |
| 3 | `$WORK/bad-run.yaml` を以下の内容で作成し、同様に add する。<br><br><pre>agent: shell<br>initial: start<br>statuses:<br>  start:<br>    run: clenaup</pre> | `run` の値が `cleanup` / `wait` 以外であるパースエラーで exit code 非0 |

いずれの手順でもタスクは作られない。

## TC-25: wait / cleanup ステータスへのエージェント実行用キーの併記

**種別**: 異常系
**目的**: `run: wait` / `run: cleanup` のステータスに `judge` や `next` を併記した定義が弾かれることを確認する(許されるキーは `run` のみ)。

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `$WORK/forbidden-key.yaml` を以下の内容で作成し、`pulsen add --workflow "$WORK/forbidden-key.yaml" --repo "$REPO"` を実行する。<br><br><pre>agent: shell<br>initial: start<br>statuses:<br>  start:<br>    prompt: "hi"<br>    next: waiting<br>  waiting:<br>    run: wait<br>    judge: ["sh", "-c", "true"]</pre> | wait ステータスへの禁止キー併記のパースエラーで exit code 非0。タスクは作られない |

## TC-26: ワークフローYAMLが構文として不正(重複キー含む)

**種別**: 異常系
**目的**: シナリオ2の異常系。インデント崩れ・重複キー等の構文エラーが位置とともに報告され、登録が失敗することを確認する。名前指定の場合は、利用者が直接書いていない解決先パスも案内されることを確認する。

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `$WORK/broken.yaml` を以下の内容(インデント崩れ)で作成し、`pulsen add --workflow "$WORK/broken.yaml" --repo "$REPO"` を実行する。<br><br><pre>agent: shell<br>initial: start<br>statuses:<br>start:<br>    prompt: "hi"</pre> | 構文エラーの位置・原因が表示されて exit code 非0。タスクは作られない |
| 2 | `$WORK/dup-key.yaml` を以下の内容(`start` キーの重複)で作成し、同様に add する。<br><br><pre>agent: shell<br>initial: start<br>statuses:<br>  start:<br>    prompt: "hi"<br>    next: waiting<br>  start:<br>    run: wait<br>  waiting:<br>    run: wait</pre> | 重複キーがパースエラーとして扱われ exit code 非0。タスクは作られない |
| 3 | `cp "$WORK/broken.yaml" "$PULSEN_HOME/workflows/broken.yaml"` を実行し、`pulsen add --workflow broken --repo "$REPO"` を名前指定で実行する | 構文エラーの位置・原因に加えて、解決先の絶対パス(`$PULSEN_HOME/workflows/broken.yaml`)が1回だけ表示されて exit code 非0。タスクは作られない(片付ける: `rm "$PULSEN_HOME/workflows/broken.yaml"`) |

## TC-27: ワークフローYAMLにスキーマ外のキーがある

**種別**: 異常系
**目的**: ステータス内の typo(`prmopt` 等)が未知キーとして拒否され、黙殺されないことを確認する。

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `$WORK/typo-key.yaml` を以下の内容で作成し、`pulsen add --workflow "$WORK/typo-key.yaml" --repo "$REPO"` を実行する。<br><br><pre>agent: shell<br>initial: start<br>statuses:<br>  start:<br>    prmopt: "hi"<br>    next: waiting<br>  waiting:<br>    run: wait</pre> | 未知キー `prmopt` のパースエラーで exit code 非0。タスクは作られない |

## TC-28: 複数の検証エラーが全件まとめて表示される

**種別**: 異常系
**目的**: 登録時検証エラーが最初の1件で打ち切られず、全件列挙されることを確認する(修正の往復回数を減らすための仕様)。

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `$WORK/multi-error.yaml` を以下の内容(未定義エージェント参照と skill 指定を複数ステータスに含む)で作成し、`pulsen add --workflow "$WORK/multi-error.yaml" --repo "$REPO"` を実行する。<br><br><pre>agent: nosuch<br>initial: a<br>statuses:<br>  a:<br>    prompt: "hi"<br>    next: b<br>  b:<br>    agent: nosuch2<br>    prompt: "hi"<br>    next: a</pre> | 複数ステータスにまたがる検証エラー(`nosuch`・`nosuch2` の未定義参照)が全件まとめて表示されて exit code 非0。タスクは作られない |

## TC-29: 名前指定で workflows/ に該当ファイルがない

**種別**: 異常系
**目的**: シナリオ4の異常系。名前解決の失敗時に、解決を試みたパスが示され、配置漏れに気づけることを確認する。

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `pulsen add --workflow nosuchflow --repo "$REPO"` を実行する | 解決を試みた絶対パス(`$PULSEN_HOME/workflows/nosuchflow.yaml`)を添えたエラーが表示されて exit code 非0。タスクは作られない |
| 2 | 回復: `$WORK` にあった定義を配置する想定で `cp "$PULSEN_HOME/workflows/implement.yaml" "$PULSEN_HOME/workflows/nosuchflow.yaml"` を実行し、手順1のコマンドを再実行する | 登録が成功して exit code 0(タスクとコピーしたファイルは片付ける: `pulsen set-status <task-id> done` → `pulsen tick`、`rm "$PULSEN_HOME/workflows/nosuchflow.yaml"`) |

## TC-30: ファイルパス指定で指定先が存在しない

**種別**: 異常系
**目的**: シナリオ4の異常系。存在しないファイルパスの指定がエラーとなり、解決を試みたパス(相対パスはカレントディレクトリからの解決)が示されることを確認する。

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `cd "$HOME"` して `pulsen add --workflow ./nosuch.yaml --repo "$REPO"` を実行する | 解決を試みたパス(`$HOME/nosuch.yaml`)を添えたエラーが表示されて exit code 非0。タスクは作られない |

## TC-31: リポジトリパスが存在しない・git リポジトリでない

**種別**: 異常系
**目的**: 対象リポジトリの検証が登録時に行われることを確認する。

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `pulsen add --workflow implement --repo "$HOME/no-such-repo"` を実行する | リポジトリ不在のエラーで exit code 非0。タスクは作られない |
| 2 | `mkdir -p "$WORK/not-a-repo"` して `pulsen add --workflow implement --repo "$WORK/not-a-repo"` を実行する | git リポジトリでない旨のエラーで exit code 非0。タスクは作られない |

## TC-32: ベースブランチがリポジトリに存在しない

**種別**: 異常系
**目的**: `--base` の存在検証が登録時に行われることを確認する。

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `pulsen add --workflow implement --repo "$REPO" --base no-such-branch` を実行する | ブランチ不在のエラーで exit code 非0。タスクは作られない |
| 2 | 回復: `pulsen add --workflow implement --repo "$REPO" --base main` を実行する | 登録が成功して exit code 0(タスクは TC-05 の手順で片付ける) |

## TC-33: HEAD からベースブランチを解決できない(detached HEAD・空リポジトリ)

**種別**: 異常系
**目的**: `--base` 省略時に HEAD からブランチを特定できない場合、`--base` の明示指定が案内されてエラーになることを確認する。

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | detached HEAD のリポジトリを作る: `git clone "$REPO" "$WORK/detached-repo" && git -C "$WORK/detached-repo" checkout --detach` | detached HEAD になる |
| 2 | `pulsen add --workflow implement --repo "$WORK/detached-repo"` を実行する | `--base` の明示指定を案内するエラーで exit code 非0。タスクは作られない |
| 3 | 回復: `pulsen add --workflow implement --repo "$WORK/detached-repo" --base main` を実行する | 登録が成功して exit code 0(タスクは片付ける) |
| 4 | 空リポジトリを作る: `git init -b main "$WORK/empty-repo"`(コミットしない)。`pulsen add --workflow implement --repo "$WORK/empty-repo"` を実行する | 同様に `--base` の明示指定を案内するエラーで exit code 非0(空リポジトリはブランチ実体も無いため、`--base main` を付けてもブランチ不在エラーになる) |

## TC-34: 登録後の config.yaml 編集でエージェント定義が失われ、spawn 失敗から stopped に至る

**種別**: 異常系
**目的**: シナリオ1の異常系(config.yaml の編集と既存タスク)。グローバル設定はスナップショットされないため編集が既存タスクに反映され、エージェント定義の削除が連続 spawn 失敗 → 凍結・通知に至ること、config 修復と `retry` で復旧できることを確認する。

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `pulsen add --workflow implement --repo "$REPO"` でタスクを登録する(tick はまだ実行しない) | タスクIDが表示されて exit code 0 |
| 2 | `$PULSEN_HOME/config.yaml` から `shell:` のエージェント定義(`shell:` / `cmd:` / `skill_input:` の3行)を削除する | ファイルが更新される(`claude` の定義は残す) |
| 3 | `pulsen tick` を実行し、`pulsen show <task-id>` で確認する | 起動時のテンプレート解決に失敗し、spawn 失敗として分類される。spawn_fail_count が 1 になり、実行状態は `pending` に戻る(直近の失敗要因に spawn 失敗のメッセージが表示される) |
| 4 | `pulsen tick` を spawn_fail_count が上限(3)を超えるまで繰り返す(計4回程度) | 上限超過で実行状態が `stopped`(凍結)になり、tick サマリーに凍結・通知として表示される。`$PULSEN_HOME/notify.log` にこのタスクの行が追記される |
| 5 | 回復: config.yaml に `shell` の定義を戻し、`pulsen retry <task-id>` を実行する | pending に戻した旨が表示されて exit code 0 |
| 6 | `pulsen tick` を実行し、数秒後にもう一度 `pulsen tick` を実行する | タスクが正常に起動・進行する(config の修正と retry だけで復旧できる)。確認後は片付ける(`review_waiting` 到達後に `pulsen set-status <task-id> done` → `pulsen tick`) |

## TC-35: notify_cmd 未定義でも stopped の確定は正常に動作する

**種別**: 異常系
**目的**: シナリオ1の異常系。`notify_cmd` が未定義の場合、通知は行われないが他の動作(stopped の確定)には影響しないことを確認する。

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `$PULSEN_HOME/config.yaml` の `notify_cmd:` の行を削除(またはコメントアウト)する | ファイルが更新される |
| 2 | `pulsen add --workflow implement --repo "$REPO"` でタスクを登録し、`pulsen abort <task-id>` を実行する | stopped を記録した旨が表示されて exit code 0。通知失敗の警告は表示されない |
| 3 | `$PULSEN_HOME/notify.log` を確認し、`pulsen show <task-id>` を実行する | notify.log にこのタスクの行は追記されていない。show では実行状態 `stopped` と凍結要因(人間による abort)が表示される |
| 4 | 回復: config.yaml に `notify_cmd` を戻す。タスクは `pulsen set-status <task-id> done` → `pulsen tick` で片付ける | 以後のTCで通知が再び機能する |

## TC-36: 文字列形式テンプレートではパイプ・クォートがシェル解釈されない

**種別**: 異常系
**目的**: シナリオ1の異常系。コマンドはシェルを介さず直接起動されるため、文字列テンプレート中のパイプは機能せず、クォートもグルーピングとして解釈されないことを確認する(観点: 特殊入力)。

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `$PULSEN_HOME/config.yaml` の `agents:` に以下を追記する。<br><br><pre>  pipe-demo:<br>    cmd: echo {input} \| tr a-z A-Z<br>  quote-demo:<br>    cmd: sh -c "echo hi"</pre> | ファイルが更新される |
| 2 | `$WORK/pipe.yaml` を以下の内容で作成し、`pulsen add --workflow "$WORK/pipe.yaml" --repo "$REPO"` → `pulsen tick` を実行する。<br><br><pre>agent: pipe-demo<br>initial: piped<br>statuses:<br>  piped:<br>    prompt: "hello"<br>    next: waiting<br>  waiting:<br>    run: wait</pre> | タスクが起動される |
| 3 | 数秒待って `pulsen tick` を 2〜3 秒間隔で2回実行し(spawn確認 → 判定)、`cat "$PULSEN_HOME/state/runs/<task-id>/attempt-1/stdout.log"` を確認する | `hello \| tr a-z A-Z` がそのまま1行出力されている(パイプはシェル機能として働かず、`\|` 以降も `echo` の引数として文字どおり渡っている)。タスク自体は exit 0 で completed になる |
| 4 | `$WORK/quote.yaml` を以下の内容で作成し、同様に add → tick を実行する。<br><br><pre>agent: quote-demo<br>initial: quoted<br>statuses:<br>  quoted:<br>    prompt: "unused"<br>    next: waiting<br>  waiting:<br>    run: wait</pre> | タスクが起動される |
| 5 | 数秒待って `pulsen tick` を 2〜3 秒間隔で2回実行し(spawn確認 → 判定)、`pulsen show <task-id>` と runディレクトリの `exit` / `stderr.log` を確認する | クォートがグルーピングされないため、トークンは `sh` / `-c` / `"echo` / `hi"` となり、`sh` には引用符が閉じていない不完全なスクリプト `"echo` が渡る。`stderr.log` に引用符の閉じ忘れを示す構文エラーが記録され、`exit` は非0(bash 系では `2`。値はシェル実装に依る)で、実行状態が `failed` になる |
| 6 | 回復: シェル機能が必要な場合は配列形式(`["sh", "-c", "{input}"]` = `shell` エージェント)を使う。片付け: 両タスクを `pulsen abort <task-id>` で止める。config.yaml から手順1の追記を削除する | 両タスクは stopped として残留する(pipe.yaml / quote.yaml にはクリーンアップステータスがなく、set-status で終端処理に乗せられない。最終処分が必要なら `monitoring.md` の手動アーカイブ移動の手順を使う)。config は元に戻る |

## TC-37: 判定コマンドが 0 / 10 / 20 以外で終了すると判定失敗として凍結に至る

**種別**: 異常系
**目的**: シナリオ5の異常系。慣習的な `exit 1` が failed ではなく判定失敗として扱われ、judge_attempt_count の上限超過で stopped・通知されること、エージェントのリトライは行われないことを確認する。

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `echo 1 > "$PULSEN_HOME/judge-exit"` を実行し、`pulsen add --workflow judge-demo --repo "$REPO"` → `pulsen tick` でタスクを起動する | タスクが起動される |
| 2 | 数秒待って `pulsen tick` を 2〜3 秒間隔で2回実行し(spawn確認 → 判定)、`pulsen show <task-id>` で確認する | 判定失敗として judge_attempt_count が 1 になる(上限3が併記される)。attempt_count は増えず、エージェントの再実行は行われない(attempt 番号が変わらない) |
| 3 | `pulsen tick` を judge_attempt_count が上限(3)を超えるまで繰り返す(計3回程度。running のまま毎 tick 再判定される) | 上限超過で実行状態が `stopped` になり、`$PULSEN_HOME/notify.log` に通知の行が追記される |
| 4 | 回復: `echo 0 > "$PULSEN_HOME/judge-exit"` にして `pulsen retry <task-id>` → `pulsen tick`(起動)→ 数秒後に `pulsen tick` を 2〜3 秒間隔で2回(spawn確認 → 判定) | 再実行の判定が completed になりタスクが進む。片付け: `pulsen set-status <task-id> done` → `pulsen tick` |

## TC-38: 判定コマンドの実体が見つからない

**種別**: 異常系
**目的**: シナリオ5の異常系。判定コマンドが起動できない場合に判定失敗として扱われることを確認する。

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `$WORK/judge-missing.yaml` を以下の内容で作成し、`pulsen add --workflow "$WORK/judge-missing.yaml" --repo "$REPO"` → `pulsen tick` でタスクを起動する。<br><br><pre>agent: shell<br>initial: checking<br>statuses:<br>  checking:<br>    prompt: "echo hi"<br>    judge: /no/such/judge.sh<br>    next: waiting<br>  waiting:<br>    run: wait</pre> | 登録は成功する(`judge` のコマンド実体は登録時に検証されない)。タスクが起動される |
| 2 | 数秒待って `pulsen tick` を 2〜3 秒間隔で2回実行し(spawn確認 → 判定)、`pulsen show <task-id>` で確認する | 判定コマンドの起動失敗が判定失敗として扱われ、judge_attempt_count が 1 になる |
| 3 | 片付け: `pulsen abort <task-id>` を実行する | stopped が記録されて exit code 0 |

## TC-39: 判定コマンドが応答しない(判定 timeout)

**種別**: 異常系
**目的**: シナリオ5の異常系とフロー5。判定コマンド自体の timeout(グローバル設定で調整可)の超過が判定失敗として扱われることを確認する。

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `$PULSEN_HOME/config.yaml` の `judge_timeout: 60s` を `judge_timeout: 5s` に変更する | ファイルが更新される |
| 2 | `$WORK/judge-hang.yaml` を以下の内容で作成し、`pulsen add --workflow "$WORK/judge-hang.yaml" --repo "$REPO"` → `pulsen tick` でタスクを起動する。<br><br><pre>agent: shell<br>initial: checking<br>statuses:<br>  checking:<br>    prompt: "echo hi"<br>    judge: ["sh", "-c", "sleep 120"]<br>    next: waiting<br>  waiting:<br>    run: wait</pre> | タスクが起動される |
| 3 | 数秒待って `pulsen tick` を実行し(spawn確認)、さらに 2〜3 秒待って `pulsen tick` を実行する(判定。tick はこの判定で5秒強ブロックする) | timeout 超過が判定失敗として扱われ、`pulsen show <task-id>` で judge_attempt_count が 1 になっている |
| 4 | 回復: `judge_timeout: 60s` に戻し、タスクは `pulsen abort <task-id>` で止める | 環境が元に戻る |

## 境界値

## TC-40: --workflow の名前 / パス解釈は決定的規則で決まる

**種別**: 境界値
**目的**: シナリオ4の異常系(パス指定とワークフロー名の曖昧さ)。パス区切り文字・`.yaml` / `.yml` 拡張子の有無だけで解釈が決まり、ファイルの存在有無やカレントディレクトリの同名ファイルに依存しないことを確認する。

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `cd "$WORK"` して、TC-07 の内容で `./nameclash.yaml` を作成する。`$PULSEN_HOME/workflows/nameclash.yaml` は置かない | ファイルが作成される |
| 2 | `pulsen add --workflow nameclash --repo "$REPO"` を実行する | 区切り文字も拡張子も含まないため常に名前として解釈され、`$PULSEN_HOME/workflows/nameclash.yaml` の解決失敗(解決を試みたパスの表示)で exit code 非0。カレントディレクトリの `nameclash.yaml` は使われない |
| 3 | `pulsen add --workflow ./nameclash.yaml --repo "$REPO"` を実行する | パス区切り文字を含むためファイルパスとして解釈され、登録が成功して exit code 0(タスクは片付ける) |

## TC-41: 名前解決は .yaml のみ(.yml へのフォールバックなし)

**種別**: 境界値
**目的**: 名前指定の解決先が `workflows/<name>.yaml` のみであり、`.yml` は拡張子付きパス指定でのみ扱えることを確認する。

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `cp "$PULSEN_HOME/workflows/implement.yaml" "$PULSEN_HOME/workflows/impl2.yml"` を実行する(`.yml` のみ配置) | ファイルが作成される |
| 2 | `pulsen add --workflow impl2 --repo "$REPO"` を実行する | `workflows/impl2.yaml` の解決失敗で exit code 非0(`.yml` へのフォールバックはしない) |
| 3 | `pulsen add --workflow "$PULSEN_HOME/workflows/impl2.yml" --repo "$REPO"` を実行する | `.yml` で終わる指定はファイルパスとして解釈され、登録が成功して exit code 0(タスクとファイルは片付ける) |

## TC-42: 空の config.yaml は全デフォルトで動作する

**種別**: 境界値
**目的**: キーを1つも持たない config.yaml が「全キー省略」として受理され、デフォルト値で動作することを確認する(すべてのキーは任意)。

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `mkdir -p "$HOME/pulsen-default-home"` して空の config を作る: `touch "$HOME/pulsen-default-home/config.yaml"` | 空ファイルが作られる |
| 2 | `pulsen ls --home "$HOME/pulsen-default-home"` を実行する | エラーにならず、空一覧が表示されて exit code 0 |
| 3 | `pulsen tick --home "$HOME/pulsen-default-home"` を実行する | 処理対象がない旨で exit code 0(`agents` 空・`notify_cmd` なし・デフォルト上限で動作する) |

## TC-43: timeout: none は受理、timeout: 0s は拒否

**種別**: 境界値
**目的**: 期間指定の境界。無制限の明示(`none`)は受理され、不正な期間(`0s`)は登録時に弾かれることを確認する(無指定 = 無制限とは扱われない)。

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `$WORK/timeout-none.yaml` を以下の内容で作成し、`pulsen add --workflow "$WORK/timeout-none.yaml" --repo "$REPO"` を実行する。<br><br><pre>agent: shell<br>initial: start<br>statuses:<br>  start:<br>    prompt: "echo hi"<br>    timeout: none<br>    next: waiting<br>  waiting:<br>    run: wait</pre> | 無制限として受理され、登録が成功して exit code 0(この定義にはクリーンアップステータスがないため set-status で終端処理に乗せられない。タスクは `pulsen abort <task-id>` で止め、stopped としての残留を許容する。最終処分が必要なら monitoring.md TC-34 の手動アーカイブ移動の手順を使う) |
| 2 | 同じファイルの `timeout: none` を `timeout: 0s` に変えて再度 add する | 期間の検証エラーで exit code 非0。タスクは作られない |

## TC-44: retries: 0 は正当な値として受理される

**種別**: 境界値
**目的**: リトライ上限の下限境界。`retries: 0`(初回失敗で即 stopped の意味)が登録時に受理されることを確認する。

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `$WORK/retries-zero.yaml` を以下の内容で作成し、`pulsen add --workflow "$WORK/retries-zero.yaml" --repo "$REPO"` を実行する。<br><br><pre>agent: shell<br>initial: start<br>statuses:<br>  start:<br>    prompt: "echo hi"<br>    retries: 0<br>    next: waiting<br>  waiting:<br>    run: wait</pre> | 受理されて登録が成功し exit code 0(この定義にはクリーンアップステータスがないため set-status で終端処理に乗せられない。タスクは `pulsen abort <task-id>` で止め、stopped としての残留を許容する。最終処分が必要なら monitoring.md TC-34 の手動アーカイブ移動の手順を使う) |

## TC-45: 循環・到達不能ステータス・終端なしの定義が受理される

**種別**: 境界値
**目的**: シナリオ2のフロー4。`next` の自己参照(循環)、遷移経路のない到達不能ステータス、クリーンアップ終端の省略がいずれも正当な定義として受理されることを確認する(ポーリング型ワークフローの表現)。

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `$WORK/polling.yaml` を以下の内容(自己参照循環 + 到達不能な `done`)で作成し、`pulsen add --workflow "$WORK/polling.yaml" --repo "$REPO"` を実行する。<br><br><pre>workflow: polling<br>agent: shell<br>initial: check<br>statuses:<br>  check:<br>    prompt: "echo polled"<br>    next: check<br>  done:<br>    run: cleanup</pre> | 受理されて登録が成功し exit code 0 |
| 2 | 同じ内容から `done` ステータスを削除した `$WORK/polling-noend.yaml` を作成し、同様に add する | クリーンアップ終端が無くても受理されて exit code 0 |
| 3 | 片付け: 手順1のタスクは `pulsen set-status <task-id> done` → `pulsen tick`。手順2のタスクはクリーンアップステータスが無いため `pulsen abort <task-id>` で止める | 手順1はアーカイブされる。手順2は stopped として残留する(これがシナリオ4の異常系の言う「abort が代替手段」の状態) |

## TC-46: 表示名を決定できないファイル名のパス指定は拒否される

**種別**: 境界値
**目的**: `workflow:` キーがなく、ファイル名の語幹が空白のみになるパス指定が、表示名の決定失敗として弾かれることを確認する。

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `workflow:` キーを含まない有効な定義(TC-20 の YAML の先頭に `agent: shell` の行を追記したもの)を `$WORK/ .yaml`(先頭が空白)という名前で作成し、`pulsen add --workflow "$WORK/ .yaml" --repo "$REPO"` を実行する | 表示名の決定失敗として exit code 非0。タスクは作られない |

## TC-47: エージェントの異常終了(シグナル死)の EXIT_CODE は非0に符号化されて判定に渡る

**種別**: 境界値
**目的**: シナリオ5の異常系。シグナル死した実行の終了結果が非0の数値(POSIX では 128+シグナル番号)として符号化され、判定コマンドの `EXIT_CODE` に渡ることを確認する。

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `echo 10 > "$PULSEN_HOME/judge-exit"` を実行する。`$WORK/sigkill.yaml` を以下の内容で作成し、`pulsen add --workflow "$WORK/sigkill.yaml" --repo "$REPO"` → `pulsen tick` でタスクを起動する。<br><br><pre>agent: shell<br>initial: dying<br>statuses:<br>  dying:<br>    prompt: "kill -KILL $$"<br>    judge: ["sh", "-c", "\"$HOME/pulsen-manual-test/judge.sh\""]<br>    next: waiting<br>  waiting:<br>    run: wait</pre> | タスクが起動される |
| 2 | 数秒待って `pulsen tick` を 2〜3 秒間隔で2回実行し(spawn確認 → 判定)、`cat "$PULSEN_HOME/state/runs/<task-id>/attempt-1/exit"` と `$PULSEN_HOME/judge.log` の末尾を確認する | exit ファイルに 137(128+9)相当の非0値が記録され、judge.log の当該行に `exit=137` として渡っている。判定は exit 10 により failed となり、タスクは `failed` でリトライ待ちになる |
| 3 | 片付け: `pulsen abort <task-id>` を実行する | stopped が記録されて exit code 0 |

## TC-48: グローバルホーム解決の優先順位(フラグ > 環境変数 > 既定)

**種別**: 境界値
**目的**: シナリオ1の異常系(`PULSEN_HOME` と `--home` の両指定)。フラグが優先されること、エラー時に解決後のホームパスで誤解決に気づけることを確認する。

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `PULSEN_HOME=/no/such/home pulsen ls --home "$HOME/pulsen-manual-test"` を実行する | フラグのホームが優先され、正常に一覧が表示されて exit code 0 |
| 2 | `PULSEN_HOME=/no/such/home pulsen ls` を実行する(フラグなし) | 環境変数のホームが使われ、未初期化エラーに解決後のパス `/no/such/home` が含まれて exit code 非0(意図しないホームを見ていることがエラーメッセージから分かる) |

## カバレッジ

### ユースケースエラーケース対応表

RegisterTask(add)のエラーケース(`spec/usecases/task.md`。パースエラー・登録時検証エラーは `spec/testcases/task/register-task.md` の列挙で展開)と、このカテゴリに関係する config 縮退・Tick のエラーケースの対応。

| ユースケース | エラーケース | 対応TC | 備考 |
|---|---|---|---|
| RegisterTask | ロック競合 | task-execution.md TC-24 | notify_cmd の長時間実行でロック保持時間を作る決定的手法(intervention.md TC-25 と同じ)で、add の非0拒否・タスク非作成を確認する |
| RegisterTask | ワークフロー解決失敗: 名前指定で不在(`NotFound`) | TC-29 | |
| RegisterTask | ワークフロー解決失敗: パス指定で不在(`NotFound`) | TC-30 | |
| RegisterTask | ワークフロー解決失敗: 読み取り不能(`Io`) | TC-15 | ファイル権限で再現(手順3) |
| RegisterTask | パースエラー: 構文不正・重複キー(`YamlSyntax`) | TC-26 | |
| RegisterTask | パースエラー: スキーマ外キー(`UnknownKey`) | TC-27 | |
| RegisterTask | パースエラー: wait / cleanup への禁止キー併記(`ForbiddenKey`) | TC-25 | |
| RegisterTask | パースエラー: `initial` 欠落(`MissingInitial`) | TC-21 | |
| RegisterTask | パースエラー: `initial` の参照先不在(`InitialNotFound`) | TC-22 | |
| RegisterTask | パースエラー: `statuses` 空・欠落(`EmptyStatuses`) | TC-21 | |
| RegisterTask | パースエラー: 動作宣言なし(`NoAction`) | TC-24 | |
| RegisterTask | パースエラー: 動作宣言が複数(`MultipleActions`) | TC-24 | |
| RegisterTask | パースエラー: `run` の値が不正(`UnknownRunValue`) | TC-24 | |
| RegisterTask | パースエラー: エージェント実行に `next` なし(`MissingNext`) | TC-23 | |
| RegisterTask | パースエラー: `next` の参照先不在(`NextNotFound`) | TC-22 | |
| RegisterTask | パースエラー: 値の不正(`InvalidValue`。`timeout: 0s` 等) | TC-43 | 手順2 |
| RegisterTask | 表示名の決定失敗(パス指定でファイル名由来の名前が不正) | TC-46 | |
| RegisterTask | リポジトリ不在・非リポジトリ | TC-31 | |
| RegisterTask | ベースブランチ不在 | TC-32 | |
| RegisterTask | HEAD 解決不能: detached HEAD | TC-33 | |
| RegisterTask | HEAD 解決不能: 空リポジトリ | TC-33 | |
| RegisterTask | 対象検証の git 操作自体の失敗(`TargetError::Failed`) | 対象外 | git 内部の障害注入が必要で、CLI操作から再現不能 |
| RegisterTask | 登録時検証: エージェント指定なし(`MissingAgent`) | TC-20 | |
| RegisterTask | 登録時検証: 未定義エージェント参照(`UnknownAgent`) | TC-19 | 定義済み一覧の表示を含めて検証 |
| RegisterTask | 登録時検証: テンプレート不備(`InvalidAgentDefinition`) | TC-16 | |
| RegisterTask | 登録時検証: `skill_input` 欠落(`MissingSkillInput`) | TC-17 | |
| RegisterTask | 登録時検証: `{model}` への値の供給なし(`MissingModel`) | TC-18 | |
| RegisterTask | 登録時検証エラーの全件列挙 | TC-28 | |
| RegisterTask | ID 衝突の再発 | 対象外 | ID生成器への衝突注入が必要で、CLI操作から再現不能 |
| RegisterTask | タスクファイル作成の Io 失敗 | 対象外 | 書き込み中のファイルシステム障害の注入が必要 |
| 共通(config 縮退) | config.yaml 不在(`NotFound`) | TC-12 | 解決後のホームパスの表示を含めて検証 |
| 共通(config 縮退) | config.yaml 構文不正(`Invalid`) | TC-13 | |
| 共通(config 縮退) | config.yaml スキーマ外キー(`Invalid`) | TC-14 | |
| 共通(config 縮退) | config.yaml 読み取り不能(`Io`) | TC-15 | |
| 共通 | ロック機構自体の異常(`LockError::Failed`) | 対象外 | ロック機構の障害注入が必要で、CLI操作から再現不能 |
| Tick | 判定コマンドが 0 / 10 / 20 以外で終了(判定失敗 → 上限超過で stopped・通知) | TC-37 | |
| Tick | 判定コマンドの timeout 超過(判定失敗) | TC-39 | |
| Tick | 判定コマンドの起動不能(判定失敗) | TC-38 | |
| Tick | テンプレート展開失敗(登録後の config 編集)→ 連続 spawn 失敗 → stopped・通知 | TC-34 | シナリオ1異常系の復旧手順(config 修正 + retry)を含む |
| Tick / AbortTask | `notify_cmd` 未定義(通知なしで stopped 確定) | TC-35 | |

### 観点チェックリスト

| 観点 | 対応TC | 対象外の理由 |
|---|---|---|
| 入力バリデーション(コマンド引数・YAML内容の不正) | TC-16〜TC-28, TC-31〜TC-33 | |
| 境界値 | TC-40〜TC-48 | |
| 認証・権限(ファイル権限) | TC-15 | ログイン・ロールの概念はない。権限系の境界は設定・定義ファイルの読み取り権限のみ |
| 空状態・初期状態(未初期化ホーム・タスク0件) | TC-01(タスク0件の ls), TC-02(タスク0件の tick), TC-12(未初期化ホーム), TC-42(空 config) | |
| 重複・競合 | TC-04(二重登録は仕様上許容: 重複排除しない) | ロック競合(tick 二重起動の 0 スキップ・tick 中の CLI 操作の非0拒否)は task-execution.md TC-24 / intervention.md TC-25 で代表確認する |
| 削除・変更の影響 | TC-08(元ファイルの編集・削除は既存タスクに影響しない), TC-34(config のエージェント定義削除は既存タスクに反映される) | |
| 操作の中断・逸脱 | 対象外 | 対話フォームを持たない CLI のため入力途中の離脱は存在しない。tick の途中クラッシュはUI操作から注入不能(アトミック置換と冪等な再導出で設計上保護され、`spec/testcases/execution/tick.md` のユニットテストが担う) |
| 特殊入力 | TC-36(文字列形式でのクォート・パイプの非解釈), TC-47(シグナル死 exit code の符号化) | |
| UIの状態(エラー後の修正 → 再実行) | TC-12〜TC-16, TC-29, TC-32〜TC-34, TC-37(各TCの回復手順として検証) | 送信中の再操作に相当する並行実行はロック競合と同様タイミング依存のため対象外 |

シナリオ4の異常系「クリーンアップステータスのない定義で登録したタスクの最終処分(タスクファイルの手動アーカイブ)」は、タスクファイルの直接操作を伴うため「状態の確認と追跡」カテゴリの monitoring.md TC-34(手動アーカイブ移動)で扱い、本書では TC-45 手順3 で stopped 残留の状態を作るところまでを確認する。
