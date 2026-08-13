# レビュー 001 — 共通ユーティリティ・並行性・OS 抽象

対象: PR #12（`issue/10/ci-msrv-cross-platform`、ベース `main`）
契約: `.thread/10/plan.md` / `.thread/10/adr.md`（ADR-008 / ADR-010 / ADR-012）

## 共通ユーティリティ・並行性・OS 抽象

### Blockers

なし。

`write_atomic` / `rename_atomic` の3つの契約（失敗時に一時ファイルを残さない・対象が変わらない・読み手はロックなしで常に一貫した内容を見る）は、再試行の導入で崩れていない。ADR-008 が引いた境界（置換方式・シグネチャ・契約・ポート trait・ドメインを変えない）も越えていない。AC-5 の機械確認（`crates/pulsen-domain/src/` の target 述語つき `cfg` が 0 件）は再測して 0 件を確認した。

### Warnings

- **[W-001]** 同じ Windows 共有違反の**読み手側**が未処理のまま残っており、ADR-012 の「044 が Windows で最初に走る時点で既に吸収されている」は書き手側の半分しか成立していない
  - 場所: `crates/pulsen/src/adapter/task_repository.rs:65`（`lookup`）/ `crates/pulsen/src/adapter/task_repository.rs:151`（`list`）、`crates/pulsen-conformance/src/task_repository.rs:705`・`843`
  - 理由: `MoveFileExW(MOVEFILE_REPLACE_EXISTING)` は置き換えられる側を delete-pending に落とす。その窓で `CreateFileW` が走ると `STATUS_DELETE_PENDING` → `ERROR_ACCESS_DENIED`(5) になる。ウイルス対策が `FILE_SHARE_DELETE` 無しでハンドルを握る経路も同じコードを返す。**書き手が踏んだのとまったく同じ原因が、読み手にも当たる。**
    `lookup` / `list` は `NotFound` 以外の `fs::read` エラーを `ReadError::Io` に写す（`task_repository.rs:73-77` / `:139-143`）ため、これは `find` / `list_active` の `Err` として上がる。そして TC-042 / TC-044 の読み手スレッドは `Ok(TaskLookup::…(Intact))` 以外をすべて `panic!("完全な保存内容だけが観測される")` で落とす — **スキップにも許容集合にもならず、原因の見えない Windows のフレークとして現れる。**
    このズレは既にコードの中に矛盾として現れている。`util/atomic.rs:270-272` の既存ユニットテストは「置換の途中でファイルを開けない瞬間はプラットフォームによってありうる」と明記して `if let Ok(observed)` で読み取り失敗を**許容している**のに、同じ契約をポート水準で述べた TC-042/044 は許容していない。同じ契約について2つのテストが違うことを主張している。
    加えて利用者に見える面がある。Windows で `pulsen status`（`find`）が tick の `save` と重なると、内容が壊れているわけでもないのに入出力エラーで落ちうる。
  - 提案: 3つのうちどれかを選び、選んだ理由を残す。(a) `FsTaskRepository` の読み取りにも `util/atomic.rs` と同じ分類 + 上限で再試行を掛ける（吸収先は `crates/pulsen/src/adapter/` なので ADR-008 の「本 Issue で扱う」側に収まる。ただし分類器を `util` から公開する必要がある）。(b) 本 Issue の射程外と判定し、**別 Issue へ切り出す**（ADR-008 の「迷ったら切り出す」）。(c) 少なくとも `.thread/10/progress.md` の「残っている見立て」に `spawn_holder` の項と並べて記録する。
    どれを採るにせよ、ADR-012 の Consequences「044 が Windows で最初に走る時点で既に吸収されている」は「書き手側は吸収されている」に限定して書き直したい。今の書き方だと、後で 042/044 が Windows で落ちたときに「吸収済みのはず」から調査が始まる。
    なお ADR-012 は「042 で同一原因が実測済み」を根拠に未観測の `rename_atomic` へ先回りしている。同じ論法は読み手側にもそのまま当たるので、ここだけ「未観測だから触らない」に戻すと判断基準が一貫しない。

