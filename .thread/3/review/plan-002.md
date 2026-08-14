# 修正の実行計画 — ラウンド002

判定は `triage.md`（本ラウンド分は fix 21 / wont-fix 1 / defer 0）。単位は担当ファイルが重ならないように割ってあり、単位1〜5 は完全に並列実行できる。単位6（ドキュメント）は plan.md / testing.md を先に直し、`.thread/3/adr.md` への追記だけ単位1・3・4 の完了報告を待つ。

共通の制約:

- `.thread/3/adr.md` を編集するのは**単位6だけ**。ほかの単位で ADR に足すべき判断が出たら、最終報告で文面案を返す。
- スコープは plan.md の「含まれないもの」（手続きB / 手続きE / gc / abort / retry / set-status / `RunStore::attempt_exists` / CI ワークフロー）を越えない。
- 完了条件は `cargo fmt --check` / `cargo clippy --workspace --all-targets -- -D warnings` / `cargo test` が通ること。AC-7 の隔離（`pulsen-domain` の依存ゼロ・`#[allow(unsafe_code)]` 1箇所・ターゲット述語つき `cfg` は `util/atomic.rs` / `adapter/process.rs` / `adapter/task_repository.rs` の3ファイル）を増やさない。
- **ポート適合スイートに行を足さない。** 適合ケースは `spec/testcases/ports/*.md` の台帳と1:1で、行を増やすと `spec/inventory` と `HOOKS.md` の集計（10ポート196行）が spec から外れる。契約の外側で守りたい性質はアダプターのユニットテストで主張する。

## 単位1: 実行単位の終了を「parse → 実行 → 観測」で組み直す（このラウンドの本体）

- 担当する指摘: B-001, W-001, W-002, W-003, W-005, W-006
- 触るファイル: `crates/pulsen/src/adapter/process.rs`, `crates/pulsen-conformance/HOOKS.md`

### 決定した方針

ラウンド1 の修正（`--` の追加）が Alpine（busybox）で裏目に出た事実が示すのは、`--` の有無ではなく **「成否を外部コマンドの終了ステータスだけに預けている構造」** が実体依存だということ。3件をまとめて次の形にする。

1. **同定子を境界で parse する（B-001）。** `terminate` の冒頭でプラットフォーム形式へ parse し、満たさなければ**終了操作を1度も起動せずに**返す。
   - POSIX: `-<n>`（`n` が `u32` としてパースでき、かつ `n >= 2`）。`-1`（全プロセス）・`-0` / `0`（呼び出し側のプロセスグループ = tick 自身と呼び出し元シェル）を弾く。
   - Windows: `<pid>`（`u32` としてパースでき非0）。
   - 失敗の写像: `kill` は `KillError::Failed { message }`、`try_kill_remnants` は `NotIdentifiable`（`unit_is_live` の既存の `Ok(false)` 経路に合流）。
   - parse は `terminate` と `identity::unit_is_live` で**同じ関数を共有**する（現状は `unit_is_live` だけが `strip_prefix('-')` と `u32` パースを持ち、`terminate` は素通し）。CLAUDE.md の「検証は境界で一度だけ行う」に揃える。
2. **成否を観測で決める（W-002）。** 終了操作を起動したあと `unit_is_live` で消滅を確かめ、消えていれば終了ステータスによらず `Ok(())` を返す。busybox が `--` をオペランドとして読んで rc=1 を返しても、対象が消えていれば成功になる。
3. **SIGTERM のあと生存していれば SIGKILL へ昇格する（W-001）。** 契約（`spec/testcases/ports/process-controller.md`）は `kill` の `Ok` に「実行単位に属する全プロセスが終了する」を求めており、SIGTERM を捕まえるエージェントが生きたまま `Ok` を返すのは契約違反。Windows（`taskkill /T /F`）との保証差もここで消える。

最終形（POSIX。Windows は `taskkill /T /F` が1回で強制終了なので昇格が空回りするだけで同じ骨格を通す）:

