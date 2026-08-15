# 動作確認計画 — Issue #9: spec 追従: Issue #1 / #2 / #3 の実装で判明した記述の食い違い

**Issue:** #9
**作成日:** 2026-08-15

---

## 確認環境

このIssueの変更を確認するために必要な手順のみ記載（プロジェクト全体のセットアップは省略）。

本 Issue の25件のうち **23件は `spec/` のドキュメント変更**であり、実行して確認できるものではない。これらの検証手段は「実機での動作確認」ではなく**ドキュメントの整合確認** — steps.md ステップ20 の `grep` 表12語の実行と、台帳（`spec/inventory/`）の ID の一意性・末尾追記・`最終同期` の確認である（確認項目1〜3）。

**実機で動かして確認できるのは A2 / C5 の2件だけ**（確認項目4〜8）。この2件は `crates/` の実装も変える。ワークスペースは CLI バイナリ1本（`pulsen`）とライブラリ2本（`pulsen-domain` / `pulsen-conformance`）のみで（`Cargo.toml` の `[workspace] members`）、**Web UI も dev サーバも無い**。確認はすべてターミナル上の CLI 実行・ファイルの直読・`grep` で行う。

本スライスに存在するサブコマンドは `pulsen add` / `pulsen tick` / `pulsen wrapper`（隠し）の3つだけで、`ls` / `show` / `abort` は未実装（`crates/pulsen/src/cli/args.rs` の `Command`）。したがってタスクの状態は `state/tasks/<task-id>.json` の直読で観測し、通知失敗の再現も `abort` ではなく **tick によるリトライ上限超過の凍結**で作る。

### 検証環境の起動

`.envrc` は `use flake` のみ。direnv がこのリポジトリで許可済みであれば、リポジトリに `cd` するだけで `flake.nix` の devShell（cargo / rustc / clippy / rustfmt / git）に入る。direnv を使わない場合は各コマンドを `nix develop -c <command>` で包む。

```sh
cd /Users/hikaru/github.com/tuanemuy/pulsen
cargo --version    # devShell が効いていることの確認
git --version      # worktree 操作で使う（flake.nix の buildInputs に git がある）
```

ビルドとバイナリの解決。bin 名は `pulsen`（`crates/pulsen/Cargo.toml` の package 名 + `src/main.rs`。`[[bin]]` セクションは無い）。

```sh
cargo build
export PATH="$PWD/target/debug:$PATH"
pulsen --help
```

確認を始める前の基準状態（この4つが緑であること。コマンドの形は `.github/workflows/ci.yml` の各ステップに合わせてある）。

```sh
cargo fmt --all --check
cargo build --workspace --locked
cargo test --workspace --locked --no-fail-fast -- --nocapture
cargo clippy --workspace --all-targets --locked -- -D warnings
```

`cargo fmt` に `--locked` は付けない（rustfmt へのラッパーで依存解決を行わないため受け付けない）。ローカルの rustfmt は nixpkgs 同梱で CI の現行 stable とは版が違うため、ここが緑でも CI の fmt ジョブが赤になりうる。

`--nocapture` を付けるのは、適合スイートのスキップ行（`SKIP …`）が標準出力に出ないと確認項目8 の読み取りができないため（ci.yml「テストする」ステップのコメント）。`--no-fail-fast` を付けるのは、テストターゲット単位の打ち切りで後続のテストバイナリが「赤でない」ではなく「未観測」になるのを避けるため（同ステップのコメント）。

JSON の項目を絞って読むときはシステムの `jq` を使う（devShell には含まれない）。

### A2 / C5 の実機確認用フィクスチャ

確認項目4〜7 が使う。実運用のホーム（`~/.pulsen/`）を汚さないため、専用ホームを `PULSEN_HOME` で与える（`crates/pulsen/src/cli/wire.rs` の解決順は `--home` > `PULSEN_HOME` > `~/.pulsen/`）。

```sh
export P9=/tmp/pulsen-issue9
rm -rf "$P9"
mkdir -p "$P9/home/workflows"
export PULSEN_HOME="$P9/home"

# 対象リポジトリ（worktree 内でコミットするため identity をリポジトリローカルに設定する）
git init -b main "$P9/repo"
git -C "$P9/repo" config user.name pulsen-test
git -C "$P9/repo" config user.email pulsen-test@example.com
git -C "$P9/repo" commit --allow-empty -m init

# グローバル設定（notify_cmd は確認項目6 で差し替える。控えを取っておく）
cat > "$P9/home/config.yaml" <<'EOF'
agents:
  shell:
    cmd: ["sh", "-c", "{input}"]
notify_cmd: ["sh", "-c", "echo \"stopped $TASK_ID $WORKFLOW $TASK_STATUS\" >> /tmp/pulsen-issue9/notify.log"]
EOF
cp "$P9/home/config.yaml" "$P9/config.bak"
: > "$P9/notify.log"

# 正常なワークフロー（対照用）
cat > "$P9/home/workflows/ok.yaml" <<'EOF'
workflow: ok
agent: shell
initial: work
statuses:
  work:
    prompt: "true"
    next: done
  done:
    run: cleanup
EOF

# YAML 構文エラーの定義（A2 用）
cat > "$P9/home/workflows/broken-syntax.yaml" <<'EOF'
statuses: [
EOF

# スキーマ違反の定義（A2 用。未知キー）
cat > "$P9/home/workflows/broken-schema.yaml" <<'EOF'
typo_key: 1
agent: shell
initial: queued
statuses:
  queued:
    prompt: 実装して
    next: queued
EOF

# 即座に凍結するワークフロー（C5 用。retries: 0 なので 3 tick で stopped になる）
cat > "$P9/home/workflows/fail0.yaml" <<'EOF'
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
```

連続する tick の間は 2〜3 秒あける（`spec/manual-tests/task-execution.md`「実行上の注意」）。1 attempt の消化には「起動 → 起動確認 → 判定」の3 tick を要する。

### デプロイ方法

なし。成果物は CLI バイナリ1本で、配布・常駐プロセス・マイグレーションはいずれも存在しない。確認はローカルの `cargo build` で作ったバイナリで行う。

---

## 確認項目

