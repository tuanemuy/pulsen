//! 出力文言の組み立て。
//!
//! ユースケースは原因を値として返すだけで、利用者に見せる言葉はここで決める。
//! 出力は人間可読なテキストに限る — 機械可読形式は提供しない(pages 共通事項)。

use std::path::Path;

use pulsen_domain::definition::{
    AgentName, CommandError, ConfigLoadError, RegistrationError, SourceLocation, WorkflowLoadError,
    WorkflowParseError, WorkflowStructureError,
};
use pulsen_domain::execution::{InconsistentRunFiles, RunFileError, TargetError};
use pulsen_domain::task::{
    AbsolutePathError, AttemptNumber, CreateError, ExecutionStateKind, RunDirPath, SaveError,
    TaskId, TransitionError,
};

use crate::application::register_task::{RegisterTaskError, RegisteredTask};
use crate::application::tick::{TickError, TickIssue, TickSummary};

use super::add::AddError;
use super::tick::TickCommandError;
use super::wire::WireError;
use super::wrapper::WrapperError;

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

/// ロック競合でスキップしたこと。cron 運用でアラートにしないため 0 で終わる。
pub fn tick_skipped() -> String {
    "別の操作が実行中のため、今回の tick はスキップしました。".to_owned()
}

/// tick の結果のサマリー。値の入っている項目だけを並べる。
pub fn tick_summary(summary: &TickSummary) -> String {
    if summary.is_empty() {
        return "処理対象のタスクはありませんでした。".to_owned();
    }

    let mut out = String::from("tick を実行しました。\n");
    push_ids(&mut out, "起動", &summary.launched);
    push_ids(&mut out, "起動確認", &summary.confirmed_running);
    push_ids(&mut out, "遷移", &summary.transitioned);
    push_ids(&mut out, "実行待ちへ復帰", &summary.skipped_back);
    push_ids(&mut out, "凍結", &summary.frozen);
    push_ids(&mut out, "通知", &summary.notified);
    push_ids(&mut out, "終端処理", &summary.archived);
    push_attempts(&mut out, "gcで削除", &summary.gc_deleted);
    push_attempts(&mut out, "gcで削除できず", &summary.gc_errors);

    let mut recorded = Vec::new();
    let mut unsettled = Vec::new();
    let mut skipped = Vec::new();
    for issue in &summary.errors {
        match issue_outcome(issue) {
            IssueOutcome::Recorded => recorded.push(issue),
            IssueOutcome::LaunchUnsettled => unsettled.push(issue),
            IssueOutcome::Skipped => skipped.push(issue),
        }
    }
    push_issues(&mut out, "失敗を記録", &recorded);
    push_issues(&mut out, "起動の結果が未確定", &unsettled);
    push_issues(&mut out, "スキップ", &skipped);

    out.trim_end().to_owned()
}

/// 報告がタスクファイルに何を残したか。運用者が次に取る行動はこれで分かれる。
enum IssueOutcome {
    /// 失敗を記録した。カウンタを消費し、上限を超えれば同じ tick で凍結する。
    Recorded,
    /// launching の記録は保存済みで、次の tick が猶予経路で分類する。
    LaunchUnsettled,
    /// タスクファイルへの書き込みが無く、次の tick がそのまま再試行する。
    Skipped,
}

/// 報告の結末の分類。
///
/// 網羅 `match` に置き、分類が増えたときに振り分け先を決めないと通らないようにする。
fn issue_outcome(issue: &TickIssue) -> IssueOutcome {
    match issue {
        TickIssue::WorktreeCreateFailed { .. }
        | TickIssue::CommandExpansionFailed { .. }
        | TickIssue::SpawnNotObserved { .. } => IssueOutcome::Recorded,
        TickIssue::PrepareAttemptFailed { .. } | TickIssue::SpawnFailed { .. } => {
            IssueOutcome::LaunchUnsettled
        }
        TickIssue::CorruptTaskFile { .. }
        | TickIssue::SnapshotUnreadable { .. }
        | TickIssue::MissingCurrentAttempt { .. }
        | TickIssue::Transition { .. }
        | TickIssue::RunFileUnreadable { .. }
        | TickIssue::InconsistentRunFiles { .. }
        | TickIssue::MarkerWriteFailed { .. }
        | TickIssue::SaveFailed { .. } => IssueOutcome::Skipped,
    }
}

