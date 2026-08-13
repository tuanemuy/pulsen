//! プラットフォーム依存のプロセス操作による `ProcessController` の実装。
//!
//! `unsafe` は Windows のハンドル継承を止める `inheritance` モジュールだけに閉じ、
//! それ以外は std の安全 API と外部コマンドの起動だけで組む(ADR-075 / ADR-100)。
//!
//! 同定情報(起動時刻・プロセスグループ)の取得は、プラットフォームごとの `identity`
//! モジュールにある**1つの関数**に閉じる。記録側(`own_identity`)と照合側が同じ表現を
//! 得ることが `ProcessStartTime` の契約の実質だからで、取得時のロケール・タイムゾーンも
//! ここで固定する。戻り値は三値 — 取得できた(`Ok(Some)`)/ 対象が存在しない(`Ok(None)`)/
//! 取得機構そのものの失敗(`Err(Io)`)。**不在を機構の失敗に畳まない。**

use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};

use pulsen_domain::definition::CommandLine;
use pulsen_domain::execution::{
    ExitCode, Io, ProcessController, SpawnError, WrapperIdentity, WrapperLaunchSpec,
};
use pulsen_domain::task::{Clock, KillIdent, Pid, ProcessStartTime, StartTimeRecord, WorktreePath};

use super::clock::SystemClock;

/// ラッパーモードを表すサブコマンド名。
///
/// CLI のパーサと定義箇所が分かれるため、往復テストで受理を主張する。
pub const WRAPPER_SUBCOMMAND: &str = "wrapper";
/// run ディレクトリを渡すフラグ。
pub const RUN_DIR_FLAG: &str = "--run-dir";
/// worktree を渡すフラグ。
pub const WORKSPACE_FLAG: &str = "--workspace";
/// エージェントコマンドの開始を示す区切り。
pub const COMMAND_SEPARATOR: &str = "--";

/// エージェントを起動できなかったことの符号化値(シェル慣例)。
const NOT_EXECUTABLE: i32 = 126;
/// コマンドが存在しないことの符号化値(シェル慣例)。
const NOT_FOUND: i32 = 127;

/// 同定情報の取得元。
///
/// 何を指すかはプラットフォームで違う(POSIX 非 Linux は `ps` の実行ファイル、Linux は
/// procfs のルート、Windows は powershell の実行ファイル)。構築時に注入することで、
/// 適合テストが「取得機構そのものが失敗する状況」を別のインスタンスとして作れる
/// (ADR-076)。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentitySource(PathBuf);

impl IdentitySource {
    /// 取得元のパスを包む。
    pub fn new(path: PathBuf) -> Self {
        Self(path)
    }

    /// このプラットフォームの既定の取得元。
    ///
    /// 合成ルートがプラットフォームごとの取得元を知らずに済むよう、既定を型の中に置く。
    pub fn platform_default() -> Self {
        Self(identity::default_source())
    }

    fn as_path(&self) -> &Path {
        &self.0
    }
}

/// 1回の観測から得られる同定情報。
///
/// 起動時刻とプロセスグループを同じ観測から返すのは、どちらも POSIX では同じ1回の
/// 取得(`ps` の1回の起動 / `/proc/<pid>/stat` の1回の読み取り)で得られるため。
#[derive(Debug, Clone, PartialEq, Eq)]
struct ObservedProcess {
    starttime: ProcessStartTime,
    kill_ident: KillIdent,
}

/// std の安全 API と外部コマンドの起動でプラットフォーム抽象を満たすコントローラー。
#[derive(Debug, Clone)]
pub struct SystemProcessController {
    self_exe: Option<PathBuf>,
    identity_source: IdentitySource,
    clock: SystemClock,
}

impl SystemProcessController {
    /// 自バイナリのパスと同定情報の取得元を受け取る(ADR-076)。
    ///
    /// `std::env::current_exe()` を読むのは合成ルートの1箇所だけにする — 適合テストは
    /// テストバイナリではなく本物の `pulsen` を注入する必要がある。
    pub fn new(self_exe: PathBuf, identity_source: IdentitySource, clock: SystemClock) -> Self {
        Self {
            self_exe: Some(self_exe),
            identity_source,
            clock,
        }
    }

    /// 自バイナリのパスを持たない構築。
    ///
    /// `self_exe` を使うのは `spawn_wrapper` だけなので、ラッパー自身のように起動を行わない
    /// 経路は自バイナリのパスを解決せずに組める(各コマンドは自身の動作に必要なリソース
    /// だけを検証する)。
    ///
    /// この構成の `spawn_wrapper` は構造上必ず `SpawnError` を返すため、`spawn_wrapper` の
    /// 適合契約の対象外である — 適合スイートが検証するのは [`SystemProcessController::new`]
    /// の構成に限る。ラッパー自身は spawn を行わないので、この経路には到達しない(ADR-076)。
    pub fn without_self_exe(identity_source: IdentitySource, clock: SystemClock) -> Self {
        Self {
            self_exe: None,
            identity_source,
            clock,
        }
    }
}

