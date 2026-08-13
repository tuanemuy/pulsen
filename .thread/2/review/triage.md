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
| `conformance/process_controller.rs:spawn::tc_002/デタッチ性` | R1 | fix | `detach()` を消しても全緑。永続化される `KillIdent` の破れを受け入れテストで止める | 1 |
| `adapter/process.rs:run_agent/ログ順序` | R1 | fix-editorial | 契約（エージェントを起動しない）は守られており、コメントの主張範囲を実態に合わせる | 1 |
| `tests/common/mod.rs:LOCK_HOLDER_CASES/スキップID` | R1 | fix | スキップ宣言IDの取り違えがチェックリストの根拠を直接誤らせる | 0 |
| `conformance_worktree.rs:worktree_present/アサーション` | R1 | fix | `is_dir()` 止まりで `git branch` + `mkdir` の実装でも通る | 0 |
| `conformance/process_controller.rs:spawn/フック契約` | R1 | fix | 常に `Some(true)` を返すハーネスで3件とも通る。RunStore 側の規律を spawn にも適用する | 0 |
| `tests/tick_confirm_spawn.rs/TC-084` | R1 | fix | 「繰り返しても凍結しない」が tick 1回でしか確かめられていない | 0 |
| `tests/tick_confirm_spawn.rs/TC-085` | R1 | fix | 台帳行と関数の1:1対応が崩れ、消化箇所が追えない | 0 |
| `execution/launching.rs:InconsistentRunFiles/文言` | R1 | fix | 永続化されない文言をドメインが完成形で持つ。既存の `describe()` 群の作法に揃える | 0 |
| `task/task.rs:execution_kind,is_wait,is_cleanup/未使用pub` | R1 | fix-editorial | AC-4 が要求する問い合わせなので落とさず、why を doc に添える（`execution_kind` は `is_stopped` の修正で呼び出し元がつく） | 0 |
| `cli/render.rs:push_attempts/到達不能` | R1 | fix | 誰も見たことのない表示が #3 / #6 でそのまま利用者に出る。表示規則を本スライスで確定させる | 0 |
| `execution/port.rs:read_exit/未使用` | R1 | fix-editorial | モジュール doc の宣言規則の唯一の例外が無説明。doc を1行精密化する | 0 |
| `adapter/worktree.rs:create/同定の間欠破れ` | R2 | fix | 3つの観測の連言に fail-closed が無く、外れると別ブランチの worktree を掴んだまま `Ok` になる。診断情報も残らない | 0 |
| `adapter/process.rs:identity(windows)::observe/stderr分岐` | R2 | fix | 終了コードで機構失敗を判定済みなのに stderr 非空で「不在」を `Err(Io)` に畳み、ADR-067 の写像表に無い3本目の分岐になっている | 0 |
| `adapter/process.rs:identity/default_source` | R2 | fix | 記録と照合の取得手段が PATH の安定性に依存する。既定を絶対パスにする（requirements §4.3） | 0 |
| `adapter/process.rs:without_self_exe/逸脱の記録` | R2 | fix-editorial | ADR-068 の3引数固定からの逸脱と「この構成は spawn_wrapper の適合契約の外」が記録されていない | 0 |
| `task/task.rs:is_wait,is_cleanup/whyの記述` | R2 | fix-editorial | R1 で添えた why が挙げる3用途のうち「待機の素通し」は `branch_of` の網羅 `match` が担い、実態と食い違う | 0 |
| `task/planner.rs:BRANCH_PREFIX/未使用pub` | R2 | fix | 自モジュールと自テストからしか参照されない `pub`。ADR-061 の既定に従って落とす | 0 |
| `task/task.rs:record_spawn_failure/カバレッジ` | R2 | fix | 6遷移で唯一、前提状態の掃引と他カウンタの保持が主張されていない（AC-5 の網羅要求） | 0 |
| `task/task.rs:record_tool_failure/kindの絞り込み` | R2 | fix | `SpawnFail` / `JudgeFail` を受理でき、カウンタと失敗種別が食い違う帳簿が型で書ける | 0 |
| `task/attempt.rs:rehydrate/docのスライス参照` | R2 | fix-editorial | 「新規採番は後続スライスが担う」が、同じファイルの `launching` を未来形で説明する文になっている | 0 |
| `execution/launching.rs:InconsistentRunFiles/spec追従提起` | R2 | fix-editorial | 決定は ADR-086 にあるが、spec の `message: String` との食い違いが progress.md の提起一覧に無い | 0 |
| `task/transition.rs:TransitionError/表示文言` | R2 | fix | 永続化されない表示専用のエラーが `"pending \| failed"` と日本語の完成文をドメインに持つ（ADR-073 と不整合） | 0 |
| `tick/mod.rs:commit/frozenの導出` | R2 | fix | 凍結を「保存後の状態が Stopped」で判定しており、ADR-066 が定めた #3 の拡張点でそのまま誤集計になる。ADR-084 の不変も呼び出し側の規律頼み | 0 |
| `cli/render.rs:tick_summary/スキップ見出し` | R2 | fix | カウンタを消費した記録済みの失敗が、消費しなかったスキップと同じ見出しに束ねられる | 0 |
| `tick/mod.rs:TickIssue::SpawnFailed/経路の兼務` | R2 | fix | 同期エラー（状態不変）と猶予超過の確定（カウンタ消費・凍結あり）が同じ分類で、取り違えを分類が止めない | 0 |
| `tests/tick_scan.rs/未配線アームの否定的主張` | R2 | fix | `Running` / `Completed` / `Stopped` / Pending × `Cleanup` が「起動されない」ことを守るものが目視だけ。否定的主張はスライスをまたいで真 | 0 |
| `tests/cli_tick.rs/滞留エージェントの空虚合格` | R2 | fix | 滞留 800ms のため負荷次第でラッパー終了後に2回目の tick が走り、FD を継承していても緑になる | 0 |
| `tests/cli_wrapper.rs/TC-019の値域` | R2 | fix | 境界値の行なのに実バイナリ経路が 0 / 3 / 42 のみで、126 / 127 / 128+n を通していない | 0 |
| `tests/cli_wrapper.rs/TC-015の合成経路` | R2 | fix | シグナル死の符号化が適合スイートとユニットの2層に分かれ、実バイナリで結線が閉じていない | 0 |
| `tests/cli_tick.rs/F6の主張` | R2 | fix | 「base から作られたブランチが存在する」という名前に対し、主張が存在の有無まで | 0 |
| `conformance/HOOKS.md:環境依存表` | R2 | fix-editorial | 28行目の但し書き（003 は表に現れない）と41行目の列挙（003 を含む）が矛盾する | 0 |
| `steps.md:ステップ19/ADR件数` | R2 | fix-editorial | 「adr.md の14件」とあるが ADR-065〜086 の22件。選別対象が漏れる | 0 |
| `cli/render.rs:recorded_failure/スキップの語義` | R3 | fix | `PrepareAttemptFailed` / `SpawnFailed` は launching を保存した後の報告なのに「何も記録しないスキップ」に束ねられ、同一タスクが「起動」と「スキップ」に同時に出る | 0 |
| `plan.md:AC-1/cfg grepの期待値` | R3 | fix-editorial | 述語を4つに広げた grep は3ファイルにヒットする。期待値が2ファイルのままでは AC-1 の確認が成立しない（統合元: Architecture / Adapter の同一指摘） | 0 |
| `task/task.rs:record_launching/カウンタ保持` | R3 | fix | 事後条件「カウンタを保持する」が未主張。リセットを入れても全緑で、`SpawnFailLimitExceeded` の凍結経路が死ぬ退行を検出できない | 0 |
| `task/task.rs:record_tool_failure,record_spawn_failure_in_place/updated_at` | R3 | fix | 6遷移中この2つだけ `updated_at` 更新の主張が無く、`..self` で古い時刻が残る退行を誰も検知しない | 0 |
| `tick/mod.rs:commit/保存できた遷移だけを積む規則` | R3 | fix | 保存前に `frozen` を積むミューテーションで全緑。凍結を伴う save 失敗の経路が1件も無く、既存の主張は空虚に成立している | 0 |
| `tests/cli_wrapper.rs/TC-014,016のスキップ記録` | R3 | fix | 126 の裏付けが権限依存の適合行だけにあり、権限制限が効かない環境でチェックの根拠が消えるのにスキップ表に載らない | 0 |
| `tests/cli_tick.rs/滞留の実時間依存` | R3 | fix | 「ラッパー生存中に2回目の tick が走る」前提が実時間5秒に依存し、崩れるとスキップではなく赤で現れる | 0 |
| `conformance/HOOKS.md:.adr/027との二重管理` | R3 | fix-editorial | HOOKS.md が「`.adr/027` のフック表と同一」と宣言するが、本 PR で 125行7ポート対169行9ポートに割れた | 0 |

