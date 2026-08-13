# 070: ラッパーは config もホームも読まず、`RunDirPath` から state root を復元して RunStore を組む

## ステータス

承認済み

## コンテキスト

`RunWrapper` の入力は `WrapperLaunchSpec { run_dir, workspace, agent_cmd }` の3つだけで、spec は「config は読まない（必要な情報はすべて起動引数で受け取る）」と定める。config.yaml が不在・破損した環境でもラッパーの動作に影響しないことが適合ケースの要求である。

一方 `RunStore::prepare_attempt(id, n)` は `RunDirPath::derive(state_root, id, n)` と一致するパスを返す契約なので、fs 実装は `StateRoot` を構築時に注入される（ADR-043）。ラッパーは `prepare_attempt` を呼ばないが、`RunStore` の実装を1つの型として構築する以上 `StateRoot` の値が要る。

## 決定

`RunDirPath::state_root(&self) -> Option<StateRoot>` を `derive` の直下に置き、`<state_root>/runs/<task-id>/attempt-<n>` の形に合致しない値には `None` を返す。ラッパーの合成は `--run-dir` の値だけから RunStore を構築し、ホームの解決も `ConfigStore::load` も行わない。

復元できない `run_dir` は起動引数の不正として扱い、**何も書かずに非0終了**する（tick の猶予経路が spawn 失敗として分類する）。

## 検討した代替案

- **ラッパーでもホームを解決して `PulsenHome::state_root()` を使う** — `--home` で起動された tick が spawn したラッパーは `--home` を受け取らないため、既定の `~/.pulsen` を解決してしまう。値が使われないので実害は無いが、「使われないことに依存した配線」は次の変更で壊れる
- **合成ルートが `run_dir` の親を3つ遡って組み立てる** — レイアウト知識（`runs/` という段があること）が合成ルートに漏れ、`RunDirPath::derive` と2箇所に分かれる

## 影響

- ラッパーが config・ホーム・環境変数のいずれにも依存しなくなり、「config を読まない」が定義どおりに成立する。レイアウトの知識は `task/path.rs` の1箇所に留まる
- トレードオフ: `derive` の逆写像はドメイン台帳（`spec/inventory/domain.md`）に無い追加であり、spec 追従が必要になる
