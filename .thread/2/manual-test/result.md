# 動作確認結果 — Issue #2

**実行日:** 2026-08-13
**対象:** PR #11 / ブランチ `issue/2/tick-agent-run-launch` / コミット `cce44f3`
**環境:** macOS 26.4.1 (Darwin 25.4.0, build 25E253) / git 2.55.0 / cargo 1.97.0 (c980f4866 2026-06-30)

## サマリー

- 確認項目: 10件（PASS 10 / FAIL 0 / SKIP 0）
- エッジケース・異常系: 9件（PASS 9 / FAIL 0 / SKIP 0）
- 既存機能への影響: 6件（PASS 6 / FAIL 0 / SKIP 0）

FAIL は無い。testing.md の手順そのものに2箇所の誤りがあり、うち1件は手順どおりでは実行不能だったため代替手順で目的を裏付けた（後述「testing.md 自体の誤り」）。

## 確認環境

ビルドと自動テストの状態（AC-1 の前提）。

```
$ cargo build && cargo build --examples
Finished `dev` profile

$ cargo test | grep '^test result' | awk '{p+=$4; f+=$6} END {print p, f}'
passed=685 failed=0

$ cargo clippy --all-targets -- -D warnings
clippy_exit=0

$ cargo fmt --check
fmt_exit=0
```

AC-1 の OS 依存分岐の隔離:

```
$ grep -rnE 'cfg\([^)]*\b(unix|windows|target_os|target_family)\b' crates/*/src/
crates/pulsen-conformance/src/lib.rs:259:#[cfg(unix)]
crates/pulsen-conformance/src/lib.rs:289:#[cfg(not(unix))]
crates/pulsen/src/adapter/task_repository.rs:278:#[cfg(all(test, unix))]
crates/pulsen/src/adapter/process.rs:212,219,232,242,248,445,692,857
crates/pulsen/src/util/atomic.rs:71,78
```

`crates/pulsen-domain/` は0件。`crates/pulsen/src/` 側は `util/atomic.rs` / `adapter/process.rs` / `adapter/task_repository.rs` の3ファイルのみで、testing.md「既存機能への影響確認」の更新後の期待値どおり。

AC-6 の未実装メソッド不在:

```
$ grep -nE 'fn (attempt_exists|list_runs|delete_attempt|remove_task_dir_if_empty|starttime_of|kill|try_kill_remnants|remove)\(' crates/pulsen-domain/src/execution/port.rs
exit=1  （1件もヒットなし）
```

フィクスチャA（`/tmp/pulsen-test/`）とフィクスチャB（`$HOME/pulsen-manual-test`）は testing.md の記載どおりに作成した。実運用ホーム `~/.pulsen` は確認の開始時点から存在せず、終了後も存在しない。

## 確認項目

### 1. タスク0件での tick と `state/tasks/` 未作成での tick

- **判定:** PASS
- **実行したコマンド:**

  ```sh
  ls "$SETUP_HOME/state" 2>/dev/null; echo $?
  pulsen tick --home "$SETUP_HOME"; echo $?
  ls -a "$SETUP_HOME/state/"; ls "$SETUP_HOME/state/tasks/"
  ls "$SETUP_HOME/worktrees/" "$SETUP_HOME/state/runs/" 2>/dev/null; echo $?
  pulsen tick --home "$SETUP_HOME"; echo $?
  ```

- **実際の出力:**

  ```
  --- step1: ls state
  exit=2
  --- step2: tick
  処理対象のタスクはありませんでした。
  exit=0
  --- step3: ls -a state / state/tasks
  lock
  (tasks:)
  --- step4: worktrees / runs
  exit=2
  --- step5: tick again
  処理対象のタスクはありませんでした。
  exit=0
  ```

- **期待との差:** なし。tick が作ったのは `state/` と `state/lock` だけで `state/tasks/` は空、`worktrees/` と `state/runs/` は未作成。表示は「処理対象のタスクはありませんでした。」の1行のみで `launched` / `frozen` / `errors` の空フィールドは並ばない（ADR-101）。2回目も同一表示・exit 0。

### 2. 起動フェーズ — worktree・ブランチ・launching 記録・runディレクトリ

- **判定:** PASS
- **実行したコマンド:**

  ```sh
  pulsen add --workflow pipeline --repo /tmp/pulsen-test/repo    # → T3=20260813t043653-3rdmd3ll
  sed -i.orig 's/prompt: "echo planning .*"/prompt: "echo edited-should-not-appear"/' \
    /tmp/pulsen-test/home/workflows/pipeline.yaml
  pulsen tick; echo $?
  cat "$PULSEN_HOME/state/tasks/$T3.json"
  ls "$PULSEN_HOME/worktrees/"; git -C /tmp/pulsen-test/repo branch --list 'pulsen/*'
  git -C /tmp/pulsen-test/repo worktree list
  ls -la "$PULSEN_HOME/state/runs/$T3/attempt-1/"
  ```

- **実際の出力:**

  ```
  === item2 step3: tick ===
  tick を実行しました。
    起動: 20260813t043653-3rdmd3ll
  exit=0

  === item2 step4: task json（抜粋） ===
  "task_status": "queued",
  "execution": { "state": "launching", "recorded_at": "2026-08-13T04:37:03Z" },
  "workspace": {
    "path": "/tmp/pulsen-test/home/worktrees/20260813t043653-3rdmd3ll",
    "branch": "pulsen/20260813t043653-3rdmd3ll"
  },
  "current_attempt": {
    "number": 1,
    "run_dir": "/tmp/pulsen-test/home/state/runs/20260813t043653-3rdmd3ll/attempt-1",
    "process": null
  },
  "counters": { "attempt_count": 0, "judge_attempt_count": 0, "spawn_fail_count": 0 },

  === item2 step5 ===
  -- branches --
  + pulsen/20260813t043653-3rdmd3ll
  -- worktree list --
  /private/tmp/pulsen-test/repo                                    3ac0975 [main]
  /private/tmp/pulsen-test/home/worktrees/20260813t043653-3rdmd3ll dae3dc6 [pulsen/20260813t043653-3rdmd3ll]

  === item2 step6 ===
  exit  pid  starttime  stderr.log  stdout.log

  [pid]       { "pid": 50106, "kill_ident": "-50106" }
  [starttime] { "ident": "Thu Aug 13 04:37:03 2026", "wall": "2026-08-13T04:37:03Z" }
  ```

- **期待との差:** なし。`workspace.path` / `current_attempt.run_dir` はいずれも絶対パス。`kill_ident` は `-50106`（プロセスグループID）で空でも `0` でもない。`starttime.ident` は非空。ブランチは `main`（3ac0975）から作られている。

### 3. spawn確認 — 次 tick での running 取込と猶予内の冪等性

- **判定:** PASS
- **実行したコマンド:**

  ```sh
  # 手順1（項目2 の tick 直後に即座に打つ）
  pulsen tick; echo $?; cat "$PULSEN_HOME/state/tasks/$T3.json"; md5 -q ...
  # 手順4
  ls "$PULSEN_HOME/state/runs/$T3/attempt-1/"
  # ident 一致確認
  jq -r '.ident' .../attempt-1/starttime
  jq -r '.current_attempt.process.starttime.ident' .../tasks/$T3.json
  ```

