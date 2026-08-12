### Adapter / Infrastructure

#### Blockers

なし

#### Warnings

- **[W-001]** `create` の「達成済み」判定が git の `prunable` 注記だけに依存していて、実体の存在を直接確かめていない
  - 場所: `crates/pulsen/src/adapter/worktree.rs:207-223`
  - 理由: 登録が鍵と一致し、ブランチも一致したとき、`if !entry.prunable { return Ok(()) }` で即座に達成済みとする。`ws.path` に実体があることは一度も観測していない。`worktree list --porcelain` の `prunable` 行は git 2.36 以降でしか出力されず、リポジトリに `extensions.worktreeConfig` 等で古い git が使われる環境では、実体の消えた登録が `prunable` 無しで列挙される。そのとき `create` は worktree が無いまま `Ok` を返し、ADR-013 が置いた復旧分岐(`add -f`)は一度も実行されない。結末は ADR-013 自身が「張り直さず `Failed` を返す案を採らない理由」として書いているものと同じ — `confirm_workspace` → `record_launching` → spawn と進み、`run_agent` が cwd 不在で 126 を書き、リトライのたびに 126 を繰り返して上限超過 stopped に至る。適合スイート側は `workspace_with_prunable_registration` の前提(`registration.prunable`)が作れず `None` → スキップになり、スキップ許容集合に無いので失敗として現れる(そこは良い)が、リポジトリのどこにも git の最低バージョンが宣言されていないため、適合スイートを走らせない環境では検出されない。
  - 提案: 判定条件を git の porcelain の機能水準から切り離す。`entry.prunable || !matches!(path.try_exists(), Ok(true))` を「張り直す」条件にする(あるいは `prunable` を見ずに実体の有無だけで分ける)。「登録あり・ブランチ一致・実体なし」は直接観測できる状態で、`-f` を使ってよい範囲(ADR-013 が限定した1つの保護)もこの条件でそのまま閉じる。あわせて git の最低バージョンを requirements か README に明記する。

- **[W-002]** Windows の同定情報取得が、機構の失敗を「対象プロセスが存在しない」に畳みうる
  - 場所: `crates/pulsen/src/adapter/process.rs:632-660`
  - 理由: `powershell -NoProfile -Command <script>` の結果を「exit 0 かつ出力が空 → `Ok(None)`」に写している。PowerShell の既定の `$ErrorActionPreference` は `Continue` なので、`Get-CimInstance` が非終端エラー(アクセス拒否・WMI リポジトリの異常・CIM サービス停止)を出したときも **exit 0・stdout 空** になり、`Ok(None)` に落ちる。ADR-003 の写像表は「Windows: それ以外の失敗・形式外 → `Err(Io)`」と定めているが、現在のスクリプトはこの2つを区別する情報を返していない。畳む向きが plan.md の想定と逆で、影響は重い — #3 の `starttime_of` が生存中のプロセスを `Ok(None)` = Dead と読み、`DiedWithoutExit` → failed → 再起動 → **同一 worktree での並走**という、この設計が一貫して避けている失敗モードに直結する。plan.md の「含まれないもの」が外しているのは Windows **実機での検証**であって、取得手段の三値の写像は本スライスの責務(ADR-003 が写像表まで書いている)。
  - 提案: スクリプト冒頭に `$ErrorActionPreference = 'Stop'` を置き、`try { ... } catch { exit 3 }` で機構失敗を非0の exit code として区別する(あるいは `output.stderr` が非空なら `Err(Io)` にする)。実機確認は #10 に委ねるとしても、「不在」と「機構失敗」を区別する情報をコマンドから取り出す形にしておかないと、#10 で気づいたときには帳簿の誤判定が起きた後になる。

