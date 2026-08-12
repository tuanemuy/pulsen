### Architecture / CLI

対象: PR #11(`issue/2/tick-agent-run-launch`)/ 契約: `.thread/2/plan.md`
検証環境: macOS(darwin 25.4.0)、`cargo fmt --check` / `cargo clippy --all-targets -- -D warnings` / `cargo test` はいずれも成功。実バイナリでの動作確認(tick の起動 → running 取込 → ロック競合 → wrapper の引数往復)を実施した。

依存方向・レイヤー配置・スコープ境界については、逸脱を見つけられなかった。

- `pulsen-domain` の `[dependencies]` は空のまま、ターゲット述語つき `cfg` は `crates/pulsen-domain/` に0件、`crates/pulsen/src/` 側は `util/atomic.rs` と `adapter/process.rs` だけ(AC-1)。
- スコープ外のメソッド・型(`attempt_exists` / `list_runs` / `delete_attempt` / `remove_task_dir_if_empty` / `starttime_of` / `kill` / `try_kill_remnants` / `remove` / `GcPolicy` / `RunningClassifier` / `NotificationService` / `advance` / `set_status` 等)は、宣言もスタブも1件も無い。出現するのは適合スイート冒頭の「このスライスでは扱わない」旨の doc コメントだけ(AC-6)。`todo!` / `unimplemented!` / `TODO` / `FIXME` も0件。
- `dispatch` の未配線アーム(`Cleanup` / `Observe` / `Advance` / `Notify`)は空で、ダミー処理もエラー報告も無い(ADR-001 のとおり)。
- `current_exe()` の読み取りは `wire::process_controller()` の1箇所だけで、`compose()` に載っていない。`add` の経路に新しい失敗が増えていないことを実バイナリで確認した(AC-11 / ADR-004)。
- `compose_wrapper` はホーム解決も `ConfigStore::load` も行わず、`RunDirPath::state_root()` の逆写像だけで `RunStore` を組んでいる。config 不在・破損の環境で `wrapper` の挙動が変わらないことを受け入れテストが主張している(AC-11 / ADR-006)。
- 終了コードは spec どおり。ロック競合 0(`別の操作が実行中のため、今回の tick はスキップしました。`)、`LockError::Failed` と `list_active` の Io は非0、1タスクの失敗は `errors` に積んで全体は 0、config 不在は非0。すべて実バイナリで確認した(AC-12)。
- `wrapper` はヘルプの一覧に現れず(`add` / `tick` / `help` のみ)、`pulsen wrapper --help` は到達できる。ハイフン始まり・空文字列・シェルメタ文字(`$HOME` / `*` / `a;b` / `--home`)を含むトークンがリテラルのまま往復することを実測した(AC-11)。
- 指摘への弁明・修正の経緯を残すコメントは見当たらなかった。残っているのは why / why not とドキュメンテーションコメントだけ。

#### Blockers

- **[B-001]** 状態を書き換えた tick が「処理対象のタスクはありませんでした。」と表示する
  - 場所: `crates/pulsen/src/cli/render.rs:53-56`(判定は `crates/pulsen/src/application/tick/mod.rs:156-167`、書き込みは `crates/pulsen/src/application/tick/confirm_spawn.rs:115-129`)
  - 理由: 実バイナリで `add` → `tick`(起動)→ `tick`(spawn確認)を実行すると、2回目の tick はタスクファイルを `launching` → `running` に書き換え、`current_attempt.process` に pid / kill同定子 / starttime を取り込んだうえで、標準出力に「処理対象のタスクはありませんでした。」と出す。`TickSummary` に起動確認を記録するフィールドが無いため `is_empty()` が真になるためで、これは本スライスの主経路(F2 / AC-15)で**毎回**起きる。pages の tick は「実行したアクションのサマリーを表示」「処理対象がなければその旨を表示」を別の結末として書いており、書き込みを行った tick を後者で報告するのは spec からの逸脱にあたる。cron 運用ではこの出力が唯一の窓なので、「タスクが running に入った」ことが運用者に一切見えない。ADR-020 の根拠(「走査は待機ステータスのタスクや猶予内の launching タスクを列挙するが、**それらには書き込みも報告も発生しない**」)も、書き込みを伴う起動確認の経路までは支えていない — 決定が想定していなかった場合に規則が適用されている。
  - 提案: 起動確認をサマリーに載せる(`transitioned` に入れるか、`confirmed` 相当のフィールドを1つ足して progress.md の「spec 追従の提起」に加える)。少なくとも「1件でもタスクファイルを書いた tick」は「処理対象なし」の表示に落とさないこと。あわせて `crates/pulsen/tests/cli_tick.rs::次のtickはpidの出現をもってrunningへ取り込む` はタスクファイルしか見ていないため、標準出力に対する主張を足して退行を止める。

