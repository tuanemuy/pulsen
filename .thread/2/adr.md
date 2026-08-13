# ADR — Issue #2: tick によるエージェント実行の起動

採番は `.adr/` の続き番号(既存の最大番号は 072 なので 073 から)。`.adr/` への昇格判定は片付けフェーズで行い、昇格しても番号は変わらない。Status の直下に `.adr/` のファイル名を持つエントリが昇格済み。ADR-073 / 074 / 083 / 084 / 087 / 088 は本スライス限りの作業判断か他のエントリに吸収される派生であり、昇格しない — `.adr/` 側ではこの6番が欠番になる。

## ADR-073: tick の分岐は全実行状態を網羅する `match` で書き、本スライス外の手続きは配線しない

### Status

Proposed

### Context

`UC-execution-002`(Tick)はチェックリスト行だが、その分岐先のうち手続きB(終端処理)・D(観測・判定)・E(gc)・`Completed → advance`・`Stopped → notify` は Issue #3 / #6 のチェックリストにある。一方で `TC-exec-tick-013`(実行状態の異なる複数のタスクがある)は本スライスの in-scope で、「各タスクがそれぞれの分岐で1ステップずつ処理され、exit は 0」を要求する。

選択肢:

1. 走査で in-scope の実行状態(Pending / Failed × AgentRun・Pending / Failed × Wait・Launching)だけを拾い、他は列挙もしない
2. 全実行状態を網羅する `match` を書き、未配線のアームを `errors` に「未実装」として積む
3. 全実行状態を網羅する `match` を書き、未配線のアームは何もしない(報告もしない)

### Decision

3 を採る。tick の分岐を `ExecutionStateKind`(および Pending / Failed では `current_status_def()` の動作種別)に対する網羅 `match` として書き、本スライスで配線するのは次のアームだけにする。

