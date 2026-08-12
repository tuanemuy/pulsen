# 残存課題 — Issue #2

## 既知の制限

### Windows の `KillIdent` は pid 文字列のまま

`unsafe` 禁止のままジョブオブジェクトを扱えないため、Windows ではプロセスグループに相当する同定子を永続化できない。#3 の `try_kill_remnants` が「グループごと落とす」契約を Windows で満たせない可能性がある。ADR-003 の申し送りとして残し、Issue #10（クロスプラットフォーム検証）で実機確認する。

影響範囲: Windows 実機のみ。Unix 系では PGID を観測して永続化しているため影響しない。

### `cargo test --test <単一ターゲット>` では ProcessController の適合が落ちる

適合ケースが `target/debug/examples/` のプローブに依存するのに対し、単一テストターゲット指定では examples がビルドされない。プローブ不在をスキップ許容集合に入れない方針（ADR-055）を採っているため、スキップではなく失敗になる。

回避策: `cargo test`（パッケージ全体）または `cargo build --examples` を先に実行する。CI では全体実行するので影響しない。

### worktree の内容持ち越し（F4）の観測手段

`agent_probe` に「cwd へ書き込む」モードがなく、追加は本スライスのスコープ外。リトライ間で worktree が作り直されないことは、tick の間にテスト側から worktree へファイルを書いて残存を観測する形で検証している。エージェント自身の生成物で観測しているわけではない。

## spec 追従の提起（Phase 5 で Issue 化する候補）

実装で spec と食い違った、または spec が規定していない箇所。いずれも既存規約を優先した結果で、コード側の修正は不要。

- `CommandLine::rehydrate` の追加 — `DOM-definition-023` は「`expand` の結果としてのみ生成される」としているが、ラッパーが argv から復元するために生成経路が2つになった（ADR-007）
- `RunDirPath::state_root` の追加 — ラッパーが config もホームも読まずに `RunStore` を構築するための逆写像。台帳に無い追加（ADR-006 / ADR-015）
- `wrapper` の終了コード — spec は引数不正の1行しか規定していない。「ラッパー自身が責務を果たせたか」を表す規約を置いた（ADR-017）
- tick の `errors` — spec は `message: String` だが、文言の組み立てを CLI 層に寄せる規約に従って構造化した値にした（ADR-009）

## 未着手（後続フェーズで行う）

steps.md ステップ19（手動確認・チェックリストの記帳・ADR の昇格判定）は Phase 4 以降で実施する。
