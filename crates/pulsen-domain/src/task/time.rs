//! UTC 秒精度の時刻。
//!
//! タスクファイルの直列化表現である RFC3339(`YYYY-MM-DDTHH:MM:SSZ`)との相互変換を
//! ドメインに持つ。暦計算は proleptic Gregorian の days-from-civil /
//! civil-from-days を自前で持ち、外部クレートに依存しない。

/// 1日の秒数。
const SECS_PER_DAY: i64 = 24 * 60 * 60;
/// days-from-civil の基準(1970-01-01)を 0000-03-01 起点の内部表現へ移す差分。
const CIVIL_EPOCH_SHIFT: i64 = 719_468;
/// 400年周期の日数。
const DAYS_PER_ERA: i64 = 146_097;

/// 時刻の生成失敗。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimestampError {
    /// `YYYY-MM-DDTHH:MM:SSZ` の形式ではない。
    InvalidFormat {
        /// 与えられた文字列。
        given: String,
    },
    /// 形式は正しいが暦上存在しない日付・時刻を指す。
    InvalidDateTime {
        /// 与えられた文字列。
        given: String,
    },
    /// 表現可能範囲の外。
    OutOfRange {
        /// 範囲外の Unix 秒。
        unix_secs: i64,
    },
}

impl TimestampError {
    /// 制約の説明。
    ///
    /// タスクファイルの復号に失敗した理由は、破損したタスクの一覧・表示を通じて
    /// 利用者に見せる修復の材料になる。説明の定義箇所をドメインに1つ置く。
    pub fn describe(&self) -> String {
        match self {
            Self::InvalidFormat { given } => {
                format!("`YYYY-MM-DDTHH:MM:SSZ` の形式ではありません(実際は `{given}`)")
            }
            Self::InvalidDateTime { given } => {
                format!("暦上存在しない日時です(`{given}`)")
            }
            Self::OutOfRange { unix_secs } => format!(
                "扱える範囲({}〜{} 秒)の外です(実際は {unix_secs} 秒)",
                Timestamp::MIN_UNIX_SECS,
                Timestamp::MAX_UNIX_SECS
            ),
        }
    }
}

/// UTC・秒精度の時刻。
///
/// 生成経路は `from_unix_secs`(`Clock` 実装からの唯一の口)と `parse_rfc3339` の2つだけ。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp {
    unix_secs: i64,
}

impl Timestamp {
    /// 表現可能な最小値(0001-01-01T00:00:00Z)の Unix 秒。
    pub const MIN_UNIX_SECS: i64 = -62_135_596_800;
    /// 表現可能な最大値(9999-12-31T23:59:59Z)の Unix 秒。
    pub const MAX_UNIX_SECS: i64 = 253_402_300_799;
    /// RFC3339 表現の文字数(`YYYY-MM-DDTHH:MM:SSZ`)。
    pub const RFC3339_LENGTH: usize = 20;

    /// Unix 秒(UTC・秒精度)から生成する。範囲外は拒否する。
    ///
    /// 範囲を 0001-01-01〜9999-12-31 に閉じることで、`to_rfc3339` の出力が常に
    /// `parse_rfc3339` に受理される(4桁年を超えると往復が壊れる)。
    pub fn from_unix_secs(unix_secs: i64) -> Result<Self, TimestampError> {
        if !(Self::MIN_UNIX_SECS..=Self::MAX_UNIX_SECS).contains(&unix_secs) {
            return Err(TimestampError::OutOfRange { unix_secs });
        }
        Ok(Self { unix_secs })
    }

    /// Unix 秒(UTC・秒精度)から、表現可能範囲の端へ飽和させて生成する。
    ///
    /// `Clock::now` は無謬なので、壁時計が範囲外を指したときアダプターは値を返すしかない。
    /// 範囲の知識をドメインに置いたまま総関数を1つ用意することで、アダプターが既定値を
    /// 捏造することも、範囲外でパニックすることもなくなる。
    pub fn saturating_from_unix_secs(unix_secs: i64) -> Self {
        Self {
            unix_secs: unix_secs.clamp(Self::MIN_UNIX_SECS, Self::MAX_UNIX_SECS),
        }
    }

