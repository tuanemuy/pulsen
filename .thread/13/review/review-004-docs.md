# レビュー 004 — ドキュメント整合・契約遵守

対象 PR: #14 / ベース: `origin/main` / HEAD: `cb73567`

## ドキュメント整合・契約遵守

### Blockers

なし

### Warnings

- **[W-001]** `.thread/13/testing.md` 確認項目7 の手順どおりに実行しても、期待結果「5件が失敗する」が観測できない。
  - 場所: `.thread/13/testing.md:173`（確認項目7 手順2 `cargo test -p pulsen -- --nocapture`）と `.thread/13/steps.md:341` の同じコマンド
  - 理由: `cargo test` は既定でテストターゲット単位に打ち切るため、`cli_add_error` が失敗した時点で `conformance_lock` が走らない。実測（`start_holder` に確認項目7 手順1 の差し込みを入れて実行）では `Running` 行が `pulsen` lib / `pulsen` bin / `cli_add_boundary` / `cli_add_error` の4本で止まり、失敗は `tc_task_register_task_017` の**1件だけ**、パニックメッセージ「probe は同じ手順で成立している」も1件しか出ない。同じ差し込みのまま `--no-fail-fast` を足すと `tc_port_exclusive_lock_002 / 003 / 004 / 005` を加えた5件が失敗し、当該メッセージも5件出る。ci.yml の why コメント（`.github/workflows/ci.yml:146-147`）が「`--no-fail-fast` が無いと最初に落ちたバイナリ以降が一度も走らない」と述べているのと同じ事象が、この手順書だけ反映されていない。確認項目5 の手順は `--no-fail-fast` 付き、確認項目8・9 は単一ターゲット指定なので影響を受けず、ずれているのは確認項目7 だけ。
  - 提案: 確認項目7 手順2（および steps.md ステップ8-3）のコマンドを `cargo test -p pulsen --no-fail-fast -- --nocapture` にする。AC-5 の実質は `--no-fail-fast` 付きで成立しているため、記述のみの修正で足りる。

### testing.md の実行結果

