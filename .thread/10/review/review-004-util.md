# レビュー 004 — 共通ユーティリティ・並行性・OS 抽象

対象: PR #12(`issue/10/ci-msrv-cross-platform` ← `main`)
差分取得: `git diff --no-renames --name-status origin/main...HEAD`(23 ファイル / コード 3 ファイル + ワークフロー1 + ドキュメント19)
契約: `.thread/10/plan.md` / 決定: `.thread/10/adr.md` ADR-008 / 010 / 012 / 013
実測: run 31665658371(`e19c973`)全7ジョブ success。`git log -1 -- '*.rs' .github/ Cargo.toml Cargo.lock` が `e19c973` を指し、それ以降の3コミットはドキュメントのみ(`.thread/` と `HOOKS.md`)。**CI が実測したコードと現 HEAD のコードは同一。**

## 結論

**マージできる状態。** 本観点の Blocker・Warning ともに無し。

## 総評

再試行の実体は `util/atomic.rs` の1箇所に集約されている。分類(`transiently_denied`)・上限(`MAX_ATTEMPTS`)・初回の待ち(`FIRST_RETRY_WAIT`)・待ちの列(`retry_waits`)・ループ(`retry_while_transient`)がすべて同じファイルにあり、呼び出し側(`adapter/task_repository.rs`)には再実装が無い。CLAUDE.md「アトミック性・排他が要る操作は共通のユーティリティに集約し、個別に再実装しない」を満たす。

契約は3つとも保たれている。

- **失敗時に一時ファイルを残さない** — `persist` は失敗時に `PersistError.file` で `NamedTempFile` を返し、`retry_while_transient` が `state` として持ち越す。打ち切り時は `state` が関数の戻りとともに drop され、Drop が削除する。再試行中も一時ファイルは1個を超えない。
- **失敗時に対象が変わらない** — `MoveFileEx` / `fs::rename` は全成功か無変化で、再試行は同じ操作の反復にすぎない。中間状態を作る経路は増えていない。
- **読み手はロックなしで常に一貫した内容を見る** — 吸収が読み手側にも揃った(ADR-013)。`FsTaskRepository` の内容読み取り3箇所(`lookup:75` / `list:130` / `save_degraded:201`)がすべて `read_atomic` を通り、それ以外に内容を読む経路が無いことを `grep`(`fs::read(` / `read_to_string(`)で確認した。残る `config_store:48` / `workflow_store:68` は `write_atomic` で書く書き手が存在せず、置換の窓が生じない。

エラー分類は意味を変えていない。真になるのは `ERROR_ACCESS_DENIED`(5)/ `ERROR_SHARING_VIOLATION`(32)の2つだけで、`ERROR_FILE_NOT_FOUND`(2)/ `ERROR_PATH_NOT_FOUND`(3)/ `ERROR_DISK_FULL`(112)は含まれない。`unreachable_entry` による「消えたエントリ(`Ok(None)`)」と「読めないエントリ(`Corrupt`)」の判別は不変で、`NotFound` は初回で返る。

上限とバックオフは `retry_waits()` に一本化されている。`successors(1ms, *2).take(MAX_ATTEMPTS - 1)` の和が 511ms で、ループはこの列だけを消費する。試行回数は列の本数 + 1 = 10 で、`MAX_ATTEMPTS` と一致する。`NonZeroU32` の why(0 だと `- 1` がリリースで巻き戻り、列が上端まで伸びる)は `take(MAX_ATTEMPTS.get() as usize - 1)` という実装と一致している。`Duration * 2` の反復は `take` が9本で止まるため最大 512ms までしか計算されず、溢れない。

`cfg` は網羅している。`transiently_denied` が `#[cfg(windows)]` / `#[cfg(not(windows))]`、`sync_dir` が `#[cfg(unix)]` / `#[cfg(not(unix))]` で、どちらも2分岐で尽きる。unix では分類が恒常的に偽なので3つの公開関数はいずれも再試行に入らず、挙動は `origin/main` と一致する。AC-5 のドメイン層 grep は 0 件(属性形・マクロ形・`cfg_attr` とも)。

ADR-008 の停止規則にも収まっている。触れたのは `crates/pulsen/src/util/` と `crates/pulsen/src/adapter/` だけで、「切り出す」側に置かれた項目(`persist` を使わない置換方式・シグネチャや契約の変更・ポート trait・ドメイン層)にはどれも当たっていない。`read_atomic` は関数の追加であって既存2関数の契約変更ではない。

