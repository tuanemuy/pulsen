//! コマンドテンプレートとプレースホルダ展開。

use std::collections::BTreeSet;
use std::path::PathBuf;

use super::command::{CommandError, RawCommand};
use super::name::{InputText, ModelName, Prompt, SkillName};

/// テンプレート中の `{name}`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Placeholder {
    /// `{input}`。
    Input,
    /// `{model}`。
    Model,
    /// `{workspace}`。
    Workspace,
    /// `{skill}`。`skill_input` テンプレートでのみ使える。
    Skill,
}

/// プレースホルダ名として解釈できなかったことを表す。
/// どのトークンで起きたかは呼び出し側が付与する。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnknownPlaceholderName;

impl Placeholder {
    /// 既知のプレースホルダ名のみを受理する。
    pub fn parse(name: &str) -> Result<Self, UnknownPlaceholderName> {
        match name {
            "input" => Ok(Self::Input),
            "model" => Ok(Self::Model),
            "workspace" => Ok(Self::Workspace),
            "skill" => Ok(Self::Skill),
            _ => Err(UnknownPlaceholderName),
        }
    }

    /// テンプレート中に書かれる名前。
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Input => "input",
            Self::Model => "model",
            Self::Workspace => "workspace",
            Self::Skill => "skill",
        }
    }
}

/// テンプレートを構成する断片。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Segment {
    /// そのまま出力される文字列。
    Literal(String),
    /// 展開時に値へ置き換わる穴。
    Hole(Placeholder),
}

/// 1トークン分のテンプレート。
pub type TemplateToken = Vec<Segment>;

/// テンプレートの生成失敗。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplateError {
    /// 許容されないプレースホルダ名を参照している。
    UnknownPlaceholder {
        /// 対象のトークン。
        token: String,
        /// 参照された名前。
        name: String,
    },
    /// 閉じない `{` または空の `{}`。
    MalformedBrace {
        /// 対象のトークン。
        token: String,
    },
}

/// 展開の失敗。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExpansionError {
    /// テンプレートが参照するプレースホルダに値がない。
    MissingValue(Placeholder),
}

/// エージェント実行に与える入力。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentInput {
    /// プロンプトをそのまま渡す。
    Prompt(Prompt),
    /// スキル名を `skill_input` で変換して渡す。
    Skill(SkillName),
}

/// 展開に与える値。`skill` は持たない — `{skill}` は `SkillInputTemplate` 専用で
/// コマンドテンプレートには現れない。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlaceholderValues {
    /// `{input}` の値。
    pub input: Option<InputText>,
    /// `{model}` の値。
    pub model: Option<ModelName>,
    /// `{workspace}` の値。
    pub workspace: Option<PathBuf>,
}

/// 展開済みのトークン列。先頭がプログラムで、シェルを介さず直接起動される。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandLine {
    tokens: Vec<String>,
}

impl CommandLine {
    /// プロセス境界(ラッパーの起動引数)からの再構築。
    ///
    /// 検証済みのトークン列がそのまま往復することだけを保証する経路であり、展開の代わりに
    /// 使うものではない。0トークンは直列化の破れとして拒否する — 先頭がプログラムである
    /// という不変条件が保てない。
    pub fn rehydrate(tokens: Vec<String>) -> Result<Self, CommandError> {
        if tokens.is_empty() {
            return Err(CommandError::Empty);
        }
        Ok(Self { tokens })
    }

    /// トークン列を借用する。
    pub fn tokens(&self) -> &[String] {
        &self.tokens
    }
}

/// プレースホルダを含み得るトークン列。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandTemplate {
    tokens: Vec<TemplateToken>,
}

impl CommandTemplate {
    /// `allowed` に含まれるプレースホルダだけを受理してテンプレート化する。
    pub fn parse(raw: &RawCommand, allowed: &[Placeholder]) -> Result<Self, TemplateError> {
        let mut tokens = Vec::with_capacity(raw.tokens().len());
        for token in raw.tokens() {
            tokens.push(parse_segments(token, allowed)?);
        }
        Ok(Self { tokens })
    }

    /// テンプレートが参照するプレースホルダの集合。
    pub fn placeholders(&self) -> BTreeSet<Placeholder> {
        let mut found = BTreeSet::new();
        for token in &self.tokens {
            for segment in token {
                match segment {
                    Segment::Literal(_) => {}
                    Segment::Hole(placeholder) => {
                        found.insert(*placeholder);
                    }
                }
            }
        }
        found
    }

    /// トークン単位で穴を値に置き換える。置換結果は再走査しない(1パス)。
    pub fn expand(&self, values: &PlaceholderValues) -> Result<CommandLine, ExpansionError> {
        let mut expanded = Vec::with_capacity(self.tokens.len());
        for token in &self.tokens {
            let mut rendered = String::new();
            for segment in token {
                match segment {
                    Segment::Literal(text) => rendered.push_str(text),
                    Segment::Hole(placeholder) => {
                        rendered.push_str(&resolve(*placeholder, values)?);
                    }
                }
            }
            expanded.push(rendered);
        }
        Ok(CommandLine { tokens: expanded })
    }

