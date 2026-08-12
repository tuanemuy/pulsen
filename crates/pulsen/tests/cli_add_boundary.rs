//! `pulsen add` の境界値とエッジケース(spec/testcases/task/register-task.md 境界値・エッジケース)。
//!
//! 受理側は「登録が成功する」ことを、拒否側は「タスクが作られず、利用者が用意した
//! リソースが変更されない」ことを確かめる。

mod common;

use std::fs;
use std::path::Path;

use common::{CONFIG, Home, Repo, WORKFLOW, add, scratch};
use serde_json::Value;

/// 与えた定義を名前 `flow` で登録し、作られたタスクを返す。
fn register(config: &str, definition: &str) -> Value {
    let home = Home::new();
    home.write_config(config);
    home.write_workflow("flow", definition);
    let repo = Repo::with_commit();

    add("flow", repo.path())
        .home(home.path())
        .run()
        .assert_succeeded();

    home.only_task()
}

/// 既定の設定で定義だけを与えて登録する。
fn register_definition(definition: &str) -> Value {
    register(CONFIG, definition)
}

/// カレントディレクトリからの相対指定で定義を置き、登録した結果のワークフロー名を返す。
fn register_by_relative_path(relative: &str) -> String {
    let home = Home::new();
    let dir = scratch();
    let definition = dir.path().join(relative);
    if let Some(parent) = definition.parent() {
        fs::create_dir_all(parent).expect("置き場を作れる");
    }
    fs::write(&definition, WORKFLOW).expect("定義を書ける");
    let repo = Repo::with_commit();

    add(relative, repo.path())
        .home(home.path())
        .cwd(dir.path())
        .run()
        .assert_succeeded();

    home.only_task()["workflow_name"]
        .as_str()
        .expect("workflow_name は文字列である")
        .to_owned()
}

/// `--base` の値だけを変えて登録を試み、拒否されたことを確かめる。
fn reject_base(base: &str) {
    let home = Home::new();
    home.write_workflow("implement", WORKFLOW);
    let repo = Repo::with_commit();
    let untouched = home.untouched();

    let run = add("implement", repo.path())
        .home(home.path())
        .base(base)
        .run();

    run.assert_rejected()
        .assert_reports(&["--base", "ブランチ名として不正"]);
    // 引数の使い方の誤り(clap の 2)ではなく、入力の検証エラーとして扱われる。
    assert_eq!(run.code, Some(1), "入力の検証エラーの終了コードになる");
    assert!(home.has_no_task(), "タスクは作られない: --base {base}");
    untouched.assert_unchanged();
}

/// エージェント実行ステータスの `queued` の定義。
fn queued(task: &Value) -> &Value {
    &task["snapshot"]["statuses"]["queued"]
}

#[test]
fn tc_task_register_task_049_区切り文字を含む指定はパスとして解決される() {
    assert_eq!(register_by_relative_path("./flows/x"), "x");
}

#[test]
fn tc_task_register_task_050_yaml拡張子の指定はパスとして解決される() {
    assert_eq!(register_by_relative_path("custom.yaml"), "custom");
}

#[test]
fn tc_task_register_task_051_yml拡張子の指定はパスとして解決される() {
    assert_eq!(register_by_relative_path("custom.yml"), "custom");
}

#[test]
fn tc_task_register_task_052_区切りも拡張子もない指定はymlへ退避せず名前として解決される() {
    let home = Home::new();
    home.write_workflow("implement", WORKFLOW);
    let repo = Repo::with_commit();

    add("implement", repo.path())
        .home(home.path())
        .run()
        .assert_succeeded();
    assert_eq!(home.only_task()["workflow_name"], "implement");

    let fallback = Home::new();
    fallback.write_workflow_file("other.yml", WORKFLOW);

    let run = add("other", repo.path()).home(fallback.path()).run();

    let attempted = fallback.workflow_path("other").display().to_string();
    run.assert_rejected()
        .assert_reports(&["見つかりません", attempted.as_str()]);
    assert!(fallback.has_no_task(), "タスクは作られない");
}

#[test]
fn tc_task_register_task_053_ワークフロー指定が空文字列なら入力境界で拒否される() {
    let home = Home::new();
    home.write_workflow("implement", WORKFLOW);
    let repo = Repo::with_commit();
    let untouched = home.untouched();

    let run = add("", repo.path()).home(home.path()).run();

    run.assert_rejected()
        .assert_reports(&["--workflow", "空文字列"]);
    assert_eq!(run.code, Some(1), "入力の検証エラーの終了コードになる");
    assert!(home.has_no_task(), "タスクは作られない");
    untouched.assert_unchanged();
}

#[test]
fn tc_task_register_task_054_ベースブランチが空文字列なら拒否される() {
    reject_base("");
}

#[test]
fn tc_task_register_task_055_ブランチ名として不正な形は拒否される() {
    for base in ["-main", "fix..main", "/main", "main/", "main.lock", "ma in"] {
        reject_base(base);
    }
}

#[test]
fn tc_task_register_task_056_retriesが0の定義は受理される() {
    let task = register_definition(
        "\
agent: claude
initial: queued
statuses:
  queued:
    prompt: 実装して
    retries: 0
    next: done
  done:
    run: cleanup
",
    );

    assert_eq!(queued(&task)["retries"], 0);
}

#[test]
fn tc_task_register_task_057_timeoutがnoneの定義は無制限として受理される() {
    let task = register_definition(
        "\
agent: claude
initial: queued
statuses:
  queued:
    prompt: 実装して
    timeout: none
    next: done
  done:
    run: cleanup
",
    );

    assert_eq!(queued(&task)["timeout"], "none");
}

