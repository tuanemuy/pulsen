//! 作業ディレクトリを失った状態からの `tick` の受け入れ検証。
//!
//! 作業ディレクトリはプロセス全体の状態なので、この前提を作るテストは独立した実行
//! ファイルに1つだけ置く。同じ実行ファイルの他のテストと並行させると、一方が消した
//! ディレクトリが他方の相対パス解決に効く。
//!
//! Windows は実行中のプロセスの作業ディレクトリを削除できないため、この前提そのものが
//! 作れない。ファイルごと POSIX に限る。
#![cfg(unix)]

mod common;

use std::env;
use std::fs;

use common::{Home, tick};

#[test]
fn 作業ディレクトリを失っていてもtickは処理対象なしで終わる() {
    let home = Home::new();
    let gone = common::scratch();
    let original = env::current_dir().expect("作業ディレクトリを取れる");

    // 起動時に作業ディレクトリを指定すると `chdir` が先に失敗して起動そのものが
    // 成立しない。消えた作業ディレクトリは継承させる以外に子へ渡せない。
    env::set_current_dir(gone.path()).expect("作業ディレクトリを移せる");
    fs::remove_dir_all(gone.path()).expect("作業ディレクトリを消せる");
    let run = tick().home(home.path()).run();
    env::set_current_dir(&original).expect("作業ディレクトリを戻せる");

    run.assert_succeeded();
    assert!(
        run.stdout.contains("処理対象のタスクはありませんでした"),
        "作業ディレクトリは tick の判断に関与しない: {}",
        run.stdout
    );
}
