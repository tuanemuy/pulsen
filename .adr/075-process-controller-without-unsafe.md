# 075: `ProcessController` のプラットフォーム実装は `unsafe` なしで組み、同定情報は単一の観測関数に閉じる

## ステータス

承認済み

## コンテキスト

workspace lints の `unsafe_code = "forbid"` を全クレートが継承しており、本番依存は6クレートに閉じている（ADR-023）。`libc` / `nix` / `rustix` / `sysinfo` はいずれも入っていない。一方 requirements §4.3 は、デタッチ起動・プロセス起動時刻の取得・kill 同定子の記録を OS 依存操作として要求する。

起動時刻（`ProcessStartTime`）は不透明値の**等価比較専用**（`Ord` を持たない）なので、記録側（`own_identity`）と照合側（`starttime_of`）が**同じプロセスに対して必ず同じ文字列を得る**ことが契約の実質になる。関数を1つにするだけでは足りない — 同じ関数でも実行環境が違えば違う表現を返す。実測（macOS の `ps -o lstart=`）:

```
(既定, ja_JP)   水  8/12 20:14:45 2026
LC_ALL=C        Wed Aug 12 20:14:45 2026
LC_ALL=C TZ=UTC Wed Aug 12 11:14:45 2026
```

ラッパーは tick から環境を継承して起動される。cron の tick（`LANG` 未設定）が spawn したラッパーが記録し、対話シェル（`ja_JP.UTF-8`）の tick が照合すると不一致になる。結末は `IdentityCheck = Dead` → `DiedWithoutExit` → 再起動 → **エージェント実行中の同一 worktree での並走**で、精度ではなく表現の非決定性から最悪の失敗モードに到達する。

## 決定

std の安全 API と外部コマンドの起動だけで組み、`unsafe_code = "forbid"` を維持する。

**デタッチ起動（`spawn_wrapper`）**

- POSIX: `std::os::unix::process::CommandExt::process_group(0)` で新しいプロセスグループの長にする。`setsid(1)` には依存しない — macOS に同名の実行ファイルが無い
- Windows: `std::os::windows::process::CommandExt::creation_flags` に `CREATE_NEW_PROCESS_GROUP | DETACHED_PROCESS` を渡す
- 共通: stdin / stdout / stderr を `Stdio::null()` にし、`spawn` の戻り値を待たない（`Child` を drop する）

**kill 同定子（`KillIdent`）**

- POSIX: プロセスグループIDを**観測して** `-<pgid>` の文字列とする（`kill(1)` にそのまま渡せる負の PGID 表記）
- Windows: プロセスグループIDに相当する値として自身の PID の文字列とする

`process_group(0)` の効果から `pgid == pid` を**仮定して** `-<pid>` を書く案は採らない。`own_identity` は `spawn_wrapper` 経由の起動以外からも呼ばれる（`pulsen wrapper` は隠しているだけで手でも起動でき、適合ケースはテストプロセス自身に対して呼ぶ）。そのとき `-<pid>` は存在しないプロセスグループか、最悪シェル・cron のプロセスグループを指す。`KillIdent` は pidファイルとタスクファイルに**永続化**され kill がそのまま使う値なので、誤った値が帳簿に残ると無関係なプロセス群を殺す経路になる。適合ケースの期待は「非空」なので、この破れはスイートでは検出されない。

**同定情報の観測**

取得手段を `adapter::process` の1つの private 関数に閉じ、`own_identity` と `starttime_of` が共有する。署名は**三値**とする。

`fn observe_process(&self, pid: Pid) -> Result<Option<ObservedProcess>, Io>`（`ObservedProcess { starttime, kill_ident }`）

- `Ok(Some)` = 取得できた / `Ok(None)` = **対象プロセスが存在しない** / `Err(Io)` = 取得機構そのものの失敗
- PGID を同じ関数から返すのは、POSIX ではどちらも同じ1回の観測（`ps` の1回の起動 / `/proc/<pid>/stat` の1回の読み取り）から取れるため
- `own_identity` は自プロセスを観測するので二値でよく、`Ok(None)`（自分が見つからない = ありえない）を `Err(Io)` に畳む1行を**呼び出し側**に置く。畳むのは呼び出し側であって共有関数ではない

不在を `Err(Io)` に畳んではならない。畳むと「生存プロセスを観測できない tick が状態を変更せずスキップし続け、`DiedWithoutExit` にも `KillOnTimeout` にも到達しない」= running のまま永久滞留になる。実測では不在も**エラーの形**で返る（macOS の `ps -o lstart=,pgid= -p <不在pid>` は exit 1・stdout 空、Linux の `/proc/<pid>/stat` は `NotFound`）ため、二値のままだと畳む実装が自然に書けてしまう。

