//! 登録時検証を通過した正規化済みワークフロー定義。

use std::collections::BTreeMap;

use super::duration::TimeoutSpec;
use super::name::{AgentName, ModelName, StatusName};
use super::workflow::{StatusDefinition, WorkflowDefinition};

/// タスクに埋め込まれるワークフロー定義。
///
/// 生成経路は「登録時検証の通過」と「永続化からの再構築」の2つだけ。構造検証は
/// `WorkflowDefinition` の生成時に済んでおり、グローバル設定との再突き合わせは行わない
/// (登録後の config.yaml 編集は読み込みを失敗させず、実行時に表面化させる。requirements §7.1)。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowSnapshot(WorkflowDefinition);

impl WorkflowSnapshot {
    /// 永続化からの再構築。TaskRepository アダプターがデコード時に使う。
    pub fn rehydrate(definition: WorkflowDefinition) -> Self {
        Self(definition)
    }

    /// 登録時検証を通過した定義から生成する。
    pub(crate) fn from_validated(definition: WorkflowDefinition) -> Self {
        Self(definition)
    }

    /// 包んでいる定義。
    pub fn definition(&self) -> &WorkflowDefinition {
        &self.0
    }

    /// ワークフロー単位のデフォルトエージェント。
    pub fn default_agent(&self) -> Option<&AgentName> {
        self.0.default_agent()
    }

    /// ワークフロー単位のデフォルトモデル。
    pub fn default_model(&self) -> Option<&ModelName> {
        self.0.default_model()
    }

    /// 初期ステータス。
    pub fn initial(&self) -> &StatusName {
        self.0.initial()
    }

    /// 全ステータス。
    pub fn statuses(&self) -> &BTreeMap<StatusName, StatusDefinition> {
        self.0.statuses()
    }

    /// ステータス定義を引く。
    pub fn status(&self, name: &StatusName) -> Option<&StatusDefinition> {
        self.0.status(name)
    }

    /// 実効エージェント。
    pub fn effective_agent(&self, status: &StatusName) -> Option<&AgentName> {
        self.0.effective_agent(status)
    }

    /// 実効モデル。
    pub fn effective_model(&self, status: &StatusName) -> Option<&ModelName> {
        self.0.effective_model(status)
    }

    /// 実効 timeout。
    pub fn effective_timeout(&self, status: &StatusName) -> TimeoutSpec {
        self.0.effective_timeout(status)
    }

    /// 実効リトライ上限。
    pub fn effective_retry_limit(&self, status: &StatusName) -> u32 {
        self.0.effective_retry_limit(status)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status(name: &str) -> StatusName {
        StatusName::parse(name.to_owned()).expect("受理される")
    }

    fn definition() -> WorkflowDefinition {
        WorkflowDefinition::new(
            None,
            None,
            status("waiting"),
            BTreeMap::from([(status("waiting"), StatusDefinition::Wait)]),
        )
        .expect("不変条件を満たす")
    }

    #[test]
    fn 再構築したスナップショットは定義をそのまま包む() {
        let snapshot = WorkflowSnapshot::rehydrate(definition());
        assert_eq!(snapshot.definition(), &definition());
    }

    #[test]
    fn スナップショットは定義の問い合わせを委譲する() {
        let snapshot = WorkflowSnapshot::rehydrate(definition());
        assert_eq!(snapshot.initial(), &status("waiting"));
        assert_eq!(snapshot.statuses().len(), 1);
        assert_eq!(
            snapshot.status(&status("waiting")),
            Some(&StatusDefinition::Wait)
        );
        assert_eq!(snapshot.default_agent(), None);
        assert_eq!(snapshot.default_model(), None);
        assert_eq!(snapshot.effective_agent(&status("waiting")), None);
        assert_eq!(snapshot.effective_model(&status("waiting")), None);
        assert_eq!(
            snapshot.effective_timeout(&status("waiting")),
            WorkflowDefinition::DEFAULT_TIMEOUT
        );
        assert_eq!(
            snapshot.effective_retry_limit(&status("waiting")),
            WorkflowDefinition::DEFAULT_RETRY_LIMIT
        );
    }
}