```
fn terminate(ident) -> Result<(), KillError>:
    target = parse(ident)?                       # 不正形式はここで終了。実体を起動しない
    st1 = run(TERM, target)?                     # 実体を起動できない場合は即 Err
    if not live_within_grace(target): return Ok  # 観測で消えた → 成功（実体の rc は見ない）
    st2 = run(KILL, target)?                     # 生存が観測された、または観測できなかった
    if not live_within_grace(target): return Ok
    if st2.success() or st1.success(): return Ok # 観測が使えない環境では従来どおり rc に従う
    return Err(KillError::Failed { message })
```

- `live_within_grace` は `unit_is_live` を `TERMINATION_GRACE`（新設の定数。2秒程度）まで `POLL_INTERVAL` 相当（50ms。`adapter/command_runner.rs` の先例と同じ粒度）でポーリングし、`Ok(false)` を1度でも観測したら「消えた」とする。`Err(Io)` は「観測できなかった」として扱い、**最終判定を rc へ委ねる**（観測不能を失敗に写像しない — 壊れた取得元が正常な終了を失敗に見せると、`KillFailed` が毎 tick 積まれる）。
- **`Ok(true)`（生存の観測）だけでは失敗にしない**理由をコードの why に残す。呼び出し側がそのプロセスの親のままだと、終了したメンバーがゾンビとして列挙に残りうる（Linux の `/proc/<pid>` は残り、`ps -g` も並べる）。SIGKILL まで送ったうえで rc が成功なら成功と読むほうが、偽の `KillFailed` を作らない。**この帰結として「既に消滅している実行単位への `kill` は `Ok`」になる**（目標状態が満たされている）ことも why に書く。
- `TERMINATION_GRACE` は tick が排他ロックを保持したまま待つ時間なので、`judge_timeout`（既定60秒）に対して十分小さい値を1箇所の定数に置く。値の妥当性はレビューで見る前提で、**2秒を既定とする**。

### 併せて直すもの

- **W-003**: POSIX `unit_is_live`（`:645-647`）のコメントを実態に合わせる。`Err(Io)` と `Ok(false)` の区別が呼び出し側の結末（`RemnantsUnhandled`）に現れないのは ADR-002 の写像どおりで、区別を残すのは(a)このモジュールのユニットテストで取得元の異常を固定するため、(b)本方針2 で `terminate` の最終判定が `Err(Io)` を「観測できなかった」として扱うため、と書き直す。「畳むと何も報告されない」という、実際には成立していない利益の主張はやめる。
- **W-005**: `HOOKS.md:49` の `TC-port-command-runner-001〜016` を `TC-port-command-runner-001 / 002 / 005〜016` に直す（003 は `missing_command`、004 は `non_executable_command` で組み立てるため `judge_probe` を要さない。004 は権限系の行を別に持つ）。1つ上の ProcessController の行が 022 / 023 を外しているのと形を揃える。
- **W-006**: `HOOKS.md:78-80` の「ubuntu 102件 / macOS 100件 / Windows 92件」と内訳（`ps` 系6件）を、本単位のテスト追加後の構造に合わせる。件数そのものは CI 実測なので、`:32` の規律に倣い「件数は run <出典> 時点の実測、内訳の構造は本コミット時点」と読める形にし、変わった箇所（`ps` 系の件数）は `未測定` 相当の扱いにする。数を推測で書かない。

### テスト（すべて `adapter/process.rs` 内のユニットテスト。適合スイートには足さない）

- `-1` / `-0` / `0` / `abc` を同定子にした `kill` は `KillError::Failed` になり、**終了操作の実体を1度も起動しない**。起動されたら痕跡が残る `TerminatorSource`（起動されるとファイルを書くスクリプト等）を注入して主張する。
- 同じ同定子に対する `try_kill_remnants` は `NotIdentifiable` で、やはり実体を起動しない。
- 終了ステータスが非0でも実行単位が消えていれば `Ok`（busybox 相当。非0で終了する実体を注入して、対象の実行単位は別途終了させた状態にする）。
- SIGTERM を無視する実行単位（`sh -c 'trap "" TERM; ...'` 等、`#[cfg(unix)]`）に対する `kill` が `Ok` を返し、対象が実際に消えている（昇格が効いている）。
- 既存の適合スイート 43件（ProcessController 27 / CommandRunner 16）が緑のままであること。可能なら Docker `ubuntu:24.04` と `alpine:3`（busybox）でも `conformance_process_controller` を回す — W-002 の実測が示すとおり macOS だけでは実体差を検出できない。

