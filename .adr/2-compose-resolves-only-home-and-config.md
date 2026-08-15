# ADR: 合成ルートはホームと設定だけを解決し、コマンドが使う資源は呼ばれた時点で構築する

## ステータス

承認済み

## コンテキスト

`compose()` は複数のコマンドが共有する合成ルートで、ホームの解決とグローバル設定の読み込み（全コマンド共通）のほかに、`env::current_dir()` と `DefaultTaskIdGenerator::new`（`getrandom` による初期化）を無条件に行っていた。前者は `FsWorkflowStore` の `base_dir` — `--workflow` に与えた相対パスの解決基準にしか使われず、後者はタスクIDの発行にしか使われない。どちらも `tick` の動作には要らない資源だが、失敗すれば `WireError::CurrentDirUnavailable` / `WireError::IdGenerator` として `tick` が非0で終わる。

pages の「縮退状態の共通規則1」（各コマンドは自身の動作に必要なリソースだけを検証する）を根拠に `current_exe()` を `compose()` から切り出したのが `.adr/2-process-controller-injects-self-exe-and-identity-source.md` で、同じ規則をラッパーの中でもう一段適用したのが `without_self_exe` の判断だった。同じ規則が同じ合成ルートの中で `current_dir` と乱数にだけ適用されていない。tick は任意の作業ディレクトリから起動しても同じ結果になることを要求されており、帳簿がすべて絶対パスで閉じているのに結線だけが作業ディレクトリを要求する形は、規則の非対称であると同時にその要求を結線の側から裏切る。

## 決定

`.adr/2-process-controller-injects-self-exe-and-identity-source.md` と同じ形で切り出す。`compose()` に残すのはホームの解決とグローバル設定の読み込みだけにする。

- `Runtime` は `FsWorkflowStore` ではなく `workflows_dir: PathBuf` を持ち、`Runtime::workflow_store() -> Result<FsWorkflowStore, WireError>` が呼ばれた時点でカレントディレクトリを読む。ワークフローの置き場は解決済みのホームから決まるので、`Runtime` のメソッドにする
- `wire::id_generator() -> Result<DefaultTaskIdGenerator<SystemClock>, WireError>` は `process_controller()` と同じ自由関数にする。ID の発行はホームから何も受け取らない
- タスクを登録するコマンドが両方を呼ぶ。`tick` / `wrapper` の経路からは消える

`WireError` の2変種と `cli::render` の文言はそのまま残す — 登録コマンドから到達する失敗であることは変わらない。

## 検討した代替案

- **`Runtime` に `OnceCell` を持たせて遅延構築を隠す** — 内部可変性が入る。構築が要るのは1経路で高々1回、`compose` の呼び出し元は各コマンドの `execute` 1箇所しかないので、呼び名で構築だと分かる形のほうが読める
- **コマンドごとに `Runtime` を型で分ける** — 共有する項目（config・state_root・worktrees・clock・tasks・lock）が大半で、コマンドが増えるたびに組み合わせの型が増える。ラッパーだけを別の構成にしたのは、ホームも config も読まないという**構成そのものの違い**があるから（`.adr/2-wrapper-restores-state-root-from-run-dir.md`）で、ここにはその違いが無い

## 影響

- 各コマンドの非0終了が、そのコマンドが実際に使う資源の失敗だけに閉じる。「tick は任意の作業ディレクトリから起動しても同じ結果になる」が結線の形としても読めるようになり、規則の適用が `current_exe` / `current_dir` / 乱数で揃う
- `compose()` に残るのがホームと config だけになり、「全コマンド共通の起動時処理」という宣言と中身が一致する
- トレードオフ: `Runtime` のメソッドに、参照を返すアクセサと構築して `Result` を返すものが混ざる。`workflow_store` / `id_generator` という呼び名を `process_controller` に揃えて、構築であることを名前で示す
- トレードオフ: 登録コマンドの失敗の検出点が `compose` より1段後ろへ動く。いずれもタスクを作る前で状態は変わらないので、利用者から見える挙動は文言も終了コードも変わらない