- **[W-002]** 再試行の**上限そのもの**（`MAX_ATTEMPTS` = 10 / 累計 511ms）を検証しているテストが無く、ADR-010 が seam を置いた目的が果たされていない
  - 場所: `crates/pulsen/src/util/atomic.rs:244`・`:344`
  - 理由: 打ち切り側の2件が主張しているのは `error.raw_os_error().is_some()` だけで、これは「独自エラーに差し替えていない」ことしか言わない。試行回数にはまったく触れていない。上限内で成功する2件（`:226`・`:327`）と合わせても、機械的に固定されているのは「2回以上試みる」「無限には回らない」の2点だけで、**10 という値もバックオフの段取りもテストから落ちない。**
    ADR-010 は「再試行の上限と『元のエラーを返す』ことが、Windows を待たずにユニットテストで検証される」と書き、ADR-012 はその seam を移動側にも広げた理由として同じことを挙げている。実装された seam のコストは払っているのに、それが買うはずだったもの（上限の検証）が入っていない。
    さらに上限がテストへ間接的に効いている。打ち切り2件はそれぞれ実時間で 511ms 眠る（実測: `cargo test -p pulsen --lib util::atomic -- --test-threads=1` が 2.14s、うち約 1.02s がこの2件の `sleep`）。`MAX_ATTEMPTS` を上げると待ちは `2^(N-1)` ms で伸びるので、上限を誤って大きくした変更は**アサーション失敗ではなくハング**として現れる。上限を数えるアサーションはこの経路も同時に塞ぐ。
  - 提案: `retry_while_transient` は同一モジュール内の私有関数で、テストから直接呼べる。`S = u32` のカウンタを持ち回して呼べば、ファイルシステムを介さずに上限と持ち越しの両方が固定できる。

    ```rust
    #[test]
    fn 一時的な拒否が続けば上限の回数だけ試みて打ち切る() {
        let attempted = std::cell::Cell::new(0);
        let error = retry_while_transient(
            (),
            |()| {
                attempted.set(attempted.get() + 1);
                Err((io::Error::from_raw_os_error(5), ()))
            },
            |_| true,
        )
        .expect_err("打ち切られる");

        assert_eq!(attempted.get(), MAX_ATTEMPTS);
        assert_eq!(error.raw_os_error(), Some(5));
    }
    ```

    既存の2件（`persist_with_retry` / `rename_with_retry` 経由）は「持ち越した状態で次の試行が成功する」「打ち切っても一時ファイルが残らない」を見ているので、そちらは残す価値がある。

- **[W-003]** `is_transient` が2つのテストで「次の試行を成功させる副作用フック」として使われており、引数の文書化された契約（分類）と実際の用法が食い違う
  - 場所: `crates/pulsen/src/util/atomic.rs:232-236`・`:332-336`
  - 理由: 述語の中で `fs::remove_dir(&target)` / `fs::write(&from, b"content")` を実行している。関数の doc（`:101-102`）は「分類を引数に取るのは、再試行の打ち切り方を、エラーがどう分類されるかと独立に検証できるようにするため」と書いており、副作用の実行は想定に入っていない。
    実害の芽が1つある。`retry_while_transient:118` は `if remaining == 0 || !is_transient(&error)` と短絡するので、**最終試行では `is_transient` が呼ばれない。** 現状の2件は2回目で成功するため踏まないが、「述語が毎回呼ばれる」ことに依存したテストを後から書くと静かに壊れる。分類器として読むなら短絡は正しい最適化で、フックとして読むなら抜けである。同じ引数が2つの意味を持っている状態が原因。
  - 提案: W-002 の直接テストに寄せて、この副作用フックの用法を消す。`retry_while_transient` を直接呼ぶ形なら、状態を進めるのは `attempt` 側の責務になるので、述語を副作用の置き場にする必要がなくなる。

