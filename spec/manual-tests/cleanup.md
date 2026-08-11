# 終端処理とアーカイブ テスト

## 概要

このドキュメントは、クリーンアップステータス到達によるworktree削除とアーカイブ、成果ブランチの回収、アーカイブ済みタスクの一覧・参照、保持期間(`run_retention`)によるrunディレクトリの自動gcに関するマニュアルテストの手順書です。

対応シナリオ: `spec/scenario/cleanup.md`

| シナリオ | 対応TC |
|---|---|
| クリーンアップ到達によるworktree削除とアーカイブ | TC-01〜TC-09 |
| 成果ブランチの回収 | TC-10〜TC-12 |
| アーカイブ済みタスクの一覧・参照 | TC-13〜TC-17 |
| 保持期間によるrunディレクトリの自動gc | TC-18〜TC-23 |

テストケースは番号順に実施する前提で書かれている。前のTCが作った状態(アーカイブ済みタスク・凍結タスク等)を後のTCが利用する箇所には、各TCの前提に依存元を明記した。

## 前提条件

### 環境

- macOS または Linux(権限操作 `chmod` による失敗再現を使うため。Windows では削除失敗の再現に「ファイルを開いたままのプロセス」等の別手段が必要)
- `pulsen` バイナリがビルド済みで PATH にあること(`cargo build --release` 後、`export PATH="$PWD/target/release:$PATH"` 等)
- `git` が使えること
- すべての手順は、事前準備で環境変数(`TESTHOME` / `PULSEN_HOME`)を設定したシェルで実行する

### テストデータ

- 専用のテストホーム(`$TESTHOME/home`)と、コミット1つの空リポジトリ(`$TESTHOME/repo`)
- 即終了する shell エージェント(`sh -c`)と、4種のワークフロー(コミットを残して完走する `finish` / 何もせず完走する `noop` / 即凍結する `fail` / 滞留する `hold`)

### 事前準備

1. テストホーム(分離ホーム)を初期化し(既存なら削除して作り直す。過去の実行や他ドキュメントの残留状態を持ち込まないため)、環境変数を設定する。

    ```bash
    export TESTHOME="$HOME/pulsen-manual-test"
    rm -rf "$TESTHOME"
    mkdir -p "$TESTHOME/home/workflows"
    export PULSEN_HOME="$TESTHOME/home"
    ```

2. グローバル設定を作成する(`run_retention` は書かない。TC-19 で追記する)。

    ```bash
    cat > "$PULSEN_HOME/config.yaml" <<EOF
    agents:
      shell:
        cmd: ["sh", "-c", "{input}"]
    notify_cmd: ["sh", "-c", "echo stopped: \$TASK_ID \$WORKFLOW \$TASK_STATUS >> $TESTHOME/notify.log"]
    EOF
    ```

3. 対象リポジトリを作成する。

    ```bash
    mkdir -p "$TESTHOME/repo"
    git -C "$TESTHOME/repo" init -b main
    git -C "$TESTHOME/repo" config user.name tester
    git -C "$TESTHOME/repo" config user.email tester@example.com
    git -C "$TESTHOME/repo" commit --allow-empty -m init
    ```

4. ワークフローを4種作成する。

    ```bash
    cat > "$PULSEN_HOME/workflows/finish.yaml" <<'EOF'
    workflow: finish
    agent: shell
    initial: work
    statuses:
      work:
        prompt: "echo agent-output && echo done > memo.txt && git add memo.txt && git commit -m task-output"
        next: done
      done:
        run: cleanup
    EOF

    cat > "$PULSEN_HOME/workflows/noop.yaml" <<'EOF'
    workflow: noop
    agent: shell
    initial: work
    statuses:
      work:
        prompt: "true"
        next: done
      done:
        run: cleanup
    EOF

    cat > "$PULSEN_HOME/workflows/fail.yaml" <<'EOF'
    workflow: fail
    agent: shell
    initial: work
    statuses:
      work:
        prompt: "exit 1"
        retries: 0
        next: done
      done:
        run: cleanup
    EOF

    cat > "$PULSEN_HOME/workflows/hold.yaml" <<'EOF'
    workflow: hold
    agent: shell
    initial: work
    statuses:
      work:
        prompt: "true"
        next: waiting
      waiting:
        run: wait
      done:
        run: cleanup
    EOF
    ```

補足:

