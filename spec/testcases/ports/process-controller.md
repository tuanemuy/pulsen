# ポート適合テスト: ProcessController

対象契約: [../../domains/execution.md#processcontroller](../../domains/execution.md#processcontroller)

すべてのプラットフォーム実装が共通で通す。実プロセスを使う統合テストとして実行してよいが、期待結果は契約の語彙(プロセスグループ相当の実行単位・起動時刻の照合等)で検証し、OS固有の機構名には依存しない。フィクスチャとして「exit codeを制御できる」「引数どおりに出力する」「一定時間実行し続ける」「子プロセスを起動する」テスト用コマンドを用いる。

| 前提条件 | 操作 | 期待結果 | 実装ステータス |
|---|---|---|---|
| `prepare_attempt` 済みの run_dir・実在するworktree・テスト用エージェントコマンドで構成した `WrapperLaunchSpec` | `spawn_wrapper(spec)` | `Ok(())`。起動後の成否は戻り値に現れない(観測はrunディレクトリ経由) | |
| 一定時間実行し続けるテスト用コマンドを spec に与えて `spawn_wrapper` 済み | 呼び出し側プロセスを終了させ、別プロセスからrunディレクトリを観測する | ラッパーは呼び出し側の終了後も生存して実行を完了する(runディレクトリに starttime・pid・exit が揃う)(デタッチ性) | |
| ラッパーの起動自体が不可能な状況(起動対象を実行できない等。当該状況を再現できるアダプター環境・プラットフォームに限る) | `spawn_wrapper(spec)` | `Err(SpawnError::Failed { message })`。runディレクトリ・プロセスに副作用を残さない | |
| テストプロセス自身 | `own_identity()` | `Ok(WrapperIdentity)`。`pid` は自プロセスのPID、`kill_ident` は非空、`starttime.wall` は呼び出し前後に取得した時刻の範囲内 | |
| 自プロセスの同定情報の取得機構自体が失敗する状況(当該状況を再現できるアダプター環境・プラットフォームに限る) | `own_identity()` | `Err(Io)` を値として返す(パニックしない。不正な同定情報で `Ok` を装わない) | |
| 生存中の自プロセス | `starttime_of(自プロセスのPID)` | `Ok(Some(ProcessStartTime))` | |
| 終了を確認済みのプロセスのPID | `starttime_of(pid)` | `Ok(None)`(不在 = 死亡) | |
| 生存中の同一プロセス | `starttime_of(pid)` を2回呼ぶ | 両方 `Ok(Some)` で値が等価(等価比較に使える安定値) | |
| `own_identity()` の結果を保持 | `starttime_of(own_identity の pid)` | `Ok(Some)` で、`own_identity` の `starttime.ident` と等価(記録と照合が同一の取得手段で行われる) | |
| 生存中のプロセスに対して起動時刻の取得機構自体が失敗する状況(取得手段への読み取り権限がない等。当該状況を再現できるアダプター環境・プラットフォームに限る) | `starttime_of(pid)` | `Err(Io)`。プロセス不在の `Ok(None)`(= 死亡)と区別される(機構失敗を死亡に写像しない。写像すると生存プロセスの Dead 誤判定から failed → 再起動 → 同一worktreeの並走を招くため、呼び出し側は状態を変更せず再観測する) | |
| 子プロセスを起動するテスト用コマンドを `spawn_wrapper` で起動し、ラッパー・エージェント・その子が生存(記録済みstarttimeとの照合 = Alive) | `kill(kill_ident)` | `Ok`。実行単位に属する全プロセス(ラッパー・エージェント・子プロセス)が終了する(`starttime_of` が各PIDで `None` になる)。呼び出し側のテストプロセスは影響を受けない(実行単位が分離されている) | |
| 子プロセスを起動するテスト用コマンドを `spawn_wrapper` で起動した spawn 元プロセスが終了済み。別プロセスが新規に構成した ProcessController を持ち、runディレクトリの pid ファイル由来の `KillIdent` のみを入力に持つ(記録済みstarttimeとの照合 = Alive) | `kill(kill_ident)` | `Ok`。実行単位に属する全プロセスが終了する(`starttime_of` が各PIDで `None` になる)。プロセス内に保持したハンドルに依存せず、タスクファイルの情報だけで kill を実行できる(ツールの再起動後の kill) | |
| 実行単位への終了操作自体が失敗する状況(当該状況を再現できるアダプター環境・プラットフォームに限る) | `kill(kill_ident)` | `Err(KillError::Failed { message })` を値として返す(パニックしない。分類・状態変更は呼び出し側が行わない前提の報告用エラー) | |
| ラッパーのみ死亡し、エージェント(および子)が実行単位に属したまま生存 | `try_kill_remnants(kill_ident)` | `Killed`。残存プロセスが終了する | |
| 対象を誤殺なく同定できない状況(実行単位の同定手段が失われている等。当該状況を再現できるアダプター環境・プラットフォームに限る) | `try_kill_remnants(kill_ident)` | `NotIdentifiable`。いかなるプロセスも終了させない(無関係なプロセスの誤殺がない) | |
| 対象は同定できるが終了操作自体が失敗する状況(当該状況を再現できるアダプター環境・プラットフォームに限る) | `try_kill_remnants(kill_ident)` | `Failed { message }` を値として返す(パニックしない。呼び出し側の failed 分類には影響しない報告用) | |
| exit 0 で終了するテスト用コマンド・実在するworktree・書き込み可能なログパス | `run_agent(cmd, cwd, stdout, stderr)` | `ExitCode(0)` | |
| 非0(例: 7)で終了するテスト用コマンド | `run_agent(...)` | `ExitCode(7)`(exit code をそのまま返す) | |
| 自身の作業ディレクトリが worktree なら 0 を返す検査コマンド | `run_agent(cmd, cwd=worktree, ...)` | `ExitCode(0)`(カレントディレクトリは常にworktree) | |
| 標準出力・標準エラーへそれぞれ既知の文字列を出力するコマンド | `run_agent(...)` | 指定した stdout パスに標準出力の内容、stderr パスに標準エラーの内容が書かれている | |
| シェルのメタ文字(`*`・`$VAR`・`&&`・リダイレクト記号等)や空白を含む引数トークン、`{input}` 等のプレースホルダ文字列を含む `CommandLine` を与え、受け取った引数がリテラル一致(展開・連結・再分割なし)なら 0 を返す検査コマンド | `run_agent(...)` | `ExitCode(0)`(引数がシェルに解釈されずそのまま渡る = シェルを介さない直接起動。requirements §3.1) | |
| 存在しないコマンド名 | `run_agent(...)` | `ExitCode(127)`(コマンド不在の符号化。エラー・パニックにならない) | |
| 実行不能なファイル(実行権限がない等、起動できない実体) | `run_agent(...)` | `ExitCode(126)`(実行不能の符号化) | |
| 実行中に外部から強制終了される(exit code を持たない終了をする)コマンド | `run_agent(...)` | 非0 の符号化値(POSIX慣例では 128+シグナル番号)の `ExitCode` を返す(常に値を返し、失敗しない) | |
| stdout のリダイレクト先が開けない(書き込み不能なパス等) | `run_agent(...)` | エージェントを起動せず `ExitCode(126)` を返す(エージェントの副作用が生じない) | |
| `cwd`(worktree)が存在しない(手動削除等。pages ※9) | `run_agent(...)` | 非0 の符号化値(起動不能 126 相当)の `ExitCode` を返す(常に値を返し、失敗しない — failed 経路への合流) | |
| 一定時間実行してから終了するコマンド | `run_agent(...)` | 呼び出しはコマンド終了まで戻らず、終了後に `ExitCode` を返す(同期実行) | |
