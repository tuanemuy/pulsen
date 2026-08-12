# レビュー2周目 — Issue #2

## Adapter / Infrastructure

検証環境: macOS (Darwin 25.4.0) / git 2.x / 非 root / TMPDIR は `/var/folders/...`（`/private` 経由のシンボリックリンクあり）。
実行した確認: `cargo build --examples`、`cargo test --workspace`（全緑）、`cargo clippy --workspace --all-targets -- -D warnings`（無警告）、`cargo fmt --check`（差分なし）、適合スイート3本の個別実行（ProcessController 16 / RunStore 22 / WorktreeManager 17、いずれもスキップ0）、AC-1 の `cfg` grep。

### Blockers

なし。

### Warnings

- **[W-001]** Windows の同定情報取得が ADR-067 の写像表から外れ、「不在」を「機構の失敗」に畳む経路を作っている
  - 場所: `crates/pulsen/src/adapter/process.rs:683-695`
  - 理由: ADR-067 の写像表は Windows について「exit 0 かつ出力が空 → `Ok(None)`（不在）」「それ以外の失敗・形式外 → `Err(Io)`」の2行しか置いていない。実装はこれに加えて「exit 0・stdout 空・**stderr 非空** → `Err(Io)`」という3本目の分岐を持つ。スクリプトは `$ErrorActionPreference = 'Stop'` + `catch { exit 3 }` で機構の失敗をすでに**終了コード**に落としており（同ファイル 646, 658-664）、この分岐が拾うのは「exit 0 で完走したが stderr に何か出た」場合だけになる。PowerShell は非終端の警告・情報ストリームや実行ポリシー等の通知を stderr に流すことがあり、そのとき**死亡したプロセスが `Err(Io)` として返る**。ADR-067 が「畳むと #3 で running のまま永久滞留になる」と書いた縮退そのものに、Windows だけが到達する。Linux / macOS 側は不在と機構失敗を厳密に分けているので、三値の一貫性がプラットフォーム間で崩れている。
  - 提案: 分岐を落とし、終了コードだけで機構失敗を判定する（`exit 0` かつ trim 後が空なら `Ok(None)`）。診断を残したいなら `Ok(None)` を返したうえで stderr を捨てるのではなく、`MECHANISM_FAILURE` を返す条件をスクリプト側に足す（例: `$p` が null でも CIM 接続自体の失敗は `catch` で捕まる）。いずれにせよ Windows 実機検証は #10 だが、写像の**決定**は本スライスの責務（ADR-067 の宣言どおり）なので、表と実装の食い違いはここで閉じたい。

- **[W-002]** `spawn_wrapper` が常に失敗する `ProcessController` 実装が本番の合成に結線されており、ADR-068 からの逸脱が記録されていない
  - 場所: `crates/pulsen/src/adapter/process.rs:98-110, 113-116`、`crates/pulsen/src/cli/wire.rs:224-238`
  - 理由: ADR-068 は `SystemProcessController::new(self_exe, identity_source, clock)` の3引数固定と、「`wire::process_controller()` を `cli/tick.rs` と `cli/wrapper.rs` から呼ぶ」ことを決めている。実装は `self_exe: Option<PathBuf>` に変え、`without_self_exe` を足して `compose_wrapper` がこちらを使う形になった。判断自体は妥当（ラッパーで `current_exe()` が失敗すると、何も書かずに非0終了 → 猶予経路が spawn 失敗として積む、という余計な失敗経路が増える）だが、結果として **本番に結線されるポート実装のうち1つは `spawn_wrapper` が構造上必ず `SpawnError` を返す**。適合スイートが適用されるのは `new(...)` の側だけで、この構成は契約検証の外にある。後続スライスがラッパー側の経路で `spawn_wrapper` を使うようになっても、コンパイルは通り実行時エラーになるだけで、型が止めない（CLAUDE.md「不正な状態を型で表現不能にする」との緊張）。加えて progress.md の「spec 追従の提起」にも「既知の制限」にもこの逸脱が載っていない — ADR-071 / 073 / 079 など他の逸脱はすべて記録されているので、記録の抜けとして目立つ。
  - 提案: (a) ADR-068 に「ラッパーの合成は `self_exe` を持たない」分岐を追記して逸脱を記録に残す、かつ (b) 型で閉じるなら `spawn_wrapper` を持たない狭いポート（例: `WrapperProcess`: `own_identity` + `run_agent`）を切り出し、`SystemProcessController` がその上位に載る形にする。(b) が本スライスに重いなら、最低限 (a) と、`without_self_exe` の doc に「この構成は `spawn_wrapper` の適合契約の対象外である」ことを明記する。

