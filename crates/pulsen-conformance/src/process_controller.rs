//! ProcessController の適合ケース(`spec/testcases/ports/process-controller.md` の27行)。
//!
//! スイートを3つに分けるのは、要る前提が違うため(ADR-083)。`own_identity` / `run_agent`
//! の13件はアダプター単体で閉じ、`spawn_wrapper` の3件と `starttime_of` / `kill` /
//! `try_kill_remnants` の11件はラッパーモードの実装(実バイナリ)を要する。
//!
//! 期待結果は契約の語彙(「非0の符号化値」「実行単位」)で書き、プラットフォーム固有の
//! 具体値(`128+シグナル番号`)には踏み込まない(ADR-082)。

/// `own_identity` / `run_agent` のスイート(13行)。
pub mod identity_and_agent {
    use std::time::Instant;

    use pulsen_domain::execution::{ExitCode, Io, ProcessController};

    use crate::{AgentBehavior, CaseOutcome, ProcessControllerHarness, require};

    pub fn tc_port_process_controller_004_自プロセスの同定情報が取得できる(
        harness: &impl ProcessControllerHarness,
    ) -> CaseOutcome {
        let before = require!(harness.observe_wall_clock());

        let identity = harness
            .controller()
            .own_identity()
            .expect("自プロセスの同定情報を取得できる");

        let after = require!(harness.observe_wall_clock());
        assert_eq!(identity.pid().get(), std::process::id(), "自プロセスの pid");
        assert!(
            !identity.kill_ident().as_str().is_empty(),
            "kill 同定子は非空"
        );
        let wall = identity.starttime().wall();
        assert!(
            before <= wall && wall <= after,
            "壁時計時刻が呼び出し前後の範囲に入る"
        );
        CaseOutcome::Ran
    }

    pub fn tc_port_process_controller_005_取得機構の失敗は不正な同定情報で装われない(
        harness: &impl ProcessControllerHarness,
    ) -> CaseOutcome {
        let failing = require!(harness.failing_identity_controller());

        match failing.own_identity() {
            Err(Io::Failed { message }) => assert!(!message.is_empty(), "原因が説明される"),
            Ok(identity) => panic!("取得機構が失敗しても `Ok` を装わない: {identity:?}"),
        }
        CaseOutcome::Ran
    }

    pub fn tc_port_process_controller_017_成功したエージェントの終了結果はゼロになる(
        harness: &impl ProcessControllerHarness,
    ) -> CaseOutcome {
        let command = require!(harness.agent_command(AgentBehavior::Exit(0)));
        let cwd = require!(harness.worktree());
        let (stdout, stderr) = require!(harness.log_paths());

        let code = harness
            .controller()
            .run_agent(&command, &cwd, &stdout, &stderr);

        assert_eq!(code, ExitCode::new(0));
        CaseOutcome::Ran
    }

    pub fn tc_port_process_controller_018_非ゼロの終了コードはそのまま返る(
        harness: &impl ProcessControllerHarness,
    ) -> CaseOutcome {
        let command = require!(harness.agent_command(AgentBehavior::Exit(7)));
        let cwd = require!(harness.worktree());
        let (stdout, stderr) = require!(harness.log_paths());

        let code = harness
            .controller()
            .run_agent(&command, &cwd, &stdout, &stderr);

        assert_eq!(code, ExitCode::new(7));
        CaseOutcome::Ran
    }

    pub fn tc_port_process_controller_019_作業ディレクトリは常にworktreeになる(
        harness: &impl ProcessControllerHarness,
    ) -> CaseOutcome {
        let cwd = require!(harness.worktree());
        let command = require!(harness.agent_command(AgentBehavior::CheckCwd(cwd.clone())));
        let (stdout, stderr) = require!(harness.log_paths());

        let code = harness
            .controller()
            .run_agent(&command, &cwd, &stdout, &stderr);

        assert_eq!(
            code,
            ExitCode::new(0),
            "エージェントは worktree で実行される"
        );
        CaseOutcome::Ran
    }

