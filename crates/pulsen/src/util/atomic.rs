//! ファイルのアトミックな置換・移動と、その窓を挟んで行う読み取り。
//!
//! 読み手はロックなしで常に一貫した内容を見る、という永続化ポートの契約
//! (spec/domains/task.md `TaskRepository`)を支える唯一の実装。契約は書き手と読み手の
//! 両側が揃って初めて成り立つため、置換の窓で拒まれる読み取りも同じ分類・同じ上限で
//! ここが吸収する。

use std::fs;
use std::io::{self, Write};
use std::num::NonZeroU32;
use std::path::Path;
use std::thread;
use std::time::Duration;

use tempfile::NamedTempFile;

use super::fsdir::ensure_dir;

/// 置換・移動・読み取りを試みる回数の上限。
///
/// 0 を表せない型にするのは、再試行の待ちを `上限 - 1` 本の列として組み立てるから。
/// 上限が 0 なら本数の減算が成り立たず、リリースでは列が長さの上端まで伸びて、
/// 一時的な拒否が続く限りループが返らない。
const MAX_ATTEMPTS: NonZeroU32 = NonZeroU32::new(10).expect("上限は1回以上");

/// 最初の再試行までの待ち時間。試行ごとに倍にする。
const FIRST_RETRY_WAIT: Duration = Duration::from_millis(1);

/// 試行と試行の間に待つ時間の列。上限まで拒まれ続けたときに待つ全量でもある。
///
/// 再試行ループはこの列だけを消費して眠るため、実際の待ちの出典はここ1つに閉じる。
/// 公開関数の doc と ADR-072 が根拠にしている「1回の呼び出しあたり最大 511ms」は
/// この列の和であり、ユニットテストが公称値との一致を固定している。待ちの決め方を
/// ループから切り出しているのは、上限を壁時計で測らずに検証できるようにするため。
fn retry_waits() -> impl Iterator<Item = Duration> {
    std::iter::successors(Some(FIRST_RETRY_WAIT), |wait| Some(*wait * 2))
        .take(MAX_ATTEMPTS.get() as usize - 1)
}

/// 内容全体を書き、対象パスをアトミックに置き換える。
///
/// 同一ディレクトリに一時ファイルを作って書き込み・`sync_all` してから `rename` するため、
/// 読み手は常に旧内容か新内容のどちらかだけを観測する。途中で失敗した場合は一時ファイルを
/// 残さず、対象パスの内容も変わらない。
///
/// 書き込み先のディレクトリは必要に応じて作成する(状態ディレクトリはツール管理領域であり、
/// 書き込み系の操作が自動作成する契約)。
///
/// 対象ファイルの権限は置換のたびに一時ファイルのもの(Unix では所有者のみ読み書き)に
/// 作り直される。タスクファイルはツールの管理領域であり所有者限定でよい、という意図で
/// あって、既存の権限を引き継ぐ設計ではない。
///
/// 一時的な拒否(`transiently_denied`)を吸収するため、成功する呼び出しも失敗する
/// 呼び出しも**1回の呼び出しあたり最大 511ms ブロックしうる**。短い待ちは OS のタイマー
/// 粒度に丸め上げられるため、実測はこれを上回る(Windows の既定粒度は約 15.6ms)。排他
/// ロックを保持したまま呼ぶ側では、この遅延がロックの保持時間にそのまま乗り、1つの操作が
/// 複数回呼ぶ経路ではその回数だけ積み上がる。
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let dir = parent_of(path)?;
    ensure_dir(dir)?;

    let mut temp = NamedTempFile::new_in(dir)?;
    temp.write_all(bytes)?;
    temp.as_file().sync_all()?;
    persist_with_retry(temp, path, transiently_denied)?;

    sync_dir(dir);
    Ok(())
}

