//! `tick` の文言。

use pulsen_domain::definition::TimeoutSpec;
use pulsen_domain::execution::{
    InconsistentRunFiles, NotificationService, NotifyFailureCause, RunFileError,
};
use pulsen_domain::task::{ExecutionStateKind, SaveError, TransitionError};

use crate::application::tick::{RemnantsLeft, RunFailureCause, TickError, TickIssue, TickSummary};
use crate::cli::tick::TickCommandError;

use super::{problem, push_attempts, push_ids, wire_error};

/// ロック競合でスキップしたこと。cron 運用でアラートにしないため 0 で終わる。
pub fn tick_skipped() -> String {
    "別の操作が実行中のため、今回の tick はスキップしました。".to_owned()
}

/// tick の結果のサマリー。値の入っている項目だけを並べる。
pub fn tick_summary(summary: &TickSummary) -> String {
    if summary.is_empty() {
        return "処理対象のタスクはありませんでした。".to_owned();
    }

    let mut out = String::from("tick を実行しました。\n");
    push_ids(&mut out, "起動", &summary.launched);
    push_ids(&mut out, "起動確認", &summary.confirmed_running);
    push_ids(&mut out, "判定確定", &summary.judged);
    push_ids(&mut out, "遷移", &summary.transitioned);
    push_ids(&mut out, "実行待ちへ復帰", &summary.skipped_back);
    push_ids(&mut out, "凍結", &summary.frozen);
    push_ids(&mut out, "通知", &summary.notified);
    push_ids(&mut out, "終端処理", &summary.archived);
    push_attempts(&mut out, "gcで削除", &summary.gc_deleted);
    push_attempts(&mut out, "gcで削除できず", &summary.gc_errors);

    let mut recorded = Vec::new();
    let mut unsettled = Vec::new();
    let mut skipped = Vec::new();
    let mut cleanup = Vec::new();
    for issue in &summary.errors {
        match issue_outcome(issue) {
            IssueOutcome::Recorded => recorded.push(issue),
            IssueOutcome::LaunchUnsettled => unsettled.push(issue),
            IssueOutcome::Skipped => skipped.push(issue),
            IssueOutcome::CleanupLeft => cleanup.push(issue),
        }
    }
    push_issues(&mut out, "失敗を記録", &recorded);
    push_issues(&mut out, "起動の結果が未確定", &unsettled);
    push_issues(&mut out, "スキップ", &skipped);
    push_issues(&mut out, "後始末が残っている", &cleanup);

    out.trim_end().to_owned()
}

/// 報告が何を残したか。運用者が次に取る行動はこれで分かれる。
enum IssueOutcome {
    /// 失敗を記録した。カウンタを消費し、上限を超えれば同じ tick で凍結する。
    Recorded,
    /// launching の記録は保存済みで、次の tick が猶予経路で分類する。
    LaunchUnsettled,
    /// タスクファイルへの書き込みが無く、次の tick がそのまま再試行する。
    Skipped,
    /// タスクファイルには何も残っていないが、OS 側に後始末が残っている。
    /// tick は残存終了を再試行しないので、終了させるのは人間になる。
    CleanupLeft,
}

/// 報告の結末の分類。
///
/// 網羅 `match` に置き、分類が増えたときに振り分け先を決めないと通らないようにする。
fn issue_outcome(issue: &TickIssue) -> IssueOutcome {
    match issue {
        TickIssue::WorktreeCreateFailed { .. }
        | TickIssue::CommandExpansionFailed { .. }
        | TickIssue::SpawnNotObserved { .. }
        | TickIssue::JudgeFailed { .. }
        | TickIssue::RunFailed { .. } => IssueOutcome::Recorded,
        TickIssue::PrepareAttemptFailed { .. } | TickIssue::SpawnFailed { .. } => {
            IssueOutcome::LaunchUnsettled
        }
        // 残存の報告は保存の成否と独立に積まれ、タスクファイルには何も残さない。
        // 失敗の記録と並べるとカウンタを消費していない tick が消費したように読め、
        // スキップと並べると tick が再試行しない後始末が再試行されるように読める。
        TickIssue::RemnantsUnhandled { .. } => IssueOutcome::CleanupLeft,
        TickIssue::CorruptTaskFile { .. }
        | TickIssue::SnapshotUnreadable { .. }
        | TickIssue::MissingCurrentAttempt { .. }
        | TickIssue::MissingProcessIdent { .. }
        | TickIssue::Transition { .. }
        | TickIssue::MissingWorkspace { .. }
        | TickIssue::RunFileUnreadable { .. }
        | TickIssue::InconsistentRunFiles { .. }
        | TickIssue::MarkerWriteFailed { .. }
        | TickIssue::ObservationFailed { .. }
        | TickIssue::KillFailed { .. }
        | TickIssue::NotifyFailed { .. }
        | TickIssue::SaveFailed { .. } => IssueOutcome::Skipped,
    }
}

