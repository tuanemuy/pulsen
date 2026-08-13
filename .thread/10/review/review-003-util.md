# レビュー 003 — 共通ユーティリティ・並行性・OS 抽象

対象: PR #12(`issue/10/ci-msrv-cross-platform` ← `main`)
差分取得: `git diff --no-renames --name-status origin/main...HEAD`(18 ファイル / コード 4 ファイル)
契約: `.thread/10/plan.md` / 決定: `.thread/10/adr.md` ADR-008 / 010 / 012 / 013
実測: 最新 CI run 31663925152(`1766c7b` = 現 HEAD)全7ジョブ緑

## 総評

`retry_while_transient` / `transiently_denied` / `MAX_ATTEMPTS` / `FIRST_RETRY_WAIT` への集約は ADR-010 / 012 / 013 の決定どおりで、CLAUDE.md「アトミック性・排他が要る操作は共通のユーティリティに集約し、個別に再実装しない」を満たしている。分類・上限・バックオフの出典は `util/atomic.rs` の1箇所だけで、`adapter/task_repository.rs` 側には再実装が無い。

`write_atomic` / `rename_atomic` の3契約はいずれも保たれている。

- **失敗時に一時ファイルを残さない** — `persist` は失敗時に `PersistError.file` で `NamedTempFile` を返し、`retry_while_transient` が `state` として持ち越す。上限到達時は `state` が関数の戻りとともに drop され、`NamedTempFile` の Drop が削除する。再試行中も一時ファイルは1個を超えない(`置換の一時的な拒否が続けば打ち切られ一時ファイルも残らない` が `entry_names` で実測)。
- **失敗時に対象が変わらない** — `MoveFileEx` / `fs::rename` はいずれも全成功か無変化で、再試行は同じ操作の反復にすぎない。中間状態を新たに作る経路は増えていない。
- **読み手はロックなしで常に一貫した内容を見る** — 書き手側だけだった吸収が読み手側にも揃った(ADR-013)。`FsTaskRepository` の内容読み取り3箇所(`lookup` / `list` / `save_degraded`)がすべて `read_atomic` を通っている。読み手が握るハンドルは unix では inode、Windows では `FILE_SHARE_DELETE` 付きで開いた旧ファイルオブジェクトを指し続けるため、置換が挟まっても内容が混ざる経路は無い。

読み手側の再試行はエラーの意味を変えていない。`transiently_denied` が真とするのは `ERROR_ACCESS_DENIED`(5)/ `ERROR_SHARING_VIOLATION`(32)の2つだけで、`ERROR_FILE_NOT_FOUND`(2)/ `ERROR_PATH_NOT_FOUND`(3)は含まれない。**`read_atomic` は `NotFound` を再試行しない**ことを `時間で解けない拒否に再試行の予算を使わない` が実測しており(`read_atomic` / `rename_atomic` の両方について経過時間 < 511ms をアサート)、`unreachable_entry` による「消えたエントリ(`Ok(None)`)」と「読めないエントリ(`Corrupt`)」の判別は不変。むしろ Windows の delete-pending 窓(open が 5 で拒まれる)を待ち切ってから `NotFound` に到達するので、判別の精度は上がる向きにある。

`cfg` の分岐に穴は無い。`transiently_denied` が `#[cfg(windows)]` / `#[cfg(not(windows))]`、`sync_dir` が `#[cfg(unix)]` / `#[cfg(not(unix))]` で、どちらも2分岐で尽きている。unix では分類が恒常的に偽なので3つの公開関数はいずれも再試行の経路に入らず、挙動は `origin/main` と一致する。AC-5 のドメイン層 grep は 0 件(属性形・マクロ形・`cfg_attr` とも)。吸収先は `crates/pulsen/src/util/` と `crates/pulsen/src/adapter/` に収まり、ADR-008 が「切り出す」側に置いた項目(置換方式の変更・シグネチャや契約の変更・ポート trait・ドメイン層)にはどれも触れていない。`read_atomic` は関数の追加であって既存2関数の契約変更ではない。

