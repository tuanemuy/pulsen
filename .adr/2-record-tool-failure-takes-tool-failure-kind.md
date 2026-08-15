# ADR: ツール操作の失敗を記録する遷移は、専用の失敗種別だけを受け取る

## ステータス

承認済み

## コンテキスト

`spec/domains/task.md` の遷移表は `record_tool_failure` の引数を `kind` とだけ書き、実装は `FailureKind`（`WorktreeCreate | WorktreeRemove | ArchiveMove | SpawnFail | JudgeFail`）をそのまま受け取っていた。しかし後ろの2値は専用の遷移（`record_spawn_failure` / `record_spawn_failure_in_place` / `record_judge_failure`）が `spawn_fail_count` / `judge_attempt_count` を進めながら記録するものである。

`record_tool_failure(FailureKind::SpawnFail, ..)` は型として書けてしまい、書けた場合は `attempt_count` を進めつつ `SpawnFail` の失敗要因を残す — カウンタと失敗種別が食い違った帳簿が生まれ、`applicable_retry_limit` の併記（`.adr/2026-08-11-retry-limit-source-for-tool-op-failures.md`）が実態と合わなくなる。

## 決定

`ToolFailureKind`（`WorktreeCreate | WorktreeRemove | ArchiveMove`）を新設し、`record_tool_failure` の引数をこれに絞る。帳簿に残す `FailureKind` への写像はドメイン内部（`ToolFailureKind::recorded`、`pub(super)`）に閉じ、呼び出し側は写像を知らない。

永続化される値としての `FailureKind` は5値のまま変えない — タスクファイルの直列化形式と `FailureNote::kind()` の読み取り口は spec のとおり。

spec の表記から逸脱するが、CLAUDE.md の「不正な状態を型で表現不能にする」を優先する。

## 影響

- カウンタと失敗種別が食い違う帳簿が型として書けなくなる。worktree 削除・アーカイブ移動を扱う後続スライスも同じ入口を使う
- トレードオフ: 失敗種別の型が2つになる（永続化される `FailureKind` と、遷移の入口の `ToolFailureKind`）。写像はドメイン内部の1関数に閉じており、外からは見えない
- トレードオフ: `spec/domains/task.md` の遷移表と引数型が一致しないため、「`kind` はツール操作の3種に限る」の spec 追従が必要になる
