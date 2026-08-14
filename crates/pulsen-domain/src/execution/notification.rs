//! stopped 確定通知の構成。

use crate::definition::{DurationSpec, StatusName, WorkflowName};
use crate::task::TaskId;

/// stopped 確定通知の構成(純粋)。
///
/// 通知に要る 3 値はいずれもスナップショットに依存しない — これがスナップショット破損
/// タスクにも再通知を行える根拠になる。
pub struct NotificationService;

impl NotificationService {
    /// notify_cmd に必ず適用する timeout(組み込み。ADR-018)。
    ///
    /// ハングした通知コマンドが排他ロックを保持したまま tick / CLI を塞ぐことを防ぐ。
    /// 超過・起動不能・非 0 終了はいずれも通知失敗であり、`notified_at` を書かずに
    /// 終える(次の tick が再通知する。at-least-once)。
    pub const NOTIFY_TIMEOUT: DurationSpec = DurationSpec::from_secs_unchecked(60);

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
