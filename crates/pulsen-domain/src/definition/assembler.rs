//! 構造化済み入力からワークフロー定義を組み立てる純粋なドメインサービス。

use std::collections::BTreeMap;

use super::command::{CommandError, PlainCommand};
use super::duration::{DurationError, TimeoutSpec};
use super::name::{AgentName, ModelName, NameError, Prompt, SkillName, StatusName, WorkflowName};
use super::template::AgentInput;
use super::workflow::{StatusDefinition, WorkflowDefinition, WorkflowStructureError};

/// YAML テキスト上の位置。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceLocation {
    /// 1 始まりの行。
    pub line: usize,
    /// 1 始まりの桁。
    pub column: usize,
}

/// 登録時パースエラー。
///
/// `YamlSyntax` / `UnknownKey` の2種は WorkflowStore アダプター(テキスト →
/// `RawWorkflowDoc` の変換時)が生成する。残りは `WorkflowAssembler` が生成する。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkflowParseError {
    /// YAML として不正(重複キーを含む)。
    YamlSyntax {
        /// パーサからのメッセージ。
        message: String,
        /// 位置が取れた場合のテキスト上の位置。
        location: Option<SourceLocation>,
    },
    /// スキーマに無いキー(ADR-013)。
    UnknownKey {
        /// 論理パス(例: `statuses.queued`)。
        location: String,
        /// 見つかったキー。
        key: String,
    },
    /// 動作種別に無関係なキー(ADR-013)。
    ForbiddenKey {
        /// 対象のステータス名。
        status: String,
        /// 書かれていたキー。
        key: String,
    },
    /// `initial` の欠落。
    MissingInitial,
    /// `initial` の参照先が無い。
    InitialNotFound {
        /// 参照された初期ステータス名。
        initial: String,
    },
    /// `statuses` が空・欠落。
    EmptyStatuses,
    /// 動作宣言が無い。
    NoAction {
        /// 対象のステータス名。
        status: String,
    },
    /// 動作宣言が複数ある。
    MultipleActions {
        /// 対象のステータス名。
        status: String,
        /// 併記されていたキー。
        keys: Vec<String>,
    },
    /// `run` の値が `cleanup` / `wait` 以外。
    UnknownRunValue {
        /// 対象のステータス名。
        status: String,
        /// 書かれていた値。
        value: String,
    },
    /// エージェント実行ステータスの `next` 欠落。
    MissingNext {
        /// 対象のステータス名。
        status: String,
    },
    /// `next` の参照先が無い。
    NextNotFound {
        /// 参照元のステータス名。
        status: String,
        /// 参照された遷移先。
        next: String,
    },
    /// 名前系・期間・コマンドの生成エラー。
    InvalidValue {
        /// 論理パス(例: `statuses.queued.timeout`)。
        location: String,
        /// 生成エラーの説明。
        message: String,
    },
}

/// `judge` が保持する未パースのコマンド入力。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RawCommandDoc {
    /// 文字列形式。
    Text(String),
    /// トークン配列形式。
    Tokens(Vec<String>),
}

/// ステータスの未パースな全キー。全キーを保持することで `ForbiddenKey` を検出できる。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RawStatusDoc {
    /// `prompt`。
    pub prompt: Option<String>,
    /// `skill`。
    pub skill: Option<String>,
    /// `run`。
    pub run: Option<String>,
    /// `agent`。
    pub agent: Option<String>,
    /// `model`。
    pub model: Option<String>,
    /// `timeout`。
    pub timeout: Option<String>,
    /// `retries`。
    pub retries: Option<u32>,
    /// `judge`。
    pub judge: Option<RawCommandDoc>,
    /// `next`。
    pub next: Option<String>,
}

/// ワークフロー定義の未パースな全キー。YAML 構文解析後のドメイン側入力。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RawWorkflowDoc {
    /// `workflow`。
    pub declared_name: Option<String>,
    /// トップレベルの `agent`。
    pub default_agent: Option<String>,
    /// トップレベルの `model`。
    pub default_model: Option<String>,
    /// `initial`。
    pub initial: Option<String>,
    /// `statuses`。宣言順を保つ。
    pub statuses: Vec<(String, RawStatusDoc)>,
}

/// 組み立て結果。表示名への採否は関知しない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedWorkflow {
    declared_name: Option<WorkflowName>,
    definition: WorkflowDefinition,
}

