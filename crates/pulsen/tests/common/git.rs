//! git リポジトリのフィクスチャ(ADR-033)。
//!
//! 開発者のグローバル設定・既定ブランチ名・TMPDIR の位置でテストの結果が変わらないよう、
//! 起動する git の環境と初期化オプションをここで固定する。本番のアダプターは
//! ユーザーのグローバル設定を尊重するため、この固定はテスト側にだけ置く(ADR-024)。

use std::ffi::OsStr;
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

/// 固定した環境で git を実行し、標準出力を返す。
fn capture(dir: &Path, args: &[&OsStr]) -> Option<String> {
    let output = git(dir).stdout(Stdio::piped()).args(args).output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// 既定ブランチ `main` の空リポジトリを作る。
pub fn init_repo(dir: &Path) -> Option<()> {
    fs::create_dir_all(dir).ok()?;
    run(dir, &["init", "-b", "main"])
}

/// 内容のあるコミットを1つ積む。
pub fn commit(dir: &Path, file_name: &str) -> Option<()> {
    commit_file(dir, file_name, "pulsen\n")
}

/// 指定した内容のファイルをコミットする。
pub fn commit_file(dir: &Path, file_name: &str, content: &str) -> Option<()> {
    fs::write(dir.join(file_name), content).ok()?;
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

/// 新しいブランチを作りながら worktree を張る。
pub fn add_worktree_with_branch(repo: &Path, path: &Path, branch: &str, base: &str) -> Option<()> {
    let status = git(repo)
        .args([OsStr::new("worktree"), OsStr::new("add"), OsStr::new("-b")])
        .args([OsStr::new(branch), path.as_os_str(), OsStr::new(base)])
        .status()
        .ok()?;
    status.success().then_some(())
}

/// worktree の登録と実体を取り除く。ブランチは残る。
pub fn remove_worktree(repo: &Path, path: &Path) -> Option<()> {
    let status = git(repo)
        .args([
            OsStr::new("worktree"),
            OsStr::new("remove"),
            OsStr::new("--force"),
        ])
        .arg(path)
        .status()
        .ok()?;
    status.success().then_some(())
}

/// ブランチ先端のコミット同定子。
pub fn branch_tip(repo: &Path, branch: &str) -> Option<String> {
    capture(repo, &[OsStr::new("rev-parse"), OsStr::new(branch)])
}

/// 指定パスの worktree の登録状態。登録が無ければ `None`。
///
/// `prunable`(登録は残るが実体が消えている)を含めて返す — 登録の有無だけでは
/// クラッシュ後の復旧の前提を作れたかどうかが判定できない。
pub fn worktree_registration(repo: &Path, path: &Path) -> Option<Registration> {
    let key = physical_key(path)?;
    let listing = capture(
        repo,
        &[
            OsStr::new("worktree"),
            OsStr::new("list"),
            OsStr::new("--porcelain"),
        ],
    )?;
    let mut found: Option<Registration> = None;
    for line in listing.lines() {
        if let Some(value) = line.strip_prefix("worktree ") {
            if found.is_some() {
                break;
            }
            if physical_key(Path::new(value)).as_deref() == Some(key.as_path()) {
                found = Some(Registration::default());
            }
            continue;
        }
        let Some(registration) = found.as_mut() else {
            continue;
        };
        if let Some(value) = line.strip_prefix("branch ") {
            registration.branch = Some(value.to_owned());
        } else if line == "prunable" || line.starts_with("prunable ") {
            registration.prunable = true;
        }
    }
    found
}

/// worktree の登録状態。
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Registration {
    /// 登録が指すブランチの完全な参照名。detached なら `None`。
    pub branch: Option<String>,
    /// 登録は残るが実体が消えているか。
    pub prunable: bool,
}

/// 同定の鍵。実体が消えていても比較が成立するよう、親だけを正規化する。
///
/// アダプターと同じ規則で突き合わせないと、置き場がシンボリックリンク経由のとき
/// フィクスチャ側だけが登録を見つけられない。
fn physical_key(path: &Path) -> Option<std::path::PathBuf> {
    let parent = path.parent()?;
    let name = path.file_name()?;
    Some(fs::canonicalize(parent).ok()?.join(name))
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