### 1. 言い換え前の語が `spec/` に残っていない（23件の消化）

- **対応する受け入れ基準:** AC-Z1、および各件の gate（AC-A2 の `Parse(`、AC-A6 の `スナップショットフィールドのみ(を)?構文不正`、AC-B4 の `message: String`、AC-B8 の `InvariantViolated`、AC-C3 の `RunningClassifier の2段規則`）
- **目的:** 23件の spec 言い換えは実行して確かめられない。言い換え前の語が0件になることが、本体と台帳の両方を直したことの唯一の機械的な証拠になる。各語は「正しく実装すれば0件になり、無関係な行（doc コメント等）を拾わない」ことを実測済みで、括弧内は**実装前**のヒット数（steps.md ステップ20 の表と同じ）。
- **手順:**
  1. 次の12コマンドを実行し、すべて0件になることを確認する。

     ```sh
     grep -rn 'InvariantViolated' spec/                                  # 実装前 12
     grep -rn "expected: &'static str" spec/                             # 実装前 1
     grep -rn 'InconsistentRunFiles { message' spec/                     # 実装前 1
     grep -n 'message: String' spec/usecases/execution.md                # 実装前 1
     grep -rn 'の結果としてのみ' spec/                                    # 実装前 2
     grep -rn 'いずれも `parse' spec/                                     # 実装前 1
     grep -rn '拡張子を除くと空になる' spec/                              # 実装前 3
     grep -rnE 'スナップショットフィールドのみ(を)?構文不正' spec/         # 実装前 4
     grep -rn -- '-> RunningDecision' spec/                              # 実装前 1
     grep -rn 'RunningClassifier の2段規則' spec/                         # 実装前 3
     grep -rn -- '-> JudgeOutcome' spec/                                 # 実装前 1
     grep -rn 'Parse(' spec/                                             # 実装前 28
     ```
  2. `.thread/9/research.md`「波及の洗い出し」の表の全行について、`git diff --name-only spec/` に「spec 本体」列と「台帳」列の両方のファイルが現れることを確認する。
  3. 「対応なし」で確定させた4行（`DOM-definition-050` / `UC-execution-007` / `DOM-execution-028`〜`033` / `PAGE-wrapper-005`）が research.md の `RegistrationValidator` 行・注3・注6・注7 にその旨とともに書かれていることを確認する。
- **期待結果:**
  - 手順1: 12コマンドすべて**ヒット0件**（`grep` の終了コードは1）。
  - 手順2: 波及表の全行が本体・台帳の両方で消化されている。片側だけのファイルしか現れない行が無い。
  - 手順3: 消化確認で宙に浮く行が無い。
- **確認ポイント:** #7 の語は `拡張子を除くと空` **ではなく** `拡張子を除くと空になる`。ステップ7 が指定する置換後テキスト自身が「『拡張子を除くと空』は作れない」という why としてこの語を含むため、絞らないと正しく実装しても1件残る。#8 の `(を)?` を落とさないこと — `state/archive/` 側の2行は助詞が無い（「のみ構文不正」）。#12 の `Parse(` が0件になるのは、ポート表・台帳だけでなく**適合ケース13行と台帳13行**（`spec/testcases/ports/workflow-store.md` 37〜49行 / `spec/inventory/test.md` 600〜612行）まで記法をそろえたときだけ。

### 2. 台帳の ID が一意で、新規行が末尾に追記され、既存 ID が変わっていない

- **対応する受け入れ基準:** AC-Z2
- **目的:** 台帳の連番は「spec 内の出現順」で、表の途中に行を挿入すると以降の ID がすべてずれる。完全性ゲート・`implement-audit`・`spec-to-issues` はすべてこの ID を基準に走るため、ずれは下流のすべての検証を静かに壊す。
- **手順:**
  1. ID の重複を検出する。

     ```sh
     for f in spec/inventory/*.md; do
       echo "== $f"
       awk -F'|' 'NF>2 {gsub(/[ \t]/,"",$2); print $2}' "$f" \
         | grep -v '^ID$' | grep -v '^-*$' | sort | uniq -d
     done
     ```
  2. 既存 ID の並びが変わっていないこと（新規行が末尾にだけ足されたこと）を、変更前後の ID 列の差分で見る。

     ```sh
     ids() { awk -F'|' 'NF>2 {gsub(/[ \t]/,"",$2); print $2}' | grep -v '^ID$' | grep -v '^-*$'; }
     for f in spec/inventory/*.md; do
       echo "== $f"
       diff <(git show "origin/main:$f" | ids) <(ids < "$f")
     done
     ```
  3. 新規 ID が各グループの最大番号 + 1 であることを確認する（13件。実装前の各グループの最大は `DOM-definition-056` / `DOM-task-079` / `DOM-execution-071` / `PAGE-wrapper-005` / `PAGE-tick-009` / `TC-port-run-store-034` / `TC-exec-tick-159` / `TC-exec-run-wrapper-027`）。

     ```sh
     grep -n 'DOM-definition-057\|DOM-task-080\|DOM-task-081' spec/inventory/domain.md
     grep -n 'DOM-execution-07[23456]' spec/inventory/domain.md
     grep -n 'PAGE-wrapper-006\|PAGE-tick-010' spec/inventory/frontend.md
     grep -n 'TC-port-run-store-035\|TC-exec-tick-160\|TC-exec-run-wrapper-028' spec/inventory/test.md
     ```
  4. `最終同期` の日付が5ファイルすべてで更新されていることを確認する。

     ```sh
     grep -rn '最終同期' spec/inventory/*.md
     ```
- **期待結果:**
  - 手順1: 5ファイルとも**出力なし**（重複ゼロ）。実装前の行数は adapter 37 / domain 206 / frontend 83 / test 629 / usecase 23 で、いずれも重複は無い。
  - 手順2: `diff` の出力が**末尾への追加（`>` 行）だけ**。既存 ID の削除（`<` 行）や順序の入れ替わりが1件も無い。追加されるのは domain 8行・frontend 2行・test 3行（`TC-port-run-store-035` / `TC-exec-tick-160` / `TC-exec-run-wrapper-028`）の計13行で、`usecase.md` / `adapter.md` には新規行が無い。
  - 手順3: 13個の新規 ID がすべて1件ずつヒットし、各ファイルの**表の末尾**にある。
  - 手順4: 5ファイルすべてが実装前の `2026-08-11` から更新されている。
- **確認ポイント:** 手順2 が「末尾への追加だけ」であることが、`spec/testcases/` の表に行を挿入していないことの裏付けになる（ステップ9 の `TC-port-run-store-035` とステップ11 の新規ケース2行（`tick.md` 手続きD 異常系・`run-wrapper.md` 異常系）がいずれも表の末尾への追加であり、ステップ10 の適合ケース13行は行数不変であること）。要点欄だけを書き換えた行は ID が変わらないので、この `diff` には現れない。

### 3. `.adr/` の3ファイルが更新され、古い形が残っていない

- **対応する受け入れ基準:** AC-Z4
- **目的:** Issue の完了条件が「実装を直す判断になったものは、対応する `.adr/` エントリも更新する」と明示している。更新しないと、`.adr/` を根拠に読む後続の実装者が「現行の形（回避策・`detail`）が正しい」と読み続ける。追記だけで済ませると、同じ節に新旧2つの形が並ぶ。
- **手順:**
  1. `( git diff --name-only origin/main...HEAD -- .adr/; git diff --name-only -- .adr/ ) | sort -u`
  2. `grep -n 'Failed { detail }' .adr/3-notification-procedure-layering.md`
  3. `grep -n 'Failed { cause' .adr/3-notification-procedure-layering.md`
  4. `grep -n 'resolved_from\|Issue #9' .adr/1-workflow-error-file-path-goes-into-free-form-messages.md .adr/1-schema-error-location-is-logical.md`
  5. `git diff origin/main -- .adr/` を読み、`.adr/1-schema-error-location-is-logical.md` の「`location` を論理位置に限る」決定本体が消えていないことを見る。
  6. `head -6 .adr/1-workflow-error-file-path-goes-into-free-form-messages.md` でタイトルとステータスを読み、決定節が決定時点の文面のままであることを `git diff origin/main -- .adr/1-workflow-error-file-path-goes-into-free-form-messages.md` で見る。
- **期待結果:**
  - 手順1: **5ファイル**が現れる。うち3ファイルが AC-Z4 の対象（`1-workflow-error-file-path-goes-into-free-form-messages.md` / `1-schema-error-location-is-logical.md` / `3-notification-procedure-layering.md`）、残り2ファイル（`1-task-file-json-and-corrupt-classification.md` / `2-transition-error-holds-classification-only.md`）は**旧文面を現行として引用していた箇所を引用と読める形に直したもの**で、決定そのものは変えていない（plan.md「含まれないもの」）。新規ファイルは1つも現れない。
  - 手順2: **ヒット0件**。実装前は決定節29行と影響節47行の2件ある。
  - 手順3: 決定節に `Failed { cause: NotifyFailureCause }` がある。
  - 手順4: 2ファイルとも、`Parse { resolved_from }` の新設で前提が変わったことが読める（前置に残るのは `Io { message }` だけであること、スキーマ違反の案内にパスが出ない旨のトレードオフが解消済みであること）。
  - 手順5: `location` を論理位置に限る決定そのものは有効なまま残っている（ADR を無効化するのではなく前提の変化を書き足す形）。
  - 手順6: タイトルに `(置き換え済み)`、ステータスに `置き換え済み(Issue #9)` と置き換え後の形があり、**決定節と代替案節は決定時点の文面のまま**で、どの代替案を採ったか・退けた理由がなぜ成立しなくなったかは影響節が述べている。
- **確認ポイント:** 手順2 が0件であることが、`.adr/` の同じ節に `Failed { detail }` と `Failed { cause }` が並ばないことの機械的な保証。手順5・手順6 が見ているのは `.thread/9/adr.md` ADR-006 の規則で、**置き換え**（決定節は当時のまま・Status と影響節が置き換えを述べる）と**改訂**（決定本体が生きているので該当箇所を現行へ書き換える）が Status から区別できること。`.thread/3/adr.md` の未昇格 ADR（`judged` / `AlreadyNotified`）が `.adr/` へ昇格していないこと — これはスコープ外で、手順1 に新規ファイルが現れないことで見る。

### 4. A2 — 名前指定のワークフロー定義が YAML 構文エラーのとき、解決先の絶対パスが**1回だけ**出る

- **対応する受け入れ基準:** AC-A2, AC-A2b
- **目的:** A2 の動機は「`--workflow` を**名前**で指定した場合、利用者が直接書いていない解決先（`<home>/workflows/<name>.yaml`）が案内に出ない」ことの解消。同時に、`resolved_from` を構造として持つようになった以上、自由形式のメッセージへの前置は二重表示になるので外す（`.thread/9/adr.md` ADR-005「構造化フィールドで示せるなら自由形式へ前置しない」）。**パスが出ること**と**1回しか出ないこと**の両方を同じ実行で見る。
- **手順:**
  1. 上の「A2 / C5 の実機確認用フィクスチャ」を実行する。
  2. `pulsen add --workflow broken-syntax --repo "$P9/repo"; echo "exit=$?"`
  3. 出力に現れる `$P9/home/workflows/broken-syntax.yaml` の**出現回数**を数える。

     ```sh
     pulsen add --workflow broken-syntax --repo "$P9/repo" 2>&1 \
       | grep -c "$P9/home/workflows/broken-syntax.yaml"
     ```
  4. `ls "$P9/home/state/tasks/" 2>/dev/null; echo "exit=$?"`
- **期待結果:**
  - 手順2: 非0で終了し、「ワークフロー定義が不正です。」に続いて「YAML 構文エラー: …」「位置: 行 … 列 …」と、**解決先の絶対パス** `/tmp/pulsen-issue9/home/workflows/broken-syntax.yaml` が案内に出る。
  - 手順3: 出現回数が**ちょうど 1**。実装前も1（そのときの1行は原因への前置 `YAML 構文エラー: <絶対パス>: <原因>`）なので、回数だけでは実装前後を区別できない。手順2 の出力で、その1行が独立した案内行「`解決したパス: <絶対パス>`」に変わっていることを併せて見る。`resolved_from` を足しただけで前置を外さないと**2**になる。
  - 手順4: タスクは1件も作られない（`state/tasks/` が存在しないか空）。
- **確認ポイント:** パスが「原因の前置」ではなく**解決先を示す独立した案内行**として出ていること（`NotFound { attempted }` の「解決を試みたパス: …」と同じ見せ方に揃っている）。`位置:` が示すのは YAML の行・列であって解決先パスではないこと — この2つが1行に混ざっていないこと。exit code が非0であること（`echo "exit=$?"`）。

### 5. A2 — スキーマ違反でも解決先が出る／`--workflow` にパスを与えた場合も二重表示しない

- **対応する受け入れ基準:** AC-A2, AC-A2b
- **目的:** `WorkflowParseError` の12種は `location` に**論理位置**（`agents.claude.cmd` 等）しか持たず、実装前はスキーマ違反の案内に対象ファイルが1文字も出なかった（`.adr/1-schema-error-location-is-logical.md` の影響節のトレードオフ）。`Parse { resolved_from }` は12種すべての解決先を1フィールドで示すので、構文エラーだけでなくスキーマ違反でもパスが出る。あわせて、名前ではなくパスで指定した場合に案内が二重にならないことを見る。
- **手順:**
  1. `pulsen add --workflow broken-schema --repo "$P9/repo"; echo "exit=$?"`
  2. 解決先パスの出現回数を数える。

     ```sh
     pulsen add --workflow broken-schema --repo "$P9/repo" 2>&1 \
       | grep -c "$P9/home/workflows/broken-schema.yaml"
     ```
  3. パス指定（絶対パス）でも同じことを見る。

     ```sh
     pulsen add --workflow "$P9/home/workflows/broken-syntax.yaml" --repo "$P9/repo" 2>&1 \
       | grep -c "$P9/home/workflows/broken-syntax.yaml"
     ```
  4. 正常系が壊れていないことを見る。

     ```sh
     pulsen add --workflow ok --repo "$P9/repo"; echo "exit=$?"
     ```
- **期待結果:**
  - 手順1: 非0で終了し、「`(トップレベル)` にスキーマ外のキー `typo_key` があります。」に加えて解決先の絶対パスが出る。実装前はパスが1文字も出ない。
  - 手順2: 出現回数が**ちょうど 1**。
  - 手順3: 出現回数が**ちょうど 1**（利用者が自分で書いたパスでも、案内に2回は出さない）。
  - 手順4: exit code 0 で、タスクID・ワークフロー名 `ok`・解決先パス `/tmp/pulsen-issue9/home/workflows/ok.yaml` が表示される。
- **確認ポイント:** `location`（論理位置）が絶対パスに置き換わっていないこと — A2 が変えるのは「対象ファイルをどこが持つか」であって、`location` の契約（`.adr/1-schema-error-location-is-logical.md` の決定本体）は変えない。手順3 は「前置を外したせいでパス指定の経路から案内が消える」退行を落とすためのもの（`resolved_from` は名前でもパスでも同じフィールドで出る）。

### 6. A2 — 解決先の提示が構造化フィールドに一本化されている（コード側）

- **対応する受け入れ基準:** AC-A2b, AC-Z3
- **目的:** 前置を「たまたま今は二重に見えないだけ」で残していないことを、呼び出し元の数で機械的に見る。前置が残ってよいのは、構造化フィールドを持たない `WorkflowLoadError::Io { message }` の経路だけ。
- **手順:**
  1. `grep -n 'at(' crates/pulsen/src/adapter/workflow_store.rs`
  2. `grep -n -A 6 'fn at' crates/pulsen/src/adapter/workflow_store.rs`
  3. `grep -rn 'WorkflowLoadError::Parse' crates/`
  4. `grep -n -A 4 'WorkflowLoadError::Parse' crates/pulsen/src/cli/render.rs`
  5. `grep -n 'expected_path_for_name' crates/pulsen-conformance/HOOKS.md crates/pulsen-conformance/src/workflow_store.rs`
- **期待結果:**
  - 手順1: `at(` の**呼び出し元は `read_error`（`Io` の枝）の1箇所だけ**。`parse_document` の失敗アーム（実装前は74行）からは消えている。
  - 手順2: `at()` の doc から `WorkflowParseError::YamlSyntax` の記述が外れ、利用者が `Io` だけであることが読める。
  - 手順3: 構築側が `Parse { error, resolved_from }` の形になっており、タプル記法 `Parse(...)` が残っていない。
  - 手順4: `render.rs` の `Parse` アームが `resolved_from` を案内に出している。
  - 手順5: `HOOKS.md` の `TC-port-workflow-store-017` の「組み立て手段」欄に `expected_path_for_name` が現れ、「そのフックがあれば `resolved_from` が期待値と一致することも観測し、無ければその1主張だけを飛ばす。`YamlSyntax` の分類と位置、`message` に解決先を前置しないこと、`resolved_from` が絶対パスであることは常に観測する」と読める。`tc_port_workflow_store_017` は `harness.expected_path_for_name("wf")` をケース先頭で取り、**期待値との一致だけを `if let Some(..)` で条件化**している（ケース全体を `require!` で落とすと、フックを持たないハーネスでは `YamlSyntax` の適合確認まで一緒に消える）。前置しないことはポートが返した `resolved_from` と突き合わせれば書けるので、フックを要さず無条件に走る。
- **確認ポイント:** 手順1 が1箇所であることが、確認項目4・5 の「1回だけ」を構造として保証する条件。`Io { message }` の前置を**外さない**こと — `Io` は解決先を構造として持たず、CLI 側も解決先を知らないため、前置以外に案内する場所が無い（変種ごとの個別判断ではなく「構造化フィールドで示せるなら前置しない」という規則の帰結）。`HOOKS.md` の件数（冒頭の196行・`## WorkflowStore（31行 …）`）が動いていないこと — 変わるのは組み立て手段のセル1つだけ。

### 7. C5 — 通知コマンドの3つの失敗原因が、区別できるメッセージとして出る

- **対応する受け入れ基準:** AC-C5, AC-C5b
- **目的:** `NotifyOutcome::Failed { detail: String }`（完成文言をドメインが持つ）を `Failed { cause: NotifyFailureCause }`（分類）に改める。**非0終了 / timeout / 起動不能の3つが利用者から区別できること**が、分類が意味を持つことの実地の証拠になる。3つとも `notified_at` を書かないので、同じタスクを使い回して1原因ずつ確かめられる（at-least-once）。
- **手順:**
  1. 通知が**非0で終わる**形に差し替える。

     ```sh
     cat > "$PULSEN_HOME/config.yaml" <<'EOF'
     agents:
       shell:
         cmd: ["sh", "-c", "{input}"]
     notify_cmd: ["sh", "-c", "exit 3"]
     EOF
     ```
  2. 凍結するタスクを登録し、stopped まで進める。

     ```sh
     pulsen add --workflow fail0 --repo "$P9/repo"     # 表示されたタスクIDを T9 に控える
     export T9=<task-id>
     pulsen tick; echo "exit=$?"    # 起動
     sleep 3
     pulsen tick; echo "exit=$?"    # 起動確認
     sleep 3
     pulsen tick; echo "exit=$?"    # 判定 → 凍結 → 通知（ここで通知が失敗する）
     ```
  3. `jq '.execution' "$PULSEN_HOME/state/tasks/$T9.json"`
  4. 通知を**起動できない**形に差し替えて、もう1 tick 回す。

     ```sh
     sed -i.bak 's|^notify_cmd:.*|notify_cmd: ["/no/such/pulsen-notify"]|' "$PULSEN_HOME/config.yaml"
     pulsen tick; echo "exit=$?"
     ```
  5. 通知が**期限内に終わらない**形に差し替えて、もう1 tick 回す（組み込みの `NOTIFY_TIMEOUT` は60秒なので、この tick だけ約60秒かかる）。

     ```sh
     sed -i.bak 's|^notify_cmd:.*|notify_cmd: ["sh", "-c", "sleep 120"]|' "$PULSEN_HOME/config.yaml"
     time pulsen tick; echo "exit=$?"
     ```
  6. 通知を成功する形に戻し、もう1 tick 回す。

     ```sh
     cp "$P9/config.bak" "$PULSEN_HOME/config.yaml"; rm -f "$PULSEN_HOME/config.yaml.bak"
     pulsen tick; echo "exit=$?"
     jq '.execution' "$PULSEN_HOME/state/tasks/$T9.json"
     cat "$P9/notify.log"
     ```
- **期待結果:**
  - 手順2 の3回目・手順4・手順5: いずれも **exit code 0**（通知の失敗は tick 全体を落とさない）。サマリーの報告に**「スキップ」**見出しが出て、`<T9>: 凍結を通知できません(…)。次の tick が再通知します` が1件並ぶ（`TickIssue::NotifyFailed` はタスクファイルに何も残さない結末なので「失敗を記録」ではない）。
  - 括弧内の文言が3回とも**互いに異なり、原因が読める**:
    - 手順2 の3回目（非0終了）: 終了コード **3** が読める。
    - 手順4（起動不能）: 起動できなかったこと（`/no/such/pulsen-notify` を起動できない旨）が読める。
    - 手順5（timeout）: 期限 **60** 秒を超えたことが読める。`time` の実測が60秒強。
  - 手順3・手順4・手順5 のいずれの時点でも `.execution` が `{"state":"stopped","reason":"retry_limit_exceeded","notified_at":null}`。`notified_at` は `null` のまま。
  - 手順2 の3回目のサマリーで「凍結」に `<T9>` が現れ、「通知」には現れない。
  - 手順6: `notify.log` に `stopped <T9> fail0 work` の行が**ちょうど1行**追加され、`.execution.notified_at` に時刻が入る。サマリーの「通知」に `<T9>` が現れる（「凍結」には現れない）。
- **確認ポイント:** 3つのメッセージが**同一文言に潰れていない**こと — 潰れていれば分類が表示に届いておらず、`cause` を足した意味が無い。timeout の秒数が `NotificationService::NOTIFY_TIMEOUT` から読まれていること（表示側が定数を読む。`TimedOut` がフィールドを持たない理由）。「凍結を通知できません」と括弧内の原因が**二重に同じことを述べていない**こと（ADR-005 の趣旨をユースケース層側でも守る — 分類で示せる内容を自由形式でもう一度書かない）。手順5 の tick が60秒ブロックすること自体は仕様（ハングした通知コマンドが排他ロックを保持したまま tick を塞ぐことを防ぐ組み込み timeout）。**手順6 の復元を飛ばさないこと** — `notify_cmd` が失敗する形のまま残ると以降の確認が毎回通知失敗を出す。

### 8. C5 — 通知失敗の完成文言がドメインから消えている（コード側）

- **対応する受け入れ基準:** AC-C5, AC-Z3
- **目的:** 「表示専用のエラーは分類だけを持ち、完成文言は CLI 層が組み立てる」という規則（`.adr/2-transition-error-holds-classification-only.md`）が効いていることを、ドメイン側に文言が1つも残っていないことで見る。
- **手順:**
  1. `grep -rnE '通知コマンドが終了コード|秒のうちに終了しませんでした|通知コマンドを起動できませんでした' crates/pulsen-domain/`
  2. `grep -rn 'detail' crates/pulsen-domain/src/execution/notification.rs`
  3. `grep -n 'NOTIFY_TIMEOUT' crates/pulsen-domain/src/execution/notification.rs`
  4. `grep -n 'pub use notification' crates/pulsen-domain/src/execution/mod.rs`
  5. `grep -n -A 8 'NotifyFailed' crates/pulsen/src/cli/render.rs`
- **期待結果:**
  - 手順1: **ヒット0件**（実装前は `notification.rs` の43 / 47 / 52行の3件。いずれも `format!` の中）。
  - 手順2: **ヒット0件**（実装前は9件。テストヘルパー `detail_of` の廃止まで含めて0になる）。
  - 手順3: `NOTIFY_TIMEOUT` の定数と doc コメントは**残っている**（timeout を置く理由を述べる why であり、C5 が消す対象ではない）。
  - 手順4: `pub use notification::{NotificationService, NotifyFailureCause, NotifyOutcome};` になっている（`mod notification;` は非公開なので、広げないと `application` / `cli` から型を名指しできずコンパイルが通らない）。
  - 手順5: `cli/render.rs` が `cause` の網羅 `match` から文言を組み立てており、`TimedOut` の秒数を `NotificationService::NOTIFY_TIMEOUT` から読んでいる。見出しの振り分け（`issue_outcome`）は `NotifyFailed { .. }` で受けたまま。
- **確認ポイント:** 手順1 に `grep -rn '通知コマンドが' crates/pulsen-domain/` を**使わない**こと — `notification.rs` の `NOTIFY_TIMEOUT` の doc コメントを拾い、正しく実装しても1件残る（この doc は `.adr/2026-08-11-notify-cmd-timeout.md` 由来の why）。`FailedToStart` の文言は「通知コマンド**を**起動できませんでした」なので、助詞違いで取りこぼさないこと。`NotifyOutcome` を4変種に平坦化していないこと（`Delivered` / `Failed` の2分岐が at-least-once の規則そのもの）。

### 9. `crates/` の変更が A2 / C5 と、その2件が偽にした記述の訂正の14ファイルに閉じている

- **対応する受け入れ基準:** AC-Z3
- **目的:** 23件は「実装が正しい」と決着させた件なので、`crates/` に**振る舞い**の差分が出てはならない。範囲を機械的に確認できる形にしておく。基準はファイル数の維持ではなく「A2 / C5 の外へ振る舞いを波及させない」ことなので、doc コメントとテストの主張の訂正は数に含める。
- **手順:**
  1. コミット済みと未コミットの和集合を見る。

     ```sh
     ( git diff --name-only origin/main...HEAD -- crates/; git diff --name-only -- crates/ ) | sort -u
     ```
  2. `git diff --stat origin/main -- crates/pulsen-conformance/HOOKS.md`
  3. `git diff --name-only origin/main -- crates/pulsen/tests/`
  4. `git diff --name-only origin/main -- spec/ .adr/ .thread/`
- **期待結果:**
  - 手順1: **次の14ファイルだけ**が現れる。
    - A2（5）: `crates/pulsen-domain/src/definition/port.rs` / `crates/pulsen/src/adapter/workflow_store.rs` / `crates/pulsen-conformance/src/workflow_store.rs` / `crates/pulsen-conformance/HOOKS.md` / （`crates/pulsen/src/cli/render.rs` は C5 と共有）
    - C5（5）: `crates/pulsen-domain/src/execution/notification.rs` / `crates/pulsen-domain/src/execution/mod.rs` / `crates/pulsen/src/application/tick/mod.rs` / `crates/pulsen/src/application/tick/notify.rs` / `crates/pulsen/src/cli/render.rs`
    - A2 / C5 が偽にした記述の訂正（5）: `crates/pulsen-conformance/src/lib.rs`（`expected_path_for_name` の doc に 017 を足す）/ `crates/pulsen/src/adapter/task_file.rs`（doc の `TransitionError::InvariantViolated` は B8 で消えた変種名）/ `crates/pulsen/tests/cli_add_error.rs`（TC-022 の期待語に解決先パスを足す）/ `crates/pulsen/src/application/run_wrapper.rs` / `crates/pulsen/src/cli/wrapper.rs`（B3。doc の「何も書き残さず終わる」は starttime が先に書かれるため偽）
  - 手順2: `HOOKS.md` の差分は**1行の変更のみ**（`TC-port-workflow-store-017` の「組み立て手段」のセル）。冒頭の合計196行と `## RunStore` 節の件数は動いていない。
  - 手順3: `crates/pulsen/tests/` の差分は **`cli_add_error.rs` の1ファイルだけ**。
  - 手順4: `spec/` 側の差分が25件の追従に限られ、ついでのリファクタリング・Markdown の整形が混ざっていない。
- **確認ポイント:** 手順3 の `cli_add_error.rs` の差分が **TC-022 の期待語に解決先パスを足した1件に限る**こと（`reject` / `reject_resolved` のヘルパー整理を含む。他のテストの期待語は1つも変わらない）。結合テスト `tick_notify.rs` / `tick_scan.rs` は `TickIssue::NotifyFailed { .. }` で受けているため差分が出ない。A2 は前置を外して案内の形を変えるので、**受け入れテストが変わること自体は退行ではない** — 変えてよいのは「A2 が新たに守るべきにした形」を主張に足す方向だけで、既存の主張を弱める向きの変更（期待語を減らす・条件を緩める）が混ざっていないことを差分で見る。`crates/pulsen-conformance/src/run_store.rs` に差分が無いこと — `TC-port-run-store-035` の適合スイート実装は後続スライスに残す（plan.md スコープ）。

### 10. ツールチェーンが通る

- **対応する受け入れ基準:** AC-Z5
- **目的:** A2 / C5 の実装変更に既存テストが追従していることを、CI と同じ形の4コマンドで確認する。`cli/render.rs` を A2 と C5 の両方が触るため、片方だけ直して他方の `match` が壊れたまま残る取りこぼしはここでしか捕まらない。
- **手順:**
  1. `cargo fmt --all --check`
  2. `cargo build --workspace --locked`
  3. `cargo test --workspace --locked --no-fail-fast -- --nocapture`
  4. `cargo clippy --workspace --all-targets --locked -- -D warnings`
  5. 手順3 の出力から `SKIP ` を含む行を拾い、変更前と同じ集合であることを見る。
- **期待結果:**
  - 手順1〜4: 4つとも緑（手順1 は差分なしで exit 0、手順4 は警告0）。
  - 手順3: 全テストが緑。`crates/pulsen-domain/src/execution/notification.rs` のユニットテスト `通知の失敗の3つの原因は分類として判別できる` が、完成文言の差ではなく `NotifyFailureCause` の変種の判別で通る。A2 / C5 が新たに守るべきにした形を主張する `cli::render::tests::解釈できない定義は読んだパスを一度だけ添えて案内される` と `cli::render::tests::通知できなかった原因は3つが区別できる形で示される` の2件も走っている。
  - 手順5: `SKIP` 行の集合が変わっていない。`tc_port_workflow_store_017` が新たにスキップされない（期待値との一致だけを `expected_path_for_name` の有無で条件化しており、ケース自体は常に `Ran` を返すため。`allowed_skips` の変更も要らない）。
- **確認ポイント:** 手順4 の `--all-targets` を省かないこと（テストターゲットもここで初めて lint に掛かる）。ローカルの rustfmt は nixpkgs 同梱で CI の現行 stable と版が違うため、手順1 が緑でも CI の fmt ジョブが赤になりうる。その場合の解消は CI の stable で `cargo fmt --all` を掛け直した差分をコミットすることであり、`rustfmt.toml` での抑止ではない。

---

## エッジケース・異常系

### 1. A2 — 名前解決に失敗した場合（`NotFound`）の案内が変わっていない

- **目的:** `NotFound { attempted }` は実装前から解決先を構造として持っており、A2 の対象外。`Parse` に `resolved_from` を足した際に、こちらの見せ方まで巻き込んで変えていないことを確認する。
- **手順:**
  1. `pulsen add --workflow no-such-workflow --repo "$P9/repo" 2>&1; echo "exit=$?"`
  2. `pulsen add --workflow no-such-workflow --repo "$P9/repo" 2>&1 | grep -c "$P9/home/workflows/no-such-workflow.yaml"`
- **期待結果:** 非0で終了し、「ワークフロー定義が見つかりません。」と「解決を試みたパス: /tmp/pulsen-issue9/home/workflows/no-such-workflow.yaml」が出る。パスの出現回数は**ちょうど 1**。

### 2. A2 — 読み取り自体が失敗した場合（`Io`）は前置が残る

- **目的:** `Io { message }` は解決先を構造として持たず、CLI 側も解決先を知らないため、自由形式のメッセージへの前置が唯一の案内手段として**残る**。前置を一律に外していないことを確認する（ADR-005 が前置を残す唯一の変種として名指ししている）。
- **手順:**
  1. 読み取れない定義を作る（unix のみ。Windows では POSIX の権限操作が効かないためこの項目はスキップする）。

     ```sh
     cp "$P9/home/workflows/ok.yaml" "$P9/home/workflows/denied.yaml"
     chmod 000 "$P9/home/workflows/denied.yaml"
     ```
  2. `pulsen add --workflow denied --repo "$P9/repo" 2>&1; echo "exit=$?"`
  3. `chmod 644 "$P9/home/workflows/denied.yaml"` で戻す。
- **期待結果:** 手順2 が非0で終了し、「ワークフロー定義を読み込めません。」と、`原因:` に**解決先パスが前置された**メッセージが出る。パスの出現回数は1（前置が唯一の案内なので二重にはならない）。手順3 のあと `pulsen add --workflow denied --repo "$P9/repo"` が成功する。

### 3. C5 — `notify_cmd` 未定義なら報告そのものが出ない

- **目的:** `Delivery::NotConfigured` は通知も `notified_at` の記録も行わず、**報告も積まない**（「通知した」という虚偽の記録も、失敗の報告も作らない）。分類化で報告の形を変えた際に、未定義の経路を巻き込んでいないことを確認する。
- **手順:**
  1. `grep -v '^notify_cmd:' "$P9/config.bak" > "$PULSEN_HOME/config.yaml"; grep -c notify_cmd "$PULSEN_HOME/config.yaml"`
  2. `pulsen add --workflow fail0 --repo "$P9/repo"` → タスクIDを `T9B` に控える → `pulsen tick` を2〜3秒間隔で3回。
  3. `jq '.execution' "$PULSEN_HOME/state/tasks/$T9B.json"`; `wc -l < "$P9/notify.log"`
  4. `cp "$P9/config.bak" "$PULSEN_HOME/config.yaml"` → `pulsen tick` → 同じ2つを再確認する。
- **期待結果:**
  - 手順1: `notify_cmd` の行数が0。
  - 手順2 の3回目: exit code 0。サマリーの「凍結」に `<T9B>` が現れ、報告（`errors`）には `NotifyFailed` が**1件も出ない**。
  - 手順3: `.execution.notified_at` が `null`。`notify.log` の行数が増えていない。
  - 手順4: 次の tick が未通知の stopped を検出して catch-up し、`notify.log` に1行増え、`notified_at` に時刻が入る。サマリーの「通知」に `<T9B>` が現れる（「凍結」には現れない）。

### 4. A3 — 訂正した TC-46 の手順が実際に登録失敗になる

- **目的:** A3 は spec だけの変更だが、`spec/manual-tests/setup.md` TC-46 は**手順自体が誤っている**（`$WORK/.yaml` では `Path::file_stem` が `.yaml` を返して登録が成功してしまう）。訂正後の手順が本当に到達するかは、1回だけ実機で確かめられる。
- **手順:**
  1. **`workflow:` キーを持たない**定義を用意する（キーがあるとそれが表示名になり、ファイル名由来の判定に到達しない。`crates/pulsen-domain/src/definition/reference.rs` の `display_name`）。

     ```sh
     cat > "$P9/noname.yaml" <<'EOF'
     agent: shell
     initial: work
     statuses:
       work:
         prompt: "true"
         next: done
       done:
         run: cleanup
     EOF
     ```
  2. 誤っていた側（実装前の TC-46 手順1）を再現する。

     ```sh
     cp "$P9/noname.yaml" "$P9/.yaml"
     pulsen add --workflow "$P9/.yaml" --repo "$P9/repo"; echo "exit=$?"
     ```
  3. 訂正後の手順（クォート付き。先頭が空白のファイル名）を実行する。

     ```sh
     cp "$P9/noname.yaml" "$P9/ .yaml"
     pulsen add --workflow "$P9/ .yaml" --repo "$P9/repo"; echo "exit=$?"
     ```
  4. `grep -n ' .yaml' spec/manual-tests/setup.md` で TC-46 の手順1 とコマンド例を読む。
- **期待結果:**
  - 手順2: **exit code 0 で登録が成功してしまう**（`file_stem(".yaml")` が `.yaml` を返すため表示名 `.yaml` が決まる）。これが「到達しない手順」であったことの実地の裏付け。
  - 手順3: 非0で終了し、表示名を決められない旨で拒否される（語幹が空白のみ ` ` は `WorkflowName::parse` が非空・前後空白なしで拒否する）。
  - 手順4: TC-46 の目的文が「ファイル名の語幹が空白のみになるパス指定」になっており、手順1 のファイル名が ` .yaml`、コマンド例が `pulsen add --workflow "$WORK/ .yaml" --repo "$REPO"` の形（引数がクォートされ、シェルが先頭の空白で分割しない）になっている。

### 5. 検証用フィクスチャの後片付け

- **目的:** 一時的に作った状態（読み取り不可の定義・失敗する `notify_cmd`・`/tmp/pulsen-issue9`）が残ると、以降の確認や自動テストが本 Issue と無関係な失敗を出す。
- **手順:**
  1. `chmod 644 "$P9/home/workflows/denied.yaml" 2>/dev/null`（エッジケース2 を実行した場合）
  2. `rm -rf /tmp/pulsen-issue9`
  3. `git status --short` と `git diff --stat`
  4. `cargo test --workspace --locked --no-fail-fast -- --nocapture`
- **期待結果:** 手順3 に `/tmp` 側の痕跡が現れず（そもそもリポジトリ外）、差分が確認項目9 の範囲（`spec/` / `crates/` の14ファイル / `.adr/` の3ファイル / `.thread/9/`）に収まっている。手順4 が緑。

---

## 既存機能への影響確認

- **`pulsen add` の正常系（`register_task.rs` / `cli_add_normal.rs` の受け入れテスト）:** A2 は `WorkflowLoadError::Parse` の形だけを変え、`NotFound` / `Io` と成功経路には触らない。`crates/pulsen/src/application/register_task.rs` は `WorkflowLoadError` を包むだけ、`crates/pulsen/tests/register_task.rs` と `crates/pulsen-conformance/src/doubles/` は `NotFound` しか使わないため変更を要さない。確認項目5 手順4（正常系の `pulsen add`）と確認項目10 手順3 で見る。

- **`pulsen add` のエラー表示に依存した受け入れテスト:** `crates/pulsen/tests/cli_add_error.rs` の TC-022 は変更前 `["YAML 構文エラー", "位置:", "行"]` の3語しか見ておらず、前置を外しても壊れない。A2 ではそこへ解決先パスを4語目として足す（`assert_reports` は `stderr.contains` の部分一致なので、出現**回数**はここでは見ない — 回数は `cli/render.rs` のユニットテストが固定する）。`tc_task_register_task_021`（読み取り不可）は `definition.display()` を期待に含むが、これは `Io` の経路で前置が残る側なので影響しない。確認項目10 手順3 で見る。

- **`WorkflowStore` の適合スイートとスキップ予算:** 変更前の `crates/pulsen-conformance/src/workflow_store.rs` は `message` の非空と `location` の存在しか主張しておらず、前置された絶対パスを見ている主張は1つも無い。`tc_port_workflow_store_017` に `resolved_from` の一致・絶対性と「`message` に前置しない」の主張を足すが、フックを要する一致だけを `if let Some(..)` で条件化してケース内に閉じ、`require!` は使わないため、ケースの結末は常に `Ran` のままでスキップは発生せず、`allowed_skips` の変更も要らない。確認項目10 手順5 で `SKIP` 集合が変わっていないことを見る。

- **`UnknownKey` / `InvalidValue` の `location`:** 適合スイートが値の一致で固定している論理位置（`agents.claude.cmd` 等）は変えない。A2 が変えるのは「対象ファイルをどこが持つか」だけで、`.adr/1-schema-error-location-is-logical.md` の決定本体（`location` は論理位置に限る）は有効なまま。確認項目3 手順5 と確認項目5 の確認ポイントで見る。

- **tick の通知経路（`tick_notify.rs` / `tick_scan.rs`）:** 結合テストは `TickIssue::NotifyFailed { .. }` で受けているため変更を要さない。通知の順序契約（「stopped を書く → notify_cmd → 成功時だけ `mark_notified`」）と at-least-once は C5 で変えない — 確認項目7 手順6 で、失敗が続いた後に成功した tick で初めて `notified_at` が入ることを見る。

- **AbortTask の通知の報告先:** 共通手続き notify は Tick と AbortTask が共有しており、AbortTask 側の報告先は `notify_warning: Option<NotifyFailureCause>`（分類だけを持ち、完成文言は表示層が組み立てる。Tick 側の `errors` に積む `NotifyFailed { task_id, cause }` とは別の受け皿）。AbortTask は未実装（`ls` / `show` / `abort` は #5）なので `cargo test` では検出されないため、**spec の記述として**確認する — 共通手順が固定するのは「`interpret_notify_completion` を経由し、`Delivered` のときだけ `mark_notified` → `save`」までで、AbortTask に存在しない `errors` を要求していないこと（AC-C5b）。`grep -n 'notify_warning' spec/usecases/execution.md` が `Option<NotifyFailureCause>` を返すことを見る。

- **tick サマリーの見出し（B11/C7）:** 報告の4見出し（失敗を記録 / 起動の結果が未確定 / スキップ / 後始末が残っている）と `issue_outcome` の振り分けは**実装済みで正しい**側なので `crates/` に差分は出ない。B11 は spec の追従のみ。確認項目7 で、通知失敗が「スキップ」見出しに出る現行の振る舞いが変わっていないことを併せて見る。

- **`RunStore` の write 系（B6）:** 契約を spec に足すだけで実装は既にこれを満たしている。`crates/pulsen-conformance/src/run_store.rs` に `TC-port-run-store-035` を実装しないのは意図した判断（plan.md スコープ「含まれないもの」）。確認項目9 手順1 で `run_store.rs` に差分が無いことを見る。

- **`crates/pulsen-conformance/HOOKS.md` の件数:** ステップ9 が `spec/testcases/ports/run-store.md` に1行足すが、`HOOKS.md` が数えるのは「これまでのスライスで扱った行」であり、適合スイート実装を伴わない新規行はまだ数えない。冒頭の196行と `## RunStore` 節は据え置き。確認項目9 手順2 で差分が1行に収まっていることを見る。
