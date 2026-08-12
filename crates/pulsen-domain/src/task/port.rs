//! Task ドメインが外界に要求する操作。

use std::path::PathBuf;

use super::degraded::DegradedTask;
use super::id::TaskId;
use super::task::Task;
use super::time::Timestamp;

/// `create` の失敗。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CreateError {
    /// 同IDが現役・アーカイブのいずれかに存在する。
    Conflict,
    /// 実行環境の入出力エラー。
    Io {
        /// 原因の説明。
        message: String,
    },
}

/// `save` / `save_degraded` の失敗。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaveError {
    /// 現役に存在しない。
    NotFound,
    /// 実行環境の入出力エラー。
    Io {
        /// 原因の説明。
        message: String,
    },
}

/// 読み取り(`find` / `list_active` / `list_archived`)の失敗。
///
/// 個別のタスクファイルの破損はエラーではなく結果の値(`Corrupt`)として返るため、
/// 読み取り経路の失敗は入出力エラーだけになる。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadError {
    /// 実行環境の入出力エラー。
    Io {
        /// 原因の説明。
        message: String,
    },
}

/// `archive` の失敗。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArchiveError {
    /// 現役に存在しない。
    NotFound,
    /// 実行環境の入出力エラー(移動先の作成不可・権限等)。
    Io {
        /// 原因の説明。
        message: String,
    },
}

/// 読み取れたタスク1件。
///
/// スナップショットのみが読めないタスクを、ファイル全体の破損と区別して表現する。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskRecord {
    /// 全フィールドが読めた。
    Intact(Task),
    /// スナップショットだけが読めない。
    SnapshotUnreadable(DegradedTask),
}

/// `find` の結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskLookup {
    /// 現役側で見つかった。
    Active(TaskRecord),
    /// アーカイブ側で見つかった。
    Archived(TaskRecord),
    /// どちらにも無い。
    NotFound,
    /// ファイル全体が読めない。
    Corrupt {
        /// 対象のファイルのパス。
        path: PathBuf,
        /// 読めない理由。
        message: String,
    },
}

/// 走査結果1件。
///
/// 破損側との大きさの差は許容する — 走査は全件読み込み(数十〜数百件)であり、`Box` に
/// よる間接化はポートの契約の形を変えるだけで得るものがない。
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::large_enum_variant)]
pub enum TaskEntry {
    /// 読み取れたタスク。
    Record(TaskRecord),
    /// ファイル全体が読めない。
    Corrupt {
        /// 対象のファイルのパス。
        path: PathBuf,
        /// 読めない理由。
        message: String,
    },
}

/// タスクファイルの読み書き・走査・アーカイブ移動。
///
/// 契約:
/// - **解決順**: `find` は現役 → アーカイブの順で探索する
/// - **一意性**: `create` はID衝突をポートが担保し `Conflict` を返す(呼び出し側の
///   事前確認に依存しない)
/// - **原子性・可視性**: `save` / `archive` の部分的な結果(書きかけの内容・移動の
///   中間状態)は読み手から観測されない。`archive` の直後、対象は現役側から消え
///   アーカイブ側に現れる(read-your-writes)
/// - **並行性**: 書き込みメソッドの呼び出し側は排他ロックの取得を前提とする。ポートは
///   並行書き込みを調停しない。読み取りはロックなしで常に一貫した内容を返す
/// - **破損への書き込み禁止**: `Corrupt` と報告したファイルへ書き込みメソッドを
///   呼んではならない(呼び出し側の責務)。`SnapshotUnreadable` は例外で
///   `save_degraded` のみ許される
/// - **破損スナップショットの温存**: `save_degraded` はファイル内のスナップショットを
///   元の内容のまま書き戻す(修復の材料を消さない)
/// - **ディレクトリの不在**: 走査対象ディレクトリの不在は空結果(`find` は `NotFound`)。
///   走査はタスクファイルの命名形式に合致するエントリのみを対象とし、形式外のエントリは
///   列挙せず触れない。書き込みメソッドは必要なディレクトリを自動作成する
/// - **走査中の消失**: 列挙した後に消えたエントリ(`archive` による移動を含む)は結果から
///   落とす — その領域にもう無いだけで失敗ではない。エントリが残ったまま内容へ到達
///   できない場合は落とさず `Corrupt` として報告する(修復の入口を消さない)
/// - **クエリ要件**: 走査は全件読み込み。絞り込み・並び順は呼び出し側で行い、
///   ページングは持たない
/// - **デコード**: 直列化形式と検証はアダプター境界の責務。タスク側フィールドの破れは
///   `Corrupt`、スナップショットの構造不変条件と `task_status` の照合の破れは
///   `SnapshotUnreadable`。状態間整合の不変条件2〜4は検証しない
pub trait TaskRepository {
    /// 新規タスクを作成する。
    fn create(&self, task: &Task) -> Result<(), CreateError>;

    /// 現役タスクを保存する。
    fn save(&self, task: &Task) -> Result<(), SaveError>;

    /// スナップショット破損タスクを保存する。
    fn save_degraded(&self, task: &DegradedTask) -> Result<(), SaveError>;

    /// IDでタスクを探す。
    fn find(&self, id: &TaskId) -> Result<TaskLookup, ReadError>;

    /// 現役タスクを全件走査する。
    fn list_active(&self) -> Result<Vec<TaskEntry>, ReadError>;

    /// アーカイブ済みタスクを全件走査する。
    fn list_archived(&self) -> Result<Vec<TaskEntry>, ReadError>;

    /// 現役タスクをアーカイブへ移動する。
    fn archive(&self, id: &TaskId) -> Result<(), ArchiveError>;
}

/// タスクIDの発行。
///
/// 契約: 呼び出しごとに実用上衝突しないIDを返す(形式はアダプターに委ねるが `TaskId` の
/// 文字集合制約を満たすこと)。厳密な一意性は `TaskRepository::create` の `Conflict` が
/// バックストップになるため要求しない(呼び出し側は衝突時に再発行してよい)。
pub trait TaskIdGenerator {
    /// タスクIDを発行する。
    fn generate(&self) -> TaskId;
}

/// 現在時刻の取得。
///
/// 契約: 単調性は要求しない(壁時計。マシン再起動・時刻合わせで巻き戻りうる)。
/// 巻き戻りは `Timestamp::elapsed_since` が 0 に丸めることで吸収する。
pub trait Clock {
    /// 現在時刻を返す。
    fn now(&self) -> Timestamp;
}
