//! 発行するIDを与える `TaskIdGenerator` のダブル。

use std::cell::RefCell;
use std::collections::VecDeque;

use pulsen_domain::task::{TaskId, TaskIdGenerator};

/// あらかじめ与えたIDを順に発行するジェネレーター。
///
/// 実アダプターは乱数でIDを作るため、外から「既存タスクと衝突するID」を与えられない
/// (TC-task-register-task-012 / 047)。発行される値を決められるのはダブルだけになる。
#[derive(Debug)]
pub struct ScriptedTaskIdGenerator {
    ids: RefCell<VecDeque<TaskId>>,
    issued: RefCell<Vec<TaskId>>,
}

impl ScriptedTaskIdGenerator {
    /// 発行するIDの列を与える。
    pub fn new(ids: impl IntoIterator<Item = TaskId>) -> Self {
        Self {
            ids: RefCell::new(ids.into_iter().collect()),
            issued: RefCell::new(Vec::new()),
        }
    }

    /// これまでに発行したID。
    pub fn issued(&self) -> Vec<TaskId> {
        self.issued.borrow().clone()
    }
}

impl TaskIdGenerator for ScriptedTaskIdGenerator {
    fn generate(&self) -> TaskId {
        let Some(id) = self.ids.borrow_mut().pop_front() else {
            panic!("発行するIDを使い切った")
        };
        self.issued.borrow_mut().push(id.clone());
        id
    }
}
