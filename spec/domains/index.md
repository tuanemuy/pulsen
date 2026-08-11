# ドメイン一覧

pulsen のドメインは3つに分割する(境界の根拠は ADR-017)。上流成果物は [../requirements.md](../requirements.md)(正本)・[../scenario/index.md](../scenario/index.md)・[../pages/index.md](../pages/index.md)。

| ドメイン | 責務 | 詳細 |
|---|---|---|
| Definition | ユーザーが記述する定義(グローバル設定・ワークフロー定義)の構造・検証・コマンドテンプレート展開 | [definition.md](definition.md) |
| Task | タスク集約(タスクステータス・実行状態・カウンタ・スナップショット)と遷移ロジック | [task.md](task.md) |
| Execution | 実行の観測と判断(launching / running の分類・判定プロトコル・runディレクトリ・gc規則)と外界のポート | [execution.md](execution.md) |

## 依存方向

```
Definition ← Task ← Execution
    ↑                  |
    └──────────────────┘
```

- Task → Definition: タスクはワークフロー定義のスナップショット(`WorkflowSnapshot`)を保持する
- Execution → Task / Definition: 分類サービスはタスクの実行状態と定義の実効値(timeout等)を入力に取る
- Definition は他ドメインに依存しない。循環はない

## ポートの帰属

| ポート | ドメイン | 目的 |
|---|---|---|
| ConfigStore | Definition | グローバル設定(config.yaml)の読み込み |
| WorkflowStore | Definition | ワークフロー定義(名前 / パス)の解決と読み込み |
| TaskRepository | Task | タスクファイルの読み書き・走査・アーカイブ |
| TaskIdGenerator | Task | タスクIDの発行 |
| Clock | Task | 現在時刻の取得 |
| RunStore | Execution | runディレクトリ(pid / starttime / exit / マーカー / gc)の読み書き |
| ProcessController | Execution | デタッチ起動・起動時刻取得・kill・ラッパー自身の同定(§4.3 のプラットフォーム抽象) |
| WorktreeManager | Execution | 対象検証・worktree の作成・削除 |
| CommandRunner | Execution | 判定コマンド・通知コマンドの直接起動 |
| ExclusiveLock | Execution | tick と状態変更CLIの相互排他(§4.3) |

## トランザクション境界

**UnitOfWork ポートは定義しない。** 永続化される可変状態はタスクファイル(1タスク = 1ファイル)のみで、すべての書き込みは単一タスクに閉じる(1メソッド = 1操作で足りる)。複数リソースにまたがる処理(worktree削除 + アーカイブ移動、stopped記録 + 通知)は原子性で守るのではなく、requirements の設計どおり「冪等な再導出」(tickが永続化された事実から同じ判断をやり直す)と at-least-once(通知)で回復する。各ユースケースの書き込み境界は Phase 2 で個別に確認する。

## OS依存操作の隔離

requirements §4.3 の一覧(起動時刻取得・デタッチ起動・プロセスグループ相当のkill・排他ロック・アトミック置換)のうち、アトミック置換はポートではなく **TaskRepository / RunStore のアダプター実装の契約**(部分的な書き込みが観測されない)として吸収する。残りは ProcessController / ExclusiveLock として定義する。この一覧にないOS依存操作を追加しないことが設計上の制約であり、必要になった場合は §4.3 とここに追記して隔離する。