プラットフォームごとの取得は次のとおり。取得元（POSIX 非 Linux は `ps` の実行ファイルパス、Linux は procfs のルート、Windows は powershell の実行ファイルパス）は構築時に注入する（ADR-076）。

- **Linux**: `<procfs_root>/<pid>/stat`（既定は `/proc`）の起動時刻（clock ticks）を **boot id と合成した `<boot_id>:<ticks>`** とする。ticks は**最後の `)` より後ろを空白で分割した20番目**（全体の22番目）として読む — 2番目のフィールド（comm）は実行ファイル名を `()` で囲んだもので、名前に空白や `)` を含むと素朴な空白分割では位置がずれる（実測: `sleep` を `sl ee) p` という名前で起動すると素朴な22番目は `1` を返す）。この関数は任意 pid を取る `starttime_of` と共有されるため、自プロセス名で顕在化しないことを根拠にできない。boot id は `<procfs_root>/sys/kernel/random/boot_id`
- **その他の POSIX**: `<identity_source> -o lstart=,pgid= -p <pid>`（既定は `/bin/ps`）の1行を、最後の空白で PGID（最終トークン）と `lstart`（残りを trim した文字列）に分ける。起動時に `LC_ALL=C` と `TZ=UTC` を**注入**し、`LANG` / `LC_TIME` / `LC_ALL` の継承を落とす。キーワードの順を逆にすると `lstart` 側に空白パディングが残るので、`lstart=,pgid=` の順で固定する
- **Windows**: `powershell -NoProfile -Command "(Get-CimInstance Win32_Process -Filter \"ProcessId=<pid>\").CreationDate"` 相当。`ToString()` はカルチャ依存なので `.ToUniversalTime().ToString("o")` の**不変形式**を明示する
- `StartTimeRecord.wall` は注入した `Clock` から取る

**既定の取得元は絶対パスで固定する**

環境の固定だけでは足りない。固定したのは実行時の環境であって、**どの実行ファイルが起動されるか**は PATH に委ねられたままになる。cron の tick（`PATH=/usr/bin:/bin`）と対話シェルの tick（`PATH=/opt/homebrew/bin:...`）で `ps` が別実装に解決されれば `lstart` の整形が変わり、結末はロケール非固定の場合とまったく同じ。`git` を PATH 解決している判断（ADR-024）とは性質が違う — git は結果を分類にしか使わないが、`ps` の出力は**帳簿に永続化されて後の tick と突き合わされる**。

- POSIX 非 Linux: 既定を `/bin/ps` とする（`/usr/bin/ps` は macOS に存在しない）
- Linux: `/proc` が元から絶対パスなので影響しない
- Windows: 既定を `powershell` の PATH 解決のまま残す。`%SystemRoot%\System32\WindowsPowerShell\v1.0\powershell.exe` は未検証であり、検証していない絶対パスを固定すると取得そのものが不能になる — PATH 解決より悪い結末になる

**結末の写像**

三値のどこに落ちるかを結末ごとに書き分ける。「中身が空なら `Err(Io)`」だけでは非0終了の場合を規定できず、不在を機構失敗に畳む実装を防げない。

| 結末 | 写像 |
|---|---|
| POSIX 非 Linux: `ps` の起動自体が失敗（実行ファイル不在・実行不能） | `Err(Io)` |
| POSIX 非 Linux: 非0終了 **かつ** stdout が空 | `Ok(None)`（不在） |
| POSIX 非 Linux: それ以外の非0終了 | `Err(Io)` |
| POSIX 非 Linux: exit 0 かつ trim 後が空 / 最終トークンが PGID として読めない | `Err(Io)` |
| Linux: 注入された procfs ルートが存在しない・読めない | `Err(Io)`（機構そのものが無い） |
| Linux: ルートは在るが `<root>/<pid>/stat` が `NotFound` | `Ok(None)`（不在） |
| Linux: その他の読み取り失敗・フィールドが形式外・boot_id が読めない | `Err(Io)` |
| Windows: powershell の起動自体が失敗 | `Err(Io)` |
| Windows: exit 0 かつ出力が空 | `Ok(None)`（不在） |
| Windows: それ以外の失敗・形式外 | `Err(Io)` |