- 確認項目1（能力の型と probe が1組）: **成立** — `HolderCapability` は `Available(HolderProgram)` / `SignalTimedOut` / `ProgramMissing` / `ProgramUnusable(io::Error)` の4変種ちょうど。`HolderProgram(PathBuf)` のフィールドは非公開。`OnceLock` の宣言は `holder_capability()` の中の1件。`grep -rn "OnceLock\|LazyLock" crates/pulsen/tests/` は期待どおり**6行**（`use` 3 + `static` 3 = `lock.rs:CAPABILITY` / `mod.rs:SKIPS` / `git.rs:OUTSIDE`）。`Started` は private・4変種で、変種名が `Started` で終わらない。
- 確認項目2（判断の源が1点）: **成立** — `fn holder_program(` に `pub` なし。`holder_program` のヒットは `lock.rs` の定義1 + `probe_holder` からの呼び出し1のみ。`holder_capability` は期待どおり**5行**（定義1 / `use` 1 / 呼び出し3）。`try_acquire_from_other_process` は `spawn_holder` を呼ぶ形のままで `git diff origin/main` に本文の差分なし。`cargo build --workspace --locked` 緑。
- 確認項目3（`None` の意味が1つ）: **成立** — `spawn_holder` の `None` は `SignalTimedOut` 腕の1箇所のみ。`hold` は `?` 伝播のみで `!locked` は `kill_and_wait` + パニック。`hold_from_other_process` は `hold(&self.lock_path())` の1行で、`conformance_lock.rs` に `common::lock::` のフルパス呼び出しは残っていない。`recv_timeout` の失敗は `Timeout` / `Disconnected` に分岐し `Err(_)` は無い。
- 確認項目4（許容集合の宣言）: **成立** — 両 `allowed_skips()` とも `_` の無い `match` で `SignalTimedOut` だけを許容側へ振り分けており、`Available(_) | ProgramMissing | ProgramUnusable(_)` は空。`LOCK_HOLDER_CASES` の doc は両ファイルとも「保持プロセスの合図が期限内に返らない環境でのみスキップされるケース」で件数は4件 / 1件。
- 確認項目5（`Available` 経路）: **成立** — `cargo test --workspace --locked --no-fail-fast -- --nocapture` が緑（`test result: ok` 18本、FAILED 0）。`SKIP` 行は実在の `tc_port_clock_005_巻き戻した時刻はそのまま返る` 1件と、`SkipBudget` 自身のユニットテストが出す架空の3行のみ。ロック系5件は0件。`target/debug/examples/lock_holder` 存在。
- 確認項目6（`SignalTimedOut` 経路）: **成立** — `SIGNAL_DEADLINE` を `Duration::from_nanos(1)` にして `cargo test -p pulsen -- --nocapture` を回し、**緑**のまま `tc_port_exclusive_lock_002 / 003 / 004 / 005` と `tc_task_register_task_017` の5件が `SKIP` 行として出た。復元後 `git diff` 差分なし。
- 確認項目7（probe 成立後のタイムアウト）: **不成立**（W-001）— 手順1 の差し込み自体は期待どおり機能し、`--no-fail-fast` を足せば5件が失敗してメッセージに「保持プロセスの合図が 10s 以内に返らなかった。probe は同じ手順で成立している(この環境で繰り返し起きるなら SIGNAL_DEADLINE を見直す)」が5件出る。5件が `SKIP` として現れないことも確認。ただし手順2 に書かれたコマンドのままでは失敗が1件しか観測できない。復元後 `git diff` 差分なし。
- 確認項目8（`ProgramMissing` 経路）: **成立** — 成果物を削除したうえで `--test conformance_lock` が4件、`--test cli_add_error` が1件失敗。メッセージは「ロック保持フィクスチャ(examples/lock_holder)の実行ファイルが無い。単一のテストターゲットを指定した実行では example がビルドされないため、`cargo test --workspace` のように example をビルドする形で実行する」で、不在・単一ターゲットではビルドされないこと・回避方法の3点を含む。`SKIP` 行0件。手順6 の `--workspace` 再実行で緑に復帰。
- 確認項目9（`ProgramUnusable` 経路）: **成立** — `chmod 000` 後に `--test conformance_lock` で4件が失敗し、メッセージは testing.md の記載と一字一句一致（`ロック保持フィクスチャ(examples/lock_holder)を起動できなかった(probe の起動時に観測した理由): Permission denied (os error 13)`）。`SKIP` 行0件。`chmod 755` で復元し、`--workspace` 再実行で緑。Windows 側のコードレビュー分（手順8〜11）も、`SpawnFailed(error)` → `ProgramUnusable(error)` の受け渡しと両腕の `{error}` 埋め込み、`stdout` 取得失敗が `io::Error::other(..)` を包んだ `SpawnFailed` になることを確認。
- 確認項目10（一時的な差し替えが残っていない）: **成立** — `git status` / `git diff` が空（レビュー成果物の未追跡ファイルを除く）。`SIGNAL_DEADLINE` の差分は doc コメントのみで値は `Duration::from_secs(10)` のまま。`grep -rn "AtomicUsize\|from_nanos" crates/pulsen/tests/` は0件。`lock_holder` が存在し実行ビット `rwxr-xr-x`。`pgrep -fl lock_holder` は0件。
- 確認項目11（fmt / clippy）: **成立** — `cargo fmt --all --check` 差分なし、`cargo clippy --workspace --all-targets --locked -- -D warnings` が警告0・exit 0。
- 確認項目12（HOOKS.md）: **成立** — 表43行の「前提を作れない環境」列が「保持プロセスの合図が期限内に返らない」、判定列がフック水準＋括弧補足、3 OS 列は `実行` のまま。前書き（209行）も同じ述語に改まり、失敗になる旨が続く。表の直後（45行）に不在・起動失敗がケースの失敗である旨が明記。`grep -n "SIGNAL_DEADLINE\|holder_capability\|holder_program\|HolderCapability"` は**0件**。`grep -n "example がビルドされ"` は区分 B の bullet の1行のみで、実測に解釈は足されていない。旧述語（「保持させる実行ファイル」「実行ファイルを要する」）の残存も0件。
- 確認項目13（ci.yml の why コメント）: **成立** — `grep -n "宣言済みスキップ"` のヒットは非 root 確認ステップの1件のみ。テストステップのコメントは「4件＋1件が失敗する（実行ファイルの不在は環境の能力ではないので、SkipBudget の許容集合に入れない）」に改まり、成立条件（`target/` に成果物が残っていない場合）と `--workspace` のままにする理由が残っている。`grep -n -- "--test " .github/workflows/ci.yml` は**0件**。差分はこのコメント塊のみで `run:` は不変。
- 確認項目14（`.adr/073`）: **成立** — `073-holder-capability-skip-vs-fail.md` が既存最大 072 の次に1件。見出しは `## ステータス` / `## コンテキスト` / `## 決定` / `## 検討した代替案` / `## 影響`、ステータスは「承認済み」。「決定」に能力側と失敗側を分ける基準があり、そこから `ProgramUnusable` を失敗側に置く理由が読める。「影響」に 068 の帰結が改まった旨が成立条件つきで書かれている。`lock.rs` の `PROGRAM_MISSING` と `conformance_lock.rs` の `allowed_skips()` の双方の doc から 073 と 068 を辿れる。`.thread/13/adr.md` は7エントリすべての Status 行から昇格 / 作業ログ限りが判別でき、ADR-004 / 007 のみ作業ログ限り。`git diff origin/main -- .adr/068-*.md` は空。
- 確認項目15（3 OS の CI）: **成立** — run 31683976608（`headSha` = `b344401`）の `conclusion` は `success`、7ジョブすべて success（msrv 3 OS のビルドステップを含む）。予測（steps.md ステップ9 手順1）は実測の前に述語から導く形で書かれており、`.thread/13/testing.md:338-341` と PR 本文に予測 → 実測 → 突き合わせの順で記録されている。現 HEAD の `gh pr checks 14` も7ジョブ pass。
- エッジケース1（probe が保持プロセスを取り残さない）: **成立** — `kill_and_wait` は結果を捨てる小さなヘルパー1つ。呼び出しは期待どおり4経路5箇所（`start_holder` 2 / `probe_holder` 1 / `spawn_holder` の `SignalUnreadable` 1 / `hold` の `!locked` 1）。`lock.rs` 内に `release` の呼び出しは無く、ヒットは定義行のみ。
- エッジケース2（ロックを使わないケースのスキップが probe を起こす）: **成立** — `holder_capability()` は `SKIPS`（`LazyLock`）初期化経路にあり、成果物削除下の `--test cli_add_error` は待ちなく `ProgramMissing` で失敗した（確認項目8 手順4 の実測）。成果物がある状態では緑。
- エッジケース3（評価順が結果を変えない）: **成立** — `from_nanos(1)` の状態で `cargo test -p pulsen -- --nocapture` を2回連続で回し、どちらも緑・ロック系5件が `SKIP`。`--test conformance_lock` 単体も緑で4件が `SKIP`。
- エッジケース4（成果物を消さずに `--test` 指定）: **成立** — 削除せずに `--test conformance_lock` を回すと緑（7 passed）で `SKIP` 0件。ci.yml に書き加えた成立条件の実地の裏付けになっている。