impl ProcessController for SystemProcessController {
    fn spawn_wrapper(&self, spec: &WrapperLaunchSpec) -> Result<(), SpawnError> {
        let self_exe = self.self_exe.as_ref().ok_or_else(|| SpawnError::Failed {
            message: "自バイナリのパスを持たないため、ラッパーを起動できない".to_owned(),
        })?;
        let mut command = Command::new(self_exe);
        command
            .arg(WRAPPER_SUBCOMMAND)
            .arg(RUN_DIR_FLAG)
            .arg(spec.run_dir().as_path())
            .arg(WORKSPACE_FLAG)
            .arg(spec.workspace().as_path())
            .arg(COMMAND_SEPARATOR)
            .args(spec.agent_cmd().tokens())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        detach(&mut command);

        // 起動後の成否には関知しない。`Child` を待たずに落とすことで、呼び出し側プロセスの
        // 終了がラッパーの完走を妨げない。
        let spawned = {
            let _handles = inheritance::suppress();
            command.spawn()
        };
        spawned
            .map(|_child| ())
            .map_err(|error| SpawnError::Failed {
                message: format!("ラッパー ({}) を起動できない: {error}", self_exe.display()),
            })
    }

    fn own_identity(&self) -> Result<WrapperIdentity, Io> {
        let pid = Pid::new(std::process::id());
        // 自プロセスの観測なので「対象が存在しない」はありえない。三値を二値へ畳むのは
        // 呼び出し側の責務であって、共有する取得関数には持ち込まない(ADR-075)。
        let observed =
            identity::observe(&self.identity_source, pid)?.ok_or_else(|| Io::Failed {
                message: format!("自プロセス (pid {}) を観測できない", pid.get()),
            })?;
        Ok(WrapperIdentity::new(
            pid,
            observed.kill_ident,
            StartTimeRecord::new(observed.starttime, self.clock.now()),
        ))
    }

    fn run_agent(
        &self,
        cmd: &CommandLine,
        cwd: &WorktreePath,
        stdout: &Path,
        stderr: &Path,
    ) -> ExitCode {
        // cwd の到達性を起動前に確かめる。`Command::spawn` は cwd 不在でも `NotFound` を
        // 返すため、確かめないとコマンド不在の 127 と区別できない。
        if !matches!(cwd.as_path().try_exists(), Ok(true)) {
            return ExitCode::new(NOT_EXECUTABLE);
        }
        let (Ok(out), Ok(err)) = (File::create(stdout), File::create(stderr)) else {
            // リダイレクト先を開けないときはエージェントを起動しない。両辺を評価するため、
            // 片方だけ開けたときはそのログが空のまま残る。
            return ExitCode::new(NOT_EXECUTABLE);
        };

        let (program, args) = cmd
            .tokens()
            .split_first()
            .expect("CommandLine は1トークン以上であることが不変条件");
        let status = Command::new(program)
            .args(args)
            .current_dir(cwd.as_path())
            .stdin(Stdio::null())
            .stdout(out)
            .stderr(err)
            .status();

        match status {
            Ok(status) => ExitCode::new(encode(&status)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => ExitCode::new(NOT_FOUND),
            Err(_) => ExitCode::new(NOT_EXECUTABLE),
        }
    }
}

/// 終了状態を符号化値にする。
fn encode(status: &ExitStatus) -> i32 {
    match status.code() {
        Some(code) => code,
        // exit code を持たない終了(シグナル死等)。
        None => signal_code(status),
    }
}

/// 新しいプロセスグループ相当の単位で、呼び出し側から切り離して起動する。
///
/// `process_group(0)` はセッションを分けない(`setsid` 相当ではない)。呼び出し側の終了
/// 後も完走するという契約は満たすが、端末セッションの強制終了には `nohup` 相当の耐性が
/// 無い。cron 運用が主経路なので、`setsid(1)` への依存(macOS に無い)は避ける。
#[cfg(unix)]
fn detach(command: &mut Command) {
    use std::os::unix::process::CommandExt;

    command.process_group(0);
}

#[cfg(windows)]
fn detach(command: &mut Command) {
    use std::os::windows::process::CommandExt;

    /// 新しいプロセスグループの長にする。
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    /// コンソールを引き継がない。
    const DETACHED_PROCESS: u32 = 0x0000_0008;

    command.creation_flags(CREATE_NEW_PROCESS_GROUP | DETACHED_PROCESS);
}

/// 起動の瞬間だけ、呼び出し側から受け継いだハンドルが子へ渡るのを止める(ADR-100)。
///
/// POSIX では std が自前で開いた fd に `CLOEXEC` を立てるため、`Stdio::null()` を渡した
/// 時点で子へ渡るのは NUL だけになる。何もしない。
#[cfg(unix)]
mod inheritance {
    /// 生存期間が「止めている区間」を表す。POSIX では区間そのものが無い。
    pub(super) struct Suppressed;