    pub fn tc_port_process_controller_020_標準出力と標準エラーは指定パスへ書かれる(
        harness: &impl ProcessControllerHarness,
    ) -> CaseOutcome {
        let command = require!(harness.agent_command(AgentBehavior::Print {
            stdout: "標準出力の内容".to_owned(),
            stderr: "標準エラーの内容".to_owned(),
        }));
        let cwd = require!(harness.worktree());
        let (stdout, stderr) = require!(harness.log_paths());

        assert_eq!(
            harness
                .controller()
                .run_agent(&command, &cwd, &stdout, &stderr),
            ExitCode::new(0)
        );

        assert_eq!(read(&stdout), "標準出力の内容");
        assert_eq!(read(&stderr), "標準エラーの内容");
        CaseOutcome::Ran
    }

    pub fn tc_port_process_controller_021_引数はシェルに解釈されずリテラルで渡る(
        harness: &impl ProcessControllerHarness,
    ) -> CaseOutcome {
        let tokens: Vec<String> = LITERAL_TOKENS
            .iter()
            .map(|token| (*token).to_owned())
            .collect();
        let command = require!(harness.agent_command(AgentBehavior::EchoArgs(tokens.clone())));
        let cwd = require!(harness.worktree());
        let (stdout, stderr) = require!(harness.log_paths());

        let code = harness
            .controller()
            .run_agent(&command, &cwd, &stdout, &stderr);

        assert_eq!(code, ExitCode::new(0));
        let received: Vec<String> = read(&stdout).lines().map(str::to_owned).collect();
        assert_eq!(
            received, tokens,
            "展開・連結・再分割のいずれも起きずリテラルで渡る"
        );
        CaseOutcome::Ran
    }

    pub fn tc_port_process_controller_022_存在しないコマンドはコマンド不在に符号化される(
        harness: &impl ProcessControllerHarness,
    ) -> CaseOutcome {
        let command = require!(harness.missing_command());
        let cwd = require!(harness.worktree());
        let (stdout, stderr) = require!(harness.log_paths());

        let code = harness
            .controller()
            .run_agent(&command, &cwd, &stdout, &stderr);

        assert_eq!(code, ExitCode::new(127));
        CaseOutcome::Ran
    }

    pub fn tc_port_process_controller_023_実行できない実体は実行不能に符号化される(
        harness: &impl ProcessControllerHarness,
    ) -> CaseOutcome {
        let command = require!(harness.non_executable_command());
        let cwd = require!(harness.worktree());
        let (stdout, stderr) = require!(harness.log_paths());

        let code = harness
            .controller()
            .run_agent(&command, &cwd, &stdout, &stderr);

        assert_eq!(code, ExitCode::new(126));
        CaseOutcome::Ran
    }

    pub fn tc_port_process_controller_024_exitコードを持たない終了は非ゼロの符号化値になる(
        harness: &impl ProcessControllerHarness,
    ) -> CaseOutcome {
        let command = require!(harness.agent_command(AgentBehavior::Abort));
        let cwd = require!(harness.worktree());
        let (stdout, stderr) = require!(harness.log_paths());

        let code = harness
            .controller()
            .run_agent(&command, &cwd, &stdout, &stderr);

        // 具体値(POSIX 慣例の 128+シグナル番号)はプラットフォーム実装の性質であり、
        // 契約が要求するのは「常に値を返し、非0である」ことまで(ADR-082)。
        assert!(!code.is_success(), "非0の符号化値になる: {code:?}");
        CaseOutcome::Ran
    }

    pub fn tc_port_process_controller_025_ログを開けなければエージェントを起動しない(
        harness: &impl ProcessControllerHarness,
    ) -> CaseOutcome {
        let command = require!(harness.agent_command(AgentBehavior::Exit(0)));
        let cwd = require!(harness.worktree());
        let (_, stderr) = require!(harness.log_paths());
        let (stdout, _restore) = require!(harness.unwritable_log_path());

        let code = harness
            .controller()
            .run_agent(&command, &cwd, &stdout, &stderr);

        // 起動していれば成功(0)が返る。126 はエージェントの副作用が生じていないこと。
        assert_eq!(code, ExitCode::new(126));
        CaseOutcome::Ran
    }

