//! 直近の失敗要因。

use super::time::Timestamp;

/// 失敗した操作の種別。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureKind {
    /// worktree の作成。
    WorktreeCreate,
    /// worktree の削除。
    WorktreeRemove,
    /// アーカイブへの移動。
    ArchiveMove,
    /// エージェントの起動。
    SpawnFail,
    /// 判定の実行。
    JudgeFail,
}

/// ツール操作の失敗種別。
///
/// `attempt_count` を進める遷移が受け取る種別を、この3値に閉じる。spawn と判定の失敗は
/// 専用の遷移が `spawn_fail_count` / `judge_attempt_count` とともに記録するため、
/// カウンタと失敗種別が食い違った帳簿は型として書けない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolFailureKind {
    /// worktree の作成。
    WorktreeCreate,
    /// worktree の削除。
    WorktreeRemove,
    /// アーカイブへの移動。
    ArchiveMove,
}

impl ToolFailureKind {
    /// 帳簿に残す失敗種別。
    pub(super) fn recorded(self) -> FailureKind {
        match self {
            Self::WorktreeCreate => FailureKind::WorktreeCreate,
            Self::WorktreeRemove => FailureKind::WorktreeRemove,
            Self::ArchiveMove => FailureKind::ArchiveMove,
        }
    }
}

/// 失敗要因の生成失敗。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FailureNoteError {
    /// 説明が空。
    EmptyMessage,
}

impl FailureNoteError {
    /// 制約の説明。
    ///
    /// タスクファイルの復号に失敗した理由は、破損したタスクの一覧・表示を通じて
    /// 利用者に見せる修復の材料になる。説明の定義箇所をドメインに1つ置く。
    pub fn describe(&self) -> &'static str {
        match self {
            Self::EmptyMessage => "説明に空文字列は指定できません",
        }
    }
}

/// 直近のツール操作・判定の失敗の記録。
///
/// 新しい失敗の発生時にのみ上書きし、成功時にクリアしない(「直近の」失敗要因)。
/// エージェント実行自体の失敗はここに記録しない — 証跡は run ディレクトリにある。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailureNote {
    kind: FailureKind,
    message: String,
    at: Timestamp,
}

impl FailureNote {
    /// 説明が得られなかったときの既定文言。
    const UNSPECIFIED_MESSAGE: &'static str = "詳細不明";

    /// 遷移関数が失敗を記録するための総関数。空の説明は既定文言に畳む。
    ///
    /// 説明はポートが返す不透明な文字列で、非空を型で保証できない。空文字列のために
    /// 遷移関数へ失敗経路を増やすより、既定文言を与えて記録を必ず残す(ADR-036)。
    pub(super) fn record(kind: FailureKind, message: String, at: Timestamp) -> Self {
        let message = if message.is_empty() {
            Self::UNSPECIFIED_MESSAGE.to_owned()
        } else {
            message
        };
        Self { kind, message, at }
    }

    /// 非空の説明を伴う失敗としてのみ生成する。
    pub fn parse(
        kind: FailureKind,
        message: String,
        at: Timestamp,
    ) -> Result<Self, FailureNoteError> {
        if message.is_empty() {
            return Err(FailureNoteError::EmptyMessage);
        }
        Ok(Self { kind, message, at })
    }

    /// 失敗した操作の種別。
    pub fn kind(&self) -> FailureKind {
        self.kind
    }

    /// 失敗の説明。
    pub fn message(&self) -> &str {
        &self.message
    }

    /// 失敗した時刻。
    pub fn at(&self) -> Timestamp {
        self.at
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at() -> Timestamp {
        Timestamp::parse_rfc3339("2026-08-11T09:15:30Z").expect("受理される")
    }

    #[test]
    fn 説明のない失敗要因は受理されない() {
        assert_eq!(
            FailureNote::parse(FailureKind::SpawnFail, String::new(), at()),
            Err(FailureNoteError::EmptyMessage)
        );
    }

    #[test]
    fn ツール操作の失敗種別は対応する失敗種別として記録される() {
        for (tool, expected) in [
            (ToolFailureKind::WorktreeCreate, FailureKind::WorktreeCreate),
            (ToolFailureKind::WorktreeRemove, FailureKind::WorktreeRemove),
            (ToolFailureKind::ArchiveMove, FailureKind::ArchiveMove),
        ] {
            assert_eq!(tool.recorded(), expected, "{tool:?}");
        }
    }

    #[test]
    fn 失敗要因は種別と説明と時刻を保持する() {
        let note = FailureNote::parse(
            FailureKind::WorktreeCreate,
            "git worktree add に失敗".to_owned(),
            at(),
        )
        .expect("受理される");

        assert_eq!(note.kind(), FailureKind::WorktreeCreate);
        assert_eq!(note.message(), "git worktree add に失敗");
        assert_eq!(note.at(), at());
    }
}
