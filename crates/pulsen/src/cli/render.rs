//! 出力文言の組み立て。
//!
//! ユースケースは原因を値として返すだけで、利用者に見せる言葉はここで決める。
//! 出力は人間可読なテキストに限る — 機械可読形式は提供しない(pages 共通事項)。

use std::path::Path;

use pulsen_domain::definition::{
    AgentDefError, AgentName, ConfigLoadError, NameError, RegistrationError, SourceLocation,
    TemplateError, WorkflowLoadError, WorkflowParseError,
};
use pulsen_domain::execution::TargetError;
use pulsen_domain::task::{AbsolutePathError, BranchNameError, CreateError};

use crate::application::register_task::{RegisterTaskError, RegisteredTask};

use super::add::AddError;
use super::wire::WireError;

/// 登録の成功。タスクID・ワークフロー名・解決先を示す。
pub fn registered(registered: &RegisteredTask) -> String {
    let mut out = String::from("タスクを登録しました。\n");
    push_field(&mut out, "タスクID", registered.task_id.as_str());
    push_field(&mut out, "ワークフロー", registered.workflow_name.as_str());
    push_field(
        &mut out,
        "解決先",
        &registered.resolved_from.display().to_string(),
    );
    out.push_str("  次回の tick で実行されます。");
    out
}

/// `add` の失敗。
pub fn add_error(error: &AddError) -> String {
    match error {
        AddError::Wire(error) => wire_error(error),
        AddError::Register(error) => register_error(error),
    }
}

/// 結線そのものの失敗。
fn wire_error(error: &WireError) -> String {
    match error {
        WireError::HomeUnresolvable => problem(
            "グローバルホームを特定できません。",
            &["--home か環境変数 PULSEN_HOME でディレクトリを指定してください。".to_owned()],
        ),
        WireError::HomeUnusable { given, message } => problem(
            "指定されたグローバルホームを使えません。",
            &[
                format!("指定: {}", given.display()),
                format!("原因: {message}"),
            ],
        ),
        WireError::CurrentDirUnavailable { message } => problem(
            "カレントディレクトリを取得できません。",
            &[format!("原因: {message}")],
        ),
        WireError::RepoPathUnusable { given, message } => problem(
            "--repo のパスを解決できません。",
            &[
                format!("指定: {}", given.display()),
                format!("原因: {message}"),
            ],
        ),
        WireError::Config { config_path, error } => config_error(config_path, error),
        WireError::IdGenerator { message } => {
            problem("タスクIDを発行できません。", &[format!("原因: {message}")])
        }
    }
}

/// グローバル設定の読み込みの失敗(pages ※1)。
fn config_error(config_path: &Path, error: &ConfigLoadError) -> String {
    match error {
        ConfigLoadError::NotFound { home } => problem(
            "グローバルホームが未初期化です。",
            &[
                format!("グローバルホーム: {}", home.display()),
                format!(
                    "グローバル設定 {} を作成してください。",
                    config_path.display()
                ),
            ],
        ),
        ConfigLoadError::Invalid { message, location } => {
            let mut details = vec![
                format!("ファイル: {}", config_path.display()),
                format!("原因: {message}"),
            ];
            if let Some(location) = location {
                details.push(format!("位置: {}", source_location(*location)));
            }
            problem("グローバル設定を解釈できません。", &details)
        }
        ConfigLoadError::Io { message } => problem(
            "グローバル設定を読み込めません。",
            &[
                format!("ファイル: {}", config_path.display()),
                format!("原因: {message}"),
            ],
        ),
    }
}

/// 登録の失敗。
fn register_error(error: &RegisterTaskError) -> String {
    match error {
        RegisterTaskError::LockBusy => problem(
            "別の操作が実行中です。",
            &["時間をおいて再実行してください。タスクは作られていません。".to_owned()],
        ),
        RegisterTaskError::LockFailed { message } => {
            problem("排他ロックを扱えません。", &[format!("原因: {message}")])
        }
        RegisterTaskError::InvalidWorkflowRef(error) => problem(
            "--workflow の値が不正です。",
            &[format!("原因: {}", name_error(error))],
        ),
        RegisterTaskError::WorkflowLoad(error) => workflow_load_error(error),
        RegisterTaskError::UndecidableWorkflowName(error) => problem(
            "ワークフロー名を決められません。",
            &[
                format!("原因: ファイル名から決めた名前が{}", name_error(error)),
                "定義YAMLの workflow: キーで名前を明示してください。".to_owned(),
            ],
        ),
        RegisterTaskError::InvalidRepoPath(AbsolutePathError::NotAbsolute { given }) => problem(
            "--repo を絶対パスとして解決できません。",
            &[format!("指定: {}", given.display())],
        ),
        RegisterTaskError::InvalidBaseBranch(error) => problem(
            "--base の値がブランチ名として不正です。",
            &[format!("原因: {}", branch_name_error(error))],
        ),
        RegisterTaskError::Target(error) => target_error(error),
        RegisterTaskError::BaseBranchNotFound { branch } => problem(
            "指定したベースブランチがリポジトリに存在しません。",
            &[format!("ブランチ: {}", branch.as_str())],
        ),
        RegisterTaskError::Registration(errors) => registration_errors(errors),
        RegisterTaskError::Create(CreateError::Conflict) => problem(
            "タスクIDが再発行後も衝突しました。",
            &["時間をおいて再実行してください。タスクは作られていません。".to_owned()],
        ),
        RegisterTaskError::Create(CreateError::Io { message }) => problem(
            "タスクファイルを作成できません。",
            &[format!("原因: {message}")],
        ),
    }
}

