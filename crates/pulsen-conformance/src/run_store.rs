//! RunStore の適合ケース(`spec/testcases/ports/run-store.md` のうち、本スライスで
//! 使う10メソッド分の23行)と、write 系のディレクトリ作成の追加1件。
//!
//! `list_runs` / `delete_attempt` / `remove_task_dir_if_empty` の11行は、それらを
//! ポートに足すスライスで扱う。
//!
//! 1行 = 1ケース関数 = 1 `#[test]`。run ディレクトリのファイル配置は契約の語彙
//! (`RunDirPath` の導出関数)で指し、置き場の実体はハーネスのフックに委ねる。

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use pulsen_domain::execution::{ExitCode, Io, PidFileContent, RunFileError, RunStore};
use pulsen_domain::task::{
    AttemptNumber, KillIdent, Pid, ProcessStartTime, RunDirPath, StartTimeRecord, TaskId, Timestamp,
};

use crate::{CaseOutcome, RunFileKind, RunStoreHarness, require};

pub fn tc_port_run_store_001_準備でattemptディレクトリが親ごと作られる(
    harness: &impl RunStoreHarness,
) -> CaseOutcome {
    let id = task_id("task-a");
    let expected = require!(harness.expected_run_dir(&id, AttemptNumber::FIRST));
    let before = require!(harness.attempt_dir_present(&expected));
    assert!(!before, "準備の前は attempt ディレクトリが無い");

    let run_dir = harness
        .store()
        .prepare_attempt(&id, AttemptNumber::FIRST)
        .expect("attempt を用意できる");

    assert_eq!(run_dir, expected, "返るパスは導出結果と一致する");
    // 前半でフックが `Some` を返した以上、後半の `None` は環境の制約ではなく実装の失敗。
    // ここで `require!` を使うとその失敗がスキップに化ける(ADR-084)。
    assert_eq!(
        harness.attempt_dir_present(&run_dir),
        Some(true),
        "準備の後は attempt ディレクトリが在る"
    );
    assert_eq!(
        harness.store().attempt_exists(&run_dir),
        Ok(true),
        "準備の後は attempt_exists が true になる"
    );
    CaseOutcome::Ran
}

pub fn tc_port_run_store_002_準備は冪等で書き込み済みの内容に影響しない(
    harness: &impl RunStoreHarness,
) -> CaseOutcome {
    let id = task_id("task-a");
    let store = harness.store();
    let run_dir = prepare(harness, &id);
    store
        .write_pid_file(&run_dir, &pid_content(4242, "-4242"))
        .expect("pid を書ける");
    store
        .write_starttime(&run_dir, &starttime("871234"))
        .expect("starttime を書ける");
    store
        .write_exit(&run_dir, &ExitCode::new(7))
        .expect("exit を書ける");

    let again = store
        .prepare_attempt(&id, AttemptNumber::FIRST)
        .expect("再度の準備も成功する");

    assert_eq!(again, run_dir);
    assert_eq!(
        store.read_pid_file(&run_dir),
        Ok(Some(pid_content(4242, "-4242")))
    );
    assert_eq!(
        store.read_starttime(&run_dir),
        Ok(Some(starttime("871234")))
    );
    assert_eq!(store.read_exit(&run_dir), Ok(Some(ExitCode::new(7))));
    CaseOutcome::Ran
}

pub fn tc_port_run_store_003_pidファイル未書き込みは不在になる(
    harness: &impl RunStoreHarness,
) -> CaseOutcome {
    let run_dir = prepare(harness, &task_id("task-a"));

    assert_eq!(harness.store().read_pid_file(&run_dir), Ok(None));
    CaseOutcome::Ran
}

pub fn tc_port_run_store_004_attemptディレクトリ不在のpid読み取りは不在になる(
    harness: &impl RunStoreHarness,
) -> CaseOutcome {
    let run_dir = require!(harness.expected_run_dir(&task_id("task-a"), AttemptNumber::FIRST));

    assert_eq!(harness.store().read_pid_file(&run_dir), Ok(None));
    CaseOutcome::Ran
}

