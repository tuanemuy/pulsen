# 動作確認計画 — Issue #4: 状態の確認と追跡(ls / show)

**Issue:** #4
**作成日:** 2026-08-16

---

## 確認環境

この Issue の変更を確認するために必要な手順のみ記載する（プロジェクト全体のセットアップは省略）。

本スライスで `pulsen ls` / `pulsen show` が初めて実装される。既存のコマンドは `pulsen add` / `pulsen tick` / `pulsen wrapper`（隠しサブコマンド）で、これに2つ増えて5つになる（`crates/pulsen/src/cli/args.rs` の `Command`）。`abort` / `retry` / `set-status`（#5）と終端処理・gc（#6）は引き続き存在しない。Web UI は無く、確認はすべてターミナル上の CLI 実行になる。

### 検証環境の起動

`.envrc` は `use flake` のみ。direnv がこのリポジトリで許可済みであれば、リポジトリに `cd` するだけで `flake.nix` の devShell（cargo / rustc / clippy / rustfmt / rust-analyzer / git）に入る。direnv を使わない場合は各コマンドを `nix develop -c <command>` で包む。

```sh
cd /Users/hikaru/github.com/tuanemuy/pulsen
cargo --version    # devShell が効いていることの確認
git --version      # worktree 操作で使う
```

ビルドとバイナリの解決。ワークスペースの target ディレクトリはリポジトリ直下で、bin 名は `pulsen`。

```sh
cargo build
export PATH="$PWD/target/debug:$PATH"
pulsen --help
```

以降の手順はすべてこの PATH が通ったシェルから実行する。exit code は各コマンドの直後に `echo $?` で確認する。JSON の項目を絞って見る場合はシステムの `jq` を使う（devShell には含まれない。`.thread/3/testing.md` で `/usr/bin/jq` を確認済み）。

自動テスト側の状態（本確認の前提となるグリーン状態。AC-8）:

```sh
cargo fmt --all --check
cargo build --workspace --locked
cargo test --workspace --locked --no-fail-fast -- --nocapture
cargo clippy --workspace --all-targets --locked -- -D warnings
```

4つとも `.github/workflows/ci.yml` の各ステップと同じ形にそろえてある。`cargo fmt` に `--locked` は付けない（rustfmt ラッパーが依存解決をしないため受け付けない）。`--nocapture` は適合スイート（`pulsen-conformance`）の `SKIP …` 行を標準出力に出すために必須で、これが読めないとステップ13 の記帳ができない。`--no-fail-fast` はテストターゲット単位の打ち切りを避ける。

AC-8 の隔離を機械的に確認する3つの grep:

```sh
grep -rlE 'cfg\([^)]*\b(unix|windows|target_os|target_family)\b' crates/*/src/
grep -rn '#\[allow(unsafe_code)\]' crates/
grep -n -A 10 '^\[dependencies\]' crates/pulsen-domain/Cargo.toml crates/pulsen/Cargo.toml
```

- 1つ目: `crates/pulsen/src/` 側のヒットが `util/atomic.rs` / `adapter/process.rs` / `adapter/task_repository.rs` の3ファイルだけで、新設の `application/list_tasks.rs` / `show_task.rs` / `cli/ls.rs` / `cli/show.rs` / `cli/render/*` が1つも現れないこと。`crates/pulsen-conformance/src/lib.rs` の1件は適合ハーネスが権限制限の効き目を probe する分岐で、本番の実行経路には乗らない。
- 2つ目: `crates/pulsen/src/adapter/process.rs:451` の1件だけであること。
- 3つ目: `pulsen-domain` の `[dependencies]` が空のまま（依存の行が1つも無く、空行を挟んで `[lints.rust]` が続く）で、`pulsen` の本番依存が `pulsen-domain` / `clap` / `getrandom` / `serde` / `serde_json` / `serde_yaml_ng` / `tempfile` の7行から増えていないこと。

### 本スライスでの実行範囲（手順書からの読み替え）

一次テストドキュメントは `spec/manual-tests/monitoring.md`（TC-01〜TC-34）。その事前準備は本スライスに無いコマンドを2箇所で使うため、下記のとおり読み替える。読み替えの根拠は plan.md「spec との差分として提起するもの」と部分消化の表。

- **`TASK_G` の `pulsen abort`（事前準備 手順5）は実行しない**（#5）。代わりに、tick を打ち終えたあとに **wf-wait のタスクファイルを直編集して `stopped` にする**。凍結要因は `retry_limit_exceeded` になるため、**abort 経路の判別（TC-15）は確認できない**。`StopReason::Aborted` の表示は自動テスト（`crates/pulsen/tests/show_task.rs`）で担保する。
- **`TASK_D` を tick の終端処理でアーカイブ済みにすることはできない**（#6。`Branch::Cleanup` は未配線で、`done` に到達したタスクは `pending` のまま `state/tasks/` に滞留する）。代わりに、**手順書自身が TC-34 で定める手動移動**（`git worktree remove` → `state/archive/` へ `mv`）でアーカイブ済みの前提を作る。したがって TC-05 / TC-06 / TC-10 / TC-11 / TC-25 は**表示としては消化するが、アーカイブが生まれる経路そのものは確認しない**（`PAGE-ls-004` / `PAGE-show-008` / `TC-task-show-task-031` は部分消化。plan.md の表）。
- `exit` ファイルは `0` という素の数値ではなく `{"code": 0}` の**整形 JSON**（2スペース字下げ・末尾に改行なし。ADR-080。`crates/pulsen/src/adapter/run_store.rs` の `write_exit` / `encode`）。手順書が「数値 `0`」と書く箇所は `.code` の値として読む。`show` が表示するのはこの `.code` の値である。
- 手順書は**記載順の実行**を前提にしている。本書の項目も記載順に実行し、各項目の冒頭にその項目が前提とする状態を明記してある。**破壊操作の復元手順は、その TC が実行範囲外でも必ず実行する**。
- 連続する tick の間は 2〜3 秒あける（1回のエージェント実行の消化に「起動 → spawn確認 → 判定 → 遷移」の4 tick を要する）。

実行する TC（`spec/manual-tests/monitoring.md`）:

| TC | 範囲 | 備考 |
|---|---|---|
| TC-01 | 全手順 | 手順3 の `TASK_G` は直編集で作った `hold` / `stopped` |
| TC-02 / TC-03 / TC-04 | 全手順 | |
| TC-05 / TC-06 | 全手順 | `TASK_D` は手動移動でアーカイブ済みにしてある |
| TC-07 / TC-08 | 全手順 | |
| TC-09 | 手順1〜3 | 手順4 の `pulsen abort` は #5。`TASK_H` は放置する |
| TC-10 / TC-11 / TC-12 / TC-13 / TC-14 | 全手順 | TC-11 手順4 の期待は `.code` = 0 に読み替え |
| TC-16 / TC-17 / TC-18 | 全手順 | TC-17 手順6 の config 復元は必ず実行する |
| TC-19 〜 TC-33 | 全手順 | TC-26 手順5・TC-28 は権限制限が効く環境でのみ |
| TC-34 | 手順2〜6 | 手順1 の `pulsen set-status` は #5 |