- **[W-003]** `spawn_wrapper` が本当に新しいプロセスグループへ切り離しているかを主張するテストが無い
  - 場所: `crates/pulsen-conformance/src/process_controller.rs:294-306`、`crates/pulsen/tests/cli_tick.rs:117-123`
  - 理由: デタッチ性の適合ケース TC-002 が主張するのは「呼び出し側プロセスの終了後もラッパーが完走する」ことだけで、POSIX ではこれは `process_group(0)` が**無くても**成立する(非セッションリーダーの親が終了しても子に SIGHUP は届かない)。`detach(&mut command)` の呼び出しを消しても、適合スイート・受け入れテストとも緑のままになる。ところがこの1行が消えたときの実害は「完走しない」ではなく、`own_identity` が観測する PGID が**呼び出し側(cron / シェル)のプロセスグループ**になり、その値が `KillIdent` として pid ファイルとタスクファイルに**永続化**されること — ADR-003 が `pgid == pid` を仮定しない理由として長く書いている「無関係なプロセス群を殺す経路」そのものが、逆側から開く。`cli_tick.rs:117-123` の `kill_ident` に対するアサーションは「非空」だけで、この破れを素通りさせる(TC-004 が非空しか主張しないのと同じ穴が、実起動経路にも残っている)。
  - 提案: `cli_tick.rs` の「次のtickはpidの出現をもってrunningへ取り込む」で、既に読んでいる pid ファイル(または `current_attempt.process`)に対して `kill_ident == format!("-{pid}")` を主張する。実測(本レビューで手動確認)では tick 経由で起動したラッパーは PGID == PID・PPID == 1 になり、この等式は成立する。1行のアサーションで `detach` の退行が検出できるようになる。

- **[W-004]** `run_agent` は stdout のログを開けてから stderr のログを開くため、stderr 側だけが開けないときに stdout のログを作った状態で 126 を返す
  - 場所: `crates/pulsen/src/adapter/process.rs:155-158`
  - 理由: `(File::create(stdout), File::create(stderr))` はタプルの両辺が評価される。stdout が開けて stderr が開けない順序では、`stdout.log` を作成・切り詰めてからエージェントを起動せずに 126 を返す。適合ケース TC-025 は stdout 側だけを開けなくする(`unwritable_log_path` が stdout に割り当てられている)ので、この順序は一度も実行されない。契約の「エージェントを起動しない」は守られているので破れではないが、「リダイレクト先を開けない場合は副作用を生まない」というコメント(`crates/pulsen/src/adapter/process.rs:156`)の主張とは食い違う。
  - 提案: どちらかを先に開いて失敗したら即座に返す形にするか、コメントの主張を「エージェントを起動しない」に限定して、ログの作成は副作用として認める旨を書く。

#### カバレッジ

