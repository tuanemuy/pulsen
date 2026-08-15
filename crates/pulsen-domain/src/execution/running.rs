//! running タスクの分類。

use crate::definition::TimeoutSpec;
use crate::task::{ProcessStartTime, Timestamp};

use super::value::ExitCode;

/// 同一プロセスが生存しているか。
///
/// `Dead` は「取得不能(プロセス不在)」と「起動時刻の不一致(PID 再利用)」の両方を含む。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aliveness {
    /// 記録した起動時刻と一致するプロセスが生きている。
    Alive,
    /// 同一プロセスはもういない。
    Dead,
}

/// running タスクの分類結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunningDecision {
    /// exit ファイルがある — 判定を実行する。
    Judge(ExitCode),
    /// exit なし・生存・timeout 未超過 — 何もしない。
    KeepRunning,
    /// exit なし・生存・timeout 超過 — kill してから失敗にする。
    KillOnTimeout,
    /// exit なし・死亡 — 残存の終了をベストエフォートで試みてから失敗にする。
    DiedWithoutExit,
}

/// exit を残していない running タスクの分類結果(2 段目)。
///
/// `RunningDecision` から `Judge` を除いた 3 値。生存の観測からは判定が導かれないことを、
/// 返り値の型そのものが述べる。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AliveDecision {
    /// 生存・timeout 未超過 — 何もしない。
    KeepRunning,
    /// 生存・timeout 超過 — kill してから失敗にする。
    KillOnTimeout,
    /// 死亡 — 残存の終了をベストエフォートで試みてから失敗にする。
    DiedWithoutExit,
}

impl From<AliveDecision> for RunningDecision {
    fn from(decision: AliveDecision) -> Self {
        match decision {
            AliveDecision::KeepRunning => Self::KeepRunning,
            AliveDecision::KillOnTimeout => Self::KillOnTimeout,
            AliveDecision::DiedWithoutExit => Self::DiedWithoutExit,
        }
    }
}

/// PID 再利用対策の起動時刻照合(純粋)。
pub struct IdentityCheck;

impl IdentityCheck {
    /// 観測した起動時刻を記録済みの値と照合する。
    ///
    /// 取得できなかった(`None`)場合も不一致と同じ `Dead` に写像する。生存を確認できない
    /// 以上、kill の対象としても分類の対象としても「同一プロセスではない」で足りる。
    /// 取得機構そのものの失敗はこの関数へ到達しない — ポートが `Err` として返し、
    /// 呼び出し側は状態を変更しない。
    pub fn check(observed: Option<&ProcessStartTime>, recorded: &ProcessStartTime) -> Aliveness {
        match observed {
            Some(observed) if observed == recorded => Aliveness::Alive,
            Some(_) | None => Aliveness::Dead,
        }
    }
}

/// running タスクの分類(純粋)。
///
/// 分類は 2 段で行う。exit ファイルがあれば実行は終了しており、生存観測は**不要かつ
/// 行わない**(観測の一過性の失敗で判定を遅延させない)。1 段目(exit の有無)は
/// 観測を行うユースケース側にあり、この型が受け持つのは 2 段目だけ — 返り値を
/// `AliveDecision` に絞ることで、その規則を型で担保する。
pub struct RunningClassifier;

