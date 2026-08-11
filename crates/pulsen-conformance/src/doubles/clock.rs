//! 固定時刻を返す `Clock` のダブル。

use pulsen_domain::task::{Clock, Timestamp};

/// いつ呼んでも同じ時刻を返すクロック。
///
/// 時刻を入力として受け取るドメインの判断を、実時間に依存させずに検証できる。
#[derive(Debug, Clone, Copy)]
pub struct FixedClock {
    now: Timestamp,
}

impl FixedClock {
    /// 返す時刻を与える。
    pub fn new(now: Timestamp) -> Self {
        Self { now }
    }
}

impl Clock for FixedClock {
    fn now(&self) -> Timestamp {
        self.now
    }
}
