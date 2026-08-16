//! 結果を与える `TaskRepository` のダブル。

use std::cell::RefCell;
use std::collections::VecDeque;

use pulsen_domain::task::{
    ArchiveError, CreateError, DegradedTask, ReadError, SaveError, Task, TaskEntry, TaskId,
    TaskLookup, TaskRepository,
};

use super::RecordSeq;

/// あらかじめ与えた結果を順に返し、渡されたタスクを記録するリポジトリ。
///
/// 扱うのは `create` / `find` / `list_active` / `list_archived` / `save` /
/// `save_degraded` の6メソッドにする — ここまでのユースケース(タスク登録・tick・
/// 一覧・詳細)が呼ぶのはこれらであり、残り(`archive`)に台本を持たせても検証する
/// 対象がない。呼ばれた場合はテスト側の前提が崩れているため、値を返さずパニックさせる。
#[derive(Debug, Default)]
pub struct ScriptedTaskRepository {
    create: RefCell<VecDeque<Result<(), CreateError>>>,
    find: RefCell<VecDeque<Result<TaskLookup, ReadError>>>,
    list_active: RefCell<VecDeque<Result<Vec<TaskEntry>, ReadError>>>,
    list_archived: RefCell<VecDeque<Result<Vec<TaskEntry>, ReadError>>>,
    save: RefCell<VecDeque<Result<(), SaveError>>>,
    save_degraded: RefCell<VecDeque<Result<(), SaveError>>>,
    created: RefCell<Vec<Task>>,
    looked_up: RefCell<Vec<TaskId>>,
    saved: RefCell<Vec<(RecordSeq, Task)>>,
    saved_degraded: RefCell<Vec<(RecordSeq, DegradedTask)>>,
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

    /// `find` が返す結果の列を与える。
    pub fn with_find(
        self,
        results: impl IntoIterator<Item = Result<TaskLookup, ReadError>>,
    ) -> Self {
        *self.find.borrow_mut() = results.into_iter().collect();
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

    /// `list_archived` が返す結果の列を与える。
    pub fn with_list_archived(
        self,
        results: impl IntoIterator<Item = Result<Vec<TaskEntry>, ReadError>>,
    ) -> Self {
        *self.list_archived.borrow_mut() = results.into_iter().collect();
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
        self.saved_degraded
            .borrow()
            .iter()
            .map(|(_, task)| task.clone())
            .collect()
    }

    /// これまでに `create` へ渡されたタスク。
    ///
    /// 失敗した呼び出しも記録する — 「どのIDで何回試みたか」が検証の対象になる。
    pub fn created(&self) -> Vec<Task> {
        self.created.borrow().clone()
    }

    /// これまでに `find` へ渡されたタスクID。
    ///
    /// 解決順(現役 → アーカイブ)はポートの契約であり、呼び出し側が主張するのは
    /// 「どのIDを何回引いたか」だけになる。
    pub fn looked_up(&self) -> Vec<TaskId> {
        self.looked_up.borrow().clone()
    }

    /// これまでに `save` へ渡されたタスク。
    ///
    /// tick の主張は「何が永続化されたか」なので、成否によらず渡された値を残す。
    /// 「1件も書き込まれない」という主張も、この列が空であることとして書ける。
    pub fn saved(&self) -> Vec<Task> {
        self.saved
            .borrow()
            .iter()
            .map(|(_, task)| task.clone())
            .collect()
    }

    /// これまでに `save` へ渡されたタスクを、ほかのダブルの記録と並べられる採番つきで返す。
    pub fn saved_in_order(&self) -> Vec<(RecordSeq, Task)> {
        self.saved.borrow().clone()
    }

    /// これまでに `save_degraded` へ渡されたタスクを、ほかのダブルの記録と並べられる
    /// 採番つきで返す。
    ///
    /// 通知の順序の契約は書き戻し先の型で変わらないため、縮退したタスクの再通知も
    /// `save` / `run` と同じ1本の列に並べられる必要がある。
    pub fn saved_degraded_in_order(&self) -> Vec<(RecordSeq, DegradedTask)> {
        self.saved_degraded.borrow().clone()
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
        self.saved
            .borrow_mut()
            .push((RecordSeq::next(), task.clone()));
        let Some(result) = self.save.borrow_mut().pop_front() else {
            panic!("save の結果を使い切った")
        };
        result
    }

    fn save_degraded(&self, task: &DegradedTask) -> Result<(), SaveError> {
        self.saved_degraded
            .borrow_mut()
            .push((RecordSeq::next(), task.clone()));
        let Some(result) = self.save_degraded.borrow_mut().pop_front() else {
            panic!("save_degraded の結果を使い切った")
        };
        result
    }

    fn find(&self, id: &TaskId) -> Result<TaskLookup, ReadError> {
        self.looked_up.borrow_mut().push(id.clone());
        let Some(result) = self.find.borrow_mut().pop_front() else {
            panic!("find の結果を使い切った")
        };
        result
    }

    fn list_active(&self) -> Result<Vec<TaskEntry>, ReadError> {
        let Some(result) = self.list_active.borrow_mut().pop_front() else {
            panic!("list_active の結果を使い切った")
        };
        result
    }

    fn list_archived(&self) -> Result<Vec<TaskEntry>, ReadError> {
        let Some(result) = self.list_archived.borrow_mut().pop_front() else {
            panic!("list_archived の結果を使い切った")
        };
        result
    }

    fn archive(&self, _id: &TaskId) -> Result<(), ArchiveError> {
        panic!("このダブルは archive を扱わない(#6 で台本を足す)")
    }
}
