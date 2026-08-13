# レビュー 002 — 共通ユーティリティ・並行性・OS 抽象

対象: PR #12（`issue/10/ci-msrv-cross-platform` ← `main`）
差分取得: `git diff --no-renames --name-status origin/main...HEAD`（14 ファイル）
契約: `.thread/10/plan.md` / 決定: `.thread/10/adr.md` ADR-008 / 010 / 012 / 013

## 総評

`retry_while_transient` / `transiently_denied` / `MAX_ATTEMPTS` への集約は ADR-010 / 012 / 013 の決定と一致しており、CLAUDE.md「アトミック性・排他が要る操作は共通のユーティリティに集約し、個別に再実装しない」を満たしている。分類が `#[cfg(windows)]` / `#[cfg(not(windows))]` の 2 分岐で尽くされ、unix 側は `transiently_denied` が常に偽なので再試行の経路に一度も入らない — 挙動は `origin/main` と一致する。`sync_dir` の `#[cfg(unix)]` / `#[cfg(not(unix))]` も含め、`cfg` の分岐に穴は無い。

3 契約の検証結果:

- **失敗時に一時ファイルを残さない** — 保たれている。`persist` は失敗時に `PersistError.file` で `NamedTempFile` を返し、`retry_while_transient` が `state` として持ち越す。上限到達時は `state` がスコープ末尾で drop され、`NamedTempFile` の Drop が削除する。再試行の途中でも一時ファイルは 1 個以上に増えない（`置換の一時的な拒否が続けば打ち切られ一時ファイルも残らない` がこれを実測している）。
- **失敗時に対象が変わらない** — 保たれている。`persist` / `fs::rename` はいずれも「全部成功か何も起きないか」で、再試行は同じ操作の繰り返しにすぎない。
- **読み手はロックなしで常に一貫した内容を見る** — 書き手側だけだった吸収が読み手側にも揃った（ADR-013）。`FsTaskRepository` の内容読み取り 3 箇所（`lookup` / `list` / `save_degraded`）がすべて `read_atomic` を通っている。

読み手側のエラー分類も崩れていない。`transiently_denied` が真とするのは `ERROR_ACCESS_DENIED`(5) / `ERROR_SHARING_VIOLATION`(32) の 2 つだけで、`ERROR_FILE_NOT_FOUND`(2) / `ERROR_PATH_NOT_FOUND`(3) は含まれない。`read_atomic` の `NotFound` は初回で返り、`unreachable_entry` による「消えたエントリ（`Ok(None)`）」と「読めないエントリ（`Corrupt`）」の判別は不変。むしろ Windows の delete-pending 窓（open が 5 で拒まれる）を待ち切ってから `NotFound` に到達するようになるため、判別の精度は上がる方向にある。

同じ原因の取りこぼしを探したが、実害のある残りは見つからなかった。

- `config_store.rs:48` / `workflow_store.rs:68` の `read_to_string` — この 2 つの経路に `write_atomic` で書く書き手は存在しない（`grep write_atomic` の結果、書き手は `task_repository.rs` の 3 箇所のみ）。置換の窓が生じないので ADR-013 の「対象外」は成立している。
- `exists()` の `symlink_metadata` / `save` / `archive` の `try_exists` — std の Windows 実装が `ERROR_SHARING_VIOLATION` / `ERROR_ACCESS_DENIED` で `FindFirstFileW` にフォールバックするため、置換の窓でも結果が返る。ADR-013 の除外理由と一致。
- 適合スイートの並行ケース（TC-042 / 044）の読み手は `repo.find` / `repo.list_*`、すなわちポート経由なので `read_atomic` を通る。`FsTaskRepositoryHarness` の生 `fs::read`（`record_bytes` / `snapshot_bytes` / `edit`）は逐次ケース専用で、並行ケースからは呼ばれない。取りこぼしではない。
- `pulsen-conformance/src/lib.rs:258` と `tests/common/mod.rs:554` の権限 probe は生 `fs::read` のまま。ここで再試行が掛かると probe の意味（読めないことの確認）が壊れるので、通していないのが正しい。

AC-5 のドメイン層 grep は 0 件を確認（`crates/pulsen-domain/` に target 述語つき `cfg` は属性形・マクロ形とも無し）。吸収先はすべて `crates/pulsen/src/util/` と `crates/pulsen/src/adapter/` に収まっており、ADR-008 の「本 Issue で扱う」側の境界（置換手順・シグネチャ・契約を変えず、失敗時の再試行と分類をその手順の中に足す）を越えていない。`read_atomic` の追加は関数の**追加**であって既存 2 関数の契約変更ではないので、ADR-008 の「切り出す」側にも当たらない。

### 共通ユーティリティ・並行性・OS 抽象

#### Blockers

なし。

#### Warnings

