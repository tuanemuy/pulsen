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
    Home, Repo, Run, Untouched, add, agent_probe, git, probe_config, run_cli, scratch, tick,
    wait_until,
};
use serde_json::{Value, json};

/// エージェントが 0 で終わる既定のモード(標準出力に入力、標準エラーは空)。
const PRINT_INPUT: [&str; 3] = ["print", "{input}", ""];

/// 起動されたエージェントを滞留させるモード。
const SLEEP: [&str; 2] = ["sleep", "800"];

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
    assert!(
        git::branch_tip(repo.path(), &branch_of(&id)).is_some(),
        "base から作られたブランチが存在する"
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
    let home = probe_home(&SLEEP);
    let repo = Repo::with_commit();
    let id = register(&home, &repo);

    run_tick(&home).assert_succeeded();
    let run_dir = home.run_dir(&id, 1);
    wait_until("pid", &run_dir, || run_dir.join("pid").is_file());

    let run = run_tick(&home);

    run.assert_succeeded();
    assert!(
        !run.stdout.contains("スキップしました"),
        "ラッパーはロックを継承しない: {}",
        run.stdout
    );
    assert_eq!(home.task(&id)["execution"]["state"], json!("running"));

    wait_for_exit(&home, &id, 1);
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

    let Some(holder) = common::lock::hold(&home.lock_path()) else {
        common::skipped("tc_exec_tick_015", "lock::hold");
        return;
    };

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

    for id in &ids {
        assert!(home.worktree(id).is_dir(), "{id}: worktree");
        assert!(
            git::branch_tip(repo.path(), &branch_of(id)).is_some(),
            "{id}: ブランチ"
        );
        assert_eq!(home.task(id)["execution"]["state"], json!("launching"));
    }
    assert_ne!(home.worktree(&ids[0]), home.worktree(&ids[1]));
    for id in &ids {
        wait_for_exit(&home, id, 1);
    }
}

#[test]
fn tickはヘルプに現れる利用者向けのコマンドである() {
    let run = run_cli(&["tick", "--help"]);

    run.assert_succeeded();
    assert!(!run.stdout.is_empty());
}
