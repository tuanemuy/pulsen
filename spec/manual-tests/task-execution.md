# タスクの登録と自動進行 テスト

## 概要

このドキュメントは、タスク投入から完了までの自動進行(タスク登録 / tickによる自動進行 / 同一対象の並行実行 / 自動リトライ / ポーリング型ワークフロー)に関するマニュアルテストの手順書です。

対応シナリオ: `spec/scenario/task-execution.md`

pulsen は CLI ツールのため、すべての操作はコマンドラインで行い、結果は `pulsen ls` / `pulsen show <task-id>` の出力とグローバルホーム配下のファイル(タスクファイル・runディレクトリ・worktree)で確認する。自動進行の検証は外部スケジューラーを使わず、`pulsen tick` を手動で繰り返し実行して各 tick 後の状態遷移を1ステップずつ観測する。

## 前提条件

### 環境

- pulsen のバイナリがビルド済みで、`pulsen` として実行できる(例: `cargo build --release` 後に `target/release/pulsen` へ PATH を通す)
- POSIX 環境(Linux / macOS)。`sh` と `git` が使える(Windows で実施する場合はスクリプト・kill 手順を同等の操作に読み替える)
- 外部スケジューラー(cron 等)に `tick` が登録されて**いない**こと。定期 tick が走っていると、手動 tick の合間に状態が進んでしまい、1ステップずつの観測ができない
- テスト全体を通して環境変数 `PULSEN_HOME` でテスト専用のグローバルホームを使う:

    ```sh
    export PULSEN_HOME=/tmp/pulsen-test/home
    ```

### テストデータ

テスト領域は `/tmp/pulsen-test/` 配下に集約する。

グローバル設定 `/tmp/pulsen-test/home/config.yaml`。エージェントは LLM なしで再現できるよう任意コマンド用の `shell` を使い(setup.md シナリオ1)、意図的に起動不能なコマンドを指す `broken` をテスト用に加える。`notify_cmd` は通知の発火をファイルで観測するためのログ追記コマンドとする:

```yaml
agents:
  shell:
    cmd: ["sh", "-c", "{input}"]
  broken:
    cmd: ["/tmp/pulsen-test/bin/no-such-binary"]
notify_cmd: ["sh", "-c", "echo \"stopped $TASK_ID $WORKFLOW $TASK_STATUS\" >> /tmp/pulsen-test/notify.log"]
```

ワークフロー定義(`/tmp/pulsen-test/home/workflows/` に配置)。

`pipeline.yaml` — シナリオの `implement` 例(queued → planned → implemented → done)と同じ構造を、LLM なしで再現できる shell コマンドに置き換えたもの:

```yaml
workflow: pipeline
agent: shell
initial: queued
statuses:
  queued:
    prompt: "echo planning && echo plan > plan.txt && git add plan.txt && git commit -m plan"
    next: planned
  planned:
    prompt: "echo impl > impl.txt && git add impl.txt && git commit -m impl"
    next: implemented
  implemented:
    prompt: "echo review > review.txt && git add review.txt && git commit -m review"
    next: done
  done:
    run: cleanup
```

`fail.yaml` — 意図的に失敗し続けるテスト用ワークフロー(リトライ上限超過 → stopped の再現用。`retries: 1` で凍結までの tick 回数を短縮):

```yaml
workflow: fail
agent: shell
initial: work
statuses:
  work:
    prompt: "exit 1"
    retries: 1
    next: done
  done:
    run: cleanup
```

`flaky.yaml` — 1回目だけ失敗し2回目で成功する(一過性失敗の自動リトライ回復の再現用。worktree がリトライ間で引き継がれることを利用したマーカー方式):

```yaml
workflow: flaky
agent: shell
initial: work
statuses:
  work:
    prompt: "if [ -f done.marker ]; then exit 0; else touch done.marker; exit 1; fi"
    next: done
  done:
    run: cleanup
```

`sleeper.yaml` — timeout 超過による kill の再現用:

```yaml
workflow: sleeper
agent: shell
initial: work
statuses:
  work:
    prompt: "sleep 120"
    timeout: 10s
    retries: 1
    next: done
  done:
    run: cleanup
```

`judgefail.yaml` — 判定コマンドがプロトコル外の exit code(1)で終了する(判定失敗 → 再判定上限超過の再現用):

```yaml
workflow: judgefail
agent: shell
initial: work
statuses:
  work:
    prompt: "true"
    judge: ["sh", "-c", "exit 1"]
    next: done
  done:
    run: cleanup
```

`pr-review-watch.yaml` — シナリオのポーリング型ワークフロー例。チェックスクリプトをフラグファイルで制御できるものに置き換え、`fix` を shell のコミット操作に置き換えたもの:

```yaml
workflow: pr-review-watch
agent: shell
initial: watch
statuses:
  watch:
    prompt: "/tmp/pulsen-test/bin/check-reviews.sh"
    judge: ["sh", "-c", "case \"$EXIT_CODE\" in 0) exit 0;; 20) exit 20;; *) exit 10;; esac"]
    timeout: 5m
    next: fix
  fix:
    prompt: "echo fixed >> fixes.log && git add fixes.log && git commit -m fix"
    timeout: 45m
    next: watch
  done:
    run: cleanup
```

`wtloss.yaml` — 進行中の worktree 手動削除の再現用(step1 で成果をコミットし、step2 以降で失敗させる):

```yaml
workflow: wtloss
agent: shell
initial: step1
statuses:
  step1:
    prompt: "echo one > one.txt && git add one.txt && git commit -m step1"
    next: step2
  step2:
    prompt: "true"
    retries: 1
    next: done
  done:
    run: cleanup
```

`broken.yaml` — 起動できないコマンドを実体に持つエージェントを参照する(exit 127 経路の再現用):

```yaml
workflow: broken
agent: broken
initial: work
statuses:
  work:
    prompt: "unused"
    retries: 0
    next: done
  done:
    run: cleanup
```

`longrun.yaml` — 実行中プロセスの外部 kill(プロセス死亡)の再現用:

```yaml
workflow: longrun
agent: shell
initial: work
statuses:
  work:
    prompt: "sleep 600"
    timeout: none
    next: done
  done:
    run: cleanup
```

`fail0.yaml` — `retries: 0` の境界確認用:

```yaml
workflow: fail0
agent: shell
initial: work
statuses:
  work:
    prompt: "exit 1"
    retries: 0
    next: done
  done:
    run: cleanup
```

`exit20.yaml` — 判定コマンド未定義で exit 20 を返す境界確認用:

```yaml
workflow: exit20
agent: shell
initial: work
statuses:
  work:
    prompt: "exit 20"
    retries: 0
    next: done
  done:
    run: cleanup
```

ファイルパス指定用の定義 `/tmp/pulsen-test/draft.yaml`(`workflows/` の外に置く):

```yaml
workflow: my-flow
agent: shell
initial: done
statuses:
  done:
    run: cleanup
```

パース不能な定義 `/tmp/pulsen-test/broken-syntax.yaml`(YAML 構文エラー):

```yaml
workflow: [unclosed
statuses
```

チェックスクリプト `/tmp/pulsen-test/bin/check-reviews.sh`(フラグファイルで結果を制御する。実行権限を付与すること):