/// ワークフロー定義の読み込みの失敗。
fn workflow_load_error(error: &WorkflowLoadError) -> String {
    match error {
        WorkflowLoadError::NotFound { attempted } => problem(
            "ワークフロー定義が見つかりません。",
            &[format!("解決を試みたパス: {}", attempted.display())],
        ),
        WorkflowLoadError::Io { message } => problem(
            "ワークフロー定義を読み込めません。",
            &[format!("原因: {message}")],
        ),
        WorkflowLoadError::Parse(error) => {
            problem("ワークフロー定義が不正です。", &workflow_parse_error(error))
        }
    }
}

/// 登録時パースエラーの内訳。
fn workflow_parse_error(error: &WorkflowParseError) -> Vec<String> {
    match error {
        WorkflowParseError::YamlSyntax { message, location } => {
            let mut details = vec![format!("YAML 構文エラー: {message}")];
            if let Some(location) = location {
                details.push(format!("位置: {}", source_location(*location)));
            }
            details
        }
        WorkflowParseError::UnknownKey { location, key } => {
            vec![format!(
                "{location} にスキーマ外のキー `{key}` があります。"
            )]
        }
        WorkflowParseError::ForbiddenKey { status, key } => vec![format!(
            "ステータス `{status}` の動作種別では使えないキー `{key}` があります。"
        )],
        WorkflowParseError::MissingInitial => {
            vec!["initial が指定されていません。".to_owned()]
        }
        WorkflowParseError::InitialNotFound { initial } => vec![format!(
            "initial が指すステータス `{initial}` が statuses にありません。"
        )],
        WorkflowParseError::EmptyStatuses => {
            vec!["statuses が空、または指定されていません。".to_owned()]
        }
        WorkflowParseError::NoAction { status } => vec![format!(
            "ステータス `{status}` に動作宣言(prompt / skill / run)がありません。"
        )],
        WorkflowParseError::MultipleActions { status, keys } => vec![format!(
            "ステータス `{status}` に動作宣言が複数あります: {}",
            keys.join(", ")
        )],
        WorkflowParseError::UnknownRunValue { status, value } => vec![format!(
            "ステータス `{status}` の run の値 `{value}` は cleanup / wait のいずれでもありません。"
        )],
        WorkflowParseError::MissingNext { status } => vec![format!(
            "エージェント実行のステータス `{status}` に next がありません。"
        )],
        WorkflowParseError::NextNotFound { status, next } => vec![format!(
            "ステータス `{status}` の next が指す `{next}` が statuses にありません。"
        )],
        WorkflowParseError::InvalidValue { location, message } => {
            vec![format!("{location} の値が不正です: {message}")]
        }
    }
}

/// 対象の検証の失敗。HEAD からブランチを決められない場合は `--base` の明示を案内する。
fn target_error(error: &TargetError) -> String {
    match error {
        TargetError::NotFound => problem("指定したリポジトリのパスが存在しません。", &[]),
        TargetError::NotARepository => {
            problem("指定したパスは git リポジトリではありません。", &[])
        }
        TargetError::DetachedHead => problem(
            "HEAD がブランチを指していません(detached HEAD)。",
            &["--base でベースブランチを明示してください。".to_owned()],
        ),
        TargetError::EmptyRepository => problem(
            "コミットのない空のリポジトリです。",
            &["--base でベースブランチを明示してください。".to_owned()],
        ),
        TargetError::Failed { message } => {
            problem("対象の検証に失敗しました。", &[format!("原因: {message}")])
        }
    }
}

