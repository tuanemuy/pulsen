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
/// 0 を表せない型にするのは、上限が「1回は試みる」を含むため。回数を数え上げる側が
/// 引き算で 0 を扱わずに済む。
const MAX_ATTEMPTS: NonZeroU32 = NonZeroU32::new(10).expect("上限は1回以上");

/// 最初の再試行までの待ち時間。試行ごとに倍にする。
const FIRST_RETRY_WAIT: Duration = Duration::from_millis(1);

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
/// 呼び出しも**最大 511ms ブロックしうる**。排他ロックを保持したまま呼ぶ側では、
/// この遅延がロックの保持時間にそのまま乗る。
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
/// `write_atomic` と同じく、一時的な拒否を吸収するため**最大 511ms ブロックしうる**。
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
/// 同じく**最大 511ms ブロックしうる**。
pub fn read_atomic(path: &Path) -> io::Result<Vec<u8>> {
    retry_while_transient(
        (),
        |()| fs::read(path).map_err(|error| (error, ())),
        transiently_denied,
    )
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

/// `is_transient` が真とするエラーに限り、上限内で `attempt` を繰り返す。
///
/// 置換も移動も読み取りも Windows では同じ拒否(他のハンドルが対象を開いている・対象が
/// delete-pending にある)で失敗するため、分類と上限を1つに集約する。`attempt` が失敗時に
/// 状態 `S` を返すのは、`NamedTempFile::persist` が一時ファイルを消費し、失敗したときだけ
/// 返してくるため。成果 `T` があるのは、読み取りのように値を返す試行を同じループに
/// 載せるため。
///
/// 分類を引数に取るのは、再試行の打ち切り方(上限に達したら元のエラーを返す)を、
/// エラーがどう分類されるかと独立に検証できるようにするため。
fn retry_while_transient<S, T>(
    mut state: S,
    mut attempt: impl FnMut(S) -> Result<T, (io::Error, S)>,
    is_transient: impl Fn(&io::Error) -> bool,
) -> io::Result<T> {
    let mut attempted: u32 = 0;
    let mut wait = FIRST_RETRY_WAIT;
    loop {
        let error = match attempt(state) {
            Ok(value) => return Ok(value),
            Err((error, returned)) => {
                state = returned;
                error
            }
        };
        attempted += 1;
        if attempted == MAX_ATTEMPTS.get() || !is_transient(&error) {
            return Err(error);
        }
        thread::sleep(wait);
        wait *= 2;
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
/// 無い。
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
    fn 置換が一時的に拒まれても上限内なら置き換わる() {
        let base = tempfile::tempdir().expect("一時ディレクトリを作れる");
        let target = base.path().join("task.json");
        fs::create_dir(&target).expect("置換を拒ませるものを置ける");
        let temp = NamedTempFile::new_in(base.path()).expect("一時ファイルを作れる");

        retry_while_transient(
            temp,
            |temp| match temp.persist(&target) {
                Ok(_) => Ok(()),
                Err(failed) => {
                    // 拒否が続かない状況を作る。持ち越した一時ファイルで次の試行が置き換える。
                    let _ = fs::remove_dir(&target);
                    Err((failed.error, failed.file))
                }
            },
            |_| true,
        )
        .expect("最終的に置き換わる");

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
    fn 移動が一時的に拒まれても上限内なら移動する() {
        let base = tempfile::tempdir().expect("一時ディレクトリを作れる");
        let from = base.path().join("task.json");
        let to = base.path().join("archived.json");

        retry_while_transient(
            (),
            |()| {
                fs::rename(&from, &to).map_err(|error| {
                    // 拒否が続かない状況を作る。次の試行では移動できる。
                    let _ = fs::write(&from, b"content");
                    (error, ())
                })
            },
            |_| true,
        )
        .expect("最終的に移動する");

        assert!(!from.exists());
        assert_eq!(fs::read(&to).expect("読める"), b"content");
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
