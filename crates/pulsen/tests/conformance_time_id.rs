//! Clock / TaskIdGenerator の適合スイートを `SystemClock` / `DefaultTaskIdGenerator` に
//! 適用する。

use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use pulsen::adapter::clock::SystemClock;
use pulsen::adapter::task_id::DefaultTaskIdGenerator;
use pulsen_conformance::{ClockHarness, TaskIdGeneratorHarness};
use pulsen_domain::task::Timestamp;

/// 秒精度の時計で「確実に前進した」と言える待ち時間。
const ADVANCE: Duration = Duration::from_millis(1_100);

/// システム時計を対象にしたハーネス。
struct SystemClockHarness {
    clock: SystemClock,
}

impl SystemClockHarness {
    fn new() -> Self {
        Self {
            clock: SystemClock::new(),
        }
    }
}

impl ClockHarness for SystemClockHarness {
    type Clock = SystemClock;

    fn clock(&self) -> &Self::Clock {
        &self.clock
    }

    fn observe_wall_clock(&self) -> Option<Timestamp> {
        let elapsed = SystemTime::now().duration_since(UNIX_EPOCH).ok()?;
        Timestamp::from_unix_secs(i64::try_from(elapsed.as_secs()).ok()?).ok()
    }

    fn advance(&self) -> Option<()> {
        // システム時計は動かせないため、秒が変わるまで実時間を待つ。
        thread::sleep(ADVANCE);
        Some(())
    }

    // 時刻を過去に設定するにはシステム全体の設定変更が要るため、巻き戻しは提供しない
    // (TC-port-clock-005 はスキップされる)。
}

/// 既定のジェネレーターを対象にしたハーネス。
struct DefaultTaskIdGeneratorHarness {
    generator: DefaultTaskIdGenerator<SystemClock>,
    another: DefaultTaskIdGenerator<SystemClock>,
}

impl DefaultTaskIdGeneratorHarness {
    fn new() -> Self {
        Self {
            generator: DefaultTaskIdGenerator::new(SystemClock::new()).expect("構築できる"),
            another: DefaultTaskIdGenerator::new(SystemClock::new()).expect("構築できる"),
        }
    }
}

impl TaskIdGeneratorHarness for DefaultTaskIdGeneratorHarness {
    type Generator = DefaultTaskIdGenerator<SystemClock>;

    fn generator(&self) -> &Self::Generator {
        &self.generator
    }

    fn another_generator(&self) -> Option<&Self::Generator> {
        Some(&self.another)
    }
}

/// システム時計で許容するスキップ件数。
///
/// 時刻を過去に設定するにはシステム全体の設定変更が要るため、
/// TC-port-clock-005 はどの環境でも走らない。
const ALLOWED_CLOCK_SKIPS: usize = 1;

pulsen_conformance::clock_conformance!(SystemClockHarness::new(), ALLOWED_CLOCK_SKIPS);
pulsen_conformance::task_id_generator_conformance!(DefaultTaskIdGeneratorHarness::new(), 0);
