# ADR: Windows のデタッチ起動は自プロセスの標準ハンドルの継承を止めてから行う

## ステータス

承認済み

## コンテキスト

`spawn_wrapper` は stdin / stdout / stderr を `Stdio::null()` にし、`CREATE_NEW_PROCESS_GROUP | DETACHED_PROCESS` を付けてラッパーを起動する（`.adr/2-process-controller-without-unsafe.md`）。これで呼び出し側の標準入出力は引き継がれない、という前提で契約（呼び出し側プロセスの終了がラッパーの完走を妨げない）を組んでいた。

前提が成り立っていない。std の `Command::spawn` は Windows で `CreateProcessW` を `bInheritHandles = TRUE` 固定で呼ぶ。この値が `TRUE` のとき子へ渡るのは `STARTUPINFO` に載せたハンドルだけではなく、**継承可能フラグ（`HANDLE_FLAG_INHERIT`）の立った全ハンドル**である。`pulsen tick` 自身が親からパイプで起動されていれば、その書き込み端は tick のハンドル表に継承可能なまま在り、`Stdio::null()` を渡してもラッパーへ、さらにラッパーが起動するエージェントへ複製される。

結末は、呼び出し側の `read` がエージェントの終了まで EOF に到達しないことである。`pulsen tick | tee` や、出力を捕るスケジューラーから起動した tick は、デタッチしたはずのエージェントが終わるまで返らない。デタッチ起動という契約が、**呼び出し方によって**崩れる。

Windows CI（`.adr/10-ci-on-github-actions-with-runner-rustup.md`）で `cli_tick` の「滞留するエージェントを起動したままでも次のtickは競合しない」が落ちて表面化した。エージェントの滞留上限（120秒）まで tick が返らず、上限で打ち切られた `exit` が観測される。POSIX では起きない — std が自前で開いた fd に `CLOEXEC` を立てるため、`Stdio::null()` を渡した時点で子へ渡るのは NUL だけになる。

適合スイートはこれを捕まえられなかった。`spawn_from_other_process` が `Stdio::null()` で観測しており、パイプを持つ呼び出し側が居ないためである。

## 決定

Windows のデタッチ起動に限り `unsafe` を許し、起動の瞬間だけ自プロセスの標準ハンドルの継承可能フラグを落とす。

- `adapter::process::inheritance` モジュールを置き、`GetStdHandle` / `GetHandleInformation` / `SetHandleInformation` を `unsafe extern "system"` で宣言する。新規依存は足さない
- `suppress()` は生存期間が「止めている区間」を表す値を返し、`Drop` で元の継承可能性へ戻す。`spawn_wrapper` は `command.spawn()` だけをこの区間で囲む
- 落とすのは自プロセスの標準ハンドル3本だけ。`Stdio::null()` が起動時に開く NUL は区間の外で作られるため影響を受けない
- `unsafe_code` の lint は `pulsen` クレートでのみ workspace の `forbid` を継承せず `deny` にする。`pulsen-domain` / `pulsen-conformance` は `forbid` のまま。許可は `inheritance` モジュールの `#[allow(unsafe_code)]` 1箇所に閉じる
- POSIX 側は同名・同形の `inheritance` モジュールを持ち、区間は何もしない。呼び出し側に `#[cfg]` を出さないため

drop で戻すのは、`Stdio::inherit()` が `STARTUPINFO` に載せるハンドルの継承可能性を前提にするためである。落としたまま放置すると、以後に標準入出力を引き継がせて起動する経路が壊れる。

## 検討した代替案

- **`PROC_THREAD_ATTRIBUTE_HANDLE_LIST` で継承するハンドルを列挙する** — 正確だが `STARTUPINFOEX` の組み立てが要り、std の `Command` には載せる口が無い（`raw_attribute` は unstable）。`CreateProcessW` 自体を自前で呼ぶことになり、`unsafe` の面積が桁違いに増える
- **`cmd.exe /C start` を経由して起動する** — `unsafe` は避けられるが、エージェントのトークンが cmd の解釈を通る。`&` `|` `^` `%` `"` を含むトークンをそのまま渡すという契約（シェル非経由）と両立しない
- **テスト側でパイプを使わないようにする** — CI は緑になるが、欠陥は製品に残る。落ちたのはテストの都合ではなく契約の破れである
- **Windows の制約として受け入れ、記録に留める** — 「呼び出し側が出力を捕っていなければデタッチが効く」は利用者が事前に知りえない条件で、破れたときの結末（tick が返らない）が静かすぎる

## 影響

- Windows でも、呼び出し側が出力をどう扱うかに関係なく `spawn_wrapper` が即座に返る。`cli_tick` の当該ケースが3 OS で同じ主張のまま通る
- トレードオフ: `unsafe` ゼロを維持できなくなった。`pulsen` クレートの lint が `deny` に緩み、`forbid` による構造的な保証は `pulsen-domain` / `pulsen-conformance` に限られる。`unsafe` の実在箇所は `inheritance` モジュールだけなので、`grep -rn 'unsafe' crates/pulsen/src` で全件を確認できる
- トレードオフ: 区間の間だけプロセス全体の状態（標準ハンドルの継承可能性）を変える。tick は単一スレッドで起動を行うので競合しないが、将来スレッドから並行して起動する場合はこの前提が要る
- `.adr/2-process-controller-without-unsafe.md` の「`unsafe` は使わない」は Windows のデタッチ起動についてのみ本 ADR が置き換える。同定情報の取得・`run_agent`・`KillIdent` の各判断は変わらない
- 適合スイートは依然としてこの破れを捕まえられない。`spawn_wrapper` の適合契約に「呼び出し側がパイプで出力を捕っていても起動は即座に返る」を足すかは、`spawn_from_other_process` のフィクスチャがハングで失敗する形になるため別途判断する
