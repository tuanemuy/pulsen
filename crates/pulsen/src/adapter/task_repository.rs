//! タスクファイルを読み書きする `TaskRepository` の実装。
//!
//! パスの導出と命名形式はドメインの `TaskFilePath` が単一の定義箇所であり、この
//! アダプターはレイアウトを組み立て直さない。書き込みは常に全体のアトミック置換で、
//! 移動は単一ファイルの rename に帰着する。
//!
//! 内容の読み取りは `fs::read` ではなく `util::atomic::read_atomic` を通す。置換の窓は
//! 書き手と読み手の両方に当たるため、片方だけを吸収すると「読み手はロックなしで常に
//! 一貫した内容を見る」が読み取り側で崩れる。
//!
//! 一時的な拒否の吸収でブロックしうる時間(1回あたり最大 511ms)は、複数回呼ぶ経路では
//! その回数だけ積み上がる。`save_degraded` は読んでから書くので最大2倍、`list` はエントリ
//! 数 N だけ読むので最大 N 倍になる。

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use pulsen_domain::task::{
    ArchiveError, CreateError, DegradedTask, ReadError, SaveError, StateRoot, Task, TaskEntry,
    TaskFilePath, TaskId, TaskLookup, TaskRepository,
};

use super::task_file;
use crate::util::atomic::{read_atomic, rename_atomic, write_atomic};

/// タスクファイルの置き場。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Area {
    Active,
    Archived,
}

/// `state/` 配下のタスクファイルを読み書きするリポジトリ。
pub struct FsTaskRepository {
    state_root: StateRoot,
}

impl FsTaskRepository {
    /// 状態ディレクトリ(`<home>/state`)を受け取る。
    pub fn new(state_root: StateRoot) -> Self {
        Self { state_root }
    }

    fn path(&self, area: Area, id: &TaskId) -> PathBuf {
        match area {
            Area::Active => TaskFilePath::active(&self.state_root, id),
            Area::Archived => TaskFilePath::archived(&self.state_root, id),
        }
    }

    fn dir(&self, area: Area) -> PathBuf {
        match area {
            Area::Active => TaskFilePath::active_dir(&self.state_root),
            Area::Archived => TaskFilePath::archived_dir(&self.state_root),
        }
    }

    /// エントリの有無だけを見る。デコードの可否も内容へ到達できるかも問わない —
    /// 読めないファイルも「その ID は使われている」ことの証拠であり、上書きすると
    /// 修復材料が消える。リンクを辿らないのはそのため。
    ///
    /// 書き込み系(`save` / `archive`)の存在確認がリンクを辿る `try_exists` のままなのは、
    /// 到達できないエントリを `NotFound` として拒み、リンクの先を書き換えないため。
    fn exists(&self, area: Area, id: &TaskId) -> io::Result<bool> {
        match self.path(area, id).symlink_metadata() {
            Ok(_) => Ok(true),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error),
        }
    }

    fn lookup(&self, area: Area, id: &TaskId) -> Result<Option<TaskLookup>, ReadError> {
        let path = self.path(area, id);
        let bytes = match read_atomic(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(unreachable_entry(&path, &error)?
                    .map(|message| TaskLookup::Corrupt { path, message }));
            }
            Err(error) => {
                return Err(ReadError::Io {
                    message: message(&path, &error),
                });
            }
        };

        Ok(Some(match task_file::decode(&bytes) {
            Ok(record) => match area {
                Area::Active => TaskLookup::Active(record),
                Area::Archived => TaskLookup::Archived(record),
            },
            Err(reason) => TaskLookup::Corrupt {
                path,
                message: reason,
            },
        }))
    }

    fn list(&self, area: Area) -> Result<Vec<TaskEntry>, ReadError> {
        let dir = self.dir(area);
        let read_dir = match fs::read_dir(&dir) {
            Ok(read_dir) => read_dir,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => {
                return Err(ReadError::Io {
                    message: message(&dir, &error),
                });
            }
        };

        let mut paths = Vec::new();
        for entry in read_dir {
            let entry = entry.map_err(|error| ReadError::Io {
                message: message(&dir, &error),
            })?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            if TaskFilePath::parse_file_name(name).is_none() {
                continue;
            }
            paths.push(entry.path());
        }
        paths.sort();

        let mut entries = Vec::with_capacity(paths.len());
        for path in paths {
            let bytes = match read_atomic(&path) {
                Ok(bytes) => bytes,
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    match unreachable_entry(&path, &error)? {
                        None => continue,
                        Some(message) => {
                            entries.push(TaskEntry::Corrupt { path, message });
                            continue;
                        }
                    }
                }
                Err(error) => {
                    return Err(ReadError::Io {
                        message: message(&path, &error),
                    });
                }
            };
            entries.push(match task_file::decode(&bytes) {
                Ok(record) => TaskEntry::Record(record),
                Err(reason) => TaskEntry::Corrupt {
                    path,
                    message: reason,
                },
            });
        }
        Ok(entries)
    }
}