- tick は1タスクにつき1回1ステップだけ進める。以降「tick を N 回進める」は `for i in $(seq N); do pulsen tick; sleep 2; done` を指す。連続する tick の間は 2〜3 秒あける(ラッパーの pid ファイル書き込みを待つため。間隔が短いと spawn 確認が KeepWaiting(猶予時間内)となって以降の tick 刻みの期待がすべてずれる)。
- 「done / pending まで進める」は、`pulsen tick && sleep 2` を1回実行するたびに `pulsen ls` を確認し、対象タスクのタスクステータスが `done`・実行状態が `pending` になったところで止めることを指す(`noop` / `finish` では通常4回。5回目の tick でクリーンアップが走るため、進めすぎない)。
- `add` の出力に表示されるタスクIDを控え、手順中の `<T1>` 等を実際のIDに読み替える。

## TC-01: クリーンアップ到達によるworktree削除とアーカイブ

**種別**: 正常系
**目的**: 完走したタスクのworktreeが自動で削除され、タスクファイルが `state/archive/` へ移動し、ブランチとrunディレクトリは残ることを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `pulsen ls --all` を実行する | タスクが1件もなく、空である旨が表示され exit code 0(`echo $?` で確認) |
| 2 | `pulsen add --workflow finish --repo "$TESTHOME/repo"` を実行する | タスクIDが表示され exit code 0。このIDを `<T1>` として控える |
| 3 | tick を5回進める(起動 → running → 判定 → 遷移 → クリーンアップ。進行中 `pulsen ls` で work → done と進むのが見える) | 5回目の tick のサマリーに `<T1>` の終端処理(アーカイブ)が報告される |
| 4 | `ls "$PULSEN_HOME/worktrees/"` を実行する | `<T1>` のディレクトリが存在しない(worktree削除済み) |
| 5 | `ls "$PULSEN_HOME/state/tasks/" "$PULSEN_HOME/state/archive/"` を実行する | `<T1>` のタスクファイルが `state/tasks/` に無く、`state/archive/` にある |
| 6 | `git -C "$TESTHOME/repo" branch --list 'pulsen/*'` を実行する | `pulsen/<T1>` が残存している(ブランチは削除されない) |
| 7 | `ls "$PULSEN_HOME/state/runs/<T1>/"` を実行する | `attempt-1` ディレクトリが残存している(クリーンアップでは削除されない) |

**確認ポイント**:

- 手順3以降にもう一度 `pulsen tick` を実行しても `<T1>` に関する処理は報告されない(アーカイブ済みは走査対象外)

## TC-02: 凍結(stopped)タスクのworktreeは削除されない

**種別**: 正常系
**目的**: stopped のタスクはクリーンアップステータスに達していないため、tick を重ねてもworktree・タスクファイルが保持されることを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `pulsen add --workflow fail --repo "$TESTHOME/repo"` を実行する | タスクIDが表示される。`<T2>` として控える |
| 2 | tick を3回進める | 3回目の tick で凍結が報告される。`pulsen ls` で `<T2>` がタスクステータス `work`・実行状態 `stopped` |
| 3 | `cat "$TESTHOME/notify.log"` を実行する | `stopped: <T2> fail work` の行がある(凍結確定時の通知) |
| 4 | さらに tick を2回進める | `<T2>` に対する起動・遷移・削除は行われない。`notify.log` に行は増えない(通知済みの再通知はない) |
| 5 | `ls "$PULSEN_HOME/worktrees/" "$PULSEN_HOME/state/tasks/"` を実行する | `<T2>` のworktreeディレクトリとタスクファイルの両方が残存している |

## TC-03: stoppedタスクのset-statusによる手動終端

**種別**: 正常系
**目的**: 調査を終えた凍結タスクを set-status でクリーンアップステータスへ遷移させると、通常と同じ終端処理に乗ることを検証する

**前提**: TC-02 実施直後(`<T2>` が stopped)

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `pulsen set-status <T2> done` を実行する | `work` → `done` へ遷移した旨が表示され exit code 0 |
| 2 | `pulsen ls` を実行する | `<T2>` がタスクステータス `done`・実行状態 `pending`(凍結が解けている) |
| 3 | `pulsen tick` を実行する | サマリーに `<T2>` の終端処理が報告される |
| 4 | `ls "$PULSEN_HOME/worktrees/"` と `ls "$PULSEN_HOME/state/archive/"` を実行する | `<T2>` のworktreeが消滅し、タスクファイルが `state/archive/` にある |

**確認ポイント**:

- `git -C "$TESTHOME/repo" branch --list 'pulsen/*'` で `pulsen/<T2>` は残存している(失敗タスクでもブランチには関与しない)

## TC-04: worktree削除の失敗(ファイルを掴んでいる状態)

