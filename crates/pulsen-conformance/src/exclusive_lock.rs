//! ExclusiveLock の適合ケース(`spec/testcases/ports/exclusive-lock.md` の7行)。
//!
//! 排他の単位はプロセス間なので、保持側は必ずハーネスのフックが用意する別プロセス
//! (または別ハンドル)にする。同一プロセス内の再取得は契約の対象外。

use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use pulsen_domain::execution::{ExclusiveLock, LockError};

use crate::{CaseOutcome, ExclusiveLockHarness, require};

/// 「ブロックしない」とみなす上限。解放を待つ実装は保持が続く限り返らない。
const NON_BLOCKING: Duration = Duration::from_secs(5);

/// 別スレッドで試みた取得の結果。
///
/// ガードはスレッドの中で落とす。`LockGuard` は `Send` を要求しないため、値のまま
/// スレッド境界を越えられない。
enum Attempt {
    /// 取得できた。
    Acquired,
    /// 競合して取得できなかった。
    Contended,
    /// 機構の異常。
    Failed(String),
}

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

/// 保持中の取得を別スレッドで試み、期限の監視をこのスレッドに残す。
///
/// 同じスレッドで試みると、解放を待つ実装では `try_acquire` が返るまで判定に到達せず、
/// スイート全体がハングする。期限を超えたら先に保持を解放して試行を返らせ、待ったことを
/// 呼び出し側へ返す。`Lock: Sync` はこの経路を通るケースだけの要求で、ハーネス全体には
/// 課さない(ADR-060)。
///
/// 戻り値の `Option<Holder>` は、期限内に返って保持が続いている場合だけ `Some`。
fn attempt_while_held<Harness>(
    harness: &Harness,
    holder: Harness::Holder,
) -> (Attempt, bool, Option<Harness::Holder>)
where
    Harness: ExclusiveLockHarness,
    Harness::Lock: Sync,
{
    let mut holder = Some(holder);
    let lock = harness.lock();
    let returned = AtomicBool::new(false);

    let (attempt, waited) = thread::scope(|scope| {
        let trying = scope.spawn(|| {
            let attempt = match lock.try_acquire() {
                Ok(Some(_guard)) => Attempt::Acquired,
                Ok(None) => Attempt::Contended,
                Err(LockError::Failed { message }) => Attempt::Failed(message),
            };
            returned.store(true, Ordering::Release);
            attempt
        });

        let deadline = Instant::now() + NON_BLOCKING;
        while !returned.load(Ordering::Acquire) && Instant::now() < deadline {
            thread::yield_now();
        }

        let waited = !returned.load(Ordering::Acquire);
        if waited {
            // 解放しないと、待つ実装ではスレッドの合流も返らない。
            let _ = harness.release_holder(holder.take().expect("保持している"));
        }
        (
            trying.join().expect("取得の試行がパニックしていない"),
            waited,
        )
    });

    (attempt, waited, holder)
}

pub fn tc_port_exclusive_lock_002_別プロセスの保持中は取得できない<Harness>(
    harness: &Harness,
) -> CaseOutcome
where
    Harness: ExclusiveLockHarness,
    Harness::Lock: Sync,
{
    let holder = require!(harness.hold_from_other_process());

    let (attempt, waited, holder) = attempt_while_held(harness, holder);

    assert!(!waited, "競合した取得が期限内に返る");
    match attempt {
        Attempt::Contended => {}
        Attempt::Acquired => panic!("保持中のロックが取得できた"),
        Attempt::Failed(message) => {
            panic!("競合はエラーではなく取得できなかったこととして返る: {message}")
        }
    }
    let holder = holder.expect("期限内に返ったので保持は続いている");
    assert!(harness.release_holder(holder).is_some(), "保持を解放できる");
    CaseOutcome::Ran
}

pub fn tc_port_exclusive_lock_003_保持中の取得は待たずに返る<Harness>(
    harness: &Harness,
) -> CaseOutcome
where
    Harness: ExclusiveLockHarness,
    Harness::Lock: Sync,
{
    let holder = require!(harness.hold_from_other_process());

    let (attempt, waited, holder) = attempt_while_held(harness, holder);

    assert!(!waited, "保持の解放を待たずに返る");
    match attempt {
        Attempt::Contended => {}
        Attempt::Acquired => panic!("保持中のロックが取得できた"),
        Attempt::Failed(message) => panic!("競合はエラーではない: {message}"),
    }
    let holder = holder.expect("期限内に返ったので保持は続いている");
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
/// この環境でスキップを許容するケース(TC ID)の集合で、集合の外のスキップはその
/// ケースの失敗になる。
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
