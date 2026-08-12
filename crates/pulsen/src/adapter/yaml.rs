//! YAML テキストの構文解析と、スキーマ走査が使う値表現。
//!
//! 外部の YAML クレートに任せるのは「テキスト → 値」の一段だけにして、スキーマの走査は
//! 各ストアのアダプターが手書きで行う(ADR-021)。serde の派生デシリアライズと
//! `deny_unknown_fields` に頼ると、未知キー・型不一致・値の生成失敗がすべて同じ
//! `serde::de::Error` になり、`UnknownKey` と `InvalidValue` を区別できない。
//!
//! この分担により、YAML クレートを差し替えても影響はこのモジュールに閉じる。

use pulsen_domain::definition::SourceLocation;

/// 構文解析の失敗。YAML として不正な内容と重複キーの両方がここに入る。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YamlSyntaxError {
    /// パーサからのメッセージ。
    pub message: String,
    /// 位置が取れた場合のテキスト上の位置。
    pub location: Option<SourceLocation>,
}

/// スキーマ走査が扱う YAML の値。
///
/// スカラーの解決は YAML 1.2 core schema 相当で、`true` / `false` だけが真偽値になる
/// (`no` / `yes` / `off` は文字列)。
#[derive(Debug, Clone)]
pub enum Yaml {
    /// 値なし(`null` / `~` / 値の書かれていないキー / 空ドキュメント)。
    Null,
    /// 真偽値。
    Bool(bool),
    /// 整数。
    Integer(i64),
    /// 小数、および `i64` に収まらない整数。
    Float(f64),
    /// 文字列。
    Text(String),
    /// 配列。
    Sequence(Vec<Yaml>),
    /// マッピング。宣言順を保つ。
    Mapping(YamlMapping),
}

/// 「文字列またはトークン配列」で書かれたコマンドの、未パースの形。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandInput {
    /// 文字列形式。
    Text(String),
    /// トークン配列形式。
    Tokens(Vec<String>),
}

/// 空のマッピング。「値の書かれていないキー」をキー0個のマッピングとして見るために使う。
static EMPTY_MAPPING: YamlMapping = YamlMapping {
    entries: Vec::new(),
};

impl Yaml {
    /// 型を表す日本語。型不一致のメッセージに使う。
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Null => "値なし",
            Self::Bool(_) => "真偽値",
            Self::Integer(_) | Self::Float(_) => "数値",
            Self::Text(_) => "文字列",
            Self::Sequence(_) => "配列",
            Self::Mapping(_) => "マッピング",
        }
    }

    /// 値が書かれていない。全キーが任意である定義では「キーの省略」と同じに扱う。
    pub fn is_absent(&self) -> bool {
        match self {
            Self::Null => true,
            Self::Bool(_)
            | Self::Integer(_)
            | Self::Float(_)
            | Self::Text(_)
            | Self::Sequence(_)
            | Self::Mapping(_) => false,
        }
    }

    /// 文字列として読む。
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text),
            Self::Null
            | Self::Bool(_)
            | Self::Integer(_)
            | Self::Float(_)
            | Self::Sequence(_)
            | Self::Mapping(_) => None,
        }
    }

    /// 非負整数として読む。
    pub fn as_u32(&self) -> Option<u32> {
        match self {
            Self::Integer(value) => u32::try_from(*value).ok(),
            Self::Null
            | Self::Bool(_)
            | Self::Float(_)
            | Self::Text(_)
            | Self::Sequence(_)
            | Self::Mapping(_) => None,
        }
    }

    /// 配列として読む。
    pub fn as_sequence(&self) -> Option<&[Yaml]> {
        match self {
            Self::Sequence(items) => Some(items),
            Self::Null
            | Self::Bool(_)
            | Self::Integer(_)
            | Self::Float(_)
            | Self::Text(_)
            | Self::Mapping(_) => None,
        }
    }

    /// マッピングとして読む。
    pub fn as_mapping(&self) -> Option<&YamlMapping> {
        match self {
            Self::Mapping(mapping) => Some(mapping),
            Self::Null
            | Self::Bool(_)
            | Self::Integer(_)
            | Self::Float(_)
            | Self::Text(_)
            | Self::Sequence(_) => None,
        }
    }

    /// マッピングとして読む。値が書かれていない場合はキー0個のマッピングとして見る。
    pub fn as_mapping_or_empty(&self) -> Option<&YamlMapping> {
        if self.is_absent() {
            return Some(&EMPTY_MAPPING);
        }
        self.as_mapping()
    }

    /// 「文字列またはトークン配列」として読む(`cmd` / `notify_cmd` / `judge`)。
    /// 配列に文字列以外の要素があれば型不一致とする。
    pub fn as_command_input(&self) -> Option<CommandInput> {
        if let Some(text) = self.as_text() {
            return Some(CommandInput::Text(text.to_owned()));
        }
        let items = self.as_sequence()?;
        let mut tokens = Vec::with_capacity(items.len());
        for item in items {
            tokens.push(item.as_text()?.to_owned());
        }
        Some(CommandInput::Tokens(tokens))
    }
}

