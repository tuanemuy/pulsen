//! ユースケース。
//!
//! 依存は `pulsen-domain` と std だけに閉じる。アダプターの選択と結線は合成ルート
//! (`crate::cli::wire`)が行う。

pub mod home;
pub mod register_task;