### 受け入れ基準の充足判定

- AC-1: **充足** — 確認項目1 のとおり、4区別の `HolderCapability` と `OnceLock` で1度だけ評価する `holder_capability()` が `crates/pulsen/tests/common/lock.rs` に1組だけある。
- AC-2: **充足** — `holder_program()` は private で、ヒットは `lock.rs` 内の定義と `probe_holder` からの1呼び出しのみ。`spawn_holder` / `hold` と2つの `allowed_skips()` はすべて `holder_capability()` を見る。`try_acquire_from_other_process` は差分なしで、`spawn_holder` 経由で同じ1点を見る。`cargo build --workspace --locked` が通ることで private 化の保証も成立。
- AC-3: **充足** — 2つの `allowed_skips()` がともにワイルドカードなしの `match` で4区別を網羅し、許容側は `SignalTimedOut` のみ。確認項目5（`Available` で0件）と確認項目6（`SignalTimedOut` で5件 `SKIP`・緑）で実地に確認。
- AC-4: **充足** — 確認項目8 で不在の案内（原因＋`cargo test --workspace` での回避）、確認項目9 で `Permission denied (os error 13)` がそのまま載ることを実測。いずれもスキップではなく失敗。
- AC-5: **充足** — 確認項目7 の差し込みで、probe が `Available` のまま5件が失敗し、メッセージに「probe は同じ手順で成立している」と閾値見直しの示唆が載ることを確認（W-001 は手順書のコマンドの問題で、振る舞いは基準を満たす）。
- AC-6: **充足** — 確認項目3 のとおり `None` は `SignalTimedOut` 腕の1箇所のみで、取得できなかった場合（`!locked`）と読み取れなかった場合はいずれもパニック。
- AC-7: **充足** — `git diff origin/main...HEAD` 上で `SIGNAL_DEADLINE` の値は `Duration::from_secs(10)` のまま（差分は doc コメントとパニック文言のみ）。検証で入れた一時変更はすべて復元済みで作業ツリーは clean。
- AC-8: **充足** — 確認項目12 の (a)(b)(c) がいずれも成立し、適用側の定数名・関数名・型名の grep は0件、旧述語の残存も0件。
- AC-9: **充足** — 確認項目6 で実測。
- AC-10: **充足** — run 31683976608 が7ジョブ success で、予測（unix 1件 / Windows 11件 / ロック系5件は3 OS とも0件）と実測が一致した記録が testing.md・steps.md・PR 本文に残り、架空3行を数えない旨も明示されている。
- AC-11: **充足** — 確認項目11 で実測（`clippy::enum_variant_names` の発火なし）。
- AC-12: **充足** — 確認項目14 のとおり。
- AC-13: **充足** — 確認項目13 のとおり。`--test ` の綴りは0件のまま。