実行しない TC と理由（1行ずつ）:

| TC | 除外理由 |
|---|---|
| monitoring.md TC-09 手順4 | 後片付けの `pulsen abort` が #5 で未実装 |
| monitoring.md TC-15 | abort 経路で凍結したタスクを CLI で作れない（#5）。直編集で `reason` を `aborted` にしても、確認できるのは表示だけで「abort が凍結を記録した」ことは主張できない |
| monitoring.md TC-34 手順1 | `pulsen set-status` が #5 で未実装（拒否されることの確認そのものが打てない） |
| cleanup.md TC-13 | 前提「TC-01〜TC-09 実施済み(アーカイブ済み `<T1>`〜`<T6>`)」が tick の終端処理を要する（#6） |
| cleanup.md TC-14 | 前提「TC-01 実施済み(`<T1>` がアーカイブ済み)」が同じく #6 を要する |
| cleanup.md TC-15 | 同上（アーカイブ済み `<T1>` の run ディレクトリ参照） |
| cleanup.md TC-17 | 手順1 は monitoring.md TC-19 と同一の確認で重複、手順2 の `pulsen retry` は #5 |
| cleanup.md TC-23 | 前提「TC-19 実施済み(run ディレクトリが gc 済み)」が gc（#6）を要する。gc 相当の不在表示は monitoring.md TC-25 で確認する |

### 検証用のフィクスチャ準備

`spec/manual-tests/monitoring.md` の「事前準備」を、上記の読み替えを織り込んだ形で書き下ろす。パスは手順書の記載どおりに保つ。

1. 分離ホームとテスト領域を初期化し、設定を作成する（手順書 事前準備1 のまま）。

    ```sh
    export PULSEN_HOME=$HOME/pulsen-manual-test
    rm -rf $PULSEN_HOME $HOME/pulsen-test-repo $HOME/pulsen-manual-test-empty /tmp/pulsen-notify.log
    mkdir -p $PULSEN_HOME/workflows
    cat > $PULSEN_HOME/config.yaml <<'EOF'
    agents:
      sh:
        cmd: ["sh", "-c", "{input}"]
    notify_cmd: ["sh", "-c", "echo \"stopped: $TASK_ID ($WORKFLOW/$TASK_STATUS)\" >> /tmp/pulsen-notify.log"]
    EOF
    ```

2. ワークフローを4つ作成する（手順書 事前準備2 のまま）。

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

3. 対象リポジトリを作成する（identity はリポジトリローカルに設定する）。

    ```sh
    git init $HOME/pulsen-test-repo
    git -C $HOME/pulsen-test-repo config user.name pulsen-test
    git -C $HOME/pulsen-test-repo config user.email pulsen-test@example.com
    git -C $HOME/pulsen-test-repo commit --allow-empty -m init
    ```

4. tick の影響を受けないタスク（wf-wait）を4つ登録し、IDを控える。

    ```sh
    pulsen add --workflow wf-wait --repo $HOME/pulsen-test-repo   # → export TASK_A=<task-id>
    pulsen add --workflow wf-wait --repo $HOME/pulsen-test-repo   # → export TASK_E=<task-id>
    pulsen add --workflow wf-wait --repo $HOME/pulsen-test-repo   # → export TASK_F=<task-id>
    pulsen add --workflow wf-wait --repo $HOME/pulsen-test-repo   # → export TASK_G=<task-id>
    ```

    手順書 事前準備5 の `pulsen abort $TASK_G` はここでは実行しない（#5）。`TASK_G` は手順7 の後で作る。

5. 実行系のタスクを3つ登録し、IDを控える（手順書 事前準備6 のまま）。

    ```sh
    pulsen add --workflow wf-sleep --repo $HOME/pulsen-test-repo  # → export TASK_B=<task-id>
    pulsen add --workflow wf-fail  --repo $HOME/pulsen-test-repo  # → export TASK_C=<task-id>
    pulsen add --workflow wf-echo  --repo $HOME/pulsen-test-repo  # → export TASK_D=<task-id>
    ```

6. tick を繰り返し実行し、各タスクを目標状態へ進める。

    ```sh
    for i in $(seq 1 15); do pulsen tick; sleep 2; done
    ```

    `TASK_C` は「起動 → spawn確認 → 判定」の3 tick で1 attempt を消化し、`attempt_count` 3 > 既定上限 2 となる9回目の tick で凍結する。`TASK_D` は4 tick で `done` に到達し、そこで止まる（`run: cleanup` は #6 のため未配線）。`TASK_B` は `sleep 3000` のまま `running` で滞留する。

7. `TASK_G` を stopped にする（手順書 事前準備5 の読み替え。tick が触らないよう `notified_at` も入れる）。

    ```sh
    jq --arg at "$(date -u '+%Y-%m-%dT%H:%M:%SZ')" \
      '.execution = {"state":"stopped","reason":"retry_limit_exceeded","notified_at":$at}' \
      "$PULSEN_HOME/state/tasks/$TASK_G.json" > /tmp/pulsen-taskG.json \
      && mv /tmp/pulsen-taskG.json "$PULSEN_HOME/state/tasks/$TASK_G.json"
    cat "$PULSEN_HOME/state/tasks/$TASK_G.json"
    ```

    タスクファイルの形は `crates/pulsen/src/adapter/task_file.rs` の `ExecutionDto`（`state` をタグに持つ直和。`stopped` は `reason` と `notified_at` を同じ階層に並べる）と `StopReasonDto`（`retry_limit_exceeded` / `judge_limit_exceeded` / `spawn_fail_limit_exceeded` / `aborted` の snake_case）に従う。

8. `TASK_D` をアーカイブ済みにする（手順書 TC-34 手順2〜3 と同じ手動移動。`state/archive/` は書き込み系が必要になったときに作られる領域なので、先に作る）。

    ```sh
    git -C $HOME/pulsen-test-repo worktree remove $PULSEN_HOME/worktrees/$TASK_D
    mkdir -p $PULSEN_HOME/state/archive
    mv $PULSEN_HOME/state/tasks/$TASK_D.json $PULSEN_HOME/state/archive/$TASK_D.json
    ls $PULSEN_HOME/state/archive/
    git -C $HOME/pulsen-test-repo branch --list "pulsen/*"
    ```

    worktree を先に消すのは、`show` の「worktree は削除済み」注記が**アーカイブ済みという事実から導かれる**（ファイルシステムには問わない。pages ※4 / ※9）ため、実体を残すと表示と実体が食い違うから。ブランチ `pulsen/<TASK_D>` は残す（成果の回収に使う）。