    pub(super) fn suppress() -> Suppressed {
        Suppressed
    }
}

/// 起動の瞬間だけ、呼び出し側から受け継いだハンドルが子へ渡るのを止める(ADR-100)。
///
/// std の `CreateProcess` は `bInheritHandles = TRUE` 固定で呼ばれる。渡るのは
/// `STARTUPINFO` に載せたハンドルだけではなく、**継承可能フラグの立った全ハンドル**な
/// ので、`Stdio::null()` を渡してもラッパーは呼び出し側の標準ハンドルを掴んだままにな
/// る。呼び出し側が出力をパイプで捕っていると、その書き込み端がラッパーとエージェント
/// に残り、`tick` はエージェントが終わるまで EOF に到達できない — デタッチ起動の契約が
/// 呼び出し方によって崩れる。
///
/// 落とすのは自プロセスの標準ハンドルだけで、`Stdio::null()` が起動時に開く NUL は区間
/// の外で作られるため影響を受けない。復帰させるのは、`Stdio::inherit()` が
/// `STARTUPINFO` に載せるハンドルの継承可能性を前提にするため。
#[cfg(windows)]
#[allow(unsafe_code)]
mod inheritance {
    use std::os::windows::io::RawHandle;

    type Bool = i32;
    type Dword = u32;

    const STD_INPUT_HANDLE: Dword = -10i32 as Dword;
    const STD_OUTPUT_HANDLE: Dword = -11i32 as Dword;
    const STD_ERROR_HANDLE: Dword = -12i32 as Dword;
    const HANDLE_FLAG_INHERIT: Dword = 0x0000_0001;
    /// `GetStdHandle` の失敗を表す値(`INVALID_HANDLE_VALUE`)。
    const INVALID_HANDLE: isize = -1;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        #[link_name = "GetStdHandle"]
        fn get_std_handle(id: Dword) -> RawHandle;
        #[link_name = "GetHandleInformation"]
        fn get_handle_information(handle: RawHandle, flags: *mut Dword) -> Bool;
        #[link_name = "SetHandleInformation"]
        fn set_handle_information(handle: RawHandle, mask: Dword, flags: Dword) -> Bool;
    }

    /// 標準ハンドルが実在するか。
    ///
    /// 標準入出力を持たないプロセスでは `NULL` が、取得に失敗すると `INVALID_HANDLE_VALUE`
    /// が返る。どちらも問い合わせの対象にしない。
    fn usable(handle: RawHandle) -> bool {
        !handle.is_null() && handle as isize != INVALID_HANDLE
    }

    /// 生存期間が「止めている区間」を表す。drop で元の継承可能性へ戻す。
    pub(super) struct Suppressed {
        restore: Vec<RawHandle>,
    }

    pub(super) fn suppress() -> Suppressed {
        let mut restore = Vec::new();
        for id in [STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
            // SAFETY: 引数は Win32 が定める標準ハンドルの識別子。返るのは所有権を伴わな
            // い擬似ハンドルで、閉じてはならない値として扱う。
            let handle = unsafe { get_std_handle(id) };
            if !usable(handle) {
                continue;
            }
            let mut flags: Dword = 0;
            // SAFETY: handle は直前に得た有効なハンドル、flags は書き込める自動変数。
            if unsafe { get_handle_information(handle, &mut flags) } == 0 {
                continue;
            }
            if flags & HANDLE_FLAG_INHERIT == 0 {
                continue;
            }
            // SAFETY: 同上。マスクで継承フラグだけを触る。
            if unsafe { set_handle_information(handle, HANDLE_FLAG_INHERIT, 0) } == 0 {
                continue;
            }
            restore.push(handle);
        }
        Suppressed { restore }
    }

    impl Drop for Suppressed {
        fn drop(&mut self) {
            for handle in &self.restore {
                // SAFETY: 落としたときと同じハンドル。戻せなかった場合に呼び出し側が採れ
                // る手が無いので、結果は捨てる。
                unsafe {
                    set_handle_information(*handle, HANDLE_FLAG_INHERIT, HANDLE_FLAG_INHERIT)
                };
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn flags_of(id: Dword) -> Option<Dword> {
            // SAFETY: 引数は Win32 が定める標準ハンドルの識別子。
            let handle = unsafe { get_std_handle(id) };
            if !usable(handle) {
                return None;
            }
            let mut flags: Dword = 0;
            // SAFETY: handle は直前に得た有効なハンドル、flags は書き込める自動変数。
            (unsafe { get_handle_information(handle, &mut flags) } != 0).then_some(flags)
        }

        #[test]
        fn 区間の中では標準ハンドルが継承されない() {
            let suppressed = suppress();

            for id in [STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
                if let Some(flags) = flags_of(id) {
                    assert_eq!(
                        flags & HANDLE_FLAG_INHERIT,
                        0,
                        "標準ハンドル {id} の継承フラグが残っている"
                    );
                }
            }

            drop(suppressed);
        }

        #[test]
        fn 区間を抜けると元の継承可能性へ戻る() {
            let ids = [STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE];
            let before: Vec<_> = ids.into_iter().map(flags_of).collect();

            drop(suppress());

            let after: Vec<_> = ids.into_iter().map(flags_of).collect();
            assert_eq!(before, after);
        }
    }
}

/// シグナル等による終了を POSIX 慣例の `128+シグナル番号` に符号化する。
#[cfg(unix)]
fn signal_code(status: &ExitStatus) -> i32 {
    use std::os::unix::process::ExitStatusExt;

    status
        .signal()
        .map_or(NOT_EXECUTABLE, |signal| 128 + signal)
}

/// シグナルの概念を持たないプラットフォームでは、起動不能と同じ符号化値に落とす。
#[cfg(not(unix))]
fn signal_code(_status: &ExitStatus) -> i32 {
    NOT_EXECUTABLE
}

/// POSIX(Linux 以外)の同定情報の取得。
#[cfg(all(unix, not(target_os = "linux")))]
mod identity {
    use std::path::PathBuf;
    use std::process::Command;

