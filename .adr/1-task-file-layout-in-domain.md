# ADR: タスクファイルのディレクトリ導出と命名形式の判定も `TaskFilePath` に置く

## ステータス

承認済み

## コンテキスト

spec/domains/task.md は `TaskFilePath` を「レイアウトの単一の定義箇所(アダプターも同じ導出を使う)」と定めているが、持っていたのは `active` / `archived`(ファイル1件のパス)だけだった。`TaskRepository` の走査(`list_active` / `list_archived`)は**タスクIDを持たない状態でディレクトリ**を必要とし、さらに「命名形式(`<task-id>.json`)に合致するエントリのみを対象とし、形式外は列挙せず触れない」というフィルタを必要とする。

アダプターが `state_root.join("tasks")` と `.json` の除去を自前で書くと、レイアウトの定義箇所が2つになり、`TaskFilePath` を置いた理由(show の「スナップショット保存先パス」表示・monitoring.md の直接閲覧の導線が同じ導出に乗ること)が崩れる。

## 決定

`TaskFilePath` に次の4つを置き、`active` / `archived` をこれらの合成として定義する。

- `active_dir(state_root)` / `archived_dir(state_root)` — 走査対象のディレクトリ
- `file_name(id)` — `<task-id>.json`
- `parse_file_name(name) -> Option<TaskId>` — 命名形式に合致する名前からタスクIDを読み取る

アダプターはディレクトリもフィルタもこの4つ経由で得る。拡張子はドメイン側の定数のままとし(spec のパス導出がすでに拡張子を含む)、**ファイルの中身の形式**(JSON であること・キー構成・復号の分類)はアダプターの責務(`.adr/1-task-file-json-and-corrupt-classification.md`)という分担を保つ。

## 検討した代替案

- アダプターにディレクトリ名と拡張子の定数を持たせる — レイアウトの定義が2箇所になる。タスクファイルの置き場を変えるときに、ドメインとアダプターの両方を同時に直す必要が生まれる
- `TaskFilePath::active(state_root, id).parent()` からディレクトリを導く — ディレクトリを得るためだけにダミーのタスクIDが要る

## 影響

- 走査の命名形式フィルタが `TaskId::parse` の帰結として定義される。アトミック置換の一時ファイル残骸(`.tmpXXXXXX`)は「先頭が英数字でない」ことで自動的に外れ、フィルタの規則を別に維持しなくてよい
- 後続スライスの RunStore も同じ形(`attempt-<n>` の判定をドメインの導出関数に置く)で書ける
- トレードオフ: ドメインの公開APIが4つ増える