/// 見出しと件数を伴う報告の一覧。空なら見出しごと出さない。
fn push_issues(out: &mut String, label: &str, issues: &[&TickIssue]) {
    if issues.is_empty() {
        return;
    }
    out.push_str(&format!("  {label}({}件):\n", issues.len()));
    for issue in issues {
        out.push_str("    - ");
        out.push_str(&tick_issue(issue));
        out.push('\n');
    }
}

/// `tick` の失敗。
pub fn tick_error(error: &TickCommandError) -> String {
    match error {
        TickCommandError::Wire(error) => wire_error(error),
        TickCommandError::Tick(TickError::LockFailed { message }) => {
            problem("排他ロックを扱えません。", &[format!("原因: {message}")])
        }
        TickCommandError::Tick(TickError::Scan { message }) => problem(
            "タスクを走査できません。",
            &[
                format!("原因: {message}"),
                "状態は変更していません。".to_owned(),
            ],
        ),
    }
}

/// スキップしたタスク1件の理由。タスクID(または対象のパス)と原因が読み取れる形にする。
fn tick_issue(issue: &TickIssue) -> String {
    match issue {
        TickIssue::CorruptTaskFile { path, message } => {
            format!("{}: タスクファイルを読めません({message})", path.display())
        }
        TickIssue::SnapshotUnreadable { task_id, message } => format!(
            "{}: 埋め込まれたワークフロー定義を読めません({message})",
            task_id.as_str()
        ),
        TickIssue::MissingCurrentAttempt { task_id } => format!(
            "{}: 起動記録済みですが現在 attempt がありません(タスクファイルの修復が必要です)",
            task_id.as_str()
        ),
        TickIssue::Transition { task_id, error } => format!(
            "{}: 遷移の前提が成立しません({})",
            task_id.as_str(),
            transition_error(error)
        ),
        TickIssue::RunFileUnreadable { task_id, error } => format!(
            "{}: runディレクトリのファイルを読めません({})",
            task_id.as_str(),
            run_file_error(error)
        ),
        TickIssue::InconsistentRunFiles { task_id, kind } => {
            format!("{}: {}", task_id.as_str(), inconsistent_run_files(kind))
        }
        TickIssue::WorktreeCreateFailed { task_id, message } => {
            format!("{}: worktree を作成できません({message})", task_id.as_str())
        }
        TickIssue::CommandExpansionFailed { task_id, message } => format!(
            "{}: 起動コマンドを組み立てられません({message})",
            task_id.as_str()
        ),
        TickIssue::MarkerWriteFailed { task_id, message } => format!(
            "{}: 無効化マーカーを書けません({message})",
            task_id.as_str()
        ),
        TickIssue::PrepareAttemptFailed { task_id, message } => format!(
            "{}: attempt の runディレクトリを用意できません({message})",
            task_id.as_str()
        ),
        TickIssue::SpawnFailed { task_id, message } => {
            format!("{}: ラッパーを起動できません({message})", task_id.as_str())
        }
        TickIssue::SpawnNotObserved { task_id, message } => format!(
            "{}: 起動を確認できず spawn 失敗として記録しました({message})",
            task_id.as_str()
        ),
        TickIssue::SaveFailed { task_id, error } => format!(
            "{}: タスクファイルを保存できません({})",
            task_id.as_str(),
            save_error(error)
        ),
    }
}