impl TaskRepository for FsTaskRepository {
    fn create(&self, task: &Task) -> Result<(), CreateError> {
        let taken = |path: PathBuf| {
            move |error: io::Error| CreateError::Io {
                message: message(&path, &error),
            }
        };
        let active = self.path(Area::Active, task.id());
        let archived = self.path(Area::Archived, task.id());
        if self
            .exists(Area::Active, task.id())
            .map_err(taken(active))?
            || self
                .exists(Area::Archived, task.id())
                .map_err(taken(archived))?
        {
            return Err(CreateError::Conflict);
        }

        let bytes = task_file::encode_task(task).map_err(|message| CreateError::Io { message })?;
        let path = self.path(Area::Active, task.id());
        write_atomic(&path, &bytes).map_err(|error| CreateError::Io {
            message: message(&path, &error),
        })
    }

    fn save(&self, task: &Task) -> Result<(), SaveError> {
        let path = self.path(Area::Active, task.id());
        if !path.try_exists().map_err(|error| SaveError::Io {
            message: message(&path, &error),
        })? {
            return Err(SaveError::NotFound);
        }

        let bytes = task_file::encode_task(task).map_err(|message| SaveError::Io { message })?;
        write_atomic(&path, &bytes).map_err(|error| SaveError::Io {
            message: message(&path, &error),
        })
    }

    fn save_degraded(&self, task: &DegradedTask) -> Result<(), SaveError> {
        let path = self.path(Area::Active, task.id());
        let existing = match read_atomic(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Err(SaveError::NotFound);
            }
            Err(error) => {
                return Err(SaveError::Io {
                    message: message(&path, &error),
                });
            }
        };

        let snapshot = task_file::carried_snapshot(&existing).map_err(|reason| SaveError::Io {
            message: format!(
                "{}: 既存の内容からスナップショットを引き継げません({reason})",
                path.display()
            ),
        })?;
        let bytes = task_file::encode_degraded(task, snapshot)
            .map_err(|message| SaveError::Io { message })?;
        write_atomic(&path, &bytes).map_err(|error| SaveError::Io {
            message: message(&path, &error),
        })
    }

    fn find(&self, id: &TaskId) -> Result<TaskLookup, ReadError> {
        if let Some(lookup) = self.lookup(Area::Active, id)? {
            return Ok(lookup);
        }
        if let Some(lookup) = self.lookup(Area::Archived, id)? {
            return Ok(lookup);
        }
        Ok(TaskLookup::NotFound)
    }

    fn list_active(&self) -> Result<Vec<TaskEntry>, ReadError> {
        self.list(Area::Active)
    }

    fn list_archived(&self) -> Result<Vec<TaskEntry>, ReadError> {
        self.list(Area::Archived)
    }

    fn archive(&self, id: &TaskId) -> Result<(), ArchiveError> {
        let from = self.path(Area::Active, id);
        if !from.try_exists().map_err(|error| ArchiveError::Io {
            message: message(&from, &error),
        })? {
            return Err(ArchiveError::NotFound);
        }

        let to = self.path(Area::Archived, id);
        // 移動の失敗は移動元にも移動先にも起因しうる。片方だけを載せると、無関係な
        // パスを指した案内になる。
        rename_atomic(&from, &to).map_err(|error| ArchiveError::Io {
            message: format!("{} -> {}: {error}", from.display(), to.display()),
        })
    }
}

fn message(path: &Path, error: &io::Error) -> String {
    format!("{}: {error}", path.display())
}