pub fn tc_port_run_store_005_pidファイルは書いた値のまま読み戻せる(
    harness: &impl RunStoreHarness,
) -> CaseOutcome {
    let run_dir = prepare(harness, &task_id("task-a"));
    let content = pid_content(4242, "-4242");

    harness
        .store()
        .write_pid_file(&run_dir, &content)
        .expect("pid を書ける");

    assert_eq!(harness.store().read_pid_file(&run_dir), Ok(Some(content)));
    CaseOutcome::Ran
}

pub fn tc_port_run_store_006_解釈不能なpidファイルは破損として返る(
    harness: &impl RunStoreHarness,
) -> CaseOutcome {
    let run_dir = prepare(harness, &task_id("task-a"));
    require!(harness.put_unreadable_content(&run_dir, RunFileKind::Pid));

    assert_corrupt(harness.store().read_pid_file(&run_dir), &run_dir.pid_file());
    CaseOutcome::Ran
}

pub fn tc_port_run_store_007_pidファイルの読み取り失敗は機構の失敗として返る(
    harness: &impl RunStoreHarness,
) -> CaseOutcome {
    let run_dir = prepare(harness, &task_id("task-a"));
    harness
        .store()
        .write_pid_file(&run_dir, &pid_content(4242, "-4242"))
        .expect("pid を書ける");
    let _restore = require!(harness.make_unreadable(&run_dir, RunFileKind::Pid));

    match harness.store().read_pid_file(&run_dir) {
        Err(RunFileError::Io { message }) => assert!(!message.is_empty(), "原因が説明される"),
        other => panic!("読み取れない状況では機構の失敗になる: {other:?}"),
    }
    CaseOutcome::Ran
}

pub fn tc_port_run_store_008_starttimeファイル未書き込みは不在になる(
    harness: &impl RunStoreHarness,
) -> CaseOutcome {
    let run_dir = prepare(harness, &task_id("task-a"));

    assert_eq!(harness.store().read_starttime(&run_dir), Ok(None));
    CaseOutcome::Ran
}

pub fn tc_port_run_store_009_attemptディレクトリ不在のstarttime読み取りは不在になる(
    harness: &impl RunStoreHarness,
) -> CaseOutcome {
    let run_dir = require!(harness.expected_run_dir(&task_id("task-a"), AttemptNumber::FIRST));

    assert_eq!(harness.store().read_starttime(&run_dir), Ok(None));
    CaseOutcome::Ran
}

pub fn tc_port_run_store_010_starttimeファイルは書いた値のまま読み戻せる(
    harness: &impl RunStoreHarness,
) -> CaseOutcome {
    let run_dir = prepare(harness, &task_id("task-a"));
    let record = starttime("871234");

    harness
        .store()
        .write_starttime(&run_dir, &record)
        .expect("starttime を書ける");

    assert_eq!(harness.store().read_starttime(&run_dir), Ok(Some(record)));
    CaseOutcome::Ran
}

pub fn tc_port_run_store_011_解釈不能なstarttimeファイルは破損として返る(
    harness: &impl RunStoreHarness,
) -> CaseOutcome {
    let run_dir = prepare(harness, &task_id("task-a"));
    require!(harness.put_unreadable_content(&run_dir, RunFileKind::StartTime));

    assert_corrupt(
        harness.store().read_starttime(&run_dir),
        &run_dir.starttime_file(),
    );
    CaseOutcome::Ran
}

pub fn tc_port_run_store_012_exitファイル未書き込みは不在になる(
    harness: &impl RunStoreHarness,
) -> CaseOutcome {
    let run_dir = prepare(harness, &task_id("task-a"));

    assert_eq!(harness.store().read_exit(&run_dir), Ok(None));
    CaseOutcome::Ran
}

pub fn tc_port_run_store_013_attemptディレクトリ不在のexit読み取りは不在になる(
    harness: &impl RunStoreHarness,
) -> CaseOutcome {
    let run_dir = require!(harness.expected_run_dir(&task_id("task-a"), AttemptNumber::FIRST));

    assert_eq!(harness.store().read_exit(&run_dir), Ok(None));
    CaseOutcome::Ran
}

pub fn tc_port_run_store_014_exitファイルは書いた値のまま読み戻せる(
    harness: &impl RunStoreHarness,
) -> CaseOutcome {
    let run_dir = prepare(harness, &task_id("task-a"));

    for code in [ExitCode::new(0), ExitCode::new(134)] {
        harness
            .store()
            .write_exit(&run_dir, &code)
            .expect("exit を書ける");

        assert_eq!(harness.store().read_exit(&run_dir), Ok(Some(code)));
    }
    CaseOutcome::Ran
}