```sh
#!/bin/sh
# チェック自体の失敗 → exit 1 / 未対応レビューあり → exit 0 / なし → exit 20
if [ -f /tmp/pulsen-test/check-broken ]; then exit 1; fi
if [ -f /tmp/pulsen-test/review-flag ]; then exit 0; fi
exit 20
```

### 事前準備

1. テスト領域とグローバルホーム(分離ホーム)を初期化する(既存なら削除して作り直す。過去の実行の残留状態を持ち込まないため):

    ```sh
    rm -rf /tmp/pulsen-test
    mkdir -p /tmp/pulsen-test/home/workflows /tmp/pulsen-test/bin
    export PULSEN_HOME=/tmp/pulsen-test/home
    ```

2. 上記テストデータの `config.yaml`・各ワークフローYAML・`draft.yaml`・`broken-syntax.yaml`・`check-reviews.sh` を記載のパスに作成し、スクリプトに実行権限を付ける(`chmod +x /tmp/pulsen-test/bin/check-reviews.sh`)
3. 対象リポジトリを作成する(worktree 内でコミットするため user 設定をローカルに入れる):

    ```sh
    git init -b main /tmp/pulsen-test/repo
    git -C /tmp/pulsen-test/repo config user.name pulsen-test
    git -C /tmp/pulsen-test/repo config user.email pulsen-test@example.com
    git -C /tmp/pulsen-test/repo commit --allow-empty -m init
    ```

4. TC-12(登録後のリポジトリ消失)用に、使い捨てのリポジトリ `/tmp/pulsen-test/repo2` を同じ手順で作成する
5. 通知ログを空にしておく: `: > /tmp/pulsen-test/notify.log`
6. フラグファイル(`/tmp/pulsen-test/review-flag`・`/tmp/pulsen-test/check-broken`)が存在しないことを確認する

### 実行上の注意

- テストケースは**記載順に実行する**前提で書かれている(先行ケースが残す状態を踏まえている)
- tick は1タスクにつき1ステップだけ進める。1回のエージェント実行の消化には「起動 → running 取り込み → 判定 → 遷移」の約4回の tick を要する
- 連続する tick の間は 2〜3 秒あける(ラッパーの pid ファイル書き込みを待つため)。間隔が短いと spawn 確認が KeepWaiting(猶予時間内)となって launching のまま進まず、以降の tick 刻みの期待がすべてずれる
- tick のサマリー出力には、その時点の他の現役タスクの処理も含まれ得る。各テストケースで確認するのは**対象タスクIDに関する行・状態のみ**とする
- stopped で終わるテストケースのタスクは以降も `pulsen ls` に残り続ける(tick は stopped タスクに何もしないため無害)
- `add` の成功時に表示されるタスクIDを各ケースで控え、手順中の `<task-id>` に読み替える。runディレクトリは `$PULSEN_HOME/state/runs/<task-id>/attempt-<n>/`、worktree は `$PULSEN_HOME/worktrees/<task-id>`、ブランチは `pulsen/<task-id>`、タスクファイルは `$PULSEN_HOME/state/tasks/<task-id>.json`

## TC-01: タスク登録(ワークフロー名指定・ベースブランチ省略)

**種別**: 正常系
**目的**: `add` がワークフロー名を解決してタスクを pending で登録し、実行は行わないことを検証する(空状態の表示・`state/` 自動作成を含む)

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | シェルで `pulsen ls` を実行する | タスクが空である旨が表示され、exit code 0(`echo $?` で確認) |
| 2 | `pulsen tick` を実行する | 処理対象がない旨が表示され、exit code 0 |
| 3 | `pulsen add --workflow pipeline --repo /tmp/pulsen-test/repo` を実行する(`--base` 省略) | タスクIDが表示され、解決したワークフロー名 `pipeline` と解決先(`workflows/pipeline.yaml` のパス)が表示されて exit code 0。表示されたIDを T1 として控える |
| 4 | `pulsen ls` を実行する | T1 が1行表示され、ワークフロー名 `pipeline`・タスクステータス `queued`・実行状態 `pending`・attempt_count 0 |
| 5 | `pulsen show <T1>` を実行する | ベースブランチが `main`(HEAD から解決)、タスクステータス `queued`、実行状態 `pending`、attempt_count / judge_attempt_count / spawn_fail_count がすべて 0、workspace は「未作成」、attempt 関連の項目は「なし」、スナップショットの保存先パスが表示される |
| 6 | `ls /tmp/pulsen-test/home/worktrees/ 2>/dev/null; ls /tmp/pulsen-test/home/state/runs/ 2>/dev/null` を実行する | T1 の worktree・runディレクトリは存在しない(`add` は実行を開始しない。次の tick に委ねられる) |
| 7 | `ls /tmp/pulsen-test/home/state/tasks/` を実行する | `<T1>.json` が作成されている(`state/` 配下は自動作成される) |
| 8 | `pulsen set-status <T1> done` を実行する(次ケース以降の tick で片付けるための予約) | `queued` から `done` へ遷移した旨が表示され exit code 0 |

**確認ポイント**:

- 手順3の後、次回の定期 tick を待たず即座に開始したい場合の合成は `pulsen add ... && pulsen tick` である(登録専用の即実行機構はない)ことを README 等の利用手順と突き合わせて確認する

## TC-02: タスク登録(ファイルパス指定)と即時アーカイブ

**種別**: 正常系
**目的**: `workflows/` 外のYAMLをファイルパスで指定して登録でき、ワークフロー名が `workflow:` キーから決まることを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `pulsen add --workflow /tmp/pulsen-test/draft.yaml --repo /tmp/pulsen-test/repo` を実行する | タスクIDが表示され exit code 0。ワークフロー名はファイル名(`draft`)ではなく `workflow:` キーの `my-flow` になる。IDを T2 として控える |
| 2 | `pulsen ls` を実行する | T2 の行にワークフロー名 `my-flow`・タスクステータス `done`(この定義の `initial`)・実行状態 `pending` が表示される |
| 3 | `pulsen tick` を実行する | サマリーに T2 のアーカイブ(worktree 未作成のため削除は達成済み扱いでアーカイブのみ)と、TC-01 で `done` に遷移済みの T1 のアーカイブが記録され exit code 0 |
| 4 | `pulsen ls` を実行する | T1・T2 は表示されない(走査対象から外れた) |
| 5 | `pulsen ls --all` を実行する | T1・T2 がアーカイブ済みの印付きで表示される |
| 6 | `pulsen show <T2>` を実行する | アーカイブ済みであることが注記された詳細が表示され、workspace は「未作成」(一度も実行されておらず worktree が作られていないため)と表示されて exit code 0 |

## TC-03: tickによる自動進行(登録から完了まで)