- **[W-004]** `retry_while_transient<S, T>` の型引数 `T` が両方の呼び出し元で `()` に固定されており、抽象の幅が使われていない
  - 場所: `crates/pulsen/src/util/atomic.rs:103-107`
  - 理由: `persist_with_retry` は `Ok(_) => Ok(())`、`rename_with_retry` は `fs::rename(...)` の `()` で、`T` が `()` 以外になる呼び出しは存在しない。ADR-012 が「型引数を2つ持つ」ことのトレードオフとして受け入れているのは `S`（`persist` が一時ファイルを消費して返す都合）だけで、`T` にはその根拠が無い。共通ユーティリティのシグネチャは呼び出し側が読むものなので、根拠のある複雑さと無い複雑さは分けたい。
  - 提案: `T` を落として `-> io::Result<()>` にする。`attempt: impl FnMut(S) -> Result<(), (io::Error, S)>`。ADR-012 の「トレードオフ: 共通ループが型引数を2つ持ち」も1つに書き直せる。

- **[W-005]** `MAX_ATTEMPTS - 1` が定数値に依存した引き算で、0 を与えると壊れる形になっている
  - 場所: `crates/pulsen/src/util/atomic.rs:108`
  - 理由: `let mut remaining = MAX_ATTEMPTS - 1;` は定数畳み込みされない実行時の引き算。`MAX_ATTEMPTS` を 0 にした瞬間、debug では減算オーバーフローで panic、release では `u32::MAX` に巻き戻って事実上無限に再試行し、やがて `wait *= 2`（`:123`）が Duration の乗算オーバーフローで panic する。CLAUDE.md「不正な状態を型で表現不能にする」「パニックは不変条件違反にのみ使う」に対して、定数の値でしか守られていない。
  - 提案: `const MAX_ATTEMPTS: NonZeroU32` にするか、引き算を消して数え上げにする。後者なら「10回試みる」がコードの字面と一致し、W-002 のアサーションとも読み比べやすい。

    ```rust
    for attempt in 1..=MAX_ATTEMPTS {
        match attempt_fn(state) { ... }
        if attempt == MAX_ATTEMPTS || !is_transient(&error) { return Err(error); }
        thread::sleep(wait);
        wait *= 2;
    }
    ```

- **[W-006]** 呼び出し元から見た `write_atomic` / `rename_atomic` の**遅延特性**が変わったのに、公開関数の doc にも ADR の Consequences にも「最大 511ms ブロックしうる」ことが書かれていない
  - 場所: `crates/pulsen/src/util/atomic.rs:21-32`（`write_atomic` の doc）・`:46-49`（`rename_atomic` の doc）
  - 理由: ADR-010 は「**失敗が**最大 511ms 遅れる」とだけ書いているが、実際には成功する呼び出しも途中の共有違反を吸収するぶん遅れる。`FsTaskRepository::save` / `create` / `save_degraded` / `archive` はグローバルな排他ロックを保持した tick の中から呼ばれるので、遅延はロック保持時間に直に乗る。タスク数ぶん積み上がったときの上限（N × 511ms）が、共通ユーティリティの doc からは読み取れない。
    合わせて、一時ファイルが残りうる**クラッシュ窓**も広がっている。従来は `persist` 前後の数マイクロ秒だったものが、再試行の間ずっと（最大 511ms）一時ファイルを抱えた状態になる。契約「失敗した場合は一時ファイルを残さない」は関数が返る限り守られている（`:254` のアサーションで確認済み）が、その途中で落ちた場合は対象外で、掃除する主体もいない。実害は小さい（`list` は `TaskFilePath::parse_file_name` で弾くので `.tmpXXXX` は走査に混ざらない。`task_repository.rs:113-115`）が、事実として記録はしておきたい。
  - 提案: `write_atomic` / `rename_atomic` の doc に「一時的な拒否を吸収するため最大 511ms 待つことがある」の1行を足す。クラッシュ窓は ADR-010 か ADR-012 の Consequences に1行。

## 契約が保たれているかの検証

