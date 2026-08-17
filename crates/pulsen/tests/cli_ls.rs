//! `ls` の受け入れ検証(実バイナリ・実ファイルシステム)。
//!
//! 台帳の PAGE 行が主張するのは**表示**であり、綴りまで含めた確認は実バイナリの出力で
//! しかできない。ロックを取らないことも、別プロセスが保持したままで結果が返ることとして
//! 外から確かめる。

mod common;

use std::fs;
use std::path::Path;

use common::{Home, Repo, Run, WORKFLOW, add, agent_probe, ls, probe_config, tick, wait_until};
use serde_json::json;

/// タスクステータスと実行状態が同名になり得るワークフロー。
const AMBIGUOUS_WORKFLOW: &str = "\
agent: claude
initial: running
statuses:
  running:
    prompt: 実装して
    next: done
  done:
    run: cleanup
";

/// エージェントが 0 で終わる既定のモード(標準出力に入力、標準エラーは空)。
const PRINT_INPUT: [&str; 3] = ["print", "{input}", ""];

/// tick と同時に読み取る周回で回す tick の回数。
const TICK_ROUNDS: usize = 3;

/// 有効な設定と `implement` ワークフローを備えたホーム。
fn home_with_workflow() -> Home {
    let home = Home::new();
    home.write_workflow("implement", WORKFLOW);
    home
}

/// タスクを1件登録してそのIDを返す。
fn register(home: &Home, workflow: &str, repo: &Repo) -> String {
    let before: Vec<String> = task_ids(home);
    add(workflow, repo.path())
        .home(home.path())
        .run()
        .assert_succeeded();
    let mut added: Vec<String> = task_ids(home)
        .into_iter()
        .filter(|id| !before.contains(id))
        .collect();
    assert_eq!(added.len(), 1, "登録されたタスクはちょうど1件");
    added.remove(0)
}

fn task_ids(home: &Home) -> Vec<String> {
    home.tasks()
        .iter()
        .map(|task| task["task_id"].as_str().expect("文字列").to_owned())
        .collect()
}

fn run_ls(home: &Home) -> Run {
    ls().home(home.path()).run()
}

/// 一覧のうち指定タスクの行。
fn row<'a>(run: &'a Run, id: &str) -> &'a str {
    run.stdout
        .lines()
        .find(|line| line.contains(id))
        .unwrap_or_else(|| panic!("{id} の行がある: {}", run.stdout))
}

#[test]
fn tc_task_list_tasks_001_一覧は各タスクの表示項目を並べて0で終わる() {
    let home = home_with_workflow();
    home.write_workflow("review", WORKFLOW);
    let repo = Repo::with_commit();
    let queued = register(&home, "implement", &repo);
    let running = register(&home, "review", &repo);
    home.patch_task(&running, |task| {
        task["task_status"] = json!("done");
        task["execution"] = json!({ "state": "running" });
        task["counters"]["attempt_count"] = json!(2);
        task["workspace"] = json!({
            "path": home.worktree(&running),
            "branch": format!("pulsen/{running}"),
        });
    });

    let run = run_ls(&home);

    run.assert_succeeded();
    let repo_path = repo.path().display().to_string();
    assert!(row(&run, &queued).contains("implement"), "{}", run.stdout);
    assert!(row(&run, &queued).contains("queued"), "{}", run.stdout);
    assert!(row(&run, &queued).contains("pending"), "{}", run.stdout);
    assert!(row(&run, &queued).contains(&repo_path), "{}", run.stdout);

    let running_row = row(&run, &running);
    assert!(running_row.contains("review"), "{running_row}");
    assert!(running_row.contains("done"), "{running_row}");
    assert!(running_row.contains("running"), "{running_row}");
    assert!(
        running_row.contains(&format!("pulsen/{running}")),
        "ブランチが並ぶ: {running_row}"
    );
    let attempt = home.task(&running)["counters"]["attempt_count"].to_string();
    assert!(
        running_row.split_whitespace().any(|cell| cell == attempt),
        "attempt_count が並ぶ: {running_row}"
    );
    let updated = home.task(&running)["updated_at"]
        .as_str()
        .expect("文字列")
        .to_owned();
    assert!(
        running_row.contains(&updated),
        "更新日時が並ぶ: {running_row}"
    );
}

