//! エージェント定義。読み込み時は構造のみ、参照時に内容(テンプレート)を検証する。

use std::path::Path;

use super::command::RawCommand;
use super::name::{InputText, ModelName};
use super::template::{
    AgentInput, CommandLine, CommandTemplate, ExpansionError, Placeholder, PlaceholderValues,
    SkillInputTemplate, TemplateError,
};

/// `cmd` テンプレートで参照できるプレースホルダ。
pub const AGENT_CMD_PLACEHOLDERS: &[Placeholder] = &[
    Placeholder::Input,
    Placeholder::Model,
    Placeholder::Workspace,
];

/// エージェント定義の内容検証の失敗。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentDefError {
    /// `cmd` のテンプレート不備。
    InvalidCmd(TemplateError),
    /// `skill_input` のテンプレート不備。
    InvalidSkillInput(TemplateError),
    /// スキル入力が要るのに `skill_input` が未定義。
    MissingSkillInput,
}

/// 構造のみ検証済みのエージェント定義。グローバル設定が保持する形。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawAgentDefinition {
    cmd: RawCommand,
    skill_input: Option<String>,
}

impl RawAgentDefinition {
    /// 構造化済みの `cmd` と、未検証の `skill_input` テンプレートから組み立てる。
    pub fn new(cmd: RawCommand, skill_input: Option<String>) -> Self {
        Self { cmd, skill_input }
    }

    /// 構造化済みのコマンド。
    pub fn cmd(&self) -> &RawCommand {
        &self.cmd
    }

    /// 未検証の `skill_input` テンプレート。
    pub fn skill_input(&self) -> Option<&str> {
        self.skill_input.as_deref()
    }

    /// 参照時検証の境界。テンプレートの内容をここで一度だけ検証する。
    pub fn parse(&self) -> Result<AgentDefinition, AgentDefError> {
        let cmd = CommandTemplate::parse(&self.cmd, AGENT_CMD_PLACEHOLDERS)
            .map_err(AgentDefError::InvalidCmd)?;
        let skill_input = match &self.skill_input {
            Some(text) => {
                Some(SkillInputTemplate::parse(text).map_err(AgentDefError::InvalidSkillInput)?)
            }
            None => None,
        };
        Ok(AgentDefinition { cmd, skill_input })
    }
}

/// 内容まで検証済みのエージェント定義。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentDefinition {
    cmd: CommandTemplate,
    skill_input: Option<SkillInputTemplate>,
}

impl AgentDefinition {
    /// 検証済みのコマンドテンプレート。
    pub fn cmd(&self) -> &CommandTemplate {
        &self.cmd
    }

    /// 検証済みの `skill_input` テンプレート。
    pub fn skill_input(&self) -> Option<&SkillInputTemplate> {
        self.skill_input.as_ref()
    }

    /// ステータスの入力を `{input}` へ渡る値に変換する。
    pub fn render_input(&self, input: &AgentInput) -> Result<InputText, AgentDefError> {
        match input {
            AgentInput::Prompt(prompt) => Ok(InputText::new(prompt.as_str().to_owned())),
            AgentInput::Skill(skill) => match &self.skill_input {
                Some(template) => Ok(template.render(skill)),
                None => Err(AgentDefError::MissingSkillInput),
            },
        }
    }

    /// 展開値を組んでコマンドラインを作る。テンプレートが参照しない値は単に渡らない。
    pub fn build_command_line(
        &self,
        input: &InputText,
        model: Option<&ModelName>,
        workspace: &Path,
    ) -> Result<CommandLine, ExpansionError> {
        self.cmd.expand(&PlaceholderValues {
            input: Some(input.clone()),
            model: model.cloned(),
            workspace: Some(workspace.to_path_buf()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::name::{Prompt, SkillName};
    use super::*;

    fn raw(cmd: &str, skill_input: Option<&str>) -> RawAgentDefinition {
        RawAgentDefinition::new(
            RawCommand::parse_text(cmd).expect("コマンドとして受理される"),
            skill_input.map(str::to_owned),
        )
    }

    #[test]
    fn 壊れたテンプレートは読み込み時ではなく参照時に検出される() {
        let definition = raw("claude {unknown}", None);
        assert_eq!(
            definition.parse(),
            Err(AgentDefError::InvalidCmd(
                TemplateError::UnknownPlaceholder {
                    token: "{unknown}".to_owned(),
                    name: "unknown".to_owned()
                }
            ))
        );
    }

    #[test]
    fn スキル入力テンプレートの不備は参照時に検出される() {
        let definition = raw("claude {input}", Some("{input}"));
        assert_eq!(
            definition.parse(),
            Err(AgentDefError::InvalidSkillInput(
                TemplateError::UnknownPlaceholder {
                    token: "{input}".to_owned(),
                    name: "input".to_owned()
                }
            ))
        );
    }

    #[test]
    fn プロンプト入力はそのまま入力テキストになる() {
        let definition = raw("claude {input}", None).parse().expect("受理される");
        let rendered = definition
            .render_input(&AgentInput::Prompt(
                Prompt::parse("実装して".to_owned()).expect("受理される"),
            ))
            .expect("変換できる");
        assert_eq!(rendered.as_str(), "実装して");
    }

    #[test]
    fn スキル入力はスキル入力テンプレートで変換される() {
        let definition = raw("claude {input}", Some("/skill {skill}"))
            .parse()
            .expect("受理される");
        let rendered = definition
            .render_input(&AgentInput::Skill(
                SkillName::parse("review".to_owned()).expect("受理される"),
            ))
            .expect("変換できる");
        assert_eq!(rendered.as_str(), "/skill review");
    }

    #[test]
    fn スキル入力テンプレートがなければスキル入力は変換できない() {
        let definition = raw("claude {input}", None).parse().expect("受理される");
        assert_eq!(
            definition.render_input(&AgentInput::Skill(
                SkillName::parse("review".to_owned()).expect("受理される")
            )),
            Err(AgentDefError::MissingSkillInput)
        );
    }

    #[test]
    fn コマンドラインは入力とモデルとワークスペースを展開する() {
        let definition = raw("claude -m {model} --dir {workspace} {input}", None)
            .parse()
            .expect("受理される");
        let command_line = definition
            .build_command_line(
                &InputText::new("実装して".to_owned()),
                Some(&ModelName::parse("sonnet".to_owned()).expect("受理される")),
                Path::new("/tmp/wt"),
            )
            .expect("展開できる");
        assert_eq!(
            command_line.tokens(),
            ["claude", "-m", "sonnet", "--dir", "/tmp/wt", "実装して"]
        );
    }

    #[test]
    fn モデルを参照しないテンプレートにモデルを渡しても展開は成功する() {
        let definition = raw("claude {input}", None).parse().expect("受理される");
        let command_line = definition
            .build_command_line(
                &InputText::new("実装して".to_owned()),
                Some(&ModelName::parse("sonnet".to_owned()).expect("受理される")),
                Path::new("/tmp/wt"),
            )
            .expect("展開できる");
        assert_eq!(command_line.tokens(), ["claude", "実装して"]);
    }

    #[test]
    fn モデルを参照するテンプレートにモデルがなければ展開に失敗する() {
        let definition = raw("claude -m {model} {input}", None)
            .parse()
            .expect("受理される");
        assert_eq!(
            definition.build_command_line(
                &InputText::new("実装して".to_owned()),
                None,
                Path::new("/tmp/wt")
            ),
            Err(ExpansionError::MissingValue(Placeholder::Model))
        );
    }
}