9. 準備の完了を確認する。

    ```sh
    pulsen ls; echo $?
    pulsen ls --all; echo $?
    ```

    - `TASK_B` が `work` / `running`
    - `TASK_C` が `work` / `stopped`、attempt_count が 3
    - `TASK_A` / `TASK_E` / `TASK_F` が `hold` / `pending`、`TASK_G` が `hold` / `stopped`
    - `pulsen ls` に `TASK_D` が現れず、`pulsen ls --all` では `done` の行としてアーカイブ済みの印付きで現れる

    ここまでが揃わなければ tick を追加実行する（`TASK_C` の凍結が遅れている場合のみ）。

### デプロイ方法

なし（ローカル CLI のみで確認できる。Web UI も dev サーバも無い）。

## 確認項目

### 1. タスク一覧の全体像 — タスクステータスと実行状態の併記

- **対応する受け入れ基準:** AC-2
- **対応する手順書:** `monitoring.md` TC-01 全手順（手順3 の `TASK_G` は直編集で作った `hold` / `stopped`）
- **前提:** フィクスチャ準備 直後。
- **目的:** `ls` が1行にタスクID・ワークフロー名・リポジトリ・ブランチ・タスクステータス・実行状態・attempt_count・更新日時を並べること、タスクステータス（ユーザー定義語彙）と実行状態（固定6値）が**別の列として区別できる**こと、既定ではアーカイブ済みが現れないことを確認する。
- **手順:**
  1. `pulsen ls; echo $?`
  2. 1行の表示項目を数え上げる（タスクID / ワークフロー名 / リポジトリ / ブランチ / タスクステータス / 実行状態 / attempt_count / 更新日時）
  3. `TASK_A`（`hold` / `pending`）と `TASK_G`（`hold` / `stopped`）の行を見比べる
  4. `TASK_B`（`work` / `running`）と `TASK_C`（`work` / `stopped`）の行を見比べる
  5. 一覧に `TASK_D` が無いことを確認する
- **期待結果:**
  - 手順1: 現役6件（`TASK_A` / `TASK_B` / `TASK_C` / `TASK_E` / `TASK_F` / `TASK_G`）が並び、exit code 0。
  - 手順3・4: 同じタスクステータスでも実行状態で区別でき、逆に同じ実行状態でもタスクステータスで区別できる。どちらの列がどちらであるかが見出しか区切りから読める。
  - 手順5: `TASK_D` は現れない（`state/archive/` は既定の対象集合に入らない）。
  - `TASK_A` / `TASK_E` / `TASK_F` / `TASK_G` はブランチが未確定（worktree 未作成）であり、その旨が読める。
- **確認ポイント:** `TASK_C` の attempt_count が 3（リトライ上限 2 の超過）であること — 一覧の時点で「注意が必要なタスク」が数字で見えることがこのシナリオの目的。並び順は spec が規定していないので合否の対象にしない。

### 2. 絞り込みの合成 — `--status` / `--state` / `--all`

- **対応する受け入れ基準:** AC-2
- **対応する手順書:** `monitoring.md` TC-02 / TC-03 / TC-04 / TC-05 / TC-06 / TC-31、TC-33 手順3
- **前提:** 確認項目1 の直後。
- **目的:** `--status` と `--state` が AND で合成されること、`--all` は絞り込みではなく**対象集合の拡張**であり拡張後に絞り込みが適用されること、`--state` の6値がすべて受理されること、未知のタスクステータス名が該当0件（エラーではない）になることを確認する。
- **手順:**
  1. `pulsen ls --status work; echo $?`
  2. `pulsen ls --state stopped; echo $?` / `pulsen ls --state pending; echo $?` / `pulsen ls --state running; echo $?`
  3. `pulsen ls --status work --state running; echo $?` / `pulsen ls --status work --state stopped; echo $?`
  4. `pulsen ls --all; echo $?`
  5. `pulsen ls --status done; echo $?` → `pulsen ls --all --status done; echo $?`
  6. `pulsen ls --status no-such-status; echo $?`
  7. `pulsen ls --state launching; echo $?` / `--state completed` / `--state failed`
- **期待結果:**
  - 手順1: `TASK_B` / `TASK_C` のみ（`hold` の4件は出ない）。
  - 手順2: `stopped` は `TASK_C` と `TASK_G`、`pending` は `TASK_A` / `TASK_E` / `TASK_F`、`running` は `TASK_B`。いずれも 0。
  - 手順3: 1つ目は `TASK_B` のみ、2つ目は `TASK_C` のみ（`TASK_G` は `hold` なので落ちる）。AND であることがこの差で分かる。
  - 手順4: 現役6件に加えて `TASK_D` が現れ、アーカイブ済みの印とブランチ `pulsen/<TASK_D>` が表示される。
  - 手順5: `--all` なしは該当0件で「空である旨」を表示して 0（`done` は `TASK_D` だけで現役集合に無い）。`--all` 付きは `TASK_D` のみ。
  - 手順6: エラーにならず該当0件で 0（タスクステータスは検証しない語彙）。
  - 手順7: 3つとも受理され、該当なしの空表示で 0。
- **確認ポイント:** 手順5 の2つの結果の差が「`--all` を絞り込み条件として扱っていない」ことの外形的な証拠になる。該当0件のとき無言で終わらず「空である」と述べること（PAGE-ls-010）。

### 3. タスク詳細の確認 — running / 未実行 / launching

- **対応する受け入れ基準:** AC-3
- **対応する手順書:** `monitoring.md` TC-07 全手順、TC-08 全手順、TC-09 手順1〜3（手順4 の `pulsen abort` は #5 のため実行せず、`TASK_H` は放置する）
- **前提:** 確認項目2 の直後。
- **目的:** `show` が1タスクの全属性・カウンタと適用上限・現在attemptの実行メタデータ・定義済みステータス一覧・スナップショット保存先を表示すること、未実行タスクでは workspace が「未作成」・attempt が「なし」になること、launching で同定情報が未取り込みなら「未取得」になることを確認する。
- **手順:**
  1. `pulsen show $TASK_B; echo $?`
  2. 基本属性（ワークフロー名 `wf-sleep` / リポジトリ `$HOME/pulsen-test-repo` / ベースブランチ / タスクステータス `work` / 実行状態 `running` / 更新日時）を確認する
  3. workspace_path（`$PULSEN_HOME/worktrees/$TASK_B`）とブランチ（`pulsen/$TASK_B`）を確認し、`ls "$PULSEN_HOME/worktrees/$TASK_B"` で実在を確認する
  4. 3つのカウンタと上限の併記を確認する
  5. 定義済みステータス一覧とスナップショット保存先パスを確認する
  6. 現在attemptの番号・runディレクトリ・PID・kill同定子・starttime を確認する
  7. `pulsen show $TASK_A; echo $?`（未実行タスク）→ workspace / attempt の表示と、`ls $PULSEN_HOME/state/runs/$TASK_A; echo $?`
  8. `pulsen add --workflow wf-sleep --repo $HOME/pulsen-test-repo` → `export TASK_H=<task-id>` → `pulsen tick && pulsen show $TASK_H; echo $?`