/// 内容の読み取りが `NotFound` になったエントリを2つの状況に分ける。
///
/// `Ok(None)` は「この領域にもう無い」— 失敗ではない。読み取りはロックなしで常に一貫した
/// 内容を返す契約なので、`archive` の中間状態を検索や走査の失敗として観測させない。
/// `Ok(Some(message))` は「エントリは残っているが内容へ到達できない(宙ぶらりんのリンク等)」で、
/// これはファイル全体が読めない状況にあたる。黙って落とすと修復の入口が消えるため、
/// `find` と走査の双方が同じ区分(`Corrupt`)で報告する。
fn unreachable_entry(path: &Path, error: &io::Error) -> Result<Option<String>, ReadError> {
    match path.symlink_metadata() {
        Err(meta_error) if meta_error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(meta_error) => Err(ReadError::Io {
            message: message(path, &meta_error),
        }),
        Ok(_) => Ok(Some(format!("読み取れません: {error}"))),
    }
}

/// 検索・走査・作成が「消えたエントリ」と「読めないエントリ」を取り違えないことの確認。
///
/// unix 限定なのは、宙ぶらりんのリンク(列挙できるが内容へ到達できないエントリ)を
/// 決定的に作れるのが symlink だけであるため。判定そのものは OS に依存しない。
#[cfg(all(test, unix))]
mod tests {
    use std::collections::BTreeMap;
    use std::os::unix::fs::symlink;

    use pulsen_domain::definition::{
        StatusDefinition, StatusName, WorkflowDefinition, WorkflowName, WorkflowSnapshot,
    };
    use pulsen_domain::task::{
        BranchName, ExecutionState, RepoPath, RetryCounters, Target, TaskFields, Timestamp,
    };

    use super::*;

    fn task_id() -> TaskId {
        TaskId::parse("20240101t000000-abcdef01".to_owned()).expect("受理される")
    }

    /// 最小構成のタスク。作成の衝突判定はIDだけで決まるため内容は問わない。
    fn task(repo: &Path) -> Task {
        let done = StatusName::parse("done".to_owned()).expect("受理される");
        let definition = WorkflowDefinition::new(
            None,
            None,
            done.clone(),
            BTreeMap::from([(done.clone(), StatusDefinition::Cleanup)]),
        )
        .expect("構造不変条件を満たす");
        Task::rehydrate(TaskFields {
            id: task_id(),
            workflow_name: WorkflowName::parse("implement".to_owned()).expect("受理される"),
            target: Target::new(
                RepoPath::parse(repo.to_path_buf()).expect("受理される"),
                BranchName::parse("main".to_owned()).expect("受理される"),
            ),
            snapshot: WorkflowSnapshot::rehydrate(definition),
            task_status: done,
            execution: ExecutionState::Pending,
            workspace: None,
            current_attempt: None,
            counters: RetryCounters::initial(),
            last_failure: None,
            updated_at: Timestamp::parse_rfc3339("2026-08-11T09:15:30Z").expect("受理される"),
        })
        .expect("不変条件1を満たす")
    }

    /// 現役側に「内容へ到達できないタスクファイル」を1件だけ置いたリポジトリと、そのパス。
    fn repository_with_unreachable_entry() -> (tempfile::TempDir, FsTaskRepository, PathBuf) {
        let temp = tempfile::tempdir().expect("一時ディレクトリを作れる");
        let state_root = StateRoot::parse(temp.path().to_path_buf()).expect("絶対パス");
        let dir = TaskFilePath::active_dir(&state_root);
        fs::create_dir_all(&dir).expect("作成できる");
        let path = dir.join(TaskFilePath::file_name(&task_id()));
        symlink(dir.join("missing.json"), &path).expect("リンクを張れる");
        (temp, FsTaskRepository::new(state_root), path)
    }

    #[test]
    fn 内容へ到達できないエントリは走査から落とさず破損として報告される() {
        let (_temp, repository, path) = repository_with_unreachable_entry();

        let entries = repository.list_active().expect("走査は失敗しない");

        match entries.as_slice() {
            [TaskEntry::Corrupt { path: reported, .. }] => assert_eq!(reported, &path),
            other => panic!("破損として1件報告される: {other:?}"),
        }
    }

    #[test]
    fn 内容へ到達できないエントリは検索でも走査と同じ区分で返る() {
        let (_temp, repository, path) = repository_with_unreachable_entry();

        match repository.find(&task_id()).expect("検索は失敗しない") {
            TaskLookup::Corrupt { path: reported, .. } => assert_eq!(reported, path),
            other => panic!("破損として返る: {other:?}"),
        }
    }

    #[test]
    fn 内容へ到達できないエントリのidは使用済みとして衝突になる() {
        let (temp, repository, _path) = repository_with_unreachable_entry();

        assert_eq!(
            repository.create(&task(temp.path())),
            Err(CreateError::Conflict)
        );
    }
}
