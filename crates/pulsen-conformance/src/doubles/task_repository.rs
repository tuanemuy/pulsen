//! `create` の結果を与える `TaskRepository` のダブル。

use std::cell::RefCell;
use std::collections::VecDeque;

use pulsen_domain::task::{
    ArchiveError, CreateError, DegradedTask, ReadError, SaveError, Task, TaskEntry, TaskId,
    TaskLookup, TaskRepository,
};

/// あらかじめ与えた結果を順に返し、作成されたタスクを記録するリポジトリ。
///
/// 扱うのは `create` だけにする — 本スライスのユースケース(タスク登録)が呼ぶのは
/// この1メソッドであり、残りの6メソッドに台本を持たせても検証する対象がない。
/// 呼ばれた場合はテスト側の前提が崩れているため、値を返さずパニックさせる。
#[derive(Debug)]
pub struct ScriptedTaskRepository {
    create: RefCell<VecDeque<Result<(), CreateError>>>,
    created: RefCell<Vec<Task>>,
}

impl ScriptedTaskRepository {
    /// `create` が返す結果の列を与える。
    pub fn new(results: impl IntoIterator<Item = Result<(), CreateError>>) -> Self {
        Self {
            create: RefCell::new(results.into_iter().collect()),
            created: RefCell::new(Vec::new()),
        }
    }

    /// これまでに `create` へ渡されたタスク。
    ///
    /// 失敗した呼び出しも記録する — 「どのIDで何回試みたか」が検証の対象になる。
    pub fn created(&self) -> Vec<Task> {
        self.created.borrow().clone()
    }
}

impl TaskRepository for ScriptedTaskRepository {
    fn create(&self, task: &Task) -> Result<(), CreateError> {
        self.created.borrow_mut().push(task.clone());
        let Some(result) = self.create.borrow_mut().pop_front() else {
            panic!("create の結果を使い切った")
        };
        result
    }

    fn save(&self, _task: &Task) -> Result<(), SaveError> {
        panic!("このダブルは create のみを扱う")
    }

    fn save_degraded(&self, _task: &DegradedTask) -> Result<(), SaveError> {
        panic!("このダブルは create のみを扱う")
    }

    fn find(&self, _id: &TaskId) -> Result<TaskLookup, ReadError> {
        panic!("このダブルは create のみを扱う")
    }

    fn list_active(&self) -> Result<Vec<TaskEntry>, ReadError> {
        panic!("このダブルは create のみを扱う")
    }

    fn list_archived(&self) -> Result<Vec<TaskEntry>, ReadError> {
        panic!("このダブルは create のみを扱う")
    }

    fn archive(&self, _id: &TaskId) -> Result<(), ArchiveError> {
        panic!("このダブルは create のみを扱う")
    }
}
