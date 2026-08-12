# 動作確認結果 — Issue #1 / PR #8

**実行日:** 2026-08-12（初回） / 2026-08-12 再実行（エッジケース1・3 と `pulsen add --help`）
**実行環境:** macOS 26.4.1 (Darwin 25.4.0, arm64) / rustc 1.97.1 / cargo 1.97.0 / git 2.51.2 / uid 501（非 root）/ TMPDIR `/var/folders/l_/pptqtzd976329fq6qp57cgh40000gn/T/`
**テストソース:** .thread/1/testing.md
**検証バイナリ:** `target/debug/pulsen`（`cargo build`）/ `target/debug/examples/lock_holder`
**隔離ホーム:** `HOME` をスクラッチパッド配下（`.../scratchpad/fakehome`）に差し替えて全項目を実行。実運用の `~/.pulsen/` は一度も参照・作成していない。

**再実行の経緯（2026-08-12）:** 初回実行でエッジケース1・3 が FAIL したが、原因は testing.md のフィクスチャが「構文エラーになるはずの YAML」を作れていなかったこと（インデント崩しは構文として妥当）。フィクスチャを角括弧・引用符の閉じ忘れに差し替えた testing.md と、`--base` の doc コメントから実装者向け注記を通常コメントへ移した実装で、該当2項目と `pulsen add --help` を再実行した。

## サマリー

- 確認項目: 15件（PASS: 15 / FAIL: 0 / スキップ: 0）
- 初回 FAIL の2件（エッジケース1・3）は、testing.md のフィクスチャ修正後の再実行でいずれも PASS。実装は初回時点から変わっていない（`--base` のヘルプ文言を除く）。
- 前提のグリーン状態: `cargo test`（全スイート 0 failed）/ `cargo clippy --all-targets -- -D warnings` / `cargo fmt --check` すべて exit 0。再実行時にも再確認済み。
- AC-1 の機械的確認: `grep -rn 'cfg(unix)\|cfg(windows)' crates/*/src/` のヒットは `crates/pulsen-conformance/src/lib.rs:238` と `crates/pulsen/src/util/atomic.rs:72` の2件のみ。`crates/pulsen-domain/` は0件で期待どおり。

## 確認項目

### 1. 未初期化ホームの案内文言 — PASS

- **対応する受け入れ基準:** AC-13
- **実行:**
  ```sh
  pulsen add --workflow implement --repo "$REPO" --home "$HOME/pulsen-empty-home"; echo $?
  ls "$HOME/pulsen-empty-home" 2>/dev/null; echo $?
  ```
- **結果:** exit code 1。stderr に3行。
  ```
  エラー: グローバルホームが未初期化です。
    グローバルホーム: /.../fakehome/pulsen-empty-home
    グローバル設定 /.../fakehome/pulsen-empty-home/config.yaml を作成してください。
  ```
  `ls` は exit 2（ディレクトリ自体が作られていない）。`2>/dev/null` を付けて実行すると stdout は空で、案内は全文 stderr に出ている。
- **期待との突き合わせ:** 期待どおり。「未初期化である旨」「解決後の絶対パス」「config.yaml の作成が必要」の3点が揃い、`~` 表記や相対パスは現れない。`state/` も `state/lock` も作られておらず、ロック取得より前に config 読み込みが走っていることが確認できる。

### 2. グローバルホーム解決の優先順位 — PASS

- **対応する受け入れ基準:** AC-13, AC-16
- **実行:**
  ```sh
  PULSEN_HOME=/no/such/home pulsen add --workflow implement --repo "$REPO" --home "$PULSEN_HOME"; echo $?
  PULSEN_HOME=/no/such/home pulsen add --workflow implement --repo "$REPO"; echo $?
  FAKEHOME=$(mktemp -d); env -u PULSEN_HOME HOME="$FAKEHOME" pulsen add --workflow implement --repo "$REPO"; echo $?
  ```