/// 見出しと件数を伴う報告の一覧。空なら見出しごと出さない。
fn push_issues(out: &mut String, label: &str, issues: &[&TickIssue]) {
    if issues.is_empty() {
        return;
    }
    out.push_str(&format!("  {label}({}件):\n", issues.len()));
    for issue in issues {
        out.push_str("    - ");
        out.push_str(&tick_issue(issue));
        out.push('\n');
    }
}

/// `tick` の失敗。
pub fn tick_error(error: &TickCommandError) -> String {
    match error {
        TickCommandError::Wire(error) => wire_error(error),
        TickCommandError::Tick(TickError::LockFailed { message }) => {
            problem("排他ロックを扱えません。", &[format!("原因: {message}")])
        }
        TickCommandError::Tick(TickError::Scan { message }) => problem(
            "タスクを走査できません。",
            &[
                format!("原因: {message}"),
                "状態は変更していません。".to_owned(),
            ],
        ),
    }
}

/// スキップしたタスク1件の理由。タスクID(または対象のパス)と原因が読み取れる形にする。
fn tick_issue(issue: &TickIssue) -> String {
    match issue {
        TickIssue::CorruptTaskFile { path, message } => {
            format!("{}: タスクファイルを読めません({message})", path.display())
        }
        TickIssue::SnapshotUnreadable { task_id, message } => format!(
            "{}: 埋め込まれたワークフロー定義を読めません({message})",
            task_id.as_str()
        ),
        // 実行状態を名指ししない — この破れは起動記録済み・起動確認済みのどちらの手続きでも
        // 積まれ、片方を名乗ると帳簿の実行状態と食い違う。
        TickIssue::MissingCurrentAttempt { task_id } => format!(
            "{}: 観測の前提となる現在 attempt がありません(タスクファイルの修復が必要です)",
            task_id.as_str()
        ),
        TickIssue::Transition { task_id, error } => format!(
            "{}: 遷移の前提が成立しません({})",
            task_id.as_str(),
            transition_error(error)
        ),
        TickIssue::RunFileUnreadable { task_id, error } => format!(
            "{}: runディレクトリのファイルを読めません({})",
            task_id.as_str(),
            run_file_error(error)
        ),
        TickIssue::InconsistentRunFiles { task_id, kind } => {
            format!("{}: {}", task_id.as_str(), inconsistent_run_files(kind))
        }
        TickIssue::WorktreeCreateFailed { task_id, message } => {
            format!("{}: worktree を作成できません({message})", task_id.as_str())
        }
        TickIssue::CommandExpansionFailed { task_id, message } => format!(
            "{}: 起動コマンドを組み立てられません({message})",
            task_id.as_str()
        ),
        TickIssue::MarkerWriteFailed { task_id, message } => format!(
            "{}: 無効化マーカーを書けません({message})",
            task_id.as_str()
        ),
        TickIssue::PrepareAttemptFailed { task_id, message } => format!(
            "{}: attempt の runディレクトリを用意できません({message})",
            task_id.as_str()
        ),
        TickIssue::SpawnFailed { task_id, message } => {
            format!("{}: ラッパーを起動できません({message})", task_id.as_str())
        }
        TickIssue::SpawnNotObserved { task_id, message } => format!(
            "{}: 起動を確認できず spawn 失敗として記録しました({message})",
            task_id.as_str()
        ),
        TickIssue::MissingProcessIdent { task_id } => format!(
            "{}: 起動確認済みですが同定情報がありません(pid ファイルからの修復が必要です)",
            task_id.as_str()
        ),
        TickIssue::ObservationFailed { task_id, message } => format!(
            "{}: プロセスの生存を観測できません({message})",
            task_id.as_str()
        ),
        TickIssue::KillFailed { task_id, message } => format!(
            "{}: timeout を超えた実行を終了させられません({message})",
            task_id.as_str()
        ),
        TickIssue::RemnantsUnhandled { task_id, remnants } => {
            format!("{}: {}", task_id.as_str(), remnants_left(remnants))
        }
        TickIssue::JudgeFailed { task_id, detail } => format!(
            "{}: 判定できず判定失敗として記録しました({detail})",
            task_id.as_str()
        ),
        TickIssue::RunFailed { task_id, cause } => format!(
            "{}: 実行の失敗を記録しました({})",
            task_id.as_str(),
            run_failure_cause(cause)
        ),
        TickIssue::MissingWorkspace { task_id } => format!(
            "{}: 判定コマンドへ渡すワークスペースが未確定です(タスクファイルの修復が必要です)",
            task_id.as_str()
        ),
        TickIssue::NotifyFailed { task_id, cause } => format!(
            "{}: 凍結を通知できません({})。次の tick が再通知します",
            task_id.as_str(),
            notify_failure_cause(cause)
        ),
        TickIssue::SaveFailed { task_id, error } => format!(
            "{}: タスクファイルを保存できません({})",
            task_id.as_str(),
            save_error(error)
        ),
    }
}

