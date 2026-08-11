//! ExclusiveLock の適合ケース(`spec/testcases/ports/exclusive-lock.md` の7行)。
//!
//! 排他の単位はプロセス間なので、保持側は必ずハーネスのフックが用意する別プロセス
//! (または別ハンドル)にする。同一プロセス内の再取得は契約の対象外。

use std::time::{Duration, Instant};

use pulsen_domain::execution::{ExclusiveLock, LockError};

use crate::{CaseOutcome, ExclusiveLockHarness, require};

/// 「ブロックしない」とみなす上限。解放を待つ実装は保持が続く限り返らない。
const NON_BLOCKING: Duration = Duration::from_secs(5);

pub fn tc_port_exclusive_lock_001_誰も保持していなければ取得できる(
    harness: &impl ExclusiveLockHarness,
) -> CaseOutcome {
    let acquired = harness.lock().try_acquire();

    match acquired {
        Ok(Some(_guard)) => {}
        Ok(None) => panic!("誰も保持していないロックが取得できなかった"),
        Err(LockError::Failed { message }) => panic!("ロックの取得に失敗した: {message}"),
    }
    CaseOutcome::Ran
}

pub fn tc_port_exclusive_lock_002_別プロセスの保持中は取得できない(
    harness: &impl ExclusiveLockHarness,
) -> CaseOutcome {
    let holder = require!(harness.hold_from_other_process());

    let acquired = harness.lock().try_acquire();

    assert!(
        matches!(acquired, Ok(None)),
        "競合はエラーではなく取得できなかったこととして返る"
    );
    assert!(harness.release_holder(holder).is_some(), "保持を解放できる");
    CaseOutcome::Ran
}

pub fn tc_port_exclusive_lock_003_保持中の取得は待たずに返る(
    harness: &impl ExclusiveLockHarness,
) -> CaseOutcome {
    let holder = require!(harness.hold_from_other_process());

    let started = Instant::now();
    let acquired = harness.lock().try_acquire();
    let elapsed = started.elapsed();

    assert!(matches!(acquired, Ok(None)));
    assert!(elapsed < NON_BLOCKING, "解放を待たずに返る: {elapsed:?}");
    assert!(harness.release_holder(holder).is_some(), "保持を解放できる");
    CaseOutcome::Ran
}

pub fn tc_port_exclusive_lock_004_ガードのドロップで別プロセスが取得できる(
    harness: &impl ExclusiveLockHarness,
) -> CaseOutcome {
    let guard = harness.lock().try_acquire();
    assert!(matches!(&guard, Ok(Some(_))));
    drop(guard);

    let acquired = require!(harness.try_acquire_from_other_process());

    assert!(acquired, "ドロップで解放され、別プロセスが取得できる");
    CaseOutcome::Ran
}

pub fn tc_port_exclusive_lock_005_保持プロセスの強制終了後は取得できる(
    harness: &impl ExclusiveLockHarness,
) -> CaseOutcome {
    let holder = require!(harness.hold_from_other_process());
    require!(harness.kill_holder(holder));

    let acquired = harness.lock().try_acquire();

    assert!(
        matches!(acquired, Ok(Some(_))),
        "解放処理を実行しない終了でもロックは解放される"
    );
    CaseOutcome::Ran
}

pub fn tc_port_exclusive_lock_006_別のホームの保持には影響されない(
    harness: &impl ExclusiveLockHarness,
) -> CaseOutcome {
    let separate = require!(harness.separate_home());
    let held = separate.try_acquire();
    assert!(matches!(&held, Ok(Some(_))), "別ホームのロックを保持できる");

    let acquired = harness.lock().try_acquire();

    assert!(
        matches!(acquired, Ok(Some(_))),
        "ロックはグローバルホームごとに1つになる"
    );
    drop(held);
    CaseOutcome::Ran
}

pub fn tc_port_exclusive_lock_007_ロック機構が使えなければ失敗になる(
    harness: &impl ExclusiveLockHarness,
) -> CaseOutcome {
    let unusable = require!(harness.unusable_lock());

    let acquired = unusable.try_acquire();

    match acquired {
        Err(LockError::Failed { message }) => assert!(!message.is_empty(), "原因が説明される"),
        Ok(None) => panic!("機構の異常は「取得できなかった」と区別される"),
        Ok(Some(_)) => panic!("使えない機構でロックが取得できた"),
    }
    CaseOutcome::Ran
}

/// ExclusiveLock の適合スイートをアダプターに適用する。
///
/// `$setup` はケースごとに評価され、ハーネスは共有されない。`$allowed_skips` は
/// この環境で許容するスキップ件数で、超えたスキップはケースの失敗になる。
#[macro_export]
macro_rules! exclusive_lock_conformance {
    ($setup:expr, $allowed_skips:expr) => {
        use $crate::exclusive_lock as __pulsen_conformance_exclusive_lock;

        $crate::conformance_cases!(
            __pulsen_conformance_exclusive_lock,
            $setup,
            __PULSEN_CONFORMANCE_EXCLUSIVE_LOCK_SKIPS = $allowed_skips,
            [
                tc_port_exclusive_lock_001_誰も保持していなければ取得できる,
                tc_port_exclusive_lock_002_別プロセスの保持中は取得できない,
                tc_port_exclusive_lock_003_保持中の取得は待たずに返る,
                tc_port_exclusive_lock_004_ガードのドロップで別プロセスが取得できる,
                tc_port_exclusive_lock_005_保持プロセスの強制終了後は取得できる,
                tc_port_exclusive_lock_006_別のホームの保持には影響されない,
                tc_port_exclusive_lock_007_ロック機構が使えなければ失敗になる,
            ]
        );
    };
}
