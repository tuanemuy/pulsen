//! ユーザーが記述する定義 — グローバル設定とワークフロー定義 — の構造・検証・
//! コマンドテンプレート展開を担うドメイン。値オブジェクト中心で、エンティティは存在しない。

mod agent;
mod assembler;
mod command;
mod config;
mod duration;
mod name;
mod reference;
mod snapshot;
mod template;
mod validator;
mod workflow;

pub use agent::{AGENT_CMD_PLACEHOLDERS, AgentDefError, AgentDefinition, RawAgentDefinition};
pub use assembler::{
    ParsedWorkflow, RawCommandDoc, RawStatusDoc, RawWorkflowDoc, SourceLocation, WorkflowAssembler,
    WorkflowParseError,
};
pub use command::{CommandError, PlainCommand, RawCommand};
pub use config::{GlobalConfig, GlobalConfigError, GlobalConfigInput};
pub use duration::{DurationError, DurationSpec, TimeoutSpec};
pub use name::{
    AgentName, InputText, ModelName, NameError, Prompt, SkillName, StatusName, WorkflowName,
};
pub use reference::{
    POSIX_SEPARATORS, WINDOWS_SEPARATORS, WORKFLOW_EXTENSIONS, WorkflowRef, platform_separators,
};
pub use snapshot::WorkflowSnapshot;
pub use template::{
    AgentInput, CommandLine, CommandTemplate, ExpansionError, Placeholder, PlaceholderValues,
    Segment, SkillInputTemplate, TemplateError, TemplateToken, UnknownPlaceholderName,
};
pub use validator::{RegistrationError, RegistrationValidator};
pub use workflow::{StatusDefinition, WorkflowDefinition, WorkflowStructureError};
