//! タスクIDを発行する `TaskIdGenerator` の実装。

use std::cell::Cell;

use pulsen_domain::task::{Clock, TaskId, TaskIdError, TaskIdGenerator};

/// ランダム成分の桁数。
///
/// 適合テストは1万回の発行で重複しないことを求める。base36 6桁では誕生日問題で
/// 約2.3%の確率で衝突し、テストがフレーキーになる。8桁(36^8 ≒ 2.8e12)で衝突確率は
/// 1.8e-5 になる。
const RANDOM_LENGTH: usize = 8;
/// base36 の底。
const BASE36: u64 = 36;
/// 発行されるIDの文字数(`yyyymmdd` + `t` + `hhmmss` + `-` + ランダム成分)。
const ID_LENGTH: usize = 8 + 1 + 6 + 1 + RANDOM_LENGTH;

/// ジェネレーターの構築の失敗。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdGeneratorInitError {
    /// エントロピー源から種を取得できない。
    Entropy {
        /// 原因の説明。
        message: String,
    },
    /// 組み立て規則が `TaskId` の制約を満たさない。
    InvalidFormat(TaskIdError),
}

/// `<UTC yyyymmdd>t<hhmmss>-<base36 8桁の乱数>` を発行するジェネレーター。
///
/// 時刻成分は注入した `Clock` の `to_rfc3339()` から導出する — 暦計算はドメインの
/// 1箇所に留め、アダプターで再実装しない。
///
/// `generate` は無謬(spec)なので、失敗しうる処理は構築時に寄せる。
/// エントロピーの取得は `new` の1回だけで、以降は内部 PRNG が進む。
#[derive(Debug)]
pub struct DefaultTaskIdGenerator<C> {
    clock: C,
    state: Cell<u64>,
    verified: TaskId,
}

impl<C: Clock> DefaultTaskIdGenerator<C> {
    /// 時刻成分の取得元を受け取り、乱数の種を1度だけ取る。
    ///
    /// 組み立て規則が `TaskId` の制約を満たすことも、ここで一度検証する。
    pub fn new(clock: C) -> Result<Self, IdGeneratorInitError> {
        let seed = getrandom::u64().map_err(|error| IdGeneratorInitError::Entropy {
            message: error.to_string(),
        })?;
        let state = Cell::new(seed);
        let verified =
            TaskId::parse(compose(&clock, &state)).map_err(IdGeneratorInitError::InvalidFormat)?;
        Ok(Self {
            clock,
            state,
            verified,
        })
    }
}

impl<C: Clock> TaskIdGenerator for DefaultTaskIdGenerator<C> {
    fn generate(&self) -> TaskId {
        match TaskId::parse(compose(&self.clock, &self.state)) {
            Ok(id) => id,
            // 組み立て規則は文字集合制約を常に満たすため到達しない。無謬の契約を守るため
            // パニックさせず、構築時に検証したIDを返す(重複は `create` の `Conflict` が拾う)。
            Err(
                TaskIdError::Empty
                | TaskIdError::TooLong
                | TaskIdError::InvalidChar { .. }
                | TaskIdError::InvalidLeadingChar,
            ) => self.verified.clone(),
        }
    }
}

/// 時刻成分とランダム成分からIDの文字列を組み立てる。
///
/// 時刻成分は RFC3339 表現から区切り(`-` / `:` / `Z`)を落とし、`T` を小文字にした
/// ものになる。残る文字は `[0-9t]` だけで、ランダム成分は `[0-9a-z]` の8文字なので、
/// 結果は常に `TaskId` の制約(`[a-z0-9-]`・先頭英数字・64文字以内)を満たす。
fn compose(clock: &impl Clock, state: &Cell<u64>) -> String {
    let mut id = String::with_capacity(ID_LENGTH);
    for found in clock.now().to_rfc3339().chars() {
        match found {
            '0'..='9' => id.push(found),
            'T' => id.push('t'),
            _ => {}
        }
    }
    // 時刻成分が空になる表現はないが、空なら先頭が `-` になって制約を破るため区切りも落とす。
    if !id.is_empty() {
        id.push('-');
    }
    let mut random = next_random(state) % BASE36.pow(RANDOM_LENGTH as u32);
    let mut digits = ['0'; RANDOM_LENGTH];
    for digit in digits.iter_mut().rev() {
        *digit = base36_digit(random % BASE36);
        random /= BASE36;
    }
    id.extend(digits);
    id
}

/// base36 の1桁。
fn base36_digit(value: u64) -> char {
    let value = (value % BASE36) as u8;
    if value < 10 {
        char::from(b'0' + value)
    } else {
        char::from(b'a' + value - 10)
    }
}

/// SplitMix64 で内部状態を1つ進める。
///
/// 発行のたびにエントロピー源へ問い合わせないための最小の疑似乱数列。
fn next_random(state: &Cell<u64>) -> u64 {
    let next = state.get().wrapping_add(0x9E37_79B9_7F4A_7C15);
    state.set(next);
    let mut value = next;
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use pulsen_conformance::doubles::FixedClock;
    use pulsen_domain::task::Timestamp;

    use super::*;

    fn now() -> Timestamp {
        Timestamp::parse_rfc3339("2026-08-11T09:15:30Z").expect("受理される")
    }

    fn generator() -> DefaultTaskIdGenerator<FixedClock> {
        DefaultTaskIdGenerator::new(FixedClock::new(now())).expect("構築できる")
    }

    #[test]
    fn 発行されるidは時刻成分とランダム成分から成る() {
        let id = generator().generate();

        let (stamp, random) = id.as_str().split_once('-').expect("区切りを持つ");
        assert_eq!(stamp, "20260811t091530");
        assert_eq!(random.len(), RANDOM_LENGTH);
        assert!(
            random
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        );
    }

    #[test]
    fn 同じ時刻で連続して発行してもidは重複しない() {
        let generator = generator();

        let issued: HashSet<String> = (0..1_000)
            .map(|_| generator.generate().into_string())
            .collect();

        assert_eq!(issued.len(), 1_000);
    }

    #[test]
    fn 組み立て規則はどのランダム成分でもタスクidの制約を満たす文字列を作る() {
        // `generate` は制約を満たさない文字列を構築時のIDに読み替えるため、組み立て規則が
        // 制約を破っても発行されたIDからは分からない。組み立てそのものを検証する。
        let clock = FixedClock::new(now());
        let state = Cell::new(0x0123_4567_89ab_cdef);

        for _ in 0..1_000 {
            let composed = compose(&clock, &state);
            assert!(
                TaskId::parse(composed.clone()).is_ok(),
                "組み立てた {composed} はタスクIDとして受理される"
            );
        }
    }
}