| アーム | 本スライス |
|---|---|
| `Corrupt` | 報告のみ(実装する) |
| `SnapshotUnreadable` | 定義依存の判断をスキップして報告(実装する。`Stopped` の再通知は #3) |
| Pending / Failed × `Wait` | 何もしない(実装する) |
| Pending / Failed × `AgentRun` | 手続きA(実装する) |
| Pending / Failed × `Cleanup` | 何もしない(#6 が手続きBを入れる) |
| `Launching` | 手続きC(実装する) |
| `Running` | 何もしない(#3 が手続きDを入れる) |
| `Completed` | 何もしない(#3 が `advance` を入れる) |
| `Stopped` | 何もしない(#3 が notify を入れる) |

未配線のアームには「このアームを埋めるスライス」を why コメントとして残し、そこにダミーの処理もエラー報告も置かない。サマリー DTO は spec の全フィールド(`launched` / `transitioned` / `skipped_back` / `frozen` / `notified` / `archived` / `errors` / `gc_deleted` / `gc_errors`)を持たせるが、値が入るのは本スライスで配線したアームが埋めるものだけとし、空のフィールドは表示しない。

1 を採らない理由: `list_active` の全件走査と実行状態による分岐は `UC-execution-002` の骨格そのもので、後から「走査対象の絞り込みを外す」形の書き換えになる。2 を採らない理由: 仕様に無い語("未実装")が tick のサマリーに出て、利用者から見える挙動が spec から乖離する。

### Consequences

- 良い点: 後続スライスの作業が「アームを1つ埋める」に閉じる。骨格の正しさ(走査・破損の扱い・サマリー・exit code)を本スライスで確定できる。
- トレードオフ: `Cleanup` / `Running` / `Completed` / `Stopped` のタスクは #3 / #6 がマージされるまで tick が黙って素通りする。本スライスのテストはこれらのアームに**何が起きるか**の期待を持たない(持たせると #3 / #6 で書き換えることになる)。一方で「エージェントを起動しない・書き込まない」という否定的な主張はスライスをまたいで真であり、アームが埋まっても書き換えにならないので固定する — 実行中や終端処理待ちのタスクに2本目のラッパーが起動する誤りは、新しい attempt の run ディレクトリに無効化マーカーが無いため本スライスのマーカープロトコルでは防げない。
- `UC-execution-002` と `TC-exec-tick-013` は、未配線のアームが残る限り台帳の PASS 要件を満たさない(部分消化)。チェックは付けず、消化範囲を Issue のコメントに残す(plan.md「チェックリスト行にチェックを付ける基準」)。

---

## ADR-074: stopped の記録と `frozen` への集計までを実装し、notify の呼び出しは #3 に残す

### Status

Proposed

### Context

手続きA(worktree作成失敗でリトライ上限超過・展開失敗で spawn_fail 上限超過)と手続きC(猶予超過の spawn 失敗で上限超過)は `Stopped` を書く。spec は「`save` 直後に共通手続き notify を実行し `frozen` / `notified` に記録する」と定め、`TC-exec-tick-038 / 045 / 076` の期待文にも notify が現れる。

しかし notify を構成する要素は1つもチェックリストに無い — `UC-execution-001`(共通手続き)・`DOM-execution-022 / 071`(NotificationService・NOTIFY_TIMEOUT)・`DOM-execution-067`(CommandRunner)・`ADP-commandrunner-001` はすべて Issue #3 の担当。ここで notify を実装すると #3 のチェックリスト行を先取りすることになり、逆に「notify を呼ぶ」だけを書くと呼び先が無い。

### Decision

本スライスは **stopped の記録(`save`)と `frozen` への集計まで**を実装し、通知は行わない。手続きA・Cの中で「stopped になった」ことを保存の単一地点(`commit`)に集約し、#3 がその関数の中で notify を呼ぶだけで済む形にする。凍結かどうかは保存後の状態から導出せず、遷移を呼んだ側が `commit` に渡す(ADR-097) — #3 が足す catch-up 通知は `mark_notified` した `Stopped` を保存するため、状態からの導出では過去の凍結を毎 tick 再計上する。

根拠は requirements §8 の at-least-once — stopped は `notified_at: None` で永続化され、`notified_at` のない stopped は以降のすべての tick が検出して通知する。#3 がマージされた後の最初の tick が catch-up するため、経路として穴は開かない。

### Consequences

- 良い点: スライス境界がチェックリストと一致し、#3 の作業が「1関数の中身を埋める」に閉じる。
- トレードオフ: 本スライス単体では、上限超過で凍結したタスクの通知が行われない。受け入れテストは stopped の永続化と `frozen` への記録を主張し、notify の実行は主張しない。
- **notify 句を残す行はすべて部分消化として扱う。** 該当は `TC-exec-tick-038 / 045 / 076` と、`UC-execution-003` / `UC-execution-005` — 後者2行の台帳 PASS 要件は手続きの**途中の必須ステップ**として notify を書いており(`record_spawn_failure_in_place` → `save` →(Stopped なら notify)→ 終了)、plan.md の例外規則(末尾の下流参照・操作の主語)は当てはまらない。TC 行と UC 行で判定を変えない。いずれもチェックは付けず、消化範囲を Issue のコメントに残す。

---

## ADR-075: ProcessController のプラットフォーム実装を `unsafe` なしで構成する

### Status

Proposed

→ `.adr/075-process-controller-without-unsafe.md` に昇格

### Context

workspace lints に `unsafe_code = "forbid"` があり、`pulsen` クレートは `[lints] workspace = true` で継承している。依存クレートに `libc` / `nix` / `rustix` / `sysinfo` は無く、ADR-023 は本番依存を6クレートに閉じている。一方 requirements §4.3 は「デタッチ起動」「プロセス起動時刻の取得」を OS 依存操作として要求する。

選択肢: (a) `libc` を足して `unsafe` を許可する、(b) `pre_exec` を使う(unsafe)、(c) std の安全 API と外部コマンドの起動だけで組む。

### Decision

(c) を採り、`unsafe_code = "forbid"` を維持する。

**デタッチ起動**(`spawn_wrapper`)

- POSIX: `std::os::unix::process::CommandExt::process_group(0)` で新しいプロセスグループの長にする(安全 API)。`setsid(1)` には依存しない — macOS に同名の実行ファイルが無いため。
- Windows: `std::os::windows::process::CommandExt::creation_flags` に `CREATE_NEW_PROCESS_GROUP | DETACHED_PROCESS` を渡す(安全 API)。
- 両者共通: stdin / stdout / stderr を `Stdio::null()` にし、`spawn` の戻り値を待たない(`Child` を drop する)。呼び出し側プロセスが終了しても子は生き残る。

**kill 同定子**(`own_identity` が返す `KillIdent`)

- POSIX: プロセスグループIDを**観測して** `-<pgid>` の文字列とする(`kill(1)` にそのまま渡せる負の PGID 表記)。取得は起動時刻と同じ経路に相乗りする — macOS 等は `ps -o lstart=,pgid= -p <pid>` の1回の呼び出しで両方を得(`ps` の起動回数は増えない。最終トークンが PGID、残りを trim したものが lstart。実測で確認)、Linux は `/proc/<pid>/stat` の「最後の `)` より後ろの3番目(全体の5番目 = `pgrp`)」を起動時刻と同じ読み取りから取る。
- Windows: プロセスグループIDに相当する値として自身の PID の文字列とする。
- kill の実行自体は #3 の担当。本スライスは「タスクファイルの情報だけで kill できる形式の値を書く」ところまでを保証する。

`process_group(0)` の効果から `pgid == pid` を**仮定して** `-<pid>` を書く案を採らない理由: `own_identity` は `spawn_wrapper` 経由の起動以外からも呼ばれる(適合ケース `TC-port-process-controller-004` はテストプロセス自身に対して呼び、`pulsen wrapper` は隠しているだけで手でも起動できる)。そのとき `-<pid>` は存在しないプロセスグループか、最悪シェル・cron のプロセスグループを指す。`KillIdent` は pidファイルとタスクファイルに**永続化**され #3 の `kill` がそのまま使う値なので、誤った値が帳簿に残ると無関係なプロセス群を殺す経路になる。`TC-004` の期待は「非空」なので、この破れは適合スイートでは検出されない。本経路(`spawn_wrapper` 起動)では観測しても値は `-<pid>` のままで、既存のフィクスチャ(`-4242`)は変わらない。

**起動時刻の取得**(`own_identity` が返す `ProcessStartTime`)

`ProcessStartTime` は不透明値の**等価比較専用**(`Ord` を持たない)なので、記録側(`own_identity`)と照合側(#3 の `starttime_of`)が**同じプロセスに対して必ず同じ文字列を得る**ことが契約の実質になる。関数を1つにするだけでは足りない — 同じ関数でも実行環境が違えば違う表現を返す手段があるため、取得時の環境まで固定する。

- 取得手段を `adapter::process` の1つの private 関数に閉じ、#3 が足す `starttime_of` と共有する。署名は **三値** — `fn observe_process(&self, pid: Pid) -> Result<Option<ObservedProcess>, Io>`(`ObservedProcess { starttime: ProcessStartTime, kill_ident: KillIdent }`)。`Ok(Some)` = 取得できた / `Ok(None)` = **対象プロセスが存在しない** / `Err(Io)` = 取得機構そのものの失敗。PGID を同じ関数から返すのは、POSIX ではどちらも同じ1回の観測(`ps` の1回の起動 / `/proc/<pid>/stat` の1回の読み取り)から取れるため。
  - `own_identity` は自プロセスを観測するので二値でよく、`Ok(None)`(自分が見つからない = ありえない)を `Err(Io)` に畳む1行を呼び出し側に置く。**畳むのは呼び出し側であって共有関数ではない。**
  - 三値を本スライスで確定させるのは、#3 の `starttime_of` の契約が `Ok(Some)` / `Ok(None)`(死亡)/ `Err(Io)`(機構失敗)の三値であり、`TC-port-process-controller-007` / `011` がこの区別そのものを検証するため。二値に確定させると #3 が共有関数の署名を変えることになり、「同一の取得手段を構成で保証する」が最初の追加で崩れる。さらに実測では不在も**エラーの形**で返る(macOS の `ps -o lstart=,pgid= -p <不在pid>` は exit 1・stdout 空、Linux の `/proc/<pid>/stat` は `NotFound`)ため、二値のままだと `Err(Io)` に畳む実装が自然に書けてしまう。畳むと #3 で「生存プロセスを観測できない tick が状態を変更せずスキップし続け、`DiedWithoutExit` にも `KillOnTimeout` にも到達しない」= running のまま永久滞留になる。
  - 取得元(POSIX 非 Linux は `ps` の実行ファイルパス、Linux は procfs のルート、Windows は powershell の実行ファイルパス)は構築時に注入する(ADR-076)。
- Linux: `<procfs_root>/<pid>/stat`(既定は `/proc`)の起動時刻(clock ticks)を **boot id と合成した `<boot_id>:<ticks>`** とする。ファイル読み取りのみで、表現は整数と UUID なのでロケール非依存。ticks は**最後の `)` より後ろを空白で分割した20番目**(全体の22番目)として読む — 2番目のフィールド(comm)は実行ファイル名を `()` で囲んだもので、名前に空白や `)` を含むと素朴な空白分割では位置がずれる(実測: `sleep` を `sl ee) p` という名前で起動すると素朴な22番目は `1` を返し、この規則では正しい `939856880` を返す)。自プロセス名では顕在化しないが、この関数は任意 pid を取る #3 の `starttime_of` と共有される。boot id は `<procfs_root>/sys/kernel/random/boot_id`(非 root で読め、同一 boot 内では何度読んでも同じ値であることを実測)。
- その他の POSIX(macOS 等): `<identity_source> -o lstart=,pgid= -p <pid>`(既定は `/bin/ps`)の1行の出力を、最後の空白で区切って PGID(最終トークン)と `lstart`(残りを trim した文字列)に分ける。ただし `ps` の起動時に `LC_ALL=C` と `TZ=UTC` を**注入**し、`LANG` / `LC_TIME` / `LC_ALL` の継承を落とす(`adapter/worktree.rs` の `INHERITED_GIT_ENV` + `env_remove` と同型)。trim はこの関数の内側で1回だけ行う(末尾に空白パディングが付く。実測: `LC_ALL=C TZ=UTC` で `Wed Aug 12 11:44:13 2026     96379`)。キーワードの順を逆にすると `lstart` 側にパディングが残るので、`lstart=,pgid=` の順で固定する。
- Windows: `powershell -NoProfile -Command "(Get-CimInstance Win32_Process -Filter \"ProcessId=<pid>\").CreationDate"` 相当。`ToString()` はカルチャ依存なので `.ToUniversalTime().ToString("o")` の**不変形式**を明示する(実機検証は #10 だが、形式の決定は本スライスの責務)。
- `StartTimeRecord.wall` は注入した `Clock` から取る。

**結末の写像**(三値のどこに落ちるかを結末ごとに書き分ける。「中身が空なら `Err(Io)`」だけでは非0終了の場合を規定できず、不在を機構失敗に畳む実装を防げない)

| 結末 | 写像 |
|---|---|
| POSIX 非 Linux: `ps` の起動自体が失敗(実行ファイル不在・実行不能) | `Err(Io)` |
| POSIX 非 Linux: 非0終了 **かつ** stdout が空 | `Ok(None)`(不在。実測: 存在しない pid で exit 1・stdout 空) |
| POSIX 非 Linux: それ以外の非0終了 | `Err(Io)` |
| POSIX 非 Linux: exit 0 かつ trim 後が空 / 最終トークンが PGID として読めない | `Err(Io)` |
| Linux: 注入された procfs ルートが存在しない・読めない | `Err(Io)`(機構そのものが無い) |
| Linux: ルートは在るが `<root>/<pid>/stat` が `NotFound` | `Ok(None)`(不在) |
| Linux: その他の読み取り失敗・フィールドが形式外・boot_id が読めない | `Err(Io)` |
| Windows: powershell の起動自体が失敗 | `Err(Io)` |
| Windows: exit 0 かつ出力が空(`Get-CimInstance` が該当なしで空を返す) | `Ok(None)`(不在。実機確認は #10) |
| Windows: それ以外の失敗・形式外 | `Err(Io)` |

Linux で「ルート不在」と「`<root>/<pid>/stat` の `NotFound`」を分けるのは、取得元が注入される(ADR-076)ため両者が同じ `NotFound` として現れるから。分けないと、壊れた取得元を注入したときに `Ok(None)` = 「そのプロセスは死んでいる」が返り、#3 の `TC-port-process-controller-007` / `011`(機構失敗を死亡に写像しない)が注入で再現できなくなる。

`ProcessStartTime::parse` の `Empty` を握り潰して既定値で `Ok` を装わないことが `TC-port-process-controller-005`(不正な同定情報で `Ok` を装わない)の趣旨そのものであり、上表の `Err(Io)` 行はすべてこの趣旨に属する。

Linux で boot 相対の clock ticks を**そのまま**採らない理由: starttime は起動からの相対値なので、「再起動 → pid が再利用され、かつ起動 tick 数が一致」が同時に成立すると、`IdentityCheck` が別プロセスを `Alive` と誤判定する。確率は低いが、結末は #3 の `KillOnTimeout` が無関係なプロセスグループを kill することで、plan.md が「最悪の失敗モード」と呼ぶものより悪い。boot id は再起動ごとに新しい UUID になるため、合成した値は boot を跨いで一致しない。`/proc/stat` の `btime` を足して絶対時刻に落とす案を採らない理由: ticks を秒に直すには `CLK_TCK` が要り、`unsafe` も新規依存も使わない方針では `getconf` の起動が増える(std に `sysconf` は無い)。等価比較しか要求されていない値のために、取得のたびに外部プロセスを起動する理由がない。

環境を固定しない案(既定のロケール・TZ のまま `ps` を起動する)を採らない理由: 同一 pid・同一プロセスに対する実測が環境で変わる。

```
(既定, ja_JP)   水  8/12 20:14:45 2026
LC_ALL=C        Wed Aug 12 20:14:45 2026
LC_ALL=C TZ=UTC Wed Aug 12 11:14:45 2026
```

ラッパーは tick から環境を継承して起動されるため、「cron の tick(`LANG` 未設定)が spawn したラッパーが記録 → 対話シェル(`ja_JP.UTF-8`)の tick が照合」で不一致になる。結果は `IdentityCheck = Dead` → `DiedWithoutExit` → 再起動 → **エージェント実行中の同一 worktree での並走**で、plan.md が「最悪の失敗モード」と書いている経路に精度ではなく表現の非決定性から到達する。`LC_ALL=C TZ=UTC` の注入は周囲の `LANG` / `LC_TIME` を上書きすることを実測で確認した。

**既定の取得元は絶対パスで固定する**

環境の固定だけでは足りない。固定したのは実行時の環境であって、**どの実行ファイルが起動されるか**は PATH に委ねられたままになる。cron の tick(`PATH=/usr/bin:/bin`)と対話シェルの tick(`PATH=/opt/homebrew/bin:...`)で `ps` が別実装に解決されれば `lstart` の整形が変わり、結末はロケール非固定の場合とまったく同じ — requirements §4.3 の「記録と照合は同一の取得手段」が PATH の安定性に依存する。`git` を PATH 解決している判断(ADR-024)とは性質が違う。git は結果を分類にしか使わないが、`ps` の出力は**帳簿に永続化されて後の tick と突き合わされる**。

- POSIX 非 Linux: 既定を `/bin/ps` とする。`/usr/bin/ps` は macOS に存在しない(実測)。
- Linux: `/proc` が元から絶対パスなので影響しない。
- Windows: 既定を `powershell` の PATH 解決のまま残す。`%SystemRoot%\System32\WindowsPowerShell\v1.0\powershell.exe` は本環境で実測できず、検証していない絶対パスを固定すると取得そのものが不能になる — PATH 解決より悪い結末になる。

Windows だけ PATH の安定性への依存が残る。この非対称は既知の制限として progress.md に載せ、Issue #10(CI とクロスプラットフォーム検証)の実機確認に申し送る。注入点(ADR-076)はどのプラットフォームでも変わらないので、既定値の差は適合テストの構成にもアダプターのユニットテストにも現れない。

**`run_agent`**

- `Command` を直接起動(シェル非経由)。cwd = worktree。stdout / stderr は `File::create` したログへ。
- 符号化: cwd に到達できない → 126(エージェントを起動しない)/ ログを開けない → 126(同上)/ `ErrorKind::NotFound` → 127 / `ErrorKind::PermissionDenied` → 126 / その他の起動失敗 → 126 / 正常終了 → exit code / シグナル死 → `ExitStatusExt::signal()` から 128+n。
- cwd の存在確認を起動前に行うのは、`Command::spawn` が cwd 不在でも `NotFound` を返すため(コマンド不在の 127 と区別できない)。`TC-port-process-controller-022`(127)と `026`(126 相当)を両立させる唯一の分岐点になる。

### Consequences

- 良い点: `unsafe` ゼロ・新規依存ゼロを維持したまま §4.3 の抽象を満たせる。起動時刻の取得手段と取得時の環境が1関数に閉じるので、#3 の `starttime_of` が同じ表現を得ることを構成で保証できる。署名を三値まで固定してあるので、#3 は共有関数をそのまま呼ぶだけで `starttime_of` を足せる(署名の変更が要らない)。
- トレードオフ: `own_identity` しか消費者がいない本スライスでも三値を扱うことになり、`Ok(None)` を `Err(Io)` に畳む1行が呼び出し側に現れる。この1行は `TC-port-process-controller-005` が注入で観測する対象でもある(ADR-076)。
- トレードオフ: macOS / Windows では外部プロセス(`ps` / `powershell`)の起動が要る(1回あたり数十ms)。`ps -o lstart=` は秒精度なので、1秒以内の PID 再利用は検出できない(spec は「同一マシン内での等価比較」しか要求していないので契約上は満たす)。macOS の `ps` にはエポック秒を出す keyword が無く(`etimes` は未サポート、`start` は精度が落ちる)、整形済み文字列を固定した環境で読む以外の手段が取れない。Windows の起動時刻取得は本環境で実測できないため、Issue #10(CI とクロスプラットフォーム検証)に委ねる。
- **`process_group(0)` はセッションを分けない**(`setsid` 相当ではない)。適合契約(呼び出し側プロセスの終了後も完走する)は満たすが、ラッパーは同一セッションに残って制御端末を保持し続けるため、端末セッションの強制終了(vhangup 等)では SIGHUP が届きうる — `nohup` 相当の耐性は無い。cron 運用が主経路なので実害は小さいが、`setsid(1)` に依存しない判断とセットで残す。
- **Windows の `KillIdent`(pid 文字列)は POSIX の実行単位に相当しない**。requirements §4.3 は Windows のkill同定子として「タスクID・attempt番号から決定的に導出する名前付きジョブオブジェクト名」を例示し、`TC-port-process-controller-014`(ラッパーのみ死亡し、エージェントが実行単位に属したまま生存)は `try_kill_remnants` で `Killed` を要求する。pid 文字列 + プロセスツリー終了ではラッパー死亡時点で辿る根が無く、この契約を満たせない。`KillIdent` は pidファイルとタスクファイルに**永続化される**値なので、後から形式を変えると既存の帳簿が無効になる。`unsafe` 禁止のままジョブオブジェクトを扱う手段が無い以上、#3 / #10 で依存追加か lint 緩和の判断が要りうる申し送りとして残す。
- トレードオフ: 既定の取得元の性質がプラットフォームで揃わない(POSIX は絶対パス、Windows は PATH 解決)。requirements §4.3 の「記録と照合は同一の取得手段」が Windows でだけ PATH の安定性に依存したまま残る。
- `#[cfg(unix)]` / `#[cfg(windows)]` の出現箇所が `adapter/process.rs` に増える(既存は `util/atomic.rs` のみ)。AC-1 の grep の期待値を更新する — 述語を4つに広げた検査は `adapter/task_repository.rs` の `#[cfg(all(test, unix))]` も拾うため、`crates/pulsen/src/` 側は3ファイルになる(このヒットはテストモジュールで、本番の実行経路には乗らない)。`cfg(all(test,` を除外する規則は足さない。除外すると「test 限定」を装う本番分岐が検査を素通りする。

---

## ADR-076: `ProcessController` は自バイナリのパスと同定情報の取得元を構築時に注入される

### Status

Proposed

→ `.adr/076-process-controller-injects-self-exe-and-identity-source.md` に昇格

### Context

requirements §4.1 はラッパーを「ツール自身のバイナリをラッパーモードで再実行する方式」と定める。アダプターが `std::env::current_exe()` を直接読むと、適合テスト(テストハーネスのバイナリが実行主体)が**テストバイナリ自身**をラッパーとして再実行してしまい、`TC-port-process-controller-001 / 002` が成立しない。加えて `TC-003`(ラッパーの起動自体が不可能)を再現する手段が無くなる。

同定情報の取得も同じ形をしている。`TC-port-process-controller-005`(取得機構が失敗したとき `Err(Io)` を返し、不正な値で `Ok` を装わない)は、実行中の自プロセスに対する `ps` / procfs の読み取りを外から壊せない限り再現できない。権限操作で作る案は root 実行やファイルシステムの種類に依存し、確定的に走らせられない。

### Decision

`SystemProcessController::new(self_exe: PathBuf, identity_source: IdentitySource, clock: SystemClock)` とし、実行ファイルのパスと**同定情報の取得元**を構築時に注入する。`std::env::current_exe()` を読むのは合成ルート(`cli::wire`)の1箇所だけにする — ADR-024 が `git_program` を、ADR-031 が環境の読み取りを合成ルートに閉じたのと同型。

`IdentitySource` は `PathBuf` の newtype で、`IdentitySource::platform_default()` が POSIX 非 Linux では `ps` の実行ファイルパス、Linux では procfs のルート(`/proc`)、Windows では powershell の実行ファイルパスを返す。プラットフォーム分岐は `adapter/process.rs` の中に閉じるので、合成ルートは `platform_default()` を呼ぶだけで cfg を持たない(AC-1)。

ただし読み取りを `compose()` には**載せない**。`compose()` は `add` も通る合成ルートで、`ProcessController` を必要とするのは `tick` と `wrapper` だけである。`WireError::SelfExeUnavailable` を `compose()` に置くと、プロセス起動と無関係な `add` が `current_exe()` の失敗で落ちる。Issue #1 で確立した「各コマンドは自身の動作に必要なリソースだけを検証する」(pages 縮退状態の共通規則1)に従い、`wire::process_controller() -> Result<SystemProcessController, WireError>` として切り出し、`cli/tick.rs` と `cli/wrapper.rs` から呼ぶ。

- 適合テストのハーネスは `env!("CARGO_BIN_EXE_pulsen")` を `self_exe` として渡す(統合テストでのみ使える環境変数)。
- `SpawnError` の再現(`TC-port-process-controller-003`)は、**存在しないパスを `self_exe` として構築した2つ目のコントローラ**をハーネスの `failing_controller` フックが返す形にする(ADR-024 / ADR-027 の `failing_manager` と同型)。本番のインスタンスはイミュータブルなまま、権限操作にも root 実行の可否にも依存しない。
- 取得機構失敗の再現(`TC-port-process-controller-005`)は同型で、**存在しないパスを `identity_source` として構築した2つ目のコントローラ**を `failing_identity_controller` フックが返す形にする。ADR-075 の写像表により、どのプラットフォームでもこの構成は `Err(Io)` に落ちる — POSIX 非 Linux は `ps` の起動自体の失敗、Linux は procfs ルートの不在(`<root>/<pid>/stat` の `NotFound` と区別して機構失敗に写す規則を ADR-075 が持つ)。`005` はこのフックによって確定的に走り、スキップ見込みの行ではなくなる。
- スイートを2つに分けている(ADR-083)ので、フックも `spawn` スイートの `failing_controller` と `identity_and_agent` スイートの `failing_identity_controller` に分かれる。

**ラッパーの合成だけは `self_exe` を持たない構成を使う**

`wire::process_controller()` の3引数固定から外れ、`cli/wrapper.rs` は `SystemProcessController::without_self_exe(identity_source, clock)` を組む。ラッパーが呼ぶのは `own_identity` と `run_agent` だけで、`spawn_wrapper` は呼ばない。それでも `current_exe()` を解決させると、その失敗**だけ**でラッパーが何も書かずに非0終了し、tick の猶予経路が spawn 失敗として積む — 使わないリソースの検証が失敗経路を1本増やす形になる。ADR-076 が `compose()` から `process_controller()` を切り出したのと同じ規則(各コマンドは自身の動作に必要なリソースだけを検証する)を、コマンドの中でもう一段適用する。

この構成の `spawn_wrapper` は構造上必ず `SpawnError` を返す。適合契約の対象は `new(...)` の構成に限ると宣言し、`without_self_exe` の doc にその範囲を書く。

`spawn_wrapper` を持たない狭いポート(`own_identity` + `run_agent`)を切り出して型で閉じる案を採らない理由: ポートの分割は #3 が足す kill 系メソッドの配置と一緒に決めるべきもので、先に分けると二度分けることになる。

### Consequences

- 良い点: 適合スイートが実バイナリのラッパー動作を検証でき、`spawn_wrapper` の3件が「再現できるアダプター環境に限る」のスキップに落ちない。`current_exe()` の失敗経路がプロセス起動を行うコマンドだけに閉じる。`TC-005` も権限操作にも root の可否にも依存せず走る。
- 良い点: この注入点は、ADR-075 の三値化(不在 / 機構失敗の区別)が正しく実装されたことを #3 が `starttime_of` に対して検証するときの足場にもなる。
- トレードオフ: 合成ルートで `current_exe()` が失敗したときの経路が増える(`WireError` に1変種)。tick はそれを実行環境エラーとして非0で終え、wrapper は何も書かずに非0で終える(猶予経路が spawn 失敗として分類する)。
- トレードオフ: 本番の構築が引数3つになり、`IdentitySource::platform_default()` という「既定値を返す関数」が1つ増える。既定を型の中に置くことで、合成ルートがプラットフォームごとの取得元を知らずに済む形を選んだ。
- トレードオフ: 本番に結線されるポート実装が2構成になり、うちラッパー側は `spawn_wrapper` が必ず失敗する。後続スライスがラッパーの経路で `spawn_wrapper` を使っても型は止めず、実行時エラーになる。この構成は適合契約の対象外であることを doc の宣言で担保している。

---

## ADR-077: `wrapper` は clap の隠しサブコマンドにする

### Status

Proposed

→ `.adr/077-wrapper-is-a-hidden-subcommand.md` に昇格

### Context

pages は `wrapper` を「ツール自身のバイナリをラッパーモードで再実行するための内部サブコマンド。利用者向けのインターフェースではない(ヘルプの一覧にも表示しない)」と定める。選択肢: (a) 別の bin ターゲット、(b) `examples/` のプログラム(`lock_holder` の先例)、(c) `#[command(hide = true)]` のサブコマンド。

### Decision

(c) を採る。requirements §4.1 が「**ツール自身のバイナリ**をラッパーモードで再実行する」と明示しており、(a) は配布物が2つになる、(b) は `cargo build --release` の成果物に含まれず本番で起動できない。

- `Command` enum に `#[command(hide = true)] Wrapper(WrapperArgs)` を足す。
- `WrapperArgs` の末尾可変長(エージェントコマンド)は `trailing_var_arg` + `allow_hyphen_values` で受ける。エージェントのコマンドは `--model` のような `-` 始まりのトークンや、config の配列形式が許す空文字列トークン(`TC-port-config-store-007`)を含みうる。明示しないと `spawn_wrapper` が組んだ argv をラッパー側のパーサが受理せず、起動経路だけが壊れて runディレクトリには何も残らない(ADR-049 で `--base` のハイフン値に対して行ったのと同じ扱い)。argv の定義箇所がアダプターと CLI に分かれるため、往復テストで受理を主張する。
- 既存の `tests/cli_usage.rs::提供するサブコマンドはタスク登録だけである` を更新し、「ヘルプに現れるのは `add` / `tick` / `help` であること」と「`wrapper` はヘルプに現れないが実行はできること」の2つの主張に分ける(`PAGE-wrapper-001` の検証点)。
- `--home` は `global = true` なので `wrapper` にも付くが、wrapper はこれを**使わない**(ADR-078)。

### Consequences

- 良い点: 配布物が1つのまま、spec の「ヘルプに出さない」を clap の機能で満たせる。
- トレードオフ: `pulsen wrapper --help` は動く(隠れているだけで到達可能)。spec は「一覧に表示しない」としか言っていないので契約は満たす。

---

## ADR-078: ラッパーは config もホームも読まず、`RunDirPath` から state root を復元して RunStore を組む

### Status

Proposed

→ `.adr/078-wrapper-restores-state-root-from-run-dir.md` に昇格

### Context

`RunWrapper` の入力は `WrapperLaunchSpec { run_dir, workspace, agent_cmd }` の3つだけで、spec は「config は読まない(必要な情報はすべて起動引数で受け取る)」と定める(`TC-exec-run-wrapper-007`: config.yaml が不在・破損した環境でも動作に影響しない)。

一方 `RunStore::prepare_attempt(id, n)` は `RunDirPath::derive(state_root, id, n)` と一致するパスを返す契約なので、fs 実装は `StateRoot` を構築時に注入される(ADR-043)。ラッパーは `prepare_attempt` を呼ばないが、`RunStore` の実装を1つの型として構築する以上 `StateRoot` の値が要る。

選択肢: (a) ラッパーでもホームを解決して `PulsenHome::state_root()` を使う、(b) 合成ルートが `run_dir` の親を3つ遡って組み立てる、(c) `RunDirPath::derive` の逆写像をドメインに置き、そこから復元する。

### Decision

(c) を採る。`RunDirPath::state_root(&self) -> Option<StateRoot>` を `derive` の直下に置き、`<state_root>/runs/<task-id>/attempt-<n>` の形に合致しない値には `None` を返す。ラッパーの合成は `--run-dir` の値だけから RunStore を構築し、ホームの解決も `ConfigStore::load` も行わない。復元できない `run_dir` は起動引数の不正として扱い、**何も書かずに非0終了**する(`TC-exec-run-wrapper-009` の経路に合流し、猶予経路が spawn 失敗として分類する)。

(a) を採らない理由: `--home` で起動された tick が spawn したラッパーは `--home` を受け取らないため、既定の `~/.pulsen` を解決してしまう。値が使われないので実害は無いが、「使われないことに依存した配線」は次の変更で壊れる。(b) を採らない理由: レイアウト知識(`runs/` という段があること)が合成ルートに漏れ、`RunDirPath::derive` と2箇所に分かれる。

### Consequences

- 良い点: ラッパーが config・ホーム・環境変数のいずれにも依存しなくなり、`TC-exec-run-wrapper-007` が定義どおりに成立する。レイアウトの知識は `task/path.rs` の1箇所に留まる。
- トレードオフ: `derive` の逆写像はドメイン台帳(`spec/inventory/domain.md`)に無い追加。spec 追従を Issue のコメントで提起する(Issue #9 と同じ扱い)。

---

## ADR-079: `CommandLine` にプロセス境界からの再構築経路を足す

### Status

Proposed

→ `.adr/079-command-line-rehydrate-across-process-boundary.md` に昇格

### Context

`WrapperLaunchSpec.agent_cmd` は `CommandLine`(`DOM-definition-023`: 「`CommandTemplate::expand` の結果としてのみ生成される1トークン以上のトークン列」)。tick はこれを argv に直列化し、ラッパーは argv から復元して `run_agent` に渡す。しかし `CommandLine` には公開コンストラクタが無く、`definition` モジュールの外では作れない。

選択肢: (a) `run_agent` の引数を `PlainCommand` に変える、(b) `CommandLine` に再構築経路を足す、(c) ラッパー側でテンプレート展開をやり直す。

### Decision

(b) を採り、`CommandLine::rehydrate(tokens: Vec<String>) -> Result<Self, CommandError>`(0トークンは `Empty`)を足す。doc に「プロセス境界(ラッパーの起動引数)からの再構築専用の経路であり、検証済みのトークン列がそのまま往復することだけを保証する」と why を書く。`Task::rehydrate` / `AttemptRef::rehydrate` と同じ命名・同じ位置づけにする。

(a) を採らない理由: spec が `run_agent(cmd: &CommandLine, ...)` と型を明示している。(c) を採らない理由: ラッパーが config とスナップショットを読むことになり、「config は読まない」に反する。

### Consequences

- 良い点: 直列化 → 復元の往復がドメインの型で閉じ、ラッパー側に文字列のままのコマンドが漏れない。`TC-exec-run-wrapper-009`(直列化の破れ)が `Err` として素直に書ける。
- トレードオフ: 「`expand` の結果としてのみ生成される」という台帳の記述と食い違う。生成経路が2つ(展開・再構築)になることを spec 側に反映する提起を Issue のコメントに残す。

---

## ADR-080: runディレクトリの各ファイルは JSON で書き、マーカーは空ファイルにする

### Status

Proposed

→ `.adr/080-run-dir-files-are-json-and-markers-are-empty.md` に昇格

### Context

pid / starttime / exit の内容表現は spec が定めていない(ファイル名と意味だけが決まっている)。要件は3つ — (1) `Corrupt` を「不在」と区別できること、(2) 人間が直接辿れること(requirements §9)、(3) アトミック置換で書きかけが観測されないこと。

### Decision

タスクファイルと同じく `serde_json` の DTO で書く(ADR-023 / ADR-025 の作法をそのまま適用する)。

| ファイル | 内容 |
|---|---|
| `pid` | `{"pid": 4242, "kill_ident": "-4242"}` |
| `starttime` | `{"ident": "<不透明値>", "wall": "2026-08-12T09:15:30Z"}` |
| `exit` | `{"code": 0}` |
| `invalidated` | 空ファイル(存在のみが意味を持つ) |

- 復号は「JSON として読めない」または「値制約(`KillIdent` の非空・`ProcessStartTime` の非空・`Timestamp` の RFC3339)を満たさない」を `RunFileError::Corrupt { path, message }`、ファイル/ディレクトリの不在を `Ok(None)`、機構の失敗を `RunFileError::Io { message }` に写像する。
- 書き込みは `util::atomic::write_atomic` を呼ぶだけにする(CLAUDE.md「個別に再実装しない」)。`write_invalidation_marker` はディレクトリを `ensure_dir` してから空バイト列を書く。
- `write_atomic` が `ensure_dir` を内蔵しているため、`write_starttime` / `write_pid_file` / `write_exit` も書き込み先のディレクトリを作る。これを実装の副産物にせず**ポートの契約として明記する** — `prepare_attempt` が失敗した後でも spawn は行われる設計(状態を変えずに報告のみ)なので、ラッパーが自力でディレクトリを作って書けることが自己修復の前提になっている。契約に無いままだと、後続スライスが「write 系はディレクトリ不在で失敗する」と誤読し、適合テストの前提も定まらない。

素のテキスト(`4242` の1行など)を採らない理由: `pid` は2値を持つので区切り規則を発明することになり、`Corrupt` の判定条件が手書きになる。JSON なら「パースできない = Corrupt」が構文で決まり、タスクファイルの復号と同じ形になる。

### Consequences

- 良い点: 破損判定が一様になり、`TC-port-run-store-006 / 011 / 015` がフック1つ(「解釈不能な内容を直接置く」)で書ける。show(#4)が exit を読むときも同じ DTO を使える。
- トレードオフ: `cat exit` が `0` ではなく `{"code":0}` になる。マニュアルテストの確認手順が JSON を読む形になる。

---

## ADR-081: tick の `errors` は構造化した値で返し、文言は CLI 層で組み立てる

### Status

Proposed

→ `.adr/081-tick-errors-are-structured-values.md` に昇格

### Context

`UC-execution-002` の出力 DTO は `errors: Vec<{ task_id: Option<TaskId>, path: Option<PathBuf>, message: String }>` と書かれている。素直に読むと `message: String` はユースケースが文言を組み立てることを意味するが、Issue #1 で確立した規約(`RegisterTask` はエラーを構造で返し、文言の組み立ては `cli::render`)と食い違う。

### Decision

`errors` の要素を構造化した enum(`TickIssue`)にする。`task_id` / `path` は要素が持ち、原因は「破損したタスクファイル」「スナップショット破損」「不変条件の破れ」「runファイルの読み取り失敗」「書き込み順序の破れ」「マーカー書き込みの失敗」「`prepare_attempt` の失敗」「spawn の同期エラー」「`save` の失敗」のように**分類として**持つ。`cli::render` が spec の `message` に相当する文言へ落とす。

spec の `message: String` は「報告に足る情報が載ること」を要求していると読み、文言の**組み立て位置**は既存規約に従う。

### Consequences

- 良い点: ユースケース層のテストが文言に依存せず分類で主張できる(実装の内部構造に依存しない。CLAUDE.md テスト方針)。#3 / #6 が分類を足すのも1変種の追加で済む。
- トレードオフ: spec の DTO 表と型が一致しない。spec 追従の提起を Issue のコメントに残す。

---

## ADR-082: 適合テストのエージェントとデタッチ性の検証は `examples/` のプログラムで供給する

### Status

Proposed

→ `.adr/082-agent-and-spawn-probes-as-examples.md` に昇格

### Context

`TC-port-process-controller-017〜027`(`run_agent` 11件)は「exit code を制御できる」「引数どおりに出力する」「作業ディレクトリを検査する」「一定時間実行し続ける」「exit code を持たない終了をする」テスト用コマンドを要求する。`TC-002`(デタッチ性)は「**呼び出し側プロセスを終了させ**、別プロセスから runディレクトリを観測する」ことを要求し、in-process のテストでは表現できない。

シェル(`sh -c`)に頼るとクロスプラットフォームで破綻し、「シェルを介さない直接起動」を検証する `TC-021` と矛盾する。

### Decision

`crates/pulsen/examples/` に2つのプログラムを置く(`lock_holder.rs` の先例と同じ位置づけ — 利用者に見えるサブコマンドを増やさないため)。

- `agent_probe.rs`: 第1引数のモードで振る舞いを変えるテスト用エージェント。`exit <n>` / `print <stdout文字列> <stderr文字列>` / `check-cwd <期待パス>` / `echo-args <期待トークン...>` / `sleep <ミリ秒>` / `abort`。シグナル死は `std::process::abort()`(SIGABRT → 128+6)で作る — `unsafe` なしでシグナルによる終了を再現できる唯一の手段。
- `spawn_probe.rs`: argv で受け取ったバイナリパス・run_dir・workspace・エージェントコマンドから `SystemProcessController` を組み立て、`spawn_wrapper` を呼んで**即座に終了する**。`TC-002` はこれを起動して `wait` し、その後 runディレクトリに starttime / pid / exit が現れることを確認する。

ハーネスは `tests/common/lock.rs::holder_program()` と同じ規則(`<テストバイナリのディレクトリ>/examples/<name><EXE_SUFFIX>`)でパスを解決し、見つからなければフックが `None` を返してスキップする。

「実行権限がない実体」(`TC-023`)は POSIX でのみ作れるため、`permission_restrictions_effective()` と同じ「実際に効いたことを確認してから `Some` を返す」規則のフックにする(ADR-027)。

シグナル死のケース(`TC-port-process-controller-024`)の期待は spec でも「**非0の符号化値**(POSIX慣例では 128+シグナル番号)」であり、値そのものではない。適合スイート側は**非0の主張に留め**、`128+6` の具体値は `adapter/process.rs` の POSIX 側ユニットテストに置く。適合契約は契約の語彙(「非0の符号化値」)で書き、具体値はそれを満たす実装の性質としてアダプター側で固定する — 適合スイートのケース関数にプラットフォーム分岐を増やさない(`pulsen-conformance` の既存の `#[cfg]` は能力プローブ `probe_permission_restrictions` に限られており、ケースの主張は分岐していない)。

### Consequences

- 良い点: `run_agent` の11件と `spawn_wrapper` のデタッチ性が、シェルにもプラットフォーム固有コマンドにも依存せず検証できる。適合スイートはプラットフォーム非依存のまま保たれる。
- トレードオフ: `examples/` に2つプログラムが増え、適合テストが `cargo build --examples` に依存する(既存の `lock_holder` と同じ依存なので新しい制約ではない)。`abort` によるシグナル死と実行権限のケースは Windows でスキップになる。

---

## ADR-083: `spawn_wrapper` の適合3件は `wrapper` サブコマンドの実装後に適用する

### Status

Proposed

### Context

`TC-port-process-controller-001 / 002 / 003` は「`spawn_wrapper` の結果として runディレクトリに starttime / pid / exit が揃う」ことを主張する。これは `pulsen wrapper` が実装されて初めて成立するため、適合スイートの適用がアダプターの実装ステップでは閉じない(ADR-027 が想定する「契約を書く → 実装が通す」でステップを閉じる形が崩れる)。

### Decision

実装ステップの順序を依存関係に合わせ、`wrapper` サブコマンドをアプリケーション層の直後・`tick` より前に置く。

1. ProcessController のアダプター実装 + `own_identity` / `run_agent` の適合13件(ステップ8)
2. RunWrapper ユースケース(ステップ9)→ `wrapper` 隠しサブコマンド(ステップ10)
3. `spawn_wrapper` の適合3件を適用(ステップ11)
4. Tick のユースケースと `tick` サブコマンド(ステップ13〜17)

適合スイート側は `pulsen_conformance::process_controller` を2つのモジュールに分ける — `identity_and_agent`(`own_identity` / `run_agent` の13件)と `spawn`(`spawn_wrapper` の3件)。適用ファイル `tests/conformance_process_controller.rs` はステップ8 で作って `identity_and_agent` を適用し、ステップ11 が同じファイルに `spawn` の適用を1行足す。`conformance_cases!` はスキップ宣言の置き場をスイートごとに別名で受け取る設計なので、1つのテストファイルに2つのスイートを適用できる。

1つのモジュールに16件を書いて適用をステップ11 にまとめる案を採らない理由: ステップ8 が「契約を書く → 実装が通す」で閉じなくなり、完了条件を「アダプターのユニットテストで13件を裏付ける」に落とすことになる。同じ主張が適合ケースとユニットテストに二重に書かれ、どちらが契約でどちらが実装の性質かが読めなくなる。

### Consequences

- 良い点: 「CLI は最後」という一般則より「依存の順」を優先することで、スタブ無しで各ステップを閉じられる。スイートを分けたことで、ステップ8 は13件を実際に適用して通すところまでを完了条件にできる。
- トレードオフ: `wrapper` サブコマンドだけが `tick` より前に CLI へ現れる。`cli_usage.rs` の更新が2回に分かれる(ステップ10 で `wrapper` を隠す主張、ステップ17 で `tick` をヘルプに出す主張)。ポート1つに対する適合スイートが2つになるため、後続の実装がスイートを適用するときは両方を呼ぶ必要がある(`HOOKS.md` の対応表で対にする)。

---

## ADR-084: `prepare_attempt` の適合ケースは `attempt_exists` を使わずに観測する

### Status

Proposed

### Context

`TC-port-run-store-001` の期待は「親を含めて attempt ディレクトリが作成され、`attempt_exists` が true になる」、`TC-002` は「既存の書き込み済みファイルの内容に影響しない」。しかし `attempt_exists`(`DOM-execution-038` / `ADP-runstore-005`)は #4 のチェックリスト行で、本スライスは「使わないメソッドは宣言しない」を守る。

選択肢: (a) `attempt_exists` を宣言して #4 の行を先取りする、(b) 適合スイート内で実ファイルシステムを直接見る、(c) 観測をハーネスのフックと既存の read 系に置き換える。

### Decision

(c) を採る。

- `TC-001`: ハーネスの `attempt_dir_present(run_dir) -> bool` が `prepare_attempt` の**前は false・後は true** になること、`prepare_attempt` が `Ok` を返し、返るパスが `RunDirPath::derive` と一致すること。
- `TC-002`: write 系で書いたファイルがある状態で `prepare_attempt` を再実行しても `Ok` で、read 系が同じ値を返すこと。

主張を「後は true」だけにしない理由: `attempt_dir_present` は ADR-027 が想定する「破損・状況の**意味**を渡す(前提を作る)」フックではなく、**観測**をハーネスに委ねるフックである。常に `true` を返すハーネス実装でもケースが緑になり、この行の主眼(親を含めて attempt ディレクトリが作成されること)が検証されない。前後で反転することを主張すれば、定数を返すハーネスはどちらかの側で落ちる。`HOOKS.md` の当該行(区分 B)にもこの使い方を書き、後続スライスの別バックエンドが同じ弱め方をしないようにする。

(a) を採らない理由: 宣言だけして本スライスの呼び出し側が無いメソッドが増え、AC-6(未実装メソッドの宣言・スタブが1つも無い)と Issue の完了条件を破る。(b) を採らない理由: 適合スイートは対象のバックエンドを知らない形で書く約束(ADR-027 のフックは「破損・状況の意味」だけを受け取る)で、ファイルシステム前提を直に埋め込むと fs 以外の実装に適用できなくなる。

### Consequences

- 良い点: ポートの宣言をスライス境界どおりに保ったまま、spec が意図した「親を含めて作られる」「冪等で内容を壊さない」を検証できる。
- トレードオフ: 「ファイルを1つも書いていない空の attempt ディレクトリ」を read 系だけでは区別できないため、その観測にハーネスのフックを1つ足す(`HOOKS.md` では区分 B)。#4 が `attempt_exists` を足すときに `TC-022 / 023` とあわせてフックの要否を見直す。
- `TC-port-run-store-001` は台帳の期待文が `attempt_exists` を名指ししているため、代替観測で満たしたものとしてチェックを付ける(plan.md「チェックリスト行にチェックを付ける基準」の例外)。

---

## ADR-085: worktree の同定は物理パスで行い、実体の消えた登録は張り直す

### Status

Proposed

→ `.adr/085-worktree-identified-by-physical-path.md` に昇格

### Context

`WorktreeManager::create` の冪等性は「`ws.path` に `ws.branch` の worktree がある」場合だけが達成済み、という境界の上に立つ。この判定を `git worktree list --porcelain` の出力と `ws.path` の**文字列比較**で行うと成立しない。実測(git 2.55、macOS):

- 渡したパス `/var/folders/.../tmp.RF47.../wt/t1` に対し、git の出力は `/private/var/folders/.../tmp.RF47.../wt/t1`
- `worktree_root` がシンボリックリンクを含むとき(`home -> realhome`)、渡した `.../home/wt/t2` に対し出力は `.../realhome/wt/t2`

`ws.path` は `WorktreeRoot`(`HOME` 由来)から `WorkspacePlanner::derive` で導出した値なので、macOS の一時ホーム(受け入れ・適合テストのハーネスが必ず使う)や `~` にシンボリックリンクを含む利用者環境では、判定が**常に外れる**。

もう1つ、登録は残っているが実体が消えている状態(git が `prunable gitdir file points to non-existent location` を添えて列挙する)がある。クラッシュや利用者の手動削除で実際に起こる。実測:

- この状態でも `branch refs/heads/pulsen/t1` 付きで列挙される(ブランチだけを見た判定は「達成済み」に倒れる)
- `git worktree add <path> <branch>` は `fatal: ... is a missing but already registered worktree`(exit 128)
- `git worktree add -f <path> <branch>` は exit 0 で張り直す。ブランチ先端は変わらず、積まれたコミットの成果物が worktree に戻る。`git worktree prune` → `add` でも張り直せる

3つめに、**登録がまったく無く、ブランチだけが存在する**状態がある(利用者の `git worktree remove` / `prune`、git の gc による自動 prune、#6 の終端処理でブランチだけが残った後)。`TC-exec-tick-052` が指しているのはこの状態で、実測(git 2.55)では:

- `git worktree list --porcelain` に当該パスのエントリが現れない
- `git worktree add <path> <branch>`(`-f` なし)は exit 0 で成功し、ブランチ先端は変わらず、そのブランチに積まれていたコミットの成果物が worktree に現れる

### Decision

同定の鍵を**物理パス**にし、実体の存在を達成済みの条件に加える。

- 鍵の作り方を1つの private 関数 `physical_key(p) = canonicalize(p.parent()) . join(p.file_name())` に閉じ、`ws.path` と `worktree list --porcelain` が返した各パスの **両方**をこの関数に通してから、鍵同士を比較する。`ws.path` の親(worktree_root)は比較の前に `ensure_dir` する。パス自体を canonicalize しないのは、実体が消えている場合に失敗して比較そのものが成立しないため(親は本メソッドが作るので必ず存在する)。鍵に変換できない git 側のエントリ(親ごと消えている他タスクの登録など)は自タスクのものではないので不一致として扱う。**生のパスの文字列比較は禁じる。**
- **正規化は両側に対称に適用する。** 片側だけを正規化すると Windows で必ず外れる — `std::fs::canonicalize` は拡張長パス(`\\?\C:\...`)を返すのに対し、Git for Windows の `worktree list --porcelain` は `C:/...` 形式を出すため、git の出力をそのまま突き合わせる形にすると鍵が**恒常的に**不一致になる。そうなると既存 worktree を持つタスクは「登録と一致」の分岐に入らず、`ws.path` が実体として存在するので「登録が無く実体がある = `Failed`」の分岐に落ち、毎 tick `record_tool_failure(WorktreeCreate)` を繰り返して上限超過 stopped に至る(冪等成功が主経路にあるため、初回起動以降の全ステータスで壊れる)。両側を同じ関数に通せば、区切り文字も接頭辞も正規化の結果として揃う。
- 達成済みの条件は「鍵一致 + 自ブランチ + **実体が存在** + `prunable` でない」。自タスクの登録(自分のパス + 自分のブランチ)でも、実体が無いか `prunable` が付いていれば `git worktree add -f <path> <branch>` で張り直して `Ok` を返す。実体の有無は `try_exists` で直接観測する — `prunable` の注記は git のバージョンによっては出力されず、注記だけを分岐の鍵にすると実体の消えた登録が「達成済み」に倒れる。
- 登録がまったく無く、ブランチだけが存在する場合は `git worktree add <path> <branch>`(**`-f` なし**)で張り直して `Ok` を返す。先端を変えないので、そのブランチに積まれたコミットの成果物が worktree に戻る。
- `git worktree prune` を使わない理由: prune はリポジトリ全体の stale な登録に効くため、同一リポジトリで動く他タスクの状態にも触れる。`add -f` は対象パス1つに閉じる。
- `-f` の適用範囲をこの分岐に限る理由: `-f` は「別の worktree でチェックアウト済みのブランチ」の保護も外す。鍵が自タスクのパスと一致し、そのエントリが自タスクのブランチを指していることを確認した後だけに使えば、外す保護は「登録は残るが実体が無い」1つに閉じる。

張り直さず `Failed` を返す案を採らない理由: `create` の契約は「`ws.path` に worktree を用意する」であり、達成済みとして `Ok` を返すと `confirm_workspace` → `record_launching` → spawn と進み、ラッパーの `run_agent` が cwd 不在で 126 を書き、リトライのたびに同じ 126 を繰り返して上限超過 stopped に至る。spec は自タスクの残骸に対して冪等な成功を求めているので、張り直せる状態を失敗に落とす理由がない。

### Consequences

- 良い点: 一時ディレクトリ・シンボリックリンクを含むホーム(macOS の既定を含む)でも冪等判定が成立し、`TC-port-worktree-manager-012` と `TC-exec-tick-051`(作成成功 → 保存前クラッシュからの復旧)が環境に依らず通る。クラッシュ後に実体だけが消えた登録(`prunable` の注記が出るか否かを問わない)も、登録ごと消えてブランチだけが残った状態も自己修復する。
- 良い点: 両側対称の正規化なので、Windows 実機検証(#10)を待たずに鍵の一致条件が決まる。片側だけの正規化は、macOS / Linux では緑のまま Windows でだけ壊れるため、#10 まで顕在化しない。
- トレードオフ: `create` の中で `canonicalize` の I/O が、比較するエントリの数だけ増える。
- 復旧の2分岐は**どちらもテストで実行される必要がある**。`TC-port-worktree-manager-013`(ブランチのみ存在)は台帳の字義どおり「登録なし・ブランチのみ存在(コミットが積まれている)」の前提で作り、`-f` なしの張り直しを通す。`prunable` 登録の張り直しは台帳に無い本 ADR 由来の要求なので、ハーネスの別フックによる**追加ケース**として適合スイートに置く(plan.md AC-8)。`TC-013` を prunable 側に寄せると、`TC-010`(ブランチもパスも未使用 → `add -b`)にも該当しない「登録なし・ブランチのみ」分岐がどのケースからも実行されず、実装ごと落ちても全テストが緑になる。
- トレードオフ: ハーネスの前提(`worktree_root` をシンボリックリンク経由にする)が正規化の分岐を通す条件になる。前提の作り方が緩むと、テストは緑のまま実装が退行しうる。

---

## ADR-086: ポートの機構失敗は spec の表記どおり単一の `Io` で報告する

### Status

Proposed

→ `.adr/086-port-mechanism-failure-is-single-io-error.md` に昇格

### Context

既存のポートは操作族ごとのエラー enum(`CreateError` / `SaveError` / `ReadError` / `ArchiveError`)がそれぞれ `Io { message }` を持つ形で、ポートをまたいで共有する不透明なエラー型はまだ無い。本スライスで足す `RunStore`(6メソッド)と `ProcessController::own_identity` は、いずれも「機構そのものの失敗」だけを報告する。

### Decision

spec の表記に寄せ、型名を `Io`(`Failed { message }`)とし、`RunStore` と `ProcessController` で共有する。spec/domains/execution.md のポート表は両ポートの戻り値をどちらも `Result<..., Io>` と書いており、別々の名前を発明すると台帳の語彙と実装の語彙が乖離する。`RunFileError = Corrupt | Io { message }` も spec の表記どおりで、こちらは「不在・破損・機構失敗」を呼び出し側が区別する必要があるため別の型のままにする。

### Consequences

- 良い点: ポート表とコードが同じ名前で読める。#3 が `KillError` を足すときの規則も「呼び出し側が分類に使うなら専用のエラー型、機構失敗だけなら `Io`」と読み取れる。
- トレードオフ: ポートをまたいで共有する最初のエラー型になる。共有してよいのは「呼び出し側が分類に使わない不透明な報告」に限る、という条件を `port.rs` の doc に添える — 条件を書かずに共有すると、次の追加が「とりあえず `Io`」になって分類の必要なエラーまで潰れる。

---

## ADR-087: `RunDirPath::state_root` は `derive` との一致を条件に復元する

### Status

Proposed

### Context

ADR-078 は `RunDirPath::derive` の逆写像をドメインに置き、ラッパーの合成が `--run-dir` の値だけから RunStore を組む形にした。逆写像の受理条件をどこまで緩めるかは決めていない — `<state_root>/runs/<task-id>/attempt-<n>` の各段を読み取るだけの実装では、`attempt-01` や `attempt-+1` も「番号 1」として復元できてしまう。

これらの値は `derive` が出力しない表記であり、tick が記録した run ディレクトリとは別のパスを指す。ラッパーがそのまま受理すると、tick が観測する run ディレクトリとは違う場所に starttime / pid / exit を書き、猶予経路が「pid が現れない」と分類して spawn 失敗を積む(書き込み自体は成功するので、どこにも失敗の証跡が残らない)。

### Decision

読み取った `state_root` / `task_id` / `attempt` 番号で `derive` を呼び直し、**元の値と一致したときだけ** `Some` を返す。一致しない表記(桁揃え・符号つきの番号など)は `None` とし、ラッパーは起動引数の不正として何も書かずに非0で終える(ADR-078 の経路に合流する)。

段ごとの読み取りだけで受理する案を採らない理由: 逆写像が受理する集合が `derive` の像より広くなり、「タスクファイルに記録された run ディレクトリ = ラッパーが書く run ディレクトリ」が構成で保証されなくなる。逆写像であることを `derive` 自身との一致で定義すれば、`derive` のレイアウトを変えても両者がずれない。

### Consequences

- 良い点: 受理条件が `derive` の定義に従属し、レイアウトの知識が `task/path.rs` の1箇所に留まる(ADR-078 の目的そのもの)。
- 良い点: 手で組み立てた run ディレクトリの表記ゆれが、書き込み先の食い違いではなく起動時の拒否として現れる。
- トレードオフ: 復元のたびに `derive` を1回呼ぶ(文字列結合1回。ラッパー起動時に1度だけ通る経路)。

---

## ADR-088: `agent_probe` の引数検査は受け取ったトークンの出力で行う

### Status

Proposed

### Context

`TC-port-process-controller-021` は「シェルのメタ文字や空白を含む引数トークンを与え、受け取った引数がリテラル一致なら 0 を返す検査コマンド」を要求する。ADR-082 はこれを `agent_probe echo-args <期待トークン...>` のモードとして起票した。

しかし、期待トークンを**引数として**渡すと、プローブは「自分が受け取った値」を「自分が受け取った値」と比べることになり、常に一致する。シェルを経由した場合(`*` のグロブ展開・`$VAR` の展開・空白での再分割)に生じる食い違いは、プローブ単体では検出できない。期待をプローブ側にハードコードする案は、同じ列を適合スイートとプローブの2箇所に持つことになる。

### Decision

`echo-args` は**受け取ったトークンを1行ずつ標準出力へ書いて 0 で終了する**。期待との照合は、トークンを渡した側(適合ケース)がリダイレクト先の内容に対して行う。

適合ケースの主張は「`ExitCode(0)` であり、かつ標準出力に現れるトークン列が渡した列と一致する」となり、台帳の期待(`ExitCode(0)`)を含んだうえで、リテラル一致そのものを観測する。

### Consequences

- 良い点: 期待の定義箇所が適合スイートの1箇所に留まり、プローブは「受け取ったものを見せる」だけの無記憶なフィクスチャになる。シェルを経由した実装は、exit code ではなく**内容の不一致**として必ず落ちる。
- トレードオフ: 検査の主体がコマンドから呼び出し側へ移る。台帳の期待文(`ExitCode(0)`)は満たすが、「コマンドが検査する」という字義からは外れる。
- トレードオフ: 改行を含むトークンは行単位の比較で扱えない。検査に使うトークン列(展開・再分割・制御構文・リダイレクト・プレースホルダ)には改行を含めない。

---

## ADR-089: ラッパーの終了コードは「ラッパー自身が責務を果たせたか」を表す

### Status

Proposed

→ `.adr/089-wrapper-exit-code-reports-its-own-duty.md` に昇格

### Context

spec は `wrapper` の出力DTOを「なし(結果はすべてrunディレクトリのファイルとして現れる)」と定め、終了コードを規定しているのは「引数の不正 → 非0」の1行だけである。残る3つの結末 — エージェントを実行した(`Ran`)・マーカーがあり起動しなかった(`Suppressed`)・同定情報を残せず何も書かずに終えた(`Silent`)— の終了コードは spec に無い。

ラッパーはデタッチ起動され stdio は null なので、この値を待つ者は本経路には居ない。しかし `pulsen wrapper` は隠しサブコマンドとして手でも起動でき、`examples/spawn_probe` 経由の適合ケースも `Command::status()` を通す。値を決めないわけにはいかない。

選択肢: (a) エージェントの exit code をそのまま伝播する、(b) つねに 0 で終える、(c) ラッパー自身が責務を果たせたかを表す。

### Decision

(c) を採る。`Ran` / `Suppressed` は 0、`Silent` は非0(`WrapperError::NothingRecorded`)とする。引数の不正も非0で、これは spec の1行と一致する。

(a) を採らない理由: エージェントの結末は `exit` ファイルが持つ唯一の記録であり、終了コードにも載せると「どちらを読むべきか」が2箇所に分かれる。exit を書けなかったとき(`Ran` かつ `write_exit` 失敗)にラッパーの終了コードだけが結末を持つことになり、「exit なし = プロセス死亡として分類する」という tick 側の規則と食い違う。(b) を採らない理由: `Silent` は「同定情報をどこにも残せなかった」状態で、手で起動したときに成功と区別できないと調査の手がかりが消える。

`Suppressed` を 0 にするのは spec の「エージェントを起動せず**正常終了**する」に従う。

### Consequences

- 良い点: 終了コードの意味が「ラッパーが自分の仕事を終えたか」に一意化され、エージェントの結末は `exit` ファイルだけが持つ。
- 良い点: 手で起動したときの `Silent` が非0として見え、原因が標準エラー出力に出る(本経路では stdio が null なので誰も読まないが、副作用も無い)。
- トレードオフ: spec に無い規定を1つ増やす。spec 追従の提起を Issue のコメントに残す。

---

## ADR-090: テンプレートと定義の不備の説明はドメインの `describe` に1箇所だけ置く

### Status

Proposed

→ `.adr/090-persisted-explanations-come-from-domain-describe.md` に昇格

### Context

手続きAの展開失敗は `record_spawn_failure_in_place(message, ...)` で**タスクファイルに残る**。この `message` はポートが返す不透明な文字列ではなく、ドメインのエラー型(`AgentDefError` / `TemplateError` / `ExpansionError`)から組み立てるしかない。

一方 `cli/render.rs` には同じ3型を文言に落とす private 関数(`agent_def_error` / `template_error`)が既にあった。ユースケース側で別の文言を組むと、同じ不備が「登録時の案内」と「タスクファイルの失敗要因」で違う言葉になる。ADR-081 の「文言は CLI 層で組み立てる」は tick のサマリー(`errors`)に対する規約であって、帳簿に永続化される値には適用できない — 帳簿は CLI を経由せずに読まれる。

### Decision

`TemplateError::describe` / `AgentDefError::describe` / `ExpansionError::describe` をドメインに足し、`cli/render.rs` の private 関数を削除して委譲させる。既存の `NameError` / `BranchNameError` / `AbsolutePathError` / `TaskIdError` / `AttemptNumberError` が「説明の定義箇所をドメインに1つ置く」という doc とともに持っているのと同じ形にする。

ユースケース(`tick/launch.rs`)は、5経路それぞれの文脈(どのステータスか・どのエージェント名か)を添えたうえで `describe()` を埋め込む。文脈の付与は報告側の責務であり、`InconsistentRunFiles` に run_dir を持たせない判断と対称になる。

### Consequences

- 良い点: 同じ不備が登録時の案内でも失敗要因の記録でも同じ言葉で読める。
- 良い点: 帳簿に残る `message` の定義箇所が「ドメインの説明 + ユースケースの文脈」の2段に固定され、層をまたいだ重複が消える。
- トレードオフ: ドメインに表示用の文字列が1組増える。既存の `describe` 群と同じ位置づけなので新しい種類の責務ではない。

---

## ADR-091: tick の分岐は判断を値(`Branch`)にしてからタスクを手続きへ渡す

### Status

Proposed

→ `.adr/091-tick-branch-decision-as-value.md` に昇格

### Context

ADR-073 は tick の分岐を全実行状態の網羅 `match` として書くと決めた。素直に `match task.execution() { ... }` と書くと、アームの中でタスクを消費する手続き(`launch` / `confirm_spawn` はいずれも `self` を消費する遷移関数を呼ぶ)へ渡せない — 走査対象の借用が `match` 全体で生きるため。

回避策は3つある: (a) 手続き側で実行状態を再度 `match` する、(b) `clone()` して渡す、(c) 判断だけを所有する値に落としてから分岐する。

### Decision

(c) を採る。`Branch`(`Launch { input }` / `Wait` / `Cleanup` / `ConfirmSpawn { recorded_at }` / `Observe` / `Advance` / `Notify`)を private な列挙として定義し、`branch_of(&task) -> Branch` が実行状態と動作種別の網羅 `match` を1箇所で担う。手続きは判断済みの付随データ(エージェント入力・猶予の起点)を引数で受け取る。

これにより、手続きの中に「ここには来ないはずの実行状態」のアームが現れない。残る `expect` は手続きAの2箇所(確定済みワークスペースの取り出し・エージェント実行ステータスのリトライ上限)だけで、どちらも分岐が構成として保証する不変条件であり、CLAUDE.md の「パニックは不変条件違反にのみ使う」に収まる。

(a) を採らない理由: 同じ場合分けが2箇所に分かれ、どちらかにアームを足し忘れても網羅検査が通る。(b) を採らない理由: タスクは走査で全件をメモリに載せる値であり、分岐のためだけに複製する理由がない。

### Consequences

- 良い点: 未配線のアーム(#3 / #6 の引き取り先)が1箇所に並び、埋める作業が「アームを1つ書き換える」に閉じる。
- 良い点: 手続き側が実行状態を見ないので、前提の再検査はドメインの遷移関数だけが持つ。
- トレードオフ: 分岐の付随データ(`AgentInput`)を1回複製する。ステータス定義から取り出す小さな値であり、起動経路でしか通らない。

---

## ADR-092: 記録すべきことが1つも起きなかった tick を「処理対象なし」と表示する

### Status

Proposed

→ `.adr/092-empty-summary-means-nothing-to-process.md` に昇格

### Context

pages は「処理対象がなければその旨を表示して 0」と定めるが、「処理対象」がタスク0件を指すのか、何のアクションも起きなかったことを指すのかは書かれていない。走査は待機ステータスのタスクや猶予内の launching タスクを列挙するが、それらには書き込みも報告も発生しない。

一方で、タスクファイルを書き換える経路(起動・起動確認・spawn 失敗の起動待ち復帰・worktree 作成失敗の記録・展開失敗の記録・凍結)は「アクションが起きた tick」そのものであり、これを「処理対象なし」と表示するのは spec からの逸脱になる。cron 運用ではこの出力が唯一の窓で、状態が変わったことが利用者に一切見えなくなる。

### Decision

サマリーの全フィールドが空であることをもって「処理対象なし」とする(`TickSummary::is_empty`)。タスク0件・`state/tasks/` 未作成・待機のみ・猶予内の待機は、いずれも同じ表示になる。

あわせて、**タスクファイルへの書き込みを行った経路は必ずサマリーのいずれかのフィールドを埋める**ことを不変とする。起動確認は `confirmed_running`、凍結は `frozen`、記録した失敗(worktree 作成・テンプレート展開・spawn 失敗の起動待ち復帰)は `errors` が受け取る(ADR-094)。保存できなかった場合も `errors` に `SaveFailed` が載るため、「書き込みを試みた tick」は常に「処理対象なし」から外れる。

タスクの件数を数えて分ける案を採らない理由: 「10件走査して何も起きなかった」と「0件だった」を書き分けても、利用者が次に取る行動は変わらない。サマリーは spec 上「実行したアクション」の一覧であり、アクションが無いことの表示に走査件数は要らない。

### Consequences

- 良い点: 表示の条件がサマリーの値だけで決まり、走査件数という別の状態を持たずに済む。
- 良い点: 「書き込んだのに無報告」という結末が構成として消える。新しい書き込み経路を足すスライスは、集計先のフィールドを決めない限り不変を満たせない。
- トレードオフ: #3 / #6 がアームを埋めると「何も起きなかった」の範囲が狭まる。判定条件はサマリーに従属するので、追加のフィールドが埋まれば自動的に追随する。

---

## ADR-093: テストのフィクスチャは「ダブルに対するユースケース」と「実バイナリの受け入れ」で分ける

### Status

Proposed

→ `.adr/093-usecase-and-acceptance-fixtures-are-separated.md` に昇格

### Context

tick のテストは2種類ある — ポートをテストダブルに差し替えるユースケーステスト(`tick_scan` / `tick_launch` / `tick_confirm_spawn`)と、実バイナリ・実ファイルシステム・実プロセスを使う受け入れテスト(`cli_tick`)。前者は `Task` / `WorkflowSnapshot` / `GlobalConfig` の組み立てを、後者は一時ホーム・git リポジトリ・待ち合わせを必要とする。既存の共有フィクスチャ `tests/common/` は後者のためのもので、tempfile と実 git の起動を含む。

### Decision

ユースケーステスト用のフィクスチャを `tests/tick_fixture/` に分けて置く。受け入れテスト用の `tests/common/` には持ち込まない — 混ぜると、実 I/O を使わないはずのテストが実 I/O のフィクスチャを引き込み、境界の健全性(CLAUDE.md「ドメインとユースケースのテストにファイルシステムや実プロセスが必要になったら、境界を見直す」)が観測できなくなる。

受け入れテスト側で本スライスの CLI(`add` / `tick` / `wrapper`)だけでは作れない状態(失敗確定・スナップショットのみ破損・アーカイブ直置き)は、**`add` が書いた実物のタスクファイルを JSON として読み、必要なキーだけを書き換えて置き直す**(`Home::patch_task`)。直列化形式の DTO をテスト側に組み直さないので、形式を変えてもテストが独自に古い形を作り続けることがない。

### Consequences

- 良い点: ユースケーステストが実 I/O のフィクスチャに依存しないことが、モジュールの分離として見える。
- 良い点: 受け入れテストの前提づくりが `add` の出力に従属し、タスクファイルの形式変更が片側だけに残らない。
- トレードオフ: フィクスチャの置き場が2つになる。用途(分岐網羅 vs 主経路の裏付け)が違うので、共有点は「タスクIDの文字列」程度しかない。

---

## ADR-094: 起動確認だけをサマリーの新しいフィールドにし、記録した失敗は `errors` に載せる

### Status

Proposed

→ `.adr/094-confirmed-running-field-and-recorded-failures-in-errors.md` に昇格

### Context

spec の出力DTO は `launched` / `transitioned` / `skipped_back` / `frozen` / `notified` / `archived` / `errors` / `gc_deleted` / `gc_errors` の9フィールドで、ADR-073 はこれをそのまま持つと決めた。ところが本スライスで配線した書き込み経路のうち、次の4つはどのフィールドにも集計されない。

| 経路 | 書き込む内容 |
|---|---|
| launching → running の取込 | 実行状態と `current_attempt.process` |
| spawn 失敗による起動待ち復帰 | 実行状態・`spawn_fail_count`・失敗要因 |
| worktree 作成失敗の記録 | 実行状態・`attempt_count`・失敗要因 |
| テンプレート展開失敗の記録 | `spawn_fail_count`・失敗要因 |

集計されないまま tick を終えると、サマリーが空になり「処理対象のタスクはありませんでした。」と表示される(ADR-092 の判定)。主経路である起動確認が毎回この表示になるため、放置できない。

### Decision

失敗の記録3つは `errors` に載せる。spec の `errors` は「スキップ・破損・観測失敗・kill失敗等の報告」であり、**記録した失敗の報告**はこの定義に収まる。報告用の分類として `TickIssue::WorktreeCreateFailed` / `CommandExpansionFailed` を足し、猶予経路の spawn 失敗は既存の `TickIssue::SpawnFailed` を使う。報告は**保存できたときにだけ**積む — 保存できなかった tick は状態を変えておらず、そのときの報告は `SaveFailed` 1件が正しい。

launching → running の取込にだけ、フィールド `confirmed_running: Vec<TaskId>` を1つ足す。`transitioned` はタスクステータスの遷移(#3 の `advance`)、`skipped_back` は skipped 判定による起動待ち復帰(#3 の `skip_run`)で語義が確定しており、実行状態の取込をどちらかに混ぜると後続スライスが同じフィールドを別の意味で埋めることになる。

`errors` の形も spec の `{ task_id, path, message }` ではなく分類の列挙として持つ(ADR-081)。文言の組み立ては `cli::render` に置き、`InconsistentRunFiles` も分類だけの値にして完成文言をドメインから外す。

`skipped_back` に相乗りさせる案を採らない理由: spec が「skipped でpending復帰」と明記しており、spawn 失敗による復帰と判定による復帰が同じ行に混ざる。フィールドを足さずに `errors` だけで起動確認を報告する案を採らない理由: 起動確認は失敗でもスキップでもない正常な前進で、`errors` に出すと運用者が異常として読む。

### Consequences

- 良い点: 書き込みを行った tick が必ずサマリーに現れ、ADR-092 の不変が構成として成立する。
- 良い点: 後続スライスは `transitioned` / `skipped_back` を spec の語義のまま使える。
- トレードオフ: 出力DTO が spec から1フィールド分ずれる。`spec/usecases/execution.md` への反映を progress.md の「spec 追従の提起」に積む。
- トレードオフ: 1つのタスクが1回の tick で `errors` と `frozen` の両方に現れうる(上限超過で凍結した失敗)。失敗の原因と結末は別の情報なので、どちらか一方に畳まない。

---

## ADR-095: ツール操作の失敗を記録する遷移は、専用の失敗種別だけを受け取る

### Status

Proposed

→ `.adr/095-record-tool-failure-takes-tool-failure-kind.md` に昇格

### Context

`spec/domains/task.md` の遷移表は `record_tool_failure` の引数を `kind` とだけ書き、実装は `FailureKind`(`WorktreeCreate | WorktreeRemove | ArchiveMove | SpawnFail | JudgeFail`)をそのまま受け取っていた。しかし後ろの2値は専用の遷移(`record_spawn_failure` / `record_spawn_failure_in_place` / #3 の `record_judge_failure`)が `spawn_fail_count` / `judge_attempt_count` を進めながら記録するものである。

`record_tool_failure(FailureKind::SpawnFail, ..)` は型として書けてしまい、書けた場合は `attempt_count` を進めつつ `SpawnFail` の失敗要因を残す — カウンタと失敗種別が食い違った帳簿が生まれ、`applicable_retry_limit` の併記(ADR-014)が実態と合わなくなる。

### Decision

`ToolFailureKind`(`WorktreeCreate | WorktreeRemove | ArchiveMove`)を新設し、`record_tool_failure` の引数をこれに絞る。帳簿に残す `FailureKind` への写像はドメイン内部(`ToolFailureKind::recorded`、`pub(super)`)に閉じ、呼び出し側は写像を知らない。永続化される値としての `FailureKind` は5値のまま変えない — タスクファイルの直列化形式と `FailureNote::kind()` の読み取り口は spec のとおり。

spec の表記から逸脱するが、CLAUDE.md の「不正な状態を型で表現不能にする」を優先する。spec 側は「`kind` はツール操作の3種に限る」の追従提起として残す。

### Consequences

- 良い点: カウンタと失敗種別が食い違う帳簿が型として書けなくなる。#6 の手続きB(`WorktreeRemove` / `ArchiveMove`)も同じ入口を使う。
- トレードオフ: 失敗種別の型が2つになる(永続化される `FailureKind` と、遷移の入口の `ToolFailureKind`)。写像はドメイン内部の1関数に閉じており、外からは見えない。
- トレードオフ: spec/domains/task.md の遷移表と引数型が一致しない。spec 追従の提起に積む。

---

## ADR-096: `TransitionError` は分類だけを持ち、文言は `cli::render` が組み立てる

### Status

Proposed

→ `.adr/096-transition-error-holds-classification-only.md` に昇格

### Context

`TransitionError::InvalidState` の `expected` には `"pending | failed"` という表示用の文字列が入り、`InvariantViolated { message: String }` は日本語の完成文をドメインが作っていた。ADR-081 は同じ tick の報告について「文言は CLI 層で組み立てる」と決め、`TickIssue` / `InconsistentRunFiles` からは完成文言を外している。

`TemplateError::describe` 系(ADR-090)は「タスクファイルに失敗要因として永続化される」という why が付くので別扱いだが、`TransitionError` は永続化されず表示にしか使われないため、その why が効かない。

選択肢:

1. 検査をドメインに残し、分類だけを持たせて文言を `cli::render` に寄せる
2. 前提の検査を呼び出し側に移し、`TransitionError` から該当の変種を落とす

### Decision

1 を採る。前提の検査はドメインに残す — 遷移関数を全域に保つことは「不正な状態を型で表現不能にする」の一部で、呼び出し側の規律に依存させると手動修復で破れた帳簿がドメインを素通りする。

そのうえで完成文言をドメインから落とす。`InvalidState.expected` は `&'static [ExecutionStateKind]`(前提として受理される実行状態そのもの)にし、`InvariantViolated { message: String }` は破れの種別を表す `MissingCurrentAttempt` に置き換える。`cli::render` が `ExecutionStateKind::as_str()` を連結して `"pending | failed"` を組み立て、`MissingCurrentAttempt` の説明文も持つ。

2 を採らない理由: 前提の検査が呼び出し側に散り、手続きが増えるたびに検査の書き漏れが起こりうる。

### Consequences

- 良い点: `TickIssue` / `InconsistentRunFiles` と同じ規則(表示専用のエラーは分類だけを持つ)がドメインのエラー型にも通る。表示の変更がドメインに触れずに済む。
- 良い点: `expected` がデータになり、前提の実行状態を足しても文言が自動で追随する。
- トレードオフ: spec/domains/task.md のエラー型定義(`expected: &'static str` / `InvariantViolated { message: String }`)と一致しない。spec 追従の提起に積む。
- `TransitionError::MissingCurrentAttempt` と `TickIssue::MissingCurrentAttempt` は同じ事実の別文脈での報告になる(手続きCは観測前に検出し、遷移関数は前提として検査する)。どちらも分類なので、文言の重複は `cli::render` の中だけに閉じる。

---

## ADR-097: 凍結の判定は保存後の状態から導出せず、遷移を呼んだ側から渡す

### Status

Proposed

→ `.adr/097-freeze-is-passed-by-the-caller-of-the-transition.md` に昇格

### Context

`commit` は「保存した結果の実行状態が `Stopped` であること」で `frozen` への集計を決めていた。`frozen` の語義は spec の出力DTO のとおり「このtickで凍結した」である。

ADR-074 は「#3 は共通手続き notify の呼び出しをこの関数の中に足すだけでよい」と設計しているが、共通手続き notify のステップ4 は `mark_notified(now)` → `save` であり、保存対象は `Stopped { notified_at: Some(..) }` のままである。この保存が `commit` を通ると、何tick も前に凍結したタスクが通知に成功したtickで毎回 `frozen` に載る。spec 手順7(`Stopped` アーム)の catch-up 通知も同じ経路を通る。

### Decision

`commit(&task, freeze, summary)` の形にし、凍結かどうかを呼び出し側から渡す。`Freeze::of_recorded_failure(&task)` は「遷移前が凍結ではない」という前提(記録系3遷移は起動待ち・失敗確定・起動記録済みしか受け付けない)のもとでのみ遷移後の `Stopped` を凍結と同値に扱い、その前提を持たない保存経路は `Freeze::NotFrozen` を渡す。

`commit` は保存と集計の単一地点のまま保つ — #3 の notify も引き続きこの関数の中に足せる(`Freeze::Frozen` の枝で呼ぶ)。catch-up 通知は `Branch::Notify` の保存として `Freeze::NotFrozen` を通り、`frozen` を汚さない。

### Consequences

- 良い点: 凍結が「状態の観測」ではなく「遷移の結果」として表現され、#3 が catch-up 通知を足しても誤集計が起きない。
- 良い点: 保存経路を足す側が凍結の意味を明示せざるを得ないので、判断が暗黙の導出に戻らない。
- トレードオフ: `commit` の引数が1つ増える。凍結しうる保存は3経路しかなく、いずれも遷移の直後にあるため判定の重複はない。

---

## ADR-098: 猶予超過の spawn 失敗を独立した分類にし、記録した失敗をスキップと別の見出しで表示する

### Status

Proposed

→ `.adr/098-spawn-not-observed-classification-and-error-headings.md` に昇格

### Context

`TickIssue::SpawnFailed` は結末の異なる2つの経路を兼ねていた — (a) `spawn_wrapper` の同期エラー(状態不変・`Launching` 維持・カウンタ非消費・猶予経路が後で分類する)と、(b) 猶予超過の再確認で確定した spawn 失敗(`Pending` 復帰・`spawn_fail_count` 加算・上限超過なら凍結)。運用上は「まだ起動中かもしれない」と「起動をあきらめてカウンタを1つ消費した」で次に取る行動が違い、テストが分類だけを主張しても取り違えを検出できない。

同じ問題が表示にもある。`cli::render` は `errors` をすべて「スキップ」の見出しに束ねていたが、`WorktreeCreateFailed` / `CommandExpansionFailed` / 猶予経路の spawn 失敗はタスクファイルに書き込んだ後の報告で、カウンタを消費し上限を超えれば同じtickで凍結する。ADR-092 は「cron 運用ではこの出力が唯一の窓」を根拠に「処理対象なし」の判定を設計しており、その窓が記録した失敗と素通しのスキップを同じ語で束ねていた。

### Decision

猶予超過で確定した spawn 失敗を `TickIssue::SpawnNotObserved` として分ける。`TickIssue::SpawnFailed` は同期エラー専用にする。ADR-094 は記録した失敗を `errors` に載せることを決めただけで、1つの変種に畳むことまでは根拠づけていない。

表示では `errors` を「タスクファイルに何を残したか」で3つの見出しに分ける。`WorktreeCreateFailed` / `CommandExpansionFailed` / `SpawnNotObserved` は失敗を記録しカウンタを消費するので「失敗を記録(N件)」。`PrepareAttemptFailed` / `SpawnFailed` は launching の記録を保存した**後**の報告で、状態は `Launching` のまま次の tick が猶予経路で分類するので「起動の結果が未確定(N件)」。残りはタスクファイルへの書き込みが無く次の tick がそのまま再試行するので「スキップ(N件)」。判別は `cli::render` の網羅 `match` に置き、分類が増えたときに振り分け先を決めないと通らないようにする。

「起動の結果が未確定」を独立させるのは、`PrepareAttemptFailed` の後も spawn を続ける(手続きAの順序)ため、同一タスクが `launched` と `errors` の両方に載りうるからである。この2つを「スキップ」に束ねると、書き込んで attempt 番号も消費した tick が「何も起きなかった」と読める。ADR-092 が「書き込んだ tick が処理対象なしと表示される」ことを構成として潰したのと同じ理由で、見出しの語義も書き込みの有無と食い違わせない。

### Consequences

- 良い点: 分類だけで「カウンタを消費したか」が読め、同期エラー側で `record_spawn_failure` を呼ぶような取り違えをテストが検出する。
- 良い点: 唯一の出力窓で「記録された失敗」と「次tickでそのまま再試行されるスキップ」が読み分けられる。
- 良い点: 同一タスクが「起動」と「起動の結果が未確定」に並んでも、運用者は「起動は試みたが結末は次の tick が決める」と読める。「起動」と「スキップ」の同時掲示のような矛盾した読み方が生まれない。
- トレードオフ: spec の `errors` の分類が1つ増える。spec 追従の提起に積む。
- トレードオフ: 見出しが3つになる。どれも空なら見出しごと出さないので、通常運用の行数は変わらない。

---

## ADR-099: `tick` が使わないカレントディレクトリと乱数の解決を `compose` から切り出す

### Status

Proposed

→ `.adr/099-compose-resolves-only-home-and-config.md` に昇格

### Context

`compose()` は `add` と `tick` が共有する合成ルートで、ホームの解決とグローバル設定の読み込み(pages ※1 の全コマンド共通)のほかに、`env::current_dir()` と `DefaultTaskIdGenerator::new`(`getrandom` による初期化)を無条件に行っていた。前者は `FsWorkflowStore` の `base_dir` — `--workflow` に与えた相対パスの解決基準にしか使われず、後者はタスクIDの発行にしか使われない。どちらも `tick` の動作には要らない資源だが、失敗すれば `WireError::CurrentDirUnavailable` / `WireError::IdGenerator` として `tick` が非0で終わる。

pages「縮退状態の共通規則1」(各コマンドは自身の動作に必要なリソースだけを検証する)を根拠に `current_exe()` を `compose()` から `wire::process_controller()` へ切り出したのが ADR-076 で、同じ規則をラッパーの中でもう一段適用したのが `without_self_exe` の判断だった。同じ規則が同じ合成ルートの中で `current_dir` と乱数にだけ適用されていない。AC-12 は「tick は任意の作業ディレクトリから起動しても同じ結果になる」を掲げており、帳簿がすべて絶対パスで閉じているのに結線だけが作業ディレクトリを要求する形は、規則の非対称であると同時に AC-12 の主張を結線の側から裏切る。

### Decision

ADR-076 と同じ形で切り出す。`compose()` に残すのはホームの解決とグローバル設定の読み込みだけにする。

- `Runtime` は `FsWorkflowStore` ではなく `workflows_dir: PathBuf` を持ち、`Runtime::workflow_store() -> Result<FsWorkflowStore, WireError>` が呼ばれた時点でカレントディレクトリを読む。ワークフローの置き場は解決済みのホームから決まるので、`Runtime` のメソッドにする。
- `wire::id_generator() -> Result<DefaultTaskIdGenerator<SystemClock>, WireError>` は `process_controller()` と同じ自由関数にする。ID の発行はホームから何も受け取らない。
- `cli/add.rs` が両方を呼ぶ。`tick` / `wrapper` の経路からは消える。

`WireError` の2変種と `cli::render` の文言はそのまま残す — `add` から到達する失敗であることは変わらない。

`Runtime` に `OnceCell` を持たせて遅延構築を隠す案を採らない理由: 内部可変性が入る。構築が要るのは `add` の1経路で高々1回、`compose` の呼び出し元は各コマンドの `execute` 1箇所しかないので、呼び名で構築だと分かる形のほうが読める。

`add` 用と `tick` 用に `Runtime` を型で分ける案を採らない理由: 共有する項目(config・state_root・worktrees・clock・tasks・lock)が大半で、コマンドが増えるたびに組み合わせの型が増える。ラッパーだけを `WrapperRuntime` として分けたのは、ホームも config も読まないという**構成そのものの違い**があるから(ADR-078)で、ここにはその違いが無い。

### Consequences

- 良い点: `tick` の非0終了は `tick` が実際に使う資源の失敗だけに閉じる。AC-12 が結線の形としても読めるようになり、規則の適用が `current_exe` / `current_dir` / 乱数で揃う。
- 良い点: `compose()` に残るのがホームと config だけになり、「全コマンド共通の起動時処理」という宣言と中身が一致する。
- トレードオフ: `Runtime` のメソッドに、参照を返すアクセサと構築して `Result` を返すものが混ざる。`workflow_store` / `id_generator` という呼び名を `process_controller` に揃えて、構築であることを名前で示す。
- トレードオフ: `add` の失敗の検出点が `compose` より1段後ろへ動く。いずれもタスクを作る前で状態は変わらない(pages 規則4)ので、利用者から見える挙動は文言も終了コードも変わらない。
