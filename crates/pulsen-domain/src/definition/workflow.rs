//! ワークフロー定義とステータス定義。

use std::collections::BTreeMap;

use super::command::PlainCommand;
use super::duration::{DurationSpec, TimeoutSpec};
use super::name::{AgentName, ModelName, StatusName};
use super::template::AgentInput;

/// 1ステータスの動作と付随設定。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatusDefinition {
    /// エージェントを実行する。
    AgentRun {
        /// 与える入力。
        input: AgentInput,
        /// ワークフローデフォルトを上書きするエージェント。
        agent: Option<AgentName>,
        /// ワークフローデフォルトを上書きするモデル。
        model: Option<ModelName>,
        /// 組み込みデフォルトを上書きする timeout。
        timeout: Option<TimeoutSpec>,
        /// 組み込みデフォルトを上書きするリトライ上限(0 = 初回失敗で即 stopped)。
        retries: Option<u32>,
        /// 判定コマンド。省略時は exit code 判定。
        judge: Option<PlainCommand>,
        /// 成功時の遷移先。
        next: StatusName,
    },
    /// 何もしない。
    Wait,
    /// クリーンアップする。
    Cleanup,
}

/// ワークフロー定義の構造不変条件の破れ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkflowStructureError {
    /// ステータスが1件もない。
    EmptyStatuses,
    /// `initial` の参照先がない。
    InitialNotFound {
        /// 参照された初期ステータス名。
        initial: StatusName,
    },
    /// `next` の参照先がない。
    NextNotFound {
        /// 参照元のステータス名。
        status: StatusName,
        /// 参照された遷移先。
        next: StatusName,
    },
}

impl WorkflowStructureError {
    /// 破れた不変条件の説明。
    ///
    /// スナップショットを復元できない理由は、破損したタスクの一覧・表示を通じて
    /// 利用者に見せる修復の材料になる。説明の定義箇所をドメインに1つ置く。
    pub fn describe(&self) -> String {
        match self {
            Self::EmptyStatuses => "ステータスが1件もありません".to_owned(),
            Self::InitialNotFound { initial } => format!(
                "initial が指すステータス `{}` が statuses にありません",
                initial.as_str()
            ),
            Self::NextNotFound { status, next } => format!(
                "ステータス `{}` の next が指す `{}` が statuses にありません",
                status.as_str(),
                next.as_str()
            ),
        }
    }
}

/// ステータスの集合と初期ステータス・デフォルト。
///
/// ワークフロー名は保持しない — 表示名は登録時に `WorkflowRef` の規則で決まり、
/// タスクにのみ記録される。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowDefinition {
    default_agent: Option<AgentName>,
    default_model: Option<ModelName>,
    initial: StatusName,
    statuses: BTreeMap<StatusName, StatusDefinition>,
}

impl WorkflowDefinition {
    /// timeout の組み込みデフォルト(ADR-014)。
    pub const DEFAULT_TIMEOUT: TimeoutSpec =
        TimeoutSpec::Limited(DurationSpec::from_secs_unchecked(60 * 60));
    /// リトライ上限の組み込みデフォルト(ADR-014)。
    pub const DEFAULT_RETRY_LIMIT: u32 = 2;

    /// 構造不変条件(`initial ∈ statuses`・全 `AgentRun.next ∈ statuses`)を検証して生成する。
    /// 循環・自己参照・到達不能は許容する(ADR-010)。
    pub fn new(
        default_agent: Option<AgentName>,
        default_model: Option<ModelName>,
        initial: StatusName,
        statuses: BTreeMap<StatusName, StatusDefinition>,
    ) -> Result<Self, WorkflowStructureError> {
        if statuses.is_empty() {
            return Err(WorkflowStructureError::EmptyStatuses);
        }
        if !statuses.contains_key(&initial) {
            return Err(WorkflowStructureError::InitialNotFound { initial });
        }
        for (name, definition) in &statuses {
            match definition {
                StatusDefinition::AgentRun { next, .. } => {
                    if !statuses.contains_key(next) {
                        return Err(WorkflowStructureError::NextNotFound {
                            status: name.clone(),
                            next: next.clone(),
                        });
                    }
                }
                StatusDefinition::Wait | StatusDefinition::Cleanup => {}
            }
        }

        Ok(Self {
            default_agent,
            default_model,
            initial,
            statuses,
        })
    }

    /// ワークフロー単位のデフォルトエージェント。
    pub fn default_agent(&self) -> Option<&AgentName> {
        self.default_agent.as_ref()
    }

    /// ワークフロー単位のデフォルトモデル。
    pub fn default_model(&self) -> Option<&ModelName> {
        self.default_model.as_ref()
    }

    /// 初期ステータス。
    pub fn initial(&self) -> &StatusName {
        &self.initial
    }

    /// 全ステータス。
    pub fn statuses(&self) -> &BTreeMap<StatusName, StatusDefinition> {
        &self.statuses
    }

    /// ステータス定義を引く。
    pub fn status(&self, name: &StatusName) -> Option<&StatusDefinition> {
        self.statuses.get(name)
    }