**種別**: 正常系
**目的**: 登録したタスクが手動 tick の繰り返しだけでワークフロー定義のステータス遷移(queued → planned → implemented → done)を最後まで進み、終端でアーカイブされることを検証する。スナップショットにより元YAML編集の影響を受けないことも確認する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `pulsen add --workflow pipeline --repo /tmp/pulsen-test/repo` を実行する | タスクIDが表示され exit code 0。IDを T3 として控える |
| 2 | `workflows/pipeline.yaml` の `queued` の prompt を `"echo edited-should-not-appear"` に書き換える(登録後の元YAML編集) | ファイル編集のみ。エラーなし |
| 3 | `pulsen tick`(1回目)を実行する | サマリーに T3 の起動が記録される。`pulsen show <T3>` で: workspace が `worktrees/<T3>`・ブランチ `pulsen/<T3>` で確定し、実行状態は `launching`、attempt 番号 1・runディレクトリ `state/runs/<T3>/attempt-1` が記録されている。`ls /tmp/pulsen-test/home/worktrees/<T3>` で worktree が存在する |
| 4 | `pulsen tick`(2回目)を実行する | `pulsen show <T3>` で実行状態が `running` になり、PID・kill同定子・starttime が取り込まれている |
| 5 | `pulsen tick`(3回目)を実行する | エージェント(echo コマンド)は既に終了しているため判定が行われ、`pulsen show <T3>` で実行状態が `completed`。タスクステータスはまだ `queued`(遷移は次の tick) |
| 6 | `pulsen tick`(4回目)を実行する | サマリーに遷移が記録され、`pulsen ls` で T3 のタスクステータスが `planned`・実行状態 `pending` になっている(attempt_count は 0 のまま) |
| 7 | `cat /tmp/pulsen-test/home/state/runs/<T3>/attempt-1/stdout.log` と同ディレクトリの `exit` を確認する | `stdout.log` に `planning` が含まれ(手順2の編集後の `edited-should-not-appear` では**ない** = スナップショットが使われた)、`exit` の内容は `0`。`pid` / `starttime` / `stderr.log` も存在する |
| 8 | `planned` について同様に `pulsen tick` を4回繰り返す(起動 → running → completed → 遷移) | 各 tick 後の `ls` / `show` が手順3〜6と同じ順で進み、attempt 番号は 2(runディレクトリは `attempt-2`)。遷移後はタスクステータス `implemented`・実行状態 `pending` |
| 9 | `implemented` について同様に `pulsen tick` を4回繰り返す | attempt 番号 3 で実行され、遷移後はタスクステータス `done`・実行状態 `pending` |
| 10 | `pulsen tick` を実行する(クリーンアップ) | サマリーに T3 のアーカイブが記録される。`ls /tmp/pulsen-test/home/worktrees/` に `<T3>` が存在しない(worktree 削除)。`pulsen ls` に T3 は表示されず、`pulsen ls --all` にアーカイブ済みとして表示される |
| 11 | `git -C /tmp/pulsen-test/repo branch --list 'pulsen/*'` と `git -C /tmp/pulsen-test/repo log --oneline pulsen/<T3>` を実行する | ブランチ `pulsen/<T3>` が残っており、`plan` / `impl` / `review` の3コミットが積まれている(成果物はブランチとして回収可能) |
| 12 | `workflows/pipeline.yaml` の prompt を元の内容(`"echo planning && ..."`)に戻す | 以降のケースが元の定義を使える |

**確認ポイント**:

- 各実行は独立した新規セッションであり、ステータス間で受け渡されるのは worktree の成果のみ(plan.txt を前提に impl.txt が同じ worktree に積まれている)
- runディレクトリ(attempt-1〜3)はアーカイブ後も削除されずに残っている

## TC-04: 同一対象での複数タスクの並行実行

**種別**: 正常系
**目的**: 同一リポジトリ・同一ベースブランチ・同一ワークフローの二重登録が拒否されず、各タスクが独立した worktree・ブランチで干渉なく完走することを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `pulsen add --workflow pipeline --repo /tmp/pulsen-test/repo` を2回実行する | 2回とも exit code 0 で成功し、異なるタスクID(T4a・T4b と控える)が発行される(重複排除されない) |
| 2 | `pulsen ls` を実行する | T4a・T4b の2行が表示され、いずれも `queued` / `pending` |
| 3 | `pulsen tick`(1回目)を実行する | サマリーに**両方**の起動が記録される(並列度制御は行われない)。`ls /tmp/pulsen-test/home/worktrees/` に `<T4a>` と `<T4b>` の別々のディレクトリが存在する |
| 4 | 両タスクがアーカイブされるまで `pulsen tick` を繰り返す(13回程度)。途中で `pulsen ls` を数回確認する | 各 tick で両タスクが1ステップずつ独立に進み、`ls` で両者の進行が並んで確認できる。最終的に両方が `ls` から消え、`ls --all` にアーカイブ済みとして表示される |
| 5 | `git -C /tmp/pulsen-test/repo branch --list 'pulsen/*'` を実行する | `pulsen/<T4a>` と `pulsen/<T4b>` の両ブランチが残っており、それぞれに3コミットが積まれている |

## TC-05: 一過性失敗の自動リトライによる回復

**種別**: 正常系
**目的**: エージェント実行の失敗(exit code 非0)が failed と分類され、人手なしで同一ステータスが再実行されて回復し、completed でカウンタがリセットされることを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `pulsen add --workflow flaky --repo /tmp/pulsen-test/repo` を実行し、IDを T5 として控える | 登録成功 |
| 2 | `pulsen tick` を3回実行する(起動 → running → 判定) | 3回目の tick 後、`pulsen show <T5>` で実行状態 `failed`・attempt_count 1・タスクステータスは `work` のまま。`state/runs/<T5>/attempt-1/exit` の内容は `1` |
| 3 | `pulsen tick`(4回目)を実行する | 同じステータス `work` が新しい attempt 番号 2 で再起動される(`show` で実行状態 `launching`・runディレクトリが `attempt-2`)。worktree は前回の状態を引き継いでいる(`done.marker` が残っている) |
| 4 | `pulsen tick` を2回実行する(running → 判定) | 2回目の実行は marker を検出して exit 0 → `show` で実行状態 `completed`、attempt_count が **0 にリセット**されている |
| 5 | `pulsen tick` を2回実行する(遷移 → クリーンアップ) | `done` へ遷移し、アーカイブされる。`ls --all` で確認できる |
| 6 | `cat /tmp/pulsen-test/notify.log` を実行する | 通知は1行も増えていない(自動リトライで回復した失敗は通知されない) |

**確認ポイント**:

- 手順2〜4 を通じてタスクステータスは `work` から一切変化していない(失敗の履歴は実行状態と attempt_count にのみ表れる)

## TC-06: ポーリング型ワークフロー(skipped による周回)

**種別**: 正常系
**目的**: 判定コマンドが 20 を返すと skipped となり、タスクステータスを維持したまま pending に戻って次の tick で新しい attempt が起動されること(attempt_count 非消費・通知なし)を検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `/tmp/pulsen-test/review-flag` と `/tmp/pulsen-test/check-broken` が存在しないことを確認し、`pulsen add --workflow pr-review-watch --repo /tmp/pulsen-test/repo` を実行する。IDを T6 として控える | 登録成功。`ls` で `watch` / `pending` |
| 2 | `pulsen tick` を2回実行する(起動 → running) | attempt 1 でチェックスクリプトが起動され、`show` で実行状態 `running` |
| 3 | `pulsen tick`(3回目)を実行する | チェックは exit 20 で終了済みのため判定コマンドが 20 を返し **skipped** となる。tick サマリーに skipped で実行待ちに戻したタスクとして記録され、`show` で: タスクステータス `watch` のまま・実行状態 `pending`・attempt_count **0**(消費されない) |
| 4 | `pulsen tick`(4回目)を実行する | 同じ `watch` が新しい attempt 番号 2 で再起動される。`ls /tmp/pulsen-test/home/state/runs/<T6>/` に `attempt-1` と `attempt-2` の両方が存在する |
| 5 | `cat /tmp/pulsen-test/notify.log` を実行する | 通知は増えていない(skipped は通知されない) |

