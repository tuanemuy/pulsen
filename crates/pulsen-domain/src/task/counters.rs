//! リトライ・判定・spawn 失敗のカウンタ。

/// 連続した失敗の数(ADR-009)。リセット規則は遷移関数の事後条件として定義する。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryCounters {
    attempt_count: u32,
    judge_attempt_count: u32,
    spawn_fail_count: u32,
}

impl RetryCounters {
    /// 登録直後の値(すべて 0)。
    pub const fn initial() -> Self {
        Self {
            attempt_count: 0,
            judge_attempt_count: 0,
            spawn_fail_count: 0,
        }
    }

    /// 永続化からの再構築。
    pub fn rehydrate(attempt_count: u32, judge_attempt_count: u32, spawn_fail_count: u32) -> Self {
        Self {
            attempt_count,
            judge_attempt_count,
            spawn_fail_count,
        }
    }

    /// 連続した実行失敗の数。
    pub fn attempt_count(&self) -> u32 {
        self.attempt_count
    }

    /// 連続した判定失敗の数。
    pub fn judge_attempt_count(&self) -> u32 {
        self.judge_attempt_count
    }

    /// 連続した spawn 失敗の数。
    pub fn spawn_fail_count(&self) -> u32 {
        self.spawn_fail_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 初期のカウンタはすべて0になる() {
        let counters = RetryCounters::initial();
        assert_eq!(counters.attempt_count(), 0);
        assert_eq!(counters.judge_attempt_count(), 0);
        assert_eq!(counters.spawn_fail_count(), 0);
    }

    #[test]
    fn カウンタは3フィールドの一致で等価になる() {
        assert_eq!(RetryCounters::rehydrate(0, 0, 0), RetryCounters::initial());
        assert_ne!(RetryCounters::rehydrate(1, 0, 0), RetryCounters::initial());
        assert_ne!(RetryCounters::rehydrate(0, 1, 0), RetryCounters::initial());
        assert_ne!(RetryCounters::rehydrate(0, 0, 1), RetryCounters::initial());
    }

    #[test]
    fn 再構築したカウンタは与えた値を保持する() {
        let counters = RetryCounters::rehydrate(2, 1, 3);
        assert_eq!(counters.attempt_count(), 2);
        assert_eq!(counters.judge_attempt_count(), 1);
        assert_eq!(counters.spawn_fail_count(), 3);
    }
}
