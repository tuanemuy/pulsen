//! 実行状態 — ツールが固定管理する6状態とその判別子。

use super::time::Timestamp;

/// 実行状態の判別子の生成失敗。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateKindError {
    /// 6値のいずれでもない。
    Unknown {
        /// 与えられた文字列。
        given: String,
        /// 有効な値の一覧(案内に使う)。
        valid: [&'static str; ExecutionStateKind::COUNT],
    },
}

/// 実行状態の判別子(データなし)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ExecutionStateKind {
    /// 起動待ち。
    Pending,
    /// 起動記録済み・spawn 確認未了。
    Launching,
    /// 起動確認済み・分類未了。
    Running,
    /// 判定確定(成功)。
    Completed,
    /// 失敗確定。
    Failed,
    /// 凍結。
    Stopped,
}

impl ExecutionStateKind {
    /// 判別子の総数。
    pub const COUNT: usize = 6;

    /// 有効な値の一覧(小文字表記)。
    pub const VALID: [&'static str; Self::COUNT] = [
        "pending",
        "launching",
        "running",
        "completed",
        "failed",
        "stopped",
    ];

    /// 小文字表記の6値のみを受理する。
    pub fn parse(s: &str) -> Result<Self, StateKindError> {
        match s {
            "pending" => Ok(Self::Pending),
            "launching" => Ok(Self::Launching),
            "running" => Ok(Self::Running),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "stopped" => Ok(Self::Stopped),
            other => Err(StateKindError::Unknown {
                given: other.to_owned(),
                valid: Self::VALID,
            }),
        }
    }

    /// 小文字表記。
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Launching => "launching",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Stopped => "stopped",
        }
    }
}

/// 凍結に至った経路。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    /// リトライ上限の超過。
    RetryLimitExceeded,
    /// 判定失敗の上限の超過。
    JudgeLimitExceeded,
    /// spawn 失敗の上限の超過。
    SpawnFailLimitExceeded,
    /// 利用者による中断。
    Aborted,
}

/// 実行状態。付随データを状態ごとに型で持つ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionState {
    /// 起動待ち。
    Pending,
    /// 起動記録済み・spawn 確認未了。
    Launching {
        /// 猶予時間の起点。クラッシュを跨いで判定するため永続化する。
        recorded_at: Timestamp,
    },
    /// 起動確認済み・分類未了(同定情報は現在 attempt が持つ)。
    Running,
    /// 判定確定。次 tick が遷移先へ進める。
    Completed,
    /// 失敗確定。次 tick が再試行する。
    Failed,
    /// 凍結。
    Stopped {
        /// 凍結の要因。
        reason: StopReason,
        /// 通知済みの時刻。未通知は `None`。
        notified_at: Option<Timestamp>,
    },
}

impl ExecutionState {
    /// 判別子を導出する。
    pub fn kind(&self) -> ExecutionStateKind {
        match self {
            Self::Pending => ExecutionStateKind::Pending,
            Self::Launching { .. } => ExecutionStateKind::Launching,
            Self::Running => ExecutionStateKind::Running,
            Self::Completed => ExecutionStateKind::Completed,
            Self::Failed => ExecutionStateKind::Failed,
            Self::Stopped { .. } => ExecutionStateKind::Stopped,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> Timestamp {
        Timestamp::parse_rfc3339("2026-08-11T09:15:30Z").expect("受理される")
    }

    #[test]
    fn 小文字の6値は判別子として受理される() {
        for (given, expected) in [
            ("pending", ExecutionStateKind::Pending),
            ("launching", ExecutionStateKind::Launching),
            ("running", ExecutionStateKind::Running),
            ("completed", ExecutionStateKind::Completed),
            ("failed", ExecutionStateKind::Failed),
            ("stopped", ExecutionStateKind::Stopped),
        ] {
            assert_eq!(ExecutionStateKind::parse(given), Ok(expected));
            assert_eq!(expected.as_str(), given);
        }
    }

    #[test]
    fn 固定6値以外の判別子は有効値一覧つきで拒否される() {
        for given in ["Pending", "PENDING", "queued", ""] {
            assert_eq!(
                ExecutionStateKind::parse(given),
                Err(StateKindError::Unknown {
                    given: given.to_owned(),
                    valid: ExecutionStateKind::VALID,
                }),
                "{given}"
            );
        }
    }

    #[test]
    fn 実行状態は判別子を導出する() {
        assert_eq!(ExecutionState::Pending.kind(), ExecutionStateKind::Pending);
        assert_eq!(
            ExecutionState::Launching { recorded_at: now() }.kind(),
            ExecutionStateKind::Launching
        );
        assert_eq!(ExecutionState::Running.kind(), ExecutionStateKind::Running);
        assert_eq!(
            ExecutionState::Completed.kind(),
            ExecutionStateKind::Completed
        );
        assert_eq!(ExecutionState::Failed.kind(), ExecutionStateKind::Failed);
        assert_eq!(
            ExecutionState::Stopped {
                reason: StopReason::Aborted,
                notified_at: None,
            }
            .kind(),
            ExecutionStateKind::Stopped
        );
    }

    #[test]
    fn 凍結だけが通知時刻を持てる() {
        let stopped = ExecutionState::Stopped {
            reason: StopReason::RetryLimitExceeded,
            notified_at: Some(now()),
        };
        assert_ne!(
            stopped,
            ExecutionState::Stopped {
                reason: StopReason::RetryLimitExceeded,
                notified_at: None,
            }
        );
    }
}
