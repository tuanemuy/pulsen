//! Clock の適合ケース(`spec/testcases/ports/clock.md` の5行)。
//!
//! 契約は「壁時計の現在時刻を返す」ことだけで、単調性は要求しない。巻き戻りの扱い
//! (経過を0に丸める)は `Timestamp` の規則であり、ここでは検証しない。

use pulsen_domain::task::{Clock, Timestamp};

use crate::{CaseOutcome, ClockHarness, require};

pub fn tc_port_clock_001_現在時刻は秒精度のutcで返る(
    harness: &impl ClockHarness,
) -> CaseOutcome {
    let observed = harness.clock().now();

    let text = observed.to_rfc3339();
    assert_eq!(text.len(), Timestamp::RFC3339_LENGTH, "{text}");
    assert!(text.ends_with('Z'), "{text}");
    assert!(!text.contains('.'), "サブ秒成分を持たない: {text}");
    assert_eq!(
        Timestamp::from_unix_secs(observed.unix_secs()),
        Ok(observed),
        "秒だけで表現できる"
    );
    CaseOutcome::Ran
}

pub fn tc_port_clock_002_返値は直列化表現と往復する(
    harness: &impl ClockHarness,
) -> CaseOutcome {
    let observed = harness.clock().now();

    let restored = Timestamp::parse_rfc3339(&observed.to_rfc3339());

    assert_eq!(restored, Ok(observed));
    CaseOutcome::Ran
}

pub fn tc_port_clock_003_返値は前後に観測した実時刻の範囲に収まる(
    harness: &impl ClockHarness,
) -> CaseOutcome {
    let before = require!(harness.observe_wall_clock());

    let observed = harness.clock().now();

    let after = harness
        .observe_wall_clock()
        .expect("前提条件を用意できた環境では観測を続けられる");
    assert!(before <= observed, "{before:?} <= {observed:?}");
    assert!(observed <= after, "{observed:?} <= {after:?}");
    CaseOutcome::Ran
}

pub fn tc_port_clock_004_時刻が進めば後の値が先の値より後になる(
    harness: &impl ClockHarness,
) -> CaseOutcome {
    let earlier = harness.clock().now();
    require!(harness.advance());

    let later = harness.clock().now();

    assert!(earlier < later, "{earlier:?} < {later:?}");
    assert!(later.elapsed_since(&earlier) >= 1);
    CaseOutcome::Ran
}

pub fn tc_port_clock_005_巻き戻した時刻はそのまま返る(
    harness: &impl ClockHarness,
) -> CaseOutcome {
    let earlier = harness.clock().now();
    require!(harness.rewind());

    let rewound = harness.clock().now();

    assert!(rewound < earlier, "{rewound:?} < {earlier:?}");
    CaseOutcome::Ran
}

/// Clock の適合スイートをアダプターに適用する。
///
/// `$setup` はケースごとに評価され、ハーネスは共有されない。`$allowed_skips` は
/// この環境で許容するスキップ件数で、超えたスキップはケースの失敗になる。
#[macro_export]
macro_rules! clock_conformance {
    ($setup:expr, $allowed_skips:expr) => {
        use $crate::clock as __pulsen_conformance_clock;

        $crate::conformance_cases!(
            __pulsen_conformance_clock,
            $setup,
            __PULSEN_CONFORMANCE_CLOCK_SKIPS = $allowed_skips,
            [
                tc_port_clock_001_現在時刻は秒精度のutcで返る,
                tc_port_clock_002_返値は直列化表現と往復する,
                tc_port_clock_003_返値は前後に観測した実時刻の範囲に収まる,
                tc_port_clock_004_時刻が進めば後の値が先の値より後になる,
                tc_port_clock_005_巻き戻した時刻はそのまま返る,
            ]
        );
    };
}
