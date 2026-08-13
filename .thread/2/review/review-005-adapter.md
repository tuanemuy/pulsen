# レビュー 005 — Adapter / Infrastructure

## Adapter / Infrastructure

**結論: 問題なし（Blocker 0 / Warning 0）。**

観点ごとに実際に確認した内容と根拠を下に残す。無理な粗探しはしていない。

### Blockers

なし

### Warnings

なし

### 確認した内容

**AC-1（ビルド・lint・`cfg` の分布・依存）**

- `cargo fmt --check` / `cargo clippy --all-targets -- -D warnings` / `cargo test` はいずれも緑。全テストターゲット 0 failed。
- ターゲット述語つき `cfg`（`unix` / `windows` / `target_os` / `target_family`）を `crates/*/src/` に対して grep した結果は次のとおりで、AC-1 の記述と一致する。
  - `crates/pulsen-domain/`: 0 件。
  - `crates/pulsen/src/`: `util/atomic.rs`（既存）・`adapter/task_repository.rs`（`#[cfg(all(test, unix))]`・既存）・`adapter/process.rs`（新規）の 3 ファイルのみ。
  - `crates/pulsen-conformance/src/lib.rs` のヒットは能力プローブで、`origin/main` から差分なし（適合ケース側にプラットフォーム分岐は入っていない。ADR-074 の要求どおり）。
- 本 PR が追加する `#![cfg(unix)]` のテストファイルは `crates/pulsen/tests/cli_tick_missing_cwd.rs` の 1 つ。`crates/*/src/` の外なので AC-1 の検査対象には入らず、ファイル冒頭に「Windows では実行中プロセスの cwd を削除できず前提そのものが作れない」という why があり、cwd がプロセス全体の状態であるためファイルを分けるという判断も書かれている。妥当。
- `Cargo.toml` / `Cargo.lock` は本 PR で 1 行も変更されていない（新規依存ゼロ）。`crates/pulsen-domain/Cargo.toml` の `[dependencies]` は空のまま、workspace の `unsafe_code = "forbid"` も維持。`examples/` の 2 本は cargo の自動検出に乗るため manifest 変更が不要という形で成立している。

**起動時刻・PGID の取得手段**

- 取得は `adapter/process.rs` の private モジュール `identity` の `observe(&IdentitySource, Pid) -> Result<Option<ObservedProcess>, Io>` 1 関数に閉じている。`ObservedProcess` が起動時刻と kill 同定子を同時に返すので、POSIX では 1 回の観測（`ps` 1 回 / `stat` 1 回）で両方を得る形になっている。
- 三値（`Ok(Some)` = 取得できた / `Ok(None)` = 対象が不在 / `Err(Io)` = 機構の失敗）が実装されており、二値へ畳む 1 行は `own_identity` 側（呼び出し側）にある。共有関数側では畳んでいない。ADR-067 の写像表と実装の各分岐を突き合わせて一致を確認した。
  - macOS 系: `ps` の起動失敗 → `Err`、非 0 かつ stdout 空 → `Ok(None)`、それ以外の非 0 → `Err`、exit 0 で読めない → `Err`。
  - Linux: procfs ルート不在 → `Err`、`<root>/<pid>/stat` が `NotFound` → `Ok(None)`、その他の読み取り失敗・形式外・boot_id 不可 → `Err`。
  - Windows: `$ErrorActionPreference = 'Stop'` + `catch { exit 3 }` で非終端エラーを終了コードに落としてから、exit 0 かつ出力空を `Ok(None)` に写す。「出力の形」ではなく終了コードで機構失敗を分けており、不在を機構失敗に畳む縮退も、逆に機構失敗を不在に畳む縮退も起きない。
- ユニットテストが「不在は `Ok(None)` になり機構の失敗に畳まれない」「壊した取得元では `Err(Io)` になる」の両方を、実プロセスの起動・終了で押さえている（POSIX 側・Linux 側の双方に同名のテストがある）。三値が実効的なテストで守られている。

**`ps` へのロケール・TZ 注入**

- `identity_command` が `LC_ALL=C` / `TZ=UTC` を注入し、`LANG` / `LC_TIME` を `env_remove` している。`LC_ALL` は他の `LC_*` を上書きするので取りこぼしはない。
- 既定の取得元は `/bin/ps` の絶対パス。PATH 解決に委ねると cron と対話シェルで別実装に解決されうる、という why がコードにも残っている。
- 注入そのものを主張するユニットテスト（`取得はロケールとタイムゾーンを固定した環境で行われる`）が `get_envs()` で `LC_ALL` / `TZ` の設定と `LANG` / `LC_TIME` の削除を検査しており、後から環境固定が外れても検出できる。
- `lstart=,pgid=` の順序固定と「最後の空白で分ける」規則も実装とコメントが一致。`trim` は関数内で 1 回だけ。

