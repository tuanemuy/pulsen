# 動作確認計画 — Issue #3: tick による観測・判定・ステータス遷移(リトライ・凍結・通知)

**Issue:** #3
**作成日:** 2026-08-14

---

## 確認環境

この Issue の変更を確認するために必要な手順のみ記載する（プロジェクト全体のセットアップは省略）。

本スライスに存在するコマンドは `pulsen add` / `pulsen tick` / `pulsen wrapper`（隠しサブコマンド）の3つで、Issue #2 から増えない（`crates/pulsen/src/cli/args.rs` の `Command`）。`ls` / `show` / `abort` / `retry` / `set-status` は未実装（#4 / #5）のため、状態の観測はすべて **`state/tasks/<task-id>.json` の直読**と **`state/runs/<task-id>/attempt-<n>/` 配下のファイルの直読**で行う（plan.md「テスト方針 — 手動確認」）。Web UI は無く、確認はすべてターミナル上の CLI 実行になる。

### 検証環境の起動

`.envrc` は `use flake` のみ。direnv がこのリポジトリで許可済みであれば、リポジトリに `cd` するだけで `flake.nix` の devShell（cargo / rustc / clippy / rustfmt / git）に入る。direnv を使わない場合は各コマンドを `nix develop -c <command>` で包む。

```sh
cd /Users/hikaru/github.com/tuanemuy/pulsen
cargo --version    # devShell が効いていることの確認
git --version      # worktree 操作で使う（flake.nix の buildInputs に git がある）
```

ビルドとバイナリの解決。ワークスペースの target ディレクトリはリポジトリ直下（`.gitignore` の `/target`）で、bin 名は `pulsen`（`crates/pulsen/Cargo.toml` の package 名 + `src/main.rs`。`[[bin]]` セクションは無い）。

```sh
cargo build
export PATH="$PWD/target/debug:$PATH"
pulsen --help
```

本書の手動確認では `examples/`（`agent_probe` / `spawn_probe` / `lock_holder`、および本スライスで追加される `judge_probe`）は使わない — 判定・通知コマンドは手順書が定めるシェルスクリプトで供給する。`cargo build --examples` は不要（examples は `cargo test` が自動テストのためにビルドする）。

以降の手順はすべてこの PATH が通ったシェルから実行する。exit code は各コマンドの直後に `echo $?` で確認する。

自動テスト側の状態（本確認の前提となるグリーン状態。plan.md AC-7）:

```sh
cargo build --workspace --locked
cargo test --workspace --locked --no-fail-fast -- --nocapture
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo fmt --all --check
```

4つとも `.github/workflows/ci.yml` の各ステップと同じ形にそろえてある（AC-7 が要求するのは `cargo build` / `cargo test` / `cargo clippy -- -D warnings` / `cargo fmt --check`）。`--nocapture` を付けるのは、適合スイートのスキップ行（`SKIP …`）が標準出力に出ないとステップ15 の記帳ができないため（ci.yml「テストする」ステップのコメント）。

AC-7 の隔離を機械的に確認する3つの grep:

```sh
grep -rnE 'cfg\([^)]*\b(unix|windows|target_os|target_family)\b' crates/*/src/
grep -rn '#\[allow(unsafe_code)\]' crates/
grep -n -A 8 '^\[dependencies\]' crates/pulsen-domain/Cargo.toml crates/pulsen/Cargo.toml
```

- 1つ目: `crates/pulsen-domain/` が1件もヒットせず、`crates/pulsen/src/` 側のヒットが `util/atomic.rs` / `adapter/process.rs` / `adapter/task_repository.rs` の3ファイルだけであること。合否はこの**ファイル集合**で判定する — 件数は本スライスの実装で増える（`adapter/process.rs` に `kill` / `try_kill_remnants` / `starttime_of` が入り、`origin/main` の 4 / 10 / 1 件から 4 / 20 / 1 件になる）。新設の `adapter/command_runner.rs` がヒットしないこと — CommandRunner は `run_agent` と同じ符号化関数を共有し、OS 依存分岐を自前で持たない（plan.md AC-7）。`crates/pulsen-conformance/src/lib.rs` の2件は適合ハーネスが権限制限の効き目を probe する分岐で、本番の実行経路には乗らない。
- 2つ目: `crates/pulsen/src/adapter/process.rs` の1件（Windows ハンドル抑止モジュール。ADR-100）だけであること。
- 3つ目: `pulsen-domain` の `[dependencies]` が空のまま（依存の行が1つも無く、空行を挟んで `[lints.rust]` が続く）で、`pulsen` の本番依存が `pulsen-domain` / `clap` / `getrandom` / `serde` / `serde_json` / `serde_yaml_ng` / `tempfile` の7行から増えていないこと（ADR-023 の6クレート + 内部クレート）。`-A 8` はこの7行と `[dev-dependencies]` の手前までを1画面に収めるための幅で、`pulsen-domain` 側は空判定なので影響しない。

### 本スライスでの実行範囲（手順書からの読み替え）

plan.md「テスト方針 — 手動確認」の表が定める制約を、本書の全項目に共通の前提として先に置く。

- **`show` / `ls` が読む値は `state/tasks/<task-id>.json` の直読で代替する**（#4）。run ディレクトリのファイルは JSON なので、`cat exit` の出力は `0` ではなく `.code` に終了コードを持つ**整形 JSON**（2スペース字下げの3行・末尾に改行なし）になる（ADR-080。`crates/pulsen/src/adapter/run_store.rs` の `encode` が `to_vec_pretty`）。本書の期待結果は綴りではなく `jq '.code'` で読める値で書く。
- **`abort` / `retry` / `set-status` を使う手順は実行しない**（#5）。`abort` を前提とする3つの TC（setup TC-35 / intervention TC-15 / intervention TC-24）は、**上限超過での凍結に読み替える**。
- **終端処理（クリーンアップ・アーカイブ）は行われない**（#6。`Branch::Cleanup` は引き続き未配線。`.thread/2/adr.md` ADR-101）。`done` などのクリーンアップステータスに到達したタスクは `pending` のまま滞留し、tick のサマリーにも報告にも現れない。これは異常ではない。
- **復元手順は実行範囲外でも必ず実行する**: task-execution TC-03 手順12（`pipeline.yaml` の prompt を戻す）、setup TC-39 手順4（`judge_timeout: 60s` に戻す）、setup TC-35 手順4（`notify_cmd` を戻す）。復元しないと後続の TC の期待が必ず外れる。
- 手順書は**記載順の実行**を前提にしている（`spec/manual-tests/task-execution.md`「実行上の注意」）。本書の項目も記載順に実行し、各項目の冒頭にその項目が前提とする状態を明記してある。
- **連続する tick の間は 2〜3 秒あける**（同「実行上の注意」）。1回のエージェント実行の消化には「起動 → spawn確認 → 判定 → 遷移」の約4 tick を要する。
- tick のサマリーには同時に走っている他タスクの処理も含まれる。各項目で確認するのは**対象タスクIDに関する行・状態のみ**とする。

### 検証用のフィクスチャ準備

手動確認は3つの手順書に由来するので、フィクスチャも手順書ごとに分ける。パスは各手順書の記載どおりに保つ（読み替えによる取り違えを避けるため）。**例外は `intervention.md` の `PMT` の1つだけ** — 手順書の値（`$HOME/pulsen-manual-test`。`spec/manual-tests/intervention.md:25`）はフィクスチャB の `SETUP_HOME` と同一パスなので、フィクスチャC では `$HOME/pulsen-intervention-test` に読み替える。フィクスチャC は冒頭で `rm -rf "$PMT"` を実行するため、手順書どおりの値に戻すとフィクスチャB のホームごと消える。

#### フィクスチャA — `spec/manual-tests/task-execution.md`（TC-03 / 05 / 06 / 07 / 13 / 14 / 15 / 17 / 19 / 20 / 21 / 22 / 23 用）

同ドキュメントの「事前準備」1〜6 をそのまま実行する。

1. テスト領域と分離ホームを初期化する。

    ```sh
    rm -rf /tmp/pulsen-test
    mkdir -p /tmp/pulsen-test/home/workflows /tmp/pulsen-test/bin
    export PULSEN_HOME=/tmp/pulsen-test/home
    ```

2. グローバル設定を作成する（バックアップは通知の一時無効化・復元に使う）。

    ```sh
    cat > /tmp/pulsen-test/home/config.yaml <<'EOF'
    agents:
      shell:
        cmd: ["sh", "-c", "{input}"]
      broken:
        cmd: ["/tmp/pulsen-test/bin/no-such-binary"]
    notify_cmd: ["sh", "-c", "echo \"stopped $TASK_ID $WORKFLOW $TASK_STATUS\" >> /tmp/pulsen-test/notify.log"]
    EOF
    cp /tmp/pulsen-test/home/config.yaml /tmp/pulsen-test/config.bak
    ```

3. 本スライスで使うワークフロー定義を配置する（手順書「テストデータ」の記載どおり）。

    ```sh
    cat > /tmp/pulsen-test/home/workflows/pipeline.yaml <<'EOF'
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
    EOF
    cp /tmp/pulsen-test/home/workflows/pipeline.yaml /tmp/pulsen-test/pipeline.bak

    cat > /tmp/pulsen-test/home/workflows/fail.yaml <<'EOF'
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
    EOF

    cat > /tmp/pulsen-test/home/workflows/flaky.yaml <<'EOF'
    workflow: flaky
    agent: shell
    initial: work
    statuses:
      work:
        prompt: "if [ -f done.marker ]; then exit 0; else touch done.marker; exit 1; fi"
        next: done
      done:
        run: cleanup
    EOF

    cat > /tmp/pulsen-test/home/workflows/sleeper.yaml <<'EOF'
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
    EOF

    cat > /tmp/pulsen-test/home/workflows/judgefail.yaml <<'EOF'
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
    EOF

    cat > /tmp/pulsen-test/home/workflows/pr-review-watch.yaml <<'EOF'
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
    EOF

    cat > /tmp/pulsen-test/home/workflows/broken.yaml <<'EOF'
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
    EOF

    cat > /tmp/pulsen-test/home/workflows/longrun.yaml <<'EOF'
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
    EOF

    cat > /tmp/pulsen-test/home/workflows/fail0.yaml <<'EOF'
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
    EOF

    cat > /tmp/pulsen-test/home/workflows/exit20.yaml <<'EOF'
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
    EOF

    cat > /tmp/pulsen-test/bin/check-reviews.sh <<'EOF'
    #!/bin/sh
    if [ -f /tmp/pulsen-test/check-broken ]; then exit 1; fi
    if [ -f /tmp/pulsen-test/review-flag ]; then exit 0; fi
    exit 20
    EOF
    chmod +x /tmp/pulsen-test/bin/check-reviews.sh
    ```