## TC-07: ポーリング型ワークフロー(completed による循環と停止・片付け)

**種別**: 正常系
**目的**: 対応が必要なときだけ `fix` へ遷移し、`next: watch` の循環で戻ること、循環は自動では止まらず `abort` + `set-status` で停止・片付けできることを検証する

前提: TC-06 の T6 が `watch` の周回中であること。

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `touch /tmp/pulsen-test/review-flag` を実行し、`pulsen tick` を2回実行する(attempt 2 の spawn確認 → 判定) | TC-06 手順4 で起動済みの attempt 2 のチェックはフラグ作成前に exit 20 で終了しているため、現在の周回は **skipped** で終わる(タスクステータス `watch` のまま `pending` 復帰) |
| 2 | `pulsen tick` を4回実行する(attempt 3 の起動 → running → 判定 → 遷移。completed が観測されるまで) | フラグを見た新しい周回(attempt 3)のチェックが exit 0 で終了し、3回目の tick の判定で **completed** となる。4回目の tick でタスクステータスが `fix` へ遷移して `pending` になる |
| 3 | `pulsen tick` を4回実行する(fix の起動 → running → 判定 → 遷移) | `fix`(コミット操作)が同じ worktree で実行されて completed となり、`next: watch` により**タスクステータスが `watch` に戻る**(循環) |
| 4 | `rm /tmp/pulsen-test/review-flag` を実行し、`pulsen tick` を3回実行する | 再び skipped の周回に入る(タスクステータス `watch`・pending 復帰) |
| 5 | `git -C /tmp/pulsen-test/repo log --oneline pulsen/<T6>` を実行する | `fix` のコミットがブランチに積まれている(レビュー対応が自動で積まれた状態) |
| 6 | `pulsen abort <T6>` を実行する | stopped を記録した旨(kill の有無を含む)が表示され exit code 0。`ls` で実行状態 `stopped` |
| 7 | `grep <T6> /tmp/pulsen-test/notify.log` を実行する | `stopped <T6> pr-review-watch watch` の行が追記されている(abort でも通知が発火する) |
| 8 | `pulsen set-status <T6> done` を実行し、`pulsen tick` を実行する | `done` へ遷移して pending となり、tick で worktree が削除されてアーカイブされる。`ls --all` で確認でき、ブランチ `pulsen/<T6>` は残っている |

**確認ポイント**:

- フラグ作成が効くのはフラグ作成後に起動された attempt からである(チェックスクリプトは起動時に一度だけ実行される)。フラグ作成時点で進行中だった周回は skipped で終わり、completed の観測はその次の周回になる
- 手順1〜4 の間、attempt_count は completed / skipped の確定のたびに 0 のままであり、周回の蓄積で凍結しない

## TC-08: 存在しないワークフロー名での登録

**種別**: 異常系
**目的**: 解決できないワークフロー名の登録がエラーで失敗し、タスクが作られないことを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | 事前に `pulsen ls` の行数を控える | 現状確認 |
| 2 | `pulsen add --workflow nosuch --repo /tmp/pulsen-test/repo` を実行する | 非0 で終了し、解決を試みたパス(`/tmp/pulsen-test/home/workflows/nosuch.yaml` の絶対パス)がエラーメッセージに含まれる |
| 3 | `pulsen ls` と `ls /tmp/pulsen-test/home/state/tasks/` を実行する | タスクは増えていない(タスクファイルも作られていない) |

## TC-09: パース不能なワークフローYAMLでの登録

**種別**: 異常系
**目的**: 構文不正なYAMLのファイルパス指定登録がエラー位置を示して失敗し、タスクが作られないことを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `pulsen add --workflow /tmp/pulsen-test/broken-syntax.yaml --repo /tmp/pulsen-test/repo` を実行する | 非0 で終了し、パースエラーの位置・原因が表示される |
| 2 | 存在しないパス `pulsen add --workflow /tmp/pulsen-test/no-such.yaml --repo /tmp/pulsen-test/repo` を実行する | 非0 で終了し、解決を試みたパスが表示される |
| 3 | `pulsen ls` を実行する | タスクは増えていない |

## TC-10: 存在しないリポジトリパスでの登録

**種別**: 異常系
**目的**: 対象リポジトリの不正が登録時の検証で検知され、タスクが作られないことを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `pulsen add --workflow pipeline --repo /tmp/pulsen-test/no-such-repo` を実行する | 非0 で終了し、リポジトリが存在しない旨が表示される |
| 2 | `mkdir -p /tmp/pulsen-test/not-a-repo` を実行し、`pulsen add --workflow pipeline --repo /tmp/pulsen-test/not-a-repo` を実行する | 非0 で終了する(git リポジトリではない) |
| 3 | `pulsen ls` を実行する | タスクは増えていない |

## TC-11: 存在しないベースブランチ・HEAD解決不能での登録

**種別**: 異常系
**目的**: ベースブランチの不正・HEAD からの解決不能が登録時に検知されることを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `pulsen add --workflow pipeline --repo /tmp/pulsen-test/repo --base no-such-branch` を実行する | 非0 で終了し、ブランチが存在しない旨が表示される。タスクは作られない |
| 2 | `git -C /tmp/pulsen-test/repo checkout --detach` を実行し、`pulsen add --workflow pipeline --repo /tmp/pulsen-test/repo` を実行する(`--base` 省略) | 非0 で終了し、`--base` の明示指定を案内するメッセージが表示される |
| 3 | `git -C /tmp/pulsen-test/repo checkout main` で元に戻し、`pulsen ls` を実行する | リポジトリは復旧し、タスクは増えていない |

## TC-12: worktree作成失敗の自動リトライと凍結(登録後のリポジトリ消失)

**種別**: 異常系
**目的**: 登録後にリポジトリが失われた場合、worktree 作成の失敗が failed 相当として attempt_count を消費して再試行され、リトライ上限超過で stopped・通知されることを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `pulsen add --workflow fail --repo /tmp/pulsen-test/repo2` を実行し、IDを T12 として控える | 登録成功(登録時点ではリポジトリが存在するため検証を通る) |
| 2 | `mv /tmp/pulsen-test/repo2 /tmp/pulsen-test/repo2.gone` を実行する | リポジトリ消失を再現 |
| 3 | `pulsen tick`(1回目)を実行する | worktree 作成が失敗し、`pulsen show <T12>` で: 実行状態 `failed`・attempt_count 1・直近の失敗要因に worktree 作成の失敗メッセージが記録されている。attempt(runディレクトリ)は作られない |
| 4 | `pulsen tick`(2回目)を実行する | 再試行も失敗し、attempt_count 2 > 上限 1 のため実行状態が `stopped` になる |
| 5 | `grep <T12> /tmp/pulsen-test/notify.log` を実行する | `stopped <T12> fail work` の通知行が追記されている |
| 6 | `pulsen tick` をもう1回実行する | T12 に対して起動・遷移は行われない(状態が変化しない。凍結) |

