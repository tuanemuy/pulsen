//! ワークフロー定義ファイルを読む `WorkflowStore` の実装。

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use pulsen_domain::definition::{
    LoadedWorkflow, RawCommandDoc, RawStatusDoc, RawWorkflowDoc, WorkflowAssembler,
    WorkflowLoadError, WorkflowParseError, WorkflowRef, WorkflowStore,
};

use super::yaml::{self, CommandInput, Yaml};

/// 名前解決で使う拡張子。`.yml` へのフォールバックはしない。
const NAME_EXTENSION: &str = "yaml";

/// トップレベルに許されるキー(ADR-013)。
const TOP_LEVEL_KEYS: &[&str] = &["workflow", "agent", "model", "initial", "statuses"];

/// ステータスに書けるキー(ADR-013)。動作種別との整合は `WorkflowAssembler` が見る。
const STATUS_KEYS: &[&str] = &[
    "prompt", "skill", "run", "agent", "model", "timeout", "retries", "judge", "next",
];

/// 論理パスを持たない位置(ドキュメント自体)を指す表示。
const TOP_LEVEL: &str = "(トップレベル)";

/// ワークフロー定義ファイルを読み込むストア。
///
/// 名前は `<workflows_dir>/<name>.yaml` にのみ解決し、相対パスは注入された基準
/// ディレクトリから解決する(ADR-030)。カレントディレクトリというプロセス全体の
/// 可変状態を読まないため、複数のストアが同時に別の基準で動ける。
pub struct FsWorkflowStore {
    workflows_dir: PathBuf,
    base_dir: PathBuf,
}

impl FsWorkflowStore {
    /// 名前解決の基点となる `workflows/` と、相対パス解決の基準ディレクトリを受け取る。
    ///
    /// 合成ルートが `std::env::current_dir()` を `base_dir` に渡すことで、spec の
    /// 「相対パスはカレントディレクトリから解決される」契約になる。
    pub fn new(workflows_dir: PathBuf, base_dir: PathBuf) -> Self {
        Self {
            workflows_dir,
            base_dir,
        }
    }

    fn resolve(&self, wf_ref: &WorkflowRef) -> PathBuf {
        let path = match wf_ref {
            WorkflowRef::Name(name) => self
                .workflows_dir
                .join(format!("{}.{NAME_EXTENSION}", name.as_str())),
            WorkflowRef::Path(path) => path.clone(),
        };
        absolutize(&path, &self.base_dir)
    }
}

impl WorkflowStore for FsWorkflowStore {
    fn load(&self, wf_ref: &WorkflowRef) -> Result<LoadedWorkflow, WorkflowLoadError> {
        let resolved = self.resolve(wf_ref);
        let text = match fs::read_to_string(&resolved) {
            Ok(text) => text,
            Err(error) => return Err(read_error(&error, resolved)),
        };
        let document = yaml::parse_document(&text).map_err(|error| {
            WorkflowLoadError::Parse(WorkflowParseError::YamlSyntax {
                message: at(&resolved, &error.message),
                location: error.location,
            })
        })?;
        let doc = decode(&document).map_err(WorkflowLoadError::Parse)?;
        let parsed = WorkflowAssembler::assemble(doc).map_err(WorkflowLoadError::Parse)?;
        Ok(LoadedWorkflow::new(parsed, resolved))
    }
}

/// 基準ディレクトリで絶対化する。`.` の成分は取り除くが、カレントディレクトリは読まない。
fn absolutize(path: &Path, base_dir: &Path) -> PathBuf {
    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        base_dir.join(path)
    };
    std::path::absolute(&joined).unwrap_or(joined)
}

fn read_error(error: &io::Error, attempted: PathBuf) -> WorkflowLoadError {
    if error.kind() == io::ErrorKind::NotFound {
        return WorkflowLoadError::NotFound { attempted };
    }
    WorkflowLoadError::Io {
        message: at(&attempted, &error.to_string()),
    }
}

/// どのファイルの話かをメッセージに載せる(ADR-050)。
///
/// `WorkflowLoadError::Io` と `WorkflowParseError::YamlSyntax` はパスを持つフィールドを
/// 持たず、`NotFound` と違って CLI 側も解決先を知らないため、ここで前置する。
fn at(resolved: &Path, message: &str) -> String {
    format!("{}: {message}", resolved.display())
}