    pub fn tc_port_process_controller_026_作業ディレクトリが無ければ非ゼロで返る(
        harness: &impl ProcessControllerHarness,
    ) -> CaseOutcome {
        let command = require!(harness.agent_command(AgentBehavior::Exit(0)));
        let cwd = require!(harness.missing_worktree());
        let (stdout, stderr) = require!(harness.log_paths());

        let code = harness
            .controller()
            .run_agent(&command, &cwd, &stdout, &stderr);

        assert!(!code.is_success(), "起動不能として非0を返す: {code:?}");
        CaseOutcome::Ran
    }

    pub fn tc_port_process_controller_027_呼び出しはエージェントの終了まで戻らない(
        harness: &impl ProcessControllerHarness,
    ) -> CaseOutcome {
        let command = require!(harness.agent_command(AgentBehavior::Sleep(AGENT_RUNTIME)));
        let cwd = require!(harness.worktree());
        let (stdout, stderr) = require!(harness.log_paths());

        let started = Instant::now();
        let code = harness
            .controller()
            .run_agent(&command, &cwd, &stdout, &stderr);

        assert!(started.elapsed() >= AGENT_RUNTIME, "同期実行である");
        assert_eq!(code, ExitCode::new(0));
        CaseOutcome::Ran
    }

    /// シェルを経由すると原形をとどめないトークン。
    ///
    /// 展開(`*` / `$VAR` / バッククォート)・連結と再分割(空白)・制御構文(`&&` / `;`)・
    /// リダイレクト(`>`)と、テンプレートのプレースホルダ文字列を含める。
    const LITERAL_TOKENS: [&str; 7] = [
        "*",
        "$HOME",
        "&&",
        ">out.txt",
        "{input}",
        "引数 の 中の 空白",
        ";",
    ];

    /// エージェントを実行し続けさせる時間。
    const AGENT_RUNTIME: std::time::Duration = std::time::Duration::from_millis(300);

    /// リダイレクト先の内容。`run_agent` の契約がパスで書かれているため、観測もパスで行う。
    fn read(path: &std::path::Path) -> String {
        std::fs::read_to_string(path).unwrap_or_else(|error| {
            panic!("{}: ログを読めない: {error}", path.display());
        })
    }
}

/// `spawn_wrapper` のスイート(3行)。
pub mod spawn {
    use pulsen_domain::execution::{ProcessController, SpawnError};

    use crate::{AgentBehavior, CaseOutcome, ProcessControllerHarness, require};

    pub fn tc_port_process_controller_001_起動は成否を戻り値に持たない(
        harness: &impl ProcessControllerHarness,
    ) -> CaseOutcome {
        let spec = require!(harness.launch_spec(AgentBehavior::Exit(0)));
        assert!(
            require!(harness.run_dir_is_empty(&spec)),
            "起動の前は run ディレクトリに何も無い"
        );

        assert_eq!(harness.controller().spawn_wrapper(&spec), Ok(()));

        // 結末は run ディレクトリ経由でのみ現れる。観測が起動の前後で**反転すること**
        // まで主張する — 真偽を定数で返すハーネスはどちらかの側で落ちる(RunStore の
        // `attempt_dir_present` と同じ規律)。
        assert!(
            require!(harness.wait_for_run_files(&spec)),
            "ラッパーが起動して run ファイルを残す"
        );
        assert!(
            !require!(harness.run_dir_is_empty(&spec)),
            "起動の後は run ディレクトリに結末が現れる"
        );
        CaseOutcome::Ran
    }

    pub fn tc_port_process_controller_002_ラッパーは呼び出し側の終了後も完走する(
        harness: &impl ProcessControllerHarness,
    ) -> CaseOutcome {
        let spec = require!(harness.launch_spec(AgentBehavior::Sleep(AGENT_RUNTIME)));

        require!(harness.spawn_from_other_process(&spec));

        assert!(
            require!(harness.wait_for_run_files(&spec)),
            "呼び出し側の終了後も starttime・pid・exit が揃う"
        );
        CaseOutcome::Ran
    }

    pub fn tc_port_process_controller_003_起動不能は副作用を残さず失敗として返る(
        harness: &impl ProcessControllerHarness,
    ) -> CaseOutcome {
        let failing = require!(harness.failing_controller());
        let spec = require!(harness.launch_spec(AgentBehavior::Exit(0)));

        match failing.spawn_wrapper(&spec) {
            Err(SpawnError::Failed { message }) => assert!(!message.is_empty(), "原因が説明される"),
            Ok(()) => panic!("起動できない状況では失敗として返る"),
        }

        assert!(
            require!(harness.run_dir_is_empty(&spec)),
            "run ディレクトリに副作用を残さない"
        );
        CaseOutcome::Ran
    }