- **結果:**
  - 手順1: exit 0。`タスクID: 20260812t093834-63tgy9xy` で登録成功（フラグのホームが勝つ）。
  - 手順2: exit 1。`グローバルホーム: /no/such/home` を明示。
  - 手順3: exit 1。`グローバルホーム: /var/folders/.../tmp.NLytSTV4iw/.pulsen`。
- **期待との突き合わせ:** 期待どおり。`--home` > `PULSEN_HOME` > `~/.pulsen` の順位が3手順すべてでメッセージ中の絶対パスから読み取れる。手順3 のパスが `$FAKEHOME/.pulsen` になっており、既定ホームの解決が `HOME` を見ていることの直接の証拠になっている。

### 3. 空の config.yaml が全デフォルトで受理される — PASS

- **対応する受け入れ基準:** AC-9, AC-13
- **実行:**
  ```sh
  mkdir -p "$HOME/pulsen-default-home/workflows" && touch "$HOME/pulsen-default-home/config.yaml"
  cp "$PULSEN_HOME/workflows/implement.yaml" "$HOME/pulsen-default-home/workflows/"
  pulsen add --workflow implement --repo "$REPO" --home "$HOME/pulsen-default-home"; echo $?
  ```
- **結果:** exit code 1。
  ```
  エラー: ワークフロー定義の検証に失敗しました(1件)。
    - エージェント `shell` は config.yaml に定義されていません。
      定義済みのエージェント: (1つも定義されていません)
    タスクは作られていません。
  ```
  `pulsen-default-home/state/` は作られていない。
- **期待との突き合わせ:** 期待どおり。config のパースエラーにはならず、空 config が受理された上で登録時検証（UnknownAgent）に進んでいる。空一覧は `(1つも定義されていません)` と日本語で明示されており「一覧が空」と「一覧が出ていない」を取り違えない。

### 4. 名前指定の登録成功 — 出力とタスクファイルの中身 — PASS

- **対応する受け入れ基準:** AC-14, AC-17
- **実行:**
  ```sh
  pulsen add --workflow implement --repo "$REPO"; echo $?
  ls "$PULSEN_HOME/state/tasks/"
  cat "$PULSEN_HOME/state/tasks/$T1.json"
  ls "$PULSEN_HOME/worktrees/" "$PULSEN_HOME/state/runs/" 2>/dev/null; echo $?
  rm "$PULSEN_HOME/workflows/implement.yaml" && cat "$PULSEN_HOME/state/tasks/$T1.json"
  ```
- **結果:**
  - 手順1: exit 0。stdout に4行。
    ```
    タスクを登録しました。
      タスクID: 20260812t093856-idfcsd6a
      ワークフロー: implement
      解決先: /.../fakehome/pulsen-manual-test/workflows/implement.yaml
      次回の tick で実行されます。
    ```
  - 手順3: `20260812t093856-idfcsd6a.json` の1件。ID 長は24文字、`<yyyymmdd>t<hhmmss>-<base36 8桁>` に一致。
  - 手順4: 2スペースインデントの整形済み JSON。`task_status: "queued"`、`execution: {"state":"pending"}`、`counters` は `attempt_count`/`judge_attempt_count`/`spawn_fail_count` すべて 0、`workspace`/`current_attempt`/`last_failure` は `null`、`target` は `{"repo":"/.../fakehome/pulsen-test-repo","base_branch":"main"}`、`updated_at: "2026-08-12T09:38:56Z"`、`snapshot.statuses` に `done`/`implemented`/`queued`/`review_waiting` の4件。`implemented.timeout` は `"1800s"` に正規化されている。
  - 手順5: exit 2（`worktrees/`・`state/runs/` とも不在）。`state/` 直下は `lock` と `tasks` のみ。
  - 手順6: 定義ファイルを削除しても `snapshot.statuses` は4件のまま残存。
- **期待との突き合わせ:** 期待どおり。JSON は1行に潰れておらず目視で状態判断できる。`repo` は指定値ではなく絶対パスに正規化され、`updated_at` は UTC・秒精度（`Z` 付き）。

