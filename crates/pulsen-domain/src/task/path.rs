//! パスの値オブジェクトと、ファイルレイアウトの決定的導出。
//!
//! レイアウトの知識をドメインに置き、アダプターは導出結果を受け取るだけにする
//! (ポートの外にレイアウトが漏れない)。

use std::path::{Path, PathBuf};

use super::attempt::AttemptNumber;
use super::id::TaskId;

/// 絶対パス制約の破れ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbsolutePathError {
    /// 相対パスが与えられた。
    NotAbsolute {
        /// 与えられたパス。
        given: PathBuf,
    },
}

impl AbsolutePathError {
    /// 制約の説明。
    ///
    /// タスクファイルの復号に失敗した理由は、破損したタスクの一覧・表示を通じて
    /// 利用者に見せる修復の材料になる。説明の定義箇所をドメインに1つ置く。
    pub fn describe(&self) -> String {
        match self {
            Self::NotAbsolute { given } => {
                format!("絶対パスである必要があります(実際は `{}`)", given.display())
            }
        }
    }
}

macro_rules! absolute_path {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        ///
        /// 絶対パスとしてのみ生成する。tick は任意のカレントディレクトリから起動される
        /// ため、相対パスを帳簿に載せない。
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(PathBuf);

        impl $name {
            /// 絶対パスとしてのみ生成する。
            pub fn parse(path: PathBuf) -> Result<Self, AbsolutePathError> {
                if !path.is_absolute() {
                    return Err(AbsolutePathError::NotAbsolute { given: path });
                }
                Ok(Self(path))
            }

            /// 保持するパスを借用する。
            pub fn as_path(&self) -> &Path {
                &self.0
            }

            /// 保持するパスを取り出す。
            pub fn into_path_buf(self) -> PathBuf {
                self.0
            }
        }
    };
}

absolute_path! {
    /// タスクが作用する git リポジトリのパス。
    RepoPath
}

absolute_path! {
    /// タスクに割り当てられた worktree のパス。
    WorktreePath
}

absolute_path! {
    /// グローバルホーム配下の `state/`。
    StateRoot
}

absolute_path! {
    /// グローバルホーム配下の `worktrees/`。
    WorktreeRoot
}

absolute_path! {
    /// attempt 単位の run ディレクトリ。
    RunDirPath
}

impl RunDirPath {
    /// `<state_root>/runs/<task-id>/attempt-<n>` を導出する。
    ///
    /// 決定的導出だが、人間が直接辿れるようにタスクファイルにも記録する
    /// (再構築は `parse` を通る)。
    pub fn derive(state_root: &StateRoot, id: &TaskId, number: AttemptNumber) -> Self {
        let path = state_root
            .as_path()
            .join("runs")
            .join(id.as_str())
            .join(format!("attempt-{}", number.get()));
        Self(path)
    }
}

/// タスクファイルのパス導出。値としては構築しない名前空間。
pub enum TaskFilePath {}

impl TaskFilePath {
    /// タスクファイルの拡張子。
    const EXTENSION: &'static str = ".json";

    /// 現役タスクのディレクトリ `<state_root>/tasks/`。
    pub fn active_dir(state_root: &StateRoot) -> PathBuf {
        state_root.as_path().join("tasks")
    }

    /// アーカイブ済みタスクのディレクトリ `<state_root>/archive/`。
    pub fn archived_dir(state_root: &StateRoot) -> PathBuf {
        state_root.as_path().join("archive")
    }

    /// 現役タスクのパス `<state_root>/tasks/<task-id>.json`。
    pub fn active(state_root: &StateRoot, id: &TaskId) -> PathBuf {
        Self::active_dir(state_root).join(Self::file_name(id))
    }

    /// アーカイブ済みタスクのパス `<state_root>/archive/<task-id>.json`。
    pub fn archived(state_root: &StateRoot, id: &TaskId) -> PathBuf {
        Self::archived_dir(state_root).join(Self::file_name(id))
    }

    /// タスクファイルの名前 `<task-id>.json`。
    pub fn file_name(id: &TaskId) -> String {
        format!("{}{}", id.as_str(), Self::EXTENSION)
    }