4. 対象リポジトリを作成する（worktree 内でコミットするため identity をリポジトリローカルに設定する）。

    ```sh
    git init -b main /tmp/pulsen-test/repo
    git -C /tmp/pulsen-test/repo config user.name pulsen-test
    git -C /tmp/pulsen-test/repo config user.email pulsen-test@example.com
    git -C /tmp/pulsen-test/repo commit --allow-empty -m init
    ```

5. 通知ログを空にする。

    ```sh
    : > /tmp/pulsen-test/notify.log
    ```

6. フラグファイルが存在しないことを確認する。

    ```sh
    ls /tmp/pulsen-test/review-flag /tmp/pulsen-test/check-broken 2>/dev/null; echo $?
    ```

7. 確認の共通ヘルパー。

    ```sh
    cat "$PULSEN_HOME/state/tasks/<task-id>.json"
    ls -R "$PULSEN_HOME/state/runs/<task-id>/"
    cat "$PULSEN_HOME/state/runs/<task-id>/attempt-<n>/exit"; echo
    cat /tmp/pulsen-test/notify.log
    ```

    JSON の項目を絞って見たい場合は `jq` を使ってよい（`/usr/bin/jq` を Issue #1 の確認で確認済み。devShell には含まれないためシステムの `jq` を使う）。手順書が `pulsen show` で読む値と直読の対応は次のとおり — 実行状態 = `.execution.state`、タスクステータス = `.task_status`、attempt_count / judge_attempt_count / spawn_fail_count = `.counters.*`、attempt 番号・runディレクトリ = `.current_attempt.number` / `.current_attempt.run_dir`、PID・kill同定子 = `.current_attempt.process.pid` / `.current_attempt.process.kill_ident`、凍結要因 = `.execution.reason`、通知済み時刻 = `.execution.notified_at`、直近の失敗要因 = `.last_failure.kind` / `.last_failure.message`（`crates/pulsen/src/adapter/task_file.rs` の DTO）。

`.execution` は `state` をタグに持つ直和なので、**`.execution.state` の値は状態名の文字列**（`pending` / `launching` / `running` / `completed` / `failed` / `stopped`）で、`reason` / `notified_at` / `recorded_at` は `.execution` の同じ階層に並ぶ。オブジェクト全体を照合したいときは `.execution` を、状態名だけを見たいときは `.execution.state` を指す。

`draft.yaml` / `broken-syntax.yaml` / `wtloss.yaml` / `repo2` を使う TC（TC-02 / 09 / 12 / 18）は本スライスの対象外なので、これらのフィクスチャは作成を省略してよい。対象に含まれる TC-20 は手順1 で `draft.yaml` を使うが、その手順が要求するのは「破損させない別のタスクが同じ tick に居ること」だけなので、エッジケース6 では `pipeline` の2件目で代替する。

#### フィクスチャB — `spec/manual-tests/setup.md`（TC-09 / 10 / 11 / 37 / 38 / 39 / 47 用）

同ドキュメントの「事前準備」1〜3 と TC-01 手順1 を先行実行する（plan.md が TC-09 の前提として明示している）。フィクスチャA とは別のホームを使うので混ざらない。

`setup.md` TC-35 はこのフィクスチャには含まない — `intervention.md` TC-15 と同じ「notify_cmd 未定義 → 後から定義して catch-up」を問うので、確認項目9 手順7〜10 の1系列（フィクスチャC）で消化する。

```sh
export SETUP_HOME="$HOME/pulsen-manual-test"
export SETUP_REPO="$HOME/pulsen-test-repo"
export SETUP_WORK="$HOME/pulsen-manual-work"

rm -rf "$SETUP_HOME" "$SETUP_WORK" "$SETUP_REPO"
mkdir -p "$SETUP_HOME/workflows" "$SETUP_WORK"
git init -b main "$SETUP_REPO"
git -C "$SETUP_REPO" config user.name pulsen-test
git -C "$SETUP_REPO" config user.email pulsen-test@example.com
git -C "$SETUP_REPO" commit --allow-empty -m init

cat > "$SETUP_HOME/judge.sh" <<'EOF'
#!/bin/sh
echo "judged: task=$TASK_ID exit=$EXIT_CODE" >> "$HOME/pulsen-manual-test/judge.log"
exit "$(cat "$HOME/pulsen-manual-test/judge-exit")"
EOF
chmod +x "$SETUP_HOME/judge.sh"

cat > "$SETUP_HOME/config.yaml" <<'EOF'
agents:
  shell:
    cmd: ["sh", "-c", "{input}"]
    skill_input: "{skill}"
  claude:
    cmd: claude -p {input} --model {model}
    skill_input: "/skill {skill}"
notify_cmd: ["sh", "-c", "echo \"$TASK_ID $WORKFLOW $TASK_STATUS\" >> \"$HOME/pulsen-manual-test/notify.log\""]
judge_attempt_limit: 3
judge_timeout: 60s
spawn_fail_limit: 3
EOF
cp "$SETUP_HOME/config.yaml" "$SETUP_WORK/config.bak"

cat > "$SETUP_HOME/workflows/implement.yaml" <<'EOF'
workflow: implement
agent: shell
initial: queued
statuses:
  queued:
    prompt: "echo planned > plan.txt"
    next: implemented
  implemented:
    prompt: "echo implemented >> plan.txt"
    timeout: 30m
    next: review_waiting
  review_waiting:
    run: wait
  done:
    run: cleanup
EOF

cat > "$SETUP_HOME/workflows/judge-demo.yaml" <<'EOF'
workflow: judge-demo
agent: shell
initial: checking
statuses:
  checking:
    prompt: "echo checking"
    judge: ["sh", "-c", "\"$HOME/pulsen-manual-test/judge.sh\""]
    next: finished
  finished:
    run: wait
  done:
    run: cleanup
EOF
```

フィクスチャB を使う項目は `--home "$SETUP_HOME"` を明示するか、そのシェルで `export PULSEN_HOME="$SETUP_HOME"` に切り替えてから実行する。

#### フィクスチャC — `spec/manual-tests/intervention.md`（TC-01 / TC-15 / TC-24 用）

同ドキュメント「テストデータ」を、本スライスで使う範囲（`wf-fail` と notify_cmd の証跡）に絞って作る。`<PMT>` は絶対パスに展開して書き込む（YAML 内では環境変数が展開されない）。

```sh
export PMT="$HOME/pulsen-intervention-test"
rm -rf "$PMT"
mkdir -p "$PMT" "$PMT/home/workflows"

git init -b main "$PMT/repo"
git -C "$PMT/repo" config user.name pulsen-test
git -C "$PMT/repo" config user.email pulsen-test@example.com
git -C "$PMT/repo" commit --allow-empty -m init

cat > "$PMT/notify.sh" <<EOF
#!/bin/sh
echo "\$(date '+%H:%M:%S') TASK_ID=\$TASK_ID WORKFLOW=\$WORKFLOW TASK_STATUS=\$TASK_STATUS" >> "$PMT/notify.log"
EOF
chmod +x "$PMT/notify.sh"

cat > "$PMT/home/config.yaml" <<EOF
agents:
  shell:
    cmd: ["sh", "-c", "{input}"]
  shell2:
    cmd: ["sh", "-c", "{input}"]
notify_cmd: ["sh", "$PMT/notify.sh"]
EOF
cp "$PMT/home/config.yaml" "$PMT/config.bak"

cat > "$PMT/home/workflows/wf-fail.yaml" <<'EOF'
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
EOF

: > "$PMT/notify.log"
```

`wf-fail` の `work` は `retries` を持たないため、組み込みの既定リトライ上限 2（`WorkflowDefinition::DEFAULT_RETRY_LIMIT`。ADR-014）まで失敗を重ねてから凍結する。凍結は `attempt_count` 3 > 2 の tick で起きるので、1 attempt あたり3刻み（起動 → 起動確認 → 判定）× 3 attempt = **9回の tick** を要する（手順書の「目安 10 回」はこの9回を含む見積り）。フィクスチャC を使う項目は `--home "$PMT/home"` を明示する。

### デプロイ方法

なし（ローカルの検証環境のみで確認できる）。

## 確認項目

### 1. exit 0 の観測 → completed → 次 tick で next へ（デフォルト判定）

- **対応する受け入れ基準:** AC-2
- **対応する手順書:** `task-execution.md` TC-03 手順1〜9・11（`show` / `ls` はタスクファイル直読で代替。手順10 のクリーンアップ・アーカイブは #6 のため実行しない。**手順12 の復元は必ず実行する**）
- **前提:** フィクスチャA 直後。
- **目的:** `judge` 未定義のステータスで、エージェントの exit 0 が `default_judgement` により completed になり、その tick は `complete_run` までで止まり（1タスク1tick1ステップ）、次の tick の `advance` でタスクステータスが `next` へ進んで pending に戻ることを確認する。カウンタが 0 のまま維持されることもここで見る。
- **手順:**
  1. `pulsen add --workflow pipeline --repo /tmp/pulsen-test/repo; echo $?` → `export T3=<task-id>`
  2. 元YAMLを編集する（スナップショット非依存の確認。TC-03 手順2）:

      ```sh
      sed -i.orig 's/prompt: "echo planning .*"/prompt: "echo edited-should-not-appear"/' /tmp/pulsen-test/home/workflows/pipeline.yaml
      grep -n 'edited-should-not-appear' /tmp/pulsen-test/home/workflows/pipeline.yaml
      ```

  3. `pulsen tick; echo $?`（1回目・起動）→ `cat "$PULSEN_HOME/state/tasks/$T3.json"`
  4. 2〜3秒あけて `pulsen tick; echo $?`（2回目・spawn確認）→ タスクファイルを再確認
  5. `until [ -f "$PULSEN_HOME/state/runs/$T3/attempt-1/exit" ]; do sleep 1; done; cat "$PULSEN_HOME/state/runs/$T3/attempt-1/exit"; echo`
  6. `pulsen tick; echo $?`（3回目・判定）→ `cat "$PULSEN_HOME/state/tasks/$T3.json"`
  7. `pulsen tick; echo $?`（4回目・遷移）→ `cat "$PULSEN_HOME/state/tasks/$T3.json"`
  8. `cat "$PULSEN_HOME/state/runs/$T3/attempt-1/stdout.log"`
  9. `planned` / `implemented` について手順3〜7 を繰り返す（各4 tick。attempt 番号は 2 → 3）
  10. `git -C /tmp/pulsen-test/repo branch --list 'pulsen/*'; git -C /tmp/pulsen-test/repo log --oneline "pulsen/$T3"`
  11. `pulsen tick; echo $?; cat "$PULSEN_HOME/state/tasks/$T3.json"`（`done` 到達後の tick）
  12. 復元（後続項目のため必ず実行する）: `cp /tmp/pulsen-test/pipeline.bak /tmp/pulsen-test/home/workflows/pipeline.yaml && rm -f /tmp/pulsen-test/home/workflows/pipeline.yaml.orig && grep -n 'echo planning' /tmp/pulsen-test/home/workflows/pipeline.yaml`
