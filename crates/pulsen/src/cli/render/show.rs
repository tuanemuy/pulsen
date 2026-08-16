//! `show` の文言。
//!
//! 縮退の場合分けはユースケースが直和型に畳んで渡すため、ここは網羅 `match` で
//! 言葉を当てるだけにする。**workspace_path は表示するのみで存在を検証しない**
//! (pages ※9)。

use std::path::Path;

use pulsen_domain::definition::StatusName;
use pulsen_domain::task::{
    FailureKind, FailureNote, ProcessIdent, ReadError, StopReason, Workspace,
};

use crate::application::show_task::{
    AttemptSummary, ExitInfo, Limits, RetryLimitInfo, RunDirPresence, ShowTaskError, StopInfo,
    TaskDetail,
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
    push_field(&mut out, "実行状態", detail.execution_state.as_str());
    push_field(&mut out, "在籍", &residence(detail.archived));

    if let Some(stop) = detail.stop_info {
        push_field(&mut out, "凍結要因", stop_reason(stop.reason));
        push_field(&mut out, "通知", &notified(&stop));
    }

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
        &defined_statuses(
            detail.defined_statuses.as_deref(),
            detail.snapshot_error.as_deref(),
        ),
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
        &run_dir(attempt.run_dir.as_path(), &attempt.run_dir_exists),
    );
    push_process(out, attempt.process.as_ref());
    push_sub(out, "stdout.log", &attempt.stdout_log.display().to_string());
    push_sub(out, "stderr.log", &attempt.stderr_log.display().to_string());
    push_sub(
        out,
        "exit",
        &exit(&attempt.run_dir.exit_file(), &attempt.exit),
    );
}

/// run ディレクトリのパスと、その有無。
fn run_dir(path: &Path, presence: &RunDirPresence) -> String {
    let path = path.display().to_string();
    match presence {
        RunDirPresence::Present => path,
        RunDirPresence::Absent => format!("{path}(存在しません)"),
        RunDirPresence::Unknown { message } => {
            format!("{path}(存在を確認できません: {message})")
        }
    }
}

/// exit ファイルのパスと、読み取った値。
fn exit(path: &Path, exit: &ExitInfo) -> String {
    let path = path.display().to_string();
    match exit {
        ExitInfo::Recorded(code) => format!("{path}(値 {})", code.get()),
        ExitInfo::Absent => format!("{path}(記録なし)"),
        ExitInfo::Unreadable { message } => format!("{path}(読み取れません: {message})"),
        ExitInfo::Unread => {
            format!("{path}(runディレクトリの有無を確認できないため読んでいません)")
        }
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

/// 凍結に至った経路。
fn stop_reason(reason: StopReason) -> &'static str {
    match reason {
        StopReason::RetryLimitExceeded => "リトライ上限の超過",
        StopReason::JudgeLimitExceeded => "判定失敗の上限の超過",
        StopReason::SpawnFailLimitExceeded => "spawn 失敗の上限の超過",
        StopReason::Aborted => "利用者による中断",
    }
}

/// 凍結の通知。未通知は次の tick が再通知する(at-least-once)。
fn notified(stop: &StopInfo) -> String {
    match stop.notified_at {
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
fn defined_statuses(statuses: Option<&[StatusName]>, snapshot_error: Option<&str>) -> String {
    match statuses {
        Some(statuses) => statuses
            .iter()
            .map(StatusName::as_str)
            .collect::<Vec<_>>()
            .join(", "),
        None => match snapshot_error {
            Some(message) => format!("読み取れません({message})"),
            None => "読み取れません".to_owned(),
        },
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
        AttemptNumber, BranchName, ExecutionStateKind, KillIdent, Pid, ProcessStartTime, RepoPath,
        RetryCounters, RunDirPath, StartTimeRecord, StateRoot, Target, TaskFilePath, TaskId,
        Timestamp, WorktreePath,
    };

    use super::*;

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
            execution_state: ExecutionStateKind::Pending,
            workspace: None,
            counters: RetryCounters::initial(),
            limits: Limits {
                retry: RetryLimitInfo::Applicable(2),
                judge: 3,
                spawn: 5,
            },
            attempt: None,
            last_failure: None,
            stop_info: None,
            defined_statuses: Some(vec![status("done"), status("queued")]),
            snapshot_error: None,
            task_file_path: TaskFilePath::active(&state_root(), &task_id()),
            archived: false,
            updated_at: Timestamp::parse_rfc3339("2026-08-12T10:11:12Z").expect("受理される"),
        }
    }

    fn attempt() -> AttemptSummary {
        let run_dir = RunDirPath::derive(&state_root(), &task_id(), AttemptNumber::FIRST);
        AttemptSummary {
            number: AttemptNumber::FIRST,
            run_dir: run_dir.clone(),
            process: None,
            exit: ExitInfo::Absent,
            stdout_log: run_dir.stdout_log(),
            stderr_log: run_dir.stderr_log(),
            run_dir_exists: RunDirPresence::Present,
        }
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
                exit: ExitInfo::Recorded(ExitCode::new(0)),
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
        let absent = task_detail(&TaskDetail {
            attempt: Some(AttemptSummary {
                run_dir_exists: RunDirPresence::Absent,
                ..attempt()
            }),
            ..detail()
        });
        let unknown = task_detail(&TaskDetail {
            attempt: Some(AttemptSummary {
                run_dir_exists: RunDirPresence::Unknown {
                    message: "権限がありません".to_owned(),
                },
                exit: ExitInfo::Unread,
                ..attempt()
            }),
            ..detail()
        });

        assert!(absent.contains("(存在しません)"), "{absent}");
        assert!(
            unknown.contains("(存在を確認できません: 権限がありません)"),
            "{unknown}"
        );
    }

    #[test]
    fn 読めなかったexitは注記として示される() {
        let text = task_detail(&TaskDetail {
            attempt: Some(AttemptSummary {
                exit: ExitInfo::Unreadable {
                    message: "内容を解釈できない".to_owned(),
                },
                ..attempt()
            }),
            ..detail()
        });

        assert!(
            text.contains("(読み取れません: 内容を解釈できない)"),
            "{text}"
        );
    }

    #[test]
    fn 凍結したタスクは要因と通知の有無を示す() {
        let text = task_detail(&TaskDetail {
            execution_state: ExecutionStateKind::Stopped,
            stop_info: Some(StopInfo {
                reason: StopReason::SpawnFailLimitExceeded,
                notified_at: None,
            }),
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
        assert!(text.contains("通知: 未記録"), "{text}");
        assert!(
            text.contains("直近の失敗要因: エージェントの起動(2026-08-12T10:00:00Z): "),
            "{text}"
        );
    }

    #[test]
    fn スナップショット破損では定義済みステータスの代わりに理由が注記される() {
        let text = task_detail(&TaskDetail {
            defined_statuses: None,
            snapshot_error: Some("statuses が空".to_owned()),
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
