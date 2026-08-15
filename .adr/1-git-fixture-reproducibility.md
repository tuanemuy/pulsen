# ADR: git フィクスチャは環境変数と初期化オプションで再現性を固定する

## ステータス

承認済み

## コンテキスト

`git init` した一時リポジトリをフィクスチャにするテスト(worktree-manager 適合テスト・CLI 受け入れテスト)は、環境で結果が変わる。

- コミットを作るには `user.name` / `user.email` が要る
- 既定ブランチ名は `init.defaultBranch` やユーザーのグローバル設定で変わる
- `rev-parse --show-toplevel` は上位ディレクトリを遡るため、TMPDIR がたまたま git リポジトリ配下にあると「git リポジトリでないディレクトリ」のフィクスチャが成立しない(実測確認)

## 決定

テストのフィクスチャ生成ヘルパーで次を固定する(本番コードには入れない。`.adr/1-git-cli-shell-out-and-target-classification.md`)。

- `git init -b main <dir>` で既定ブランチ名を明示する
- 起動する git の環境に `GIT_CONFIG_GLOBAL=/dev/null`(Windows では `NUL`)・`GIT_CONFIG_SYSTEM` を設定し、`GIT_DIR` / `GIT_WORK_TREE` / `GIT_INDEX_FILE` を除去する
- コミットは `git -c user.name=pulsen-test -c user.email=pulsen-test@example.invalid commit` で作る
- 「git リポジトリでないディレクトリ」のフィクスチャは、生成直後に `git -C <dir> rev-parse --show-toplevel` が失敗することを確認してから使う。成功する場合はそのケースをスキップする

## 影響

- 開発者の git 設定・TMPDIR の位置に関わらずテストが決定的になる
- トレードオフ: フィクスチャヘルパーが少し厚くなる。スキップ条件が1つ増える
