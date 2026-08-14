//! 終了した実行の判定。

use crate::task::{RunDirPath, TaskId, WorktreePath};

use super::value::{CommandCompletion, ExitCode, JudgeConclusion, JudgeOutcome};

/// 判定コマンドが成功を表す exit code。
const JUDGE_COMPLETED: i32 = 0;
/// 判定コマンドが失敗を表す exit code。
const JUDGE_FAILED: i32 = 10;
/// 判定コマンドが見送りを表す exit code。
const JUDGE_SKIPPED: i32 = 20;

/// 終了した実行の判定(純粋)。
///
/// 判定は冪等 — 同じ exit・同じ定義に対して常に同じ結論を導く。判定コマンド自体の
/// 冪等性は利用者の責務であり、ここで担保するのは解釈の側だけ。
pub struct JudgementService;

impl JudgementService {
    /// 判定コマンドが未定義のステータスでの判定。
    ///
    /// 2 値しか返さない — 判定コマンドを持たないステータスで `Skipped` を導く経路は無く、
    /// exit 20 も他の非 0 と同じ失敗になる(ADR-008)。
    pub fn default_judgement(exit: &ExitCode) -> JudgeOutcome {
        if exit.is_success() {
            JudgeOutcome::Completed
        } else {
            JudgeOutcome::Failed
        }
    }

    /// 判定コマンドの結末を判定プロトコルの4値として解釈する。
    ///
    /// プロトコル外の exit code・timeout・起動不能はいずれも「判定自体が壊れた」であり、
    /// 実行の帳簿(`attempt_count`)ではなく判定の帳簿を消費させる。
    pub fn interpret_judge_completion(completion: &CommandCompletion) -> JudgeConclusion {
        match completion {
            CommandCompletion::Exited(exit) => match exit.get() {
                JUDGE_COMPLETED => JudgeConclusion::Outcome(JudgeOutcome::Completed),
                JUDGE_FAILED => JudgeConclusion::Outcome(JudgeOutcome::Failed),
                JUDGE_SKIPPED => JudgeConclusion::Outcome(JudgeOutcome::Skipped),
                other => JudgeConclusion::JudgeFailure {
                    detail: format!(
                        "判定コマンドがプロトコル外の終了コード {other} を返しました(有効な値は {JUDGE_COMPLETED} / {JUDGE_FAILED} / {JUDGE_SKIPPED})"
                    ),
                },
            },
            CommandCompletion::TimedOut => JudgeConclusion::JudgeFailure {
                detail: "判定コマンドが timeout までに終了しませんでした".to_owned(),
            },
            CommandCompletion::FailedToStart { message } => JudgeConclusion::JudgeFailure {
                detail: format!("判定コマンドを起動できませんでした: {message}"),
            },
        }
    }

