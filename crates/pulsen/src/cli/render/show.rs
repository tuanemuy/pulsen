//! `show` の文言。
//!
//! 縮退の場合分けはユースケースが直和型に畳んで渡すため、ここは網羅 `match` で
//! 言葉を当てるだけにする。**workspace_path は表示するのみで存在を検証しない**
//! (pages ※9)。
//!
//! パスを文言へ差し込むのはこの層に限る。機構の失敗(`Io`)のメッセージは
//! アダプターが既に対象のパスを含むため前置せず、構造として受け取ったパスだけを
//! 1度だけ出す。

use std::path::Path;

use pulsen_domain::definition::StatusName;
use pulsen_domain::execution::RunFileError;
use pulsen_domain::task::{
    ExecutionState, FailureKind, FailureNote, ProcessIdent, ReadError, StopReason, Timestamp,
    Workspace,
};

use crate::application::show_task::{
    AttemptSummary, ExitOutcome, Limits, RetryLimitInfo, RunDirPresence, ShowTaskError,
    SnapshotInfo, TaskDetail,
};
use crate::cli::show::ShowError;

use super::{problem, push_field, wire_error};

/// 値が無い項目に置く言葉。
const UNSET: &str = "未作成";

/// タスク1件の詳細。
pub fn task_detail(detail: &TaskDetail) -> String {
    let mut out = format!("タスク {} の詳細\n", detail.task_id.as_str());

    push_field(&mut out, "ワークフロー", detail.workflow_name.as_str());
    push_field(
        &mut out,
        "リポジトリ",
        &detail.target.repo().as_path().display().to_string(),
    );
    push_field(
        &mut out,
        "ベースブランチ",
        detail.target.base_branch().as_str(),
    );
    push_field(&mut out, "タスクステータス", detail.task_status.as_str());
    push_field(&mut out, "実行状態", detail.execution.kind().as_str());
    push_field(&mut out, "在籍", &residence(detail.archived));

    push_stop(&mut out, &detail.execution);

    // workspace のパスは表示するだけで、実体の有無は問わない(pages ※9)。
    // アーカイブ済みの「削除済み」も、アーカイブ済みであるという事実から導く(※4)。
    push_field(
        &mut out,
        "workspace_path",
        &workspace_path(detail.workspace.as_ref(), detail.archived),
    );
    push_field(
        &mut out,
        "branch",
        &detail
            .workspace
            .as_ref()
            .map_or_else(|| UNSET.to_owned(), |ws| ws.branch().as_str().to_owned()),
    );

    push_counters(&mut out, detail);
    push_attempt(&mut out, detail.attempt.as_ref());
    push_field(
        &mut out,
        "直近の失敗要因",
        &last_failure(detail.last_failure.as_ref()),
    );
    push_field(&mut out, "更新日時", &detail.updated_at.to_rfc3339());
    push_field(
        &mut out,
        "定義済みステータス",
        &defined_statuses(&detail.snapshot),
    );
    push_field(
        &mut out,
        "スナップショット保存先",
        &detail.task_file_path.display().to_string(),
    );

    out.trim_end().to_owned()
}

/// `show` の失敗。いずれも書き込みを行わない。
pub fn show_error(error: &ShowError) -> String {
    match error {
        ShowError::Wire(error) => wire_error(error),
        ShowError::Show(ShowTaskError::InvalidTaskId(error)) => problem(
            "タスクIDが不正です。",
            &[format!("原因: {}", error.describe())],
        ),
        ShowError::Show(ShowTaskError::NotFound { task_id }) => problem(
            "指定されたタスクが見つかりません。",
            &[
                format!("タスクID: {}", task_id.as_str()),
                "現役にもアーカイブにも存在しません。".to_owned(),
            ],
        ),
        // 修復は人間に委ねる。読めないファイルには書き込まない(pages 縮退規則2)。
        ShowError::Show(ShowTaskError::Corrupt { path, message }) => problem(
            "タスクファイルを読めません。",
            &[
                format!("ファイル: {}", path.display()),
                format!("原因: {message}"),
                "内容は変更していません。ファイルを直接確認して修復してください。".to_owned(),
            ],
        ),
        ShowError::Show(ShowTaskError::Read(ReadError::Io { message })) => {
            problem("タスクを探せません。", &[format!("原因: {message}")])
        }
    }
}