    use pulsen_domain::execution::Io;
    use pulsen_domain::task::{KillIdent, Pid, ProcessStartTime};

    use super::{IdentitySource, ObservedProcess};

    /// 既定の取得元。
    ///
    /// 絶対パスで固定する。記録した tick と照合する tick で PATH が違えば(cron の
    /// `/usr/bin:/bin` と対話シェルの `/opt/homebrew/bin:...`)別実装の `ps` に解決され、
    /// `lstart` の整形が変わって生存中のプロセスを Dead と誤判定しうる — ロケール非固定の
    /// 場合と同じ結末になる。`/usr/bin/ps` は macOS に無いため `/bin/ps` を採る。
    pub fn default_source() -> PathBuf {
        PathBuf::from("/bin/ps")
    }

    /// 起動時刻とプロセスグループを1回の `ps` の起動から取る。
    ///
    /// `lstart` の表現はロケールとタイムゾーンで変わる(実測: 既定 `水 8/12 20:04:53 2026`
    /// / `LC_ALL=C` `Wed Aug 12 20:04:53 2026` / `LC_ALL=C TZ=UTC` `Wed Aug 12 11:04:53
    /// 2026`)。記録した tick と照合する tick で環境が違えば、生存中のプロセスを Dead と
    /// 誤判定して同一 worktree での並走に至るため、取得時の環境をここで固定する。
    pub fn observe(source: &IdentitySource, pid: Pid) -> Result<Option<ObservedProcess>, Io> {
        let output = identity_command(source, pid)
            .output()
            .map_err(|error| Io::Failed {
                message: format!(
                    "同定情報の取得元 ({}) を起動できない: {error}",
                    source.as_path().display()
                ),
            })?;
        if !output.status.success() {
            // 存在しない pid では exit 1・stdout 空になる(実測)。不在と機構の失敗を
            // 分けないと、生存プロセスの Dead 誤判定か running の永久滞留を招く。
            if output.stdout.is_empty() {
                return Ok(None);
            }
            return Err(Io::Failed {
                message: format!("同定情報の取得が異常終了した (pid {})", pid.get()),
            });
        }

        let text = String::from_utf8_lossy(&output.stdout);
        let line = text.trim_end_matches(['\r', '\n']);
        // `lstart=,pgid=` の順で固定する。最終トークンが PGID、残りを trim したものが
        // lstart になる(順を逆にすると lstart 側に空白のパディングが残る)。
        let Some((lstart, pgid)) = line.rsplit_once(char::is_whitespace) else {
            return Err(Io::Failed {
                message: format!("同定情報を読み取れない (pid {})", pid.get()),
            });
        };
        let Ok(pgid) = pgid.trim().parse::<u32>() else {
            return Err(Io::Failed {
                message: format!("プロセスグループIDを読み取れない (pid {})", pid.get()),
            });
        };
        let starttime =
            ProcessStartTime::parse(lstart.trim().to_owned()).map_err(|_| Io::Failed {
                message: format!("起動時刻が空 (pid {})", pid.get()),
            })?;
        let kill_ident = KillIdent::parse(format!("-{pgid}")).map_err(|_| Io::Failed {
            message: format!("kill 同定子を作れない (pid {})", pid.get()),
        })?;
        Ok(Some(ObservedProcess {
            starttime,
            kill_ident,
        }))
    }

    /// 環境を固定した取得コマンド。
    fn identity_command(source: &IdentitySource, pid: Pid) -> Command {
        let mut command = Command::new(source.as_path());
        command
            .args(["-o", "lstart=,pgid=", "-p", &pid.get().to_string()])
            .env("LC_ALL", "C")
            .env("TZ", "UTC");
        // 周囲の設定が残ると `LC_ALL` / `TZ` の指定より優先されうる。
        command.env_remove("LANG");
        command.env_remove("LC_TIME");
        command
    }

    #[cfg(test)]
    mod tests {
        use std::path::PathBuf;

        use super::*;