- **[W-003]** POSIX 非 Linux / Windows の同定情報の取得元が PATH 解決される名前で、「記録と照合は同一の取得手段」の保証が PATH の安定性に依存している
  - 場所: `crates/pulsen/src/adapter/process.rs:255-257`（`PathBuf::from("ps")`）、`crates/pulsen/src/adapter/process.rs:638-640`（`PathBuf::from("powershell")`）
  - 理由: ADR-067 は `lstart` の表現が環境で揺れる問題を `LC_ALL=C` / `TZ=UTC` の注入で潰した。しかし固定したのは**実行時の環境**だけで、**どの実行ファイルが起動されるか**は PATH に委ねられている。cron の tick（`PATH=/usr/bin:/bin`）と対話シェルの tick（`PATH=/opt/homebrew/bin:...`）で `ps` が別実装に解決されれば、`lstart` の整形が変わりうる。結末はロケール非固定の場合とまったく同じ — `IdentityCheck = Dead` → `DiedWithoutExit` → 再起動 → 同一 worktree での並走で、ADR-067 と plan.md が「最悪の失敗モード」と呼んだ経路。Linux は `/proc` という絶対パスなので影響しない（この非対称も、Linux でだけ検証が通って macOS で漏れる形になっている）。`git` を PATH 解決している既存の判断（ADR-024）とは性質が違う — git は結果を分類にしか使わず、`ps` の出力は**帳簿に永続化されて後の tick と突き合わされる**。
  - 提案: `identity::default_source()` を絶対パス（POSIX 非 Linux は `/bin/ps`。Windows は `powershell` のままでも、あるいは `%SystemRoot%\System32\WindowsPowerShell\v1.0\powershell.exe`）にする。注入点（ADR-068）はそのままで、既定値だけを絶対パスにすれば適合テストの構成もアダプターのユニットテストも変わらない。難しければ、少なくとも「PATH が tick 間で変わると照合が壊れうる」ことを progress.md の既知の制限に残す。

- **[W-004]** `run_agent` がリダイレクト先を開けないとき、片方のログだけが空ファイルとして残る
  - 場所: `crates/pulsen/src/adapter/process.rs:168-172`
  - 理由: `(File::create(stdout), File::create(stderr))` はタプルとして両辺が評価されるので、stdout を開けなくても stderr は作られる。spec は「リダイレクト先を開けない場合はエージェントを**起動せず** 126」と定めており、`TC-port-process-controller-025` の主張も「126 はエージェントの副作用が生じていないこと」。エージェント自体の副作用は確かに無いが、`run_agent` 自身が run ディレクトリに空の `stderr.log` を残す。#4 の show や #6 の gc（`last_activity`）が run ディレクトリのファイルを見るので、「エージェントは走っていないのにログがある」状態が帳簿に残る形になる。コメントは現象を認めているが、避けられない事情ではない。
  - 提案: `let Ok(out) = File::create(stdout) else { return ExitCode::new(NOT_EXECUTABLE); };` のように短絡させ、stdout が開けたときだけ stderr を開く。stderr 側が失敗したときに stdout.log が空で残る非対称は残るが、「先に開いたものだけが残る」で規則が1つになる。

### 確認した観点と結果（指摘に至らなかったもの）