- **[B-002]** 実装コードの `ADR-NNN` 参照が `.adr/` の正本と衝突し、同じ番号が2つの別文書を指している
  - 場所: `crates/pulsen/src/cli/args.rs:30`(ADR-005)、`crates/pulsen/src/cli/wire.rs:197,237`(ADR-006 / ADR-004)、`crates/pulsen/src/cli/tick.rs:27`(ADR-004)、`crates/pulsen/src/application/tick/mod.rs:56,132,282`(ADR-009 / 001 / 002)、`crates/pulsen/src/adapter/process.rs:4,45,86,131`(ADR-003 / 004)、`crates/pulsen-conformance/src/worktree_manager.rs:2,249`(ADR-013)、`crates/pulsen-conformance/src/process_controller.rs:8,11`(ADR-010 / 011)、`crates/pulsen/tests/conformance_process_controller.rs:76`(ADR-015)ほか、計20箇所以上
  - 理由: `.thread/2/adr.md` は冒頭で「採番はこのファイル内の連番」と宣言して ADR-001〜021 を使っているが、`.adr/` には既に 001〜064 が存在する。結果として、同じリポジトリの同じ語彙が2つの文書を指す状態になった — `crates/pulsen-domain/src/task/counters.rs:3` の「連続した失敗の数(ADR-009)」は `.adr/009-counters-track-consecutive-failures.md` を、`crates/pulsen/src/application/tick/mod.rs:56` の「(ADR-009)」は `.thread/2/adr.md` の「errors は構造化した値で返す」を指す。同様の衝突が ADR-013(`adapter/config_store.rs` の正本 vs `conformance/worktree_manager.rs` のスライス)、ADR-015(`adapter/task_file.rs` vs `tests/conformance_process_controller.rs`)、ADR-010 / 011 / 020 / 021 にもある。Issue #1 は `.thread/1/adr.md` の冒頭で「本書の ADR 番号は正本 `.adr/` の続き番号として採番する(ADR-019 以降)。本文中に現れる ADR-001〜018 は `.adr/` の既存 ADR を指し、本書のエントリと番号が衝突しない」と明示して、この問題を回避する規約を確立していた。本スライスはその規約を破っている。さらに `.thread/2/adr.md` 自身が「昇格するときは `.adr/065` 以降を使う」と書いているため、昇格した時点でコード中の参照は**全件が誤り**になる(progress.md によれば昇格判定はステップ19 = 本 PR より後)。
  - 提案: Issue #1 の規約に戻し、`.thread/2/adr.md` の採番を `.adr/065` 以降に振り直してコード中の参照を更新する。振り直さない場合でも、コード中の未昇格 ADR への参照は `.thread/2/adr.md ADR-005` のように出典を明示して、正本の番号と読み分けられる形にすること。

#### Warnings

- **[W-001]** ラッパーの合成が、ラッパーが使わない `current_exe()` を要求している
  - 場所: `crates/pulsen/src/cli/wire.rs:221-232`(`compose_wrapper` → `process_controller()`)
  - 理由: `RunWrapper` が呼ぶのは `own_identity` と `run_agent` だけで、`spawn_wrapper`(= `self_exe` を使う唯一のメソッド)は呼ばない。それでも合成が `env::current_exe()` を読むため、ラッパーには「自分の仕事に不要なリソースの取得失敗で、run ディレクトリに何も書かずに終わる」経路が1つ増えている。ADR-004 が `compose()` から `current_exe()` を外した根拠(「プロセス起動と無関係な `add` が `current_exe()` の失敗で落ちる」/ pages 縮退規則1「各コマンドは、自身の動作に必要なリソースだけを検証する」)は、そのままラッパーにも当てはまる。実害の確率は低いが、落ちたときの結末は「attempt が1つ失われて猶予経路が spawn 失敗を積む」で、原因の特定が難しい部類になる。
  - 提案: `ProcessController` の3メソッドのうち `self_exe` を要するのは `spawn_wrapper` だけなので、ラッパー用の構築では `self_exe` を要求しない形(構築時に `Option` で受け、無い状態での `spawn_wrapper` は `SpawnError` を返す / あるいは spawn 能力を別トレイトに分ける)にして、ラッパーの起動経路から `current_exe()` を落とす。