        /// 一定時間実行し続けるプロセスを起動する。
        fn spawn_sleeper(own_group: bool) -> std::process::Child {
            let mut command = Command::new("/bin/sleep");
            command.arg("30");
            if own_group {
                use std::os::unix::process::CommandExt;

                command.process_group(0);
            }
            command.spawn().expect("sleep を起動できる")
        }

        fn source() -> IdentitySource {
            IdentitySource::platform_default()
        }

        #[test]
        fn 同一プロセスの2回の観測は等価な起動時刻を返す() {
            let pid = Pid::new(std::process::id());

            let first = observe(&source(), pid)
                .expect("観測できる")
                .expect("生存中");
            let second = observe(&source(), pid)
                .expect("観測できる")
                .expect("生存中");

            assert_eq!(first.starttime, second.starttime);
        }

        #[test]
        fn 取得はロケールとタイムゾーンを固定した環境で行われる() {
            let command = identity_command(&source(), Pid::new(1));

            let fixed: Vec<_> = command.get_envs().collect();
            assert!(fixed.contains(&(
                std::ffi::OsStr::new("LC_ALL"),
                Some(std::ffi::OsStr::new("C"))
            )));
            assert!(fixed.contains(&(
                std::ffi::OsStr::new("TZ"),
                Some(std::ffi::OsStr::new("UTC"))
            )));
            assert!(fixed.contains(&(std::ffi::OsStr::new("LANG"), None)));
            assert!(fixed.contains(&(std::ffi::OsStr::new("LC_TIME"), None)));
        }

        #[test]
        fn 新しいプロセスグループの長として起動した子のkill同定子はそのpidを指す() {
            let mut child = spawn_sleeper(true);
            let pid = Pid::new(child.id());

            let observed = observe(&source(), pid)
                .expect("観測できる")
                .expect("生存中");

            assert_eq!(observed.kill_ident.as_str(), format!("-{}", pid.get()));
            let _ = child.kill();
            let _ = child.wait();
        }

        #[test]
        fn プロセスグループを引き継いだ子のkill同定子は自身のpidを指さない() {
            let mut child = spawn_sleeper(false);
            let pid = Pid::new(child.id());

            let observed = observe(&source(), pid)
                .expect("観測できる")
                .expect("生存中");

            assert_eq!(
                observed.kill_ident,
                observe(&source(), Pid::new(std::process::id()))
                    .expect("観測できる")
                    .expect("生存中")
                    .kill_ident,
                "観測した PGID から作られる"
            );
            let _ = child.kill();
            let _ = child.wait();
        }

        #[test]
        fn 存在しないプロセスは不在として返り機構の失敗に畳まれない() {
            let mut child = spawn_sleeper(true);
            let pid = Pid::new(child.id());
            child.kill().expect("終了させられる");
            child.wait().expect("回収できる");

            assert_eq!(observe(&source(), pid), Ok(None));
        }

        #[test]
        fn 壊した取得元では不在ではなく機構の失敗になる() {
            let broken = IdentitySource::new(PathBuf::from("/no/such/identity-source"));

            assert!(matches!(
                observe(&broken, Pid::new(std::process::id())),
                Err(Io::Failed { .. })
            ));
        }
    }
}

/// Linux の同定情報の取得。
#[cfg(target_os = "linux")]
mod identity {
    use std::path::PathBuf;

    use pulsen_domain::execution::Io;
    use pulsen_domain::task::{KillIdent, Pid, ProcessStartTime};

    use super::{IdentitySource, ObservedProcess};

    /// `<procfs_root>/<pid>/stat` の「最後の `)` より後ろ」における位置。
    ///
    /// 2番目のフィールド(comm)は実行ファイル名を `()` で囲んだもので、名前に空白や `)` を
    /// 含むと素朴な空白分割では位置がずれる。
    const PGRP_INDEX: usize = 2;
    const STARTTIME_INDEX: usize = 19;

    /// 既定の取得元。
    pub fn default_source() -> PathBuf {
        PathBuf::from("/proc")
    }

    /// 起動時刻とプロセスグループを1回の読み取りから取る。
    ///
    /// 起動時刻は boot 相対の clock ticks なので、そのままでは再起動を跨いだ PID 再利用と
    /// 起動 tick の一致で別プロセスを同一と誤判定しうる。boot ごとに変わる boot id と
    /// 合成することでその同時成立を消す。表現は整数と UUID なのでロケールに依存しない。
    pub fn observe(source: &IdentitySource, pid: Pid) -> Result<Option<ObservedProcess>, Io> {
        let root = source.as_path();
        // 取得元は注入されるため、ルートの不在と対象プロセスの不在が同じ `NotFound` として
        // 現れる。分けないと、壊れた取得元が「そのプロセスは死んでいる」を返してしまう。
        if !matches!(root.try_exists(), Ok(true)) {
            return Err(Io::Failed {
                message: format!("同定情報の取得元 ({}) が無い", root.display()),
            });
        }

        let stat_path = root.join(pid.get().to_string()).join("stat");
        let stat = match std::fs::read_to_string(&stat_path) {
            Ok(stat) => stat,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(Io::Failed {
                    message: format!("{}: 読み取れない: {error}", stat_path.display()),
                });
            }
        };

