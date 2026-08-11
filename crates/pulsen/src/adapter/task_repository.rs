//! タスクファイルを読み書きする `TaskRepository` の実装。
//!
//! パスの導出と命名形式はドメインの `TaskFilePath` が単一の定義箇所であり、この
//! アダプターはレイアウトを組み立て直さない。書き込みは常に全体のアトミック置換で、
//! 移動は単一ファイルの rename に帰着する(ADR-015)。

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use pulsen_domain::task::{
    ArchiveError, CreateError, DegradedTask, ReadError, SaveError, StateRoot, Task, TaskEntry,
    TaskFilePath, TaskId, TaskLookup, TaskRepository,
};

use super::task_file;
use crate::util::atomic::{rename_atomic, write_atomic};

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

    /// 存在の有無だけを見る。デコードの可否は問わない — 破損したファイルも
    /// 「その ID は使われている」ことの証拠であり、上書きすると修復材料が消える。
    fn exists(&self, area: Area, id: &TaskId) -> io::Result<bool> {
        self.path(area, id).try_exists()
    }

    fn lookup(&self, area: Area, id: &TaskId) -> Result<Option<TaskLookup>, ReadError> {
        let path = self.path(area, id);
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
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
            let bytes = match fs::read(&path) {
                Ok(bytes) => bytes,
                // 走査中にアーカイブされたエントリは、この領域にもう無いだけで失敗ではない。
                // 読み取りはロックなしで常に一貫した内容を返す契約なので、`archive` の
                // 中間状態を走査全体の失敗として観測させない。
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
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
        let existing = match fs::read(&path) {
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
        rename_atomic(&from, &to).map_err(|error| ArchiveError::Io {
            message: message(&to, &error),
        })
    }
}

fn message(path: &Path, error: &io::Error) -> String {
    format!("{}: {error}", path.display())
}
