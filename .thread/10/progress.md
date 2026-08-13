# 進捗メモ — Issue #10

`.github/workflows/ci.yml` を新規追加し、3ランナーで実行して**全7ジョブが緑**になった（run 31665658371、コミット `e19c973` = 現在の HEAD）。steps.md のステップ1〜11 が完了し、ステップ12 の機械確認と、PR 本文・Issue #10・PR #11 への記録も済んでいる。

## CI の実測結果

| ジョブ | ubuntu | macOS | Windows |
|---|---|---|---|
| `fmt` | 緑 | — | — |
| `test`（build / test / clippy） | 緑 | 緑 | 緑 |
| `msrv`（`cargo build --all-targets`） | 緑 | 緑 | 緑 |

- **MSRV 1.89 は宣言だけの状態を脱した。** `Cargo.toml` の `workspace.package.rust-version` を読み出した toolchain で、3 OS すべてがリンクまで通る。
- **Windows で初めて build / test / clippy の結果が得られた。** 初回実行では `-p pulsen --lib` の6件が落ち、2回目で全緑。
- **fmt は初回から緑。** nixpkgs の rustfmt 1.97.1 と CI の現行 stable で整形結果が食い違わなかったため、steps.md ステップ10 の rustfmt 掛け直しは発火していない。
- **clippy も初回から緑。** `cfg(not(unix))` 側の未 lint コードから新規指摘が出る想定（plan.md の [高] リスク）だったが、実際には出なかった。

## 実測に使った run

AC-2 / AC-6 / AC-8 の出典は、**最後にコードまたはワークフローが変わったコミットに対する実測**とする。ドキュメントだけを直したコミットで取り直した run に出典を貼り替えても、読み手が知りたい「この実測がどの実装に対するものか」は変わらないので追いかけない。それ以前の run も残すのは、どの変更がどの実測に載ったかを PR の記録から辿れるようにするため。

| run | コミット | 結果 | この run で初めて回った変更 |
|---|---|---|---|
| 31656955322 | `09e282c` | Windows の stable が赤 | ワークフローの追加のみ（`-p pulsen --lib` の6件で停止） |
| 31657976822 | `af24360` | 全7ジョブ緑 | フィクスチャの可搬化（ADR-011）と `write_atomic` の再試行（ADR-010・ADR-012） |
| 31661056619 | `2c99c1b` | 全7ジョブ緑 | 前提検査への `id` 追加 / 非 root アサートの fail-open 解消 / `--no-fail-fast` / 除外件数の表示 / MSRV の `env` 経由 / 読み取りを `read_atomic` に通す（ADR-013） |
| 31662960664 | `9675b2f` | 全7ジョブ緑 | SKIP の抽出を、進捗行に連結された行にも当たる形に直した。再試行のテストが本番の配線を通るようにした |
| 31663925152 | `1766c7b` | 全7ジョブ緑 | SKIP の抽出を、行のどこに現れても拾う形に広げた |
| 31665658371 | `e19c973` | 全7ジョブ緑 | 再試行の上限を待ちの列の和として検証する形にした。msrv ジョブに版の名乗りを足し、checkout の認証情報を残さないようにした |

スキップの集合（unix 1件 / Windows 11件）とテストバイナリの本数は、緑になった5つの run すべてで同一である。

## 初回実行で落ちた Windows の6件と、その吸収

いずれも adr.md ADR-008 の「本 Issue で扱う」側に収まり、別 Issue への切り出し・撤退条件の適用はしていない。

| 失敗 | 原因 | 吸収 |
|---|---|---|
| `adapter::task_file::tests` 5件 | フィクスチャが `/abs/path` を絶対パスとして渡していた。Windows では絶対パスではないので、ドメインの `RepoPath` 検証が正しく `NotAbsolute` を返していた | フィクスチャを `MAIN_SEPARATOR` から組む既存の書き方（ADR-037）に揃えた。ドメイン側は無変更 |
| `util::atomic::tests::読み手は旧内容か新内容のどちらかだけを観測する` 1件 | 読み手がハンドルを開いている間の置換を Windows が `ERROR_ACCESS_DENIED`(5) で拒む | `write_atomic` に共有違反限定・上限付きの再試行を入れた（adr.md ADR-010） |