/// 現役かアーカイブ済みか。アーカイブ済みは worktree が削除済みであることも示す(※4)。
fn residence(archived: bool) -> String {
    if archived {
        "アーカイブ済み(worktree は削除済み)".to_owned()
    } else {
        "現役".to_owned()
    }
}

/// workspace のパス。存在検証は行わない。
fn workspace_path(workspace: Option<&Workspace>, archived: bool) -> String {
    let Some(workspace) = workspace else {
        return UNSET.to_owned();
    };
    let path = workspace.path().as_path().display().to_string();
    if archived {
        return format!("{path}(削除済み)");
    }
    path
}

/// 3つのカウンタと、それぞれに適用される上限。
fn push_counters(out: &mut String, detail: &TaskDetail) {
    let Limits {
        retry,
        judge,
        spawn,
    } = detail.limits;
    let counters = detail.counters;

    push_field(
        out,
        "attempt_count",
        &retry_limit(counters.attempt_count(), retry),
    );
    push_field(
        out,
        "judge_attempt_count",
        &format!("{}(上限 {judge})", counters.judge_attempt_count()),
    );
    push_field(
        out,
        "spawn_fail_count",
        &format!("{}(上限 {spawn})", counters.spawn_fail_count()),
    );
}

/// リトライ上限の併記。
///
/// 「適用対象がない」と「導出できない」を書き分ける — 前者は併記そのものが無く、
/// 後者はスナップショットの修復で読めるようになる。
fn retry_limit(count: u32, limit: RetryLimitInfo) -> String {
    match limit {
        RetryLimitInfo::Applicable(limit) => format!("{count}(上限 {limit})"),
        RetryLimitInfo::NotApplicable => count.to_string(),
        RetryLimitInfo::Unknown => {
            format!("{count}(上限 導出不能: スナップショットを読めません)")
        }
    }
}

/// 現在 attempt の実行メタデータ。
fn push_attempt(out: &mut String, attempt: Option<&AttemptSummary>) {
    let Some(attempt) = attempt else {
        push_field(out, "現在attempt", "なし");
        return;
    };

    push_field(out, "現在attempt", &attempt.number.get().to_string());
    push_sub(
        out,
        "runディレクトリ",
        &run_dir(attempt.run_dir.as_path(), &attempt.run_dir_presence),
    );
    push_process(out, attempt.process.as_ref());
    // run ディレクトリ配下のパスはすべて `run_dir` から導出する(派生値を DTO に持たない)。
    push_sub(
        out,
        "stdout.log",
        &attempt.run_dir.stdout_log().display().to_string(),
    );
    push_sub(
        out,
        "stderr.log",
        &attempt.run_dir.stderr_log().display().to_string(),
    );
    push_sub(
        out,
        "exit",
        &exit(&attempt.run_dir.exit_file(), &attempt.run_dir_presence),
    );
}

/// run ディレクトリのパスと、その有無。
///
/// exit は同じ値から別の項目行として出すため、ここでは読まない。
fn run_dir(path: &Path, presence: &RunDirPresence) -> String {
    match presence {
        RunDirPresence::Present { exit: _ } => path.display().to_string(),
        RunDirPresence::Absent => format!("{}(存在しません)", path.display()),
        // 機構の失敗のメッセージは対象のパスを既に含む。
        RunDirPresence::Unknown { message } => format!("存在を確認できません: {message}"),
    }
}