    /// 実効エージェント。ステータスの上書き > ワークフローデフォルト。
    ///
    /// エージェント実行以外のステータスには適用対象がないため `None` を返す。
    /// この定義に無いステータス名も同じく `None` を返すが、タスクの不変条件1
    /// (`task_status ∈ snapshot.statuses`)により実運用では到達しない。
    pub fn effective_agent(&self, status: &StatusName) -> Option<&AgentName> {
        match self.statuses.get(status) {
            Some(StatusDefinition::AgentRun { agent, .. }) => {
                agent.as_ref().or(self.default_agent.as_ref())
            }
            Some(StatusDefinition::Wait | StatusDefinition::Cleanup) => None,
            None => None,
        }
    }

    /// 実効モデル。ステータスの上書き > ワークフローデフォルト。
    ///
    /// 適用対象のないステータスと、この定義に無いステータス名はいずれも `None`
    /// (`effective_agent` と同じ扱い)。
    pub fn effective_model(&self, status: &StatusName) -> Option<&ModelName> {
        match self.statuses.get(status) {
            Some(StatusDefinition::AgentRun { model, .. }) => {
                model.as_ref().or(self.default_model.as_ref())
            }
            Some(StatusDefinition::Wait | StatusDefinition::Cleanup) => None,
            None => None,
        }
    }

    /// 実効 timeout。ステータスの指定 > 組み込みデフォルト。
    ///
    /// 適用対象のないステータスと、この定義に無いステータス名はいずれも組み込み
    /// デフォルトになる(`effective_agent` と同じく、後者は不変条件1により到達しない)。
    pub fn effective_timeout(&self, status: &StatusName) -> TimeoutSpec {
        match self.statuses.get(status) {
            Some(StatusDefinition::AgentRun { timeout, .. }) => {
                timeout.unwrap_or(Self::DEFAULT_TIMEOUT)
            }
            Some(StatusDefinition::Wait | StatusDefinition::Cleanup) => Self::DEFAULT_TIMEOUT,
            None => Self::DEFAULT_TIMEOUT,
        }
    }

