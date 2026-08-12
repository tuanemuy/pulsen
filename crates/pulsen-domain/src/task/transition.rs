//! 遷移の前提が満たされないことの表現。

use crate::definition::StatusName;

use super::state::ExecutionStateKind;

/// 遷移関数の前提の破れ。
///
/// 状態間整合の不変条件(2〜4)は手動修復で破られたまま再構築されうるため、遷移関数が
/// 前提として検査し値で返す。パニックは遷移関数自身が事後条件を破った場合に限る。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransitionError {
    /// 前提状態の不一致。
    InvalidState {
        /// 前提となる実行状態。
        expected: &'static str,
        /// 実際の実行状態。
        actual: ExecutionStateKind,
    },
    /// ワークスペースの再確定(一度確定したら変更されない)。
    WorkspaceAlreadySet,
    /// ワークスペース未確定のままの起動記録。
    WorkspaceNotSet,
    /// エージェント実行を前提とする遷移を、他の動作種別のステータスで呼んだ。
    NotAgentRunStatus {
        /// 呼び出し時のタスクステータス。
        status: StatusName,
    },
    /// 手動修復で破られた不変条件(起動記録済みなのに現在 attempt が無い等)。
    InvariantViolated {
        /// 破れの説明。
        message: String,
    },
}
