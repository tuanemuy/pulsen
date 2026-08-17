# ポート適合テスト: RunStore

対象契約: [../../domains/execution.md#runstore](../../domains/execution.md#runstore)

すべてのアダプター実装(全対象OSのプラットフォーム実装を含む)が共通で通す。前提条件はポートのメソッド呼び出しと、runディレクトリのファイル配置(契約の語彙)に対する直接操作のみで構成する。

| 前提条件 | 操作 | 期待結果 | 実装ステータス |
|---|---|---|---|
| runディレクトリ階層が未作成 | `prepare_attempt(id, 1)` | `Ok(RunDirPath)`。親を含めてattemptディレクトリが作成され、`attempt_exists` が true になる。返るパスは `RunDirPath::derive(state_root, id, 1)` の導出結果と一致する | |
| `prepare_attempt` 済みで、write系でファイルを書き込み済み | 同じ引数で再度 `prepare_attempt` | `Ok`(冪等)。既存の書き込み済みファイルの内容に影響しない | |
| `prepare_attempt` 済み・pidファイル未書き込み | `read_pid_file(run_dir)` | `Ok(None)`(ファイル不在) | |
| attemptディレクトリ自体が不在 | `read_pid_file(run_dir)` | `Ok(None)`(ディレクトリ不在もファイル不在と同様に扱う) | |
| `write_pid_file(run_dir, {pid, kill_ident})` 済み | `read_pid_file(run_dir)` | `Ok(Some)`。pid・kill同定子とも書いた値と等しい(往復可能) | |
| pidファイルの位置に解釈不能な内容を直接置いた状態 | `read_pid_file(run_dir)` | `Err(RunFileError::Corrupt { path, message })`。不在(`Ok(None)`)と区別される | |
| pidファイルは存在するが読み取り自体が失敗する状況(読み取り権限がない等。再現できるアダプター環境に限る) | `read_pid_file(run_dir)` | `Err(RunFileError::Io { message })`。内容不正の `Corrupt`・不在の `Ok(None)` と区別される | |
| `prepare_attempt` 済み・starttimeファイル未書き込み | `read_starttime(run_dir)` | `Ok(None)` | |
| attemptディレクトリ自体が不在 | `read_starttime(run_dir)` | `Ok(None)` | |
| `write_starttime(run_dir, {ident, wall})` 済み | `read_starttime(run_dir)` | `Ok(Some)`。ident・wall とも書いた値と等しい | |
| starttimeファイルの位置に解釈不能な内容を直接置いた状態 | `read_starttime(run_dir)` | `Err(RunFileError::Corrupt)`。不在と区別される | |
| `prepare_attempt` 済み・exitファイル未書き込み | `read_exit(run_dir)` | `Ok(None)` | |
| attemptディレクトリ自体が不在 | `read_exit(run_dir)` | `Ok(None)` | |
| `write_exit(run_dir, code)` 済み(0 と非0 の両方で確認) | `read_exit(run_dir)` | `Ok(Some)`。書いた `ExitCode` と等しい | |
| exitファイルの位置に解釈不能な内容を直接置いた状態 | `read_exit(run_dir)` | `Err(RunFileError::Corrupt)`。不在と区別される | |
| write系(`write_starttime` / `write_pid_file` / `write_exit`)のそれぞれについて、同一attemptへ異なる値で連続書き込み中 | 対応する read系を並行して繰り返し呼ぶ | すべての読み取りが「不在」または「書き込まれたいずれかの完全な値」のみを観測する。書きかけ・新旧混合の内容は観測されない(アトミック置換。観測可能な範囲での検証) | |
| write系の書き込みが途中で失敗する状況(当該状況を再現できるアダプター環境に限る) | `Err(Io)` を観測した後、対応する read系を呼ぶ | 「不在(`Ok(None)`)」または「従前の完全な値」のみを観測し、`Corrupt`(部分的な書きかけ)にはならない(失敗時もアトミック置換の非観測性が保たれる — 部分的な pid 等が残ると tick が `Corrupt` としてスキップし続け、通知されない launching 滞留を生むため) | |
| attemptディレクトリ自体が不在 | `write_invalidation_marker(run_dir)` | `Ok`。ディレクトリごと作成してマーカーを書く。`marker_exists` が true になる | |
| `write_invalidation_marker` 済み | `write_invalidation_marker(run_dir)` を再実行 | `Ok`(冪等)。`marker_exists` は true のまま | |
| `prepare_attempt` 済み・マーカー未書き込み | `marker_exists(run_dir)` | `Ok(false)` | |
| `write_invalidation_marker` 済み | `marker_exists(run_dir)` | `Ok(true)` | |
| `prepare_attempt` 済みでファイルを1つも書いていない(空のattempt) | `attempt_exists(run_dir)` | `Ok(true)`(read系の `Ok(None)` では区別できない「空ディレクトリ」を「ディレクトリごと不在」と区別できる) | |
| attemptディレクトリ自体が不在 | `attempt_exists(run_dir)` | `Ok(false)` | |
| runディレクトリの格納領域(`state/runs/` 相当)自体が未作成 | `list_runs()` | `Ok(空の RunListing)` | |
| 2タスク分のattempt(それぞれ attempt-1・attempt-2)を `prepare_attempt` 済み | `list_runs()` | 各タスクの `dir_name` と、attempt番号 1・2 の `AttemptInfo` がすべて列挙される | |
| attempt内に write系でファイルを時間差をおいて複数書き込み済み | `list_runs()` | 当該attemptの `last_activity` がディレクトリ内ファイルの最終更新時刻の最大値(最後に書いたファイルの時刻)になる | |
| ファイルが1つもない空のattemptディレクトリ | `list_runs()` | 当該attemptの `last_activity` がディレクトリ自体の最終更新時刻になる | |
| タスクディレクトリ配下に `attempt-<n>` 形式に合致しないエントリ(任意名のファイル・`attempt-abc` 等の不正な名前のディレクトリ)を直接置いた状態 | `list_runs()` | 形式外エントリは列挙されない(`attempt-<n>` 形式のattemptのみが列挙される) | |
| `TaskId` としてパースできない名前のタスクディレクトリ(孤児)配下に `attempt-<n>` がある | `list_runs()` | `dir_name` が生文字列のまま列挙される(gcの孤児削除対象にできる) | |
| 複数attemptが存在し、うち1つにファイルがある | `delete_attempt(dir_name, n)` | `Ok`。当該attemptは `attempt_exists` が false になり `list_runs` から消える。他のattemptには影響しない | |
| 対象attemptの削除自体が失敗する状況(削除権限がない等。再現できるアダプター環境に限る) | `delete_attempt(dir_name, n)` | `Err(Io)` を値として返す(パニックしない。失敗は呼び出し側がスキップ・報告する前提の報告用エラー) | |
| attemptがすべて削除され空になったタスクディレクトリ | `remove_task_dir_if_empty(dir_name)` | `Ok`。タスクディレクトリが削除され、`list_runs` に `dir_name` が現れない | |
| attemptが1つ以上残っているタスクディレクトリ | `remove_task_dir_if_empty(dir_name)` | 削除せず `Ok` を返す(非空はエラーではない)。残っているattemptに影響しない | |
| タスクディレクトリに `attempt-<n>` 形式外のエントリのみが残存 | `remove_task_dir_if_empty(dir_name)` | 親ディレクトリを削除せず `Ok` を返し、残存エントリにも触れない(ユーザーが置いたものを黙って消さない。非空はエラーではなく、残存が毎tick `gc_errors` に報告され続けない) | |
| `prepare_attempt` を経ずに attempt ディレクトリが不在 | `write_starttime` / `write_pid_file` / `write_exit` のいずれか | `Ok`。書き込み先のディレクトリが作られ、対応する read系が書いた値を返す(`prepare_attempt` の失敗後も spawn は行われるため、ラッパーが自力で置き場を作って書けることが自己修復の前提) | |

## 対象外

- 残りメソッドの `Io`(`read_starttime` / `read_exit` / `prepare_attempt` / `write_starttime` / `write_pid_file` / `write_exit` / `write_invalidation_marker` / `marker_exists` / `attempt_exists` / `list_runs` / `remove_task_dir_if_empty`): 契約横断の共通規則「機構失敗は `Err(Io)` の値として返す(パニックしない・不在の `Ok(None)` や `Corrupt` に写像しない)」は、読み取り側は `read_pid_file` の `Io` ケース、削除側は `delete_attempt` の `Io` ケースが代表して検証する。呼び出し側の分岐(tick が `write_invalidation_marker` の `Err(Io)` で pending に戻さない判断をする等)が依存するのは「Err が値として返る」ことのみで各メソッドで同型のため、メソッド個別のケースは置かない
- `attempt_exists` の述語(attempt の位置にディレクトリでないものが在るときに `Ok(false)` を返す): 適合ケースにするには「attempt の位置に通常ファイルを置く」フックが要り、全バックエンドに実装を求めることになる。この述語が破れるのはアダプター内部の実装差(`metadata` で型まで見るか `exists` で有無だけ見るか)によるため、フックを増やすよりアダプター側のユニットテストで固定するほうが費用対効果が高い
