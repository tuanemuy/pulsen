# 085: worktree の同定は物理パスで行い、実体の消えた登録は張り直す

## ステータス

承認済み

## コンテキスト

`WorktreeManager::create` の冪等性は「`ws.path` に `ws.branch` の worktree がある」場合だけが達成済み、という境界の上に立つ。この判定を `git worktree list --porcelain` の出力と `ws.path` の**文字列比較**で行うと成立しない。実測（git 2.55、macOS）:

- 渡したパス `/var/folders/.../tmp.RF47.../wt/t1` に対し、git の出力は `/private/var/folders/.../tmp.RF47.../wt/t1`
- `worktree_root` がシンボリックリンクを含むとき（`home -> realhome`）、渡した `.../home/wt/t2` に対し出力は `.../realhome/wt/t2`

`ws.path` は `WorktreeRoot`（`HOME` 由来）から `WorkspacePlanner::derive` で導出した値なので、macOS の一時ホーム（受け入れ・適合テストのハーネスが必ず使う）や `~` にシンボリックリンクを含む利用者環境では、判定が**常に外れる**。

もう1つ、登録は残っているが実体が消えている状態（git が `prunable gitdir file points to non-existent location` を添えて列挙する）がある。クラッシュや利用者の手動削除で実際に起こる。実測:

- この状態でも `branch refs/heads/pulsen/t1` 付きで列挙される（ブランチだけを見た判定は「達成済み」に倒れる）
- `git worktree add <path> <branch>` は `fatal: ... is a missing but already registered worktree`（exit 128）
- `git worktree add -f <path> <branch>` は exit 0 で張り直す。ブランチ先端は変わらず、積まれたコミットの成果物が worktree に戻る

3つめに、**登録がまったく無く、ブランチだけが存在する**状態がある（利用者の `git worktree remove` / `prune`、git の gc による自動 prune、終端処理でブランチだけが残った後）。実測では `git worktree list --porcelain` に当該パスのエントリが現れず、`git worktree add <path> <branch>`（`-f` なし）が exit 0 で成功し、ブランチ先端を変えずに成果物が worktree に現れる。

## 決定

同定の鍵を**物理パス**にし、実体の存在を達成済みの条件に加える。

- 鍵の作り方を1つの private 関数 `physical_key(p) = canonicalize(p.parent()) . join(p.file_name())` に閉じ、`ws.path` と `worktree list --porcelain` が返した各パスの**両方**をこの関数に通してから、鍵同士を比較する。`ws.path` の親（worktree_root）は比較の前に `ensure_dir` する。パス自体を canonicalize しないのは、実体が消えている場合に失敗して比較そのものが成立しないため（親は本メソッドが作るので必ず存在する）。鍵に変換できない git 側のエントリ（親ごと消えている他タスクの登録など）は自タスクのものではないので不一致として扱う。**生のパスの文字列比較は禁じる**
- **正規化は両側に対称に適用する**。片側だけを正規化すると Windows で必ず外れる — `std::fs::canonicalize` は拡張長パス（`\\?\C:\...`）を返すのに対し、Git for Windows の `worktree list --porcelain` は `C:/...` 形式を出すため、git の出力をそのまま突き合わせる形にすると鍵が**恒常的に**不一致になる。そうなると既存 worktree を持つタスクは「登録と一致」の分岐に入らず、`ws.path` が実体として存在するので「登録が無く実体がある = `Failed`」の分岐に落ち、毎 tick `record_tool_failure(WorktreeCreate)` を繰り返して上限超過 stopped に至る（冪等成功が主経路にあるため、初回起動以降の全ステータスで壊れる）。両側を同じ関数に通せば、区切り文字も接頭辞も正規化の結果として揃う
- 達成済みの条件は「鍵一致 + 自ブランチ + **実体が存在** + `prunable` でない」。自タスクの登録（自分のパス + 自分のブランチ）でも、実体が無いか `prunable` が付いていれば `git worktree add -f <path> <branch>` で張り直して `Ok` を返す。実体の有無は `try_exists` で直接観測する — `prunable` の注記は git のバージョンによっては出力されず、注記だけを分岐の鍵にすると実体の消えた登録が「達成済み」に倒れる
- 登録がまったく無く、ブランチだけが存在する場合は `git worktree add <path> <branch>`（**`-f` なし**）で張り直して `Ok` を返す。先端を変えないので、そのブランチに積まれたコミットの成果物が worktree に戻る
- `-f` の適用範囲をこの分岐に限る。`-f` は「別の worktree でチェックアウト済みのブランチ」の保護も外す。鍵が自タスクのパスと一致し、そのエントリが自タスクのブランチを指していることを確認した後だけに使えば、外す保護は「登録は残るが実体が無い」1つに閉じる

## 検討した代替案

- **`git worktree prune` で掃除してから `add` する** — prune はリポジトリ全体の stale な登録に効くため、同一リポジトリで動く他タスクの状態にも触れる。`add -f` は対象パス1つに閉じる
- **張り直さず `Failed` を返す** — `create` の契約は「`ws.path` に worktree を用意する」であり、達成済みとして `Ok` を返すと `confirm_workspace` → `record_launching` → spawn と進み、ラッパーの `run_agent` が cwd 不在で 126 を書き、リトライのたびに同じ 126 を繰り返して上限超過 stopped に至る。spec は自タスクの残骸に対して冪等な成功を求めているので、張り直せる状態を失敗に落とす理由がない

## 影響

- 一時ディレクトリ・シンボリックリンクを含むホーム（macOS の既定を含む）でも冪等判定が成立し、「作成成功 → 保存前クラッシュからの復旧」が環境に依らず通る。クラッシュ後に実体だけが消えた登録（`prunable` の注記が出るか否かを問わない）も、登録ごと消えてブランチだけが残った状態も自己修復する
- 両側対称の正規化なので、Windows 実機検証を待たずに鍵の一致条件が決まる。片側だけの正規化は、macOS / Linux では緑のまま Windows でだけ壊れるため実機まで顕在化しない
- トレードオフ: `create` の中で `canonicalize` の I/O が、比較するエントリの数だけ増える
- 復旧の2分岐は**どちらもテストで実行される必要がある**。台帳の「ブランチのみ存在」のケースは字義どおり「登録なし・ブランチのみ存在（コミットが積まれている）」の前提で作り、`-f` なしの張り直しを通す。`prunable` 登録の張り直しは台帳に無い本 ADR 由来の要求なので、ハーネスの別フックによる**追加ケース**として適合スイートに置く。前者を prunable 側に寄せると、「ブランチもパスも未使用 → `add -b`」にも該当しない「登録なし・ブランチのみ」分岐がどのケースからも実行されず、実装ごと落ちても全テストが緑になる
- トレードオフ: ハーネスの前提（`worktree_root` をシンボリックリンク経由にする）が正規化の分岐を通す条件になる。前提の作り方が緩むと、テストは緑のまま実装が退行しうる