    /// ラッパーが呼び出し側より長く生きることを確かめるための実行時間。
    const AGENT_RUNTIME: std::time::Duration = std::time::Duration::from_millis(300);
}

/// `starttime_of` / `kill` / `try_kill_remnants` のスイート(11行)。
///
/// 期待結果は契約の語彙で書く — 終了の主張は「実行単位に属する全プロセスが終了する」で
/// あり、その観測は `starttime_of` が各PIDで `None` を返すことに還元する(ADR-082)。
pub mod observation {
    use std::time::{Duration, Instant};

    use pulsen_domain::execution::{Io, KillError, ProcessController, RemnantOutcome};
    use pulsen_domain::task::Pid;

    use crate::{CaseOutcome, ExecutionUnit, ProcessControllerHarness, require};

    pub fn tc_port_process_controller_006_生存中のプロセスの起動時刻は取得できる(
        harness: &impl ProcessControllerHarness,
    ) -> CaseOutcome {
        let observed = harness
            .controller()
            .starttime_of(own_pid())
            .expect("取得機構は成功する");

        assert!(observed.is_some(), "生存中のプロセスは不在にならない");
        CaseOutcome::Ran
    }

    pub fn tc_port_process_controller_007_終了したプロセスは不在として返る(
        harness: &impl ProcessControllerHarness,
    ) -> CaseOutcome {
        let pid = require!(harness.terminated_pid());

        let observed = harness
            .controller()
            .starttime_of(pid)
            .expect("取得機構は成功する");

        assert_eq!(observed, None, "不在 = 死亡");
        CaseOutcome::Ran
    }

    pub fn tc_port_process_controller_008_同一プロセスの2回の取得は等価な値を返す(
        harness: &impl ProcessControllerHarness,
    ) -> CaseOutcome {
        let controller = harness.controller();

        let first = controller.starttime_of(own_pid()).expect("取得できる");
        let second = controller.starttime_of(own_pid()).expect("取得できる");

        assert!(first.is_some(), "生存中である");
        assert_eq!(first, second, "等価比較に使える安定値である");
        CaseOutcome::Ran
    }

    pub fn tc_port_process_controller_009_記録と照合は同一の取得手段で行われる(
        harness: &impl ProcessControllerHarness,
    ) -> CaseOutcome {
        let controller = harness.controller();
        let identity = controller.own_identity().expect("自プロセスを観測できる");

        let observed = controller
            .starttime_of(identity.pid())
            .expect("取得できる")
            .expect("生存中である");

        assert_eq!(
            &observed,
            identity.starttime().ident(),
            "記録側と照合側が同じ表現を得る"
        );
        CaseOutcome::Ran
    }

    pub fn tc_port_process_controller_010_取得機構の失敗は死亡に写像されない(
        harness: &impl ProcessControllerHarness,
    ) -> CaseOutcome {
        let failing = require!(harness.failing_identity_controller());

        match failing.starttime_of(own_pid()) {
            Err(Io::Failed { message }) => assert!(!message.is_empty(), "原因が説明される"),
            Ok(observed) => panic!(
                "取得機構の失敗を不在(= 死亡)に畳まない: {observed:?}。畳むと生存プロセスの \
                 Dead 誤判定から同一 worktree での並走に至る"
            ),
        }
        CaseOutcome::Ran
    }

    pub fn tc_port_process_controller_011_killは実行単位の全プロセスを終了させる(
        harness: &impl ProcessControllerHarness,
    ) -> CaseOutcome {
        let unit = require!(harness.live_execution_unit());
        let controller = harness.controller();
        assert!(
            all_alive(controller, &unit),
            "終了させる前は全メンバーが生存している"
        );

        assert_eq!(controller.kill(&unit.kill_ident), Ok(()));

        assert!(
            wait_until_all_gone(controller, &unit),
            "実行単位に属する全プロセスが終了する"
        );
        assert!(
            controller
                .starttime_of(own_pid())
                .expect("取得できる")
                .is_some(),
            "呼び出し側は実行単位が分離されているため影響を受けない"
        );
        CaseOutcome::Ran
    }

