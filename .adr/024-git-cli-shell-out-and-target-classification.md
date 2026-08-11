# 024: git 操作は git CLI へのシェルアウトで実装し、対象の分類は問い合わせコマンドの組み合わせで導く

## ステータス

承認済み

## コンテキスト

WorktreeManager は本スライスで `validate_repo` / `head_branch` / `branch_exists`、後続スライスで `create`(`git worktree add` 相当)と `remove`(dirty な worktree も削除する `git worktree remove --force` 相当)を要求される。実装手段は libgit2 バインディング(`git2`)と git CLI へのシェルアウト。

当初は `head_branch` を `symbolic-ref --short HEAD` 1本で「detached / 空リポジトリを区別する」としていたが、実測で成立しないことが分かった。

| リポジトリの状態 | `symbolic-ref --short HEAD` | `rev-parse --verify --quiet HEAD` |
|---|---|---|
| コミットあり・HEADがブランチ | exit 0 / ブランチ名 | exit 0 / コミットID |
| コミットのない空リポジトリ(unborn HEAD) | **exit 0 / ブランチ名** | exit 1 |
| detached HEAD | exit 128 | exit 0 |

空リポジトリでも `symbolic-ref` が exit 0 で `main` を返すため、旧方針では `TargetError::EmptyRepository` を返す経路が存在せず、「空リポジトリで `--base` 省略 → `main` をベースブランチとして登録成功」という誤った成功になる。

また、`TargetError::Failed` へ到達する経路を「リポジトリのメタデータを壊す」で作れるという当初の想定も実測で崩れた。`.git/HEAD` の破壊・権限剥奪、`.git/objects` の削除、`.git/config` の構文不正、`repositoryformatversion = 99` のいずれでも、`rev-parse --show-toplevel` は exit 128 になり「git リポジトリでない」と区別できない。

## 決定

git CLI へシェルアウトする(`git -C <repo> ...`)。`git` を実行時の前提とし、`flake.nix` の devShell にも追加する。

**git 実行ファイルのパスを構築時に注入する**: `GitCliWorktreeManager::new(git_program: PathBuf)`(合成ルートが既定値 `"git"` を渡す)。ADR-030 が `base_dir` を注入したのと同型で、結線は `cli::wire` の1箇所に閉じる。これにより適合テストのハーネスは、存在しないパスを `git_program` として構築した2つ目の `GitCliWorktreeManager` を持つだけで、3メソッドすべてが `Failed` に落ちる実装を供給できる(ADR-027 の `failing_manager`)。本番アダプターはイミュータブルなままで、権限操作にも root 実行の可否にも依存しない。

各メソッドの判定は次のとおり固定する。

`validate_repo(repo)`

1. パスが存在しない → `NotFound`
2. `git -C <repo> rev-parse --show-toplevel` の**起動自体に失敗** → `Failed`
3. 起動できて exit が非0 → `NotARepository`(メタデータ破損もすべてここに落ちる)
4. exit 0 → `Ok(())`。リポジトリ配下のサブディレクトリ指定も受理する

`head_branch(repo)` — どちらかの起動に失敗したら `Failed`。

| `symbolic-ref --short HEAD` | `rev-parse --verify --quiet HEAD` | 結果 |
|---|---|---|
| exit 0 | exit 0 | 出力を `BranchName::parse` に通し、成功なら `Ok(ブランチ名)`、失敗なら `Err(Failed)` |
| exit 0 | 非0 | `Err(EmptyRepository)` |
| 非0 | exit 0 | `Err(DetachedHead)` |
| 非0 | 非0 | `Err(Failed)` |

`BranchName` は git より狭い実用サブセットなので、git 側で有効な名前がドメインで弾かれうる。その場合は `Failed`(実行環境エラー)に落とす — 対象の分類としては正常に読めており、ツール側が扱えないという実行環境の制約だからである。分類を増やすことはしない(spec の `TargetError` は5種で閉じている)。

`branch_exists(repo, branch)` — `git -C <repo> show-ref --verify --quiet refs/heads/<branch>` の exit 0 → `true`、exit 1 → `false`、それ以外と起動失敗 → `Failed`。

共通: 起動する git プロセスの環境から、**`-C` で指した対象の解決結果を呼び出し元の環境が上書きしうる変数**を除去する。判断基準はこの目的であって特定の変数名ではない。現時点の対象は `GIT_DIR` / `GIT_WORK_TREE` / `GIT_INDEX_FILE` / `GIT_CEILING_DIRECTORIES` / `GIT_COMMON_DIR` / `GIT_OBJECT_DIRECTORY` / `GIT_ALTERNATE_OBJECT_DIRECTORIES`。`GIT_CEILING_DIRECTORIES` は上位探索を打ち切るため、設定されていると正当なリポジトリが `NotARepository` に落ちる(実測: `GIT_CEILING_DIRECTORIES=<repo> git -C <repo>/sub rev-parse --show-toplevel` が exit 128)。cron からの無人実行では継承された環境の内容を利用者が意識していないため、対象の分類が呼び出し元の環境で変わってはならない。

ユーザーのグローバル設定(`safe.directory` 等)は本番では尊重する — 対象の解決先を変えるものではなく、無効化すると所有者の異なるリポジトリを扱えなくなる。テストフィクスチャ側の環境固定は ADR-033。

## 影響

- `worktree add` / `remove --force` の意味論が git 本体と完全に一致し、後続スライスの冪等要件を素直に書ける。C ツールチェーンが不要で Nix devShell の構成も軽いまま
- `TargetError::Failed` の到達経路は「git を起動できない」1本に定まる。リポジトリメタデータの破損は `NotARepository`(`validate_repo`)/ `Failed`(`head_branch` / `branch_exists`)に分かれるという非対称を許容する — spec が要求するのは「分類と区別された `Failed` を値として返す」ことである
- トレードオフ: `git` の存在が実行時依存になる。`head_branch` が2プロセス起動になる。メッセージ文字列への依存は避け、exit code と起動可否だけで分類する