    /// テンプレートのトークン列を借用する。
    pub fn tokens(&self) -> &[TemplateToken] {
        &self.tokens
    }
}

/// `skill_input` の変換規則。単一の文字列テンプレートで、`{skill}` のみを参照できる。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillInputTemplate {
    segments: Vec<Segment>,
}

impl SkillInputTemplate {
    /// `{skill}` のみを許容プレースホルダとして解釈する。
    pub fn parse(s: &str) -> Result<Self, TemplateError> {
        Ok(Self {
            segments: parse_segments(s, &[Placeholder::Skill])?,
        })
    }

    /// スキル名を流し込んで `{input}` へ渡る値を作る。
    pub fn render(&self, skill: &SkillName) -> InputText {
        let mut rendered = String::new();
        for segment in &self.segments {
            match segment {
                Segment::Literal(text) => rendered.push_str(text),
                Segment::Hole(Placeholder::Skill) => rendered.push_str(skill.as_str()),
                Segment::Hole(Placeholder::Input | Placeholder::Model | Placeholder::Workspace) => {
                    unreachable!("parse が {{skill}} 以外の穴を作らないことが不変条件")
                }
            }
        }
        InputText::new(rendered)
    }

    /// テンプレートの断片列を借用する。
    pub fn segments(&self) -> &[Segment] {
        &self.segments
    }
}

fn resolve(placeholder: Placeholder, values: &PlaceholderValues) -> Result<String, ExpansionError> {
    let value = match placeholder {
        Placeholder::Input => values.input.as_ref().map(|input| input.as_str().to_owned()),
        Placeholder::Model => values.model.as_ref().map(|model| model.as_str().to_owned()),
        Placeholder::Workspace => values
            .workspace
            .as_ref()
            .map(|path| path.to_string_lossy().into_owned()),
        // 値を持たないため、`cmd` に `{skill}` が現れれば必ず欠落として扱われる。
        Placeholder::Skill => None,
    };
    value.ok_or(ExpansionError::MissingValue(placeholder))
}

