# 動作確認計画 — Issue #2: tick によるエージェント実行の起動(worktree確保・デタッチ起動・spawn確認)

**Issue:** #2
**作成日:** 2026-08-12

---

## 確認環境

この Issue の変更を確認するために必要な手順のみ記載する（プロジェクト全体のセットアップは省略）。

本スライスに存在するコマンドは `pulsen add` / `pulsen tick` / `pulsen wrapper`（隠しサブコマンド）の3つ。`ls` / `show` / `abort` / `retry` / `set-status` は未実装のため、状態の観測はすべて **`state/tasks/<task-id>.json` の直読**と **`state/runs/<task-id>/attempt-<n>/` 配下のファイルの直読**で行う（plan.md「テスト方針 — 手動確認」）。Web UI は無く、確認はすべてターミナル上の CLI 実行になる。

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
cargo build --examples
export PATH="$PWD/target/debug:$PATH"
pulsen --help
```

`--examples` が要るのは、ロック競合の確認に `crates/pulsen/examples/lock_holder.rs` を使うため（steps.md ステップ18 / plan.md テスト方針の TC-24 読み替え）。本スライスで追加される `agent_probe` / `spawn_probe`（steps.md ステップ8・11）は自動テスト用で、手動確認では使わない。examples の実体は `target/debug/examples/<name>` に出る。

以降の手順はすべてこの PATH が通ったシェルから実行する。exit code は各コマンドの直後に `echo $?` で確認する。

自動テスト側の状態（本確認の前提となるグリーン状態。plan.md AC-1）:

```sh
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

AC-1 が要求するのは `cargo clippy -- -D warnings` だが、examples とテストを含めて見るため `--all-targets` を付ける。

AC-1 の「OS 依存分岐がアダプター層に隔離されている」ことの機械的確認。Issue #1 の `cfg(unix)` / `cfg(windows)` だけを見る grep では ADR-067 が生む `#[cfg(target_os = "linux")]` を素通りさせるため、述語を4つに広げる。

```sh
grep -rnE 'cfg\([^)]*\b(unix|windows|target_os|target_family)\b' crates/*/src/
```

`crates/pulsen-domain/` が1件もヒットせず、`crates/pulsen/src/` 側のヒットが `util/atomic.rs` と `adapter/process.rs` の2ファイルだけであること。`crates/pulsen-conformance/src/lib.rs` のヒットは適合ハーネスが権限制限の効き目を probe する分岐で、本番の実行経路には乗らない（Issue #1 と同じ扱い）。

AC-6 の「未実装メソッドの宣言・スタブが1つも無い」ことの機械的確認:

```sh
grep -nE 'fn (attempt_exists|list_runs|delete_attempt|remove_task_dir_if_empty|starttime_of|kill|try_kill_remnants|remove)\(' crates/pulsen-domain/src/execution/port.rs
```

1件もヒットしないこと（いずれも Issue #3 / #4 / #6 の担当）。

### 検証用のフィクスチャ準備

手動確認は2つの手順書に由来するので、フィクスチャも手順書ごとに分けて用意する。パスは各手順書の記載どおりに保つ（読み替えによる取り違えを避けるため）。

#### フィクスチャA — `spec/manual-tests/task-execution.md`（TC-03 / TC-04 / TC-12 / TC-16 / TC-20 / TC-24 / TC-25 用）

同ドキュメントの「事前準備」1〜6 をそのまま実行する。

1. テスト領域と分離ホームを初期化する。

    ```sh
    rm -rf /tmp/pulsen-test
    mkdir -p /tmp/pulsen-test/home/workflows /tmp/pulsen-test/bin
    export PULSEN_HOME=/tmp/pulsen-test/home
    ```

2. グローバル設定を作成する。

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

3. 本スライスで使うワークフロー定義を配置する（`pipeline` は TC-03 / TC-04 / TC-16 / TC-20 / TC-24 / TC-25、`fail` は TC-12、`draft.yaml` は TC-20 の T20h、`broken-syntax.yaml` は事前準備の一部）。

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

    cat > /tmp/pulsen-test/draft.yaml <<'EOF'
    workflow: my-flow
    agent: shell
    initial: done
    statuses:
      done:
        run: cleanup
    EOF

    printf 'workflow: [unclosed\nstatuses\n' > /tmp/pulsen-test/broken-syntax.yaml
    ```

4. 対象リポジトリと、TC-12 用の使い捨てリポジトリを作成する（worktree 内でコミットするため identity をリポジトリローカルに設定する）。

    ```sh
    for r in repo repo2; do
      git init -b main "/tmp/pulsen-test/$r"
      git -C "/tmp/pulsen-test/$r" config user.name pulsen-test
      git -C "/tmp/pulsen-test/$r" config user.email pulsen-test@example.com
      git -C "/tmp/pulsen-test/$r" commit --allow-empty -m init
    done
    ```

5. 通知ログを空にする（本スライスでは通知は行われないので「増えないこと」の観測用）。

    ```sh
    : > /tmp/pulsen-test/notify.log
    ```

6. 確認の共通ヘルパー。`state/tasks/` は登録が1件も成功していない時点では存在しない。

    ```sh
    ls "$PULSEN_HOME/state/tasks/" 2>/dev/null
    cat "$PULSEN_HOME/state/tasks/<task-id>.json"
    ls -R "$PULSEN_HOME/state/runs/<task-id>/" 2>/dev/null
    ls "$PULSEN_HOME/worktrees/" 2>/dev/null
    git -C /tmp/pulsen-test/repo branch --list 'pulsen/*'
    ```

    JSON の項目を絞って見たい場合は `jq` を使ってよい（`/usr/bin/jq` を Issue #1 の確認で確認済み。devShell には含まれないためシステムの `jq` を使う）。

本スライスでは `sleeper` / `judgefail` / `flaky` / `longrun` / `pr-review-watch` / `wtloss` / `broken` / `fail0` / `exit20` の各定義と `check-reviews.sh` は使わない（いずれも判定・遷移・kill を要し Issue #3 / #6 の担当）。作成は省略してよい。

#### フィクスチャB — `spec/manual-tests/setup.md`（TC-02 / TC-06 / TC-34 用）

同ドキュメントの「事前準備」1〜3 と TC-01 手順1・TC-03 手順1 を先行実行する（plan.md のテスト方針が明示している前提）。フィクスチャA とは別のホームを使うので、混ざらない。

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
```

`judge.sh` は setup.md 事前準備3 の一部として作るだけで、本スライスでは参照されない（判定は Issue #3）。

フィクスチャB を使う項目は `--home "$SETUP_HOME"` を明示するか、そのシェルで `export PULSEN_HOME="$SETUP_HOME"` に切り替えてから実行する。以降、フィクスチャA を使う項目は `PULSEN_HOME=/tmp/pulsen-test/home` を前提に書く。

#### 実行順序

`spec/manual-tests/task-execution.md`「実行上の注意」が記載順の実行を前提にしているため、フィクスチャA を使う項目は本書の記載順（確認項目2 → 3 → 4 → 5 → 6 → エッジケース1 → 2 → 3 → 4 → 5 → …）に実行する。特に config.yaml と `pipeline.yaml` を改変する項目は、各項目の末尾に置いた復元手順を必ず実行する（復元しないと後続項目の期待が必ず外れる）。

各項目の冒頭には、その項目が前提とする状態を明記してある。

