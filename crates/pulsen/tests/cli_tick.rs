//! `tick` の受け入れ検証(実バイナリ・実ファイルシステム・実プロセス)。
//!
//! ポート単位・ユースケース単位で緑でも、合成ルートの結線とクロス tick の引き継ぎが
//! 壊れていれば主経路は動かない。ここではエージェントとして `examples/agent_probe` を
//! 使い、実在するエージェントに依存せずに起動から取り込みまでを通す。
//!
//! ラッパーはデタッチ起動で非同期に完了する。run ディレクトリの内容を読む前と次の tick を
//! 打つ前は `wait_until` で待ち合わせ、**これから観測する成果物そのもの**に条件を立てる。

mod common;

use std::fs;
use std::path::Path;

use common::{
    Home, Repo, Run, Untouched, add, agent_probe, git, judge_probe, probe_config, scratch, tick,
    wait_until,
};
use serde_json::{Value, json};

/// エージェントが 0 で終わる既定のモード(標準出力に入力、標準エラーは空)。
const PRINT_INPUT: [&str; 3] = ["print", "{input}", ""];

/// 滞留したエージェントの解放が来ないときの上限(ミリ秒)。
///
/// 滞留は「次の tick を打つ間ラッパーが生きている」状況を作るためのもので、終わりは
/// テストが置く解放ファイルが決める。この上限は解放し損ねた孫プロセスを残さないための
/// 歯止めであって、生存の窓の長さではない。
const HOLD_LIMIT_MILLIS: &str = "120000";

/// `probe` エージェントとワークフローを備えたホーム。
fn probe_home(mode: &[&str]) -> Home {
    let home = Home::uninitialized();
    let probe = agent_probe().expect("cargo test は examples をビルドする");
    home.write_config(&probe_config(&probe, mode));
    home.write_workflow("implement", common::PROBE_WORKFLOW);
    home
}

/// タスクを1件登録してそのIDを返す。
fn register(home: &Home, repo: &Repo) -> String {
    add("implement", repo.path())
        .home(home.path())
        .run()
        .assert_succeeded();
    home.only_task_id()
}

fn run_tick(home: &Home) -> Run {
    tick().home(home.path()).run()
}

/// attempt の `exit` が現れるまで待つ。
///
/// 滞留したエージェントを残したままにすると、一時ホームの削除と孫プロセスの書き込みが
/// 競合して削除済みのホームが部分的に復活する。
fn wait_for_exit(home: &Home, id: &str, attempt: u32) -> Value {
    let run_dir = home.run_dir(id, attempt);
    wait_until("exit", &run_dir, || run_dir.join("exit").is_file());
    read_json(&run_dir.join("exit"))
}

fn read_json(path: &Path) -> Value {
    let bytes = fs::read(path).expect("ファイルを読める");
    serde_json::from_slice(&bytes).expect("JSON である")
}

fn branch_of(id: &str) -> String {
    format!("pulsen/{id}")
}

/// ラッパー自身を長とする実行単位を指す kill 同定子。
///
/// デタッチせずに起動すると、この値は呼び出し側(cron / シェル)のプロセスグループを
/// 指したまま pid ファイルとタスクファイルに永続化され、終了処理が無関係なプロセス群へ
/// 届く経路が開く。
#[cfg(unix)]
fn own_unit_kill_ident(pid: u64) -> String {
    format!("-{pid}")
}

/// プロセスグループ相当の実行単位を扱う手段が無いため、同定子は pid そのものになる。
#[cfg(not(unix))]
fn own_unit_kill_ident(pid: u64) -> String {
    pid.to_string()
}

#[test]
fn 登録したタスクはworktreeを確保して起動され成果物がrunディレクトリに現れる() {
    let home = probe_home(&PRINT_INPUT);
    let repo = Repo::with_commit();
    let id = register(&home, &repo);

    let run = run_tick(&home);
    run.assert_succeeded();
    assert!(run.stdout.contains(&id), "起動したタスクが表示される");

    assert!(home.worktree(&id).is_dir(), "worktree が作られる");
    let base_tip = git::branch_tip(repo.path(), "main").expect("base の先端を取れる");
    assert_eq!(
        git::branch_tip(repo.path(), &branch_of(&id)).as_deref(),
        Some(base_tip.as_str()),
        "ブランチは base の先端から作られる"
    );

    let task = home.task(&id);
    assert_eq!(task["execution"]["state"], json!("launching"));
    assert!(task["execution"]["recorded_at"].is_string());
    assert_eq!(
        task["current_attempt"]["run_dir"],
        json!(home.run_dir(&id, 1).display().to_string())
    );

    let exit = wait_for_exit(&home, &id, 1);
    assert_eq!(exit, json!({"code": 0}));

    let run_dir = home.run_dir(&id, 1);
    for name in ["starttime", "pid", "stdout.log", "stderr.log"] {
        assert!(run_dir.join(name).is_file(), "{name} が現れる");
    }
    let stdout = fs::read_to_string(run_dir.join("stdout.log")).expect("ログを読める");
    assert_eq!(stdout, "実装して", "スナップショットの入力が展開されている");
}