impl ParsedWorkflow {
    /// YAML の `workflow:` キーの値。
    pub fn declared_name(&self) -> Option<&WorkflowName> {
        self.declared_name.as_ref()
    }

    /// 組み立てられた定義。
    pub fn definition(&self) -> &WorkflowDefinition {
        &self.definition
    }

    /// 所有権ごと取り出す。
    pub fn into_parts(self) -> (Option<WorkflowName>, WorkflowDefinition) {
        (self.declared_name, self.definition)
    }
}

/// 厳格スキーマでワークフロー定義を組み立てる(純粋)。
pub struct WorkflowAssembler;

impl WorkflowAssembler {
    /// 構造化済み入力を検証して `ParsedWorkflow` にする。
    ///
    /// 前提条件: `doc.statuses` のキーは重複しない。ステータス名の重複は入力表現(YAML)の
    /// 重複キーであり、`WorkflowParseError` に重複用の種別は無く `YamlSyntax`(重複キーを
    /// 含む)としてアダプターが弾く分担になっている。
    pub fn assemble(doc: RawWorkflowDoc) -> Result<ParsedWorkflow, WorkflowParseError> {
        let declared_name = match doc.declared_name {
            Some(value) => {
                Some(WorkflowName::parse(value).map_err(|error| invalid_name("workflow", &error))?)
            }
            None => None,
        };
        let default_agent = match doc.default_agent {
            Some(value) => {
                Some(AgentName::parse(value).map_err(|error| invalid_name("agent", &error))?)
            }
            None => None,
        };
        let default_model = match doc.default_model {
            Some(value) => {
                Some(ModelName::parse(value).map_err(|error| invalid_name("model", &error))?)
            }
            None => None,
        };

        if doc.statuses.is_empty() {
            return Err(WorkflowParseError::EmptyStatuses);
        }

        let mut statuses = BTreeMap::new();
        for (raw_name, status_doc) in doc.statuses {
            let name = StatusName::parse(raw_name.clone())
                .map_err(|error| invalid_name(&format!("statuses.{raw_name}"), &error))?;
            let definition = assemble_status(&raw_name, status_doc)?;
            // 重複キーはアダプターが弾いている前提なので、後勝ちの畳み込みは起こらない。
            statuses.insert(name, definition);
        }

        let initial = match doc.initial {
            Some(value) => {
                StatusName::parse(value).map_err(|error| invalid_name("initial", &error))?
            }
            None => return Err(WorkflowParseError::MissingInitial),
        };

        let definition = WorkflowDefinition::new(default_agent, default_model, initial, statuses)
            .map_err(structure_error)?;
        Ok(ParsedWorkflow {
            declared_name,
            definition,
        })
    }
}

/// ステータスが宣言する動作。1つに定まったことを型で表す。
enum Action {
    Prompt(String),
    Skill(String),
    Run(String),
}

/// 動作宣言以外のキー。
struct StatusOptions {
    agent: Option<String>,
    model: Option<String>,
    timeout: Option<String>,
    retries: Option<u32>,
    judge: Option<RawCommandDoc>,
    next: Option<String>,
}

fn assemble_status(
    status: &str,
    doc: RawStatusDoc,
) -> Result<StatusDefinition, WorkflowParseError> {
    let RawStatusDoc {
        prompt,
        skill,
        run,
        agent,
        model,
        timeout,
        retries,
        judge,
        next,
    } = doc;

    let action = match (prompt, skill, run) {
        (None, None, None) => {
            return Err(WorkflowParseError::NoAction {
                status: status.to_owned(),
            });
        }
        (Some(prompt), None, None) => Action::Prompt(prompt),
        (None, Some(skill), None) => Action::Skill(skill),
        (None, None, Some(run)) => Action::Run(run),
        (prompt, skill, run) => {
            let mut keys = Vec::new();
            if prompt.is_some() {
                keys.push("prompt".to_owned());
            }
            if skill.is_some() {
                keys.push("skill".to_owned());
            }
            if run.is_some() {
                keys.push("run".to_owned());
            }
            return Err(WorkflowParseError::MultipleActions {
                status: status.to_owned(),
                keys,
            });
        }
    };

    let options = StatusOptions {
        agent,
        model,
        timeout,
        retries,
        judge,
        next,
    };

    match action {
        Action::Run(run) => assemble_run_status(status, &run, &options),
        Action::Prompt(prompt) => {
            let input = AgentInput::Prompt(
                Prompt::parse(prompt)
                    .map_err(|error| invalid_name(&format!("statuses.{status}.prompt"), &error))?,
            );
            assemble_agent_run_status(status, input, options)
        }
        Action::Skill(skill) => {
            let input = AgentInput::Skill(
                SkillName::parse(skill)
                    .map_err(|error| invalid_name(&format!("statuses.{status}.skill"), &error))?,
            );
            assemble_agent_run_status(status, input, options)
        }
    }
}