### 単位6へ返す ADR 文面（メインが `.thread/3/adr.md` に追記する）

**ADR-015: 実行単位の終了は境界で parse し、成否は終了ステータスではなく観測で決める**（ADR-002 の Decision のうち「同定子をそのまま渡す」「`kill` は列挙を挟まない」「成否は実体の終了ステータス」の3点を置き換える。ADR-002 の本文は判断が下された時点の記録として書き換えない）。Context には `-1` / `-0` / `0` の到達可能性（永続化された不透明値・手動修復・破損した run ファイル）と、busybox / procps-ng / macOS で終了ステータスの規則が割れる実測を書く。Decision は上記の3点＋昇格。Consequences に「観測不能（`Err(Io)`）は rc に委ねる」「ゾンビ列挙で偽の失敗を作らない」「既に消滅した実行単位への `kill` は `Ok`」「`TERMINATION_GRACE` のぶん tick が排他ロックを保持する」を書く。

## 単位2: 通知順序の主張を縮退経路と判定上限へ広げる

- 担当する指摘: W-004, W-009
- 触るファイル: `crates/pulsen-conformance/src/doubles/task_repository.rs`, `crates/pulsen/tests/tick_notify.rs`, `crates/pulsen/tests/tick_observe.rs`
- 方針:
  - **W-004**: `saved_degraded` を `Vec<(RecordSeq, DegradedTask)>` にして `saved_degraded_in_order()` を足す（採番は ADR-014 のとおりプロセス内の単一カウンタから）。既存アクセサ `saved_degraded()` は採番を落とした形で維持する。`tick_notify.rs` の `notify_steps` を `save_degraded` の記録も1本の列に並べられる形へ広げ、「スナップショットが読めない未通知の凍結にも再通知が行われる」に**順序の主張**（`SavedDegraded`（凍結）→ `RanNotifyCmd` → `SavedDegraded`（`notified_at`）の並び）を足す。`mark_notified` を先に保存する実装が赤になることが完了条件。
  - **W-009**: `tick_observe.rs:313` の `判定失敗は上限と等しい回数では凍結せず超えると凍結する` に `summary.frozen` の主張を足す（等号側は空・超過側は当該 `TaskId`）。加えて `tick_notify.rs` に「判定上限の超過はその tick で通知される」を1件足し、`config_notifying` + 成功する `ScriptedCommandRunner` で `summary.notified` と `notify_steps` の並び（凍結の `save` → `RanNotifyCmd` → `notified_at` の `save`）まで主張する。これで上限超過3経路（リトライ / spawn / 判定）がすべて同じ共通手続きに乗っていることが閉じ、`Freeze::of_recorded_failure` の取り違えが検出できるようになる。
- 注意: `cli_tick.rs` は触らない（受け入れ側は既存の主張で足り、ユニット側で `Freeze` の取り違えが落ちるようになれば W-009 の穴は閉じる）。

## 単位3: デフォルト判定の2値を型で述べる

- 担当する指摘: W-007
- 触るファイル: `crates/pulsen-domain/src/execution/judgement.rs`, `crates/pulsen/src/application/tick/observe.rs`
- 方針: ADR-009（`AliveDecision`）と同じ手を判定側に当てる。`DefaultJudgement`（`Completed` / `Failed`）を足して `default_judgement` の返り値をこれに絞り、`From<DefaultJudgement> for JudgeOutcome` で3値へ埋め込む。`JudgeOutcome` は `DOM-execution-004` の3値要件があるので残す。`observe.rs` の `Settled::by_default` は2アームの網羅 `match` になり、到達不能な `Skipped` アームが消える（ADR-008 が禁じた「判定コマンドを持たないステータスでの skipped 周回」が型で表現不能になる）。
- テスト: 既存の `デフォルト判定は終了コード20も失敗として扱う` を `DefaultJudgement` に対する主張へ移し、`From` の写像（2値がそのまま `JudgeOutcome` へ対応する）を1件足す。
- 単位6へ返す ADR 文面: **ADR-016**（`default_judgement` の返り値を2値の専用型に絞る。根拠は ADR-009 と同じ「規則の担保をコメントから型へ戻す」で、判定側だけが手当てを受けていなかった残りを埋める）。

