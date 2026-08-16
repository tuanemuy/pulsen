//! `show` の受け入れ検証(実バイナリ・実ファイルシステム・実プロセス)。
//!
//! 台帳の PAGE 行が主張するのは**表示**であり、綴りまで含めた確認は実バイナリの出力で
//! しかできない。実行メタデータの参照(runディレクトリ・ログ・exit・同定情報)は
//! `examples/agent_probe` を起動して本物の tick で作る。

mod common;

use std::fs;

use common::{Home, Repo, Run, WORKFLOW, add, agent_probe, probe_config, show, tick, wait_until};
use serde_json::json;

/// エージェントが 0 で終わる既定のモード(標準出力に入力、標準エラーは空)。
const PRINT_INPUT: [&str; 3] = ["print", "{input}", ""];

/// tick と同時に読み取る周回で回す tick の回数。
const TICK_ROUNDS: usize = 3;

/// 凍結に至る4つの経路と、それぞれの表示。タスクファイルの綴りと文言の対応を固定する。
const STOP_REASONS: [(&str, &str); 4] = [
    ("retry_limit_exceeded", "リトライ上限の超過"),
    ("judge_limit_exceeded", "判定失敗の上限の超過"),
    ("spawn_fail_limit_exceeded", "spawn 失敗の上限の超過"),
    ("aborted", "利用者による中断"),
];

/// 有効な設定と `implement` ワークフローを備えたホーム。
fn home_with_workflow() -> Home {
    let home = Home::new();
    home.write_workflow("implement", WORKFLOW);
    home
}

/// `probe` エージェントとワークフローを備えたホーム。
fn probe_home() -> Home {
    let home = Home::uninitialized();
    let probe = agent_probe().expect("cargo test は examples をビルドする");
    home.write_config(&probe_config(&probe, &PRINT_INPUT));
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

fn run_show(home: &Home, id: &str) -> Run {
    show(id).home(home.path()).run()
}

/// 表示から項目行を1つ取り出す(現在attempt配下の項目は字下げされる)。
fn sub_line<'a>(stdout: &'a str, label: &str) -> &'a str {
    let prefix = format!("{label}: ");
    stdout
        .lines()
        .find(|line| line.trim_start().starts_with(&prefix))
        .unwrap_or_else(|| panic!("{label} の項目行がある: {stdout}"))
}

/// 起動 → 完走 → running への取り込みまで進める。
fn launch_and_confirm(home: &Home, id: &str) {
    tick().home(home.path()).run().assert_succeeded();
    let run_dir = home.run_dir(id, 1);
    wait_until("exit", &run_dir, || run_dir.join("exit").is_file());
    tick().home(home.path()).run().assert_succeeded();
    assert_eq!(
        home.task(id)["execution"]["state"],
        json!("running"),
        "起動を確認して running へ取り込む"
    );
}

#[test]
fn tc_task_show_task_001_未実行のタスクはワークスペース未作成とattemptなしを示す() {
    let home = home_with_workflow();
    let repo = Repo::with_commit();
    let id = register(&home, &repo);

    let run = run_show(&home, &id);

    run.assert_succeeded().assert_shows(&[
        id.as_str(),
        "ワークフロー: implement",
        "タスクステータス: queued",
        "実行状態: pending",
        "workspace_path: 未作成",
        "branch: 未作成",
        "現在attempt: なし",
        &repo.path().display().to_string(),
        "ベースブランチ: main",
    ]);
}

#[test]
fn tc_task_show_task_002_実行履歴のあるタスクはattemptとログのパスを示す() {
    let home = probe_home();
    let repo = Repo::with_commit();
    let id = register(&home, &repo);
    launch_and_confirm(&home, &id);

    let run = run_show(&home, &id);
    let run_dir = home.run_dir(&id, 1);

    run.assert_succeeded().assert_shows(&[
        "現在attempt: 1",
        &run_dir.display().to_string(),
        &run_dir.join("stdout.log").display().to_string(),
        &run_dir.join("stderr.log").display().to_string(),
        &run_dir.join("exit").display().to_string(),
        "(値 0)",
    ]);
}

#[test]
fn tc_task_show_task_003_runningのタスクはpidとkill同定子とstarttimeを示す() {
    let home = probe_home();
    let repo = Repo::with_commit();
    let id = register(&home, &repo);
    launch_and_confirm(&home, &id);

    let run = run_show(&home, &id);

    let process = home.task(&id)["current_attempt"]["process"].clone();
    let pid = process["pid"].as_u64().expect("PID は数値");
    let kill_ident = process["kill_ident"].as_str().expect("文字列").to_owned();
    let starttime = process["starttime"]["ident"]
        .as_str()
        .expect("文字列")
        .to_owned();

    run.assert_succeeded().assert_shows(&[
        &format!("PID: {pid}"),
        &format!("kill同定子: {kill_ident}"),
        &starttime,
    ]);
}

