//! 取得結果を与える `ExclusiveLock` のダブル。

use std::cell::RefCell;
use std::collections::VecDeque;

use pulsen_domain::execution::{ExclusiveLock, LockError, LockGuard};

/// `try_acquire` の1回分の結果。
///
/// `Result<Option<Box<dyn LockGuard>>, LockError>` はガードを値として持てないため、
/// 台本には結果の種類だけを並べる。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LockOutcome {
    /// 取得できた。
    Acquired,
    /// 別の操作が保持している。
    Busy,
    /// ロック機構自体の異常。
    Failed {
        /// 原因の説明。
        message: String,
    },
}

/// ダブルが渡すガード。ドロップ以外の契約を持たない。
struct ScriptedGuard;

impl LockGuard for ScriptedGuard {}

/// あらかじめ与えた結果を順に返すロック。
#[derive(Debug)]
pub struct ScriptedExclusiveLock {
    outcomes: RefCell<VecDeque<LockOutcome>>,
    attempts: RefCell<usize>,
}

impl ScriptedExclusiveLock {
    /// `try_acquire` が返す結果の列を与える。
    pub fn new(outcomes: impl IntoIterator<Item = LockOutcome>) -> Self {
        Self {
            outcomes: RefCell::new(outcomes.into_iter().collect()),
            attempts: RefCell::new(0),
        }
    }

    /// これまでの取得の試行回数。
    pub fn attempts(&self) -> usize {
        *self.attempts.borrow()
    }
}

impl ExclusiveLock for ScriptedExclusiveLock {
    fn try_acquire(&self) -> Result<Option<Box<dyn LockGuard>>, LockError> {
        let Some(outcome) = self.outcomes.borrow_mut().pop_front() else {
            panic!("ロックの取得結果を使い切った")
        };
        *self.attempts.borrow_mut() += 1;
        match outcome {
            LockOutcome::Acquired => Ok(Some(Box::new(ScriptedGuard))),
            LockOutcome::Busy => Ok(None),
            LockOutcome::Failed { message } => Err(LockError::Failed { message }),
        }
    }
}