    /// 実効リトライ上限。`AgentRun` は `retries` > 組み込みデフォルト、
    /// `Cleanup` は常に組み込みデフォルト(ADR-014)。
    ///
    /// `Wait` に対する呼び出しは spec が規定しない(attempt_count を消費する操作が無く
    /// 適用対象がない。呼び出し側が動作種別で分岐してから使う)。この定義に無いステータス
    /// 名も同じ値を返すが、不変条件1により実運用では到達しない。
    pub fn effective_retry_limit(&self, status: &StatusName) -> u32 {
        match self.statuses.get(status) {
            Some(StatusDefinition::AgentRun { retries, .. }) => {
                retries.unwrap_or(Self::DEFAULT_RETRY_LIMIT)
            }
            Some(StatusDefinition::Wait | StatusDefinition::Cleanup) => Self::DEFAULT_RETRY_LIMIT,
            None => Self::DEFAULT_RETRY_LIMIT,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::name::Prompt;
    use super::*;

    fn status(name: &str) -> StatusName {
        StatusName::parse(name.to_owned()).expect("受理される")
    }

    fn agent(name: &str) -> AgentName {
        AgentName::parse(name.to_owned()).expect("受理される")
    }

    fn model(name: &str) -> ModelName {
        ModelName::parse(name.to_owned()).expect("受理される")
    }

    fn agent_run_with(
        next: &str,
        agent: Option<AgentName>,
        model: Option<ModelName>,
        timeout: Option<TimeoutSpec>,
        retries: Option<u32>,
    ) -> StatusDefinition {
        StatusDefinition::AgentRun {
            input: AgentInput::Prompt(Prompt::parse("実装して".to_owned()).expect("受理される")),
            agent,
            model,
            timeout,
            retries,
            judge: None,
            next: status(next),
        }
    }

    fn agent_run(next: &str) -> StatusDefinition {
        agent_run_with(next, None, None, None, None)
    }

    fn definition(statuses: Vec<(&str, StatusDefinition)>) -> WorkflowDefinition {
        let initial = status(statuses.first().expect("1件以上").0);
        WorkflowDefinition::new(
            None,
            None,
            initial,
            statuses
                .into_iter()
                .map(|(name, definition)| (status(name), definition))
                .collect(),
        )
        .expect("不変条件を満たす")
    }

    #[test]
    fn ステータスが空の定義は受理されない() {
        assert_eq!(
            WorkflowDefinition::new(None, None, status("queued"), BTreeMap::new()),
            Err(WorkflowStructureError::EmptyStatuses)
        );
    }

    #[test]
    fn 初期ステータスが存在しない定義は受理されない() {
        let statuses = BTreeMap::from([(status("done"), StatusDefinition::Cleanup)]);
        assert_eq!(
            WorkflowDefinition::new(None, None, status("queued"), statuses),
            Err(WorkflowStructureError::InitialNotFound {
                initial: status("queued")
            })
        );
    }

    #[test]
    fn 遷移先が存在しない定義は受理されない() {
        let statuses = BTreeMap::from([(status("queued"), agent_run("missing"))]);
        assert_eq!(
            WorkflowDefinition::new(None, None, status("queued"), statuses),
            Err(WorkflowStructureError::NextNotFound {
                status: status("queued"),
                next: status("missing")
            })
        );
    }

    #[test]
    fn 自己参照と循環と到達不能なステータスは受理される() {
        let cyclic = definition(vec![
            ("a", agent_run("b")),
            ("b", agent_run("a")),
            ("self", agent_run("self")),
            ("unreachable", StatusDefinition::Wait),
        ]);
        assert_eq!(cyclic.statuses().len(), 4);
    }

    #[test]
    fn 終端のないワークフローも受理される() {
        let definition = definition(vec![("only", agent_run("only"))]);
        assert_eq!(definition.initial(), &status("only"));
    }

    #[test]
    fn 実効エージェントはステータスの上書きを優先する() {
        let statuses = BTreeMap::from([(
            status("queued"),
            agent_run_with("queued", Some(agent("override")), None, None, None),
        )]);
        let definition =
            WorkflowDefinition::new(Some(agent("default")), None, status("queued"), statuses)
                .expect("不変条件を満たす");
        assert_eq!(
            definition.effective_agent(&status("queued")),
            Some(&agent("override"))
        );
    }

    #[test]
    fn 実効エージェントは上書きがなければワークフローデフォルトを使う() {
        let statuses = BTreeMap::from([(status("queued"), agent_run("queued"))]);
        let definition =
            WorkflowDefinition::new(Some(agent("default")), None, status("queued"), statuses)
                .expect("不変条件を満たす");
        assert_eq!(
            definition.effective_agent(&status("queued")),
            Some(&agent("default"))
        );
    }

    #[test]
    fn 実効エージェントはどちらの指定もなければ解決できない() {
        let definition = definition(vec![("queued", agent_run("queued"))]);
        assert_eq!(definition.effective_agent(&status("queued")), None);
    }

    #[test]
    fn 実効モデルはステータスの上書きをワークフローデフォルトより優先する() {
        let statuses = BTreeMap::from([
            (
                status("queued"),
                agent_run_with("plain", None, Some(model("opus")), None, None),
            ),
            (status("plain"), agent_run("plain")),
        ]);
        let definition =
            WorkflowDefinition::new(None, Some(model("sonnet")), status("queued"), statuses)
                .expect("不変条件を満たす");
        assert_eq!(
            definition.effective_model(&status("queued")),
            Some(&model("opus"))
        );
        assert_eq!(
            definition.effective_model(&status("plain")),
            Some(&model("sonnet"))
        );
    }

    #[test]
    fn 実効timeoutは指定がなければ組み込みデフォルトの1時間になる() {
        let definition = definition(vec![("queued", agent_run("queued"))]);
        assert_eq!(
            definition.effective_timeout(&status("queued")),
            TimeoutSpec::Limited(DurationSpec::parse("1h").expect("受理される"))
        );
    }

    #[test]
    fn 実効timeoutはステータスの指定を優先する() {
        let statuses = BTreeMap::from([(
            status("queued"),
            agent_run_with("queued", None, None, Some(TimeoutSpec::Unlimited), None),
        )]);
        let definition = WorkflowDefinition::new(None, None, status("queued"), statuses)
            .expect("不変条件を満たす");
        assert_eq!(
            definition.effective_timeout(&status("queued")),
            TimeoutSpec::Unlimited
        );
    }

    #[test]
    fn 実効リトライ上限は指定がなければ組み込みデフォルトの2になる() {
        let definition = definition(vec![
            ("queued", agent_run("queued")),
            ("done", StatusDefinition::Cleanup),
        ]);
        assert_eq!(definition.effective_retry_limit(&status("queued")), 2);
        assert_eq!(definition.effective_retry_limit(&status("done")), 2);
    }

    #[test]
    fn 実効リトライ上限はステータスの指定を優先しゼロも受理する() {
        let statuses = BTreeMap::from([(
            status("queued"),
            agent_run_with("queued", None, None, None, Some(0)),
        )]);
        let definition = WorkflowDefinition::new(None, None, status("queued"), statuses)
            .expect("不変条件を満たす");
        assert_eq!(definition.effective_retry_limit(&status("queued")), 0);
    }

    #[test]
    fn クリーンアップのリトライ上限は常に組み込みデフォルトになる() {
        let definition = definition(vec![("done", StatusDefinition::Cleanup)]);
        assert_eq!(definition.effective_retry_limit(&status("done")), 2);
    }

    #[test]
    fn ステータス定義は名前で引ける() {
        let definition = definition(vec![("waiting", StatusDefinition::Wait)]);
        assert_eq!(
            definition.status(&status("waiting")),
            Some(&StatusDefinition::Wait)
        );
        assert_eq!(definition.status(&status("missing")), None);
    }
}
