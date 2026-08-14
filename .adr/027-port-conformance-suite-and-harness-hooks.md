# 027: ポート適合テストはマクロで1ケース1テストに展開し、ハーネスのフックは意図レベルにする

## ステータス

承認済み

## コンテキスト

spec/testcases/ports/*.md は「すべてのアダプター実装が共通で通す」スイートとして書かれており、本スライスの fs / システム実装だけでなく、後続スライスの in-memory 実装や別プラットフォーム実装にも同じ検証を適用したい。一方、Rust の `#[test]` はクレート内に静的に置く必要がある。

当初はハーネスに `put_raw(area, id, content: &str)` / `read_raw(...)` を持たせ、ケース関数が JSON 文字列を組み立てる形にしていた。しかしこれだと破損系ケース(約12件 — 最も価値のある部分)が fs 実装専用のコードになり、狙いが破れる。

## 決定

- `pulsen-conformance` にポートごとの `Harness` トレイトとケース関数(1関数 = spec の表の1行)を置く
- `#[macro_export]` の宣言的マクロが、与えられたセットアップ式に対して各ケースの `#[test]` 関数を生成する。アダプター側のテストファイルはマクロ呼び出し1行で済む
- **ハーネスのフックは「破損・状況の意味」だけを受け取る**。実現方法(生 JSON の配置・権限操作・プロセス起動)は各ハーネス実装の内側に閉じる。生の文字列を受け渡す `put_raw` / `read_raw` はスイートの API から外す
- **フックの一覧は spec/testcases/ports/\*.md の前提条件から導く**。ステップ9の完了条件を「125行 × フックの対応表を埋めきること」とし、各行に「ポートのメソッドだけで組める / このフックで組める / spec が明示するスキップ可」のいずれかを記入する。埋まらない行が残ったらフックを足す

  | ポート | フック |
  |---|---|
  | TaskRepository | `corrupt_whole_record` / `break_task_field` / `corrupt_snapshot` / `drop_snapshot_field` / `set_task_status_outside_snapshot` / `break_snapshot_invariant` / `place_in_both_areas` / `put_unnamed_entry` / `record_bytes` / `snapshot_bytes` / `make_unreadable` / `make_unwritable` / `concurrent_repo` |
  | ConfigStore | `put_config(text)` / `remove_config()` / `home_path()` / `make_unreadable()` |
  | WorkflowStore | `put_named` / `put_named_with_ext` / `expected_path_for_name` / `put_at_absolute` / `put_at_relative` / `missing_absolute_path` / `make_unreadable` |
  | ExclusiveLock | `hold_from_other_process` / `kill_holder` / `release_holder` / `try_acquire_from_other_process` / `separate_home` / `unusable_lock` |
  | WorktreeManager | `repo_with_commit` / `repo_without_commit` / `detached_repo` / `non_repo_dir` / `missing_path` / `head_branch_name` / `absent_branch_name` / `failing_manager` |
  | Clock | `observe_wall_clock` / `advance` / `rewind` |
  | TaskIdGenerator | `another_generator` |

  対応表の正本は `crates/pulsen-conformance/HOOKS.md` 一本とする。上の表は決定時点のポートとフックの記録であり、後続スライスでポートやフックが増えても更新しない。正本を2つ置くと、ポートが増えるたびに恒久の決定記録を書き換え続けることになり、食い違ったときにどちらが正しいかを決める根拠も無くなる。

- すべてのフックは既定実装が `None` を返し、スイートはスキップして理由を出力する
- **権限操作系のフック(`make_unreadable` / `make_unwritable`)は、制限が実際に効いたことを確認してから `Some` を返す**。`chmod 000` は root では効かないため、確認せずに `Some` を返すと `Err(Io)` を期待するケースがスキップに落ちずに FAIL する。実装規則は「制限を掛ける → 実際に読み(書き)を試す → 通ってしまったら復元して `None` を返す」。root 実行・Windows・特殊なファイルシステムのすべてをこの1つの規則で吸収する
- **「対象を壊すフック」ではなく「壊れた対象を別ハンドルとして返すフック」を既定の形にする**。ハーネスは対象をアクセサ越しに共有参照で渡すため、構築時に注入した値がイミュータブルなアダプターを後から壊すには、本番アダプターにテスト専用の内部可変性を持ち込むしかなくなる。WorktreeManager の TC-009 は `failing_manager(&self) -> Option<&Self::Manager>` とし、git ハーネスは「存在しないパスを `git_program` として構築した2つ目の manager」を保持するだけでよい。ExclusiveLock の「機構自体が利用不能」(TC-007)も同じ形の `unusable_lock` にする
- **「存在するブランチ名」と「存在しないブランチ名」は別のフックにする**。同じフックの値を反転させて使うとハーネスが前提を保証できず、`absent_branch_name` が偶然存在するリポジトリでケースの主張が空虚になる
- **原子性の観測面(TC-port-task-repository-042・044)だけを `Sync` 境界から隔離する**。当該ケースは `std::thread::scope` で書くには `Repo: Sync` が要る(実測: 境界なしでは `E0277`)。しかし `Harness::Repo: Sync` を無条件に置くと `RefCell` ベースの in-memory 実装がスイート全体を適用できなくなる。そこで `concurrent_repo(&self) -> Option<&(dyn TaskRepository + Sync)>` というスキップ可能なフックを1つ置き、並行読み取りを前提とするケースはこのハンドル越しにのみ読み書きする(TC-043 は並行の前提を持たず、`repo()` で書ける)

## 検討した代替案

- スイートを `Vec<(&str, fn(&H))>` として返し、1つの `#[test]` でループする — 失敗したケース名が1テストの中に埋もれる
- ケースを各アダプターのテストにコピーする — spec の1行が複数箇所に散り、後続スライスで乖離する
- `Harness::Repo: Sync` を無条件の境界にする — `RefCell` ベースの in-memory 実装がスイートを一切適用できなくなる

## 影響

- `cargo test` の出力が spec の行と1:1 で対応し、チェックリスト消化を機械的に確認できる。破損系ケースが実装非依存になる。フックの粒度が spec 由来であることが対応表で構造的に担保される
- トレードオフ: マクロが長くなる。ハーネスのフックが増える。原子性のうち並行読み取りを前提とする2ケースは `Sync` な実装でしか走らない