fn assemble_run_status(
    status: &str,
    run: &str,
    options: &StatusOptions,
) -> Result<StatusDefinition, WorkflowParseError> {
    let definition = match run {
        "wait" => StatusDefinition::Wait,
        "cleanup" => StatusDefinition::Cleanup,
        _ => {
            return Err(WorkflowParseError::UnknownRunValue {
                status: status.to_owned(),
                value: run.to_owned(),
            });
        }
    };

    // エージェント実行にのみ許されるキー。宣言順に検査する。
    for (key, is_present) in [
        ("agent", options.agent.is_some()),
        ("model", options.model.is_some()),
        ("timeout", options.timeout.is_some()),
        ("retries", options.retries.is_some()),
        ("judge", options.judge.is_some()),
        ("next", options.next.is_some()),
    ] {
        if is_present {
            return Err(WorkflowParseError::ForbiddenKey {
                status: status.to_owned(),
                key: key.to_owned(),
            });
        }
    }

    Ok(definition)
}

fn assemble_agent_run_status(
    status: &str,
    input: AgentInput,
    options: StatusOptions,
) -> Result<StatusDefinition, WorkflowParseError> {
    let agent = match options.agent {
        Some(value) => Some(
            AgentName::parse(value)
                .map_err(|error| invalid_name(&format!("statuses.{status}.agent"), &error))?,
        ),
        None => None,
    };
    let model = match options.model {
        Some(value) => Some(
            ModelName::parse(value)
                .map_err(|error| invalid_name(&format!("statuses.{status}.model"), &error))?,
        ),
        None => None,
    };
    let timeout = match options.timeout {
        Some(value) => Some(
            TimeoutSpec::parse(&value)
                .map_err(|error| invalid_duration(&format!("statuses.{status}.timeout"), &error))?,
        ),
        None => None,
    };
    let judge = match options.judge {
        Some(command) => Some(
            parse_plain_command(command)
                .map_err(|error| invalid_command(&format!("statuses.{status}.judge"), &error))?,
        ),
        None => None,
    };
    let next = match options.next {
        Some(value) => StatusName::parse(value)
            .map_err(|error| invalid_name(&format!("statuses.{status}.next"), &error))?,
        None => {
            return Err(WorkflowParseError::MissingNext {
                status: status.to_owned(),
            });
        }
    };

    Ok(StatusDefinition::AgentRun {
        input,
        agent,
        model,
        timeout,
        retries: options.retries,
        judge,
        next,
    })
}

fn parse_plain_command(doc: RawCommandDoc) -> Result<PlainCommand, CommandError> {
    match doc {
        RawCommandDoc::Text(text) => PlainCommand::parse_text(&text),
        RawCommandDoc::Tokens(tokens) => PlainCommand::parse_tokens(tokens),
    }
}

fn structure_error(error: WorkflowStructureError) -> WorkflowParseError {
    match error {
        WorkflowStructureError::EmptyStatuses => WorkflowParseError::EmptyStatuses,
        WorkflowStructureError::InitialNotFound { initial } => {
            WorkflowParseError::InitialNotFound {
                initial: initial.into_string(),
            }
        }
        WorkflowStructureError::NextNotFound { status, next } => WorkflowParseError::NextNotFound {
            status: status.into_string(),
            next: next.into_string(),
        },
    }
}

fn invalid_name(location: &str, error: &NameError) -> WorkflowParseError {
    WorkflowParseError::InvalidValue {
        location: location.to_owned(),
        message: error.describe().to_owned(),
    }
}

fn invalid_duration(location: &str, error: &DurationError) -> WorkflowParseError {
    WorkflowParseError::InvalidValue {
        location: location.to_owned(),
        message: error.describe(),
    }
}