- **実際の出力:**

  ```
  === item3 step1: immediate 2nd tick ===
  tick を実行しました。
    起動確認: 20260813t043653-3rdmd3ll
  exit=0

  "execution": { "state": "running" },
  "current_attempt": {
    "number": 1,
    "run_dir": ".../attempt-1",
    "process": {
      "pid": 50106,
      "kill_ident": "-50106",
      "starttime": { "ident": "Thu Aug 13 04:37:03 2026", "wall": "2026-08-13T04:37:03Z" }
    }
  },
  "counters": { "attempt_count": 0, "judge_attempt_count": 0, "spawn_fail_count": 0 }

  === step4: attempt-1 の中身 ===
  exit  pid  starttime  stderr.log  stdout.log     ← invalidated なし

  === ident match check ===
  file  : [Thu Aug 13 04:37:03 2026]
  ledger: [Thu Aug 13 04:37:03 2026]
  IDENT MATCH
  ```

- **期待との差:** 手順1 の「猶予内経路（`KeepWaiting`）」は、tick を1秒以内に打った時点で既に `pid` が書かれていたため、この手順では観測できなかった（testing.md 自身が「pid が既にあれば『観測できず』と記録して次へ進む」と指示している経路）。**代替として、`launching` かつ `recorded_at` が現在時刻・run ディレクトリが空（pid 未出現）の状態を人工的に作って tick を打ち、猶予内経路を直接確認した:**

  ```
  === tick within grace (pid absent) ===
  処理対象のタスクはありませんでした。
  exit=0
  md5  before=e223eaf524da58d6e7799cafacedee6f after=e223eaf524da58d6e7799cafacedee6f
  mtime before=1786596281.249755012 after=1786596281.249755012
  "execution": {"state":"launching","recorded_at":"2026-08-13T04:44:41Z"}   ← 変化なし
  attempt-3 の中身: （空。invalidated なし）
  ```

  書き込みが一切発生せず（md5・mtime とも不変）、`invalidated` も作られず、サマリーにも何も出ない。取り込み後も `attempt_count` は 0 のまま。`starttime.ident` は帳簿とファイルで完全一致。期待どおり。

### 4. ラッパーの成果物とスナップショットの有効性

- **判定:** PASS
- **実行したコマンド:**

  ```sh
  until [ -f "$D/exit" ]; do sleep 1; done; ls -la "$D"
  cat "$D/stdout.log"; cat "$D/stderr.log"; cat "$D/exit"; cat "$D/pid"; cat "$D/starttime"
  ls "$PULSEN_HOME/worktrees/$T3/"; git -C "$PULSEN_HOME/worktrees/$T3" log --oneline
  stat -c '%.9Y %n' "$D/starttime" "$D/pid" "$D/exit" "$D/stdout.log" "$D/stderr.log"
  grep -c 'edited-should-not-appear' "$D/stdout.log"
  ```

- **実際の出力:**

  ```
  === stdout.log ===
  planning
  [pulsen/20260813t043653-3rdmd3ll dae3dc6] plan
   1 file changed, 1 insertion(+)
   create mode 100644 plan.txt
  === stderr.log ===
  （空。ファイルは存在する）
  === exit ===
  { "code": 0 }
  === pid / starttime ===
  { "pid": 50106, "kill_ident": "-50106" }
  { "ident": "Thu Aug 13 04:37:03 2026", "wall": "2026-08-13T04:37:03Z" }
  === worktree ===
  plan.txt
  dae3dc6 plan
  3ac0975 init
  === mtime ordering (ns) ===
  1786595823.904120702 starttime
  1786595823.913677343 pid
  1786595824.069710128 stdout.log
  1786595824.071584429 exit
  === edited-should-not-appear ===
  0 （grep_exit=1、ヒットなし）
  ```

- **期待との差:** なし。`starttime` (…823.904) < `pid` (…823.913) < `exit` (…824.071) の順序が成立。`exit` は ADR-080 どおり JSON。stdout に `planning` が出て `edited-should-not-appear` は出ない = 登録時スナップショットが使われている。worktree に `plan.txt` と `plan` コミット1件。

### 5. 同一リポジトリ・2タスクの独立した起動

- **判定:** PASS
- **実行したコマンド:**

  ```sh
  pulsen add --workflow pipeline --repo /tmp/pulsen-test/repo   # ×2 → T4A / T4B
  pulsen tick; echo $?
  ls "$PULSEN_HOME/worktrees/"; git -C /tmp/pulsen-test/repo branch --list 'pulsen/*'
  git -C /tmp/pulsen-test/repo worktree list
  git -C /tmp/pulsen-test/repo merge-base --is-ancestor main "pulsen/$T4A"; echo $?
  git -C /tmp/pulsen-test/repo log --oneline "pulsen/$T4A"
  ```

- **実際の出力:**

  ```
  === tick ===
  tick を実行しました。
    起動: 20260813t043735-ouv5ipru, 20260813t043737-un9kiqbv
  exit=0

  === worktree list ===
  /private/tmp/pulsen-test/home/worktrees/20260813t043735-ouv5ipru faf3411 [pulsen/20260813t043735-ouv5ipru]
  /private/tmp/pulsen-test/home/worktrees/20260813t043737-un9kiqbv 3ac0975 [pulsen/20260813t043737-un9kiqbv]

  T4A: ws=/tmp/pulsen-test/home/worktrees/20260813t043735-ouv5ipru  br=pulsen/20260813t043735-ouv5ipru
       run_dir=/tmp/pulsen-test/home/state/runs/20260813t043735-ouv5ipru/attempt-1
  T4B: ws=/tmp/pulsen-test/home/worktrees/20260813t043737-un9kiqbv  br=pulsen/20260813t043737-un9kiqbv
       run_dir=/tmp/pulsen-test/home/state/runs/20260813t043737-un9kiqbv/attempt-1

  === merge-base check ===
  T4A ancestor exit=0
  T4B ancestor exit=0

  === step7 ===
  -- T4A --  faf3411 plan / 3ac0975 init
  -- T4B --  faf3411 plan / 3ac0975 init
  === exits ===  { "code": 0 } / { "code": 0 }
  ```

- **期待との差:** なし。1回の tick で2件が起動され、worktree・ブランチ・run_dir がすべて分離。両ブランチとも `main` の子孫。並列度制御は行われていない。

### 6. failed からの再起動 — attempt の採番と worktree の引き継ぎ

- **判定:** PASS
- **実行したコマンド:**

  ```sh
  jq '.execution = {"state":"failed"} | .counters.attempt_count = 1' "$S" > new && mv new "$S"
  ls "$PULSEN_HOME/worktrees/$T3/"
  pulsen tick; echo $?
  jq -c '{exec:.execution,ws:.workspace,att:.current_attempt,ctr:.counters}' "$S"
  ls "$PULSEN_HOME/state/runs/$T3/"
  until [ -f ".../attempt-2/exit" ]; do sleep 1; done
  ls "$PULSEN_HOME/worktrees/$T3/"; git -C "$PULSEN_HOME/worktrees/$T3" log --oneline
  ```

