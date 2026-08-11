# ADR — Issue #1: 基盤・グローバル設定・ワークフロー定義とタスク登録(add)

本書の ADR 番号は、プロジェクトの正本 `.adr/`（既存18件）の**続き番号**として採番する（ADR-019 以降）。本文中に現れる ADR-001〜018 は `.adr/` の既存 ADR を指し、本書のエントリと番号が衝突しない。実装時の起票手順は ADR-035 を参照。

## ADR-019: ドメインを独立クレートに切り出した3クレートのワークスペースにする

### Status
Proposed

### Context

CLAUDE.md は「ドメイン層は外部クレートや I/O に依存しない」「依存は常に外側から内側へ」を要求する。実現手段は2つあった。

- 単一クレート内のモジュール分割（`domain` / `adapter` / `application` / `cli`）— 境界は規約とレビューで守る
- ワークスペース分割 — 境界を Cargo の依存グラフで守る

グリーンフィールドであり、以降10前後のスライスがこの構成の上に積まれる。

### Decision

3クレートのワークスペースにする。

- `pulsen-domain`: definition / task / execution の3ドメインとポート。**`[dependencies]` を空に保つ**
- `pulsen-conformance`: ポート適合テストのスイート（ADR-027）と、ポートのテストダブル（ADR-028）。`pulsen-domain` のみに依存し、`pulsen` の **`[dev-dependencies]`** としてのみ参照される（本番バイナリには載らない）
- `pulsen`: bin + lib。アダプター・アプリケーション・CLI・共通ユーティリティ

ドメイン内のドメイン間依存（Definition ← Task ← Execution。ADR-017）はモジュール規約で守る。ドメインを3クレートに割るところまではしない — 3者は同一の変更単位（spec/domains/）で動くことが多く、クレート分割のコストに見合わない。

### Consequences

- 良い点: 「ドメインが serde / chrono に依存した」がコンパイル時に検出される。適合テストスイートを独立に配布でき、後続スライスの in-memory アダプターからも使える
- トレードオフ: `Cargo.toml` が3つになり、型を跨いで動かすときに `pub` の付け替えが要る。ドメイン間の依存方向（definition ← task）はコンパイラでは守られない

---

## ADR-020: ドメイン型に serde を実装せず、`Timestamp` の RFC3339 変換はドメインに持たせる

### Status
Accepted（ステップ5で確定。`.adr/020-no-serde-in-domain-timestamp-conversion-in-domain.md`）

### Context

タスクファイル（JSON）とグローバル設定・ワークフロー定義（YAML）の読み書きが必要になる。ドメイン型に `#[derive(Serialize, Deserialize)]` を付ければ実装は短くなるが、`pulsen-domain` が serde に依存する（ADR-019 に反する）。加えて spec は「デコード（YAML/JSON → ドメイン型）はアダプター境界の責務」であり、復号の失敗を `Corrupt` / `SnapshotUnreadable` に**区別して**写像することを求めている。

当初は `Timestamp` ↔ RFC3339 の変換も `pulsen::util::rfc3339` に置く方針だったが、前提が2点崩れた。

- spec/domains/task.md の `Timestamp` は「生成: `Clock` ポート、または RFC3339 文字列のパース」と定めており、変換をドメインの外に出すのは spec からの逸脱になる
- TC-port-clock-002 は「`now` の返値を RFC3339 に直列化して再パースし、元の `Timestamp` と等価」を要求するが、`pulsen-domain` のみに依存する適合スイート（ADR-019）からは `pulsen::util` を呼べず、このケースが書けない

### Decision

- ドメイン型は serde を持たない。アダプター側に永続化 DTO（serde derive 付き）を定義し、DTO ↔ ドメインの変換で必ず `parse` / `rehydrate` を通す。
- `Timestamp` はドメインで Unix 秒（UTC・秒精度）を保持し、`Timestamp::parse_rfc3339(&str) -> Result<Self, TimestampError>` と `to_rfc3339(&self) -> String` を**ドメインに持たせる**。暦計算は proleptic Gregorian の days-from-civil / civil-from-days を自前で書く（外部クレート不要。約35行。往復・うるう年・不正日付の拒否を実測で確認済み）。受理形式は `YYYY-MM-DDTHH:MM:SSZ` のみに限定する（spec の直列化表現がこれ1つであり、オフセット付き表記やサブ秒を受理すると往復可能性が壊れる）。
- **日数と秒への分解は `div_euclid` / `rem_euclid` を使う**。Rust の `/` / `%` はゼロ方向に丸めるため、epoch より前（負の Unix 秒）で1日ずれる（実測: `-1 / 86400 = 0`・`-1 % 86400 = -1` に対し `div_euclid = -1`・`rem_euclid = 86399`）。
- **`Clock` 実装が `Timestamp` を作る唯一の口を `Timestamp::from_unix_secs(secs: i64) -> Result<Self, TimestampError>` としてドメインに置く**。`SystemClock` は `SystemTime` の経過秒しか持たないため、この口がないと (a) ドメインに場当たりの生成関数を足す、(b) アダプターで暦計算を再実装する（本 ADR が禁じたばかり）のどちらかに流れる。`parse_rfc3339` と合わせ、生成経路はこの2つだけになる（`Timestamp` のフィールドは非公開）。
- **`Timestamp` の表現可能範囲を 0001-01-01T00:00:00Z 〜 9999-12-31T23:59:59Z に閉じる**。生成経路（`from_unix_secs`・`parse_rfc3339`）でこの範囲外を拒否する。`format!("{:04}", y)` は5桁年で単に桁が増え、20文字固定の `parse_rfc3339` が受け付けないため、範囲を閉じないと `to_rfc3339 ∘ parse_rfc3339 = id` が全域で成り立たない（実測確認）。範囲外の壁時計を `Clock` がどう扱うかは ADR-036。
- この結果、`time` クレートは不要になる（ADR-023）。`util::rfc3339` モジュールは設けない。
- 直列化形式（JSON のキー名・列挙値の綴り）はアダプターの関心事であり、spec の型名に機械的に一致させる義務を負わない。

### Consequences

- 良い点: 「不正な状態を型で表現不能にする」が直列化経路でも保たれる（デシリアライズが不変条件を迂回しない）。`Corrupt` / `SnapshotUnreadable` の判定を DTO 層で自然に書ける。適合スイートが Clock TC-002 を1テストとして書ける。spec の記述との逸脱が消え、依存が1つ減る
- トレードオフ: DTO とドメイン型の二重定義になり、フィールド追加のたびに2箇所を触る。暦計算をドメインに持つ（純粋関数なのでユニットテストで網羅できる）。往復可能性はテスト（適合テストの「往復可能性」節）で担保する

---

## ADR-021: YAML は Value 化してから手書きでスキーマ走査する（serde の deny_unknown_fields に頼らない）

### Status
Proposed

### Context

ADR-013 はワークフローYAML・config.yaml の未知キーを読み込み時エラーにすることを要求し、spec は `WorkflowParseError` として `YamlSyntax`（重複キー含む）/ `UnknownKey` / `InvalidValue` を**区別**することを要求する。serde の `deny_unknown_fields` を使うと、未知キー・型不一致・値エラーがすべて `serde::de::Error` になり、エラー種の区別が文字列マッチ頼みになる。

### Decision

`serde_yaml_ng`（実測で重複キー検出・エラー位置取得を確認済み）で YAML を `Value` に落とすところまでを外部クレートに任せ、`Value` → `RawWorkflowDoc` / `GlobalConfig` のスキーマ走査は手書きにする。

- 構文エラー・重複キー → `YamlSyntax { message, location }`（config では `Invalid`）
- スキーマに無いキー → `UnknownKey { location, key }`（config では `Invalid`）。`location` は論理パス（例: `statuses.queued`）で表現する
- 型不一致・値の生成失敗 → `InvalidValue { location, message }`
- 空ファイル・null ドキュメント → 「全キー省略」として `Ok`（`Err` にしない）

### Consequences