    pub fn tc_port_process_controller_012_killは同定子だけを入力に実行できる(
        harness: &impl ProcessControllerHarness,
    ) -> CaseOutcome {
        let (unit, restarted) = require!(harness.detached_execution_unit());

        assert_eq!(restarted.kill(&unit.kill_ident), Ok(()));

        assert!(
            wait_until_all_gone(restarted, &unit),
            "起動した側とも起動時のインスタンスとも縁が切れていても終了させられる"
        );
        CaseOutcome::Ran
    }

    pub fn tc_port_process_controller_013_終了操作の失敗は値として返る(
        harness: &impl ProcessControllerHarness,
    ) -> CaseOutcome {
        let failing = require!(harness.failing_terminator_controller());
        let unit = require!(harness.live_execution_unit());

        match failing.kill(&unit.kill_ident) {
            Err(KillError::Failed { message }) => assert!(!message.is_empty(), "原因が説明される"),
            Ok(()) => panic!("終了操作が失敗する状況では失敗として返る"),
        }
        CaseOutcome::Ran
    }

    pub fn tc_port_process_controller_014_残存プロセスはベストエフォートで終了する(
        harness: &impl ProcessControllerHarness,
    ) -> CaseOutcome {
        let unit = require!(harness.orphaned_execution_unit());
        let controller = harness.controller();
        assert!(
            all_alive(controller, &unit),
            "ラッパーの死後も残存メンバーが生存している"
        );

        assert_eq!(
            controller.try_kill_remnants(&unit.kill_ident),
            RemnantOutcome::Killed
        );

        assert!(
            wait_until_all_gone(controller, &unit),
            "残存プロセスが終了する"
        );
        CaseOutcome::Ran
    }

    pub fn tc_port_process_controller_015_同定できなければ何も終了させない(
        harness: &impl ProcessControllerHarness,
    ) -> CaseOutcome {
        let unidentifiable = require!(harness.failing_identity_controller());
        let unit = require!(harness.live_execution_unit());
        let observer = harness.controller();

        assert_eq!(
            unidentifiable.try_kill_remnants(&unit.kill_ident),
            RemnantOutcome::NotIdentifiable
        );

        assert!(
            all_alive(observer, &unit),
            "いかなるプロセスも終了させない(無関係なプロセスの誤殺がない)"
        );
        CaseOutcome::Ran
    }

    pub fn tc_port_process_controller_016_残存終了の失敗は値として返る(
        harness: &impl ProcessControllerHarness,
    ) -> CaseOutcome {
        let failing = require!(harness.failing_terminator_controller());
        let unit = require!(harness.orphaned_execution_unit());

        match failing.try_kill_remnants(&unit.kill_ident) {
            RemnantOutcome::Failed { message } => assert!(!message.is_empty(), "原因が説明される"),
            other => panic!("終了操作が失敗する状況では失敗として返る: {other:?}"),
        }
        CaseOutcome::Ran
    }

    /// 自プロセスのPID。
    fn own_pid() -> Pid {
        Pid::new(std::process::id())
    }

    /// 全メンバーが観測できるか。
    fn all_alive(controller: &impl ProcessController, unit: &ExecutionUnit) -> bool {
        unit.members.iter().all(|pid| {
            controller
                .starttime_of(*pid)
                .expect("取得機構は成功する")
                .is_some()
        })
    }