- **期待結果:**
  - 手順5: `exit` の `.code` が 0（ADR-080 により整形 JSON）。
  - 手順6: サマリーの**「判定確定」**に T3 が現れ（「起動確認」ではない）、`.execution.state` が `completed`。**`.task_status` はまだ `queued`**（遷移は次の tick）。`.counters` は全0のまま。
  - 手順7: サマリーの「遷移」に T3 が現れ、`.task_status` が `planned`、`.execution.state` が `pending`、`.counters.attempt_count` と `.counters.judge_attempt_count` が 0。`.current_attempt` は attempt-1 のまま残っていてよい。
  - 手順8: `planning` が出力され、`edited-should-not-appear` は出ていない（スナップショットが使われた証拠）。
  - 手順9: 同じ4刻みで `implemented` → `done` まで進み、runディレクトリは `attempt-1` 〜 `attempt-3` の3つになる。
  - 手順10: ブランチ `pulsen/<T3>` に `plan` / `impl` / `review` の3コミットが積まれている。
  - 手順11: exit code 0。T3 は `.task_status` が `done`・`.execution.state` が `pending` のまま変化せず、サマリーにも報告にも現れない（`Branch::Cleanup` は #6 で未配線。ADR-101）。
- **確認ポイント:** 判定の tick（手順6）が `advance` まで進んでしまわないこと — 1タスク1tick1ステップが崩れると、以降の手順書の tick 刻みがすべてずれる（TC-exec-tick-025）。手順7 の tick が `.updated_at` を更新すること（書き込みを行った tick は必ずサマリーに現れる。ADR-092）。ステータス間で受け渡されるのは worktree の成果だけであること（`impl.txt` が `plan.txt` と同じ worktree に積まれている）。

### 2. judge 定義ありでの exit 0 → completed → 次ステータスへ

- **対応する受け入れ基準:** AC-2（判定コマンド経路）
- **対応する手順書:** `setup.md` TC-09 全手順（前提として事前準備1〜3・TC-01 手順1 を先行実行する = フィクスチャB）
- **前提:** フィクスチャB。`export PULSEN_HOME="$SETUP_HOME"` に切り替える。
- **目的:** `judge` が定義されたステータスで、判定コマンドが実際に起動されて exit 0 が completed と解釈されること、判定コマンドに `TASK_ID` / `EXIT_CODE` が渡ることを確認する。
- **手順:**
  1. `echo 0 > "$SETUP_HOME/judge-exit"; : > "$SETUP_HOME/judge.log"`
  2. `pulsen add --workflow judge-demo --repo "$SETUP_REPO"; echo $?` → `export T9=<task-id>`
  3. `pulsen tick; echo $?`（起動）
  4. 数秒待って `pulsen tick`（spawn確認）→ 2〜3秒あけて `pulsen tick`（判定）
  5. `cat "$SETUP_HOME/judge.log"`
  6. `cat "$SETUP_HOME/state/tasks/$T9.json"`
  7. `pulsen tick; echo $?; cat "$SETUP_HOME/state/tasks/$T9.json"`
- **期待結果:** 手順5 に `judged: task=<T9> exit=0` の行が1行だけ追記されている。手順6 で `.execution.state` が `completed`・`.task_status` が `checking` のまま。手順7 で `.task_status` が `finished`・`.execution.state` が `pending`（`finished` は `run: wait` なので以降の tick では何も起きない）。
- **確認ポイント:** `judge.log` の行が判定の tick で**1行だけ**増えること（判定は1 tick に1回）。`EXIT_CODE` が10進文字列で渡っていること。判定コマンドが `sh` を介さず直接起動されるにもかかわらず（`judge` の値が配列形式）、`$HOME` の展開はコマンド側の `sh -c` が行っていること — ツールがシェルを経由しないことの裏返し。

### 3. 一過性失敗の自動リトライと回復（カウンタのリセット）

- **対応する受け入れ基準:** AC-3
- **対応する手順書:** `task-execution.md` TC-05 手順1〜4 と手順6（手順5 のうち `done` への遷移までは確認し、アーカイブは #6 のため確認しない）、TC-19 手順1〜5（手順6 の `abort` / `set-status` 片付けは #5）、`setup.md` TC-10 手順1〜4（手順5 の `set-status` 片付けは #5）
- **前提:** 確認項目1 の直後（フィクスチャA、`pipeline.yaml` 復元済み）。setup 側はフィクスチャB。
- **目的:** 非0 exit が `fail_run` で failed になって `attempt_count` を消費し、次の tick が**同じタスクステータス**を新しい attempt 番号で再起動し、completed の確定で `attempt_count` が 0 に戻ることを確認する。judge の exit 10 も同じ failed 経路に落ちることを setup TC-10 で確かめる。
- **手順:**
  1. `export PULSEN_HOME=/tmp/pulsen-test/home`; `pulsen add --workflow flaky --repo /tmp/pulsen-test/repo; echo $?` → `export T5=<task-id>`
  2. `pulsen tick` を2〜3秒間隔で3回（起動 → spawn確認 → 判定）→ `cat "$PULSEN_HOME/state/tasks/$T5.json"`; `cat "$PULSEN_HOME/state/runs/$T5/attempt-1/exit"; echo`
  3. `pulsen tick; echo $?`（4回目・再起動）→ `cat "$PULSEN_HOME/state/tasks/$T5.json"`; `ls "$PULSEN_HOME/worktrees/$T5/"`
  4. `pulsen tick` を2〜3秒間隔で2回（spawn確認 → 判定）→ `cat "$PULSEN_HOME/state/tasks/$T5.json"`
  5. `pulsen tick; echo $?; cat "$PULSEN_HOME/state/tasks/$T5.json"`（遷移）
  6. `cat /tmp/pulsen-test/notify.log`
  7. setup 側（`export PULSEN_HOME="$SETUP_HOME"`）: `echo 10 > "$SETUP_HOME/judge-exit"` → `pulsen add --workflow judge-demo --repo "$SETUP_REPO"` → `export T10=<task-id>` → `pulsen tick` を2〜3秒間隔で3回 → `cat "$SETUP_HOME/state/tasks/$T10.json"`
  8. `pulsen tick`（再起動）→ `ls "$SETUP_HOME/state/runs/$T10/"`
  9. `echo 0 > "$SETUP_HOME/judge-exit"` → 数秒待って `pulsen tick` を2回（spawn確認 → 判定）→ `cat "$SETUP_HOME/state/tasks/$T10.json"`
  10. チェックの一過性失敗（TC-19。`export PULSEN_HOME=/tmp/pulsen-test/home` に戻す）: `touch /tmp/pulsen-test/check-broken` → `pulsen add --workflow pr-review-watch --repo /tmp/pulsen-test/repo` → `export T19=<task-id>` → `pulsen tick` を2〜3秒間隔で3回（起動 → spawn確認 → 判定）→ `cat "$PULSEN_HOME/state/tasks/$T19.json"`
  11. `rm /tmp/pulsen-test/check-broken` → `pulsen tick` を2〜3秒間隔で3回（再起動 → spawn確認 → 判定）→ `cat "$PULSEN_HOME/state/tasks/$T19.json"`
  12. `grep "$T19" /tmp/pulsen-test/notify.log; echo $?`
- **期待結果:**
  - 手順2: `.execution.state` が `failed`、`.counters.attempt_count` が 1、`.task_status` は `work` のまま。`attempt-1/exit` の `.code` が 1。サマリーの「失敗を記録」に T5 が現れる。
  - 手順3: `.execution.state` が `launching`、`.current_attempt.number` が 2、`run_dir` が `.../attempt-2`。worktree に `done.marker` が残っている（worktree はリトライ間で引き継がれる）。
  - 手順4: マーカー検出により exit 0 → `.execution.state` が `completed` で `.counters.attempt_count` が **0 にリセット**されている。
  - 手順5: `.task_status` が `done`・`.execution.state` が `pending`（アーカイブは #6 のため起きない）。
  - 手順6: T5 の通知行は1行も無い（自動リトライで回復した失敗は通知されない）。
  - 手順7: judge の exit 10 が failed に写像され、`.execution.state` が `failed`・`.counters.attempt_count` が 1・`.task_status` は `checking` のまま。
  - 手順8: `attempt-1` と `attempt-2` の両方がある。
  - 手順9: `.execution.state` が `completed` で `.counters.attempt_count` が 0。
  - 手順10: チェックの exit 1 が判定コマンドにより 10 へ写像され、`.execution.state` が `failed`・`.counters.attempt_count` が 1・`.task_status` は `watch` のまま。
  - 手順11: 障害解消後のチェックが exit 20 で終わって skipped となり、`.execution.state` が `pending`・`.counters.attempt_count` が **0 にリセット**されている（散発失敗の蓄積で凍結しない）。
  - 手順12: T19 の通知行は無い（凍結に至っていない）。
- **確認ポイント:** 手順2〜4 を通じて `.task_status` が `work` から一切変化しないこと（失敗の履歴は実行状態と `attempt_count` にのみ表れる）。`.counters.spawn_fail_count` が終始 0 であること（`complete_run` / `skip_run` は `spawn_fail_count` を触らない）。`.last_failure` が `null` のまま動かないこと — `FailureNote` はツール操作の失敗（worktree 作成・削除、アーカイブ移動、spawn 失敗）と判定失敗だけを記録し、**エージェント実行自体の失敗は記録しない**（`spec/domains/task.md#failurenote`。`fail_run` は `last_failure` を触らない）。実行が失敗した要因は run ディレクトリの `exit` / `stderr.log` と tick の報告文から読む。

### 4. skipped によるポーリング周回（judge の exit 20）

