//! グローバル設定。

use std::collections::BTreeMap;

use super::agent::RawAgentDefinition;
use super::command::PlainCommand;
use super::duration::DurationSpec;
use super::name::AgentName;

/// グローバル設定の値の制約違反。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GlobalConfigError {
    /// `judge_attempt_limit` が 1 未満。
    InvalidJudgeAttemptLimit {
        /// 与えられた値。
        given: u32,
    },
    /// `spawn_fail_limit` が 1 未満。
    InvalidSpawnFailLimit {
        /// 与えられた値。
        given: u32,
    },
}

/// 読み込まれたキーだけを持つ入力。省略されたキーは `None` で表す。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GlobalConfigInput {
    /// `agents`。
    pub agents: Option<BTreeMap<AgentName, RawAgentDefinition>>,
    /// `notify_cmd`。
    pub notify_cmd: Option<PlainCommand>,
    /// `judge_attempt_limit`。
    pub judge_attempt_limit: Option<u32>,
    /// `judge_timeout`。
    pub judge_timeout: Option<DurationSpec>,
    /// `spawn_fail_limit`。
    pub spawn_fail_limit: Option<u32>,
    /// `run_retention`。
    pub run_retention: Option<DurationSpec>,
}

/// 構造検証済みのグローバル設定。全キーが任意で、省略時は既定値が入る。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalConfig {
    agents: BTreeMap<AgentName, RawAgentDefinition>,
    notify_cmd: Option<PlainCommand>,
    judge_attempt_limit: u32,
    judge_timeout: DurationSpec,
    spawn_fail_limit: u32,
    run_retention: Option<DurationSpec>,
}

impl GlobalConfig {
    /// `judge_attempt_limit` の既定値。
    pub const DEFAULT_JUDGE_ATTEMPT_LIMIT: u32 = 3;
    /// `judge_timeout` の既定値。
    pub const DEFAULT_JUDGE_TIMEOUT: DurationSpec = DurationSpec::from_secs_unchecked(60);
    /// `spawn_fail_limit` の既定値。
    pub const DEFAULT_SPAWN_FAIL_LIMIT: u32 = 3;

    /// 省略されたキーを既定値で埋めて生成する。
    pub fn parse(input: GlobalConfigInput) -> Result<Self, GlobalConfigError> {
        let judge_attempt_limit = input
            .judge_attempt_limit
            .unwrap_or(Self::DEFAULT_JUDGE_ATTEMPT_LIMIT);
        if judge_attempt_limit == 0 {
            return Err(GlobalConfigError::InvalidJudgeAttemptLimit {
                given: judge_attempt_limit,
            });
        }
        let spawn_fail_limit = input
            .spawn_fail_limit
            .unwrap_or(Self::DEFAULT_SPAWN_FAIL_LIMIT);
        if spawn_fail_limit == 0 {
            return Err(GlobalConfigError::InvalidSpawnFailLimit {
                given: spawn_fail_limit,
            });
        }

        Ok(Self {
            agents: input.agents.unwrap_or_default(),
            notify_cmd: input.notify_cmd,
            judge_attempt_limit,
            judge_timeout: input.judge_timeout.unwrap_or(Self::DEFAULT_JUDGE_TIMEOUT),
            spawn_fail_limit,
            run_retention: input.run_retention,
        })
    }

    /// 定義済みのエージェント。
    pub fn agents(&self) -> &BTreeMap<AgentName, RawAgentDefinition> {
        &self.agents
    }

    /// 名前でエージェント定義を引く。
    pub fn agent(&self, name: &AgentName) -> Option<&RawAgentDefinition> {
        self.agents.get(name)
    }

    /// 通知コマンド。未設定なら通知しない。
    pub fn notify_cmd(&self) -> Option<&PlainCommand> {
        self.notify_cmd.as_ref()
    }

    /// 判定失敗の再判定上限。
    pub fn judge_attempt_limit(&self) -> u32 {
        self.judge_attempt_limit
    }

    /// 判定コマンドの timeout。
    pub fn judge_timeout(&self) -> DurationSpec {
        self.judge_timeout
    }

    /// 連続 spawn 失敗の上限。
    pub fn spawn_fail_limit(&self) -> u32 {
        self.spawn_fail_limit
    }

    /// run ディレクトリの保持期間。未設定なら gc は無効。
    pub fn run_retention(&self) -> Option<DurationSpec> {
        self.run_retention
    }
}

#[cfg(test)]
mod tests {
    use super::super::command::RawCommand;
    use super::*;

    #[test]
    fn 全キー省略の設定は既定値になる() {
        let config = GlobalConfig::parse(GlobalConfigInput::default()).expect("受理される");
        assert!(config.agents().is_empty());
        assert_eq!(config.notify_cmd(), None);
        assert_eq!(config.judge_attempt_limit(), 3);
        assert_eq!(config.judge_timeout().seconds(), 60);
        assert_eq!(config.spawn_fail_limit(), 3);
        assert_eq!(config.run_retention(), None);
    }

    #[test]
    fn 指定されたキーは既定値を上書きする() {
        let agent = AgentName::parse("claude".to_owned()).expect("受理される");
        let definition = RawAgentDefinition::new(
            RawCommand::parse_text("claude {input}").expect("受理される"),
            None,
        );
        let config = GlobalConfig::parse(GlobalConfigInput {
            agents: Some(BTreeMap::from([(agent.clone(), definition.clone())])),
            notify_cmd: Some(PlainCommand::parse_text("notify-send").expect("受理される")),
            judge_attempt_limit: Some(5),
            judge_timeout: Some(DurationSpec::parse("2m").expect("受理される")),
            spawn_fail_limit: Some(1),
            run_retention: Some(DurationSpec::parse("7d").expect("受理される")),
        })
        .expect("受理される");

        assert_eq!(config.agent(&agent), Some(&definition));
        assert_eq!(config.judge_attempt_limit(), 5);
        assert_eq!(config.judge_timeout().seconds(), 120);
        assert_eq!(config.spawn_fail_limit(), 1);
        assert_eq!(
            config.run_retention().map(|spec| spec.seconds()),
            Some(604800)
        );
    }

    #[test]
    fn 上限がゼロの設定は受理されない() {
        assert_eq!(
            GlobalConfig::parse(GlobalConfigInput {
                judge_attempt_limit: Some(0),
                ..GlobalConfigInput::default()
            }),
            Err(GlobalConfigError::InvalidJudgeAttemptLimit { given: 0 })
        );
        assert_eq!(
            GlobalConfig::parse(GlobalConfigInput {
                spawn_fail_limit: Some(0),
                ..GlobalConfigInput::default()
            }),
            Err(GlobalConfigError::InvalidSpawnFailLimit { given: 0 })
        );
    }

    #[test]
    fn 壊れたテンプレートを含むエージェント定義も設定としては受理される() {
        let agent = AgentName::parse("broken".to_owned()).expect("受理される");
        let definition = RawAgentDefinition::new(
            RawCommand::parse_text("claude {unknown}").expect("受理される"),
            None,
        );
        let config = GlobalConfig::parse(GlobalConfigInput {
            agents: Some(BTreeMap::from([(agent.clone(), definition)])),
            ..GlobalConfigInput::default()
        })
        .expect("受理される");
        assert!(config.agent(&agent).is_some());
    }
}
