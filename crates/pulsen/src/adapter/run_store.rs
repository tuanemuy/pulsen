//! ファイルシステム上の run ディレクトリに対する `RunStore` の実装。
//!
//! ドメイン型は serde を持たない(ADR-020)ため、各ファイルの表現はここの DTO が担い、
//! 復元は必ずドメインの `parse` を通す。復号の分類は次のとおり。
//!
//! | 状態 | 分類 |
//! |---|---|
//! | ファイル・ディレクトリの不在 | `Ok(None)` |
//! | JSON として読めない・値制約の破れ | `RunFileError::Corrupt` |
//! | 読み取り機構の失敗 | `RunFileError::Io` |

use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use pulsen_domain::execution::{ExitCode, Io, PidFileContent, RunFileError, RunStore};
use pulsen_domain::task::{
    AttemptNumber, KillIdent, Pid, ProcessStartTime, RunDirPath, StartTimeRecord, StateRoot,
    TaskId, Timestamp,
};

use crate::util::atomic::write_atomic;
use crate::util::fsdir::ensure_dir;

/// pid ファイルの表現。
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PidDto {
    pid: u32,
    kill_ident: String,
}

/// starttime ファイルの表現。
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StartTimeDto {
    ident: String,
    wall: String,
}

/// exit ファイルの表現。
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExitDto {
    code: i32,
}

/// 状態ディレクトリ配下の run ディレクトリを読み書きするストア。
///
/// パスの導出はドメイン(`RunDirPath::derive`)に従い、レイアウトの知識をここに持たない。
#[derive(Debug, Clone)]
pub struct FsRunStore {
    state_root: StateRoot,
}

impl FsRunStore {
    /// 状態ディレクトリを受け取る(合成ルートがホームから解決した値を渡す)。
    pub fn new(state_root: StateRoot) -> Self {
        Self { state_root }
    }
}

impl RunStore for FsRunStore {
    fn prepare_attempt(&self, id: &TaskId, number: AttemptNumber) -> Result<RunDirPath, Io> {
        let run_dir = RunDirPath::derive(&self.state_root, id, number);
        ensure_dir(run_dir.as_path()).map_err(|error| Io::Failed {
            message: format!(
                "{}: attempt ディレクトリを用意できない: {error}",
                run_dir.as_path().display()
            ),
        })?;
        Ok(run_dir)
    }

    fn read_pid_file(&self, run_dir: &RunDirPath) -> Result<Option<PidFileContent>, RunFileError> {
        let path = run_dir.pid_file();
        let Some(dto) = read_json::<PidDto>(&path)? else {
            return Ok(None);
        };
        let kill_ident = KillIdent::parse(dto.kill_ident)
            .map_err(|error| corrupt(&path, format!("kill 同定子: {}", error.describe())))?;
        Ok(Some(PidFileContent::new(Pid::new(dto.pid), kill_ident)))
    }

    fn read_starttime(
        &self,
        run_dir: &RunDirPath,
    ) -> Result<Option<StartTimeRecord>, RunFileError> {
        let path = run_dir.starttime_file();
        let Some(dto) = read_json::<StartTimeDto>(&path)? else {
            return Ok(None);
        };
        let ident = ProcessStartTime::parse(dto.ident)
            .map_err(|error| corrupt(&path, format!("起動時刻: {}", error.describe())))?;
        let wall = Timestamp::parse_rfc3339(&dto.wall)
            .map_err(|error| corrupt(&path, format!("壁時計時刻: {}", error.describe())))?;
        Ok(Some(StartTimeRecord::new(ident, wall)))
    }

    fn read_exit(&self, run_dir: &RunDirPath) -> Result<Option<ExitCode>, RunFileError> {
        let Some(dto) = read_json::<ExitDto>(&run_dir.exit_file())? else {
            return Ok(None);
        };
        Ok(Some(ExitCode::new(dto.code)))
    }

    fn write_invalidation_marker(&self, run_dir: &RunDirPath) -> Result<(), Io> {
        // 存在のみが意味を持つ空ファイル。ディレクトリの作成は `write_atomic` が担う。
        write(&run_dir.marker_file(), b"")
    }

    fn marker_exists(&self, run_dir: &RunDirPath) -> Result<bool, Io> {
        let path = run_dir.marker_file();
        // `exists()` と違い `try_exists()` は I/O エラーを `false` に丸めない。マーカーの
        // 有無はラッパーが起動を抑止する判断そのもので、機構の失敗を「無い」に写せない。
        path.try_exists().map_err(|error| Io::Failed {
            message: format!("{}: マーカーの有無を確認できない: {error}", path.display()),
        })
    }