#[test]
fn 次のtickはpidの出現をもってrunningへ取り込む() {
    let home = probe_home(&PRINT_INPUT);
    let repo = Repo::with_commit();
    let id = register(&home, &repo);

    run_tick(&home).assert_succeeded();
    wait_for_exit(&home, &id, 1);

    run_tick(&home).assert_succeeded();

    let task = home.task(&id);
    assert_eq!(task["execution"]["state"], json!("running"));
    let process = &task["current_attempt"]["process"];
    let pid = process["pid"].as_u64().expect("pid が取り込まれる");
    assert_eq!(
        process["kill_ident"],
        json!(own_unit_kill_ident(pid)),
        "kill 同定子はラッパー自身の実行単位を指す"
    );
    assert!(process["starttime"]["ident"].is_string());
    assert_eq!(task["counters"]["spawn_fail_count"], json!(0));
}

#[test]
fn アーカイブ済みのタスクは走査対象に含まれない() {
    let home = probe_home(&PRINT_INPUT);
    let repo = Repo::with_commit();
    let id = register(&home, &repo);
    home.move_task_to_archive(&id);

    let run = run_tick(&home);

    run.assert_succeeded();
    assert!(run.stdout.contains("処理対象"), "{}", run.stdout);
    assert!(!home.worktree(&id).exists(), "worktree は作られない");
    assert!(
        !home.run_dir(&id, 1).exists(),
        "runディレクトリも作られない"
    );
}

#[test]
fn 滞留するエージェントを起動したままでも次のtickは競合しない() {
    let release_dir = scratch();
    let release = release_dir.path().join("release");
    let release_arg = release.display().to_string();
    let home = probe_home(&["wait-for", &release_arg, HOLD_LIMIT_MILLIS]);
    let repo = Repo::with_commit();
    let id = register(&home, &repo);

    run_tick(&home).assert_succeeded();
    let run_dir = home.run_dir(&id, 1);
    wait_until("pid", &run_dir, || run_dir.join("pid").is_file());

    let run = run_tick(&home);

    // ラッパーが既に終わっていればロックは解放済みで、継承していても競合は起きない。
    // エージェントは解放するまで終わらないので、この主張は環境の速さに左右されない。
    assert!(
        !run_dir.join("exit").is_file(),
        "2回目の tick はラッパーの生存中に走る"
    );
    run.assert_succeeded();
    assert!(
        !run.stdout.contains("スキップしました"),
        "ラッパーはロックを継承しない: {}",
        run.stdout
    );
    assert_eq!(home.task(&id)["execution"]["state"], json!("running"));

    fs::write(&release, "").expect("解放ファイルを置ける");
    assert_eq!(
        wait_for_exit(&home, &id, 1),
        json!({"code": 0}),
        "滞留は解放で終わる(上限で打ち切られてはいない)"
    );
}

#[test]
fn 対象リポジトリの外から起動しても同じ結果になる() {
    let home = probe_home(&PRINT_INPUT);
    let repo = Repo::with_commit();
    let id = register(&home, &repo);
    let elsewhere = scratch();

    tick()
        .home(home.path())
        .cwd(elsewhere.path())
        .run()
        .assert_succeeded();

    assert!(home.worktree(&id).is_dir());
    assert_eq!(home.task(&id)["execution"]["state"], json!("launching"));
    wait_for_exit(&home, &id, 1);
}