/// 宣言順を保つマッピング。
#[derive(Debug, Clone)]
pub struct YamlMapping {
    entries: Vec<(String, Yaml)>,
}

impl YamlMapping {
    /// 宣言順のキーと値。
    pub fn entries(&self) -> &[(String, Yaml)] {
        &self.entries
    }

    /// キーの値。書かれていないキーと、値の書かれていないキーを区別しない。
    pub fn value(&self, key: &str) -> Option<&Yaml> {
        self.entries
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value)
            .filter(|value| !value.is_absent())
    }

    /// 許容されるキーの集合に無い最初のキー(ADR-013)。
    pub fn unknown_key(&self, allowed: &[&str]) -> Option<&str> {
        self.entries
            .iter()
            .map(|(name, _)| name.as_str())
            .find(|name| !allowed.contains(name))
    }
}

/// YAML テキストを1つの値として読む。
///
/// 空ファイル・null ドキュメントは `Yaml::Null` になる(「全キー省略」の表現。ADR-021)。
/// 重複キーは構文エラーとして落ちる。
pub fn parse_document(text: &str) -> Result<Yaml, YamlSyntaxError> {
    let value: serde_yaml_ng::Value =
        serde_yaml_ng::from_str(text).map_err(|error| YamlSyntaxError {
            message: error.to_string(),
            location: error.location().map(|location| SourceLocation {
                line: location.line(),
                column: location.column(),
            }),
        })?;
    convert(value, "")
}

/// 値を変換する。`path` は入れ子の論理位置(最上位は空文字)で、拒否した記法が
/// どこにあるかを示すために持ち回る。
fn convert(value: serde_yaml_ng::Value, path: &str) -> Result<Yaml, YamlSyntaxError> {
    match value {
        serde_yaml_ng::Value::Null => Ok(Yaml::Null),
        serde_yaml_ng::Value::Bool(value) => Ok(Yaml::Bool(value)),
        serde_yaml_ng::Value::Number(number) => Ok(match number.as_i64() {
            Some(value) => Yaml::Integer(value),
            None => Yaml::Float(number.as_f64().unwrap_or(f64::NAN)),
        }),
        serde_yaml_ng::Value::String(text) => Ok(Yaml::Text(text)),
        serde_yaml_ng::Value::Sequence(items) => {
            let mut converted = Vec::with_capacity(items.len());
            for (index, item) in items.into_iter().enumerate() {
                converted.push(convert(item, &format!("{path}[{index}]"))?);
            }
            Ok(Yaml::Sequence(converted))
        }
        serde_yaml_ng::Value::Mapping(mapping) => {
            let mut entries = Vec::with_capacity(mapping.len());
            for (key, value) in mapping {
                let key = key_text(convert(key, path)?, path)?;
                let value = convert(value, &child_path(path, &key))?;
                entries.push((key, value));
            }
            Ok(Yaml::Mapping(YamlMapping { entries }))
        }
        serde_yaml_ng::Value::Tagged(tagged) => Err(YamlSyntaxError {
            message: format!(
                "{}にタグ({})があります。タグは定義の記法に含まれません",
                position(path),
                tagged.tag
            ),
            location: None,
        }),
    }
}