/// ファイルを別のパスへアトミックに移動する。
///
/// 移動先のディレクトリは必要に応じて作成する。移動は同一ファイルシステム内を前提とし、
/// 中間状態(両方に現れる・どちらにも完全体が無い)を作らない。
///
/// `write_atomic` と同じく、一時的な拒否を吸収するため**1回の呼び出しあたり最大 511ms
/// ブロックしうる**(短い待ちは OS のタイマー粒度で伸びる)。
pub fn rename_atomic(from: &Path, to: &Path) -> io::Result<()> {
    let source = parent_of(from)?;
    let dir = parent_of(to)?;
    ensure_dir(dir)?;

    rename_with_retry(from, to, transiently_denied)?;

    sync_dir(dir);
    // 別ディレクトリへの移動は2つのディレクトリエントリを変える。移動先だけを永続化すると
    // クラッシュ後に「両方に在る」中間状態が残りうる。
    if source != dir {
        sync_dir(source);
    }
    Ok(())
}

/// 内容全体を読む。置換・移動が一時的に読み取りを拒ませる間は上限内で再試行する。
///
/// 置換の窓は書き手だけでなく**読み手にも当たる**。Windows の `MoveFileEx` は置き換え
/// られる側を delete-pending に落とすため、その間に開こうとした読み手は書き手と同じ
/// `ERROR_ACCESS_DENIED` を受ける。「読み手はロックなしで常に一貫した内容を見る」は
/// 書き手側の吸収だけでは満たせないので、分類と上限を共有した読み取りをここに置く。
///
/// 吸収するのは一時的な拒否だけで、エラーの意味は変えない。上限に達したら OS が返した
/// エラーをそのまま返す(`NotFound` は一時的な拒否ではないので初回で返る)。書き込み側と
/// 同じく**1回の呼び出しあたり最大 511ms ブロックしうる**(短い待ちは OS のタイマー粒度で
/// 伸びる)。エントリを走査して1件ずつ読む呼び出し側では、この上限が件数だけ積み上がる。
pub fn read_atomic(path: &Path) -> io::Result<Vec<u8>> {
    read_with_retry(path, transiently_denied)
}

/// 一時ファイルを対象へ置き換える。`is_transient` が真とするエラーに限って再試行する。
fn persist_with_retry(
    temp: NamedTempFile,
    path: &Path,
    is_transient: impl Fn(&io::Error) -> bool,
) -> io::Result<()> {
    retry_while_transient(
        temp,
        |temp| match temp.persist(path) {
            Ok(_) => Ok(()),
            Err(failed) => Err((failed.error, failed.file)),
        },
        is_transient,
    )
}

/// ファイルを移動先へ移す。`is_transient` が真とするエラーに限って再試行する。
fn rename_with_retry(
    from: &Path,
    to: &Path,
    is_transient: impl Fn(&io::Error) -> bool,
) -> io::Result<()> {
    retry_while_transient(
        (),
        |()| fs::rename(from, to).map_err(|error| (error, ())),
        is_transient,
    )
}

/// 内容全体を読む。`is_transient` が真とするエラーに限って再試行する。
fn read_with_retry(path: &Path, is_transient: impl Fn(&io::Error) -> bool) -> io::Result<Vec<u8>> {
    retry_while_transient(
        (),
        |()| fs::read(path).map_err(|error| (error, ())),
        is_transient,
    )
}