/// 実行を失敗として確定させた根拠。判断した主体が読めるように書く。
fn run_failure_cause(cause: &RunFailureCause) -> String {
    match cause {
        RunFailureCause::DefaultJudgement { exit } => {
            format!("実行が終了コード {} で終了しました", exit.get())
        }
        RunFailureCause::JudgeCommand { exit } => format!(
            "判定コマンドが失敗と判定しました(実行の終了コードは {})",
            exit.get()
        ),
        RunFailureCause::TimedOut {
            timeout: TimeoutSpec::Limited(limit),
        } => format!(
            "実行が timeout({}秒)を超えたため終了させました",
            limit.seconds()
        ),
        // 無制限の timeout では超過が成立しないため、終了させた事実だけを述べる。
        RunFailureCause::TimedOut {
            timeout: TimeoutSpec::Unlimited,
        } => "実行を終了させました".to_owned(),
        RunFailureCause::DiedWithoutExit => "実行が終了コードを残さずに終わりました".to_owned(),
    }
}

/// 通知が届かなかった原因。timeout の秒数は組み込み定数から読む。
fn notify_failure_cause(cause: &NotifyFailureCause) -> String {
    match cause {
        NotifyFailureCause::ExitedNonZero { exit } => {
            format!("通知コマンドが終了コード {} で終了しました", exit.get())
        }
        NotifyFailureCause::TimedOut => format!(
            "通知コマンドが {} 秒のうちに終了しませんでした",
            NotificationService::NOTIFY_TIMEOUT.seconds()
        ),
        NotifyFailureCause::FailedToStart { message } => {
            format!("通知コマンドを起動できませんでした: {message}")
        }
    }
}

/// ベストエフォートの残存終了のあとに残った後始末。OS ツールでの後始末を促す。
fn remnants_left(remnants: &RemnantsLeft) -> String {
    match remnants {
        RemnantsLeft::NotIdentifiable => {
            "残存プロセスを誤殺なく同定できませんでした(終了操作は行っていません)".to_owned()
        }
        RemnantsLeft::Failed { message } => {
            format!("残存プロセスを終了できませんでした: {message}")
        }
    }
}