/// 遷移の前提の破れ。修復は人間に委ねるため、破れた前提そのものを示す。
fn transition_error(error: &TransitionError) -> String {
    match error {
        TransitionError::InvalidState { expected, actual } => format!(
            "実行状態が {} ではなく {}",
            expected
                .iter()
                .map(ExecutionStateKind::as_str)
                .collect::<Vec<_>>()
                .join(" | "),
            actual.as_str()
        ),
        TransitionError::WorkspaceAlreadySet => "ワークスペースが確定済み".to_owned(),
        TransitionError::WorkspaceNotSet => "ワークスペースが未確定".to_owned(),
        TransitionError::NotAgentRunStatus { status } => format!(
            "ステータス `{}` はエージェント実行ではない",
            status.as_str()
        ),
        TransitionError::MissingCurrentAttempt => {
            "起動記録済みなのに現在 attempt が無い".to_owned()
        }
    }
}

/// ラッパーの書き込み順序の破れ。次の tick が再観測するので、修復の指示は添えない。
fn inconsistent_run_files(kind: &InconsistentRunFiles) -> String {
    match kind {
        InconsistentRunFiles::MissingStartTime => {
            "pid ファイルがあるのに starttime ファイルがありません(ラッパーは starttime を先に書く)"
                .to_owned()
        }
    }
}

/// run ファイルの読み取りの失敗。
fn run_file_error(error: &RunFileError) -> String {
    match error {
        RunFileError::Corrupt { path, message } => {
            format!("{} を解釈できない: {message}", path.display())
        }
        RunFileError::Io { message } => message.clone(),
    }
}

/// タスクファイルの保存の失敗。
fn save_error(error: &SaveError) -> String {
    match error {
        SaveError::NotFound => "現役のタスクとして存在しない".to_owned(),
        SaveError::Io { message } => message.clone(),
    }
}

/// タスクIDの並ぶ項目行。空なら行ごと出さない。
fn push_ids(out: &mut String, label: &str, ids: &[TaskId]) {
    if ids.is_empty() {
        return;
    }
    let ids = ids
        .iter()
        .map(TaskId::as_str)
        .collect::<Vec<_>>()
        .join(", ");
    push_field(out, label, &ids);
}

/// attempt の並ぶ項目行。空なら行ごと出さない。
fn push_attempts(out: &mut String, label: &str, attempts: &[(String, AttemptNumber)]) {
    if attempts.is_empty() {
        return;
    }
    let attempts = attempts
        .iter()
        .map(|(dir, number)| format!("{dir}/{}", RunDirPath::attempt_dir_name(*number)))
        .collect::<Vec<_>>()
        .join(", ");
    push_field(out, label, &attempts);
}