- **[W-001] 文書化された 511ms の上限が、合成経路と Windows のスリープ粒度のどちらでも実際より小さい**

  場所: `crates/pulsen/src/util/atomic.rs:40-43, 62, 86-88`（`write_atomic` / `rename_atomic` / `read_atomic` の doc）、`crates/pulsen/src/adapter/task_repository.rs:96-152, 195-220`

  理由: 3 つの doc はいずれも「最大 511ms ブロックしうる」「排他ロックを保持したまま呼ぶ側では、この遅延がロックの保持時間にそのまま乗る」と**1 呼び出しあたり**の上限を書くが、呼び出し側はこれを合成する。`save_degraded` は `read_atomic` → `write_atomic` で最大 2 倍、`list` は `read_atomic` をエントリ数 N だけ回すので最大 N 倍になる（N の上限は無い）。ロック保持時間への影響を警告するのが doc の目的である以上、読み手が知りたいのは合成後の値のほう。加えて `FIRST_RETRY_WAIT` が 1ms なのに対し、Windows の `thread::sleep` は既定のタイマー粒度（約 15.6ms）に丸め上げられる — 再試行が実際に発火する唯一のプラットフォームで、短い待ちの実測は公称値より大きい（1+2+4+8 の 4 回ぶんで公称 15ms に対し実測 60ms 前後）。ADR-010 / 013 の Consequences もこの 511ms をそのまま根拠に使っているので、数値の出典が 1 つズレると評価も一緒にズレる。

  提案: doc の記述を「1 呼び出しあたり最大 511ms（短い待ちは OS のタイマー粒度で伸びうる）」に直し、走査・読んで書く経路が合成されることを `read_atomic` の doc か `task_repository.rs` のモジュール doc のどちらかに 1 行足す。定数を変える必要は無い。

- **[W-002] 公開 3 関数と `transiently_denied` の結線が、どのプラットフォームのテストでも観測されない**

  場所: `crates/pulsen/src/util/atomic.rs:296-332`（`置換が一時的に拒まれても上限内なら置き換わる` / `置換の一時的な拒否が続けば打ち切られ一時ファイルも残らない`）、`:403-436`（移動側の 2 件）、`:89-95`（`read_atomic`）

  理由: 「上限内なら置き換わる / 移動する」の 2 件は `retry_while_transient` を直接呼び、`persist_with_retry` / `rename_with_retry` の本体を試験側で書き直している。打ち切り側の 2 件だけが薄い当て先を通る。結果として、`write_atomic` → `persist_with_retry` → `transiently_denied`、`rename_atomic` → `rename_with_retry` → `transiently_denied` という**結線そのもの**を検証するテストが 1 件も無い（unix では分類が常に偽なので発火せず、Windows でも決定的に窓を作る手段がない）。テスト名は「置換が」「移動が」と公開関数の振る舞いを名乗っているのに、実際に見ているのは共通ループだけで、名前と検査対象がズレている。`read_atomic` に至っては ADR-010 / 012 が「seam を置く理由」として挙げた分類の差し替え口すら無く、3 つの入り口で形が揃っていない。

  なお `is_transient` は再試行の**前**に呼ばれるので、`persist_with_retry(temp, &target, |_| { let _ = fs::remove_dir(&target); true })` の形にすれば当て先を通したまま同じ状況を作れる（副作用を述語に載せる是非は残る）。

  提案: 少なくともテスト名を検査対象に合わせる（`上限内で解消すれば再試行は成功する` のように共通ループの振る舞いとして名付ける）か、成功側の 2 件を `persist_with_retry` / `rename_with_retry` 経由に寄せる。`read_atomic` に seam を足すかは、足さない理由（読み取りは状態を持たず、ループの検証で尽きる）を doc に 1 行残せば十分。

- **[W-003] `NonZeroU32` の why が実際に防いでいるものを説明していない**

  場所: `crates/pulsen/src/util/atomic.rs:19-23, 141-157`

  理由: doc は「回数を数え上げる側が引き算で 0 を扱わずに済む」と書くが、`retry_while_transient` はカウンタを 0 から増やすだけで引き算はどこにも無い。この型が実際に防いでいるのは**無限ループ**で、打ち切り判定が `attempted == MAX_ATTEMPTS.get()` と等値比較である以上、上限が 0 なら `attempted` は 1 から増え続けて条件が永久に成立しない（`is_transient` が真を返し続ける限りループが返らない）。CLAUDE.md が求める「残すのは現在の形が成り立つ理由」に照らすと、存在しない仕組みを理由として書いている状態になっている。

  提案: doc を「上限 0 は打ち切り条件（等値比較）が成立しない状態を作るので、型で表現不能にする」に直す。等値比較を `>=` にすれば `NonZeroU32` 無しでも安全になるが、型で落とす現在の形のほうが CLAUDE.md の方針に沿うので変える必要は無い。

