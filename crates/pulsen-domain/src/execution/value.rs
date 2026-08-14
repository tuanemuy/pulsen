//! run ディレクトリに現れる観測値。

use crate::task::{KillIdent, Pid};

/// エージェント実行の終了結果の符号化値。
///
/// ラッパーが書いた値をそのまま保持し、意味づけ(成功・失敗の分類)は判定側が行う。
/// 正常終了は exit code、シグナル等による終了は POSIX 慣例の 128+シグナル番号、
/// エージェントの起動不能はシェル慣例の 127 / 126 が入る。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExitCode(i32);

impl ExitCode {
    /// 符号化値を包む。
    pub fn new(value: i32) -> Self {
        Self(value)
    }

    /// 保持する符号化値。
    pub fn get(&self) -> i32 {
        self.0
    }

    /// 成功(0)か。
    pub fn is_success(&self) -> bool {
        self.0 == 0
    }
}

/// pid ファイルの内容。
///
/// ラッパーは starttime → pid の順に書く。この出現は「同定情報一式が揃った」という
/// シグナルであり、tick はこれをもって running への取り込みを判断する。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PidFileContent {
    pid: Pid,
    kill_ident: KillIdent,
}

impl PidFileContent {
    /// プロセスIDと kill 同定子の組を作る。
    pub fn new(pid: Pid, kill_ident: KillIdent) -> Self {
        Self { pid, kill_ident }
    }

    /// プロセスID。
    pub fn pid(&self) -> Pid {
        self.pid
    }

    /// kill 同定子。
    pub fn kill_ident(&self) -> &KillIdent {
        &self.kill_ident
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 終了結果はゼロのときだけ成功になる() {
        assert!(ExitCode::new(0).is_success());
        for value in [1, 2, 126, 127, 134, -1] {
            let code = ExitCode::new(value);
            assert!(!code.is_success(), "{value}");
            assert_eq!(code.get(), value);
        }
    }

    #[test]
    fn 終了結果は数値の一致で等価になる() {
        assert_eq!(ExitCode::new(127), ExitCode::new(127));
        assert_ne!(ExitCode::new(127), ExitCode::new(126));
    }

    #[test]
    fn pidファイルの内容はプロセスidとkill同定子を保持する() {
        let content = PidFileContent::new(
            Pid::new(4242),
            KillIdent::parse("-4242".to_owned()).expect("受理される"),
        );

        assert_eq!(content.pid(), Pid::new(4242));
        assert_eq!(content.kill_ident().as_str(), "-4242");
    }
}
