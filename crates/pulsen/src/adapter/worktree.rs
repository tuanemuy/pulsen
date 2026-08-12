//! git CLI へのシェルアウトによる `WorktreeManager` の実装。

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use pulsen_domain::execution::{TargetError, WorktreeError, WorktreeManager};
use pulsen_domain::task::{BranchName, RepoPath, Workspace};

use crate::util::fsdir::ensure_dir;

/// 起動する git から取り除く環境変数。
///
/// 呼び出し元の環境に残っていると `-C` で指した対象の解決結果を上書きしてしまう
/// (`GIT_CEILING_DIRECTORIES` は上位探索を打ち切り、正当なリポジトリを
/// `NotARepository` に落とす)。ADR-024 の判断基準はこの目的であって変数名の列挙ではない。
///
/// 公開しているのは、テストのフィクスチャが「このディレクトリはリポジトリでない」といった
/// 前提を判定するとき、アダプターと同じ環境で git を起動する必要があるため。集合が2箇所に
/// 分かれると、前提の判定とアダプターの観測が食い違う環境が生まれる。
pub const INHERITED_GIT_ENV: &[&str] = &[
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_INDEX_FILE",
    "GIT_CEILING_DIRECTORIES",
    "GIT_COMMON_DIR",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
];

/// git CLI に問い合わせて対象を検証するマネージャー(ADR-024)。
///
/// 分類はコマンドの終了ステータスと起動の可否だけで決め、メッセージ文字列は見ない。
/// ユーザーのグローバル設定(`safe.directory` 等)は尊重する — 無効化すると所有者の
/// 異なるリポジトリを扱えなくなる。
#[derive(Debug, Clone)]
pub struct GitCliWorktreeManager {
    git_program: PathBuf,
}

impl GitCliWorktreeManager {
    /// 起動する git 実行ファイルのパスを受け取る(合成ルートが既定の `git` を渡す)。
    ///
    /// 注入にしておくと、適合テストが「git を起動できない」状況を別のインスタンスとして
    /// 作れる(ADR-024・ADR-027)。本番のインスタンスはイミュータブルなまま保たれる。
    pub fn new(git_program: PathBuf) -> Self {
        Self { git_program }
    }

    /// `git -C <repo> <args...>` を実行する。起動自体の失敗は説明の文字列にする。
    ///
    /// 起動失敗の写し先(`TargetError` / `WorktreeError`)は呼び出し側が決める。環境を
    /// 落とす規則を1箇所に保つため、起動そのものはこの関数に閉じる。
    fn output<Args, Arg>(&self, repo: &RepoPath, args: Args) -> Result<Output, String>
    where
        Args: IntoIterator<Item = Arg>,
        Arg: AsRef<OsStr>,
    {
        let mut command = Command::new(&self.git_program);
        command.arg("-C").arg(repo.as_path()).args(args);
        for name in INHERITED_GIT_ENV {
            command.env_remove(name);
        }
        command.output().map_err(|error| {
            format!(
                "git ({}) を起動できない: {error}",
                self.git_program.display()
            )
        })
    }

    /// 検証系の `git -C <repo> <args...>`。
    fn run(&self, repo: &RepoPath, args: &[&str]) -> Result<Output, TargetError> {
        self.output(repo, args)
            .map_err(|message| TargetError::Failed { message })
    }

    /// worktree 操作の `git -C <repo> <args...>`。非0終了も失敗として畳む。
    fn run_worktree<Args, Arg>(&self, repo: &RepoPath, args: Args) -> Result<Output, WorktreeError>
    where
        Args: IntoIterator<Item = Arg>,
        Arg: AsRef<OsStr>,
    {
        self.output(repo, args)
            .map_err(|message| WorktreeError::Failed { message })
    }