#[test]
fn tc_task_list_tasks_002_タスクステータスと実行状態は同名でも併記される() {
    let home = Home::new();
    home.write_workflow("ambiguous", AMBIGUOUS_WORKFLOW);
    let repo = Repo::with_commit();
    let id = register(&home, "ambiguous", &repo);
    home.patch_task(&id, |task| {
        task["execution"] = json!({ "state": "running" });
    });

    let run = run_ls(&home);

    run.assert_succeeded();
    let row = row(&run, &id);
    assert_eq!(
        row.matches("running").count(),
        2,
        "タスクステータスと実行状態の両方が出る: {row}"
    );
}

#[test]
fn 絞り込みは論理積で効き未知のタスクステータスは該当なしで0になる() {
    let home = home_with_workflow();
    let repo = Repo::with_commit();
    let queued = register(&home, "implement", &repo);
    let done = register(&home, "implement", &repo);
    home.patch_task(&done, |task| {
        task["task_status"] = json!("done");
        task["execution"] = json!({ "state": "running" });
    });

    let by_status = ls().home(home.path()).status("done").run();
    let by_state = ls().home(home.path()).state("pending").run();
    let both = ls().home(home.path()).status("done").state("pending").run();
    let unknown = ls().home(home.path()).status("存在しない名前").run();

    by_status.assert_succeeded().assert_shows(&[done.as_str()]);
    assert!(!by_status.stdout.contains(&queued), "{}", by_status.stdout);

    by_state.assert_succeeded().assert_shows(&[queued.as_str()]);
    assert!(!by_state.stdout.contains(&done), "{}", by_state.stdout);

    both.assert_succeeded()
        .assert_shows(&["該当するタスクはありません"]);

    unknown
        .assert_succeeded()
        .assert_shows(&["該当するタスクはありません"]);
}

#[test]
fn tc_task_list_tasks_009_タスクが1件もなければ空である旨を表示して0で終わる() {
    let home = home_with_workflow();
    fs::create_dir_all(home.tasks_dir()).expect("置き場を作れる");

    let run = run_ls(&home);

    run.assert_succeeded()
        .assert_shows(&["該当するタスクはありません"]);
}

#[test]
fn tc_task_list_tasks_011_設定が読めなければ非0で終わる() {
    let home = Home::uninitialized();

    let run = run_ls(&home);

    run.assert_rejected()
        .assert_reports(&["グローバルホームが未初期化です"]);
}

#[test]
fn tc_task_list_tasks_019_読めないタスクファイルが混ざっても一覧は失敗しない() {
    let home = home_with_workflow();
    let repo = Repo::with_commit();
    let id = register(&home, "implement", &repo);
    let broken = home.write_raw_task("20260812t090000-broken01", "{ これは JSON ではない");

    let run = run_ls(&home);

    run.assert_succeeded();
    assert!(
        run.stdout.contains(&id),
        "残りのタスクは表示される: {}",
        run.stdout
    );
    assert!(
        run.stdout.contains(&broken.display().to_string()),
        "修復の入口としてファイルパスが出る: {}",
        run.stdout
    );
}

#[test]
fn スナップショットだけが読めないタスクは印つきの行として現れ破損報告には出ない() {
    let home = home_with_workflow();
    let repo = Repo::with_commit();
    let id = register(&home, "implement", &repo);
    home.patch_task(&id, |task| {
        task["snapshot"] = json!({ "initial": "queued" });
    });

    let run = run_ls(&home);
    let filtered = ls().home(home.path()).state("pending").run();

    run.assert_succeeded();
    let listed = row(&run, &id);
    assert!(listed.contains("スナップショット読み取り不能"), "{listed}");
    assert!(
        !run.stdout.contains("読み取れなかったタスクファイル"),
        "タスクファイル自体は読めており破損の報告には寄せない: {}",
        run.stdout
    );

    filtered.assert_succeeded();
    assert!(
        row(&filtered, &id).contains("スナップショット読み取り不能"),
        "実行状態は読めているので絞り込みにも乗る: {}",
        filtered.stdout
    );
}

#[test]
fn tc_task_list_tasks_022_タスクの置き場が無くても空の一覧として0で終わる() {
    let home = home_with_workflow();
    assert!(!home.tasks_dir().exists(), "置き場はまだ作られていない");

    let run = run_ls(&home);

    run.assert_succeeded()
        .assert_shows(&["該当するタスクはありません"]);
    assert!(!home.tasks_dir().exists(), "読み取りは置き場を作らない");
}