## TC-13: 実行失敗の連続によるリトライ上限超過と凍結

**種別**: 異常系
**目的**: exit code 非0 の連続失敗が自動リトライされ、リトライ上限(`retries: 1`)の**等号では凍結せず超過で stopped** となり通知されること、タスクステータスが終始変化しないことを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `pulsen add --workflow fail --repo /tmp/pulsen-test/repo` を実行し、IDを T13 として控える | 登録成功 |
| 2 | `pulsen tick` を3回実行する(起動 → running → 判定) | `show` で: 実行状態 `failed`・attempt_count 1(= 上限 1。**等号のため凍結しない**)・タスクステータス `work` のまま。`attempt-1/exit` の内容は `1` |
| 3 | `pulsen tick` を3回実行する(再起動 → running → 判定) | attempt 番号 2 で再実行されて再び失敗し、attempt_count 2 > 1 のため実行状態が `stopped` になる |
| 4 | `pulsen show <T13>` を実行する | 凍結要因(リトライ上限超過。直前実行の終了情報・最終出力への参照)と notified_at が表示される。タスクステータスは `work` のまま |
| 5 | `grep <T13> /tmp/pulsen-test/notify.log` を実行する | 通知行が追記されている |
| 6 | `ls /tmp/pulsen-test/home/worktrees/<T13>` と `git -C /tmp/pulsen-test/repo branch --list "pulsen/<T13>"` を実行する | worktree は保持されたまま、ブランチも残っている(凍結タスクの調査材料は自動削除されない) |
| 7 | `pulsen tick` をもう1回実行する | T13 は再起動されない(以降のtickは実行の起動・遷移を行わない) |

## TC-14: timeout超過によるkillとリトライ・凍結

**種別**: 異常系
**目的**: ステータスの timeout(10s)を超えた実行が kill されて failed となり自動リトライ経路に入ること、および実行中の連続 tick が状態を変えないこと(冪等性)を検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `pulsen add --workflow sleeper --repo /tmp/pulsen-test/repo` を実行し、IDを T14 として控える | 登録成功 |
| 2 | `pulsen tick` を2回実行する(起動 → running) | `show` で実行状態 `running`(`sleep 120` が実行中) |
| 3 | 間を置かず `pulsen tick` を続けて2回実行する | timeout(starttime 起点)未超過のため何も変化しない(`show` の内容が同一。tick の冪等性) |
| 4 | 起動から10秒以上待ってから `pulsen tick` を実行する | プロセスグループ相当の単位が kill され、`show` で: 実行状態 `failed`・attempt_count 1(= 上限のため凍結しない)。`ps` 等で sleep プロセスが残っていないことを確認できる |
| 5 | `pulsen tick` を2回実行し(再起動 → running)、10秒以上待って `pulsen tick` を実行する | attempt 2 も timeout kill され、attempt_count 2 > 1 のため `stopped` になる |
| 6 | `grep <T14> /tmp/pulsen-test/notify.log` と `ls /tmp/pulsen-test/home/worktrees/<T14>` を実行する | 通知行が追記され、worktree は保持されている |

## TC-15: 判定コマンドの判定失敗と再判定上限超過による凍結

**種別**: 異常系
**目的**: 判定コマンドが 0 / 10 / 20 以外で終了すると「判定自体が壊れた」として再判定され、再判定上限(デフォルト3)の超過で**リトライ(再実行)なしに** stopped・通知されることを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `pulsen add --workflow judgefail --repo /tmp/pulsen-test/repo` を実行し、IDを T15 として控える | 登録成功 |
| 2 | `pulsen tick` を2回実行する(起動 → running) | エージェント(`true`)は exit 0 で即終了している |
| 3 | `pulsen tick`(判定1回目)を実行する | 判定コマンドが exit 1(プロトコル外)で終了し、`show` で: judge_attempt_count 1・実行状態は `running` のまま・attempt_count 0・直近の失敗要因に判定失敗が記録される |
| 4 | `pulsen tick` を2回実行する(判定2〜3回目) | judge_attempt_count が 2 → 3 と増える(3 = 上限。等号のため凍結せず、次の tick で再判定される) |
| 5 | `pulsen tick`(判定4回目)を実行する | judge_attempt_count 4 > 3 で実行状態が `stopped` になる |
| 6 | `pulsen show <T15>` と `ls /tmp/pulsen-test/home/state/runs/<T15>/` を実行する | attempt_count は 0 のままで、runディレクトリは `attempt-1` のみ(エージェントの再実行は一度も行われていない) |
| 7 | `grep <T15> /tmp/pulsen-test/notify.log` を実行する | 通知行が追記されている |

**確認ポイント**:

- 判定コマンドの timeout 超過・起動不能も同じ判定失敗経路(judge_attempt_count 加算)であり、本ケースはプロトコル外 exit code で経路を代表確認している

## TC-16: spawn失敗の恒久化(登録後の設定破壊)による凍結

**種別**: 異常系
**目的**: 登録後の config.yaml 編集でエージェント定義が解決できなくなると、起動のたびに同期的な spawn 失敗(attempt 採番なし・実行状態不変)となり、連続 spawn 失敗の上限(デフォルト3)超過で stopped・通知されることを検証する

前提: 他に `pending` / `failed` の現役タスクが残っていないこと(残っていると同様に spawn 失敗を蓄積してしまう)。

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `pulsen add --workflow pipeline --repo /tmp/pulsen-test/repo` を実行し、IDを T16 として控える | 登録成功(この時点の config は正常なので検証を通る) |
| 2 | `config.yaml` の `shell:` キーを `shellx:` に書き換える(登録後の設定破壊) | ファイル編集のみ |
| 3 | `pulsen tick`(1回目)を実行する | worktree は作成されるが、テンプレート展開(エージェント `shell` の解決)に失敗する。`show` で: 実行状態は **`pending` のまま**・spawn_fail_count 1・attempt は「なし」(採番されない)・直近の失敗要因に展開エラーが記録される。`state/runs/<T16>/` は作られない |
| 4 | `pulsen tick` を2回実行する(2〜3回目) | spawn_fail_count が 2 → 3 と増える(3 = 上限。等号のため凍結しない) |
| 5 | `pulsen tick`(4回目)を実行する | spawn_fail_count 4 > 3 で実行状態が `stopped` になる |
| 6 | `grep <T16> /tmp/pulsen-test/notify.log` を実行する | 通知行が追記されている(設定の不具合は再試行では解決しないため凍結・エスカレーションされる) |
| 7 | `config.yaml` の `shellx:` を `shell:` に戻す | 以降のケースが正常な設定を使える(グローバル設定はスナップショットされないため、修正は既存タスクの以後の実行に反映される) |

**確認ポイント**:

- attempt_count は 0 のまま(spawn_fail_count は attempt_count と独立のカウンタ)

## TC-17: エージェント実体の起動不能(exit 127)は通常のfailed経路