Linux で「ルート不在」と「`<root>/<pid>/stat` の `NotFound`」を分けるのは、取得元が注入される（ADR-076）ため両者が同じ `NotFound` として現れるからである。分けないと、壊れた取得元を注入したときに `Ok(None)` =「そのプロセスは死んでいる」が返り、機構失敗を死亡に写像しない契約が注入で再現できなくなる。

`ProcessStartTime::parse` の `Empty` を握り潰して既定値で `Ok` を装わないことが「不正な同定情報で `Ok` を装わない」の趣旨そのものであり、上表の `Err(Io)` 行はすべてこの趣旨に属する。

**`run_agent`**

- `Command` を直接起動（シェル非経由）。cwd = worktree。stdout / stderr は `File::create` したログへ
- 符号化: cwd に到達できない → 126（エージェントを起動しない） / ログを開けない → 126 / `ErrorKind::NotFound` → 127 / `ErrorKind::PermissionDenied` → 126 / その他の起動失敗 → 126 / 正常終了 → exit code / シグナル死 → `ExitStatusExt::signal()` から 128+n
- cwd の存在確認を起動前に行うのは、`Command::spawn` が cwd 不在でも `NotFound` を返し、コマンド不在の 127 と区別できないため。127 と 126 を両立させる唯一の分岐点になる

## 検討した代替案

- **`libc` を足して `unsafe` を許可する / `pre_exec` を使う** — 依存とアンセーフの両方を増やす。std の安全 API で契約を満たせる
- **Linux で boot 相対の clock ticks をそのまま採る** — starttime は起動からの相対値なので、「再起動 → pid が再利用され、かつ起動 tick 数が一致」が同時に成立すると `IdentityCheck` が別プロセスを `Alive` と誤判定する。確率は低いが、結末は無関係なプロセスグループの kill である。boot id は再起動ごとに新しい UUID になるため、合成した値は boot を跨いで一致しない
- **`/proc/stat` の `btime` を足して絶対時刻に落とす** — ticks を秒に直すには `CLK_TCK` が要り、`unsafe` も新規依存も使わない方針では `getconf` の起動が増える。等価比較しか要求されていない値のために、取得のたびに外部プロセスを起動する理由がない

## 影響

- `unsafe` ゼロ・新規依存ゼロを維持したまま requirements §4.3 の抽象を満たせる。起動時刻の取得手段と取得時の環境が1関数に閉じるので、`starttime_of` が同じ表現を得ることを構成で保証できる。署名が三値まで固定されているため、照合側を足すときに共有関数の署名を変えずに済む
- トレードオフ: 消費者が `own_identity` だけの局面でも三値を扱うことになり、`Ok(None)` を `Err(Io)` に畳む1行が呼び出し側に現れる
- トレードオフ: macOS / Windows では外部プロセス（`ps` / `powershell`）の起動が要る（1回あたり数十ms）。`ps -o lstart=` は秒精度なので、1秒以内の PID 再利用は検出できない（spec は「同一マシン内での等価比較」しか要求していないので契約上は満たす）。macOS の `ps` にはエポック秒を出す keyword が無く、整形済み文字列を固定した環境で読む以外の手段が取れない
- **`process_group(0)` はセッションを分けない**（`setsid` 相当ではない）。適合契約（呼び出し側プロセスの終了後も完走する）は満たすが、ラッパーは同一セッションに残って制御端末を保持し続けるため、端末セッションの強制終了（vhangup 等）では SIGHUP が届きうる — `nohup` 相当の耐性は無い。cron 運用が主経路なので実害は小さい
- **Windows の `KillIdent`（pid 文字列）は POSIX の実行単位に相当しない**。requirements §4.3 は Windows の kill 同定子として「タスクID・attempt番号から決定的に導出する名前付きジョブオブジェクト名」を例示し、「ラッパーのみ死亡しエージェントが実行単位に属したまま生存」した状況で `try_kill_remnants` が `Killed` を返すことを要求する。pid 文字列 + プロセスツリー終了ではラッパー死亡時点で辿る根が無く、この契約を満たせない。`KillIdent` は**永続化される**値なので、後から形式を変えると既存の帳簿が無効になる。`unsafe` 禁止のままジョブオブジェクトを扱う手段が無い以上、kill を実装するスライスで依存追加か lint 緩和の判断が要りうる
- トレードオフ: 既定の取得元の性質がプラットフォームで揃わない（POSIX は絶対パス、Windows は PATH 解決）。requirements §4.3 の「記録と照合は同一の取得手段」が Windows でだけ PATH の安定性に依存したまま残る
- `#[cfg(unix)]` / `#[cfg(windows)]` の出現箇所が `adapter/process.rs` に増える（従来は `util/atomic.rs` のみ）
