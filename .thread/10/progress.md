# 進捗メモ — Issue #10

`.github/workflows/ci.yml` を新規追加した。steps.md のステップ1〜4（無条件のワークフロー作成）が完了で、ステップ5 以降は CI の実行を要するため未着手。

## 完了

| ステップ | 内容 |
|---|---|
| 1 | ワークフローの骨格（トリガー・`permissions`・`concurrency`・`env`・`defaults.run.shell`）と fmt ジョブ |
| 2 | stable マトリクスジョブ（前提の検査 → 非 root アサート → build → test → clippy）。ジョブ名は steps.md の設計ブロックに合わせて `test` |
| 3 | スキップ報告ステップ。期待スキップ集合は steps.md ステップ3 の表に実行前から確定済み（unix 1件 / Windows 11件） |
| 4 | MSRV マトリクスジョブ。版数は `cargo metadata` の全メンバー `rust_version` から読み出す |

why コメントは steps.md が指示した5点（`concurrency` を PR 限定にする理由、`RUST_BACKTRACE` を置かない理由と意図的なパニック2件、`--workspace` を保つ理由、スキップの判定主体を CI に持たせない理由、版数をハードコードしない理由）をワークフローに残した。

## 未着手

- **ステップ5**（CI の初回実行と結果採取）: `workflow_dispatch` は既定ブランチに到達するまで使えず、`push` は main 限定なので、実行手段は PR 一択。
- **ステップ6〜10**（条件付き修正）: CI を回すまで何が落ちるか確定しない。机上推定での先回り修正は入れていない。
- **ステップ11**（HOOKS.md への実測記録）: 3ランナーでの実測が前提。
- **ステップ12**（最終確認）: CI の実測を要する項目が残る。

## ローカルで確認したこと

push できないぶん、ワークフローの静的な確認とシェル断片の実行をローカルで行った。

- YAML が Psych でパースでき、トップレベルキーが `name` / `on` / `permissions` / `concurrency` / `env` / `defaults` / `jobs`。
- ジョブは fmt 1・test 3・msrv 3 の計7（AC-1）。`test` / `msrv` は `fail-fast: false`。
- スキップ報告スクリプトを `bash --noprofile --norc -eo pipefail` で4状態すべて実行し、いずれも exit 0 で意図した文言を出す（`test.log` 不在 / テストが走っていない / SKIP 0件 / SKIP あり）。`pulsen-conformance` の lib ユニットテストの区間が落ち、色あり・色なしの両方で結果が変わらないことを gawk 5.4.1 と macOS 同梱 awk（20200816）で確認した。
- MSRV 読み出しコマンドが `1.89` を返す。0件・値が割れた場合はいずれも exit 1。
- `grep` を使うステップは該当なしでもステップを落とさない（`|| true` と `case` の被検査式で `-e` から切り離してある）。
- `sort` / 版数のハードコード / 第三者製 Action / ステップ単位の `shell:` 指定がいずれも 0 件。
- `cargo build` / `cargo test` / `cargo clippy --all-targets -- -D warnings` / `cargo fmt --all --check` が引き続き緑（macOS・rustc 1.97.1）。

## CI が実測する範囲

**この CI が実測するのは `origin/main`（9d54376）時点のコードであり、PR #11 が追加するプロセス同定・デタッチ起動の Windows 挙動は含まれない。** Issue #10 のクローズは「クロスプラットフォームが検証済み」を意味しない。この1行は PR 本文と Issue #10 のコメントにも残す（steps.md ステップ12）。
