# 031: グローバルホームのレイアウトはアプリケーション層に置く

## ステータス

承認済み

## コンテキスト

当初は `PulsenHome`(`config_path` / `workflows_dir` / `state_root` / `worktree_root` / `lock_path` の導出)を `adapter::home` に置き、`application` がそれを import する構成だった。これはアプリケーション層がアダプターを直接参照することになり、依存が内 → 外になる。ADR-019 がクレート境界で守ろうとしている性質がクレート内で緩む。

## 決定

- `PulsenHome` は `application::home` に置く。ホームのレイアウトは「配線情報」であってアダプターの実装詳細ではない
- アダプターは導出済みのパス(`StateRoot` / `workflows_dir` / `lock_path` 等)を構築時に受け取るだけにする
- ホームの**解決**(`--home` > `PULSEN_HOME` > 既定 `~/.pulsen/`)は `cli` の合成ルートが行い、`std::env::home_dir()` / `std::env::var` を読むのもそこ1箇所にする
- ドメイン側で `StateRoot` / `WorktreeRoot` / `RunDirPath::derive` / `TaskFilePath::active|archived` がレイアウト導出を持つ設計と一貫させる

## 影響

- `application` の import が `pulsen_domain` + std だけになり、依存方向がクレート内でも保たれる
- トレードオフ: 合成ルート(`cli`)の結線コードが少し厚くなる
