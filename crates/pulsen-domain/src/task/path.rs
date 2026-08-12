//! パスの値オブジェクトと、ファイルレイアウトの決定的導出。
//!
//! レイアウトの知識をドメインに置き、アダプターは導出結果を受け取るだけにする
//! (ポートの外にレイアウトが漏れない)。

use std::ffi::OsStr;
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
    /// run ディレクトリを収める段の名前。
    const RUNS_DIR: &'static str = "runs";
    /// attempt ディレクトリの接頭辞。
    const ATTEMPT_PREFIX: &'static str = "attempt-";
    /// pid ファイルの名前。
    const PID_FILE: &'static str = "pid";
    /// starttime ファイルの名前。
    const STARTTIME_FILE: &'static str = "starttime";
    /// exit ファイルの名前。
    const EXIT_FILE: &'static str = "exit";
    /// 標準出力ログの名前。
    const STDOUT_LOG: &'static str = "stdout.log";
    /// 標準エラーログの名前。
    const STDERR_LOG: &'static str = "stderr.log";
    /// 無効化マーカーの名前。
    const MARKER_FILE: &'static str = "invalidated";

    /// `<state_root>/runs/<task-id>/attempt-<n>` を導出する。
    ///
    /// 決定的導出だが、人間が直接辿れるようにタスクファイルにも記録する
    /// (再構築は `parse` を通る)。
    pub fn derive(state_root: &StateRoot, id: &TaskId, number: AttemptNumber) -> Self {
        let path = state_root
            .as_path()
            .join(Self::RUNS_DIR)
            .join(id.as_str())
            .join(format!("{}{}", Self::ATTEMPT_PREFIX, number.get()));
        Self(path)
    }

    /// `derive` の逆写像。`<state_root>/runs/<task-id>/attempt-<n>` の形でなければ `None`。
    ///
    /// ラッパーは config もホームも読まず、起動引数の run ディレクトリだけから run
    /// ディレクトリの読み書きを組み立てる。レイアウトの知識を合成ルートに漏らさないため、
    /// 復元を `derive` の直下に置く。
    pub fn state_root(&self) -> Option<StateRoot> {
        let number = self.0.file_name()?.to_str()?;
        let number = number.strip_prefix(Self::ATTEMPT_PREFIX)?;
        let number = AttemptNumber::parse(number.parse().ok()?).ok()?;

        let task_dir = self.0.parent()?;
        let id = TaskId::parse(task_dir.file_name()?.to_str()?.to_owned()).ok()?;

        let runs_dir = task_dir.parent()?;
        if runs_dir.file_name()? != OsStr::new(Self::RUNS_DIR) {
            return None;
        }
        let state_root = StateRoot::parse(runs_dir.parent()?.to_path_buf()).ok()?;

        // 逆写像であることを `derive` との一致で確かめる。番号の表記ゆれ(`attempt-01` /
        // `attempt-+1`)は数値としては読めるが、`derive` はその値を出力しない。
        (Self::derive(&state_root, &id, number) == *self).then_some(state_root)
    }

    /// pid ファイル `<run_dir>/pid`。
    pub fn pid_file(&self) -> PathBuf {
        self.0.join(Self::PID_FILE)
    }

    /// starttime ファイル `<run_dir>/starttime`。
    pub fn starttime_file(&self) -> PathBuf {
        self.0.join(Self::STARTTIME_FILE)
    }

    /// exit ファイル `<run_dir>/exit`。
    pub fn exit_file(&self) -> PathBuf {
        self.0.join(Self::EXIT_FILE)
    }

    /// 標準出力ログ `<run_dir>/stdout.log`。
    pub fn stdout_log(&self) -> PathBuf {
        self.0.join(Self::STDOUT_LOG)
    }

    /// 標準エラーログ `<run_dir>/stderr.log`。
    pub fn stderr_log(&self) -> PathBuf {
        self.0.join(Self::STDERR_LOG)
    }

    /// 無効化マーカー `<run_dir>/invalidated`。存在のみが意味を持つ空ファイル。
    pub fn marker_file(&self) -> PathBuf {
        self.0.join(Self::MARKER_FILE)
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
    fn runディレクトリのファイル配置は固定の名前で導出される() {
        let run_dir = RunDirPath::derive(
            &state_root(),
            &task_id(),
            AttemptNumber::parse(3).expect("受理される"),
        );
        let in_run_dir = |name: &str| run_dir.as_path().join(name);

        assert_eq!(run_dir.pid_file(), in_run_dir("pid"));
        assert_eq!(run_dir.starttime_file(), in_run_dir("starttime"));
        assert_eq!(run_dir.exit_file(), in_run_dir("exit"));
        assert_eq!(run_dir.stdout_log(), in_run_dir("stdout.log"));
        assert_eq!(run_dir.stderr_log(), in_run_dir("stderr.log"));
        assert_eq!(run_dir.marker_file(), in_run_dir("invalidated"));
    }

    #[test]
    fn 導出したrunディレクトリからstateルートを復元できる() {
        for number in [1, 2, 42] {
            let run_dir = RunDirPath::derive(
                &state_root(),
                &task_id(),
                AttemptNumber::parse(number).expect("受理される"),
            );
            assert_eq!(run_dir.state_root(), Some(state_root()), "attempt-{number}");
        }
    }

    #[test]
    fn 規定の形でないrunディレクトリからはstateルートを復元しない() {
        for segments in [
            vec!["state", "runs", "20260811t091530-k3f9qa1b"],
            vec!["state", "runs", "20260811t091530-k3f9qa1b", "attempt-0"],
            vec!["state", "runs", "20260811t091530-k3f9qa1b", "attempt-01"],
            vec!["state", "runs", "20260811t091530-k3f9qa1b", "attempt-+1"],
            vec!["state", "runs", "20260811t091530-k3f9qa1b", "attempt-x"],
            vec!["state", "runs", "20260811t091530-k3f9qa1b", "attempt-"],
            vec!["state", "runs", "20260811t091530-k3f9qa1b", "1"],
            vec!["state", "attempts", "20260811t091530-k3f9qa1b", "attempt-1"],
            vec!["state", "runs", "大文字と記号を含む名前", "attempt-1"],
            vec!["attempt-1"],
        ] {
            let run_dir = RunDirPath::parse(absolute(&segments)).expect("受理される");
            assert_eq!(run_dir.state_root(), None, "{segments:?}");
        }
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
