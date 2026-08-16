//! 出力文言の組み立て。
//!
//! ユースケースは原因を値として返すだけで、利用者に見せる言葉はここで決める。
//! 出力は人間可読なテキストに限る — 機械可読形式は提供しない。
//!
//! コマンドごとに1モジュールを置き、このモジュールには複数コマンドが共有する組み立て
//! (項目行・エラー文言の骨組み)と、下位のエラー文言(結線・グローバル設定)だけを残す。

pub mod add;
pub mod ls;
pub mod show;
pub mod tick;
pub mod wrapper;

use std::path::Path;

use pulsen_domain::definition::{ConfigLoadError, SourceLocation};
use pulsen_domain::task::{AttemptNumber, RunDirPath, TaskId};

use super::wire::WireError;

pub use add::{add_error, registered};
pub use ls::{ls_error, task_list};
pub use show::{show_error, task_detail};
pub use tick::{tick_error, tick_skipped, tick_summary};
pub use wrapper::wrapper_error;

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

/// グローバル設定の読み込みの失敗。
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

    use crate::cli::add::AddError;

    use super::*;

    fn wire(error: WireError) -> String {
        add_error(&AddError::Wire(error))
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
}