### 5. 二重登録の独立性と `state/` の自動作成 — PASS

- **対応する受け入れ基準:** AC-16, AC-17
- **実行:**
  ```sh
  rm -rf "$PULSEN_HOME/state"
  pulsen add --workflow implement --repo "$REPO"; echo $?
  pulsen add --workflow implement --repo "$REPO"; echo $?
  ls "$PULSEN_HOME/state/tasks/"; ls -a "$PULSEN_HOME/state/"
  ```
- **結果:** 2回とも exit 0、タスクID は `20260812t093906-69cdvfr5` と `20260812t093907-5lkud50r` で別物。`state/tasks/` に2件。`state/` 直下は `lock`（0バイトの通常ファイル）と `tasks/` のみで、`archive/`・`runs/` は未作成。
- **期待との突き合わせ:** 期待どおり。重複警告は一切出ない。ロックファイル名は `lock` で `<task-id>.json` 形式と衝突しないため、タスク走査に混ざらない。

### 6. `--workflow` の解釈規則と表示名の決定 — PASS

- **対応する受け入れ基準:** AC-3, AC-16
- **実行:**
  ```sh
  cd "$WORK" && pulsen add --workflow ./draft.yaml --repo "$REPO"; echo $?
  pulsen add --workflow "$WORK/custom.yaml" --repo "$REPO"; echo $?
  cp "$WORK/draft.yaml" "$WORK/nameclash.yaml" && cd "$WORK" && pulsen add --workflow nameclash --repo "$REPO"; echo $?
  cp "$PULSEN_HOME/workflows/implement.yaml" "$PULSEN_HOME/workflows/impl2.yml" && pulsen add --workflow impl2 --repo "$REPO"; echo $?
  pulsen add --workflow "$PULSEN_HOME/workflows/impl2.yml" --repo "$REPO"; echo $?
  ```
- **結果:**
  - 手順1: exit 0。`ワークフロー: my-flow` / `解決先: /.../pulsen-manual-work/draft.yaml`（`cd` 先からの相対指定が絶対化されている）。
  - 手順2: exit 0。`ワークフロー: custom`（`workflow:` キーが無くファイル名由来）。
  - 手順3: exit 1。`解決を試みたパス: /.../pulsen-manual-test/workflows/nameclash.yaml`。カレントの `nameclash.yaml` は使われない。
  - 手順4: exit 1。`解決を試みたパス: /.../workflows/impl2.yaml`（`.yml` にフォールバックしない）。
  - 手順5: exit 0。`ワークフロー: implement` / `解決先: .../impl2.yml`。
  - 手順6: タスクファイルの `workflow_name` は `my-flow` / `custom` / `implement` で表示と一致。
- **期待との突き合わせ:** 期待どおり。手順3・4 のエラーはどちらも「解決を試みた絶対パス」を含み、配置漏れ・拡張子違いに気づける。

### 7. `--base` の解決（HEAD 由来と明示指定） — PASS

- **対応する受け入れ基準:** AC-14
- **実行:**
  ```sh
  pulsen add --workflow implement --repo "$REPO"; echo $?
  pulsen add --workflow implement --repo "$REPO" --base develop; echo $?
  cd "$HOME" && pulsen add --workflow implement --repo ./pulsen-test-repo; echo $?
  jq -c '.target' "$PULSEN_HOME/state/tasks/"*.json
  ```
- **結果:** 3件とも exit 0。`target` はそれぞれ `base_branch: "main"` / `"develop"` / `"main"`。相対指定した手順3 の `repo` は `/.../fakehome/pulsen-test-repo` の絶対パスに正規化されている。
- **期待との突き合わせ:** 期待どおり。ベースブランチは登録時点で確定しタスクファイルから確認できる。なお成功時の stdout にはベースブランチもリポジトリパスも出ない（確認ポイントは「出力**または**タスクファイル」なので合致。観察としては後述）。

### 8. 受理される定義の広さ（循環・到達不能・終端なし・境界値） — PASS