pub fn tc_port_run_store_015_解釈不能なexitファイルは破損として返る(
    harness: &impl RunStoreHarness,
) -> CaseOutcome {
    let run_dir = prepare(harness, &task_id("task-a"));
    require!(harness.put_unreadable_content(&run_dir, RunFileKind::Exit));

    assert_corrupt(harness.store().read_exit(&run_dir), &run_dir.exit_file());
    CaseOutcome::Ran
}

pub fn tc_port_run_store_016_書き込みと並行する読み取りは完全な値だけを観測する(
    harness: &impl RunStoreHarness,
) -> CaseOutcome {
    let store = require!(harness.concurrent_store());
    let run_dir = store
        .prepare_attempt(&task_id("task-a"), AttemptNumber::FIRST)
        .expect("attempt を用意できる");

    let first = pid_content(4242, "-4242");
    let second = pid_content(9, "-9");
    assert_concurrent_reads(
        |round| {
            let content = if round == 0 { &first } else { &second };
            store
                .write_pid_file(&run_dir, content)
                .expect("pid を書ける");
        },
        || match store.read_pid_file(&run_dir) {
            Ok(None) => {}
            Ok(Some(observed)) => assert!(
                observed == first || observed == second,
                "書きかけの pid を観測した: {observed:?}"
            ),
            other => panic!("不在か完全な値だけが観測される: {other:?}"),
        },
    );

    let early = starttime("871234");
    let late = starttime("999999999999999999999999");
    assert_concurrent_reads(
        |round| {
            let record = if round == 0 { &early } else { &late };
            store
                .write_starttime(&run_dir, record)
                .expect("starttime を書ける");
        },
        || match store.read_starttime(&run_dir) {
            Ok(None) => {}
            Ok(Some(observed)) => assert!(
                observed == early || observed == late,
                "書きかけの starttime を観測した: {observed:?}"
            ),
            other => panic!("不在か完全な値だけが観測される: {other:?}"),
        },
    );

    let success = ExitCode::new(0);
    let failure = ExitCode::new(134);
    assert_concurrent_reads(
        |round| {
            let code = if round == 0 { &success } else { &failure };
            store.write_exit(&run_dir, code).expect("exit を書ける");
        },
        || match store.read_exit(&run_dir) {
            Ok(None) => {}
            Ok(Some(observed)) => assert!(
                observed == success || observed == failure,
                "書きかけの exit を観測した: {observed:?}"
            ),
            other => panic!("不在か完全な値だけが観測される: {other:?}"),
        },
    );
    CaseOutcome::Ran
}

pub fn tc_port_run_store_017_失敗した書き込みは部分的な内容を残さない(
    harness: &impl RunStoreHarness,
) -> CaseOutcome {
    let store = harness.store();
    let run_dir = prepare(harness, &task_id("task-a"));
    let recorded = pid_content(4242, "-4242");
    store
        .write_pid_file(&run_dir, &recorded)
        .expect("pid を書ける");
    store
        .write_starttime(&run_dir, &starttime("871234"))
        .expect("starttime を書ける");
    let _restore = require!(harness.make_attempt_unwritable(&run_dir));

    assert_write_failed(store.write_pid_file(&run_dir, &pid_content(9, "-9")));
    assert_write_failed(store.write_starttime(&run_dir, &starttime("999")));
    assert_write_failed(store.write_exit(&run_dir, &ExitCode::new(7)));

    // 従前の完全な値か不在のどちらかであり、書きかけの残骸(`Corrupt`)にはならない。
    assert_eq!(store.read_pid_file(&run_dir), Ok(Some(recorded)));
    assert_eq!(
        store.read_starttime(&run_dir),
        Ok(Some(starttime("871234")))
    );
    assert_eq!(store.read_exit(&run_dir), Ok(None));
    CaseOutcome::Ran
}