再試行の導入で崩れうる箇所を1つずつ当たった。結論はいずれも「崩れていない」。

| 契約 | 検証 | 結果 |
|---|---|---|
| 失敗時に一時ファイルを残さない（上限到達時） | `retry_while_transient` の `state` は関数のローカル。`Err` で抜けると `NamedTempFile` が drop され `remove_file` が走る。テスト `一時的な拒否が続けば上限で打ち切って元のエラーを返す:254` が `entry_names` で実測 | 保たれる |
| 失敗時に一時ファイルを残さない（再試行の途中） | tempfile 3.27.0 の `NamedTempFile::persist`（`src/file/mod.rs:767`）は失敗時に `PersistError { file, error }` として **path と file ハンドルを揃えたまま** 返す。Windows 実装（`src/file/imp/windows.rs:90`）は `MoveFileExW` 失敗時に `FILE_ATTRIBUTE_TEMPORARY` を戻す。よって持ち越した一時ファイルは次の試行でも同じ実体を指し、最終的に drop で消える | 保たれる（W-006 のクラッシュ窓を除く） |
| 対象が変わらない | `MoveFileExW` / `rename` は成功か失敗のどちらかで、部分置換の中間状態を作らない。再試行は失敗した後にしか走らない | 保たれる |
| 読み手はロックなしで常に一貫した内容を見る | **書き手側は保たれる**。読み手側は W-001 |
| シグネチャ・置換方式（ADR-008 の切り出し対象） | `pub fn write_atomic(&Path, &[u8]) -> io::Result<()>` / `pub fn rename_atomic(&Path, &Path) -> io::Result<()>` は無変更。`persist` / `fs::rename` の手順も無変更 | 越えていない |

## エラー分類の妥当性

- **`ERROR_ACCESS_DENIED`(5) の握り潰し**: ADR-010 が明示的に受け入れたトレードオフで、Win32 のコードから「読み手が開いている」と「本当の権限不足・読み取り専用属性・ウイルス対策のロック」は区別できない。上限を持つことで遅延を有界にする、という落とし方は妥当。分類を `PermissionDenied`（`io::ErrorKind`）でなく `raw_os_error()` の生コードに置いたのも正しい — `ErrorKind` で分類すると unix の `EACCES` が同じ穴に落ち、「unix では再試行の経路に一度も入らない」が崩れる。
- **上限到達時のエラー**: `retry_while_transient:119` は `return Err(error)` で**最後の試行が返した OS エラーをそのまま**返す。独自の打ち切りエラーへの差し替えも、`io::Error::new` によるラップもしていないので `raw_os_error()` / `kind()` が呼び出し元まで保たれる。`FsTaskRepository` 側は `format!("{}: {error}", ...)` で文字列化するだけなので、情報の欠落は無い。
  - ただし返るのは「最初のエラー」ではなく「最後のエラー」である。10回とも同じ拒否なら同じだが、途中で原因が変わった場合（5 → 32、あるいは 5 → 別の恒久エラー）に見えるのは最後の1つだけ。分類が真である限り再試行を続ける以上これが自然で、ADR の「元のエラーをそのまま返す」とも矛盾しない。指摘にはしない。
- **待ち時間**: 1, 2, 4, …, 256ms の9回で累計 511ms、`wait` は最大 512ms までしか育たないので Duration のオーバーフローは無い（`MAX_ATTEMPTS` が現在値である限り。W-005）。読み手の `fs::read`（64KB）はマイクロ秒オーダーなので、初回の 1ms でほぼ解ける想定は妥当。

## unix 側の挙動

