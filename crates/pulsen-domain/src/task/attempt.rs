//! attempt の番号と現在 attempt への参照。

use super::path::RunDirPath;
use super::process::ProcessIdent;

/// attempt 番号の生成失敗。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttemptNumberError {
    /// 0 が与えられた(採番は 1 から始まる)。
    Zero,
}

impl AttemptNumberError {
    /// 制約の説明。
    ///
    /// タスクファイルの復号に失敗した理由は、破損したタスクの一覧・表示を通じて
    /// 利用者に見せる修復の材料になる。説明の定義箇所をドメインに1つ置く。
    pub fn describe(&self) -> &'static str {
        match self {
            Self::Zero => "1 以上である必要があります",
        }
    }
}

/// attempt 番号。1 以上の単調増加値。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AttemptNumber(u32);

impl AttemptNumber {
    /// 最初の attempt 番号。
    pub const FIRST: Self = Self(1);

    /// 1 以上としてのみ生成する。
    pub fn parse(value: u32) -> Result<Self, AttemptNumberError> {
        if value == 0 {
            return Err(AttemptNumberError::Zero);
        }
        Ok(Self(value))
    }

    /// 次の番号。
    ///
    /// 飽和させて無謬に保つ — `u32` の上限に達するには 40 億回の起動記録が要り、
    /// 到達しない領域のためにエラー分岐を全呼び出し側へ広げない。
    pub fn next(&self) -> Self {
        Self(self.0.saturating_add(1))
    }

    /// 保持する番号。
    pub fn get(&self) -> u32 {
        self.0
    }
}

/// 現在(最新)の attempt への参照。
///
/// 起動記録で丸ごと新しい値に置き換わり、それ以外のどの遷移でも書き換えられない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttemptRef {
    number: AttemptNumber,
    run_dir: RunDirPath,
    process: Option<ProcessIdent>,
}

impl AttemptRef {
    /// 永続化からの再構築。
    ///
    /// 新規採番は起動記録(後続スライス)が担い、そこでは `run_dir` を `number` から
    /// 導出して番号とパスの整合を構成で保証する。
    pub fn rehydrate(
        number: AttemptNumber,
        run_dir: RunDirPath,
        process: Option<ProcessIdent>,
    ) -> Self {
        Self {
            number,
            run_dir,
            process,
        }
    }

    /// attempt 番号。
    pub fn number(&self) -> AttemptNumber {
        self.number
    }

    /// run ディレクトリ。
    pub fn run_dir(&self) -> &RunDirPath {
        &self.run_dir
    }

    /// プロセス同定情報。起動確認前は `None`。
    pub fn process(&self) -> Option<&ProcessIdent> {
        self.process.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::task::id::TaskId;
    use crate::task::path::StateRoot;

    fn state_root() -> StateRoot {
        StateRoot::parse(PathBuf::from(if std::path::MAIN_SEPARATOR == '\\' {
            "C:\\state"
        } else {
            "/state"
        }))
        .expect("受理される")
    }

    #[test]
    fn ゼロのattempt番号は受理されない() {
        assert_eq!(AttemptNumber::parse(0), Err(AttemptNumberError::Zero));
    }

    #[test]
    fn attempt番号は1から始まる() {
        assert_eq!(AttemptNumber::parse(1), Ok(AttemptNumber::FIRST));
        assert_eq!(AttemptNumber::FIRST.get(), 1);
    }

    #[test]
    fn 次のattempt番号は1つ大きい() {
        let number = AttemptNumber::parse(3).expect("受理される");
        assert_eq!(number.next().get(), 4);
        assert!(number < number.next());
    }

    #[test]
    fn 再構築したattempt参照は与えた値をそのまま持つ() {
        let id = TaskId::parse("t1".to_owned()).expect("受理される");
        let number = AttemptNumber::parse(2).expect("受理される");
        let run_dir = RunDirPath::derive(&state_root(), &id, number);
        let attempt = AttemptRef::rehydrate(number, run_dir.clone(), None);

        assert_eq!(attempt.number(), number);
        assert_eq!(attempt.run_dir(), &run_dir);
        assert_eq!(attempt.process(), None);
    }
}
