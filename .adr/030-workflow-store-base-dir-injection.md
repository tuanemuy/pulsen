# 030: `FsWorkflowStore` は基準ディレクトリを注入して相対パスを解決する

## ステータス

承認済み

## コンテキスト

spec/testcases/ports/workflow-store.md は「相対パスはプロセスのカレントディレクトリから解決される」と定める。アダプターが `std::env::current_dir()` を直接読む実装にすると、TC-port-workflow-store-005 を in-process の適合テストで検証するのに `std::env::set_current_dir` が必要になる。これはプロセス全体の可変状態であり、`cargo test` の既定(マルチスレッド並列)では他テストのパス解決を壊す。

## 決定

`FsWorkflowStore::new(workflows_dir: PathBuf, base_dir: PathBuf)` の形で基準ディレクトリを構築時に受け取り、`WorkflowRef::Path(p)` が相対なら `base_dir` で絶対化する。合成ルート(`cli`)が `std::env::current_dir()` を渡すことで spec の契約はそのまま保たれる。`--repo` の相対パスも同じ理由で合成ルートで絶対化してからユースケースに渡す。cwd を読むのは合成ルートの1箇所だけにする。

**`base_dir` が絶対パスであることを呼び出し側の前提とする**。絶対化に使う `std::path::absolute` は引数が相対なら内部で cwd を読むため、`base_dir` が相対だと「cwd を読むのは合成ルートの1箇所だけ」がアダプターの中で破れ、同時に走る別のストアの解決結果がプロセスの状態に左右される。前提はホームの解決(`PulsenHome::new` が絶対性を検証する)と `std::env::current_dir()` の戻り値がいずれも絶対パスであることから、合成ルートで満たされる。

## 影響

- 「グローバル可変状態に依存しない自己完結した値」(CLAUDE.md)になり、適合テストが cwd を触らずに並列実行できる
- トレードオフ: 構築時の引数が1つ増える。「カレントディレクトリから解決」という契約が合成ルートの結線に依存するため、その結線を CLI 受け入れテスト(相対 `--repo` / 相対 `--workflow`)で確認する
- トレードオフ: `base_dir` の絶対性は型でも実行時検査でも要求せず、呼び出し側の前提として doc に書くにとどめる。破ると診断されずに cwd 依存へ落ちるため、`FsWorkflowStore::new` の doc とこの ADR の両方に前提を明記する