- **[W-002]** `InconsistentRunFiles` がドメインに完成した表示文言を持ち、CLI がそれをそのまま出している
  - 場所: `crates/pulsen-domain/src/execution/launching.rs:29-42,100-107`、`crates/pulsen/src/cli/render.rs`(`TickIssue::InconsistentRunFiles` の分岐)
  - 理由: `TickIssue` の他の変種は分類(と不透明な原因文字列)だが、この1つだけはドメインが組み立てた日本語の完成文がユースケースを素通りして CLI の出力になる。ADR-009 が定めた「文言の組み立ては `cli::render`」から外れており、ADR-018 の例外(帳簿に永続化される値は CLI を経由せずに読まれるため `describe` をドメインに置く)にも当たらない — この文言は永続化されず、tick のサマリーにしか出ない。`InconsistentRunFiles` は構造としてのデータを1つも持たないので、`message` を除けば単位型になる。
  - 提案: `InconsistentRunFiles` を分類だけの値にし(必要なら「pid あり / starttime なし」という破れの種別を enum で持たせ)、文言は `cli::render` に置く。あるいは他の `describe` 群と同じく `describe()` として公開し、CLI が明示的に呼ぶ形に揃える。

- **[W-003]** 本スライスに呼び出しの無い `pub` な問い合わせが3つあり、why が添えられていない
  - 場所: `crates/pulsen-domain/src/task/task.rs:185-187`(`execution_kind`)、`:214-220`(`is_wait`)、`:222-228`(`is_cleanup`)
  - 理由: tick の分岐は `branch_of` が `task.execution()` と `task.current_status_def()` に対する網羅 `match` で行っており、この3つを呼んでいない(`is_agent_run` だけは `record_launching` の前提検査で使われている)。テスト以外の呼び出し元が無い `pub` であり、ADR-061 が定めた「呼び出しの無い `pub` は落とし、必要になったスライスで理由つきで戻す」に照らすと、少なくとも why が要る。`Runtime::state_root()` / `worktree_root()` には why が添えられているのに、こちらには無いのが非対称。
  - 提案: 台帳(DOM-task-048〜052)の要求で先に置いているのなら、その旨と引き取り先(#3 / #6 のどの手続きが使うか)を doc に1行添える。使う当てが無いなら落として、必要になったスライスで戻す。

- **[W-004]** 本スライスでは到達できないサマリー表示経路が、テストでも実行されない
  - 場所: `crates/pulsen/src/cli/render.rs`(`push_attempts` と `transitioned` / `skipped_back` / `notified` / `archived` / `gc_deleted` / `gc_errors` の各行)
  - 理由: ADR-001 の判断でサマリー DTO は spec の9フィールドを持つが、値が入るのは `launched` / `frozen` / `errors` の3つだけ。残り6フィールドの表示コードは本番から到達できず、`render.rs` のユニットテストも `launched` / `frozen` / `errors` しか組み立てていない。特に `push_attempts` の `<dir>/attempt-<n>` という表記は本スライスで発明された文字列で、どのテストにも現れない。#3 / #6 が「フィールドを埋めるだけ」で済ませたとき、誰も見たことのない表示がそのまま利用者に出る。
  - 提案: フィールドを保持する判断は維持したうえで、`render.rs` のユニットテストで9フィールドすべてが埋まったサマリーの表示を1件主張する(表示規則を本スライスで確定させる)。

- **[W-005]** `RunStore::read_exit` は本番の呼び出し元が無く、`port.rs` 冒頭の宣言規則と食い違う
  - 場所: `crates/pulsen-domain/src/execution/port.rs:1-5,82`
  - 理由: モジュール doc は「宣言するのは、そのメソッドを呼ぶスライスが揃ったものだけにする」と定めているが、`read_exit` を呼ぶのは #3 の手続きD であり、本スライスの呼び出し元は適合スイート(`crates/pulsen-conformance/src/run_store.rs`)だけ。plan.md AC-6 が9メソッドの1つとして明示しているので契約どおりではあり、実装もスタブではないが、doc に書かれた規則の唯一の例外が説明無しに存在する状態になっている。規則の例外が説明されないと、次のスライスが「適合ケースがあるから宣言してよい」と読んで宣言だけのメソッドを増やす余地が残る。
  - 提案: `read_exit` の doc に「書き込み(`write_exit`)の往復を適合契約として閉じるために本スライスで宣言する。判断の呼び出し元は #3」の一行を添えるか、モジュール doc の規則を「呼ぶスライスが揃ったもの、または同一スライスの書き込みと対になる読み取り」に精密化する。

#### カバレッジ

変更ファイル一覧は 62 行だった(依頼文の「61ファイル」と1件ずれる)。以下は 62 行すべてに1対1で対応する。

- 確認: `.thread/2/adr.md`, `.thread/2/plan.md`, `.thread/2/progress.md`, `.thread/2/steps.md`, `.thread/2/testing.md`, `crates/pulsen-conformance/src/process_controller.rs`, `crates/pulsen-conformance/src/run_store.rs`, `crates/pulsen-conformance/src/worktree_manager.rs`, `crates/pulsen-domain/src/definition/agent.rs`, `crates/pulsen-domain/src/definition/template.rs`, `crates/pulsen-domain/src/execution/launching.rs`, `crates/pulsen-domain/src/execution/mod.rs`, `crates/pulsen-domain/src/execution/port.rs`, `crates/pulsen-domain/src/execution/value.rs`, `crates/pulsen-domain/src/task/counters.rs`, `crates/pulsen-domain/src/task/mod.rs`, `crates/pulsen-domain/src/task/path.rs`, `crates/pulsen-domain/src/task/planner.rs`, `crates/pulsen-domain/src/task/task.rs`, `crates/pulsen-domain/src/task/transition.rs`, `crates/pulsen/examples/spawn_probe.rs`, `crates/pulsen/src/adapter/mod.rs`, `crates/pulsen/src/adapter/process.rs`, `crates/pulsen/src/adapter/run_store.rs`, `crates/pulsen/src/adapter/worktree.rs`, `crates/pulsen/src/application/mod.rs`, `crates/pulsen/src/application/run_wrapper.rs`, `crates/pulsen/src/application/tick/confirm_spawn.rs`, `crates/pulsen/src/application/tick/launch.rs`, `crates/pulsen/src/application/tick/mod.rs`, `crates/pulsen/src/cli/args.rs`, `crates/pulsen/src/cli/mod.rs`, `crates/pulsen/src/cli/render.rs`, `crates/pulsen/src/cli/tick.rs`, `crates/pulsen/src/cli/wire.rs`, `crates/pulsen/src/cli/wrapper.rs`, `crates/pulsen/tests/cli_tick.rs`, `crates/pulsen/tests/cli_usage.rs`, `crates/pulsen/tests/cli_wrapper.rs`, `crates/pulsen/tests/common/mod.rs`, `crates/pulsen/tests/run_wrapper.rs`, `crates/pulsen/tests/tick_confirm_spawn.rs`, `crates/pulsen/tests/tick_launch.rs`, `crates/pulsen/tests/tick_scan.rs`(44件)
- スキップ: `crates/pulsen-conformance/HOOKS.md`, `crates/pulsen-conformance/src/lib.rs`, `crates/pulsen-conformance/src/doubles/clock.rs`, `.../doubles/mod.rs`, `.../doubles/process.rs`, `.../doubles/run_store.rs`, `.../doubles/task_repository.rs`, `.../doubles/tests.rs`, `.../doubles/worktree.rs` — 適合ハーネスとテストダブルの実装でテスト観点の担当(ポート宣言の範囲外にスコープ外メソッドが無いことだけ機械的に確認した)
- スキップ: `crates/pulsen-domain/src/task/attempt.rs`, `crates/pulsen-domain/src/task/failure.rs` — 値型・失敗要因の表現でドメイン観点の担当
- スキップ: `crates/pulsen/examples/agent_probe.rs` — テスト用フィクスチャでテスト観点の担当(受け入れ確認の実行では利用した)
- スキップ: `crates/pulsen/tests/common/git.rs`, `crates/pulsen/tests/tick_fixture/mod.rs`, `crates/pulsen/tests/conformance_process_controller.rs`, `crates/pulsen/tests/conformance_run_store.rs`, `crates/pulsen/tests/conformance_worktree.rs`, `crates/pulsen/tests/register_task.rs` — 適合スイートの適用とフィクスチャでテスト観点の担当(B-002 の参照箇所としてのみ参照した)