#[test]
fn tc_task_list_tasks_023_アーカイブの置き場が無くても全件指定は現役を表示する() {
    let home = home_with_workflow();
    let repo = Repo::with_commit();
    let id = register(&home, "implement", &repo);
    assert!(!home.archive_dir().exists(), "アーカイブの置き場はまだ無い");

    let run = ls().home(home.path()).all().run();

    run.assert_succeeded().assert_shows(&[id.as_str()]);
}

#[test]
fn tc_task_list_tasks_025_別の操作がロックを保持していても一覧は返る() {
    let home = home_with_workflow();
    let repo = Repo::with_commit();
    let id = register(&home, "implement", &repo);

    let Some(holder) = common::lock::hold(&home.lock_path()) else {
        common::skipped("tc_task_list_tasks_025", "lock::hold");
        return;
    };

    let run = run_ls(&home);
    common::lock::release(holder).expect("保持プロセスを終了できる");

    run.assert_succeeded().assert_shows(&[id.as_str()]);
}

#[test]
fn tc_task_list_tasks_026_tickの更新と同時に読んでも書きかけを観測しない() {
    let home = probe_home();
    let repo = Repo::with_commit();
    let id = register(&home, "implement", &repo);

    let ticking = std::thread::spawn({
        let path = home.path().to_path_buf();
        move || {
            for _ in 0..TICK_ROUNDS {
                tick().home(&path).run();
            }
        }
    });

    let mut observations = 0;
    while !ticking.is_finished() {
        let run = run_ls(&home);
        run.assert_succeeded();
        assert!(
            !run.stdout.contains("読み取れなかった"),
            "アトミック置換なので書きかけは観測されない: {}",
            run.stdout
        );
        assert!(run.stdout.contains(&id), "{}", run.stdout);
        observations += 1;
    }
    ticking.join().expect("tick を回し切れる");

    assert!(observations > 0, "tick と重なる読み取りが1回は起きる");
    settle(&home, &id);
}

#[test]
fn アーカイブ済みのタスクは全件指定でのみ印つきで現れる() {
    let home = home_with_workflow();
    let repo = Repo::with_commit();
    let active = register(&home, "implement", &repo);
    let archived = register(&home, "implement", &repo);
    home.patch_task(&archived, |task| {
        task["workspace"] = json!({
            "path": home.worktree(&archived),
            "branch": format!("pulsen/{archived}"),
        });
    });
    home.archive_task(&archived);

    let default = run_ls(&home);
    let all = ls().home(home.path()).all().run();

    default.assert_succeeded().assert_shows(&[active.as_str()]);
    assert!(
        !default.stdout.contains(&archived),
        "既定は現役のみ: {}",
        default.stdout
    );

    all.assert_succeeded();
    let row = row(&all, &archived);
    assert!(row.contains("アーカイブ済み"), "{row}");
    assert!(
        row.contains(&format!("pulsen/{archived}")),
        "成果の回収に使うブランチも出る: {row}"
    );
}

#[test]
fn 実行状態の不正値は有効な値の一覧を添えて非0で終わる() {
    let home = home_with_workflow();

    for given in ["Pending", ""] {
        let run = ls().home(home.path()).state(given).run();

        run.assert_rejected().assert_reports(&[
            "--state の値が不正です",
            "pending",
            "launching",
            "running",
            "completed",
            "failed",
            "stopped",
        ]);
    }
}

#[test]
fn 実行状態の値を省いて次のフラグを置いてもそれを値として拒否する() {
    let home = home_with_workflow();

    let run = ls().home(home.path()).state("--all").run();

    run.assert_rejected().assert_reports(&[
        "--state の値が不正です",
        "指定: `--all`",
        "pending",
        "launching",
        "running",
        "completed",
        "failed",
        "stopped",
    ]);
}

/// `probe` エージェントとワークフローを備えたホーム。
fn probe_home() -> Home {
    let home = Home::uninitialized();
    let probe = agent_probe().expect("cargo test は examples をビルドする");
    home.write_config(&probe_config(&probe, &PRINT_INPUT));
    home.write_workflow("implement", common::PROBE_WORKFLOW);
    home
}

/// 起動したラッパーの完了を待つ。
///
/// 孫プロセスの書き込みと一時ホームの削除が競合すると、削除済みのホームが部分的に
/// 復活する。読み取りの検証が終わっても、起動した実行の完了までは待つ。
fn settle(home: &Home, id: &str) {
    let run_dir = home.run_dir(id, 1);
    if !run_dir.exists() {
        return;
    }
    wait_until("exit", &run_dir, || exists(&run_dir.join("exit")));
}

fn exists(path: &Path) -> bool {
    path.is_file()
}
