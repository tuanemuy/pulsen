//! 結果を与える `TaskRepository` のダブル。

use std::cell::RefCell;
use std::collections::VecDeque;

use pulsen_domain::task::{
    ArchiveError, CreateError, DegradedTask, ReadError, SaveError, Task, TaskEntry, TaskId,
    TaskLookup, TaskRepository,
};

/// あらかじめ与えた結果を順に返し、渡されたタスクを記録するリポジトリ。
///
/// 扱うのは `create` / `list_active` / `save` / `save_degraded` の4メソッドにする —
/// ここまでのユースケース(タスク登録・tick)が呼ぶのはこれらであり、残りに台本を
/// 持たせても検証する対象がない。呼ばれた場合はテスト側の前提が崩れているため、値を
/// 返さずパニックさせる。
#[derive(Debug, Default)]
pub struct ScriptedTaskRepository {
    create: RefCell<VecDeque<Result<(), CreateError>>>,
    list_active: RefCell<VecDeque<Result<Vec<TaskEntry>, ReadError>>>,
    save: RefCell<VecDeque<Result<(), SaveError>>>,
    save_degraded: RefCell<VecDeque<Result<(), SaveError>>>,
    created: RefCell<Vec<Task>>,
    saved: RefCell<Vec<Task>>,
    saved_degraded: RefCell<Vec<DegradedTask>>,
}

impl ScriptedTaskRepository {
    /// どのメソッドの台本も持たないリポジトリを作る。
    pub fn new() -> Self {
        Self::default()
    }

    /// `create` が返す結果の列を与える。
    pub fn with_create(self, results: impl IntoIterator<Item = Result<(), CreateError>>) -> Self {
        *self.create.borrow_mut() = results.into_iter().collect();
        self
    }

    /// `list_active` が返す結果の列を与える。
    pub fn with_list_active(
        self,
        results: impl IntoIterator<Item = Result<Vec<TaskEntry>, ReadError>>,
    ) -> Self {
        *self.list_active.borrow_mut() = results.into_iter().collect();
        self
    }

    /// `save` が返す結果の列を与える。
    pub fn with_save(self, results: impl IntoIterator<Item = Result<(), SaveError>>) -> Self {
        *self.save.borrow_mut() = results.into_iter().collect();
        self
    }

    /// `save_degraded` が返す結果の列を与える。
    pub fn with_save_degraded(
        self,
        results: impl IntoIterator<Item = Result<(), SaveError>>,
    ) -> Self {
        *self.save_degraded.borrow_mut() = results.into_iter().collect();
        self
    }

    /// これまでに `save_degraded` へ渡されたタスク。
    pub fn saved_degraded(&self) -> Vec<DegradedTask> {
        self.saved_degraded.borrow().clone()
    }

    /// これまでに `create` へ渡されたタスク。
    ///
    /// 失敗した呼び出しも記録する — 「どのIDで何回試みたか」が検証の対象になる。
    pub fn created(&self) -> Vec<Task> {
        self.created.borrow().clone()
    }

    /// これまでに `save` へ渡されたタスク。
    ///
    /// tick の主張は「何が永続化されたか」なので、成否によらず渡された値を残す。
    /// 「1件も書き込まれない」という主張も、この列が空であることとして書ける。
    pub fn saved(&self) -> Vec<Task> {
        self.saved.borrow().clone()
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

    fn save(&self, task: &Task) -> Result<(), SaveError> {
        self.saved.borrow_mut().push(task.clone());
        let Some(result) = self.save.borrow_mut().pop_front() else {
            panic!("save の結果を使い切った")
        };
        result
    }

    fn save_degraded(&self, task: &DegradedTask) -> Result<(), SaveError> {
        self.saved_degraded.borrow_mut().push(task.clone());
        let Some(result) = self.save_degraded.borrow_mut().pop_front() else {
            panic!("save_degraded の結果を使い切った")
        };
        result
    }

    fn find(&self, _id: &TaskId) -> Result<TaskLookup, ReadError> {
        panic!("このダブルは create / list_active / save のみを扱う")
    }

    fn list_active(&self) -> Result<Vec<TaskEntry>, ReadError> {
        let Some(result) = self.list_active.borrow_mut().pop_front() else {
            panic!("list_active の結果を使い切った")
        };
        result
    }

    fn list_archived(&self) -> Result<Vec<TaskEntry>, ReadError> {
        panic!("このダブルは create / list_active / save のみを扱う")
    }

    fn archive(&self, _id: &TaskId) -> Result<(), ArchiveError> {
        panic!("このダブルは create / list_active / save のみを扱う")
    }
}