### デプロイ方法

なし（ローカル CLI のみで確認できる）。

## 確認項目

### 1. タスク0件での tick と `state/tasks/` 未作成での tick

- **対応する受け入れ基準:** AC-12
- **前提:** フィクスチャB（`setup.md` TC-01 完了直後 = タスク未登録）。`spec/manual-tests/setup.md` TC-02 の全手順。setup.md 内では TC-03 の登録より前に実行する必要がある。
- **目的:** 走査対象が無い状態で tick が何も書かず「処理対象なし」を表示して 0 で終わること、`state/tasks/` そのものが未作成でも同じ結果になることを確認する。
- **手順:**
  1. `ls "$SETUP_HOME/state" 2>/dev/null; echo $?`（この時点では `state/` は存在しない）
  2. `pulsen tick --home "$SETUP_HOME"; echo $?`
  3. `ls -a "$SETUP_HOME/state/" 2>/dev/null; ls "$SETUP_HOME/state/tasks/" 2>/dev/null`
  4. `ls "$SETUP_HOME/worktrees/" "$SETUP_HOME/state/runs/" 2>/dev/null; echo $?`
  5. `pulsen tick --home "$SETUP_HOME"; echo $?`（2回目。冪等性）
- **期待結果:** 手順2・5 とも exit code 0 で、「処理対象がない」旨が表示される。手順3 で `state/tasks/` が空（またはロック取得の副産物として `state/` と `state/lock` だけがある）。手順4 で `worktrees/` も `state/runs/` も作られていない。
- **確認ポイント:** サマリーに `launched` / `frozen` / `errors` などの空フィールドが並ばず、「対象なし」だけが読めること（ADR-065: 値の入っていないフィールドは表示しない）。2回目の実行で表示が変わらないこと。

### 2. 起動フェーズ — worktree・ブランチ・launching 記録・runディレクトリ

- **対応する受け入れ基準:** AC-12, AC-13, AC-15（F2 / F4 / F6）
- **前提:** フィクスチャA。`spec/manual-tests/task-execution.md` TC-03 手順1〜3。
- **目的:** 手続きAが spec の順序（worktree確保 → テンプレート展開 → launching記録 → デタッチspawn）で1 tick 分だけ進み、worktree・ブランチ・タスクファイル・runディレクトリの4箇所に期待どおりの痕跡が残ることを確認する。
- **手順:**
  1. `pulsen add --workflow pipeline --repo /tmp/pulsen-test/repo; echo $?` → 表示された ID を `export T3=<task-id>`
  2. `workflows/pipeline.yaml` の `queued` の prompt を書き換える（登録後の元YAML編集。手順6 で観測する）:

      ```sh
      sed -i.orig 's/prompt: "echo planning .*"/prompt: "echo edited-should-not-appear"/' /tmp/pulsen-test/home/workflows/pipeline.yaml
      grep -n 'edited-should-not-appear' /tmp/pulsen-test/home/workflows/pipeline.yaml
      ```

  3. `pulsen tick; echo $?`
  4. `cat "$PULSEN_HOME/state/tasks/$T3.json"`
  5. `ls "$PULSEN_HOME/worktrees/"; ls -a "$PULSEN_HOME/worktrees/$T3"; git -C /tmp/pulsen-test/repo branch --list 'pulsen/*'; git -C /tmp/pulsen-test/repo worktree list`
  6. `ls -la "$PULSEN_HOME/state/runs/$T3/attempt-1/"`
- **期待結果:**
  - 手順3: exit code 0。サマリーに T3 の起動（`launched`）が1件記録される。
  - 手順4: `execution` が `{"state":"launching","recorded_at":"<RFC3339 UTC>"}`、`workspace` が `{"path":"<PULSEN_HOME>/worktrees/<T3>","branch":"pulsen/<T3>"}`、`current_attempt` が `{"number":1,"run_dir":"<PULSEN_HOME>/state/runs/<T3>/attempt-1","process":null}`、`counters` は全0のまま、`task_status` は `queued` のまま。
  - 手順5: `worktrees/<T3>` が実体として存在し、`git worktree list` に登録されている。ブランチ `pulsen/<T3>` が `main` から作られている。
  - 手順6: `attempt-1/` が存在し、`starttime` / `pid` が現れる（ラッパーはデタッチ起動で非同期に完了するため、直後に見えなければ2〜3秒あけて再実行する）。
- **確認ポイント:** `workspace.path` と `current_attempt.run_dir` がいずれも**絶対パス**であること（外部スケジューラーからの起動が成立する前提。AC-12）。`pid` の中身が `{"pid":<数値>,"kill_ident":"<非空文字列>"}` であること — `kill_ident` が空文字や `0` になっていると Issue #3 の kill が無関係なプロセスを対象にしうる（ADR-067）。`starttime` が `{"ident":"...","wall":"..."}` の形で、`ident` が非空であること。

### 3. spawn確認 — 次 tick での running 取込と猶予内の冪等性

- **対応する受け入れ基準:** AC-14
- **前提:** 確認項目2 の直後（T3 が `launching`）。`task-execution.md` TC-03 手順4。
- **目的:** 手続きCが pid + starttime の出現をもって `running` へ取り込むこと、猶予時間内（30秒以内）に pid がまだ無い場合は書き込みを一切起こさずに `launching` のまま待つことを確認する。
- **手順:**
  1. 確認項目2 の手順3 の直後（1秒以内）にもう一度 `pulsen tick; echo $?` を打ち、`cat "$PULSEN_HOME/state/tasks/$T3.json"` と `stat` でタスクファイルの更新時刻を確認する（猶予内経路。pid がまだ書かれていない場合にだけ意味を持つ手順なので、pid が既にあれば「観測できず」と記録して次へ進む）
  2. 2〜3秒あけて `pulsen tick; echo $?`
  3. `cat "$PULSEN_HOME/state/tasks/$T3.json"`
  4. `ls "$PULSEN_HOME/state/runs/$T3/attempt-1/"`（`invalidated` が無いこと）
- **期待結果:**
  - 手順1: exit code 0。タスクファイルの内容も `updated_at` も変わらない（`KeepWaiting` は書き込みを一切発生させない）。
  - 手順2・3: `execution` が `{"state":"running"}` になり、`current_attempt.process` に `pid` / `kill_ident` / `starttime`（`ident` と `wall`）が入る。`counters.spawn_fail_count` は 0。`task_status` は `queued` のまま。
  - 手順4: `invalidated`（無効化マーカー）が作られていない（猶予超過していないため）。
- **確認ポイント:** `current_attempt.process.starttime.ident` が `attempt-1/starttime` ファイルの `ident` と**完全に一致**すること — ラッパーが記録した値がそのまま帳簿へ移ることが、Issue #3 の生存判定の前提になる（ADR-067）。取り込み後も `attempt_count` が 0 のままであること（起動は attempt_count を消費しない）。

### 4. ラッパーの成果物とスナップショットの有効性