- **対応する受け入れ基準:** AC-5
- **対応する手順書:** `task-execution.md` TC-06 全手順、TC-07 手順1〜5（手順6〜8 の `abort` / `set-status` は #5）、`setup.md` TC-11 手順1〜4（手順5 は実行しない。片付けの `set-status` が #5 で、回復（`judge-exit` を 0 に戻して completed → 次ステータス）は setup TC-09 と同じ筋なので確認項目2 で消化済み）
- **前提:** 確認項目3 の直後。フィクスチャA の `review-flag` / `check-broken` が存在しないこと。
- **目的:** 判定コマンドの exit 20 が `skip_run` になり、**タスクステータス不変**のまま pending へ戻って `skipped_back` に記録されること、`attempt_count` を消費せず通知も起きないこと、次の tick が同じタスクステータスを新しい attempt で起動することを確認する。completed による循環（`next: watch`）も併せて見る。
- **手順:**
  1. `ls /tmp/pulsen-test/review-flag /tmp/pulsen-test/check-broken 2>/dev/null; echo $?`（いずれも無いこと）
  2. `pulsen add --workflow pr-review-watch --repo /tmp/pulsen-test/repo; echo $?` → `export T6=<task-id>`
  3. `pulsen tick` を2〜3秒間隔で2回（起動 → spawn確認）→ `cat "$PULSEN_HOME/state/tasks/$T6.json"`
  4. `pulsen tick; echo $?`（判定）→ `cat "$PULSEN_HOME/state/tasks/$T6.json"`
  5. `pulsen tick; echo $?`（再起動）→ `ls "$PULSEN_HOME/state/runs/$T6/"`
  6. `cat /tmp/pulsen-test/notify.log`
  7. `touch /tmp/pulsen-test/review-flag` → `pulsen tick` を2〜3秒間隔で2回（attempt 2 の spawn確認 → 判定）→ タスクファイル確認
  8. `pulsen tick` を2〜3秒間隔で4回（attempt 3 の起動 → spawn確認 → 判定 → 遷移）→ タスクファイル確認
  9. `pulsen tick` を2〜3秒間隔で4回（`fix` の1周）→ タスクファイル確認
  10. `rm /tmp/pulsen-test/review-flag` → `pulsen tick` を3回 → タスクファイル確認
  11. `git -C /tmp/pulsen-test/repo log --oneline "pulsen/$T6"`
  12. setup 側（`export PULSEN_HOME="$SETUP_HOME"`）: `echo 20 > "$SETUP_HOME/judge-exit"` → `pulsen add --workflow judge-demo --repo "$SETUP_REPO"` → `export T11=<task-id>` → `pulsen tick` を2〜3秒間隔で3回 → `cat "$SETUP_HOME/state/tasks/$T11.json"`; `cat "$SETUP_HOME/notify.log"` → `pulsen tick` → `ls "$SETUP_HOME/state/runs/$T11/"`
- **期待結果:**
  - 手順4: tick サマリーの「実行待ちへ復帰」に T6 が現れる。`.task_status` は `watch` のまま、`.execution.state` が `pending`、`.counters.attempt_count` は **0**（消費されない）。
  - 手順5: `attempt-1` と `attempt-2` の両方が存在する。
  - 手順6・12: `notify.log` に T6 / T11 の行が1行も無い（skipped は通知されない）。
  - 手順7: フラグ作成前に起動済みの attempt 2 は exit 20 で終わっているため、再び skipped（`watch` のまま pending 復帰）。
  - 手順8: フラグを見た attempt 3 が exit 0 → completed → 次の tick で `.task_status` が `fix`・`pending`。
  - 手順9: `fix` が completed になり、`next: watch` により `.task_status` が `watch` に戻る（循環）。
  - 手順10: 再び skipped の周回に入る。`.counters.attempt_count` は一貫して 0。
  - 手順11: `fix` のコミットがブランチに積まれている。
  - 手順12: `.task_status` が `checking` のまま `pending`、`attempt_count` 0、次の tick で attempt 番号が増える。
- **確認ポイント:** 周回のたびに `attempt_count` / `judge_attempt_count` が 0 に戻り、蓄積で凍結しないこと（TC-07 確認ポイント）。skipped の tick が `.task_status` を書き換えないこと — ここが崩れるとポーリング型ワークフローが1周で出口へ抜けてしまう。フラグ作成が効くのは**フラグ作成後に起動された attempt から**であること（チェックスクリプトは起動時に一度だけ走る）。

### 5. デフォルト判定は2値 — exit 20 も exit 127 も failed

- **対応する受け入れ基準:** AC-5（デフォルト判定の2値性）、AC-3
- **対応する手順書:** `task-execution.md` TC-23 全手順、TC-17 全手順
- **前提:** 確認項目4 の直後（`export PULSEN_HOME=/tmp/pulsen-test/home` に戻す）。
- **目的:** `judge` 未定義のステータスでは `default_judgement` が 0 / 非0 の2値でしか判定せず、exit 20 が skipped ではなく failed になること、コマンド実体の起動不能（exit 127）が spawn 失敗ではなくラッパー経由の通常の failed 経路に落ちることを確認する。
- **手順:**
  1. `pulsen add --workflow exit20 --repo /tmp/pulsen-test/repo; echo $?` → `export T23=<task-id>`
  2. `pulsen tick` を2〜3秒間隔で3回（起動 → spawn確認 → 判定）
  3. `cat "$PULSEN_HOME/state/runs/$T23/attempt-1/exit"; echo; cat "$PULSEN_HOME/state/tasks/$T23.json"`
  4. `grep "$T23" /tmp/pulsen-test/notify.log`
  5. `pulsen add --workflow broken --repo /tmp/pulsen-test/repo; echo $?` → `export T17=<task-id>`
  6. `pulsen tick` を2〜3秒間隔で3回（起動 → spawn確認 → 判定）
  7. `cat "$PULSEN_HOME/state/runs/$T17/attempt-1/exit"; echo; cat "$PULSEN_HOME/state/tasks/$T17.json"`
  8. `grep "$T17" /tmp/pulsen-test/notify.log`
- **期待結果:**
  - 手順3: `exit` の `.code` が 20。pending 復帰（skipped）ではなく、`retries: 0` により `.execution` が `{"state":"stopped","reason":"retry_limit_exceeded","notified_at":"<RFC3339 UTC>"}`。`.counters.attempt_count` が 1。
  - 手順4: `stopped <T23> exit20 work` の通知行が1行ある。
  - 手順7: `exit` の `.code` が 127（コマンド不在の符号化）。`.execution.state` が `stopped` で `.execution.reason` は `retry_limit_exceeded`、`.counters.spawn_fail_count` は **0**、`.last_failure` は `null`（エージェント実行の失敗は `FailureNote` に記録されないので、`spawn_fail` としても残らない）。
  - 手順8: 通知行が1行ある。
- **確認ポイント:** exit 20 の扱いが判定コマンドの有無で変わること（判定コマンドがあれば skipped、なければ failed。ADR-008）。TC-17 と TC-16（spawn 失敗）の経路の違いが `spawn_fail_count` と attempt の採番有無で識別できること — 起動不能でも attempt は採番され run ディレクトリが作られる。

### 6. シグナル死の符号化が EXIT_CODE として判定コマンドへ渡る

- **対応する受け入れ基準:** AC-3（非0 exit の failed 分類）、AC-5（判定プロトコルへの受け渡し）
- **対応する手順書:** `setup.md` TC-47 手順1〜2（手順3 の `abort` 片付けは #5 — 代わりにタスクを放置する）
- **前提:** フィクスチャB（`export PULSEN_HOME="$SETUP_HOME"`）。
- **目的:** エージェントがシグナルで死んだ場合でもラッパーが非0の数値（POSIX では 128+シグナル番号）に符号化して `exit` を書き、その値が `judge_env` の `EXIT_CODE` として判定コマンドに渡ることを確認する。
- **手順:**
  1. `echo 10 > "$SETUP_HOME/judge-exit"`
  2. `$SETUP_WORK/sigkill.yaml` を作成する:

      ```sh
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
      ```

  3. `pulsen add --workflow "$SETUP_WORK/sigkill.yaml" --repo "$SETUP_REPO"; echo $?` → `export T47=<task-id>`
  4. `pulsen tick` を2〜3秒間隔で3回（起動 → spawn確認 → 判定）
  5. `cat "$SETUP_HOME/state/runs/$T47/attempt-1/exit"; echo; tail -n 3 "$SETUP_HOME/judge.log"; cat "$SETUP_HOME/state/tasks/$T47.json"`
- **期待結果:** `exit` の `.code` が 137 相当の非0値（128+9）で、`judge.log` の当該行が `exit=137` になっている。判定コマンドの exit 10 により `.execution.state` が `failed` でリトライ待ち（`.counters.attempt_count` が 1）。
- **確認ポイント:** `EXIT_CODE` が10進文字列としてそのまま渡ること（符号ビットや `-9` のような表現になっていないこと）。この確認以降 T47 は failed → 再起動を繰り返すため、後続項目のサマリーに現れ続けることを織り込む（`abort` が無いので止められない。#5）。

### 7. リトライ上限の等号と超過 — 凍結と at-least-once 通知

- **対応する受け入れ基準:** AC-4
- **対応する手順書:** `task-execution.md` TC-13 全手順（手順4 の `show` は直読）、TC-22 全手順、`intervention.md` TC-01 手順1〜3・5・7・8（手順3 の `ls` と手順4・6 の `ls --state` / `show` は直読で代替）
- **前提:** フィクスチャA（`export PULSEN_HOME=/tmp/pulsen-test/home`）とフィクスチャC。
- **目的:** 連続失敗が**上限の等号では凍結せず**、超過で `Stopped { reason: retry_limit_exceeded }` を保存すること、その `save` 直後の**同一 tick 内**で notify_cmd が `TASK_ID` / `WORKFLOW` / `TASK_STATUS` を伴って起動され、`Exited(0)` のときだけ `mark_notified` → `save` が走ること、通知済みの stopped が再通知されないことを確認する。
- **手順:**
  1. `pulsen add --workflow fail --repo /tmp/pulsen-test/repo; echo $?` → `export T13=<task-id>`
  2. `pulsen tick` を2〜3秒間隔で3回（起動 → spawn確認 → 判定）→ `cat "$PULSEN_HOME/state/tasks/$T13.json"`; `cat "$PULSEN_HOME/state/runs/$T13/attempt-1/exit"; echo`
  3. `grep "$T13" /tmp/pulsen-test/notify.log; echo $?`（この時点では無いこと）
  4. `pulsen tick` を2〜3秒間隔で3回（再起動 → spawn確認 → 判定）→ `cat "$PULSEN_HOME/state/tasks/$T13.json"`
  5. `grep "$T13" /tmp/pulsen-test/notify.log`
  6. `ls -a "$PULSEN_HOME/worktrees/$T13"; git -C /tmp/pulsen-test/repo branch --list "pulsen/$T13"`
  7. `md5 -q "$PULSEN_HOME/state/tasks/$T13.json" 2>/dev/null || md5sum "$PULSEN_HOME/state/tasks/$T13.json"` → `pulsen tick; echo $?` → 同じチェックサムを再取得 → `grep -c "$T13" /tmp/pulsen-test/notify.log`
  8. `retries: 0` の境界: `pulsen add --workflow fail0 --repo /tmp/pulsen-test/repo` → `export T22=<task-id>` → `pulsen tick` を3回 → `cat "$PULSEN_HOME/state/tasks/$T22.json"`; `ls "$PULSEN_HOME/state/runs/$T22/"`; `grep "$T22" /tmp/pulsen-test/notify.log`
  9. 通知内容の証跡（フィクスチャC）: `pulsen --home "$PMT/home" add --workflow wf-fail --repo "$PMT/repo"` → `export TI1=<task-id>` → `: > "$PMT/notify.log"` → `pulsen --home "$PMT/home" tick` を2〜3秒間隔で繰り返す（9回目の tick で凍結する。途中で `cat "$PMT/notify.log"` と `cat "$PMT/home/state/tasks/$TI1.json"` を確認する）
  10. stopped 到達後: `cat "$PMT/notify.log"`; `cat "$PMT/home/state/tasks/$TI1.json"`; `cat "$PMT/home/state/runs/$TI1/attempt-<最新>/stderr.log"`; `ls -a "$PMT/home/worktrees/$TI1"`
  11. `pulsen --home "$PMT/home" tick; echo $?; wc -l < "$PMT/notify.log"`
