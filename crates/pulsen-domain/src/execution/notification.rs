//! stopped 確定通知の構成。

use crate::definition::{DurationSpec, StatusName, WorkflowName};
use crate::task::TaskId;

use super::value::{CommandCompletion, ExitCode};

/// 通知の結末。
///
/// 成功だけが `notified_at` を書く根拠になる。失敗は原因を伴い、`notified_at` を書かずに
/// 終える(次の tick が再通知する。at-least-once)。
///
/// `Failed` を平坦化しないのは、`Delivered` / `Failed` の 2 分岐が at-least-once の規則
/// そのものだからである。原因は内側の分類として持つ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotifyOutcome {
    /// 通知できた。
    Delivered,
    /// 通知できなかった。
    Failed {
        /// 通知が届かなかった原因。
        cause: NotifyFailureCause,
    },
}

/// 通知が届かなかった原因の分類。
///
/// 帳簿に残らず表示にしか使われないため、分類だけを持ち完成文言は持たない(文言は CLI 層が
/// 組み立てる)。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotifyFailureCause {
    /// 通知コマンドが非 0 で終了した。
    ExitedNonZero {
        /// 通知コマンドの終了コード。
        exit: ExitCode,
    },
    /// 通知コマンドが timeout のうちに終わらなかった。
    ///
    /// 秒数を持たないのは、通知の timeout が設定値ではなく組み込み定数
    /// [`NotificationService::NOTIFY_TIMEOUT`] の 1 つに定まるためである(表示側が定数を読む)。
    TimedOut,
    /// 通知コマンドを起動できなかった。
    FailedToStart {
        /// 起動できなかった原因の説明(OS 由来)。
        message: String,
    },
}

/// stopped 確定通知の構成(純粋)。
///
/// 通知に要る 3 値はいずれもスナップショットに依存しない — これがスナップショット破損
/// タスクにも再通知を行える根拠になる。
pub struct NotificationService;

impl NotificationService {
    /// notify_cmd に必ず適用する timeout(組み込み)。
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
                cause: NotifyFailureCause::ExitedNonZero { exit: *code },
            },
            CommandCompletion::TimedOut => NotifyOutcome::Failed {
                cause: NotifyFailureCause::TimedOut,
            },
            CommandCompletion::FailedToStart { message } => NotifyOutcome::Failed {
                cause: NotifyFailureCause::FailedToStart {
                    message: message.clone(),
                },
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
    fn 通知の失敗の3つの原因は分類として判別できる() {
        assert_eq!(
            NotificationService::interpret_notify_completion(&CommandCompletion::Exited(
                ExitCode::new(3)
            )),
            NotifyOutcome::Failed {
                cause: NotifyFailureCause::ExitedNonZero {
                    exit: ExitCode::new(3)
                }
            }
        );
        assert_eq!(
            NotificationService::interpret_notify_completion(&CommandCompletion::TimedOut),
            NotifyOutcome::Failed {
                cause: NotifyFailureCause::TimedOut
            }
        );
        assert_eq!(
            NotificationService::interpret_notify_completion(&CommandCompletion::FailedToStart {
                message: "実体が見つかりません".to_owned(),
            }),
            NotifyOutcome::Failed {
                cause: NotifyFailureCause::FailedToStart {
                    message: "実体が見つかりません".to_owned()
                }
            }
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