**種別**: 異常系
**目的**: worktree削除に失敗した tick が attempt_count を消費してタスクを `state/tasks/` に残し、次の tick が同じ判断で再試行することを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `pulsen add --workflow noop --repo "$TESTHOME/repo"` を実行し、`<T3>` として控える。tick を done / pending まで進める(通常4回) | `pulsen ls` で `<T3>` が `done` / `pending`。worktree `worktrees/<T3>` はまだ存在する |
| 2 | worktree内に削除できないファイルを仕込む: `mkdir -p "$PULSEN_HOME/worktrees/<T3>/lockdir" && touch "$PULSEN_HOME/worktrees/<T3>/lockdir/pin" && chmod 555 "$PULSEN_HOME/worktrees/<T3>/lockdir"` | エラーなく完了する |
| 3 | `pulsen tick; echo $?` を実行する | tick 自体は 0 で終了し、サマリーに `<T3>` のworktree削除失敗が報告される |
| 4 | `pulsen ls` を実行する | `<T3>` が `done` / `failed`・attempt_count 1 のまま一覧に残っている(アーカイブへ進んでいない) |
| 5 | `pulsen show <T3>` を実行する | 直近の失敗要因としてworktree削除の失敗が表示される |
| 6 | `ls "$PULSEN_HOME/state/tasks/" "$PULSEN_HOME/worktrees/"` を実行する | タスクファイルもworktreeディレクトリも残存している |

## TC-05: クリーンアップのリトライ上限超過で凍結・通知

**種別**: 境界値
**目的**: クリーンアップの失敗に適用されるリトライ上限が組み込みの 2 であり、加算後 attempt_count = 2 では failed のまま、= 3 で stopped となって通知されることを検証する

**前提**: TC-04 実施直後(`<T3>` が worktree削除失敗の failed・attempt_count 1。`lockdir` の権限はそのまま)

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `pulsen tick` を実行する(2回目の失敗) | `pulsen ls` で `<T3>` は `done` / `failed`・attempt_count 2(上限との等号では凍結しない) |
| 2 | `pulsen tick` を実行する(3回目の失敗) | サマリーに凍結が報告され、`pulsen ls` で `<T3>` が `stopped` |
| 3 | `cat "$TESTHOME/notify.log"` を実行する | `stopped: <T3> noop done` の行が追加されている(確定時の通知) |
| 4 | `pulsen show <T3>` を実行する | stopped であること・凍結要因(worktree削除の失敗)が表示される |

## TC-06: 原因解消後のretryによるクリーンアップ再試行

**種別**: 異常系
**目的**: 凍結の原因(削除できないファイル)を解消して retry すると、次の tick からクリーンアップが再試行され完了することを検証する

**前提**: TC-05 実施直後(`<T3>` が stopped)

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | 原因を解消する: `chmod -R u+w "$PULSEN_HOME/worktrees/<T3>"` | エラーなく完了する |
| 2 | `pulsen retry <T3>` を実行する | pending に戻した旨が表示され exit code 0。`pulsen ls` で `done` / `pending`・attempt_count 0 |
| 3 | `pulsen tick` を実行する | サマリーに `<T3>` の終端処理が報告される |
| 4 | `ls "$PULSEN_HOME/worktrees/"` と `ls "$PULSEN_HOME/state/archive/"` を実行する | `<T3>` のworktreeが消滅し、タスクファイルが `state/archive/` にある |

## TC-07: アーカイブ移動の失敗と移動からの再開

**種別**: 異常系
**目的**: `state/archive/` への移動に失敗した場合も attempt_count を消費してタスクが残り、再試行時はworktree削除が「既に存在しない = 成功」となるため実質アーカイブ移動から再開されること(冪等)を検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `pulsen add --workflow noop --repo "$TESTHOME/repo"` を実行し `<T4>` として控える。tick を done / pending まで進める | `pulsen ls` で `<T4>` が `done` / `pending` |
| 2 | `chmod 555 "$PULSEN_HOME/state/archive"` を実行する | エラーなく完了する |
| 3 | `pulsen tick; echo $?` を実行する | tick は 0。サマリーに `<T4>` のアーカイブ移動失敗が報告される |
| 4 | `ls "$PULSEN_HOME/worktrees/"` と `ls "$PULSEN_HOME/state/tasks/" "$PULSEN_HOME/state/archive/"` を実行する | worktree `<T4>` は削除済み。タスクファイルは `state/tasks/` にあり `state/archive/` には無い(両方に存在する中間状態が観測されない) |
| 5 | `pulsen ls` と `pulsen show <T4>` を実行する | `done` / `failed`・attempt_count 1。失敗要因としてアーカイブ移動の失敗が表示される |
| 6 | `chmod 755 "$PULSEN_HOME/state/archive"` を実行し、`pulsen tick` を実行する | worktree不在はエラーにならず、アーカイブ移動が成功して終端処理が報告される。タスクファイルが `state/archive/` にある |

**確認ポイント**:

- 手順3〜6のどの時点でも、タスクファイルが `state/tasks/` と `state/archive/` の両方に存在する・どちらにも存在しない状態は観測されない

## TC-08: worktreeが既に存在しない場合は達成済みとして続行

**種別**: 異常系
**目的**: 開発者が手動でworktreeを削除済みでも、クリーンアップはエラーにせずアーカイブ処理へ進むことを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `pulsen add --workflow noop --repo "$TESTHOME/repo"` を実行し `<T5>` として控える。tick を done / pending まで進める | `pulsen ls` で `<T5>` が `done` / `pending` |
| 2 | `git -C "$TESTHOME/repo" worktree remove "$PULSEN_HOME/worktrees/<T5>"` を実行する | worktreeが手動で削除される。`ls "$PULSEN_HOME/worktrees/"` に `<T5>` が無い |
| 3 | `pulsen tick; echo $?` を実行する | 0。エラー報告なしに `<T5>` の終端処理(アーカイブ)が報告される |
| 4 | `ls "$PULSEN_HOME/state/archive/"` を実行する | `<T5>` のタスクファイルがある |

## TC-09: worktree作成前のタスクがクリーンアップに到達

**種別**: 異常系
**目的**: 一度も実行されていない(workspace未確定の)タスクをクリーンアップへ手動遷移させた場合、決定的導出パスへの削除が「既に存在しない = 達成済み」となり、アーカイブされることを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `pulsen add --workflow noop --repo "$TESTHOME/repo"` を実行し `<T6>` として控える(tick は実行しない) | タスクIDが表示される |
| 2 | `pulsen set-status <T6> done` を実行する | 遷移した旨が表示され exit code 0 |
| 3 | `pulsen tick; echo $?` を実行する | 0。`<T6>` の終端処理が報告される |
| 4 | `ls "$PULSEN_HOME/worktrees/"` と `git -C "$TESTHOME/repo" branch --list "pulsen/<T6>"` を実行する | worktreeもブランチ `pulsen/<T6>` も存在しない(作成されないまま終端した) |
| 5 | `pulsen show <T6>` を実行する | アーカイブ済みとして表示され、workspace は「未作成」、attempt 関連の項目は「なし」 |

## TC-10: 成果ブランチの回収(git log・マージ)

**種別**: 正常系
**目的**: 完了タスクのブランチ名を show / ls --all で確認し、通常のgit操作で成果を確認・取り込みできることを検証する

**前提**: TC-01 実施済み(`<T1>` がアーカイブ済みで、コミット `task-output` を持つ)

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `pulsen show <T1>` を実行する | 対象リポジトリのパスとブランチ名 `pulsen/<T1>` が表示される |
| 2 | `git -C "$TESTHOME/repo" log --oneline "pulsen/<T1>"` を実行する | 先頭に `task-output` コミットが表示される |
| 3 | `git -C "$TESTHOME/repo" merge --no-edit "pulsen/<T1>"` を実行する(main 上で) | マージが成功し、`cat "$TESTHOME/repo/memo.txt"` が `done` を表示する |
| 4 | `git -C "$TESTHOME/repo" branch -d "pulsen/<T1>"` を実行する | 取り込み済みのため削除に成功する(削除は開発者の判断・操作) |

**確認ポイント**:

- 手順3〜4の間もツール側の状態は変化しない(`pulsen ls --all` の `<T1>` の行・`state/archive/` の内容は不変。ブランチのライフサイクルにツールは関与しない)

## TC-11: コミットのないブランチ(成果0件)

**種別**: 境界値
**目的**: エージェントがコミットせずに完走した場合、ブランチはベースブランチと同一のまま残り、異常扱いにならないことを検証する

**前提**: TC-07 実施済み(`<T4>` は `noop` ワークフローでアーカイブ済み)

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `git -C "$TESTHOME/repo" branch --list "pulsen/<T4>"` を実行する | ブランチが存在する |
| 2 | `git -C "$TESTHOME/repo" rev-list --count "main..pulsen/<T4>"` を実行する | `0` が表示される(ベースブランチに対する追加コミットがない) |
| 3 | `pulsen show <T4>; echo $?` を実行する | 0。エラーや警告なく通常どおり表示される(回収するものがないだけで異常ではない) |
| 4 | `git -C "$TESTHOME/repo" branch -d "pulsen/<T4>"` を実行する | 不要なブランチを開発者が削除できる |

## TC-12: 回収前にブランチを削除してしまった場合

**種別**: 異常系
**目的**: リポジトリ側でブランチを消した場合、ツールは再生成せず、runディレクトリのログから実行内容を辿れるのみであることを検証する