- **対応する受け入れ基準:** AC-10, AC-11, AC-15
- **前提:** 確認項目3 の直後。`task-execution.md` TC-03 手順7 と手順12。
- **目的:** ラッパーが `own_identity` → `starttime` → `pid` → マーカー確認 → エージェント実行 → `exit` の順に成果を残すこと、実行されたコマンドが**登録時のスナップショット**由来であって元YAMLの編集に影響されないことを確認する。
- **手順:**
  1. `exit` ファイルが現れるまで待つ: `until [ -f "$PULSEN_HOME/state/runs/$T3/attempt-1/exit" ]; do sleep 1; done; ls -la "$PULSEN_HOME/state/runs/$T3/attempt-1/"`
  2. `cat "$PULSEN_HOME/state/runs/$T3/attempt-1/stdout.log"`
  3. `cat "$PULSEN_HOME/state/runs/$T3/attempt-1/stderr.log"`
  4. `cat "$PULSEN_HOME/state/runs/$T3/attempt-1/exit"; echo`
  5. `cat "$PULSEN_HOME/state/runs/$T3/attempt-1/pid"; echo; cat "$PULSEN_HOME/state/runs/$T3/attempt-1/starttime"; echo`
  6. `ls "$PULSEN_HOME/worktrees/$T3/"; git -C "$PULSEN_HOME/worktrees/$T3" log --oneline`
  7. 復元（後続項目のため必ず実行する）: `cp /tmp/pulsen-test/pipeline.bak /tmp/pulsen-test/home/workflows/pipeline.yaml && rm -f /tmp/pulsen-test/home/workflows/pipeline.yaml.orig && grep -n 'echo planning' /tmp/pulsen-test/home/workflows/pipeline.yaml`
- **期待結果:**
  - 手順1: `pid` / `starttime` / `stdout.log` / `stderr.log` / `exit` の5ファイルが揃う。`invalidated` は無い。
  - 手順2: `planning` が出力されている。手順2 で書き換えた `edited-should-not-appear` は**出ていない**（スナップショットが使われた証拠）。
  - 手順4: `{"code":0}`（ADR-072 により JSON。素の `0` ではない）。
  - 手順5: `pid` は `{"pid":<数値>,"kill_ident":"<非空>"}`、`starttime` は `{"ident":"<非空>","wall":"<RFC3339>"}`。
  - 手順6: worktree に `plan.txt` があり、`plan` のコミットが1件積まれている。
- **確認ポイント:** `starttime` の mtime が `pid` の mtime より**古いか同時**であること（`ls -la --time-style=full-iso` や `stat` で確認。starttime → pid の順序が二重起動排除の前提。plan.md リスク項）。`exit` が `pid` より後に現れていること。`stderr.log` が存在すること（空でもよい）。

### 5. 同一リポジトリ・2タスクの独立した起動

- **対応する受け入れ基準:** AC-15（F4 / F6）
- **前提:** フィクスチャA（確認項目4 まで完了。`pipeline.yaml` は復元済み）。`task-execution.md` TC-04 手順1〜3 と手順5。
- **目的:** 同一リポジトリ・同一ベースブランチ・同一ワークフローの2タスクが重複排除されず、別々の worktree・ブランチ・runディレクトリで**同一 tick のうちに**並行して起動されることを確認する。
- **手順:**
  1. `pulsen add --workflow pipeline --repo /tmp/pulsen-test/repo; echo $?` → `export T4A=<task-id>`
  2. `pulsen add --workflow pipeline --repo /tmp/pulsen-test/repo; echo $?` → `export T4B=<task-id>`
  3. `pulsen tick; echo $?`
  4. `ls "$PULSEN_HOME/worktrees/"; git -C /tmp/pulsen-test/repo branch --list 'pulsen/*'; git -C /tmp/pulsen-test/repo worktree list`
  5. `cat "$PULSEN_HOME/state/tasks/$T4A.json"; cat "$PULSEN_HOME/state/tasks/$T4B.json"`
  6. `ls "$PULSEN_HOME/state/runs/$T4A/" "$PULSEN_HOME/state/runs/$T4B/"`
  7. `git -C /tmp/pulsen-test/repo log --oneline "pulsen/$T4A"; git -C /tmp/pulsen-test/repo log --oneline "pulsen/$T4B"`
- **期待結果:** 手順1・2 は異なるタスクIDで exit code 0。手順3 は exit code 0 で、サマリーに**2件**の起動が記録される（並列度制御は行われない）。手順4 で `worktrees/` に `<T4A>` と `<T4B>` の別ディレクトリがあり、`pulsen/<T4A>` と `pulsen/<T4B>` の両ブランチが `main` から作られている。手順5 でそれぞれの `workspace.path` / `workspace.branch` / `current_attempt.run_dir` が互いに異なる。
- **確認ポイント:** 両ブランチの起点が `main` であること（`git merge-base --is-ancestor main pulsen/<T4A>` が 0 で終わる）。TC-04 手順5 の「3コミットが積まれている」は遷移（Issue #3）を要するため確認しない — ブランチの存在と1コミット目（`plan`）までを確認範囲とする。

### 6. failed からの再起動 — attempt の採番と worktree の引き継ぎ

- **対応する受け入れ基準:** AC-15（F4）
- **前提:** 確認項目4 まで完了（T3 の attempt-1 が完走し、worktree に `plan.txt` がある）。本スライスには判定（Issue #3）が無いため `failed` に到達する経路が無い。タスクファイルを直接書き換えて `failed` を作る（`retry` / `set-status` も本スライスに無い）。
- **目的:** 同一ステータスの再実行が**新しい attempt 番号**で行われ、runディレクトリが分かれる一方で、**worktree は同一のものが使われ内容がリセットされない**ことを確認する。
- **手順:**
  1. `cp "$PULSEN_HOME/state/tasks/$T3.json" /tmp/pulsen-test/t3.bak`
  2. 実行状態を `failed` にする（`current_attempt` はそのまま残す。次の採番が 2 になる根拠になる）:

      ```sh
      jq '.execution = {"state":"failed"} | .counters.attempt_count = 1' \
        "$PULSEN_HOME/state/tasks/$T3.json" > /tmp/pulsen-test/t3.new \
        && mv /tmp/pulsen-test/t3.new "$PULSEN_HOME/state/tasks/$T3.json"
      cat "$PULSEN_HOME/state/tasks/$T3.json"
      ```

  3. `ls "$PULSEN_HOME/worktrees/$T3/"`（`plan.txt` がある状態を確認）
  4. `pulsen tick; echo $?`
  5. `cat "$PULSEN_HOME/state/tasks/$T3.json"`
  6. `ls "$PULSEN_HOME/state/runs/$T3/"`
  7. `until [ -f "$PULSEN_HOME/state/runs/$T3/attempt-2/exit" ]; do sleep 1; done; ls "$PULSEN_HOME/worktrees/$T3/"; git -C "$PULSEN_HOME/worktrees/$T3" log --oneline`
- **期待結果:** 手順4 は exit code 0 で起動が1件。手順5 で `execution` が `launching` になり、`current_attempt.number` が **2**、`run_dir` が `.../attempt-2`。`workspace` は手順2 の値から変わっていない。手順6 で `attempt-1` と `attempt-2` の両方がある。手順7 で worktree に `plan.txt` が残ったまま（内容がリセットされていない）。
- **確認ポイント:** 再起動時に `git worktree add` が再実行されず、既存の worktree がそのまま使われること（`workspace` が確定済みなので `create` は呼ばれない。tick のサマリーに worktree 作成に関する報告が出ないこと）。`attempt_count` が起動によって増えないこと（増えるのは worktree 作成失敗と判定失敗の経路だけ）。

### 7. `wrapper` 隠しサブコマンドの単体動作

