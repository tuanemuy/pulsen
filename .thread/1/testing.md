# 動作確認計画 — Issue #1: [skeleton] 基盤・グローバル設定・ワークフロー定義とタスク登録(add)

**Issue:** #1
**作成日:** 2026-08-12

---

## 確認環境

このIssueの変更を確認するために必要な手順のみ記載（プロジェクト全体のセットアップは省略）。

本スライスに存在するコマンドは `pulsen add` のみ。`ls` / `tick` / `show` は未実装のため、状態の確認はすべて `state/tasks/` の直接確認で行う（plan.md「テスト方針 — 手動確認」の読み替え a / b に従う）。Web UI は無く、確認はすべてターミナル上の CLI 実行になる。

### 検証環境の起動

`.envrc` は `use flake` のみ。direnv がこのリポジトリで許可済みであれば、リポジトリに `cd` するだけで `flake.nix` の devShell（cargo / rustc / clippy / rustfmt）が入る。direnv を使わない場合は各コマンドを `nix develop -c <command>` で包む。

```sh
cd /Users/hikaru/github.com/tuanemuy/pulsen
cargo --version    # devShell が効いていることの確認
git --version      # add の対象検証で使う（steps.md ステップ1 で devShell にも追加される）
```

ビルドとバイナリの解決。ワークスペースの target ディレクトリはリポジトリ直下（`.gitignore` の `/target`）で、bin 名は `pulsen`（steps.md ステップ1: `crates/pulsen` に bin `pulsen` = `src/main.rs`）。

```sh
cargo build
export PATH="$PWD/target/debug:$PATH"
pulsen --help
```

以降の手順はすべてこの PATH が通ったシェルから実行する。exit code は各コマンドの直後に `echo $?` で確認する。

自動テスト側の状態（本確認の前提となるグリーン状態）:

```sh
cargo test
cargo clippy -- -D warnings
cargo fmt --check
```

AC-1 の「OS 依存分岐がアダプター層に隔離されている」ことの機械的確認:

```sh
grep -rn 'cfg(unix)\|cfg(windows)' crates/*/src/
```

`crates/pulsen-domain/` が1件もヒットせず、`crates/pulsen/src/` 側のヒットが `util/atomic.rs`（ディレクトリエントリの fsync）だけであること。`crates/pulsen-conformance/src/lib.rs` のヒットは適合ハーネスが権限制限の効き目を probe する分岐で、本番の実行経路には乗らない。

`crates/` 全体を対象にすると `crates/pulsen/tests/` 配下もヒットするが、これらは適合スイートに権限操作フックを供給するテスト側の分岐で、アダプター層の隔離とは別の話。

### 検証用のフィクスチャ準備

`spec/manual-tests/setup.md` の事前準備を、本スライスで実行できる範囲に絞って再構成したもの。実運用の `~/.pulsen/` を汚さないため、分離ホームを `PULSEN_HOME` で切り替える。

1. 環境変数を設定する。

    ```sh
    export PULSEN_HOME="$HOME/pulsen-manual-test"
    export WORK="$HOME/pulsen-manual-work"
    export REPO="$HOME/pulsen-test-repo"
    ```

2. 分離ホーム・作業領域・対象リポジトリを作り直す（グローバル git 設定のない環境でも初期コミットが成功するよう identity をリポジトリローカルに設定する）。

    ```sh
    rm -rf "$PULSEN_HOME" "$WORK" "$REPO" \
           "$HOME/pulsen-empty-home" "$HOME/pulsen-default-home"
    mkdir -p "$PULSEN_HOME/workflows" "$WORK"
    git init -b main "$REPO"
    git -C "$REPO" config user.name pulsen-test
    git -C "$REPO" config user.email pulsen-test@example.com
    git -C "$REPO" commit --allow-empty -m init
    git -C "$REPO" branch develop
    ```

3. グローバル設定を作る（setup.md TC-01 と同内容）。

    ```sh
    cat > "$PULSEN_HOME/config.yaml" <<'EOF'
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
    cp "$PULSEN_HOME/config.yaml" "$WORK/config.bak"
    ```