**種別**: 異常系
**目的**: テンプレートは展開できるがコマンド実体を起動できない場合、spawn 失敗ではなくラッパーが exit ファイル(127 等)を書く通常の実行失敗(failed)として現れることを検証する(TC-16 との経路の違い)

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `pulsen add --workflow broken --repo /tmp/pulsen-test/repo` を実行し、IDを T17 として控える | 登録成功(エージェント定義の実体が存在するかは検証されない) |
| 2 | `pulsen tick` を2回実行する(起動 → running) | attempt 1 が採番され、ラッパーの pid・starttime が取り込まれて `running` になる |
| 3 | `pulsen tick`(判定)を実行する | `cat /tmp/pulsen-test/home/state/runs/<T17>/attempt-1/exit` の内容が `127`(コマンド不在の符号化)で、デフォルト判定により failed。`retries: 0` のため attempt_count 1 > 0 で即 `stopped` になる |
| 4 | `pulsen show <T17>` を実行する | spawn_fail_count は **0**(spawn 失敗ではない)。凍結要因はリトライ上限超過であり、直前実行の終了情報(exit 127)への参照が表示される |
| 5 | `grep <T17> /tmp/pulsen-test/notify.log` を実行する | 通知行が追記されている |

## TC-18: 進行中worktreeの手動削除と復旧不能タスクの片付け

**種別**: 異常系
**目的**: 進行中に worktree が消失したタスクは(ツールは worktree を再作成しないため)失敗を繰り返して stopped に至ること、`set-status` でクリーンアップへ手動遷移させれば削除は達成済み扱いでアーカイブでき、ブランチの成果は回収できることを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `pulsen add --workflow wtloss --repo /tmp/pulsen-test/repo` を実行し、IDを T18 として控える | 登録成功 |
| 2 | `pulsen tick` を4回実行する(step1 の起動 → running → completed → step2 へ遷移) | `ls` で T18 がタスクステータス `step2`・実行状態 `pending`。ブランチに `step1` のコミットが積まれている |
| 3 | `rm -rf /tmp/pulsen-test/home/worktrees/<T18>` を実行する(worktree の手動削除) | worktree ディレクトリが消える |
| 4 | `pulsen tick` を3回実行する(step2 の起動 → running → 判定) | 実行は存在しないワークスペースを対象に失敗し(exit は 126/127 等の非0。現れ方は環境に依る)、`show` で: 実行状態 `failed`・attempt_count 1。**worktree は再作成されない** |
| 5 | `pulsen tick` を3回実行する(再試行) | 再び失敗し、attempt_count 2 > 上限 1 で `stopped` になる。`grep <T18> /tmp/pulsen-test/notify.log` で通知行を確認できる |
| 6 | `pulsen set-status <T18> done` を実行し、`pulsen tick` を実行する | `done` へ遷移して pending となり、tick の終端処理では worktree が既に存在しないため削除は達成済み扱いで、タスクがアーカイブされる(`ls --all` で確認) |
| 7 | `git -C /tmp/pulsen-test/repo log --oneline pulsen/<T18>` を実行する | `step1` のコミットが残っており、コミット済みの成果はブランチから回収できる |

## TC-19: ポーリングのチェック一過性失敗と連続失敗カウンタのリセット

**種別**: 異常系
**目的**: チェックスクリプトの一過性失敗(判定コマンドが 10 を返す)が attempt_count を消費して再実行され、その後の skipped 確定でカウンタがリセットされること(散発失敗の蓄積で凍結しない)を検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `touch /tmp/pulsen-test/check-broken` を実行し、`pulsen add --workflow pr-review-watch --repo /tmp/pulsen-test/repo` を実行する。IDを T19 として控える | 登録成功 |
| 2 | `pulsen tick` を3回実行する(起動 → running → 判定) | チェックが exit 1 で終了し、判定コマンドが 10 へ写像するため **failed**。`show` で: 実行状態 `failed`・attempt_count 1・タスクステータス `watch` のまま |
| 3 | `rm /tmp/pulsen-test/check-broken` を実行する(一過性障害の解消) | フラグ削除 |
| 4 | `pulsen tick` を3回実行する(再起動 → running → 判定) | チェックが exit 20 で終了して **skipped** となり、`show` で: 実行状態 `pending`・attempt_count が **0 にリセット**されている |
| 5 | `cat /tmp/pulsen-test/notify.log` を実行する | T19 の通知は発生していない(凍結に至っていない) |
| 6 | `pulsen abort <T19>` → `pulsen set-status <T19> done` → `pulsen tick` の順で実行する(片付け) | stopped(通知1行追記)→ done へ遷移 → アーカイブされる |

## TC-20: パース不能なタスクファイルのスキップと他タスクの続行

**種別**: 異常系
**目的**: tick が読めないタスクファイルをスキップして報告し(書き込み・stopped化・通知をしない)、他のタスクの進行に影響しないこと、`ls` が破損を報告することを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `pulsen add --workflow pipeline --repo /tmp/pulsen-test/repo` を実行し、IDを T20 として控える。続けて `pulsen add --workflow /tmp/pulsen-test/draft.yaml --repo /tmp/pulsen-test/repo` を実行し、IDを T20h として控える | 2タスクが登録される |
| 2 | `cp /tmp/pulsen-test/home/state/tasks/<T20>.json /tmp/pulsen-test/t20.bak` の後、`echo broken > /tmp/pulsen-test/home/state/tasks/<T20>.json` を実行する | T20 のタスクファイルが破損状態になる |
| 3 | `pulsen tick` を実行する | サマリーの「スキップ」の見出しに T20 のファイルパスが報告される。T20h は通常どおり処理されてアーカイブされる(他タスクへの影響なし)。tick の exit code は 0 |
| 4 | `cat /tmp/pulsen-test/home/state/tasks/<T20>.json` を実行する | 内容は `broken` のまま(破損ファイルへの書き込みは行われない) |
| 5 | `pulsen ls` を実行する | パース不能なタスクファイルの存在(パスと読めない旨)が報告される(修復の入口) |
| 6 | `cat /tmp/pulsen-test/notify.log` を実行する | T20 に関する通知は発生していない(stopped化されない) |
| 7 | `cp /tmp/pulsen-test/t20.bak /tmp/pulsen-test/home/state/tasks/<T20>.json` で復元し、`pulsen set-status <T20> done` → `pulsen tick` を実行する(片付け) | 復元後は通常どおり操作でき、アーカイブされる |

## TC-21: exit記録なしのプロセス死亡の検出と自動リトライ

**種別**: 異常系
**目的**: exit ファイルがないままプロセスが死亡した実行(OOM・マシン再起動等の代替として外部からの kill -9 で再現)を tick が failed と分類し、自動リトライで再起動することを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `pulsen add --workflow longrun --repo /tmp/pulsen-test/repo` を実行し、IDを T21 として控える | 登録成功 |
| 2 | `pulsen tick` を2回実行する(起動 → running) | `pulsen show <T21>` で実行状態 `running` と PID が表示される |
| 3 | 表示された PID を使い `kill -9 -- -<PID>` を実行する(プロセスグループごと強制終了) | ラッパーと `sleep` が exit ファイルを書けないまま死亡する |
| 4 | `pulsen tick` を実行する | 「exit なし・プロセス死亡」として failed に分類され、`show` で: 実行状態 `failed`・attempt_count 1。`state/runs/<T21>/attempt-1/` に `exit` ファイルが**存在しない** |
| 5 | `pulsen tick` を実行する | 新しい attempt 番号 2 で自動的に再起動される(利用者の操作は不要) |
| 6 | `pulsen abort <T21>` を実行する(片付け) | 実行中プロセスが kill され(kill を実行した旨の表示)、stopped が記録されて通知行が追記される |

