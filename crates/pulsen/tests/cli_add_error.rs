//! `pulsen add` の異常系(spec/testcases/task/register-task.md 異常系)。
//!
//! いずれのケースでも、非0で終了し・案内が原因を示し・タスクが作られず・利用者が用意した
//! リソース(config.yaml とワークフロー定義)が変更されないことを確かめる。最後の1点は
//! 「読めないリソースには書き込まない」(PAGE-common-006 規則2)の観測可能な帰結である。
//!
//! ロック機構自体の異常(TC-018)・git 操作自体の失敗(TC-040)・ID再衝突(TC-047)・
//! `create` の入出力エラー(TC-048)は実アダプターでは外から作れないため、
//! ユースケーステストで消化する。

mod common;

use std::fs;
use std::path::Path;

use common::{CONFIG, Home, Repo, Untouched, WORKFLOW, add, scratch};

/// `{model}` を参照するエージェント定義。
const MODEL_CONFIG: &str = "\
agents:
  claude:
    cmd: claude --model {model} {input}
";

/// 与えた設定と定義で名前指定の登録を行い、拒否の共通の帰結を確かめる。
fn reject(config: &str, definition: &str, expected: &[&str]) {
    let home = Home::new();
    home.write_config(config);
    home.write_workflow("broken", definition);
    let repo = Repo::with_commit();
    let untouched = home.untouched();

    let run = add("broken", repo.path()).home(home.path()).run();

    run.assert_rejected().assert_reports(expected);
    assert!(home.has_no_task(), "タスクは作られない");
    untouched.assert_unchanged();
}

/// 既定の設定のもとで、定義の不備だけを与える。
fn reject_definition(definition: &str, expected: &[&str]) {
    reject(CONFIG, definition, expected);
}

/// 有効な定義のもとで、対象リポジトリの指定だけを変える。
fn reject_target(repo: &Path, base: Option<&str>, expected: &[&str]) {
    let home = Home::new();
    home.write_workflow("implement", WORKFLOW);
    let untouched = home.untouched();

    let mut command = add("implement", repo).home(home.path());
    if let Some(base) = base {
        command = command.base(base);
    }
    let run = command.run();

    run.assert_rejected().assert_reports(expected);
    assert!(home.has_no_task(), "タスクは作られない");
    untouched.assert_unchanged();
}

#[test]
fn tc_task_register_task_014_設定が無ければ未初期化として案内される() {
    let home = Home::uninitialized();
    home.write_workflow("implement", WORKFLOW);
    let repo = Repo::with_commit();
    let untouched = home.untouched();

    let run = add("implement", repo.path()).home(home.path()).run();

    let home_path = home.path().display().to_string();
    let config_path = home.config_path().display().to_string();
    run.assert_rejected()
        .assert_reports(&["未初期化", home_path.as_str(), config_path.as_str()]);
    assert!(home.has_no_task(), "タスクは作られない");
    untouched.assert_unchanged();
}

#[test]
fn tc_task_register_task_015_設定がパース不能なら位置と原因を示して拒否される() {
    let home = Home::new();
    home.write_workflow("implement", WORKFLOW);
    let repo = Repo::with_commit();

    home.write_config("agents: [\n");
    let untouched = home.untouched();
    let syntax = add("implement", repo.path()).home(home.path()).run();
    syntax.assert_rejected().assert_reports(&["位置:", "行"]);
    assert!(home.has_no_task(), "タスクは作られない");
    untouched.assert_unchanged();

    home.write_config("typo_key: 1\n");
    let untouched = home.untouched();
    let unknown_key = add("implement", repo.path()).home(home.path()).run();
    unknown_key.assert_rejected().assert_reports(&["typo_key"]);
    assert!(home.has_no_task(), "タスクは作られない");
    untouched.assert_unchanged();
}

#[test]
fn tc_task_register_task_016_設定が読めなければ実行環境のエラーになる() {
    let home = Home::new();
    home.write_workflow("implement", WORKFLOW);
    let repo = Repo::with_commit();
    let untouched = home.untouched();

    let Some(restore) = common::deny_read(&home.config_path()) else {
        println!("スキップ: config.yaml を読めない状態にできない(root 実行・非 POSIX 等)");
        return;
    };

    let run = add("implement", repo.path()).home(home.path()).run();
    drop(restore);

    let config_path = home.config_path().display().to_string();
    run.assert_rejected()
        .assert_reports(&["読み込めません", config_path.as_str()]);
    assert!(home.has_no_task(), "タスクは作られない");
    untouched.assert_unchanged();
}