- **実際の出力:**

  ```
  === step4: tick ===
  tick を実行しました。
    起動: 20260813t043653-3rdmd3ll
    起動確認: 20260813t043735-ouv5ipru, 20260813t043737-un9kiqbv
  exit=0

  === step5 ===
  {"exec":{"state":"launching","recorded_at":"2026-08-13T04:37:59Z"},
   "ws":{"path":".../worktrees/20260813t043653-3rdmd3ll","branch":"pulsen/20260813t043653-3rdmd3ll"},
   "att":{"number":2,"run_dir":".../runs/20260813t043653-3rdmd3ll/attempt-2","process":null},
   "ctr":{"attempt_count":1,"judge_attempt_count":0,"spawn_fail_count":0}}

  === step6 ===  attempt-1  attempt-2

  === step7 ===
  plan.txt
  dae3dc6 plan
  3ac0975 init
  === attempt-2 exit ===  { "code": 1 }
  === attempt-2 stdout ===
  planning
  On branch pulsen/20260813t043653-3rdmd3ll
  nothing to commit, working tree clean
  ```

- **期待との差:** なし。`current_attempt.number` が 2、`run_dir` が `attempt-2`、`workspace` は不変、`attempt_count` は 1 のまま（起動は消費しない）。worktree の `plan.txt` は残り内容はリセットされていない。tick サマリーに worktree 作成に関する報告は出ていない（`git worktree add` が再実行されていない）。attempt-2 の `exit` が 1 なのは、成果物が残っているため `git commit` が「nothing to commit」で失敗するためで、worktree が引き継がれている証拠そのもの。

### 7. `wrapper` 隠しサブコマンドの単体動作

- **判定:** PASS
- **実行したコマンド:**

  ```sh
  pulsen --help
  pulsen wrapper --help; echo $?
  env PULSEN_HOME=/no/such/home HOME=/no/such/home \
    pulsen wrapper --run-dir "$WRUN" --workspace /tmp/pulsen-test/wrapws \
    -- sh -c 'echo out; echo err 1>&2; exit 7'
  ls -la "$WRUN"; cat "$WRUN/exit"; cat "$WRUN/stdout.log"; cat "$WRUN/stderr.log"
  mkdir -p "$WRUN2" && : > "$WRUN2/invalidated"
  pulsen wrapper --run-dir "$WRUN2" --workspace /tmp/pulsen-test/wrapws \
    -- sh -c 'echo should-not-run > /tmp/pulsen-test/wrapws/ran.txt'
  ls -la "$WRUN2"; ls /tmp/pulsen-test/wrapws/
  ```

- **実際の出力:**

  ```
  === pulsen --help ===
  Commands:
    add   タスクを登録する(実行はしない)
    tick  1回のtickパスを実行する
    help  Print this message or the help of the given subcommand(s)
  Options:
        --home <DIR>  グローバルホームのディレクトリ(...)
    -h, --help / -V, --version
  exit=0

  === pulsen wrapper --help ===
  デタッチ起動されるラッパーモード(内部用)
  Usage: pulsen wrapper [OPTIONS] --run-dir <DIR> --workspace <DIR> [COMMAND]...
  Arguments:
    [COMMAND]...  起動するエージェントのコマンド
  Options:
        --run-dir <DIR>    attempt の run ディレクトリ
        --workspace <DIR>  エージェントの作業ディレクトリになる worktree
  exit=0

  === step4/5 ===
  wrapper exit: 0
  exit  pid  starttime  stderr.log  stdout.log
  [exit]      { "code": 7 }
  [stdout]    out
  [stderr]    err
  [pid]       { "pid": 52285, "kill_ident": "-52282" }
  [starttime] { "ident": "Thu Aug 13 04:38:28 2026", "wall": "2026-08-13T04:38:28Z" }

  === step6 ===
  wrapper exit: 0
  --- WRUN2 ---  invalidated  pid  starttime      ← exit なし
  --- wrapws ---  （空。ran.txt なし）
  ```

- **期待との差:** なし。`wrapper` は `pulsen --help` に現れないが到達できる。存在しないホーム・`HOME` の差し替え下でも動作し、run_dir に実在しないタスクIDを指定しても動く（帳簿を読まない）。ラッパー自身の標準出力には何も出ない。無効化マーカーがあるときは `starttime` / `pid` だけ書いてエージェントを起動しない。

### 8. 外部スケジューラー(cron)からの tick と任意の作業ディレクトリからの起動

- **判定:** PASS
- **実行したコマンド:**

  ```sh
  which pulsen
  # crontab -e は対話式のため、等価な非対話手段で登録
  printf '* * * * * .../target/debug/pulsen tick --home <SETUP_HOME> >> <SETUP_HOME>/cron.log 2>&1\n' | crontab -
  crontab -l
  pulsen add --workflow implement --repo "$SETUP_REPO" --home "$SETUP_HOME"   # → T6
  （約4分待機）
  cat "$SETUP_HOME/cron.log"
  cat "$SETUP_HOME/state/tasks/$T6.json"
  ls "$SETUP_HOME/worktrees/"; git -C "$SETUP_REPO" branch --list 'pulsen/*'
  ls -R "$SETUP_HOME/state/runs/$T6/"
  crontab -l | grep -v 'pulsen tick' | crontab -
  # 追加: 最小環境・任意 cwd からの起動
  cd /tmp && env -i PATH=/usr/bin:/bin HOME="$HOME" .../pulsen tick --home "$SETUP_HOME"
  ```

- **実際の出力:**

  ```
  === which pulsen ===
  /Users/hikaru/github.com/tuanemuy/pulsen/target/debug/pulsen

  === crontab -l ===
  * * * * * /Users/hikaru/github.com/tuanemuy/pulsen/target/debug/pulsen tick \
    --home /Users/hikaru/pulsen-manual-test >> /Users/hikaru/pulsen-manual-test/cron.log 2>&1

  === cron.log ===
  tick を実行しました。
    起動: 20260813t043850-2vogiorg
  tick を実行しました。
    起動確認: 20260813t043850-2vogiorg
  処理対象のタスクはありませんでした。

  === task json ===
  {"exec":{"state":"running"},
   "ws":{"path":"/Users/hikaru/pulsen-manual-test/worktrees/20260813t043850-2vogiorg",
         "branch":"pulsen/20260813t043850-2vogiorg"},
   "att":{"number":1,"run_dir":"/Users/hikaru/pulsen-manual-test/state/runs/20260813t043850-2vogiorg/attempt-1",
          "process":{"pid":52935,"kill_ident":"-52935",
                     "starttime":{"ident":"Thu Aug 13 04:39:02 2026","wall":"2026-08-13T04:39:02Z"}}},
   "ctr":{"attempt_count":0,...},"st":"queued"}

  === runs ===
  attempt-1: exit  pid  starttime  stderr.log  stdout.log

  === エラー行の検索 ===
  grep -inE 'エラー|error|失敗|panic' cron.log → grep_exit=1（0行）

  === 最小環境・cwd=/tmp からの tick ===
  cwd=/tmp
  tick を実行しました。
    起動: 20260813t044210-nvgmpxt3
  exit=0
  tick を実行しました。
    起動確認: 20260813t044210-nvgmpxt3
  exit=0
  runs/20260813t044210-nvgmpxt3/attempt-1: exit pid starttime stderr.log stdout.log
  ```

