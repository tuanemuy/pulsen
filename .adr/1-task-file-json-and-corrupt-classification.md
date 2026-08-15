# ADR: タスクファイルは JSON 単一ファイルとし、`Corrupt` と `SnapshotUnreadable` は「JSON として有効か」で分ける

## ステータス

承認済み

## コンテキスト

`.adr/2026-08-11-snapshot-embedded-in-task-file.md` はスナップショットをタスクファイルに正規化構造として埋め込むことを決めているが、物理形式と、`save_degraded` が「破損したスナップショットフィールドを元の内容のまま書き戻す」契約をどう実装するかは未確定だった。ドメインの `DegradedTask` はスナップショットを持たないため、保存時に値をドメインから供給できない。

当初は「`snapshot` の**構文**破れ → `SnapshotUnreadable`」と書いていたが実現不能だった。`snapshot` を含むファイル全体を1回の JSON パースで読む以上、snapshot フィールドの中身が JSON 構文として壊れていればファイル全体のパースが失敗する(実測: `{"a":1,"snapshot":{broken}}` → `key must be a string at line 1 column 21`)。

## 決定

- タスクファイルは `state/tasks/<task-id>.json`(アーカイブは `state/archive/<task-id>.json`)の JSON 1ファイル。整形して書き、人間が直接読める状態にする
- 復号の分類を次のとおり定義する

  | 状態 | 分類 |
  |---|---|
  | ファイル全体が JSON として不正 | `Corrupt { path, message }` |
  | 有効な JSON だが、タスク側フィールドの型・値制約・未知キーが破れている | `Corrupt` |
  | 有効な JSON で、タスク側フィールドは読めるが、`snapshot` の値がスナップショットとして解釈できない(型不一致・必須キー欠落・フィールド不在・`initial ∉ statuses`・`next ∉ statuses`・`task_status ∉ statuses`) | `SnapshotUnreadable(DegradedTask)` |
  | 状態間整合の不変条件2〜4の破れ | 検証しない(`Intact` で返し、遷移関数の前提検査に委ねる) |

- 適合テストの破損フィクスチャはこの区分に沿って「有効な JSON でありながらスナップショットとして解釈できない」形で作る(`"snapshot": "壊れた"` / `{"initial": 123}` / `snapshot` キーの削除 / `task_status` の差し替え)
- 復号時、`snapshot` フィールドは `Box<serde_json::value::RawValue>` として生のまま保持する。`save_degraded` はディスク上の既存ファイルを読み直して `snapshot` の生バイト列を引き継ぎ、タスク側フィールドだけを差し替えて書き戻す。DTO の `Option<Box<RawValue>>` には `#[serde(skip_serializing_if = "Option::is_none")]` を付け、**キーの不在を不在のまま**書き戻す(既定の直列化ではキー削除が `"snapshot": null` に化け、「元の内容のまま温存する」契約から外れる)
- タスク側の未知キーは拒否する(`deny_unknown_fields`)。スキーマバージョンのフィールドは設けない

## 検討した代替案

- `snapshot` を JSON 文字列としてネストする — snapshot の構文破れをファイル全体から独立させられるが、エスケープだらけになって requirements §9 の人間可読性と `.adr/2026-08-11-snapshot-embedded-in-task-file.md` の「正規化構造として埋め込む」を壊す

## 影響

- 修復材料(壊れたスナップショット)が失われない。アーカイブ移動が単一ファイルの rename に帰着する。`Corrupt` / `SnapshotUnreadable` の境界が実装可能かつ機械的に判定できる
- トレードオフ: `save_degraded` が読み → 書きの2ステップになる(書き込み自体はアトミック置換なので中間状態は観測されない)
- spec への追従が要る: spec/testcases/ports/task-repository.md の「スナップショットフィールドのみを**構文不正**な内容に置き換える」は、この設計では「有効な JSON だがスナップショットとして解釈不能」と読むほかない。spec 側の語を実装可能な表現へ言い換える提案を Issue のコメントに残す
