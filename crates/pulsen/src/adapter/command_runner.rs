//! 子プロセスの同期実行による `CommandRunner` の実装。
//!
//! OS 依存の分岐を持たない。exit code を持たない終了の符号化は `run_agent` と同じ関数
//! ([`super::process::encode`])を共有する — 判定コマンドのシグナル死と、エージェントの
//! シグナル死が別の値になると、同じ「プロトコル外の値」の見え方が経路で変わる。

use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use pulsen_domain::definition::{DurationSpec, PlainCommand};
use pulsen_domain::execution::{CommandCompletion, CommandRunner, ExitCode};

use super::process::encode;

/// 期限つきの待機で `try_wait` を確かめる間隔(adr ADR-001)。
///
/// 短くしすぎると、排他ロックを保持したまま待つ tick が cron 実行のたびに無駄に起きる。
/// 長くしすぎると判定・通知の完了検出が体感の遅延になる。timeout の判定にも同じ粒度の
/// 誤差が乗るため、適合ケースの timeout はこの間隔より十分大きく取る。
const POLL_INTERVAL: Duration = Duration::from_millis(50);

/// 判定コマンド・通知コマンドをシェル非経由で同期実行するランナー。
#[derive(Debug, Clone, Default)]
pub struct SystemCommandRunner;

impl SystemCommandRunner {
    /// 構築に外部リソースを要さない。
    pub fn new() -> Self {
        Self
    }
}

impl CommandRunner for SystemCommandRunner {
    fn run(
        &self,
        cmd: &PlainCommand,
        env: &[(String, String)],
        timeout: Option<&DurationSpec>,
    ) -> CommandCompletion {
        let (program, args) = cmd
            .tokens()
            .split_first()
            .expect("PlainCommand は1トークン以上であることが不変条件");
        let mut command = Command::new(program);
        // 環境は継承したうえで上書きする(`env_clear` を呼ばない)。標準出力・標準エラーは
        // 捕捉せず、呼び出しプロセスの出力へそのまま流す。
        command.args(args).stdin(Stdio::null());
        for (name, value) in env {
            command.env(name, value);
        }

        let child = match command.spawn() {
            Ok(child) => child,
            Err(error) => {
                return CommandCompletion::FailedToStart {
                    message: format!("{program} を起動できない: {error}"),
                };
            }
        };

        match timeout {
            // 期限が無いのに繰り返し起きる理由が無い。
            None => wait(child),
            Some(limit) => wait_until(child, Duration::from_secs(limit.seconds())),
        }
    }
}

/// 終了まで待つ。
fn wait(mut child: Child) -> CommandCompletion {
    match child.wait() {
        Ok(status) => CommandCompletion::Exited(ExitCode::new(encode(&status))),
        Err(error) => wait_failed(&mut child, &error),
    }
}

/// 期限つきで待ち、超過したら終了させる。
///
/// `Child` の所有権を持ったまま待つことで、超過時に終了させる手段が残る。別スレッドへ
/// move して待たせると、期限は測れても契約(「timeout 後に生存していない」)を満たせない。
fn wait_until(mut child: Child, limit: Duration) -> CommandCompletion {
    // 期限を絶対時刻(`Instant::now() + limit`)で持たない。`DurationSpec` は上限のない秒数を
    // 受理するため、加算は溢れでパニックしうる。経過時間との比較なら全域で定義される。
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return CommandCompletion::Exited(ExitCode::new(encode(&status))),
            Ok(None) => {}
            Err(error) => return wait_failed(&mut child, &error),
        }
        if started.elapsed() >= limit {
            // 終了させたうえで回収する。回収しないとゾンビが残り、次の観測が生存と読む。
            let _ = child.kill();
            let _ = child.wait();
            return CommandCompletion::TimedOut;
        }
        std::thread::sleep(POLL_INTERVAL);
    }
}

/// 待機そのものが失敗したときの結末。
///
/// 起動には成功しているので「終了しなかった」ではない。結末を値で表す契約のもとで残る
/// 受け皿はここだけであり、判定・通知はどちらも「実行が壊れた」として扱う。畳んだうえで
/// 子は始末する — 終了させて回収しないと、待てなくなった子が生き残りゾンビが残る。
fn wait_failed(child: &mut Child, error: &std::io::Error) -> CommandCompletion {
    let _ = child.kill();
    let _ = child.wait();
    CommandCompletion::FailedToStart {
        message: format!("終了を待てない: {error}"),
    }
}