#[test]
fn tc_task_register_task_017_ロック競合は別の操作が実行中として拒否される() {
    let home = Home::new();
    home.write_workflow("implement", WORKFLOW);
    let repo = Repo::with_commit();
    let untouched = home.untouched();

    let Some(holder) = common::lock::hold(&home.lock_path()) else {
        println!("スキップ: 別プロセスにロックを保持させられない");
        return;
    };

    let run = add("implement", repo.path()).home(home.path()).run();
    common::lock::release(holder).expect("保持プロセスを終了できる");

    run.assert_rejected().assert_reports(&["別の操作が実行中"]);
    assert!(home.has_no_task(), "タスクは作られない");
    untouched.assert_unchanged();
}

#[test]
fn tc_task_register_task_019_名前が解決できなければ試みた絶対パスを添えて拒否される() {
    let home = Home::new();
    let repo = Repo::with_commit();
    let untouched = home.untouched();

    let run = add("absent", repo.path()).home(home.path()).run();

    let attempted = home.workflow_path("absent").display().to_string();
    run.assert_rejected()
        .assert_reports(&["見つかりません", attempted.as_str()]);
    assert!(home.has_no_task(), "タスクは作られない");
    untouched.assert_unchanged();
}

#[test]
fn tc_task_register_task_020_パス指定の定義が無ければ試みたパスを添えて拒否される() {
    let home = Home::new();
    let dir = scratch();
    let missing = dir.path().join("absent.yaml");
    let repo = Repo::with_commit();
    let untouched = home.untouched();

    let run = add(&missing, repo.path()).home(home.path()).run();

    let attempted = missing.display().to_string();
    run.assert_rejected()
        .assert_reports(&["見つかりません", attempted.as_str()]);
    assert!(home.has_no_task(), "タスクは作られない");
    untouched.assert_unchanged();
}

#[test]
fn tc_task_register_task_021_定義が読めなければ拒否される() {
    let home = Home::new();
    let definition = home.write_workflow("implement", WORKFLOW);
    let repo = Repo::with_commit();
    let untouched = home.untouched();

    let Some(restore) = common::deny_read(&definition) else {
        println!("スキップ: ワークフロー定義を読めない状態にできない(root 実行・非 POSIX 等)");
        return;
    };

    let run = add("implement", repo.path()).home(home.path()).run();
    drop(restore);

    run.assert_rejected()
        .assert_reports(&["読み込めません", definition.display().to_string().as_str()]);
    assert!(home.has_no_task(), "タスクは作られない");
    untouched.assert_unchanged();
}

#[test]
fn tc_task_register_task_022_定義のyamlが構文として不正なら位置と原因を示して拒否される() {
    reject_definition("statuses: [\n", &["YAML 構文エラー", "位置:", "行"]);
    reject_definition(
        "\
initial: queued
initial: done
statuses:
  queued:
    prompt: 実装して
    next: queued
",
        &["YAML 構文エラー", "位置:", "行"],
    );
}

#[test]
fn tc_task_register_task_023_スキーマ外のキーは拒否される() {
    reject_definition(
        "\
typo_key: 1
agent: claude
initial: queued
statuses:
  queued:
    prompt: 実装して
    next: queued
",
        &["スキーマ外のキー", "typo_key"],
    );
}

#[test]
fn tc_task_register_task_024_動作種別に無関係なキーは拒否される() {
    reject_definition(
        "\
agent: claude
initial: waiting
statuses:
  waiting:
    run: wait
    judge: cargo test
",
        &["使えないキー", "judge"],
    );
}

#[test]
fn tc_task_register_task_025_initialの欠落は拒否される() {
    reject_definition(
        "\
agent: claude
statuses:
  queued:
    prompt: 実装して
    next: queued
",
        &["initial が指定されていません"],
    );
}

#[test]
fn tc_task_register_task_026_initialの参照先が無ければ拒否される() {
    reject_definition(
        "\
agent: claude
initial: nowhere
statuses:
  queued:
    prompt: 実装して
    next: queued
",
        &["initial", "nowhere"],
    );
}

#[test]
fn tc_task_register_task_027_statusesが空なら拒否される() {
    reject_definition(
        "\
agent: claude
initial: queued
",
        &["statuses が空"],
    );
}

#[test]
fn tc_task_register_task_028_動作宣言のないステータスは拒否される() {
    reject_definition(
        "\
agent: claude
initial: queued
statuses:
  queued:
    next: queued
",
        &["動作宣言", "queued"],
    );
}

#[test]
fn tc_task_register_task_029_動作宣言が複数あるステータスは拒否される() {
    reject_definition(
        "\
agent: claude
initial: queued
statuses:
  queued:
    prompt: 実装して
    skill: review
    next: queued
",
        &["動作宣言が複数", "prompt", "skill"],
    );
}

#[test]
fn tc_task_register_task_030_runの値がcleanupでもwaitでもなければ拒否される() {
    reject_definition(
        "\
agent: claude
initial: paused
statuses:
  paused:
    run: pause
",
        &["cleanup / wait", "pause"],
    );
}