        let Some((_, tail)) = stat.rsplit_once(')') else {
            return Err(Io::Failed {
                message: format!("同定情報を読み取れない (pid {})", pid.get()),
            });
        };
        let fields: Vec<&str> = tail.split_whitespace().collect();
        let (Some(pgrp), Some(ticks)) = (fields.get(PGRP_INDEX), fields.get(STARTTIME_INDEX))
        else {
            return Err(Io::Failed {
                message: format!("同定情報の項目が足りない (pid {})", pid.get()),
            });
        };
        // 形式外の値から `KillIdent` / `ProcessStartTime` を組まない。どちらも非空しか
        // 見ないため、数値性を確かめずに通すと帳簿に kill できない同定子が永続化される。
        let Ok(pgrp) = pgrp.parse::<u32>() else {
            return Err(Io::Failed {
                message: format!("プロセスグループIDを読み取れない (pid {})", pid.get()),
            });
        };
        let Ok(ticks) = ticks.parse::<u64>() else {
            return Err(Io::Failed {
                message: format!("起動時刻を読み取れない (pid {})", pid.get()),
            });
        };

        let boot_id =
            std::fs::read_to_string(root.join("sys/kernel/random/boot_id")).map_err(|error| {
                Io::Failed {
                    message: format!("boot id を読み取れない: {error}"),
                }
            })?;
        let starttime =
            ProcessStartTime::parse(format!("{}:{ticks}", boot_id.trim())).map_err(|_| {
                Io::Failed {
                    message: format!("起動時刻が空 (pid {})", pid.get()),
                }
            })?;
        let kill_ident = KillIdent::parse(format!("-{pgrp}")).map_err(|_| Io::Failed {
            message: format!("kill 同定子を作れない (pid {})", pid.get()),
        })?;
        Ok(Some(ObservedProcess {
            starttime,
            kill_ident,
        }))
    }

    #[cfg(test)]
    mod tests {
        use std::path::PathBuf;
        use std::process::Command;

        use super::*;

        /// 一定時間実行し続けるプロセスを起動する。
        fn spawn_sleeper(own_group: bool) -> std::process::Child {
            let mut command = Command::new("/bin/sleep");
            command.arg("30");
            if own_group {
                use std::os::unix::process::CommandExt;

                command.process_group(0);
            }
            command.spawn().expect("sleep を起動できる")
        }

        fn source() -> IdentitySource {
            IdentitySource::platform_default()
        }

        const BOOT_ID: &str = "0e5ec6f0-2f24-4c19-9a52-9d3e6f9b3d21";

        /// `pgrp`(後半3番目)と起動時刻(後半20番目)を差し替えた `stat` の内容。
        fn stat_content(pgrp: &str, ticks: &str) -> String {
            let filler = ["0"; 16].join(" ");
            format!("4242 (sl ee) p) S 1 {pgrp} {filler} {ticks}")
        }

        /// 内容を指定した procfs 相当の取得元と、そこに置いた pid。
        fn fake_procfs(stat: &str) -> (tempfile::TempDir, IdentitySource, Pid) {
            let root = tempfile::tempdir().expect("一時ディレクトリを作れる");
            let pid = Pid::new(4242);
            let process_dir = root.path().join(pid.get().to_string());
            std::fs::create_dir_all(&process_dir).expect("作れる");
            std::fs::write(process_dir.join("stat"), stat).expect("書ける");
            let random = root.path().join("sys/kernel/random");
            std::fs::create_dir_all(&random).expect("作れる");
            std::fs::write(random.join("boot_id"), format!("{BOOT_ID}\n")).expect("書ける");
            let source = IdentitySource::new(root.path().to_path_buf());
            (root, source, pid)
        }

        #[test]
        fn 同一プロセスの2回の観測は等価な起動時刻を返す() {
            let pid = Pid::new(std::process::id());

            let first = observe(&source(), pid)
                .expect("観測できる")
                .expect("生存中");
            let second = observe(&source(), pid)
                .expect("観測できる")
                .expect("生存中");

            assert_eq!(first.starttime, second.starttime);
        }

        #[test]
        fn 形式どおりのstatからは同定情報が組み立てられる() {
            let (_root, source, pid) = fake_procfs(&stat_content("77", "939856880"));

            let observed = observe(&source, pid).expect("観測できる").expect("生存中");

            assert_eq!(observed.kill_ident.as_str(), "-77");
            assert_eq!(observed.starttime.as_str(), format!("{BOOT_ID}:939856880"));
        }

        #[test]
        fn 形式外のフィールドは不在ではなく機構の失敗になる() {
            for (pgrp, ticks) in [("x", "939856880"), ("-77", "939856880"), ("77", "x")] {
                let (_root, source, pid) = fake_procfs(&stat_content(pgrp, ticks));

                assert!(
                    matches!(observe(&source, pid), Err(Io::Failed { .. })),
                    "pgrp={pgrp} ticks={ticks}"
                );
            }
        }