fn invalid_command(location: &str, error: &CommandError) -> WorkflowParseError {
    WorkflowParseError::InvalidValue {
        location: location.to_owned(),
        message: error.describe().to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status_name(name: &str) -> StatusName {
        StatusName::parse(name.to_owned()).expect("受理される")
    }

    fn prompt_status(next: &str) -> RawStatusDoc {
        RawStatusDoc {
            prompt: Some("実装して".to_owned()),
            next: Some(next.to_owned()),
            ..RawStatusDoc::default()
        }
    }

    fn doc(statuses: Vec<(&str, RawStatusDoc)>) -> RawWorkflowDoc {
        let initial = statuses.first().expect("1件以上").0.to_owned();
        RawWorkflowDoc {
            initial: Some(initial),
            statuses: statuses
                .into_iter()
                .map(|(name, status)| (name.to_owned(), status))
                .collect(),
            ..RawWorkflowDoc::default()
        }
    }

    #[test]
    fn 有効な定義は正規化された定義に組み立てられる() {
        let parsed = WorkflowAssembler::assemble(RawWorkflowDoc {
            declared_name: Some("implement".to_owned()),
            default_agent: Some("claude".to_owned()),
            default_model: Some("sonnet".to_owned()),
            initial: Some("queued".to_owned()),
            statuses: vec![
                ("queued".to_owned(), prompt_status("waiting")),
                (
                    "waiting".to_owned(),
                    RawStatusDoc {
                        run: Some("wait".to_owned()),
                        ..RawStatusDoc::default()
                    },
                ),
                (
                    "done".to_owned(),
                    RawStatusDoc {
                        run: Some("cleanup".to_owned()),
                        ..RawStatusDoc::default()
                    },
                ),
            ],
        })
        .expect("受理される");

        assert_eq!(
            parsed.declared_name().map(WorkflowName::as_str),
            Some("implement")
        );
        assert_eq!(parsed.definition().initial(), &status_name("queued"));
        assert_eq!(parsed.definition().statuses().len(), 3);
        assert_eq!(
            parsed.definition().status(&status_name("waiting")),
            Some(&StatusDefinition::Wait)
        );
        assert_eq!(
            parsed.definition().status(&status_name("done")),
            Some(&StatusDefinition::Cleanup)
        );
    }

    #[test]
    fn エージェント実行の全キーが対応するドメイン型に落ちる() {
        let parsed = WorkflowAssembler::assemble(doc(vec![(
            "queued",
            RawStatusDoc {
                skill: Some("review".to_owned()),
                agent: Some("claude".to_owned()),
                model: Some("opus".to_owned()),
                timeout: Some("none".to_owned()),
                retries: Some(0),
                judge: Some(RawCommandDoc::Tokens(vec![
                    "check".to_owned(),
                    "{input}".to_owned(),
                ])),
                next: Some("queued".to_owned()),
                ..RawStatusDoc::default()
            },
        )]))
        .expect("受理される");

        let status = parsed
            .definition()
            .status(&status_name("queued"))
            .expect("存在する");
        match status {
            StatusDefinition::AgentRun {
                input,
                agent,
                model,
                timeout,
                retries,
                judge,
                next,
            } => {
                assert_eq!(
                    input,
                    &AgentInput::Skill(SkillName::parse("review".to_owned()).expect("受理される"))
                );
                assert_eq!(agent.as_ref().map(AgentName::as_str), Some("claude"));
                assert_eq!(model.as_ref().map(ModelName::as_str), Some("opus"));
                assert_eq!(timeout, &Some(TimeoutSpec::Unlimited));
                assert_eq!(retries, &Some(0));
                assert_eq!(
                    judge.as_ref().map(PlainCommand::tokens),
                    Some(["check".to_owned(), "{input}".to_owned()].as_slice())
                );
                assert_eq!(next, &status_name("queued"));
            }
            StatusDefinition::Wait | StatusDefinition::Cleanup => {
                panic!("エージェント実行として組み立てられる")
            }
        }
    }

    #[test]
    fn 動作種別に無関係なキーは書かれたキー名で拒否される() {
        let wait = || RawStatusDoc {
            run: Some("wait".to_owned()),
            ..RawStatusDoc::default()
        };

        for (key, status_doc) in [
            (
                "agent",
                RawStatusDoc {
                    agent: Some("claude".to_owned()),
                    ..wait()
                },
            ),
            (
                "model",
                RawStatusDoc {
                    model: Some("opus".to_owned()),
                    ..wait()
                },
            ),
            (
                "timeout",
                RawStatusDoc {
                    timeout: Some("30s".to_owned()),
                    ..wait()
                },
            ),
            (
                "retries",
                RawStatusDoc {
                    retries: Some(1),
                    ..wait()
                },
            ),
            (
                "judge",
                RawStatusDoc {
                    judge: Some(RawCommandDoc::Text("check".to_owned())),
                    ..wait()
                },
            ),
            (
                "next",
                RawStatusDoc {
                    next: Some("queued".to_owned()),
                    ..wait()
                },
            ),
        ] {
            assert_eq!(
                WorkflowAssembler::assemble(doc(vec![("queued", status_doc)])),
                Err(WorkflowParseError::ForbiddenKey {
                    status: "queued".to_owned(),
                    key: key.to_owned()
                }),
                "{key}"
            );
        }
    }

    #[test]
    fn クリーンアップでも無関係なキーは拒否される() {
        assert_eq!(
            WorkflowAssembler::assemble(doc(vec![(
                "done",
                RawStatusDoc {
                    run: Some("cleanup".to_owned()),
                    next: Some("done".to_owned()),
                    ..RawStatusDoc::default()
                }
            )])),
            Err(WorkflowParseError::ForbiddenKey {
                status: "done".to_owned(),
                key: "next".to_owned()
            })
        );
    }

    #[test]
    fn 初期ステータスの欠落は拒否される() {
        assert_eq!(
            WorkflowAssembler::assemble(RawWorkflowDoc {
                statuses: vec![("queued".to_owned(), prompt_status("queued"))],
                ..RawWorkflowDoc::default()
            }),
            Err(WorkflowParseError::MissingInitial)
        );
    }

    #[test]
    fn 初期ステータスの参照先がなければ拒否される() {
        assert_eq!(
            WorkflowAssembler::assemble(RawWorkflowDoc {
                initial: Some("missing".to_owned()),
                statuses: vec![("queued".to_owned(), prompt_status("queued"))],
                ..RawWorkflowDoc::default()
            }),
            Err(WorkflowParseError::InitialNotFound {
                initial: "missing".to_owned()
            })
        );
    }

    #[test]
    fn ステータスが空なら拒否される() {
        assert_eq!(
            WorkflowAssembler::assemble(RawWorkflowDoc {
                initial: Some("queued".to_owned()),
                ..RawWorkflowDoc::default()
            }),
            Err(WorkflowParseError::EmptyStatuses)
        );
    }

    #[test]
    fn 動作宣言のないステータスは拒否される() {
        assert_eq!(
            WorkflowAssembler::assemble(doc(vec![("queued", RawStatusDoc::default())])),
            Err(WorkflowParseError::NoAction {
                status: "queued".to_owned()
            })
        );
    }

    #[test]
    fn 動作宣言が複数あるステータスは拒否される() {
        assert_eq!(
            WorkflowAssembler::assemble(doc(vec![(
                "queued",
                RawStatusDoc {
                    prompt: Some("実装して".to_owned()),
                    skill: Some("review".to_owned()),
                    next: Some("queued".to_owned()),
                    ..RawStatusDoc::default()
                }
            )])),
            Err(WorkflowParseError::MultipleActions {
                status: "queued".to_owned(),
                keys: vec!["prompt".to_owned(), "skill".to_owned()]
            })
        );
        assert_eq!(
            WorkflowAssembler::assemble(doc(vec![(
                "queued",
                RawStatusDoc {
                    prompt: Some("実装して".to_owned()),
                    run: Some("wait".to_owned()),
                    ..RawStatusDoc::default()
                }
            )])),
            Err(WorkflowParseError::MultipleActions {
                status: "queued".to_owned(),
                keys: vec!["prompt".to_owned(), "run".to_owned()]
            })
        );
    }

    #[test]
    fn 未知のrun値は拒否される() {
        assert_eq!(
            WorkflowAssembler::assemble(doc(vec![(
                "queued",
                RawStatusDoc {
                    run: Some("sleep".to_owned()),
                    ..RawStatusDoc::default()
                }
            )])),
            Err(WorkflowParseError::UnknownRunValue {
                status: "queued".to_owned(),
                value: "sleep".to_owned()
            })
        );
    }

    #[test]
    fn エージェント実行のnext欠落は拒否される() {
        assert_eq!(
            WorkflowAssembler::assemble(doc(vec![(
                "queued",
                RawStatusDoc {
                    prompt: Some("実装して".to_owned()),
                    ..RawStatusDoc::default()
                }
            )])),
            Err(WorkflowParseError::MissingNext {
                status: "queued".to_owned()
            })
        );
    }

    #[test]
    fn nextの参照先がなければ拒否される() {
        assert_eq!(
            WorkflowAssembler::assemble(doc(vec![("queued", prompt_status("missing"))])),
            Err(WorkflowParseError::NextNotFound {
                status: "queued".to_owned(),
                next: "missing".to_owned()
            })
        );
    }

    #[test]
    fn 値の生成エラーは書かれた場所の論理位置つきで拒否される() {
        const SPACED: &str = "前後に空白を含められません";
        const EMPTY: &str = "空文字列は指定できません";

        let with_status = |status_doc: RawStatusDoc| doc(vec![("queued", status_doc)]);

        let cases = [
            (
                RawWorkflowDoc {
                    declared_name: Some(" implement".to_owned()),
                    ..with_status(prompt_status("queued"))
                },
                "workflow",
                SPACED,
            ),
            (
                RawWorkflowDoc {
                    default_agent: Some(" claude".to_owned()),
                    ..with_status(prompt_status("queued"))
                },
                "agent",
                SPACED,
            ),
            (
                RawWorkflowDoc {
                    default_model: Some(" opus".to_owned()),
                    ..with_status(prompt_status("queued"))
                },
                "model",
                SPACED,
            ),
            (
                RawWorkflowDoc {
                    initial: Some(" queued".to_owned()),
                    ..with_status(prompt_status("queued"))
                },
                "initial",
                SPACED,
            ),
            (
                RawWorkflowDoc {
                    initial: Some(" queued".to_owned()),
                    statuses: vec![(" queued".to_owned(), prompt_status(" queued"))],
                    ..RawWorkflowDoc::default()
                },
                "statuses. queued",
                SPACED,
            ),
            (
                with_status(RawStatusDoc {
                    prompt: Some(String::new()),
                    ..prompt_status("queued")
                }),
                "statuses.queued.prompt",
                EMPTY,
            ),
            (
                with_status(RawStatusDoc {
                    skill: Some(String::new()),
                    next: Some("queued".to_owned()),
                    ..RawStatusDoc::default()
                }),
                "statuses.queued.skill",
                EMPTY,
            ),
            (
                with_status(RawStatusDoc {
                    agent: Some(" claude".to_owned()),
                    ..prompt_status("queued")
                }),
                "statuses.queued.agent",
                SPACED,
            ),
            (
                with_status(RawStatusDoc {
                    model: Some(" opus".to_owned()),
                    ..prompt_status("queued")
                }),
                "statuses.queued.model",
                SPACED,
            ),
            (
                with_status(RawStatusDoc {
                    timeout: Some("0s".to_owned()),
                    ..prompt_status("queued")
                }),
                "statuses.queued.timeout",
                "期間に 0 は指定できません",
            ),
            (
                with_status(RawStatusDoc {
                    judge: Some(RawCommandDoc::Text(String::new())),
                    ..prompt_status("queued")
                }),
                "statuses.queued.judge",
                "コマンドが空です",
            ),
            (
                with_status(RawStatusDoc {
                    next: Some(" queued".to_owned()),
                    ..prompt_status("queued")
                }),
                "statuses.queued.next",
                SPACED,
            ),
        ];

        for (document, location, message) in cases {
            assert_eq!(
                WorkflowAssembler::assemble(document),
                Err(WorkflowParseError::InvalidValue {
                    location: location.to_owned(),
                    message: message.to_owned()
                }),
                "{location}"
            );
        }
    }

    #[test]
    fn 循環と自己参照と到達不能ステータスは受理される() {
        let parsed = WorkflowAssembler::assemble(doc(vec![
            ("a", prompt_status("b")),
            ("b", prompt_status("a")),
            ("self", prompt_status("self")),
            (
                "unreachable",
                RawStatusDoc {
                    run: Some("wait".to_owned()),
                    ..RawStatusDoc::default()
                },
            ),
        ]))
        .expect("受理される");
        assert_eq!(parsed.definition().statuses().len(), 4);
    }

    #[test]
    fn 終端のない定義も受理される() {
        let parsed = WorkflowAssembler::assemble(doc(vec![("only", prompt_status("only"))]))
            .expect("受理される");
        assert_eq!(parsed.definition().initial(), &status_name("only"));
    }

    #[test]
    fn 宣言名のない定義も受理される() {
        let parsed = WorkflowAssembler::assemble(doc(vec![("queued", prompt_status("queued"))]))
            .expect("受理される");
        assert_eq!(parsed.declared_name(), None);
    }
}