/// 遷移の前提の破れ。修復は人間に委ねるため、破れた前提そのものを示す。
fn transition_error(error: &TransitionError) -> String {
    match error {
        TransitionError::InvalidState { expected, actual } => format!(
            "実行状態が {} ではなく {}",
            expected
                .iter()
                .map(ExecutionStateKind::as_str)
                .collect::<Vec<_>>()
                .join(" | "),
            actual.as_str()
        ),
        TransitionError::WorkspaceAlreadySet => "ワークスペースが確定済み".to_owned(),
        TransitionError::WorkspaceNotSet => "ワークスペースが未確定".to_owned(),
        TransitionError::NotAgentRunStatus { status } => format!(
            "ステータス `{}` はエージェント実行ではない",
            status.as_str()
        ),
        // 実行状態を名指ししない — この破れは起動記録済み・実行中・判定確定のいずれからも
        // 返り、実行状態を述べると修復の入口を誤らせる。
        TransitionError::MissingCurrentAttempt => {
            "遷移の前提となる現在 attempt(または同定情報)が無い".to_owned()
        }
        TransitionError::AlreadyNotified => "凍結の通知が記録済み".to_owned(),
    }
}

/// ラッパーの書き込み順序の破れ。次の tick が再観測するので、修復の指示は添えない。
fn inconsistent_run_files(kind: &InconsistentRunFiles) -> String {
    match kind {
        InconsistentRunFiles::MissingStartTime => {
            "pid ファイルがあるのに starttime ファイルがありません(ラッパーは starttime を先に書く)"
                .to_owned()
        }
    }
}

/// run ファイルの読み取りの失敗。
fn run_file_error(error: &RunFileError) -> String {
    match error {
        RunFileError::Corrupt { path, message } => {
            format!("{} を解釈できない: {message}", path.display())
        }
        RunFileError::Io { message } => message.clone(),
    }
}