    /// RFC3339(`YYYY-MM-DDTHH:MM:SSZ`)としてのみ解釈する。
    ///
    /// オフセット付き表記・サブ秒・うるう秒は受理しない(直列化表現が1つに定まらないと
    /// 往復可能性が壊れる)。
    pub fn parse_rfc3339(s: &str) -> Result<Self, TimestampError> {
        let invalid_format = || TimestampError::InvalidFormat {
            given: s.to_owned(),
        };
        let invalid_date_time = || TimestampError::InvalidDateTime {
            given: s.to_owned(),
        };

        let bytes = s.as_bytes();
        if bytes.len() != Self::RFC3339_LENGTH {
            return Err(invalid_format());
        }
        if bytes[4] != b'-'
            || bytes[7] != b'-'
            || bytes[10] != b'T'
            || bytes[13] != b':'
            || bytes[16] != b':'
            || bytes[19] != b'Z'
        {
            return Err(invalid_format());
        }

        let year = decimal(&bytes[0..4]).ok_or_else(invalid_format)?;
        let month = decimal(&bytes[5..7]).ok_or_else(invalid_format)?;
        let day = decimal(&bytes[8..10]).ok_or_else(invalid_format)?;
        let hour = decimal(&bytes[11..13]).ok_or_else(invalid_format)?;
        let minute = decimal(&bytes[14..16]).ok_or_else(invalid_format)?;
        let second = decimal(&bytes[17..19]).ok_or_else(invalid_format)?;

        if !(1..=12).contains(&month) {
            return Err(invalid_date_time());
        }
        if day < 1 || day > days_in_month(year, month) {
            return Err(invalid_date_time());
        }
        if hour > 23 || minute > 59 || second > 59 {
            return Err(invalid_date_time());
        }

        let unix_secs = days_from_civil(year, month, day) * SECS_PER_DAY
            + hour * 60 * 60
            + minute * 60
            + second;
        Self::from_unix_secs(unix_secs)
    }

    /// RFC3339(`YYYY-MM-DDTHH:MM:SSZ`)へ直列化する。
    pub fn to_rfc3339(&self) -> String {
        let days = self.unix_secs.div_euclid(SECS_PER_DAY);
        let secs_of_day = self.unix_secs.rem_euclid(SECS_PER_DAY);
        let (year, month, day) = civil_from_days(days);
        let hour = secs_of_day / (60 * 60);
        let minute = (secs_of_day % (60 * 60)) / 60;
        let second = secs_of_day % 60;
        format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
    }

    /// Unix 秒。
    pub fn unix_secs(&self) -> i64 {
        self.unix_secs
    }

    /// `earlier` からの経過秒数。
    ///
    /// 壁時計は巻き戻りうる(`Clock` は単調性を要求しない)ため、負の経過は 0 に丸めて
    /// 猶予時間・timeout を過大評価しない。
    pub fn elapsed_since(&self, earlier: &Timestamp) -> u64 {
        let elapsed = self.unix_secs.saturating_sub(earlier.unix_secs);
        u64::try_from(elapsed).unwrap_or(0)
    }
}

/// ASCII 数字だけからなるバイト列を10進数として読む。
fn decimal(bytes: &[u8]) -> Option<i64> {
    let mut value: i64 = 0;
    for byte in bytes {
        if !byte.is_ascii_digit() {
            return None;
        }
        value = value * 10 + i64::from(byte - b'0');
    }
    Some(value)
}

/// 閏年か。
fn is_leap_year(year: i64) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

/// その年月の日数。月が範囲外なら 0(呼び出し側の日付検証で弾かれる)。
fn days_in_month(year: i64, month: i64) -> i64 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