- **期待との差:** なし。cron から1分おきに tick が走り、T6 が「起動 → 起動確認」の2 tick で `running` になり、それ以降は変化しない（手続きD は Issue #3）。`cron.log` にエラー行は0行。帳簿のパスはすべて絶対パス。`cd /tmp` + `env -i`（PATH と HOME のみ）の最小環境でも同一の結果になった。
- **備考:** testing.md は `crontab -e`（対話式エディタ）を指定しているが、自動実行のため `crontab -` による標準入力からの登録で代替した。登録内容は同一。確認後は `crontab -r` で確認前の状態（crontab 未登録）に戻した。

### 9. アーカイブ済みタスクが走査対象に入らない

- **判定:** PASS
- **実行したコマンド:**

  ```sh
  pulsen add --workflow pipeline --repo /tmp/pulsen-test/repo   # → T9
  mkdir -p "$PULSEN_HOME/state/archive" && mv "$PULSEN_HOME/state/tasks/$T9.json" "$PULSEN_HOME/state/archive/$T9.json"
  md5 -q .../archive/$T9.json; stat -c '%.9Y' .../archive/$T9.json
  pulsen tick; echo $?
  md5 -q .../archive/$T9.json; stat -c '%.9Y' .../archive/$T9.json
  ls "$PULSEN_HOME/worktrees/"; ls "$PULSEN_HOME/state/runs/"
  git -C /tmp/pulsen-test/repo branch --list "pulsen/$T9"
  ```

- **実際の出力:**

  ```
  === before ===  98f87029d16af0f43ab8c6fa306525a4  /  1786595939.245349939
  === tick ===
  tick を実行しました。
    起動確認: 20260813t043653-3rdmd3ll
  exit=0                                  ← T9 は現れない
  === after ===
  {"exec":{"state":"pending"},"ws":null,"att":null,"upd":"2026-08-13T04:38:59Z"}
  98f87029d16af0f43ab8c6fa306525a4  /  1786595939.245349939   ← md5・mtime とも不変
  === worktrees / runs / branch ===
  T9 のディレクトリもブランチも作られていない（branch --list の出力が空）
  ```

- **期待との差:** なし。アーカイブ側のファイルは md5・mtime とも1バイトも変わっていない。`errors`（スキップ／失敗を記録）にも T9 は現れない。`state/lock` も走査に混ざらない。

### 10. tick の冪等性（状態が変わらないタスク群に対する連続実行）

- **判定:** PASS
- **実行したコマンド:**

  ```sh
  md5 -q "$PULSEN_HOME/state/tasks/"*.json > before.md5
  stat -c '%.9Y %n' "$PULSEN_HOME/state/tasks/"*.json
  pulsen tick; echo $?
  pulsen tick; echo $?
  md5 -q "$PULSEN_HOME/state/tasks/"*.json > after.md5; diff before.md5 after.md5; echo $?
  stat -c '%.9Y %n' "$PULSEN_HOME/state/tasks/"*.json
  ```

- **実際の出力:**

  ```
  === before ===
  1786595946.030901537 20260813t043653-3rdmd3ll.json
  1786595879.494727805 20260813t043735-ouv5ipru.json
  1786595879.524107954 20260813t043737-un9kiqbv.json
  === tick ×2 ===
  処理対象のタスクはありませんでした。   exit=0
  処理対象のタスクはありませんでした。   exit=0
  === diff ===  diff_exit=0（差分なし）
  === after ===  mtime 3件とも before と完全一致
  ```

- **期待との差:** なし。`running` の3タスクに対して tick は何も書かず、「未実装」等の報告も出さない（ADR-101: 未配線のアームは報告もしない）。`errors` は空。

## エッジケース・異常系

### 1. worktree 作成失敗（登録後のリポジトリ消失）とリトライ上限超過

- **判定:** PASS
- **実行したコマンド:**

  ```sh
  pulsen add --workflow fail --repo /tmp/pulsen-test/repo2   # → T12
  mv /tmp/pulsen-test/repo2 /tmp/pulsen-test/repo2.gone
  pulsen tick; echo $?          # ×3
  cat "$PULSEN_HOME/state/tasks/$T12.json"
  ls "$PULSEN_HOME/state/runs/"; ls "$PULSEN_HOME/worktrees/"
  cat /tmp/pulsen-test/notify.log
  ```

- **実際の出力:**

  ```
  === 1回目 ===
  tick を実行しました。
    失敗を記録(1件):
      - 20260813t043928-3xda4nmv: worktree を作成できません(登録済みの worktree を列挙できない:
        fatal: cannot change to '/tmp/pulsen-test/repo2': No such file or directory)
  exit=0
  {"exec":{"state":"failed"},"ws":null,"att":null,
   "ctr":{"attempt_count":1,...},
   "lf":{"kind":"worktree_create","message":"...","at":"2026-08-13T04:39:33Z"}}
  runs/ と worktrees/ に T12 のディレクトリなし

  === 2回目 ===
  tick を実行しました。
    凍結: 20260813t043928-3xda4nmv
    失敗を記録(1件): - 20260813t043928-3xda4nmv: worktree を作成できません(...)
  exit=0
  {"exec":{"state":"stopped","reason":"retry_limit_exceeded","notified_at":null},
   "ctr":{"attempt_count":2,...}}

  === 3回目 ===
  処理対象のタスクはありませんでした。   exit=0
  md5   before=4375e5f6e8f8a5d24c55df7e67e2e492 after=4375e5f6e8f8a5d24c55df7e67e2e492
  mtime before=1786595974.012314781 after=1786595974.012314781

  === notify.log ===  0 バイト
  ```

- **期待との差:** なし。`attempt_count` が 1（= 上限）では凍結せず、2 > 1 で `stopped` / `retry_limit_exceeded` になる。`notified_at` は `null` のまま永続化（ADR-074 の at-least-once 前提）。失敗しても run ディレクトリは作られない（採番は worktree 確保の後）。凍結後の tick は T12 に一切書き込まない。

### 2. テンプレート展開失敗（登録後の設定破壊）と config 修復での復帰

- **判定:** PASS（手順8 のみ testing.md の手順が実行不能のため代替手順で裏付け。下記「期待との差」参照）
- **実行したコマンド:**

  ```sh
  pulsen add --workflow pipeline --repo /tmp/pulsen-test/repo   # → T16
  sed -i.bak2 's/^  shell:/  shellx:/' /tmp/pulsen-test/home/config.yaml
  pulsen tick; echo $?          # ×4
  cat "$PULSEN_HOME/state/tasks/$T16.json"; ls worktrees/; ls state/runs/
  cat /tmp/pulsen-test/notify.log
  ```