## TC-22: `retries: 0` での初回失敗の即時凍結

**種別**: 境界値
**目的**: リトライ上限 0 のステータスでは初回の失敗で即 stopped(failed を経由した再実行なし)となることを検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `pulsen add --workflow fail0 --repo /tmp/pulsen-test/repo` を実行し、IDを T22 として控える | 登録成功(`retries: 0` は登録時に受理される) |
| 2 | `pulsen tick` を3回実行する(起動 → running → 判定) | 判定で attempt_count 1 > 0 となり、failed での再実行を経ずに実行状態が `stopped` になる |
| 3 | `ls /tmp/pulsen-test/home/state/runs/<T22>/` と `grep <T22> /tmp/pulsen-test/notify.log` を実行する | runディレクトリは `attempt-1` のみ(再実行なし)。通知行が追記されている |

## TC-23: 判定コマンド未定義での exit 20(failed 分類)

**種別**: 境界値
**目的**: デフォルト判定は 0 / 非0 の2値であり、エージェントの exit 20 は skipped ではなく failed に分類されること(skipped は判定コマンドでのみ表現できる)を検証する

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `pulsen add --workflow exit20 --repo /tmp/pulsen-test/repo` を実行し、IDを T23 として控える | 登録成功 |
| 2 | `pulsen tick` を3回実行する(起動 → running → 判定) | `attempt-1/exit` の内容は `20`。デフォルト判定で **failed** に分類され(pending への skipped 復帰は起きない)、attempt_count 1 が消費される。`retries: 0` のため `stopped` になる |
| 3 | `grep <T23> /tmp/pulsen-test/notify.log` を実行する | 通知行が追記されている(skipped なら通知されないはずの経路と区別できる) |

## TC-24: ロック競合(tick の 0 スキップと add の非0拒否)

**種別**: 異常系
**目的**: tick と状態変更系 CLI が同一の排他ロックを使い、競合した tick は状態を変更せず exit code **0** でスキップし(cron 運用でアラートにしないための exit code 規約の唯一の例外)、競合した `add` は非0で終了してタスクを作らないことを検証する。ロック保持時間は intervention.md TC-25 と同じ手法(長時間かかる notify_cmd)で決定的に作る

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `config.yaml` の notify_cmd を `["sh", "-c", "sleep 30"]` に変更する(通知はロック保持中に同期実行されるため、約 30 秒のロック保持時間を作れる) | 設定が保存される |
| 2 | `pulsen add --workflow pipeline --repo /tmp/pulsen-test/repo` を実行し、IDを T24 として控える(tick はしない) | 登録成功 |
| 3 | 端末Aで `pulsen abort <T24>` を実行する | 約 30 秒間、コマンドが完了しない(ロック保持中) |
| 4 | その間に端末B(`PULSEN_HOME` を同様に設定)で `pulsen tick; echo $?` を実行する | ロック競合によりスキップした旨が表示され、exit code は **0**(非0にならない)。どのタスクの状態も変更されない |
| 5 | 続けて端末Bで `pulsen add --workflow pipeline --repo /tmp/pulsen-test/repo; echo $?` を実行する | 「別の操作が実行中」の旨が表示され exit code 非0。`ls /tmp/pulsen-test/home/state/tasks/` にタスクファイルは増えていない(タスクは作られない) |
| 6 | 端末Aの完了後、notify_cmd を元の内容(テストデータの定義)に戻し、`pulsen tick; echo $?` を実行する | 通常どおり exit code 0 で完了する(T24 は stopped として残る。無害) |

**確認ポイント**:

- 手順4の 0 スキップは scenario/setup.md シナリオ3 の異常系「tickの二重起動」に対応する(ロック競合時に状態を変更せずスキップするのは正常な動作であり、次回の tick が処理を引き継ぐ)

## TC-25: スナップショットのみ破損したタスクの tick スキップと報告

**種別**: 異常系
**目的**: タスクファイル自体は読めるがスナップショットが読めない縮退状態(`SnapshotUnreadable`)のタスクを、tick が定義依存の処理(起動・判定・遷移・終端処理)をすべてスキップして報告し、書き込み・stopped化・通知を行わないことを検証する。破損状態は monitoring.md TC-24 と同じ手順で作る

| # | 操作 | 期待結果 |
|---|---|---|
| 1 | `pulsen add --workflow pipeline --repo /tmp/pulsen-test/repo` を実行し、IDを T25 として控える(tick はしない) | 登録成功。`show` で pending・attempt「なし」 |
| 2 | `cp /tmp/pulsen-test/home/state/tasks/<T25>.json /tmp/pulsen-test/t25.bak` の後、エディタでタスクファイルの埋め込まれたワークフロー定義(スナップショット)部分の値だけを不正な構造に置き換えて保存する(monitoring.md TC-24 手順2 と同じ方法。フィールド名は実ファイルの構造に合わせ、ファイル全体は JSON として妥当なまま保つ) | タスク属性は読めるがスナップショットが読めない状態になる |
| 3 | `pulsen tick; echo $?` を実行する | 0。サマリーに T25 のスナップショット読み取り不能によるスキップが報告される。起動は行われない(`pulsen show <T25>` で実行状態 `pending` のまま・attempt「なし」。`state/runs/<T25>/` は作られない) |
| 4 | `cat /tmp/pulsen-test/home/state/tasks/<T25>.json` と `cat /tmp/pulsen-test/notify.log` を確認する | タスクファイルは手順2の内容のまま(書き込まれない)。T25 に関する通知はない(stopped化されない) |
| 5 | `cp /tmp/pulsen-test/t25.bak /tmp/pulsen-test/home/state/tasks/<T25>.json` で復元し、`pulsen set-status <T25> done` → `pulsen tick` を実行する(片付け) | 復元後は通常どおり処理され、アーカイブされる |

**確認ポイント**:

- スナップショット破損中も、`Stopped { notified_at: None }` の再通知だけは行われる(通知に必要な情報はスナップショット非依存。本ケースでは pending のため発生しない)。retry の受理+警告・set-status の拒否は intervention.md TC-31 で確認する

## カバレッジ

### ユースケースエラーケース対応表

対象ユースケース: RegisterTask(add)・Tick(tick)・RunWrapper(wrapper)。本カテゴリのシナリオに関係する失敗分岐を対応付ける。ls / show / abort / retry / set-status の縮退・エラーは「状態の確認と追跡」「人間の介入と復旧」カテゴリの手順書の対象とする。