- 良い点: エラー種の区別が構造的に決まり、文字列マッチが要らない。エラーメッセージの文言を spec の案内に合わせて自由に組める
- トレードオフ: 走査コードを書く量が増える。YAML クレートを差し替えても影響が `adapter::yaml` に閉じる点は利点でもある
- 注意: `serde_yaml_ng` のスカラー解決は YAML 1.2 core schema 相当で、`no` / `off` / `yes` / `n` は `Value::String` になり bool には変換されない（実測確認）。bool になるのは `true` / `false` のみ。「YAML 1.1 の暗黙 bool 変換」を前提としたテスト期待（`agent: no` が `InvalidValue` になる等）を書かない

---

## ADR-022: 排他ロックは標準ライブラリの `File::try_lock` で実装し、`LockGuard` はドメインのマーカートレイトにする

### Status
Proposed

### Context

requirements §4.3 はファイルの排他ロックを OS 依存操作として挙げ、spec は「ブロックしない」「取得できないのは `Ok(None)`」「機構の異常は `Err(Failed)`」「保持プロセスの異常終了でも OS が解放」を要求する。実現手段として `fs4` / `fd-lock` 等の外部クレートと標準ライブラリがある。

Rust 1.89 で `std::fs::File::try_lock() -> Result<(), std::fs::TryLockError>` が安定化しており（本環境は 1.97）、`TryLockError::WouldBlock` と `TryLockError::Error(io)` が spec の2分岐にそのまま対応する（実測確認済み）。

### Decision

- 標準ライブラリの `File::try_lock` を使い、ロック用の外部クレートを追加しない。ロックファイルは `<home>/state/lock`。
- `LockGuard` はドメインが定義するマーカートレイトとし、ポートは `Result<Option<Box<dyn LockGuard>>, LockError>` を返す。実体（`File` を保持し `Drop` で解放する構造体）はアダプターに置く。
- workspace の `Cargo.toml` に `rust-version = "1.89"` を明記する（`File::try_lock` が無いツールチェーンを引いたときに原因が即座に分かる）。

### 検討した代替案

- ポートに関連型 `type Guard: LockGuard;` を持たせ、`Box` と動的ディスパッチを避ける — spec/domains/execution.md は `try_acquire(&self) -> Result<Option<LockGuard>, LockError>` と書いており、ガードの具体型を利用側に見せない意図がある。関連型にすると型引数が合成ルートまで伝播し、実装ごとに結線先の型が変わる。取得は1コマンド1回で、動的ディスパッチのコストは無視できる。よって `Box<dyn LockGuard>` を採る

### Consequences

- 良い点: 依存が減り、Windows/POSIX の差異は標準ライブラリが吸収する。ドメインはロックの寿命だけを知り、ハンドルの型を知らない
- トレードオフ: `rust-version` が 1.89 以上に固定される。`Box<dyn>` により動的ディスパッチが1箇所入る

---

## ADR-023: 依存クレートの選定

### Status
Proposed

### Context

CLI・YAML・JSON・乱数・一時ファイルが必要になる。ドメインは zero-dep（ADR-019）なので、すべて `pulsen` クレートの依存になる。

### Decision

| 用途 | 採用 | 理由 / 却下した選択肢 |
|---|---|---|
| CLI パース | `clap`（derive） | サブコマンドの隠し表示（後続スライスの `wrapper` に必要）・ヘルプ生成・usage エラーの exit code 2 を標準で持つ。`pico-args` / `lexopt` は軽いが、ヘルプと案内の質を自前で持つことになる |
| YAML | `serde_yaml_ng`（`Value` としてのみ使用） | 重複キー検出とエラー位置を実測で確認。`serde_yaml` は deprecated、`serde_yml` は unmaintained 宣言。`saphyr` は 0.0.x で API 未安定。使用箇所を `adapter::yaml` に閉じるため差し替え可能 |
| JSON | `serde` + `serde_json`（`raw_value`） | 人間可読なタスクファイル（requirements §9）。`RawValue` が `save_degraded` の破損スナップショット温存（ADR-025）に必要 |
| 乱数 | `getrandom` | タスクIDのランダム成分にのみ使う。`rand` は分布・アルゴリズムを必要としないため過剰 |
| 一時ファイル | `tempfile` | アトミック置換の一時ファイル（同一ディレクトリ・後始末）とテストの一時ディレクトリ |
| 適合テスト・テストダブル | `pulsen-conformance`（**dev-dependency**） | `[dependencies]` に入れると本番バイナリにテストスイートが載る |

**採らないもの**:

- `home` — `std::env::home_dir()` は Rust 1.97 で非推奨が解除済みで Windows の挙動も修正されている（本環境で警告なしにコンパイル・動作することを実測確認）。std で足りるものを外部クレートにする理由がない。ホーム解決は合成ルート1箇所（ADR-031）
- `time` / `chrono` — RFC3339 変換をドメインに持たせたため不要になった（ADR-020）

### Consequences

- 良い点: いずれも用途が1〜2モジュールに閉じ、差し替え可能。本番依存は5用途・6クレート（`clap` / `serde_yaml_ng` / `serde` + `serde_json` / `getrandom` / `tempfile`）に収まる（`tempfile` は `util::atomic` のアトミック置換で本番コードが使う。`pulsen-conformance` は dev-dependency なので本番依存に数えない）
- トレードオフ: `serde_yaml_ng` はフォーク系クレートであり、長期のメンテナンスは保証されない。`Value` 化にしか使わないことで乗り換えコストを小さく保つ。暦計算を自前で持つ（ADR-020）

---

## ADR-024: git 操作は git CLI へのシェルアウトで実装し、対象の分類は専用の問い合わせコマンドの組み合わせで導く

### Status
Proposed

### Context

WorktreeManager は本スライスで `validate_repo` / `head_branch` / `branch_exists`、後続スライスで `create`（`git worktree add` 相当・自タスク残骸への冪等性）と `remove`（**dirty な worktree も削除する** `git worktree remove --force` 相当）を要求される。実装手段は libgit2 バインディング（`git2`）と git CLI へのシェルアウト。

当初は `head_branch` を `symbolic-ref --short HEAD` 1本で「detached / 空リポジトリを区別する」としていたが、実測で成立しないことが分かった。

| リポジトリの状態 | `symbolic-ref --short HEAD` | `rev-parse --verify --quiet HEAD` |
|---|---|---|
| コミットあり・HEADがブランチ | exit 0 / ブランチ名 | exit 0 / コミットID |
| コミットのない空リポジトリ（unborn HEAD） | **exit 0 / ブランチ名** | exit 1 |
| detached HEAD | exit 128 | exit 0 |

空リポジトリでも `symbolic-ref` が exit 0 で `main` を返すため、旧方針では `TargetError::EmptyRepository` を返す経路が存在せず、「空リポジトリで `--base` 省略 → `main` をベースブランチとして登録成功」という**誤った成功**になる（TC-port-worktree-manager-006・TC-task-register-task-039・PAGE-add-004 が満たせない）。

また `rev-parse --git-dir` は上位ディレクトリへ探索を遡るため、リポジトリ配下の任意のサブディレクトリで成功する（実測確認）。

さらに、`TargetError::Failed`（TC-port-worktree-manager-009）へ到達する経路を「リポジトリのメタデータを壊す」で作れるという当初の想定も、実測で崩れた。**あらゆるメタデータ破壊が `rev-parse --show-toplevel` の exit 128 に収束し、「git リポジトリでない」と区別できない**。

| 破壊の種類 | `--show-toplevel` | `symbolic-ref` | `rev-parse --verify` | `show-ref --verify` |
|---|---|---|---|---|
| `.git/HEAD` を壊す | 128 | 128 | 128 | 128 |
| `.git/HEAD` を読めなくする（chmod 000） | 128 | 128 | 128 | 128 |
| `.git/objects` を削除 | 128 | 128 | 128 | 128 |
| `.git` ディレクトリを読めなくする | 128 | 128 | 128 | 128 |
| `.git/config` が構文不正 | 128 | 128 | 128 | 128 |
| `repositoryformatversion = 99` | 128 | 128 | 128 | 128 |

`head_branch`（両コマンド失敗 → `Failed`）と `branch_exists`（exit 128 → `Failed`）はこれで `Failed` に落ちるが、`validate_repo` は `NotARepository` を返す。メッセージ文字列で分類しない方針である以上、メタデータ破壊では TC-009 の `validate_repo` 部分を満たせない。

### Decision

