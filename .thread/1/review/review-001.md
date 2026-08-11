# PR Review #001 — [skeleton] 基盤・グローバル設定・ワークフロー定義とタスク登録(add)

**PR:** #8
**Date:** 2026-08-12
**Round:** 1回目

## Summary

- Blockers: 1
- Warnings: 39
- Verdict: **BLOCKED**

## レイヤー別ファイル

- Domain: review-001-domain.md（B: 0 / W: 4）
- Use Case / CLI: review-001-usecase-cli.md（B: 0 / W: 8）
- Adapter / Infrastructure: review-001-adapter.md（B: 1 / W: 8）
- Test: review-001-test.md（B: 0 / W: 7）
- Architecture / Spec-conformance: review-001-arch-spec.md（B: 0 / W: 12）

## カバレッジ

- 確認申告ゼロのファイル: `Cargo.lock` → メインが直接確認（48パッケージ。宣言した6依存の推移閉包のみで、想定外の依存なし）

## 指摘一覧

- [B-001] adapter/task_repository.rs:list_active/list_archived — 走査中の archive を ReadError::Io に写しポート契約を破る（Adapter）
- [W-001] definition/assembler.rs:WorkflowAssembler — 重複ステータス名を黙って後勝ちで畳む（Domain）
- [W-002] definition/assembler.rs + adapter/config_store.rs — エラー説明文の二重定義（Domain）
- [W-003] definition/workflow.rs:effective_* — 不在ステータスと「上書きなし」を区別しない（Domain）
- [W-004] definition/template.rs:SkillInputTemplate — Segment 共有により render に unreachable! が必要（Domain）
- [W-005] cli/render.rs:15 — アダプター型を render が直接 import し層分離を破る（UseCase/CLI）
- [W-006] cli/wire.rs:161 — PULSEN_HOME 単独のホーム解決が未検証（UseCase/CLI）
- [W-007] cli/wire.rs:161 — 空の PULSEN_HOME を未設定扱いにする規則が未文書化（UseCase/CLI）
- [W-008] cli/exit.rs:12 — exit code 2 と --help の 0 が自動テストで未固定（UseCase/CLI）
- [W-009] tests/register_task.rs:588 — 「全件まとめて」が1件しか検証していない（UseCase/CLI・Test 重複）
- [W-010] tests/register_task.rs:526 — DetachedHead/EmptyRepository をポート契約と矛盾する台本で流す（UseCase/CLI）
- [W-011] cli/wire.rs:65 — Runtime::home() が未使用（UseCase/CLI）
- [W-012] cli/render.rs:31 — 成功時に未実装の tick を案内（UseCase/CLI）
- [W-013] adapter/workflow_store.rs:90 — Io/Parse に解決先パスが乗らない（Adapter）
- [W-014] util/atomic.rs:39 — rename_atomic が移動元の親を fsync しない（Adapter）
- [W-015] adapter/worktree.rs:51 — Path::exists() が I/O エラーを false に丸める（Adapter）
- [W-016] util/atomic.rs:26 — write_atomic が対象ファイルの mode を 0600 に置き換える（Adapter）
- [W-017] adapter/task_repository.rs:127 — create の一意性判定が TOCTOU（Adapter）
- [W-018] adapter/task_repository.rs:128 — 存在確認の失敗経路にパスが乗らない（Adapter）
- [W-019] adapter/worktree.rs:12 — GIT_CEILING_DIRECTORIES 等が未除去（Adapter）
- [W-020] adapter/yaml.rs:235 — 非文字列キーを名前として無言受理（Adapter）
- [W-021] cli_add_boundary.rs — 境界値の拒否ケースで config/定義の不変が未検証（Test・Arch 重複）
- [W-022] conformance/task_repository.rs:TC-042/044 — 観測回数の下限がなく読み取り0回でも緑（Test）
- [W-023] tests/common/mod.rs:Untouched — 新規ファイル生成を検出しない（Test）
- [W-024] tests/common/mod.rs:deny_read — ディレクトリに渡すと root で誤判定（Test）
- [W-025] cli_add_error.rs:TC-022 — 重複キー分岐だけ位置を未検証（Test）
- [W-026] conformance/lib.rs — ConfigStore/WorkflowStore のフックが YAML 生テキストを取る（Arch）
- [W-027] .adr/035 — ADR-038/043/045/046/049/051/052 が .adr/ に未起票（Arch）
- [W-028] .thread/1/adr.md — Status が実態と非同期（Arch）
- [W-029] HOOKS.md:200 — 出荷物が .thread/1/adr.md を根拠として指す（Arch）
- [W-030] conformance/lib.rs:150 — スキップが println! で緑と区別できない（Arch）
- [W-031] .thread/1/progress.md — 「全件が実行された」が同表と矛盾・Issue コメント未記載（Arch）
- [W-032] definition/name.rs:101 — InputText::new が spec の parse 規約から外れる（Arch）
- [W-033] cli/args.rs:22 — PAGE-common-011 を守る回帰テストがない（Arch）
- [W-034] tests/register_task.rs — ユースケース層の5行にテスト名の TC ID がない（Arch）
- [W-035] adapter/worktree.rs:104 — {error:?} の Debug 表現が利用者に出る（Arch）
- [W-036] HOOKS.md:132,143 — 対応表の区分・分岐が自前の基準に合わない（Arch）
