//! Definition ドメインが外界に要求する操作。

use std::path::{Path, PathBuf};

use super::assembler::{ParsedWorkflow, SourceLocation, WorkflowParseError};
use super::config::GlobalConfig;
use super::reference::WorkflowRef;

/// グローバル設定の読み込みの失敗。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigLoadError {
    /// config.yaml が存在しない(グローバルホームが未初期化)。
    NotFound {
        /// 解決後のホームパス(案内に使う)。
        home: PathBuf,
    },
    /// YAML 構文エラー・構造エラー(未知キー・型不一致・不正な期間等)。
    Invalid {
        /// 原因の説明。論理パス(例: `agents.claude`)はここに含める。
        message: String,
        /// 位置が取れた場合のテキスト上の位置。
        location: Option<SourceLocation>,
    },
    /// 存在するが読めない(権限不足・I/O 障害)。
    Io {
        /// 原因の説明。
        message: String,
    },
}

/// グローバルホームの config.yaml の読み込み。
///
/// 契約:
/// - 読み取り専用。ロック不要
/// - テンプレート内容(プレースホルダ)の検証は行わない(参照時検証の原則)。構造
///   (`cmd` が文字列 / 配列であること等)までを担保する
/// - デコード(YAML → ドメイン型)はアダプター境界の責務
/// - 可視性: 呼び出し時点のファイル内容を返す(キャッシュしない)
pub trait ConfigStore {
    /// 構造検証済みのグローバル設定を返す。
    fn load(&self) -> Result<GlobalConfig, ConfigLoadError>;
}

/// 読み込めたワークフロー定義。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedWorkflow {
    parsed: ParsedWorkflow,
    resolved_from: PathBuf,
}

impl LoadedWorkflow {
    /// パース済み定義と、実際に読み込んだ絶対パスの組を作る。
    pub fn new(parsed: ParsedWorkflow, resolved_from: PathBuf) -> Self {
        Self {
            parsed,
            resolved_from,
        }
    }

    /// パース済み定義。
    pub fn parsed(&self) -> &ParsedWorkflow {
        &self.parsed
    }

    /// 実際に読み込んだ絶対パス。
    pub fn resolved_from(&self) -> &Path {
        &self.resolved_from
    }

    /// 所有権ごと取り出す。
    pub fn into_parts(self) -> (ParsedWorkflow, PathBuf) {
        (self.parsed, self.resolved_from)
    }
}

/// ワークフロー定義の読み込みの失敗。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkflowLoadError {
    /// 解決先が存在しない。
    NotFound {
        /// 解決を試みた絶対パス(案内に使う)。
        attempted: PathBuf,
    },
    /// 読めたが定義として解釈できない。
    ///
    /// 解決先を構造として持つ — `--workflow` を名前で指定した場合、利用者が直接書いて
    /// いないパスを案内できるのはポート側だけである。内側の `WorkflowParseError` は
    /// どれもパスを持たず、`location` は論理位置だけを指す。
    Parse {
        /// 定義として解釈できなかった原因。
        error: WorkflowParseError,
        /// 実際に読み込んだ絶対パス(案内に使う)。
        resolved_from: PathBuf,
    },
    /// 存在するが読めない(権限不足・I/O 障害)。
    Io {
        /// 原因の説明。
        message: String,
    },
}

/// `WorkflowRef` からワークフロー定義を解決・読み込み・パースする。
///
/// 契約:
/// - 解決先の案内は構造化フィールドに一本化する。`NotFound { attempted }` と
///   `Parse { resolved_from }` が対象ファイルを持ち、自由形式のメッセージにパスを
///   前置するのは、解決先を構造として持たない `Io { message }` だけ
/// - 名前解決の規則: `Name(n)` → `<home>/workflows/<n>.yaml`(固定。`.yml` への
///   フォールバックはしない)。`Path(p)` → そのパス(相対はカレントディレクトリから解決)
/// - アダプターが YAML テキストを `RawWorkflowDoc` に変換し(構文エラー・スキーマ外キーは
///   ここで `YamlSyntax` / `UnknownKey` として検出)、`WorkflowAssembler::assemble` で
///   検証する。表示名の決定はしない(呼び出し側が `WorkflowRef::display_name` で行う)
/// - 読み取り専用。可視性: 呼び出し時点のファイル内容
/// - 一意性・並行性: 関与しない(読むだけ)
pub trait WorkflowStore {
    /// 参照を解決してワークフロー定義を読み込む。
    fn load(&self, wf_ref: &WorkflowRef) -> Result<LoadedWorkflow, WorkflowLoadError>;
}
