//! グローバルホームの config.yaml を読む `ConfigStore` の実装。

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use pulsen_domain::definition::{
    AgentName, ConfigLoadError, ConfigStore, DurationSpec, GlobalConfig, GlobalConfigError,
    GlobalConfigInput, PlainCommand, RawAgentDefinition, RawCommand,
};

use super::yaml::{self, CommandInput, Yaml};

/// トップレベルに許されるキー。
const TOP_LEVEL_KEYS: &[&str] = &[
    "agents",
    "notify_cmd",
    "judge_attempt_limit",
    "judge_timeout",
    "spawn_fail_limit",
    "run_retention",
];

/// `agents` のエントリに許されるキー。
const AGENT_KEYS: &[&str] = &["cmd", "skill_input"];

/// config.yaml を読み込むストア。
///
/// 検証は二層で、ここが担うのは構造(YAML 構文・キーの正当性・型・期間の形式)だけ。
/// テンプレートの内容は参照時の `RawAgentDefinition::parse` が検証する。
/// 読み込み結果をキャッシュしないため、`load` は常に呼び出し時点のファイル内容を返す。
pub struct FsConfigStore {
    config_path: PathBuf,
    home: PathBuf,
}

impl FsConfigStore {
    /// 読み込む config.yaml のパスと、未初期化の案内に使う解決後のホームパスを受け取る
    /// (レイアウトの導出はアプリケーション層)。
    pub fn new(config_path: PathBuf, home: PathBuf) -> Self {
        Self { config_path, home }
    }
}

impl ConfigStore for FsConfigStore {
    fn load(&self) -> Result<GlobalConfig, ConfigLoadError> {
        let text = match fs::read_to_string(&self.config_path) {
            Ok(text) => text,
            Err(error) => return Err(read_error(&error, &self.home)),
        };
        let document = yaml::parse_document(&text).map_err(|error| ConfigLoadError::Invalid {
            message: error.message,
            location: error.location,
        })?;
        decode(&document).map_err(|message| ConfigLoadError::Invalid {
            message,
            location: None,
        })
    }
}

fn read_error(error: &io::Error, home: &Path) -> ConfigLoadError {
    if error.kind() == io::ErrorKind::NotFound {
        return ConfigLoadError::NotFound {
            home: home.to_path_buf(),
        };
    }
    ConfigLoadError::Io {
        message: error.to_string(),
    }
}

/// 構造を走査してグローバル設定に落とす。空ドキュメントは全キー省略として扱う。
fn decode(document: &Yaml) -> Result<GlobalConfig, String> {
    let mapping = document
        .as_mapping_or_empty()
        .ok_or_else(|| type_error("", "マッピング", document))?;
    if let Some(key) = mapping.unknown_key(TOP_LEVEL_KEYS) {
        return Err(format!("スキーマに無いキーです: {key}"));
    }

    let input = GlobalConfigInput {
        agents: match mapping.value("agents") {
            Some(value) => Some(decode_agents(value)?),
            None => None,
        },
        notify_cmd: match mapping.value("notify_cmd") {
            Some(value) => Some(decode_plain_command(value, "notify_cmd")?),
            None => None,
        },
        judge_attempt_limit: decode_count(
            mapping.value("judge_attempt_limit"),
            "judge_attempt_limit",
        )?,
        judge_timeout: decode_duration(mapping.value("judge_timeout"), "judge_timeout")?,
        spawn_fail_limit: decode_count(mapping.value("spawn_fail_limit"), "spawn_fail_limit")?,
        run_retention: decode_duration(mapping.value("run_retention"), "run_retention")?,
    };
    GlobalConfig::parse(input).map_err(|error| describe_config_error(&error))
}

fn decode_agents(value: &Yaml) -> Result<BTreeMap<AgentName, RawAgentDefinition>, String> {
    let mapping = value
        .as_mapping()
        .ok_or_else(|| type_error("agents", "マッピング", value))?;

    let mut agents = BTreeMap::new();
    for (key, entry) in mapping.entries() {
        let path = format!("agents.{key}");
        let name = AgentName::parse(key.clone())
            .map_err(|error| at(&path, error.describe().to_owned()))?;
        agents.insert(name, decode_agent(&path, entry)?);
    }
    Ok(agents)
}

fn decode_agent(path: &str, value: &Yaml) -> Result<RawAgentDefinition, String> {
    let entry = value
        .as_mapping_or_empty()
        .ok_or_else(|| type_error(path, "マッピング", value))?;
    if let Some(key) = entry.unknown_key(AGENT_KEYS) {
        return Err(at(path, format!("スキーマに無いキーです: {key}")));
    }

    let cmd_path = format!("{path}.cmd");
    let cmd = match entry.value("cmd") {
        Some(value) => decode_raw_command(value, &cmd_path)?,
        None => return Err(at(path, "cmd は必須です".to_owned())),
    };
    let skill_input = match entry.value("skill_input") {
        Some(value) => {
            let path = format!("{path}.skill_input");
            Some(
                value
                    .as_text()
                    .ok_or_else(|| type_error(&path, "文字列", value))?
                    .to_owned(),
            )
        }
        None => None,
    };
    Ok(RawAgentDefinition::new(cmd, skill_input))
}

fn decode_raw_command(value: &Yaml, path: &str) -> Result<RawCommand, String> {
    let command = match command_input(value, path)? {
        CommandInput::Text(text) => RawCommand::parse_text(&text),
        CommandInput::Tokens(tokens) => RawCommand::parse_tokens(tokens),
    };
    command.map_err(|error| at(path, error.describe().to_owned()))
}

fn decode_plain_command(value: &Yaml, path: &str) -> Result<PlainCommand, String> {
    let command = match command_input(value, path)? {
        CommandInput::Text(text) => PlainCommand::parse_text(&text),
        CommandInput::Tokens(tokens) => PlainCommand::parse_tokens(tokens),
    };
    command.map_err(|error| at(path, error.describe().to_owned()))
}

fn command_input(value: &Yaml, path: &str) -> Result<CommandInput, String> {
    value
        .as_command_input()
        .ok_or_else(|| type_error(path, "文字列または文字列の配列", value))
}

fn decode_count(value: Option<&Yaml>, path: &str) -> Result<Option<u32>, String> {
    match value {
        Some(value) => Ok(Some(
            value
                .as_u32()
                .ok_or_else(|| type_error(path, "0 以上の整数", value))?,
        )),
        None => Ok(None),
    }
}

fn decode_duration(value: Option<&Yaml>, path: &str) -> Result<Option<DurationSpec>, String> {
    match value {
        Some(value) => {
            let text = value
                .as_text()
                .ok_or_else(|| type_error(path, "期間を表す文字列", value))?;
            let duration = DurationSpec::parse(text).map_err(|error| at(path, error.describe()))?;
            Ok(Some(duration))
        }
        None => Ok(None),
    }
}

/// 論理パスつきのメッセージ。トップレベルはパスを持たない。
fn at(path: &str, message: String) -> String {
    if path.is_empty() {
        return message;
    }
    format!("{path}: {message}")
}

fn type_error(path: &str, expected: &str, value: &Yaml) -> String {
    at(
        path,
        format!("{expected}である必要があります(実際は{})", value.kind()),
    )
}

fn describe_config_error(error: &GlobalConfigError) -> String {
    match error {
        GlobalConfigError::InvalidJudgeAttemptLimit { given } => {
            format!("judge_attempt_limit: 1 以上である必要があります(実際は{given})")
        }
        GlobalConfigError::InvalidSpawnFailLimit { given } => {
            format!("spawn_fail_limit: 1 以上である必要があります(実際は{given})")
        }
    }
}