**前提**: TC-08 実施済み(`<T5>` がアーカイブ済み)

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `git -C "$TESTHOME/repo" branch -D "pulsen/<T5>"` を実行する | ブランチが削除される |
| 2 | `git -C "$TESTHOME/repo" log "pulsen/<T5>"` を実行する | `unknown revision` 等のエラーになる(成果は失われた) |
| 3 | `pulsen show <T5>; echo $?` を実行する | 0。ブランチ名は帳簿の記録として表示されたまま(ブランチの存在検証は行わない) |
| 4 | `ls "$PULSEN_HOME/state/runs/<T5>/attempt-1/"` を実行し、`stdout.log` / `stderr.log` を開く | ログファイルが存在し読める(実行内容を辿る手段は残る) |

**確認ポイント**:

- ブランチやコードを復元するコマンドは提供されていない(`pulsen --help` にそれに相当する操作がない)

## TC-13: lsの既定表示と--allの表示差

**種別**: 正常系
**目的**: 既定の `ls` にはアーカイブ済みが現れず、`ls --all` では現役と区別できる形で表示されることを検証する

**前提**: TC-01〜TC-09 実施済み(アーカイブ済み `<T1>` `<T2>` `<T3>` `<T4>` `<T5>` `<T6>` が存在する)

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `pulsen ls; echo $?` を実行する | 0。アーカイブ済みタスクは1件も表示されない(現役タスクが無ければ空である旨が表示される) |
| 2 | `pulsen ls --all; echo $?` を実行する | 0。`<T1>`〜`<T6>` がアーカイブ済みである印付きの行として表示される |
| 3 | 手順2の出力の `<T3>` の行を確認する | タスクID・ワークフロー名(`noop`)・リポジトリ・ブランチ(`pulsen/<T3>`)・タスクステータス・実行状態が表示されている(ブランチ列はアーカイブ済み行でも表示される) |

## TC-14: アーカイブ済みタスクのshow

**種別**: 正常系
**目的**: `show` が現役 → アーカイブの順でタスクを解決し、アーカイブ済みタスクの詳細を注記付きで表示することを検証する

**前提**: TC-01 実施済み(`<T1>` がアーカイブ済み)

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `pulsen show <T1>; echo $?` を実行する | 0 で終了する |
| 2 | 出力内容を確認する | ワークフロー名 `finish`・対象リポジトリ・ブランチ `pulsen/<T1>`・タスクステータス `done`・attempt_count・runディレクトリへの参照が表示され、アーカイブ済みであること・worktreeは削除済みであることが明示される |
| 3 | 出力のタスクファイルパスを確認する | `state/archive/` 配下のパスが表示される |

## TC-15: アーカイブ後の実行ログ・タスクファイルの直接参照

**種別**: 正常系
**目的**: アーカイブ後もrunディレクトリのログ・終了コードと人間可読なタスクファイルを直接参照できることを検証する

**前提**: TC-01 実施済み(`<T1>` がアーカイブ済み。gc未実施)

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `cat "$PULSEN_HOME/state/runs/<T1>/attempt-1/stdout.log"` を実行する | `agent-output` が表示される(エージェントの標準出力が残っている) |
| 2 | `cat "$PULSEN_HOME/state/runs/<T1>/attempt-1/exit"` を実行する | 終了結果として `0` が記録されている |
| 3 | `state/archive/` 内の `<T1>` のタスクファイルを `cat` で開く | 人間可読な内容で、ワークフロー名・ブランチ・ステータスが読み取れる |

## TC-16: アーカイブ済みタスクへのabort / retry / set-statusの拒否

**種別**: 異常系
**目的**: アーカイブ済みタスクへの状態変更操作がすべて拒否され、帳簿が変化しないことを検証する

**前提**: TC-01 実施済み(`<T1>` がアーカイブ済み)

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `pulsen abort <T1>; echo $?` を実行する | 非0。アーカイブ済みのため操作できない旨が表示される |
| 2 | `pulsen retry <T1>; echo $?` を実行する | 非0。同上 |
| 3 | `pulsen set-status <T1> done; echo $?` を実行する | 非0。同上 |
| 4 | `ls "$PULSEN_HOME/state/archive/" "$PULSEN_HOME/state/tasks/"` を実行する | `<T1>` は `state/archive/` にあるまま。`state/tasks/` へ戻っていない(何も変更されていない) |

**確認ポイント**:

- 同じ作業をやり直したい場合の手段は新規 `add` である(`pulsen add --workflow finish --repo "$TESTHOME/repo"` が新しいタスクID・新しいworktree・新しいブランチで受理されることを確認してもよい)

## TC-17: 存在しないタスクIDの解決