git CLI へシェルアウトする（`git -C <repo> ...`）。`git` を実行時の前提とし、`flake.nix` の devShell にも追加する。

**git 実行ファイルのパスを構築時に注入する**: `GitCliWorktreeManager::new(git_program: PathBuf)`（合成ルートが既定値 `"git"` を渡す）。ADR-030 が `base_dir` を注入したのと同型で、結線は `cli::wire` の1箇所に閉じる。これにより、適合テストのハーネスは**存在しないパスを `git_program` として構築した2つ目の `GitCliWorktreeManager`** を持つだけで、3メソッドすべてが `Failed` に落ちる実装を供給できる（ADR-027 の `failing_manager`）。本番アダプターはイミュータブルなまま（テスト専用の内部可変性を持ち込まない）で、権限操作にも root 実行の可否にも依存しない、ADR-032 と同じ発想の環境非依存な再現手段になる。

各メソッドの判定は次のとおり固定する。

**`validate_repo(repo)`**

1. パスが存在しない → `NotFound`
2. `git -C <repo> rev-parse --show-toplevel` の**起動自体に失敗**（実行ファイル不在・シグナル死等） → `Failed`
3. 起動できて exit が非0 → `NotARepository`（メタデータ破損も含めてすべてここに落ちる。上表）
4. exit 0 → `Ok(())`。**リポジトリ配下のサブディレクトリ指定も受理する**（`git worktree add` はサブディレクトリ指定でも動作するため実害がない。`--show-toplevel` と指定パスの一致までは求めない）

**`head_branch(repo)`** — 2コマンドの組み合わせで分類する。どちらかの**起動**に失敗したら `Failed`。

| `symbolic-ref --short HEAD` | `rev-parse --verify --quiet HEAD` | 結果 |
|---|---|---|
| exit 0 | exit 0 | 出力を `BranchName::parse` に通し、成功なら `Ok(そのブランチ名)`、失敗なら `Err(Failed { message })` |
| exit 0 | 非0 | `Err(EmptyRepository)` |
| 非0 | exit 0 | `Err(DetachedHead)` |
| 非0 | 非0 | `Err(Failed)`（HEAD 自体・リポジトリメタデータが壊れている） |

`BranchName` は git より狭い実用サブセット（先頭が `-` でない・`..` を含まない・`/` 始まり終わりでない・`.lock` 終わりでない）なので、git 側で有効な名前がドメインで弾かれうる。その場合は `Failed`（実行環境エラー）に落とす — 対象の**分類**としては正常に読めており、ツール側が扱えないという実行環境の制約だからである。分類を増やすことはしない（spec の `TargetError` は5種で閉じている）。

**`branch_exists(repo, branch)`** — `git -C <repo> show-ref --verify --quiet refs/heads/<branch>` の exit 0 → `true`、exit 1 → `false`、それ以外 → `Failed`。起動失敗も `Failed`。

**共通**: 起動する git プロセスの環境から `GIT_DIR` / `GIT_WORK_TREE` / `GIT_INDEX_FILE` を除去する（呼び出し元の環境が `-C` の対象指定を上書きしうるため）。ユーザーのグローバル設定（`safe.directory` 等）は本番では**尊重する**（無効化すると所有者の異なるリポジトリで動かなくなる）。テストフィクスチャ側の環境固定は ADR-033。

### Consequences

- 良い点: `worktree add` / `remove --force` の意味論が git 本体と完全に一致し、後続スライスの冪等要件を素直に書ける。C ツールチェーン（`cc` / `cmake` / OpenSSL）が不要で、Nix devShell の構成も軽いまま
- **`TargetError::Failed` の到達経路は「git を起動できない」1本に定まる**。3メソッドとも同じ経路で `Failed` に落ちるため、TC-port-worktree-manager-009 が3メソッド分まとめて成立する。リポジトリメタデータの破損は `NotARepository`（`validate_repo`）/ `Failed`（`head_branch` / `branch_exists`）に分かれるという**非対称を許容する** — spec が要求するのは「分類と区別された `Failed` を値として返す」ことであり、破損の種類ごとの分類ではない
- トレードオフ: `git` の存在が実行時依存になる。`head_branch` が2プロセス起動になる（登録時に1回だけなので無視できる）。構築時の引数が1つ増える。メッセージ文字列への依存は避け、exit code と起動可否だけで分類する

---

## ADR-025: タスクファイルは JSON 単一ファイルとし、`Corrupt` と `SnapshotUnreadable` は「JSON として有効か」で分ける

### Status
Proposed

### Context

ADR-015 はスナップショットをタスクファイルに正規化構造として埋め込むことを決めているが、物理形式（拡張子・キー構成）と、`save_degraded` が「破損したスナップショットフィールドを元の内容のまま書き戻す」契約をどう実装するかは未確定だった。ドメインの `DegradedTask` はスナップショットを持たない（持てない）ため、保存時に値をドメインから供給できない。

当初は復号2段を「(a) タスク側フィールドの破れ → `Corrupt`、(b) `snapshot` の**構文**・構造不変条件・`task_status ∈ statuses` の破れ → `SnapshotUnreadable`」と書いていたが、実現不能だった。`snapshot` を含むファイル全体を1回の JSON パースで読む以上、snapshot フィールドの中身が JSON 構文として壊れていればファイル全体のパースが失敗する（実測: `{"a":1,"snapshot":{broken}}` → `key must be a string at line 1 column 21`）。TC-port-task-repository-020（ファイル全体不正 → `Corrupt`）と 022/028（snapshot のみ → `SnapshotUnreadable`）を同時に満たす実装が導けない。

### 検討した代替案

- `snapshot` を JSON 文字列としてネストする（`"snapshot": "{\"initial\": ...}"`）— snapshot の構文破れをファイル全体から独立させられるが、エスケープだらけになって requirements §9 の人間可読性と ADR-015 の「正規化構造として埋め込む」を壊す。却下

### Decision

- タスクファイルは `state/tasks/<task-id>.json`（アーカイブは `state/archive/<task-id>.json`）の JSON 1ファイル。整形して書き、人間が直接読める状態にする。
- 復号の分類を次のとおり定義する。

  | 状態 | 分類 |
  |---|---|
  | ファイル全体が JSON として不正 | `Corrupt { path, message }` |
  | 有効な JSON だが、タスク側フィールドの型・値制約が破れている（実行状態に未知の値・`task_id` の文字集合違反・未知キー等） | `Corrupt` |
  | 有効な JSON で、タスク側フィールドはすべて読めるが、`snapshot` の値が**スナップショットとして解釈できない**（型不一致・必須キー欠落・フィールドの不在・構造不変条件違反（`initial ∉ statuses`・AgentRun の `next ∉ statuses`）・`task_status ∉ snapshot.statuses`） | `SnapshotUnreadable(DegradedTask)` |
  | 状態間整合の不変条件2〜4の破れ | 検証しない（`Intact` で返す。遷移関数の前提検査 `InvariantViolated` に委ねる） |

- 適合テストの破損フィクスチャは、この区分に沿って「有効な JSON でありながらスナップショットとして解釈できない」形で作る（`"snapshot": "壊れた"` / `"snapshot": {"initial": 123}` / `"snapshot"` キーの削除 / `task_status` を `statuses` に無い名前に差し替え）。
- 復号時、`snapshot` フィールドは `Box<serde_json::value::RawValue>` として生のまま保持する。`save_degraded` は**ディスク上の既存ファイルを読み直して `snapshot` の生バイト列を引き継ぎ**、タスク側フィールドだけを差し替えて書き戻す。DTO の `snapshot` は `Option<Box<RawValue>>` であり、`#[serde(skip_serializing_if = "Option::is_none")]` を付けて**キーの不在を不在のまま書き戻す**（既定の直列化では `snapshot` キーの削除（TC-023）で作った状態が `"snapshot": null` に化け、分類は `SnapshotUnreadable` のままでも「元の内容のまま温存する」という ADR-015 の契約から外れる）。
- タスク側の未知キーは拒否する（`deny_unknown_fields`）。スキーマバージョンのフィールドは設けない（spec にない前方互換の作り込みは行わない）。

### Consequences

