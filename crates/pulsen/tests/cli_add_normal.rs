//! `pulsen add` の正常系(spec/testcases/task/register-task.md 正常系)。
//!
//! 実バイナリを起動し、実アダプター(一時ホーム + `git init` した一時リポジトリ)に対して
//! 終了コード・出力・作られたタスクファイルの中身で検証する。ID衝突からの再試行
//! (TC-012)は実アダプターでは外から作れないため、ユースケーステストで消化する。

mod common;

use std::fs;
use std::path::Path;

use common::{Home, Repo, WORKFLOW, add, scratch};
use serde_json::Value;

/// `workflow:` キーで名前を宣言した定義。
const NAMED_WORKFLOW: &str = "\
workflow: my-flow
agent: claude
initial: queued
statuses:
  queued:
    prompt: 実装して
    next: done
  done:
    run: cleanup
";

/// 記録された対象リポジトリが、与えたディレクトリを指す絶対パスであることを確かめる。
fn assert_repo_is(task: &Value, expected: &Path) {
    let recorded = Path::new(
        task["target"]["repo"]
            .as_str()
            .expect("repo は文字列である"),
    );
    assert!(
        recorded.is_absolute(),
        "絶対パスで記録される: {}",
        recorded.display()
    );
    assert_eq!(
        fs::canonicalize(recorded).expect("記録されたパスを解決できる"),
        fs::canonicalize(expected).expect("対象のパスを解決できる")
    );
}

#[test]
fn tc_task_register_task_001_名前指定の登録でタスクidが表示され0で終了する() {
    let home = Home::new();
    home.write_workflow("implement", WORKFLOW);
    let repo = Repo::with_commit();

    let run = add("implement", repo.path())
        .home(home.path())
        .base("main")
        .run();

    run.assert_succeeded();
    let task = home.only_task();
    let task_id = task["task_id"].as_str().expect("task_id は文字列である");
    assert!(
        run.stdout.contains(task_id),
        "タスクIDが表示される: {}",
        run.stdout
    );
    assert_eq!(task["execution"]["state"], "pending");
}

#[test]
fn tc_task_register_task_002_名前指定では指定した名前と解決先が表示される() {
    let home = Home::new();
    let resolved = home.write_workflow("implement", WORKFLOW);
    let repo = Repo::with_commit();

    let run = add("implement", repo.path()).home(home.path()).run();

    run.assert_succeeded();
    assert!(
        run.stdout.contains("implement"),
        "ワークフロー名が表示される: {}",
        run.stdout
    );
    assert!(
        run.stdout.contains(&resolved.display().to_string()),
        "解決先の絶対パスが表示される: {}",
        run.stdout
    );
}

#[test]
fn tc_task_register_task_003_名前指定では定義のworkflowキーは使われない() {
    let home = Home::new();
    home.write_workflow("implement", NAMED_WORKFLOW);
    let repo = Repo::with_commit();

    let run = add("implement", repo.path()).home(home.path()).run();

    run.assert_succeeded();
    assert_eq!(home.only_task()["workflow_name"], "implement");
}

#[test]
fn tc_task_register_task_004_パス指定では定義のworkflowキーが名前になる() {
    let home = Home::new();
    let dir = scratch();
    let definition = dir.path().join("custom.yaml");
    fs::write(&definition, NAMED_WORKFLOW).expect("定義を書ける");
    let repo = Repo::with_commit();

    let run = add(&definition, repo.path()).home(home.path()).run();

    run.assert_succeeded();
    assert_eq!(home.only_task()["workflow_name"], "my-flow");
}

#[test]
fn tc_task_register_task_005_パス指定でworkflowキーがなければファイル名が名前になる() {
    let home = Home::new();
    let dir = scratch();
    let definition = dir.path().join("custom.yaml");
    fs::write(&definition, WORKFLOW).expect("定義を書ける");
    let repo = Repo::with_commit();

    let run = add(&definition, repo.path()).home(home.path()).run();

    run.assert_succeeded();
    assert_eq!(home.only_task()["workflow_name"], "custom");
}