- **期待結果:**
  - 手順1・2: 詳細が表示され exit code 0。
  - 手順4: attempt_count 0 / judge_attempt_count 0 / spawn_fail_count 0 が、それぞれリトライ上限 2・judge 上限 3・spawn 上限 3 を**併記して**表示される（カウンタは連続失敗の数なので、attempt 番号が 1 でも attempt_count は 0）。
  - 手順5: `work` と `done` の2つが定義済みステータスとして並び、保存先として `state/tasks/<TASK_B>.json`（タスクファイル自身。ADR-015）が表示される。
  - 手順6: 番号 1・`state/runs/<TASK_B>/attempt-1`・PID・starttime（とプラットフォームの kill 同定子）が表示され、どの attempt が動いているか特定できる。
  - 手順7: exit code 0。workspace は「未作成」、ブランチは未確定、attempt は「なし」で runディレクトリ・PID・exit への参照が出ない（または「なし」と明示される）。`ls` は非0（ディレクトリ不在）で、これが自然な状態であることが show の表示と一致する。`hold` は `run: wait` なので **attempt_count にリトライ上限が併記されない**（適用対象がない）。
  - 手順8: 実行状態 `launching` が表示され exit code 0。attempt 番号と runディレクトリは出るが、PID・starttime は「未取得」と表示され、エラーにならない。
- **確認ポイント:** 手順7 の「併記なし」（`NotApplicable`）と、確認項目7 手順6 で見る「不明」（`Unknown`）が**別の文言**であること — 両者が同じ表示に潰れると、待ちステータスとスナップショット破損の区別が失われる。手順8 の3項目（PID / kill同定子 / starttime）は**まとめて**未取得になること（`ProcessIdent` は3値を1つの `Option` で持つ）。

### 4. アーカイブ済みタスクの詳細と手動アーカイブ移動

- **対応する受け入れ基準:** AC-3
- **対応する手順書:** `monitoring.md` TC-10 全手順、TC-34 手順2〜6（手順1 の `pulsen set-status` は #5 のため実行しない）
- **前提:** 確認項目3 の直後。`TASK_D` はフィクスチャ準備 手順8 でアーカイブ側にある。
- **目的:** `show` が現役 → アーカイブの順でタスクを解決すること、アーカイブ済みであること・worktree が削除済みであることを明示すること、保存先パスが `state/archive/<id>.json` になることを確認する。併せて、クリーンアップステータスを持たない定義のタスク（`TASK_G`）を手動でアーカイブ側へ移して走査対象から外せることを確認する。
- **手順:**
  1. `pulsen show $TASK_D; echo $?`
  2. アーカイブ済みの注記・workspace の「削除済み」表示・ブランチ `pulsen/$TASK_D` を確認する
  3. タスクファイルパスの表示を確認する
  4. `git -C $HOME/pulsen-test-repo branch --list "pulsen/*"`
  5. `pulsen show $TASK_G` で workspace を確認する（TC-34 手順2）
  6. `mkdir -p $PULSEN_HOME/state/archive && mv $PULSEN_HOME/state/tasks/$TASK_G.json $PULSEN_HOME/state/archive/$TASK_G.json`
  7. `pulsen ls; echo $?` と `pulsen tick; echo $?`
  8. `pulsen ls --all; echo $?`
  9. `pulsen show $TASK_G; echo $?`
- **期待結果:**
  - 手順1〜3: exit code 0。アーカイブ済みであることが明示され、workspace_path は「削除済み」と分かる形で表示され、ブランチは表示され、保存先は `state/archive/<TASK_D>.json`。
  - 手順4: `pulsen/<TASK_D>` ブランチが実在する（アーカイブしてもブランチは残る）。
  - 手順5: `TASK_G` の workspace は「未作成」（一度も実行されていない）。
  - 手順7: `ls` に `TASK_G` は現れず、tick のサマリー・報告にも `TASK_G` が現れない（走査対象から外れた）。
  - 手順8: `TASK_G` がアーカイブ済みの印付きで現れる。
  - 手順9: アーカイブ済みの注記付きで詳細が表示され exit code 0（移動後は通常のアーカイブ済みタスクと同じ扱い）。
- **確認ポイント:** 手順3 と手順9 の保存先パスが `state/tasks/` ではなく `state/archive/` を指すこと — 別ファイルのパスを組み立てず、解決に使ったパスをそのまま示していること。手順2 の「削除済み」がファイルシステムを見た結果ではなく**アーカイブ済みという事実から**導かれていること（pages ※4）。tick が `state/archive/` を書き換えないこと。

### 5. エージェント実行ログの確認の起点

- **対応する受け入れ基準:** AC-4
- **対応する手順書:** `monitoring.md` TC-11 全手順（手順4 は `.code` に読み替え）、TC-12 全手順、TC-13 全手順、TC-25 全手順（TC-11 の後に実行する）
- **前提:** 確認項目4 の直後。
- **目的:** `show` が現在attemptの `stdout.log` / `stderr.log` / `exit` のパスと exit の値を示し、そこからログの実体へ直接たどれること、attempt 番号がパスに含まれるため試行が混ざらないこと、runディレクトリが存在しなければ「存在しない」と明示して 0 で終わることを確認する。
- **手順:**
  1. `pulsen show $TASK_D` で runディレクトリ（`state/runs/<TASK_D>/attempt-1`）と3つのパス・exit の値を控える
  2. `cat $PULSEN_HOME/state/runs/$TASK_D/attempt-1/stdout.log`
  3. `cat $PULSEN_HOME/state/runs/$TASK_D/attempt-1/stderr.log`
  4. `cat $PULSEN_HOME/state/runs/$TASK_D/attempt-1/exit; echo`
  5. `pulsen show $TASK_C` で現在attemptのパスを控える → `ls $PULSEN_HOME/state/runs/$TASK_C/`
  6. `for n in 1 2 3; do cat "$PULSEN_HOME/state/runs/$TASK_C/attempt-$n/exit"; echo; cat "$PULSEN_HOME/state/runs/$TASK_C/attempt-$n/stderr.log"; done`
  7. `pulsen show $TASK_B` で現在attemptのパスを控える → `ls $PULSEN_HOME/state/runs/$TASK_B/attempt-1/`
  8. `tail -f $PULSEN_HOME/state/runs/$TASK_B/attempt-1/stdout.log` を数秒実行して Ctrl-C で抜ける
  9. `rm -rf $PULSEN_HOME/state/runs/$TASK_D` → `pulsen show $TASK_D; echo $?`