4. ワークフロー定義を配置する（setup.md TC-03 と同内容）。

    ```sh
    cat > "$PULSEN_HOME/workflows/implement.yaml" <<'EOF'
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

5. パス指定用のドラフト定義を2つ作る（`workflow:` キーあり / なし）。

    ```sh
    cat > "$WORK/draft.yaml" <<'EOF'
    workflow: my-flow
    agent: shell
    initial: greet
    statuses:
      greet:
        prompt: "echo hello from draft"
        next: waiting
      waiting:
        run: wait
      done:
        run: cleanup
    EOF

    sed 's/^workflow: my-flow$//' "$WORK/draft.yaml" > "$WORK/custom.yaml"
    ```

6. HEAD 解決不能なリポジトリを2つ作る（setup.md TC-33）。

    ```sh
    git clone "$REPO" "$WORK/detached-repo"
    git -C "$WORK/detached-repo" checkout --detach
    git init -b main "$WORK/empty-repo"
    ```

7. 確認の共通ヘルパー（タスクファイルの一覧と中身）。`state/tasks/` は登録が1件も成功していない時点では存在しない。

    ```sh
    ls "$PULSEN_HOME/state/tasks/" 2>/dev/null
    cat "$PULSEN_HOME/state/tasks/<task-id>.json"
    ```

    JSON の項目を絞って見たい場合は `jq` を使ってよい（`/usr/bin/jq` を確認済み。devShell には含まれないためシステムの `jq` を使う）。

各確認項目は上記フィクスチャ直後の状態から開始できるように書いてある（先行項目の残す状態に依存しない）。タスクは登録されるだけで実行されないため（`tick` が無い）、成功した登録の片付けは不要。必要なら `rm -f "$PULSEN_HOME/state/tasks/"*.json` で戻す。

### デプロイ方法

なし（ローカル CLI のみで確認できる）。

## 確認項目

### 1. 未初期化ホームの案内文言

- **対応する受け入れ基準:** AC-13
- **目的:** config.yaml が無いホームに対して、「未初期化である旨」「解決後のホームパス」「config.yaml の作成が必要であること」の3点が揃った案内が出て非0で終了し、タスクが作られないことを確認する（setup.md TC-12 手順2 の読み替え）。
- **手順:**
  1. `pulsen add --workflow implement --repo "$REPO" --home "$HOME/pulsen-empty-home"; echo $?`
  2. `ls "$HOME/pulsen-empty-home" 2>/dev/null; echo $?`
- **期待結果:** 手順1 は exit code 非0（1）。stderr に「グローバルホームが未初期化である旨」「`$HOME/pulsen-empty-home`（解決後の絶対パス）」「config.yaml を作成する必要があること」の3点が含まれる。手順2 でホームディレクトリ自体が作られていない（`state/` も `state/lock` も無い。plan.md「ロック取得より前に config 読み込みがある」）。
- **確認ポイント:** メッセージだけを読んで「どのパスに何のファイルを置けばよいか」が分かること。パスが `~` のまま表示されたり、相対パスで表示されたりしないこと（cron などホーム解決を間違えやすい環境で誤解決に気づけるかがこの表示の目的）。

### 2. グローバルホーム解決の優先順位

- **対応する受け入れ基準:** AC-13, AC-16
- **目的:** `--home` > `PULSEN_HOME` > `~/.pulsen/` の優先順位が実際に効いていることを、エラーメッセージ中の解決後パスで確認する（setup.md TC-48、register-task TC-067 の読み替え）。
- **手順:**
  1. `PULSEN_HOME=/no/such/home pulsen add --workflow implement --repo "$REPO" --home "$PULSEN_HOME"; echo $?`
  2. `PULSEN_HOME=/no/such/home pulsen add --workflow implement --repo "$REPO"; echo $?`
  3. `FAKEHOME=$(mktemp -d); env -u PULSEN_HOME HOME="$FAKEHOME" pulsen add --workflow implement --repo "$REPO"; echo $?`
- **期待結果:** 手順1 はフラグのホームが優先され、登録が成功して exit code 0。手順2 は環境変数のホームが使われ、未初期化エラーに `/no/such/home` が含まれて非0。手順3 は既定の `~/.pulsen/`（= `$FAKEHOME/.pulsen`）が解決され、そこに config.yaml が無いので未初期化エラーにそのパスが表示されて非0。
- **確認ポイント:** 手順3 の案内に出るパスが `$FAKEHOME/.pulsen` であること（既定ホームの解決が `HOME` を見ていることの証拠になる）。手順2 のメッセージで「意図しないホームを見ている」ことが読み取れること。手順3 が `HOME` を一時ディレクトリに差し替えるのは、確認したいのが「既定へ落ちる経路が使うホームの解決結果」であって実運用の `~/.pulsen/` そのものではないため。

### 3. 空の config.yaml が全デフォルトで受理される

- **対応する受け入れ基準:** AC-9, AC-13
- **目的:** キーを1つも持たない config.yaml が「全キー省略」として受理され、デフォルト値で動作することを確認する（setup.md TC-42 の読み替え。AC-9 の全デフォルト受理の唯一の手動裏付け）。
- **手順:**
  1. `mkdir -p "$HOME/pulsen-default-home/workflows" && touch "$HOME/pulsen-default-home/config.yaml"`
  2. `cp "$PULSEN_HOME/workflows/implement.yaml" "$HOME/pulsen-default-home/workflows/"`
  3. `pulsen add --workflow implement --repo "$REPO" --home "$HOME/pulsen-default-home"; echo $?`
- **期待結果:** 手順3 は config のパースエラーにならない。`agents` が空のため `implement` が参照する `shell` が未定義となり、**登録時検証の `UnknownAgent` エラー**で非0終了する。定義済みエージェント名の一覧は空である旨が示される。
- **確認ポイント:** エラーが「config が読めない/壊れている」ではなく「エージェント `shell` が config.yaml に定義されていない」として出ること（空 config が受理された上で参照時検証に進んだ証拠になる）。空一覧の表示が「（なし）」等として読めること。

### 4. 名前指定の登録成功 — 出力とタスクファイルの中身

- **対応する受け入れ基準:** AC-14, AC-17
- **目的:** 成功時の表示3点（タスクID・解決したワークフロー名・解決先パス）と、登録直後のタスクファイルの中身（初期状態とスナップショット埋め込み）を確認する（setup.md TC-03 手順1〜2 + 手順3・4の読み替え a、task-execution.md TC-01 手順3・6・7、setup.md TC-08 手順1〜2）。
- **手順:**
  1. `pulsen add --workflow implement --repo "$REPO"; echo $?`
  2. 表示されたタスクIDを控えて `export T1=<task-id>`
  3. `ls "$PULSEN_HOME/state/tasks/"`
  4. `cat "$PULSEN_HOME/state/tasks/$T1.json"`
  5. `ls "$PULSEN_HOME/worktrees/" "$PULSEN_HOME/state/runs/" 2>/dev/null; echo $?`
  6. `rm "$PULSEN_HOME/workflows/implement.yaml" && cat "$PULSEN_HOME/state/tasks/$T1.json"`
  7. 片付け: 手順6 で削除した定義を戻す（フィクスチャ準備の手順4を再実行する）
- **期待結果:**
  - 手順1: exit code 0。stdout にタスクID・ワークフロー名 `implement`・解決先の**絶対パス** `$PULSEN_HOME/workflows/implement.yaml` が表示される。
  - 手順3: `<task-id>.json` が1件ある。ID は `<yyyymmdd>t<hhmmss>-<base36 8桁>` の24文字形式。
  - 手順4: 人間可読な（インデントされた）JSON で、`task_status` が `queued`（= snapshot の `initial`）、`execution` が `{"state":"pending"}`、`counters` が全0、`workspace` / `current_attempt` / `last_failure` が `null`、`target` が `{"repo": <$REPO の絶対パス>, "base_branch": "main"}`、`updated_at` が `YYYY-MM-DDTHH:MM:SSZ` 形式、`snapshot.statuses` に `queued` / `implemented` / `review_waiting` / `done` の4件が含まれる。
  - 手順5: `worktrees/` も `state/runs/` も存在しない（add は実行を開始しない）。
  - 手順6: 元ファイルを削除してもタスクファイルの `snapshot` はそのまま残っている。
- **確認ポイント:** JSON が1行に潰れておらず、人間が直接読んで状態を判断できること（`ls` / `show` が無い本スライスでは、これが唯一の状態確認手段になる）。`repo` が指定した値ではなく絶対パスに正規化されていること。`updated_at` が UTC・秒精度であること。

### 5. 二重登録の独立性と `state/` の自動作成

- **対応する受け入れ基準:** AC-16, AC-17
- **目的:** 重複排除が行われず独立した別タスクになること、および `state/` 配下が自動作成されることを確認する（setup.md TC-04 の読み替え a、register-task TC-060）。
- **手順:**
  1. `rm -rf "$PULSEN_HOME/state"`
  2. `pulsen add --workflow implement --repo "$REPO"; echo $?`
  3. `pulsen add --workflow implement --repo "$REPO"; echo $?`
  4. `ls "$PULSEN_HOME/state/tasks/"`
  5. `ls -a "$PULSEN_HOME/state/"`
- **期待結果:** 手順2・3 とも exit code 0 で、**異なるタスクID**が表示される。手順4 でタスクファイルが2件ある。手順5 で `tasks/` と `lock` が自動作成されている（`archive/` `runs/` は書き込みが発生するまで作られていなくてよい）。
- **確認ポイント:** 「同じワークフロー・同じリポジトリで既に登録済み」といった重複警告が出ないこと（重複排除しないのが仕様）。ロックファイル `state/lock` がタスク走査に混ざらない名前（`<task-id>.json` 形式でない）であること。

### 6. `--workflow` の解釈規則と表示名の決定

- **対応する受け入れ基準:** AC-3, AC-16
- **目的:** 名前 / パスの解釈がファイルの存在に依存しない決定的規則で決まること、`.yml` へのフォールバックが無いこと、表示名が「名前指定 = その名前 / パス指定 = `workflow:` キー、なければ拡張子を除いたファイル名」で決まることを確認する（setup.md TC-07 手順1〜2・TC-40・TC-41、task-execution.md TC-02 手順1、register-task TC-049〜052）。
- **手順:**
  1. `cd "$WORK" && pulsen add --workflow ./draft.yaml --repo "$REPO"; echo $?`
  2. `pulsen add --workflow "$WORK/custom.yaml" --repo "$REPO"; echo $?`
  3. `cp "$WORK/draft.yaml" "$WORK/nameclash.yaml" && cd "$WORK" && pulsen add --workflow nameclash --repo "$REPO"; echo $?`
  4. `cp "$PULSEN_HOME/workflows/implement.yaml" "$PULSEN_HOME/workflows/impl2.yml" && pulsen add --workflow impl2 --repo "$REPO"; echo $?`
  5. `pulsen add --workflow "$PULSEN_HOME/workflows/impl2.yml" --repo "$REPO"; echo $?`
  6. 各成功ケースのタスクIDについて `cat "$PULSEN_HOME/state/tasks/<task-id>.json"` で `workflow_name` を確認する
- **期待結果:**
  - 手順1: exit code 0。ワークフロー名は `my-flow`（ファイル名 `draft` ではなく `workflow:` キーの値）。解決先は `$WORK/draft.yaml` の絶対パス。
  - 手順2: exit code 0。ワークフロー名は `custom`（`workflow:` キーが無いためファイル名由来）。
  - 手順3: 区切り文字も拡張子も含まないため常に名前として解釈され、`$PULSEN_HOME/workflows/nameclash.yaml` の解決失敗で非0。**カレントディレクトリの `nameclash.yaml` は使われない**。
  - 手順4: `.yml` しか無いため `workflows/impl2.yaml` の解決失敗で非0（フォールバックしない）。
  - 手順5: `.yml` で終わる指定はパスとして解釈され、exit code 0。
- **確認ポイント:** 手順3・4 のエラーが「解決を試みた絶対パス」を含んでいて、配置漏れ・拡張子違いに気づけること。手順1 で `cd` したディレクトリからの相対パスが正しく絶対化されて表示されること。

### 7. `--base` の解決（HEAD 由来と明示指定）

- **対応する受け入れ基準:** AC-14
- **目的:** `--base` 省略時に HEAD の指すブランチへ解決されること、明示指定時はそのブランチが記録されることを確認する（setup.md TC-03 確認ポイント・TC-32 手順2、register-task TC-006/007）。
- **手順:**
  1. `pulsen add --workflow implement --repo "$REPO"; echo $?`（省略）
  2. `pulsen add --workflow implement --repo "$REPO" --base develop; echo $?`
  3. `cd "$HOME" && pulsen add --workflow implement --repo ./pulsen-test-repo; echo $?`（相対パス）
  4. 各タスクファイルの `target` を確認する
- **期待結果:** 手順1 の `base_branch` が `main`、手順2 が `develop`。手順3 は exit code 0 で `repo` が `$HOME/pulsen-test-repo` の**絶対パス**に正規化されている。
- **確認ポイント:** 解決されたベースブランチが成功時の出力またはタスクファイルから確認でき、「どのブランチから worktree が作られるのか」が登録時点で確定していること。

### 8. 受理される定義の広さ（循環・到達不能・終端なし・境界値）

- **対応する受け入れ基準:** AC-16
- **目的:** ADR-010 が許容する定義（自己参照循環・到達不能ステータス・クリーンアップ終端なし）と境界値（`retries: 0` / `timeout: none` / statuses 1件）が拒否されないことを確認する（setup.md TC-43 手順1・TC-44・TC-45）。
- **手順:**
  1. 以下を作成して順に add する。

      ```sh
      cat > "$WORK/polling.yaml" <<'EOF'
      workflow: polling
      agent: shell
      initial: check
      statuses:
        check:
          prompt: "echo polled"
          next: check
        done:
          run: cleanup
      EOF

      cat > "$WORK/polling-noend.yaml" <<'EOF'
      workflow: polling-noend
      agent: shell
      initial: check
      statuses:
        check:
          prompt: "echo polled"
          next: check
      EOF

      cat > "$WORK/boundary.yaml" <<'EOF'
      workflow: boundary
      agent: shell
      initial: start
      statuses:
        start:
          prompt: "echo hi"
          retries: 0
          timeout: none
          next: start
      EOF

      for f in polling polling-noend boundary; do
        pulsen add --workflow "$WORK/$f.yaml" --repo "$REPO"; echo "$f -> $?"
      done
      ```
- **期待結果:** 3件とも exit code 0 で登録が成功する。`boundary.yaml` は statuses 1件・自己参照・`retries: 0`・`timeout: none` を同時に含むが受理される。
- **確認ポイント:** 「到達不能」「循環」「終端なし」を警告として出さないこと（ポーリング型ワークフローの正当な表現であり、警告はノイズになる）。タスクファイルの `snapshot` に `retries: 0` と `timeout: none` に相当する値が失われずに残っていること。

## エッジケース・異常系

すべてのケースで **exit code が非0であること** と **`state/tasks/` にタスクが増えていないこと**（`ls "$PULSEN_HOME/state/tasks/" | wc -l` を実行前後で比較）を共通の確認手段とする（AC-15）。

### 1. config.yaml のパース不能（構文エラー・未知キー）

- **対応する受け入れ基準:** AC-13, AC-15
- **目的:** 壊れた設定で部分的に動作せず、エラー位置と未知キーが具体的に示されることを確認する（setup.md TC-13 手順1・2・4・5、TC-14 の読み替え b）。
- **手順:**
  1. `printf 'agents:\n  shell:\n    cmd: ["sh"\n' > "$PULSEN_HOME/config.yaml"` で角括弧を閉じ忘れる（インデント崩しは別のトップレベルキーになるだけで構文としては妥当なので、構文エラーにならない）
  2. `pulsen add --workflow implement --repo "$REPO"; echo $?`
  3. `cp "$WORK/config.bak" "$PULSEN_HOME/config.yaml" && printf 'run_retension: 30d\n' >> "$PULSEN_HOME/config.yaml"`（typo キー）
  4. `pulsen add --workflow implement --repo "$REPO"; echo $?`
  5. 回復: `cp "$WORK/config.bak" "$PULSEN_HOME/config.yaml" && pulsen add --workflow implement --repo "$REPO"; echo $?`
- **期待結果:** 手順2 は**パースエラーの位置（行・列）**を添えて非0。手順4 は未知キー `run_retension` を名指しして非0。手順5 は exit code 0 に復帰する。どちらの失敗でも `state/tasks/` は変化せず、config.yaml も書き換わらない。
- **確認ポイント:** 手順4 のメッセージから「typo が黙殺されず、どのキーが未知なのか」が分かること（正しいキー名の候補提示までは要求しない）。手順2 で行番号が実際の崩れた行を指していること。

### 2. ワークフロー定義の解決失敗

- **対応する受け入れ基準:** AC-15
- **目的:** 名前解決・パス解決の失敗時に、解決を試みた絶対パスが示されて配置漏れに気づけることを確認する（setup.md TC-29 手順1・TC-30、task-execution.md TC-08・TC-09 手順2）。
- **手順:**
  1. `pulsen add --workflow nosuchflow --repo "$REPO"; echo $?`
  2. `cd "$HOME" && pulsen add --workflow ./nosuch.yaml --repo "$REPO"; echo $?`
  3. `chmod 000 "$PULSEN_HOME/workflows/implement.yaml" && pulsen add --workflow implement --repo "$REPO"; echo $?; chmod 644 "$PULSEN_HOME/workflows/implement.yaml"`（POSIX のみ。root では手順3 をスキップする）
- **期待結果:** 手順1 は `$PULSEN_HOME/workflows/nosuchflow.yaml` の絶対パスを添えて非0。手順2 は `$HOME/nosuch.yaml`（カレントディレクトリからの解決結果）を添えて非0。手順3 は読み取り不能を実行環境エラーとして非0。
- **確認ポイント:** 手順1 と手順2 で「不在」のメッセージが同じ形（試みたパスを添える）に揃っていること。手順3 のメッセージが「不在」ではなく「読めない」として区別されていること。

### 3. ワークフローYAMLの構造エラー

- **対応する受け入れ基準:** AC-4, AC-15
- **目的:** 厳格スキーマ（ADR-013）の各エラー種が読み込み時に検出され、原因が特定できる形で表示されることを確認する（setup.md TC-21〜TC-27、task-execution.md TC-09 手順1）。
- **手順:** 以下を作成して順に add し、それぞれの exit code とメッセージを記録する。

    ```sh
    printf 'agent: shell\nstatuses:\n  start:\n    prompt: "hi"\n    next: waiting\n  waiting:\n    run: wait\n' > "$WORK/e-no-initial.yaml"       # MissingInitial
    printf 'agent: shell\ninitial: start\n' > "$WORK/e-no-statuses.yaml"                                                                     # EmptyStatuses
    printf 'agent: shell\ninitial: nosuch\nstatuses:\n  start:\n    prompt: "hi"\n    next: start\n' > "$WORK/e-bad-initial.yaml"            # InitialNotFound
    printf 'agent: shell\ninitial: start\nstatuses:\n  start:\n    prompt: "hi"\n    next: nosuch\n' > "$WORK/e-bad-next.yaml"               # NextNotFound
    printf 'agent: shell\ninitial: start\nstatuses:\n  start:\n    prompt: "hi"\n' > "$WORK/e-no-next.yaml"                                  # MissingNext
    printf 'agent: shell\ninitial: start\nstatuses:\n  start:\n    next: start\n' > "$WORK/e-no-action.yaml"                                 # NoAction
    printf 'agent: shell\ninitial: start\nstatuses:\n  start:\n    prompt: "hi"\n    skill: plan\n    next: start\n' > "$WORK/e-two.yaml"    # MultipleActions
    printf 'agent: shell\ninitial: start\nstatuses:\n  start:\n    run: clenaup\n' > "$WORK/e-bad-run.yaml"                                  # UnknownRunValue
    printf 'agent: shell\ninitial: start\nstatuses:\n  start:\n    prompt: "hi"\n    next: waiting\n  waiting:\n    run: wait\n    judge: ["sh", "-c", "true"]\n' > "$WORK/e-forbidden.yaml"  # ForbiddenKey
    printf 'agent: shell\ninitial: start\nstatuses:\n  start:\n    prmopt: "hi"\n    next: start\n' > "$WORK/e-unknown-key.yaml"             # UnknownKey
    printf 'agent: shell\ninitial: start\nstatuses:\n  start:\n    prompt: "hi\n' > "$WORK/e-syntax.yaml"                                    # YamlSyntax(引用符の閉じ忘れ)
    printf 'agent: shell\ninitial: start\nstatuses:\n  start:\n    prompt: "hi"\n    next: waiting\n  start:\n    run: wait\n  waiting:\n    run: wait\n' > "$WORK/e-dup.yaml"  # YamlSyntax(重複キー)
    printf 'agent: shell\ninitial: start\nstatuses:\n  start:\n    prompt: "hi"\n    timeout: 0s\n    next: start\n' > "$WORK/e-timeout0.yaml"  # InvalidValue

    for f in "$WORK"/e-*.yaml; do
      out=$(pulsen add --workflow "$f" --repo "$REPO" 2>&1); code=$?
      printf '=== %s -> %s\n%s\n' "$f" "$code" "$out"
    done
    ```
- **期待結果:** 13件すべてが非0で終了し、タスクが1件も作られない。`e-syntax.yaml` / `e-dup.yaml` は構文エラーとして**位置（行）**が示される。`e-unknown-key.yaml` は未知キー `prmopt` を名指しする。`e-forbidden.yaml` は `run: wait` に併記できないキー（`judge`）を名指しする。
- **確認ポイント:** メッセージだけを見て「YAML のどの行の何を直せばよいか」が分かること。エラー種ごとの文言が使い回しの汎用文（「定義が不正です」だけ）になっていないこと。実行後に `$WORK/e-*.yaml` と `$PULSEN_HOME/config.yaml` の mtime が変わっていないこと（読めないリソースには書き込まない。PAGE-common-006）。

### 4. 登録時検証エラー（参照時検証の文言と全件列挙）

- **対応する受け入れ基準:** AC-5, AC-15
- **目的:** config.yaml のエージェント定義との突き合わせで生じるエラーが、修復に必要な情報（定義済みエージェント一覧）を添えて、かつ**全件まとめて**表示されることを確認する（setup.md TC-16〜TC-20・TC-28）。
- **手順:**
  1. 未定義エージェント: `printf 'agent: cladue\ninitial: start\nstatuses:\n  start:\n    prompt: "hi"\n    next: start\n' > "$WORK/v-agent.yaml" && pulsen add --workflow "$WORK/v-agent.yaml" --repo "$REPO"; echo $?`
  2. エージェント指定なし: `printf 'initial: start\nstatuses:\n  start:\n    prompt: "hi"\n    next: start\n' > "$WORK/v-noagent.yaml" && pulsen add --workflow "$WORK/v-noagent.yaml" --repo "$REPO"; echo $?`
  3. `{model}` 未供給: `printf 'agent: claude\ninitial: start\nstatuses:\n  start:\n    prompt: "hi"\n    next: start\n' > "$WORK/v-model.yaml" && pulsen add --workflow "$WORK/v-model.yaml" --repo "$REPO"; echo $?`
  4. `skill_input` 欠落: config.yaml の `shell` から `skill_input:` 行を削除し、`printf 'agent: shell\ninitial: start\nstatuses:\n  start:\n    skill: plan\n    next: start\n' > "$WORK/v-skill.yaml" && pulsen add --workflow "$WORK/v-skill.yaml" --repo "$REPO"; echo $?`。確認後 `cp "$WORK/config.bak" "$PULSEN_HOME/config.yaml"` で戻す
  5. テンプレート不備: config.yaml の `shell` の `cmd` を `["sh", "-c", "{inptu}"]` に書き換えて `pulsen add --workflow implement --repo "$REPO"; echo $?`。確認後 `cp "$WORK/config.bak" "$PULSEN_HOME/config.yaml"` で戻す
  6. 全件列挙: `printf 'agent: nosuch\ninitial: a\nstatuses:\n  a:\n    prompt: "hi"\n    next: b\n  b:\n    agent: nosuch2\n    prompt: "hi"\n    next: a\n' > "$WORK/v-multi.yaml" && pulsen add --workflow "$WORK/v-multi.yaml" --repo "$REPO"; echo $?`
  7. 参照されない壊れた定義の許容: config.yaml の `agents` に `broken: { cmd: "echo {nosuch}" }` を追記し、`pulsen add --workflow implement --repo "$REPO"; echo $?`。確認後に戻す
- **期待結果:** 手順1 は `cladue` が未定義である旨と **config.yaml に定義済みのエージェント名一覧（`shell` / `claude`）**を添えて非0。手順2〜5 はそれぞれ「エージェント指定の欠落」「model の値が供給できない」「`skill_input` の欠落」「未知プレースホルダ `{inptu}`」として非0。手順6 は `nosuch` と `nosuch2` の**両方**が1回の実行で列挙されて非0。手順7 は参照されない `broken` が検証されず exit code 0。
- **確認ポイント:** 手順6 で最初の1件で打ち切られていないこと（修正の往復回数を減らすための仕様）。エラー行にステータス名が添えられ、どのステータスの問題か特定できること。手順1 の一覧が config.yaml の記述順・アルファベット順など安定した順序で出ること。

### 5. 対象リポジトリ・ベースブランチの検証

- **対応する受け入れ基準:** AC-15
- **目的:** 対象の不正が登録時に検出され、detached HEAD / 空リポジトリでは `--base` の明示指定が案内されることを確認する（setup.md TC-31・TC-32 手順1・TC-33、task-execution.md TC-10・TC-11）。
- **手順:**
  1. `pulsen add --workflow implement --repo "$HOME/no-such-repo"; echo $?`
  2. `mkdir -p "$WORK/not-a-repo" && pulsen add --workflow implement --repo "$WORK/not-a-repo"; echo $?`
  3. `pulsen add --workflow implement --repo "$REPO" --base no-such-branch; echo $?`
  4. `pulsen add --workflow implement --repo "$REPO" --base ""; echo $?`
  5. `pulsen add --workflow implement --repo "$WORK/detached-repo"; echo $?`
  6. `pulsen add --workflow implement --repo "$WORK/empty-repo"; echo $?`
  7. `pulsen add --workflow implement --repo "$WORK/empty-repo" --base main; echo $?`
- **期待結果:** 手順1 はパス不在、手順2 は git リポジトリでない旨、手順3 はブランチ不在、手順4 はブランチ名の検証エラー、手順5・6 は **`--base` の明示指定を案内**して、いずれも非0。手順7 は空リポジトリにはブランチ実体が無いためブランチ不在エラーで非0。
- **確認ポイント:** 手順1 と手順2 のメッセージが区別されていること（「パスが無い」と「git リポジトリでない」）。手順5・6 で案内される回復手順（`--base <branch>` を付ける）が具体的なコマンド形で読み取れること。手順6 と手順7 のメッセージが違うこと（`--base` を足しても解決しない状況であることが分かること）。

### 6. config.yaml が権限で読めない

- **対応する受け入れ基準:** AC-13, AC-15
- **目的:** 読み取り権限のないファイルが実行環境エラーとして扱われることを確認する（setup.md TC-15 手順1・2 の読み替え b）。**POSIX のみ・root では実行しない**（`chmod 000` が効かないため）。
- **手順:**
  1. `id -u`（`0` なら本項目をスキップし、理由を Issue にコメントする）
  2. `chmod 000 "$PULSEN_HOME/config.yaml" && pulsen add --workflow implement --repo "$REPO"; echo $?`
  3. `chmod 644 "$PULSEN_HOME/config.yaml"`
- **期待結果:** 手順2 は読み取りエラー（実行環境エラー）として非0。「未初期化」の案内ではないこと。タスクは作られない。
- **確認ポイント:** メッセージが「config.yaml が無い」ではなく「読めない」であること（不在と権限は修復手段が違うため、取り違えると利用者が config.yaml を作り直してしまう）。

### 7. ロック競合

- **対応する受け入れ基準:** AC-14, AC-15
- **目的:** 別の操作がロックを保持している間の add が「別の操作が実行中」として登録前に非0終了し、タスクが作られないことを確認する。ロック保持は steps.md ステップ14 のフィクスチャ `crates/pulsen/examples/lock_holder.rs`（引数のパスをロックし `locked` を1行出力して stdin が閉じるまで保持）で作る。
- **手順:**
  1. 別ターミナル（またはバックグラウンド）で `cargo run -p pulsen --example lock_holder -- "$PULSEN_HOME/state/lock"` を実行し、`locked` の出力を待つ
  2. `pulsen add --workflow implement --repo "$REPO"; echo $?`
  3. `ls "$PULSEN_HOME/state/tasks/"`
  4. 手順1 のプロセスを終了させる（stdin を閉じる / Ctrl-D）
  5. `pulsen add --workflow implement --repo "$REPO"; echo $?`
- **期待結果:** 手順2 は「別の操作が実行中」として非0（待ちに入らず即座に返る）。手順3 でタスクは増えていない。手順5 は解放後に exit code 0 で成功する。
- **確認ポイント:** 手順2 がブロックしないこと（ロック待ちはしない仕様）。メッセージから「再実行すればよい」ことが読み取れること。手順1 のプロセスを **強制終了（`kill -9`）した場合でも** 手順5 が成功すること（保持プロセスの異常終了でロックが残らない）。

## 既存機能への影響確認

グリーンフィールドであり既存機能は無い。代わりに以下を確認する。

- **spec 側との整合:** 本スライスに `ls` / `tick` / `show` / `abort` / `retry` / `set-status` は存在しない。`spec/manual-tests/setup.md` および `task-execution.md` の手順のうちこれらを要する部分は実行できないため、plan.md「テスト方針 — 手動確認」の表に従って範囲を絞ってある。**落とした手順は Issue のチェックリストにチェックを付けず、見送った旨と理由を Issue のコメントに残す**（Issue 完了条件）。今回の確認項目でカバーしたのは setup.md TC-01・03・04・12・13・14・15（手順1・2）・16〜33・40〜46・48 の add 実行可能部分と、task-execution.md TC-01（手順3・6・7）・02（手順1）・08〜11。
- **`pulsen --help` の表示:** サブコマンドとして `add` のみが並び、未実装のコマンドや内部の `wrapper` が現れないこと。引数の使い方の誤り（例: `pulsen add` を必須オプションなしで実行）が clap 既定の exit code 2 になり、エラー・使用法が表示されること。
- **機械可読出力と生成コマンドの不提供:** `pulsen add --help` に JSON 等の機械可読出力オプションが無いこと、config.yaml / ワークフローYAML を生成するサブコマンドが `pulsen --help` に無いこと（PAGE-common-007 / PAGE-common-011 は「提供しないことを確認する行」）。
- **実運用ホームの非汚染:** 全項目の実行後に `ls -a "$HOME/.pulsen" 2>/dev/null` が実行前と変わらないこと（分離ホームの徹底）。
- **後片付け:** `rm -rf "$PULSEN_HOME" "$WORK" "$REPO" "$HOME/pulsen-empty-home" "$HOME/pulsen-default-home"`。
