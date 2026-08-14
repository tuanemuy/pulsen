//! 結果を与える `RunStore` のダブル。

use std::cell::RefCell;
use std::collections::VecDeque;

use pulsen_domain::execution::{ExitCode, Io, PidFileContent, RunFileError, RunStore};
use pulsen_domain::task::{AttemptNumber, RunDirPath, StartTimeRecord, TaskId};

/// 記録された呼び出し。
///
/// 「starttime を pid より先に書く」「マーカーの確認は pid の後」のような**順序**は、
/// 結果の値では表せない。呼び出しの並びとして観測できる形で残す。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunStoreCall {
    /// `prepare_attempt`。
    PrepareAttempt {
        /// 対象のタスク。
        id: TaskId,
        /// attempt 番号。
        number: AttemptNumber,
    },
    /// `read_pid_file`。
    ReadPidFile {
        /// 対象の run ディレクトリ。
        run_dir: RunDirPath,
    },
    /// `read_starttime`。
    ReadStarttime {
        /// 対象の run ディレクトリ。
        run_dir: RunDirPath,
    },
    /// `read_exit`。
    ReadExit {
        /// 対象の run ディレクトリ。
        run_dir: RunDirPath,
    },
    /// `write_invalidation_marker`。
    WriteInvalidationMarker {
        /// 対象の run ディレクトリ。
        run_dir: RunDirPath,
    },
    /// `marker_exists`。
    MarkerExists {
        /// 対象の run ディレクトリ。
        run_dir: RunDirPath,
    },
    /// `write_starttime`。
    WriteStarttime {
        /// 対象の run ディレクトリ。
        run_dir: RunDirPath,
        /// 書いた内容。
        record: StartTimeRecord,
    },
    /// `write_pid_file`。
    WritePidFile {
        /// 対象の run ディレクトリ。
        run_dir: RunDirPath,
        /// 書いた内容。
        content: PidFileContent,
    },
    /// `write_exit`。
    WriteExit {
        /// 対象の run ディレクトリ。
        run_dir: RunDirPath,
        /// 書いた内容。
        code: ExitCode,
    },
}

/// メソッドごとにあらかじめ与えた結果を順に返すストア。
///
/// 実ファイルシステムでは外から作れない状況(`Io` の注入・`RunFileError` の3分類・
/// 読み書きの失敗)をユースケースに与える。
#[derive(Debug, Default)]
pub struct ScriptedRunStore {
    prepare_attempt: RefCell<VecDeque<Result<RunDirPath, Io>>>,
    read_pid_file: RefCell<VecDeque<Result<Option<PidFileContent>, RunFileError>>>,
    read_starttime: RefCell<VecDeque<Result<Option<StartTimeRecord>, RunFileError>>>,
    read_exit: RefCell<VecDeque<Result<Option<ExitCode>, RunFileError>>>,
    write_invalidation_marker: RefCell<VecDeque<Result<(), Io>>>,
    marker_exists: RefCell<VecDeque<Result<bool, Io>>>,
    write_starttime: RefCell<VecDeque<Result<(), Io>>>,
    write_pid_file: RefCell<VecDeque<Result<(), Io>>>,
    write_exit: RefCell<VecDeque<Result<(), Io>>>,
    calls: RefCell<Vec<RunStoreCall>>,
}

impl ScriptedRunStore {
    /// どのメソッドの台本も持たないストアを作る。
    pub fn new() -> Self {
        Self::default()
    }

    /// `prepare_attempt` が返す結果の列を与える。
    pub fn with_prepare_attempt(
        self,
        results: impl IntoIterator<Item = Result<RunDirPath, Io>>,
    ) -> Self {
        *self.prepare_attempt.borrow_mut() = results.into_iter().collect();
        self
    }

    /// `read_pid_file` が返す結果の列を与える。
    pub fn with_read_pid_file(
        self,
        results: impl IntoIterator<Item = Result<Option<PidFileContent>, RunFileError>>,
    ) -> Self {
        *self.read_pid_file.borrow_mut() = results.into_iter().collect();
        self
    }

    /// `read_starttime` が返す結果の列を与える。
    pub fn with_read_starttime(
        self,
        results: impl IntoIterator<Item = Result<Option<StartTimeRecord>, RunFileError>>,
    ) -> Self {
        *self.read_starttime.borrow_mut() = results.into_iter().collect();
        self
    }

    /// `read_exit` が返す結果の列を与える。
    pub fn with_read_exit(
        self,
        results: impl IntoIterator<Item = Result<Option<ExitCode>, RunFileError>>,
    ) -> Self {
        *self.read_exit.borrow_mut() = results.into_iter().collect();
        self
    }

    /// `write_invalidation_marker` が返す結果の列を与える。
    pub fn with_write_invalidation_marker(
        self,
        results: impl IntoIterator<Item = Result<(), Io>>,
    ) -> Self {
        *self.write_invalidation_marker.borrow_mut() = results.into_iter().collect();
        self
    }