- **[W-004] 読み手が無制限にスピンする一方で書き手だけが上限を持つ非対称が、テストの中に残っている**

  場所: `crates/pulsen/src/util/atomic.rs:334-364`（`読み手は旧内容か新内容のどちらかだけを観測する`）

  理由: この差分で読み手が `if let Ok(observed) = fs::read(...)` から `read_atomic(...).expect("読み手は常に読める")` に強められた。契約に合わせる意図（ADR-013）は妥当だが、読み手ループはバックオフも譲歩も持たず 64KB の読み取りを回し続けるのに対し、書き手は 10 回・累計 511ms の予算内で窓を取り切れなければ `write_atomic` が `Err` を返し `expect("新内容")` で落ちる。つまりこのテストは、ポート契約が述べていない**時間の性質**（「窓は常に 511ms 以内に閉じる」）を両側から要求している。負荷の高い Windows ランナー（ウイルス対策のスキャンが重なる）で、原因の見えにくい赤として現れうる — plan.md がリスク欄で警戒したのとまさに同じ形。同型の構造は適合スイートの TC-042（本 PR の変更対象外）にもあり、そちらは読み手の遅延が `CONCURRENT_OBSERVATIONS` / `OBSERVATION_WAIT` の予算も食う。

  現時点では run 31661056619（`2c99c1b`）の全 7 ジョブが緑なので、実測上は予算が足りている。マージをブロックするほどではないが、後で不安定な赤が出たときに「再試行が足りない」ではなく「テストが契約より強い」ことが原因だと辿れるようにしておきたい。

  提案: 読み手ループの末尾に `thread::yield_now()` を 1 行入れる（書き手が窓を取れる隙を作るだけで、テストが主張する内容は変わらない）。または、この非対称を doc コメントとして読み手ループに残す。

#### カバレッジ

確認:

- `crates/pulsen/src/util/atomic.rs` — ファイル全体。契約 3 点・再試行の途中と上限到達時の一時ファイル・`cfg` 分岐の網羅・定数と上限の妥当性・テスト 14 件の実効性。`cargo test -p pulsen --lib util::atomic` を実行（14 passed / 1.85s）。
- `crates/pulsen/src/adapter/task_repository.rs` — `read_atomic` を通す 3 箇所（`lookup` / `list` / `save_degraded`）、`NotFound` の扱いと `unreachable_entry` による「消えた / 読めない」の判別、`exists` / `try_exists` を通していない理由、`list` の合成遅延。
- `crates/pulsen/src/adapter/task_file.rs` — `absolute()` / `repo()` / `encoded_repo()` の追加と整形期待値の差し込み（ADR-011）。OS 抽象の観点で `MAIN_SEPARATOR` から組み立てる既存作法と一致していること、整形の検査が緩んでいないことを確認。
- `.thread/10/adr.md` — ADR-008 / 010 / 012 / 013 を通読、ADR-001〜013 の見出しを走査。実装との一致を上記のとおり検証。
- `.thread/10/plan.md` — 受け入れ基準全 8 件、特に AC-5（ドメイン層 grep を実行して 0 件を確認）とスコープ・リスク欄。
- `.github/workflows/ci.yml` — 本観点に関わる範囲（`defaults.run.shell: bash` によるシェルの OS 差の排除、`runner.os != 'Windows'` での非 root アサートの分岐、マトリクスの `fail-fast: false`、`CARGO_INCREMENTAL: 0`）。CI 設計そのものの評価は CI 観点のレビューに委ねる。
- `crates/pulsen-conformance/HOOKS.md` — 3 ランナーの実測表と実測コミット（`af24360`）の明記。OS 差がスキップ集合に現れる形になっていること、Windows でコンパイルされない `#[cfg(all(test, unix))]` の 3 件が SKIP としては現れないと注記されていることを確認。
- `.thread/10/progress.md` / `.thread/10/steps.md` / `.thread/10/testing.md` — `atomic` / 再試行 / 511ms に関する記述を突き合わせ、実装・ADR と矛盾が無いことを確認。
- 変更外の関連コード（同じ原因の残存を探すため）: `crates/pulsen/src/adapter/config_store.rs`、`crates/pulsen/src/adapter/workflow_store.rs`、`crates/pulsen/src/adapter/lock.rs`、`crates/pulsen/src/application/register_task.rs`、`crates/pulsen-conformance/src/task_repository.rs`（TC-042 / 044）、`crates/pulsen/tests/conformance_task_repository.rs`、`crates/pulsen/tests/common/mod.rs`、`crates/pulsen-conformance/src/lib.rs`。

スキップ:

- `.thread/10/review/review-001-ci.md` / `review-001-test-docs.md` / `review-001-util.md` — 前回ラウンドの指摘。ゼロベースでレビューする指示のため参照しない。
- `.thread/10/triage.md` — 上記 3 件のトリアージ結果。同じ理由。