- **対応する受け入れ基準:** AC-11
- **前提:** フィクスチャA（`PULSEN_HOME` の値は使わない）。`pulsen wrapper` は tick を介さず手でも起動できる。
- **目的:** ラッパーが config もホームも読まず、起動引数だけで動作し、結果をすべて runディレクトリのファイルとして残すことを確認する。
- **手順:**
  1. `pulsen --help`（`wrapper` が一覧に現れないこと）
  2. `pulsen wrapper --help; echo $?`（隠れているだけで到達できること）
  3. 実行用のパスを作る:

      ```sh
      export WRUN=/tmp/pulsen-test/home/state/runs/20260812t000000-aaaaaaaa/attempt-1
      mkdir -p /tmp/pulsen-test/wrapws
      ```

  4. config を一切参照しない状態で起動する（存在しないホームを指し、`HOME` も差し替える）:

      ```sh
      env PULSEN_HOME=/no/such/home HOME=/no/such/home \
        pulsen wrapper --run-dir "$WRUN" --workspace /tmp/pulsen-test/wrapws \
        -- sh -c 'echo out; echo err 1>&2; exit 7'
      echo "wrapper exit: $?"
      ```

  5. `ls -la "$WRUN"; cat "$WRUN/exit"; echo; cat "$WRUN/stdout.log"; cat "$WRUN/stderr.log"`
  6. 無効化マーカーがある場合にエージェントを起動しないこと:

      ```sh
      export WRUN2=/tmp/pulsen-test/home/state/runs/20260812t000000-bbbbbbbb/attempt-1
      mkdir -p "$WRUN2" && : > "$WRUN2/invalidated"
      pulsen wrapper --run-dir "$WRUN2" --workspace /tmp/pulsen-test/wrapws -- sh -c 'echo should-not-run > /tmp/pulsen-test/wrapws/ran.txt'
      echo "wrapper exit: $?"
      ls -la "$WRUN2"; ls /tmp/pulsen-test/wrapws/
      ```

- **期待結果:**
  - 手順1: サブコマンド一覧は `add` / `tick` / `help` のみ。`wrapper` は現れない。
  - 手順2: ヘルプが表示され、`--run-dir` / `--workspace` と末尾のエージェントコマンドの3値を取ることが読める。
  - 手順4・5: `$WRUN` に `starttime` / `pid` / `stdout.log` / `stderr.log` / `exit` が揃い、`exit` は `{"code":7}`、`stdout.log` に `out`、`stderr.log` に `err`。config が無くても影響しない。
  - 手順6: `$WRUN2` に `starttime` と `pid` は書かれるが `exit` は書かれず、`ran.txt` も作られない（マーカーがあるのでエージェントを起動せず正常終了する）。
- **確認ポイント:** 手順4 の run_dir は `state/runs/<task-id>/attempt-<n>` の形をしているだけで、**タスクが実在しなくても動く**こと（ラッパーは帳簿を読まない。ADR-070）。ラッパー自身の標準出力には何も出ないこと（結果はファイルにしか現れない）。ラッパー自身の終了コードは spec が規定していないので主張しない。

### 8. 外部スケジューラー(cron)からの tick と任意の作業ディレクトリからの起動

- **対応する受け入れ基準:** AC-12
- **前提:** フィクスチャB。`spec/manual-tests/setup.md` TC-06 の手順1〜3 と、手順4・5 のうち**起動フェーズまで**、および手順7。手順6（worktree 内の成果物）と手順8（`set-status`）は範囲外。
- **目的:** tick が対象リポジトリの外・cron のような最小環境から起動されても同じ結果になること（帳簿がすべて絶対パスで閉じていること）を確認する。
- **手順:**
  1. `which pulsen`（絶対パスを控える。`$PWD/target/debug/pulsen` になる）
  2. `crontab -e` で以下の1行を登録する（`<pulsen>` は手順1、`<home>` は `$SETUP_HOME` の絶対パス）:

      ```
      * * * * * <pulsen> tick --home <home> >> <home>/cron.log 2>&1
      ```

  3. `pulsen add --workflow implement --repo "$SETUP_REPO" --home "$SETUP_HOME"; echo $?` → `export T6=<task-id>`
  4. 3〜4分待ってから `cat "$SETUP_HOME/cron.log"`
  5. `cat "$SETUP_HOME/state/tasks/$T6.json"`
  6. `ls "$SETUP_HOME/worktrees/"; git -C "$SETUP_REPO" branch --list 'pulsen/*'; ls -R "$SETUP_HOME/state/runs/$T6/"`
  7. `crontab -e` で手順2 の行を削除する
  8. cron を使えない環境（macOS でフルディスクアクセスが許可できない等）では手順2・4 を次で代替し、cron 分をスキップとして記録する:

      ```sh
      cd /tmp && pulsen tick --home "$SETUP_HOME" >> "$SETUP_HOME/cron.log" 2>&1; echo $?
      sleep 3
      cd /tmp && pulsen tick --home "$SETUP_HOME" >> "$SETUP_HOME/cron.log" 2>&1; echo $?
      cd /Users/hikaru/github.com/tuanemuy/pulsen
      ```

- **期待結果:** 手順4 で `cron.log` に tick のサマリーが1分おきに追記され、T6 の起動が記録されている。手順5 で `execution` が `running`（起動 → spawn確認の2 tick を経ている。以降の tick では変化しない = 手続きD は Issue #3）。手順6 で `worktrees/<T6>` とブランチ `pulsen/<T6>`、`state/runs/<T6>/attempt-1/` に `starttime` / `pid` / `stdout.log` / `stderr.log` / `exit` が揃う。
- **確認ポイント:** `cron.log` にエラー行（ホーム未初期化・パス解決の失敗）が1行も無いこと。`cd /tmp` してから打った tick と、リポジトリ内から打った tick で結果が変わらないこと。`--home` を付け忘れると cron 環境では既定の `~/.pulsen` が解決されるため、この確認は `--home` の絶対パス指定とセットで意味を持つ。TC-06 手順4 の「`review_waiting` まで進行」と手順5 の成果物・手順6・手順8 は判定と遷移（Issue #3）・終端処理（Issue #6）を要するので確認しない。

### 9. アーカイブ済みタスクが走査対象に入らない

- **対応する受け入れ基準:** AC-12（PAGE-tick-008）
- **前提:** フィクスチャA。本スライスにはアーカイブを行う手続き（Issue #6）が無いため、`state/archive/` へファイルを直に置いて状態を作る。
- **目的:** `list_active` が `state/tasks/` だけを走査し、アーカイブ済みのタスクに対して起動も報告も行わないことを確認する。
- **手順:**
  1. `pulsen add --workflow pipeline --repo /tmp/pulsen-test/repo; echo $?` → `export T9=<task-id>`
  2. `mkdir -p "$PULSEN_HOME/state/archive" && mv "$PULSEN_HOME/state/tasks/$T9.json" "$PULSEN_HOME/state/archive/$T9.json"`
  3. `pulsen tick; echo $?`
  4. `cat "$PULSEN_HOME/state/archive/$T9.json"`
  5. `ls "$PULSEN_HOME/worktrees/" ; ls "$PULSEN_HOME/state/runs/" ; git -C /tmp/pulsen-test/repo branch --list "pulsen/$T9"`
