### General Review（計画ドキュメント）

#### Blockers

- **[B-001]** tick サマリー DTO のフィールド数が実装の実態（11）と食い違ったまま「10」で固定されている
  - 場所: `.thread/3/steps.md:29`、`.thread/3/steps.md:107`、`.thread/3/steps.md:174`、`.thread/3/steps.md:220`
  - 理由: 実装は `.thread/3/adr.md` ADR-005 でフィールド `judged: Vec<TaskId>` を足し、`crates/pulsen/src/application/tick/mod.rs` の `TickSummary` は 11 フィールド、`crates/pulsen/src/cli/render.rs` の `tick_summary` は「判定確定」を「起動確認」と「遷移」の間に出す。ところが steps.md は 4 箇所とも旧前提のまま — 107 行が「サマリー DTO のフィールドは #2 で10個そろっており」、29 行と 174 行が `render.rs` の追加を `transitioned` / `skipped_back` / `notified` の3つに限り、220 行が「3フィールドが観測可能になる」と書く。後続の #4 / #5 / #6 は `PAGE-tick-004` の残りをここから引き継ぐため、`judged` の存在を知らないまま「spec の9フィールド + `confirmed_running`」で数え直すことになる
  - 提案: 107 行を「#2 で10個・本スライスで `judged` を足して11個（ADR-005）」に、29 / 174 / 220 行の列挙に `judged`（表示は「判定確定」）を加える。220 行の「3フィールド」は「4フィールド」に直す

- **[B-002]** `TransitionError` を「5種・新規実装は不要（確認のみ）」と書いているが、実装は6種に増えている
  - 場所: `.thread/3/steps.md:62`、`.thread/3/steps.md:142`
  - 理由: `.thread/3/adr.md` ADR-006 が `AlreadyNotified` を追加し、`crates/pulsen-domain/src/task/transition.rs` は6変種、`crates/pulsen/src/cli/render.rs` の `transition_error` も6アームで受けている。steps.md ステップ4 は「`TransitionError` が `DOM-task-053` の PASS 要件（5種を区別する）を満たすことを**確認する**（**新規実装は不要**）」としたままで、変種を足す作業がステップのどこにも現れない。`DOM-task-053` の台帳（`spec/inventory/domain.md:111`）も5種なので、#5 が `abort` / `retry` を足すときにこの表を読むと「変種は増えていない」という誤った前提を引き継ぐ
  - 提案: ステップ4 の変更内容に「`AlreadyNotified` の追加（ADR-006）と `mark_notified` の前提検査での使用」を明記し、62 行の表の備考を「実装済み5種の確認 + 1種の追加」に直す。あわせて「spec 差分の提起」が plan.md の2件から3件に増えることに触れる（ステップ15 が提起する内容が変わる）

- **[B-003]** tick サマリーの見出し列と「初めて値が入るフィールド」の記述が実際の出力と一致しない
  - 場所: `.thread/3/testing.md:879`
  - 理由: 「`render_tick` の見出しは「起動 / 起動確認 / 遷移 / 実行待ちへ復帰 / 凍結 / 通知 / 終端処理」の順に並ぶ」とあるが、実際は `crates/pulsen/src/cli/render.rs:60-67` のとおり **「起動 / 起動確認 / 判定確定 / 遷移 / 実行待ちへ復帰 / 凍結 / 通知 / 終端処理」**で、「判定確定」が抜けている。同じ段落の「本スライスで初めて値が入るのは「遷移」「実行待ちへ復帰」「通知」の3つ」も、`judged`（判定確定）を含めて4つが正しい。exit 0 の観測は主経路なので、この見出しを知らずに確認すると「未知の行が出た」と誤判定する。加えて同段落は関数名を `render_tick` と書いているが、`render.rs` にその名前の関数は無い（`tick_summary`）。「値の無いフィールドが見出しごと出ないこと」を ADR-101 に帰しているのも誤り（ADR-101 は「本スライス外の手続きを配線しない」の決定）
  - 提案: 見出し列に「判定確定」を入れ、「4つ」に直し、関数名を `cli::render::tick_summary` にする。空フィールドの非表示は ADR-092（記録すべきことが起きなかった tick の表示）側に付け替えるか、出典を落とす。確認項目1 手順6 の期待結果（`.thread/3/testing.md:447`「判定側の結果として現れ」）も「サマリーの「判定確定」に T3 が現れる」と具体化する