- `#[cfg(windows)]` / `#[cfg(not(windows))]` の対で `transiently_denied` を定義しており、**述語の組に穴が無い**（どの target でも定義が過不足なく1つ）。
- unix では常に `false` を返すので `retry_while_transient:118` の `!is_transient(&error)` が初回で真になり、**`thread::sleep` に一度も到達しない**。挙動は実装前と一致する。
- 同ファイルの `sync_dir` は `unix` / `not(unix)` で分かれており述語が違うが、これは意味が違う（ディレクトリを開けるか vs. Win32 のエラーコードを持つか）ので揃える必要はない。Windows は `not(unix)` 側 = no-op、`windows` 側 = コード分類、と両方が正しく効く。
- `crates/` 全体の target 述語つき `cfg` を再走査した結果、増えたのは `util/atomic.rs` の2件（`transiently_denied` の対）だけで、plan.md AC-5 が実測した分布から他は動いていない。`crates/pulsen-domain/src/` は 0 件のまま。

## 他に同じ原因が残っていないか

`fs::rename` / `persist` / ファイルハンドルを跨ぐ操作をワークスペース全体で走査した（`fs::rename|\.persist\(|remove_file|remove_dir_all|remove_dir\(|File::create|OpenOptions|fs::copy`）。

| 箇所 | 判定 |
|---|---|
| `crates/pulsen/src/adapter/task_repository.rs:65`（`lookup` の `fs::read`）・`:151`（`list` の `fs::read`） | **W-001**。同一原因の読み手側 |
| `crates/pulsen/src/adapter/task_repository.rs:203`（`save_degraded` の `fs::read`） | 同上。読み書きを跨ぐが、書き手は自プロセスの直列呼び出しなので W-001 に含めて扱えばよい |
| `crates/pulsen/src/adapter/lock.rs:31`（`OpenOptions::open` + `try_lock`） | 問題なし。Rust の既定 share mode は `READ|WRITE|DELETE` で、`LockFileEx` の競合は `TryLockError::WouldBlock` に写る。共有違反の経路ではない |
| `crates/pulsen/src/util/fsdir.rs:12`（`create_dir_all`） | 指摘にしない。delete-pending のディレクトリに当たる理論上の経路はあるが、このコードベースはディレクトリを消さない |
| `crates/pulsen/tests/conformance_task_repository.rs:121`（`fs::copy`）・`:174`（`remove_file`）、`crates/pulsen/tests/common/mod.rs:124`、`crates/pulsen-conformance/src/lib.rs:262` | 問題なし。いずれもフィクスチャ／probe の直列操作で、並行する書き手がいない |
| `crates/pulsen/src/adapter/worktree.rs` | 直接のファイル操作を持たず git へシェルアウトするのみ。本 PR の差分外 |

`MoveFileEx` を踏むのは `write_atomic`（`persist` 経由）と `rename_atomic`（`fs::rename` 経由）の2つだけで、**書き手側はこの PR で両方とも塞がっている。** 残っているのは読み手側だけ（W-001）。

## CLAUDE.md の原則との整合

- 「アトミック性・排他が要る操作は共通のユーティリティに集約し、個別に再実装しない」— **適合。** ADR-012 が選択肢1（`rename_atomic` に同じループを書く）を明示的に退けており、分類・上限・バックオフの出典が `retry_while_transient` の1箇所に保たれている。片方だけ値が動く事故が構造的に起きない。
- 「OS 依存の処理はアダプター層に隔離する」— **適合と判定。** `crates/pulsen/src/util/` は字義どおりのアダプター層ではないが、plan.md「レビューで見る観点」が吸収先として `crates/pulsen/src/util/` を明示的に挙げており、`util/atomic.rs` は既に `sync_dir` を cfg で分けている OS 抽象の置き場である。ドメイン（`crates/pulsen-domain/`）へは 1 件も漏れていない（AC-5 再測で 0 件）。
- 「エラーは値として返す。パニックは不変条件違反にのみ使う」— 適合。新規コードに `unwrap` / `expect` / `panic!` は無い。ADR-012 が `Option` + `unwrap` を避けるために型引数 `S` を選んだ判断も、この原則に沿っている（W-005 の潜在的な減算オーバーフローだけが例外で、定数の値でしか守られていない）。
- 「不正な状態を型で表現不能にする」— W-005 が唯一の指摘点。
- 「分岐は網羅する。`match` でワイルドカードを避ける」— 適合。`retry_while_transient` の `match` は `Ok` / `Err` の2腕、`transiently_denied` の `matches!` はパターン照合で `_` を使っていない。
- テスト方針「テストは振る舞いを表す」— 名前は仕様の言葉になっている（「置換が一時的に拒まれても上限内なら置き換わる」等）。中身の実効性は W-002 / W-003。