**種別**: 異常系
**目的**: 現役にもアーカイブにも存在しないIDへの照会・操作が「タスク不在」として非0で拒否されることを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `pulsen show no-such-task; echo $?` を実行する | 非0。タスクが存在しない旨が表示される(無言で空を返さない) |
| 2 | `pulsen retry no-such-task; echo $?` を実行する | 非0。同上 |

**確認ポイント**:

- ID解決は `state/tasks/` → `state/archive/` の順で行われる。アーカイブ済みIDの `show` が成功する(TC-14)一方で未知IDはエラーになることが、この順序の外形的な確認になる

## TC-18: run_retention未設定ではgcされない

**種別**: 異常系
**目的**: `run_retention` が未設定なら、保持期間相当を超えた古いattemptがあってもgcが行われないこと(明示オプトイン)を検証する

**前提**: 事前準備の config.yaml のまま(`run_retention` 未定義)。TC-01 実施済み(`<T1>` のrunディレクトリが存在する)

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `grep run_retention "$PULSEN_HOME/config.yaml"; echo $?` を実行する | 非0(設定されていないことの確認) |
| 2 | `<T1>` のattemptを過去化する: `find "$PULSEN_HOME/state/runs/<T1>" -exec touch -t 202501010000 {} +` | エラーなく完了する |
| 3 | `pulsen tick; echo $?` を実行する | 0。サマリーにgcによる削除の報告が含まれない |
| 4 | `ls "$PULSEN_HOME/state/runs/<T1>/"` を実行する | `attempt-1` が残存している |

## TC-19: 保持期間超過attemptのgcと空親ディレクトリの削除

**種別**: 正常系
**目的**: `run_retention` を設定すると、保持期間を超えたattemptのrunディレクトリが削除されてサマリーに報告され、空になった親ディレクトリも削除されることを検証する

**前提**: TC-18 実施直後(`<T1>` のattemptが2025年時刻へ過去化済み)

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `printf 'run_retention: 1h\n' >> "$PULSEN_HOME/config.yaml"` を実行する | エラーなく完了する |
| 2 | `pulsen tick; echo $?` を実行する | 0。サマリーにgcで削除したattempt(`<T1>` の attempt-1)が報告される |
| 3 | `ls "$PULSEN_HOME/state/runs/"` を実行する | `<T1>` のディレクトリ自体が消滅している(attemptがすべて消え、空になった親も削除される) |
| 4 | 過去化していない他のアーカイブ済みタスク(`<T3>` 等)のrunディレクトリを確認する | 最終更新が保持期間(1h)以内のため削除されていない |

**確認ポイント**:

- 手順2は「長期運用後に初めて `run_retention` を設定すると、次のtickで超過分が一括削除される」ことの確認を兼ねる(TC-18で過去化したものがこの1回で消えた)

## TC-20: gcの保護規則(現役参照attempt・stopped全attempt・破損・孤児)

**種別**: 正常系
**目的**: 期間超過でも、現役タスクが現在参照するattemptと stopped タスクの全attempt・パース不能なタスクファイルのattemptは保護され、孤児のrunディレクトリは削除されることを検証する

**前提**: TC-19 実施済み(`run_retention: 1h` 設定済み)

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `pulsen add --workflow hold --repo "$TESTHOME/repo"` を実行し `<T7>` として控える。tick を4回進める | `pulsen ls` で `<T7>` が `waiting` / `pending`(wait ステータスで滞留。attempt-1 が現在参照のattempt) |
| 2 | `pulsen add --workflow fail --repo "$TESTHOME/repo"` を実行し `<T8>` として控える。tick を3回進める | `pulsen ls` で `<T8>` が `stopped`。`notify.log` に行が追加される |
| 3 | 孤児runディレクトリを作る: `mkdir -p "$PULSEN_HOME/state/runs/orphan-1/attempt-1" && echo x > "$PULSEN_HOME/state/runs/orphan-1/attempt-1/stdout.log"` | エラーなく完了する |
| 4 | パース不能タスクを作る: `echo '{broken' > "$PULSEN_HOME/state/tasks/broken-1.json" && mkdir -p "$PULSEN_HOME/state/runs/broken-1/attempt-1" && echo x > "$PULSEN_HOME/state/runs/broken-1/attempt-1/stdout.log"` | エラーなく完了する |
| 5 | 4者すべてを過去化する: `find "$PULSEN_HOME/state/runs/<T7>" "$PULSEN_HOME/state/runs/<T8>" "$PULSEN_HOME/state/runs/orphan-1" "$PULSEN_HOME/state/runs/broken-1" -exec touch -t 202501010000 {} +` | エラーなく完了する |
| 6 | `pulsen tick; echo $?` を実行する | 0。gcで削除されるのは `orphan-1` のみ。`broken-1.json` はパース不能として報告される(書き込まれない) |
| 7 | `ls "$PULSEN_HOME/state/runs/"` を実行する | `<T7>`(現役の現在参照)・`<T8>`(stopped全attempt)・`broken-1`(保護規則を判定できない)は残存し、`orphan-1` は消滅している |
| 8 | 後片付け: `rm "$PULSEN_HOME/state/tasks/broken-1.json"` を実行し、`pulsen tick` を実行する | タスクファイルが消えた `broken-1` は孤児となり、gcで削除される(タスクの決着でgc対象へ戻る規則の確認) |

