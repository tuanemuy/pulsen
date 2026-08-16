//! `ls` の文言。
//!
//! 読めたタスクは1行1タスクの桁揃えテーブル、読めなかったファイルは一覧の後ろの
//! 独立したブロックに置く(adr.md ADR-001)。破損ファイルはタスクIDもステータスも
//! 読めておらず、テーブルの列を埋められない。

use pulsen_domain::task::{ReadError, StateKindError};

use crate::application::list_tasks::{ListTasksError, TaskList, TaskRow, UnreadableRow};
use crate::cli::ls::LsError;

use super::{problem, wire_error};

/// 一覧の列見出し。`spec/pages/index.md#ls` の「機能」に書かれた順で固定する。
const HEADERS: [&str; COLUMNS] = [
    "タスクID",
    "ワークフロー",
    "リポジトリ",
    "ブランチ",
    "ステータス",
    "実行状態",
    "attempt",
    "更新日時",
    "備考",
];

/// 一覧の列数。
const COLUMNS: usize = 9;

/// 値が無い項目に置く記号。
///
/// 空欄にしない — 「値が無い」と「列がずれた」を区別できなくなる。
const UNSET: &str = "(未作成)";

/// タスクの一覧。該当が無ければ空である旨だけを返す。
pub fn task_list(list: &TaskList) -> String {
    let mut out = if list.rows.is_empty() {
        "該当するタスクはありません。".to_owned()
    } else {
        table(&list.rows)
    };

    // 破損の報告は絞り込みの結果に関わらず出す — 修復の入口を絞り込みで消さない。
    if !list.unreadable.is_empty() {
        out.push_str("\n\n");
        out.push_str(&unreadable(&list.unreadable));
    }
    out
}

/// `ls` の失敗。
pub fn ls_error(error: &LsError) -> String {
    match error {
        LsError::Wire(error) => wire_error(error),
        LsError::List(ListTasksError::InvalidState(StateKindError::Unknown { given, valid })) => {
            problem(
                "--state の値が不正です。",
                &[
                    format!("指定: `{given}`"),
                    format!("有効な値: {}", valid.join(" / ")),
                ],
            )
        }
        LsError::List(ListTasksError::Scan(ReadError::Io { message })) => {
            problem("タスクを走査できません。", &[format!("原因: {message}")])
        }
    }
}

/// 桁揃えのテーブル。ヘッダー行を1行置く。
fn table(rows: &[TaskRow]) -> String {
    let cells: Vec<[String; COLUMNS]> = rows.iter().map(row_cells).collect();
    let widths = widths(&cells);

    let mut out = line(&HEADERS.map(str::to_owned), &widths);
    for row in &cells {
        out.push('\n');
        out.push_str(&line(row, &widths));
    }
    out
}

/// 1行分の値。
fn row_cells(row: &TaskRow) -> [String; COLUMNS] {
    [
        row.task_id.as_str().to_owned(),
        row.workflow_name.as_str().to_owned(),
        row.repo.as_path().display().to_string(),
        row.branch
            .as_ref()
            .map_or_else(|| UNSET.to_owned(), |branch| branch.as_str().to_owned()),
        row.task_status.as_str().to_owned(),
        row.execution_state.as_str().to_owned(),
        row.attempt_count.to_string(),
        row.updated_at.to_rfc3339(),
        notes(row),
    ]
}

/// 行の末尾に付ける印。
fn notes(row: &TaskRow) -> String {
    let mut notes = Vec::new();
    if row.archived {
        notes.push("アーカイブ済み");
    }
    if row.snapshot_unreadable {
        notes.push("スナップショット読み取り不能");
    }
    notes.join(" / ")
}

/// 列ごとの幅(文字数)。
fn widths(rows: &[[String; COLUMNS]]) -> [usize; COLUMNS] {
    let mut widths = HEADERS.map(|header| header.chars().count());
    for row in rows {
        for (width, cell) in widths.iter_mut().zip(row) {
            *width = (*width).max(cell.chars().count());
        }
    }
    widths
}

/// 値を列幅に合わせて並べた1行。末尾の余白は残さない。
///
/// 桁揃えは文字数で行う。長い値は切り詰めない — 成果の回収に使うリポジトリのパスや
/// ブランチ名が読めなくなる(ADR-001)。
fn line(cells: &[String; COLUMNS], widths: &[usize; COLUMNS]) -> String {
    let mut out = String::new();
    for (cell, width) in cells.iter().zip(widths) {
        out.push_str(cell);
        out.push_str(&" ".repeat(width.saturating_sub(cell.chars().count())));
        out.push_str("  ");
    }
    out.trim_end().to_owned()
}

