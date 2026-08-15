//! 読み込み結果を与える `WorkflowStore` のダブル。
//!
//! `ConfigStore` のダブルは置かない — タスク登録のユースケースは設定を読み込み済みの
//! `GlobalConfig` として受け取り、`ConfigStore` ポートを引数に取らないため、差し替える
//! 場所が無い。

use std::cell::RefCell;
use std::collections::VecDeque;

use pulsen_domain::definition::{LoadedWorkflow, WorkflowLoadError, WorkflowRef, WorkflowStore};

/// あらかじめ与えた結果を順に返し、解決を求められた参照を記録するストア。
#[derive(Debug)]
pub struct ScriptedWorkflowStore {
    results: RefCell<VecDeque<Result<LoadedWorkflow, WorkflowLoadError>>>,
    requested: RefCell<Vec<WorkflowRef>>,
}

impl ScriptedWorkflowStore {
    /// `load` が返す結果の列を与える。
    pub fn new(
        results: impl IntoIterator<Item = Result<LoadedWorkflow, WorkflowLoadError>>,
    ) -> Self {
        Self {
            results: RefCell::new(results.into_iter().collect()),
            requested: RefCell::new(Vec::new()),
        }
    }

    /// これまでに解決を求められた参照。
    pub fn requested(&self) -> Vec<WorkflowRef> {
        self.requested.borrow().clone()
    }
}

impl WorkflowStore for ScriptedWorkflowStore {
    fn load(&self, wf_ref: &WorkflowRef) -> Result<LoadedWorkflow, WorkflowLoadError> {
        self.requested.borrow_mut().push(wf_ref.clone());
        let Some(result) = self.results.borrow_mut().pop_front() else {
            panic!("load の結果を使い切った")
        };
        result
    }
}