#[test]
fn tc_task_show_task_009_スナップショットの定義済みステータス一覧を示す() {
    let home = home_with_workflow();
    let repo = Repo::with_commit();
    let id = register(&home, &repo);

    let run = run_show(&home, &id);

    run.assert_succeeded()
        .assert_shows(&["定義済みステータス: done, queued"]);
}

#[test]
fn tc_task_show_task_010_スナップショット保存先はタスクファイル自身のパスになる() {
    let home = home_with_workflow();
    let repo = Repo::with_commit();
    let id = register(&home, &repo);

    let run = run_show(&home, &id);

    run.assert_succeeded().assert_shows(&[&format!(
        "スナップショット保存先: {}",
        home.task_path(&id).display()
    )]);
}

#[test]
fn tc_task_show_task_018_設定が読めなければ非0で終わる() {
    let home = Home::uninitialized();

    let run = run_show(&home, "20260812t090000-k3f9qa1b");

    run.assert_rejected()
        .assert_reports(&["グローバルホームが未初期化です"]);
}

#[test]
fn tc_task_show_task_035_別の操作がロックを保持していても詳細は返る() {
    let home = home_with_workflow();
    let repo = Repo::with_commit();
    let id = register(&home, &repo);

    let Some(holder) = common::lock::hold(&home.lock_path()) else {
        common::skipped("tc_task_show_task_035", "lock::hold");
        return;
    };

    let run = run_show(&home, &id);
    common::lock::release(holder).expect("保持プロセスを終了できる");

    run.assert_succeeded().assert_shows(&[id.as_str()]);
}

#[test]
fn tc_task_show_task_036_tickの更新と同時に読んでも書きかけを観測しない() {
    let home = probe_home();
    let repo = Repo::with_commit();
    let id = register(&home, &repo);

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
        let run = run_show(&home, &id);
        run.assert_succeeded().assert_shows(&[id.as_str()]);
        observations += 1;
    }
    ticking.join().expect("tick を回し切れる");

    assert!(observations > 0, "tick と重なる読み取りが1回は起きる");
    // 孫プロセスの書き込みと一時ホームの削除が競合しないよう、起動した実行の完了を待つ。
    let run_dir = home.run_dir(&id, 1);
    if run_dir.exists() {
        wait_until("exit", &run_dir, || run_dir.join("exit").is_file());
    }
}

#[test]
fn 存在しないタスクidは見つからないこととして非0で終わる() {
    let home = home_with_workflow();

    let run = run_show(&home, "20260812t090000-k3f9qa1b");

    run.assert_rejected().assert_reports(&[
        "指定されたタスクが見つかりません",
        "20260812t090000-k3f9qa1b",
    ]);
}

#[test]
fn 不正なタスクidは入力の誤りとして非0で終わる() {
    let home = home_with_workflow();

    for given in ["", "-leading", "大文字を含む"] {
        let run = run_show(&home, given);

        run.assert_rejected()
            .assert_reports(&["タスクIDが不正です"]);
    }
}

#[test]
fn 読めないタスクファイルはパースエラーの内容とパスを添えて非0で終わる() {
    let home = home_with_workflow();
    let broken = home.write_raw_task("20260812t090000-broken01", "{ これは JSON ではない");
    let untouched = common::Untouched::of([broken.clone()]);

    let run = run_show(&home, "20260812t090000-broken01");

    run.assert_rejected()
        .assert_reports(&["タスクファイルを読めません", &broken.display().to_string()]);
    untouched.assert_unchanged();
}

#[test]
fn スナップショットだけが読めないタスクは注記つきで表示して0で終わる() {
    let home = home_with_workflow();
    let repo = Repo::with_commit();
    let id = register(&home, &repo);
    home.patch_task(&id, |task| {
        task["snapshot"] = json!({ "initial": "queued" });
    });

    let run = run_show(&home, &id);

    run.assert_succeeded().assert_shows(&[
        "タスクステータス: queued",
        "実行状態: pending",
        "定義済みステータス: 読み取れません",
        "上限 導出不能",
    ]);
    assert!(
        run.stdout.contains("judge_attempt_count: 0(上限 3)"),
        "スナップショット非依存の上限は通常どおり出る: {}",
        run.stdout
    );
}

#[test]
fn アーカイブ済みのタスクは削除済みの注記とアーカイブ側の保存先を示す() {
    let home = home_with_workflow();
    let repo = Repo::with_commit();
    let id = register(&home, &repo);
    home.patch_task(&id, |task| {
        task["workspace"] = json!({
            "path": home.worktree(&id),
            "branch": format!("pulsen/{id}"),
        });
    });
    let archived = home.archive_task(&id);

    let run = run_show(&home, &id);

    run.assert_succeeded().assert_shows(&[
        "アーカイブ済み(worktree は削除済み)",
        "削除済み",
        &format!("スナップショット保存先: {}", archived.display()),
    ]);
}