- 良い点: 修復材料（壊れたスナップショット）が失われない。アーカイブ移動が単一ファイルの rename に帰着する（ADR-015 の狙いどおり）。`Corrupt` / `SnapshotUnreadable` の境界が実装可能かつ機械的に判定できる
- トレードオフ: `save_degraded` が読み → 書きの2ステップになる（書き込み自体はアトミック置換なので中間状態は観測されない）
- **spec への追従が要る**: spec/testcases/ports/task-repository.md の「スナップショットフィールドのみを**構文不正**な内容に置き換える」は、この設計では「有効な JSON だがスナップショットとして解釈不能」と読むほかない。spec 側の語を実装可能な表現へ言い換える提案を Issue のコメントに残す（勝手に簡略化せず、判断の記録を spec 側へ戻す）

---

## ADR-026: タスクIDは「UTC時刻成分 + ランダム成分」で発行する

### Status
Proposed

### Context

`TaskId` は `[a-z0-9-]`・1〜64文字・先頭英数字（ファイル名主部と `pulsen/<task-id>` ブランチ名として常に安全）。適合テストは「同一時刻の連続発行でも重複しない」「複数インスタンス間でも重複しない」を要求する。候補は UUIDv4/v7、ULID（小文字化）、自前の時刻+乱数。

### Decision

`<UTC yyyymmdd>t<hhmmss>-<base36 8桁の乱数>` 形式で発行する（例: `20260811t091530-k3f9qa1b`）。乱数は `getrandom` から取る。

- **時刻成分は `Timestamp::to_rfc3339()` の出力から導出する**（`-` / `:` / `Z` を除き `T` を `t` に置き換える）。時刻は構築時に受け取った `Clock` から取る（ADR-036 の「`generate` は無謬」と両立する。`SystemClock` を内部に隠さないのは、テストで時刻成分を固定できるようにするため）。暦計算（civil-from-days）は ADR-020 でドメインに置いた1箇所に留め、アダプターで再実装しない（CLAUDE.md「個別に再実装しない」）。テストも1箇所で済む。
- `generate(&self) -> TaskId` は無謬なので、エントロピーの取得は**構築時に一度だけ**行い、以降は内部 PRNG で進める（ADR-036）。
- **ランダム成分の桁数は適合テストの要求から逆算した**。spec/testcases/ports/task-id-generator.md は「`generate` を 10,000 回呼んで互いに重複しない」を要求する。`generate` がシステムコールを持たない（ADR-036）以上、1万回は同一秒内で完了し、一意性はランダム成分だけが担う。base36 6桁（36^6 ≒ 2.18e9）では誕生日問題で衝突確率が 1 - exp(-10^8/(2·36^6)) ≒ 2.3%（400 試行の実測で 14/400）となり、適合テストが 30〜40 回に1回フレーキーに落ちる。**8桁（36^8 ≒ 2.8e12）で衝突確率は 1.8e-5**（400 試行で 0）。後続スライスがこの桁数を縮めないよう、根拠をここに残す。

### Consequences

- 良い点: 制約を常に満たし、時刻順に並び、CLI で人間が打てる長さ（24文字。`TaskId` の 64 文字制限に十分収まる）に収まる。依存が `getrandom` だけで済む。暦計算がドメインの1箇所に閉じる。適合テストの一意性要求を確率的に安全な余裕で満たす
- トレードオフ: 厳密な一意性は保証しない（同秒内 36^8 の衝突確率）。仕様どおり `TaskRepository::create` の `Conflict` がバックストップになり、ユースケースが1回だけ再発行する。衝突経路のテストは実アダプターでは作れないため、テストダブルで消化する（ADR-028）

---

## ADR-027: ポート適合テストはマクロで1ケース1テストに展開し、ハーネスのフックは意図レベルにする

### Status
Accepted（ステップ9で確定。`.adr/027-port-conformance-suite-and-harness-hooks.md`。125行の対応表を埋めた結果の差分は ADR-041）

### Context

