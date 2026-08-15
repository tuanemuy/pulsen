//! stopped 確定通知の構成。

use crate::definition::{DurationSpec, StatusName, WorkflowName};
use crate::task::TaskId;

use super::value::CommandCompletion;

/// 通知の結末。
///
/// 成功だけが `notified_at` を書く根拠になる。失敗は理由を伴い、`notified_at` を書かずに
/// 終える(次の tick が再通知する。at-least-once)。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotifyOutcome {
    /// 通知できた。
    Delivered,
    /// 通知できなかった(非 0 終了・timeout・起動不能)。
    Failed {
        /// 原因の説明。
        detail: String,
    },
}

/// stopped 確定通知の構成(純粋)。
///
/// 通知に要る 3 値はいずれもスナップショットに依存しない — これがスナップショット破損
/// タスクにも再通知を行える根拠になる。
pub struct NotificationService;

impl NotificationService {
    /// notify_cmd に必ず適用する timeout(組み込み。ADR-018)。
    ///
    /// ハングした通知コマンドが排他ロックを保持したまま tick / CLI を塞ぐことを防ぐ。
    pub const NOTIFY_TIMEOUT: DurationSpec = DurationSpec::from_secs_unchecked(60);

    /// 通知コマンドの結末を成否として解釈する。
    ///
    /// 判断をドメインに置くのは、stopped を書いたすべての経路がこの規則を共有するため —
    /// 呼び出し側ごとに書くと、at-least-once の破れが片方だけに入る。
    pub fn interpret_notify_completion(completion: &CommandCompletion) -> NotifyOutcome {
        match completion {
            CommandCompletion::Exited(code) if code.is_success() => NotifyOutcome::Delivered,
            CommandCompletion::Exited(code) => NotifyOutcome::Failed {
                detail: format!("通知コマンドが終了コード {} で終了しました", code.get()),
            },
            CommandCompletion::TimedOut => NotifyOutcome::Failed {
                detail: format!(
                    "通知コマンドが {} 秒のうちに終了しませんでした",
                    Self::NOTIFY_TIMEOUT.seconds()
                ),
            },
            CommandCompletion::FailedToStart { message } => NotifyOutcome::Failed {
                detail: format!("通知コマンドを起動できませんでした: {message}"),
            },
        }
    }

    /// 通知コマンドへ渡す環境変数。
    pub fn notify_env(
        task_id: &TaskId,
        workflow: &WorkflowName,
        task_status: &StatusName,
    ) -> Vec<(String, String)> {
        vec![
            ("TASK_ID".to_owned(), task_id.as_str().to_owned()),
            ("WORKFLOW".to_owned(), workflow.as_str().to_owned()),
            ("TASK_STATUS".to_owned(), task_status.as_str().to_owned()),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution::ExitCode;

    fn detail_of(outcome: NotifyOutcome) -> String {
        match outcome {
            NotifyOutcome::Failed { detail } => detail,
            NotifyOutcome::Delivered => unreachable!("通知は失敗している"),
        }
    }

    fn task_id() -> TaskId {
        TaskId::parse("20260811t091530-k3f9qa1b".to_owned()).expect("受理される")
    }

    fn workflow() -> WorkflowName {
        WorkflowName::parse("implement".to_owned()).expect("受理される")
    }

    fn status() -> StatusName {
        StatusName::parse("queued".to_owned()).expect("受理される")
    }

    #[test]
    fn 通知の組み込みtimeoutは60秒である() {
        assert_eq!(NotificationService::NOTIFY_TIMEOUT.seconds(), 60);
    }

    #[test]
    fn 通知は終了コード0のときだけ成功になる() {
        assert_eq!(
            NotificationService::interpret_notify_completion(&CommandCompletion::Exited(
                ExitCode::new(0)
            )),
            NotifyOutcome::Delivered
        );
        for code in [1, 10, 20, 127, -1] {
            assert!(
                matches!(
                    NotificationService::interpret_notify_completion(&CommandCompletion::Exited(
                        ExitCode::new(code)
                    )),
                    NotifyOutcome::Failed { .. }
                ),
                "{code}"
            );
        }
    }

    #[test]
    fn 通知の失敗の3つの原因は説明から判別できる() {
        let non_zero = detail_of(NotificationService::interpret_notify_completion(
            &CommandCompletion::Exited(ExitCode::new(3)),
        ));
        let timed_out = detail_of(NotificationService::interpret_notify_completion(
            &CommandCompletion::TimedOut,
        ));
        let failed_to_start = detail_of(NotificationService::interpret_notify_completion(
            &CommandCompletion::FailedToStart {
                message: "実体が見つかりません".to_owned(),
            },
        ));

        assert_ne!(non_zero, timed_out);
        assert_ne!(timed_out, failed_to_start);
        assert_ne!(failed_to_start, non_zero);
        assert!(non_zero.contains('3'), "{non_zero}");
        assert!(
            timed_out.contains(&NotificationService::NOTIFY_TIMEOUT.seconds().to_string()),
            "{timed_out}"
        );
        assert!(
            failed_to_start.contains("実体が見つかりません"),
            "{failed_to_start}"
        );
    }

    #[test]
    fn 通知コマンドへ渡す環境変数は3つになる() {
        let env = NotificationService::notify_env(&task_id(), &workflow(), &status());

        assert_eq!(
            env,
            vec![
                ("TASK_ID".to_owned(), "20260811t091530-k3f9qa1b".to_owned()),
                ("WORKFLOW".to_owned(), "implement".to_owned()),
                ("TASK_STATUS".to_owned(), "queued".to_owned()),
            ]
        );
    }
}
