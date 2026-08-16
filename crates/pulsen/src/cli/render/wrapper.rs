//! `wrapper`(内部コマンド)の文言。

use pulsen_domain::definition::CommandError;
use pulsen_domain::task::AbsolutePathError;

use crate::cli::wrapper::WrapperError;

use super::{problem, wire_error};

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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::cli::wire::WireError;

    use super::*;

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
}
