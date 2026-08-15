//! 結果を与える `CommandRunner` のダブル。

use std::cell::RefCell;
use std::collections::VecDeque;

use pulsen_domain::definition::{DurationSpec, PlainCommand};
use pulsen_domain::execution::{CommandCompletion, CommandRunner};

use super::RecordSeq;

/// 記録された呼び出し。
///
/// 判定と通知は同じポートに乗るため、「どのコマンドをどの環境変数とどの timeout で
/// 起動したか」を残さないと両者を取り違えた実装が検出できない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandRunnerCall {
    /// 起動したコマンド。
    pub cmd: PlainCommand,
    /// 追加・上書きした環境変数。
    pub env: Vec<(String, String)>,
    /// 適用した timeout。
    pub timeout: Option<DurationSpec>,
}

/// あらかじめ与えた結末を順に返すランナー。
///
/// 実コマンドでは確定的に作れない結末(timeout 超過・起動不能・任意の exit code)を
/// ユースケースに与える。
#[derive(Debug, Default)]
pub struct ScriptedCommandRunner {
    run: RefCell<VecDeque<CommandCompletion>>,
    calls: RefCell<Vec<(RecordSeq, CommandRunnerCall)>>,
}

impl ScriptedCommandRunner {
    /// 台本を持たないランナーを作る。
    pub fn new() -> Self {
        Self::default()
    }

    /// `run` が返す結末の列を与える。
    pub fn with_run(self, results: impl IntoIterator<Item = CommandCompletion>) -> Self {
        *self.run.borrow_mut() = results.into_iter().collect();
        self
    }

    /// これまでに受け取った呼び出し。
    pub fn calls(&self) -> Vec<CommandRunnerCall> {
        self.calls
            .borrow()
            .iter()
            .map(|(_, call)| call.clone())
            .collect()
    }

    /// これまでに受け取った呼び出しを、ほかのダブルの記録と並べられる採番つきで返す。
    pub fn calls_in_order(&self) -> Vec<(RecordSeq, CommandRunnerCall)> {
        self.calls.borrow().clone()
    }
}

impl CommandRunner for ScriptedCommandRunner {
    fn run(
        &self,
        cmd: &PlainCommand,
        env: &[(String, String)],
        timeout: Option<&DurationSpec>,
    ) -> CommandCompletion {
        self.calls.borrow_mut().push((
            RecordSeq::next(),
            CommandRunnerCall {
                cmd: cmd.clone(),
                env: env.to_vec(),
                timeout: timeout.copied(),
            },
        ));
        let Some(result) = self.run.borrow_mut().pop_front() else {
            panic!("run の結果を使い切った")
        };
        result
    }
}
