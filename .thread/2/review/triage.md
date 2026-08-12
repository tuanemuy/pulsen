# 指摘台帳 — Issue #2 実装レビュー

| Key | 初出 | 判定 | 理由（一行） | 再指摘 |
|---|---|---|---|---|
| `cli/render.rs:tick_summary` | R1 | fix | 状態を書き換えた tick が「処理対象なし」と表示する（統合元: Use Case `cli/render.rs:tick_summary/表示契約`。書き込みが無報告になるのは起動確認だけでなく worktree 作成失敗・展開失敗も同じ） | 0 |
| `crates/**:ADR参照` | R1 | fix-editorial | 同じ `ADR-NNN` が `.adr/` 正本と `.thread/2/adr.md` の2文書を指す。Issue #1 の続き番号規約に戻す | 0 |
| `tests/cli_tick.rs/カバレッジ` | R1 | fix | TC-exec-tick-017（config 不在・破損）の検証がどこにも無く、完了条件の未消化 | 0 |
| `task/task.rs:confirm_workspace/カバレッジ` | R1 | fix | TC-exec-tick-035 のカウンタ保存が空虚な主張になっている（ドメイン + ユースケースの2層で埋める） | 0 |
| `execution/port.rs:RunStore/write系のディレクトリ作成` | R1 | fix | ポート契約に格上げした write 系のディレクトリ作成に適合ケースが無い。自己修復の前提が無検証 | 0 |
| `tick/launch.rs:attempt番号の導出源` | R1 | fix | 採番が遷移関数の外で再導出され、導出点が2つに分かれている（統合元: Domain / Use Case の同一指摘） | 0 |
| `task/task.rs:record_launching/初回採番` | R1 | fix | 全タスクの初回起動経路（`current_attempt = None`）のドメイン主張が無い | 0 |
| `task/path.rs:state_root/否定ケース` | R1 | fix | ADR が根拠に挙げる `attempt-+1` が否定ケースに無く、一致検査を緩めても緑になる | 0 |
| `tick/mod.rs:is_stopped/execution_kind` | R1 | fix | 実行状態の判別がドメインの問い合わせ口とユースケースに二重化している | 0 |
| `tick/confirm_spawn.rs:read_run_files/RunFileError` | R1 | fix | starttime 側の失敗経路が一度も注入されず、握り潰す実装に退行しても検出できない | 0 |
| `cli/wire.rs:compose_wrapper/依存` | R1 | fix | ラッパーが使わない `current_exe()` の失敗で起動できない（統合元: Use Case / Architecture の同一指摘） | 0 |
| `adapter/worktree.rs:create/達成済み判定` | R1 | fix | 実体の存在を観測せず git の `prunable` 注記だけに依存し、復旧分岐が開かない環境がある | 0 |
| `adapter/process.rs:identity(windows)::observe` | R1 | fix | 非終端エラーが「プロセス不在」に畳まれ、ADR-003 の写像表と逆向き。構造だけ直し実機確認は #10 | 0 |
| `conformance/process_controller.rs:spawn::tc_002/デタッチ性` | R1 | fix | `detach()` を消しても全緑。永続化される `KillIdent` の破れを受け入れテストで止める | 0 |
| `adapter/process.rs:run_agent/ログ順序` | R1 | fix-editorial | 契約（エージェントを起動しない）は守られており、コメントの主張範囲を実態に合わせる | 0 |
| `tests/common/mod.rs:LOCK_HOLDER_CASES/スキップID` | R1 | fix | スキップ宣言IDの取り違えがチェックリストの根拠を直接誤らせる | 0 |
| `conformance_worktree.rs:worktree_present/アサーション` | R1 | fix | `is_dir()` 止まりで `git branch` + `mkdir` の実装でも通る | 0 |
| `conformance/process_controller.rs:spawn/フック契約` | R1 | fix | 常に `Some(true)` を返すハーネスで3件とも通る。RunStore 側の規律を spawn にも適用する | 0 |
| `tests/tick_confirm_spawn.rs/TC-084` | R1 | fix | 「繰り返しても凍結しない」が tick 1回でしか確かめられていない | 0 |
| `tests/tick_confirm_spawn.rs/TC-085` | R1 | fix | 台帳行と関数の1:1対応が崩れ、消化箇所が追えない | 0 |
| `execution/launching.rs:InconsistentRunFiles/文言` | R1 | fix | 永続化されない文言をドメインが完成形で持つ。既存の `describe()` 群の作法に揃える | 0 |
| `task/task.rs:execution_kind,is_wait,is_cleanup/未使用pub` | R1 | fix-editorial | AC-4 が要求する問い合わせなので落とさず、why を doc に添える（`execution_kind` は `is_stopped` の修正で呼び出し元がつく） | 0 |
| `cli/render.rs:push_attempts/到達不能` | R1 | fix | 誰も見たことのない表示が #3 / #6 でそのまま利用者に出る。表示規則を本スライスで確定させる | 0 |
| `execution/port.rs:read_exit/未使用` | R1 | fix-editorial | モジュール doc の宣言規則の唯一の例外が無説明。doc を1行精密化する | 0 |

R1: 新規 24 / fix 20 / fix-editorial 4 / wont-fix 0 / defer 0 / 継承 0