- **期待結果:** 手順3 は exit code 0 で、サマリーに T9 は現れない。手順4 でアーカイブ側のファイルが手順2 の内容のまま（`execution` は `pending`、`updated_at` も変化なし）。手順5 で `worktrees/<T9>` も `state/runs/<T9>/` もブランチ `pulsen/<T9>` も作られていない。
- **確認ポイント:** `state/archive/` にファイルがあること自体が `errors` に報告されないこと（走査対象外は異常ではない）。`state/lock` が `<task-id>.json` 形式でないため走査に混ざらないこと（Issue #1 で確認済みの性質が tick 側でも保たれている）。

### 10. tick の冪等性（状態が変わらないタスク群に対する連続実行）

- **対応する受け入れ基準:** AC-12, AC-14
- **前提:** 確認項目6 まで完了（`running` や `launching` のタスクが混在している状態）。
- **目的:** 同じ事実から常に同じ判断が再導出され、余分な書き込みが発生しないこと（CLAUDE.md「冪等性を前提に設計する」）を確認する。
- **手順:**
  1. `ls -l "$PULSEN_HOME/state/tasks/" > /tmp/pulsen-test/before.txt; md5 -q "$PULSEN_HOME/state/tasks/"*.json > /tmp/pulsen-test/before.md5 2>/dev/null || md5sum "$PULSEN_HOME/state/tasks/"*.json > /tmp/pulsen-test/before.md5`
  2. `pulsen tick; echo $?`
  3. `pulsen tick; echo $?`
  4. `md5 -q "$PULSEN_HOME/state/tasks/"*.json > /tmp/pulsen-test/after.md5 2>/dev/null || md5sum "$PULSEN_HOME/state/tasks/"*.json > /tmp/pulsen-test/after.md5; diff /tmp/pulsen-test/before.md5 /tmp/pulsen-test/after.md5; echo $?`
  5. `ls -l "$PULSEN_HOME/state/tasks/"`（`updated_at` の変化を伴う mtime 更新が無いこと）
- **期待結果:** 手順2・3 とも exit code 0。手順4 の `diff` が差分なし（exit 0）で、`running` のタスクにも `launching`（猶予内）のタスクにも書き込みが発生しない。
- **確認ポイント:** `running` のタスクに対して tick が「未実装」等の報告を出さないこと（ADR-065: 未配線のアームは報告もしない）。サマリーの `errors` が空であること。

## エッジケース・異常系

すべてのケースで、**tick 自体の exit code が 0 であること**（tick は1タスクの失敗で全体を落とさない。ロック機構の異常と走査の Io だけが非0）と、**該当タスク以外の状態が変わっていないこと**を共通の確認手段とする。

### 1. worktree 作成失敗（登録後のリポジトリ消失）とリトライ上限超過

- **対応する受け入れ基準:** AC-13
- **前提:** フィクスチャA。`task-execution.md` TC-12。通知の受領（Issue #3）は確認しない。
- **目的:** worktree 作成の失敗が `record_tool_failure(WorktreeCreate)` により `failed` として `attempt_count` を消費し、リトライ上限（`fail.yaml` の `retries: 1`）の**超過**で `stopped` になることを確認する。
- **手順:**
  1. `pulsen add --workflow fail --repo /tmp/pulsen-test/repo2; echo $?` → `export T12=<task-id>`
  2. `mv /tmp/pulsen-test/repo2 /tmp/pulsen-test/repo2.gone`
  3. `pulsen tick; echo $?`
  4. `cat "$PULSEN_HOME/state/tasks/$T12.json"; ls "$PULSEN_HOME/state/runs/" ; ls "$PULSEN_HOME/worktrees/"`
  5. `pulsen tick; echo $?`
  6. `cat "$PULSEN_HOME/state/tasks/$T12.json"`
  7. `pulsen tick; echo $?; cat "$PULSEN_HOME/state/tasks/$T12.json"`
  8. `cat /tmp/pulsen-test/notify.log`
- **期待結果:**
  - 手順3: exit code 0。サマリーに T12 の失敗が報告される。
  - 手順4: `execution` が `{"state":"failed"}`、`counters.attempt_count` が 1（= 上限 1。**等号では凍結しない**）、`last_failure.kind` が `worktree_create` でメッセージが記録されている。`workspace` は `null` のまま。`state/runs/<T12>/` と `worktrees/<T12>` は作られていない。
  - 手順6: `attempt_count` が 2 > 1 のため `execution` が `{"state":"stopped","reason":"retry_limit_exceeded","notified_at":null}` になり、tick サマリーの `frozen` に1件記録される。
  - 手順7: 以降の tick は T12 に何もしない（`updated_at` が変わらない。ADR-065 で `Stopped` のアームは未配線）。
  - 手順8: T12 の通知行は無い（notify は Issue #3。ADR-066）。
- **確認ポイント:** `notified_at` が `null` のまま永続化されていること（Issue #3 がマージされた後の最初の tick が catch-up するための at-least-once の前提。ADR-066）。失敗しても runディレクトリが作られないこと（採番は worktree 確保の**後**）。

### 2. テンプレート展開失敗（登録後の設定破壊）と config 修復での復帰

- **対応する受け入れ基準:** AC-13
- **前提:** フィクスチャA。`task-execution.md` TC-16 手順1〜5（手順6 の通知は Issue #3）と `setup.md` TC-34 手順1〜4 が同じ経路を通る。前提として `pending` / `failed` の現役タスクが他に残っていないこと — 残っていると同じ spawn 失敗を蓄積する。確認項目6 で `failed` にした T3 は `running` に戻っているので該当しない。
- **目的:** テンプレート展開の失敗が `record_spawn_failure_in_place` に落ち、**実行状態も attempt 番号も変えずに** `spawn_fail_count` だけを増やすこと、上限（既定 3）の超過で `stopped` になること、config を直すと次の tick で起動に成功すること（`TC-exec-tick-055`）を確認する。
- **手順:**
  1. `pulsen add --workflow pipeline --repo /tmp/pulsen-test/repo; echo $?` → `export T16=<task-id>`
  2. `sed -i.bak2 's/^  shell:/  shellx:/' /tmp/pulsen-test/home/config.yaml && grep -n 'shellx' /tmp/pulsen-test/home/config.yaml`
  3. `pulsen tick; echo $?`
  4. `cat "$PULSEN_HOME/state/tasks/$T16.json"; ls "$PULSEN_HOME/worktrees/"; ls "$PULSEN_HOME/state/runs/" `
  5. `pulsen tick; pulsen tick; cat "$PULSEN_HOME/state/tasks/$T16.json"`
  6. `pulsen tick; echo $?; cat "$PULSEN_HOME/state/tasks/$T16.json"`
  7. `cat /tmp/pulsen-test/notify.log`
  8. `TC-exec-tick-055`（config 修正が次の tick で反映される）は上限に達していない**別タスク**で裏付ける:

      ```sh
      pulsen add --workflow pipeline --repo /tmp/pulsen-test/repo; echo $?   # → T16b
      pulsen tick                      # spawn_fail_count 1
      cat "$PULSEN_HOME/state/tasks/<T16b>.json"
      cp /tmp/pulsen-test/config.bak /tmp/pulsen-test/home/config.yaml       # 復元（後続項目のためにも必須）
      pulsen tick; echo $?
      cat "$PULSEN_HOME/state/tasks/<T16b>.json"
      ```

  9. 片付け: `rm -f /tmp/pulsen-test/home/config.yaml.bak2 && grep -n 'shell:' /tmp/pulsen-test/home/config.yaml`