あわせて **`rename_atomic`（`archive` の経路、TC-port-task-repository-044）にも同じ再試行を掛けた**（adr.md ADR-012）。Windows は最初の失敗ターゲットで cargo が停止していたため 044 は「赤でない」のではなく「未観測」で、plan.md のリスク欄が 042 と 044 を同一原因の対として挙げていた。分類と上限は1つのループに集約してある。

同じ理由で **読み手側にも吸収を入れた**（adr.md ADR-013）。`MoveFileEx` は置き換えられる側を delete-pending に落とすため、置換の窓では読み手も `ERROR_ACCESS_DENIED` を受ける。`FsTaskRepository` の内容の読み取りを `util::atomic::read_atomic` に通し、書き手と同じ分類・上限を共有させた。Windows の適合スイートはこの窓を一度も踏んでいないので、044 と同じく「赤でない」ではなく「未観測」として扱っている。

## スキップの実測（AC-6(d)）

**3 OS とも steps.md ステップ3 の期待集合と完全に一致した。期待値の事後更新は発生していない。**

| OS | 実在のスキップ | 内訳 |
|---|---|---|
| ubuntu / macOS | 1件 | `tc_port_clock_005`（ハーネスが `rewind` を提供しない恒久スキップ） |
| Windows | 11件 | 上記 + 権限系10件（適合行8 + CLI 受け入れ2） |

走ったテストバイナリは3 OS とも15本で同数（`Running` 行を数えたもの。`test result:` 行は Doc-tests 3本を含めて18）。詳細は `crates/pulsen-conformance/HOOKS.md` の「3ランナーでの実測」節に記録した。

`pulsen-conformance` の lib ユニットテストは `SkipBudget` 自身を架空のケース名で検証するため、実在の適合ケースと区別できない `SKIP` 行を全 OS で3件出す。ジョブサマリーの集計はこの区間を除外しており、除外件数は3 OS とも「3 件」と表示された。

## 未検証のまま残ること

**この CI が実測したのは `e19c973`（本 PR の変更をすべて適用した時点）のコードであり、PR #11 が追加するプロセス同定・デタッチ起動の Windows 挙動は含まれない。** Issue #10 のクローズは「クロスプラットフォームが検証済み」を意味しない。この事実は PR 本文と [Issue #10 のコメント](https://github.com/tuanemuy/pulsen/issues/10#issuecomment-5275112376) に残してある。

PR #11 へは次の3件を引き継ぐ。[PR #11 のコメント](https://github.com/tuanemuy/pulsen/pull/11#issuecomment-5275112482) で伝えた:

1. HOOKS.md の実測は #11 のマージ前のもので、#11 がスイートと example を足した時点で部分的に古くなる。更新は #11 の責務。
2. 本 Issue の修正が入った `crates/pulsen/src/adapter/task_file.rs` / `crates/pulsen/src/util/atomic.rs` / `crates/pulsen/src/adapter/task_repository.rs` とのコンフリクト解消は #11 側で行う（マージ順は本 Issue 先行）。
3. #11 の `ProcessController` が Windows で赤になった場合の対応も #11 側。

## 残っている見立て

CI が緑になった今も、次は環境の揺らぎで赤くなりうる。撤退条件（adr.md ADR-008）は適用していないので、これらは通常の CI の赤として扱う。

- `spawn_holder` は `SIGNAL_DEADLINE`(10秒) 超過でも `None` を返すが、`SkipBudget` の許容集合は `holder_program().is_some()` だけを見る。Windows で Defender のスキャンにより初回起動が遅れると、ロック適合4件が「スキップ」ではなく**失敗**として現れる。今回の実行では踏んでいない。
- `rustup update stable` で常に現行 stable を使うため、新しい clippy lint / rustfmt の整形変更が入った日に、コードを変えていなくても赤くなりうる（adr.md ADR-004 が意図的に受け入れたトレードオフ）。受け皿は steps.md ステップ10。