/// `is_transient` が真とするエラーに限り、`retry_waits` の列が尽きるまで `attempt` を繰り返す。
///
/// 置換も移動も読み取りも Windows では同じ拒否(他のハンドルが対象を開いている・対象が
/// delete-pending にある)で失敗するため、分類と上限を1つに集約する。`attempt` が失敗時に
/// 状態 `S` を返すのは、`NamedTempFile::persist` が一時ファイルを消費し、失敗したときだけ
/// 返してくるため。成果 `T` があるのは、読み取りのように値を返す試行を同じループに
/// 載せるため。
///
/// 分類を引数に取るのは、再試行の打ち切り方(列を使い切ったら元のエラーを返す)を、
/// エラーがどう分類されるかと独立に検証できるようにするため。
fn retry_while_transient<S, T>(
    mut state: S,
    mut attempt: impl FnMut(S) -> Result<T, (io::Error, S)>,
    is_transient: impl Fn(&io::Error) -> bool,
) -> io::Result<T> {
    let mut waits = retry_waits();
    loop {
        let error = match attempt(state) {
            Ok(value) => return Ok(value),
            Err((error, returned)) => {
                state = returned;
                error
            }
        };
        // 列を先に見るのは、最後の試行のあとで分類を問うても結果が変わらないから。
        let Some(wait) = waits.next() else {
            return Err(error);
        };
        if !is_transient(&error) {
            return Err(error);
        }
        thread::sleep(wait);
    }
}

/// 置換・移動・読み取りが一時的に拒まれたことを表すエラーか。
///
/// Windows では、対象を他のハンドル(ロックを取らない読み手・ウイルス対策のスキャン)が
/// 開いている間、置換と移動が `ERROR_ACCESS_DENIED` / `ERROR_SHARING_VIOLATION` で拒まれる。
/// 置き換えられる側は delete-pending に落ちるため、同じ窓では読み手の `CreateFile` も
/// 同じコードで拒まれる。「読み手はロックなしで常に一貫した内容を見る」は永続化ポートの
/// 契約なので、どちらの側もこの窓を待って吸収する。時間で解けない失敗(権限そのものの
/// 不足・容量の不足)を再試行で遅らせないため、分類はこの2つに限る。
#[cfg(windows)]
fn transiently_denied(error: &io::Error) -> bool {
    const ERROR_ACCESS_DENIED: i32 = 5;
    const ERROR_SHARING_VIOLATION: i32 = 32;

    matches!(
        error.raw_os_error(),
        Some(ERROR_ACCESS_DENIED | ERROR_SHARING_VIOLATION)
    )
}

/// unix の `rename` と `open` は開いているハンドルに影響されないため、待って解ける拒否が
/// 無い。分類が空である以上、公開関数が再試行に入る様子は unix では原理的に観測できない
/// — テストは分類そのもの・再試行の経路・「一時的でない拒否に予算を使わない」の3つに
/// 分けて押さえる。
#[cfg(not(windows))]
fn transiently_denied(_error: &io::Error) -> bool {
    false
}

fn parent_of(path: &Path) -> io::Result<&Path> {
    path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("親ディレクトリを持たないパス: {}", path.display()),
        )
    })
}

/// ディレクトリエントリの更新を永続化する。
///
/// 失敗しても書き込み自体は成功しているため、エラーは伝えない。ディレクトリを開けない
/// プラットフォームでは何もしない。
#[cfg(unix)]
fn sync_dir(dir: &Path) {
    if let Ok(handle) = std::fs::File::open(dir) {
        let _ = handle.sync_all();
    }
}