#[test]
fn workspaceのパスは実体が無くても表示されるだけで0で終わる() {
    let home = home_with_workflow();
    let repo = Repo::with_commit();
    let id = register(&home, &repo);
    home.patch_task(&id, |task| {
        task["workspace"] = json!({
            "path": home.worktree(&id),
            "branch": format!("pulsen/{id}"),
        });
    });
    assert!(!home.worktree(&id).exists(), "worktree の実体は無い");

    let run = run_show(&home, &id);

    run.assert_succeeded().assert_shows(&[
        &format!("workspace_path: {}", home.worktree(&id).display()),
        &format!("branch: pulsen/{id}"),
    ]);
}

#[test]
fn 読めないexitは注記して0で終わり対象のパスを重ねて出さない() {
    let home = home_with_workflow();
    let repo = Repo::with_commit();
    let id = register(&home, &repo);
    let run_dir = home.run_dir(&id, 1);
    fs::create_dir_all(&run_dir).expect("runディレクトリを作れる");
    let exit_file = run_dir.join("exit");
    fs::write(&exit_file, "{ これは JSON ではない").expect("exit を壊せる");
    home.patch_task(&id, |task| {
        task["current_attempt"] = json!({
            "number": 1,
            "run_dir": run_dir,
            "process": null,
        });
    });

    let run = run_show(&home, &id);

    run.assert_succeeded();
    let line = sub_line(&run.stdout, "exit");
    assert!(line.contains("読み取れません: "), "{line}");
    assert_eq!(
        line.matches(&exit_file.display().to_string()).count(),
        1,
        "パスを文言へ入れる層は1つ: {line}"
    );
}

#[test]
fn runディレクトリが無い現在attemptは存在しないと注記して0で終わる() {
    let home = home_with_workflow();
    let repo = Repo::with_commit();
    let id = register(&home, &repo);
    let run_dir = home.run_dir(&id, 1);
    home.patch_task(&id, |task| {
        task["current_attempt"] = json!({
            "number": 1,
            "run_dir": run_dir,
            "process": null,
        });
    });
    assert!(!run_dir.exists(), "runディレクトリは作られていない");

    let run = run_show(&home, &id);

    run.assert_succeeded().assert_shows(&[
        "現在attempt: 1",
        &format!("{}(存在しません)", run_dir.display()),
        "同定情報: 未取得",
    ]);
}

#[test]
fn runディレクトリの有無を確認できない現在attemptはパスを添えて注記し0で終わる() {
    if !common::lookup_under_regular_file_fails() {
        common::skipped(
            "runディレクトリの有無を確認できない現在attemptはパスを添えて注記し0で終わる",
            "lookup_under_regular_file_fails",
        );
        return;
    }
    let home = home_with_workflow();
    let repo = Repo::with_commit();
    let id = register(&home, &repo);
    let run_dir = home.run_dir(&id, 1);
    // タスクの run ディレクトリの置き場を通常ファイルにすると、その配下にある attempt の
    // 有無は不在ではなく機構の失敗として返る。権限を操作せずに確認失敗を作れる。
    fs::create_dir_all(home.runs_dir()).expect("run ディレクトリのルートを作れる");
    fs::write(home.runs_dir().join(&id), b"").expect("通常ファイルを置ける");
    home.patch_task(&id, |task| {
        task["current_attempt"] = json!({
            "number": 1,
            "run_dir": run_dir,
            "process": null,
        });
    });

    let run = run_show(&home, &id);

    run.assert_succeeded();
    let line = sub_line(&run.stdout, "runディレクトリ");
    assert!(line.contains("存在を確認できません: "), "{line}");
    assert_eq!(
        line.matches(&run_dir.display().to_string()).count(),
        1,
        "確認できなかった対象のパスがちょうど1度現れる: {line}"
    );
    assert!(
        sub_line(&run.stdout, "exit").contains("読んでいません"),
        "{}",
        run.stdout
    );
}

#[test]
fn 凍結したタスクは要因と通知の有無と上限つきのカウンタで原因を辿れる() {
    let home = home_with_workflow();
    let repo = Repo::with_commit();
    let id = register(&home, &repo);

    for (reason, described) in STOP_REASONS {
        home.patch_task(&id, |task| {
            task["execution"] = json!({
                "state": "stopped",
                "reason": reason,
                "notified_at": null,
            });
            task["last_failure"] = json!({
                "kind": "spawn_fail",
                "message": "エージェントを起動できません",
                "at": "2026-08-12T10:00:00Z",
            });
            task["counters"]["spawn_fail_count"] = json!(4);
        });

        let run = run_show(&home, &id);

        run.assert_succeeded().assert_shows(&[
            "実行状態: stopped",
            &format!("凍結要因: {described}"),
            "通知: 未記録",
            "直近の失敗要因: エージェントの起動(2026-08-12T10:00:00Z): エージェントを起動できません",
            "spawn_fail_count: 4(上限 3)",
        ]);
    }
}