#[test]
fn tc_task_register_task_058_期間として不正なtimeoutは拒否される() {
    for timeout in ["0s", "30", "-5s"] {
        let home = Home::new();
        home.write_workflow(
            "flow",
            &format!(
                "\
agent: claude
initial: queued
statuses:
  queued:
    prompt: 実装して
    timeout: \"{timeout}\"
    next: done
  done:
    run: cleanup
"
            ),
        );
        let repo = Repo::with_commit();
        let untouched = home.untouched();

        let run = add("flow", repo.path()).home(home.path()).run();

        run.assert_rejected()
            .assert_reports(&["statuses.queued.timeout"]);
        assert!(home.has_no_task(), "タスクは作られない: timeout {timeout}");
        untouched.assert_unchanged();
    }
}

#[test]
fn tc_task_register_task_059_statusesが1件だけの定義は受理される() {
    let task = register_definition(
        "\
initial: done
statuses:
  done:
    run: cleanup
",
    );

    assert_eq!(task["task_status"], "done");
    assert_eq!(
        task["snapshot"]["statuses"]
            .as_object()
            .expect("statuses はマッピングである")
            .len(),
        1
    );
}

#[test]
fn tc_task_register_task_060_state配下は自動作成されて登録が成功する() {
    let home = Home::new();
    home.write_workflow("implement", WORKFLOW);
    let repo = Repo::with_commit();
    assert!(!home.state_dir().exists(), "state/ は登録前には無い");

    add("implement", repo.path())
        .home(home.path())
        .run()
        .assert_succeeded();

    assert!(home.tasks_dir().is_dir(), "state/tasks が自動作成される");
    assert_eq!(home.tasks().len(), 1);
}

#[test]
fn tc_task_register_task_061_自己参照や循環のある定義は受理される() {
    let task = register_definition(
        "\
agent: claude
initial: queued
statuses:
  queued:
    prompt: 実装して
    next: reviewing
  reviewing:
    prompt: 見直して
    next: queued
",
    );

    assert_eq!(queued(&task)["next"], "reviewing");
}

#[test]
fn tc_task_register_task_062_到達不能なステータスを含む定義は受理される() {
    let task = register_definition(
        "\
agent: claude
initial: queued
statuses:
  queued:
    prompt: 実装して
    next: done
  done:
    run: cleanup
  parked:
    run: wait
",
    );

    assert_eq!(task["snapshot"]["statuses"]["parked"]["action"], "wait");
}

#[test]
fn tc_task_register_task_063_終端のない定義は受理される() {
    let task = register_definition(
        "\
agent: claude
initial: queued
statuses:
  queued:
    prompt: 実装して
    next: parked
  parked:
    run: wait
",
    );

    assert_eq!(task["snapshot"]["statuses"]["parked"]["action"], "wait");
}

#[test]
fn tc_task_register_task_064_参照しない壊れたエージェント定義は検証されない() {
    let task = register(
        "\
agents:
  claude:
    cmd: claude {input}
  broken:
    cmd: broken {nope}
",
        WORKFLOW,
    );

    assert_eq!(task["snapshot"]["default_agent"], "claude");
}

#[test]
fn tc_task_register_task_065_参照されないプレースホルダへの値の指定は許容される() {
    let task = register_definition(
        "\
agent: claude
model: sonnet
initial: queued
statuses:
  queued:
    prompt: 実装して
    next: done
  done:
    run: cleanup
",
    );

    assert_eq!(task["snapshot"]["default_model"], "sonnet");
}

#[test]
fn tc_task_register_task_066_judgeの波括弧はプレースホルダとして検査されない() {
    let task = register_definition(
        "\
agent: claude
initial: queued
statuses:
  queued:
    prompt: 実装して
    judge: [\"test\", \"{not-a-placeholder}\"]
    next: done
  done:
    run: cleanup
",
    );

    assert_eq!(
        queued(&task)["judge"],
        Value::from(vec!["test", "{not-a-placeholder}"])
    );
}

#[test]
fn tc_task_register_task_067_homeフラグは環境変数より優先される() {
    let flagged = Home::new();
    flagged.write_workflow("implement", WORKFLOW);
    let from_env = Home::uninitialized();
    let repo = Repo::with_commit();

    add("implement", repo.path())
        .home(flagged.path())
        .home_env(from_env.path())
        .run()
        .assert_succeeded();

    assert_eq!(flagged.tasks().len(), 1, "フラグのホームに登録される");
    assert!(
        from_env.has_no_task(),
        "環境変数のホームには登録されない: {}",
        Path::new(from_env.path()).display()
    );
}

#[test]
fn 環境変数だけが指定されていればそのホームに登録される() {
    let from_env = Home::new();
    from_env.write_workflow("implement", WORKFLOW);
    let user_home = scratch();
    let repo = Repo::with_commit();

    add("implement", repo.path())
        .home_env(from_env.path())
        .user_home(user_home.path())
        .run()
        .assert_succeeded();

    assert_eq!(from_env.tasks().len(), 1, "環境変数のホームに登録される");
    assert!(
        !user_home.path().join(".pulsen").exists(),
        "既定のホームは使われない"
    );
}

#[test]
fn 空の環境変数は未設定として既定のホームに落ちる() {
    let user_home = scratch();
    let repo = Repo::with_commit();

    let run = add("implement", repo.path())
        .home_env(Path::new(""))
        .user_home(user_home.path())
        .run();

    let default_home = user_home.path().join(".pulsen");
    run.assert_rejected().assert_reports(&[
        "未初期化",
        default_home.display().to_string().as_str(),
        default_home
            .join("config.yaml")
            .display()
            .to_string()
            .as_str(),
    ]);
    assert!(!default_home.exists(), "既定のホームには何も作られない");
}