- **対応する受け入れ基準:** AC-16
- **実行:**
  ```sh
  for f in polling polling-noend boundary; do
    pulsen add --workflow "$WORK/$f.yaml" --repo "$REPO"; echo "$f -> $?"
  done
  ```
- **結果:** 3件とも exit 0 で登録成功。警告出力は一切なし。snapshot は次のとおり保存されていた。
  - `polling`: `check` が自分自身を `next` に持ち、`done`（cleanup）は到達不能のまま残る。
  - `polling-noend`: statuses が `check` 1件のみ、cleanup 終端なし。
  - `boundary`: `{"start":{"action":"agent_run",...,"timeout":"none","retries":0,...,"next":"start"}}`。
- **期待との突き合わせ:** 期待どおり。境界値 `retries: 0` と `timeout: none` は 0/None に潰されず、それぞれ `0` と文字列 `"none"` として snapshot に残っている。到達不能・循環・終端なしのいずれも警告扱いされない。

## エッジケース・異常系

### 1. config.yaml のパース不能（構文エラー・未知キー） — PASS（2026-08-12 再実行）

- **対応する受け入れ基準:** AC-13, AC-15
- **実行:**
  ```sh
  printf 'agents:\n  shell:\n    cmd: ["sh"\n' > "$PULSEN_HOME/config.yaml"   # 角括弧の閉じ忘れ
  pulsen add --workflow implement --repo "$REPO"; echo $?
  cp "$WORK/config.bak" "$PULSEN_HOME/config.yaml" && printf 'run_retension: 30d\n' >> "$PULSEN_HOME/config.yaml"
  pulsen add --workflow implement --repo "$REPO"; echo $?
  cp "$WORK/config.bak" "$PULSEN_HOME/config.yaml" && pulsen add --workflow implement --repo "$REPO"; echo $?
  ```
- **結果:**
  - 手順2: exit 1。
    ```
    エラー: グローバル設定を解釈できません。
      ファイル: /.../pulsen-manual-test/config.yaml
      原因: did not find expected ',' or ']' at line 4 column 1, while parsing a flow sequence at line 3 column 10
      位置: 4行1列
    ```
  - 手順4: exit 1。`原因: スキーマに無いキーです: run_retension`（typo キーを名指し）。
  - 手順5: exit 0 に復帰（`タスクID: 20260812t094941-d03sgx1n`）。
  - `state/tasks/` は手順2・4 の前後とも0件。config.yaml の mtime・内容とも失敗前後で変化なし。
- **期待との突き合わせ:** 期待どおり。手順2 は**位置（4行1列）**を添えた構文エラーとして非0で拒否され、あわせて「未終端のフロー列が始まった位置（3行10列）」も示されるため、実際に壊れている行（`cmd: ["sh"` の 3 行目）へ直接たどり着ける。手順4 は未知キーを名指し、手順5 は正常復帰。

### 2. ワークフロー定義の解決失敗 — PASS

- **対応する受け入れ基準:** AC-15
- **実行:**
  ```sh
  pulsen add --workflow nosuchflow --repo "$REPO"; echo $?
  cd "$HOME" && pulsen add --workflow ./nosuch.yaml --repo "$REPO"; echo $?
  chmod 000 "$PULSEN_HOME/workflows/implement.yaml" && pulsen add --workflow implement --repo "$REPO"; echo $?; chmod 644 "$PULSEN_HOME/workflows/implement.yaml"
  ```
- **結果:** 3件とも exit 1、タスクは0件のまま。
  - 手順1: `エラー: ワークフロー定義が見つかりません。` / `解決を試みたパス: /.../pulsen-manual-test/workflows/nosuchflow.yaml`
  - 手順2: 同じ文言で `解決を試みたパス: /.../fakehome/nosuch.yaml`（カレントディレクトリからの解決結果）
  - 手順3: `エラー: ワークフロー定義を読み込めません。` / `原因: /.../implement.yaml: Permission denied (os error 13)`