**`/proc/<pid>/stat` のパース**

- 「最後の `)` より後ろ」を空白分割し、`PGRP_INDEX = 2` / `STARTTIME_INDEX = 19` で読む。全体の 5 番目（pgrp）と 22 番目（starttime）に正しく対応している。comm に空白や `)` を含む場合のずれを避ける理由もコメントに残っている。
- 起動時刻は `<boot_id>:<ticks>` に合成しており、再起動を跨いだ PID 再利用と起動 tick 一致の同時成立を消している。boot_id も注入されたルート配下から読むので、壊れた取得元の注入がそのまま機構失敗に落ちる。

**デタッチ起動 / FD の継承**

- `spawn_wrapper` は `process_group(0)`（POSIX）/ `CREATE_NEW_PROCESS_GROUP | DETACHED_PROCESS`（Windows）を掛け、stdin / stdout / stderr をすべて `Stdio::null()` にして `Child` を待たずに落としている。`setsid` 相当ではないことと、その帰結（端末セッションの強制終了には `nohup` 相当の耐性が無い）が doc に残っている。
- デタッチ性は適合ケース `TC-port-process-controller-002` が `examples/spawn_probe` で「呼び出し側プロセスを終了させてから run ディレクトリを観測する」形で検証している。同一プロセス内では表現できない性質を、別プロセス経由で実際に確かめている。
- ロック FD の継承は、実バイナリの受け入れテスト `滞留するエージェントを起動したままでも次のtickは競合しない` が押さえている。ラッパーの生存中（`exit` 未出現を先に主張してから）に 2 回目の tick を打ち、「スキップしました」が出ないことを主張する形で、待ち条件が実行環境の速さに依存しない。`FileExclusiveLock` は std の `OpenOptions` でファイルを開くため FD は CLOEXEC で、この主張と実装が整合している。
- ラッパーがエージェントを起動するときも stdin は `Stdio::null()`、stdout / stderr は指定パスのファイル。余分な FD を子へ渡していない。

**アトミック置換・排他ロック**

- `FsRunStore` の write 系 3 種とマーカーはすべて `util::atomic::write_atomic` を呼ぶだけで、置換手順の再実装は無い。ディレクトリ作成も `util::fsdir::ensure_dir` 経由。CLAUDE.md の「個別に再実装しない」を満たしている。
- `write_atomic` は同一ディレクトリの一時ファイル → `sync_all` → `persist` → ディレクトリの `sync_dir` で、適合ケース `TC-port-run-store-016`（並行読み取り）/ `017`（失敗時に部分的内容を残さない）が実際に走って通っている（この環境では権限制限が効くのでスキップされていない）。
- ロックは既存の `FileExclusiveLock` のまま。tick / wrapper のどちらもロック機構を再実装していない（ラッパーはロックを取らない）。

**read 系の 3 分類**

- `read_json` が `NotFound` → `Ok(None)`、JSON として読めない → `Corrupt`、その他 → `Io` に写し、値制約の破れ（`KillIdent` の非空・`ProcessStartTime` の非空・`Timestamp` の RFC3339）も `Corrupt` に落ちる。ディレクトリごと不在も `NotFound` として同じ経路に合流する契約がコメントに明記されている。
- `marker_exists` は `exists()` ではなく `try_exists()` を使い、機構の失敗を「無い」に丸めていない。ラッパーの起動抑止の判断そのものであるという why も添えられている。
- 適合スイートは 21 行 + 追加 1 件の計 22 ケースが全件走って通った（skip なし）。追加 1 件（`write_準備を経ない書き込みも置き場ごと作って残る`）は、`prepare_attempt` 失敗後も spawn を続ける設計が依拠している「write 系は置き場ごと作る」契約を、台帳行の無い 3 メソッドについて実行している。HOOKS.md でも件数に数えず「追加ケース」として区別されている。

**`GitCliWorktreeManager::create` の冪等性の境界**

- 同定は `physical_key(p) = canonicalize(p.parent()).join(p.file_name())` の 1 関数に閉じ、`ws.path` と `worktree list --porcelain` の各エントリの**両側**に対称に適用している。生のパスの文字列比較は残っていない。親は `ensure_dir` 済みなので鍵の生成が必ず成立する。
- 分岐は次のとおりで、spec の境界（「`ws.path` に `ws.branch` の worktree がある」場合だけが達成済み）と一致する。
  - 登録あり・ブランチ一致・実体あり・`prunable` なし → `Ok`（達成済み）。
  - 登録あり・ブランチ一致・実体なし or `prunable` → `worktree add -f`（保護を外す範囲が対象パス 1 つに閉じることも確認済み）。
  - 登録あり・ブランチ不一致（`branch` が `None` の detached も含む）→ `Failed`。自動修復しない。
  - 登録なし・実体あり → `Failed`。パスの存在だけでは達成済みにしない。
  - 登録なし・実体なし・ブランチあり → `worktree add`（`-f` なし。先端を変えない）。
  - 登録なし・実体なし・ブランチなし → `worktree add -b <branch> <path> <base>`。