- **期待結果:**
  - 手順2: `.execution.state` が `failed`、`.counters.attempt_count` が **1（= 上限 1。等号では凍結しない）**、`.task_status` は `work` のまま。`exit` の `.code` が 1。
  - 手順3: 通知は無い（failed の間は通知されない）。
  - 手順4: `attempt_count` 2 > 1 により `.execution` が `{"state":"stopped","reason":"retry_limit_exceeded","notified_at":"<RFC3339 UTC>"}`。tick サマリーの「凍結」と「通知」の両方に T13 が現れる。`.task_status` は `work` のまま。
  - 手順5: `stopped <T13> fail work` の行が**ちょうど1行**。
  - 手順6: worktree もブランチも残っている（凍結タスクの調査材料は自動削除されない）。`fail` は成果物を書かないので worktree の中身は `.git` だけで、`ls -a` でないと存在が見えない。
  - 手順7: exit code 0。タスクファイルのチェックサムが変わらず、通知行数も増えない（通知済みの stopped は再通知されない・書き込みも起きない）。
  - 手順8: `.execution.state` が `stopped` / `retry_limit_exceeded`、run ディレクトリは `attempt-1` のみ（failed を経た再実行なしの即時凍結）、通知行が1行。
  - 手順9〜10: failed でリトライしている間は `notify.log` に行が増えず、stopped 到達の tick でちょうど1行 `TASK_ID=<TI1> WORKFLOW=wf-fail TASK_STATUS=work` が追記される。`stderr.log` に `boom` があり、凍結原因をログから特定できる。worktree は残っている（`wf-fail` も成果物を書かないので中身は `.git` だけ）。
  - 手順11: 行数が増えない。
- **確認ポイント:** 通知が**凍結を保存した同じ tick 内**で起きること（次の tick に持ち越されない）。`notified_at` が `null` のまま残る tick が1つも無いこと（成功する notify_cmd の場合）。通知の環境変数3つがすべて非空で、`TASK_STATUS` が凍結時点のタスクステータス（`work`）であること。凍結後の tick が `.updated_at` を更新しないこと — 更新されるなら「凍結の判定を保存後の状態から導出している」疑い（ADR-097）。

### 8. 判定失敗の上限超過 — エージェントを再実行せずに凍結

- **対応する受け入れ基準:** AC-4（判定上限の経路）
- **対応する手順書:** `task-execution.md` TC-15 全手順、`setup.md` TC-37 手順1〜3（手順4 の `retry` は #5）
- **前提:** 確認項目7 の直後。setup 側はフィクスチャB。
- **目的:** 判定コマンドが 0 / 10 / 20 以外で終了すると `record_judge_failure` により `judge_attempt_count` だけが増え、実行状態は `running` のまま保たれること、上限（既定3）の**等号では凍結せず超過で** `Stopped { reason: judge_limit_exceeded }` になること、その間エージェントの再実行が一度も起きないことを確認する。
- **手順:**
  1. `pulsen add --workflow judgefail --repo /tmp/pulsen-test/repo; echo $?` → `export T15=<task-id>`
  2. `pulsen tick` を2〜3秒間隔で2回（起動 → spawn確認）→ `cat "$PULSEN_HOME/state/runs/$T15/attempt-1/exit"; echo`
  3. `pulsen tick; echo $?`（判定1回目）→ `cat "$PULSEN_HOME/state/tasks/$T15.json"`
  4. `pulsen tick` を2回（判定2〜3回目）→ タスクファイル確認
  5. `pulsen tick; echo $?`（判定4回目）→ タスクファイル確認
  6. `ls "$PULSEN_HOME/state/runs/$T15/"`
  7. `grep "$T15" /tmp/pulsen-test/notify.log`
  8. setup 側（`export PULSEN_HOME="$SETUP_HOME"`）: `echo 1 > "$SETUP_HOME/judge-exit"` → `pulsen add --workflow judge-demo --repo "$SETUP_REPO"` → `export T37=<task-id>` → `pulsen tick` を2〜3秒間隔で2回（起動 → spawn確認）→ タスクファイル確認 → 上限を超えるまで `pulsen tick` を繰り返す（計4回程度）→ `cat "$SETUP_HOME/state/tasks/$T37.json"`; `cat "$SETUP_HOME/notify.log"`; `ls "$SETUP_HOME/state/runs/$T37/"`
- **期待結果:**
  - 手順3: `.counters.judge_attempt_count` が 1、`.execution.state` は **`running` のまま**、`.counters.attempt_count` は 0、`.last_failure.kind` が `judge_fail`。
  - 手順4: `judge_attempt_count` が 2 → 3（3 = 上限。等号では凍結しない）。実行状態は `running` のまま。
  - 手順5: `judge_attempt_count` 4 > 3 で `.execution` が `{"state":"stopped","reason":"judge_limit_exceeded","notified_at":"<RFC3339 UTC>"}`。
  - 手順6: run ディレクトリは `attempt-1` のみ、`.counters.attempt_count` は 0 のまま（エージェントの再実行が一度も行われていない）。
  - 手順7・8: 通知行が1行追記されている。setup 側も同じ経路（judge スクリプトの exit 1）で `judge_limit_exceeded` に至る。
- **確認ポイント:** 判定失敗のたびに判定コマンドが**再実行**されること（`judge.log` の行が tick ごとに増える）— エージェントは再起動しないが判定はやり直す、という非対称がこの経路の要。凍結の `reason` が `retry_limit_exceeded` ではなく `judge_limit_exceeded` であること。判定の冪等性（同じ exit・同じ定義に対して毎回同じ結論）が tick を跨いで保たれること。

### 9. 通知の失敗と再通知 / notify_cmd 未定義と catch-up

- **対応する受け入れ基準:** AC-4（at-least-once）
- **対応する手順書:** `intervention.md` TC-24（**手順2 の `abort` を上限超過での凍結に読み替える**）、`intervention.md` TC-15（同じ読み替え。手順3 の `show` は直読。手順7〜10 がこの TC の筋そのものを消化する）、`setup.md` TC-35（同じ読み替え。手順4 の `notify_cmd` 復元は必ず行う）
- **前提:** フィクスチャC。フィクスチャA / B とは独立に実行できる。
- **目的:** notify_cmd が非0で終わったときに `notified_at` が残らず、次の tick が同じ判断を再導出して再通知すること、通知済みになった後は再通知されないこと、notify_cmd 未定義なら通知も `notified_at` の記録も行わず、後から定義すると次の tick が catch-up することを確認する。
- **手順:**
  1. 必ず失敗する通知に差し替える: `sed -i.bak 's|^notify_cmd:.*|notify_cmd: ["sh", "-c", "exit 1"]|' "$PMT/home/config.yaml"; grep -n notify_cmd "$PMT/home/config.yaml"; : > "$PMT/notify.log"`
  2. `pulsen --home "$PMT/home" add --workflow wf-fail --repo "$PMT/repo"` → `export TI24=<task-id>`
  3. `pulsen --home "$PMT/home" tick; echo $?` を2〜3秒間隔で繰り返し（9回目の tick で凍結する）、`.execution.state` が `stopped` になるまで進める → `cat "$PMT/home/state/tasks/$TI24.json"`
  4. `cat "$PMT/notify.log"`
  5. 通知を成功する形に戻す: `cp "$PMT/config.bak" "$PMT/home/config.yaml"; rm -f "$PMT/home/config.yaml.bak"`（手順1 の `sed -i.bak` が残す控え） → `pulsen --home "$PMT/home" tick; echo $?` → `cat "$PMT/notify.log"`; `cat "$PMT/home/state/tasks/$TI24.json"`
  6. `pulsen --home "$PMT/home" tick; echo $?; wc -l < "$PMT/notify.log"`
  7. notify_cmd 未定義（setup TC-35 の読み替え）: `grep -v '^notify_cmd:' "$PMT/config.bak" > "$PMT/home/config.yaml"; grep -c notify_cmd "$PMT/home/config.yaml"` → `pulsen --home "$PMT/home" add --workflow wf-fail --repo "$PMT/repo"` → `export TI35=<task-id>` → `pulsen --home "$PMT/home" tick` を繰り返して stopped まで進める（同じく9回）
  8. `cat "$PMT/home/state/tasks/$TI35.json"`; `wc -l < "$PMT/notify.log"`
  9. catch-up: `cp "$PMT/config.bak" "$PMT/home/config.yaml"` → `pulsen --home "$PMT/home" tick; echo $?` → `cat "$PMT/notify.log"`; `cat "$PMT/home/state/tasks/$TI35.json"`
  10. `pulsen --home "$PMT/home" tick; echo $?; wc -l < "$PMT/notify.log"`
- **期待結果:**
  - 手順3: 凍結の tick は exit code **0** で完了する（通知の失敗は tick 全体を落とさない）。サマリーの「凍結」に TI24 が現れ、「通知」には現れない。通知の失敗は報告4見出しのうち**「スキップ」**に `凍結を通知できません(…)。次の tick が再通知します` として出る（`TickIssue::NotifyFailed` はタスクファイルに何も残さない結末なので「失敗を記録」ではない。`crates/pulsen/src/cli/render.rs`）。
  - 手順3・4: `.execution.notified_at` が `null` のまま。`notify.log` は空。
  - 手順5: 次の tick で `notify.log` に `TASK_ID=<TI24>` の行が1行追加され、`.execution.notified_at` に時刻が入る。サマリーの「通知」に TI24 が現れる（「凍結」には現れない — 通知アームの保存は `Freeze::NotFrozen`。ADR-097）。
  - 手順6: 行が増えない。
  - 手順8: TI35 は `stopped` になるが `.execution.notified_at` は `null` のまま、`notify.log` の行数は変わらない（未定義なら `notified_at` を書かない）。
  - 手順9: catch-up で `TASK_ID=<TI35>` の行が1行追加され、`notified_at` が記録される。
  - 手順10: 行が増えない。