| ユースケース | エラーケース | 対応TC | 備考 |
|---|---|---|---|
| RegisterTask | ロック競合(登録前) | TC-24 | notify_cmd の長時間実行でロック保持時間を作り決定的に再現(intervention.md TC-25 と同じ手法) |
| RegisterTask | ワークフロー解決失敗(`NotFound`) | TC-08, TC-09 | 名前指定(TC-08)・パス指定(TC-09 手順2)の両方 |
| RegisterTask | ワークフロー解決失敗(`Io`) | 対象外 | 読み取りI/O障害の注入が必要 |
| RegisterTask | ワークフローのパースエラー(`WorkflowParseError` 全種) | TC-09 | 構文エラーで経路を代表確認。キー単位の検証エラー全種(`MissingInitial` 等)は `testcases/task/register-task.md` の自動テストで網羅 |
| RegisterTask | 表示名の決定失敗(`NameError`) | 対象外 | 語幹が空白のみになるファイル名という特殊入力のみ。自動テストで網羅 |
| RegisterTask | リポジトリ不在・非リポジトリ | TC-10 | |
| RegisterTask | ブランチ不在 / HEAD 解決不能(detached HEAD) | TC-11 | 空リポジトリのケースは同一経路のため detached HEAD で代表確認 |
| RegisterTask | 対象検証の git 操作自体の失敗(`TargetError::Failed`) | 対象外 | git 実行環境の障害注入が必要 |
| RegisterTask | 登録時検証エラー(`RegistrationError` 全種: 未定義エージェント・`skill_input` 欠落等) | 対象外 | エージェント定義・ワークフロー定義の検証は「セットアップとワークフロー定義」カテゴリの手順書で扱う |
| RegisterTask | ID 衝突の再発 / `create` の Io | 対象外 | ID 衝突・書き込みI/O障害の注入が必要 |
| Tick | ロック競合(0 でスキップ) | TC-24 | exit code 規約の唯一の例外(0 スキップ)を含めて検証 |
| Tick(手続きA) | worktree 作成失敗 → failed → リトライ上限超過で stopped | TC-12 | 登録後のリポジトリ消失で再現。ベースブランチ削除も同一経路 |
| Tick(手続きA) | テンプレート展開失敗(同期spawn失敗)→ spawn_fail_count 上限超過で stopped | TC-16 | 登録後の config.yaml 破壊で再現 |
| Tick(手続きA) | `prepare_attempt` 失敗 / `spawn_wrapper` の同期エラー | 対象外 | runディレクトリ作成・プロセス生成の障害注入が必要 |
| Tick(手続きC) | 猶予時間超過による spawn 失敗(pid 未出現 → pending 復帰) | 対象外 | ラッパーが pid を書けない状況(30秒の猶予内の障害)の注入が必要。恒久化の経路は TC-16 で確認 |
| Tick(手続きC) | runファイル破損(`Corrupt` / `InconsistentRunFiles`)での滞留 | 対象外 | runファイルの部分破損の作り込みが必要。修復は「状態の確認と追跡」カテゴリ(直接修復)の対象 |
| Tick(手続きD) | timeout 超過 → kill → failed | TC-14 | `timeout: 10s` で再現。デフォルト 1h・`timeout: none` の確認は自動テストに委ねる |
| Tick(手続きD) | kill 失敗(`KillError`) | 対象外 | シグナル送出自体を失敗させる障害注入が必要 |
| Tick(手続きD) | exit なし・プロセス死亡(exit code 不明)→ failed | TC-21 | 外部からの `kill -9` で OOM・マシン再起動を代替再現 |
| Tick(手続きD) | 判定失敗(プロトコル外 exit / 判定 timeout / 起動不能)→ 再判定 → 上限超過で stopped | TC-15 | プロトコル外 exit で代表確認。timeout・起動不能は同一経路(`record_judge_failure`) |
| Tick(手続きD) | 判定コマンドが 10 → failed(自動リトライ) | TC-19 | ポーリングのチェック失敗として確認 |
| Tick(手続きD) | 判定コマンドが 20 → skipped(pending 復帰・通知なし) | TC-06 | |
| Tick(手続きD) | `fail_run` の上限超過で stopped | TC-13, TC-22 | 等号(凍結しない)と超過の境界を含む |
| Tick(手続きB) | worktree 削除失敗 / アーカイブ移動失敗 | 対象外 | 削除・移動を失敗させる権限操作等が環境依存。終端処理の異常系は「終端処理とアーカイブ」カテゴリで扱う |
| Tick | パース不能なタスクファイルのスキップ(書き込まない・通知しない) | TC-20 | |
| Tick | スナップショットのみ破損(`SnapshotUnreadable`)のスキップ | TC-25 | monitoring.md TC-24 と同じ手順で破損状態を作る。縮退表示・修復は「状態の確認と追跡」カテゴリ、retry / set-status の挙動は「人間の介入と復旧」カテゴリ(intervention.md TC-31)で扱う |
| Tick | `TransitionError` / 不変条件の破れ(手動修復による) | 対象外 | タスクファイルの手動改変による破れの注入は「状態の確認と追跡」カテゴリ(直接修復)の対象 |
| Tick(notify) | notify_cmd の失敗・timeout → 再通知(at-least-once) | 対象外 | 通知の発火自体は TC-07・TC-12〜18・TC-21〜23 で確認済み。失敗時の再通知・notified_at の観測は「人間の介入と復旧」カテゴリ(通知受領)で扱う |
| Tick(手続きE) | runディレクトリ gc の失敗・保護規則 | 対象外 | 本テストは `run_retention` 未設定(gc 無効)で行う。gc は「終端処理とアーカイブ」カテゴリで扱う |
| RunWrapper | エージェント起動不能の exit 符号化(127 / 126)→ 通常の failed 経路 | TC-17 | spawn 失敗(TC-16)との経路の違いを確認 |

### 観点チェックリスト

Step 2-5 の観点を CLI 向けに読み替えて一巡した結果。

| 観点 | 対応TC | 対象外の理由 |
|---|---|---|
| 入力バリデーション(存在しないワークフロー名・パス・リポジトリ・ブランチ) | TC-08, TC-09, TC-10, TC-11 | |
| 境界値(リトライ上限の等号/超過・`retries: 0`・判定プロトコルの境界・spawn/再判定上限の等号) | TC-13, TC-14, TC-15, TC-16, TC-22, TC-23 | |
| 認証・権限 | 対象外 | 単一利用者のローカルCLIツールであり、認証・権限の機構を持たない |
| 空状態・初期状態(タスク0件の ls / tick・`state/` 未作成) | TC-01 | |
| 重複・競合(同一対象の二重登録・tick の多重実行) | TC-04, TC-24 | 二重登録は排除しない仕様を TC-04 で、tick / add のロック競合(tick は 0 スキップ・add は非0)を TC-24 で確認 |
| 削除・変更の影響(元YAML編集・登録後のリポジトリ消失・config編集・worktree消失) | TC-03, TC-12, TC-16, TC-18 | スナップショットの非影響(TC-03)とグローバル設定の即時反映(TC-16 手順7)の対比を含む |
| 操作の中断・逸脱(実行中プロセスの外部強制終了) | TC-21 | tick 自体の任意時点クラッシュは注入手段がなく対象外(冪等な再導出は TC-14 手順3 の連続 tick で部分確認) |
| 特殊入力(不正なブランチ名・空文字・特殊文字) | 対象外 | 入力境界の細粒度検証は `testcases/task/register-task.md` の自動テスト(境界値の節)で網羅する |
| UIの状態(実行中の再操作・冪等性・エラー後の再試行) | TC-05, TC-14, TC-19 | 実行中の連続 tick で状態が変わらないこと(TC-14)、失敗後の自動再試行と回復(TC-05・TC-19)で確認 |
