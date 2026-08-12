//! タスクID。

/// タスクIDの生成失敗。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskIdError {
    /// 空文字列。
    Empty,
    /// 最大長を超える。
    TooLong,
    /// 使える文字集合の外の文字を含む。
    InvalidChar {
        /// 見つかった文字。
        char: char,
        /// 0 始まりの位置。
        position: usize,
    },
    /// 先頭が英数字でない。
    InvalidLeadingChar,
}

impl TaskIdError {
    /// 制約の説明。
    ///
    /// タスクファイルの復号に失敗した理由は、破損したタスクの一覧・表示を通じて
    /// 利用者に見せる修復の材料になる。説明の定義箇所をドメインに1つ置く。
    pub fn describe(&self) -> String {
        match self {
            Self::Empty => "空文字列は指定できません".to_owned(),
            Self::TooLong => format!("{}文字を超えられません", TaskId::MAX_LENGTH),
            // 制御文字や空白も位置つきで見えるように、文字は Debug 表現で示す。
            Self::InvalidChar { char, position } => format!(
                "{}文字目に使えない文字({char:?})があります。使えるのは英小文字・数字・`-` です",
                position + 1
            ),
            Self::InvalidLeadingChar => "先頭は英小文字か数字である必要があります".to_owned(),
        }
    }
}

/// タスクID。
///
/// 文字集合を `[a-z0-9-]`(先頭は英数字)に閉じることで、ファイル名の主部としても
/// `pulsen/<task-id>` という git ブランチ名の構成要素としても常に安全になる。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TaskId(String);

impl TaskId {
    /// 最大長。
    pub const MAX_LENGTH: usize = 64;

    /// 1〜64文字・`[a-z0-9-]`・先頭英数字としてのみ生成する。
    pub fn parse(s: String) -> Result<Self, TaskIdError> {
        let Some(leading) = s.chars().next() else {
            return Err(TaskIdError::Empty);
        };
        if s.chars().count() > Self::MAX_LENGTH {
            return Err(TaskIdError::TooLong);
        }
        for (position, found) in s.chars().enumerate() {
            if !Self::is_allowed(found) {
                return Err(TaskIdError::InvalidChar {
                    char: found,
                    position,
                });
            }
        }
        if !leading.is_ascii_alphanumeric() {
            return Err(TaskIdError::InvalidLeadingChar);
        }
        Ok(Self(s))
    }

    /// 保持する文字列を借用する。
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// 保持する文字列を取り出す。
    pub fn into_string(self) -> String {
        self.0
    }

    fn is_allowed(c: char) -> bool {
        c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 空のタスクidは受理されない() {
        assert_eq!(TaskId::parse(String::new()), Err(TaskIdError::Empty));
    }

    #[test]
    fn 上限を超える長さのタスクidは受理されない() {
        let at_limit = "a".repeat(TaskId::MAX_LENGTH);
        assert!(TaskId::parse(at_limit).is_ok());

        let over_limit = "a".repeat(TaskId::MAX_LENGTH + 1);
        assert_eq!(TaskId::parse(over_limit), Err(TaskIdError::TooLong));
    }

    #[test]
    fn 文字集合の外の文字を含むタスクidは位置つきで拒否される() {
        assert_eq!(
            TaskId::parse("ab_cd".to_owned()),
            Err(TaskIdError::InvalidChar {
                char: '_',
                position: 2
            })
        );
        assert_eq!(
            TaskId::parse("abC".to_owned()),
            Err(TaskIdError::InvalidChar {
                char: 'C',
                position: 2
            })
        );
        assert_eq!(
            TaskId::parse("a.b".to_owned()),
            Err(TaskIdError::InvalidChar {
                char: '.',
                position: 1
            })
        );
        assert_eq!(
            TaskId::parse("a/b".to_owned()),
            Err(TaskIdError::InvalidChar {
                char: '/',
                position: 1
            })
        );
    }

    #[test]
    fn 先頭が英数字でないタスクidは受理されない() {
        assert_eq!(
            TaskId::parse("-abc".to_owned()),
            Err(TaskIdError::InvalidLeadingChar)
        );
    }

    #[test]
    fn 発行形式のタスクidは受理される() {
        let id = TaskId::parse("20260811t091530-k3f9qa1b".to_owned()).expect("受理される");
        assert_eq!(id.as_str(), "20260811t091530-k3f9qa1b");
    }

    #[test]
    fn タスクidの等価性は文字列の完全一致で決まる() {
        assert_eq!(
            TaskId::parse("a1".to_owned()),
            TaskId::parse("a1".to_owned())
        );
        assert_ne!(
            TaskId::parse("a1".to_owned()),
            TaskId::parse("a2".to_owned())
        );
    }
}