- **期待結果:**
  - 手順1: 3つのパスが表示され、exit の値として 0 が表示される。
  - 手順2: `hello from pulsen`（ラッパーがリダイレクトした生の出力。加工されていない）。
  - 手順3: 空（空のログは異常ではない）。
  - 手順4: `{"code": 0}` の整形 JSON（ADR-080）。show が表示していた値と一致する。
  - 手順5: 現在attemptは `attempt-3`（最終試行）。`attempt-1` / `attempt-2` / `attempt-3` が別ディレクトリとして存在する。
  - 手順6: すべての attempt で `.code` が 1。毎回同じ失敗であることが試行ごとに分離されたログから読める。
  - 手順7: `stdout.log` / `stderr.log` は存在するが `exit` が無い（実行は未終了）。show の exit も「なし」として表示される。
  - 手順8: エラーなく追跡できる（`sleep` のため出力は増えない）。
  - 手順9: exit code **0**。attempt の runディレクトリが「存在しない」と明示され、他の項目は通常どおり表示される。
- **確認ポイント:** show が指すのが**タスクファイルの現在attempt**であること（手順5 の `attempt-3`）— 過去の試行や、spawn 失敗で pending に戻った痕跡と取り違えない。手順9 で `exit` の項目が「存在しない」に吸収され、エラー扱い（非0）に昇格しないこと。手順9 は gc 相当の状態をディレクトリの直接削除で作っており、**gc がディレクトリを消す経路そのものは #6**（`TC-task-show-task-031` は部分消化）。

### 6. stopped タスクの原因調査 — 3経路の判別

- **対応する受け入れ基準:** AC-5
- **対応する手順書:** `monitoring.md` TC-14 全手順、TC-16 全手順、TC-17 全手順（手順6 の config 復元は必ず実行する）
- **前提:** 確認項目5 の直後。TC-16 / TC-17 は自分でタスクを登録し tick を打つ。
- **目的:** 凍結要因と notified_at が表示され、リトライ上限超過・判定上限超過・連続spawn失敗が「カウンタと併記された上限」から判別できること、ツール操作の失敗で凍結した場合は `last_failure` が、同期spawn失敗経路では attempt が「なし」であることが表示されることを確認する。
- **手順:**
  1. `pulsen show $TASK_C; echo $?`（リトライ上限超過）
  2. 3つのカウンタと上限を見比べ、`notified_at` を確認する
  3. `cat /tmp/pulsen-notify.log`
  4. `ls -a $PULSEN_HOME/worktrees/$TASK_C`（stopped の worktree は自動削除されない）
  5. 判定不能の経路（TC-16）: `wf-judge.yaml` を作る

      ```sh
      cat > $PULSEN_HOME/workflows/wf-judge.yaml <<'EOF'
      workflow: wf-judge
      agent: sh
      initial: work
      statuses:
        work:
          prompt: "echo checked"
          judge: ["sh", "-c", "exit 5"]
          next: done
        done:
          run: cleanup
      EOF
      ```

  6. `pulsen add --workflow wf-judge --repo $HOME/pulsen-test-repo` → `export TASK_J=<task-id>` → `for i in $(seq 1 10); do pulsen tick; sleep 2; done` → `pulsen ls --state stopped`
  7. `pulsen show $TASK_J; echo $?` → カウンタと上限、スナップショット保存先を確認 → 保存先のタスクファイルで `work` の `judge` 定義を確認する → 現在attemptの `exit` を確認する
  8. 連続spawn失敗の経路（TC-17）: `pulsen add --workflow wf-echo --repo $HOME/pulsen-test-repo` → `export TASK_I=<task-id>`
  9. `cp $PULSEN_HOME/config.yaml $PULSEN_HOME/config.yaml.bak` → `config.yaml` の `sh` の `cmd` を `["sh", "-c", "{input}", "{bogus}"]` に書き換える
  10. `pulsen tick` を `TASK_I` が stopped になるまで繰り返す（4回程度。`pulsen ls --state stopped` で確認）
  11. `pulsen show $TASK_I; echo $?` → `ls $PULSEN_HOME/state/runs/$TASK_I; echo $?` → `grep "$TASK_I" /tmp/pulsen-notify.log`
  12. 復元（必ず実行する）: `cp $PULSEN_HOME/config.yaml.bak $PULSEN_HOME/config.yaml`
- **期待結果:**
  - 手順1・2: exit code 0。実行状態 `stopped` と凍結要因が表示され、attempt_count 3 がリトライ上限 2 を**超過**していることが併記から読める。judge / spawn のカウンタは上限未満。`notified_at` に日時が入っている。
  - 手順3: `stopped: <TASK_C> (wf-fail/work)` の行がある。
  - 手順4: worktree が残っている（`wf-fail` は成果物を書かないので中身は `.git` だけ。`ls -a` でないと見えない）。
  - 手順7: exit code 0。judge_attempt_count が上限 3 を超過し、attempt_count は上限未満 → 「判定自体の不能」と判別できる。スナップショットには `["sh", "-c", "exit 5"]` が固定されており、エージェント再実行では解決しない不具合であることが分かる。現在attemptの `.code` は 0（エージェント自体は成功している）。
  - 手順11: exit code 0。spawn_fail_count が上限 3 を超過し、直近の失敗要因に展開エラー（未知プレースホルダ）の内容が表示され、**attempt は「なし」**（採番されない）。`ls` は非0（runディレクトリが無い）。通知ログに `TASK_I` の行がある。
- **確認ポイント:** 3経路が「どのカウンタがどの上限を超えたか」だけで判別できること — 凍結要因の文言に頼らず数字で判別できるのがこのシナリオの目的。手順11 の「attempt なし」と手順5 の「runディレクトリ不在」が、同期検出の spawn 失敗と猶予時間超過の経路を分ける点であること。手順12 の復元を忘れると以降のすべての spawn が失敗する。

### 7. タスクファイルの直接閲覧・修復