spec/testcases/ports/*.md は「すべてのアダプター実装が共通で通す」スイートとして書かれており、本スライスの fs / システム実装だけでなく、後続スライスの in-memory 実装や別プラットフォーム実装にも同じ検証を適用したい。一方、Rust の `#[test]` はクレート内に静的に置く必要がある。

当初はハーネスに `put_raw(area, id, content: &str)` / `read_raw(...)` を持たせ、ケース関数が JSON 文字列を組み立てる形にしていた。しかしこれだと破損系ケース（TC-port-task-repository-018・020〜030 等 約12件 — 最も価値のある部分）が fs 実装専用のコードになり、「後続の in-memory 実装にも同じスイートを適用できる」という本 ADR の狙いが破れる。

### Decision

- `pulsen-conformance` にポートごとの `Harness` トレイトとケース関数（1関数 = spec の表の1行）を置く。
- `#[macro_export]` の宣言的マクロが、与えられたセットアップ式に対して各ケースの `#[test]` 関数を生成する。アダプター側のテストファイルはマクロ呼び出し1行で済む。
- **ハーネスのフックは「破損・状況の意味」だけを受け取る**。実現方法（生 JSON の配置・権限操作・プロセス起動）は各ハーネス実装の内側に閉じる。生の文字列を受け渡す `put_raw` / `read_raw` はスイートの API から外す。
- **フックの一覧は spec/testcases/ports/\*.md の前提条件から導く**。フックの粒度をハーネス設計者の裁量に委ねると、ケースを書く段になって毎回フックを後付け拡張することになり、枠組みを先に確定する意味が失われる。そこで**ステップ9の完了条件を「125行 × フックの対応表を埋めきること」とする** — 各行に「ポートのメソッドだけで組める / このフックで組める / スキップ可（spec が『再現できるアダプター環境に限る』と明示する行のみ）」のいずれかを記入し、埋まらない行が残ったらフックを足す。

  spec と突き合わせて確定したフックは次のとおり。

  | ポート | フック |
  |---|---|
  | TaskRepository | `corrupt_whole_record(area, id)` / `break_task_field(area, id)`（有効 JSON のままタスク側の値制約を破る。TC-021）/ `corrupt_snapshot(area, id)` / `drop_snapshot_field(area, id)` / `set_task_status_outside_snapshot(area, id, name)` / `break_snapshot_invariant(area, id)` / `place_in_both_areas(id)` / `put_unnamed_entry(area)` / `record_bytes(area, id)`（レコード全体の不変を観測。TC-004）/ `snapshot_bytes(area, id)`（破損スナップショットの温存を観測。TC-009）/ `make_unreadable(area)` / `make_unwritable(area)` / `concurrent_repo()`（下記） |
  | ConfigStore | `put_config(text)` / `remove_config()`（TC-013）/ `home_path()`（`NotFound` が解決後のホームパスを含むことの期待値。TC-013）/ `make_unreadable()` |
  | WorkflowStore | `put_named(name, text)`（`workflows/<name>.yaml`。上書きも兼ねる）/ `put_named_with_ext(name, ext, text)`（`.yml` のみの配置。TC-002）/ `expected_path_for_name(name)`（`NotFound` の `attempted` の期待値。TC-002/003/006）/ `put_at_absolute(text) -> PathBuf`（TC-004）/ `put_at_relative(text) -> (相対, 絶対)`（TC-005。相対の基準がハーネス側にあることを型で示す。ADR-030）/ `missing_absolute_path()`（TC-006）/ `make_unreadable(name)` |
  | ExclusiveLock | `hold_from_other_process() -> Option<Holder>` / `kill_holder(h)` / `release_holder(h)` / `try_acquire_from_other_process() -> Option<bool>`（TC-004/005。`None` = 別プロセスを扱えない実装、`Some(false)` = 競合。両者を混同すると TC-004 がスキップに落ちず FAIL する）/ `separate_home()` / `break_lock_location()` |
  | WorktreeManager | `repo_with_commit()` / `repo_without_commit()` / `detached_repo()` / `non_repo_dir()` / `missing_path()`（TC-002。スイートがパスを捏造すると in-memory 実装で意味を持たない）/ `head_branch_name()`（TC-004/007/008 の期待値と不在ブランチ名）/ `failing_manager()`（TC-009。下記） |
  | Clock | `observe_wall_clock()`（TC-003。システム時計を参照する実装が外部から実時刻を観測する口）/ `advance()`（TC-004。「時刻が確実に前進した状態にする」。**実時間を待つ実装を含む**）/ `rewind()`（TC-005）。RFC3339 往復は `Timestamp` 自身が持つため変換フックは要らない（ADR-020） |

- すべてのフックは既定実装が `None` を返し、スイートはスキップして理由を出力する（「再現できるアダプター環境に限る」ケースと同じ扱い）。
- **権限操作系のフック（`make_unreadable` / `make_unwritable`）は、制限が実際に効いたことを確認してから `Some` を返す**。`chmod 000` は root では効かないため、確認せずに `Some(Restore)` を返すと、`Err(Io)` を期待するケースが `Ok` を観測して**スキップに落ちずに FAIL する**。実装規則は「制限を掛ける → 実際に読み（書き）を試す → 通ってしまったら復元して `None` を返す」。root 実行・Windows・特殊なファイルシステムのすべてをこの1つの規則で吸収でき、round-2 で `try_acquire_from_other_process` について定めた「未対応とスキップを区別する」原則と同じ形になる。
- **「対象を壊すフック」ではなく「壊れた対象を別ハンドルとして返すフック」を既定の形にする**。ハーネスは対象をアクセサ（`fn repo(&self) -> &Self::Repo` 等）越しに共有参照で渡すため、構築時に注入した値がイミュータブルなアダプター（`GitCliWorktreeManager::new(git_program)`・ADR-024）を後から壊すには、本番アダプターにテスト専用の内部可変性を持ち込むしかなくなる（Issue の「スタブ・仮実装不可」と CLAUDE.md「共有可変状態を持たない」に反する）。したがって WorktreeManager の TC-009 は

  ```rust
  /// 3メソッドとも Err(Failed) を返す状態の実装を返す。提供できなければ None（スキップ）
  fn failing_manager(&self) -> Option<&Self::Manager> { None }
  ```

  とし、git ハーネスは「存在しないパスを `git_program` として構築した2つ目の `GitCliWorktreeManager`」を保持するだけでよい（シム・ファイル操作・権限操作が要らず、Windows でも成立する）。`concurrent_repo` と同じ型の、スキップ可能な別ハンドルフックである。
- **原子性の観測面（TC-port-task-repository-042〜044）だけを `Sync` 境界から隔離する**。当該3ケースは「別スレッド/プロセスから `find` / `list_active` を繰り返し呼び続けている」状態を前提にしており、`std::thread::scope` で書くには `&Repo: Send`（= `Repo: Sync`）が要る（実測: 境界なしでは `E0277` でコンパイルできない）。しかし `Harness::Repo: Sync` を無条件に置くと、素直な in-memory 実装（`RefCell` ベース）が `Sync` を満たさずスイート全体が適用不能になり、本 ADR の狙いが壊れる。そこで

  ```rust
  fn concurrent_repo(&self) -> Option<&(dyn TaskRepository + Sync)> { None }
  ```

  という**スキップ可能なフック**を1つ置き、3ケースはこのハンドル越しにのみ読み書きする。他の41ケースは `Sync` を要求しない。`TaskRepository` の7メソッドはいずれもジェネリックでないため dyn 互換であり、この形が成立することは実測で確認した。

### 検討した代替案

- スイートを `Vec<(&str, fn(&H))>` として返し、1つの `#[test]` でループする — 失敗したケース名が1テストの中に埋もれ、どのケースが落ちたかが `cargo test` の出力から一目で分からない
- ケースを各アダプターのテストにコピーする — spec の1行が複数箇所に散り、後続スライスで乖離する
- `Harness::Repo: Sync` を無条件の境界にする — 3ケースは素直に書けるが、`RefCell` ベースの in-memory 実装がスイートを一切適用できなくなる

### Consequences

- 良い点: `cargo test` の出力が spec の行と1:1 で対応し、Issue のチェックリスト消化を機械的に確認できる。破損系ケースが実装非依存になり、後続スライスの in-memory 実装がフックを実装するだけで同じ125件を通せる。フックの粒度が spec 由来であることが対応表で構造的に担保される
- トレードオフ: マクロが長くなる（ケース追加のたびにマクロにも1行足す）。ハーネスのフックが増える。原子性の3ケースは `Sync` な実装でしか走らない（fs 実装では走る）

---

## ADR-028: ユースケースの異常系はテストダブルで消化し、ダブルは `pulsen-conformance` に置く

### Status
Proposed

### Context

TC-task-register-task の次の5行は、実アダプター（乱数 ID・std のファイルロック・git CLI・実ファイルシステム）を使う限り外から状況を作れない。

- TC-012「ID発行が既存タスクと衝突する」/ TC-047「ID再発行後もIDが衝突する」— `DefaultTaskIdGenerator` の出力値を外から一致させられない
- TC-018「ロック機構自体が異常（`LockError::Failed`）」
- TC-040「対象検証の git 操作自体が失敗する（`TargetError::Failed`）」
- TC-048「タスクファイルの作成が I/O エラー」

Issue の完了条件は「スタブ・仮実装・部分実装は不可。実装をレビューで確認できた行にのみチェックを付ける」であり、これらの行をテストなしで残せない。加えて CLAUDE.md は「テストでは実アダプターを差し替えられることを設計の健全性の指標とする」と定めており、walking skeleton である本スライスに差し替え実装が1つも無いと、その指標が一度も行使されないまま以降10スライスの土台が確定する。

選択肢は2つあった。

- (a) 後続スライスでも使うテスト用のポート実装を本スライスの実装ステップに正式に組み込む
- (b) 適合テストのハーネスに障害注入の口を設け、実アダプターを壊して再現する

### Decision

**(a) を採る。**

(b) を採らない理由:

1. TC-012/047 は「発行される ID の値を外から決める」ことが要求であり、障害注入では表現できない（生成器そのものを差し替えるほかない）
2. 障害注入の口を実アダプターの本番コードに開けると、テスト専用の分岐が本番コードに入り、Issue の「スタブ・仮実装不可」と衝突する
3. ハーネスのフック（ADR-027）は**ポートの契約を検証する**ためのものであり、ユースケースの分岐網羅とは目的が異なる。同じ口に両方の役割を負わせると、どちらの意図で追加されたフックか判別できなくなる

具体:

- ダブルは `pulsen-conformance` の `doubles` モジュールに置く。`pulsen-domain` のみに依存し、`pulsen` からは `[dev-dependencies]` 経由でのみ到達する（本番バイナリには載らない。ADR-019）
- 範囲は「適合テストと add の異常系検証に必要なポートに限る」。**スクリプト式**（呼び出しごとに返す結果をあらかじめ与える）とし、汎用の in-memory ストアは作らない
  - `ScriptedTaskIdGenerator` — 発行する ID 列を与える
  - `ScriptedExclusiveLock` — `Ok(Some)` / `Ok(None)` / `Err(Failed)`
  - `ScriptedWorktreeManager` — 3メソッドの結果（`TargetError` 5分岐を含む）
  - `ScriptedTaskRepository` — `create` の結果（`Ok` / `Conflict` / `Io`）
  - `FixedClock` — 固定時刻
  - `ScriptedWorkflowStore` — `load` の結果（`RegisterTask` が `WorkflowStore` ポートを受け取るため必要）
  - **`ScriptedConfigStore` は置かない** — `RegisterTask` は config を「読み込み済みの `GlobalConfig`」として受け取り、`ConfigStore` ポートを引数に取らない（読み込みは合成ルート `cli::wire`）。適合テストは実アダプター `FsConfigStore` に対して書く。よって消費者が存在せず、本 ADR の「add の異常系検証に必要なポートに限る」という自己制約から外れる
- **全44ケースを通す in-memory `TaskRepository` は作らない** — 後続スライスの範囲（plan.md スコープ「含まれないもの」を維持）
- `RegisterTask` はポートをジェネリック引数で受け取り、合成ルート（`cli`）で実アダプターを結線する。ユースケースのテストはすべてダブルで書き、実プロセス・実ファイルシステムを使わない（plan.md のテスト方針「ドメイン・ユースケースのテストに実プロセスは使わない」と整合させる）。実アダプターとの結線は CLI 受け入れテストが検証する

### Consequences

- 良い点: 実行不能な TC が消える。ポートの引数・戻り値が本当に差し替え可能かを本スライスで一度検証でき、後続スライスがトレイトを流用する前に設計の誤りが露出する
- トレードオフ: ダブルの記述量が増える。後続スライスで in-memory 実装を作るときも、スクリプト式ダブルとは別物として共存する（用途が違う: 分岐網羅 vs 契約適合）

---

## ADR-029: `clippy::wildcard_enum_match_arm` はドメインクレートにのみ適用する

### Status
Proposed

### Context

CLAUDE.md は「`match` でワイルドカード（`_`）を避ける」と定め、当初は workspace lints に `clippy::wildcard_enum_match_arm` を warn として設定する方針だった。しかし AC-1 が `cargo clippy -- -D warnings` の通過を求めるため、warn 設定は実質 deny になる。

実測で、この lint は `#[non_exhaustive]` な外部 enum の `_` アームにも発火することを確認した（`std::io::ErrorKind` の `_` に対して既知の全バリアント列挙を提案する。網羅 match は**そもそも書けない**）。同じことが `std::fs::TryLockError` / `serde_yaml_ng::Value` / `serde_json::Value` にも当てはまる。

### Decision

`crates/pulsen-domain/Cargo.toml` の `[lints.clippy]` にだけ設定する。workspace lints には全クレート共通のもの（`unsafe_code = "forbid"` 等）だけを置く。`pulsen` クレートには掛けない。

### Consequences

- 良い点: CLAUDE.md の規約が本来の適用対象（ドメインの網羅 match）で強制され、外部 enum を扱うアダプターに `#[allow]` を撒かずに済む
- トレードオフ: `pulsen` クレート内のドメイン enum への `_` は lint で捕まらない。レビューで見る

---

## ADR-030: `FsWorkflowStore` は基準ディレクトリを注入して相対パスを解決する

### Status
Proposed

### Context

spec/testcases/ports/workflow-store.md は「相対パスはプロセスのカレントディレクトリから解決される」と定める。アダプターが `std::env::current_dir()` を直接読む実装にすると、TC-port-workflow-store-005 を in-process の適合テストで検証するのに `std::env::set_current_dir` が必要になる。これはプロセス全体の可変状態であり、`cargo test` の既定（マルチスレッド並列）では他テストのパス解決を壊す。

### Decision

`FsWorkflowStore::new(workflows_dir: PathBuf, base_dir: PathBuf)` の形で基準ディレクトリを構築時に受け取り、`WorkflowRef::Path(p)` が相対なら `base_dir` で絶対化する。合成ルート（`cli`）が `std::env::current_dir()` を渡すことで spec の契約はそのまま保たれる。`--repo` の相対パスも同じ理由で合成ルートで絶対化してからユースケースに渡す。cwd を読むのは合成ルートの1箇所だけにする。

### Consequences

- 良い点: 「グローバル可変状態に依存しない自己完結した値」（CLAUDE.md）になり、適合テストが cwd を触らずに並列実行できる
- トレードオフ: 構築時の引数が1つ増える。「カレントディレクトリから解決」という契約が合成ルートの結線に依存するため、その結線を CLI 受け入れテスト（相対 `--repo` / 相対 `--workflow`）で確認する

---

## ADR-031: グローバルホームのレイアウトはアプリケーション層に置く

### Status
Proposed

### Context

当初は `PulsenHome`（`config_path` / `workflows_dir` / `state_root` / `worktree_root` / `lock_path` の導出）を `adapter::home` に置き、`application::context` がそれを import する構成だった。これはアプリケーション層がアダプターを直接参照することになり、依存が内 → 外になる。ADR-019 がクレート境界で守ろうとしている性質がクレート内で緩む。

### Decision

- `PulsenHome` は `application::home` に置く。ホームのレイアウトは「配線情報」であってアダプターの実装詳細ではない。
- アダプターは導出済みのパス（`StateRoot` / `workflows_dir` / `lock_path` 等）を構築時に受け取るだけにする。
- ホームの**解決**（`--home` > `PULSEN_HOME` > 既定 `~/.pulsen/`）は `cli` の合成ルートが行い、`std::env::home_dir()` / `std::env::var` を読むのもそこ1箇所にする。
- ドメイン側で `StateRoot` / `WorktreeRoot` / `RunDirPath::derive` / `TaskFilePath::active|archived` がレイアウト導出を持つ設計（spec/domains/task.md「アダプターも同じ導出を使い、ポートの外にレイアウト知識を漏らさない」）と一貫させる。

### Consequences

- 良い点: `application` の import が `pulsen_domain` + std だけになり、依存方向がクレート内でも保たれる
- トレードオフ: 合成ルート（`cli`）の結線コードが少し厚くなる

---

## ADR-032: ロックの別プロセスフィクスチャは `examples/lock_holder.rs` で供給する

### Status
Proposed

### Context

spec/testcases/ports/exclusive-lock.md は「ロックを取得・保持する別プロセス」「ロックを保持したまま強制終了できるテスト用プロセス」をフィクスチャとして明示的に要求する（TC-002〜005）。本スライスのバイナリは `pulsen`（サブコマンドは `add` のみ）だけで、ロックを保持し続けるモードを持たない。std の `File::try_lock` は同一プロセス内の再取得の挙動が未規定なので、同一プロセスでの代替も取れない（spec も「同一プロセス内の再取得では検証しない」と明記）。

### Decision

- `crates/pulsen/examples/lock_holder.rs` を用意する。引数でロックファイルのパスを受け取り、取得できたら `locked` を stdout に1行書いてフラッシュし、標準入力が閉じるまで保持し続ける。
- 統合テストは `env!("CARGO_BIN_EXE_pulsen")` の親ディレクトリ配下 `examples/lock_holder` として実行ファイルを解決する（`cargo test` は example もビルドするため常に存在する）。強制終了は `Child::kill()`。
- **実測で確認済み**: 保持中の別ハンドルからの `try_lock` は `TryLockError::WouldBlock`、保持プロセスを `kill` した直後は `Ok(())`。TC-002〜005 がこの1つのフィクスチャで書ける。
- `LockError::Failed`（TC-port-exclusive-lock-007 / TC-task-register-task-018）の再現は、**ロックファイルのパスにディレクトリを置く**（実測: `IsADirectory` で `open` が失敗する）。権限操作と違って root 実行でも Windows でも成立するため、環境依存スキップにしない。
- サブコマンドとしてロック保持モードを足すことはしない（利用者に見えるインターフェースを増やさない）。

### Consequences

- 良い点: 追加の bin ターゲットを増やさずに済み、`cargo install` の成果物が `pulsen` 1つのままになる。`LockError::Failed` が環境非依存に再現できる
- トレードオフ: example の出力パスがビルドレイアウトに依存する（`target/<profile>/examples/`）。パス解決を1箇所のヘルパーに閉じる

---

## ADR-033: git フィクスチャは環境変数と初期化オプションで再現性を固定する

### Status
Proposed

### Context

`git init` した一時リポジトリをフィクスチャにするテスト（worktree-manager 適合テスト・CLI 受け入れテスト）は、環境で結果が変わる。

- コミットを作るには `user.name` / `user.email` が要る
- 既定ブランチ名は `init.defaultBranch` やユーザーのグローバル設定で変わる（テストが `main` を前提に書かれていると落ちる）
- `rev-parse --show-toplevel` は上位ディレクトリを遡るため、TMPDIR がたまたま git リポジトリ配下にあると「git リポジトリでないディレクトリ」のフィクスチャが成立しない（実測確認）

### Decision

テストのフィクスチャ生成ヘルパーで次を固定する（本番コードには入れない。ADR-024）。

- `git init -b main <dir>` で既定ブランチ名を明示する
- 起動する git の環境に `GIT_CONFIG_GLOBAL=/dev/null`（Windows では `NUL`）・`GIT_CONFIG_SYSTEM` を設定し、`GIT_DIR` / `GIT_WORK_TREE` / `GIT_INDEX_FILE` を除去する
- コミットは `git -c user.name=pulsen-test -c user.email=pulsen-test@example.invalid commit` で作る
- 「git リポジトリでないディレクトリ」のフィクスチャは、生成直後に `git -C <dir> rev-parse --show-toplevel` が**失敗する**ことを確認してから使う。成功する（= TMPDIR 自体がリポジトリ配下）場合はそのケースをスキップする

### Consequences

- 良い点: 開発者の git 設定・TMPDIR の位置に関わらずテストが決定的になる
- トレードオフ: フィクスチャヘルパーが少し厚くなる。スキップ条件が1つ増える

---

## ADR-034: `WorkflowRef` のパス区切り文字集合を定数として切り出す

### Status
Proposed

### Context

spec/domains/definition.md は `WorkflowRef::parse` の規則を「値がパス区切り文字（`/`、Windows では `\` も）を含むか、`.yaml` / `.yml` で終わる場合は `Path`」と定める。素直に書くと `#[cfg(windows)]` がドメインに入るが、CLAUDE.md は「OS依存の処理はアダプター層に隔離する」と定めている。`#[cfg]` をドメインに置くと、ドメインのユニットテストがプラットフォームで結果を変え、Linux CI では `\` を含むケースが一度も検証されない。

### Decision

区切り文字の集合を定数として切り出し、`parse` は集合を参照する純粋関数にする。プラットフォーム既定の集合（`PLATFORM_SEPARATORS`）は `#[cfg]` で選ぶが、判定ロジック自体は集合を引数に取る内部関数に閉じ、ユニットテストは `&['/']` と `&['/', '\\']` の両方を明示的に渡して検証する。

### Consequences

- 良い点: `\` を含むケースが全プラットフォームで検証される。ドメインの判定ロジックが純粋関数のまま保たれる
- トレードオフ: 定数の選択に `#[cfg]` が1箇所残る（データの選択であって処理の分岐ではない）

---

## ADR-035: 本スライスの ADR を `.adr/019` 以降として正本に起票する

### Status
Proposed

### Context

`.adr/` はプロジェクトの正本として18件で運用されている。本書の決定のうち、クレート構成（ADR-019）・ドメインに serde を持たせない／`Timestamp` の境界（ADR-020）・タスクファイルの物理形式とスナップショット温存（ADR-025）・タスクID形式（ADR-026）・適合テストの枠組み（ADR-027）・テストダブルの置き場（ADR-028）は、Issue #1 限りの判断ではなく以降10前後のスライス全部を縛る。`.thread/1/` にだけ残すと、後続スライスの担当が根拠を辿れない。

### Decision

- 本書の採番を `.adr/` の続き（019 以降）に合わせる。本文中の ADR-001〜018 は既存の正本を指す。
- 実装ステップ1で `.adr/019-*.md` 〜 `.adr/036-*.md` として起票する。ただし **Status は `Proposed` で起票し、その決定の形が確定するステップの完了時に `Accepted` へ更新する**。実装前に18件すべてを `Accepted` にすると、正本と実装が食い違う期間が最大化する（ADR-025・027 のように実装で形が変わりうる決定を含むため）。
- 確定ステップの対応:

  | ADR | 確定するステップ |
  |---|---|
  | 019・022・023・029・035 | 1（構成そのものがステップ1の成果物） |
  | 020・034 | 3・5（`WorkflowRef` / `Timestamp` の実装） |
  | 021 | 10（`adapter::yaml`） |
  | 027 | 9（ハーネスの形と125行×フックの対応表） |
  | 030 | 11（`FsWorkflowStore`） |
  | 025 | 12（DTO と復号の分類） |
  | 026・036 | 13（`DefaultTaskIdGenerator` / `SystemClock`） |
  | 024・032・033 | 14（git / ロックのアダプターとフィクスチャ） |
  | 028 | 15・16（ダブルとユースケーステスト） |
  | 031 | 16（`application::home`） |

- ADR-025 が spec/testcases/ports/task-repository.md の語（「構文不正」）と食い違う点は、Issue のコメントで spec 側の追従を提起する。**この提起はステップ12の成果物に含める**（分類を実装して境界が確定した時点で書く）。

### Consequences

- 良い点: 後続スライスが `.adr/` だけを見れば土台の判断を辿れる。番号の衝突が構造的に起きない。`Proposed` / `Accepted` が「実装で裏が取れたか」を表すため、正本の信頼性が保たれる
- トレードオフ: ステップ1の成果物が増える（18ファイル）。Status の更新が各ステップに1行ずつ乗る

---

## ADR-036: 無謬なポートの実装が持つ失敗は、構築時か値への写像で吸収する

### Status
Proposed

### Context

spec のポートのうち `TaskIdGenerator::generate(&self) -> TaskId` と `Clock::now(&self) -> Timestamp` は**エラーを持たない**。一方、実装に使う手段は失敗しうる。

- `getrandom` は `Result` を返す（エントロピー源が使えない場合）
- `SystemTime::now().duration_since(UNIX_EPOCH)` は `Result` を返す（時計が epoch より前の場合）
- `Timestamp` の表現可能範囲は 0001-01-01〜9999-12-31 に閉じている（ADR-020）

素直に書くと呼び出しごとに `unwrap` が入る。CLAUDE.md は「パニックは不変条件違反にのみ使う」と定めており、どちらも不変条件違反ではなく実行環境の異常なので、`unwrap` は規約違反になる。ポートにエラーを足すのは spec からの逸脱であり、無謬であること自体はドメイン側の設計（時刻・ID の取得で分岐を作らない）として意図されたものなので、変えない。

### Decision

失敗を**呼び出し時から追い出す**。

- `DefaultTaskIdGenerator::new(clock) -> Result<Self, IdGeneratorInitError>`: 構築時に一度だけ `getrandom` からシードを取り、以降は内部 PRNG（`Cell<u64>` の SplitMix64 相当）で進める。時刻成分の取得元となる `Clock` も構築時に受け取る（ADR-026）。失敗は合成ルート `cli::wire` が実行環境エラーとして扱う。`generate` は無謬になる。
- `SystemClock::now`: `duration_since(UNIX_EPOCH)` の `Err` を **epoch 前の符号付き秒**に写す（`Err(e)` は `e.duration()` で epoch からの差を持つので `-secs` にできる）。パニックしない。
- 表現可能範囲外（西暦1年より前・9999年より後）の壁時計は、範囲の**端に飽和させる**。ここは「起こったら他の何もかもが壊れている」領域であり、値を返して処理を続けるほうが、パニックで tick を落とすより縮退設計（pages）に沿う。

### Consequences

- 良い点: `unwrap` が実装から消え、ポートの無謬性が spec のまま保たれる。エントロピー取得が1回に減る（ID発行のたびのシステムコールが無くなる）
- トレードオフ: 内部 PRNG を自前で持つ（数行。一意性は「同秒内 36^8」の確率的なものであり、`create` の `Conflict` がバックストップである点も変わらない。桁数の根拠は ADR-026）。飽和は観測不能な領域なので適合テストでは検証しない

---

## ADR-037: プラットフォーム既定のパス区切り集合は `#[cfg]` ではなく `MAIN_SEPARATOR` から選ぶ

### Status
Accepted（ステップ3で確定。`.adr/037-platform-separator-set-without-cfg.md`）

### Context

ADR-034 は「プラットフォーム既定の集合（`PLATFORM_SEPARATORS`）は `#[cfg]` で選ぶ」としていたが、plan.md の AC-1 は「`#[cfg(unix)]` / `#[cfg(windows)]` の出現箇所が `crates/pulsen/src/{adapter,util}/` 配下に限られ、`crates/pulsen-domain/` には1つも現れないことを grep で確認できる」を要求する。ドメインに `#[cfg(windows)]` を1つ置くと AC-1 が成立しない。

### Decision

既定集合の選択を `std::path::MAIN_SEPARATOR` の比較で行う（`pub const fn platform_separators()`）。`MAIN_SEPARATOR` は std の const であり、分岐はコンパイル時に畳まれる。判定ロジックは引き続き集合を引数に取る純粋関数（`WorkflowRef::parse_with_separators`）に閉じ、ユニットテストは `&['/']` と `&['/', '\\']` の両方を明示的に渡す。ADR-034 の狙い（集合はデータ・判定は純粋関数）はそのまま保たれる。

### Consequences

- 良い点: ドメインクレートに条件付きコンパイルが1つも現れず、AC-1 の grep が成立する
- トレードオフ: 既定集合が「区切り文字が `\` かどうか」という間接的な条件で決まる

---

## ADR-038: `.adr/` の起票は正本の既存フォーマットに合わせる

### Status
Accepted（ステップ1で確定）

### Context

本書のエントリは `### Status` / `Proposed` / `Accepted` という見出しと語で書かれているが、`.adr/001`〜`018` は `## ステータス` / `承認済み` という日本語の見出しと語で運用されている。正本に2つの書式が混在すると、ステータスの機械的な確認（どの決定が実装で裏を取られたか）が書式ごとに分かれる。

### Decision

`.adr/019` 以降も既存の書式（`## ステータス` / `## コンテキスト` / `## 決定` / `## 検討した代替案` / `## 影響`）で起票し、ステータスの語は `Proposed` = **提案中**、`Accepted` = **承認済み** と対応させる。ADR-035 の運用（確定したステップで承認済みへ更新する）はそのまま適用する。

### Consequences

- 良い点: `.adr/` 全体で見出しとステータスの語が1つに揃い、`grep -l 承認済み .adr/*.md` が全件に効く
- トレードオフ: 本書（`.thread/1/adr.md`）と正本で語が異なるため、対応を本エントリで明示しておく必要がある

---

## ADR-039: 走査系ポートメソッドの読み取りエラーは `ReadError` に統一する

### Status
Accepted（ステップ7で確定。`.adr/039-read-error-shared-by-find-and-list.md`）

### Context

spec/domains/task.md のポート表は `find` の失敗を `ReadError`（`Io` のみ）とし、`list_active` / `list_archived` の失敗を `Io` と書いている。`Io` という独立した型は spec のどこにも定義がなく、spec/inventory/domain.md にも `ReadError`（DOM-task-068）だけが行として立っている。素直に読むと選択肢は2つある。

- `ListError { Io { message } }` を別に定義する — 台帳に無い型が1つ増え、`ReadError` との違いが説明できない
- `ReadError` を3メソッドで共有する

### Decision

`find` / `list_active` / `list_archived` の3メソッドがいずれも `ReadError` を返す。`ReadError` は `Io { message }` の1種のみで、個別のタスクファイルの破損はエラーではなく結果の値（`TaskLookup::Corrupt` / `TaskEntry::Corrupt`）として返る、という契約をドキュメンテーションコメントに書く。

### Consequences

- 良い点: 台帳（DOM-task-068）と1対1のまま、読み取り経路のエラーが1つの型に閉じる。アダプターと適合テストが `Io` の分岐を1箇所しか持たない
- トレードオフ: spec のポート表の綴り（`Io`）とは字面が一致しない。表の意図（読み取り失敗は入出力エラーだけ）は保たれる

---

## ADR-040: 永続化からの再構築は「フィールド束の struct」を入力に取る

### Status
Accepted（ステップ6で確定。`.adr/040-rehydrate-takes-field-bundle.md`）

### Context

`Task::rehydrate` は spec 上「全フィールド」を受け取る唯一の再構築経路であり、フィールドは11個ある。位置引数で並べると呼び出し側（TaskRepository アダプターの復号、適合テストのフィクスチャ）が型の同じ `Option` を取り違えても気づけず、フィールドが増えるたびに全呼び出しの引数順が壊れる。`AttemptRef` も spec では「`record_launching` の内部でのみ生成される」と定められており、そのままでは永続化から組み直せない。

### Decision

- `Task::rehydrate(fields: TaskFields)` / `DegradedTask::rehydrate(fields: DegradedTaskFields)` の形にする。`TaskFields` / `DegradedTaskFields` は公開フィールドの struct で、既存の `GlobalConfigInput`（definition ドメイン）と同じ「境界で一度だけ検証する入力の束」の型である。検証（不変条件1）は `Task::rehydrate` が行い、`TaskFields` 自体は不変条件を持たない。
- `AttemptRef::rehydrate` / `RetryCounters::rehydrate` を公開の再構築コンストラクタとして置く。新規採番（`record_launching` が `RunDirPath::derive` で番号とパスの整合を構成で保証する）は後続スライスの責務で、再構築経路はそれと別の口にする。

### Consequences

- 良い点: フィールドの追加が呼び出し側の引数順に波及しない。適合テストのフィクスチャが名前つきで組め、6状態・全 Optional フィールドの組み合わせを網羅できる
- トレードオフ: 束の struct が2つ増え、`Task` のフィールドと二重に並ぶ。生成経路が `register` / `rehydrate` の2つだけである点は変わらない

---

## ADR-041: 125行の対応表を埋めた結果に合わせてハーネスのフックとマクロの置き場を確定する

### Status
Accepted（ステップ9で確定。`.adr/027-port-conformance-suite-and-harness-hooks.md` に反映済み）

### Context

ADR-027 はフックの一覧を spec の前提条件から導くと定め、その検証をステップ9の対応表（`crates/pulsen-conformance/HOOKS.md`）に委ねた。125行を実際に1行ずつ割り当てると、ADR-027 が先に挙げた一覧との差が3点、マクロの置き方に判断が1点出た。

### Decision

- **`TaskIdGeneratorHarness::another_generator` を足す**。ADR-027 の表は TaskIdGenerator の行を持たなかったが、TC-port-task-id-generator-004「同じ構成のジェネレーターを複数用意する」はポートのメソッドだけでは組めない（スイートは対象の構築方法を知らない）。`concurrent_repo` / `failing_manager` と同じ「別ハンドルを返すスキップ可能フック」にする。
- **`WorktreeManagerHarness::absent_branch_name` を `head_branch_name` から分ける**。ADR-027 は「TC-004/007/008 の期待値と不在ブランチ名」を1つのフックに束ねていたが、返す値の意味（HEAD が指すブランチ / 存在しないブランチ）が違い、片方だけ供給できる実装もありうる。1フック1意味にする。
- **ExclusiveLock の `break_lock_location` を `unusable_lock() -> Option<&Self::Lock>` にする**。ADR-027 が「対象を壊すのではなく壊れた対象を別ハンドルとして返す」を既定の形と定めた以上、ロック機構が使えない状況も別ハンドルで表すのが一貫する。ハーネスは置き場を用意できないロック（ADR-032 のディレクトリを置いたパス）で構築した2つ目のハンドルを保持するだけでよく、共有参照で渡した対象を後から壊す必要がない。
- **並行の前提を持つのは TC-port-task-repository-042・044 の2件**とし、`concurrent_repo` はその2件だけが使う。043「`save` が `Err` を返した後に部分的な書き込み結果が残らない」は spec の前提条件に並行読み取りを含まないため、`repo()` で書ける。ADR-027 の「3ケース」は原子性の節の行数を指していた。
- **スイート適用のマクロはケース関数と同じモジュールに置き、ケースが無いうちは置かない**。ステップ9の成果物は、`#[test]` の生成・ハーネスの構築・スキップ報告を担う `conformance_cases!` と、フックの欠落でケースを畳む `require!` の2つにする。ポートごとの `<port>_conformance!` は、ケース関数を足すステップ（10〜14）で同じファイルに定義する。空のケース列を持つマクロを先に置くと、呼び出しても1件もテストが生成されないことが呼び出し側から見えず、「適用したのに何も検証していない」状態を作れてしまう。

### 検討した代替案

- ポートごとのマクロを空の本体で先に置く — ステップ9で「1行で適用できる」形は見えるが、上記のとおり無言で0件になる口を残す。`conformance_cases!` をクレート内のテストで実際に展開して検証する方が、枠組みの動作確認としても強い
- `unusable_lock` を `break_lock_location(&self) -> Option<Restore>` のままにする — ロックの置き場を壊す操作は対象のハンドルに影響するため、対象がイミュータブルであるという前提を崩す

### Consequences

- 良い点: 125行すべてが「ポートのみ28件 / フック86件 / spec が明示するスキップ可11件」のいずれかに埋まり、フックが spec 由来であることを対応表で確認できる。フックの形が「対象アクセサ + 意味だけを受け取るフック + 別ハンドル」の3種に揃う
- トレードオフ: ステップ10〜14 はケース関数と同時にポートごとのマクロも書く（1ポートにつき1回）。ADR-027 の一覧と実際のフックの差分をこのエントリで辿る必要がある