- **確認ポイント:** 「stopped を書く → notify_cmd → 成功時のみ `mark_notified`」の順序が破れていないこと — 順序が逆なら手順3 の時点で `notified_at` が入ってしまい、手順5 の再通知が永久に起きない（欠落）。二重通知は許容だが**欠落は許容しない**（requirements §8）。手順9 の catch-up が「凍結」として再計上されないこと（過去の凍結が毎 tick 再計上されるなら ADR-097 の破れ）。フィクスチャA / B の config を触っていないこと（本項目は `$PMT/home` に閉じる）。

### 10. timeout 超過での kill と failed、実行中の連続 tick の冪等性

- **対応する受け入れ基準:** AC-6
- **対応する手順書:** `task-execution.md` TC-14 全手順
- **前提:** フィクスチャA（`export PULSEN_HOME=/tmp/pulsen-test/home`）。
- **目的:** exit が無く生存（`starttime.ident` の照合一致）で `starttime.wall` からの経過が timeout（10s）を超えたら `kill` してから `fail_run` すること、未超過の連続 tick では書き込みが一切起きないこと、上限超過で `stopped` になることを確認する。
- **手順:**
  1. `pulsen add --workflow sleeper --repo /tmp/pulsen-test/repo; echo $?` → `export T14=<task-id>`
  2. `pulsen tick` を2〜3秒間隔で2回（起動 → spawn確認）→ `cat "$PULSEN_HOME/state/tasks/$T14.json"`（`running` と PID を控える: `export P14=<pid>`）
  3. 冪等性: `md5 -q "$PULSEN_HOME/state/tasks/$T14.json" 2>/dev/null || md5sum "$PULSEN_HOME/state/tasks/$T14.json"` → 間を置かず `pulsen tick; echo $?` を2回 → 同じチェックサムを再取得して比較
  4. 起動から10秒以上待って `pulsen tick; echo $?` → `cat "$PULSEN_HOME/state/tasks/$T14.json"`
  5. `ps -p "$P14"; echo $?; pgrep -f 'sleep 120'; echo $?`
  6. `ls "$PULSEN_HOME/state/runs/$T14/attempt-1/"`
  7. `pulsen tick` を2回（再起動 → spawn確認）→ 10秒以上待って `pulsen tick` → `cat "$PULSEN_HOME/state/tasks/$T14.json"`
  8. `grep "$T14" /tmp/pulsen-test/notify.log; ls -a "$PULSEN_HOME/worktrees/$T14"`
- **期待結果:**
  - 手順3: 2回とも exit code 0 で、チェックサムが変わらない（未超過の `KeepRunning` は書き込みを1回も起こさない）。サマリーに T14 は現れない。
  - 手順4: `.execution.state` が `failed`、`.counters.attempt_count` が 1（= 上限のため凍結しない）、`.task_status` は `work` のまま。
  - 手順5: 対象の `sleep 120` が残っていない（`ps` が非0 / `pgrep` が該当なし）— プロセスグループ相当の実行単位が終了している。
  - 手順6: `exit` ファイルは存在しない（kill されたのでラッパーは書けない）。`pid` / `starttime` は残っている。
  - 手順7: attempt 2 も timeout kill され、`attempt_count` 2 > 1 で `.execution.state` が `stopped` / `retry_limit_exceeded` / `notified_at` あり。
  - 手順8: 通知行が1行。worktree は保持されている（`sleeper` は成果物を書かないので中身は `.git` だけ）。
- **確認ポイント:** kill が**タスクファイルに記録された `kill_ident` を使って**行われ、無関係なプロセスを巻き込んでいないこと（手順5 の前に `pgrep -f sleep` で他の sleep が無いことを確認しておくと差が読める。ADR-002 / ADR-015）。`kill_ident` はそのまま渡されるのではなく境界で `terminate::UnitTarget` に parse して組み直され、成否は終了コマンドの終了ステータスではなく実行単位の消滅の観測（`TERMINATION_GRACE` 2秒 / `TERMINATION_POLL` 50ms）で決まり、猶予のうちに消えなければ強い終了へ昇格する — ADR-015 が ADR-002 のこの3点を置き換えている。昇格の段は `terminate::ESCALATES` が宣言し、本確認を行う POSIX は真（`-TERM` → `-KILL` の2段。待ちの上限は 2秒 × 2 = 4秒）、昇格を持たないプラットフォームは偽で2段目を起動しない（待ちの上限は 2秒）。経過の起点が `starttime.wall` であること — tick 実行時刻や `recorded_at` を起点にすると手順3 の連続 tick で早期 kill が起きる。手順4 の tick が「kill → fail_run」の順で1ステップだけ進むこと。

### 11. exit 記録なしのプロセス死亡の検出と自動リトライ

- **対応する受け入れ基準:** AC-6
- **対応する手順書:** `task-execution.md` TC-21 手順1〜5（手順1 に確認ポイント用の2件目を足す。手順2 の PID はタスクファイル直読で得る。手順6 の `abort` 片付けは #5）
- **前提:** 確認項目10 の直後。
- **目的:** exit ファイルが無いままプロセスが死亡した実行（外部からの `kill -9` で OOM・マシン再起動を代替再現）を、`starttime_of` が `Ok(None)` または `ident` 不一致を返す経路で `DiedWithoutExit` と分類し、`try_kill_remnants` をベストエフォートで試みてから `fail_run` すること、次の tick が新しい attempt で自動再起動することを確認する。
- **手順:**
  1. `longrun` を**2件**登録する: `pulsen add --workflow longrun --repo /tmp/pulsen-test/repo; echo $?` → `export T21=<task-id>`、`pulsen add --workflow longrun --repo /tmp/pulsen-test/repo; echo $?` → `export T21B=<task-id>`（2件目は確認ポイントが要求する対照 — `try_kill_remnants` が巻き込んでよいのは T21 の実行単位だけであることを、無傷で残る他タスクの `sleep 600` で見る）
  2. `pulsen tick` を2〜3秒間隔で2回（起動 → spawn確認）→ `cat "$PULSEN_HOME/state/tasks/$T21.json"` から `export P21=<.current_attempt.process.pid>`、`kill_ident` の値も控える。T21B も同じ2 tick で `running` になる
  3. `pgrep -fl 'sleep 600'`（この時点の一覧を控える。T21 と T21B の2件）→ `kill -9 -- "-$P21"; echo $?`（T21 のプロセスグループだけを強制終了。`kill_ident` が `-<pgid>` 形式であることをタスクファイルで確認してから実行する）
  4. `ls "$PULSEN_HOME/state/runs/$T21/attempt-1/"`（`exit` が無いこと）
  5. `pulsen tick; echo $?` → `cat "$PULSEN_HOME/state/tasks/$T21.json"`
  6. `pulsen tick; echo $?` → `cat "$PULSEN_HOME/state/tasks/$T21.json"`; `ls "$PULSEN_HOME/state/runs/$T21/"`
- **期待結果:**
  - 手順4: `pid` / `starttime` はあるが `exit` は無い。
  - 手順5: exit code 0。`.execution.state` が `failed`、`.counters.attempt_count` が 1。`attempt-1/exit` は依然として存在しない（tick は run ディレクトリに書かない）。残存終了の結果は報告として現れうるが、状態の分類には影響しない。
  - 手順6: 新しい attempt 番号 2 で自動的に再起動される（`.execution.state` が `launching`、`run_dir` が `.../attempt-2`）。利用者の操作は不要。T21B は `running` のまま無傷で、その `sleep 600` も残っている。
- **確認ポイント:** exit が無いときにだけ生存観測が行われること（確認項目1・3 のように exit が Some のケースでは `starttime_of` を呼ばずに判定へ進む。2段規則）。`try_kill_remnants` が誤って無関係なプロセスを終了させていないこと（手順3 で控えた一覧と比べ、tick 後に**T21B 由来の `sleep 600` が残っている**ことを確認する）。この確認以降 T21 / T21B は `timeout: none` のまま `sleep 600` を再実行し続けるため、後続項目のサマリーに現れ続けることを織り込む。

## エッジケース・異常系

すべてのケースで、**tick 自体の exit code が 0 であること**（tick は1タスクの失敗で全体を落とさない。ロック機構の異常と走査の Io だけが非0）と、**該当タスク以外の状態が変わっていないこと**を共通の確認手段とする。

タスク単位の報告は、結末別に「失敗を記録(<n>件):」「起動の結果が未確定(<n>件):」「スキップ(<n>件):」「後始末が残っている(<n>件):」の4見出しで表示される（`crates/pulsen/src/cli/render.rs`）。見出しは**報告が何を残したか（運用者が次に取る行動）**で分かれる（ADR-017）— 最後の1つは残存プロセスの終了を確認できなかった報告（`RemnantsUnhandled`）だけが並ぶ場所で、タスクファイルには何も書かれておらず、OS 側に残った実行単位を終了させるのは人間になる。

### 1. スナップショットのみ破損した未通知 stopped への再通知

- **対応する受け入れ基準:** AC-4（`SnapshotUnreadable` 経路の at-least-once。PAGE-tick-007）
- **対応する手順書:** なし（`spec/manual-tests/` に対応する手順が無く、PAGE-tick-007 から起こした確認。前提のフィクスチャを担当する側が実行する）
- **前提:** フィクスチャC。本スライスには `abort` が無いため、未通知 stopped のタスクを**タスクファイルの直編集**で作る。
- **目的:** タスク属性は読めるがスナップショットが読めない縮退状態でも、`Stopped { notified_at: None }` に対しては再通知**だけ**は行われること（`save_degraded` で `notified_at` が残り、スナップショットは元の内容のまま温存されること）、それ以外の縮退タスクは従来どおり報告のみでスキップされることを確認する。
- **手順:**
  1. `pulsen --home "$PMT/home" add --workflow wf-fail --repo "$PMT/repo"` → `export TD=<task-id>`
  2. `cp "$PMT/home/state/tasks/$TD.json" "$PMT/td.bak"`
  3. 未通知 stopped かつスナップショット破損の状態を作る:

      ```sh
      jq '.execution = {"state":"stopped","reason":"retry_limit_exceeded","notified_at":null}
          | .snapshot.statuses = "broken"' "$PMT/td.bak" > "$PMT/td.new" \
        && mv "$PMT/td.new" "$PMT/home/state/tasks/$TD.json"
      cat "$PMT/home/state/tasks/$TD.json"
      : > "$PMT/notify.log"
      ```

  4. `pulsen --home "$PMT/home" tick; echo $?`
  5. `cat "$PMT/notify.log"`; `cat "$PMT/home/state/tasks/$TD.json"`
  6. `pulsen --home "$PMT/home" tick; echo $?; wc -l < "$PMT/notify.log"`
  7. 対照（stopped 以外の縮退。TC-exec-tick-020）。同じ TD を `pending` に戻して打ち直す — タスクIDはファイル名と内容の両方に現れるので、`td.bak` をファイル名だけ変えて置くと同一 task_id のタスクファイルが2つできてしまう:

      ```sh
      jq '.execution = {"state":"pending"}' "$PMT/home/state/tasks/$TD.json" > "$PMT/td.pending" \
        && mv "$PMT/td.pending" "$PMT/home/state/tasks/$TD.json"
      grep -c '"broken"' "$PMT/home/state/tasks/$TD.json"
      md5 -q "$PMT/home/state/tasks/$TD.json" 2>/dev/null || md5sum "$PMT/home/state/tasks/$TD.json"
      pulsen --home "$PMT/home" tick; echo $?
      md5 -q "$PMT/home/state/tasks/$TD.json" 2>/dev/null || md5sum "$PMT/home/state/tasks/$TD.json"
      wc -l < "$PMT/notify.log"
      ```