/// 登録時検証のエラー。最初の1件で打ち切らず全件を並べる。
fn registration_errors(errors: &[RegistrationError]) -> String {
    let mut out = format!(
        "エラー: ワークフロー定義の検証に失敗しました({}件)。\n",
        errors.len()
    );
    for error in errors {
        for (index, line) in registration_error(error).into_iter().enumerate() {
            if index == 0 {
                out.push_str("  - ");
            } else {
                out.push_str("    ");
            }
            out.push_str(&line);
            out.push('\n');
        }
    }
    out.push_str("  タスクは作られていません。");
    out
}

/// 登録時検証エラー1件の説明(2行目以降は補足)。
fn registration_error(error: &RegistrationError) -> Vec<String> {
    match error {
        RegistrationError::MissingAgent { status } => vec![format!(
            "ステータス `{}`: エージェントが指定されていません(ステータスの agent かワークフローの agent が要ります)。",
            status.as_str()
        )],
        RegistrationError::UnknownAgent { name, defined } => vec![
            format!(
                "エージェント `{}` は config.yaml に定義されていません。",
                name.as_str()
            ),
            format!("定義済みのエージェント: {}", agent_names(defined)),
        ],
        RegistrationError::InvalidAgentDefinition { agent, error } => vec![format!(
            "エージェント `{}` の定義が不正です: {}",
            agent.as_str(),
            agent_def_error(error)
        )],
        RegistrationError::MissingSkillInput { status, agent } => vec![format!(
            "ステータス `{}` は skill を使いますが、エージェント `{}` に skill_input がありません。",
            status.as_str(),
            agent.as_str()
        )],
        RegistrationError::MissingModel { status, agent } => vec![format!(
            "エージェント `{}` の cmd が {{model}} を参照していますが、ステータス `{}` にもワークフローにも model の指定がありません。",
            agent.as_str(),
            status.as_str()
        )],
    }
}

/// config.yaml に定義済みのエージェント名の一覧。
fn agent_names(defined: &[AgentName]) -> String {
    if defined.is_empty() {
        return "(1つも定義されていません)".to_owned();
    }
    defined
        .iter()
        .map(AgentName::as_str)
        .collect::<Vec<_>>()
        .join(", ")
}

/// エージェント定義の内容検証の失敗。
fn agent_def_error(error: &AgentDefError) -> String {
    match error {
        AgentDefError::InvalidCmd(error) => format!("cmd の{}", template_error(error)),
        AgentDefError::InvalidSkillInput(error) => {
            format!("skill_input の{}", template_error(error))
        }
        AgentDefError::MissingSkillInput => "skill_input がありません".to_owned(),
    }
}

/// テンプレートの不備。
fn template_error(error: &TemplateError) -> String {
    match error {
        TemplateError::UnknownPlaceholder { token, name } => {
            format!("`{token}` は使えないプレースホルダ `{name}` を参照しています")
        }
        TemplateError::MalformedBrace { token } => {
            format!("`{token}` の波括弧が閉じていないか空です")
        }
    }
}

/// 名前系の生成エラー。
fn name_error(error: &NameError) -> String {
    match error {
        NameError::Empty => "空文字列です".to_owned(),
        NameError::SurroundingWhitespace => "前後に空白を含みます".to_owned(),
    }
}

/// ブランチ名の生成エラー。
fn branch_name_error(error: &BranchNameError) -> String {
    match error {
        BranchNameError::Empty => "空文字列です".to_owned(),
        BranchNameError::ContainsWhitespaceOrControl { char, position } => format!(
            "{}文字目に空白または制御文字({char:?})を含みます",
            position + 1
        ),
        BranchNameError::LeadingHyphen => "`-` で始まっています".to_owned(),
        BranchNameError::ContainsDotDot => "`..` を含みます".to_owned(),
        BranchNameError::LeadingSlash => "`/` で始まっています".to_owned(),
        BranchNameError::TrailingSlash => "`/` で終わっています".to_owned(),
        BranchNameError::LockSuffix => "`.lock` で終わっています".to_owned(),
    }
}

/// テキスト上の位置。
fn source_location(location: SourceLocation) -> String {
    format!("{}行{}列", location.line, location.column)
}

/// 見出し1行と字下げした詳細から成るエラー文言。
fn problem(headline: &str, details: &[String]) -> String {
    let mut out = format!("エラー: {headline}");
    for detail in details {
        out.push_str("\n  ");
        out.push_str(detail);
    }
    out
}

/// 成功表示の項目行。
fn push_field(out: &mut String, label: &str, value: &str) {
    out.push_str("  ");
    out.push_str(label);
    out.push_str(": ");
    out.push_str(value);
    out.push('\n');
}