    fn write_starttime(&self, run_dir: &RunDirPath, record: &StartTimeRecord) -> Result<(), Io> {
        let dto = StartTimeDto {
            ident: record.ident().as_str().to_owned(),
            wall: record.wall().to_rfc3339(),
        };
        write(&run_dir.starttime_file(), &encode(&dto)?)
    }

    fn write_pid_file(&self, run_dir: &RunDirPath, content: &PidFileContent) -> Result<(), Io> {
        let dto = PidDto {
            pid: content.pid().get(),
            kill_ident: content.kill_ident().as_str().to_owned(),
        };
        write(&run_dir.pid_file(), &encode(&dto)?)
    }

    fn write_exit(&self, run_dir: &RunDirPath, code: &ExitCode) -> Result<(), Io> {
        let dto = ExitDto { code: code.get() };
        write(&run_dir.exit_file(), &encode(&dto)?)
    }
}

/// JSON を読み、不在・破損・機構の失敗に分類する。
fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<Option<T>, RunFileError> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        // ディレクトリごと不在の場合も `NotFound` として現れる(契約どおり不在に写す)。
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(RunFileError::Io {
                message: format!("{}: 読み取れない: {error}", path.display()),
            });
        }
    };
    let value = serde_json::from_slice(&bytes)
        .map_err(|error| corrupt(path, format!("内容を解釈できない: {error}")))?;
    Ok(Some(value))
}

/// アトミック置換で書く。書き込み先のディレクトリは `write_atomic` が用意する。
fn write(path: &Path, bytes: &[u8]) -> Result<(), Io> {
    write_atomic(path, bytes).map_err(|error| Io::Failed {
        message: format!("{}: 書き込めない: {error}", path.display()),
    })
}

/// DTO を JSON バイト列にする。
///
/// 人間が直接辿れること(requirements §9)を満たすため整形して書く。
fn encode<T: Serialize>(dto: &T) -> Result<Vec<u8>, Io> {
    serde_json::to_vec_pretty(dto).map_err(|error| Io::Failed {
        message: format!("内容を直列化できない: {error}"),
    })
}

fn corrupt(path: &Path, message: String) -> RunFileError {
    RunFileError::Corrupt {
        path: PathBuf::from(path),
        message,
    }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    fn store() -> (TempDir, FsRunStore) {
        let home = tempfile::tempdir().expect("一時ディレクトリを作れる");
        let state_root = StateRoot::parse(home.path().join("state")).expect("絶対パスになる");
        (home, FsRunStore::new(state_root))
    }

    fn task_id() -> TaskId {
        TaskId::parse("task-a".to_owned()).expect("受理される")
    }

    #[test]
    fn 準備したattemptのパスは導出結果と一致する() {
        let (_home, store) = store();

        let run_dir = store
            .prepare_attempt(&task_id(), AttemptNumber::FIRST)
            .expect("用意できる");

        assert!(run_dir.as_path().is_dir());
        assert!(run_dir.as_path().ends_with("runs/task-a/attempt-1"));
    }

    #[test]
    fn 書き込んだファイルは人間が読めるjsonになる() {
        let (_home, store) = store();
        let run_dir = store
            .prepare_attempt(&task_id(), AttemptNumber::FIRST)
            .expect("用意できる");

        store
            .write_exit(&run_dir, &ExitCode::new(0))
            .expect("書ける");

        let text = std::fs::read_to_string(run_dir.exit_file()).expect("読める");
        assert!(text.contains("\"code\": 0"), "{text}");
    }

    #[test]
    fn 無効化マーカーは空ファイルとして書かれる() {
        let (_home, store) = store();
        let run_dir = store
            .prepare_attempt(&task_id(), AttemptNumber::FIRST)
            .expect("用意できる");

        store
            .write_invalidation_marker(&run_dir)
            .expect("マーカーを書ける");

        assert_eq!(std::fs::read(run_dir.marker_file()).expect("読める"), b"");
    }

    #[test]
    fn 値制約を破る内容は破損として返る() {
        let (_home, store) = store();
        let run_dir = store
            .prepare_attempt(&task_id(), AttemptNumber::FIRST)
            .expect("用意できる");
        std::fs::write(run_dir.pid_file(), br#"{"pid":1,"kill_ident":""}"#).expect("置ける");

        let error = store.read_pid_file(&run_dir).expect_err("破損として返る");

        assert!(matches!(error, RunFileError::Corrupt { .. }), "{error:?}");
    }
}
