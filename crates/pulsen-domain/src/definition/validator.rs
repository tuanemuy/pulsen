//! 登録時検証。ワークフロー定義とグローバル設定を突き合わせる純粋なドメインサービス。

use super::agent::AgentDefError;
use super::config::GlobalConfig;
use super::name::{AgentName, StatusName};
use super::snapshot::WorkflowSnapshot;
use super::template::{AgentInput, Placeholder};
use super::workflow::{StatusDefinition, WorkflowDefinition};

/// 登録時検証の失敗。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistrationError {
    /// 実効エージェントが解決できない。
    MissingAgent {
        /// 対象のステータス。
        status: StatusName,
    },
    /// 実効エージェントがグローバル設定に存在しない。
    UnknownAgent {
        /// 参照されたエージェント名。
        name: AgentName,
        /// グローバル設定に定義済みのエージェント名。
        defined: Vec<AgentName>,
    },
    /// エージェント定義のテンプレートが壊れている。
    InvalidAgentDefinition {
        /// 対象のエージェント名。
        agent: AgentName,
        /// 参照時検証のエラー。
        error: AgentDefError,
    },
    /// スキル入力を使うのにエージェントが `skill_input` を持たない。
    MissingSkillInput {
        /// 対象のステータス。
        status: StatusName,
        /// 対象のエージェント名。
        agent: AgentName,
    },
    /// `{model}` を参照するのに実効モデルが解決できない。
    MissingModel {
        /// 対象のステータス。
        status: StatusName,
        /// 対象のエージェント名。
        agent: AgentName,
    },
}

/// ワークフロー定義とグローバル設定の突き合わせ検証(純粋)。
pub struct RegistrationValidator;

impl RegistrationValidator {
    /// 全 `AgentRun` ステータスを検証する。エラーは最初の1件で打ち切らず全件返す。
    pub fn validate(
        definition: WorkflowDefinition,
        config: &GlobalConfig,
    ) -> Result<WorkflowSnapshot, Vec<RegistrationError>> {
        let mut errors = Vec::new();

        for (status, status_definition) in definition.statuses() {
            match status_definition {
                StatusDefinition::AgentRun { input, .. } => {
                    validate_agent_run(&definition, config, status, input, &mut errors);
                }
                StatusDefinition::Wait | StatusDefinition::Cleanup => {}
            }
        }

        if errors.is_empty() {
            Ok(WorkflowSnapshot::from_validated(definition))
        } else {
            Err(errors)
        }
    }
}