抽象化も妥当。`retry_while_transient<S, T>` の `S`(`persist` が一時ファイルを消費し失敗時だけ返す)と `T`(読み取りが値を返す)にはどちらも実在の呼び出し元があり、ADR-012 が退けた `Option` + `unwrap` を持ち込んでいない。`MAX_ATTEMPTS: NonZeroU32` の why(上限 0 は等値比較の打ち切り条件を成立させず無限ループになる)は、`attempted == MAX_ATTEMPTS.get()` という実装と一致している。

同じ原因の取りこぼしを自分で探したが、実害のある残りは見つからなかった(下記カバレッジ参照)。

## 変異テストで確認した歯

`crates/pulsen/src/util/atomic.rs` に1つずつ変異を当て、`cargo test -p pulsen --lib util::atomic`(17件)で確認した。**すべて確認後に元へ戻し、`git status` が clean・`cargo clippy --workspace --all-targets -- -D warnings` と `cargo fmt --all --check` が通ることを確認済み。**

| 変異 | 結果 |
|---|---|
| `MAX_ATTEMPTS` 10 → 1 | 検出(4件が FAILED) |
| `wait *= 2` → `wait *= 1`(バックオフを伸ばさない) | 検出(3件が FAILED) |
| `read_atomic` の本体を `fs::read(path)` に置換(再試行の結線を外す) | **未検出**(17 passed) |
| `wait *= 2` → `wait *= 4`(バックオフの伸び幅) | **未検出**(17 passed、所要 2.62s → 87.52s) |

下限側(上限回数・バックオフが伸びること)には歯があり、上限側(合計待ち時間)には無い。詳細は W-001。

### 共通ユーティリティ・並行性・OS 抽象

#### Blockers

なし。

#### Warnings

- **[W-001] 文書化された 511ms の上限を検証するアサーションが無く、バックオフの伸び幅を変えると全緑のまま予算が170倍になる**

  場所: `crates/pulsen/src/util/atomic.rs:24-27`(`MAX_ATTEMPTS` / `FIRST_RETRY_WAIT`)、`:166`(`wait *= 2`)、`:278-295`(`一時的な拒否が続けば上限の回数だけ試みて元のエラーを返す`)、`:526-530`(`retry_budget`)

  理由: 「1回の呼び出しあたり最大 511ms ブロックしうる」は doc コメント4箇所(`write_atomic` / `rename_atomic` / `read_atomic` / `task_repository.rs` のモジュール doc)が明記し、ADR-010 の Consequences と **ADR-013 が「tick は排他ロックを保持したまま読むので、この遅延がロックの保持時間にそのまま乗る」というトレードオフを受け入れる根拠**にしている数値である。ところがこの値を観測しているテストは1件も無い。既存のテストが押さえているのは (a) 試行回数が `MAX_ATTEMPTS` に等しいこと、(b) 一時的でない拒否では予算を使わないこと(`< retry_budget()`)の2つで、どちらも**再試行が発火したときの合計待ち時間**には触れていない。

  変異テストで確認すると、`wait *= 2` を `wait *= 4` に変えるだけで17件すべてが緑のまま通り、テストスイートの実行時間が 2.62s → 87.52s になる。この変異後の実際の予算は 1+4+16+…+4^8 = 87,381ms ≒ 87秒で、公称 511ms の約170倍。`list` はエントリ数 N だけこれを積み上げ、排他ロックを保持したまま走る(`task_repository.rs:11-13` が自ら書いている合成経路)。数値が静かにこの規模まで動いても CI が緑で通る状態は、doc と ADR が根拠にしている「有界であること」の保証を実際には誰も見ていないことを意味する。

  triage.md の R1 は `util/atomic.rs:テスト/上限の未検証`(「10回・511ms を観測するアサーションが無く、seam を置いた目的が未達」)を fix 判定にしているが、実際に入ったのは「10回」の側だけで、「511ms」の側は残っている。

  提案: 経過時間を測るとランナーの負荷で不安定になるので、待ち時間の列を純粋関数に切り出して和をアサートするのが安定する。例えば `fn wait_before(retry: u32) -> Duration`(`FIRST_RETRY_WAIT * 2u32.pow(retry)`)を `retry_while_transient` と共有し、テストで `(0..MAX_ATTEMPTS.get()-1).map(wait_before).sum::<Duration>() == Duration::from_millis(511)` を確認する。これなら `retry_budget()` がテスト側に持っている式の二重化も同時に消え、doc の 511ms と定数の出典が1つになる。定数そのものを変える必要は無い。