## TC-21: 保持期間の境界

**種別**: 境界値
**目的**: 最終更新からの経過が保持期間(1h)以内のattemptは残り、超過したattemptだけが削除されることを検証する

**前提**: TC-19 実施済み(`run_retention: 1h`)

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | 孤児runディレクトリを2つ作る: `mkdir -p "$PULSEN_HOME/state/runs/gc-recent/attempt-1" "$PULSEN_HOME/state/runs/gc-old/attempt-1" && echo x > "$PULSEN_HOME/state/runs/gc-recent/attempt-1/stdout.log" && echo x > "$PULSEN_HOME/state/runs/gc-old/attempt-1/stdout.log"` | エラーなく完了する |
| 2 | 片方を30分前、もう片方を2時間前へ過去化する: `find "$PULSEN_HOME/state/runs/gc-recent" -exec touch -t "$(date -v-30M +%Y%m%d%H%M)" {} +` および `find "$PULSEN_HOME/state/runs/gc-old" -exec touch -t "$(date -v-2H +%Y%m%d%H%M)" {} +`(Linux では `date -d '30 minutes ago'` / `date -d '2 hours ago'`) | エラーなく完了する |
| 3 | `pulsen tick` を実行する | サマリーで削除されるのは `gc-old` のみ |
| 4 | `ls "$PULSEN_HOME/state/runs/"` を実行する | `gc-recent` は残存(経過30分 < 1h)、`gc-old` は消滅(経過2時間 > 1h) |
| 5 | 後片付け: `rm -rf "$PULSEN_HOME/state/runs/gc-recent"` | エラーなく完了する |

## TC-22: gcの削除失敗はスキップ・報告され、タスクに影響しない

**種別**: 異常系
**目的**: attemptの削除に失敗してもgcはスキップして報告するのみで、どのタスクのattempt_countも消費せず stopped も発生させず、次のtickが再試行することを検証する

**前提**: TC-19 実施済み(`run_retention: 1h`)

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | 削除できない孤児attemptを作る: `mkdir -p "$PULSEN_HOME/state/runs/gc-locked/attempt-1" && echo x > "$PULSEN_HOME/state/runs/gc-locked/attempt-1/stdout.log" && find "$PULSEN_HOME/state/runs/gc-locked" -exec touch -t 202501010000 {} + && chmod 555 "$PULSEN_HOME/state/runs/gc-locked/attempt-1"` | エラーなく完了する |
| 2 | `pulsen ls` で現役タスク(`<T7>` `<T8>`)の実行状態と attempt_count を控えたうえで、`pulsen tick; echo $?` を実行する | 0。サマリーにgcの削除失敗が報告される |
| 3 | `ls "$PULSEN_HOME/state/runs/"` を実行する | `gc-locked` が残存している |
| 4 | `pulsen ls` を実行する | すべての現役タスクの実行状態・attempt_count が手順2の時点から変化していない(新たな stopped も発生していない) |
| 5 | `chmod 755 "$PULSEN_HOME/state/runs/gc-locked/attempt-1"` を実行し、`pulsen tick` を実行する | 次のtickの再試行で `gc-locked` が削除される |

## TC-23: gc後のshowはログ不在を明示する

**種別**: 異常系
**目的**: runディレクトリがgcで削除済みのタスクの `show` が、タスクファイル由来の情報を表示しつつログ参照の不在を明示し、エラーにならないことを検証する

**前提**: TC-19 実施済み(`<T1>` のrunディレクトリが削除済み)

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `pulsen show <T1>; echo $?` を実行する | 0 で終了する |
| 2 | 出力内容を確認する | ワークフロー名・ブランチ名・タスクステータス等のタスクファイル由来の情報は表示され、runディレクトリ(ログ・exit)への参照は「存在しない」ことが明示される |

## カバレッジ

### ユースケースエラーケース対応表

