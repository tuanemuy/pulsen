//! プロセス同定系。kill と PID 再利用照合に必要な値を持つ。

use super::time::Timestamp;

/// プロセス同定系の値の生成失敗。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessValueError {
    /// 空文字列。
    Empty,
}

impl ProcessValueError {
    /// 制約の説明。
    ///
    /// タスクファイルの復号に失敗した理由は、破損したタスクの一覧・表示を通じて
    /// 利用者に見せる修復の材料になる。説明の定義箇所をドメインに1つ置く。
    pub fn describe(&self) -> &'static str {
        match self {
            Self::Empty => "空文字列は指定できません",
        }
    }
}

/// プロセスID。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Pid(u32);

impl Pid {
    /// プロセスIDを包む。
    pub fn new(value: u32) -> Self {
        Self(value)
    }

    /// 保持する値。
    pub fn get(&self) -> u32 {
        self.0
    }
}

/// kill 同定子。
///
/// プラットフォーム実装が定義する不透明な値(POSIX: プロセスグループID、Windows:
/// ジョブオブジェクト名等)。ツールの再起動後もタスクファイルの情報だけで kill できる。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct KillIdent(String);

impl KillIdent {
    /// 非空の文字列としてのみ生成する。
    pub fn parse(s: String) -> Result<Self, ProcessValueError> {
        if s.is_empty() {
            return Err(ProcessValueError::Empty);
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

/// プロセスの起動時刻の不透明表現。
///
/// 等価比較のみに使い、可搬な意味・順序を持たせない(取得手段はプラットフォーム実装が
/// 記録時と同一のものを使う)。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProcessStartTime(String);

impl ProcessStartTime {
    /// 非空の文字列としてのみ生成する。
    pub fn parse(s: String) -> Result<Self, ProcessValueError> {
        if s.is_empty() {
            return Err(ProcessValueError::Empty);
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

/// 記録された起動時刻。
///
/// `ident` は PID 再利用照合に、`wall` は timeout の起点に使う — 照合用の値は不透明で
/// 時間演算できないため、書き込み時点の壁時計時刻を併記する。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartTimeRecord {
    ident: ProcessStartTime,
    wall: Timestamp,
}

impl StartTimeRecord {
    /// 照合用の値と壁時計時刻の組を作る。
    pub fn new(ident: ProcessStartTime, wall: Timestamp) -> Self {
        Self { ident, wall }
    }

    /// 照合用の起動時刻。
    pub fn ident(&self) -> &ProcessStartTime {
        &self.ident
    }

    /// 書き込み時点の壁時計時刻。
    pub fn wall(&self) -> Timestamp {
        self.wall
    }
}

/// running への遷移時にタスクファイルへ取り込まれる同定情報一式。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessIdent {
    pid: Pid,
    kill_ident: KillIdent,
    starttime: StartTimeRecord,
}

impl ProcessIdent {
    /// 同定情報一式を作る。
    pub fn new(pid: Pid, kill_ident: KillIdent, starttime: StartTimeRecord) -> Self {
        Self {
            pid,
            kill_ident,
            starttime,
        }
    }

    /// プロセスID。
    pub fn pid(&self) -> Pid {
        self.pid
    }

    /// kill 同定子。
    pub fn kill_ident(&self) -> &KillIdent {
        &self.kill_ident
    }

    /// 記録された起動時刻。
    pub fn starttime(&self) -> &StartTimeRecord {
        &self.starttime
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wall() -> Timestamp {
        Timestamp::parse_rfc3339("2026-08-11T09:15:30Z").expect("受理される")
    }

    #[test]
    fn 空のkill同定子と起動時刻は受理されない() {
        assert_eq!(
            KillIdent::parse(String::new()),
            Err(ProcessValueError::Empty)
        );
        assert_eq!(
            ProcessStartTime::parse(String::new()),
            Err(ProcessValueError::Empty)
        );
    }

    #[test]
    fn 起動時刻は文字列の完全一致でのみ等価になる() {
        let recorded = ProcessStartTime::parse("871234".to_owned()).expect("受理される");
        assert_eq!(
            recorded,
            ProcessStartTime::parse("871234".to_owned()).expect("受理される")
        );
        assert_ne!(
            recorded,
            ProcessStartTime::parse("871235".to_owned()).expect("受理される")
        );
    }

    #[test]
    fn 同定情報一式はkillに必要な値をすべて持つ() {
        let ident = ProcessIdent::new(
            Pid::new(4242),
            KillIdent::parse("-4242".to_owned()).expect("受理される"),
            StartTimeRecord::new(
                ProcessStartTime::parse("871234".to_owned()).expect("受理される"),
                wall(),
            ),
        );

        assert_eq!(ident.pid().get(), 4242);
        assert_eq!(ident.kill_ident().as_str(), "-4242");
        assert_eq!(ident.starttime().ident().as_str(), "871234");
        assert_eq!(ident.starttime().wall(), wall());
    }
}
