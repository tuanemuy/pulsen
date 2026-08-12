//! ファイルロックによる `ExclusiveLock` の実装。

use std::fs::{File, OpenOptions, TryLockError};
use std::io;
use std::path::{Path, PathBuf};

use pulsen_domain::execution::{ExclusiveLock, LockError, LockGuard};

use crate::util::fsdir::ensure_dir;

/// グローバルホームのロックファイルを使う排他ロック。
///
/// アドバイザリロックなので、保持プロセスが異常終了しても OS が解放する。同一プロセス内
/// での再取得の挙動は契約が規定していない(std も未規定)ため、ここでも約束しない。
#[derive(Debug, Clone)]
pub struct FileExclusiveLock {
    path: PathBuf,
}

impl FileExclusiveLock {
    /// ロックファイルのパスを受け取る(パスの導出はアプリケーション層。ADR-031)。
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// ロックファイルを開く。置き場が無ければ作る(ツール管理領域の自動作成)。
    fn open(&self) -> io::Result<File> {
        if let Some(parent) = self.path.parent() {
            ensure_dir(parent)?;
        }
        OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&self.path)
    }
}

impl ExclusiveLock for FileExclusiveLock {
    fn try_acquire(&self) -> Result<Option<Box<dyn LockGuard>>, LockError> {
        let file = self.open().map_err(|error| failed(&self.path, &error))?;
        match file.try_lock() {
            Ok(()) => Ok(Some(Box::new(FileLockGuard { _file: file }))),
            Err(TryLockError::WouldBlock) => Ok(None),
            Err(TryLockError::Error(error)) => Err(failed(&self.path, &error)),
        }
    }
}

/// 保持中のロックファイル。ドロップで閉じられ、ロックが解放される。
struct FileLockGuard {
    _file: File,
}

impl LockGuard for FileLockGuard {}

/// ロック機構自体の異常。
fn failed(path: &Path, error: &io::Error) -> LockError {
    LockError::Failed {
        message: format!("ロックファイル {} を扱えない: {error}", path.display()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 置き場のディレクトリが無ければ作られる() {
        let home = tempfile::tempdir().expect("一時ディレクトリを作れる");
        let lock = FileExclusiveLock::new(home.path().join("state").join("lock"));

        let guard = lock.try_acquire().expect("取得できる");

        assert!(guard.is_some());
        assert!(home.path().join("state").join("lock").is_file());
    }

    #[test]
    fn ロックの置き場を用意できなければ機構の異常になる() {
        let home = tempfile::tempdir().expect("一時ディレクトリを作れる");
        let occupied = home.path().join("lock");
        std::fs::create_dir(&occupied).expect("ディレクトリを作れる");
        let lock = FileExclusiveLock::new(occupied);

        let acquired = lock.try_acquire();

        assert!(matches!(acquired, Err(LockError::Failed { .. })));
    }
}