- **期待結果:**
  - 手順4: exit code 0。`execution` は `{"state":"pending"}` の**まま**、`current_attempt` は `null` の**まま**（採番されない）、`counters.spawn_fail_count` が 1、`last_failure.kind` が `spawn_fail`。`worktrees/<T16>` は**作成される**（worktree 確保はテンプレート展開より前）。`state/runs/<T16>/` は作られない。
  - 手順5: `spawn_fail_count` が 2 → 3（3 = 上限。等号では凍結しない）。実行状態は `pending` のまま。
  - 手順6: `spawn_fail_count` 4 > 3 で `execution` が `{"state":"stopped","reason":"spawn_fail_limit_exceeded","notified_at":null}`。サマリーの `frozen` に1件。
  - 手順7: T16 の通知行は無い（Issue #3）。
  - 手順8: config 復元後の tick で T16b が `launching` になり、`worktrees/<T16b>` と `state/runs/<T16b>/attempt-1/` が作られる。
- **確認ポイント:** 手順4 で「worktree は作られるが runディレクトリは作られない」という非対称が成り立つこと（手続きAの段階の順序がそのまま観測される）。`attempt_count` が一度も増えないこと（spawn 失敗と実行失敗はカウンタが別）。**手順8 の config 復元は後続項目の前提なので必ず実行する**（`shellx` のままだと以降のすべての起動が spawn 失敗になる）。

### 3. パース不能なタスクファイルの混在と他タスクの続行

- **対応する受け入れ基準:** AC-12
- **前提:** フィクスチャA（config 復元済み）。`task-execution.md` TC-20 手順1〜4。plan.md の読み替えにより、手順1 に `pipeline` のタスク（T20p）を1件追加する — `draft.yaml` の T20h は Pending × Cleanup で、`Cleanup` のアームは本スライスで配線しないため（ADR-065）「他タスクへの影響なし」を裏付ける対象にならない。
- **目的:** 読めないタスクファイルが**報告のみ**でスキップされ、書き込み・stopped 化が起きないこと、同一 tick の他タスクが通常どおり起動されることを確認する。
- **手順:**
  1. `pulsen add --workflow pipeline --repo /tmp/pulsen-test/repo; echo $?` → `export T20=<task-id>`
  2. `pulsen add --workflow /tmp/pulsen-test/draft.yaml --repo /tmp/pulsen-test/repo; echo $?` → `export T20H=<task-id>`
  3. `pulsen add --workflow pipeline --repo /tmp/pulsen-test/repo; echo $?` → `export T20P=<task-id>`
  4. `cp "$PULSEN_HOME/state/tasks/$T20.json" /tmp/pulsen-test/t20.bak && echo broken > "$PULSEN_HOME/state/tasks/$T20.json"`
  5. `pulsen tick; echo $?`
  6. `cat "$PULSEN_HOME/state/tasks/$T20.json"`
  7. `cat "$PULSEN_HOME/state/tasks/$T20H.json"`
  8. `cat "$PULSEN_HOME/state/tasks/$T20P.json"; ls "$PULSEN_HOME/worktrees/"; ls "$PULSEN_HOME/state/runs/$T20P/"`
  9. `cat /tmp/pulsen-test/notify.log`
  10. 復元: `cp /tmp/pulsen-test/t20.bak "$PULSEN_HOME/state/tasks/$T20.json"`
- **期待結果:**
  - 手順5: exit code **0**。サマリーの `errors` に「読めないタスクファイル」として **T20 のファイルパス**が報告される。
  - 手順6: 内容は `broken` のまま（破損ファイルへの書き込みは行われない）。
  - 手順7: T20H は `pending` のまま何も起きない（`Cleanup` のアームが未配線。ADR-065。TC-20 の「アーカイブされる」は Issue #6 に読み替え）。
  - 手順8: T20P は破損ファイルと**同一 tick で起動され**、`launching`・`worktrees/<T20P>`・`state/runs/<T20P>/attempt-1/` が作られる。
  - 手順9: T20 に関する通知は無い。
- **確認ポイント:** `errors` の行から「どのファイルが読めなかったか」がパスで特定できること（修復の入口。`ls` が無い本スライスではこれが唯一の報告経路）。破損ファイルの mtime が変わっていないこと。

### 4. スナップショットのみ破損したタスクのスキップと報告

- **対応する受け入れ基準:** AC-12
- **前提:** フィクスチャA。`task-execution.md` TC-25 手順1〜4（手順4 の notify.log と再通知は Issue #3、手順5 の `set-status` は Issue #5 のため範囲外）。
- **目的:** タスク属性は読めるがスナップショットが読めない縮退状態（`SnapshotUnreadable`）で、定義依存の判断（起動）をすべてスキップして報告し、書き込みも stopped 化も行わないことを確認する。
- **手順:**
  1. `pulsen add --workflow pipeline --repo /tmp/pulsen-test/repo; echo $?` → `export T25=<task-id>`
  2. `cp "$PULSEN_HOME/state/tasks/$T25.json" /tmp/pulsen-test/t25.bak`
  3. スナップショット部分だけを不正な構造にする（ファイル全体は JSON として妥当なまま保つ）:

      ```sh
      jq '.snapshot.statuses = "broken"' /tmp/pulsen-test/t25.bak > /tmp/pulsen-test/t25.new \
        && mv /tmp/pulsen-test/t25.new "$PULSEN_HOME/state/tasks/$T25.json"
      cat "$PULSEN_HOME/state/tasks/$T25.json"
      ```

  4. `md5 -q "$PULSEN_HOME/state/tasks/$T25.json" 2>/dev/null || md5sum "$PULSEN_HOME/state/tasks/$T25.json"`
  5. `pulsen tick; echo $?`
  6. `cat "$PULSEN_HOME/state/tasks/$T25.json"`（手順4 と同じチェックサムになること）
  7. `ls "$PULSEN_HOME/state/runs/$T25/" 2>/dev/null; echo $?; ls "$PULSEN_HOME/worktrees/" `
  8. `cat /tmp/pulsen-test/notify.log`
  9. 復元: `cp /tmp/pulsen-test/t25.bak "$PULSEN_HOME/state/tasks/$T25.json"`
- **期待結果:** 手順5 は exit code 0 で、サマリーの `errors` に T25 の**タスクID**とスナップショット読み取り不能によるスキップが報告される。手順6 でファイル内容が変わっていない。手順7 で `state/runs/<T25>/` も `worktrees/<T25>` も作られていない。手順8 で T25 に関する通知は無い。
- **確認ポイント:** 破損したタスクファイル（エッジケース3）と違い、**タスクIDが報告に出る**こと（属性は読めているため）。`stopped` にされていないこと — 縮退状態は利用者が直せる状態であり、ツールが凍結させてよい状態ではない。

### 5. ロック競合 — tick は 0 でスキップ、add は非0で拒否