- **実際の出力:**

  ```
  === 1回目 ===
  tick を実行しました。
    失敗を記録(1件):
      - 20260813t043953-4nfy69v3: 起動コマンドを組み立てられません(エージェント `shell` は config.yaml に定義されていません)
  exit=0
  {"exec":{"state":"pending"},                         ← pending のまま
   "ws":{"path":".../worktrees/20260813t043953-4nfy69v3","branch":"pulsen/..."},   ← worktree は作られる
   "att":null,                                          ← 採番されない
   "ctr":{"attempt_count":0,"judge_attempt_count":0,"spawn_fail_count":1},
   "lf":{"kind":"spawn_fail","message":"エージェント `shell` は config.yaml に定義されていません",...}}
  worktrees/ に 20260813t043953-4nfy69v3 あり / state/runs/ に無し

  === 2・3回目 ===  spawn_fail_count 2 → 3、execution は pending のまま

  === 4回目 ===
  tick を実行しました。
    凍結: 20260813t043953-4nfy69v3
    失敗を記録(1件): - ...
  exit=0
  {"exec":{"state":"stopped","reason":"spawn_fail_limit_exceeded","notified_at":null},
   "ctr":{"attempt_count":0,"judge_attempt_count":0,"spawn_fail_count":4}}

  === notify.log ===  0 バイト
  ```

  手順8（`TC-exec-tick-055` / config 修復で次の tick が起動に成功する）:

  ```
  === testing.md の手順どおりの実行（config が壊れたまま add） ===
  $ pulsen add --workflow pipeline --repo /tmp/pulsen-test/repo
  エラー: ワークフロー定義の検証に失敗しました(1件)。
    - エージェント `shell` は config.yaml に定義されていません。
      定義済みのエージェント: broken, shellx
    タスクは作られていません。
  exit=1                          ← testing.md は exit 0 を前提にしている

  === 代替手順（config を戻して add → 再度壊して tick → 直して tick） ===
  $ cp config.bak config.yaml && pulsen add --workflow pipeline --repo /tmp/pulsen-test/repo
  タスクID: 20260813t044011-vvkahpq4                       exit=0
  $ sed -i.bak3 's/^  shell:/  shellx:/' config.yaml && pulsen tick
  tick を実行しました。
    失敗を記録(1件): - 20260813t044011-vvkahpq4: 起動コマンドを組み立てられません(...)
  exit=0
  {"exec":{"state":"pending"},"ws":{...},"att":null,"ctr":{...,"spawn_fail_count":1}}
  $ cp config.bak config.yaml && pulsen tick
  tick を実行しました。
    起動: 20260813t044011-vvkahpq4
  exit=0
  {"exec":{"state":"launching","recorded_at":"2026-08-13T04:40:19Z"},
   "ws":{"path":".../worktrees/20260813t044011-vvkahpq4",...},
   "att":{"number":1,"run_dir":".../runs/20260813t044011-vvkahpq4/attempt-1","process":null},
   "ctr":{"attempt_count":0,...,"spawn_fail_count":1}}
  worktrees/20260813t044011-vvkahpq4 あり / state/runs/.../attempt-1 あり
  ```

- **期待との差:** 本体（手順1〜7）は期待どおり。`spawn_fail_count` だけが増え、実行状態も attempt 番号も変わらず、上限3の**超過**（4）で `stopped` / `spawn_fail_limit_exceeded`。「worktree は作られるが run ディレクトリは作られない」非対称も成立。`attempt_count` は一度も増えない。
  手順8 は **testing.md の手順に誤りがある**。`pulsen add` は登録時にワークフローが参照するエージェントを config に照合するため（Issue #1 の登録時検証）、config が壊れた状態では T16b を追加できず exit 1 になる。順序を「config 復元 → add → config 破壊 → tick（spawn_fail_count 1）→ config 復元 → tick」に組み替えて実行し、**config を直すと次の tick で起動に成功する**という `TC-exec-tick-055` の主張は裏付けられた。実装側の不具合ではなく手順書の順序の誤り。

### 3. パース不能なタスクファイルの混在と他タスクの続行

- **判定:** PASS
- **実行したコマンド:**

  ```sh
  pulsen add --workflow pipeline --repo /tmp/pulsen-test/repo            # → T20
  pulsen add --workflow /tmp/pulsen-test/draft.yaml --repo ...           # → T20H
  pulsen add --workflow pipeline --repo /tmp/pulsen-test/repo            # → T20P
  cp .../tasks/$T20.json t20.bak && echo broken > .../tasks/$T20.json
  pulsen tick; echo $?
  cat .../tasks/$T20.json; stat -c '%.9Y' .../tasks/$T20.json
  jq -c ... .../tasks/$T20H.json; jq -c ... .../tasks/$T20P.json
  ```

- **実際の出力:**

  ```
  === tick ===
  tick を実行しました。
    起動: 20260813t044033-0qdcul2p
    起動確認: 20260813t044011-vvkahpq4
    スキップ(1件):
      - /tmp/pulsen-test/home/state/tasks/20260813t044031-2pbq08py.json:
        タスクファイルを読めません(expected value at line 1 column 1)
  exit=0

  === T20（破損ファイル） ===
  broken
  mtime before=1786596041.681913917  after=1786596041.681913917   ← 不変

  === T20H ===  {"exec":{"state":"pending"},"ws":null,"att":null,"st":"done"}
  === T20P ===  {"exec":{"state":"launching","recorded_at":"2026-08-13T04:40:41Z"},
                 "ws":{"path":".../worktrees/20260813t044033-0qdcul2p",...},
                 "att":{"number":1,"run_dir":".../attempt-1","process":null}}
                 worktrees/20260813t044033-0qdcul2p あり / runs/.../attempt-1 あり
  === notify.log ===  0 バイト
  ```

- **期待との差:** なし。tick は exit 0 で、報告行に**読めなかったファイルのフルパス**が出る。破損ファイルは内容も mtime も変わらない。T20H（`Cleanup`）は未配線なので何も起きない。T20P は同一 tick で起動されている。
- **補足:** testing.md は「サマリーの `errors` に報告される」と書いているが、CLI の表示ラベルは `errors` ではなく **「スキップ」**。`crates/pulsen/src/cli/render.rs:73-82` で `summary.errors` を結末（`Recorded` / `LaunchUnsettled` / `Skipped`）で仕分けして「失敗を記録」「起動の結果が未確定」「スキップ」の3見出しに分けて出しており、いずれも `errors` の描画。期待の実体は満たされている。

### 4. スナップショットのみ破損したタスクのスキップと報告

- **判定:** PASS
- **実行したコマンド:**

  ```sh
  pulsen add --workflow pipeline --repo /tmp/pulsen-test/repo   # → T25
  jq '.snapshot.statuses = "broken"' t25.bak > new && mv new "$S"
  md5 -q "$S"; stat -c '%.9Y' "$S"
  pulsen tick; echo $?
  md5 -q "$S"; stat -c '%.9Y' "$S"
  ls "$PULSEN_HOME/state/runs/$T25/" 2>/dev/null; echo $?; ls "$PULSEN_HOME/worktrees/" | grep "$T25"
  ```

