//! ラッパーモードのユースケース(UC-execution-009)。
//!
//! デタッチ起動されたラッパーが、自身の同定情報と終了結果を run ディレクトリへ
//! 永続化し、エージェントを worktree で実行する。config は読まず、ロックも取らない —
//! 必要な情報はすべて起動引数で受け取り、書き先は自 attempt の run ディレクトリに閉じる。
//!
//! ラッパーは自前のエラー報告経路を持たない。同定情報一式(starttime → pid)を書き切れ
//! なかったときは、その先の書き込みを行わずに終わる(starttime だけが残ることはある)。
//! tick は pid が現れないことから分類する。

use pulsen_domain::execution::{
    ExitCode, Io, PidFileContent, ProcessController, RunStore, WrapperLaunchSpec,
};

/// ラッパー1回の実行の結末。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WrapperOutcome {
    /// エージェントを実行した(終了結果は run ディレクトリにも書かれる)。
    Ran(ExitCode),
    /// エージェントを起動せずに終えた(無効化マーカーがあった、または確認自体に失敗した)。
    Suppressed,
    /// 同定情報一式を残せずに終えた(starttime だけが残ることはある)。
    Silent,
}

/// 同定情報を記録し、エージェントを実行して終了結果を残す。
///
/// ポートはジェネリック引数で受け取り、実アダプターとテストダブルのどちらにも同じ
/// 制御フローが乗ることを型で示す。
pub struct RunWrapper<'a, S, P> {
    runs: &'a S,
    processes: &'a P,
}

impl<'a, S, P> RunWrapper<'a, S, P>
where
    S: RunStore,
    P: ProcessController,
{
    /// run ディレクトリの読み書きとプロセス操作を結線する。
    pub fn new(runs: &'a S, processes: &'a P) -> Self {
        Self { runs, processes }
    }

    /// spec の処理フローの順に実行する。
    pub fn execute(&self, spec: &WrapperLaunchSpec) -> WrapperOutcome {
        let run_dir = spec.run_dir();

        let Ok(identity) = self.processes.own_identity() else {
            return WrapperOutcome::Silent;
        };

        // starttime は必ず pid より先に書く。tick は pid の出現をもって「同定情報一式が
        // 揃った」と判断するため、順序が逆になると pid だけを取り込んだ状態が生じる。
        if self
            .runs
            .write_starttime(run_dir, identity.starttime())
            .is_err()
        {
            return WrapperOutcome::Silent;
        }
        let content = PidFileContent::new(identity.pid(), identity.kill_ident().clone());
        if self.runs.write_pid_file(run_dir, &content).is_err() {
            return WrapperOutcome::Silent;
        }

        // マーカーの確認は pid の書き込みより後に行う。ツール側の「マーカー書き込み後に
        // pid 再確認」と対になり、どちらの観測が先でも遅延起動と新 attempt が並走しない。
        match self.runs.marker_exists(run_dir) {
            Ok(false) => {}
            // 無効化されていないことを確認できない以上、起動は安全側に倒す。
            Ok(true) | Err(Io::Failed { .. }) => return WrapperOutcome::Suppressed,
        }

        let code = self.processes.run_agent(
            spec.agent_cmd(),
            spec.workspace(),
            &run_dir.stdout_log(),
            &run_dir.stderr_log(),
        );

        // exit を書けなければ書けないまま終える。次の tick が「exit なし・プロセス死亡」
        // として分類するので、エージェントの結末そのものは変わらない。
        let _ = self.runs.write_exit(run_dir, &code);
        WrapperOutcome::Ran(code)
    }
}