R1: 新規 24 / fix 20 / fix-editorial 4 / wont-fix 0 / defer 0 / 継承 0

R2: 新規 21 / fix 15 / fix-editorial 6 / wont-fix 0 / defer 0 / 継承 2

R3: 新規 8 / fix 6 / fix-editorial 2 / wont-fix 0 / defer 0 / 継承 0

R3 は Key が完全一致する既出指摘が無く、8件すべて新規。うち3件は前ラウンドの修正が残した取りこぼし
（`cli/render.rs:tick_summary/スキップ見出し` → 分割の切り口が書き込みの有無とずれた、
`tick/mod.rs:commit/frozenの導出` → 導出は直ったが規則を張るテストが無い、
`tests/cli_tick.rs/滞留エージェントの空虚合格` → 滞留を伸ばした結果が実時間依存になった）。
| `cli/wire.rs:compose/使わない資源の検証` | R4 | fix | `tick` が自身の動作に不要な `current_dir()` と ID発行の初期化の失敗で非0終了しうる。pages 縮退規則1 と ADR-068 の判断に対して非対称 | 0 |
| `steps.md:設計/実装との食い違い` | R4 | fix-editorial | `InconsistentRunFiles` / `TransitionError` / `TickIssue` / サマリーのフィールド一覧が ADR-073・086・088 の実装と食い違ったまま残っている | 0 |

R4: 新規 2 / fix 1 / fix-editorial 1 / wont-fix 0 / defer 0 / 継承 0（方針フェーズ: 省略 — 新規2件・独立・すべて fix 系）
