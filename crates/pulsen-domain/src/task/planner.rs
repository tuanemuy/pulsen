//! タスクIDからワークスペースを導出する。

use super::branch::{BranchName, Workspace};
use super::id::TaskId;
use super::path::{WorktreePath, WorktreeRoot};

/// ワークスペース(worktree のパス・ブランチ)の決定的導出(純粋)。
pub struct WorkspacePlanner;

impl WorkspacePlanner {
    /// タスクのブランチ名の接頭辞。
    pub const BRANCH_PREFIX: &'static str = "pulsen/";

    /// `path = <worktree_root>/<task-id>`、`branch = pulsen/<task-id>` を導出する。
    ///
    /// 起動(手続きA)と終端処理(手続きB)が同じワークスペースへ到達するための単一の
    /// 導出点。`TaskId` の文字集合はパス・git 参照名として常に安全なので全域関数とし、
    /// 生成の失敗は不変条件の破れとして扱う。
    pub fn derive(worktree_root: &WorktreeRoot, id: &TaskId) -> Workspace {
        let path = WorktreePath::parse(worktree_root.as_path().join(id.as_str()))
            .expect("絶対パスの worktree_root にタスクIDを足したパスは絶対パスである");
        let branch = BranchName::parse(format!("{}{}", Self::BRANCH_PREFIX, id.as_str()))
            .expect("タスクIDの文字集合は git 参照名として常に有効である");
        Workspace::new(path, branch)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn worktree_root() -> WorktreeRoot {
        WorktreeRoot::parse(PathBuf::from(if std::path::MAIN_SEPARATOR == '\\' {
            "C:\\home\\u\\.pulsen\\worktrees"
        } else {
            "/home/u/.pulsen/worktrees"
        }))
        .expect("受理される")
    }

    #[test]
    fn ワークスペースはworktreeルートとタスクidから導出される() {
        let id = TaskId::parse("20260811t091530-k3f9qa1b".to_owned()).expect("受理される");
        let workspace = WorkspacePlanner::derive(&worktree_root(), &id);

        assert_eq!(
            workspace.path().as_path(),
            worktree_root()
                .as_path()
                .join("20260811t091530-k3f9qa1b")
                .as_path()
        );
        assert_eq!(
            workspace.branch().as_str(),
            "pulsen/20260811t091530-k3f9qa1b"
        );
    }

    #[test]
    fn 同じタスクidからは常に同じワークスペースが導出される() {
        let id = TaskId::parse("t1".to_owned()).expect("受理される");
        assert_eq!(
            WorkspacePlanner::derive(&worktree_root(), &id),
            WorkspacePlanner::derive(&worktree_root(), &id)
        );
    }

    #[test]
    fn タスクidが違えばワークスペースも違う() {
        let one = TaskId::parse("t1".to_owned()).expect("受理される");
        let other = TaskId::parse("t2".to_owned()).expect("受理される");
        assert_ne!(
            WorkspacePlanner::derive(&worktree_root(), &one),
            WorkspacePlanner::derive(&worktree_root(), &other)
        );
    }

    #[test]
    fn 文字集合の端のタスクidからも導出できる() {
        for raw in [
            "a",
            "0",
            "a--b",
            "abc-",
            &"z9-".repeat(TaskId::MAX_LENGTH / 3),
        ] {
            let id = TaskId::parse(raw.to_owned()).expect("受理される");
            let workspace = WorkspacePlanner::derive(&worktree_root(), &id);
            assert_eq!(
                workspace.branch().as_str(),
                format!("{}{raw}", WorkspacePlanner::BRANCH_PREFIX),
            );
        }
    }
}
