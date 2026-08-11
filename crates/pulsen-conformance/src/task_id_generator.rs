//! TaskIdGenerator の適合ケース(`spec/testcases/ports/task-id-generator.md` の5行)。
//!
//! IDの形式(時刻成分・ランダム成分の構成)はアダプターに委ねられているため、検証するのは
//! 契約 — `TaskId` の文字集合制約と、実用上の一意性 — だけにする。

use std::collections::HashSet;
use std::env;

use pulsen_domain::task::{
    BranchName, StateRoot, TaskFilePath, TaskId, TaskIdGenerator, WorktreePath,
};

use crate::{CaseOutcome, TaskIdGeneratorHarness, require};

/// 一意性を確かめる発行回数(spec の例に合わせる)。
const MANY: usize = 10_000;
/// インスタンスを跨ぐ一意性を確かめる発行回数。
const SOME: usize = 1_000;

pub fn tc_port_task_id_generator_001_発行されたidは制約を満たす(
    harness: &impl TaskIdGeneratorHarness,
) -> CaseOutcome {
    let id = harness.generator().generate();

    assert_eq!(TaskId::parse(id.as_str().to_owned()), Ok(id.clone()));
    assert!((1..=TaskId::MAX_LENGTH).contains(&id.as_str().chars().count()));
    assert!(
        id.as_str()
            .chars()
            .all(|found| found.is_ascii_lowercase() || found.is_ascii_digit() || found == '-'),
        "{id:?}"
    );
    let leading = id.as_str().chars().next().expect("非空");
    assert!(leading.is_ascii_alphanumeric(), "{id:?}");
    CaseOutcome::Ran
}

pub fn tc_port_task_id_generator_002_多数回発行しても重複しない(
    harness: &impl TaskIdGeneratorHarness,
) -> CaseOutcome {
    let mut issued = HashSet::with_capacity(MANY);

    for _ in 0..MANY {
        let id = harness.generator().generate();
        assert_eq!(TaskId::parse(id.as_str().to_owned()), Ok(id.clone()));
        assert!(issued.insert(id.into_string()), "重複したIDが発行された");
    }

    assert_eq!(issued.len(), MANY);
    CaseOutcome::Ran
}

pub fn tc_port_task_id_generator_003_間隔を置かない連続発行でも重複しない(
    harness: &impl TaskIdGeneratorHarness,
) -> CaseOutcome {
    let first = harness.generator().generate();
    let second = harness.generator().generate();

    assert_ne!(first, second);
    CaseOutcome::Ran
}

pub fn tc_port_task_id_generator_004_別のジェネレーターとも重複しない(
    harness: &impl TaskIdGeneratorHarness,
) -> CaseOutcome {
    let other = require!(harness.another_generator());

    let mine: HashSet<String> = (0..SOME)
        .map(|_| harness.generator().generate().into_string())
        .collect();
    let theirs: HashSet<String> = (0..SOME).map(|_| other.generate().into_string()).collect();

    assert_eq!(mine.len(), SOME);
    assert_eq!(theirs.len(), SOME);
    assert!(
        mine.is_disjoint(&theirs),
        "インスタンスを跨いでIDが重複した"
    );
    CaseOutcome::Ran
}

pub fn tc_port_task_id_generator_005_発行されたidから導出する名前は常に有効になる(
    harness: &impl TaskIdGeneratorHarness,
) -> CaseOutcome {
    let base = env::temp_dir();
    let state_root = StateRoot::parse(base.clone()).expect("一時ディレクトリは絶対パス");

    let id = harness.generator().generate();

    assert!(WorktreePath::parse(base.join(id.as_str())).is_ok());
    assert!(BranchName::parse(format!("pulsen/{}", id.as_str())).is_ok());
    let file_name = TaskFilePath::file_name(&id);
    assert_eq!(TaskFilePath::parse_file_name(&file_name), Some(id.clone()));
    assert_eq!(
        TaskFilePath::active(&state_root, &id),
        TaskFilePath::active_dir(&state_root).join(file_name)
    );
    CaseOutcome::Ran
}

/// TaskIdGenerator の適合スイートをアダプターに適用する。
///
/// `$setup` はケースごとに評価され、ハーネスは共有されない。`$allowed_skips` は
/// この環境で許容するスキップ件数で、超えたスキップはケースの失敗になる。
#[macro_export]
macro_rules! task_id_generator_conformance {
    ($setup:expr, $allowed_skips:expr) => {
        use $crate::task_id_generator as __pulsen_conformance_task_id_generator;

        $crate::conformance_cases!(
            __pulsen_conformance_task_id_generator,
            $setup,
            __PULSEN_CONFORMANCE_TASK_ID_GENERATOR_SKIPS = $allowed_skips,
            [
                tc_port_task_id_generator_001_発行されたidは制約を満たす,
                tc_port_task_id_generator_002_多数回発行しても重複しない,
                tc_port_task_id_generator_003_間隔を置かない連続発行でも重複しない,
                tc_port_task_id_generator_004_別のジェネレーターとも重複しない,
                tc_port_task_id_generator_005_発行されたidから導出する名前は常に有効になる,
            ]
        );
    };
}