        #[test]
        fn 起動時刻はbootを跨いで一致しない形に合成される() {
            let observed = observe(&source(), Pid::new(std::process::id()))
                .expect("観測できる")
                .expect("生存中");

            let (boot_id, ticks) = observed
                .starttime
                .as_str()
                .rsplit_once(':')
                .expect("boot id と ticks が合成されている");
            assert!(!boot_id.is_empty());
            assert!(ticks.parse::<u64>().is_ok(), "{ticks}");
        }

        #[test]
        fn 新しいプロセスグループの長として起動した子のkill同定子はそのpidを指す() {
            let mut child = spawn_sleeper(true);
            let pid = Pid::new(child.id());

            let observed = observe(&source(), pid)
                .expect("観測できる")
                .expect("生存中");

            assert_eq!(observed.kill_ident.as_str(), format!("-{}", pid.get()));
            let _ = child.kill();
            let _ = child.wait();
        }

        #[test]
        fn プロセスグループを引き継いだ子のkill同定子は自身のpidを指さない() {
            let mut child = spawn_sleeper(false);
            let pid = Pid::new(child.id());

            let observed = observe(&source(), pid)
                .expect("観測できる")
                .expect("生存中");

            assert_eq!(
                observed.kill_ident,
                observe(&source(), Pid::new(std::process::id()))
                    .expect("観測できる")
                    .expect("生存中")
                    .kill_ident,
                "観測した PGID から作られる"
            );
            let _ = child.kill();
            let _ = child.wait();
        }

        #[test]
        fn 存在しないプロセスは不在として返り機構の失敗に畳まれない() {
            let mut child = spawn_sleeper(true);
            let pid = Pid::new(child.id());
            child.kill().expect("終了させられる");
            child.wait().expect("回収できる");

            assert_eq!(observe(&source(), pid), Ok(None));
        }

        #[test]
        fn 壊した取得元では不在ではなく機構の失敗になる() {
            let broken = IdentitySource::new(PathBuf::from("/no/such/identity-source"));

            assert!(matches!(
                observe(&broken, Pid::new(std::process::id())),
                Err(Io::Failed { .. })
            ));
        }
    }
}

/// Windows の同定情報の取得。
#[cfg(windows)]
mod identity {
    use std::path::PathBuf;
    use std::process::Command;

    use pulsen_domain::execution::Io;
    use pulsen_domain::task::{KillIdent, Pid, ProcessStartTime};

    use super::{IdentitySource, ObservedProcess};

    /// 既定の取得元。
    ///
    /// POSIX 側と違い PATH 解決の名前のままにする。`%SystemRoot%\System32\...` の絶対パスは
    /// 本環境で実測できず、誤った固定値は取得そのものを不能にする。PATH が tick 間で変わると
    /// 照合が壊れうる制約は Issue #10 の実機確認に申し送る(ADR-075)。
    pub fn default_source() -> PathBuf {
        PathBuf::from("powershell")
    }

    /// スクリプトが捕まえた取得機構の失敗を表す終了コード。
    ///
    /// PowerShell 自身が構文エラー等で使う 1 と重ならない値にして、非0の由来を終了コード
    /// から読めるようにする(どちらも `Err(Io)` に写る)。
    const MECHANISM_FAILURE: i32 = 3;

    /// 起動時刻を不変形式で取る。
    ///
    /// `ToString()` はカルチャ依存なので、丸め込みの効かない `"o"`(ISO 8601 の往復可能
    /// 形式)を明示する。該当プロセスが無ければ出力が空になり、不在として返る。
    ///
    /// 既定の `$ErrorActionPreference` は `Continue` で、`Get-CimInstance` の非終端エラー
    /// (アクセス拒否・CIM サービスの異常)は exit 0・stdout 空になる — 不在と同じ形をして
    /// おり、そのままでは生存中のプロセスを死亡と読む。`Stop` に変えて `catch` で非0終了に
    /// 落とし、機構の失敗を出力の形ではなく終了コードから区別する。
    pub fn observe(source: &IdentitySource, pid: Pid) -> Result<Option<ObservedProcess>, Io> {
        let script = format!(
            "$ErrorActionPreference = 'Stop'; \
             try {{ \
             $p = Get-CimInstance Win32_Process -Filter \"ProcessId={}\"; \
             if ($p) {{ $p.CreationDate.ToUniversalTime().ToString(\"o\") }} \
             }} catch {{ exit {MECHANISM_FAILURE} }}",
            pid.get()
        );
        let output = Command::new(source.as_path())
            .args(["-NoProfile", "-Command", &script])
            .output()
            .map_err(|error| Io::Failed {
                message: format!(
                    "同定情報の取得元 ({}) を起動できない: {error}",
                    source.as_path().display()
                ),
            })?;
        if !output.status.success() {
            return Err(Io::Failed {
                message: format!("同定情報の取得が異常終了した (pid {})", pid.get()),
            });
        }

        let text = String::from_utf8_lossy(&output.stdout);
        let trimmed = text.trim();
        if trimmed.is_empty() {
            // 機構の失敗は終了コードで判定済みなので、完走した空出力は対象の不在だけを表す。
            // stderr の非空を機構の失敗に足すと、警告・情報ストリームを stderr に流した完走が
            // 死亡したプロセスを `Err(Io)` に畳み、#3 で running のまま永久滞留する。
            return Ok(None);
        }
        let starttime = ProcessStartTime::parse(trimmed.to_owned()).map_err(|_| Io::Failed {
            message: format!("起動時刻が空 (pid {})", pid.get()),
        })?;
        // プロセスグループ相当の実行単位を `unsafe` なしで扱う手段が無いため、pid を
        // そのまま同定子にする。ジョブオブジェクトへの移行は #3 / #10 の申し送り。
        let kill_ident = KillIdent::parse(pid.get().to_string()).map_err(|_| Io::Failed {
            message: format!("kill 同定子を作れない (pid {})", pid.get()),
        })?;
        Ok(Some(ObservedProcess {
            starttime,
            kill_ident,
        }))
    }
}

#[cfg(test)]
mod tests {
    use pulsen_domain::task::{AttemptNumber, RunDirPath, StateRoot, TaskId, WorktreePath};