pub fn tc_port_run_store_018_マーカーはattemptディレクトリごと作成して書かれる(
    harness: &impl RunStoreHarness,
) -> CaseOutcome {
    let run_dir = require!(harness.expected_run_dir(&task_id("task-a"), AttemptNumber::FIRST));

    harness
        .store()
        .write_invalidation_marker(&run_dir)
        .expect("マーカーを書ける");

    assert_eq!(harness.store().marker_exists(&run_dir), Ok(true));
    // ディレクトリごと作られたことは環境に問う。観測できない実装でも行の主張
    // (マーカーが書かれる)は上で成立するため、この分岐だけを飛ばす。
    if let Some(present) = harness.attempt_dir_present(&run_dir) {
        assert!(present, "attempt ディレクトリごと作られる");
    }
    CaseOutcome::Ran
}

pub fn tc_port_run_store_019_マーカーの書き込みは冪等(
    harness: &impl RunStoreHarness,
) -> CaseOutcome {
    let run_dir = prepare(harness, &task_id("task-a"));
    harness
        .store()
        .write_invalidation_marker(&run_dir)
        .expect("マーカーを書ける");

    harness
        .store()
        .write_invalidation_marker(&run_dir)
        .expect("再度の書き込みも成功する");

    assert_eq!(harness.store().marker_exists(&run_dir), Ok(true));
    CaseOutcome::Ran
}

pub fn tc_port_run_store_020_マーカー未書き込みは偽になる(
    harness: &impl RunStoreHarness,
) -> CaseOutcome {
    let run_dir = prepare(harness, &task_id("task-a"));

    assert_eq!(harness.store().marker_exists(&run_dir), Ok(false));
    CaseOutcome::Ran
}

pub fn tc_port_run_store_021_マーカー書き込み済みは真になる(
    harness: &impl RunStoreHarness,
) -> CaseOutcome {
    let run_dir = prepare(harness, &task_id("task-a"));

    harness
        .store()
        .write_invalidation_marker(&run_dir)
        .expect("マーカーを書ける");

    assert_eq!(harness.store().marker_exists(&run_dir), Ok(true));
    CaseOutcome::Ran
}

pub fn tc_port_run_store_022_ファイルのない準備済みattemptは存在する(
    harness: &impl RunStoreHarness,
) -> CaseOutcome {
    // ファイルを1つも書かない。read 系はいずれも `Ok(None)` を返す状態で、
    // 「空ディレクトリ」と「ディレクトリごと不在」の区別がここでしか付かない。
    let run_dir = prepare(harness, &task_id("task-a"));

    assert_eq!(harness.store().read_pid_file(&run_dir), Ok(None));
    assert_eq!(harness.store().attempt_exists(&run_dir), Ok(true));
    CaseOutcome::Ran
}

pub fn tc_port_run_store_023_attemptディレクトリ不在は存在しない(
    harness: &impl RunStoreHarness,
) -> CaseOutcome {
    let run_dir = require!(harness.expected_run_dir(&task_id("task-a"), AttemptNumber::FIRST));

    assert_eq!(harness.store().attempt_exists(&run_dir), Ok(false));
    CaseOutcome::Ran
}

/// 台帳行に対応しない追加ケース。
///
/// 「write 系はいずれも書き込み先のディレクトリを必要に応じて作る」はポートの契約で、
/// `prepare_attempt` が失敗しても起動を続ける経路がこれに乗っている。台帳行が主張するのは
/// マーカー(TC-018)の分だけで、残る3つの write 系には対応する行が無い。
pub fn write_準備を経ない書き込みも置き場ごと作って残る(
    harness: &impl RunStoreHarness,
) -> CaseOutcome {
    let store = harness.store();

    let starttime_dir =
        require!(harness.expected_run_dir(&task_id("task-a"), AttemptNumber::FIRST));
    assert_attempt_dir_absent(harness, &starttime_dir);
    let record = starttime("871234");
    store
        .write_starttime(&starttime_dir, &record)
        .expect("starttime を書ける");
    assert_eq!(store.read_starttime(&starttime_dir), Ok(Some(record)));

    let pid_dir = require!(harness.expected_run_dir(&task_id("task-b"), AttemptNumber::FIRST));
    assert_attempt_dir_absent(harness, &pid_dir);
    let content = pid_content(4242, "-4242");
    store
        .write_pid_file(&pid_dir, &content)
        .expect("pid を書ける");
    assert_eq!(store.read_pid_file(&pid_dir), Ok(Some(content)));

    let exit_dir = require!(harness.expected_run_dir(&task_id("task-c"), AttemptNumber::FIRST));
    assert_attempt_dir_absent(harness, &exit_dir);
    store
        .write_exit(&exit_dir, &ExitCode::new(7))
        .expect("exit を書ける");
    assert_eq!(store.read_exit(&exit_dir), Ok(Some(ExitCode::new(7))));
    CaseOutcome::Ran
}