- **実際の出力:**

  ```
  === before ===  ca1ff306fc1103a90ac21f4590f8626e  /  1786596065.755748605
  === tick ===
  tick を実行しました。
    起動: 20260813t044031-2pbq08py
    起動確認: 20260813t044033-0qdcul2p
    スキップ(1件):
      - 20260813t044058-y5a73lp7: 埋め込まれたワークフロー定義を読めません
        (invalid type: string "broken", expected a map at line 5 column 24)
  exit=0
  === after ===   ca1ff306fc1103a90ac21f4590f8626e  /  1786596065.755748605   ← 不変
  === runs / worktrees ===  ls exit=2（runs/<T25> なし）／ grep exit=1（worktree なし）
  === notify.log ===  0 バイト
  ```

- **期待との差:** なし。エッジケース3（パス表示）と違い、**タスクID** が報告に出る（属性は読めているため）。ファイルは md5・mtime とも不変で `stopped` にもされていない。

### 5. ロック競合 — tick は 0 でスキップ、add は非0で拒否

- **判定:** PASS
- **実行したコマンド:**

  ```sh
  pulsen add --workflow pipeline --repo /tmp/pulsen-test/repo   # → T24
  # 標準入力を開いたまま保持するため fifo を使って lock_holder を起動
  mkfifo holder.fifo
  target/debug/examples/lock_holder /tmp/pulsen-test/home/state/lock < holder.fifo &
  exec 9> holder.fifo
  ls -l "$PULSEN_HOME/state/tasks/" > lock-before.txt
  pulsen tick; echo $?
  pulsen add --workflow pipeline --repo /tmp/pulsen-test/repo; echo $?
  ls -l "$PULSEN_HOME/state/tasks/" | diff lock-before.txt -; echo $?
  exec 9>&-        # 標準入力を閉じる（Ctrl-D 相当）
  pulsen tick; echo $?
  ```

- **実際の出力:**

  ```
  holder pid=55434 out=[locked]
  === step4: tick under contention ===
  別の操作が実行中のため、今回の tick はスキップしました。
  exit=0
  elapsed=0s
  === step5: add under contention ===
  エラー: 別の操作が実行中です。
    時間をおいて再実行してください。タスクは作られていません。
  exit=1
  elapsed=0s
  === step6 ===  diff_exit=0（差分なし）
  === step7 ===  holder_exit=0
  === step8: tick after release ===
  tick を実行しました。
    起動: 20260813t044058-y5a73lp7, 20260813t044112-sq249ew8
    起動確認: 20260813t044031-2pbq08py
  exit=0
  ```

  確認ポイント（保持プロセスを `kill -9` した場合）:

  ```
  holder pid=55657 out=[locked]
  === tick under contention ===
  別の操作が実行中のため、今回の tick はスキップしました。   exit=0
  === kill -9 the holder ===  killed
  === tick after SIGKILL ===
  tick を実行しました。
    起動確認: 20260813t044058-y5a73lp7, 20260813t044112-sq249ew8
  exit=0
  ```

- **期待との差:** なし。tick は「エラー」ではなく「スキップ」として読める文言で exit 0、`add` は exit 1 でタスクを作らない。どちらも待ちに入らず即座に返る（elapsed 0秒）。保持プロセスを `kill -9` で強制終了してもロックは残らず、次の tick が通常どおり進む。
- **備考:** testing.md は「別端末で `lock_holder` を実行し標準入力を開いたままにする」と書いているが、自動実行のため名前付きパイプ（fifo）の書き込み端をシェルの fd 9 で開いたままにする方法で等価な状態を作った。

### 6. 滞留するエージェントを起動したまま次の tick を打つ（ロックFDの非継承）

- **判定:** PASS
- **実行したコマンド:**

  ```sh
  cat > /tmp/pulsen-test/home/workflows/stay.yaml <<'EOF'   # prompt: "sleep 60"
  pulsen add --workflow stay --repo /tmp/pulsen-test/repo   # → T26
  pulsen tick; echo $?
  until [ -f ".../attempt-1/pid" ]; do sleep 1; done
  ps -ef | grep -E 'pulsen wrapper|sleep 60' | grep -v grep
  pulsen tick; echo $?
  cat ".../tasks/$T26.json"
  until [ -f ".../attempt-1/exit" ]; do sleep 2; done; cat ".../attempt-1/exit"
  ```

- **実際の出力:**

  ```
  === step3: tick ===
  tick を実行しました。
    起動: 20260813t044224-ocz9p0fo
  exit=0
  === step4: pid ===  { "pid": 55856, "kill_ident": "-55856" }
  === ps ===
    501 55856     1  ... pulsen wrapper --run-dir .../attempt-1 --workspace .../worktrees/... -- sh -c sleep 60
    501 55883 55856  ... sleep 60
  === step5: tick while agent sleeping ===
  tick を実行しました。
    起動確認: 20260813t044224-ocz9p0fo
  exit=0
  === step6 ===
  {"exec":{"state":"running"},"att":{"number":1,...,"process":{"pid":55856,"kill_ident":"-55856",
   "starttime":{"ident":"Thu Aug 13 04:42:29 2026","wall":"2026-08-13T04:42:29Z"}}}}
  === step7 ===  { "code": 0 }
  ```

- **期待との差:** なし。`sleep 60` の最中の tick は「別の操作が実行中」にならず通常のサマリーを出して exit 0。ラッパーは PPID 1 に付け替わって（デタッチ）生存しており、それでも次の tick が走る = ロックFDを継承していない。60秒後に `exit` として `{"code":0}` が書かれた。

### 7. 進行中の worktree の手動削除 — エージェント実行の失敗として既存経路に落ちる

- **判定:** PASS（testing.md の期待文言に不正確な箇所あり。下記参照）
- **実行したコマンド:**

  ```sh
  pulsen add --workflow pipeline --repo /tmp/pulsen-test/repo   # → T27
  pulsen tick; until [ -f ".../attempt-1/exit" ]; do sleep 1; done
  jq '.execution = {"state":"failed"}' "$S" > new && mv new "$S"
  rm -rf "$PULSEN_HOME/worktrees/$T27"
  pulsen tick; echo $?
  jq -c '{exec,ws,att,ctr,lf}' "$S"
  until [ -f ".../attempt-2/exit" ]; do sleep 1; done
  cat ".../attempt-2/exit"; ls ".../attempt-2/"
  ```

- **実際の出力:**

  ```
  === step5: tick ===
  tick を実行しました。
    起動: 20260813t044237-nxow3z0a
  exit=0                                  ← worktree 作成に関する失敗の報告なし
  === step6 ===
  {"exec":{"state":"launching","recorded_at":"2026-08-13T04:42:44Z"},
   "ws":{"path":".../worktrees/20260813t044237-nxow3z0a","branch":"pulsen/..."},
   "att":{"number":2,"run_dir":".../attempt-2","process":null},
   "ctr":{"attempt_count":0,"judge_attempt_count":0,"spawn_fail_count":0},"lf":null}
  === step7 ===
  [exit]  { "code": 126 }
  [files] exit  pid  starttime            ← stdout.log / stderr.log は作られない
  ```

