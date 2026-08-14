//! 結果を与える `ProcessController` のダブル。

use std::cell::RefCell;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};

use pulsen_domain::definition::CommandLine;
use pulsen_domain::execution::{
    ExitCode, Io, KillError, ProcessController, RemnantOutcome, SpawnError, WrapperIdentity,
    WrapperLaunchSpec,
};
use pulsen_domain::task::{KillIdent, Pid, ProcessStartTime, WorktreePath};

/// 記録された呼び出し。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessControllerCall {
    /// `spawn_wrapper`。
    SpawnWrapper {
        /// 渡された起動仕様。
        spec: WrapperLaunchSpec,
    },
    /// `own_identity`。
    OwnIdentity,
    /// `run_agent`。
    RunAgent {
        /// 起動するコマンド。
        cmd: CommandLine,
        /// 作業ディレクトリ。
        cwd: WorktreePath,
        /// 標準出力のリダイレクト先。
        stdout: PathBuf,
        /// 標準エラーのリダイレクト先。
        stderr: PathBuf,
    },
    /// `starttime_of`。
    StarttimeOf {
        /// 観測対象のプロセスID。
        pid: Pid,
    },
    /// `kill`。
    Kill {
        /// 終了対象の実行単位。
        ident: KillIdent,
    },
    /// `try_kill_remnants`。
    TryKillRemnants {
        /// 終了対象の実行単位。
        ident: KillIdent,
    },
}

/// メソッドごとにあらかじめ与えた結果を順に返すコントローラー。
///
/// 実プロセスでは外から作れない状況(同定情報の取得失敗・spawn の同期エラー・任意の
/// 終了結果)をユースケースに与える。
#[derive(Debug, Default)]
pub struct ScriptedProcessController {
    spawn_wrapper: RefCell<VecDeque<Result<(), SpawnError>>>,
    own_identity: RefCell<VecDeque<Result<WrapperIdentity, Io>>>,
    run_agent: RefCell<VecDeque<ExitCode>>,
    starttime_of: RefCell<VecDeque<Result<Option<ProcessStartTime>, Io>>>,
    kill: RefCell<VecDeque<Result<(), KillError>>>,
    try_kill_remnants: RefCell<VecDeque<RemnantOutcome>>,
    calls: RefCell<Vec<ProcessControllerCall>>,
}

impl ScriptedProcessController {
    /// どのメソッドの台本も持たないコントローラーを作る。
    pub fn new() -> Self {
        Self::default()
    }

    /// `spawn_wrapper` が返す結果の列を与える。
    pub fn with_spawn_wrapper(
        self,
        results: impl IntoIterator<Item = Result<(), SpawnError>>,
    ) -> Self {
        *self.spawn_wrapper.borrow_mut() = results.into_iter().collect();
        self
    }

    /// `own_identity` が返す結果の列を与える。
    pub fn with_own_identity(
        self,
        results: impl IntoIterator<Item = Result<WrapperIdentity, Io>>,
    ) -> Self {
        *self.own_identity.borrow_mut() = results.into_iter().collect();
        self
    }

    /// `run_agent` が返す終了結果の列を与える。
    pub fn with_run_agent(self, results: impl IntoIterator<Item = ExitCode>) -> Self {
        *self.run_agent.borrow_mut() = results.into_iter().collect();
        self
    }

    /// `starttime_of` が返す結果の列を与える。
    pub fn with_starttime_of(
        self,
        results: impl IntoIterator<Item = Result<Option<ProcessStartTime>, Io>>,
    ) -> Self {
        *self.starttime_of.borrow_mut() = results.into_iter().collect();
        self
    }

    /// `kill` が返す結果の列を与える。
    pub fn with_kill(self, results: impl IntoIterator<Item = Result<(), KillError>>) -> Self {
        *self.kill.borrow_mut() = results.into_iter().collect();
        self
    }

    /// `try_kill_remnants` が返す結果の列を与える。
    pub fn with_try_kill_remnants(self, results: impl IntoIterator<Item = RemnantOutcome>) -> Self {
        *self.try_kill_remnants.borrow_mut() = results.into_iter().collect();
        self
    }

    /// これまでに受け取った呼び出し。
    pub fn calls(&self) -> Vec<ProcessControllerCall> {
        self.calls.borrow().clone()
    }
}

impl ProcessController for ScriptedProcessController {
    fn spawn_wrapper(&self, spec: &WrapperLaunchSpec) -> Result<(), SpawnError> {
        self.calls
            .borrow_mut()
            .push(ProcessControllerCall::SpawnWrapper { spec: spec.clone() });
        let Some(result) = self.spawn_wrapper.borrow_mut().pop_front() else {
            panic!("spawn_wrapper の結果を使い切った")
        };
        result
    }

    fn own_identity(&self) -> Result<WrapperIdentity, Io> {
        self.calls
            .borrow_mut()
            .push(ProcessControllerCall::OwnIdentity);
        let Some(result) = self.own_identity.borrow_mut().pop_front() else {
            panic!("own_identity の結果を使い切った")
        };
        result
    }

    fn run_agent(
        &self,
        cmd: &CommandLine,
        cwd: &WorktreePath,
        stdout: &Path,
        stderr: &Path,
    ) -> ExitCode {
        self.calls
            .borrow_mut()
            .push(ProcessControllerCall::RunAgent {
                cmd: cmd.clone(),
                cwd: cwd.clone(),
                stdout: stdout.to_path_buf(),
                stderr: stderr.to_path_buf(),
            });
        let Some(result) = self.run_agent.borrow_mut().pop_front() else {
            panic!("run_agent の結果を使い切った")
        };
        result
    }

    fn starttime_of(&self, pid: Pid) -> Result<Option<ProcessStartTime>, Io> {
        self.calls
            .borrow_mut()
            .push(ProcessControllerCall::StarttimeOf { pid });
        let Some(result) = self.starttime_of.borrow_mut().pop_front() else {
            panic!("starttime_of の結果を使い切った")
        };
        result
    }

    fn kill(&self, ident: &KillIdent) -> Result<(), KillError> {
        self.calls.borrow_mut().push(ProcessControllerCall::Kill {
            ident: ident.clone(),
        });
        let Some(result) = self.kill.borrow_mut().pop_front() else {
            panic!("kill の結果を使い切った")
        };
        result
    }

    fn try_kill_remnants(&self, ident: &KillIdent) -> RemnantOutcome {
        self.calls
            .borrow_mut()
            .push(ProcessControllerCall::TryKillRemnants {
                ident: ident.clone(),
            });
        let Some(result) = self.try_kill_remnants.borrow_mut().pop_front() else {
            panic!("try_kill_remnants の結果を使い切った")
        };
        result
    }
}