#### カバレッジ

差分の全量(18 ファイル)に対する確認・スキップの対応。

確認:

- `crates/pulsen/src/util/atomic.rs` — **ファイル全体**(1-541行)。3契約の成立、`read_atomic` の `NotFound` 非再試行、`cfg` 2分岐の網羅、`retry_while_transient<S, T>` のシグネチャと型引数の必要性、`NonZeroU32` の why と `attempted == MAX_ATTEMPTS.get()` の整合、`Duration` の倍加が定数値10では溢れないこと、`parent_of` / `sync_dir` の非変更部分。テスト17件を実行(17 passed / 2.62s)し、上表の4変異で歯を確認。
- `crates/pulsen/src/adapter/task_repository.rs` — `read_atomic` へ切り替えた3箇所(`lookup:75` / `list:130` / `save_degraded:201`)、`NotFound` 分岐と `unreachable_entry` の判別が不変であること、モジュール doc が合成経路(2倍・N倍)を明記していること。`exists()` の `symlink_metadata` と `save` / `archive` の `try_exists` を通していないことも確認 — 前者は std の Windows 実装が open 失敗時に `FindFirstFileExW` へフォールバックするため窓でも結果が返り、後者は書き手側の事前確認で、排他ロックで直列化される経路にしか無い(読み手はどちらも通らない)。ADR-013 の除外リストと一致する。なお ADR-013 の除外理由は `ERROR_SHARING_VIOLATION` だけを挙げているが、delete-pending 窓が返すのは `ERROR_ACCESS_DENIED` のほうで、std のフォールバックは両方を対象にしている。結論は変わらないので指摘には上げない。
- `crates/pulsen/src/adapter/task_file.rs` — `absolute()` / `repo()` / `encoded_repo()` の追加(ADR-011)。`MAIN_SEPARATOR` から組み立てる既存作法と一致し、整形(インデント・キー順・末尾改行)の検査が1通りのリテラルのまま保たれていること、差し込む綴りを `serde_json` に作らせてエスケープを手書きしていないことを確認。
- テストの安定性 — `OBSTACLE_LIFETIME`(20ms)で阻害要因を別スレッドが取り除く3件(`置換が…置き換わる` / `移動が…移動する` / `読み取りが…読める`)。本体側の締切は再試行の予算 511ms で、20ms 公称に対して約25倍の余裕がある。不安定になるのはスレッド生成と削除が本体の10回目の試行(t≈511ms)より後ろへずれた場合に限られる。Windows ではタイマー粒度(約15.6ms)で阻害要因の除去も試行間隔も同じ向きに伸びるので余裕はむしろ広がる。`wait *= 1` 変異でこの3件が落ちることから、20ms の窓が実際に再試行を発火させていて空虚ではないことも確認した。**遅い CI で不安定になる形ではない。**
- 並行テスト `読み手は旧内容か新内容のどちらかだけを観測する` — 読み手が `read_atomic(...).expect(...)` へ強められ(ADR-013)、`thread::yield_now()` と `StopOnDrop` が入った形。読み手・書き手ともに 511ms の予算を持ち、`fs::read` の Windows 側共有モードに `FILE_SHARE_DELETE` が含まれるため、置換が読み手のハンドルで拒まれるのはウイルス対策等が割り込んだ場合に限られる。3回の緑の run で再現しているとおり実測上は予算が足りている。ADR-013 が非対称を doc として残す形を採っており、これ以上の対処は「検出したいインターリーブの窓を消す」側に倒れる。
- 公開関数と `transiently_denied` の結線(`read_atomic` の本体を `fs::read` に戻しても全緑)— ADR-012 の Consequences が「分類が空である unix では、公開関数が再試行に入る様子を原理的に観測できない」として3分割の形を明示的に採用しており、テスト (2) の対象を薄い当て先(`*_with_retry`)に限ると宣言している。決定と実装が一致しているため指摘に上げない。`atomic.rs:190-196` の doc コメントがこの限界をコード側にも残している。
- 同じ原因の残存を探した範囲 — `adapter/config_store.rs:48` / `adapter/workflow_store.rs:68` の `read_to_string`(`write_atomic` で書く書き手が存在せず置換の窓が生じない。`grep` で書き手は `task_repository.rs` の3箇所のみと確認)、`adapter/lock.rs`(`try_lock` の非ブロッキングのみで、期限や stale 奪取が無いため再試行の遅延が別プロセスへ波及しない)、`application/`(現時点で tick は未実装で、合成経路は将来の話)、`pulsen-conformance/src/task_repository.rs` の TC-042 / 044(読み手は `repo.find` / `repo.list_*` のポート経由で `read_atomic` を通る。書き手は単一スレッド)、`tests/conformance_task_repository.rs` / `tests/common/mod.rs` の生 `fs::read`(ハーネスの逐次検査用)、`pulsen-conformance/src/lib.rs:258` と `tests/common/mod.rs:554` の権限 probe(ここで再試行を掛けると probe の意味が壊れるので、通していないのが正しい)。**吸収の取りこぼしは無い。**
- `.thread/10/adr.md` — ADR-008 / 010 / 012 / 013 を通読し、実装との一致を上記のとおり検証。ADR-010 の「`rename_atomic` には入れない」が ADR-012 に置き換えられた旨がその場に残されていることも確認。
- `.thread/10/plan.md` — 受け入れ基準8件を通読。AC-5 のドメイン層 grep を実行して 0 件を確認。「レビューで見る観点」の3点(吸収先の層・probe + 許容集合・AC-5 対象外の理由)と、テスト方針の「振る舞いを足した場合のみユニットテストを足す」(「上限内に解ければ置き換わる」「回数を使い切ったら Err を返す」の2つ)を突き合わせた。後者は両方とも入っているが、doc が主張する時間の上限だけが抜けている(W-001)。
- `.github/workflows/ci.yml` — 本観点に関わる範囲のみ。`defaults.run.shell: bash`(36行)でシェルの OS 差が消えていること、非 root アサートが `if: runner.os != 'Windows'`(95行)で分岐すること、`fail-fast: false`(73 / 226行)、`--no-fail-fast`(142行)、`CARGO_INCREMENTAL: 0`(29行)、`timeout-minutes`(43 / 77 / 230行)。CI 設計そのものの評価は CI 観点のレビューに委ねる。
- `crates/pulsen-conformance/HOOKS.md` — OS 差がスキップ集合として現れる形になっていること、Windows で非コンパイルになる `#[cfg(all(test, unix))]` の3件が `SKIP` としても現れないと注記されていることを、OS 抽象の観点から確認。実測値・出典 run の正確性はテスト/ドキュメント観点のレビューに委ねる。
- `.thread/10/progress.md` / `steps.md` / `testing.md` — `atomic` / 再試行 / 511ms に関する記述を実装・ADR と突き合わせ、矛盾が無いことを確認。
- `.thread/10/review/triage.md` — W-001 が R1 の同名指摘に対する fix の残りであることを確かめるためにのみ参照。

スキップ:

- `.thread/10/review/review-001-*.md` / `review-002-*.md` / `review-001.md` — 前回ラウンドの指摘。ゼロベースでレビューする指示のため、指摘の内容は参照せず結論を導いた(`review-002-util.md` は W-001 の重複判定のためだけに事後参照した)。
- `.github/workflows/ci.yml` の CI 設計そのもの(ジョブ構成・MSRV 版数の読み出し・SKIP サマリーの抽出) — CI 観点の担当。
- `crates/pulsen-conformance/HOOKS.md` の実測値の正しさ — テスト/ドキュメント観点の担当。