- 確認: `crates/pulsen/src/adapter/process.rs`, `crates/pulsen/src/adapter/run_store.rs`, `crates/pulsen/src/adapter/worktree.rs`, `crates/pulsen/src/adapter/mod.rs`, `crates/pulsen/src/cli/wire.rs`, `crates/pulsen/src/cli/wrapper.rs`, `crates/pulsen/src/cli/tick.rs`, `crates/pulsen/src/cli/args.rs`, `crates/pulsen/src/cli/mod.rs`, `crates/pulsen/src/application/run_wrapper.rs`, `crates/pulsen/src/application/tick/mod.rs`, `crates/pulsen/src/application/tick/launch.rs`, `crates/pulsen/src/application/tick/confirm_spawn.rs`, `crates/pulsen-domain/src/execution/port.rs`, `crates/pulsen-domain/src/task/path.rs`, `crates/pulsen/examples/agent_probe.rs`, `crates/pulsen/examples/spawn_probe.rs`, `crates/pulsen-conformance/src/lib.rs`, `crates/pulsen-conformance/src/run_store.rs`, `crates/pulsen-conformance/src/process_controller.rs`, `crates/pulsen-conformance/src/worktree_manager.rs`, `crates/pulsen-conformance/src/doubles/process.rs`, `crates/pulsen-conformance/src/doubles/run_store.rs`, `crates/pulsen-conformance/HOOKS.md`, `crates/pulsen/tests/conformance_process_controller.rs`, `crates/pulsen/tests/conformance_run_store.rs`, `crates/pulsen/tests/conformance_worktree.rs`, `crates/pulsen/tests/cli_wrapper.rs`, `crates/pulsen/tests/cli_tick.rs`, `crates/pulsen/tests/common/mod.rs`, `crates/pulsen/tests/common/git.rs`, `.thread/2/plan.md`, `.thread/2/adr.md`
- スキップ: `.thread/2/progress.md`, `.thread/2/steps.md`, `.thread/2/testing.md` — 進行管理・手順の記録で、アダプターの実装判断を含まない
- スキップ: `crates/pulsen-domain/src/definition/agent.rs`, `crates/pulsen-domain/src/definition/template.rs`, `crates/pulsen-domain/src/execution/launching.rs`, `crates/pulsen-domain/src/execution/mod.rs`, `crates/pulsen-domain/src/execution/value.rs`, `crates/pulsen-domain/src/task/attempt.rs`, `crates/pulsen-domain/src/task/counters.rs`, `crates/pulsen-domain/src/task/failure.rs`, `crates/pulsen-domain/src/task/mod.rs`, `crates/pulsen-domain/src/task/planner.rs`, `crates/pulsen-domain/src/task/task.rs`, `crates/pulsen-domain/src/task/transition.rs` — I/O に触れない純粋なドメイン(値・遷移・分類)でドメイン観点の担当。ポート定義(`execution/port.rs`)とアダプターが導出に使う `task/path.rs` だけを確認側に入れた
- スキップ: `crates/pulsen-conformance/src/doubles/clock.rs`, `crates/pulsen-conformance/src/doubles/mod.rs`, `crates/pulsen-conformance/src/doubles/task_repository.rs`, `crates/pulsen-conformance/src/doubles/tests.rs`, `crates/pulsen-conformance/src/doubles/worktree.rs` — 本スライスで追加したアダプターに対応しないダブル(と、そのメタテスト)でユースケース観点の担当
- スキップ: `crates/pulsen/src/application/mod.rs` — モジュール宣言の追加のみ
- スキップ: `crates/pulsen/src/cli/render.rs` — 利用者向け文言の組み立てで、アダプターの挙動に影響しない
- スキップ: `crates/pulsen/tests/cli_usage.rs`, `crates/pulsen/tests/register_task.rs`, `crates/pulsen/tests/run_wrapper.rs`, `crates/pulsen/tests/tick_confirm_spawn.rs`, `crates/pulsen/tests/tick_fixture/mod.rs`, `crates/pulsen/tests/tick_launch.rs`, `crates/pulsen/tests/tick_scan.rs` — テストダブルに対するユースケース層テストとサブコマンド一覧の受け入れで、実アダプターの挙動を検証しない

補足: 変更ファイル一覧の実件数は 62 件(依頼文の 61 件と1件ずれる)。上の確認 33 件 + スキップ 29 件 = 62 件で一覧と1対1に対応する。

#### 受け入れ基準の確認(自観点)