- **`cfg` の分布（AC-1）**: `grep -rn 'cfg(\(unix\|windows\|target_os\|target_family\)' crates/*/src/` のヒットは `pulsen-conformance/src/lib.rs`（能力プローブ・既存）、`pulsen/src/util/atomic.rs`（既存）、`pulsen/src/adapter/process.rs`（新規6件）のみ。`crates/pulsen-domain/` は0件。`adapter/task_repository.rs:278` の `#[cfg(all(test, unix))]` は `cfg(` 直後が `all` なので AC-1 の grep（「`cfg(` に続く4述語」）には掛からず、かつ main 時点から存在する。期待どおり。
- **依存と `unsafe`**: `Cargo.toml` は3ファイルとも未変更、`unsafe_code = "forbid"` は workspace lints のまま。`libc` / `nix` / `rustix` / `sysinfo` の追加なし。デタッチは `CommandExt::process_group(0)` / `creation_flags`、シグナル符号化は `ExitStatusExt::signal()` で、すべて安全 API。
- **取得手段の単一化と三値**: `mod identity` は `adapter::process` の中の非公開モジュールで、プラットフォームごとに `observe(&IdentitySource, Pid) -> Result<Option<ObservedProcess>, Io>` が1つだけコンパイルされる。`own_identity` が `Ok(None)` を `Err(Io)` に畳む1行を**呼び出し側**に置いている（`process.rs:145-148`）ので、#3 の `starttime_of` は共有関数をそのまま呼べる。三値は各プラットフォームのユニットテスト（`存在しないプロセスは不在として返り機構の失敗に畳まれない` / `壊した取得元では不在ではなく機構の失敗になる`）と適合ケース `TC-005` の両方で守られている。
- **ロケール・TZ の注入**: `identity_command` が `LC_ALL=C` / `TZ=UTC` を設定し `LANG` / `LC_TIME` を `env_remove` する。ユニットテスト `取得はロケールとタイムゾーンを固定した環境で行われる` が `get_envs()` を直接主張しており、注入が消えれば落ちる。`lstart=,pgid=` の順固定と `rsplit_once(whitespace)` → `trim` の組み合わせも実測どおり。
- **`/proc/<pid>/stat` のパース**: `rsplit_once(')')` で comm を切り落としてから空白分割し、`pgrp` = 索引2、`starttime` = 索引19（全体の5番目 / 22番目）。comm に空白・`)` を含んでもずれない。boot id との合成で boot を跨いだ一致も消えている。
- **デタッチ性とロック FD**: `TC-port-process-controller-002` が `examples/spawn_probe` の**終了後**に starttime / pid / exit が揃うことを主張し、スキップ許容集合に入れていない。加えて受け入れテスト `次のtickはpidの出現をもってrunningへ取り込む` が `kill_ident == "-<ラッパーのpid>"` を主張しており、これは `process_group(0)` が効いていなければ（呼び出し側のグループを指すので）必ず落ちる — デタッチ性を pid ファイルの値そのもので裏付けている。ロック FD の非継承は `滞留するエージェントを起動したままでも次のtickは競合しない`（`cli_tick.rs:161-182`）が行動として観測している。`FileExclusiveLock` は std の `OpenOptions`（Unix では `O_CLOEXEC`）しか使っておらず、子へ渡る FD は `Stdio::null()` の3本だけ。
- **アトミック置換の共通化**: `FsRunStore` の write 系3種とマーカーはすべて `util::atomic::write_atomic` を1行呼ぶだけで、置換・`ensure_dir`・一時ファイルの後始末を再実装していない。`TC-port-run-store-016 / 017` が「書きかけを観測しない」「失敗しても部分的な内容を残さない」を実測で通す（`017` は `permission_restrictions_effective` が真の本環境で実際に走った）。
- **read 系の3分類**: `read_json` が `NotFound` を `Ok(None)`（ディレクトリごと不在も同じ `NotFound` として現れるためコメントどおり合流）、JSON 構文と値制約の破れを `Corrupt { path, message }`、その他の I/O を `Io` に写す。`marker_exists` が `exists()` ではなく `try_exists()` を使い、機構の失敗を「無い」に丸めていない点も適切。
- **`GitCliWorktreeManager::create` の冪等性の境界**: 鍵は `physical_key(p) = canonicalize(p.parent()).join(p.file_name())` の1関数に閉じ、`ws.path` と `worktree list --porcelain` の各エントリの**両側**を同じ関数に通してから比較している（Windows の `\\?\` 接頭辞と git の `C:/` 表記が正規化の結果として揃う）。生のパスの文字列比較は無い。達成済みの条件は「鍵一致 **かつ** ブランチ一致 **かつ** 実体が在る **かつ** `prunable` でない」。別ブランチの worktree / 通常ディレクトリ / base 不在はいずれも `Failed` で、ブランチも作らないことを適合ケースが主張している。ブランチのみ残存は `-f` **なし**の `worktree add` で、先端の不変とコミット済み成果物の復帰を `branch_tip` / `worktree_marker` で観測している。ハーネスの `worktree_root` はシンボリックリンク経由なので、正規化の分岐が確実に通る。
- **git のバージョン差**: 実体の有無を `entry.prunable` の注記だけでなく `path.try_exists()` で直接観測しているため、`prunable` を出さない git でも「登録あり・実体なし」が `add -f` の分岐に落ちる。注記に依存していない点は AC-8 の「バージョン差で判定が倒れないか」を満たす。
- **シェル非経由・リテラル渡し**: git も `ps` も `powershell` もエージェントも `Command::new(...).args(...)`。`create` の可変長引数は `OsStr` のまま渡しており、パスの非 UTF-8 でも壊れない。`TC-port-process-controller-021` が `*` / `$HOME` / `&&` / `>out.txt` / `{input}` / 空白入りトークン / `;` を `echo-args` の出力と突き合わせて、リテラル一致そのものを観測している（ADR-080 の反転）。
- **エラーの取りこぼし**: 本番コードの `expect` は `run_agent` の `CommandLine は1トークン以上であることが不変条件` 1箇所のみで、`CommandLine` の parse 境界が保証する不変条件（CLAUDE.md の許容範囲）。`let _ =` の握り潰しは `run_wrapper` の `write_exit`（tick 側の「exit なし = 死亡」規則に合流させる意図つき）と `atomic::sync_dir`（既存）だけ。`physical_key` の `.ok()?` は不一致に写す設計がコメントで明示されている。
- **HOOKS.md の整合**: 125行 → 169行（+44 = RunStore 21 + ProcessController 16 + WorktreeManager 7）、区分別 A 28→38 / B 85→113 / C 12→18 がすべて内訳と一致。ポート数7→9、「環境で走らなくなりうる行」の権限依存8→12行と残る C 6行（うち `TC-003` / `005` は注入で確定的に走るため表外）の説明も数が合う。
- **弁明・経緯コメント**: 差分の追加コメントを grep したが、指摘への応答・修正履歴・「以前は〜だった」の類は無い。残っているのは why / why not のみ。

### カバレッジ

- 確認: `crates/pulsen/src/adapter/process.rs`, `crates/pulsen/src/adapter/run_store.rs`, `crates/pulsen/src/adapter/worktree.rs`, `crates/pulsen/src/adapter/mod.rs`, `crates/pulsen/src/cli/wire.rs`, `crates/pulsen/src/cli/wrapper.rs`, `crates/pulsen/src/cli/tick.rs`, `crates/pulsen/src/cli/args.rs`, `crates/pulsen/src/application/run_wrapper.rs`, `crates/pulsen/examples/agent_probe.rs`, `crates/pulsen/examples/spawn_probe.rs`, `crates/pulsen/tests/conformance_process_controller.rs`, `crates/pulsen/tests/conformance_run_store.rs`, `crates/pulsen/tests/conformance_worktree.rs`, `crates/pulsen/tests/cli_wrapper.rs`, `crates/pulsen/tests/cli_tick.rs`, `crates/pulsen/tests/common/git.rs`, `crates/pulsen/tests/common/mod.rs`, `crates/pulsen-conformance/src/lib.rs`, `crates/pulsen-conformance/src/run_store.rs`, `crates/pulsen-conformance/src/process_controller.rs`, `crates/pulsen-conformance/src/worktree_manager.rs`, `crates/pulsen-conformance/src/doubles/process.rs`, `crates/pulsen-conformance/src/doubles/run_store.rs`, `crates/pulsen-conformance/HOOKS.md`
  （差分外の参照: `crates/pulsen/src/util/atomic.rs`, `crates/pulsen/src/util/fsdir.rs`, `crates/pulsen/src/adapter/lock.rs`, `Cargo.toml` 3件, `crates/pulsen/src/application/tick/launch.rs` の `create` 呼び出し箇所, `spec/domains/execution.md`, `spec/requirements.md`, `CLAUDE.md`, `.thread/2/plan.md`, `.thread/2/adr.md`, `.thread/2/progress.md`）
- スキップ: `.thread/2/adr.md`, `.thread/2/plan.md`, `.thread/2/progress.md` — 判定の基準として読んだが、レビュー対象の成果物としては扱っていない（設計文書はアーキテクチャ観点の担当）
- スキップ: `.thread/2/review/review-001-adapter.md`, `.thread/2/review/review-001-architecture.md`, `.thread/2/review/review-001-domain.md`, `.thread/2/review/review-001-test.md`, `.thread/2/review/review-001-usecase.md`, `.thread/2/review/review-001.md`, `.thread/2/review/triage.md` — 1周目の記録。ゼロベース判定のため読まない指示
- スキップ: `.thread/2/steps.md`, `.thread/2/testing.md` — 進行管理・検証手順の記録で、アダプター実装の判定材料にならない
- スキップ: `crates/pulsen-domain/src/definition/agent.rs`, `crates/pulsen-domain/src/definition/template.rs`, `crates/pulsen-domain/src/execution/launching.rs`, `crates/pulsen-domain/src/execution/mod.rs`, `crates/pulsen-domain/src/execution/value.rs`, `crates/pulsen-domain/src/task/attempt.rs`, `crates/pulsen-domain/src/task/counters.rs`, `crates/pulsen-domain/src/task/failure.rs`, `crates/pulsen-domain/src/task/mod.rs`, `crates/pulsen-domain/src/task/path.rs`, `crates/pulsen-domain/src/task/planner.rs`, `crates/pulsen-domain/src/task/task.rs`, `crates/pulsen-domain/src/task/transition.rs` — ドメイン層。`unsafe` と `cfg` の不在、および `RunDirPath` の導出関数がアダプターの前提を満たすことだけを確認し、遷移ロジック・型設計はドメイン観点の担当
- スキップ: `crates/pulsen-domain/src/execution/port.rs` — ポート定義。宣言されたメソッドが AC-6 の9+3+1 と一致することと `Io` の共有条件（ADR-078）が doc にあることは確認したが、契約文言の妥当性はドメイン / アーキテクチャ観点の担当
- スキップ: `crates/pulsen/src/application/mod.rs`, `crates/pulsen/src/application/tick/mod.rs`, `crates/pulsen/src/application/tick/launch.rs`, `crates/pulsen/src/application/tick/confirm_spawn.rs` — ユースケース層。`create` の呼び出し条件（workspace 未確定時のみ）だけを AC-16 の裏取りに使い、手続きの順序・分類はユースケース観点の担当
- スキップ: `crates/pulsen/src/cli/mod.rs`, `crates/pulsen/src/cli/render.rs` — 文言の組み立てと終了コードの写像。CLI / ユースケース観点の担当
- スキップ: `crates/pulsen-conformance/src/doubles/clock.rs`, `crates/pulsen-conformance/src/doubles/mod.rs`, `crates/pulsen-conformance/src/doubles/task_repository.rs`, `crates/pulsen-conformance/src/doubles/tests.rs`, `crates/pulsen-conformance/src/doubles/worktree.rs` — テストダブルの拡張。ポート実装ではあるが実 I/O を持たず、アダプター層の判定材料にならない（テスト観点の担当）
- スキップ: `crates/pulsen/tests/tick_scan.rs`, `crates/pulsen/tests/tick_launch.rs`, `crates/pulsen/tests/tick_confirm_spawn.rs`, `crates/pulsen/tests/tick_fixture/mod.rs`, `crates/pulsen/tests/run_wrapper.rs` — ダブルに対するユースケーステスト。実アダプターを通らないためテスト / ユースケース観点の担当
- スキップ: `crates/pulsen/tests/cli_usage.rs`, `crates/pulsen/tests/register_task.rs` — サブコマンドの見え方と既存の登録経路の受け入れ。CLI / テスト観点の担当