## 単位4: 残存の後始末を表示の第4分類にする

- 担当する指摘: W-010
- 触るファイル: `crates/pulsen/src/cli/render.rs`
- 方針: `IssueOutcome` に第4の分類（例 `RemnantsLeft` / 見出し「後始末が残っている」）を足し、`RemnantsUnhandled` だけをそこへ振る。ADR-098 は見出しを「タスクファイルに何を残したか」で分けると定めており、残存の報告は**何も残していない**のに「失敗を記録」（= カウンタを消費し、上限を超えれば同じ tick で凍結する）の見出しに出ている。「スキップ」（= 次の tick がそのまま再試行する）も、tick は残存終了を再試行しないので合わない。`issue_outcome` は網羅 `match` なので振り分け漏れは型が防ぐ。
- テスト: `render.rs:1145` の `残存プロセスの後始末は同定できたかで書き分けられる` に見出しの主張を足す。`保存に失敗した tick で「失敗を記録」に残存が現れない`（= 見出しが分かれる）ことも1行で主張する。
- 単位6へ返す ADR 文面: **ADR-017**（ADR-098 の見出し表に「後始末が残っている」を足す。根拠は ADR-098 の「見出しの語義を書き込みの有無と食い違わせない」で、その表が作られた時点には `RemnantsUnhandled` を保存の成否と独立に積む形（ADR-010）が無かった）。

## 単位5: 走査レベルの主張と前提コメント

- 担当する指摘: W-008, W-011
- 触るファイル: `crates/pulsen/src/application/tick/mod.rs`, `crates/pulsen/tests/tick_scan.rs`
- 方針:
  - **W-008**: `Freeze::of_recorded_failure`（`:533-535`）の前提の列挙を実際の呼び出し元（`record_spawn_failure` / `record_spawn_failure_in_place` / `record_tool_failure` / `fail_run` / `record_judge_failure`）が受け付ける状態、すなわち「起動待ち・失敗確定・起動記録済み・起動確認済みのいずれか」に直す。件数の明示（「3つの」）は増減のたびに腐るので落とす。結論（遷移後の `Stopped` は今回凍結したものだけ）は変わらない。
  - **W-011**: `tick_scan.rs:205` の `実行状態の異なる複数のタスクがそれぞれの分岐で1ステップずつ処理される` に、本スライスで配線した3アーム（running（exit 0 観測）・completed・未通知 stopped）を足し、`judged` / `transitioned` / `notified` が同じサマリーに1件ずつ載ることを主張する。台本は `with_read_exit` と `config_notifying` の追加で足りる。既存の5件（Corrupt / SnapshotUnreadable / Wait / Pending / Launching）は落とさない。

## 単位6: 計画ドキュメントの整合