- **期待との突き合わせ:** 期待どおり。手順1・2 は「不在」として同一の形（試みたパス付き）に揃い、手順3 は「読み込めません」と別の見出しで区別されている。uid は 501 なので `chmod 000` は有効（root スキップ条件に該当せず）。

### 3. ワークフローYAMLの構造エラー — PASS（2026-08-12 再実行）

- **対応する受け入れ基準:** AC-4, AC-15
- **実行:** testing.md 記載の13ファイル（`e-syntax.yaml` は引用符の閉じ忘れ `prompt: "hi`）を生成し、`for f in "$WORK"/e-*.yaml; do pulsen add --workflow "$f" --repo "$REPO"; done`
- **結果:** 13件すべて exit 1、`state/tasks/` は前後とも0件。個々の文言:

  | ファイル | 出力（`エラー: ワークフロー定義が不正です。` に続く行） |
  | --- | --- |
  | e-no-initial | `initial が指定されていません。` |
  | e-no-statuses | `statuses が空、または指定されていません。` |
  | e-bad-initial | ``initial が指すステータス `nosuch` が statuses にありません。`` |
  | e-bad-next | ``ステータス `start` の next が指す `nosuch` が statuses にありません。`` |
  | e-no-next | ``エージェント実行のステータス `start` に next がありません。`` |
  | e-no-action | ``ステータス `start` に動作宣言(prompt / skill / run)がありません。`` |
  | e-two | ``ステータス `start` に動作宣言が複数あります: prompt, skill`` |
  | e-bad-run | ``ステータス `start` の run の値 `clenaup` は cleanup / wait のいずれでもありません。`` |
  | e-forbidden | ``ステータス `waiting` の動作種別では使えないキー `judge` があります。`` |
  | e-unknown-key | ``statuses.start にスキーマ外のキー `prmopt` があります。`` |
  | e-syntax | `YAML 構文エラー: .../e-syntax.yaml: found unexpected end of stream at line 6 column 1, while scanning a quoted scalar at line 5 column 13` / `位置: 6行1列` |
  | e-dup | `YAML 構文エラー: .../e-dup.yaml: statuses: duplicate entry with key "start" at line 4 column 3` / `位置: 4行3列` |
  | e-timeout0 | `statuses.start.timeout の値が不正です: 期間に 0 は指定できません` |

  実行前後で `$WORK/e-*.yaml` と `$PULSEN_HOME/config.yaml` の mtime（`stat -c '%Y %n'`）に変化なし。`e-syntax.yaml` が本当に構文エラーであることは第三者パーサでも確認した（`Psych::SyntaxError: found unexpected end of stream while scanning a quoted scalar at line 5 column 13`）。
- **期待との突き合わせ:** 期待どおり。13件非0・タスク0件・`e-syntax` と `e-dup` の位置表示・`e-unknown-key` の `prmopt` 名指し・`e-forbidden` の `judge` 名指しをすべて満たす。`e-syntax` は未終端の引用符が始まった位置（5行13列）も併記され、直すべき行が特定できる。文言もエラー種ごとに書き分けられており汎用文の使い回しはない。

### 4. 登録時検証エラー（参照時検証の文言と全件列挙） — PASS

- **対応する受け入れ基準:** AC-5, AC-15
- **実行:** testing.md 手順1〜7 をそのまま実行。
- **結果:** すべて `エラー: ワークフロー定義の検証に失敗しました(N件)。` … `タスクは作られていません。` の形で、`state/tasks/` は増えない。
  - 手順1: exit 1。``エージェント `cladue` は config.yaml に定義されていません。`` / `定義済みのエージェント: claude, shell`
  - 手順2: exit 1。``ステータス `start`: エージェントが指定されていません(ステータスの agent かワークフローの agent が要ります)。``
  - 手順3: exit 1。``エージェント `claude` の cmd が {model} を参照していますが、ステータス `start` にもワークフローにも model の指定がありません。``
  - 手順4: exit 1。``ステータス `start` は skill を使いますが、エージェント `shell` に skill_input がありません。``
  - 手順5: exit 1。``エージェント `shell` の定義が不正です: cmd の`{inptu}` は使えないプレースホルダ `inptu` を参照しています``
  - 手順6: exit 1。`(2件)` として `nosuch` と `nosuch2` の**両方**を1回の実行で列挙。
  - 手順7: exit 0（参照されない `broken` は検証されない）。
