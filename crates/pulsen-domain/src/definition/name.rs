//! 名前系の文字列 newtype。いずれも `parse` を通してのみ生成する。

/// 名前系 newtype の生成失敗。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NameError {
    /// 空文字列。
    Empty,
    /// 前後に空白を含む。
    SurroundingWhitespace,
}

macro_rules! trimmed_name {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            /// 非空かつ前後に空白のない文字列としてのみ生成する。
            pub fn parse(s: String) -> Result<Self, NameError> {
                if s.is_empty() {
                    return Err(NameError::Empty);
                }
                if s.trim() != s {
                    return Err(NameError::SurroundingWhitespace);
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
        }
    };
}

trimmed_name! {
    /// エージェント名。グローバル設定の `agents` のキー。
    AgentName
}

trimmed_name! {
    /// モデル名。`{model}` へ渡る値。
    ModelName
}

trimmed_name! {
    /// スキル名。`skill_input` テンプレートの `{skill}` へ渡る値。
    SkillName
}

trimmed_name! {
    /// ステータス名。CLI 引数・YAML キーとして使われる。
    StatusName
}

trimmed_name! {
    /// ワークフローの表示名。タスクに記録される。
    WorkflowName
}

/// エージェントに与えるプロンプト。前後の空白は書き手の意図として保つ。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Prompt(String);

impl Prompt {
    /// 非空の文字列としてのみ生成する。
    pub fn parse(s: String) -> Result<Self, NameError> {
        if s.is_empty() {
            return Err(NameError::Empty);
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
}

/// `{input}` へ渡る値。プロンプトそのもの、または `skill_input` の展開結果。
///
/// 制約を持たないため生成は無謬にする。`SkillInputTemplate::render` のように
/// 失敗しえない経路が `Result` を扱わずに済む。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InputText(String);

impl InputText {
    /// 任意の文字列から生成する。
    pub fn new(s: String) -> Self {
        Self(s)
    }

    /// 保持する文字列を借用する。
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// 保持する文字列を取り出す。
    pub fn into_string(self) -> String {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 空文字列の名前は受理されない() {
        assert_eq!(AgentName::parse(String::new()), Err(NameError::Empty));
        assert_eq!(ModelName::parse(String::new()), Err(NameError::Empty));
        assert_eq!(SkillName::parse(String::new()), Err(NameError::Empty));
        assert_eq!(StatusName::parse(String::new()), Err(NameError::Empty));
        assert_eq!(WorkflowName::parse(String::new()), Err(NameError::Empty));
    }

    #[test]
    fn 前後に空白のある名前は受理されない() {
        assert_eq!(
            AgentName::parse(" claude".to_owned()),
            Err(NameError::SurroundingWhitespace)
        );
        assert_eq!(
            StatusName::parse("queued\n".to_owned()),
            Err(NameError::SurroundingWhitespace)
        );
        assert_eq!(
            WorkflowName::parse("implement ".to_owned()),
            Err(NameError::SurroundingWhitespace)
        );
    }

    #[test]
    fn 空白だけの名前は前後空白として拒否される() {
        assert_eq!(
            AgentName::parse("   ".to_owned()),
            Err(NameError::SurroundingWhitespace)
        );
    }

    #[test]
    fn 名前の内部の空白は受理される() {
        let name = AgentName::parse("claude code".to_owned()).expect("受理される");
        assert_eq!(name.as_str(), "claude code");
    }

    #[test]
    fn 名前の等価性は文字列の完全一致で決まる() {
        assert_eq!(
            AgentName::parse("claude".to_owned()),
            AgentName::parse("claude".to_owned())
        );
        assert_ne!(
            AgentName::parse("claude".to_owned()),
            AgentName::parse("Claude".to_owned())
        );
    }

    #[test]
    fn 空文字列のプロンプトは受理されない() {
        assert_eq!(Prompt::parse(String::new()), Err(NameError::Empty));
    }

    #[test]
    fn プロンプトは前後の空白を許容する() {
        let prompt = Prompt::parse("  実装して\n".to_owned()).expect("受理される");
        assert_eq!(prompt.as_str(), "  実装して\n");
    }

    #[test]
    fn 入力テキストは制約なく生成できる() {
        assert_eq!(InputText::new(String::new()).as_str(), "");
        assert_eq!(InputText::new(" a ".to_owned()).as_str(), " a ");
    }
}
