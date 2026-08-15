//! ロックを別プロセスに保持させるフィクスチャ。

use std::env;
use std::io::{self, BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{OnceLock, mpsc};
use std::thread;
use std::time::Duration;

/// ロックを取得した保持プロセスが書き出す合図。
const LOCKED: &str = "locked";

/// 合図を待つ期限。probe ではこの期限を超えたことが「この環境は合図を期限内に返せない」
/// という能力の判定になり、probe が成立したあとに超えた場合は、同じ手順が一度は期限内に
/// 返っている以上その超過を能力の宣言としては読めないので失敗になる(繰り返し起きるなら
/// 期限そのものの見直しとして扱う)。いずれの場合も待ち続けないのは、
/// フィクスチャのハングがテストの失敗より診断が難しいため。
const SIGNAL_DEADLINE: Duration = Duration::from_secs(10);

/// 実行ファイルが無いときの案内。環境の能力ではなくビルド構成の誤りなので、
/// スキップにはせず失敗させる(HOOKS.md)。
const PROGRAM_MISSING: &str = "ロック保持フィクスチャ(examples/lock_holder)の実行ファイルが無い。\
    単一のテストターゲットを指定した実行では example がビルドされないため、\
    `cargo test --workspace` のように example をビルドする形で実行する";

/// 別プロセスにロックを保持させられるか。
pub enum HolderCapability {
    /// 保持プロセスを起動でき、合図の期限の超過を観測しなかった。この変種を名乗る根拠は
    /// 経路によって「合図が期限内に返った」と「合図を観測しないまま読み取りが終わった」の
    /// 2つに分かれる。
    Available(HolderProgram),
    /// 起動はできるが、合図が期限内に返らない。この観測は原因を決めず、環境の遅さと
    /// 保持プロセス側の退行を区別しない。
    SignalTimedOut,
    /// 実行ファイルが無い(example がビルドされていない)。
    ProgramMissing,
    /// 実行ファイルはあるが起動できない(起動しても合図の経路を用意できない場合を含む)。
    /// 値は起動が失敗した理由。
    ProgramUnusable(io::Error),
}

/// probe が解決した保持プロセスの実行ファイル。
///
/// パスを取り出せる手段を持たせない — 持たせると、能力の判定を迂回して直接起動する
/// 呼び出し側が `lock.rs` の外に書けてしまう。
pub struct HolderProgram(PathBuf);

impl HolderProgram {
    fn path(&self) -> &Path {
        &self.0
    }
}

/// 保持プロセスの起動を試みた結果。
enum Started {
    /// 期限内に子が応答した(合図を書いたか、書かずに終了したか)。`locked` は合図が
    /// `LOCKED` だったか。
    Signaled { holder: Child, locked: bool },
    /// 合図を読み取れなかった(子が読めない出力を返した場合と、読み取りが結果を返さずに
    /// 終わった場合)。
    SignalUnreadable { holder: Child, error: io::Error },
    /// 合図も終了も期限内に返らなかった(保持プロセスは終了させてある)。
    SignalTimedOut,
    /// 起動できなかった(起動したが `stdout` を取得できなかった場合を含む)。
    SpawnFailed(io::Error),
}

/// ロックを保持し続けるフィクスチャの実行ファイル。
///
/// パッケージ全体を対象にした `cargo test` は example もビルドするため、バイナリと同じ
/// 出力ディレクトリの `examples/` に置かれる。不在はフィクスチャがビルドされていないことを
/// 意味し、環境の能力ではない。
fn holder_program() -> Option<PathBuf> {
    let binary = Path::new(env!("CARGO_BIN_EXE_pulsen"));
    let program = binary
        .parent()?
        .join("examples")
        .join(format!("lock_holder{}", env::consts::EXE_SUFFIX));
    program.is_file().then_some(program)
}

/// 実際に1回保持させてみて能力を決める(1度だけ評価して使い回す)。
///
/// フィクスチャが本番で踏む手順そのもの(起動して合図を待つ)で判定するため、
/// 判定と実際のスキップが食い違わない(`permission_restrictions_effective` と同じ性質)。
pub fn holder_capability() -> &'static HolderCapability {
    static CAPABILITY: OnceLock<HolderCapability> = OnceLock::new();
    CAPABILITY.get_or_init(probe_holder)
}

fn probe_holder() -> HolderCapability {
    let Some(program) = holder_program() else {
        return HolderCapability::ProgramMissing;
    };
    // 本番のロックとぶつからない置き場を使い、判定が終わるまで保持する。
    let dir = tempfile::tempdir().expect("一時ディレクトリを作れる");
    match start_holder(&program, &dir.path().join("lock")) {
        // probe が測るのは合図が期限内に返るかだけで、読み取れたかどうかは能力の判定に
        // 入れない(読み取りの異常はケース側で失敗として扱う)。読み取りが結果を返さずに
        // 終わった場合も同じ腕に入り、合図を観測しないまま Available に数える。倒れる向きは
        // 失敗側で、後続のケースは spawn_holder でパニックするため、静かな緑にはならない。
        Started::Signaled { holder, locked: _ }
        | Started::SignalUnreadable { holder, error: _ } => {
            kill_and_wait(holder);
            HolderCapability::Available(HolderProgram(program))
        }
        Started::SignalTimedOut => HolderCapability::SignalTimedOut,
        Started::SpawnFailed(error) => HolderCapability::ProgramUnusable(error),
    }
}

