//! ディレクトリの用意。

use std::fs;
use std::io;
use std::path::Path;

/// 親を含めてディレクトリを用意する。既に存在していれば何もしない。
///
/// 状態ディレクトリ配下はツール管理領域であり、書き込み系の操作が必要に応じて自動作成する
/// 契約になっている(spec/pages ※3)。その自動作成をここ1箇所に集約する。
pub fn ensure_dir(path: &Path) -> io::Result<()> {
    fs::create_dir_all(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 途中のディレクトリも含めて作られる() {
        let base = tempfile::tempdir().expect("一時ディレクトリを作れる");
        let nested = base.path().join("state").join("tasks");

        ensure_dir(&nested).expect("作成できる");

        assert!(nested.is_dir());
    }

    #[test]
    fn 既に存在するディレクトリに対しては成功する() {
        let base = tempfile::tempdir().expect("一時ディレクトリを作れる");

        ensure_dir(base.path()).expect("1回目");
        ensure_dir(base.path()).expect("2回目");

        assert!(base.path().is_dir());
    }

    #[test]
    fn 同名のファイルがある場所には作れない() {
        let base = tempfile::tempdir().expect("一時ディレクトリを作れる");
        let occupied = base.path().join("state");
        fs::write(&occupied, b"file").expect("ファイルを置ける");

        let result = ensure_dir(&occupied);

        assert!(result.is_err());
    }
}
