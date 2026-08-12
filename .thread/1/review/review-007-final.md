# 最終レビュー（7周目・収束確認） — Issue #1 / PR #8

対象: PR #8（ベース `main`、HEAD `4ed6866`）。重点は6周目以降の2コミット分（`git diff HEAD~1...HEAD`）で、あわせて `.thread/1/plan.md` の AC-1〜AC-20 を再検証した。

ラウンド1〜6の台帳（`.thread/1/review/triage.md`）を読み、1周目に `wont-fix` と判定された3件（W-004 `unreachable!` / W-012 成功時の tick 案内 / W-017 `create` の TOCTOU）は本ラウンドでも蒸し返していない。

## 結論

**Blocker 0 / Warning 1**。

6周目以降の変更（`crates/pulsen/tests/common/lock.rs` の期限つき合図待ち、`HOOKS.md` / `progress.md` の表への4行追加）が生んだ**回帰はゼロ**。製品コード（ドメイン・アダプター・アプリケーション・CLI）に残る欠陥もゼロで、AC-1〜AC-20 は全件合格。

唯一の指摘 W-001 は、**6周目のレビュー（`review-006-final.md`）が挙げた2件のうち1件が台帳（`triage.md` ラウンド6）に取り込まれておらず、未修正のまま残っている**というもの。新規に見つけた欠陥ではなく、記帳の取りこぼしによる持ち越しである。

## Blockers

なし。

## Warnings

- **[W-001]** 適合ケース `tc_port_exclusive_lock_002` に期限が無く、「解放を待つ」実装に当てるとテストバイナリがハングする（6周目 W-001 の持ち越し・未修正）
  - 場所: `/Users/hikaru/github.com/tuanemuy/pulsen/crates/pulsen-conformance/src/exclusive_lock.rs:43-56`
  - 理由: TC-002 は `hold_from_other_process` で別プロセスに保持させたうえで `harness.lock().try_acquire()` を**呼び出しスレッドで**呼び、保持の解放（`release_holder`）をその**後ろ**に置く。ケースが名指しする失敗モード（競合時に解放を待って返る実装）に当てると、`try_acquire` が返らず `release_holder` にも到達しないため互いを待って停止する。同じスイートの TC-003 は `thread::scope` + 期限監視でこの形を明示的に避けており（`:58-63` の doc が理由を書いている）、TC-002 だけが取り残されている。ADR-060 / ADR-063 が定めた「並行の待ちは期限を持ち、ハングではなく失敗させる」の適用漏れで、同型の指摘は2周目 R2-W-026（TC-003）・3周目 R3-W-002（TC-042/044）・6周目 R6-W-002（`spawn_holder` の合図待ち）といずれも `fix` 判定されている。
  - 影響の範囲: 現行の `FileExclusiveLock` は `try_lock` を使う非ブロッキング実装なので、本 PR のテストは緑のまま（実測でも `conformance_lock` は 7 passed）。効くのは AC-8 が約束する「後続スライスの別実装に同じスイートを当てる」場面で、そこでは診断の無い無限待ちになる。
  - 6周目の修正（`common/lock.rs` の 10 秒期限）では解消しない: あの期限が効くのは**保持プロセスの起動と合図の受信まで**で、TC-002 のハングは合図を受け取って `hold_from_other_process` が `Some` を返した**後**の `try_acquire` で始まる。
  - 実測（本ラウンドで再現）: 「解放を待つ」`ExclusiveLock` 実装とそれを返すハーネスを一時テストとして置き、同じ実装に対して2ケースを走らせた。
    - `tc_port_exclusive_lock_002` … 45 秒の外部タイムアウトで打ち切られるまで `running 1 test` から進まず、結果行が出ない（ハング）。
    - `tc_port_exclusive_lock_003` … `5.02s` で `FAILED`、パニックメッセージは `保持の解放を待たずに返る`（`exclusive_lock.rs:102`）。
    - 一時テストは検証後に削除し、`git status` に残っていないことを確認した。
  - 記帳の状況: `review-006-final.md` の結論は「Blocker 0 / **Warning 2**」で、W-001 がこの件、W-002 が `HOOKS.md` の表。ところが `triage.md` の「ラウンド6」節は「Warning 1」として R6-W-001（`HOOKS.md`）しか載せていない（別途 R6-W-002 としてメインが観測した `spawn_holder` のハングを追加）。`wont-fix` としての判断も記録されていないため、**判定を経ずに落ちた**とみられる。
  - 提案: TC-003 の `thread::scope` + 期限監視を共通ヘルパー（例 `fn attempt_within(lock: &(impl ExclusiveLock + Sync), limit: Duration) -> Option<Attempt>`）に括り出し、TC-002 も経由させる。`Harness::Lock: Sync` の要求が TC-002 にも広がるが、`FileExclusiveLockHarness` は TC-003 のために既に同じ境界を満たしているのでハーネス側の負担は増えない。ADR-060 の「この1ケースだけの要求」の記述と `HOOKS.md:190` の TC-003 の注記も2ケースへ広げる形に揃える。**引き取らないと判断する場合は、`triage.md` に `wont-fix` として理由を残すこと**（現状は判定そのものが無い）。