- 担当する指摘: B-002, B-003, B-004, W-012, W-013, W-014, W-015, W-016, W-017（＋単位1・3・4 が返す ADR-015 / 016 / 017 の追記）
- 触るファイル: `.thread/3/plan.md`, `.thread/3/testing.md`, `.thread/3/adr.md`
- 方針（`plan.md`。AC-8 の記帳がこの表を典拠にするので、実装の最終形に合わせる）:
  - **B-002**（`:63-72`）: 「実行環境が前提を作れないとスキップで終わる行」の表を probe の2集合に書き直す。`TC-port-command-runner-004`（`permission_restrictions_effective`）/ `TC-port-process-controller-011 / 012 / 013 / 015`（実行単位を起こせるか）/ `TC-port-process-controller-014 / 016`（実行単位の一部だけを終了させられるか）の3行にする。`TC-port-command-runner-005` と `TC-port-process-controller-010` は表から外し、「注入で確定的に走るため許容集合に入れない（ADR-007 / ADR-013）」と1行添える。実装の正本は `tests/conformance_command_runner.rs:167-179` と `tests/conformance_process_controller.rs:454-576`。
  - **B-003**（`:44`, `:49`）: `DOM-task-053` を「(6変種。うち `AlreadyNotified` は本スライスで追加。ADR-006)」に直し、確認すること欄を「5種を区別すること（PASS 要件）を確認したうえで `AlreadyNotified` を足す」にする。`:44` の「新規実装は不要」は `DOM-execution-002` にだけ掛かる書き方へ。
  - **B-004**（`:96`, `:97`）: `:96` に `AlreadyNotified`（spec 5種 → 実装6種）を足し、`:97` を「実装は `confirmed_running` と `judged` を加えた11フィールド（ADR-094 / ADR-005）」に直す。`steps.md:259` の記述と字句をそろえる（steps.md 側は正しいので触らない）。
  - **W-014**（`:115-138`）: 手動確認の表に `task-execution.md` TC-20（手順1〜4・6）と `intervention.md` TC-15（手順2 の `abort` を上限超過での凍結に読み替え）の2行を足す。testing.md のエッジケース6・確認項目9 と `:914` の記帳に合わせる。
  - **W-015**（`:113`）: `cat exit` の期待値を「出力は `0` ではなく `.code` に終了コードを持つ整形 JSON になる（ADR-080）」に直す（testing.md 側は修正済み）。
- 方針（`testing.md`）:
  - **W-012**（`:812-813`）: 判定 timeout の期待を「`run` が返った時点で直接の子（判定コマンド）が終了していること」に改め、`pgrep` の結果には「`sh -c` が畳まれない実体では孫が残りうる。ADR-001 が許容した範囲」と但し書きを添える（または判定コマンドを `sh -c` を挟まない形にして直接の子＝観測対象にそろえる）。
  - **W-013**（`:752`）: 手順4・5 の期待に「サマリーの『スキップ』に TD が『埋め込まれたワークフロー定義を読めません』として現れ、同時に『通知』にも TD が現れる（報告は通知に置き換わらない。ADR-012）」を足す。
  - **W-017**（`:890`）: 「`SystemCommandRunner` の構築失敗で `tick` 全体が落ちないこと」を「`SystemCommandRunner` の構築が外部リソースの読み取りを伴わないこと（`wire::command_runner` は無謬）と、`add` の経路がランナーを必要としないままであること」に置き換える。
- 方針（`.thread/3/adr.md`）:
  - **W-016**（`:75`）: 「既定は絶対パスで固定し」を「既定は POSIX が絶対パス、Windows が PATH 解決の固定名で固定し」に直す（同ファイル ADR-007 `:216` の言い回しにそろえる）。
  - 単位1・3・4 の完了報告を受けて **ADR-015 / ADR-016 / ADR-017** を追記する（各単位の節に文面の骨子がある）。ADR-002 / ADR-098 の本文は書き換えず、置き換えた点を新 ADR 側に書く（ADR-013 が ADR-007 に対して採った扱いと同じ）。
- 注意: `plan.md` / `testing.md` の修正は単位1〜5 と独立に着手できる。`adr.md` への ADR 追記だけが単位1・3・4 の完了待ちになる。

## 並列実行

- **同時に走らせてよい**: 単位1 / 単位2 / 単位3 / 単位4 / 単位5 / 単位6（plan.md・testing.md の部分）。担当ファイルは1つも重ならない。
- **待ちがあるのは1点だけ**: 単位6 の `.thread/3/adr.md` への ADR-015 / 016 / 017 の追記は、単位1・3・4 の完了報告を待つ（実装が方針から動いた場合に ADR が実態と割れないようにする）。`:75`（W-016）の1行修正は待たずに入れてよい。
- 単位1 だけは他単位より重い。並列に流すなら最初に着手する。