- **対応する受け入れ基準:** AC-6
- **対応する手順書:** `monitoring.md` TC-18 全手順、TC-22 全手順、TC-23 全手順、TC-24 全手順（手順2 の「エディタで編集」は下記の `jq` 手順に具体化する）
- **前提:** 確認項目6 の直後（config.yaml が復元済みであること）。
- **目的:** パース不能なタスクファイル（`Corrupt`）が混ざっても `ls` が一覧全体を失敗させずファイルパスと読めない旨を報告して 0 で終わること、同じファイルを `show` で指すとパースエラーの内容とパスを表示して非0で終わること、スナップショットのみ読めないタスクは**行として表示され絞り込みの対象にもなる**こと、修復すれば通常表示に復帰することを確認する。
- **手順:**
  1. `cat $PULSEN_HOME/state/tasks/$TASK_A.json`（TC-18）→ ワークフロー名・対象・タスクステータス・実行状態・カウンタ・更新日時・スナップショット（`hold` の定義）が読めることを確認する
  2. `pulsen show $TASK_A; echo $?`（閲覧は状態に影響しない）
  3. `cp $PULSEN_HOME/state/tasks/$TASK_E.json $PULSEN_HOME/backup-taskE.json`（`state/tasks/` の外に置く）
  4. `printf '{ "broken":' > $PULSEN_HOME/state/tasks/$TASK_E.json`
  5. `pulsen ls; echo $?`
  6. `pulsen show $TASK_E; echo $?`
  7. `cp $PULSEN_HOME/backup-taskE.json $PULSEN_HOME/state/tasks/$TASK_E.json` → `pulsen show $TASK_E; echo $?` → `pulsen ls; echo $?`
  8. `cp $PULSEN_HOME/state/tasks/$TASK_F.json $PULSEN_HOME/backup-taskF.json`
  9. スナップショット部分だけを不正にする（ファイル全体は JSON として妥当なまま保つ）:

      ```sh
      jq '.snapshot.initial = ""' "$PULSEN_HOME/state/tasks/$TASK_F.json" > /tmp/pulsen-taskF.json \
        && mv /tmp/pulsen-taskF.json "$PULSEN_HOME/state/tasks/$TASK_F.json"
      jq '.task_status, .execution, .snapshot.initial' "$PULSEN_HOME/state/tasks/$TASK_F.json"
      ```

  10. `pulsen ls; echo $?` → `pulsen ls --state pending; echo $?`
  11. `pulsen show $TASK_F; echo $?` → リトライ上限の併記を確認する
  12. `cp $PULSEN_HOME/backup-taskF.json $PULSEN_HOME/state/tasks/$TASK_F.json` → `pulsen show $TASK_F; echo $?`
- **期待結果:**
  - 手順1・2: 人間可読な JSON が読め、show は変わらず 0。
  - 手順5: exit code **0**。`TASK_E` の**ファイルパス**と読み取り不能である旨が報告され、残り（`TASK_A` / `TASK_B` / `TASK_C` / `TASK_F` / `TASK_G` と TC-16・TC-17 で追加した `TASK_H` / `TASK_I` / `TASK_J`）は通常どおり表示される。
  - 手順6: パースエラーの内容と対象ファイルパス（`state/tasks/<TASK_E>.json`）が表示され、**非0**で終わる。書き込みは起きない。
  - 手順7: 修復後は show が 0 で詳細を表示し、`ls` から読み取り不能の報告が消えて `TASK_E` が通常の行に戻る。
  - 手順9: `task_status` と `execution` は読めるまま、`snapshot.initial` が空文字になっている（`crates/pulsen/src/adapter/task_file.rs` は `initial` の値制約の破れをスナップショット側の破損として扱い、タスク側フィールドは読める `SnapshotUnreadable` に分類する）。
  - 手順10: `TASK_F` は**行として表示され**、スナップショット読み取り不能の印が付く。exit code 0。`--state pending` でも `TASK_F` が現れる（実行状態はタスク側フィールドなので絞り込みが効く）。
  - 手順11: exit code 0。タスクファイル由来の項目（ステータス・実行状態・カウンタ・更新日時）は表示され、スナップショットが読めない理由が注記され、**定義済みステータス一覧は表示されない**。リトライ上限は「不明（導出不能）」として表示され、judge / spawn の上限は config 由来のため通常どおり表示される。
  - 手順12: 定義済みステータス一覧を含む通常表示に戻り 0。
- **確認ポイント:** `Corrupt` は行にならずパス付きで報告され、`SnapshotUnreadable` は**行として出て絞り込みの対象になる**という2系統の違いが出力から読めること（plan.md「spec との差分として提起するもの」の1点目。実装はユースケース側の DTO に従う）。手順11 の「不明」が確認項目3 手順7 の「併記なし」と別の文言であること。破損ファイルに対して `ls` / `show` が書き込み・正規化を一切行わないこと（修復材料を失わせない）。

### 8. 読み取り専用であることの外形確認

- **対応する受け入れ基準:** AC-7
- **対応する手順書:** `monitoring.md` TC-29 全手順（TC-14 の後に実行する）
- **前提:** 確認項目7 の直後。ロック保持の確認はリポジトリのディレクトリで `cargo run` するため、`cd /Users/hikaru/github.com/tuanemuy/pulsen` 済みのシェルで行う。
- **目的:** `ls` / `show` が排他ロックを取得しないこと（別の保持者がいても通常どおり 0 で結果を返すこと）、`show` が workspace_path の存在検証を行わないこと、tick の書き込みと同時に読んでも書きかけを観測しないことを確認する。
- **手順:**
  1. `pulsen show $TASK_C` で workspace_path を控える → `rm -rf $PULSEN_HOME/worktrees/$TASK_C`
  2. `pulsen show $TASK_C; echo $?`
  3. 別の保持者がいる状態を作る（`locked` が出るまで待つ）:

      ```sh
      mkdir -p "$PULSEN_HOME/state"
      sleep 30 | cargo run -p pulsen --example lock_holder -- "$PULSEN_HOME/state/lock" &
      ```

  4. 保持中に `pulsen ls; echo $?` / `pulsen ls --all; echo $?` / `pulsen show $TASK_B; echo $?`
  5. 対照として保持中に `pulsen tick; echo $?`
  6. `wait` で保持を終えたあと、`pulsen ls; echo $?` が手順4 と同じ結果になることを確認する
  7. tick と同時の読み取りを数回:

      ```sh
      for i in 1 2 3; do pulsen tick & pulsen ls; echo $?; wait; sleep 2; done
      ```

  8. 型としての担保を grep で補う:

      ```sh
      grep -rn 'lock()' crates/pulsen/src/cli/
      grep -rn 'exists\|try_exists' crates/pulsen/src/application/show_task.rs crates/pulsen/src/cli/render/show.rs
      ```

- **期待結果:**
  - 手順2: workspace_path は記録どおりそのまま表示され（存在検証は行われない）、exit code 0。
  - 手順4: 3つとも通常どおり結果を表示して exit code 0（ロック競合によるスキップも待ちも起きない）。
  - 手順5: tick はロック競合として何もせずスキップする（exit code 0）。`ls` / `show` との扱いの差がここで見える。
  - 手順6: 手順4 と同じ結果。
  - 手順7: `ls` が毎回 0 で、行が欠けたり途中まで書かれた JSON に由来する破損報告が出たりしない（タスクファイルはアトミック置換で更新される）。
  - 手順8: 1つ目のヒットが `cli/wire.rs` の定義と `cli/tick.rs` の呼び出しだけで、`cli/ls.rs` / `cli/show.rs` に現れないこと。2つ目のヒットが0件であること。
- **確認ポイント:** 手順4 で「ロックを取れなかったのでスキップした」旨のメッセージが**出ないこと** — 出るならロックを取りに行っている。手順1 の worktree 削除後も表示が変わらないこと（`Path::exists()` を呼んでいない証拠。pages ※9）。