## 検証の記録

### 6周目以降の変更のレビュー

対象は `git diff HEAD~1...HEAD` の2コミット分。コード変更は `crates/pulsen/tests/common/lock.rs` の1ファイルのみ（+24 / -3）。

**`common/lock.rs` の期限つき合図待ち — 回帰なし。**

| 観点 | 判定 |
|---|---|
| スレッドリーク | 無い。期限超過時は `holder.kill()` → `holder.wait()` で子プロセスを刈るため、子の stdout 書き込み端が閉じて読み手スレッドの `read_line` が `Ok(0)` で返る。送信先の `Receiver` は落ちているので `send` は `Err` になるが `let _` で捨てており、スレッドは必ず終了する |
| `recv_timeout` の扱い | 3分岐すべてを網羅している。`Ok(Some(_))` = 合図あり、`Ok(None)` = `read_line` が I/O エラー、`Err(_)` = 期限超過または送信側の消失（読み手スレッドのパニック）。後2者は同じ後始末（kill + wait → `None`）に落ちるので、待ち方の失敗が黙って通る枝は無い |
| `holder.kill()` 後の `wait()` | 正しい。`stdout` は `take()` 済みで `Child` から外れているため `wait()` がパイプで詰まることはなく、`stdin` は `Child` が保持したまま `wait()` → 関数を抜けて drop される。既に終了しているプロセスに対する `kill()` の `Err` も `let _` で無視され、`wait()` が刈り取る |
| 正常系のオーバーヘッド | 実質ゼロ。`lock_holder` は非ブロッキングの `try_acquire` の直後に `locked` を1行書いて `flush` するため、合図は即座に届き `recv_timeout` はすぐ返る。増えるのは呼び出しごとのスレッド生成1回だけで、`conformance_lock` は単体実行で `0.18s`（4ケース実走）と修正前と変わらない |
| フレーキーの余地 | 実質ない。期限 10 秒に対し、実測される合図までの時間はミリ秒オーダー（`conformance_lock` の7ケース全体で 0.18s）。負荷で 10 秒を超えた場合は**スキップではなく失敗**になるが、これは R6-W-002 の意図どおり（`holder_program()` が存在する環境では許容集合が空なので、`SkipBudget` が宣言外のスキップとして落とす）。**修正前に観測された 62 秒のハングは、4回の連続実行いずれでも再現しなかった** |
| 修正前より悪くなった点 | 無い。むしろ修正前は `read_line` が I/O エラーを返したとき `.ok()?` で子プロセスを刈らずに `None` を返しており、保持プロセスが残る余地があった。現在はその枝も kill + wait を通る |

**`HOOKS.md` の表と実装の一致 — 一致している。**

「環境で走らなくなりうる行」の表の全10行（13 ID）を、各テストファイルの `allowed_skips()` と1件ずつ突き合わせた。

| HOOKS.md の行 | 判定として書かれた述語 | 実装 | 一致 |
|---|---|---|---|
| config-store-023 | `permission_restrictions_effective` | `conformance_config_store.rs:62-75`（`PERMISSION_CASES` 1件） | ○ |
| workflow-store-030 | 同上 | `conformance_workflow_store.rs:86-99`（1件） | ○ |
| task-repository-005 / 011 / 012 / 035 / 019 / 041 | 同上 | `conformance_task_repository.rs:207-228`（`PERMISSION_CASES` 6件） | ○ |
| clock-003 / clock-005 | `observe_wall_clock` / `rewind` の有無 | `conformance_time_id.rs` の宣言（clock-005 のみ許容） | ○（clock-003 はこの環境で実走） |
| exclusive-lock-007 | `unusable_lock` の有無 | `conformance_lock.rs:92-94` が常に `Some`（ディレクトリを置く手段。ADR-032） | ○（許容集合に入れる必要がない） |
| worktree-manager-009 | `failing_manager` / `repo_with_commit` / `head_branch_name` | `conformance_worktree.rs` が常に提供 | ○ |
| worktree-manager-003 | `non_repo_dir` の有無 | `conformance_worktree.rs:113-125`（`tmpdir_outside_repository()` で判定） | ○ |
| **exclusive-lock-002 / 003 / 004 / 005（今回追加）** | `hold_from_other_process` / `try_acquire_from_other_process` の有無 | `conformance_lock.rs:98-117`（`holder_program().is_some()` で判定） | ○ |