/// 入れ子の論理位置。最上位のキーは名前だけになる。
fn child_path(parent: &str, key: &str) -> String {
    if parent.is_empty() {
        key.to_owned()
    } else {
        format!("{parent}.{key}")
    }
}

/// メッセージに埋める論理位置。
fn position(path: &str) -> String {
    if path.is_empty() {
        "最上位".to_owned()
    } else {
        format!("`{path}`")
    }
}

/// キーを文字列として読む。文字列でないキーは記法として拒否する。
///
/// `agents:` / `statuses:` の直下はキー自体が自由形式なので、非文字列キーを文字列化すると
/// スキーマ走査は通り、そのまま名前として受理される(複合キーは同じ表現に潰れて後勝ちで
/// 消える)。スキーマに無い記法を黙って読み替えないのはタグの拒否(ADR-042)と同じ根拠。
///
/// `location` が `None` なのは `Yaml` がテキスト上の位置を持たないため(位置は
/// `serde_yaml_ng` がパースを終えた時点で失われる)。代わりに、どのキーがどこで
/// 拒否されたかを論理位置つきでメッセージに載せる。
fn key_text(key: Yaml, path: &str) -> Result<String, YamlSyntaxError> {
    let kind = key.kind();
    match key {
        Yaml::Text(text) => Ok(text),
        Yaml::Null => Err(non_string_key(path, kind, Some("null"))),
        Yaml::Bool(value) => Err(non_string_key(path, kind, Some(&value.to_string()))),
        Yaml::Integer(value) => Err(non_string_key(path, kind, Some(&value.to_string()))),
        Yaml::Float(value) => Err(non_string_key(path, kind, Some(&value.to_string()))),
        Yaml::Sequence(_) | Yaml::Mapping(_) => Err(non_string_key(path, kind, None)),
    }
}