- 実体の有無を `prunable` の注記ではなく `try_exists()` で直接観測している点は、注記を出さない git バージョンでも達成済みへ倒れないという意味で ADR-077 より強く、正しい方向の実装。
- 復旧の 2 分岐（`prunable` からの `add -f` / 登録なし・ブランチのみからの `add`）は、それぞれ `create_prunable_...` の追加ケースと `TC-port-worktree-manager-013` が実行しており、片方が落ちてもスイートが緑のままにならない構成になっている。ハーネスは `worktree_root` をシンボリックリンク経由で組んでおり、正規化の分岐が実際に通ることが担保されている（適合スイート 17 件が全件走って通過）。
- CLI 側の受け入れテスト `ブランチだけが残っている残骸には先端を変えずにworktreeを張り直す` が、先端不変と積まれた成果物の復帰まで実バイナリで確認している。

**git CLI のシェル非経由**

- `output()` は `Command::new(git_program).arg("-C").arg(repo).args(args)` の形で、シェルを一切介さない。パスは `OsStr` のまま渡されており文字列連結も無い。`INHERITED_GIT_ENV` を落とす規則が 1 箇所（`output`）に集約され、`run` / `run_worktree` / `require_success` はその上に載る形になっている。
- `run_agent` も `Command` の直接起動で、適合ケース `TC-port-process-controller-021` がシェルのメタ文字（`*` / `$HOME` / `&&` / `>out.txt` / `;` / 空白入りトークン）のリテラル通過を主張している。CLI 往復も `cli_wrapper.rs::シェルのメタ文字や空文字列を含むトークンはリテラルのまま渡る` で空文字列トークンと `--model` を含めて押さえられており、`trailing_var_arg` + `allow_hyphen_values` の必要性が実測で守られている。

**エラーの握り潰し・`unwrap`**

- 本 PR が追加したアダプター本体（`process.rs` / `run_store.rs` / `worktree.rs` / `wire.rs`）に `unwrap()` は無い。`expect` は `run_agent` の `split_first`（`CommandLine` の「1 トークン以上」という不変条件）1 箇所のみで、CLAUDE.md の「パニックは不変条件違反にのみ」に合致する。
- `let _ =` によるエラー破棄はテストの後片付け（子プロセスの kill / wait）に限られる。本番経路で結果を捨てている箇所は無い。
- `WorktreeError` / `Io` / `RunFileError` の各メッセージは原因を含み、`create` の失敗メッセージには突き合わせた鍵と登録の指す先が載る（同定が外れたのか本当に別の worktree が居るのかを分けられるようにするため、という why 付き）。

**残す必要のない記述**

- コード・コメントを通読したが、指摘への弁明・修正の経緯・レビュー周回への言及は見つからなかった。長めのコメントはいずれも「なぜこの形か / なぜ別の形を採らないか」に閉じており、CLAUDE.md の方針どおり。
- `.adr/027` の変更は「対応表の正本を HOOKS.md 一本にする」という決定の更新で、正本が 2 つあることによる恒久記録の書き換え連鎖を止める内容。妥当。

### カバレッジ