    /// 成功を要求する worktree 操作。非0終了は git の説明を添えて失敗にする。
    fn require_success<Args, Arg>(&self, repo: &RepoPath, args: Args) -> Result<(), WorktreeError>
    where
        Args: IntoIterator<Item = Arg>,
        Arg: AsRef<OsStr>,
    {
        let output = self.run_worktree(repo, args)?;
        if output.status.success() {
            return Ok(());
        }
        Err(WorktreeError::Failed {
            message: format!(
                "git worktree の操作が失敗した: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        })
    }
}

impl WorktreeManager for GitCliWorktreeManager {
    fn validate_repo(&self, repo: &RepoPath) -> Result<(), TargetError> {
        // `exists()` と違い `try_exists()` は I/O エラーを `false` に丸めない。親ディレクトリを
        // 読めないだけのリポジトリを「存在しません」と案内すると、利用者は実在するパスを疑う。
        match repo.as_path().try_exists() {
            Ok(true) => {}
            Ok(false) => return Err(TargetError::NotFound),
            Err(error) => {
                return Err(TargetError::Failed {
                    message: format!(
                        "{}: リポジトリのパスを確認できない: {error}",
                        repo.as_path().display()
                    ),
                });
            }
        }
        // メタデータの破損も含め、問い合わせが通らないものはすべて「リポジトリでない」に
        // 落ちる(ADR-024 の実測)。リポジトリ配下のサブディレクトリ指定は受理する。
        let output = self.run(repo, &["rev-parse", "--show-toplevel"])?;
        if output.status.success() {
            return Ok(());
        }
        Err(TargetError::NotARepository)
    }

    fn head_branch(&self, repo: &RepoPath) -> Result<BranchName, TargetError> {
        // 空リポジトリでも `symbolic-ref` は exit 0 でブランチ名を返すため、コミットの
        // 有無を `rev-parse` で併せて見ないと空リポジトリを検出できない(ADR-024)。
        let symbolic = self.run(repo, &["symbolic-ref", "--short", "HEAD"])?;
        let head = self.run(repo, &["rev-parse", "--verify", "--quiet", "HEAD"])?;
        match (symbolic.status.success(), head.status.success()) {
            (true, true) => branch_name(&symbolic.stdout),
            (true, false) => Err(TargetError::EmptyRepository),
            (false, true) => Err(TargetError::DetachedHead),
            (false, false) => Err(TargetError::Failed {
                message: "HEAD を読み取れない".to_owned(),
            }),
        }
    }

    fn branch_exists(&self, repo: &RepoPath, branch: &BranchName) -> Result<bool, TargetError> {
        let output = self.run(
            repo,
            &["show-ref", "--verify", "--quiet", &head_ref(branch)],
        )?;
        match output.status.code() {
            Some(0) => Ok(true),
            Some(1) => Ok(false),
            Some(code) => Err(TargetError::Failed {
                message: format!("ブランチの照会が異常終了した (exit {code})"),
            }),
            None => Err(TargetError::Failed {
                message: "ブランチの照会がシグナルで終了した".to_owned(),
            }),
        }
    }

    fn create(
        &self,
        repo: &RepoPath,
        base: &BranchName,
        ws: &Workspace,
    ) -> Result<(), WorktreeError> {
        let path = ws.path().as_path();
        let parent = path.parent().ok_or_else(|| WorktreeError::Failed {
            message: format!("{}: worktree の置き場を決められない", path.display()),
        })?;
        // ツールが管理する領域なので自動作成する。同定の鍵を作る前に済ませておく —
        // 鍵は親の正規化に依存する。
        ensure_dir(parent).map_err(|error| WorktreeError::Failed {
            message: format!(
                "{}: worktree の置き場を用意できない: {error}",
                parent.display()
            ),
        })?;

        let key = physical_key(path).ok_or_else(|| WorktreeError::Failed {
            message: format!("{}: worktree のパスを正規化できない", path.display()),
        })?;
        let listing = self.run_worktree(repo, ["worktree", "list", "--porcelain"])?;
        if !listing.status.success() {
            return Err(WorktreeError::Failed {
                message: format!(
                    "登録済みの worktree を列挙できない: {}",
                    String::from_utf8_lossy(&listing.stderr).trim()
                ),
            });
        }

        let reference = head_ref(ws.branch());
        match registered_entry(&listing.stdout, &key) {
            Some(entry) => {
                if entry.branch.as_deref() != Some(reference.as_str()) {
                    return Err(WorktreeError::Failed {
                        message: format!(
                            "{} には別の worktree が登録されている(自動修復しない)",
                            path.display()
                        ),
                    });
                }
                if !entry.prunable {
                    return Ok(());
                }
                // 登録は残っているが実体が消えている。鍵が自タスクのパスと一致し、その
                // エントリが自タスクのブランチを指していることを確認した後なので、`-f` が
                // 外す保護は「登録は残るが実体が無い」1つに閉じる。
                self.require_success(
                    repo,
                    [
                        OsStr::new("worktree"),
                        OsStr::new("add"),
                        OsStr::new("-f"),
                        path.as_os_str(),
                        OsStr::new(ws.branch().as_str()),
                    ],
                )
            }
            None => {
                let occupied = path.try_exists().map_err(|error| WorktreeError::Failed {
                    message: format!("{}: パスを確認できない: {error}", path.display()),
                })?;
                if occupied {
                    return Err(WorktreeError::Failed {
                        message: format!(
                            "{} に worktree でない実体がある(自動修復しない)",
                            path.display()
                        ),
                    });
                }
                let branch_present = self
                    .run_worktree(repo, ["show-ref", "--verify", "--quiet", &reference])?
                    .status
                    .success();
                if branch_present {
                    // `-f` を付けない。先端を変えずに張り直し、積まれたコミットを保つ。
                    self.require_success(
                        repo,
                        [
                            OsStr::new("worktree"),
                            OsStr::new("add"),
                            path.as_os_str(),
                            OsStr::new(ws.branch().as_str()),
                        ],
                    )
                } else {
                    self.require_success(
                        repo,
                        [
                            OsStr::new("worktree"),
                            OsStr::new("add"),
                            OsStr::new("-b"),
                            OsStr::new(ws.branch().as_str()),
                            path.as_os_str(),
                            OsStr::new(base.as_str()),
                        ],
                    )
                }
            }
        }
    }
}

/// ブランチの完全な参照名。
fn head_ref(branch: &BranchName) -> String {
    format!("refs/heads/{}", branch.as_str())
}

/// `git worktree list --porcelain` の1エントリのうち、同定に使う属性。
#[derive(Debug, Default)]
struct WorktreeEntry {
    branch: Option<String>,
    prunable: bool,
}

/// 同定の鍵。
///
/// パス自体を正規化しないのは、実体が消えている場合に失敗して比較そのものが成立しない
/// ため(親は `create` が作るので必ず存在する)。
fn physical_key(path: &Path) -> Option<PathBuf> {
    let parent = path.parent()?;
    let name = path.file_name()?;
    Some(std::fs::canonicalize(parent).ok()?.join(name))
}

/// 鍵に一致する登録を探す。
///
/// git の出力側も同じ `physical_key` に通してから突き合わせる(正規化を両側に対称に
/// 適用する。生のパスの文字列比較は、シンボリックリンクを含むホームや Windows の
/// 拡張長パスで恒常的に外れる)。鍵に変換できないエントリは自タスクのものではないので
/// 不一致として扱う。
///
/// git が引用する必要のある文字(改行・引用符・制御文字)をパスに含む worktree は、
/// 引用された表記のまま鍵に変換されるため一致しない。
fn registered_entry(stdout: &[u8], key: &Path) -> Option<WorktreeEntry> {
    let text = String::from_utf8_lossy(stdout);
    let mut entries: Vec<(PathBuf, WorktreeEntry)> = Vec::new();
    for line in text.lines() {
        if let Some(value) = line.strip_prefix("worktree ") {
            entries.push((PathBuf::from(value), WorktreeEntry::default()));
            continue;
        }
        let Some((_, entry)) = entries.last_mut() else {
            continue;
        };
        if let Some(value) = line.strip_prefix("branch ") {
            entry.branch = Some(value.to_owned());
        } else if line == "prunable" || line.starts_with("prunable ") {
            entry.prunable = true;
        }
    }
    entries
        .into_iter()
        .find(|(path, _)| physical_key(path).as_deref() == Some(key))
        .map(|(_, entry)| entry)
}

/// `symbolic-ref` の出力をブランチ名にする。
///
/// git 側で有効でもドメインの実用サブセットに乗らない名前は、対象の分類ではなく
/// 実行環境の制約として扱う(`TargetError` の分類は5種で閉じている。ADR-024)。
fn branch_name(stdout: &[u8]) -> Result<BranchName, TargetError> {
    let text = String::from_utf8(stdout.to_vec()).map_err(|error| TargetError::Failed {
        message: format!("HEAD のブランチ名が UTF-8 ではない: {error}"),
    })?;
    let name = text.trim_end_matches(['\r', '\n']).to_owned();
    // 破れた制約の言い換えは CLI の文言層が持つ。アダプターは扱えなかった名前だけを載せる
    // (エラー値をそのまま埋めると Rust の Debug 表現が利用者に出る)。
    BranchName::parse(name.clone()).map_err(|_| TargetError::Failed {
        message: format!("HEAD のブランチ名を扱えない: {name}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 存在しないパスはgitを起動する前に見つからないと判定される() {
        let root = tempfile::tempdir().expect("一時ディレクトリを作れる");
        let manager = GitCliWorktreeManager::new(root.path().join("no-such-git"));
        let repo = RepoPath::parse(root.path().join("missing")).expect("絶対パス");

        assert_eq!(manager.validate_repo(&repo), Err(TargetError::NotFound));
    }
}