#[test]
fn 失敗確定からの再起動は新しいattemptで同じworktreeを使う() {
    let home = probe_home(&PRINT_INPUT);
    let repo = Repo::with_commit();
    let id = register(&home, &repo);

    run_tick(&home).assert_succeeded();
    wait_for_exit(&home, &id, 1);

    // 前回の実行が残した成果を模す。リトライで worktree の内容がリセットされないことは、
    // 同じパスが使われることだけでは主張できない。
    let artifact = home.worktree(&id).join("carried-over.txt");
    fs::write(&artifact, "前回の成果").expect("worktree に書ける");
    home.patch_task(&id, |task| {
        task["execution"] = json!({"state": "failed"});
    });

    run_tick(&home).assert_succeeded();

    let task = home.task(&id);
    assert_eq!(task["current_attempt"]["number"], json!(2));
    assert_eq!(
        task["current_attempt"]["run_dir"],
        json!(home.run_dir(&id, 2).display().to_string())
    );
    assert_eq!(
        fs::read_to_string(&artifact).ok().as_deref(),
        Some("前回の成果"),
        "同一 worktree の内容が引き継がれる"
    );
    wait_for_exit(&home, &id, 2);
}

#[test]
fn ブランチだけが残っている残骸には先端を変えずにworktreeを張り直す() {
    let home = probe_home(&PRINT_INPUT);
    let repo = Repo::with_commit();
    let id = register(&home, &repo);

    let branch = branch_of(&id);
    git::commit_on_new_branch(repo.path(), &branch, "main", "artifact.txt", "積まれた成果")
        .expect("ブランチにコミットを積める");
    let tip = git::branch_tip(repo.path(), &branch).expect("先端を取れる");

    run_tick(&home).assert_succeeded();

    assert_eq!(
        git::branch_tip(repo.path(), &branch).as_deref(),
        Some(tip.as_str()),
        "先端は変わらない"
    );
    assert_eq!(
        fs::read_to_string(home.worktree(&id).join("artifact.txt"))
            .ok()
            .as_deref(),
        Some("積まれた成果"),
        "積まれたコミットの成果物が worktree に戻る"
    );
    wait_for_exit(&home, &id, 1);
}

#[test]
fn 進行中のworktree消失はエージェント実行の失敗として表面化する() {
    let home = probe_home(&PRINT_INPUT);
    let repo = Repo::with_commit();
    let id = register(&home, &repo);

    run_tick(&home).assert_succeeded();
    wait_for_exit(&home, &id, 1);

    fs::remove_dir_all(home.worktree(&id)).expect("worktree を消せる");
    home.patch_task(&id, |task| {
        task["execution"] = json!({"state": "failed"});
    });

    run_tick(&home).assert_succeeded();

    let exit = wait_for_exit(&home, &id, 2);
    assert_ne!(exit, json!({"code": 0}), "非0で符号化される: {exit}");
}

#[test]
fn エージェント定義を壊すと起動できず直せば次のtickで起動する() {
    let home = probe_home(&PRINT_INPUT);
    let repo = Repo::with_commit();
    let id = register(&home, &repo);
    home.write_config("agents:\n  other:\n    cmd: other {input}\n");

    run_tick(&home).assert_succeeded();

    let task = home.task(&id);
    assert_eq!(
        task["execution"]["state"],
        json!("pending"),
        "実行状態は不変"
    );
    assert_eq!(task["task_status"], json!("queued"));
    assert_eq!(task["counters"]["spawn_fail_count"], json!(1));
    assert!(
        !home.run_dir(&id, 1).exists(),
        "attempt は採番されず runディレクトリも作られない"
    );

    let probe = agent_probe().expect("cargo test は examples をビルドする");
    home.write_config(&probe_config(&probe, &PRINT_INPUT));

    run_tick(&home).assert_succeeded();

    assert_eq!(home.task(&id)["execution"]["state"], json!("launching"));
    wait_for_exit(&home, &id, 1);
}

#[test]
fn 状態の置き場が未作成でも処理対象なしとして終わる() {
    let home = probe_home(&PRINT_INPUT);

    let run = run_tick(&home);

    run.assert_succeeded();
    assert!(run.stdout.contains("処理対象"), "{}", run.stdout);
    assert!(!home.state_dir().join("tasks").exists());
}

#[test]
fn パース不能なタスクファイルが混ざっても残りは起動され0で終わる() {
    let home = probe_home(&PRINT_INPUT);
    let repo = Repo::with_commit();
    let id = register(&home, &repo);
    let broken = home.write_raw_task("20260812t000000-brokentask", "{ これは JSON ではない");
    let untouched = fs::read(&broken).expect("読める");

    let run = run_tick(&home);

    run.assert_succeeded();
    assert_eq!(
        home.task(&id)["execution"]["state"],
        json!("launching"),
        "残りのタスクは処理される"
    );
    assert_eq!(
        fs::read(&broken).expect("読める"),
        untouched,
        "破損したファイルには書き込まない"
    );
    wait_for_exit(&home, &id, 1);
}

