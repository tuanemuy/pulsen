//! 終了コードの規約(pages 共通事項)。

use std::process::ExitCode;

/// 成功。
pub const SUCCESS: ExitCode = ExitCode::SUCCESS;

/// 入力・状態・実行環境のエラー。
pub const FAILURE: ExitCode = ExitCode::FAILURE;

/// 引数の使い方の誤り。clap の既定値に合わせる。
pub const USAGE: u8 = 2;

/// 引数の解釈の結果を終了コードに写す。
///
/// `--help` / `--version` はエラーではなく標準出力への表示要求なので 0 で終わる。
pub fn from_clap(error: &clap::Error) -> ExitCode {
    let _ = error.print();
    if error.use_stderr() {
        ExitCode::from(USAGE)
    } else {
        SUCCESS
    }
}