- **対応する受け入れ基準:** AC-12
- **前提:** フィクスチャA。`task-execution.md` TC-24 の手順2・4・5 と手順6 の `pulsen tick; echo $?`。ロック保持は手順1・3（長時間の notify_cmd + `pulsen abort`）を使わず `examples/lock_holder` で作る — notify は Issue #3、`abort` は Issue #5 で、どちらも本スライスに無いため。
- **目的:** tick と状態変更系 CLI が同一の排他ロックを使い、競合した tick は状態を変更せず exit code **0** でスキップし（cron 運用でアラートにしないための唯一の例外）、競合した `add` は非0で終了してタスクを作らないことを確認する。
- **手順:**
  1. `pulsen add --workflow pipeline --repo /tmp/pulsen-test/repo; echo $?` → `export T24=<task-id>`
  2. 別端末（`PULSEN_HOME` を同様に設定）でロックを保持する。`locked` の出力がロック取得の合図。**標準入力を開いたまま**にする:

      ```sh
      /Users/hikaru/github.com/tuanemuy/pulsen/target/debug/examples/lock_holder /tmp/pulsen-test/home/state/lock
      ```

  3. 保持中の端末で `ls -l "$PULSEN_HOME/state/tasks/" > /tmp/pulsen-test/lock-before.txt`
  4. `pulsen tick; echo $?`
  5. `pulsen add --workflow pipeline --repo /tmp/pulsen-test/repo; echo $?`
  6. `ls -l "$PULSEN_HOME/state/tasks/" | diff /tmp/pulsen-test/lock-before.txt -; echo $?`
  7. 手順2 のプロセスの標準入力を閉じる（Ctrl-D）
  8. `pulsen tick; echo $?`
- **期待結果:** 手順4 は「別の操作が実行中のためスキップした」旨を表示して exit code **0**（待ちに入らず即座に返る）。手順5 は「別の操作が実行中」として非0。手順6 で差分なし（タスクファイルは増えても変わってもいない）。手順8 は解放後に通常どおり exit code 0 で処理が進み、T24 が起動される。
- **確認ポイント:** 手順4 の表示が「エラー」ではなく「スキップ」として読めること（cron のログに毎分エラーが出ないことが exit 0 規約の目的）。手順4・5 のどちらもブロックしないこと。手順2 のプロセスを `kill -9` で強制終了した場合でも手順8 が成功すること（保持プロセスの異常終了でロックが残らない）。

### 6. 滞留するエージェントを起動したまま次の tick を打つ（ロックFDの非継承）

- **対応する受け入れ基準:** AC-12
- **前提:** フィクスチャA。長時間実行するエージェントを持つ定義を1つ足す（`longrun.yaml` は `task-execution.md` のテストデータにあるが `timeout: none` を持ち Issue #3 向けなので、本スライス用に短い滞留を作る）。
- **目的:** デタッチ起動したラッパーが tick のロックFDを継承していないことを確認する。継承していると、エージェントが動いている間のすべての tick がロック競合でスキップされ、cron 運用が完全に停止する。
- **手順:**
  1. 定義を足す:

      ```sh
      cat > /tmp/pulsen-test/home/workflows/stay.yaml <<'EOF'
      workflow: stay
      agent: shell
      initial: work
      statuses:
        work:
          prompt: "sleep 60"
          next: done
        done:
          run: cleanup
      EOF
      ```

  2. `pulsen add --workflow stay --repo /tmp/pulsen-test/repo; echo $?` → `export T26=<task-id>`
  3. `pulsen tick; echo $?`（起動）
  4. `until [ -f "$PULSEN_HOME/state/runs/$T26/attempt-1/pid" ]; do sleep 1; done`
  5. `pulsen tick; echo $?`（エージェントが `sleep 60` の最中）
  6. `cat "$PULSEN_HOME/state/tasks/$T26.json"`
  7. 片付け: `until [ -f "$PULSEN_HOME/state/runs/$T26/attempt-1/exit" ]; do sleep 2; done; cat "$PULSEN_HOME/state/runs/$T26/attempt-1/exit"; echo`
- **期待結果:** 手順5 は exit code 0 で、**ロック競合のスキップにならず**通常のサマリーが表示される。手順6 で T26 が `running` に取り込まれている。手順7 で最終的に `{"code":0}` が書かれる。
- **確認ポイント:** 手順5 の出力に「別の操作が実行中」が現れないこと。ラッパーが `sleep 60` の間も生き続け（`ps` で確認できる）、それでも次の tick が走ること。片付けで `exit` の出現まで待つのは、一時領域の削除と孫プロセスの書き込みが競合しないようにするため。

### 7. 進行中の worktree の手動削除 — エージェント実行の失敗として既存経路に落ちる

- **対応する受け入れ基準:** AC-16（PAGE-tick-009）
- **前提:** フィクスチャA。`workspace` が**確定済み**のタスクの worktree を消す（確定済みなので `create` は呼ばれない）。
- **目的:** worktree が消えた状態での起動が tick 側の新しい分岐を作らず、ラッパーの `run_agent` が cwd 不在を符号化して `exit` に非0を書く経路に落ちることを確認する。
- **手順:**
  1. `pulsen add --workflow pipeline --repo /tmp/pulsen-test/repo; echo $?` → `export T27=<task-id>`
  2. `pulsen tick`（起動。`workspace` が確定し worktree が作られる）→ `until [ -f "$PULSEN_HOME/state/runs/$T27/attempt-1/exit" ]; do sleep 1; done`
  3. `jq '.execution = {"state":"failed"}' "$PULSEN_HOME/state/tasks/$T27.json" > /tmp/pulsen-test/t27.new && mv /tmp/pulsen-test/t27.new "$PULSEN_HOME/state/tasks/$T27.json"`
  4. `rm -rf "$PULSEN_HOME/worktrees/$T27"`
  5. `pulsen tick; echo $?`
  6. `cat "$PULSEN_HOME/state/tasks/$T27.json"`
  7. `until [ -f "$PULSEN_HOME/state/runs/$T27/attempt-2/exit" ]; do sleep 1; done; cat "$PULSEN_HOME/state/runs/$T27/attempt-2/exit"; echo; ls "$PULSEN_HOME/state/runs/$T27/attempt-2/"`
- **期待結果:** 手順5 は exit code 0 で通常どおり起動が記録される（サマリーに worktree 作成に関する失敗は出ない）。手順6 で `execution` が `launching`、`current_attempt.number` が 2。手順7 で `attempt-2/exit` が **非0**（cwd に到達できないため 126）で、`stdout.log` は空。
- **確認ポイント:** tick 側に新しい分岐が生じないこと — この失敗は帳簿上「エージェント実行が非0で終わった」ことと区別できず、判定（Issue #3）が既存の failed 経路で扱う。`exit` が書かれること自体が重要（書かれないと Issue #3 が「exit なし・プロセス死亡」という別経路で扱うことになる）。

### 8. ブランチのみ残存した状態からの worktree 張り直し