- **期待結果:** 手順4 は exit code 0 で、そのサマリーの「通知」に TD が現れ、**同時に「スキップ」にも TD が「埋め込まれたワークフロー定義を読めません」として現れる**（報告は通知に置き換わらない。ADR-012）。手順5 で `notify.log` に `TASK_ID=<TD> WORKFLOW=wf-fail TASK_STATUS=work` の行が1行追加され、タスクファイルの `.execution.notified_at` に時刻が入り、**`.snapshot.statuses` は `"broken"` のまま**温存されている。手順6 で行が増えない。手順7 は exit code 0 で、TD が「スキップ」の見出しに「埋め込まれたワークフロー定義を読めません」として報告され、チェックサムも `notify.log` の行数も変わらない（pending の縮退タスクには再通知の経路が無い）。
- **期待結果の補足:** 通知に必要な3値（`TASK_ID` / `WORKFLOW` / `TASK_STATUS`）はいずれもスナップショット非依存の属性から取れる。取れずに報告へ落ちるなら、`DegradedTask` への通知経路が実装されていない。
- **確認ポイント:** 保存が `save_degraded` 経由で行われ、破損したスナップショットが正規化・上書きされていないこと（上書きされると利用者が直すべき情報が失われる）。tick が縮退タスクを `stopped` にし直さないこと。

### 2. 判定コマンドの実体が見つからない