/// `wrapper` の失敗。
///
/// 内部コマンドなので利用者への案内ではないが、run ディレクトリに証跡が残らない失敗
/// でもあるため、原因は標準エラー出力に書く。
pub fn wrapper_error(error: &WrapperError) -> String {
    match error {
        WrapperError::InvalidRunDir(AbsolutePathError::NotAbsolute { given }) => problem(
            "--run-dir を絶対パスとして解決できません。",
            &[format!("指定: {}", given.display())],
        ),
        WrapperError::InvalidWorkspace(AbsolutePathError::NotAbsolute { given }) => problem(
            "--workspace を絶対パスとして解決できません。",
            &[format!("指定: {}", given.display())],
        ),
        WrapperError::InvalidAgentCommand(error) => problem(
            "エージェントのコマンドが不正です。",
            &[format!("原因: {}", CommandError::describe(error))],
        ),
        WrapperError::Wire(error) => wire_error(error),
        WrapperError::NothingRecorded { run_dir } => problem(
            "同定情報を記録できませんでした。",
            &[format!("runディレクトリ: {}", run_dir.display())],
        ),
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
        WireError::SelfExeUnavailable { message } => problem(
            "自身の実行ファイルのパスを取得できません。",
            &[format!("原因: {message}")],
        ),
        WireError::RunDirUnusable { given } => problem(
            "runディレクトリが規定の形ではありません。",
            &[format!("指定: {}", given.display())],
        ),
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
            &[format!("原因: {}", error.describe())],
        ),
        RegisterTaskError::WorkflowLoad(error) => workflow_load_error(error),
        RegisterTaskError::UndecidableWorkflowName(error) => problem(
            "ワークフロー名を決められません。",
            &[
                format!(
                    "原因: ファイル名から決めた名前が使えません({})",
                    error.describe()
                ),
                "定義YAMLの workflow: キーで名前を明示してください。".to_owned(),
            ],
        ),
        RegisterTaskError::InvalidRepoPath(AbsolutePathError::NotAbsolute { given }) => problem(
            "--repo を絶対パスとして解決できません。",
            &[format!("指定: {}", given.display())],
        ),
        RegisterTaskError::InvalidBaseBranch(error) => problem(
            "--base の値がブランチ名として不正です。",
            &[format!("原因: {}", error.describe())],
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
            "{}。",
            WorkflowStructureError::describe_initial_not_found(initial)
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
            "{}。",
            WorkflowStructureError::describe_next_not_found(status, next)
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
            error.describe()
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use pulsen_domain::definition::{NameError, StatusName, WorkflowName};
    use pulsen_domain::task::{BranchName, BranchNameError, TaskId};

    use super::*;

    fn wire(error: WireError) -> String {
        add_error(&AddError::Wire(error))
    }

    fn register(error: RegisterTaskError) -> String {
        add_error(&AddError::Register(error))
    }

    fn task(id: &str) -> TaskId {
        TaskId::parse(id.to_owned()).expect("受理される")
    }

    #[test]
    fn ホームを特定できない場合は指定方法が案内される() {
        assert_eq!(
            wire(WireError::HomeUnresolvable),
            "エラー: グローバルホームを特定できません。\n  \
             --home か環境変数 PULSEN_HOME でディレクトリを指定してください。"
        );
    }

    #[test]
    fn 使えないホームは指定された値と原因つきで案内される() {
        assert_eq!(
            wire(WireError::HomeUnusable {
                given: PathBuf::from("relative/home"),
                message: "絶対パスにならない".to_owned(),
            }),
            "エラー: 指定されたグローバルホームを使えません。\n  \
             指定: relative/home\n  \
             原因: 絶対パスにならない"
        );
    }

    #[test]
    fn カレントディレクトリを取得できない場合は原因が案内される() {
        assert_eq!(
            wire(WireError::CurrentDirUnavailable {
                message: "権限がありません".to_owned(),
            }),
            "エラー: カレントディレクトリを取得できません。\n  原因: 権限がありません"
        );
    }

    #[test]
    fn 解決できないリポジトリパスは指定された値と原因つきで案内される() {
        assert_eq!(
            wire(WireError::RepoPathUnusable {
                given: PathBuf::from("repo"),
                message: "カレントディレクトリを読めない".to_owned(),
            }),
            "エラー: --repo のパスを解決できません。\n  \
             指定: repo\n  \
             原因: カレントディレクトリを読めない"
        );
    }

    #[test]
    fn 未初期化のホームは解決後のパスと作成すべき設定ファイルつきで案内される() {
        assert_eq!(
            wire(WireError::Config {
                config_path: PathBuf::from("/home/u/.pulsen/config.yaml"),
                error: ConfigLoadError::NotFound {
                    home: PathBuf::from("/home/u/.pulsen"),
                },
            }),
            "エラー: グローバルホームが未初期化です。\n  \
             グローバルホーム: /home/u/.pulsen\n  \
             グローバル設定 /home/u/.pulsen/config.yaml を作成してください。"
        );
    }

    #[test]
    fn 解釈できないグローバル設定は位置つきで案内される() {
        assert_eq!(
            wire(WireError::Config {
                config_path: PathBuf::from("/home/u/.pulsen/config.yaml"),
                error: ConfigLoadError::Invalid {
                    message: "キーが文字列ではありません".to_owned(),
                    location: Some(SourceLocation { line: 3, column: 5 }),
                },
            }),
            "エラー: グローバル設定を解釈できません。\n  \
             ファイル: /home/u/.pulsen/config.yaml\n  \
             原因: キーが文字列ではありません\n  \
             位置: 3行5列"
        );
    }

    #[test]
    fn 読み込めないグローバル設定はファイルと原因つきで案内される() {
        assert_eq!(
            wire(WireError::Config {
                config_path: PathBuf::from("/home/u/.pulsen/config.yaml"),
                error: ConfigLoadError::Io {
                    message: "権限がありません".to_owned(),
                },
            }),
            "エラー: グローバル設定を読み込めません。\n  \
             ファイル: /home/u/.pulsen/config.yaml\n  \
             原因: 権限がありません"
        );
    }

    #[test]
    fn タスクidを発行できない場合は原因が案内される() {
        assert_eq!(
            wire(WireError::IdGenerator {
                message: "乱数を取得できない: entropy source unavailable".to_owned(),
            }),
            "エラー: タスクIDを発行できません。\n  \
             原因: 乱数を取得できない: entropy source unavailable"
        );
    }

    #[test]
    fn ロック機構の異常は原因つきで案内される() {
        assert_eq!(
            register(RegisterTaskError::LockFailed {
                message: "ロックファイルを開けない".to_owned(),
            }),
            "エラー: 排他ロックを扱えません。\n  原因: ロックファイルを開けない"
        );
    }

    #[test]
    fn 対象の分類を確定できない場合は原因つきで案内される() {
        assert_eq!(
            register(RegisterTaskError::Target(TargetError::Failed {
                message: "git を起動できない".to_owned(),
            })),
            "エラー: 対象の検証に失敗しました。\n  原因: git を起動できない"
        );
    }

    #[test]
    fn 相対パスのリポジトリ指定は与えられた値つきで案内される() {
        assert_eq!(
            register(RegisterTaskError::InvalidRepoPath(
                AbsolutePathError::NotAbsolute {
                    given: PathBuf::from("repo"),
                }
            )),
            "エラー: --repo を絶対パスとして解決できません。\n  指定: repo"
        );
    }

    #[test]
    fn タスクidの再衝突は再実行の案内とともにタスク不作成を伝える() {
        assert_eq!(
            register(RegisterTaskError::Create(CreateError::Conflict)),
            "エラー: タスクIDが再発行後も衝突しました。\n  \
             時間をおいて再実行してください。タスクは作られていません。"
        );
    }

    #[test]
    fn タスクファイルを作成できない場合は原因が案内される() {
        assert_eq!(
            register(RegisterTaskError::Create(CreateError::Io {
                message: "書き込めません".to_owned(),
            })),
            "エラー: タスクファイルを作成できません。\n  原因: 書き込めません"
        );
    }

    #[test]
    fn 名前の制約の説明はドメインの言葉がそのまま出る() {
        assert_eq!(
            register(RegisterTaskError::InvalidWorkflowRef(NameError::Empty)),
            format!(
                "エラー: --workflow の値が不正です。\n  原因: {}",
                NameError::Empty.describe()
            )
        );
        assert_eq!(
            register(RegisterTaskError::UndecidableWorkflowName(
                NameError::SurroundingWhitespace
            )),
            format!(
                "エラー: ワークフロー名を決められません。\n  \
                 原因: ファイル名から決めた名前が使えません({})\n  \
                 定義YAMLの workflow: キーで名前を明示してください。",
                NameError::SurroundingWhitespace.describe()
            )
        );
        assert_eq!(
            register(RegisterTaskError::InvalidBaseBranch(
                BranchNameError::LeadingHyphen
            )),
            format!(
                "エラー: --base の値がブランチ名として不正です。\n  原因: {}",
                BranchNameError::LeadingHyphen.describe()
            )
        );
    }

    #[test]
    fn 登録時検証のエラーは全件が並びタスク不作成を伝える() {
        let text = register(RegisterTaskError::Registration(vec![
            RegistrationError::UnknownAgent {
                name: AgentName::parse("missing".to_owned()).expect("受理される"),
                defined: vec![AgentName::parse("claude".to_owned()).expect("受理される")],
            },
            RegistrationError::MissingAgent {
                status: StatusName::parse("queued".to_owned()).expect("受理される"),
            },
        ]));

        assert_eq!(
            text,
            "エラー: ワークフロー定義の検証に失敗しました(2件)。\n  \
             - エージェント `missing` は config.yaml に定義されていません。\n    \
             定義済みのエージェント: claude\n  \
             - ステータス `queued`: エージェントが指定されていません(ステータスの agent \
             かワークフローの agent が要ります)。\n  \
             タスクは作られていません。"
        );
    }

    #[test]
    fn 登録の成功はタスクidとワークフロー名と解決先を示す() {
        assert_eq!(
            registered(&RegisteredTask {
                task_id: TaskId::parse("20260812t101112-abcd1234".to_owned()).expect("受理される"),
                workflow_name: WorkflowName::parse("implement".to_owned()).expect("受理される"),
                resolved_from: PathBuf::from("/home/u/.pulsen/workflows/implement.yaml"),
            }),
            "タスクを登録しました。\n  \
             タスクID: 20260812t101112-abcd1234\n  \
             ワークフロー: implement\n  \
             解決先: /home/u/.pulsen/workflows/implement.yaml\n  \
             次回の tick で実行されます。"
        );
    }

    #[test]
    fn 規定の形でないrunディレクトリは指定された値つきで案内される() {
        assert_eq!(
            wrapper_error(&WrapperError::Wire(WireError::RunDirUnusable {
                given: PathBuf::from("/home/u/.pulsen/state/runs/t/attempt-01"),
            })),
            "エラー: runディレクトリが規定の形ではありません。\n  \
             指定: /home/u/.pulsen/state/runs/t/attempt-01"
        );
    }

    #[test]
    fn 記録できなかったラッパーはrunディレクトリを示す() {
        assert_eq!(
            wrapper_error(&WrapperError::NothingRecorded {
                run_dir: PathBuf::from("/home/u/.pulsen/state/runs/t/attempt-1"),
            }),
            "エラー: 同定情報を記録できませんでした。\n  \
             runディレクトリ: /home/u/.pulsen/state/runs/t/attempt-1"
        );
    }

    #[test]
    fn ラッパーの起動引数の制約の説明はドメインの言葉がそのまま出る() {
        assert_eq!(
            wrapper_error(&WrapperError::InvalidRunDir(
                AbsolutePathError::NotAbsolute {
                    given: PathBuf::from("state/runs/t/attempt-1"),
                }
            )),
            "エラー: --run-dir を絶対パスとして解決できません。\n  指定: state/runs/t/attempt-1"
        );
        assert_eq!(
            wrapper_error(&WrapperError::InvalidWorkspace(
                AbsolutePathError::NotAbsolute {
                    given: PathBuf::from("worktree"),
                }
            )),
            "エラー: --workspace を絶対パスとして解決できません。\n  指定: worktree"
        );
        assert_eq!(
            wrapper_error(&WrapperError::InvalidAgentCommand(CommandError::Empty)),
            format!(
                "エラー: エージェントのコマンドが不正です。\n  原因: {}",
                CommandError::Empty.describe()
            )
        );
    }

    #[test]
    fn 処理対象がなければその旨だけを表示する() {
        assert_eq!(
            tick_summary(&TickSummary::default()),
            "処理対象のタスクはありませんでした。"
        );
    }

    #[test]
    fn サマリーは値の入っている項目だけを並べる() {
        let summary = TickSummary {
            launched: vec![task("20260812t101112-abcd1234")],
            frozen: vec![task("20260812t101112-efgh5678")],
            ..TickSummary::default()
        };

        assert_eq!(
            tick_summary(&summary),
            "tick を実行しました。\n  \
             起動: 20260812t101112-abcd1234\n  \
             凍結: 20260812t101112-efgh5678"
        );
    }

    #[test]
    fn すべての項目が埋まったサマリーは決まった順で並ぶ() {
        let summary = TickSummary {
            launched: vec![task("20260812t101112-aaaa0001")],
            confirmed_running: vec![task("20260812t101112-aaaa0002")],
            transitioned: vec![task("20260812t101112-aaaa0003")],
            skipped_back: vec![task("20260812t101112-aaaa0004")],
            frozen: vec![task("20260812t101112-aaaa0005")],
            notified: vec![task("20260812t101112-aaaa0006")],
            archived: vec![task("20260812t101112-aaaa0007")],
            errors: vec![TickIssue::MissingCurrentAttempt {
                task_id: task("20260812t101112-aaaa0008"),
            }],
            gc_deleted: vec![(
                "20260812t101112-aaaa0009".to_owned(),
                AttemptNumber::parse(1).expect("受理される"),
            )],
            gc_errors: vec![(
                "20260812t101112-aaaa0010".to_owned(),
                AttemptNumber::parse(2).expect("受理される"),
            )],
        };

        assert_eq!(
            tick_summary(&summary),
            "tick を実行しました。\n  \
             起動: 20260812t101112-aaaa0001\n  \
             起動確認: 20260812t101112-aaaa0002\n  \
             遷移: 20260812t101112-aaaa0003\n  \
             実行待ちへ復帰: 20260812t101112-aaaa0004\n  \
             凍結: 20260812t101112-aaaa0005\n  \
             通知: 20260812t101112-aaaa0006\n  \
             終端処理: 20260812t101112-aaaa0007\n  \
             gcで削除: 20260812t101112-aaaa0009/attempt-1\n  \
             gcで削除できず: 20260812t101112-aaaa0010/attempt-2\n  \
             スキップ(1件):\n    \
             - 20260812t101112-aaaa0008: 起動記録済みですが現在 attempt がありません\
             (タスクファイルの修復が必要です)"
        );
    }

    #[test]
    fn 起動を確認したタスクはサマリーに現れる() {
        let summary = TickSummary {
            confirmed_running: vec![task("20260812t101112-abcd1234")],
            ..TickSummary::default()
        };

        assert_eq!(
            tick_summary(&summary),
            "tick を実行しました。\n  起動確認: 20260812t101112-abcd1234"
        );
    }

    #[test]
    fn 記録した失敗と素通しのスキップは別の見出しに分かれる() {
        let summary = TickSummary {
            errors: vec![
                TickIssue::WorktreeCreateFailed {
                    task_id: task("20260812t101112-abcd1234"),
                    message: "git worktree add に失敗".to_owned(),
                },
                TickIssue::InconsistentRunFiles {
                    task_id: task("20260812t101112-ijkl9012"),
                    kind: InconsistentRunFiles::MissingStartTime,
                },
                TickIssue::CommandExpansionFailed {
                    task_id: task("20260812t101112-efgh5678"),
                    message: "エージェント `claude` は config.yaml に定義されていません".to_owned(),
                },
                TickIssue::SpawnNotObserved {
                    task_id: task("20260812t101112-mnop3456"),
                    message: "起動から 30 秒のうちに pid ファイルが現れませんでした".to_owned(),
                },
            ],
            ..TickSummary::default()
        };

        assert_eq!(
            tick_summary(&summary),
            "tick を実行しました。\n  \
             失敗を記録(3件):\n    \
             - 20260812t101112-abcd1234: worktree を作成できません(git worktree add に失敗)\n    \
             - 20260812t101112-efgh5678: 起動コマンドを組み立てられません\
             (エージェント `claude` は config.yaml に定義されていません)\n    \
             - 20260812t101112-mnop3456: 起動を確認できず spawn 失敗として記録しました\
             (起動から 30 秒のうちに pid ファイルが現れませんでした)\n  \
             スキップ(1件):\n    \
             - 20260812t101112-ijkl9012: pid ファイルがあるのに starttime ファイルが\
             ありません(ラッパーは starttime を先に書く)"
        );
    }

    #[test]
    fn 遷移の前提の破れは破れた前提そのものを示す() {
        let transition = |error: TransitionError| {
            tick_summary(&TickSummary {
                errors: vec![TickIssue::Transition {
                    task_id: task("20260812t101112-abcd1234"),
                    error,
                }],
                ..TickSummary::default()
            })
        };

        assert!(
            transition(TransitionError::InvalidState {
                expected: &[ExecutionStateKind::Pending, ExecutionStateKind::Failed],
                actual: ExecutionStateKind::Running,
            })
            .ends_with("遷移の前提が成立しません(実行状態が pending | failed ではなく running)"),
        );
        assert!(
            transition(TransitionError::MissingCurrentAttempt)
                .ends_with("遷移の前提が成立しません(起動記録済みなのに現在 attempt が無い)"),
        );
    }

    #[test]
    fn スキップしたタスクは件数と原因つきで並ぶ() {
        let summary = TickSummary {
            errors: vec![
                TickIssue::CorruptTaskFile {
                    path: PathBuf::from("/home/u/.pulsen/state/tasks/broken.json"),
                    message: "JSON として読めない".to_owned(),
                },
                TickIssue::SaveFailed {
                    task_id: task("20260812t101112-abcd1234"),
                    error: SaveError::Io {
                        message: "書き込めません".to_owned(),
                    },
                },
            ],
            ..TickSummary::default()
        };

        assert_eq!(
            tick_summary(&summary),
            "tick を実行しました。\n  \
             スキップ(2件):\n    \
             - /home/u/.pulsen/state/tasks/broken.json: タスクファイルを読めません\
             (JSON として読めない)\n    \
             - 20260812t101112-abcd1234: タスクファイルを保存できません(書き込めません)"
        );
    }

    #[test]
    fn 起動の結果が未確定の報告は起動と同じサマリーに別の見出しで並ぶ() {
        let summary = TickSummary {
            launched: vec![task("20260812t101112-abcd1234")],
            errors: vec![
                TickIssue::PrepareAttemptFailed {
                    task_id: task("20260812t101112-abcd1234"),
                    message: "runディレクトリを作成できない".to_owned(),
                },
                TickIssue::SpawnFailed {
                    task_id: task("20260812t101112-efgh5678"),
                    message: "自身のバイナリを起動できない".to_owned(),
                },
            ],
            ..TickSummary::default()
        };

        assert_eq!(
            tick_summary(&summary),
            "tick を実行しました。\n  \
             起動: 20260812t101112-abcd1234\n  \
             起動の結果が未確定(2件):\n    \
             - 20260812t101112-abcd1234: attempt の runディレクトリを用意できません\
             (runディレクトリを作成できない)\n    \
             - 20260812t101112-efgh5678: ラッパーを起動できません(自身のバイナリを起動できない)"
        );
    }

    #[test]
    fn 走査自体の失敗は状態を変更していないことを添えて案内される() {
        assert_eq!(
            tick_error(&TickCommandError::Tick(TickError::Scan {
                message: "タスクの置き場を読めない".to_owned(),
            })),
            "エラー: タスクを走査できません。\n  \
             原因: タスクの置き場を読めない\n  \
             状態は変更していません。"
        );
    }

    #[test]
    fn ロック競合のスキップは失敗として案内しない() {
        assert!(!tick_skipped().starts_with("エラー"));
    }

    #[test]
    fn ベースブランチの不在は指定されたブランチ名を示す() {
        assert_eq!(
            register(RegisterTaskError::BaseBranchNotFound {
                branch: BranchName::parse("develop".to_owned()).expect("受理される"),
            }),
            "エラー: 指定したベースブランチがリポジトリに存在しません。\n  ブランチ: develop"
        );
    }
}