    /// 全メンバーが観測できなくなるまで期限つきで待つ。
    ///
    /// 終了操作の送出と各プロセスの消滅の間には時間差がある。期限は「終了しない実装を
    /// 検出する」ための歯止めであって、契約の一部ではない。
    fn wait_until_all_gone(controller: &impl ProcessController, unit: &ExecutionUnit) -> bool {
        let deadline = Instant::now() + TERMINATION_DEADLINE;
        loop {
            if unit
                .members
                .iter()
                .all(|pid| controller.starttime_of(*pid) == Ok(None))
            {
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(POLL_INTERVAL);
        }
    }

    /// 実行単位の消滅を待つ期限。
    const TERMINATION_DEADLINE: Duration = Duration::from_secs(10);
    /// 消滅を確かめる間隔。
    const POLL_INTERVAL: Duration = Duration::from_millis(50);
}

/// `own_identity` / `run_agent` の適合スイートをアダプターに適用する。
///
/// `$setup` はケースごとに評価され、ハーネスは共有されない。`$allowed_skips` は
/// この環境でスキップを許容するケース(TC ID)の集合。
#[macro_export]
macro_rules! process_controller_identity_conformance {
    ($setup:expr, $allowed_skips:expr) => {
        use $crate::process_controller::identity_and_agent as __pulsen_conformance_identity;

        $crate::conformance_cases!(
            __pulsen_conformance_identity,
            $setup,
            __PULSEN_CONFORMANCE_PROCESS_IDENTITY_SKIPS = $allowed_skips,
            [
                tc_port_process_controller_004_自プロセスの同定情報が取得できる,
                tc_port_process_controller_005_取得機構の失敗は不正な同定情報で装われない,
                tc_port_process_controller_017_成功したエージェントの終了結果はゼロになる,
                tc_port_process_controller_018_非ゼロの終了コードはそのまま返る,
                tc_port_process_controller_019_作業ディレクトリは常にworktreeになる,
                tc_port_process_controller_020_標準出力と標準エラーは指定パスへ書かれる,
                tc_port_process_controller_021_引数はシェルに解釈されずリテラルで渡る,
                tc_port_process_controller_022_存在しないコマンドはコマンド不在に符号化される,
                tc_port_process_controller_023_実行できない実体は実行不能に符号化される,
                tc_port_process_controller_024_exitコードを持たない終了は非ゼロの符号化値になる,
                tc_port_process_controller_025_ログを開けなければエージェントを起動しない,
                tc_port_process_controller_026_作業ディレクトリが無ければ非ゼロで返る,
                tc_port_process_controller_027_呼び出しはエージェントの終了まで戻らない,
            ]
        );
    };
}

/// `spawn_wrapper` の適合スイートをアダプターに適用する。
///
/// ラッパーモード(実バイナリ)の実装を前提にするため、`identity` のスイートとは別に
/// 適用する(ADR-083)。1つのテストファイルに両方を適用できる。
#[macro_export]
macro_rules! process_controller_spawn_conformance {
    ($setup:expr, $allowed_skips:expr) => {
        use $crate::process_controller::spawn as __pulsen_conformance_spawn;

        $crate::conformance_cases!(
            __pulsen_conformance_spawn,
            $setup,
            __PULSEN_CONFORMANCE_PROCESS_SPAWN_SKIPS = $allowed_skips,
            [
                tc_port_process_controller_001_起動は成否を戻り値に持たない,
                tc_port_process_controller_002_ラッパーは呼び出し側の終了後も完走する,
                tc_port_process_controller_003_起動不能は副作用を残さず失敗として返る,
            ]
        );
    };
}

/// `starttime_of` / `kill` / `try_kill_remnants` の適合スイートをアダプターに適用する。
#[macro_export]
macro_rules! process_controller_observation_conformance {
    ($setup:expr, $allowed_skips:expr) => {
        use $crate::process_controller::observation as __pulsen_conformance_observation;

        $crate::conformance_cases!(
            __pulsen_conformance_observation,
            $setup,
            __PULSEN_CONFORMANCE_PROCESS_OBSERVATION_SKIPS = $allowed_skips,
            [
                tc_port_process_controller_006_生存中のプロセスの起動時刻は取得できる,
                tc_port_process_controller_007_終了したプロセスは不在として返る,
                tc_port_process_controller_008_同一プロセスの2回の取得は等価な値を返す,
                tc_port_process_controller_009_記録と照合は同一の取得手段で行われる,
                tc_port_process_controller_010_取得機構の失敗は死亡に写像されない,
                tc_port_process_controller_011_killは実行単位の全プロセスを終了させる,
                tc_port_process_controller_012_killは同定子だけを入力に実行できる,
                tc_port_process_controller_013_終了操作の失敗は値として返る,
                tc_port_process_controller_014_残存プロセスはベストエフォートで終了する,
                tc_port_process_controller_015_同定できなければ何も終了させない,
                tc_port_process_controller_016_残存終了の失敗は値として返る,
            ]
        );
    };
}