/// 構造を走査して未パースのドキュメントに落とす。値の検証は `WorkflowAssembler` が行う。
fn decode(document: &Yaml) -> Result<RawWorkflowDoc, WorkflowParseError> {
    let mapping = document
        .as_mapping_or_empty()
        .ok_or_else(|| type_error(TOP_LEVEL, "マッピング", document))?;
    if let Some(key) = mapping.unknown_key(TOP_LEVEL_KEYS) {
        return Err(WorkflowParseError::UnknownKey {
            location: TOP_LEVEL.to_owned(),
            key: key.to_owned(),
        });
    }

    Ok(RawWorkflowDoc {
        declared_name: text(mapping.value("workflow"), "workflow")?,
        default_agent: text(mapping.value("agent"), "agent")?,
        default_model: text(mapping.value("model"), "model")?,
        initial: text(mapping.value("initial"), "initial")?,
        statuses: match mapping.value("statuses") {
            Some(value) => decode_statuses(value)?,
            None => Vec::new(),
        },
    })
}

fn decode_statuses(value: &Yaml) -> Result<Vec<(String, RawStatusDoc)>, WorkflowParseError> {
    let mapping = value
        .as_mapping()
        .ok_or_else(|| type_error("statuses", "マッピング", value))?;

    let mut statuses = Vec::with_capacity(mapping.entries().len());
    for (name, entry) in mapping.entries() {
        statuses.push((name.clone(), decode_status(name, entry)?));
    }
    Ok(statuses)
}

fn decode_status(name: &str, value: &Yaml) -> Result<RawStatusDoc, WorkflowParseError> {
    let path = format!("statuses.{name}");
    let entry = value
        .as_mapping_or_empty()
        .ok_or_else(|| type_error(&path, "マッピング", value))?;
    if let Some(key) = entry.unknown_key(STATUS_KEYS) {
        return Err(WorkflowParseError::UnknownKey {
            location: path,
            key: key.to_owned(),
        });
    }

    Ok(RawStatusDoc {
        prompt: text(entry.value("prompt"), &format!("{path}.prompt"))?,
        skill: text(entry.value("skill"), &format!("{path}.skill"))?,
        run: text(entry.value("run"), &format!("{path}.run"))?,
        agent: text(entry.value("agent"), &format!("{path}.agent"))?,
        model: text(entry.value("model"), &format!("{path}.model"))?,
        timeout: text(entry.value("timeout"), &format!("{path}.timeout"))?,
        retries: count(entry.value("retries"), &format!("{path}.retries"))?,
        judge: command(entry.value("judge"), &format!("{path}.judge"))?,
        next: text(entry.value("next"), &format!("{path}.next"))?,
    })
}

fn text(value: Option<&Yaml>, path: &str) -> Result<Option<String>, WorkflowParseError> {
    match value {
        Some(value) => Ok(Some(
            value
                .as_text()
                .ok_or_else(|| type_error(path, "文字列", value))?
                .to_owned(),
        )),
        None => Ok(None),
    }
}

fn count(value: Option<&Yaml>, path: &str) -> Result<Option<u32>, WorkflowParseError> {
    match value {
        Some(value) => Ok(Some(
            value
                .as_u32()
                .ok_or_else(|| type_error(path, "0 以上の整数", value))?,
        )),
        None => Ok(None),
    }
}

fn command(value: Option<&Yaml>, path: &str) -> Result<Option<RawCommandDoc>, WorkflowParseError> {
    match value {
        Some(value) => {
            let input = value
                .as_command_input()
                .ok_or_else(|| type_error(path, "文字列または文字列の配列", value))?;
            Ok(Some(match input {
                CommandInput::Text(text) => RawCommandDoc::Text(text),
                CommandInput::Tokens(tokens) => RawCommandDoc::Tokens(tokens),
            }))
        }
        None => Ok(None),
    }
}

fn type_error(location: &str, expected: &str, value: &Yaml) -> WorkflowParseError {
    WorkflowParseError::InvalidValue {
        location: location.to_owned(),
        message: format!("{expected}である必要があります(実際は{})", value.kind()),
    }
}
