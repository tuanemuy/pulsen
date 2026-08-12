//! ファイルのアトミックな置換と移動。
//!
//! 読み手はロックなしで常に一貫した内容を見る、という永続化ポートの契約
//! (spec/domains/task.md `TaskRepository`)を支える唯一の実装。

use std::io::{self, Write};
use std::path::Path;

use tempfile::NamedTempFile;

use super::fsdir::ensure_dir;

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
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let dir = parent_of(path)?;
    ensure_dir(dir)?;

    let mut temp = NamedTempFile::new_in(dir)?;
    temp.write_all(bytes)?;
    temp.as_file().sync_all()?;
    temp.persist(path).map_err(|failed| failed.error)?;

    sync_dir(dir);
    Ok(())
}

/// ファイルを別のパスへアトミックに移動する。
///
/// 移動先のディレクトリは必要に応じて作成する。移動は同一ファイルシステム内を前提とし、
/// 中間状態(両方に現れる・どちらにも完全体が無い)を作らない。
pub fn rename_atomic(from: &Path, to: &Path) -> io::Result<()> {
    let source = parent_of(from)?;
    let dir = parent_of(to)?;
    ensure_dir(dir)?;

    std::fs::rename(from, to)?;

    sync_dir(dir);
    // 別ディレクトリへの移動は2つのディレクトリエントリを変える。移動先だけを永続化すると
    // クラッシュ後に「両方に在る」中間状態が残りうる。
    if source != dir {
        sync_dir(source);
    }
    Ok(())
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
    use std::fs;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::thread;

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
                    // 置換の途中でファイルを開けない瞬間はプラットフォームによってありうる。
                    // 検証したいのは「読めたときの内容が完全な版のどちらかである」こと。
                    if let Ok(observed) = fs::read(&target) {
                        assert!(
                            observed == old || observed == new,
                            "書きかけの内容を観測した: {} バイト",
                            observed.len()
                        );
                    }
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