スコープ逸脱は無い。`git diff --no-renames --name-status origin/main...HEAD` の変更は `.adr/073`（新規）・`.github/workflows/ci.yml`・`crates/pulsen-conformance/HOOKS.md`・`crates/pulsen/tests/` の3ファイル・`.thread/13/` に収まり、`crates/pulsen-conformance/src/` と `crates/pulsen/examples/lock_holder.rs`、`.adr/068` にはいずれも差分が無い。`.thread/13/plan.md` がスコープ外と宣言した HOOKS.md 47行 / 59行の古さも、そのまま残されている。

参照の実在確認: `.adr/027 / 032 / 035 / 038 / 053 / 055 / 060 / 062 / 068 / 071 / 072 / 073` はいずれも実在。`.thread/13/adr.md:30` の `crates/pulsen-conformance/src/lib.rs:236-237`、同 `:50` の `conformance_lock.rs:112` / `common/mod.rs:46`（いずれも `origin/main` 時点）、`.thread/13/plan.md:47` と `steps.md:313` の `ci.yml` 137-142行、`steps.md:301 / 305` の HOOKS.md 43行 / 205行（`origin/main` 時点）は、実際に引いてすべて一致した。

CLAUDE.md の「指摘への弁明や修正の経緯を残さない」に反する記述は、コード・`.adr/073`・HOOKS.md・ci.yml のいずれにも見当たらない。`.adr/073` の「検討した代替案」にある「従来の形」は不採用案の説明であって修正の経緯ではない。

### カバレッジ

- 確認: `/Users/hikaru/github.com/tuanemuy/pulsen_2/.adr/073-holder-capability-skip-vs-fail.md`, `/Users/hikaru/github.com/tuanemuy/pulsen_2/.github/workflows/ci.yml`, `/Users/hikaru/github.com/tuanemuy/pulsen_2/.thread/13/adr.md`, `/Users/hikaru/github.com/tuanemuy/pulsen_2/.thread/13/plan.md`, `/Users/hikaru/github.com/tuanemuy/pulsen_2/.thread/13/steps.md`, `/Users/hikaru/github.com/tuanemuy/pulsen_2/.thread/13/testing.md`, `/Users/hikaru/github.com/tuanemuy/pulsen_2/.thread/13/review/triage.md`, `/Users/hikaru/github.com/tuanemuy/pulsen_2/crates/pulsen-conformance/HOOKS.md`, `/Users/hikaru/github.com/tuanemuy/pulsen_2/crates/pulsen/tests/common/lock.rs`, `/Users/hikaru/github.com/tuanemuy/pulsen_2/crates/pulsen/tests/common/mod.rs`, `/Users/hikaru/github.com/tuanemuy/pulsen_2/crates/pulsen/tests/conformance_lock.rs`, PR #14 本文
- スキップ: `/Users/hikaru/github.com/tuanemuy/pulsen_2/.thread/13/review/fix-plan-001.md`, `fix-plan-002.md`, `review-001.md`, `review-001-concurrency.md`, `review-001-docs.md`, `review-001-test.md`, `review-001-type-design.md`, `review-002.md`, `review-002-concurrency.md`, `review-002-docs.md`, `review-002-test.md`, `review-002-type-design.md`, `review-003.md`, `review-003-docs.md`, `review-003-test.md`, `review-003-type-design.md` — このレビューループ自身の成果物のためカバレッジ対象外（`triage.md` のみ読了）
