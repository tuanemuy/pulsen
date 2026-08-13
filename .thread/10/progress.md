# 進捗メモ — Issue #10

`.github/workflows/ci.yml` を新規追加し、3ランナーで実行して**全7ジョブが緑**になった（run 31657976822）。steps.md のステップ1〜12 がすべて完了。

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

## 初回実行で落ちた Windows の6件と、その吸収

いずれも ADR-008 の「本 Issue で扱う」側に収まり、別 Issue への切り出し・撤退条件の適用はしていない。

| 失敗 | 原因 | 吸収 |
|---|---|---|
| `adapter::task_file::tests` 5件 | フィクスチャが `/abs/path` を絶対パスとして渡していた。Windows では絶対パスではないので、ドメインの `RepoPath` 検証が正しく `NotAbsolute` を返していた | フィクスチャを `MAIN_SEPARATOR` から組む既存の書き方（ADR-037）に揃えた。ドメイン側は無変更 |
| `util::atomic::tests::読み手は旧内容か新内容のどちらかだけを観測する` 1件 | 読み手がハンドルを開いている間の置換を Windows が `ERROR_ACCESS_DENIED`(5) で拒む | `write_atomic` に共有違反限定・上限付きの再試行を入れた（ADR-010） |

あわせて **`rename_atomic`（`archive` の経路、TC-port-task-repository-044）にも同じ再試行を掛けた**（ADR-012）。Windows は最初の失敗ターゲットで cargo が停止していたため 044 は「赤でない」のではなく「未観測」で、plan.md のリスク欄が 042 と 044 を同一原因の対として挙げていた。分類と上限は1つのループに集約してある。

## スキップの実測（AC-6(d)）

**3 OS とも steps.md ステップ3 の期待集合と完全に一致した。期待値の事後更新は発生していない。**

| OS | 実在のスキップ | 内訳 |
|---|---|---|
| ubuntu / macOS | 1件 | `tc_port_clock_005`（ハーネスが `rewind` を提供しない恒久スキップ） |
| Windows | 11件 | 上記 + 権限系10件（適合行8 + CLI 受け入れ2） |

走ったテストバイナリは3 OS とも19個で同数。詳細は `crates/pulsen-conformance/HOOKS.md` の「3ランナーでの実測」節に記録した。

`pulsen-conformance` の lib ユニットテストは `SkipBudget` 自身を架空のケース名で検証するため、実在の適合ケースと区別できない `SKIP` 行を全 OS で3件出す。ジョブサマリーの集計はこの区間を除外している。

## 未検証のまま残ること

**この CI が実測したのは `origin/main`（9d54376）時点のコードであり、PR #11 が追加するプロセス同定・デタッチ起動の Windows 挙動は含まれない。** Issue #10 のクローズは「クロスプラットフォームが検証済み」を意味しない。この事実は PR 本文と Issue #10 のコメントにも残してある。

PR #11 へは次の3件を引き継ぐ（#11 にコメントで伝える）:

1. HOOKS.md の実測は `origin/main` 時点のもので、#11 がスイートと example を足した時点で部分的に古くなる。更新は #11 の責務。
2. 本 Issue の修正が入った `crates/pulsen/src/adapter/task_file.rs` / `crates/pulsen/src/util/atomic.rs` とのコンフリクト解消は #11 側で行う（マージ順は本 Issue 先行）。
3. #11 の `ProcessController` が Windows で赤になった場合の対応も #11 側。

## 残っている見立て

CI が緑になった今も、次は環境の揺らぎで赤くなりうる。撤退条件（ADR-008）は適用していないので、これらは通常の CI の赤として扱う。

- `spawn_holder` は `SIGNAL_DEADLINE`(10秒) 超過でも `None` を返すが、`SkipBudget` の許容集合は `holder_program().is_some()` だけを見る。Windows で Defender のスキャンにより初回起動が遅れると、ロック適合4件が「スキップ」ではなく**失敗**として現れる。今回の実行では踏んでいない。
- `rustup update stable` で常に現行 stable を使うため、新しい clippy lint / rustfmt の整形変更が入った日に、コードを変えていなくても赤くなりうる（ADR-004 が意図的に受け入れたトレードオフ）。受け皿は steps.md ステップ10。