- **期待との突き合わせ:** 期待どおり。全件列挙は最初の1件で打ち切られていない。エージェント一覧は `claude, shell` とアルファベット順で安定している。ステータス固有のエラー（手順2〜4）にはステータス名が添えられている。

### 5. 対象リポジトリ・ベースブランチの検証 — PASS

- **対応する受け入れ基準:** AC-15
- **実行:** testing.md 手順1〜7 をそのまま実行。
- **結果:** 7件すべて exit 1、`state/tasks/` は前後とも0件。
  - 手順1: `指定したリポジトリのパスが存在しません。`
  - 手順2: `指定したパスは git リポジトリではありません。`
  - 手順3: `指定したベースブランチがリポジトリに存在しません。` / `ブランチ: no-such-branch`
  - 手順4: `--base の値がブランチ名として不正です。` / `原因: 空文字列は指定できません`
  - 手順5: `HEAD がブランチを指していません(detached HEAD)。` / `--base でベースブランチを明示してください。`
  - 手順6: `コミットのない空のリポジトリです。` / `--base でベースブランチを明示してください。`
  - 手順7: `指定したベースブランチがリポジトリに存在しません。` / `ブランチ: main`
- **期待との突き合わせ:** 期待どおり。手順1（パス不在）と手順2（git でない）は別文言。手順5・6 は `--base` の明示という回復手順を提示。手順6 と手順7 は文言が異なり、「`--base` を足しても空リポジトリでは解決しない」ことが読み取れる。

### 6. config.yaml が権限で読めない — PASS

- **対応する受け入れ基準:** AC-13, AC-15
- **実行:**
  ```sh
  id -u                       # -> 501（root ではないので実行可）
  chmod 000 "$PULSEN_HOME/config.yaml" && pulsen add --workflow implement --repo "$REPO"; echo $?
  chmod 644 "$PULSEN_HOME/config.yaml"
  ```
- **結果:** exit 1。
  ```
  エラー: グローバル設定を読み込めません。
    ファイル: /.../pulsen-manual-test/config.yaml
    原因: Permission denied (os error 13)
  ```
  タスクは作られない。権限を戻した直後の add は exit 0 に復帰。
- **期待との突き合わせ:** 期待どおり。「未初期化です」でも「見つかりません」でもなく「読み込めません」＋`Permission denied` で、不在と権限が明確に区別されている。

### 7. ロック競合 — PASS

- **対応する受け入れ基準:** AC-14, AC-15
- **実行:**
  ```sh
  mkfifo "$SP/fifo"
  target/debug/examples/lock_holder "$PULSEN_HOME/state/lock" < "$SP/fifo" > "$SP/holder.out" &
  exec 3> "$SP/fifo"          # stdin を開いたまま保持
  # holder.out に "locked" が出るまで待つ
  pulsen add --workflow implement --repo "$REPO"; echo $?
  exec 3>&-                   # stdin を閉じて解放
  pulsen add --workflow implement --repo "$REPO"; echo $?
  ```
- **結果:**
  - 保持中の add: exit 1。`エラー: 別の操作が実行中です。` / `時間をおいて再実行してください。タスクは作られていません。` 所要 **13ms**（ブロックしていない）。
  - `state/tasks/` は0件のまま。
  - stdin クローズ後の add: exit 0 で登録成功。
  - 追加確認（確認ポイント）: 別セッションで holder を `kill -9` した場合も、直後の add は exit 0 で成功した（保持プロセスの異常終了でロックが残らない）。
- **期待との突き合わせ:** 期待どおり。待ちに入らず即座に返り、メッセージから「再実行すればよい」ことが読み取れる。

