# 076: `ProcessController` は自バイナリのパスと同定情報の取得元を構築時に注入される

## ステータス

承認済み

## コンテキスト

requirements §4.1 はラッパーを「ツール自身のバイナリをラッパーモードで再実行する方式」と定める。アダプターが `std::env::current_exe()` を直接読むと、適合テスト（テストハーネスのバイナリが実行主体）が**テストバイナリ自身**をラッパーとして再実行してしまい、`spawn_wrapper` の適合ケースが成立しない。加えて「ラッパーの起動自体が不可能」な状況を再現する手段が無くなる。

同定情報の取得も同じ形をしている。「取得機構が失敗したとき `Err(Io)` を返し、不正な値で `Ok` を装わない」ケースは、実行中の自プロセスに対する `ps` / procfs の読み取りを外から壊せない限り再現できない。権限操作で作る案は root 実行やファイルシステムの種類に依存し、確定的に走らせられない。

## 決定

`SystemProcessController::new(self_exe: PathBuf, identity_source: IdentitySource, clock: SystemClock)` とし、実行ファイルのパスと**同定情報の取得元**を構築時に注入する。`std::env::current_exe()` を読むのは合成ルート（`cli::wire`）の1箇所だけにする — ADR-024 が `git_program` を、ADR-031 が環境の読み取りを合成ルートに閉じたのと同型。

`IdentitySource` は `PathBuf` の newtype で、`IdentitySource::platform_default()` が POSIX 非 Linux では `ps` の実行ファイルパス、Linux では procfs のルート（`/proc`）、Windows では powershell の実行ファイルパスを返す。プラットフォーム分岐は `adapter/process.rs` の中に閉じるので、合成ルートは `platform_default()` を呼ぶだけで `#[cfg]` を持たない。

ただし読み取りを `compose()` には**載せない**。`compose()` は全コマンドが通る合成ルートで、`ProcessController` を必要とするのは `tick` と `wrapper` だけである。`WireError::SelfExeUnavailable` を `compose()` に置くと、プロセス起動と無関係なコマンドが `current_exe()` の失敗で落ちる。pages の縮退状態の共通規則1（各コマンドは自身の動作に必要なリソースだけを検証する）に従い、`wire::process_controller() -> Result<SystemProcessController, WireError>` として切り出し、`cli/tick.rs` と `cli/wrapper.rs` から呼ぶ。

- 適合テストのハーネスは `env!("CARGO_BIN_EXE_pulsen")` を `self_exe` として渡す
- `SpawnError` の再現は、**存在しないパスを `self_exe` として構築した2つ目のコントローラ**をハーネスの `failing_controller` フックが返す形にする（ADR-024 / ADR-027 の `failing_manager` と同型）。本番のインスタンスはイミュータブルなまま、権限操作にも root 実行の可否にも依存しない
- 取得機構失敗の再現は同型で、**存在しないパスを `identity_source` として構築した2つ目のコントローラ**を `failing_identity_controller` フックが返す。ADR-075 の写像表により、どのプラットフォームでもこの構成は `Err(Io)` に落ちる

**ラッパーの合成だけは `self_exe` を持たない構成を使う**

`cli/wrapper.rs` は `SystemProcessController::without_self_exe(identity_source, clock)` を組む。ラッパーが呼ぶのは `own_identity` と `run_agent` だけで、`spawn_wrapper` は呼ばない。それでも `current_exe()` を解決させると、その失敗**だけ**でラッパーが何も書かずに非0終了し、tick の猶予経路が spawn 失敗として積む — 使わないリソースの検証が失敗経路を1本増やす形になる。`compose()` から `process_controller()` を切り出したのと同じ規則を、コマンドの中でもう一段適用する。

この構成の `spawn_wrapper` は構造上必ず `SpawnError` を返す。適合契約の対象は `new(...)` の構成に限ると宣言し、`without_self_exe` の doc にその範囲を書く。

## 検討した代替案

- **`spawn_wrapper` を持たない狭いポート（`own_identity` + `run_agent`）を切り出して型で閉じる** — ポートの分割は kill 系メソッドの配置と一緒に決めるべきもので、先に分けると二度分けることになる
- **権限操作で取得機構を壊す** — root 実行やファイルシステムの種類に依存し、確定的に走らせられない

## 影響

- 適合スイートが実バイナリのラッパー動作を検証でき、`spawn_wrapper` のケースが「再現できるアダプター環境に限る」のスキップに落ちない。`current_exe()` の失敗経路がプロセス起動を行うコマンドだけに閉じる。取得機構失敗のケースも権限操作にも root の可否にも依存せず走る
- この注入点は、ADR-075 の三値化（不在 / 機構失敗の区別）が正しく実装されたことを照合側（`starttime_of`）に対して検証するときの足場にもなる
- トレードオフ: 合成ルートで `current_exe()` が失敗したときの経路が増える（`WireError` に1変種）。tick はそれを実行環境エラーとして非0で終え、wrapper は何も書かずに非0で終える
- トレードオフ: 本番の構築が引数3つになり、`IdentitySource::platform_default()` という「既定値を返す関数」が1つ増える。既定を型の中に置くことで、合成ルートがプラットフォームごとの取得元を知らずに済む形を選んだ
- トレードオフ: 本番に結線されるポート実装が2構成になり、うちラッパー側は `spawn_wrapper` が必ず失敗する。後続スライスがラッパーの経路で `spawn_wrapper` を使っても型は止めず、実行時エラーになる。この構成は適合契約の対象外であることを doc の宣言で担保している
