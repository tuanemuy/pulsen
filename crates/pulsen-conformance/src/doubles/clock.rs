//! 時刻を与える `Clock` のダブル。

use std::cell::Cell;

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

/// 返す時刻を任意の値に置けるクロック。
///
/// 猶予時間の境界(30秒 / 31秒 / 巻き戻り)は実時間で待てないため、tick を回す間に
/// 時刻そのものを動かす。`FixedClock` と分けるのは、時刻が動かないことを前提にする
/// テストが「うっかり動かせる」形にならないようにするため。
#[derive(Debug)]
pub struct SettableClock {
    now: Cell<Timestamp>,
}

impl SettableClock {
    /// 最初に返す時刻を与える。
    pub fn new(now: Timestamp) -> Self {
        Self {
            now: Cell::new(now),
        }
    }

    /// 以降が返す時刻を置き換える。過去へ戻すこともできる。
    pub fn set(&self, now: Timestamp) {
        self.now.set(now);
    }
}

impl Clock for SettableClock {
    fn now(&self) -> Timestamp {
        self.now.get()
    }
}