- **[B-004]** タスクファイル直読で参照している JSON パスが実 DTO と一致しない箇所がある
  - 場所: `.thread/3/testing.md:284`、`.thread/3/testing.md:552`、`.thread/3/testing.md:609`、`.thread/3/testing.md:636`
  - 理由: 本書は `show` / `ls` の不在をタスクファイル直読で代替する前提なので、パスの正しさがそのまま手順の実行可能性になる。(1) 284 行の対応表が「PID・kill同定子 = `.current_attempt.process.pid` / `.kill_ident`」としているが、`crates/pulsen/src/adapter/task_file.rs` の `ProcessDto` は `kill_ident` を `current_attempt.process` の下に持つ。`jq '.kill_ident'` は常に `null` を返す（確認項目11 手順2 がこの値を控える手順になっている）。(2) 552 / 609 / 636 行は「`.execution.state` が `{"state":"stopped","reason":...,"notified_at":...}`」と書くが、`.execution.state` の値は文字列 `"stopped"` で、オブジェクト全体は `.execution` の値。jq でそのまま照合すると必ず外れる。その他のパス（`.execution.state` / `.execution.reason` / `.execution.notified_at` / `.counters.*` / `.current_attempt.number` / `.current_attempt.run_dir` / `.last_failure.kind` / `.task_status` / `.snapshot.statuses` / `.updated_at`）は DTO と一致していることを確認済み
  - 提案: 284 行を `.current_attempt.process.kill_ident` に直す。552 / 609 / 636 行はオブジェクト全体を当てるなら `.execution` に、状態名だけを見るなら `.execution.state` が `"stopped"`・`.execution.reason` が `retry_limit_exceeded` … と分けて書く

- **[B-005]** AC-7 の `#[cfg]` grep の期待件数が実装後の実態と合わない
  - 場所: `.thread/3/testing.md:55`
  - 理由: 「（現状は 4 / 10 / 1 件）」とあるが、本スライスで `adapter/process.rs` に `kill` / `try_kill_remnants` / `starttime_of` が入ったことで実際は **4 / 12 / 1 件**（`origin/main` 時点が 10 件で、そこから2件増えた）。この grep は AC-7 の合否を機械的に判定するための手順なので、件数が合わないと実行者は隔離が破れたのか計画が古いのかを判別できない
  - 提案: 「4 / 12 / 1 件」に更新する。あるいは件数の記載をやめ、「ヒットするファイルがこの3つだけであること」だけを合否条件にする（ファイル集合は変わっていないので、そのほうが以降のスライスでも腐らない）

#### Warnings

- **[W-001]** ステップ8 が列挙する `TickIssue` の新分類に `RunFailed` が無い
  - 場所: `.thread/3/steps.md:174`
  - 理由: 「生存観測の機構失敗 / `kill` の失敗 / 残存終了の報告 / 判定失敗の記録 / 通知の失敗 / 不変条件3の破れ」の6つを挙げているが、実装は `ObservationFailed` / `KillFailed` / `RemnantsUnhandled` / `JudgeFailed` / `NotifyFailed` / `MissingProcessIdent` に加えて `RunFailed`（実行の失敗を記録した）の7つを足している（`origin/main` には無い変種）。`RunFailed` は `render.rs` で「失敗を記録」見出しに振り分けられる分類なので、列挙漏れは #5 が `abort` の報告分類を足すときの参照値としてそのまま効く
  - 提案: ステップ8 の列挙に「実行の失敗の記録（`RunFailed`）」を足す

- **[W-002]** 実装中に起票された ADR-005 / 006 / 007 が steps.md に1件も反映されていない
  - 場所: `.thread/3/steps.md:113-254`（実装ステップ全体）、とくに `.thread/3/steps.md:166`
  - 理由: steps.md は adr.md の ADR-001〜004 だけを参照して書かれており、実装で確定した ADR-005（`judged`）・ADR-006（`AlreadyNotified`）・ADR-007（`TerminatorSource` の注入）が本文のどこにも現れない。B-001 / B-002 はその現れで、ADR-007 についてはステップ7 が「`HOOKS.md` の「環境で走らなくなりうる行」（`004` の権限操作、`005` の強制終了）を更新」としているのに対し、実際の `crates/pulsen-conformance/HOOKS.md` は TC-port-command-runner-005 を区分 B として表に載せていない（載っているのは 004 と「`judge_probe` がビルドされていない」行）。steps.md をそのまま読むと、あるはずの行が無いことになる
  - 提案: ステップ6 / 7 / 4 / 8 / 12 の該当箇所に ADR-005〜007 への参照を入れ、ステップ7 の HOOKS.md 更新内容を実装後の形（004 のみが権限系、005 は前提を作れない環境が無いため表に載せない）に合わせる

- **[W-003]** `exit` ファイルの期待値 `{"code":0}` が実際の出力と綴りで一致しない
  - 場所: `.thread/3/testing.md:63`、および期待結果の各所（`446` / `492` / `552` / `554` / `607` / `885`）
  - 理由: `crates/pulsen/src/adapter/run_store.rs` の `encode` は `to_vec_pretty` なので、`cat exit` の出力は `{\n  "code": 0\n}` の整形 JSON になる。「`0` ではなく JSON」という要点は正しいが、リテラル一致で確認する手順としては外れる（plan.md 113 行から引き継いだ綴り）
  - 提案: 「`{"code": 0}` 相当の整形 JSON（`.code` が 0）」のように、綴りではなく値で書く

