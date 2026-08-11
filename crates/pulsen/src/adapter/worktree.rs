//! git CLI へのシェルアウトによる `WorktreeManager` の実装。

use std::path::PathBuf;
use std::process::{Command, Output};

use pulsen_domain::execution::{TargetError, WorktreeManager};
use pulsen_domain::task::{BranchName, RepoPath};

/// 起動する git から取り除く環境変数。
///
/// 呼び出し元の環境に残っていると `-C` で指した対象の解決結果を上書きしてしまう
/// (`GIT_CEILING_DIRECTORIES` は上位探索を打ち切り、正当なリポジトリを
/// `NotARepository` に落とす)。ADR-024 の判断基準はこの目的であって変数名の列挙ではない。
const INHERITED_GIT_ENV: &[&str] = &[
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

    /// `git -C <repo> <args...>` を実行する。起動自体の失敗は実行環境のエラー。
    fn run(&self, repo: &RepoPath, args: &[&str]) -> Result<Output, TargetError> {
        let mut command = Command::new(&self.git_program);
        command.arg("-C").arg(repo.as_path()).args(args);
        for name in INHERITED_GIT_ENV {
            command.env_remove(name);
        }
        command.output().map_err(|error| TargetError::Failed {
            message: format!(
                "git ({}) を起動できない: {error}",
                self.git_program.display()
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
                    message: format!("リポジトリのパスを確認できない: {error}"),
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
        let reference = format!("refs/heads/{}", branch.as_str());
        let output = self.run(repo, &["show-ref", "--verify", "--quiet", &reference])?;
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