- **期待との差:** `exit` が 126、tick 側に新しい分岐が生じない、`current_attempt.number` が 2 — いずれも期待どおり。
  1点だけ testing.md の期待文言と食い違う: testing.md は「`stdout.log` は空」と書いているが、実際には `stdout.log` / `stderr.log` は**作成されない**。これは実装の意図した挙動で、`crates/pulsen/src/adapter/process.rs:839-853` のユニットテスト `作業ディレクトリが存在しなければエージェントを起動せず起動不能を返す` が `assert!(!base.path().join("stdout.log").exists(), "ログも作られない")` として固定している。確認ポイントである「`exit` が書かれること自体」は成立しているため PASS とし、**手順書側の記述の誤り**として記録する。

### 8. ブランチのみ残存した状態からの worktree 張り直し

- **判定:** PASS
- **実行したコマンド:**

  ```sh
  pulsen add --workflow pipeline --repo /tmp/pulsen-test/repo   # → T28
  git -C repo branch "pulsen/$T28" main
  git -C repo worktree add /tmp/pulsen-test/tmpwt "pulsen/$T28"
  echo carried > tmpwt/carried.txt && git -C tmpwt add carried.txt && git -C tmpwt commit -m carried
  git -C repo worktree remove /tmp/pulsen-test/tmpwt
  git -C repo rev-parse "pulsen/$T28" > tip-before.txt
  pulsen tick; echo $?
  jq -c '{exec,ws,att,ctr,lf}' "$S"
  ls "$PULSEN_HOME/worktrees/$T28/"
  git -C repo rev-parse "pulsen/$T28" | diff tip-before.txt -; echo $?
  ```

- **実際の出力:**

  ```
  === step2 ===
  worktree list に当該ブランチの登録なし（grep_exit=1）
  tip-before = 57c61598a4a200206f1b4b1e4d7683c690c87a0b
  === step3: tick ===
  tick を実行しました。
    起動: 20260813t044309-hxruhyr7
    起動確認: 20260813t044237-nxow3z0a
  exit=0
  === step4 ===
  {"exec":{"state":"launching","recorded_at":"2026-08-13T04:43:16Z"},
   "ws":{"path":".../worktrees/20260813t044309-hxruhyr7","branch":"pulsen/20260813t044309-hxruhyr7"},
   "att":{"number":1,"run_dir":".../attempt-1","process":null},
   "ctr":{"attempt_count":0,...},"lf":null}
  === step5 ===
  carried.txt
  diff_exit=0（ブランチ先端は 57c6159 のまま変わっていない）
  ```

- **期待との差:** なし。`record_tool_failure(WorktreeCreate)` に落ちず（`attempt_count` は 0、`last_failure` は `null`）、`worktree add`（`-f` なし）で張り直され、積まれた `carried.txt` が worktree に戻り、ブランチ先端も変わっていない。

### 9. ラッパーの起動引数が不正な場合

- **判定:** PASS
- **実行したコマンド:**

  ```sh
  cd /tmp && pulsen wrapper --run-dir ./relative --workspace /tmp/pulsen-test/wrapws -- sh -c true; echo $?
  pulsen wrapper --run-dir "$WRUN3" --workspace ./relative -- sh -c true; echo $?
  pulsen wrapper --run-dir "$WRUN3" --workspace /tmp/pulsen-test/wrapws --; echo $?
  mkdir -p /tmp/pulsen-test/notrun
  pulsen wrapper --run-dir /tmp/pulsen-test/notrun --workspace /tmp/pulsen-test/wrapws -- sh -c true; echo $?
  ls -la "$WRUN3" /tmp/pulsen-test/notrun
  ```

- **実際の出力:**

  ```
  === 相対 --run-dir ===
  エラー: --run-dir を絶対パスとして解決できません。
    指定: ./relative
  exit=1        （stdout は 0 バイト）
  === 相対 --workspace ===
  エラー: --workspace を絶対パスとして解決できません。
    指定: ./relative
  exit=1
  === トークン0個 ===
  エラー: エージェントのコマンドが不正です。
    原因: コマンドが空です
  exit=1
  === 形式外の run_dir ===
  エラー: runディレクトリが規定の形ではありません。
    指定: /tmp/pulsen-test/notrun
  exit=1
  === step4 ===
  WRUN3 entries=0
  notrun entries=0
  ```

- **期待との差:** なし。4ケースすべて非0で終了し、両ディレクトリとも空のまま（`starttime` / `pid` / `exit` / ログのいずれも作られない）。エラーはすべて標準エラーに出て標準出力は0バイト。実在するディレクトリでも `<state_root>/runs/<task-id>/attempt-<n>` の形でなければ拒否される（ADR-078 の逆写像が `None`）。

## 既存機能への影響確認

### 1. `pulsen add` の経路が壊れていないこと（`.thread/1/testing.md` 確認項目1・2・4 の再実行）

- **判定:** PASS
- **実際の出力:**

  ```
  ### 確認項目1: 未初期化ホームの案内文言
  エラー: グローバルホームが未初期化です。
    グローバルホーム: /Users/hikaru/pulsen-empty-home
    グローバル設定 /Users/hikaru/pulsen-empty-home/config.yaml を作成してください。
  exit=1
  ls /Users/hikaru/pulsen-empty-home → exit=2（ホーム自体が作られていない）

  ### 確認項目2: ホーム解決の優先順位
  1) PULSEN_HOME=/no/such/home + --home <SETUP_HOME> → 登録成功 exit=0（フラグが優先）
  2) PULSEN_HOME=/no/such/home のみ →
     エラー: グローバルホームが未初期化です。
       グローバルホーム: /no/such/home                                    exit=1
  3) env -u PULSEN_HOME HOME=$FAKEHOME →
     エラー: グローバルホームが未初期化です。
       グローバルホーム: /var/folders/.../tmp.zRKLWuoSKR/.pulsen         exit=1
     （FAKEHOME 配下に .pulsen は作られていない）

  ### 確認項目4: 名前指定の登録成功（真新しいホームで実行）
  タスクを登録しました。
    タスクID: 20260813t044412-f0zapwgh        （24文字。<yyyymmdd>t<hhmmss>-<base36 8桁>）
    ワークフロー: implement
    解決先: /Users/hikaru/pulsen-impact-home/workflows/implement.yaml
  exit=0
  タスクファイル: インデント済み JSON。task_status="queued"、execution={"state":"pending"}、
    counters 全0、workspace/current_attempt/last_failure が null、
    target={"repo":"/Users/hikaru/pulsen-test-repo","base_branch":"main"}、
    updated_at="2026-08-13T04:44:12Z"（UTC・秒精度）、
    snapshot.statuses = ["done","implemented","queued","review_waiting"]（4件）
  元 YAML 削除後も snapshot はそのまま残る
  ```

