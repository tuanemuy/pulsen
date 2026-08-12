//! グローバルホームのレイアウト。
//!
//! ホームの下に何がどう並ぶかは配線情報であってアダプターの実装詳細ではないため、
//! アプリケーション層に置く(ADR-031)。アダプターは導出済みのパスを受け取るだけにする。
//! ホームの**解決**(`--home` > `PULSEN_HOME` > 既定)は合成ルートの責務。
//!
//! レイアウトは全コマンドで1つなので、まだ使うコマンドが無い項目もここに揃う。
//! 呼び出しの無い `pub` を残すのは、ADR-031 がレイアウトとして列挙し**構築時に検証する**
//! 項目に限り、why を添えたときだけ。それ以外(結線の途中結果など)は落とし、必要になった
//! スライスで理由つきで戻す。

use std::path::{Path, PathBuf};

use pulsen_domain::task::{AbsolutePathError, StateRoot, WorktreeRoot};

/// グローバル設定のファイル名。
const CONFIG_FILE: &str = "config.yaml";
/// ワークフロー定義を名前で解決するディレクトリ名。
const WORKFLOWS_DIR: &str = "workflows";
/// ツールが管理する状態のディレクトリ名。
const STATE_DIR: &str = "state";
/// タスクの worktree を置くディレクトリ名。
const WORKTREES_DIR: &str = "worktrees";
/// 排他ロックのファイル名(`state/` 直下)。
const LOCK_FILE: &str = "lock";

/// 解決済みのグローバルホームと、そこから決まる配置。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PulsenHome {
    path: PathBuf,
    state_root: StateRoot,
    worktree_root: WorktreeRoot,
}

impl PulsenHome {
    /// 解決済みのホームパスから配置を導出する。
    ///
    /// 帳簿に載るパスは絶対パスに限る(ドメインの制約)ため、ホーム自体も絶対パスを要求する。
    pub fn new(path: PathBuf) -> Result<Self, AbsolutePathError> {
        let state_root = StateRoot::parse(path.join(STATE_DIR))?;
        let worktree_root = WorktreeRoot::parse(path.join(WORKTREES_DIR))?;
        Ok(Self {
            path,
            state_root,
            worktree_root,
        })
    }

    /// 解決後のホームパス。案内文言に載る。
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// グローバル設定のパス `<home>/config.yaml`。
    pub fn config_path(&self) -> PathBuf {
        self.path.join(CONFIG_FILE)
    }

    /// 名前指定のワークフローを解決するディレクトリ `<home>/workflows`。
    pub fn workflows_dir(&self) -> PathBuf {
        self.path.join(WORKFLOWS_DIR)
    }

    /// 状態のルート `<home>/state`。
    pub fn state_root(&self) -> &StateRoot {
        &self.state_root
    }

    /// worktree のルート `<home>/worktrees`。
    ///
    /// worktree を作る操作は本スライスに無いが、`WorktreeRoot::parse` による絶対性の
    /// 検証は `PulsenHome::new` の一部で、ホームが使えるかどうかの判定に効いている。
    /// 導出結果を取り出す口が無ければ、検証だけが残って使い道が消える。
    pub fn worktree_root(&self) -> &WorktreeRoot {
        &self.worktree_root
    }

    /// 排他ロックのパス `<home>/state/lock`。
    ///
    /// ツール管理領域に置き、タスクファイルの命名形式に合致しないため走査に混ざらない。
    pub fn lock_path(&self) -> PathBuf {
        self.state_root.as_path().join(LOCK_FILE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn absolute(segments: &[&str]) -> PathBuf {
        let mut path = if std::path::MAIN_SEPARATOR == '\\' {
            PathBuf::from("C:\\")
        } else {
            PathBuf::from("/")
        };
        for segment in segments {
            path.push(segment);
        }
        path
    }

    fn home() -> PulsenHome {
        PulsenHome::new(absolute(&["home", "u", ".pulsen"])).expect("受理される")
    }

    #[test]
    fn 設定と定義はホーム直下に配置される() {
        assert_eq!(
            home().config_path(),
            absolute(&["home", "u", ".pulsen", "config.yaml"])
        );
        assert_eq!(
            home().workflows_dir(),
            absolute(&["home", "u", ".pulsen", "workflows"])
        );
    }

    #[test]
    fn 状態とworktreeはそれぞれのルートに配置される() {
        assert_eq!(
            home().state_root().as_path(),
            absolute(&["home", "u", ".pulsen", "state"]).as_path()
        );
        assert_eq!(
            home().worktree_root().as_path(),
            absolute(&["home", "u", ".pulsen", "worktrees"]).as_path()
        );
    }

    #[test]
    fn ロックは状態のルート直下に1つ置かれる() {
        assert_eq!(
            home().lock_path(),
            absolute(&["home", "u", ".pulsen", "state", "lock"])
        );
    }

    #[test]
    fn 相対パスのホームは受理されない() {
        let given = PathBuf::from(".pulsen");
        assert_eq!(
            PulsenHome::new(given.clone()),
            Err(AbsolutePathError::NotAbsolute {
                given: given.join("state")
            })
        );
    }
}