/// 読めなかったタスクファイルの報告。パスと理由が修復の入口になる(pages ※5)。
fn unreadable(rows: &[UnreadableRow]) -> String {
    let mut out = format!("読み取れなかったタスクファイル({}件):\n", rows.len());
    for row in rows {
        out.push_str(&format!("  - {}: {}\n", row.path.display(), row.message));
    }
    out.push_str("  内容を直接確認して修復してください。");
    out
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use pulsen_domain::definition::{StatusName, WorkflowName};
    use pulsen_domain::task::{
        BranchName, ExecutionStateKind, RepoPath, StateKindError, TaskId, Timestamp,
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

    fn row(id: &str) -> TaskRow {
        TaskRow {
            task_id: TaskId::parse(id.to_owned()).expect("受理される"),
            workflow_name: WorkflowName::parse("implement".to_owned()).expect("受理される"),
            repo: RepoPath::parse(absolute(&["repos", "pulsen"])).expect("受理される"),
            branch: None,
            task_status: StatusName::parse("queued".to_owned()).expect("受理される"),
            execution_state: ExecutionStateKind::Pending,
            attempt_count: 0,
            updated_at: Timestamp::parse_rfc3339("2026-08-12T10:11:12Z").expect("受理される"),
            archived: false,
            snapshot_unreadable: false,
        }
    }

    #[test]
    fn 該当がなければ空である旨を表示する() {
        assert_eq!(
            task_list(&TaskList::default()),
            "該当するタスクはありません。"
        );
    }

    #[test]
    fn 各行にタスクステータスと実行状態がともに現れる() {
        let text = task_list(&TaskList {
            rows: vec![TaskRow {
                task_status: StatusName::parse("running".to_owned()).expect("受理される"),
                execution_state: ExecutionStateKind::Running,
                ..row("20260812t101112-abcd1234")
            }],
            unreadable: Vec::new(),
        });
        let row = text.lines().nth(1).expect("見出しの次に行がある");

        assert!(row.contains("20260812t101112-abcd1234"), "{row}");
        assert!(row.contains("implement"), "{row}");
        assert!(row.contains("2026-08-12T10:11:12Z"), "{row}");
        assert_eq!(
            row.matches("running").count(),
            2,
            "同名でもタスクステータスと実行状態の両方が出る: {row}"
        );
    }

    #[test]
    fn 未確定のブランチは空欄ではなく記号で埋まる() {
        let text = task_list(&TaskList {
            rows: vec![row("20260812t101112-abcd1234")],
            unreadable: Vec::new(),
        });

        assert!(text.contains(UNSET), "{text}");
    }

    #[test]
    fn アーカイブ済みとスナップショット破損は行の印で判別できる() {
        let text = task_list(&TaskList {
            rows: vec![
                TaskRow {
                    archived: true,
                    branch: Some(BranchName::parse("pulsen/a".to_owned()).expect("受理される")),
                    ..row("20260812t101112-aaaa0001")
                },
                TaskRow {
                    snapshot_unreadable: true,
                    ..row("20260812t101112-aaaa0002")
                },
            ],
            unreadable: Vec::new(),
        });

        let archived = text.lines().nth(1).expect("1行目がある");
        assert!(archived.contains("アーカイブ済み"), "{archived}");
        assert!(
            archived.contains("pulsen/a"),
            "アーカイブ済みでもブランチが出る: {archived}"
        );
        let degraded = text.lines().nth(2).expect("2行目がある");
        assert!(
            degraded.contains("スナップショット読み取り不能"),
            "{degraded}"
        );
    }

    #[test]
    fn 読めなかったファイルは一覧の後ろにパスと理由つきで並ぶ() {
        let text = task_list(&TaskList {
            rows: vec![row("20260812t101112-abcd1234")],
            unreadable: vec![UnreadableRow {
                path: PathBuf::from("/home/u/.pulsen/state/tasks/broken.json"),
                message: "JSON として読めない".to_owned(),
            }],
        });

        assert!(
            text.contains("20260812t101112-abcd1234"),
            "残りのタスクは表示される: {text}"
        );
        assert!(
            text.contains("/home/u/.pulsen/state/tasks/broken.json: JSON として読めない"),
            "{text}"
        );
    }

    #[test]
    fn 一覧が空でも読めなかったファイルは報告される() {
        let text = task_list(&TaskList {
            rows: Vec::new(),
            unreadable: vec![UnreadableRow {
                path: PathBuf::from("/home/u/.pulsen/state/tasks/broken.json"),
                message: "JSON として読めない".to_owned(),
            }],
        });

        assert!(text.starts_with("該当するタスクはありません。"), "{text}");
        assert!(text.contains("broken.json"), "{text}");
    }

    #[test]
    fn 実行状態の不正値は有効値6つを添えて拒否される() {
        let text = ls_error(&LsError::List(ListTasksError::InvalidState(
            StateKindError::Unknown {
                given: "Pending".to_owned(),
                valid: ExecutionStateKind::VALID,
            },
        )));

        assert!(
            text.starts_with("エラー: --state の値が不正です。"),
            "{text}"
        );
        assert!(text.contains("指定: `Pending`"), "{text}");
        for valid in ExecutionStateKind::VALID {
            assert!(
                text.contains(valid),
                "有効な値 {valid} が案内される: {text}"
            );
        }
    }

    #[test]
    fn 走査自体の失敗は原因つきで案内される() {
        assert_eq!(
            ls_error(&LsError::List(ListTasksError::Scan(ReadError::Io {
                message: "タスクの置き場を読めない".to_owned(),
            }))),
            "エラー: タスクを走査できません。\n  原因: タスクの置き場を読めない"
        );
    }
}