- **対応する受け入れ基準:** AC-4（判定失敗の分類）
- **対応する手順書:** `setup.md` TC-38 手順1〜2（手順3 の `abort` 片付けは #5 — 代わりに放置する。判定コマンドの実体が `/no/such/judge.sh` で `judge-exit` を読まないため回復の手立ては無く、判定上限超過で凍結して止まる）
- **前提:** フィクスチャB（`export PULSEN_HOME="$SETUP_HOME"`）。
- **目的:** 判定コマンドが起動できない場合（`FailedToStart`）も、プロトコル外 exit と同じ判定失敗経路に落ちて `judge_attempt_count` が増えることを確認する。
- **手順:**
  1. `$SETUP_WORK/judge-missing.yaml` を作成する:

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
      ```

  2. `pulsen add --workflow "$SETUP_WORK/judge-missing.yaml" --repo "$SETUP_REPO"; echo $?` → `export T38=<task-id>`
  3. `pulsen tick` を2〜3秒間隔で3回（起動 → spawn確認 → 判定）→ `cat "$SETUP_HOME/state/tasks/$T38.json"`
- **期待結果:** 手順2 の登録は成功する（`judge` のコマンド実体は登録時に検証されない。`spec/manual-tests/setup.md` TC-38）。手順3 で `.counters.judge_attempt_count` が 1、`.execution.state` は `running` のまま、`.last_failure.kind` が `judge_fail` で、メッセージから「判定コマンドを起動できなかった」ことが読める。tick の exit code は 0。
- **確認ポイント:** 起動不能が `Failed`（エージェントの失敗）に誤分類されていないこと — 誤分類すると `attempt_count` を消費してエージェントを再実行してしまう。この TC のタスクは以降 running のまま毎 tick 再判定され、上限超過で凍結する（放置してよい）。

### 3. 判定 timeout と tick のブロック

- **対応する受け入れ基準:** AC-4（判定失敗の分類）、AC-6 に隣接する時間の扱い
- **対応する手順書:** `setup.md` TC-39 手順1〜3 と手順4 の `judge_timeout` の復元
- **前提:** フィクスチャB。**フィクスチャB の最後に実行する**（記載順ではこれがフィクスチャB の最終項目） — T39 は判定が決着しないまま `running` で残り、`judge_timeout` を 60s に戻したあとは以降のフィクスチャB の tick を毎回 60秒ブロックする。本スライスには `abort` が無く途中で止められない（#5）ので、T39 を作るのは以降 tick を打たない位置に置く。
- **目的:** 判定コマンドが応答しないとき、`CommandRunner` の timeout が効いて `TimedOut` として判定失敗に落ちること、その間 tick が排他ロックを保持したままブロックすること（ADR-018 が承知のうえで置いた設計）を確認する。
- **手順:**
  1. `sed -i.bak 's/^judge_timeout: 60s$/judge_timeout: 5s/' "$SETUP_HOME/config.yaml"; grep -n judge_timeout "$SETUP_HOME/config.yaml"`
  2. `$SETUP_WORK/judge-hang.yaml` を作成する:

      ```sh
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
      ```

  3. `pulsen add --workflow "$SETUP_WORK/judge-hang.yaml" --repo "$SETUP_REPO"; echo $?` → `export T39=<task-id>`
  4. `pulsen tick`（起動）→ 数秒待って `pulsen tick`（spawn確認）→ 2〜3秒待って `time pulsen tick; echo $?`（判定）
  5. `cat "$SETUP_HOME/state/tasks/$T39.json"`; `pgrep -f 'sleep 180'; echo $?`
  6. 復元（必ず実行する）: `cp "$SETUP_WORK/config.bak" "$SETUP_HOME/config.yaml"; rm -f "$SETUP_HOME/config.yaml.bak"; grep -n judge_timeout "$SETUP_HOME/config.yaml"`
- **期待結果:** 手順4 の3回目の tick が **5秒強ブロック**してから exit code 0 で返る。手順5 で `.counters.judge_attempt_count` が 1、`.execution.state` は `running` のまま。`pgrep` が該当なし（非0）で、`tick` が返った時点で判定コマンドのプロセスが残っていない（timeout 超過時に**起動した直接の子**を終了させる契約。TC-port-command-runner-012）。
- **期待結果の補足:** 判定コマンドを `sh -c` で包まないのは、契約が保証するのが直接の子までで、その子が起こした孫の残存は許容されているため（ADR-001）。`sh` を挟むと、`-c` の単一コマンドを exec に畳まない実体で孫が残り、契約どおりの実装が不合格に見える。`180` は他のフィクスチャが使う `sleep 120` / `sleep 600` と重ならない値として選んでいる（`pgrep` が他タスク由来のプロセスを拾わない）。
- **確認ポイント:** ブロック時間が `judge_timeout`（5s）に対して過大でないこと — ポーリング間隔の粒度ぶんの誤差は許容するが、桁が違えば `try_wait` のポーリング実装（ADR-001）が疑わしい。判定コマンドの残存が無いこと。復元を忘れると後続の判定がすべて5秒で切られる。

### 4. 手動修復で不変条件が破れたタスクの報告とスキップ

- **対応する受け入れ基準:** AC-6 に隣接する手続きD 冒頭の検査（TC-exec-tick-022）
- **対応する手順書:** なし（`spec/manual-tests/` に対応する手順が無く、TC-exec-tick-022 から起こした確認。前提のフィクスチャを担当する側が実行する）
- **前提:** フィクスチャA。`running` なのに `current_attempt` / `current_attempt.process` が無い状態を直編集で作る。
- **目的:** 不変条件2（`current_attempt` が Some）と不変条件3（`current_attempt.process` が Some）の破れを、報告してスキップすること・タスクファイルに書き込まないこと・パニックしないことを確認する。
- **手順:**
  1. `pulsen add --workflow pipeline --repo /tmp/pulsen-test/repo` → `export TX=<task-id>`
  2. `pulsen tick` を2〜3秒間隔で2回（`running` にする）→ `cp "$PULSEN_HOME/state/tasks/$TX.json" /tmp/pulsen-test/tx.bak`
  3. 不変条件2 の破れ:

      ```sh
      jq '.execution = {"state":"running"} | .current_attempt = null' /tmp/pulsen-test/tx.bak \
        > /tmp/pulsen-test/tx.new && mv /tmp/pulsen-test/tx.new "$PULSEN_HOME/state/tasks/$TX.json"
      md5 -q "$PULSEN_HOME/state/tasks/$TX.json" 2>/dev/null || md5sum "$PULSEN_HOME/state/tasks/$TX.json"
      pulsen tick; echo $?
      md5 -q "$PULSEN_HOME/state/tasks/$TX.json" 2>/dev/null || md5sum "$PULSEN_HOME/state/tasks/$TX.json"
      ```

  4. 不変条件3 の破れ:

      ```sh
      jq '.execution = {"state":"running"} | .current_attempt.process = null' /tmp/pulsen-test/tx.bak \
        > /tmp/pulsen-test/tx.new && mv /tmp/pulsen-test/tx.new "$PULSEN_HOME/state/tasks/$TX.json"
      pulsen tick; echo $?
      cat "$PULSEN_HOME/state/tasks/$TX.json"
      ```

  5. 復元: `cp /tmp/pulsen-test/tx.bak "$PULSEN_HOME/state/tasks/$TX.json"`
- **期待結果:** 手順3・4 とも exit code 0 で、「スキップ」の見出しに TX のタスクIDと原因（現在 attempt が無い / 同定情報が無い）が読める形で報告される。タスクファイルのチェックサム・内容は変わらない。他タスクの処理は続行される。
- **確認ポイント:** 2つの破れが**区別できる文言**で報告されること（`TickIssue` の新分類。adr.md ADR-004）。破れたタスクが `stopped` にされないこと — 手で直せる状態をツールが凍結させない。

### 5. run ファイル(exit)の破損での滞留

- **対応する受け入れ基準:** AC-2 / AC-6 の前段（`read_exit` の `RunFileError`。TC-exec-tick-104）
- **対応する手順書:** なし（`spec/manual-tests/` に対応する手順が無く、TC-exec-tick-104 から起こした確認。前提のフィクスチャを担当する側が実行する）
- **前提:** フィクスチャA。エージェント完了後の `exit` を壊して tick を打つ。
- **目的:** `RunStore::read_exit` が `Corrupt` を返す状況で、tick が報告してスキップし、**書き込みを1回も起こさない**ことを確認する（判定も遷移も行われない）。
- **手順:**
  1. `pulsen add --workflow pipeline --repo /tmp/pulsen-test/repo` → `export TR=<task-id>`
  2. `pulsen tick` を2〜3秒間隔で2回 → `until [ -f "$PULSEN_HOME/state/runs/$TR/attempt-1/exit" ]; do sleep 1; done`
  3. `cp "$PULSEN_HOME/state/runs/$TR/attempt-1/exit" /tmp/pulsen-test/tr-exit.bak; echo 'broken' > "$PULSEN_HOME/state/runs/$TR/attempt-1/exit"`
  4. `md5 -q "$PULSEN_HOME/state/tasks/$TR.json" 2>/dev/null || md5sum "$PULSEN_HOME/state/tasks/$TR.json"` → `pulsen tick; echo $?` → 同じチェックサムを再取得
  5. `cat "$PULSEN_HOME/state/tasks/$TR.json"`
  6. 復元: `cp /tmp/pulsen-test/tr-exit.bak "$PULSEN_HOME/state/runs/$TR/attempt-1/exit"` → `pulsen tick; echo $?` → タスクファイル確認
- **期待結果:** 手順4 は exit code 0 で、報告に TR と「runディレクトリのファイルを読めません」旨が出る。タスクファイルのチェックサムが変わらず、`.execution.state` は `running` のまま。手順6 で復元後の tick が通常どおり判定して `completed` になる（同じ事実から同じ判断が再導出される）。
- **確認ポイント:** 破損した `exit` を tick が書き換えたり削除したりしないこと（読めないリソースには書き込まない。PAGE-common-006）。`Corrupt` を「exit なし」と取り違えて生存観測へ進まないこと — 進むと死亡判定 → failed → 再起動で同一 worktree の並走に至る。

### 6. 1タスクの失敗が他タスクを止めない・冪等な連続 tick

- **対応する受け入れ基準:** AC-2 / AC-4 / AC-6 に共通する走査レベルの性質（TC-exec-tick-023 / 024）
- **対応する手順書:** `task-execution.md` TC-20 手順1〜4・6（手順1 の2件目は `draft.yaml` の代わりに `pipeline` で登録する。手順3 の期待のうちアーカイブは #6 のため確認しない。手順5 の `ls` と手順7 の `set-status` は #4 / #5）
- **前提:** フィクスチャA（ここまでの項目で running / failed / stopped / pending が混在した状態）。
- **目的:** 読めないタスクファイル・縮退タスクが混ざっても他タスクの判定・遷移・通知が同一 tick で進むこと、状態が変わらないタスク群に対する連続 tick が書き込みを1回も起こさないことを確認する。
- **手順:**
  1. `pulsen add --workflow pipeline --repo /tmp/pulsen-test/repo` → `export T20=<task-id>`; `pulsen add --workflow pipeline --repo /tmp/pulsen-test/repo` → `export T20P=<task-id>`
  2. `cp "$PULSEN_HOME/state/tasks/$T20.json" /tmp/pulsen-test/t20.bak && echo broken > "$PULSEN_HOME/state/tasks/$T20.json"`
  3. `pulsen tick; echo $?`
  4. `cat "$PULSEN_HOME/state/tasks/$T20.json"`; `cat "$PULSEN_HOME/state/tasks/$T20P.json"`
  5. `grep -c "$T20" /tmp/pulsen-test/notify.log`
  6. 冪等性: 全タスクファイルのチェックサムを取る → `pulsen tick` を2回 → 再取得して比較

      ```sh
      md5 -q "$PULSEN_HOME/state/tasks/"*.json > /tmp/pulsen-test/before.md5 2>/dev/null \
        || md5sum "$PULSEN_HOME/state/tasks/"*.json > /tmp/pulsen-test/before.md5
      pulsen tick; echo $?; sleep 3; pulsen tick; echo $?
      md5 -q "$PULSEN_HOME/state/tasks/"*.json > /tmp/pulsen-test/after.md5 2>/dev/null \
        || md5sum "$PULSEN_HOME/state/tasks/"*.json > /tmp/pulsen-test/after.md5
      diff /tmp/pulsen-test/before.md5 /tmp/pulsen-test/after.md5; echo $?
      ```

  7. 復元: `cp /tmp/pulsen-test/t20.bak "$PULSEN_HOME/state/tasks/$T20.json"`
- **期待結果:** 手順3 は exit code **0**。「スキップ」に T20 のファイルパスが報告され、T20P は同一 tick で通常どおり起動される。手順4 で破損ファイルの内容は `broken` のまま。手順5 で T20 に関する通知は 0 件（破損は stopped 化されない）。手順6 は、進行中のタスク（running のまま滞留・stopped・`done` 滞留）しか残っていない状態で打てば差分なしになる — 判定待ちのタスクが混じっていると差分が出るので、その場合は差分の出たタスクが「その tick で1ステップ進んだ」ことを確認して冪等性の対象から外す。
- **確認ポイント:** 走査が1件の失敗で打ち切られないこと。`stopped` かつ通知済みのタスクに対して毎 tick 何も起きないこと（通知が増えない・`updated_at` が動かない）。`done`（クリーンアップ）到達済みのタスクが報告にも現れないこと（#6 が引き取るまで無反応が正しい。ADR-101）。

## 既存機能への影響確認

- **起動・spawn確認（Issue #2 の主経路）:** `Tick` にジェネリック引数 `C: CommandRunner` が増え、`cli::wire` がランナーを構築するようになる（steps.md ステップ8）。`.thread/2/testing.md` の確認項目2（worktree・ブランチ・launching 記録・runディレクトリ）と確認項目3（spawn確認と猶予内の冪等性）をスポットチェックとして再実行し、Issue #2 時点と同じ結果になることを確認する。特に `SystemCommandRunner` の構築が外部リソースの読み取りを伴わないこと（`cli::wire::command_runner` は `SystemCommandRunner::new() -> Self` を返すだけで失敗しない）と、`add` の経路がランナーを必要としないままであること（`grep -rn 'command_runner()' crates/pulsen/src/cli/` のヒットが `cli/wire.rs` の定義1件と `cli/tick.rs` の呼び出し1件の計2件で、`cli/add.rs` に現れないこと）。

- **サマリー表示の追随:** `cli::render::tick_summary` の項目行は「起動 / 起動確認 / 判定確定 / 遷移 / 実行待ちへ復帰 / 凍結 / 通知 / 終端処理 / gcで削除 / gcで削除できず」の順に並び、そのあとに報告の4見出し（「失敗を記録」「起動の結果が未確定」「スキップ」「後始末が残っている」）が続く（`crates/pulsen/src/cli/render.rs`）。本スライスで初めて値が入るのは**「判定確定」「遷移」「実行待ちへ復帰」「通知」の4つ**（「判定確定」は completed を確定したタスクだけが並ぶ。failed は「失敗を記録」、skipped は「実行待ちへ復帰」に出る）。値の無い項目が行ごと出ないこと、書き込みを行った tick が必ずサマリーに現れること（ADR-092）を、確認項目1（判定確定・遷移）・4（実行待ちへ復帰）・7〜9（通知）で併せて確認する。`archived` / `gc_deleted` / `gc_errors` は引き続き値の入る経路を持たない（#6）。

- **`pulsen --help` の表示:** サブコマンドは `add` / `tick` / `help` のみで、`wrapper` は現れない（ADR-077）。`ls` / `show` / `abort` / `retry` / `set-status` が増えていないこと（#4 / #5）。`pulsen tick --help` に引数が無く、`--home` が global フラグとして現れること。

- **ロックの保持と cron 運用:** 判定・通知は排他ロックを保持したまま同期実行される（ADR-018）。エッジケース3 で tick が5秒強ブロックすることを実測済み。加えて、`.thread/2/testing.md` エッジケース6（滞留するエージェントを起動したまま次の tick を打つ = ロックFDの非継承）を再実行し、判定コマンド・通知コマンドの起動でもロックFDが継承されていないことを確認する — 継承していると、判定コマンドが長い間すべての tick がロック競合でスキップされる。

- **ラッパー（`pulsen wrapper`）:** 本スライスはラッパーを変更しない。確認項目1・10・11 で `exit` / `pid` / `starttime` が Issue #2 と同じキー構成（`exit` は `code`、`pid` は `pid` / `kill_ident`、`starttime` は `ident` / `wall`）の整形 JSON として書かれることをもって影響なしとする。

- **実運用ホームの非汚染:** 全項目の実行後に `ls -a "$HOME/.pulsen" 2>/dev/null` が実行前と変わらないこと。フィクスチャA は `/tmp/pulsen-test/`、B は `$HOME/pulsen-manual-test` / `$HOME/pulsen-test-repo` / `$HOME/pulsen-manual-work` の3つ（手順書の `PULSEN_HOME` / `REPO` / `WORK` と同じパス。B は冒頭でこの3つを `rm -rf` する）、C は `$HOME/pulsen-intervention-test` に閉じている。

- **後片付け:**

    ```sh
    ps -ef | grep -E 'pulsen wrapper|sleep (120|180|600)' | grep -v grep   # 残留プロセスが無いこと
    pkill -f 'sleep 600'; pkill -f 'sleep 120'; pkill -f 'sleep 180'       # 残っていれば
    git -C /tmp/pulsen-test/repo worktree list
    rm -rf /tmp/pulsen-test "$HOME/pulsen-manual-test" "$HOME/pulsen-manual-work" \
           "$HOME/pulsen-test-repo" "$HOME/pulsen-intervention-test"
    ```

  worktree を作ったリポジトリごと削除するので `git worktree prune` は不要。残留プロセスの確認は、kill / try_kill_remnants が実行単位を取り逃していないことの確認を兼ねる。

- **落とした手順の記帳:** 本スライスに無いコマンド（`ls` / `show` / `abort` / `retry` / `set-status`）と終端処理（#6）を要する手順は実行していない。カバーしたのは `spec/manual-tests/task-execution.md` TC-03(手順1〜9・11・12)・TC-05(手順1〜4・6 と手順5 の遷移まで)・TC-06・TC-07(手順1〜5)・TC-13・TC-14・TC-15・TC-17・TC-19(手順1〜5)・TC-20(手順1〜4・6。手順1 の2件目を `pipeline` に置き換え)・TC-21(手順1〜5。手順1 に対照用の2件目を足した)・TC-22・TC-23、`spec/manual-tests/setup.md` TC-09・TC-10(手順1〜4)・TC-11(手順1〜4)・TC-35(読み替え)・TC-37(手順1〜3)・TC-38(手順1〜2)・TC-39(手順1〜3・4の復元)・TC-47(手順1〜2)、`spec/manual-tests/intervention.md` TC-01(手順1〜3・5・7・8)・TC-15(読み替え)・TC-24(読み替え)。`intervention.md` TC-15 と `setup.md` TC-35 は同じ「notify_cmd 未定義 → 後から定義して catch-up」を問うので、確認項目9 手順7〜10 の1系列で両方を消化した(TC-15 手順2 の `abort` は上限超過での凍結に読み替え)。**落とした手順は Issue のチェックリストにチェックを付けず、見送った旨と理由を Issue #3 のコメントに残す**（Issue 完了条件 / steps.md ステップ15）。