fn parse_segments(token: &str, allowed: &[Placeholder]) -> Result<Vec<Segment>, TemplateError> {
    let malformed = || TemplateError::MalformedBrace {
        token: token.to_owned(),
    };

    let mut segments = Vec::new();
    let mut literal = String::new();
    let mut rest = token;

    while let Some(open) = rest.find('{') {
        literal.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let close = match after.find('}') {
            Some(close) => close,
            None => return Err(malformed()),
        };
        let name = &after[..close];
        if name.is_empty() {
            return Err(malformed());
        }
        let placeholder =
            Placeholder::parse(name).map_err(|_| TemplateError::UnknownPlaceholder {
                token: token.to_owned(),
                name: name.to_owned(),
            })?;
        if !allowed.contains(&placeholder) {
            return Err(TemplateError::UnknownPlaceholder {
                token: token.to_owned(),
                name: name.to_owned(),
            });
        }
        if !literal.is_empty() {
            segments.push(Segment::Literal(std::mem::take(&mut literal)));
        }
        segments.push(Segment::Hole(placeholder));
        rest = &after[close + 1..];
    }

    literal.push_str(rest);
    if !literal.is_empty() {
        segments.push(Segment::Literal(literal));
    }
    Ok(segments)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CMD_ALLOWED: &[Placeholder] = &[
        Placeholder::Input,
        Placeholder::Model,
        Placeholder::Workspace,
    ];

    fn template(text: &str) -> Result<CommandTemplate, TemplateError> {
        let raw = RawCommand::parse_text(text).expect("コマンドとして受理される");
        CommandTemplate::parse(&raw, CMD_ALLOWED)
    }

    fn input(text: &str) -> InputText {
        InputText::new(text.to_owned())
    }

    fn model(name: &str) -> ModelName {
        ModelName::parse(name.to_owned()).expect("受理される")
    }

    #[test]
    fn リテラルだけのテンプレートはそのまま展開される() {
        let expanded = template("claude --print")
            .expect("受理される")
            .expand(&PlaceholderValues::default())
            .expect("展開できる");
        assert_eq!(expanded.tokens(), ["claude", "--print"]);
    }

    #[test]
    fn 参照するプレースホルダの集合を返す() {
        let placeholders = template("claude -m {model} {input} --dir {workspace}")
            .expect("受理される")
            .placeholders();
        assert_eq!(
            placeholders,
            BTreeSet::from([
                Placeholder::Input,
                Placeholder::Model,
                Placeholder::Workspace
            ])
        );
    }

    #[test]
    fn トークン内の穴は値に置き換わる() {
        let expanded = template("claude --dir={workspace} {input}")
            .expect("受理される")
            .expand(&PlaceholderValues {
                input: Some(input("実装して")),
                model: None,
                workspace: Some(PathBuf::from("/tmp/wt")),
            })
            .expect("展開できる");
        assert_eq!(expanded.tokens(), ["claude", "--dir=/tmp/wt", "実装して"]);
    }

    #[test]
    fn 展開は1パスで置換結果を再走査しない() {
        let expanded = template("echo {input}")
            .expect("受理される")
            .expand(&PlaceholderValues {
                input: Some(input("{workspace}")),
                model: None,
                workspace: Some(PathBuf::from("/tmp/wt")),
            })
            .expect("展開できる");
        assert_eq!(expanded.tokens(), ["echo", "{workspace}"]);
    }

    #[test]
    fn 展開後に空白が現れてもトークンは分割されない() {
        let expanded = template("echo {input}")
            .expect("受理される")
            .expand(&PlaceholderValues {
                input: Some(input("a b")),
                model: None,
                workspace: None,
            })
            .expect("展開できる");
        assert_eq!(expanded.tokens(), ["echo", "a b"]);
    }

    #[test]
    fn 参照する値が欠けていると展開に失敗する() {
        let error = template("claude -m {model} {input}")
            .expect("受理される")
            .expand(&PlaceholderValues {
                input: Some(input("実装して")),
                model: None,
                workspace: None,
            })
            .expect_err("展開できない");
        assert_eq!(error, ExpansionError::MissingValue(Placeholder::Model));
    }

    #[test]
    fn 参照しない値を渡しても展開は成功する() {
        let expanded = template("claude {input}")
            .expect("受理される")
            .expand(&PlaceholderValues {
                input: Some(input("実装して")),
                model: Some(model("sonnet")),
                workspace: Some(PathBuf::from("/tmp/wt")),
            })
            .expect("展開できる");
        assert_eq!(expanded.tokens(), ["claude", "実装して"]);
    }

    #[test]
    fn 許容されないプレースホルダは未知として拒否される() {
        assert_eq!(
            template("claude {skill}"),
            Err(TemplateError::UnknownPlaceholder {
                token: "{skill}".to_owned(),
                name: "skill".to_owned()
            })
        );
        assert_eq!(
            template("claude {prompt}"),
            Err(TemplateError::UnknownPlaceholder {
                token: "{prompt}".to_owned(),
                name: "prompt".to_owned()
            })
        );
    }

    #[test]
    fn 閉じない波括弧は不正な波括弧として拒否される() {
        assert_eq!(
            template("claude {input"),
            Err(TemplateError::MalformedBrace {
                token: "{input".to_owned()
            })
        );
    }

    #[test]
    fn 空の波括弧は不正な波括弧として拒否される() {
        assert_eq!(
            template("claude {}"),
            Err(TemplateError::MalformedBrace {
                token: "{}".to_owned()
            })
        );
    }

    #[test]
    fn 閉じ波括弧だけならリテラルとして扱われる() {
        let expanded = template("echo a}b")
            .expect("受理される")
            .expand(&PlaceholderValues::default())
            .expect("展開できる");
        assert_eq!(expanded.tokens(), ["echo", "a}b"]);
    }

    #[test]
    fn リテラルの波括弧を表すエスケープ機構はない() {
        assert_eq!(
            template("echo \\{input\\}"),
            Err(TemplateError::UnknownPlaceholder {
                token: "\\{input\\}".to_owned(),
                name: "input\\".to_owned()
            })
        );
    }

    #[test]
    fn 展開したコマンドはトークン列として往復する() {
        let expanded = template("claude --dir={workspace} {input} --model")
            .expect("受理される")
            .expand(&PlaceholderValues {
                input: Some(input("a b && echo *")),
                model: None,
                workspace: Some(PathBuf::from("/tmp/wt")),
            })
            .expect("展開できる");

        let restored =
            CommandLine::rehydrate(expanded.tokens().to_vec()).expect("再構築を受理する");

        assert_eq!(restored, expanded);
    }

    #[test]
    fn 空文字列トークンも再構築で保たれる() {
        let restored = CommandLine::rehydrate(vec!["prog".to_owned(), String::new()])
            .expect("再構築を受理する");

        assert_eq!(restored.tokens(), ["prog", ""]);
    }

    #[test]
    fn ゼロトークンの再構築は拒否される() {
        assert_eq!(CommandLine::rehydrate(Vec::new()), Err(CommandError::Empty));
    }

    #[test]
    fn スキル入力テンプレートはスキル名を展開する() {
        let rendered = SkillInputTemplate::parse("/skill {skill} を実行")
            .expect("受理される")
            .render(&SkillName::parse("review".to_owned()).expect("受理される"));
        assert_eq!(rendered.as_str(), "/skill review を実行");
    }

    #[test]
    fn スキル入力テンプレートはスキル以外のプレースホルダを拒否する() {
        assert_eq!(
            SkillInputTemplate::parse("{input}"),
            Err(TemplateError::UnknownPlaceholder {
                token: "{input}".to_owned(),
                name: "input".to_owned()
            })
        );
    }

    #[test]
    fn スキル入力テンプレートは空文字列を受理する() {
        let rendered = SkillInputTemplate::parse("")
            .expect("受理される")
            .render(&SkillName::parse("review".to_owned()).expect("受理される"));
        assert_eq!(rendered.as_str(), "");
    }
}