- **AC-1**: 満たす。`cargo fmt --check` / `cargo clippy --all-targets -- -D warnings` / `cargo test` を実行して通ることを確認した(全 20 テストターゲット・0 failed)。`Cargo.toml` / `crates/*/Cargo.toml` に差分は無く、新規依存クレートは 0、`unsafe_code = "forbid"` は workspace とドメインの両方で維持されている。`grep -rE 'cfg\((unix|windows|target_os|target_family)' crates/*/src/` のヒットは `crates/pulsen/src/util/atomic.rs:71`、`crates/pulsen/src/adapter/process.rs`(194 / 201 / 214 / 422 / 613 / 730)、`crates/pulsen-conformance/src/lib.rs:259`(本 PR 以前から存在)の3ファイルで、`crates/pulsen-domain/` は 0 件、`crates/pulsen/src/` 側は AC-1 が挙げた2ファイルだけ。
- **AC-6**: 満たす。`RunStore` 9 / `ProcessController` 3 / `WorktreeManager` の `create` 1 で spec のポート表と一致し、`attempt_exists` / `list_runs` / `delete_attempt` / `remove_task_dir_if_empty` / `starttime_of` / `kill` / `try_kill_remnants` / `remove` の宣言もスタブも無い。`WrapperLaunchSpec` / `WrapperIdentity` / `SpawnError` / `WorktreeError` / `RunFileError` も定義されている。
- **AC-7**: 満たす。適合スイート 21 件が実装され、`conformance_run_store` は 21 件すべて実行して通る。アトミック置換は `crate::util::atomic::write_atomic` を経由しており再実装は無い。read 系の3分類はディレクトリ不在(`NotFound` → `Ok(None)`)を含めて正しく、`marker_exists` は `exists()` ではなく `try_exists()` を使って機構の失敗を「無い」に丸めていない。
- **AC-8**: 満たす。適合ケースは 9 → 17 件(台帳7行 + ADR-013 由来の追加1件)。`physical_key` は `ws.path` と git の出力の**両側**に対称にかかっており、ハーネスの `worktree_root` がシンボリックリンク経由になっていて正規化の分岐が必ず実行される。別ブランチの worktree・通常ディレクトリ・base 不在はいずれも `Failed` で自動修復しない。ブランチのみ残存は `-f` なしの `worktree add` で先端を変えずに張り直す。実体の消えた登録の分岐についてのみ W-001。
- **AC-9**: 満たす。`identity_and_agent` 13 件 + `spawn` 3 件が実装され、この環境で 16 件すべて実行して通る。起動時刻・PGID の取得はプラットフォームごとの `identity::observe` **1関数**に閉じ、戻り値は三値(`Ok(Some)` / `Ok(None)` / `Err(Io)`)。不在を機構失敗へ畳む1行は呼び出し側(`own_identity`)にあり、共有関数には無い。`ps` へは `LC_ALL=C` / `TZ=UTC` を注入し `LANG` / `LC_TIME` を `env_remove` していて、実測で `env -i LANG=ja_JP.UTF-8 TZ=Asia/Tokyo` の下でも出力が一致することを確認した(`COLUMNS` による桁切りは macOS のパイプ出力では起きないことも確認)。Linux の `/proc/<pid>/stat` は「最後の `)` より後ろ」を起点に索引しており、`comm` に空白・括弧を含んでも壊れない。
- **AC-16**: 満たす。`crates/pulsen/tests/cli_wrapper.rs:163`(exit に 126・`stdout.log` 不在)と `crates/pulsen/tests/cli_tick.rs:248`(進行中の worktree 消失)で既存経路に落ちることを検証しており、tick 側に新しい分岐は無い。

#### 実測で確かめたこと

`tick` から起動したラッパーを実バイナリで観測し(手動、macOS / git 2.55):

- PGID == PID(4755)・PPID == 1 — `process_group(0)` が効いて実際に切り離されている
- 開いている fd は `/dev/null`(0/1/2)と `stdout.log` / `stderr.log` のみで、**ロックファイルの fd は継承されていない**(Rust の `File` が `O_CLOEXEC` を付けるため)
- pid ファイルの `kill_ident` は `-4755` = `-<pid>`(観測した PGID から作られている)
- starttime ファイルの `ident`(`Wed Aug 12 15:54:39 2026`)が、後から `LC_ALL=C TZ=UTC ps -o lstart=,pgid= -p 4755` で観測した値と完全に一致 — 記録側と照合側が同じ表現を得る構成になっている

#### スコープ・規約

- 「含まれないもの」を越えた変更は見つからなかった。`WorktreeManager::remove` / `RunStore` の gc 系 / `ProcessController` の観測・kill 系 / `NotificationService` / `CommandRunner` はいずれも宣言されていない。
- `git` へのシェルアウトはすべて `Command::new(git_program).arg("-C")...` の形で、シェルを経由せず引数はリテラルのまま渡る。`BranchName::parse` が先頭 `-` を拒むため、ブランチ名経由のオプション注入も成立しない。
- `Result` の握り潰しは `run_wrapper.rs:83`(`write_exit` の失敗を意図的に無視。理由がコメントにある)と `util/atomic.rs` の `sync_dir`(既存)だけ。`expect` は `CommandLine` が1トークン以上・ワークスペース確定済み・リトライ上限の適用対象という不変条件に対してのみ使われている。
- コメントに指摘への弁明・修正の経緯は見つからなかった(diff の追加行を機械的に走査しても該当なし)。残っているのは why / why not とドキュメンテーションコメントのみ。
