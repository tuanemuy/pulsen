# ADR: ストアのアダプターはホームのレイアウトを持たず、必要なパスをすべて注入される

## ステータス

承認済み

## コンテキスト

`ConfigLoadError::NotFound` は「解決後のグローバルホームパス」を含む契約であり、未初期化を案内する文言(pages の共通挙動)がこの値を使う。一方 `.adr/1-pulsen-home-layout-in-application-layer.md` は、ホームのレイアウト(`config.yaml` がホーム直下にあること、ワークフロー定義が `<home>/workflows` にあること)をアプリケーション層に置き、アダプターには導出済みのパスだけを渡すと定めている。

アダプターがホームだけを受け取って `home.join("config.yaml")` を組み立てると、レイアウトの知識がアダプターにも漏れ、定義箇所が2つになる。

## 決定

ストアのアダプターは、**読む対象のパスと、案内に載せるパスの両方を構築時に注入される**。

- `FsConfigStore::new(config_path, home)` — 読むファイルのパスと、`NotFound` に載せるホームパス
- `FsWorkflowStore::new(workflows_dir, base_dir)` — 走査するディレクトリと、相対パス解決の基準(`.adr/1-workflow-store-base-dir-injection.md`)

`<home>/config.yaml` や `<home>/workflows` の組み立ては合成ルートが行う。アダプターが持つのは「名前は `<workflows_dir>/<name>.yaml` に解決する」というポートの契約そのものだけにする。後続スライスで足すストア(RunStore 等)も同じ形にする。

## 検討した代替案

- アダプターにホームを渡してレイアウトを組み立てさせる — レイアウトの定義がアプリケーション層とアダプターの2箇所になり、`.adr/1-pulsen-home-layout-in-application-layer.md` が守ろうとした依存方向がアダプター側から崩れる
- パスをポートのメソッド引数にする — spec のポート表と signature が食い違い、ホームの解決を毎回呼び出し側が意識することになる

## 影響

- ホームのレイアウトを変えてもアダプターは無変更で済む。適合テストのハーネスが任意のパス構成でストアを組める
- トレードオフ: 構築時の引数が1つ増え、2つのパスの整合(`config_path` が `home` の下にあること)は合成ルートの責任になる