#[test]
fn スナップショットだけが壊れたタスクは報告されて書き込まれない() {
    let home = probe_home(&PRINT_INPUT);
    let repo = Repo::with_commit();
    let id = register(&home, &repo);
    home.patch_task(&id, |task| {
        task["snapshot"] = json!({});
    });
    let untouched = fs::read(home.task_path(&id)).expect("読める");

    let run = run_tick(&home);

    run.assert_succeeded();
    assert!(run.stdout.contains(&id), "報告される: {}", run.stdout);
    assert_eq!(
        fs::read(home.task_path(&id)).expect("読める"),
        untouched,
        "書き込まない"
    );
    assert!(!home.run_dir(&id, 1).exists());
    assert!(!home.worktree(&id).exists());
}

#[test]
fn グローバル設定が不在なら非0で終わり状態を変えない() {
    let home = probe_home(&PRINT_INPUT);
    let repo = Repo::with_commit();
    let id = register(&home, &repo);
    home.remove_config();
    let untouched = Untouched::of([home.task_path(&id)]);

    let run = run_tick(&home);

    run.assert_rejected().assert_reports(&["未初期化"]);
    untouched.assert_unchanged();
    assert!(!home.worktree(&id).exists(), "worktree は作られない");
    assert!(
        !home.run_dir(&id, 1).exists(),
        "runディレクトリも作られない"
    );
}

#[test]
fn グローバル設定がパース不能なら非0で終わり状態を変えない() {
    let home = probe_home(&PRINT_INPUT);
    let repo = Repo::with_commit();
    let id = register(&home, &repo);
    home.write_config("agents: [\n");
    let untouched = Untouched::of([home.task_path(&id)]);

    let run = run_tick(&home);

    run.assert_rejected().assert_reports(&["解釈できません"]);
    untouched.assert_unchanged();
    assert!(!home.worktree(&id).exists(), "worktree は作られない");
    assert!(
        !home.run_dir(&id, 1).exists(),
        "runディレクトリも作られない"
    );
}

#[test]
fn 別の操作がロックを保持していればスキップして0で終わる() {
    let home = probe_home(&PRINT_INPUT);
    let repo = Repo::with_commit();
    let id = register(&home, &repo);

    let holder = common::lock::hold(&home.lock_path()).expect("ロックを別プロセスに保持させられる");

    let run = run_tick(&home);

    run.assert_succeeded();
    assert!(run.stdout.contains("スキップ"), "{}", run.stdout);
    assert_eq!(home.task(&id)["execution"]["state"], json!("pending"));
    assert!(!home.worktree(&id).exists());

    common::lock::release(holder).expect("保持プロセスを終了できる");
}

#[test]
fn 同一リポジトリの複数タスクは別々のworktreeとブランチで起動される() {
    let home = probe_home(&PRINT_INPUT);
    let repo = Repo::with_commit();
    register(&home, &repo);
    add("implement", repo.path())
        .home(home.path())
        .run()
        .assert_succeeded();

    let ids: Vec<String> = home
        .tasks()
        .iter()
        .map(|task| task["task_id"].as_str().expect("文字列").to_owned())
        .collect();
    assert_eq!(ids.len(), 2);

    run_tick(&home).assert_succeeded();

    let base_tip = git::branch_tip(repo.path(), "main").expect("base の先端を取れる");
    for id in &ids {
        assert!(home.worktree(id).is_dir(), "{id}: worktree");
        assert_eq!(
            git::branch_tip(repo.path(), &branch_of(id)).as_deref(),
            Some(base_tip.as_str()),
            "{id}: ブランチは base の先端から作られる"
        );
        assert_eq!(home.task(id)["execution"]["state"], json!("launching"));
    }
    assert_ne!(home.worktree(&ids[0]), home.worktree(&ids[1]));
    for id in &ids {
        wait_for_exit(&home, id, 1);
    }
}

// --- 判定・遷移・凍結・通知の一周(Issue #3) ---