- **対応する受け入れ基準:** AC-15（ADR-077）
- **前提:** フィクスチャA。`workspace` が**未確定**のタスクに対して、リポジトリには `pulsen/<task-id>` ブランチだけがコミットを積んだ状態で存在する。利用者の `git worktree remove` / git の自動 prune / Issue #6 の終端処理の後に実際に生じる状態。
- **目的:** 登録が無くブランチだけが残っている状態から、`worktree add`（`-f` なし）で**ブランチ先端を変えずに**張り直され、積まれたコミットの成果物が worktree に戻ることを確認する。
- **手順:**
  1. `pulsen add --workflow pipeline --repo /tmp/pulsen-test/repo; echo $?` → `export T28=<task-id>`
  2. ブランチだけを作り、コミットを積む:

      ```sh
      git -C /tmp/pulsen-test/repo branch "pulsen/$T28" main
      git -C /tmp/pulsen-test/repo worktree add /tmp/pulsen-test/tmpwt "pulsen/$T28"
      echo carried > /tmp/pulsen-test/tmpwt/carried.txt
      git -C /tmp/pulsen-test/tmpwt add carried.txt
      git -C /tmp/pulsen-test/tmpwt commit -m carried
      git -C /tmp/pulsen-test/repo worktree remove /tmp/pulsen-test/tmpwt
      git -C /tmp/pulsen-test/repo worktree list          # 当該ブランチの登録が無いこと
      git -C /tmp/pulsen-test/repo rev-parse "pulsen/$T28" > /tmp/pulsen-test/tip-before.txt
      ```

  3. `pulsen tick; echo $?`
  4. `cat "$PULSEN_HOME/state/tasks/$T28.json"`
  5. `ls "$PULSEN_HOME/worktrees/$T28/"; git -C /tmp/pulsen-test/repo rev-parse "pulsen/$T28" | diff /tmp/pulsen-test/tip-before.txt -; echo $?`
- **期待結果:** 手順3 は exit code 0 で起動が記録される。手順4 で `workspace` が `worktrees/<T28>` / `pulsen/<T28>` に確定し `launching` になる。手順5 で worktree に `carried.txt` が存在し、ブランチ先端が手順2 の値から**変わっていない**（`diff` が差分なし）。
- **確認ポイント:** 起動が `record_tool_failure(WorktreeCreate)` に落ちないこと — ここで失敗すると、利用者が worktree を消しただけで毎 tick リトライを消費し上限超過で凍結する。先端が変わらないこと（`-f` で作り直したり `add -b` で作り直したりすると、積まれた成果物が失われる）。

### 9. ラッパーの起動引数が不正な場合

- **対応する受け入れ基準:** AC-11
- **前提:** 確認項目7 のフィクスチャ（`$WRUN` が既に使われているので別のパスを使う）。
- **目的:** 起動引数の検証に失敗したラッパーが runディレクトリに**何も書かずに**非0で終わることを確認する。何かを書くと、猶予経路（手続きC）が「pid はあるが実体が無い」という帳簿と矛盾した状態を掴む。
- **手順:**
  1. 相対パス:

      ```sh
      export WRUN3=/tmp/pulsen-test/home/state/runs/20260812t000000-cccccccc/attempt-1
      mkdir -p "$WRUN3"
      cd /tmp && pulsen wrapper --run-dir ./relative --workspace /tmp/pulsen-test/wrapws -- sh -c true; echo $?
      pulsen wrapper --run-dir "$WRUN3" --workspace ./relative -- sh -c true; echo $?
      ```

  2. トークン0個: `pulsen wrapper --run-dir "$WRUN3" --workspace /tmp/pulsen-test/wrapws --; echo $?`
  3. 形式外の run_dir: `mkdir -p /tmp/pulsen-test/notrun && pulsen wrapper --run-dir /tmp/pulsen-test/notrun --workspace /tmp/pulsen-test/wrapws -- sh -c true; echo $?`
  4. `ls -la "$WRUN3" /tmp/pulsen-test/notrun`
  5. `cd /Users/hikaru/github.com/tuanemuy/pulsen`
- **期待結果:** 手順1〜3 のすべてが非0で終了する。手順4 で `$WRUN3` も `/tmp/pulsen-test/notrun` も**空のまま**（`starttime` / `pid` / `exit` / ログのいずれも作られていない）。
- **確認ポイント:** 手順3 の run_dir は実在するディレクトリだが `<state_root>/runs/<task-id>/attempt-<n>` の形をしていないため拒否されること（ADR-070 の逆写像が `None` を返す経路）。エラーが標準エラーに出て、標準出力が汚れないこと。

## 既存機能への影響確認

- **`pulsen add` の経路が壊れていないこと:** 本スライスは `wire::Runtime` にアクセサとポートを追加し（steps.md ステップ17）、`WireError` に1変種を足す（ADR-068）。`add` は `ProcessController` を必要としないので、`current_exe()` の失敗で `add` が落ちないことを確認する。`.thread/1/testing.md` の確認項目1・2・4（未初期化ホームの案内 / ホーム解決の優先順位 / 登録成功時の表示とタスクファイルの中身）をスポットチェックとして再実行し、Issue #1 時点と同じ結果になること。

- **`pulsen --help` の表示:** サブコマンドとして `add` / `tick` / `help` が並び、`wrapper` が現れないこと（ADR-069）。`pulsen tick --help` に引数が無いこと（`--home` は global フラグとして現れる）。引数の使い方の誤りが clap 既定の exit code 2 になること。

- **AC-1 の grep 期待値の更新:** Issue #1 の testing.md は `crates/pulsen/src/` 側のヒットを `util/atomic.rs` だけとしていた。本スライスで `adapter/process.rs` が加わるため、期待値は2ファイルになる（ADR-067 Consequences）。`crates/pulsen-domain/` が0件であることは変わらない。この期待値の変更は Issue #1 の testing.md 側には反映しない（各 Issue の testing.md はその時点の期待を書く）。

- **`state/` のレイアウトへの追加:** 本スライスで `state/runs/` と `worktrees/` が初めて作られる。Issue #1 の「`add` は `worktrees/` も `state/runs/` も作らない」という期待（`.thread/1/testing.md` 確認項目4 手順5）が引き続き成り立つこと — 作るのは tick だけであること。

- **実運用ホームの非汚染:** 全項目の実行後に `ls -a "$HOME/.pulsen" 2>/dev/null` が実行前と変わらないこと。フィクスチャA は `/tmp/pulsen-test/`、フィクスチャB は `$HOME/pulsen-manual-test` に閉じている。cron を使った確認項目8 では `--home` を絶対パスで指定しており、`~/.pulsen` を解決しないこと。

- **後片付け:**

    ```sh
    crontab -l | grep -v 'pulsen tick' | crontab -   # 確認項目8 の行が残っていないこと
    ps -ef | grep -i 'pulsen wrapper\|lock_holder' | grep -v grep   # 残留プロセスが無いこと
    git -C /tmp/pulsen-test/repo worktree list
    rm -rf /tmp/pulsen-test "$HOME/pulsen-manual-test" "$HOME/pulsen-manual-work" "$HOME/pulsen-test-repo"
    ```

  worktree を作ったリポジトリごと削除するので `git worktree prune` は不要。残留プロセスの確認は、デタッチ起動したラッパーが一時領域の削除後も書き込みを続けないことの確認を兼ねる。

- **落とした手順の記帳:** 本スライスに無いコマンド（`ls` / `show` / `abort` / `retry` / `set-status`）と、notify（Issue #3）・判定/遷移（Issue #3）・終端処理（Issue #6）を要する手順は実行していない。今回カバーしたのは `spec/manual-tests/setup.md` TC-02・TC-06(手順1〜3・4/5の起動フェーズ・7)・TC-34(手順1〜4)と、`spec/manual-tests/task-execution.md` TC-03(手順1〜4・7・12)・TC-04(手順1〜3・5)・TC-12・TC-16(手順1〜5・7)・TC-20(手順1〜4)・TC-24(手順2・4・5・6の一部)・TC-25(手順1〜4)。**落とした手順は Issue のチェックリストにチェックを付けず、見送った旨と理由を Issue #2 のコメントに残す**（Issue 完了条件 / steps.md ステップ19）。