- **[W-004]** エッジケース1 手順7（対照ケース）が2択のまま確定しておらず、第1案は同一 task_id のタスクファイルを2つ作る
  - 場所: `.thread/3/testing.md:738`
  - 理由: 「`jq '.snapshot.statuses = "broken"' "$PMT/td.bak" > "$PMT/td2.json"` の内容を新しいタスクIDのファイルとして置くか、`.execution` を `pending` に戻した状態で同じ tick を打ち」とあるが、`td.bak` は TD の内容そのものなので、ファイル名だけ変えて置くと `.task_id` が TD のまま重複する。他の項目がすべてコマンドまで確定しているなかでここだけ手順が未確定で、TC-exec-tick-020（stopped 以外の縮退はスキップして報告）の対照が実行者依存になる
  - 提案: 第2案（`.execution` を `pending` に戻した縮退タスクを1件置く）に一本化し、jq のワンライナーまで書き下す

- **[W-005]** `task-execution.md` TC-20 の扱いが本書の中で食い違っている
  - 場所: `.thread/3/testing.md:286` と `.thread/3/testing.md:851`、`.thread/3/testing.md:901`
  - 理由: 286 行はフィクスチャの省略理由として「本スライスの対象 TC（TC-02 / 09 / 12 / 18 / 20）に属さない」と書き TC-20 を対象外に数えているが、エッジケース6 は「`task-execution.md` TC-20 手順1〜4・6 の再確認」を実行対象にしている。さらに 901 行のカバー一覧に TC-20 が無いため、実行後の記帳でこの項目が消えてしまう（TC-20 は `draft.yaml` / `broken-syntax.yaml` / `wtloss.yaml` / `repo2` のいずれも使わないので、286 行の括弧に TC-20 を挙げていること自体も正確ではない）
  - 提案: 286 行の括弧から TC-20 を外し（省略するフィクスチャに対応するのは TC-02 / 09 / 12 / 18）、901 行のカバー一覧に「TC-20（手順1〜4・6 の再確認）」を足す

- **[W-006]** `intervention.md` TC-15 が採用にも見送りにも現れない
  - 場所: `.thread/3/testing.md:641-666`（確認項目9）、`.thread/3/testing.md:901`
  - 理由: 確認項目9 は「notify_cmd 未定義 → 後から定義した次の tick で catch-up」を確認するが、これは `intervention.md` TC-15 そのものの筋である。本書は同じ確認を `setup.md` TC-35 の読み替えで組み、TC-15 には触れていない。TC-15 は手順2 が `abort` なので実行できないのが正しい判断だが、その旨が本書に無いと、ステップ15 の記帳で「見送った理由」を書けない（plan.md の表にも TC-15 は無いので、本書が拾い直す位置にある）
  - 提案: 確認項目9 の「対応する手順書」に `intervention.md` TC-15 を併記して読み替え（手順2 の `abort` を上限超過での凍結に置き換え）で消化していることを書くか、901 行の記帳に「TC-15 は `abort` 前提のため実行せず、同等の確認を TC-35 の読み替えで行った」と残す

#### カバレッジ

- 確認: `.thread/3/steps.md`, `.thread/3/testing.md`
- 実装との突き合わせで参照したファイル（レビュー対象外・事実確認のみ）: `crates/pulsen/src/application/tick/mod.rs`, `crates/pulsen/src/cli/render.rs`, `crates/pulsen/src/adapter/task_file.rs`, `crates/pulsen/src/adapter/run_store.rs`, `crates/pulsen/src/adapter/process.rs`, `crates/pulsen/src/cli/args.rs`, `crates/pulsen-domain/src/execution/{judgement.rs,notification.rs}`, `crates/pulsen-domain/src/task/transition.rs`, `crates/pulsen-conformance/{HOOKS.md,src/process_controller.rs}`, `spec/manual-tests/{task-execution,setup,intervention}.md`, `spec/inventory/{domain,adapter,test}.md`, `.thread/3/adr.md`
- 確認して問題が無かった点: 参照している `spec/manual-tests/` の TC 番号はすべて実在する（task-execution TC-03/05/06/07/13/14/15/17/19/20/21/22/23、setup TC-09/10/11/35/37/38/39/47、intervention TC-01/24）。`TC-exec-tick-008〜159` / `DOM-*` / `ADP-*` / `TC-port-*` の台帳行も `spec/inventory/` に実在する。フィクスチャA の各 YAML は `spec/manual-tests/task-execution.md` の「テストデータ」と一致する。`judge_env`（`TASK_ID` / `WORKSPACE` / `EXIT_CODE` / `RUN_DIR`）・`notify_env`（`TASK_ID` / `WORKFLOW` / `TASK_STATUS`）・`NOTIFY_TIMEOUT = 60秒` は実装と一致。報告の3見出し（「失敗を記録」「起動の結果が未確定」「スキップ」）も `render.rs` と一致。本スライスに無いコマンド（`ls` / `show` / `abort` / `retry` / `set-status`）を実行する手順は残っていない（すべて直読または読み替えに置換済み）。`.thread/2/testing.md` への参照（確認項目2・3、エッジケース6）も実在する。見出し階層・表・コードブロックの体裁に破綻は無い
- スキップ: なし（担当外の42ファイルは他の3観点が確認済み）