    use super::*;

    /// run ディレクトリの導出に使うタスクID(形式を満たす固定値)。
    const TASK_ID: &str = "20260811t091530-k3f9qa1b";

    fn controller() -> SystemProcessController {
        SystemProcessController::new(
            PathBuf::from("pulsen"),
            IdentitySource::platform_default(),
            SystemClock::new(),
        )
    }

    fn worktree(path: &Path) -> WorktreePath {
        WorktreePath::parse(path.to_path_buf()).expect("絶対パスになる")
    }

    #[test]
    fn 自プロセスの同定情報は自身のpidと非空のkill同定子を持つ() {
        let identity = controller().own_identity().expect("取得できる");

        assert_eq!(identity.pid().get(), std::process::id());
        assert!(!identity.kill_ident().as_str().is_empty());
        assert!(!identity.starttime().ident().as_str().is_empty());
    }

    #[test]
    fn 壊した取得元では同定情報の取得が機構の失敗になる() {
        let broken = SystemProcessController::new(
            PathBuf::from("pulsen"),
            IdentitySource::new(PathBuf::from("/no/such/identity-source")),
            SystemClock::new(),
        );

        assert!(matches!(broken.own_identity(), Err(Io::Failed { .. })));
    }

    #[test]
    fn 自バイナリのパスを持たない構築ではラッパーを起動できない() {
        let base = tempfile::tempdir().expect("一時ディレクトリを作れる");
        let state_root = StateRoot::parse(base.path().join("state")).expect("絶対パスになる");
        let spec = WrapperLaunchSpec::new(
            RunDirPath::derive(
                &state_root,
                &TaskId::parse(TASK_ID.to_owned()).expect("形式を満たす"),
                AttemptNumber::parse(1).expect("1以上"),
            ),
            CommandLine::rehydrate(vec!["true".to_owned()]).expect("受理される"),
            worktree(base.path()),
        );
        let controller = SystemProcessController::without_self_exe(
            IdentitySource::platform_default(),
            SystemClock::new(),
        );

        assert!(matches!(
            controller.spawn_wrapper(&spec),
            Err(SpawnError::Failed { .. })
        ));
    }

    #[test]
    fn 作業ディレクトリが存在しなければエージェントを起動せず起動不能を返す() {
        let base = tempfile::tempdir().expect("一時ディレクトリを作れる");
        let missing = base.path().join("gone");
        let cmd = CommandLine::rehydrate(vec!["true".to_owned()]).expect("受理される");

        let code = controller().run_agent(
            &cmd,
            &worktree(&missing),
            &base.path().join("stdout.log"),
            &base.path().join("stderr.log"),
        );

        assert_eq!(code, ExitCode::new(NOT_EXECUTABLE));
        assert!(!base.path().join("stdout.log").exists(), "ログも作られない");
    }

    /// シグナル死の符号化(`128+シグナル番号`)は POSIX の慣例で、適合スイート側は
    /// 「非0の符号化値」までしか主張しない(ADR-082)。具体値はここで固定する。
    #[cfg(unix)]
    #[test]
    fn シグナルで終了したエージェントは128足すシグナル番号に符号化される() {
        let base = tempfile::tempdir().expect("一時ディレクトリを作れる");
        let cmd = CommandLine::rehydrate(vec![
            "/bin/sh".to_owned(),
            "-c".to_owned(),
            "kill -ABRT $$".to_owned(),
        ])
        .expect("受理される");

        let code = controller().run_agent(
            &cmd,
            &worktree(base.path()),
            &base.path().join("stdout.log"),
            &base.path().join("stderr.log"),
        );

        assert_eq!(code, ExitCode::new(128 + 6));
    }
}
