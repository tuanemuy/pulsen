//! git リポジトリのフィクスチャ(ADR-033)。
//!
//! 開発者のグローバル設定・既定ブランチ名・TMPDIR の位置でテストの結果が変わらないよう、
//! 起動する git の環境と初期化オプションをここで固定する。本番のアダプターは
//! ユーザーのグローバル設定を尊重するため、この固定はテスト側にだけ置く(ADR-024)。

use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::OnceLock;

use pulsen::adapter::worktree::INHERITED_GIT_ENV;

/// コミットの作者。グローバル設定を無効化するため、コミットのたびに明示する。
const AUTHOR: [&str; 4] = [
    "-c",
    "user.name=pulsen-test",
    "-c",
    "user.email=pulsen-test@example.invalid",
];

/// グローバル・システムの設定ファイルとして読ませる空のパス。
fn null_device() -> &'static str {
    if cfg!(windows) { "NUL" } else { "/dev/null" }
}

/// 環境を固定した `git -C <dir>`。
///
/// 取り除く環境変数はアダプターと同じ集合(`INHERITED_GIT_ENV`)を使う。`is_outside_repository`
/// はここで作った環境で前提を判定し、その前提に乗って動くのはアダプターなので、集合がずれると
/// 「フィクスチャはリポジトリ外と判定し、アダプターは親リポジトリを見つける」環境が生まれる。
fn git(dir: &Path) -> Command {
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(dir)
        .env("GIT_CONFIG_GLOBAL", null_device())
        .env("GIT_CONFIG_SYSTEM", null_device())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    for name in INHERITED_GIT_ENV {
        command.env_remove(name);
    }
    command
}

/// 固定した環境で git を実行し、成功したかを返す。
fn run(dir: &Path, args: &[&str]) -> Option<()> {
    git(dir).args(args).status().ok()?.success().then_some(())
}

/// 既定ブランチ `main` の空リポジトリを作る。
pub fn init_repo(dir: &Path) -> Option<()> {
    fs::create_dir_all(dir).ok()?;
    run(dir, &["init", "-b", "main"])
}

/// 内容のあるコミットを1つ積む。
pub fn commit(dir: &Path, file_name: &str) -> Option<()> {
    fs::write(dir.join(file_name), "pulsen\n").ok()?;
    run(dir, &["add", file_name])?;
    let mut args = AUTHOR.to_vec();
    args.extend(["commit", "-m", "commit"]);
    run(dir, &args)
}

/// HEAD を動かさずにブランチを作る。
pub fn create_branch(dir: &Path, name: &str) -> Option<()> {
    run(dir, &["branch", name])
}

/// HEAD をブランチから切り離す。
pub fn detach_head(dir: &Path) -> Option<()> {
    run(dir, &["checkout", "--detach"])
}

/// git リポジトリとして扱われないディレクトリか。
///
/// TMPDIR 自体がリポジトリ配下にあると上位へ遡って成功してしまうため、フィクスチャとして
/// 使う前に確かめる(ADR-033)。
pub fn is_outside_repository(dir: &Path) -> bool {
    !matches!(run(dir, &["rev-parse", "--show-toplevel"]), Some(()))
}

/// 一時ディレクトリの置き場が git リポジトリの外にあるか(1度だけ調べて使い回す)。
///
/// 「git リポジトリでないパス」を前提にするケースは、TMPDIR 自体がリポジトリ配下にある
/// 環境では走れない。スキップの宣言をこの述語で決めると、宣言が実際に前提を作れるか
/// どうかに対応する。
pub fn tmpdir_outside_repository() -> bool {
    static OUTSIDE: OnceLock<bool> = OnceLock::new();

    *OUTSIDE.get_or_init(|| tempfile::tempdir().is_ok_and(|dir| is_outside_repository(dir.path())))
}