fn validate_agent_run(
    definition: &WorkflowDefinition,
    config: &GlobalConfig,
    status: &StatusName,
    input: &AgentInput,
    errors: &mut Vec<RegistrationError>,
) {
    let Some(agent_name) = definition.effective_agent(status) else {
        errors.push(RegistrationError::MissingAgent {
            status: status.clone(),
        });
        return;
    };
    let Some(raw_agent) = config.agent(agent_name) else {
        errors.push(RegistrationError::UnknownAgent {
            name: agent_name.clone(),
            defined: config.agents().keys().cloned().collect(),
        });
        return;
    };
    let agent = match raw_agent.parse() {
        Ok(agent) => agent,
        Err(error) => {
            errors.push(RegistrationError::InvalidAgentDefinition {
                agent: agent_name.clone(),
                error,
            });
            return;
        }
    };

    match input {
        AgentInput::Skill(_) => {
            if agent.skill_input().is_none() {
                errors.push(RegistrationError::MissingSkillInput {
                    status: status.clone(),
                    agent: agent_name.clone(),
                });
            }
        }
        AgentInput::Prompt(_) => {}
    }

    if agent.cmd().placeholders().contains(&Placeholder::Model)
        && definition.effective_model(status).is_none()
    {
        errors.push(RegistrationError::MissingModel {
            status: status.clone(),
            agent: agent_name.clone(),
        });
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::super::agent::RawAgentDefinition;
    use super::super::command::RawCommand;
    use super::super::config::GlobalConfigInput;
    use super::super::name::{ModelName, Prompt, SkillName};
    use super::super::template::TemplateError;
    use super::*;

    fn status(name: &str) -> StatusName {
        StatusName::parse(name.to_owned()).expect("受理される")
    }

    fn agent_name(name: &str) -> AgentName {
        AgentName::parse(name.to_owned()).expect("受理される")
    }

    fn agent_run(input: AgentInput, agent: Option<AgentName>) -> StatusDefinition {
        StatusDefinition::AgentRun {
            input,
            agent,
            model: None,
            timeout: None,
            retries: None,
            judge: None,
            next: status("queued"),
        }
    }

    fn prompt_input() -> AgentInput {
        AgentInput::Prompt(Prompt::parse("実装して".to_owned()).expect("受理される"))
    }

    fn skill_input() -> AgentInput {
        AgentInput::Skill(SkillName::parse("review".to_owned()).expect("受理される"))
    }

    fn definition(
        default_agent: Option<AgentName>,
        default_model: Option<ModelName>,
        statuses: Vec<(&str, StatusDefinition)>,
    ) -> WorkflowDefinition {
        WorkflowDefinition::new(
            default_agent,
            default_model,
            status("queued"),
            statuses
                .into_iter()
                .map(|(name, definition)| (status(name), definition))
                .collect(),
        )
        .expect("不変条件を満たす")
    }

    fn config(agents: Vec<(&str, &str, Option<&str>)>) -> GlobalConfig {
        let agents = agents
            .into_iter()
            .map(|(name, cmd, skill_input)| {
                (
                    agent_name(name),
                    RawAgentDefinition::new(
                        RawCommand::parse_text(cmd).expect("受理される"),
                        skill_input.map(str::to_owned),
                    ),
                )
            })
            .collect::<BTreeMap<_, _>>();
        GlobalConfig::parse(GlobalConfigInput {
            agents: Some(agents),
            ..GlobalConfigInput::default()
        })
        .expect("受理される")
    }

    #[test]
    fn 突き合わせに成功するとスナップショットが生成される() {
        let definition = definition(
            Some(agent_name("claude")),
            None,
            vec![
                ("queued", agent_run(prompt_input(), None)),
                ("waiting", StatusDefinition::Wait),
            ],
        );
        let snapshot = RegistrationValidator::validate(
            definition.clone(),
            &config(vec![("claude", "claude {input}", None)]),
        )
        .expect("受理される");
        assert_eq!(snapshot.definition(), &definition);
    }

    #[test]
    fn 実効エージェントが解決できないステータスは拒否される() {
        let definition = definition(
            None,
            None,
            vec![("queued", agent_run(prompt_input(), None))],
        );
        assert_eq!(
            RegistrationValidator::validate(definition, &config(vec![])),
            Err(vec![RegistrationError::MissingAgent {
                status: status("queued")
            }])
        );
    }

    #[test]
    fn 未定義のエージェントは定義済み一覧つきで拒否される() {
        let definition = definition(
            Some(agent_name("missing")),
            None,
            vec![("queued", agent_run(prompt_input(), None))],
        );
        assert_eq!(
            RegistrationValidator::validate(
                definition,
                &config(vec![
                    ("claude", "claude {input}", None),
                    ("shell", "sh -c {input}", None)
                ])
            ),
            Err(vec![RegistrationError::UnknownAgent {
                name: agent_name("missing"),
                defined: vec![agent_name("claude"), agent_name("shell")]
            }])
        );
    }

    #[test]
    fn 壊れたテンプレートを持つエージェントを参照すると拒否される() {
        let definition = definition(
            Some(agent_name("broken")),
            None,
            vec![("queued", agent_run(prompt_input(), None))],
        );
        assert_eq!(
            RegistrationValidator::validate(
                definition,
                &config(vec![("broken", "claude {unknown}", None)])
            ),
            Err(vec![RegistrationError::InvalidAgentDefinition {
                agent: agent_name("broken"),
                error: AgentDefError::InvalidCmd(TemplateError::UnknownPlaceholder {
                    token: "{unknown}".to_owned(),
                    name: "unknown".to_owned()
                })
            }])
        );
    }

    #[test]
    fn スキル入力に対応しないエージェントを参照すると拒否される() {
        let definition = definition(
            Some(agent_name("claude")),
            None,
            vec![("queued", agent_run(skill_input(), None))],
        );
        assert_eq!(
            RegistrationValidator::validate(
                definition,
                &config(vec![("claude", "claude {input}", None)])
            ),
            Err(vec![RegistrationError::MissingSkillInput {
                status: status("queued"),
                agent: agent_name("claude")
            }])
        );
    }

    #[test]
    fn モデルを参照するテンプレートに実効モデルがなければ拒否される() {
        let definition = definition(
            Some(agent_name("claude")),
            None,
            vec![("queued", agent_run(prompt_input(), None))],
        );
        assert_eq!(
            RegistrationValidator::validate(
                definition,
                &config(vec![("claude", "claude -m {model} {input}", None)])
            ),
            Err(vec![RegistrationError::MissingModel {
                status: status("queued"),
                agent: agent_name("claude")
            }])
        );
    }

    #[test]
    fn モデルを参照するテンプレートもワークフローデフォルトのモデルで解決できる() {
        let definition = definition(
            Some(agent_name("claude")),
            Some(ModelName::parse("sonnet".to_owned()).expect("受理される")),
            vec![("queued", agent_run(prompt_input(), None))],
        );
        assert!(
            RegistrationValidator::validate(
                definition,
                &config(vec![("claude", "claude -m {model} {input}", None)])
            )
            .is_ok()
        );
    }

    #[test]
    fn 検証エラーは最初の1件で打ち切らず全件返る() {
        let definition = definition(
            None,
            None,
            vec![
                ("queued", agent_run(prompt_input(), None)),
                (
                    "reviewing",
                    agent_run(skill_input(), Some(agent_name("missing"))),
                ),
                ("waiting", StatusDefinition::Wait),
            ],
        );
        let errors = RegistrationValidator::validate(
            definition,
            &config(vec![("claude", "claude {input}", None)]),
        )
        .expect_err("拒否される");
        assert_eq!(
            errors,
            vec![
                RegistrationError::MissingAgent {
                    status: status("queued")
                },
                RegistrationError::UnknownAgent {
                    name: agent_name("missing"),
                    defined: vec![agent_name("claude")]
                }
            ]
        );
    }

    #[test]
    fn 同一ステータスの複数の不足はまとめて返る() {
        let definition = definition(
            Some(agent_name("claude")),
            None,
            vec![("queued", agent_run(skill_input(), None))],
        );
        let errors = RegistrationValidator::validate(
            definition,
            &config(vec![("claude", "claude -m {model} {input}", None)]),
        )
        .expect_err("拒否される");
        assert_eq!(
            errors,
            vec![
                RegistrationError::MissingSkillInput {
                    status: status("queued"),
                    agent: agent_name("claude")
                },
                RegistrationError::MissingModel {
                    status: status("queued"),
                    agent: agent_name("claude")
                }
            ]
        );
    }

    #[test]
    fn 実行しないステータスはエージェント定義の欠落があっても受理される() {
        let definition = definition(
            None,
            None,
            vec![
                ("queued", StatusDefinition::Wait),
                ("done", StatusDefinition::Cleanup),
            ],
        );
        assert!(RegistrationValidator::validate(definition, &config(vec![])).is_ok());
    }
}