追加行が実態と合っていることを実行でも確認した（下表「ロックの許容スキップ宣言」）。件数表（A 28 / B 85 / C 12 = 125）と ExclusiveLock 節の見出し（`7行 / A 1・B 5・C 1`）は、TC-002〜005 が元から区分 B のため変わらない。CLI 側の `common/mod.rs:41-53` も同じ3述語（権限 probe・`holder_program`・`tmpdir_outside_repository`）で組まれており、扱いの割れは無い。

### コマンドの実行結果

| コマンド | 結果 |
|---|---|
| `cargo build` | 成功（警告なし） |
| `cargo test` | 全 458 件 `ok`、0 failed（内訳: lib 62 / domain 167 / conformance lib 13 / CLI・適合 216） |
| `cargo clippy --all-targets -- -D warnings` | exit 0。キャッシュヒットでの空振りを避けるため、3クレートの `lib.rs` と `tests/common/lock.rs` を `touch` して**再リント**させたうえで確認（warning 0 件） |
| `cargo fmt --check` | exit 0 |
| `cargo test -- --nocapture \| grep -i skip` | 4行。うち**実際のスキップは `tc_port_clock_005_巻き戻した時刻はそのまま返る` の1件のみ**。残る3行（`tc_port_clock_005_時刻の巻き戻し` / `tc_port_clock_004_時刻の前進` / `tc_port_clock_0051_別のケース`）は `pulsen-conformance/src/lib.rs:757-777` にある `SkipBudget` 自身のユニットテストが出力するもので、適合ケースではない |

### フレーキーの確認（`cargo test` の連続実行）

4回連続で実行し、全 18 の `test result` 行がすべて `ok` / `0 failed`。所要時間も安定しており、6周目に観測された 62 秒のハングは一度も再現しなかった。

| 実行 | 結果 | 主要ターゲットの所要時間 |
|---|---|---|
| 1回目 | 全 458 件 ok / 0 failed | cli_add_boundary 2.61s ／ conformance_task_repository 1.14s ／ conformance_time_id 1.10s |
| 2回目 | 全 458 件 ok / 0 failed | 1.19s ／ 1.17s ／ 1.11s |
| 3回目 | 全 458 件 ok / 0 failed | 1.19s ／ 1.16s ／ 1.10s |
| 4回目 | 全 458 件 ok / 0 failed | 1.25s ／ 1.18s ／ 1.11s |

### テストバイナリの単体実行

全11ターゲット + 3クレートの lib テストを個別に実行し、**FAILED はゼロ**。

| ターゲット | 結果 |
|---|---|
| `--test cli_add_boundary` | 21 passed / 0 failed |
| `--test cli_add_error` | 31 passed / 0 failed |
| `--test cli_add_normal` | 12 passed / 0 failed |
| `--test cli_usage` | 5 passed / 0 failed |
| `--test conformance_config_store` | 24 passed / 0 failed |
| `--test conformance_lock` | 7 passed / 0 failed |
| `--test conformance_task_repository` | 44 passed / 0 failed |
| `--test conformance_time_id` | 10 passed / 0 failed |
| `--test conformance_workflow_store` | 31 passed / 0 failed |
| `--test conformance_worktree` | 9 passed / 0 failed |
| `--test register_task` | 22 passed / 0 failed |
| `--lib -p pulsen` | 62 passed / 0 failed |
| `--lib -p pulsen-domain` | 167 passed / 0 failed |
| `--lib -p pulsen-conformance` | 13 passed / 0 failed |

### ロックの許容スキップ宣言（HOOKS.md の追加行の裏取り）

| 確認 | 結果 |
|---|---|
| `rm -f target/debug/examples/lock_holder && cargo test --test conformance_lock -- --nocapture` | `7 passed / 0 failed`。TC-002/003/004/005 の4件が SKIP 行つきで通る（002/003/005 は `hold_from_other_process`、004 は `try_acquire_from_other_process`）。**HOOKS.md の追加行が示す判定どおり** |
| example を戻して同じ実行 | `7 passed`、SKIP 行なし・`0.18s`。許容集合が空になり4件が実走する |
| 確認後 | example バイナリを復元済み |

### AC-1〜AC-20 の再検証

6周目で全件合格の判定が出ているため、**変更の影響を受けうるもの（AC-1 / AC-8 / AC-12 / AC-15）を重点的に**再検証し、残りは機械的に確認できる範囲（テスト件数・grep）で追認した。

