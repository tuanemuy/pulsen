//! 期間とタイムアウト指定。

/// 期間の生成失敗。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DurationError {
    /// `<正の10進整数><s|m|h|d>` の形式ではない。
    InvalidFormat {
        /// 与えられた文字列。
        given: String,
    },
    /// 形式は正しいがゼロを指している。
    Zero,
}

impl DurationError {
    /// 制約の説明。
    ///
    /// 同じ制約は config.yaml とワークフロー定義YAMLの両方で破れるため、説明の定義箇所を
    /// ドメインに1つ置く。層ごとに書くと同じ誤りの案内が食い違う。
    pub fn describe(&self) -> String {
        match self {
            Self::InvalidFormat { given } => format!("期間の形式が不正です: {given}"),
            Self::Zero => "期間に 0 は指定できません".to_owned(),
        }
    }
}

/// 秒に正規化された期間。`1m` と `60s` は等価になる。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DurationSpec {
    seconds: u64,
}

impl DurationSpec {
    /// `<正の10進整数><単位>`(単位は `s` / `m` / `h` / `d`)のみを受理する。
    pub fn parse(s: &str) -> Result<Self, DurationError> {
        let invalid = || DurationError::InvalidFormat {
            given: s.to_owned(),
        };

        let (digits, unit) = match s.char_indices().next_back() {
            Some((index, unit)) => (&s[..index], unit),
            None => return Err(invalid()),
        };
        let multiplier = match unit {
            's' => 1,
            'm' => 60,
            'h' => 60 * 60,
            'd' => 24 * 60 * 60,
            _ => return Err(invalid()),
        };
        if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(invalid());
        }

        let value: u64 = digits.parse().map_err(|_| invalid())?;
        if value == 0 {
            return Err(DurationError::Zero);
        }
        let seconds = value.checked_mul(multiplier).ok_or_else(invalid)?;
        Ok(Self { seconds })
    }

    /// 組み込み定数(ADR-014)のための生成口。呼び出し側が非ゼロを保証する。
    pub(crate) const fn from_secs_unchecked(seconds: u64) -> Self {
        Self { seconds }
    }

    /// 正規化された秒数。
    pub fn seconds(&self) -> u64 {
        self.seconds
    }
}

/// タイムアウト指定。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeoutSpec {
    /// 期間による制限。
    Limited(DurationSpec),
    /// 無制限。
    Unlimited,
}

impl TimeoutSpec {
    /// 無制限を表す YAML 上の値。
    pub const UNLIMITED: &'static str = "none";

    /// 期間文字列を `Limited` に、`none` を `Unlimited` に変換する。
    pub fn parse(s: &str) -> Result<Self, DurationError> {
        if s == Self::UNLIMITED {
            return Ok(Self::Unlimited);
        }
        DurationSpec::parse(s).map(Self::Limited)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 期間は秒に正規化される() {
        assert_eq!(
            DurationSpec::parse("30s").expect("受理される").seconds(),
            30
        );
        assert_eq!(DurationSpec::parse("1m").expect("受理される").seconds(), 60);
        assert_eq!(
            DurationSpec::parse("2h").expect("受理される").seconds(),
            7200
        );
        assert_eq!(
            DurationSpec::parse("1d").expect("受理される").seconds(),
            86400
        );
    }

    #[test]
    fn 単位が違っても秒数が同じ期間は等価になる() {
        assert_eq!(DurationSpec::parse("1m"), DurationSpec::parse("60s"));
        assert_eq!(DurationSpec::parse("1h"), DurationSpec::parse("60m"));
    }

    #[test]
    fn ゼロの期間は受理されない() {
        assert_eq!(DurationSpec::parse("0s"), Err(DurationError::Zero));
        assert_eq!(DurationSpec::parse("00m"), Err(DurationError::Zero));
    }

    #[test]
    fn 単位のない期間は受理されない() {
        assert_eq!(
            DurationSpec::parse("30"),
            Err(DurationError::InvalidFormat {
                given: "30".to_owned()
            })
        );
    }

    #[test]
    fn 未知の単位は受理されない() {
        assert_eq!(
            DurationSpec::parse("30x"),
            Err(DurationError::InvalidFormat {
                given: "30x".to_owned()
            })
        );
    }

    #[test]
    fn 数値のない期間は受理されない() {
        assert_eq!(
            DurationSpec::parse("s"),
            Err(DurationError::InvalidFormat {
                given: "s".to_owned()
            })
        );
        assert_eq!(
            DurationSpec::parse(""),
            Err(DurationError::InvalidFormat {
                given: String::new()
            })
        );
    }

    #[test]
    fn 負値と空白混入と小数は受理されない() {
        for given in ["-1s", " 30s", "30s ", "3 0s", "1.5h", "+1s"] {
            assert_eq!(
                DurationSpec::parse(given),
                Err(DurationError::InvalidFormat {
                    given: given.to_owned()
                }),
                "{given}"
            );
        }
    }

    #[test]
    fn 秒数が桁あふれする期間は受理されない() {
        let given = "18446744073709551615d";
        assert_eq!(
            DurationSpec::parse(given),
            Err(DurationError::InvalidFormat {
                given: given.to_owned()
            })
        );
    }

    #[test]
    fn タイムアウトのnoneは無制限になる() {
        assert_eq!(TimeoutSpec::parse("none"), Ok(TimeoutSpec::Unlimited));
    }

    #[test]
    fn タイムアウトの期間文字列は制限つきになる() {
        assert_eq!(
            TimeoutSpec::parse("90s"),
            Ok(TimeoutSpec::Limited(
                DurationSpec::parse("90s").expect("受理される")
            ))
        );
    }

    #[test]
    fn タイムアウトの不正な値は期間のエラーになる() {
        assert_eq!(
            TimeoutSpec::parse("unlimited"),
            Err(DurationError::InvalidFormat {
                given: "unlimited".to_owned()
            })
        );
        assert_eq!(TimeoutSpec::parse("0s"), Err(DurationError::Zero));
    }
}
