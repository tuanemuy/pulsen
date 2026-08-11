# ポート適合テスト: WorktreeManager

対象契約: [../../domains/execution.md#worktreemanager](../../domains/execution.md#worktreemanager)

すべてのアダプター実装が共通で通す。フィクスチャとしてテスト用のgitリポジトリ(コミットあり・コミットなしの空リポジトリ・detached HEAD 状態)を用いる。

| 前提条件 | 操作 | 期待結果 | 実装ステータス |
|---|---|---|---|
| コミットのある実在するgitリポジトリ | `validate_repo(repo)` | `Ok(())` | |
| 存在しないパス | `validate_repo(repo)` | `Err(TargetError::NotFound)` | |
| 実在するがgitリポジトリでないディレクトリ | `validate_repo(repo)` | `Err(TargetError::NotARepository)` | |
| HEADがブランチを指すリポジトリ | `head_branch(repo)` | `Ok(そのブランチ名)` | |
| detached HEAD 状態のリポジトリ | `head_branch(repo)` | `Err(TargetError::DetachedHead)` | |
| コミットのない空リポジトリ | `head_branch(repo)` | `Err(TargetError::EmptyRepository)` | |
| 指定ブランチが存在するリポジトリ | `branch_exists(repo, branch)` | `Ok(true)` | |
| 指定ブランチが存在しないリポジトリ | `branch_exists(repo, branch)` | `Ok(false)` | |
| 検証対象の git 操作自体が失敗する状況(リポジトリメタデータの読み取りが失敗する等。当該状況を再現できるアダプター環境に限る) | `validate_repo(repo)` / `head_branch(repo)` / `branch_exists(repo, branch)` | `Err(TargetError::Failed { message })` を値として返す(パニックしない)。対象の分類(`NotFound` / `NotARepository` / `DetachedHead` / `EmptyRepository`)とは区別される | |
| base ブランチが存在し、`ws.branch`・`ws.path` とも未使用 | `create(repo, base, ws)` | `Ok(())`。`ws.path` に worktree が用意され、その HEAD は base の先端から作成された新ブランチ `ws.branch` を指す | |
| worktree_root(`ws.path` の親ディレクトリ)自体がまだ存在しない(初回タスクの起動) | `create(repo, base, ws)` | `Ok(())`。親ディレクトリを作成したうえで worktree が用意される(ツール管理領域の自動作成) | |
| `create` 成功済みで、`ws.path` に `ws.branch` の worktree として存在(worktree内にファイル変更あり) | 同じ引数で再度 `create` | `Ok`(達成済みとして成功)。worktree の内容(既存のファイル変更)に一切触れない(自タスク残骸への冪等性) | |
| ブランチ `ws.branch` のみ存在し(コミットが積まれている)、`ws.path` に worktree がない | `create(repo, base, ws)` | `Ok`。既存ブランチ `ws.branch` に worktree を張り直す。ブランチの先端は変更されない(積まれたコミットが保持され、base から作り直されない) | |
| `ws.path` に worktree でない通常のディレクトリ(ファイルを含む)が存在 | `create(repo, base, ws)` | `Err(WorktreeError::Failed { message })`。既存ディレクトリの内容に触れず、自動修復も行わない | |
| `ws.path` に `ws.branch` **以外**のブランチの worktree が存在 | `create(repo, base, ws)` | `Err(WorktreeError::Failed { message })`。既存 worktree(実体・登録・そのブランチ)に触れず、自動修復も行わない(冪等成功は `ws.branch` の worktree として存在する場合に限る。パスの存在のみでは達成済みとみなさない) | |
| base に指定したブランチがリポジトリに存在しない | `create(repo, base, ws)` | `Err(WorktreeError::Failed { message })`。ブランチ・worktree とも作られない | |
| `create` 成功済みの worktree が存在 | `remove(repo, ws.path)` | `Ok(Removed)`。worktree の実体と登録が消える。ブランチ `ws.branch` は残る(ブランチには一切触れない) | |
| worktree 内に未コミット変更・未追跡ファイル・`.git` 配下の残骸(`index.lock` 相当)がある(dirty な worktree) | `remove(repo, ws.path)` | `Ok(Removed)`(内容の状態によらず削除する — git worktree remove --force 相当。クリーンアップの主経路)。ブランチは残る | |
| worktree が既に存在しない(手動削除・前回削除済み等) | `remove(repo, path)` | `Ok(AlreadyAbsent)`(達成済みとして成功) | |
| `remove` で `Removed` を得た直後 | 同じ引数で再度 `remove` | `Ok(AlreadyAbsent)`(冪等)。ブランチは引き続き残る | |
| `create` 成功済みの worktree が存在し、その削除操作自体が失敗する状況(削除権限がない等。当該状況を再現できるアダプター環境に限る) | `remove(repo, ws.path)` | `Err(WorktreeError::Failed { message })` を値として返す(パニックしない。呼び出し側が `record_tool_failure(WorktreeRemove)` の入力にする報告用エラー)。worktree(実体・登録)とブランチの既存状態には触れない(次回の `remove` が同じ前提から再試行できる) | |
