//! `add` に渡されるワークフロー参照と、タスクに記録する表示名の決定規則。

use std::path::{MAIN_SEPARATOR, Path, PathBuf};

use super::name::{NameError, WorkflowName};

/// POSIX のパス区切り文字集合。
pub const POSIX_SEPARATORS: &[char] = &['/'];
/// Windows のパス区切り文字集合。
pub const WINDOWS_SEPARATORS: &[char] = &['/', '\\'];

/// ワークフロー定義として扱う拡張子。
pub const WORKFLOW_EXTENSIONS: &[&str] = &[".yaml", ".yml"];

/// 実行中のプラットフォームの区切り文字集合。
///
/// 条件付きコンパイルではなく std の定数から選ぶ(ADR-037)。判定ロジックは
/// 集合を引数に取る純粋関数に閉じており、両方の集合をテストから直接渡せる。
pub const fn platform_separators() -> &'static [char] {
    if MAIN_SEPARATOR == '\\' {
        WINDOWS_SEPARATORS
    } else {
        POSIX_SEPARATORS
    }
}

/// `add` に渡される名前またはファイルパス。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkflowRef {
    /// グローバルホームの `workflows/<name>.yaml` を指す名前。
    Name(WorkflowName),
    /// ワークフロー定義ファイルのパス。
    Path(PathBuf),
}

impl WorkflowRef {
    /// プラットフォーム既定の区切り文字集合で解釈する。
    pub fn parse(arg: &str) -> Result<Self, NameError> {
        Self::parse_with_separators(arg, platform_separators())
    }

    /// 区切り文字集合を明示して解釈する。ファイルの存在には依存しない。
    pub fn parse_with_separators(arg: &str, separators: &[char]) -> Result<Self, NameError> {
        let looks_like_path = arg.contains(separators)
            || WORKFLOW_EXTENSIONS
                .iter()
                .any(|extension| arg.ends_with(extension));
        if looks_like_path {
            return Ok(Self::Path(PathBuf::from(arg)));
        }
        WorkflowName::parse(arg.to_owned()).map(Self::Name)
    }

    /// タスクに記録するワークフロー名を決める。
    ///
    /// 名前指定では YAML の `workflow:` キーを使わない — キーが表示名になるのは
    /// ファイルパス指定のときだけ。
    pub fn display_name(&self, declared: Option<&WorkflowName>) -> Result<WorkflowName, NameError> {
        match self {
            Self::Name(name) => Ok(name.clone()),
            Self::Path(path) => match declared {
                Some(name) => Ok(name.clone()),
                None => WorkflowName::parse(file_stem(path)?),
            },
        }
    }
}

/// 拡張子を除いたファイル名。表示名として使えない形は `NameError::Empty` にする。
fn file_stem(path: &Path) -> Result<String, NameError> {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .map(str::to_owned)
        .ok_or(NameError::Empty)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name(value: &str) -> WorkflowName {
        WorkflowName::parse(value.to_owned()).expect("受理される")
    }

    #[test]
    fn 区切り文字も拡張子も含まない値は名前として解釈される() {
        assert_eq!(
            WorkflowRef::parse_with_separators("implement", POSIX_SEPARATORS),
            Ok(WorkflowRef::Name(name("implement")))
        );
    }

    #[test]
    fn 区切り文字を含む値はパスとして解釈される() {
        assert_eq!(
            WorkflowRef::parse_with_separators("./wf/implement", POSIX_SEPARATORS),
            Ok(WorkflowRef::Path(PathBuf::from("./wf/implement")))
        );
    }

    #[test]
    fn 円記号はwindowsの集合でのみ区切り文字として扱われる() {
        assert_eq!(
            WorkflowRef::parse_with_separators("wf\\implement", WINDOWS_SEPARATORS),
            Ok(WorkflowRef::Path(PathBuf::from("wf\\implement")))
        );
        assert_eq!(
            WorkflowRef::parse_with_separators("wf\\implement", POSIX_SEPARATORS),
            Ok(WorkflowRef::Name(name("wf\\implement")))
        );
    }

    #[test]
    fn 拡張子で終わる値は区切り文字がなくてもパスとして解釈される() {
        assert_eq!(
            WorkflowRef::parse_with_separators("implement.yaml", POSIX_SEPARATORS),
            Ok(WorkflowRef::Path(PathBuf::from("implement.yaml")))
        );
        assert_eq!(
            WorkflowRef::parse_with_separators("implement.yml", POSIX_SEPARATORS),
            Ok(WorkflowRef::Path(PathBuf::from("implement.yml")))
        );
    }

    #[test]
    fn 空の参照は名前として拒否される() {
        assert_eq!(
            WorkflowRef::parse_with_separators("", POSIX_SEPARATORS),
            Err(NameError::Empty)
        );
    }

    #[test]
    fn 名前指定の表示名は宣言名を使わない() {
        let reference = WorkflowRef::Name(name("implement"));
        assert_eq!(reference.display_name(None), Ok(name("implement")));
        assert_eq!(
            reference.display_name(Some(&name("declared"))),
            Ok(name("implement"))
        );
    }

    #[test]
    fn パス指定の表示名は宣言名を優先する() {
        let reference = WorkflowRef::Path(PathBuf::from("/tmp/wf/impl.yaml"));
        assert_eq!(
            reference.display_name(Some(&name("declared"))),
            Ok(name("declared"))
        );
    }

    #[test]
    fn パス指定で宣言名がなければファイル名から拡張子を除いた値を使う() {
        let reference = WorkflowRef::Path(PathBuf::from("/tmp/wf/impl.yaml"));
        assert_eq!(reference.display_name(None), Ok(name("impl")));
    }

    #[test]
    fn 表示名にできないパスは拒否される() {
        assert_eq!(
            WorkflowRef::Path(PathBuf::from("..")).display_name(None),
            Err(NameError::Empty)
        );
        assert_eq!(
            WorkflowRef::Path(PathBuf::from("/tmp/ impl.yaml")).display_name(None),
            Err(NameError::SurroundingWhitespace)
        );
    }
}