/// exit ファイルのパスと、読み取った値。
///
/// run ディレクトリの有無が exit の読めなさを決めるため、同じ値から言葉を当てる。
/// 「記録なし」と「読んでいません」を書き分ける — 後者は観測の失敗であって観測結果ではない。
fn exit(path: &Path, presence: &RunDirPresence) -> String {
    match presence {
        RunDirPresence::Present {
            exit: ExitOutcome::Recorded(code),
        } => format!("{}(値 {})", path.display(), code.get()),
        // ディレクトリごと無いなら exit ファイルも無い。記録の不在として同じ言葉を当てる。
        RunDirPresence::Present {
            exit: ExitOutcome::Absent,
        }
        | RunDirPresence::Absent => format!("{}(記録なし)", path.display()),
        RunDirPresence::Present {
            exit: ExitOutcome::Unreadable(error),
        } => unreadable(error),
        RunDirPresence::Unknown { message: _ } => format!(
            "{}(runディレクトリの有無を確認できないため読んでいません)",
            path.display()
        ),
    }
}

/// 読み取りが失敗した run ディレクトリ内のファイル。
///
/// 破損は構造として受け取ったパスをここで1度だけ出し、機構の失敗はメッセージが既に
/// 対象を含むため前置しない。
fn unreadable(error: &RunFileError) -> String {
    match error {
        RunFileError::Corrupt { path, message } => {
            format!("{}(読み取れません: {message})", path.display())
        }
        RunFileError::Io { message } => format!("読み取れません: {message}"),
    }
}

/// プロセスの同定情報。3値は一組で取り込まれるため、未取得も一組で示す。
fn push_process(out: &mut String, process: Option<&ProcessIdent>) {
    let Some(process) = process else {
        push_sub(out, "同定情報", "未取得(PID・kill同定子・starttime)");
        return;
    };

    push_sub(out, "PID", &process.pid().get().to_string());
    push_sub(out, "kill同定子", process.kill_ident().as_str());
    push_sub(
        out,
        "starttime",
        &format!(
            "{}(記録時刻 {})",
            process.starttime().ident().as_str(),
            process.starttime().wall().to_rfc3339()
        ),
    );
}

/// 凍結の要因と通知。凍結していない実行状態は付随データを持たないため項目も出ない。
fn push_stop(out: &mut String, execution: &ExecutionState) {
    match execution {
        ExecutionState::Stopped {
            reason,
            notified_at,
        } => {
            push_field(out, "凍結要因", stop_reason(*reason));
            push_field(out, "notified_at", &notified(*notified_at));
        }
        ExecutionState::Pending
        | ExecutionState::Launching { .. }
        | ExecutionState::Running
        | ExecutionState::Completed
        | ExecutionState::Failed => {}
    }
}

/// 凍結に至った経路。
fn stop_reason(reason: StopReason) -> &'static str {
    match reason {
        StopReason::RetryLimitExceeded => "リトライ上限の超過",
        StopReason::JudgeLimitExceeded => "判定失敗の上限の超過",
        StopReason::SpawnFailLimitExceeded => "spawn 失敗の上限の超過",
        StopReason::Aborted => "人間による abort",
    }
}

/// 凍結の通知。未通知は次の tick が再通知する(at-least-once)。
fn notified(notified_at: Option<Timestamp>) -> String {
    match notified_at {
        Some(at) => at.to_rfc3339(),
        None => "未記録".to_owned(),
    }
}

/// 直近のツール操作・判定の失敗。
fn last_failure(note: Option<&FailureNote>) -> String {
    let Some(note) = note else {
        return "なし".to_owned();
    };
    format!(
        "{}({}): {}",
        failure_kind(note.kind()),
        note.at().to_rfc3339(),
        note.message()
    )
}

/// 失敗した操作の種別。
fn failure_kind(kind: FailureKind) -> &'static str {
    match kind {
        FailureKind::WorktreeCreate => "worktree の作成",
        FailureKind::WorktreeRemove => "worktree の削除",
        FailureKind::ArchiveMove => "アーカイブへの移動",
        FailureKind::SpawnFail => "エージェントの起動",
        FailureKind::JudgeFail => "判定の実行",
    }
}