    /// 命名形式に合致する名前からタスクIDを読み取る。
    ///
    /// 走査は形式に合致するエントリのみを対象とする契約であり、形式外(アトミック置換の
    /// 一時ファイル残骸・手動で置かれたファイル等)は `None` になる。
    pub fn parse_file_name(file_name: &str) -> Option<TaskId> {
        let id = file_name.strip_suffix(Self::EXTENSION)?;
        TaskId::parse(id.to_owned()).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// プラットフォームで絶対になるパスを組み立てる。
    fn absolute(segments: &[&str]) -> PathBuf {
        let mut path = if std::path::MAIN_SEPARATOR == '\\' {
            PathBuf::from("C:\\")
        } else {
            PathBuf::from("/")
        };
        for segment in segments {
            path.push(segment);
        }
        path
    }

    fn state_root() -> StateRoot {
        StateRoot::parse(absolute(&["home", "u", ".pulsen", "state"])).expect("受理される")
    }

    fn task_id() -> TaskId {
        TaskId::parse("20260811t091530-k3f9qa1b".to_owned()).expect("受理される")
    }

    #[test]
    fn 相対パスは受理されない() {
        let given = PathBuf::from("relative/path");
        assert_eq!(
            RepoPath::parse(given.clone()),
            Err(AbsolutePathError::NotAbsolute {
                given: given.clone()
            })
        );
        assert_eq!(
            WorktreePath::parse(given.clone()),
            Err(AbsolutePathError::NotAbsolute {
                given: given.clone()
            })
        );
        assert_eq!(
            StateRoot::parse(given.clone()),
            Err(AbsolutePathError::NotAbsolute {
                given: given.clone()
            })
        );
        assert_eq!(
            WorktreeRoot::parse(given.clone()),
            Err(AbsolutePathError::NotAbsolute {
                given: given.clone()
            })
        );
        assert_eq!(
            RunDirPath::parse(given.clone()),
            Err(AbsolutePathError::NotAbsolute { given })
        );
    }

    #[test]
    fn 絶対パスは受理されそのまま保持される() {
        let given = absolute(&["repos", "pulsen"]);
        let repo = RepoPath::parse(given.clone()).expect("受理される");
        assert_eq!(repo.as_path(), given.as_path());
        assert_eq!(repo.into_path_buf(), given);
    }

    #[test]
    fn runディレクトリはstateルートとタスクidとattempt番号から導出される() {
        let number = AttemptNumber::parse(3).expect("受理される");
        let run_dir = RunDirPath::derive(&state_root(), &task_id(), number);
        assert_eq!(
            run_dir.as_path(),
            absolute(&[
                "home",
                "u",
                ".pulsen",
                "state",
                "runs",
                "20260811t091530-k3f9qa1b",
                "attempt-3",
            ])
            .as_path()
        );
    }

    #[test]
    fn タスクファイルのパスは現役とアーカイブで別のディレクトリに導出される() {
        assert_eq!(
            TaskFilePath::active(&state_root(), &task_id()),
            absolute(&[
                "home",
                "u",
                ".pulsen",
                "state",
                "tasks",
                "20260811t091530-k3f9qa1b.json",
            ])
        );
        assert_eq!(
            TaskFilePath::archived(&state_root(), &task_id()),
            absolute(&[
                "home",
                "u",
                ".pulsen",
                "state",
                "archive",
                "20260811t091530-k3f9qa1b.json",
            ])
        );
    }

    #[test]
    fn 命名形式に合致する名前からタスクidが読み取れる() {
        assert_eq!(
            TaskFilePath::parse_file_name(&TaskFilePath::file_name(&task_id())),
            Some(task_id())
        );
    }

    #[test]
    fn 命名形式に合致しない名前はタスクidにならない() {
        for name in [
            "20260811t091530-k3f9qa1b",
            "20260811t091530-k3f9qa1b.json.tmp",
            ".tmpA1b2C3.json",
            "notes.txt",
            "大文字を含む名前.json",
            ".json",
        ] {
            assert_eq!(TaskFilePath::parse_file_name(name), None, "{name}");
        }
    }
}