#[test]
fn tc_task_register_task_031_エージェント実行にnextが無ければ拒否される() {
    reject_definition(
        "\
agent: claude
initial: queued
statuses:
  queued:
    prompt: 実装して
",
        &["next がありません", "queued"],
    );
}

#[test]
fn tc_task_register_task_032_nextの参照先が無ければ拒否される() {
    reject_definition(
        "\
agent: claude
initial: queued
statuses:
  queued:
    prompt: 実装して
    next: nowhere
",
        &["next", "nowhere"],
    );
}

#[test]
fn tc_task_register_task_033_名前や期間の値が不正なら拒否される() {
    reject_definition(
        "\
agent: claude
initial: queued
statuses:
  queued:
    prompt: \"\"
    next: queued
",
        &["statuses.queued.prompt", "値が不正です"],
    );
    reject_definition(
        "\
agent: claude
initial: queued
statuses:
  queued:
    prompt: 実装して
    timeout: 0s
    next: queued
",
        &["statuses.queued.timeout", "値が不正です"],
    );
}

#[test]
fn tc_task_register_task_034_ファイル名から表示名を決められなければ拒否される() {
    let home = Home::new();
    let dir = scratch();
    // 拡張子を除くと名前として使えない(前後に空白のみ)ファイル名。
    let definition = dir.path().join(" .yaml");
    fs::write(&definition, WORKFLOW).expect("定義を書ける");
    let repo = Repo::with_commit();
    let untouched = Untouched::of(home.resources().into_iter().chain([definition.clone()]));

    let run = add(&definition, repo.path()).home(home.path()).run();

    run.assert_rejected()
        .assert_reports(&["ワークフロー名を決められません", "workflow:"]);
    assert!(home.has_no_task(), "タスクは作られない");
    untouched.assert_unchanged();
}

#[test]
fn tc_task_register_task_035_リポジトリのパスが存在しなければ拒否される() {
    let dir = scratch();

    reject_target(&dir.path().join("absent"), None, &["存在しません"]);
}

#[test]
fn tc_task_register_task_036_gitリポジトリでないパスは拒否される() {
    let dir = scratch();
    if !common::git::is_outside_repository(dir.path()) {
        println!("スキップ: 一時ディレクトリ自体が git リポジトリ配下にある");
        return;
    }

    reject_target(dir.path(), None, &["git リポジトリではありません"]);
}

#[test]
fn tc_task_register_task_037_存在しないベースブランチは拒否される() {
    let repo = Repo::with_commit();

    reject_target(
        repo.path(),
        Some("no-such-branch"),
        &["ベースブランチ", "no-such-branch"],
    );
}

#[test]
fn tc_task_register_task_038_detached_headでベース省略ならbaseの明示を案内される() {
    let repo = Repo::detached();

    reject_target(repo.path(), None, &["detached HEAD", "--base"]);
}

#[test]
fn tc_task_register_task_039_空リポジトリでベース省略ならbaseの明示を案内される() {
    let repo = Repo::empty();

    reject_target(repo.path(), None, &["空のリポジトリ", "--base"]);
}

#[test]
fn tc_task_register_task_041_エージェントの指定がどこにも無ければ拒否される() {
    reject_definition(
        "\
initial: queued
statuses:
  queued:
    prompt: 実装して
    next: queued
",
        &["エージェントが指定されていません", "queued"],
    );
}

#[test]
fn tc_task_register_task_042_未定義のエージェントは定義済み一覧を添えて拒否される() {
    reject_definition(
        "\
agent: ghost
initial: queued
statuses:
  queued:
    prompt: 実装して
    next: queued
",
        &[
            "ghost",
            "定義されていません",
            "定義済みのエージェント: claude",
        ],
    );
}

#[test]
fn tc_task_register_task_043_エージェント定義のテンプレートが壊れていれば拒否される() {
    reject(
        "\
agents:
  claude:
    cmd: claude {nope} {input}
",
        WORKFLOW,
        &["定義が不正です", "nope"],
    );
}

#[test]
fn tc_task_register_task_044_skill指定にskill_inputが無ければ拒否される() {
    reject_definition(
        "\
agent: claude
initial: queued
statuses:
  queued:
    skill: review
    next: queued
",
        &["skill_input がありません", "queued", "claude"],
    );
}

#[test]
fn tc_task_register_task_045_modelを参照するのにmodelの指定が無ければ拒否される() {
    reject(
        MODEL_CONFIG,
        WORKFLOW,
        &["{model}", "model の指定がありません"],
    );
}

#[test]
fn tc_task_register_task_046_複数の検証エラーは全件まとめて表示される() {
    reject(
        MODEL_CONFIG,
        "\
agent: claude
initial: queued
statuses:
  queued:
    prompt: 実装して
    next: reviewing
  reviewing:
    skill: review
    next: queued
",
        &[
            "3件",
            "`queued`",
            "`reviewing`",
            "model の指定がありません",
            "skill_input がありません",
        ],
    );
}