## エッジケース・異常系

すべてのケースで、**タスクファイルとrunディレクトリが変更されていないこと**（`ls` / `show` は読み取り専用）を共通の確認手段とする。

### 1. 存在しないタスクID・不正な書式・長さの境界

- **対応する受け入れ基準:** AC-3（不在の扱い）、AC-6（入力境界）
- **対応する手順書:** `monitoring.md` TC-19 全手順、TC-20 全手順、TC-32 全手順
- **前提:** 確認項目8 の直後。
- **目的:** 不在のIDに無言で空を返さないこと、書式の検証エラーとタスク不在エラーが**別の種類のエラーとして**区別できることを確認する。
- **手順:**
  1. `pulsen show no-such-task-0000; echo $?`
  2. `pulsen show 'TASK_A!'; echo $?`（`[a-z0-9-]` 以外の文字）
  3. `pulsen show -- -abc; echo $?`（先頭が `-`）
  4. `pulsen show $(printf 'a%.0s' $(seq 1 65)); echo $?`（65文字）
  5. `pulsen show $(printf 'a%.0s' $(seq 1 64)); echo $?`（64文字。書式は有効）
  6. `pulsen show ""; echo $?`（空文字）
- **期待結果:** すべて非0。手順1 と手順5 は「タスクが見つからない」旨、手順2・3・4・6 は書式・長さの検証エラーで、両者の文言が区別できる。空表示にはならない。
- **確認ポイント:** 手順4 と手順5 が**同じ非0でも別の理由**を述べること — 64文字が受理されて不在エラーになることが、長さ上限の境界の確認になる。手順3 が clap のオプション解釈に食われず、`--` の後で値として届くこと。

### 2. `--state` への不正値・表記揺れ・空文字

- **対応する受け入れ基準:** AC-2
- **対応する手順書:** `monitoring.md` TC-21 全手順、TC-33 手順1・2
- **前提:** エッジケース1 の直後。
- **目的:** 実行状態が固定6値であり、それ以外は**有効値の一覧を添えて**非0で拒否されることを確認する。
- **手順:**
  1. `pulsen ls --state stoped; echo $?`（typo）
  2. `pulsen ls --state Pending; echo $?`（大文字混じり）
  3. `pulsen ls --state ""; echo $?`（空文字）
  4. `pulsen ls --status ""; echo $?`（対照。タスクステータス側の空文字）
- **期待結果:** 手順1〜3 は非0で、`pending` / `launching` / `running` / `completed` / `failed` / `stopped` の6値が列挙されたエラーが表示される。手順4 は **exit code 0** で該当0件（タスクステータスは検証しない語彙）。
- **確認ポイント:** 有効値一覧が clap のエラー整形ではなく本ツールの拒否文言として出ること（`--state` を `value_parser` で先に弾いていない証拠）。手順3 と手順4 の扱いが逆であることが、検証する語彙と検証しない語彙の線引きを示す。

### 3. config.yaml 不在・パース不能

- **対応する受け入れ基準:** AC-2 / AC-3（前段の拒否）
- **対応する手順書:** `monitoring.md` TC-27 全手順
- **前提:** エッジケース2 の直後。確認項目6 手順12 で `config.yaml` が復元済みであること。
- **目的:** グローバル設定が読めないとき、`ls` / `show` が状態を変更せず非0で終了することを確認する。
- **手順:**
  1. `mv $PULSEN_HOME/config.yaml $PULSEN_HOME/config.yaml.bak`
  2. `pulsen ls; echo $?`
  3. `pulsen show $TASK_A; echo $?`
  4. `printf 'agents: [\n' > $PULSEN_HOME/config.yaml`
  5. `pulsen ls; echo $?`
  6. 復元（必ず実行する）: `mv $PULSEN_HOME/config.yaml.bak $PULSEN_HOME/config.yaml` → `pulsen ls; echo $?`
- **期待結果:** 手順2・3 は非0で、グローバルホームが未初期化である旨・解決後のホームパス（`$HOME/pulsen-manual-test`）・作成が必要であることが表示される。手順5 も非0でパースエラーの位置が示される。手順6 で通常の一覧に戻り 0。
- **確認ポイント:** ホームの解決自体は成功しており、拒否されるのが config の読み込みであること（表示されるパスが `--home` / `PULSEN_HOME` / 既定の解決順の結果になっていること）。拒否の過程でタスクファイルに触れないこと。

### 4. 状態ディレクトリの走査不能（権限エラー）

- **対応する受け入れ基準:** AC-2
- **対応する手順書:** `monitoring.md` TC-28 全手順
- **前提:** エッジケース3 の直後。**root 実行では `chmod 000` が効かないため実施できない** — その場合はこの項目をスキップし、確認した環境（OS・root か否か）とともに Issue のコメントに残す（AC-9）。
- **目的:** 走査自体ができない I/O エラーが、破損ファイルの報告（0 で継続）とは別に、**実行環境エラーとして非0**で報告されることを確認する。
- **手順:**
  1. `chmod 000 $PULSEN_HOME/state/tasks` → `pulsen ls; echo $?`
  2. `chmod 755 $PULSEN_HOME/state/tasks`（復元）
  3. `chmod 000 $PULSEN_HOME/state/archive` → `pulsen ls --all; echo $?` → `pulsen ls; echo $?`
  4. `chmod 755 $PULSEN_HOME/state/archive`（復元）→ `pulsen ls --all; echo $?`
- **期待結果:** 手順1 は非0（走査の失敗が実行環境エラーとして表示される）。手順3 は `--all` が非0、`--all` なしは **0**（アーカイブ側に依存しない）。手順4 で通常表示に戻り 0。
- **確認ポイント:** 走査の失敗が「破損ファイル1件の報告」に化けないこと — 一覧の一部だけが欠けた状態を 0 で返すと、利用者は欠落に気づけない。

### 5. exit / runディレクトリが読めない場合の表示継続

- **対応する受け入れ基準:** AC-4
- **対応する手順書:** `monitoring.md` TC-26 全手順
- **前提:** エッジケース4 の直後。手順5 は権限制限が効く環境でのみ実施する（root ではスキップして記帳する）。
- **目的:** `read_exit` の破損と `attempt_exists` の失敗が**エラーに昇格せず**、当該項目に読めない旨を注記して表示を続け 0 で終わることを確認する。
- **手順:**
  1. `cp $PULSEN_HOME/state/runs/$TASK_C/attempt-3/exit /tmp/pulsen-exit.bak`
  2. `printf 'abc' > $PULSEN_HOME/state/runs/$TASK_C/attempt-3/exit`
  3. `pulsen show $TASK_C; echo $?`
  4. `cp /tmp/pulsen-exit.bak $PULSEN_HOME/state/runs/$TASK_C/attempt-3/exit`（復元）
  5. `chmod 000 $PULSEN_HOME/state/runs/$TASK_C/attempt-3` → `pulsen show $TASK_C; echo $?`
  6. `chmod 755 $PULSEN_HOME/state/runs/$TASK_C/attempt-3`（復元）→ `pulsen show $TASK_C; echo $?`
