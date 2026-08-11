//! 統合テストが共有するフィクスチャ。

use std::fs;
use std::path::Path;

use pulsen_conformance::Restore;

/// ファイルを読み取れない状態にする。
///
/// 制限が実際に効いたことを確認してから `Some` を返す(ADR-027)。root 実行や
/// 権限を持たないファイルシステムでは `chmod` が効かず、確認を省くと `Err(Io)` を
/// 期待するケースがスキップに落ちずに失敗する。
#[cfg(unix)]
pub fn deny_read(path: &Path) -> Option<Restore> {
    use std::os::unix::fs::PermissionsExt;

    let original = fs::metadata(path).ok()?.permissions();
    let mut denied = original.clone();
    denied.set_mode(0o000);
    fs::set_permissions(path, denied).ok()?;

    let target = path.to_path_buf();
    let restore = Restore::new(move || {
        let _ = fs::set_permissions(&target, original);
    });

    if fs::read(path).is_ok() {
        return None;
    }
    Some(restore)
}

#[cfg(not(unix))]
pub fn deny_read(_path: &Path) -> Option<Restore> {
    None
}