/// 書き込みの前に attempt ディレクトリが無いことを確かめる。
///
/// 観測できない実装でもケースの主張(書いた値が読み戻せる)は成立するため、この分岐だけを
/// 飛ばす(TC-018 と同じ扱い)。
fn assert_attempt_dir_absent(harness: &impl RunStoreHarness, run_dir: &RunDirPath) {
    if let Some(present) = harness.attempt_dir_present(run_dir) {
        assert!(!present, "書き込みの前は attempt ディレクトリが無い");
    }
}

/// 最初の attempt を用意する。
fn prepare(harness: &impl RunStoreHarness, id: &TaskId) -> RunDirPath {
    harness
        .store()
        .prepare_attempt(id, AttemptNumber::FIRST)
        .expect("attempt を用意できる")
}

fn task_id(raw: &str) -> TaskId {
    TaskId::parse(raw.to_owned()).expect("タスクIDとして受理される")
}

fn pid_content(pid: u32, kill_ident: &str) -> PidFileContent {
    PidFileContent::new(Pid::new(pid), kill_ident_of(kill_ident))
}

fn kill_ident_of(raw: &str) -> KillIdent {
    KillIdent::parse(raw.to_owned()).expect("非空として受理される")
}

fn starttime(ident: &str) -> StartTimeRecord {
    StartTimeRecord::new(
        ProcessStartTime::parse(ident.to_owned()).expect("非空として受理される"),
        Timestamp::parse_rfc3339("2026-08-12T09:15:30Z").expect("受理される"),
    )
}

/// 破損として返り、不在(`Ok(None)`)と区別されることを確かめる。
fn assert_corrupt<T: std::fmt::Debug>(
    result: Result<Option<T>, RunFileError>,
    expected_path: &std::path::Path,
) {
    match result {
        Err(RunFileError::Corrupt { path, message }) => {
            assert_eq!(path, expected_path, "破損したファイルが指される");
            assert!(!message.is_empty(), "原因が説明される");
        }
        other => panic!("解釈不能な内容は破損として返る: {other:?}"),
    }
}

/// 書き込みが機構の失敗として返ることを確かめる。
fn assert_write_failed(result: Result<(), Io>) {
    match result {
        Err(Io::Failed { message }) => {
            assert!(!message.is_empty(), "原因が説明される");
        }
        Ok(()) => panic!("書き込めない状況では失敗する"),
    }
}

/// 書き込みの反復と重なる読み取りを行う。
///
/// `write` は 0 / 1 の2つの値を書き分け、`read` は観測した値を自身で検査する。観測が
/// 書き込みと重ならないまま終わると「中間状態を観測しなかった」が空虚に成立するため、
/// 重なった観測の回数を確かめる。
fn assert_concurrent_reads(write: impl Fn(usize) + Sync, read: impl Fn() + Sync) {
    let writing = AtomicBool::new(true);
    let observations = AtomicUsize::new(0);
    let overlapped = thread::scope(|scope| {
        let _stop = StopOnDrop(&writing);
        scope.spawn(|| {
            while writing.load(Ordering::Relaxed) {
                read();
                observations.fetch_add(1, Ordering::Relaxed);
            }
        });

        // 読み手が回り始めてから書き始める。書き終えてから待つと「一度は観測した」しか
        // 言えず、観測が書き込みと重なったことにならない。
        let before = yield_until_observations_exceed(&observations, 0);
        let mut rounds = 0;
        while rounds < WRITE_ROUNDS
            || (observations.load(Ordering::Relaxed) - before < CONCURRENT_OBSERVATIONS
                && rounds < WRITE_ROUND_LIMIT)
        {
            write(0);
            write(1);
            rounds += 1;
        }

        // 書き込みを止める前に数える。止めた後の観測は書き込みと重なっていない。
        observations.load(Ordering::Relaxed) - before
    });

    assert!(
        overlapped >= CONCURRENT_OBSERVATIONS,
        "書き込みの反復と重なった観測が {overlapped} 回しかない"
    );
}