/// タスクファイルの保存の失敗。
fn save_error(error: &SaveError) -> String {
    match error {
        SaveError::NotFound => "現役のタスクとして存在しない".to_owned(),
        SaveError::Io { message } => message.clone(),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use pulsen_domain::definition::DurationSpec;
    use pulsen_domain::execution::ExitCode;
    use pulsen_domain::task::{AttemptNumber, TaskId};

    use super::*;

    fn task(id: &str) -> TaskId {
        TaskId::parse(id.to_owned()).expect("受理される")
    }

    #[test]
    fn 処理対象がなければその旨だけを表示する() {
        assert_eq!(
            tick_summary(&TickSummary::default()),
            "処理対象のタスクはありませんでした。"
        );
    }

    #[test]
    fn サマリーは値の入っている項目だけを並べる() {
        let summary = TickSummary {
            launched: vec![task("20260812t101112-abcd1234")],
            frozen: vec![task("20260812t101112-efgh5678")],
            ..TickSummary::default()
        };

        assert_eq!(
            tick_summary(&summary),
            "tick を実行しました。\n  \
             起動: 20260812t101112-abcd1234\n  \
             凍結: 20260812t101112-efgh5678"
        );
    }

    #[test]
    fn すべての項目が埋まったサマリーは決まった順で並ぶ() {
        let summary = TickSummary {
            launched: vec![task("20260812t101112-aaaa0001")],
            confirmed_running: vec![task("20260812t101112-aaaa0002")],
            judged: vec![task("20260812t101112-aaaa0011")],
            transitioned: vec![task("20260812t101112-aaaa0003")],
            skipped_back: vec![task("20260812t101112-aaaa0004")],
            frozen: vec![task("20260812t101112-aaaa0005")],
            notified: vec![task("20260812t101112-aaaa0006")],
            archived: vec![task("20260812t101112-aaaa0007")],
            errors: vec![TickIssue::MissingCurrentAttempt {
                task_id: task("20260812t101112-aaaa0008"),
            }],
            gc_deleted: vec![(
                "20260812t101112-aaaa0009".to_owned(),
                AttemptNumber::parse(1).expect("受理される"),
            )],
            gc_errors: vec![(
                "20260812t101112-aaaa0010".to_owned(),
                AttemptNumber::parse(2).expect("受理される"),
            )],
        };

        assert_eq!(
            tick_summary(&summary),
            "tick を実行しました。\n  \
             起動: 20260812t101112-aaaa0001\n  \
             起動確認: 20260812t101112-aaaa0002\n  \
             判定確定: 20260812t101112-aaaa0011\n  \
             遷移: 20260812t101112-aaaa0003\n  \
             実行待ちへ復帰: 20260812t101112-aaaa0004\n  \
             凍結: 20260812t101112-aaaa0005\n  \
             通知: 20260812t101112-aaaa0006\n  \
             終端処理: 20260812t101112-aaaa0007\n  \
             gcで削除: 20260812t101112-aaaa0009/attempt-1\n  \
             gcで削除できず: 20260812t101112-aaaa0010/attempt-2\n  \
             スキップ(1件):\n    \
             - 20260812t101112-aaaa0008: 観測の前提となる現在 attempt がありません\
             (タスクファイルの修復が必要です)"
        );
    }

    #[test]
    fn 起動を確認したタスクはサマリーに現れる() {
        let summary = TickSummary {
            confirmed_running: vec![task("20260812t101112-abcd1234")],
            ..TickSummary::default()
        };

        assert_eq!(
            tick_summary(&summary),
            "tick を実行しました。\n  起動確認: 20260812t101112-abcd1234"
        );
    }

    #[test]
    fn 記録した失敗と素通しのスキップは別の見出しに分かれる() {
        let summary = TickSummary {
            errors: vec![
                TickIssue::WorktreeCreateFailed {
                    task_id: task("20260812t101112-abcd1234"),
                    message: "git worktree add に失敗".to_owned(),
                },
                TickIssue::InconsistentRunFiles {
                    task_id: task("20260812t101112-ijkl9012"),
                    kind: InconsistentRunFiles::MissingStartTime,
                },
                TickIssue::CommandExpansionFailed {
                    task_id: task("20260812t101112-efgh5678"),
                    message: "エージェント `claude` は config.yaml に定義されていません".to_owned(),
                },
                TickIssue::SpawnNotObserved {
                    task_id: task("20260812t101112-mnop3456"),
                    message: "起動から 30 秒のうちに pid ファイルが現れませんでした".to_owned(),
                },
            ],
            ..TickSummary::default()
        };

        assert_eq!(
            tick_summary(&summary),
            "tick を実行しました。\n  \
             失敗を記録(3件):\n    \
             - 20260812t101112-abcd1234: worktree を作成できません(git worktree add に失敗)\n    \
             - 20260812t101112-efgh5678: 起動コマンドを組み立てられません\
             (エージェント `claude` は config.yaml に定義されていません)\n    \
             - 20260812t101112-mnop3456: 起動を確認できず spawn 失敗として記録しました\
             (起動から 30 秒のうちに pid ファイルが現れませんでした)\n  \
             スキップ(1件):\n    \
             - 20260812t101112-ijkl9012: pid ファイルがあるのに starttime ファイルが\
             ありません(ラッパーは starttime を先に書く)"
        );
    }

    #[test]
    fn 実行の失敗の根拠は判断した主体が読める形で示される() {
        let failed = |cause: RunFailureCause| {
            tick_summary(&TickSummary {
                errors: vec![TickIssue::RunFailed {
                    task_id: task("20260812t101112-abcd1234"),
                    cause,
                }],
                ..TickSummary::default()
            })
        };

        assert!(
            failed(RunFailureCause::DefaultJudgement {
                exit: ExitCode::new(1),
            })
            .ends_with("実行の失敗を記録しました(実行が終了コード 1 で終了しました)"),
        );
        assert!(
            failed(RunFailureCause::JudgeCommand {
                exit: ExitCode::new(0),
            })
            .ends_with(
                "実行の失敗を記録しました(判定コマンドが失敗と判定しました(実行の終了コードは 0))"
            ),
            "エージェントの終了コードが 0 でも判定コマンドの判断として読める"
        );
        assert!(
            failed(RunFailureCause::TimedOut {
                timeout: TimeoutSpec::Limited(DurationSpec::parse("60s").expect("受理される")),
            })
            .ends_with("実行の失敗を記録しました(実行が timeout(60秒)を超えたため終了させました)"),
        );
        assert!(
            failed(RunFailureCause::DiedWithoutExit)
                .ends_with("実行の失敗を記録しました(実行が終了コードを残さずに終わりました)"),
        );
    }

    #[test]
    fn 通知できなかった原因は3つが区別できる形で示される() {
        let failed = |cause: NotifyFailureCause| {
            tick_summary(&TickSummary {
                errors: vec![TickIssue::NotifyFailed {
                    task_id: task("20260812t101112-abcd1234"),
                    cause,
                }],
                ..TickSummary::default()
            })
        };

        assert_eq!(
            failed(NotifyFailureCause::ExitedNonZero {
                exit: ExitCode::new(3),
            }),
            "tick を実行しました。\n  \
             スキップ(1件):\n    \
             - 20260812t101112-abcd1234: 凍結を通知できません\
             (通知コマンドが終了コード 3 で終了しました)。次の tick が再通知します"
        );
        assert!(
            failed(NotifyFailureCause::TimedOut).contains(&format!(
                "凍結を通知できません(通知コマンドが {} 秒のうちに終了しませんでした)",
                NotificationService::NOTIFY_TIMEOUT.seconds()
            )),
            "秒数は設定値ではなく組み込み定数から読む"
        );
        assert!(
            failed(NotifyFailureCause::FailedToStart {
                message: "通知コマンドが見つかりません".to_owned(),
            })
            .contains(
                "凍結を通知できません\
                 (通知コマンドを起動できませんでした: 通知コマンドが見つかりません)"
            ),
        );
    }

    #[test]
    fn 残存プロセスの後始末は同定できたかで書き分けられる() {
        let remnants = |remnants: RemnantsLeft| {
            tick_summary(&TickSummary {
                errors: vec![TickIssue::RemnantsUnhandled {
                    task_id: task("20260812t101112-abcd1234"),
                    remnants,
                }],
                ..TickSummary::default()
            })
        };

        assert_eq!(
            remnants(RemnantsLeft::NotIdentifiable),
            "tick を実行しました。\n  \
             後始末が残っている(1件):\n    \
             - 20260812t101112-abcd1234: 残存プロセスを誤殺なく同定できませんでした\
             (終了操作は行っていません)"
        );
        assert!(
            remnants(RemnantsLeft::Failed {
                message: "終了操作を起動できない".to_owned(),
            })
            .ends_with("残存プロセスを終了できませんでした: 終了操作を起動できない"),
        );
    }

    #[test]
    fn 保存できなかった残存の報告は記録した失敗の見出しに現れない() {
        let summary = TickSummary {
            errors: vec![
                TickIssue::RemnantsUnhandled {
                    task_id: task("20260812t101112-abcd1234"),
                    remnants: RemnantsLeft::Failed {
                        message: "終了操作を起動できない".to_owned(),
                    },
                },
                TickIssue::SaveFailed {
                    task_id: task("20260812t101112-abcd1234"),
                    error: SaveError::Io {
                        message: "書き込めません".to_owned(),
                    },
                },
            ],
            ..TickSummary::default()
        };

        assert_eq!(
            tick_summary(&summary),
            "tick を実行しました。\n  \
             スキップ(1件):\n    \
             - 20260812t101112-abcd1234: タスクファイルを保存できません(書き込めません)\n  \
             後始末が残っている(1件):\n    \
             - 20260812t101112-abcd1234: 残存プロセスを終了できませんでした: 終了操作を起動できない"
        );
    }

    #[test]
    fn ワークスペースの未確定は修復すべき場所を示す() {
        assert!(
            tick_summary(&TickSummary {
                errors: vec![TickIssue::MissingWorkspace {
                    task_id: task("20260812t101112-abcd1234"),
                }],
                ..TickSummary::default()
            })
            .ends_with(
                "判定コマンドへ渡すワークスペースが未確定です(タスクファイルの修復が必要です)"
            ),
        );
    }

    #[test]
    fn 現在attemptの欠落は実行状態を名指ししない() {
        let report = tick_summary(&TickSummary {
            errors: vec![TickIssue::MissingCurrentAttempt {
                task_id: task("20260812t101112-abcd1234"),
            }],
            ..TickSummary::default()
        });

        assert!(
            report.ends_with(
                "観測の前提となる現在 attempt がありません(タスクファイルの修復が必要です)"
            ),
            "{report}"
        );
        assert!(!report.contains("起動記録"), "{report}");
        assert!(!report.contains("起動確認"), "{report}");
    }

    #[test]
    fn 遷移の前提の破れは破れた前提そのものを示す() {
        let transition = |error: TransitionError| {
            tick_summary(&TickSummary {
                errors: vec![TickIssue::Transition {
                    task_id: task("20260812t101112-abcd1234"),
                    error,
                }],
                ..TickSummary::default()
            })
        };

        assert!(
            transition(TransitionError::InvalidState {
                expected: &[ExecutionStateKind::Pending, ExecutionStateKind::Failed],
                actual: ExecutionStateKind::Running,
            })
            .ends_with("遷移の前提が成立しません(実行状態が pending | failed ではなく running)"),
        );
        assert!(
            transition(TransitionError::MissingCurrentAttempt).ends_with(
                "遷移の前提が成立しません(遷移の前提となる現在 attempt(または同定情報)が無い)"
            ),
        );
    }

    #[test]
    fn スキップしたタスクは件数と原因つきで並ぶ() {
        let summary = TickSummary {
            errors: vec![
                TickIssue::CorruptTaskFile {
                    path: PathBuf::from("/home/u/.pulsen/state/tasks/broken.json"),
                    message: "JSON として読めない".to_owned(),
                },
                TickIssue::SaveFailed {
                    task_id: task("20260812t101112-abcd1234"),
                    error: SaveError::Io {
                        message: "書き込めません".to_owned(),
                    },
                },
            ],
            ..TickSummary::default()
        };

        assert_eq!(
            tick_summary(&summary),
            "tick を実行しました。\n  \
             スキップ(2件):\n    \
             - /home/u/.pulsen/state/tasks/broken.json: タスクファイルを読めません\
             (JSON として読めない)\n    \
             - 20260812t101112-abcd1234: タスクファイルを保存できません(書き込めません)"
        );
    }

    #[test]
    fn 起動の結果が未確定の報告は起動と同じサマリーに別の見出しで並ぶ() {
        let summary = TickSummary {
            launched: vec![task("20260812t101112-abcd1234")],
            errors: vec![
                TickIssue::PrepareAttemptFailed {
                    task_id: task("20260812t101112-abcd1234"),
                    message: "runディレクトリを作成できない".to_owned(),
                },
                TickIssue::SpawnFailed {
                    task_id: task("20260812t101112-efgh5678"),
                    message: "自身のバイナリを起動できない".to_owned(),
                },
            ],
            ..TickSummary::default()
        };

        assert_eq!(
            tick_summary(&summary),
            "tick を実行しました。\n  \
             起動: 20260812t101112-abcd1234\n  \
             起動の結果が未確定(2件):\n    \
             - 20260812t101112-abcd1234: attempt の runディレクトリを用意できません\
             (runディレクトリを作成できない)\n    \
             - 20260812t101112-efgh5678: ラッパーを起動できません(自身のバイナリを起動できない)"
        );
    }

    #[test]
    fn 走査自体の失敗は状態を変更していないことを添えて案内される() {
        assert_eq!(
            tick_error(&TickCommandError::Tick(TickError::Scan {
                message: "タスクの置き場を読めない".to_owned(),
            })),
            "エラー: タスクを走査できません。\n  \
             原因: タスクの置き場を読めない\n  \
             状態は変更していません。"
        );
    }

    #[test]
    fn ロック競合のスキップは失敗として案内しない() {
        assert!(!tick_skipped().starts_with("エラー"));
    }
}
