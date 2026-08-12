# 残存課題 — Issue #2

## 既知の制限

### Windows の `KillIdent` は pid 文字列のまま

`unsafe` 禁止のままジョブオブジェクトを扱えないため、Windows ではプロセスグループに相当する同定子を永続化できない。#3 の `try_kill_remnants` が「グループごと落とす」契約を Windows で満たせない可能性がある。ADR-067 の申し送りとして残し、Issue #10（クロスプラットフォーム検証）で実機確認する。

影響範囲: Windows 実機のみ。Unix 系では PGID を観測して永続化しているため影響しない。

### Windows の同定情報の取得元は PATH 解決のまま

POSIX の既定は `/bin/ps` の絶対パスに固定したが、Windows は `powershell` の PATH 解決で残している。検証していない絶対パス（`%SystemRoot%\System32\WindowsPowerShell\v1.0\powershell.exe`）を固定すると取得そのものが不能になるため。tick 間で PATH が変われば別実装の PowerShell に解決され、requirements §4.3 の「記録と照合は同一の取得手段」が破れうる。ADR-067 の申し送りとして残し、Issue #10（クロスプラットフォーム検証）で実機確認する。

影響範囲: Windows 実機のみ。

### `cargo test --test <単一ターゲット>` では ProcessController の適合が落ちる

適合ケースが `target/debug/examples/` のプローブに依存するのに対し、単一テストターゲット指定では examples がビルドされない。プローブ不在をスキップ許容集合に入れない方針（ADR-055）を採っているため、スキップではなく失敗になる。

回避策: `cargo test`（パッケージ全体）または `cargo build --examples` を先に実行する。CI では全体実行するので影響しない。

### worktree の内容持ち越し（F4）の観測手段

`agent_probe` に「cwd へ書き込む」モードがなく、追加は本スライスのスコープ外。リトライ間で worktree が作り直されないことは、tick の間にテスト側から worktree へファイルを書いて残存を観測する形で検証している。エージェント自身の生成物で観測しているわけではない。

### `WorktreeManager` 適合ケース TC-port-worktree-manager-015 の間欠失敗（原因未特定）

- `WorktreeManager` 適合ケース TC-port-worktree-manager-015 が、レビュー2周目の `cargo test` 全体実行で1回だけ失敗した（`create` が `Ok` を返した）。`create` が `Ok` を返す経路は「鍵に一致する登録が listing に無い」「`try_exists` が偽」「`git worktree add -b`（`-f` なし）が成功」の3つが同時に成立する場合だけで、1つでも揺れれば `Failed` に倒れる。したがって同定ロジックの競合ではなく、フィクスチャが用意した占有 worktree（登録＋実体）が `create` の時点で存在しなかったことを意味する。適合スイート全体440回・TC-015 単体6,880回（並列含む）でも再現せず、診断を入れた後も本スイートの繰り返し72回で全緑。原因が特定できていないため、同定の分岐は変えていない。
- 代わりに、再発時に破れた側を名指しできるようにした。(1) フィクスチャ `workspace_over_other_branch` は、返す前に `ws.path` の登録が占有ブランチを指し実体が在ることを確認し、破れていれば `git worktree list --porcelain` の生出力・親ディレクトリの一覧・`try_exists` の結果を添えて落とす。(2) 適合ケースは `create` を呼ぶ直前に、実体の内容と `ws.branch` の不在を主張する。(3) `assert_create_failed` の `Ok` アームは `ws.path` の観測（期待した内容・観測した内容・`ws.branch` の worktree として登録されているか）をパニックに載せる。(4) `WorktreeError::Failed` は突き合わせた鍵と登録が指すブランチを含む。前提の破れはスキップではなくケースの失敗として現れる。

## spec 追従の提起（Phase 5 で Issue 化する候補）

実装で spec と食い違った、または spec が規定していない箇所。いずれも既存規約を優先した結果で、コード側の修正は不要。

- `CommandLine::rehydrate` の追加 — `DOM-definition-023` は「`expand` の結果としてのみ生成される」としているが、ラッパーが argv から復元するために生成経路が2つになった（ADR-071）
- `RunDirPath::state_root` の追加 — ラッパーが config もホームも読まずに `RunStore` を構築するための逆写像。台帳に無い追加（ADR-070 / ADR-079）
- `wrapper` の終了コード — spec は引数不正の1行しか規定していない。「ラッパー自身が責務を果たせたか」を表す規約を置いた（ADR-081）
- tick の `errors` — spec は `message: String` だが、文言の組み立てを CLI 層に寄せる規約に従って構造化した値にした（ADR-073）
- tick のサマリー DTO の拡張 — `UC-execution-002` の出力DTO に無い `confirmed_running: Vec<TaskId>` を足した。launching → running の取込は `transitioned` にも `skipped_back` にも語義が合わず、集計先が無いと「書き込んだ tick が処理対象なしと表示される」（ADR-086 / ADR-084）
- `RunStore` の write 系がディレクトリを作る契約 — `spec/domains/execution.md` のポート表に無い契約をポートの doc で宣言し、適合ケースで主張している。`prepare_attempt` の失敗後も spawn は行われる設計なので、ラッパーが自力でディレクトリを作って書けることが自己修復の前提になる（ADR-072）
- `record_tool_failure` の `kind` の型 — `DOM-task-042` / `spec/domains/task.md` の遷移表は `FailureKind` の5値を許す形だが、ツール操作の3種に閉じた `ToolFailureKind` に絞った。`SpawnFail` / `JudgeFail` を渡すとカウンタと失敗種別が食い違う帳簿になるため、型で排除した（ADR-087）
- `TransitionError` の形 — `DOM-task-053` / `spec/domains/task.md` は `InvalidState { expected: &'static str, .. }` と `InvariantViolated { message: String }` を定めるが、永続化されず表示にしか使われないエラーなので、分類だけを持つ形（`expected: &'static [ExecutionStateKind]` / `MissingCurrentAttempt`）にして文言を `cli::render` に寄せた（ADR-088 / ADR-073）
- `LaunchingClassifier` が返す `InconsistentRunFiles` の形 — `spec/domains/execution.md` は `InconsistentRunFiles { message: String }` と定めるが、同じ規則で破れの種別だけを持つ列挙にし、文言は `cli::render` に置いた（ADR-073 / ADR-086）
- tick の `errors` の分類の追加 — 猶予超過で確定した spawn 失敗を `SpawnNotObserved` として同期エラー（`SpawnFailed`）から分けた。前者はカウンタを消費し凍結しうるので、結末の違いを分類で読めるようにした（ADR-090）
- tick サマリーの表示の見出し — pages はサマリーの見出しを規定していない。カウンタを消費した「失敗を記録」と、何も記録せず次tickが再試行する「スキップ」を別の見出しに分けた（ADR-090 / ADR-084）

## 未着手（後続フェーズで行う）

steps.md ステップ19（手動確認・チェックリストの記帳・ADR の昇格判定）は Phase 4 以降で実施する。