## 既存機能への影響確認（testing.md 末尾セクション）

グリーンフィールドのため既存機能の退行確認は対象外。代わりに以下を実施。

- **`pulsen --help` の表示:** サブコマンドは `add` と clap 既定の `help` のみ。`wrapper` / `tick` / `ls` / `show` / `init` / `generate` に相当するものは現れない（単語境界付き grep で0件）。
- **引数の使い方の誤り:** `pulsen add`（必須オプションなし）は exit code 2 で `error: the following required arguments were not provided:` と Usage を表示。`pulsen`（サブコマンドなし）も exit code 2 でヘルプを表示。`pulsen --version` は `pulsen 0.1.0`。
- **機械可読出力・生成コマンドの不提供:** `pulsen add --help` に `--json` / `--format` / `--output` 等は無い（PAGE-common-007）。`pulsen --help` に config.yaml / ワークフローYAML の生成サブコマンドは無い（PAGE-common-011）。2026-08-12 の再実行でも `pulsen --help` のサブコマンドは `add` と clap 既定の `help` のみ、`pulsen add --help` のオプションは `--home` / `--workflow` / `--repo` / `--base` / `-h` のみであることを再確認した。
- **実運用ホームの非汚染:** 全項目の実行後、`ls -a /Users/hikaru/.pulsen` は実行前と同じく `No such file or directory`。`/Users/hikaru/` 直下にも `pulsen-manual-test` / `pulsen-manual-work` / `pulsen-test-repo` / `pulsen-empty-home` / `pulsen-default-home` / `nosuch.yaml` はいずれも作られていない。2026-08-12 の再実行でも実行前・実行後の両方で同じ状態を確認した（フィクスチャはすべてスクラッチパッド配下の `fakehome/` にのみ作成）。
- **後片付け:** 隔離ホーム配下のフィクスチャはすべて削除済み。リポジトリの作業ツリーはクリーン（本再実行でのコード変更なし）。

## 初回 FAIL の顛末（解消済み）

### エッジケース1 手順2 / エッジケース3 `e-syntax.yaml` — 初回 FAIL の原因は testing.md のフィクスチャ

**初回の症状:** どちらも「行・列付きの構文エラー」を期待した箇所で、位置のない未知キーエラーが返った。

- エッジ1 手順2 の入力 `agents:\nshell:\n    cmd: ["sh"]\n` → `スキーマに無いキーです: shell`
- エッジ3 `e-syntax.yaml` の入力 `agent: shell\ninitial: start\nstatuses:\nstart:\n    prompt: "hi"\n` → ``(トップレベル) にスキーマ外のキー `start` があります。``

**原因: testing.md のフィクスチャが「構文エラー」になっていなかった（実装とは無関係）。**

どちらの入力も YAML としては完全に妥当で、インデントを崩した結果「入れ子が外れてトップレベルの別キーになった」だけだった。第三者パーサでの確認:

```
$ ruby -ryaml -e 'p YAML.safe_load(File.read(ARGV[0]))' e-syntax.yaml
{"agent"=>"shell", "initial"=>"start", "statuses"=>nil, "start"=>{"prompt"=>"hi"}}
$ ruby -ryaml -e 'p YAML.safe_load(File.read(ARGV[0]))' broken-config.yaml
{"agents"=>nil, "shell"=>{"cmd"=>["sh"]}}
```

構文エラーが存在しない以上、パーサが行・列を返せる余地はなく、実装が未知キーとして扱ったのは厳格スキーマ（ADR-013）の期待どおりの振る舞いだった。

**対処と再実行:** testing.md のフィクスチャを本当の構文エラー（config = 角括弧の閉じ忘れ、ワークフロー = 引用符の閉じ忘れ）に差し替え、両項目を再実行して PASS を確認した（各項目の記録を参照）。実装には手を入れていない。