/// スナップショットの定義済みステータス一覧。読めない場合は理由を注記する(※6)。
fn defined_statuses(snapshot: &SnapshotInfo) -> String {
    match snapshot {
        SnapshotInfo::Readable(statuses) => statuses
            .iter()
            .map(StatusName::as_str)
            .collect::<Vec<_>>()
            .join(", "),
        SnapshotInfo::Unreadable(message) => format!("読み取れません({message})"),
    }
}

/// 現在 attempt に属する項目行(項目行より1段深い)。
fn push_sub(out: &mut String, label: &str, value: &str) {
    out.push_str("    ");
    out.push_str(label);
    out.push_str(": ");
    out.push_str(value);
    out.push('\n');
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use pulsen_domain::definition::WorkflowName;
    use pulsen_domain::execution::ExitCode;
    use pulsen_domain::task::{
        AttemptNumber, BranchName, KillIdent, Pid, ProcessStartTime, RepoPath, RetryCounters,
        RunDirPath, StartTimeRecord, StateRoot, Target, TaskFilePath, TaskId, WorktreePath,
    };

    use super::*;

    /// 失敗しうる5つの操作と、それぞれの表示。
    ///
    /// `failure_kind` の網羅 `match` と対になる表で、種別の取り違えは語の重複として現れる。
    /// アームが増えたらこの表の長さも合わなくなる。
    const FAILURE_KINDS: [(FailureKind, &str); 5] = [
        (FailureKind::WorktreeCreate, "worktree の作成"),
        (FailureKind::WorktreeRemove, "worktree の削除"),
        (FailureKind::ArchiveMove, "アーカイブへの移動"),
        (FailureKind::SpawnFail, "エージェントの起動"),
        (FailureKind::JudgeFail, "判定の実行"),
    ];

    fn absolute(segments: &[&str]) -> PathBuf {
        let mut path = if std::path::MAIN_SEPARATOR == '\\' {
            PathBuf::from("C:\\")
        } else {
            PathBuf::from("/")
        };
        for segment in segments {
            path.push(segment);
        }
        path
    }

    fn state_root() -> StateRoot {
        StateRoot::parse(absolute(&["home", "u", ".pulsen", "state"])).expect("受理される")
    }

    fn task_id() -> TaskId {
        TaskId::parse("20260812t101112-abcd1234".to_owned()).expect("受理される")
    }

    fn status(name: &str) -> StatusName {
        StatusName::parse(name.to_owned()).expect("受理される")
    }

    fn detail() -> TaskDetail {
        TaskDetail {
            task_id: task_id(),
            workflow_name: WorkflowName::parse("implement".to_owned()).expect("受理される"),
            target: Target::new(
                RepoPath::parse(absolute(&["repos", "pulsen"])).expect("受理される"),
                BranchName::parse("main".to_owned()).expect("受理される"),
            ),
            task_status: status("queued"),
            execution: ExecutionState::Pending,
            workspace: None,
            counters: RetryCounters::initial(),
            limits: Limits {
                retry: RetryLimitInfo::Applicable(2),
                judge: 3,
                spawn: 5,
            },
            attempt: None,
            last_failure: None,
            snapshot: SnapshotInfo::Readable(vec![status("done"), status("queued")]),
            task_file_path: TaskFilePath::active(&state_root(), &task_id()),
            archived: false,
            updated_at: Timestamp::parse_rfc3339("2026-08-12T10:11:12Z").expect("受理される"),
        }
    }

    fn attempt() -> AttemptSummary {
        AttemptSummary {
            number: AttemptNumber::FIRST,
            run_dir: RunDirPath::derive(&state_root(), &task_id(), AttemptNumber::FIRST),
            process: None,
            run_dir_presence: RunDirPresence::Present {
                exit: ExitOutcome::Absent,
            },
        }
    }

    /// 現在 attempt に属する項目行を1つ取り出す。
    fn sub_line<'a>(text: &'a str, label: &str) -> &'a str {
        let prefix = format!("{label}: ");
        text.lines()
            .find(|line| line.trim_start().starts_with(&prefix))
            .expect("項目行がある")
    }

    #[test]
    fn 一度も実行されていないタスクはワークスペースもattemptも縮退で示される() {
        let text = task_detail(&detail());

        assert!(text.contains("workspace_path: 未作成"), "{text}");
        assert!(text.contains("branch: 未作成"), "{text}");
        assert!(text.contains("現在attempt: なし"), "{text}");
        assert!(text.contains("直近の失敗要因: なし"), "{text}");
    }

    #[test]
    fn カウンタには適用される上限が併記される() {
        let text = task_detail(&detail());

        assert!(text.contains("attempt_count: 0(上限 2)"), "{text}");
        assert!(text.contains("judge_attempt_count: 0(上限 3)"), "{text}");
        assert!(text.contains("spawn_fail_count: 0(上限 5)"), "{text}");
    }

    #[test]
    fn 適用対象のない上限は併記されず導出不能とは書き分けられる() {
        let not_applicable = task_detail(&TaskDetail {
            limits: Limits {
                retry: RetryLimitInfo::NotApplicable,
                ..detail().limits
            },
            ..detail()
        });
        let unknown = task_detail(&TaskDetail {
            limits: Limits {
                retry: RetryLimitInfo::Unknown,
                ..detail().limits
            },
            ..detail()
        });

        assert!(
            not_applicable.contains("attempt_count: 0\n"),
            "{not_applicable}"
        );
        assert!(
            !not_applicable.contains("上限 導出不能"),
            "{not_applicable}"
        );
        assert!(
            unknown.contains("attempt_count: 0(上限 導出不能"),
            "{unknown}"
        );
        assert!(
            unknown.contains("judge_attempt_count: 0(上限 3)"),
            "スナップショット非依存の上限は通常どおり出る: {unknown}"
        );
    }

    #[test]
    fn 同定情報が未取り込みのattemptは未取得として示される() {
        let text = task_detail(&TaskDetail {
            attempt: Some(attempt()),
            ..detail()
        });

        assert!(text.contains("現在attempt: 1"), "{text}");
        assert!(text.contains("同定情報: 未取得"), "{text}");
        assert!(text.contains("stdout.log:"), "{text}");
        assert!(text.contains("stderr.log:"), "{text}");
    }

    #[test]
    fn 起動確認済みのattemptはpidとkill同定子とstarttimeを示す() {
        let text = task_detail(&TaskDetail {
            attempt: Some(AttemptSummary {
                process: Some(ProcessIdent::new(
                    Pid::new(4242),
                    KillIdent::parse("-4242".to_owned()).expect("受理される"),
                    StartTimeRecord::new(
                        ProcessStartTime::parse("871234".to_owned()).expect("受理される"),
                        Timestamp::parse_rfc3339("2026-08-12T09:15:30Z").expect("受理される"),
                    ),
                )),
                run_dir_presence: RunDirPresence::Present {
                    exit: ExitOutcome::Recorded(ExitCode::new(0)),
                },
                ..attempt()
            }),
            ..detail()
        });

        assert!(text.contains("PID: 4242"), "{text}");
        assert!(text.contains("kill同定子: -4242"), "{text}");
        assert!(text.contains("starttime: 871234"), "{text}");
        assert!(text.contains("(値 0)"), "{text}");
    }

    #[test]
    fn runディレクトリの不在と確認できないことは書き分けられる() {
        let path = attempt().run_dir.as_path().display().to_string();
        let absent = task_detail(&TaskDetail {
            attempt: Some(AttemptSummary {
                run_dir_presence: RunDirPresence::Absent,
                ..attempt()
            }),
            ..detail()
        });
        let unknown = task_detail(&TaskDetail {
            attempt: Some(AttemptSummary {
                run_dir_presence: RunDirPresence::Unknown {
                    message: format!("{path}: 有無を確認できない: 権限がありません"),
                },
                ..attempt()
            }),
            ..detail()
        });

        assert!(absent.contains("(存在しません)"), "{absent}");
        assert!(
            sub_line(&unknown, "runディレクトリ").contains("存在を確認できません: "),
            "{unknown}"
        );
    }

    #[test]
    fn runディレクトリの有無から決まるexitは記録なしと未読を書き分ける() {
        let path = attempt().run_dir.as_path().display().to_string();
        let absent = task_detail(&TaskDetail {
            attempt: Some(AttemptSummary {
                run_dir_presence: RunDirPresence::Absent,
                ..attempt()
            }),
            ..detail()
        });
        let unknown = task_detail(&TaskDetail {
            attempt: Some(AttemptSummary {
                run_dir_presence: RunDirPresence::Unknown {
                    message: format!("{path}: 有無を確認できない: 権限がありません"),
                },
                ..attempt()
            }),
            ..detail()
        });

        let exit_file = attempt().run_dir.exit_file().display().to_string();
        assert_eq!(
            sub_line(&absent, "exit"),
            format!("    exit: {exit_file}(記録なし)"),
            "{absent}"
        );
        assert_eq!(
            sub_line(&unknown, "exit"),
            format!("    exit: {exit_file}(runディレクトリの有無を確認できないため読んでいません)"),
            "{unknown}"
        );
    }

    #[test]
    fn 存在を確認できないrunディレクトリのパスは1度しか現れない() {
        let path = attempt().run_dir.as_path().display().to_string();
        let text = task_detail(&TaskDetail {
            attempt: Some(AttemptSummary {
                run_dir_presence: RunDirPresence::Unknown {
                    message: format!("{path}: 有無を確認できない: 権限がありません"),
                },
                ..attempt()
            }),
            ..detail()
        });

        assert_eq!(
            sub_line(&text, "runディレクトリ")
                .matches(path.as_str())
                .count(),
            1,
            "{text}"
        );
    }

    #[test]
    fn 読めなかったexitは破損と機構の失敗を書き分けて注記される() {
        let exit_file = attempt().run_dir.exit_file();
        let corrupt = task_detail(&TaskDetail {
            attempt: Some(AttemptSummary {
                run_dir_presence: RunDirPresence::Present {
                    exit: ExitOutcome::Unreadable(RunFileError::Corrupt {
                        path: exit_file.clone(),
                        message: "内容を解釈できない".to_owned(),
                    }),
                },
                ..attempt()
            }),
            ..detail()
        });
        let io = task_detail(&TaskDetail {
            attempt: Some(AttemptSummary {
                run_dir_presence: RunDirPresence::Present {
                    exit: ExitOutcome::Unreadable(RunFileError::Io {
                        message: format!("{}: 読み取れない: 権限がありません", exit_file.display()),
                    }),
                },
                ..attempt()
            }),
            ..detail()
        });

        assert!(
            sub_line(&corrupt, "exit").contains("(読み取れません: 内容を解釈できない)"),
            "{corrupt}"
        );
        assert!(sub_line(&io, "exit").contains("読み取れません: "), "{io}");
    }

    #[test]
    fn 読めなかったexitのパスは1度しか現れない() {
        let exit_file = attempt().run_dir.exit_file();
        let path = exit_file.display().to_string();

        for exit in [
            ExitOutcome::Unreadable(RunFileError::Corrupt {
                path: exit_file.clone(),
                message: "内容を解釈できない".to_owned(),
            }),
            ExitOutcome::Unreadable(RunFileError::Io {
                message: format!("{path}: 読み取れない: 権限がありません"),
            }),
        ] {
            let text = task_detail(&TaskDetail {
                attempt: Some(AttemptSummary {
                    run_dir_presence: RunDirPresence::Present { exit: exit.clone() },
                    ..attempt()
                }),
                ..detail()
            });

            assert_eq!(
                sub_line(&text, "exit").matches(path.as_str()).count(),
                1,
                "{exit:?}: {text}"
            );
        }
    }

    #[test]
    fn 凍結したタスクは要因と通知の有無を示す() {
        let text = task_detail(&TaskDetail {
            execution: ExecutionState::Stopped {
                reason: StopReason::SpawnFailLimitExceeded,
                notified_at: None,
            },
            last_failure: Some(
                FailureNote::parse(
                    FailureKind::SpawnFail,
                    "エージェント `claude` は定義されていません".to_owned(),
                    Timestamp::parse_rfc3339("2026-08-12T10:00:00Z").expect("受理される"),
                )
                .expect("受理される"),
            ),
            ..detail()
        });

        assert!(text.contains("凍結要因: spawn 失敗の上限の超過"), "{text}");
        assert!(text.contains("notified_at: 未記録"), "{text}");
        assert!(
            text.contains("直近の失敗要因: エージェントの起動(2026-08-12T10:00:00Z): "),
            "{text}"
        );
    }

    #[test]
    fn 通知済みの凍結は通知時刻を更新日時と同じ形式で示す() {
        let text = task_detail(&TaskDetail {
            execution: ExecutionState::Stopped {
                reason: StopReason::Aborted,
                notified_at: Some(
                    Timestamp::parse_rfc3339("2026-08-12T10:00:00Z").expect("受理される"),
                ),
            },
            ..detail()
        });

        assert!(text.contains("notified_at: 2026-08-12T10:00:00Z"), "{text}");
        assert!(text.contains("更新日時: 2026-08-12T10:11:12Z"), "{text}");
    }

    #[test]
    fn 直近の失敗要因は失敗した操作ごとに書き分けられる() {
        for (kind, word) in FAILURE_KINDS {
            let text = task_detail(&TaskDetail {
                last_failure: Some(
                    FailureNote::parse(
                        kind,
                        "権限がありません".to_owned(),
                        Timestamp::parse_rfc3339("2026-08-12T10:00:00Z").expect("受理される"),
                    )
                    .expect("受理される"),
                ),
                ..detail()
            });

            assert!(
                text.contains(&format!(
                    "直近の失敗要因: {word}(2026-08-12T10:00:00Z): 権限がありません"
                )),
                "{kind:?}: {text}"
            );
        }
    }

    #[test]
    fn 凍結していない実行状態は凍結要因も通知も出さない() {
        for execution in [
            ExecutionState::Pending,
            ExecutionState::Launching {
                recorded_at: Timestamp::parse_rfc3339("2026-08-12T10:00:00Z").expect("受理される"),
            },
            ExecutionState::Running,
            ExecutionState::Completed,
            ExecutionState::Failed,
        ] {
            let text = task_detail(&TaskDetail {
                execution: execution.clone(),
                ..detail()
            });

            assert!(!text.contains("凍結要因"), "{execution:?}: {text}");
            assert!(!text.contains("notified_at"), "{execution:?}: {text}");
        }
    }

    #[test]
    fn スナップショット破損では定義済みステータスの代わりに理由が注記される() {
        let text = task_detail(&TaskDetail {
            snapshot: SnapshotInfo::Unreadable("statuses が空".to_owned()),
            limits: Limits {
                retry: RetryLimitInfo::Unknown,
                ..detail().limits
            },
            ..detail()
        });

        assert!(
            text.contains("定義済みステータス: 読み取れません(statuses が空)"),
            "{text}"
        );
        assert!(
            text.contains("スナップショット保存先: "),
            "保存先は読める項目として残る: {text}"
        );
    }

    #[test]
    fn アーカイブ済みはその旨とworktreeの削除済みを明示する() {
        let text = task_detail(&TaskDetail {
            archived: true,
            workspace: Some(Workspace::new(
                WorktreePath::parse(absolute(&["home", "u", ".pulsen", "worktrees", "t"]))
                    .expect("受理される"),
                BranchName::parse("pulsen/t".to_owned()).expect("受理される"),
            )),
            task_file_path: TaskFilePath::archived(&state_root(), &task_id()),
            ..detail()
        });

        assert!(
            text.contains("在籍: アーカイブ済み(worktree は削除済み)"),
            "{text}"
        );
        assert!(text.contains("(削除済み)"), "{text}");
        assert!(
            text.contains("archive"),
            "保存先はアーカイブ側になる: {text}"
        );
    }
}