- **期待結果:** 手順3 は exit code **0** で、exit の項目にのみ読めない旨の注記が付き、他の項目（カウンタ・凍結要因・パス）は通常どおり表示される。手順5 も exit code 0 で、runディレクトリの存在確認が失敗した旨の注記付きで表示が続く。手順6 で exit の値 1 を含む通常表示に戻る。
- **確認ポイント:** 手順3 の注記が「exit が無い（未終了）」と**区別できる**こと — 同じ表示なら、実行中と読み取り失敗が見分けられない。手順5 の注記が「runディレクトリが存在しない」と区別できること（不在と確認失敗は別の結末）。読めなかったファイルを書き換え・削除しないこと。

### 6. タスク0件・`state/` 不在

- **対応する受け入れ基準:** AC-2 / AC-3
- **対応する手順書:** `monitoring.md` TC-30 全手順、TC-31 全手順
- **前提:** エッジケース5 の直後。別のホームを使うのでここまでの状態には影響しない。
- **目的:** 一覧の「空」と単一照会の「不在」が**別の扱い**であること、`state/` が存在しなくてもエラーにしないことを確認する。
- **手順:**
  1. `mkdir -p $HOME/pulsen-manual-test-empty && printf 'agents: {}\n' > $HOME/pulsen-manual-test-empty/config.yaml`
  2. `PULSEN_HOME=$HOME/pulsen-manual-test-empty pulsen ls; echo $?`
  3. `PULSEN_HOME=$HOME/pulsen-manual-test-empty pulsen ls --all; echo $?`
  4. `PULSEN_HOME=$HOME/pulsen-manual-test-empty pulsen show $TASK_A; echo $?`
  5. `pulsen ls --status no-such-status; echo $?`（元のホーム。該当0件）
- **期待結果:** 手順2・3 は空である旨を表示して exit code 0（`state/` も `state/archive/` も不在だが失敗しない）。手順4 は**非0**（タスク不在）。手順5 は 0 で空表示。
- **確認ポイント:** 手順2 と手順4 の差 — 一覧は「0件」という答えを返せるが、単一照会は答えを返せないので拒否する。読み取りが `state/` を**作らない**こと（手順2・3 の後に `ls $HOME/pulsen-manual-test-empty` で `state` が生えていないこと）。

## 既存機能への影響確認

- **`pulsen --help` の表示:** サブコマンドが `add` / `tick` / `ls` / `show` / `help` になり、`wrapper` は現れない（ADR-077）。`abort` / `retry` / `set-status` が増えていないこと（#5）。`pulsen ls --help` に `--status` / `--state` / `--all` が並び、`pulsen show --help` に位置引数のタスクIDが並び、どちらにも `--home` が global フラグとして現れること。

- **`add` / `tick` の経路:** 本スライスが既存経路に足すのは `RunStore::attempt_exists`（トレイトのメソッド1つと `FsRunStore` の実装1つ）だけで、tick は呼ばない。`.thread/3/testing.md` の確認項目1（exit 0 → completed → 遷移）を `wf-echo` の追加登録1件でスポットチェックし、tick のサマリー・報告の見出しと刻みが Issue #3 時点と同じであることを確認する。併せて `grep -rn 'attempt_exists' crates/pulsen/src/` のヒットが `adapter/run_store.rs` の実装と `application/show_task.rs` の呼び出しに限られ、`application/tick*` に現れないことを確認する。

- **`cli/render.rs` の分割（adr.md ADR-002）:** 表示層を `cli/render/` に分割するため、既存の文言が巻き添えで変わっていないことを確認する。`pulsen add` の成功・失敗の文言と `pulsen tick` のサマリー（「起動 / 起動確認 / 判定確定 / 遷移 / 実行待ちへ復帰 / 凍結 / 通知 / 終端処理 / gcで削除 / gcで削除できず」の順）と報告の4見出し（「失敗を記録」「起動の結果が未確定」「スキップ」「後始末が残っている」）が Issue #3 時点と同一であること。`archived` / `gc_deleted` / `gc_errors` は引き続き値の入る経路を持たない（#6）。

- **時刻の表示形式:** `ls` の更新日時、`show` の更新日時・`notified_at`・starttime の wall が、タスクファイルの直列化（RFC3339 UTC）と同じ形式で読めること。形式が揃っていないと、通知の at-least-once をタスクファイルと突き合わせて検証できない。

- **実運用ホームの非汚染:** 全項目の実行後に `ls -a "$HOME/.pulsen" 2>/dev/null` が実行前と変わらないこと。本書が触るのは `$HOME/pulsen-manual-test`（`PULSEN_HOME`）・`$HOME/pulsen-manual-test-empty`・`$HOME/pulsen-test-repo`・`/tmp/pulsen-notify.log`・`/tmp/pulsen-exit.bak`・`/tmp/pulsen-taskG.json` / `/tmp/pulsen-taskF.json` に限られる。

- **後片付け:**

    ```sh
    ps -ef | grep -E 'pulsen wrapper|sleep 3000' | grep -v grep   # 残留プロセスが無いこと
    pkill -f 'sleep 3000'                                        # 残っていれば(TASK_B / TASK_H)
    git -C $HOME/pulsen-test-repo worktree list
    rm -rf $PULSEN_HOME $HOME/pulsen-manual-test-empty $HOME/pulsen-test-repo \
           /tmp/pulsen-notify.log /tmp/pulsen-exit.bak
    ```

  手順書の後片付け1（`pulsen abort $TASK_B`）は #5 のため実行できないので、`sleep 3000` を直接終了させる。worktree を作ったリポジトリごと削除するので `git worktree prune` は不要。

- **落とした手順の記帳（AC-9）:** 実行しなかったのは `monitoring.md` TC-09 手順4・TC-15・TC-34 手順1（いずれも #5）と、`cleanup.md` TC-13 / TC-14 / TC-15 / TC-17 / TC-23（#6。TC-17 手順1 は TC-19 と重複）。TC-05 / TC-06 / TC-10 / TC-11 / TC-25 は**アーカイブ済みの前提を手動移動で作った読み替え**で消化しており、tick の終端処理がアーカイブを生む経路は確認していない（`PAGE-ls-004` / `PAGE-show-008` / `TC-task-show-task-031` は部分消化）。権限制限が効かない環境でスキップしたエッジケース4・5 手順5 があれば、確認した環境（OS・root か否か）を添える。以上を Issue #4 のコメントに残し、チェックリストの該当行にはチェックを付けない（steps.md ステップ13）。