**残る観察（軽微）:** 未知キーエラーには位置が付かない。エッジ3 のワークフロー側は `statuses.start にスキーマ外のキー`／`(トップレベル) にスキーマ外のキー` とキーパスが付くので特定できるが、config 側の `スキーマに無いキーです: run_retension` はキーパスも位置も無く、大きな config では該当箇所を探すのにやや手間がかかる。

## スキップ

なし。15件すべて実行した。

- エッジケース2 手順3 とエッジケース6 は「root ではスキップ」の条件付きだが、実行 uid は 501 のため条件に該当せず、通常どおり実行した。
- エッジケース7 の `lock_holder` は「別ターミナル」の代わりに名前付きパイプで stdin を開いたまま保持するバックグラウンドプロセスとして起動した（保持条件は同じ）。

## 気づいた点（観察）

1. **成功時の出力に対象リポジトリとベースブランチが出ない。** 表示されるのはタスクID・ワークフロー名・解決先パスの3点で、「どのリポジトリのどのブランチから worktree が作られるのか」は task JSON を開かないと分からない。`--repo` を相対指定したときの正規化結果や、`--base` 省略時に HEAD からどのブランチが選ばれたかは登録直後にいちばん確認したい情報なので、出力に1〜2行増やす余地がある。testing.md 項目7 の確認ポイントは「出力**または**タスクファイル」なので判定には影響しない。

2. **`pulsen add --help` の `--base` の説明に入っていた設計判断の注記は解消済み（2026-08-12 再実行で確認）。** 初回は長ヘルプに `` `-` で始まる値も値として受け取る — …(スペック境界値)`` という実装者向けの why が出ていた。doc コメントから通常コメントへ移す修正後、`pulsen add --help` / `-h` の `--base` 行は `ベースブランチ(省略時はリポジトリの HEAD が指すブランチ)` の1行のみになり、実装者向けの注記は表示されない。振る舞いの退行もなく、`--base -weird` は clap の使い方エラー（exit 2）ではなく `--base の値がブランチ名として不正です。` / ``原因: `-` で始まる名前は使えません`` で exit 1、タスクは作られない（`allow_hyphen_values` が効いたまま）。

3. **`指定したリポジトリのパスが存在しません。` / `指定したパスは git リポジトリではありません。` に該当パスが出ない。** 他のエラー（ホーム未初期化・ワークフロー解決失敗・config 読み込み失敗）はすべて絶対パスを添えているのに、リポジトリ系のエラーだけ「どのパスを見て言っているのか」が出ない。相対パス指定や cron からの実行では、正規化後にどこを見に行ったかが分からないと切り分けにくい。

4. **detached HEAD / 空リポジトリの回復案内は「フラグ名」止まり。** `--base でベースブランチを明示してください。` は正確だが、testing.md の確認ポイントが求める「具体的なコマンド形」までは踏み込んでいない（`pulsen add --workflow ... --repo ... --base <branch>` のような再実行例は出ない）。フラグ名は分かるので判定は PASS とした。

5. **UnknownAgent のエラー行にステータス名が付かない。** エッジ4 手順6 の出力は2件とも ``エージェント `nosuchN` は config.yaml に定義されていません。`` のみで、`nosuch2` がステータス `b` の指定であることは行から読み取れない。他の検証エラー（手順2〜4）はステータス名を添えているので、そこだけ粒度が揃っていない。エージェント名がファイル内で一意なら特定はできるため、期待結果の「両方が列挙される」は満たしている。

6. **`snapshot` の `timeout` 表現が揃っていない。** `30m` は `"1800s"` に正規化される一方、`none` は文字列 `"none"` のまま保持される。どちらも情報は失われておらず仕様どおりだが、JSON を直接読むときに「秒数文字列」と「特別値の文字列」が同じフィールドに同居する形になっている。

7. **`state/lock` はロック解放後も0バイトのファイルとして残る。** 参照実装として妥当（`flock` 系はファイル自体を消さない）で、名前が `<task-id>.json` 形式でないため走査にも混ざらない。`ls -a state/` に出るのが `lock` と `tasks` だけなので、`state/` を目視したときの見通しは良い。
