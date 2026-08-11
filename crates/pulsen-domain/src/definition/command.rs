//! YAML の「文字列またはトークン配列」から落とすトークン列。

/// コマンドの生成失敗。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandError {
    /// トークンが0個(空文字列・空白のみ・空配列)。
    Empty,
}

impl CommandError {
    /// 制約の説明。
    ///
    /// 同じ制約は config.yaml とワークフロー定義YAMLの両方で破れるため、説明の定義箇所を
    /// ドメインに1つ置く。層ごとに書くと同じ誤りの案内が食い違う。
    pub fn describe(&self) -> &'static str {
        match self {
            Self::Empty => "コマンドが空です",
        }
    }
}

macro_rules! token_list {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct $name {
            tokens: Vec<String>,
        }

        impl $name {
            /// 文字列形式。単純な空白分割で、連続空白は1区切りになる。
            /// クォートはグルーピングとして解釈しない。
            pub fn parse_text(text: &str) -> Result<Self, CommandError> {
                Self::new(text.split_whitespace().map(str::to_owned).collect())
            }

            /// 配列形式。各要素をそのままトークンとする。
            /// 空文字列トークンは意図的な空引数の表現として許容する。
            pub fn parse_tokens(tokens: Vec<String>) -> Result<Self, CommandError> {
                Self::new(tokens)
            }

            fn new(tokens: Vec<String>) -> Result<Self, CommandError> {
                if tokens.is_empty() {
                    return Err(CommandError::Empty);
                }
                Ok(Self { tokens })
            }

            /// トークン列を借用する。先頭がプログラム。
            pub fn tokens(&self) -> &[String] {
                &self.tokens
            }
        }
    };
}

token_list! {
    /// 構造解釈済みのトークン列。テンプレートとしての解釈はまだ行っていない。
    RawCommand
}

token_list! {
    /// 判定・通知に使うトークン列。プレースホルダ展開は行われず、
    /// `{...}` を含んでいても文字どおり渡る(requirements §6.3)。
    PlainCommand
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 文字列形式のコマンドは空白で分割される() {
        let command = RawCommand::parse_text("claude -p {input}").expect("受理される");
        assert_eq!(command.tokens(), ["claude", "-p", "{input}"]);
    }

    #[test]
    fn 文字列形式の連続空白は1区切りとして扱われる() {
        let command = RawCommand::parse_text("  claude \t -p \n {input}  ").expect("受理される");
        assert_eq!(command.tokens(), ["claude", "-p", "{input}"]);
    }

    #[test]
    fn 文字列形式のクォートはグルーピングとして解釈されない() {
        let command = RawCommand::parse_text("sh -c \"echo hi\"").expect("受理される");
        assert_eq!(command.tokens(), ["sh", "-c", "\"echo", "hi\""]);
    }

    #[test]
    fn 配列形式の要素はそのままトークンになる() {
        let command =
            RawCommand::parse_tokens(vec!["sh".to_owned(), "-c".to_owned(), "echo hi".to_owned()])
                .expect("受理される");
        assert_eq!(command.tokens(), ["sh", "-c", "echo hi"]);
    }

    #[test]
    fn 空文字列トークンは配列形式でのみ許容される() {
        let command =
            RawCommand::parse_tokens(vec!["prog".to_owned(), String::new()]).expect("受理される");
        assert_eq!(command.tokens(), ["prog", ""]);

        assert_eq!(RawCommand::parse_text(""), Err(CommandError::Empty));
    }

    #[test]
    fn 空白のみの文字列はトークン0個として拒否される() {
        assert_eq!(RawCommand::parse_text("   \t\n"), Err(CommandError::Empty));
    }

    #[test]
    fn 空配列は拒否される() {
        assert_eq!(
            RawCommand::parse_tokens(Vec::new()),
            Err(CommandError::Empty)
        );
    }

    #[test]
    fn 判定用コマンドはプレースホルダを検査しない() {
        let command = PlainCommand::parse_text("check {input} {unknown}").expect("受理される");
        assert_eq!(command.tokens(), ["check", "{input}", "{unknown}"]);
    }

    #[test]
    fn 判定用コマンドも0トークンは拒否される() {
        assert_eq!(PlainCommand::parse_text(""), Err(CommandError::Empty));
        assert_eq!(
            PlainCommand::parse_tokens(Vec::new()),
            Err(CommandError::Empty)
        );
    }
}