#[test]
fn tc_task_register_task_006_ベースブランチを省略するとheadのブランチになる() {
    let home = Home::new();
    home.write_workflow("implement", WORKFLOW);
    let repo = Repo::with_commit();

    let run = add("implement", repo.path()).home(home.path()).run();

    run.assert_succeeded();
    assert_eq!(home.only_task()["target"]["base_branch"], "main");
}

#[test]
fn tc_task_register_task_007_指定したベースブランチが対象として記録される() {
    let home = Home::new();
    home.write_workflow("implement", WORKFLOW);
    let repo = Repo::with_commit();
    repo.create_branch("develop");

    let run = add("implement", repo.path())
        .home(home.path())
        .base("develop")
        .run();

    run.assert_succeeded();
    assert_eq!(home.only_task()["target"]["base_branch"], "develop");
}

#[test]
fn tc_task_register_task_008_相対パスのリポジトリは絶対パスに正規化される() {
    let home = Home::new();
    home.write_workflow("implement", WORKFLOW);
    let repo = Repo::with_commit();
    let parent = repo.path().parent().expect("親ディレクトリがある");
    let name = repo.path().file_name().expect("ディレクトリ名がある");

    let run = add("implement", name).home(home.path()).cwd(parent).run();

    run.assert_succeeded();
    assert_repo_is(&home.only_task(), repo.path());
}

#[test]
fn tc_task_register_task_009_登録直後のタスクは初期ステータスの実行待ちになる() {
    let home = Home::new();
    home.write_workflow("implement", WORKFLOW);
    let repo = Repo::with_commit();

    add("implement", repo.path())
        .home(home.path())
        .run()
        .assert_succeeded();

    let task = home.only_task();
    assert_eq!(task["task_status"], task["snapshot"]["initial"]);
    assert_eq!(task["execution"]["state"], "pending");
    assert_eq!(task["counters"]["attempt_count"], 0);
    assert_eq!(task["counters"]["judge_attempt_count"], 0);
    assert_eq!(task["counters"]["spawn_fail_count"], 0);
    assert_eq!(task["workspace"], Value::Null);
    assert_eq!(task["current_attempt"], Value::Null);
    assert_eq!(task["last_failure"], Value::Null);
}

#[test]
fn tc_task_register_task_010_埋め込まれたスナップショットは元yamlの編集に影響されない() {
    let home = Home::new();
    home.write_workflow("implement", WORKFLOW);
    let repo = Repo::with_commit();

    add("implement", repo.path())
        .home(home.path())
        .run()
        .assert_succeeded();

    let embedded = home.only_task()["snapshot"].clone();
    assert_eq!(
        embedded["statuses"]["queued"]["input"]["prompt"],
        "実装して"
    );

    home.write_workflow(
        "implement",
        &WORKFLOW.replace("prompt: 実装して", "prompt: 別の指示"),
    );

    assert_eq!(home.only_task()["snapshot"], embedded);
}

#[test]
fn tc_task_register_task_011_登録ではエージェント実行が始まらない() {
    let home = Home::new();
    home.write_workflow("implement", WORKFLOW);
    let repo = Repo::with_commit();

    add("implement", repo.path())
        .home(home.path())
        .run()
        .assert_succeeded();

    let task = home.only_task();
    assert_eq!(task["execution"]["state"], "pending");
    assert_eq!(task["current_attempt"], Value::Null);
    assert!(
        !home.path().join("state").join("runs").exists(),
        "run ディレクトリは作られない"
    );
    assert!(
        !home.path().join("worktrees").exists(),
        "worktree は作られない"
    );
}

#[test]
fn tc_task_register_task_013_同じ指定を繰り返しても別idの独立したタスクになる() {
    let home = Home::new();
    home.write_workflow("implement", WORKFLOW);
    let repo = Repo::with_commit();

    add("implement", repo.path())
        .home(home.path())
        .run()
        .assert_succeeded();
    add("implement", repo.path())
        .home(home.path())
        .run()
        .assert_succeeded();

    let tasks = home.tasks();
    assert_eq!(tasks.len(), 2, "独立した2件のタスクになる");
    assert_ne!(tasks[0]["task_id"], tasks[1]["task_id"]);
    assert_eq!(tasks[0]["workflow_name"], tasks[1]["workflow_name"]);
    assert_eq!(tasks[0]["target"], tasks[1]["target"]);
}