/// 非文字列キーの拒否。スカラーなら書かれた値を示し、引用すれば名前として使えることも
/// 添える — `1` や `true` は名前の制約(非空・前後空白なし)としては正当なので、
/// 記法の問題であることが伝わらないと直しようがない。
fn non_string_key(path: &str, kind: &str, literal: Option<&str>) -> YamlSyntaxError {
    let message = match literal {
        Some(literal) => format!(
            "{}のキー `{literal}` が文字列ではありません({kind})。名前として使うなら \"{literal}\" のように引用してください",
            position(path)
        ),
        None => format!(
            "{}に{kind}のキーがあります。キーは文字列で書きます",
            position(path)
        ),
    };
    YamlSyntaxError {
        message,
        location: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Yaml {
        parse_document(text).expect("読める")
    }

    #[test]
    fn 空ドキュメントは値なしになる() {
        for text in ["", "\n", "null", "---\n", "# コメントだけ\n"] {
            assert!(parse(text).is_absent(), "{text:?}");
        }
    }

    #[test]
    fn 重複キーは構文エラーになる() {
        let error = parse_document("a: 1\na: 2\n").expect_err("拒否される");
        assert!(error.message.contains("duplicate"));

        let nested =
            parse_document("agents:\n  claude:\n    cmd: a\n    cmd: b\n").expect_err("拒否される");
        assert!(nested.message.contains("duplicate"));
    }

    #[test]
    fn 構文エラーは位置つきで返る() {
        let error = parse_document("@invalid").expect_err("拒否される");
        assert_eq!(error.location, Some(SourceLocation { line: 1, column: 1 }));
    }

    #[test]
    fn 真偽値になるのはtrueとfalseだけ() {
        let mapping = parse("a: true\nb: no\nc: yes\n");
        let mapping = mapping.as_mapping().expect("マッピング");
        assert_eq!(mapping.value("a").expect("ある").kind(), "真偽値");
        assert_eq!(mapping.value("b").and_then(Yaml::as_text), Some("no"));
        assert_eq!(mapping.value("c").and_then(Yaml::as_text), Some("yes"));
    }

    #[test]
    fn 値の書かれていないキーは省略と同じに読める() {
        let document = parse("a:\nb: 1\n");
        let mapping = document.as_mapping().expect("マッピング");

        assert_eq!(mapping.value("a").map(Yaml::kind), None);
        assert_eq!(mapping.entries().len(), 2);
    }

    #[test]
    fn マッピングは宣言順を保つ() {
        let document = parse("z: 1\na: 2\nm: 3\n");
        let keys: Vec<&str> = document
            .as_mapping()
            .expect("マッピング")
            .entries()
            .iter()
            .map(|(key, _)| key.as_str())
            .collect();

        assert_eq!(keys, ["z", "a", "m"]);
    }

    #[test]
    fn スキーマに無いキーが見つかる() {
        let document = parse("cmd: a\nskil_input: b\n");

        assert_eq!(
            document
                .as_mapping()
                .expect("マッピング")
                .unknown_key(&["cmd", "skill_input"]),
            Some("skil_input")
        );
    }

    #[test]
    fn スカラーのキーは書かれた値と引用の案内つきで拒否される() {
        for (text, literal) in [
            ("1: a\n", "1"),
            ("true: a\n", "true"),
            ("null: a\n", "null"),
            ("1.5: a\n", "1.5"),
        ] {
            let error = parse_document(text).expect_err("拒否される");
            assert!(
                error.message.contains(&format!("`{literal}`")),
                "{text:?}: {}",
                error.message
            );
            assert!(
                error.message.contains(&format!("\"{literal}\"")),
                "{text:?}: {}",
                error.message
            );
        }
    }

    #[test]
    fn 複合キーは記法として拒否される() {
        for (text, kind) in [
            ("? [a, b]\n: c\n", "配列"),
            ("? {a: b}\n: c\n", "マッピング"),
        ] {
            let error = parse_document(text).expect_err("拒否される");
            assert!(
                error.message.contains("最上位") && error.message.contains(kind),
                "{text:?}: {}",
                error.message
            );
        }
    }

    #[test]
    fn 入れ子のマッピングでは拒否されたキーの論理位置が示される() {
        let error = parse_document("agents:\n  1: a\n").expect_err("拒否される");
        assert!(
            error.message.contains("`agents`") && error.message.contains("`1`"),
            "{}",
            error.message
        );
    }

    #[test]
    fn コマンドは文字列でも配列でも読める() {
        let document = parse("a: sh -c\nb: [sh, -c, \"\"]\nc: [sh, 1]\nd: 1\n");
        let mapping = document.as_mapping().expect("マッピング");

        assert_eq!(
            mapping.value("a").and_then(Yaml::as_command_input),
            Some(CommandInput::Text("sh -c".to_owned()))
        );
        assert_eq!(
            mapping.value("b").and_then(Yaml::as_command_input),
            Some(CommandInput::Tokens(vec![
                "sh".to_owned(),
                "-c".to_owned(),
                String::new()
            ]))
        );
        assert_eq!(mapping.value("c").and_then(Yaml::as_command_input), None);
        assert_eq!(mapping.value("d").and_then(Yaml::as_command_input), None);
    }

    #[test]
    fn 非負整数だけがu32として読める() {
        let document = parse("a: 3\nb: -1\nc: 1.5\nd: \"3\"\n");
        let mapping = document.as_mapping().expect("マッピング");

        assert_eq!(mapping.value("a").and_then(Yaml::as_u32), Some(3));
        assert_eq!(mapping.value("b").and_then(Yaml::as_u32), None);
        assert_eq!(mapping.value("c").and_then(Yaml::as_u32), None);
        assert_eq!(mapping.value("d").and_then(Yaml::as_u32), None);
    }

    #[test]
    fn タグは定義の記法として拒否される() {
        let error = parse_document("a: !Tag 1\n").expect_err("拒否される");
        assert!(error.message.contains("Tag"));
    }
}
