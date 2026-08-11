//! システムの壁時計を読む `Clock` の実装。

use std::time::{SystemTime, UNIX_EPOCH};

use pulsen_domain::task::{Clock, Timestamp};

/// システムの壁時計を秒精度の UTC で返すクロック。
///
/// `now` は無謬(spec)なので、`SystemTime` 側の失敗は値へ写して吸収する(ADR-036)。
/// epoch より前を指す時計は負の Unix 秒になり、表現可能範囲の外は範囲の端に飽和する。
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl SystemClock {
    /// システム時計を読むクロックを作る。
    pub fn new() -> Self {
        Self
    }
}

impl Clock for SystemClock {
    fn now(&self) -> Timestamp {
        Timestamp::saturating_from_unix_secs(unix_secs(SystemTime::now()))
    }
}

/// UNIX epoch からの符号つき経過秒。サブ秒は切り捨てる。
///
/// epoch より前の時刻は `duration_since` が `Err` になるが、差分そのものは取れるので
/// 負の秒に写せる。秒数が `i64` に収まらない時刻は `Timestamp` の表現可能範囲の外側
/// なので、そのまま端へ倒す。
fn unix_secs(time: SystemTime) -> i64 {
    match time.duration_since(UNIX_EPOCH) {
        Ok(elapsed) => i64::try_from(elapsed.as_secs()).unwrap_or(i64::MAX),
        Err(error) => i64::try_from(error.duration().as_secs())
            .map(|secs| -secs)
            .unwrap_or(i64::MIN),
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn 現在時刻は呼び出しの前後に観測した実時刻の範囲に収まる() {
        let before = unix_secs(SystemTime::now());
        let observed = SystemClock::new().now();
        let after = unix_secs(SystemTime::now());

        assert!(before <= observed.unix_secs());
        assert!(observed.unix_secs() <= after);
    }

    #[test]
    fn epochより前の時刻は負の秒になる() {
        let before_epoch = UNIX_EPOCH - Duration::from_secs(90);

        assert_eq!(unix_secs(before_epoch), -90);
    }

    #[test]
    fn epochちょうどは0秒になる() {
        assert_eq!(unix_secs(UNIX_EPOCH), 0);
    }
}