impl RunningClassifier {
    /// 生存の観測結果から running タスクを分類する。
    ///
    /// timeout の経過は記録済み starttime の壁時計成分を起点に測る(launching の猶予時間は
    /// 含まれない)。巻き戻りは 0 に飽和し、超過は経過が timeout を**上回った**ときにのみ
    /// 成立する。
    pub fn classify_alive(
        aliveness: Aliveness,
        started_wall: &Timestamp,
        timeout: &TimeoutSpec,
        now: &Timestamp,
    ) -> AliveDecision {
        match aliveness {
            Aliveness::Dead => AliveDecision::DiedWithoutExit,
            Aliveness::Alive => match timeout {
                TimeoutSpec::Unlimited => AliveDecision::KeepRunning,
                TimeoutSpec::Limited(limit) => {
                    if now.elapsed_since(started_wall) > limit.seconds() {
                        AliveDecision::KillOnTimeout
                    } else {
                        AliveDecision::KeepRunning
                    }
                }
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::definition::DurationSpec;

    fn starttime(value: &str) -> ProcessStartTime {
        ProcessStartTime::parse(value.to_owned()).expect("受理される")
    }

    fn started_wall() -> Timestamp {
        Timestamp::parse_rfc3339("2026-08-11T09:15:30Z").expect("受理される")
    }

    fn after(seconds: i64) -> Timestamp {
        Timestamp::from_unix_secs(started_wall().unix_secs() + seconds).expect("受理される")
    }

    fn limited(text: &str) -> TimeoutSpec {
        TimeoutSpec::Limited(DurationSpec::parse(text).expect("受理される"))
    }

    #[test]
    fn 起動時刻を取得できなければ死亡とみなす() {
        assert_eq!(
            IdentityCheck::check(None, &starttime("871234")),
            Aliveness::Dead
        );
    }

    #[test]
    fn 起動時刻が記録と食い違えば別のプロセスとみなす() {
        assert_eq!(
            IdentityCheck::check(Some(&starttime("999999")), &starttime("871234")),
            Aliveness::Dead
        );
    }

    #[test]
    fn 起動時刻が記録と一致すれば同一プロセスが生存しているとみなす() {
        assert_eq!(
            IdentityCheck::check(Some(&starttime("871234")), &starttime("871234")),
            Aliveness::Alive
        );
    }

    #[test]
    fn 生存していてtimeout未超過なら実行を続ける() {
        for elapsed in [0, 1, 60] {
            assert_eq!(
                RunningClassifier::classify_alive(
                    Aliveness::Alive,
                    &started_wall(),
                    &limited("60s"),
                    &after(elapsed),
                ),
                AliveDecision::KeepRunning,
                "{elapsed}秒"
            );
        }
    }

    #[test]
    fn 生存していてtimeoutを超えたらkillしてから失敗にする() {
        assert_eq!(
            RunningClassifier::classify_alive(
                Aliveness::Alive,
                &started_wall(),
                &limited("60s"),
                &after(61),
            ),
            AliveDecision::KillOnTimeout
        );
    }

    #[test]
    fn timeoutが無制限ならどれだけ経過しても実行を続ける() {
        assert_eq!(
            RunningClassifier::classify_alive(
                Aliveness::Alive,
                &started_wall(),
                &TimeoutSpec::Unlimited,
                &after(86_400),
            ),
            AliveDecision::KeepRunning
        );
    }

    #[test]
    fn 時計が巻き戻っていてもtimeoutを超えたとみなさない() {
        assert_eq!(
            RunningClassifier::classify_alive(
                Aliveness::Alive,
                &started_wall(),
                &limited("60s"),
                &after(-3600),
            ),
            AliveDecision::KeepRunning
        );
    }

    #[test]
    fn 死亡していればtimeoutによらずexitなしの死亡になる() {
        for timeout in [limited("60s"), TimeoutSpec::Unlimited] {
            assert_eq!(
                RunningClassifier::classify_alive(
                    Aliveness::Dead,
                    &started_wall(),
                    &timeout,
                    &after(1),
                ),
                AliveDecision::DiedWithoutExit,
                "{timeout:?}"
            );
        }
    }

    #[test]
    fn 分類の4値は互いに区別される() {
        assert_ne!(
            RunningDecision::Judge(ExitCode::new(0)),
            RunningDecision::Judge(ExitCode::new(1))
        );
        assert_ne!(RunningDecision::KeepRunning, RunningDecision::KillOnTimeout);
        assert_ne!(
            RunningDecision::KillOnTimeout,
            RunningDecision::DiedWithoutExit
        );
    }

    #[test]
    fn 生存の分類は判定以外の3値へそのまま対応する() {
        for (alive, expected) in [
            (AliveDecision::KeepRunning, RunningDecision::KeepRunning),
            (AliveDecision::KillOnTimeout, RunningDecision::KillOnTimeout),
            (
                AliveDecision::DiedWithoutExit,
                RunningDecision::DiedWithoutExit,
            ),
        ] {
            assert_eq!(RunningDecision::from(alive), expected, "{alive:?}");
        }
    }
}