/// 読み手の停止をスコープの巻き戻しに載せる(TC-016)。
///
/// 読み手の停止条件はこのフラグだけで、`thread::scope` は巻き戻しの前に子スレッドを合流
/// させる。書き手が書き込みの失敗でパニックしたとき、停止をクロージャ末尾の文で行うと
/// 読み手が回り続けてプロセスが返らない — スイートが検出すべき欠陥が、判定ではなく
/// ハングとして現れる。
struct StopOnDrop<'a>(&'a AtomicBool);

impl Drop for StopOnDrop<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Relaxed);
    }
}

/// 観測回数が `floor` を超えるまで実行を譲り、その時点の観測回数を返す(TC-016)。
///
/// 読み手がアサーションで落ちたときに待ち続けないよう期限を置く — 期限切れは戻り値が
/// `floor` のままになることで呼び出し側のアサーションに現れる。
fn yield_until_observations_exceed(observations: &AtomicUsize, floor: usize) -> usize {
    let deadline = Instant::now() + OBSERVATION_WAIT;
    loop {
        let observed = observations.load(Ordering::Relaxed);
        if observed > floor || Instant::now() >= deadline {
            return observed;
        }
        thread::yield_now();
    }
}

/// 読み手の観測を待つ期限。
const OBSERVATION_WAIT: Duration = Duration::from_secs(5);

/// 書き込みと重なった観測に求める回数。
const CONCURRENT_OBSERVATIONS: usize = 5;

/// 読み手が観測を始めた後に回す書き込みの周回数。
const WRITE_ROUNDS: usize = 30;

/// 観測が重ならないときに書き込みを続ける上限。
const WRITE_ROUND_LIMIT: usize = 1_000;

/// RunStore の適合スイートをアダプターに適用する。
///
/// `$setup` はケースごとに評価され、ハーネスは共有されない。`$allowed_skips` は
/// この環境でスキップを許容するケース(TC ID)の集合で、集合の外のスキップはその
/// ケースの失敗になる。
#[macro_export]
macro_rules! run_store_conformance {
    ($setup:expr, $allowed_skips:expr) => {
        use $crate::run_store as __pulsen_conformance_run_store;

        $crate::conformance_cases!(
            __pulsen_conformance_run_store,
            $setup,
            __PULSEN_CONFORMANCE_RUN_STORE_SKIPS = $allowed_skips,
            [
                tc_port_run_store_001_準備でattemptディレクトリが親ごと作られる,
                tc_port_run_store_002_準備は冪等で書き込み済みの内容に影響しない,
                tc_port_run_store_003_pidファイル未書き込みは不在になる,
                tc_port_run_store_004_attemptディレクトリ不在のpid読み取りは不在になる,
                tc_port_run_store_005_pidファイルは書いた値のまま読み戻せる,
                tc_port_run_store_006_解釈不能なpidファイルは破損として返る,
                tc_port_run_store_007_pidファイルの読み取り失敗は機構の失敗として返る,
                tc_port_run_store_008_starttimeファイル未書き込みは不在になる,
                tc_port_run_store_009_attemptディレクトリ不在のstarttime読み取りは不在になる,
                tc_port_run_store_010_starttimeファイルは書いた値のまま読み戻せる,
                tc_port_run_store_011_解釈不能なstarttimeファイルは破損として返る,
                tc_port_run_store_012_exitファイル未書き込みは不在になる,
                tc_port_run_store_013_attemptディレクトリ不在のexit読み取りは不在になる,
                tc_port_run_store_014_exitファイルは書いた値のまま読み戻せる,
                tc_port_run_store_015_解釈不能なexitファイルは破損として返る,
                tc_port_run_store_016_書き込みと並行する読み取りは完全な値だけを観測する,
                tc_port_run_store_017_失敗した書き込みは部分的な内容を残さない,
                tc_port_run_store_018_マーカーはattemptディレクトリごと作成して書かれる,
                tc_port_run_store_019_マーカーの書き込みは冪等,
                tc_port_run_store_020_マーカー未書き込みは偽になる,
                tc_port_run_store_021_マーカー書き込み済みは真になる,
                tc_port_run_store_022_ファイルのない準備済みattemptは存在する,
                tc_port_run_store_023_attemptディレクトリ不在は存在しない,
                write_準備を経ない書き込みも置き場ごと作って残る,
            ]
        );
    };
}