- **期待との差:** なし。Issue #1 時点と同じ結果。`current_exe()` を要する `ProcessController` の追加で `add` が落ちることはない。

### 2. `pulsen --help` の表示

- **判定:** PASS
- **実際の出力:**

  ```
  Commands:
    add   タスクを登録する(実行はしない)
    tick  1回のtickパスを実行する
    help  Print this message or the help of the given subcommand(s)
  Options:
        --home <DIR>  グローバルホームのディレクトリ(既定: 環境変数 PULSEN_HOME、なければ ~/.pulsen)
    -h, --help / -V, --version

  $ pulsen tick --help
  Usage: pulsen tick [OPTIONS]
  Options:
        --home <DIR>  ...
    -h, --help
  exit=0

  $ pulsen add --nope       → exit=2
  $ pulsen bogus            → exit=2
  $ pulsen add              → exit=2
  ```

- **期待との差:** なし。`wrapper` はサブコマンド一覧に現れない（ADR-077）。`tick` に位置引数は無く `--home` は global フラグとして現れる。引数の誤用は clap 既定の exit 2。

### 3. AC-1 の grep 期待値の更新

- **判定:** PASS
- **期待との差:** なし。「確認環境」節に記載のとおり、`crates/pulsen-domain/` は0件、`crates/pulsen/src/` 側は `util/atomic.rs` / `adapter/process.rs` / `adapter/task_repository.rs` の3ファイル。`task_repository.rs` のヒットは `#[cfg(all(test, unix))]` のテストモジュール、`pulsen-conformance` のヒットは適合ハーネスの probe 分岐で、いずれも本番経路に乗らない。

### 4. `state/` のレイアウトへの追加

- **判定:** PASS
- **実際の出力:**

  ```
  # 真新しいホームで add のみ実行した直後
  $ ls "$IMPACT_HOME/worktrees/" "$IMPACT_HOME/state/runs/" 2>/dev/null; echo $?
  exit=2
  $ ls -a "$IMPACT_HOME"        →  config.yaml  state  workflows
  $ ls -a "$IMPACT_HOME/state"  →  lock  tasks
  ```

- **期待との差:** なし。`add` は `worktrees/` も `state/runs/` も作らない（Issue #1 の期待が保たれている）。作るのは tick だけ。

### 5. 実運用ホームの非汚染

- **判定:** PASS
- **実際の出力:**

  ```
  # 確認開始時
  $ ls -a "$HOME/.pulsen" 2>/dev/null; echo $?
  exit=2
  # 全項目の実行後
  $ ls -a "$HOME/.pulsen" 2>/dev/null; echo $?
  exit=2
  ```

- **期待との差:** なし。`~/.pulsen` は開始時も終了時も存在しない。フィクスチャA は `/tmp/pulsen-test/`、フィクスチャB は `$HOME/pulsen-manual-test` に閉じており、cron の行も `--home` を絶対パスで指定した。既定ホーム解決の確認（影響確認1 の手順3）は `HOME` を `mktemp -d` に差し替えて行った。

### 6. 後片付け

- **判定:** PASS
- **実際の出力:**

  ```
  $ crontab -r; crontab -l
  crontab: no crontab for hikaru        （確認前と同じ「crontab 未登録」の状態に戻した）

  $ ps -ef | grep -iE 'pulsen wrapper|lock_holder|agent_probe|spawn_probe' | grep -v grep
  （出力なし）
  $ pgrep -f agent_probe        → exit=1（該当なし）
  $ pgrep -f 'pulsen wrapper'   → exit=1（該当なし）
  $ pgrep -f lock_holder        → exit=1（該当なし）

  $ rm -rf /tmp/pulsen-test "$HOME/pulsen-manual-test" "$HOME/pulsen-manual-work" \
      "$HOME/pulsen-test-repo" "$HOME/pulsen-empty-home" "$HOME/pulsen-impact-home"
  $ ls -d 上記6パス → 6件とも No such file or directory

  # 削除後に再確認
  $ ps -ef | grep -iE 'pulsen wrapper|lock_holder|agent_probe|spawn_probe' | grep -v grep
  （出力なし）
  ```

- **期待との差:** なし。worktree を作ったリポジトリごと削除したため `git worktree prune` は不要。デタッチ起動したラッパーは一時領域の削除時点で1つも残っていない。
- **備考:** 影響確認1 の実行で `$HOME/pulsen-empty-home`（作られない）と `$HOME/pulsen-impact-home`（新規に作成）を使ったため、testing.md の後片付けリストに無いこの2つも削除対象に加えた。

## testing.md 自体の誤り

1. **エッジケース2 手順8 の順序が実行不能**（重要）。config を `shellx` に壊した状態で `pulsen add` を打つ手順になっているが、`add` は登録時にワークフローが参照するエージェントを config に照合するため exit 1 で拒否され、T16b を作れない。`TC-exec-tick-055` を確認するには「config 復元 → add → config 破壊 → tick → config 復元 → tick」の順にする必要がある。実装の不具合ではなく手順書の誤り。

2. **エッジケース7 の期待「`stdout.log` は空」が不正確**。cwd 不在で `run_agent` が 126 を返す経路では `stdout.log` / `stderr.log` は空ファイルとして作られるのではなく、**まったく作られない**。これは意図された挙動で `crates/pulsen/src/adapter/process.rs` のユニットテストが `assert!(!... .exists(), "ログも作られない")` として固定している。期待文言を「ログファイルは作られない」に直すべき。

3. **エッジケース3・4 のラベル名**（軽微）。「サマリーの `errors` に報告される」と書かれているが、CLI 上の見出しは `errors` ではなく結末別の「失敗を記録」「起動の結果が未確定」「スキップ」。`summary.errors` の描画であることは `crates/pulsen/src/cli/render.rs:73-82` で確認済みで、内容の期待は満たされている。読み手が「`errors` という見出しを探す」ことにならないよう補足があるとよい。

4. **対話的コマンドの指定**（軽微・自動実行時のみ）。確認項目8 の `crontab -e` とエッジケース5 の「別端末で `lock_holder` を実行し標準入力を開いたままにする」は対話操作を前提にしている。今回は `crontab -`（標準入力からの登録）と名前付きパイプ（fifo）の書き込み端を fd で保持する方法で等価な状態を作った。人手で実施する分には問題ない。

5. **確認項目3 手順1 は実際にはほぼ観測できない**（軽微）。tick 直後1秒以内でもラッパーは既に `pid` を書き終えているため、猶予内経路（`KeepWaiting`）はこの手順では踏めない。手順書自身が「観測できず」と記録して進むよう指示しているので誤りではないが、`launching` かつ run ディレクトリが空の状態を人工的に作る手順を代替として書いておくと確実に確認できる（本確認ではその方法で裏付けた）。

## 作業ツリーの最終確認

```
$ git status --short
.thread/2/manual-test/result.md（本ファイル）以外に変更なし
```

ソースコード・spec・既存の `.thread/` ドキュメントには一切変更を加えていない。