抽象化も過剰でない。`retry_while_transient<S, T>` の `S`(`persist` が一時ファイルを消費し失敗時だけ返す)と `T`(読み取りが値を返す)にはどちらも実在の呼び出し元があり、ADR-012 が退けた `Option` + `unwrap` を持ち込んでいない。分類を引数に取る薄い当て先3つ(`persist_with_retry` / `rename_with_retry` / `read_with_retry`)は、打ち切りの検証を分類から独立させるための seam として実際にテストから使われている。

前ラウンドの W-001(公称 511ms に歯が無く、バックオフの伸び幅を変えても全緑)は解消している。変異 `*wait * 2` → `*wait * 4` を当てると `再試行に費やす待ちの合計は公称する上限と一致する` が落ちる(下表)。

## 変異テストで確認した歯

`crates/pulsen/src/util/atomic.rs` に1つずつ変異を当て、`cargo test -p pulsen --lib util::atomic`(18件)で確認した。**すべて `git checkout --` で元に戻し、`git status` が clean、`cargo test --workspace`(全件 pass)・`cargo clippy --workspace --all-targets -- -D warnings`・`cargo fmt --all --check` が通ることを確認済み。**

| 変異 | 結果 |
|---|---|
| `MAX_ATTEMPTS` 10 → 9 | 検出(`再試行に費やす待ちの合計…` FAILED) |
| `take(MAX_ATTEMPTS - 1)` → `take(MAX_ATTEMPTS)`(off-by-one) | 検出(`再試行に費やす待ちの合計…` / `一時的な拒否が続けば上限の回数だけ…` FAILED) |
| `FIRST_RETRY_WAIT` 1ms → 2ms | 検出(`再試行に費やす待ちの合計…` FAILED) |
| `*wait * 2` → `*wait * 4`(バックオフの伸び幅) | 検出(`再試行に費やす待ちの合計…` FAILED。実行時間 1.6s → 87.6s) |
| 非一時的の早期 return を無効化 | 検出(`一時的でない拒否は再試行せずに返る` FAILED) |
| `#[cfg(not(windows))]` の分類を `true` に | 検出(`一時的な拒否と分類されるのは…` / `時間で解けない拒否に…` FAILED) |
| `read_atomic` の分類を `\|_\| true` に | 未検出(18 passed) |
| `read_atomic` の本体を `fs::read(path)` に戻す | 未検出(18 passed) |
| `thread::sleep(wait)` → `thread::sleep(wait * 4)` | 未検出(18 passed) |

未検出の3件について。前2件は「公開関数と分類の結線」で、ADR-012 の Consequences が「分類が空である unix では、公開関数が再試行に入る様子を原理的に観測できない」として明示的に受け入れた限界そのものであり、`atomic.rs:201-204` の doc がコード側にも残している。3件目は「ループが列の値どおりに眠るか」で、閉じるには壁時計の計測か sleep の注入が要る。前者は ADR-012 / triage R3 が「遅い Windows ランナーで実装と無関係な赤になる」として退けた形に戻ることになり、後者は1行の直接消費(`thread::sleep(wait)`)のために seam を1つ増やす。**いずれも取り違えの実害は「有界な遅延が伸びる」ことに限られ、決定と代替案の比較が済んでいる**ため、指摘には上げない。

## 共通ユーティリティ・並行性・OS 抽象

#### Blockers

なし。

#### Warnings

なし。

#### カバレッジ

差分の全量(23 ファイル)に対する確認・スキップの対応。

確認:

- `crates/pulsen/src/util/atomic.rs` — **ファイル全体**(1-564行)。3契約の成立、`read_atomic` の `NotFound` 非再試行、`cfg` 2分岐の網羅、`retry_while_transient<S, T>` の型引数の必要性、`NonZeroU32` の why と `take(… - 1)` の整合、試行回数 = 列の本数 + 1 = `MAX_ATTEMPTS` の一致、`Duration` 倍加が溢れないこと、`parent_of` / `sync_dir` の非変更部分。「列を先に見てから分類を問う」順序が最終試行で分類器を呼ばない意図と一致すること(doc:171)も確認。テスト18件を実行(18 passed / 1.60s)し、上表の9変異で歯を確認。
- `crates/pulsen/src/adapter/task_repository.rs` — `read_atomic` へ切り替えた3箇所と、`NotFound` 分岐・`unreachable_entry` の判別が不変であること。モジュール doc が合成経路(`save_degraded` 2倍・`list` N 倍)を明記していること。`exists()` の `symlink_metadata` と `save` / `archive` の `try_exists` を通していないことは ADR-013 の除外リストと一致する(前者は std の Windows 実装が open 失敗時に `FindFirstFileW` へフォールバックし、後者は排他ロックで直列化される書き手側の事前確認で、読み手はどちらも通らない)。
- `crates/pulsen/src/adapter/task_file.rs` — `absolute()` / `repo()` / `encoded_repo()` の追加(ADR-011)。`MAIN_SEPARATOR` から組み立てる既存作法と一致し、整形(インデント・キー順・末尾改行)の検査が1通りのリテラルのまま保たれ、差し込む綴りを `serde_json` に作らせている。OS 差の吸収がフィクスチャに閉じ、`RepoPath` 側の検証は緩めていない。
- `.github/workflows/ci.yml` — 本観点に関わる範囲のみ。`defaults.run.shell: bash` でシェルの OS 差が消えていること、非 root アサートが `runner.os != 'Windows'` で分岐すること、`fail-fast: false` / `--no-fail-fast` / `CARGO_INCREMENTAL: 0` / `timeout-minutes`。CI 設計そのものの評価は CI 観点に委ねる。
- `.thread/10/adr.md` — ADR-008 / 010 / 012 / 013 を通読し、実装との一致を上記のとおり検証。ADR-010 の「`rename_atomic` には入れない」が ADR-012 に置き換えられた旨がその場に残っていること、ADR-012 が挙げた「上限は `NonZeroU32`」「分類は1つ」「薄い当て先を残す」がすべて実装に現れていることを確認。
- `.thread/10/plan.md` — 受け入れ基準8件を通読。AC-5 のドメイン層 grep を実行して 0 件を確認し、吸収先が `crates/pulsen/src/util/` と `crates/pulsen/src/adapter/` に収まっていること、スコープの除外項目(置換方式の設計変更・`cfg` 決め打ち)に踏み込んでいないことを確認。テスト方針の「振る舞いを足した場合のみユニットテストを足す」(「上限内に解ければ置き換わる」「回数を使い切ったら Err を返す」)は3つの入り口すべてに揃っており、加えて公称 511ms にも歯が立った。
- `crates/pulsen-conformance/HOOKS.md` — OS 差がスキップ集合として現れる形になっていること、出典 run が `e19c973` に揃っていることを OS 抽象の観点から確認。実測値の正しさはテスト/ドキュメント観点に委ねる。
- `.thread/10/progress.md` / `.thread/10/steps.md` / `.thread/10/testing.md` — `atomic` / 再試行 / 511ms / `read_atomic` に関する記述を実装・ADR と突き合わせ、矛盾が無いことを確認。
- `.thread/10/review/triage.md` — R3 の2件(`上限511msの未検証` / `壁時計依存のアサート`)が fix 判定で、実際に `retry_waits()` の切り出しと壁時計比較の除去として入っていることの照合にのみ参照。
- `.thread/10/review/review-003-util.md` — 前ラウンドの W-001 が解消済みであることの照合にのみ事後参照。結論は変異テストの実測から独立に導いた。

スキップ:

- `.thread/10/review/review-001-util.md` / `review-002-util.md` / `review-001.md` / `review-002.md` / `review-003.md` — 前ラウンドの指摘。ゼロベースでレビューする指示のため参照しない。
- `.thread/10/review/review-001-ci.md` / `review-002-ci.md` / `review-003-ci.md` — CI 観点の担当。
- `.thread/10/review/review-001-test-docs.md` / `review-002-test-docs.md` / `review-003-test-docs.md` — テスト/ドキュメント観点の担当。
- `.github/workflows/ci.yml` の CI 設計そのもの(ジョブ構成・MSRV 版数の読み出し・SKIP サマリーの抽出・前提検査) — CI 観点の担当。
- `crates/pulsen-conformance/HOOKS.md` の実測値・期待集合の正しさ — テスト/ドキュメント観点の担当。