/// 保持プロセスを起動し、合図が期限内に返るかまでを見る。
fn start_holder(program: &Path, lock_path: &Path) -> Started {
    let mut holder = match Command::new(program)
        .arg(lock_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(holder) => holder,
        Err(error) => return Started::SpawnFailed(error),
    };
    // パイプを持たない子は合図を返しようがなく、原因も回避方法も起動の失敗と同じ経路にある。
    let Some(stdout) = holder.stdout.take() else {
        kill_and_wait(holder);
        return Started::SpawnFailed(io::Error::other("保持プロセスの標準出力を取得できない"));
    };
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut signal = String::new();
        let read = BufReader::new(stdout)
            .read_line(&mut signal)
            .map(|_| signal);
        let _ = sender.send(read);
    });
    match receiver.recv_timeout(SIGNAL_DEADLINE) {
        Ok(Ok(signal)) => Started::Signaled {
            holder,
            locked: signal.trim() == LOCKED,
        },
        Ok(Err(error)) => Started::SignalUnreadable { holder, error },
        Err(mpsc::RecvTimeoutError::Timeout) => {
            kill_and_wait(holder);
            Started::SignalTimedOut
        }
        // 期限の超過を観測していないので、環境の能力(＝スキップを許容する側)には倒さない。
        Err(mpsc::RecvTimeoutError::Disconnected) => Started::SignalUnreadable {
            holder,
            error: io::Error::other("合図の読み取りが結果を返さずに終了した"),
        },
    }
}

/// 保持プロセスを起動し、ロックを取得できたかを添えて返す。
///
/// `None` はこの環境が合図を期限内に返せないこと(宣言済みスキップ)だけを意味する。
/// 実行ファイルが無い場合は原因も回避方法も一意で、起動できない場合は理由が起動時の
/// エラーにしか無い。合図を読み取れなかった場合と、probe が成立したあとのタイムアウトも
/// 同じで、いずれもスキップの宣言だけからは次の一手が定まらないため、スキップに
/// 逃がさず失敗させる。
pub fn spawn_holder(lock_path: &Path) -> Option<(Child, bool)> {
    let program = match holder_capability() {
        HolderCapability::Available(program) => program,
        // 起動を試みない — 5件それぞれに期限ぶんの待ちを重ねない。
        HolderCapability::SignalTimedOut => return None,
        HolderCapability::ProgramMissing => panic!("{PROGRAM_MISSING}"),
        HolderCapability::ProgramUnusable(error) => panic!(
            "ロック保持フィクスチャ(examples/lock_holder)を起動できなかった\
             (probe の起動時に観測した理由): {error}"
        ),
    };
    match start_holder(program.path(), lock_path) {
        Started::Signaled { holder, locked } => Some((holder, locked)),
        Started::SignalUnreadable { holder, error } => {
            kill_and_wait(holder);
            panic!("保持プロセスの合図を読み取れなかった: {error}")
        }
        Started::SignalTimedOut => panic!(
            "保持プロセスの合図が {SIGNAL_DEADLINE:?} 以内に返らなかった。\
             probe は同じ手順で成立している(この環境で繰り返し起きるなら SIGNAL_DEADLINE を見直す)"
        ),
        Started::SpawnFailed(error) => {
            panic!("ロック保持フィクスチャ(examples/lock_holder)を起動できなかった: {error}")
        }
    }
}

/// ロックを保持している別プロセスを用意する。
///
/// `None` の意味は `spawn_holder` と同じ。誰も保持していないパスで取得の合図が返らない
/// のは環境の能力ではないので、スキップにはしない。
pub fn hold(lock_path: &Path) -> Option<Child> {
    let (holder, locked) = spawn_holder(lock_path)?;
    if !locked {
        kill_and_wait(holder);
        // 子の標準エラーは捨てているため、競合なのか機構の異常なのかを名指しする材料は無い。
        panic!("保持プロセスが、誰も保持していないロックを取得した合図を返さなかった");
    }
    Some(holder)
}

/// 保持プロセスの標準入力を閉じて終了を待つ。
pub fn release(mut holder: Child) -> Option<()> {
    drop(holder.stdin.take());
    holder.wait().ok()?;
    Some(())
}

/// 保持プロセスを畳む。正常終了できるかは測っていないので、その正常終了は待たずに
/// `kill` してから終了を回収する。回収そのものには期限が無いので、`kill` を受けても
/// 即座には終われない相手に当たればここで止まる。
fn kill_and_wait(mut holder: Child) {
    let _ = holder.kill();
    let _ = holder.wait();
}