    /// `marker_exists` が返す結果の列を与える。
    pub fn with_marker_exists(self, results: impl IntoIterator<Item = Result<bool, Io>>) -> Self {
        *self.marker_exists.borrow_mut() = results.into_iter().collect();
        self
    }

    /// `write_starttime` が返す結果の列を与える。
    pub fn with_write_starttime(self, results: impl IntoIterator<Item = Result<(), Io>>) -> Self {
        *self.write_starttime.borrow_mut() = results.into_iter().collect();
        self
    }

    /// `write_pid_file` が返す結果の列を与える。
    pub fn with_write_pid_file(self, results: impl IntoIterator<Item = Result<(), Io>>) -> Self {
        *self.write_pid_file.borrow_mut() = results.into_iter().collect();
        self
    }

    /// `write_exit` が返す結果の列を与える。
    pub fn with_write_exit(self, results: impl IntoIterator<Item = Result<(), Io>>) -> Self {
        *self.write_exit.borrow_mut() = results.into_iter().collect();
        self
    }

    /// これまでに受け取った呼び出し。
    pub fn calls(&self) -> Vec<RunStoreCall> {
        self.calls.borrow().clone()
    }

    fn record(&self, call: RunStoreCall) {
        self.calls.borrow_mut().push(call);
    }
}

impl RunStore for ScriptedRunStore {
    fn prepare_attempt(&self, id: &TaskId, number: AttemptNumber) -> Result<RunDirPath, Io> {
        self.record(RunStoreCall::PrepareAttempt {
            id: id.clone(),
            number,
        });
        let Some(result) = self.prepare_attempt.borrow_mut().pop_front() else {
            panic!("prepare_attempt の結果を使い切った")
        };
        result
    }

    fn read_pid_file(&self, run_dir: &RunDirPath) -> Result<Option<PidFileContent>, RunFileError> {
        self.record(RunStoreCall::ReadPidFile {
            run_dir: run_dir.clone(),
        });
        let Some(result) = self.read_pid_file.borrow_mut().pop_front() else {
            panic!("read_pid_file の結果を使い切った")
        };
        result
    }

    fn read_starttime(
        &self,
        run_dir: &RunDirPath,
    ) -> Result<Option<StartTimeRecord>, RunFileError> {
        self.record(RunStoreCall::ReadStarttime {
            run_dir: run_dir.clone(),
        });
        let Some(result) = self.read_starttime.borrow_mut().pop_front() else {
            panic!("read_starttime の結果を使い切った")
        };
        result
    }

    fn read_exit(&self, run_dir: &RunDirPath) -> Result<Option<ExitCode>, RunFileError> {
        self.record(RunStoreCall::ReadExit {
            run_dir: run_dir.clone(),
        });
        let Some(result) = self.read_exit.borrow_mut().pop_front() else {
            panic!("read_exit の結果を使い切った")
        };
        result
    }

    fn write_invalidation_marker(&self, run_dir: &RunDirPath) -> Result<(), Io> {
        self.record(RunStoreCall::WriteInvalidationMarker {
            run_dir: run_dir.clone(),
        });
        let Some(result) = self.write_invalidation_marker.borrow_mut().pop_front() else {
            panic!("write_invalidation_marker の結果を使い切った")
        };
        result
    }

    fn marker_exists(&self, run_dir: &RunDirPath) -> Result<bool, Io> {
        self.record(RunStoreCall::MarkerExists {
            run_dir: run_dir.clone(),
        });
        let Some(result) = self.marker_exists.borrow_mut().pop_front() else {
            panic!("marker_exists の結果を使い切った")
        };
        result
    }

    fn write_starttime(&self, run_dir: &RunDirPath, record: &StartTimeRecord) -> Result<(), Io> {
        self.record(RunStoreCall::WriteStarttime {
            run_dir: run_dir.clone(),
            record: record.clone(),
        });
        let Some(result) = self.write_starttime.borrow_mut().pop_front() else {
            panic!("write_starttime の結果を使い切った")
        };
        result
    }

    fn write_pid_file(&self, run_dir: &RunDirPath, content: &PidFileContent) -> Result<(), Io> {
        self.record(RunStoreCall::WritePidFile {
            run_dir: run_dir.clone(),
            content: content.clone(),
        });
        let Some(result) = self.write_pid_file.borrow_mut().pop_front() else {
            panic!("write_pid_file の結果を使い切った")
        };
        result
    }

    fn write_exit(&self, run_dir: &RunDirPath, code: &ExitCode) -> Result<(), Io> {
        self.record(RunStoreCall::WriteExit {
            run_dir: run_dir.clone(),
            code: *code,
        });
        let Some(result) = self.write_exit.borrow_mut().pop_front() else {
            panic!("write_exit の結果を使い切った")
        };
        result
    }
}