    /// 判定コマンドへ渡す環境変数。
    ///
    /// 判定コマンドは引数を受け取らず、必要な文脈をすべてここから得る。
    pub fn judge_env(
        task_id: &TaskId,
        workspace: &WorktreePath,
        exit: &ExitCode,
        run_dir: &RunDirPath,
    ) -> Vec<(String, String)> {
        vec![
            ("TASK_ID".to_owned(), task_id.as_str().to_owned()),
            (
                "WORKSPACE".to_owned(),
                workspace.as_path().to_string_lossy().into_owned(),
            ),
            ("EXIT_CODE".to_owned(), exit.get().to_string()),
            (
                "RUN_DIR".to_owned(),
                run_dir.as_path().to_string_lossy().into_owned(),
            ),
        ]
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::task::{AttemptNumber, StateRoot};

    fn absolute(segments: &[&str]) -> PathBuf {
        let mut path = if std::path::MAIN_SEPARATOR == '\\' {
            PathBuf::from("C:\\")
        } else {
            PathBuf::from("/")
        };
        for segment in segments {
            path.push(segment);
        }
        path
    }

    fn task_id() -> TaskId {
        TaskId::parse("20260811t091530-k3f9qa1b".to_owned()).expect("受理される")
    }

    fn workspace() -> WorktreePath {
        WorktreePath::parse(absolute(&["worktrees", "20260811t091530-k3f9qa1b"]))
            .expect("受理される")
    }

    fn run_dir() -> RunDirPath {
        RunDirPath::derive(
            &StateRoot::parse(absolute(&["home", "u", ".pulsen", "state"])).expect("受理される"),
            &task_id(),
            AttemptNumber::FIRST,
        )
    }

    fn exited(code: i32) -> CommandCompletion {
        CommandCompletion::Exited(ExitCode::new(code))
    }

    fn detail_of(conclusion: JudgeConclusion) -> String {
        match conclusion {
            JudgeConclusion::JudgeFailure { detail } => detail,
            JudgeConclusion::Outcome(outcome) => {
                unreachable!("判定自体の破れである(実際は {outcome:?})")
            }
        }
    }

    #[test]
    fn デフォルト判定は0を成功とする() {
        assert_eq!(
            JudgementService::default_judgement(&ExitCode::new(0)),
            JudgeOutcome::Completed
        );
    }

    #[test]
    fn デフォルト判定は20を含む非0をすべて失敗とする() {
        for code in [1, 10, 20, 126, 127, 143] {
            assert_eq!(
                JudgementService::default_judgement(&ExitCode::new(code)),
                JudgeOutcome::Failed,
                "{code}"
            );
        }
    }

    #[test]
    fn 判定コマンドの終了コードは4値のプロトコルとして解釈される() {
        assert_eq!(
            JudgementService::interpret_judge_completion(&exited(0)),
            JudgeConclusion::Outcome(JudgeOutcome::Completed)
        );
        assert_eq!(
            JudgementService::interpret_judge_completion(&exited(10)),
            JudgeConclusion::Outcome(JudgeOutcome::Failed)
        );
        assert_eq!(
            JudgementService::interpret_judge_completion(&exited(20)),
            JudgeConclusion::Outcome(JudgeOutcome::Skipped)
        );
    }

    #[test]
    fn プロトコル外の終了コードは判定自体の破れになる() {
        for code in [1, 2, 11, 21, 127, -1] {
            let detail = detail_of(JudgementService::interpret_judge_completion(&exited(code)));
            assert!(detail.contains(&code.to_string()), "{code}: {detail}");
        }
    }

    #[test]
    fn 判定の3つの原因は説明から判別できる() {
        let out_of_protocol = detail_of(JudgementService::interpret_judge_completion(&exited(7)));
        let timed_out = detail_of(JudgementService::interpret_judge_completion(
            &CommandCompletion::TimedOut,
        ));
        let failed_to_start = detail_of(JudgementService::interpret_judge_completion(
            &CommandCompletion::FailedToStart {
                message: "実体が見つかりません".to_owned(),
            },
        ));

        assert_ne!(out_of_protocol, timed_out);
        assert_ne!(timed_out, failed_to_start);
        assert_ne!(failed_to_start, out_of_protocol);
        assert!(timed_out.contains("timeout"), "{timed_out}");
        assert!(
            failed_to_start.contains("実体が見つかりません"),
            "{failed_to_start}"
        );
    }

    #[test]
    fn 判定コマンドへ渡す環境変数は4つになる() {
        let env =
            JudgementService::judge_env(&task_id(), &workspace(), &ExitCode::new(0), &run_dir());

        let names: Vec<&str> = env.iter().map(|(name, _)| name.as_str()).collect();
        assert_eq!(names, ["TASK_ID", "WORKSPACE", "EXIT_CODE", "RUN_DIR"]);
    }

    #[test]
    fn 終了コードは10進文字列として渡る() {
        let env_for = |code: i32| {
            JudgementService::judge_env(&task_id(), &workspace(), &ExitCode::new(code), &run_dir())
                [2]
            .1
            .clone()
        };

        assert_eq!(env_for(0), "0");
        assert_eq!(env_for(127), "127");
        assert_eq!(env_for(-1), "-1");
    }

    #[test]
    fn 環境変数の値はタスクとパスの値をそのまま渡す() {
        let env =
            JudgementService::judge_env(&task_id(), &workspace(), &ExitCode::new(0), &run_dir());

        assert_eq!(env[0].1, task_id().as_str());
        assert_eq!(env[1].1, workspace().as_path().to_string_lossy());
        assert_eq!(env[3].1, run_dir().as_path().to_string_lossy());
    }
}