- 確認: `.adr/027-port-conformance-suite-and-harness-hooks.md`, `.thread/2/adr.md`, `.thread/2/plan.md`, `.thread/2/testing.md`, `crates/pulsen-conformance/HOOKS.md`, `crates/pulsen-conformance/src/doubles/clock.rs`, `crates/pulsen-conformance/src/doubles/mod.rs`, `crates/pulsen-conformance/src/doubles/process.rs`, `crates/pulsen-conformance/src/doubles/run_store.rs`, `crates/pulsen-conformance/src/doubles/worktree.rs`, `crates/pulsen-conformance/src/lib.rs`, `crates/pulsen-conformance/src/process_controller.rs`, `crates/pulsen-conformance/src/run_store.rs`, `crates/pulsen-conformance/src/worktree_manager.rs`, `crates/pulsen/examples/agent_probe.rs`, `crates/pulsen/examples/spawn_probe.rs`, `crates/pulsen/src/adapter/mod.rs`, `crates/pulsen/src/adapter/process.rs`, `crates/pulsen/src/adapter/run_store.rs`, `crates/pulsen/src/adapter/worktree.rs`, `crates/pulsen/src/cli/args.rs`, `crates/pulsen/src/cli/mod.rs`, `crates/pulsen/src/cli/tick.rs`, `crates/pulsen/src/cli/wire.rs`, `crates/pulsen/src/cli/wrapper.rs`, `crates/pulsen/tests/cli_tick.rs`, `crates/pulsen/tests/cli_tick_missing_cwd.rs`, `crates/pulsen/tests/cli_wrapper.rs`, `crates/pulsen/tests/common/git.rs`, `crates/pulsen/tests/common/mod.rs`, `crates/pulsen/tests/conformance_process_controller.rs`, `crates/pulsen/tests/conformance_run_store.rs`, `crates/pulsen/tests/conformance_worktree.rs`（33 ファイル）
- スキップ: `.thread/2/progress.md` — 進捗の記録であり、コード・契約の観点を持たない
- スキップ: `.thread/2/review/review-001-adapter.md`, `.thread/2/review/review-001-architecture.md`, `.thread/2/review/review-001-domain.md`, `.thread/2/review/review-001-test.md`, `.thread/2/review/review-001-usecase.md`, `.thread/2/review/review-001.md`, `.thread/2/review/review-002-adapter.md`, `.thread/2/review/review-002-architecture.md`, `.thread/2/review/review-002-domain.md`, `.thread/2/review/review-002-test.md`, `.thread/2/review/review-002-usecase.md`, `.thread/2/review/review-003-adapter.md`, `.thread/2/review/review-003-architecture.md`, `.thread/2/review/review-003-domain.md`, `.thread/2/review/review-003-test.md`, `.thread/2/review/review-003-usecase.md`, `.thread/2/review/review-003.md`, `.thread/2/review/review-004-adapter.md`, `.thread/2/review/review-004-architecture.md`, `.thread/2/review/review-004-domain.md`, `.thread/2/review/review-004-test.md`, `.thread/2/review/review-004-usecase.md`, `.thread/2/review/review-004.md`, `.thread/2/review/triage.md` — ゼロベースでレビューする指示のため、過去ラウンドの成果物は読まない（24 ファイル）
- スキップ: `.thread/2/steps.md` — 実装手順の記録であり、成果物の検証対象ではない
- スキップ: `crates/pulsen-conformance/src/doubles/task_repository.rs`, `crates/pulsen-conformance/src/doubles/tests.rs` — 既存ダブルの拡張とダブル自身のテスト。テスト観点の担当
- スキップ: `crates/pulsen-domain/src/definition/agent.rs`, `crates/pulsen-domain/src/definition/template.rs`, `crates/pulsen-domain/src/execution/launching.rs`, `crates/pulsen-domain/src/execution/mod.rs`, `crates/pulsen-domain/src/execution/port.rs`, `crates/pulsen-domain/src/execution/value.rs`, `crates/pulsen-domain/src/task/attempt.rs`, `crates/pulsen-domain/src/task/counters.rs`, `crates/pulsen-domain/src/task/failure.rs`, `crates/pulsen-domain/src/task/mod.rs`, `crates/pulsen-domain/src/task/path.rs`, `crates/pulsen-domain/src/task/planner.rs`, `crates/pulsen-domain/src/task/task.rs`, `crates/pulsen-domain/src/task/transition.rs` — ドメイン層。ドメイン観点の担当（ポート宣言の妥当性は `execution/port.rs` を参照せず、アダプター実装側の適合状況として確認した）（14 ファイル）
- スキップ: `crates/pulsen/src/application/mod.rs`, `crates/pulsen/src/application/run_wrapper.rs`, `crates/pulsen/src/application/tick/confirm_spawn.rs`, `crates/pulsen/src/application/tick/launch.rs`, `crates/pulsen/src/application/tick/mod.rs` — ユースケース層。ユースケース観点の担当（5 ファイル）
- スキップ: `crates/pulsen/src/cli/add.rs`, `crates/pulsen/src/cli/render.rs` — 既存コマンドの配線と文言の組み立て。アダプター観点に関わらない
- スキップ: `crates/pulsen/tests/cli_usage.rs` — ヘルプ表示の受け入れテスト。CLI 観点の担当
- スキップ: `crates/pulsen/tests/register_task.rs` — 既存 `add` の受け入れテスト。テスト観点の担当
- スキップ: `crates/pulsen/tests/run_wrapper.rs`, `crates/pulsen/tests/tick_confirm_spawn.rs`, `crates/pulsen/tests/tick_fixture/mod.rs`, `crates/pulsen/tests/tick_launch.rs`, `crates/pulsen/tests/tick_scan.rs` — テストダブルに対するユースケース層テスト。ユースケース／テスト観点の担当（5 ファイル）

確認 33 + スキップ 56 = 89。