| AC | 判定 | 根拠（本ラウンドで確認したこと） |
|---|---|---|
| AC-1 | **合格**（重点） | build / test / clippy（強制再リント）/ fmt すべて exit 0。`crates/pulsen-domain/Cargo.toml` の `[dependencies]` が空であることを確認。`grep -rn 'cfg(unix)\|cfg(windows)' crates/*/src/` は**2件のみ** — `crates/pulsen/src/util/atomic.rs:72` と `crates/pulsen-conformance/src/lib.rs:238`（適合ハーネスの権限 probe）。`crates/pulsen-domain/` は0件で、AC-1 の文言どおり |
| AC-2 | 合格 | ドメインのユニットテスト 167 件が緑。6周目以降 `crates/pulsen-domain/` に変更なし |
| AC-3 | 合格 | 同上 |
| AC-4 | 合格 | 同上 |
| AC-5 | 合格 | 同上 |
| AC-6 | 合格 | 同上 |
| AC-7 | 合格 | 6周目以降ポートのトレイト定義に変更なし（`crates/pulsen-domain/src/**/port.rs` は差分ゼロ） |
| AC-8 | **合格**（重点） | 適合スイートは独立クレート `pulsen-conformance` として存在し、1ケース = 1 `#[test]`。実行件数は 24 + 31 + 44 + 10 + 7 + 9 = **125** で spec の行数と一致。`HOOKS.md` の対応表は今回の追加で全行が実装の `allowed_skips()` と一致した（上表）。ただし W-001 のとおり、**TC-002 に期限が無いため「別実装に当てる」場面ではハングしうる**（現行アダプターに対しては緑なので AC としては合格） |
| AC-9 | 合格 | `conformance_config_store` 24 passed |
| AC-10 | 合格 | `conformance_workflow_store` 31 passed |
| AC-11 | 合格 | `conformance_task_repository` 44 passed |
| AC-12 | **合格**（重点） | Clock 5 + TaskIdGenerator 5（`conformance_time_id` 10 passed）/ ExclusiveLock 7（`conformance_lock` 7 passed）/ WorktreeManager 9（`conformance_worktree` 9 passed）= **26件**。`--nocapture` の実測で、**実走しなかったのは `tc_port_clock_005` の1件のみ = 25件が実走**。ExclusiveLock の4件（TC-002〜005）は完全実行では実走し、単体実行では宣言どおりスキップして緑になることを両方の実行で確認した |
| AC-13 | 合格 | `cli_add_boundary` / `cli_add_error` が緑（`--home` > `PULSEN_HOME` > 既定の3段と未初期化の案内は該当ケースに含まれる） |
| AC-14 | 合格 | `cli_add_normal` 12 passed / `register_task` 22 passed |
| AC-15 | **合格**（重点） | `cli_add_error` 31 passed（TC-016 / 017 / 021 / 036 を含む）。ロック競合の TC-017 は `common::lock::hold` 経由で、期限つきになった `spawn_holder` を使う唯一の CLI ケース。完全実行でも単体実行でも緑で、単体実行では `SkipBudget` の宣言どおりスキップに落ちることを確認済み。`cli_add_boundary` 21 passed |
| AC-16 | 合格 | `cli_add_normal` / `cli_add_boundary` が緑 |
| AC-17 | 合格 | 同上（TC-009 / 010 / 060 を含む） |
| AC-18 | 合格 | `register_task` 22 passed。CLI と合わせて `tc_task_register_task_NNN` の**ユニークな TC 番号は 67**（CLI 62 + ユースケース層 5）で、spec の 67 件と1:1 |
| AC-19 | 合格 | `crates/pulsen/src/util/atomic.rs` と `crates/pulsen/src/adapter/lock.rs` が単一の共通実装であることに変更なし |
| AC-20 | 合格 | 6周目以降チェックリスト対象の実装に変更なし |

### 本ラウンドで検討したが指摘しないことにしたもの

- **期限超過時のスキップ理由の文言**: `spawn_holder` が期限超過で `None` を返すと、`SkipBudget` の失敗メッセージは「ハーネスが `hold_from_other_process` を提供しないため」と出る（実際の原因は合図のタイムアウト）。ただしこの経路は失敗として必ず可視化され、`skipped(case, fixture)` にフィクスチャ名が載るため追跡できる。診断の好みの範囲なので指摘しない。
- **`holder.stdout.take()?` の枝で子プロセスを刈らないこと**: `Stdio::piped()` を指定しているため `take()` は必ず `Some` を返し到達不能。今回の変更で入った枝でもない。

### レビュー中のリポジトリ状態

本レビューの実行中、`.thread/1/review/review-006-final.md` が別セッションによって未コミットの状態で書き換えられた（6周目 W-002 の「作業ツリーで修正済み」を「コミット `4ed6866` で解消済み」に改める内容）。本レビューの判定は HEAD `4ed6866` に対するもので、この未コミットの変更には依存していない。