## 抽象化の妥当性（`retry_while_transient` のシグネチャ）

ADR-012 が挙げた3案のうち、案3（ループだけを切り出し、薄い当て先を2つ残す）を採ったのは妥当。案1は分類・上限・バックオフを二重化して CLAUDE.md に反する。案2は「置換か移動か」をフラグで渡す形になり、呼び出し側に条件分岐が漏れる。

型引数 `S`（失敗時に状態を持ち越す）についても、より単純な形を検討したが見つからなかった。`Option<NamedTempFile>` + `take().unwrap()` は不変条件違反でないところに `unwrap` を作る。クロージャに `NamedTempFile` をキャプチャさせる形は `FnMut` で消費できないので成立しない。**`S` は `persist` の都合が必然的に押し出したもので、複雑さに見合っている。**

見合っていないのは `T`（W-004）と、2つの薄い関数が両方とも `is_transient` を引数に取ること。ただし後者については、`置換が一時的に拒まれても上限内なら置き換わる` が「持ち越した `NamedTempFile` で次の試行が本当に成功する」を検証しており、unix では `transiently_denied` が常に偽なのでこの seam 無しには検証できない。**`persist_with_retry` 側の seam には根拠がある。** `rename_with_retry` 側は `S = ()` なので持ち越しの検証価値が無く、W-002 の直接テストで代替できるが、対称性を優先した判断も理解できる範囲なので指摘にはしない。

## その他（指摘としては挙げないもの）

- `adapter/task_file.rs` の `absolute(&[…])` は、`pulsen-domain` の `task/path.rs` / `task/task.rs` / `task/degraded.rs` に既にある同名ヘルパーの4つ目のコピーになる。ADR-011 が「既存の作法に合わせる」と決めており、かつ3つはクレートが違って共有には test-support クレートが要るため、CI 導入 PR の射程で統合するのは筋が悪い。現状の判断でよい。
- 同ファイルの `<repo>` プレースホルダー方式（ADR-011）は、整形の期待値を1通りに保ったままプラットフォーム差だけを追い出せており、代替案（構造比較・期待値の二重化）より良い。`serde_json::to_string` にエスケープを作らせているのでダブルクォート込みで置換される点も正しい（期待値側が `"repo": <repo>` とクォート無しで書かれていることと整合）。
- 打ち切り2件の実時間 511ms ずつは `cargo test` の並列実行に吸収される。テスト全体への影響は無視できる。

## カバレッジ

一覧9件と1対1。

- 確認: `crates/pulsen/src/util/atomic.rs`（差分＋ファイル全体）, `crates/pulsen/src/adapter/task_file.rs`, `crates/pulsen-conformance/HOOKS.md`, `.github/workflows/ci.yml`, `.thread/10/plan.md`, `.thread/10/adr.md`, `.thread/10/progress.md`
- 確認（差分外だが本観点で必要だったもの）: `crates/pulsen/src/adapter/task_repository.rs`（`save` / `archive` / `lookup` / `list`）, `crates/pulsen/src/adapter/lock.rs`, `crates/pulsen/src/util/fsdir.rs`, `crates/pulsen-conformance/src/task_repository.rs`（TC-042 / TC-044）, `crates/pulsen/tests/conformance_task_repository.rs`, tempfile 3.27.0 の `persist` 実装
- スキップ: `.thread/10/steps.md` — 実装手順書であり、手順の妥当性は生成物（`atomic.rs` / `ci.yml`）側で判定したため本観点の判断材料にならない
- スキップ: `.thread/10/testing.md` — CI の検証手順書で、並行性・OS 抽象の是非は実装とテストコードで判定したため
