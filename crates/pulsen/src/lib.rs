//! ポートのアダプター、アプリケーション層、CLI。
//!
//! ドメインとポートの定義は `pulsen-domain` にあり、依存は常に外側(このクレート)から
//! 内側(ドメイン)へ向く。

pub mod adapter;
pub mod application;
pub mod cli;
pub mod util;