| ユースケース | エラーケース | 対応TC | 備考 |
|---|---|---|---|
| Tick(手続きB) | worktree削除の失敗(`record_tool_failure(WorktreeRemove)`: attempt_count消費・failed・tasksに残る) | TC-04 | |
| Tick(手続きB) | worktree削除の失敗でリトライ上限(組み込み2)超過 → stopped・通知 | TC-05 | |
| Tick(手続きB) | アーカイブ移動の失敗(`record_tool_failure(ArchiveMove)`) | TC-07 | |
| Tick(手続きB) | アーカイブ移動の失敗で上限超過 → stopped・通知 | 対象外 | TC-05 と同一の `record_tool_failure` → 凍結・通知経路。TC-07 の `chmod` を3tick維持すれば再現できるが、代表ケース(TC-05)で担保する |
| Tick(手続きB) | worktreeが既に存在しない(`AlreadyAbsent` = 成功) | TC-08 | |
| Tick(手続きB) | workspace未記録タスクの決定的導出パスによる削除 | TC-09 | 作成成功直後クラッシュの残骸worktreeの削除は、クラッシュ時点を手動で制御できないため未作成ケースで代表する |
| Tick(手続きB) | worktree削除成功 → 移動失敗 → 次tickは移動から再開(冪等) | TC-07 | |
| Tick(手続きB) | worktree削除とアーカイブの間のtickクラッシュからの再導出 | 対象外 | tickのクラッシュ時点を手動で狙えない。再導出の冪等性は TC-07 手順6 が近似検証する |
| Tick(手続き3・stopped) | stopped タスクへは起動・遷移・削除を行わない(未通知なら再通知のみ) | TC-02 | |
| Tick(手続きE) | `run_retention` 未設定ではgcを行わない | TC-18 | |
| Tick(手続きE) | 非保護・期間超過attemptの削除と空親ディレクトリの削除 | TC-19 | |
| Tick(手続きE) | 現役タスクの現在参照attemptの保護(`ActiveCurrent`) | TC-20 | |
| Tick(手続きE) | stopped タスクの全attempt保護(`AllProtected`) | TC-20 | |
| Tick(手続きE) | アーカイブ済み・孤児runディレクトリは保護しない | TC-19, TC-20 | |
| Tick(手続きE) | パース不能タスクファイルのrunディレクトリは全保護 | TC-20 | |
| Tick(手続きE) | 保持期間の境界(超過のみ削除) | TC-21 | |
| Tick(手続きE) | `delete_attempt` の失敗はスキップ・報告・カウンタ消費なし・次tick再試行 | TC-22 | |
| Tick(手続きE) | `list_runs` の Io 失敗(gcのみ中止) | 対象外 | CLI操作からIoエラーを注入できない |
| Tick(手続きE) | 同一tick内で凍結/アーカイブ/save失敗したタスクの保護分類 | 対象外 | 単一tick内の内部順序であり、外形からタイミングを制御できない |
| ShowTask | タスク不在 | TC-17 | |
| ShowTask | アーカイブ済みの注記付き表示(0) | TC-14 | |
| ShowTask | runディレクトリ不在(gc後)の「存在しない」明示(0) | TC-23 | |
| ListTasks | `--all` による対象集合の拡張とアーカイブ済みの印 | TC-13 | |
| AbortTask | アーカイブ済みへの操作(非0・書き込まない) | TC-16 | |
| RetryTask | アーカイブ済みへの操作(非0・書き込まない) | TC-16 | |
| SetTaskStatus | アーカイブ済みへの操作(非0・書き込まない) | TC-16 | |

### 観点チェックリスト

| 観点 | 対応TC | 対象外の理由 |
|---|---|---|
| 入力バリデーション | TC-17 | 本カテゴリのコマンド入力はタスクIDのみ(`--state` 等の値検証は「状態の確認と追跡」カテゴリの範囲) |
| 境界値 | TC-05, TC-11, TC-21 | |
| 認証・権限 | TC-04, TC-07, TC-22 | CLIに認証はないため、ファイルシステム権限による失敗として読み替えた |
| 空状態・初期状態 | TC-01(手順1), TC-13 | |
| 重複・競合 | 対象外 | ロック競合・tickの多重実行(0 スキップ)は task-execution.md TC-24 / intervention.md TC-25 で代表確認する。二重処理の無害性(冪等)は TC-07 の再開・TC-08 の達成済み扱いが近似確認する |
| 削除・変更の影響 | TC-08, TC-12, TC-16, TC-20(手順8) | 手動削除されたworktree・ブランチ・タスクファイルへの追従を確認 |
| 操作の中断・逸脱 | 対象外 | tickのクラッシュ時点を手動で制御できない。中断からの再導出は TC-07 が代表する |
| 特殊入力 | 対象外 | 本カテゴリの入力はツールが発行するタスクIDのみで自由入力欄がない(不正なIDは TC-17 で確認) |
| UIの状態(エラー後のリトライ・回復) | TC-06, TC-22 | 凍結からの retry 復旧と、gc失敗の次tick再試行 |
