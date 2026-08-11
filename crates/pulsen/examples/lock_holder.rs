//! 排他ロックを保持し続けるテスト用のプロセス(ADR-032)。
//!
//! 引数で受け取ったパスのロックを取得し、`locked` を1行書いて標準入力が閉じるまで
//! 保持する。強制終了させれば「解放処理を実行しないままプロセスが消えた」状況になる。
//! 利用者に見えるサブコマンドを増やさないため、bin ではなく example として置く。

use std::env;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use pulsen::adapter::lock::FileExclusiveLock;
use pulsen_domain::execution::{ExclusiveLock, LockError};

fn main() -> ExitCode {
    let Some(path) = env::args_os().nth(1) else {
        eprintln!("使い方: lock_holder <ロックファイルのパス>");
        return ExitCode::FAILURE;
    };

    match FileExclusiveLock::new(PathBuf::from(path)).try_acquire() {
        Ok(Some(_guard)) => hold_until_stdin_closes(),
        Ok(None) => {
            eprintln!("ロックは別の保持者がいる");
            ExitCode::FAILURE
        }
        Err(LockError::Failed { message }) => {
            eprintln!("ロックを扱えない: {message}");
            ExitCode::FAILURE
        }
    }
}

/// 取得できたことを知らせ、標準入力が閉じるまで待つ。
fn hold_until_stdin_closes() -> ExitCode {
    let mut stdout = io::stdout();
    if writeln!(stdout, "locked")
        .and_then(|()| stdout.flush())
        .is_err()
    {
        return ExitCode::FAILURE;
    }
    let mut sink = Vec::new();
    match io::stdin().read_to_end(&mut sink) {
        Ok(_) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}