/// YAML のダブルクォート表記(パスやプレースホルダをそのまま1トークンにする)。
fn yaml_token(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

/// キーの下にトークン配列を並べる YAML の断片。
fn token_list(indent: &str, key: &str, tokens: &[String]) -> String {
    let mut text = format!("{indent}{key}:\n");
    for token in tokens {
        text.push_str(&format!("{indent}  - {}\n", yaml_token(token)));
    }
    text
}

/// 判定・通知に使うプローブのトークン列。
fn probe_tokens(program: &Path, args: &[&str]) -> Vec<String> {
    let mut tokens = vec![program.display().to_string()];
    tokens.extend(args.iter().map(|arg| (*arg).to_owned()));
    tokens
}

/// `queued` に追記した定義を持つ `probe` ワークフロー。
fn workflow_with(extra: &str) -> String {
    format!(
        "agent: probe\ninitial: queued\nstatuses:\n  queued:\n    prompt: 実装して\n    next: done\n{extra}  done:\n    run: cleanup\n"
    )
}

/// 制御ファイルの終了コードで結末が決まるエージェントを持つホーム。
fn controlled_home(control: &Path, workflow: &str, config_extra: &str) -> Home {
    let home = Home::uninitialized();
    let probe = agent_probe().expect("cargo test は examples をビルドする");
    let mut config = probe_config(&probe, &["exit-from", &control.display().to_string()]);
    config.push_str(config_extra);
    home.write_config(&config);
    home.write_workflow("implement", workflow);
    home
}

/// 終了コードを制御ファイルに書く。
fn set_control(path: &Path, code: i32) {
    fs::write(path, code.to_string()).expect("制御ファイルを書ける");
}

/// 証跡ファイルの行(不在なら空)。
fn logged_lines(path: &Path) -> Vec<String> {
    fs::read_to_string(path)
        .map(|text| text.lines().map(str::to_owned).collect())
        .unwrap_or_default()
}

/// 起動 → 完走 → running への取り込みまで進める。
///
/// 1タスク1tick1ステップなので、判定に入るのはこの後の tick になる。
fn launch_and_confirm(home: &Home, id: &str, attempt: u32) {
    run_tick(home).assert_succeeded();
    wait_for_exit(home, id, attempt);
    run_tick(home).assert_succeeded();
    assert_eq!(
        home.task(id)["execution"]["state"],
        json!("running"),
        "起動を確認して running へ取り込む"
    );
}

#[test]
fn 終了コード0の観測は判定確定になり次のtickが次のステータスへ進める() {
    let scratch = scratch();
    let control = scratch.path().join("agent-exit");
    let home = controlled_home(&control, &workflow_with(""), "");
    let repo = Repo::with_commit();
    let id = register(&home, &repo);
    set_control(&control, 0);

    launch_and_confirm(&home, &id, 1);
    run_tick(&home).assert_succeeded();
    let judged = home.task(&id);
    assert_eq!(judged["execution"]["state"], json!("completed"));
    assert_eq!(judged["task_status"], json!("queued"), "遷移は次の tick");

    let advanced_run = run_tick(&home);

    advanced_run.assert_succeeded();
    let advanced = home.task(&id);
    assert_eq!(advanced["task_status"], json!("done"));
    assert_eq!(advanced["execution"]["state"], json!("pending"));
    assert_eq!(advanced["counters"]["attempt_count"], json!(0));
    assert_eq!(advanced["counters"]["judge_attempt_count"], json!(0));
}

#[test]
fn 一過性の失敗は自動リトライで回復しカウンタが0に戻る() {
    let scratch = scratch();
    let control = scratch.path().join("agent-exit");
    let home = controlled_home(&control, &workflow_with(""), "");
    let repo = Repo::with_commit();
    let id = register(&home, &repo);
    set_control(&control, 3);

    launch_and_confirm(&home, &id, 1);
    run_tick(&home).assert_succeeded();
    let failed = home.task(&id);
    assert_eq!(failed["execution"]["state"], json!("failed"));
    assert_eq!(failed["counters"]["attempt_count"], json!(1));

    set_control(&control, 0);
    launch_and_confirm(&home, &id, 2);
    assert_eq!(
        home.task(&id)["current_attempt"]["number"],
        json!(2),
        "新しい attempt 番号で再起動される"
    );
    run_tick(&home).assert_succeeded();

    let recovered = home.task(&id);
    assert_eq!(recovered["execution"]["state"], json!("completed"));
    assert_eq!(recovered["counters"]["attempt_count"], json!(0));
}

#[test]
fn 判定コマンドは文脈を受け取り3分岐をそれぞれの結末に導く() {
    for (judge_exit, expected_state, expected_status) in [
        (0, "completed", "queued"),
        (10, "failed", "queued"),
        (20, "pending", "queued"),
    ] {
        let scratch = scratch();
        let control = scratch.path().join("agent-exit");
        let judge_log = scratch.path().join("judge.log");
        let judge = judge_probe().expect("cargo test は examples をビルドする");
        let workflow = workflow_with(&token_list(
            "    ",
            "judge",
            &probe_tokens(
                &judge,
                &[
                    "log",
                    &judge_log.display().to_string(),
                    "TASK_ID",
                    "WORKSPACE",
                    "EXIT_CODE",
                    "RUN_DIR",
                ],
            ),
        ));
        let home = controlled_home(&control, &workflow, "");
        let repo = Repo::with_commit();
        let id = register(&home, &repo);
        set_control(&control, 0);
        set_control(&judge_log.with_extension("log.exit"), judge_exit);

        launch_and_confirm(&home, &id, 1);
        run_tick(&home).assert_succeeded();

        let task = home.task(&id);
        assert_eq!(
            task["execution"]["state"],
            json!(expected_state),
            "exit {judge_exit}"
        );
        assert_eq!(
            task["task_status"],
            json!(expected_status),
            "exit {judge_exit}"
        );
        let logged = logged_lines(&judge_log);
        assert_eq!(logged.len(), 1, "判定は1回だけ実行される");
        let line = &logged[0];
        assert!(line.contains(&format!("TASK_ID={id}")), "{line}");
        assert!(
            line.contains(&format!("WORKSPACE={}", home.worktree(&id).display())),
            "{line}"
        );
        assert!(line.contains("EXIT_CODE=0"), "{line}");
        assert!(
            line.contains(&format!("RUN_DIR={}", home.run_dir(&id, 1).display())),
            "{line}"
        );
    }
}

#[test]
fn リトライ上限の超過は凍結を保存して同じtickで通知する() {
    let scratch = scratch();
    let control = scratch.path().join("agent-exit");
    let notify_log = scratch.path().join("notify.log");
    let judge = judge_probe().expect("cargo test は examples をビルドする");
    let notify = token_list(
        "",
        "notify_cmd",
        &probe_tokens(
            &judge,
            &[
                "log",
                &notify_log.display().to_string(),
                "TASK_ID",
                "WORKFLOW",
                "TASK_STATUS",
            ],
        ),
    );
    let home = controlled_home(&control, &workflow_with("    retries: 0\n"), &notify);
    let repo = Repo::with_commit();
    let id = register(&home, &repo);
    set_control(&control, 1);

    launch_and_confirm(&home, &id, 1);
    let run = run_tick(&home);

    run.assert_succeeded();
    let task = home.task(&id);
    assert_eq!(task["execution"]["state"], json!("stopped"));
    assert_eq!(task["execution"]["reason"], json!("retry_limit_exceeded"));
    assert!(
        task["execution"]["notified_at"].is_string(),
        "同じ tick で通知まで済む: {task}"
    );
    let logged = logged_lines(&notify_log);
    assert_eq!(logged.len(), 1, "ちょうど1行だけ通知される");
    assert!(
        logged[0].contains(&format!("TASK_ID={id}")),
        "{}",
        logged[0]
    );
    assert!(logged[0].contains("WORKFLOW=implement"), "{}", logged[0]);
    assert!(logged[0].contains("TASK_STATUS=queued"), "{}", logged[0]);

    run_tick(&home).assert_succeeded();
    assert_eq!(
        logged_lines(&notify_log).len(),
        1,
        "通知済みの凍結は再通知されない"
    );
}

#[test]
fn 通知に失敗した凍結は次のtickで再通知される() {
    let scratch = scratch();
    let control = scratch.path().join("agent-exit");
    let notify_log = scratch.path().join("notify.log");
    let judge = judge_probe().expect("cargo test は examples をビルドする");
    let notify = token_list(
        "",
        "notify_cmd",
        &probe_tokens(
            &judge,
            &["log", &notify_log.display().to_string(), "TASK_ID"],
        ),
    );
    let home = controlled_home(&control, &workflow_with("    retries: 0\n"), &notify);
    let repo = Repo::with_commit();
    let id = register(&home, &repo);
    set_control(&control, 1);
    set_control(&notify_log.with_extension("log.exit"), 1);

    launch_and_confirm(&home, &id, 1);
    run_tick(&home).assert_succeeded();

    let frozen = home.task(&id);
    assert_eq!(frozen["execution"]["state"], json!("stopped"));
    assert_eq!(
        frozen["execution"]["notified_at"],
        Value::Null,
        "通知が失敗したので通知時刻は残らない"
    );
    assert_eq!(logged_lines(&notify_log).len(), 1, "通知自体は試みられる");

    set_control(&notify_log.with_extension("log.exit"), 0);
    run_tick(&home).assert_succeeded();

    assert!(
        home.task(&id)["execution"]["notified_at"].is_string(),
        "次の tick が再通知する"
    );
    assert_eq!(logged_lines(&notify_log).len(), 2);

    run_tick(&home).assert_succeeded();
    assert_eq!(
        logged_lines(&notify_log).len(),
        2,
        "さらに次の tick では増えない"
    );
}

#[test]
fn 通知コマンドが未定義の凍結は後から定義した次のtickで通知される() {
    let scratch = scratch();
    let control = scratch.path().join("agent-exit");
    let notify_log = scratch.path().join("notify.log");
    let home = controlled_home(&control, &workflow_with("    retries: 0\n"), "");
    let repo = Repo::with_commit();
    let id = register(&home, &repo);
    set_control(&control, 1);

    launch_and_confirm(&home, &id, 1);
    run_tick(&home).assert_succeeded();

    let frozen = home.task(&id);
    assert_eq!(frozen["execution"]["state"], json!("stopped"));
    assert_eq!(
        frozen["execution"]["notified_at"],
        Value::Null,
        "通知した虚偽の記録を作らない"
    );

    let probe = agent_probe().expect("cargo test は examples をビルドする");
    let judge = judge_probe().expect("cargo test は examples をビルドする");
    let mut config = probe_config(&probe, &["exit-from", &control.display().to_string()]);
    config.push_str(&token_list(
        "",
        "notify_cmd",
        &probe_tokens(
            &judge,
            &["log", &notify_log.display().to_string(), "TASK_ID"],
        ),
    ));
    home.write_config(&config);

    run_tick(&home).assert_succeeded();

    assert!(home.task(&id)["execution"]["notified_at"].is_string());
    assert_eq!(logged_lines(&notify_log).len(), 1);
}

#[test]
fn スナップショットだけが壊れた未通知の凍結にも再通知が行われる() {
    let scratch = scratch();
    let control = scratch.path().join("agent-exit");
    let notify_log = scratch.path().join("notify.log");
    let judge = judge_probe().expect("cargo test は examples をビルドする");
    let notify = token_list(
        "",
        "notify_cmd",
        &probe_tokens(
            &judge,
            &["log", &notify_log.display().to_string(), "TASK_ID"],
        ),
    );
    let home = controlled_home(&control, &workflow_with("    retries: 0\n"), &notify);
    let repo = Repo::with_commit();
    let id = register(&home, &repo);
    set_control(&control, 1);
    set_control(&notify_log.with_extension("log.exit"), 1);

    launch_and_confirm(&home, &id, 1);
    run_tick(&home).assert_succeeded();
    assert_eq!(home.task(&id)["execution"]["notified_at"], Value::Null);

    home.patch_task(&id, |task| {
        task["snapshot"]["statuses"] = json!({});
    });
    set_control(&notify_log.with_extension("log.exit"), 0);

    run_tick(&home).assert_succeeded();

    let task = home.task(&id);
    assert!(
        task["execution"]["notified_at"].is_string(),
        "スナップショット破損を理由にスキップしない: {task}"
    );
    assert_eq!(
        task["snapshot"]["statuses"],
        json!({}),
        "読めないスナップショットは元の内容のまま温存される"
    );
    assert_eq!(logged_lines(&notify_log).len(), 2);
}

#[test]
fn timeoutを超えた実行は終了させられて失敗確定になる() {
    let release_dir = scratch();
    let release = release_dir.path().join("release");
    let release_arg = release.display().to_string();
    let home = Home::uninitialized();
    let probe = agent_probe().expect("cargo test は examples をビルドする");
    home.write_config(&probe_config(
        &probe,
        &["wait-for", &release_arg, HOLD_LIMIT_MILLIS],
    ));
    home.write_workflow("implement", &workflow_with("    timeout: 1s\n"));
    let repo = Repo::with_commit();
    let id = register(&home, &repo);

    run_tick(&home).assert_succeeded();
    let run_dir = home.run_dir(&id, 1);
    wait_until("pid", &run_dir, || run_dir.join("pid").is_file());
    run_tick(&home).assert_succeeded();
    assert_eq!(home.task(&id)["execution"]["state"], json!("running"));

    // timeout の起点は記録済み starttime の壁時計成分。1秒の超過を実時間で待つ。
    std::thread::sleep(std::time::Duration::from_millis(2100));
    run_tick(&home).assert_succeeded();

    assert_eq!(home.task(&id)["execution"]["state"], json!("failed"));
    assert_eq!(home.task(&id)["counters"]["attempt_count"], json!(1));
    assert!(
        !run_dir.join("exit").is_file(),
        "実行単位ごと終了させたので、ラッパーは exit を書かずに終わる"
    );
    // 解放を置くのは、終了させ損ねたプロセスを一時ホームの削除まで残さないための保険。
    fs::write(&release, "").expect("解放ファイルを置ける");
}

#[test]
fn プロトコル外の判定はエージェントを再実行せず判定上限の超過で凍結する() {
    let scratch = scratch();
    let control = scratch.path().join("agent-exit");
    let judge_log = scratch.path().join("judge.log");
    let judge = judge_probe().expect("cargo test は examples をビルドする");
    let workflow = workflow_with(&token_list(
        "    ",
        "judge",
        &probe_tokens(
            &judge,
            &["log", &judge_log.display().to_string(), "TASK_ID"],
        ),
    ));
    let home = controlled_home(&control, &workflow, "judge_attempt_limit: 1\n");
    let repo = Repo::with_commit();
    let id = register(&home, &repo);
    set_control(&control, 0);
    set_control(&judge_log.with_extension("log.exit"), 7);

    launch_and_confirm(&home, &id, 1);
    run_tick(&home).assert_succeeded();

    let judged = home.task(&id);
    assert_eq!(judged["execution"]["state"], json!("running"));
    assert_eq!(judged["counters"]["judge_attempt_count"], json!(1));

    run_tick(&home).assert_succeeded();

    let frozen = home.task(&id);
    assert_eq!(frozen["execution"]["state"], json!("stopped"));
    assert_eq!(frozen["execution"]["reason"], json!("judge_limit_exceeded"));
    assert_eq!(
        frozen["counters"]["attempt_count"],
        json!(0),
        "エージェントは再実行されていない"
    );
    assert!(
        !home.run_dir(&id, 2).exists(),
        "run ディレクトリは attempt-1 だけ"
    );
    assert_eq!(logged_lines(&judge_log).len(), 2, "再判定は行われる");
}

#[test]
fn 判定と遷移と凍結と通知はサマリーに現れる() {
    let scratch = scratch();
    let control = scratch.path().join("agent-exit");
    let notify_log = scratch.path().join("notify.log");
    let judge = judge_probe().expect("cargo test は examples をビルドする");
    let notify = token_list(
        "",
        "notify_cmd",
        &probe_tokens(
            &judge,
            &["log", &notify_log.display().to_string(), "TASK_ID"],
        ),
    );
    let home = controlled_home(&control, &workflow_with(""), &notify);
    let repo = Repo::with_commit();
    let id = register(&home, &repo);
    set_control(&control, 0);

    launch_and_confirm(&home, &id, 1);
    let judged = run_tick(&home);
    judged.assert_succeeded();
    assert!(judged.stdout.contains("判定確定"), "{}", judged.stdout);
    assert!(judged.stdout.contains(&id), "{}", judged.stdout);

    let advanced = run_tick(&home);
    advanced.assert_succeeded();
    assert!(advanced.stdout.contains("遷移"), "{}", advanced.stdout);

    // 凍結と通知は帳簿を直に置いて確かめる。同じ tick の凍結からの通知は別のケースが見る。
    home.patch_task(&id, |task| {
        task["execution"] = json!({"state": "stopped", "reason": "retry_limit_exceeded"});
    });
    let notified = run_tick(&home);

    notified.assert_succeeded();
    assert!(notified.stdout.contains("通知"), "{}", notified.stdout);
    assert!(notified.stdout.contains(&id), "{}", notified.stdout);
}

#[test]
fn 観測の失敗は対象と原因が読み取れる形で報告される() {
    let home = probe_home(&PRINT_INPUT);
    let repo = Repo::with_commit();
    let id = register(&home, &repo);

    run_tick(&home).assert_succeeded();
    wait_for_exit(&home, &id, 1);
    run_tick(&home).assert_succeeded();

    // 起動確認済みなのに同定情報が無い状態(手動修復による不変条件3の破れ)を作る。
    home.patch_task(&id, |task| {
        task["current_attempt"]["process"] = Value::Null;
    });
    let untouched = Untouched::of([home.task_path(&id)]);

    let run = run_tick(&home);

    run.assert_succeeded();
    assert!(run.stdout.contains(&id), "{}", run.stdout);
    assert!(run.stdout.contains("同定情報"), "{}", run.stdout);
    untouched.assert_unchanged();
}
