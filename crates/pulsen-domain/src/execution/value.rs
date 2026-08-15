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

/// 判定の結末。
///
/// `Skipped` は判定コマンドでのみ生じる(ADR-008)。デフォルト判定は 2 値しか返さない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JudgeOutcome {
    /// 成功。次のステータスへ進める。
    Completed,
    /// 失敗。リトライ上限を消費する。
    Failed,
    /// 見送り。タスクステータス不変のまま起動待ちへ戻す。
    Skipped,
}

/// 判定コマンドの結果の解釈。
///
/// 「判定できた」と「判定自体が壊れた」を分ける — 前者は実行の帳簿を進め、後者は
/// 判定のカウンタだけを消費して running に留まる。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JudgeConclusion {
    /// 判定できた。
    Outcome(JudgeOutcome),
    /// 判定自体が壊れた(プロトコル外の exit code・判定 timeout・起動不能)。
    JudgeFailure {
        /// 原因の説明。タスクファイルに残り、凍結の要因の手がかりになる。
        detail: String,
    },
}

/// コマンド実行の結末。
///
/// 失敗を含むすべての結末を値で表す — 判定・通知の分類がこの値を入力に取るため、
/// 実行機構のエラーを `Err` に落とすと分類がエラー処理の側へ漏れる。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandCompletion {
    /// 終了した(シグナル死等の符号化値を含む)。
    Exited(ExitCode),
    /// timeout を超過したため終了させた。
    TimedOut,
    /// 起動できなかった。
    FailedToStart {
        /// 原因の説明。
        message: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 判定の結末は3値を区別する() {
        assert_eq!(JudgeOutcome::Completed, JudgeOutcome::Completed);
        assert_ne!(JudgeOutcome::Completed, JudgeOutcome::Failed);
        assert_ne!(JudgeOutcome::Failed, JudgeOutcome::Skipped);
    }

    #[test]
    fn 判定の解釈は結末と判定自体の破れを区別する() {
        let outcome = JudgeConclusion::Outcome(JudgeOutcome::Completed);
        let failure = JudgeConclusion::JudgeFailure {
            detail: "プロトコル外".to_owned(),
        };

        assert_ne!(outcome, failure);
        match failure {
            JudgeConclusion::JudgeFailure { detail } => assert_eq!(detail, "プロトコル外"),
            JudgeConclusion::Outcome(_) => unreachable!("判定自体の破れである"),
        }
    }

    #[test]
    fn コマンドの結末は3値を区別する() {
        let exited = CommandCompletion::Exited(ExitCode::new(0));
        let timed_out = CommandCompletion::TimedOut;
        let failed = CommandCompletion::FailedToStart {
            message: "実体がない".to_owned(),
        };

        assert_ne!(exited, timed_out);
        assert_ne!(timed_out, failed);
        assert_eq!(exited, CommandCompletion::Exited(ExitCode::new(0)));
        assert_ne!(exited, CommandCompletion::Exited(ExitCode::new(1)));
    }

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