#[cfg(not(unix))]
fn sync_dir(_dir: &Path) {}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::sync::atomic::{AtomicBool, Ordering};

    use super::*;

    /// 阻害要因を別スレッドから取り除くまでの待ち。
    ///
    /// 本番の経路(`persist_with_retry` / `rename_with_retry` / `read_with_retry`)をそのまま
    /// 呼ぶために、状況を変える手段を試行の外側に置く。待ちは再試行の予算の内側に収める。
    const OBSTACLE_LIFETIME: Duration = Duration::from_millis(20);

    #[test]
    fn 新しい内容で既存のファイルが置き換わる() {
        let base = tempfile::tempdir().expect("一時ディレクトリを作れる");
        let target = base.path().join("task.json");
        write_atomic(&target, b"old").expect("1回目");

        write_atomic(&target, b"new").expect("2回目");

        assert_eq!(fs::read(&target).expect("読める"), b"new");
    }

    #[test]
    fn 書き込み先のディレクトリが無ければ作られる() {
        let base = tempfile::tempdir().expect("一時ディレクトリを作れる");
        let target = base.path().join("state").join("tasks").join("task.json");

        write_atomic(&target, b"content").expect("書ける");

        assert_eq!(fs::read(&target).expect("読める"), b"content");
    }

    #[test]
    fn 書き込みの後に一時ファイルは残らない() {
        let base = tempfile::tempdir().expect("一時ディレクトリを作れる");
        let target = base.path().join("task.json");

        write_atomic(&target, b"content").expect("書ける");

        assert_eq!(entry_names(base.path()), vec!["task.json".to_owned()]);
    }

    #[test]
    fn 置換に失敗しても一時ファイルは残らない() {
        let base = tempfile::tempdir().expect("一時ディレクトリを作れる");
        let occupied = base.path().join("task.json");
        fs::create_dir(&occupied).expect("ディレクトリを置ける");

        let result = write_atomic(&occupied, b"content");

        assert!(result.is_err());
        assert_eq!(entry_names(base.path()), vec!["task.json".to_owned()]);
    }

    #[test]
    fn 一時的な拒否が続けば上限の回数だけ試みて元のエラーを返す() {
        let attempted = Cell::new(0);

        let error = retry_while_transient(
            (),
            |()| {
                attempted.set(attempted.get() + 1);
                Err::<(), _>((io::Error::from_raw_os_error(5), ()))
            },
            |_| true,
        )
        .expect_err("打ち切られる");

        assert_eq!(attempted.get(), MAX_ATTEMPTS.get());
        // 打ち切りを表す独自のエラーに差し替えず、OS が返した拒否をそのまま渡す。
        assert_eq!(error.raw_os_error(), Some(5));
    }

    #[test]
    fn 一時的でない拒否は再試行せずに返る() {
        let attempted = Cell::new(0);

        let error = retry_while_transient(
            (),
            |()| {
                attempted.set(attempted.get() + 1);
                Err::<(), _>((io::Error::from_raw_os_error(13), ()))
            },
            |_| false,
        )
        .expect_err("再試行しない");

        assert_eq!(attempted.get(), 1);
        assert_eq!(error.raw_os_error(), Some(13));
    }

    #[test]
    fn 一時的な拒否と分類されるのは共有違反とアクセス拒否だけ() {
        const ACCESS_DENIED: i32 = 5;
        const SHARING_VIOLATION: i32 = 32;
        const FILE_NOT_FOUND: i32 = 2;
        const DISK_FULL: i32 = 112;

        // 待てば解ける窓を作るのは Windows の置換・移動・読み取りだけなので、他の OS では
        // 同じコードでも一時的とは見なさない。
        for code in [ACCESS_DENIED, SHARING_VIOLATION] {
            assert_eq!(
                transiently_denied(&io::Error::from_raw_os_error(code)),
                cfg!(windows),
                "{code}"
            );
        }
        for code in [FILE_NOT_FOUND, DISK_FULL] {
            assert!(
                !transiently_denied(&io::Error::from_raw_os_error(code)),
                "{code}"
            );
        }
        assert!(!transiently_denied(&io::Error::other(
            "OS のエラーではない"
        )));
    }

    #[test]
    fn 再試行に費やす待ちの合計は公称する上限と一致する() {
        // 公開関数の doc と ADR-072 は「1回の呼び出しあたり最大 511ms」を根拠に、
        // 遅延が排他ロックの保持時間に乗るトレードオフを受け入れている。待ちの列は
        // ループが実際に消費するものなので、伸び幅も回数もここに現れる。
        assert_eq!(retry_budget(), Duration::from_millis(511));
    }

    #[test]
    fn 時間で解けない拒否は再試行の予算の対象にならない() {
        let base = tempfile::tempdir().expect("一時ディレクトリを作れる");
        let missing = base.path().join("task.json");
        let to = base.path().join("archived.json");

        let read_error = read_atomic(&missing).expect_err("読めない");
        let rename_error = rename_atomic(&missing, &to).expect_err("移動できない");

        // 見ているのは、公開関数が実際に返した拒否を分類に掛けると予算がゼロになること。
        // 経過時間で同じことを言うと、遅いランナーでは実装と無関係に判定がぶれる。
        //
        // 公開関数が `transiently_denied` を渡している配線そのものはここでは押さえられない。
        // 緩い分類に差し替えても、この拒否が `NotFound` であることは変わらないため。
        // 配線が壊れたときの被害は有界な遅延に限られる（ADR-072）。
        for error in [read_error, rename_error] {
            assert_eq!(error.kind(), io::ErrorKind::NotFound);
            assert_eq!(budget_spent_on(&error), Duration::ZERO, "{error:?}");
        }
    }

    #[test]
    fn 置換が一時的に拒まれても上限内に解ければ置き換わる() {
        let base = tempfile::tempdir().expect("一時ディレクトリを作れる");
        let target = base.path().join("task.json");
        fs::create_dir(&target).expect("置換を拒ませるものを置ける");
        let temp = NamedTempFile::new_in(base.path()).expect("一時ファイルを作れる");

        thread::scope(|scope| {
            scope.spawn(|| {
                thread::sleep(OBSTACLE_LIFETIME);
                fs::remove_dir(&target).expect("阻害要因を取り除ける");
            });

            persist_with_retry(temp, &target, |_| true).expect("最終的に置き換わる");
        });

        assert!(target.is_file());
        assert_eq!(entry_names(base.path()), vec!["task.json".to_owned()]);
    }

    #[test]
    fn 置換の一時的な拒否が続けば打ち切られ一時ファイルも残らない() {
        let base = tempfile::tempdir().expect("一時ディレクトリを作れる");
        let occupied = base.path().join("task.json");
        fs::create_dir(&occupied).expect("置換を拒ませるものを置ける");
        let temp = NamedTempFile::new_in(base.path()).expect("一時ファイルを作れる");

        let error = persist_with_retry(temp, &occupied, |_| true).expect_err("置き換わらない");

        assert!(error.raw_os_error().is_some(), "{error}");
        assert_eq!(entry_names(base.path()), vec!["task.json".to_owned()]);
    }

    #[test]
    fn 読み手は旧内容か新内容のどちらかだけを観測する() {
        let base = tempfile::tempdir().expect("一時ディレクトリを作れる");
        let target = base.path().join("task.json");
        let old = vec![b'a'; 64 * 1024];
        let new = vec![b'b'; 64 * 1024];
        write_atomic(&target, &old).expect("初期内容");

        let writing = AtomicBool::new(true);
        thread::scope(|scope| {
            let _stop = StopOnDrop(&writing);
            scope.spawn(|| {
                while writing.load(Ordering::Relaxed) {
                    // 置換の窓では読み手も拒まれうるため、書き手と同じ吸収を通して読む。
                    // 読めない瞬間を許容すると、ポート水準の契約(読み手はロックなしで
                    // 常に一貫した内容を見る)より弱いことを主張するテストになる。
                    let observed = read_atomic(&target).expect("読み手は常に読める");
                    assert!(
                        observed == old || observed == new,
                        "書きかけの内容を観測した: {} バイト",
                        observed.len()
                    );
                    // 読み手には上限が無い一方で書き手は予算の内に窓を取り切る必要がある。
                    // 譲るのは病的な CPU 飢餓だけを避けるためで、インターリーブの窓は残る。
                    thread::yield_now();
                }
            });

            for _ in 0..50 {
                write_atomic(&target, &new).expect("新内容");
                write_atomic(&target, &old).expect("旧内容");
            }
        });
    }

    /// 読み手の停止をスコープの巻き戻しに載せる。
    ///
    /// `thread::scope` は巻き戻しの前に子スレッドを合流させるため、書き手が置換の失敗で
    /// パニックしたとき、停止をクロージャ末尾の文で行うと読み手が回り続けて返らない。
    struct StopOnDrop<'a>(&'a AtomicBool);

    impl Drop for StopOnDrop<'_> {
        fn drop(&mut self) {
            self.0.store(false, Ordering::Relaxed);
        }
    }

    #[test]
    fn 移動先のディレクトリが無ければ作られて移動する() {
        let base = tempfile::tempdir().expect("一時ディレクトリを作れる");
        let from = base.path().join("tasks").join("task.json");
        let to = base.path().join("archive").join("task.json");
        write_atomic(&from, b"content").expect("書ける");

        rename_atomic(&from, &to).expect("移動できる");

        assert!(!from.exists());
        assert_eq!(fs::read(&to).expect("読める"), b"content");
    }

    #[test]
    fn 移動元が無ければ失敗し移動先も作られない() {
        let base = tempfile::tempdir().expect("一時ディレクトリを作れる");
        let from = base.path().join("tasks").join("task.json");
        let to = base.path().join("archive").join("task.json");

        let result = rename_atomic(&from, &to);

        assert!(result.is_err());
        assert!(!to.exists());
    }

    #[test]
    fn 移動が一時的に拒まれても上限内に解ければ移動する() {
        let base = tempfile::tempdir().expect("一時ディレクトリを作れる");
        let from = base.path().join("task.json");
        let to = base.path().join("archived.json");

        thread::scope(|scope| {
            scope.spawn(|| {
                thread::sleep(OBSTACLE_LIFETIME);
                fs::write(&from, b"content").expect("阻害要因を取り除ける");
            });

            rename_with_retry(&from, &to, |_| true).expect("最終的に移動する");
        });

        assert!(!from.exists());
        assert_eq!(fs::read(&to).expect("読める"), b"content");
    }

    #[test]
    fn 読み取りが一時的に拒まれても上限内に解ければ読める() {
        let base = tempfile::tempdir().expect("一時ディレクトリを作れる");
        let target = base.path().join("task.json");

        let bytes = thread::scope(|scope| {
            scope.spawn(|| {
                thread::sleep(OBSTACLE_LIFETIME);
                write_atomic(&target, b"content").expect("阻害要因を取り除ける");
            });

            read_with_retry(&target, |_| true).expect("最終的に読める")
        });

        assert_eq!(bytes, b"content");
    }

    #[test]
    fn 移動の一時的な拒否が続けば打ち切られ移動先も作られない() {
        let base = tempfile::tempdir().expect("一時ディレクトリを作れる");
        let from = base.path().join("task.json");
        let to = base.path().join("archived.json");

        let error = rename_with_retry(&from, &to, |_| true).expect_err("移動できない");

        assert!(error.raw_os_error().is_some(), "{error}");
        assert!(!to.exists());
    }

    #[test]
    fn 親ディレクトリを持たないパスは受理されない() {
        let root = Path::new("/");

        assert!(write_atomic(root, b"content").is_err());
    }

    /// 上限まで拒まれ続けたときに待つ時間の合計。
    fn retry_budget() -> Duration {
        retry_waits().sum()
    }

    /// このエラーを受け続けた再試行ループが費やす待ちの合計。
    ///
    /// ループが待つのは分類が真のときだけ(`一時的でない拒否は再試行せずに返る`)なので、
    /// 分類を通せば予算を使うかどうかが時間を測らずに決まる。
    fn budget_spent_on(error: &io::Error) -> Duration {
        if transiently_denied(error) {
            retry_budget()
        } else {
            Duration::ZERO
        }
    }

    fn entry_names(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir)
            .expect("走査できる")
            .map(|entry| entry.expect("エントリを読める").file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }
}