/// 暦日 → 1970-01-01 からの日数。
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let shifted_month = (month + 9) % 12;
    let day_of_year = (153 * shifted_month + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * DAYS_PER_ERA + day_of_era - CIVIL_EPOCH_SHIFT
}

/// 1970-01-01 からの日数 → 暦日。
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let shifted = days + CIVIL_EPOCH_SHIFT;
    let era = shifted.div_euclid(DAYS_PER_ERA);
    let day_of_era = shifted.rem_euclid(DAYS_PER_ERA);
    let year_of_era = (day_of_era - day_of_era / 1_460 + day_of_era / 36_524
        - day_of_era / (DAYS_PER_ERA - 1))
        / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    let year = if month <= 2 { year + 1 } else { year };
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(text: &str) -> Timestamp {
        Timestamp::parse_rfc3339(text).expect("受理される")
    }

    #[test]
    fn rfc3339は往復する() {
        for text in [
            "1970-01-01T00:00:00Z",
            "2026-08-11T09:15:30Z",
            "2026-12-31T23:59:59Z",
            "2024-02-29T12:00:00Z",
        ] {
            assert_eq!(parsed(text).to_rfc3339(), text);
        }
    }

    #[test]
    fn unix秒からの生成とrfc3339は同じ時刻を指す() {
        assert_eq!(
            Timestamp::from_unix_secs(0).expect("受理される"),
            parsed("1970-01-01T00:00:00Z")
        );
        let timestamp = parsed("2026-08-11T09:15:30Z");
        assert_eq!(
            Timestamp::from_unix_secs(timestamp.unix_secs()),
            Ok(timestamp)
        );
    }

    #[test]
    fn epochより前の時刻も往復する() {
        for text in [
            "1969-12-31T23:59:59Z",
            "1969-12-31T00:00:00Z",
            "1900-01-01T00:00:00Z",
            "1900-02-28T23:59:59Z",
        ] {
            assert_eq!(parsed(text).to_rfc3339(), text);
        }
        assert_eq!(parsed("1969-12-31T23:59:59Z").unix_secs(), -1);
    }

    #[test]
    fn 閏年の2月29日は受理され閏年でない年は拒否される() {
        assert_eq!(
            parsed("2024-02-29T00:00:00Z").to_rfc3339(),
            "2024-02-29T00:00:00Z"
        );
        assert_eq!(
            parsed("2000-02-29T00:00:00Z").to_rfc3339(),
            "2000-02-29T00:00:00Z"
        );
        assert_eq!(
            Timestamp::parse_rfc3339("2023-02-29T00:00:00Z"),
            Err(TimestampError::InvalidDateTime {
                given: "2023-02-29T00:00:00Z".to_owned()
            })
        );
        assert_eq!(
            Timestamp::parse_rfc3339("1900-02-29T00:00:00Z"),
            Err(TimestampError::InvalidDateTime {
                given: "1900-02-29T00:00:00Z".to_owned()
            })
        );
    }

    #[test]
    fn 暦上存在しない日付は受理されない() {
        for given in [
            "2026-02-30T00:00:00Z",
            "2026-13-01T00:00:00Z",
            "2026-00-01T00:00:00Z",
            "2026-01-00T00:00:00Z",
            "2026-04-31T00:00:00Z",
        ] {
            assert_eq!(
                Timestamp::parse_rfc3339(given),
                Err(TimestampError::InvalidDateTime {
                    given: given.to_owned()
                }),
                "{given}"
            );
        }
    }

    #[test]
    fn 存在しない時刻は受理されない() {
        for given in [
            "2026-08-11T24:00:00Z",
            "2026-08-11T00:60:00Z",
            "2026-12-31T23:59:60Z",
        ] {
            assert_eq!(
                Timestamp::parse_rfc3339(given),
                Err(TimestampError::InvalidDateTime {
                    given: given.to_owned()
                }),
                "{given}"
            );
        }
    }

    #[test]
    fn 固定形式以外のrfc3339表記は受理されない() {
        for given in [
            "2026-08-11T09:15:30+09:00",
            "2026-08-11T09:15:30.500Z",
            "2026-08-11T09:15:30",
            "2026-08-11 09:15:30Z",
            "2026-08-11t09:15:30z",
            "26-08-11T09:15:30Z",
            "2026-8-11T09:15:30Z",
            "",
        ] {
            assert_eq!(
                Timestamp::parse_rfc3339(given),
                Err(TimestampError::InvalidFormat {
                    given: given.to_owned()
                }),
                "{given}"
            );
        }
    }

    #[test]
    fn 表現可能範囲の両端は受理される() {
        let min = Timestamp::from_unix_secs(Timestamp::MIN_UNIX_SECS).expect("受理される");
        assert_eq!(min.to_rfc3339(), "0001-01-01T00:00:00Z");
        assert_eq!(Timestamp::parse_rfc3339("0001-01-01T00:00:00Z"), Ok(min));

        let max = Timestamp::from_unix_secs(Timestamp::MAX_UNIX_SECS).expect("受理される");
        assert_eq!(max.to_rfc3339(), "9999-12-31T23:59:59Z");
        assert_eq!(Timestamp::parse_rfc3339("9999-12-31T23:59:59Z"), Ok(max));
    }

    #[test]
    fn 表現可能範囲の外は両方の生成経路で拒否される() {
        assert_eq!(
            Timestamp::from_unix_secs(Timestamp::MIN_UNIX_SECS - 1),
            Err(TimestampError::OutOfRange {
                unix_secs: Timestamp::MIN_UNIX_SECS - 1
            })
        );
        assert_eq!(
            Timestamp::from_unix_secs(Timestamp::MAX_UNIX_SECS + 1),
            Err(TimestampError::OutOfRange {
                unix_secs: Timestamp::MAX_UNIX_SECS + 1
            })
        );
        assert_eq!(
            Timestamp::parse_rfc3339("0000-12-31T23:59:59Z"),
            Err(TimestampError::OutOfRange {
                unix_secs: Timestamp::MIN_UNIX_SECS - 1
            })
        );
        assert_eq!(
            Timestamp::parse_rfc3339("10000-01-01T00:00:00Z"),
            Err(TimestampError::InvalidFormat {
                given: "10000-01-01T00:00:00Z".to_owned()
            })
        );
    }

    #[test]
    fn 表現可能範囲の外は端に飽和して生成される() {
        assert_eq!(
            Timestamp::saturating_from_unix_secs(Timestamp::MIN_UNIX_SECS - 1).to_rfc3339(),
            "0001-01-01T00:00:00Z"
        );
        assert_eq!(
            Timestamp::saturating_from_unix_secs(Timestamp::MAX_UNIX_SECS + 1).to_rfc3339(),
            "9999-12-31T23:59:59Z"
        );
        assert_eq!(
            Timestamp::from_unix_secs(0),
            Ok(Timestamp::saturating_from_unix_secs(0))
        );
    }

    #[test]
    fn 時刻は全順序で比較できる() {
        let earlier = parsed("2026-08-11T09:15:30Z");
        let later = parsed("2026-08-11T09:15:31Z");
        assert!(earlier < later);
        assert_eq!(earlier.max(later), later);
    }

    #[test]
    fn 経過秒数は差分として得られる() {
        let earlier = parsed("2026-08-11T09:15:30Z");
        let later = parsed("2026-08-11T09:16:00Z");
        assert_eq!(later.elapsed_since(&earlier), 30);
        assert_eq!(later.elapsed_since(&later), 0);
    }

    #[test]
    fn 巻き戻った時刻からの経過は0に丸められる() {
        let earlier = parsed("2026-08-11T09:15:30Z");
        let later = parsed("2026-08-11T10:00:00Z");
        assert_eq!(earlier.elapsed_since(&later), 0);
    }
}
