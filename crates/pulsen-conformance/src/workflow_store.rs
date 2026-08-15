//! WorkflowStore の適合ケース(`spec/testcases/ports/workflow-store.md` の31行)。
//!
//! 厳格パースの全エラー種をポート越しに検証する。パースの分担は
//! 「アダプターが YAML テキストを `RawWorkflowDoc` に変換し(`YamlSyntax` / `UnknownKey` を
//! 検出)、`WorkflowAssembler` が残りを検証する」であり、どちらが生成したかはポートの
//! 外からは見えない。
//!
//! 固定するのはエラー種と、spec が意味を定めるフィールド(`attempted` / `resolved_from` /
//! ステータス名・キー名・ステータス配下の論理パス)までにする。メッセージの文言と、
//! アダプターが独自に決める表示(ドキュメント自体を指す位置の書き方など)は検証しない —
//! 実装ごとに変わってよい部分を固定すると、別のアダプターが同じスイートを通せなくなる。

use std::path::PathBuf;

use pulsen_domain::definition::{
    AgentInput, AgentName, LoadedWorkflow, ModelName, PlainCommand, Prompt, SkillName,
    StatusDefinition, StatusName, TimeoutSpec, WorkflowDefinition, WorkflowLoadError, WorkflowName,
    WorkflowParseError, WorkflowRef, WorkflowStore,
};

use crate::{CaseOutcome, WorkflowStoreHarness, require};

/// 有効な最小定義。名前解決と可視性のケースが共有する。
const VALID: &str = "workflow: implement
initial: queued
statuses:
  queued:
    prompt: 実装して
    next: done
  done:
    run: cleanup
";

pub fn tc_port_workflow_store_001_名前は既定の拡張子で解決される(
    harness: &impl WorkflowStoreHarness,
) -> CaseOutcome {
    let expected = require!(harness.expected_path_for_name("impl"));
    require!(harness.put_named("impl", VALID));

    let loaded = expect_ok(harness.store().load(&name("impl")));

    assert_eq!(loaded.resolved_from(), expected);
    CaseOutcome::Ran
}

pub fn tc_port_workflow_store_002_名前は別の拡張子へフォールバックしない(
    harness: &impl WorkflowStoreHarness,
) -> CaseOutcome {
    let expected = require!(harness.expected_path_for_name("impl"));
    require!(harness.put_named_with_ext("impl", "yml", VALID));

    assert_eq!(
        expect_not_found(harness.store().load(&name("impl"))),
        expected
    );
    CaseOutcome::Ran
}

pub fn tc_port_workflow_store_003_名前の解決先が無ければ試みた絶対パスつきで不在になる(
    harness: &impl WorkflowStoreHarness,
) -> CaseOutcome {
    let expected = require!(harness.expected_path_for_name("missing"));

    let attempted = expect_not_found(harness.store().load(&name("missing")));

    assert_eq!(attempted, expected);
    assert!(attempted.is_absolute(), "案内に使える絶対パスを返す");
    CaseOutcome::Ran
}

pub fn tc_port_workflow_store_004_絶対パス指定はそのパスから読み込まれる(
    harness: &impl WorkflowStoreHarness,
) -> CaseOutcome {
    let path = require!(harness.put_at_absolute(VALID));

    let loaded = expect_ok(harness.store().load(&WorkflowRef::Path(path.clone())));

    assert_eq!(loaded.resolved_from(), path);
    CaseOutcome::Ran
}

pub fn tc_port_workflow_store_005_相対パス指定は基準ディレクトリから解決される(
    harness: &impl WorkflowStoreHarness,
) -> CaseOutcome {
    let (relative, absolute) = require!(harness.put_at_relative(VALID));

    let loaded = expect_ok(harness.store().load(&WorkflowRef::Path(relative)));

    assert_eq!(loaded.resolved_from(), absolute);
    CaseOutcome::Ran
}

pub fn tc_port_workflow_store_006_指定パスにファイルが無ければ不在になる(
    harness: &impl WorkflowStoreHarness,
) -> CaseOutcome {
    let path = require!(harness.missing_absolute_path());

    let attempted = expect_not_found(harness.store().load(&WorkflowRef::Path(path.clone())));

    assert_eq!(attempted, path);
    CaseOutcome::Ran
}

pub fn tc_port_workflow_store_007_workflowキーは宣言名として読み込まれる(
    harness: &impl WorkflowStoreHarness,
) -> CaseOutcome {
    require!(harness.put_named(
        "wf",
        "workflow: implement
initial: queued
statuses:
  queued:
    prompt: 実装して
    next: waiting
  waiting:
    run: wait
  done:
    run: cleanup
"
    ));

    let loaded = expect_ok(harness.store().load(&name("wf")));
    let parsed = loaded.parsed();
    let definition = parsed.definition();

    assert_eq!(
        parsed.declared_name().map(WorkflowName::as_str),
        Some("implement")
    );
    assert_eq!(definition.initial(), &status("queued"));
    assert_eq!(definition.statuses().len(), 3);
    match expect_status(definition, "queued") {
        StatusDefinition::AgentRun { input, next, .. } => {
            assert_eq!(input, &AgentInput::Prompt(prompt("実装して")));
            assert_eq!(next, &status("waiting"));
        }
        StatusDefinition::Wait | StatusDefinition::Cleanup => {
            panic!("prompt を持つステータスはエージェント実行として読み込まれる")
        }
    }
    assert_eq!(
        definition.status(&status("waiting")),
        Some(&StatusDefinition::Wait)
    );
    assert_eq!(
        definition.status(&status("done")),
        Some(&StatusDefinition::Cleanup)
    );
    CaseOutcome::Ran
}

pub fn tc_port_workflow_store_008_トップレベルのagentとmodelはワークフローデフォルトになる(
    harness: &impl WorkflowStoreHarness,
) -> CaseOutcome {
    require!(harness.put_named(
        "wf",
        "agent: claude
model: sonnet
initial: queued
statuses:
  queued:
    prompt: 実装して
    next: queued
"
    ));

    let loaded = expect_ok(harness.store().load(&name("wf")));
    let definition = loaded.parsed().definition();

    assert_eq!(
        definition.default_agent().map(AgentName::as_str),
        Some("claude")
    );
    assert_eq!(
        definition.default_model().map(ModelName::as_str),
        Some("sonnet")
    );
    CaseOutcome::Ran
}

pub fn tc_port_workflow_store_009_workflowキーが無ければ宣言名を持たない(
    harness: &impl WorkflowStoreHarness,
) -> CaseOutcome {
    require!(harness.put_named(
        "wf",
        "initial: queued
statuses:
  queued:
    prompt: 実装して
    next: queued
"
    ));

    let loaded = expect_ok(harness.store().load(&name("wf")));

    assert_eq!(loaded.parsed().declared_name(), None);
    CaseOutcome::Ran
}

pub fn tc_port_workflow_store_010_エージェント実行の全キーが対応する型に落ちる(
    harness: &impl WorkflowStoreHarness,
) -> CaseOutcome {
    require!(harness.put_named(
        "wf",
        "initial: reviewing
statuses:
  reviewing:
    skill: review
    agent: claude
    model: opus
    timeout: 30m
    retries: 5
    judge: check --json
    next: reviewing
"
    ));

    let loaded = expect_ok(harness.store().load(&name("wf")));
    let definition = loaded.parsed().definition();

    match expect_status(definition, "reviewing") {
        StatusDefinition::AgentRun {
            input,
            agent,
            model,
            timeout,
            retries,
            judge,
            next,
        } => {
            assert_eq!(
                input,
                &AgentInput::Skill(SkillName::parse("review".to_owned()).expect("受理される"))
            );
            assert_eq!(agent.as_ref().map(AgentName::as_str), Some("claude"));
            assert_eq!(model.as_ref().map(ModelName::as_str), Some("opus"));
            assert_eq!(
                timeout,
                &Some(TimeoutSpec::parse("30m").expect("受理される"))
            );
            assert_eq!(retries, &Some(5));
            assert_eq!(
                judge.as_ref().map(PlainCommand::tokens),
                Some(["check".to_owned(), "--json".to_owned()].as_slice())
            );
            assert_eq!(next, &status("reviewing"));
        }
        StatusDefinition::Wait | StatusDefinition::Cleanup => {
            panic!("エージェント実行として読み込まれる")
        }
    }
    CaseOutcome::Ran
}

pub fn tc_port_workflow_store_011_timeout_noneは無制限になる(
    harness: &impl WorkflowStoreHarness,
) -> CaseOutcome {
    require!(harness.put_named(
        "wf",
        "initial: queued
statuses:
  queued:
    prompt: 実装して
    timeout: none
    next: queued
"
    ));

    let loaded = expect_ok(harness.store().load(&name("wf")));

    assert_eq!(
        loaded
            .parsed()
            .definition()
            .effective_timeout(&status("queued")),
        TimeoutSpec::Unlimited
    );
    CaseOutcome::Ran
}

pub fn tc_port_workflow_store_012_retriesの0は正当な値として受理される(
    harness: &impl WorkflowStoreHarness,
) -> CaseOutcome {
    require!(harness.put_named(
        "wf",
        "initial: queued
statuses:
  queued:
    prompt: 実装して
    retries: 0
    next: queued
"
    ));

    let loaded = expect_ok(harness.store().load(&name("wf")));

    assert_eq!(
        loaded
            .parsed()
            .definition()
            .effective_retry_limit(&status("queued")),
        0
    );
    CaseOutcome::Ran
}

pub fn tc_port_workflow_store_013_自己参照と循環は受理される(
    harness: &impl WorkflowStoreHarness,
) -> CaseOutcome {
    require!(harness.put_named(
        "wf",
        "initial: a
statuses:
  a:
    prompt: 実装して
    next: b
  b:
    prompt: 見直して
    next: a
  self:
    prompt: 繰り返して
    next: self
"
    ));

    let loaded = expect_ok(harness.store().load(&name("wf")));

    assert_eq!(loaded.parsed().definition().statuses().len(), 3);
    CaseOutcome::Ran
}

pub fn tc_port_workflow_store_014_到達不能ステータスは受理される(
    harness: &impl WorkflowStoreHarness,
) -> CaseOutcome {
    require!(harness.put_named(
        "wf",
        "initial: queued
statuses:
  queued:
    prompt: 実装して
    next: queued
  unreachable:
    run: wait
"
    ));

    let loaded = expect_ok(harness.store().load(&name("wf")));

    assert_eq!(
        loaded.parsed().definition().status(&status("unreachable")),
        Some(&StatusDefinition::Wait)
    );
    CaseOutcome::Ran
}

pub fn tc_port_workflow_store_015_judgeの波括弧は文字どおり保持される(
    harness: &impl WorkflowStoreHarness,
) -> CaseOutcome {
    require!(harness.put_named(
        "wf",
        "initial: queued
statuses:
  queued:
    prompt: 実装して
    judge: check {input} {workspace}
    next: queued
"
    ));

    let loaded = expect_ok(harness.store().load(&name("wf")));

    match expect_status(loaded.parsed().definition(), "queued") {
        StatusDefinition::AgentRun { judge, .. } => assert_eq!(
            judge.as_ref().map(PlainCommand::tokens),
            Some(
                [
                    "check".to_owned(),
                    "{input}".to_owned(),
                    "{workspace}".to_owned()
                ]
                .as_slice()
            )
        ),
        StatusDefinition::Wait | StatusDefinition::Cleanup => {
            panic!("エージェント実行として読み込まれる")
        }
    }
    CaseOutcome::Ran
}

pub fn tc_port_workflow_store_016_未定義のエージェント名を参照する定義も受理される(
    harness: &impl WorkflowStoreHarness,
) -> CaseOutcome {
    require!(harness.put_named(
        "wf",
        "agent: 定義されていないエージェント
initial: queued
statuses:
  queued:
    prompt: 実装して
    agent: これも定義されていない
    next: queued
"
    ));

    let loaded = expect_ok(harness.store().load(&name("wf")));

    assert_eq!(
        loaded
            .parsed()
            .definition()
            .default_agent()
            .map(AgentName::as_str),
        Some("定義されていないエージェント")
    );
    CaseOutcome::Ran
}

pub fn tc_port_workflow_store_017_構文エラーと重複キーは位置つきで拒否される(
    harness: &impl WorkflowStoreHarness,
) -> CaseOutcome {
    for text in [
        "initial: [\n",
        "initial: queued
initial: done
statuses:
  queued:
    prompt: 実装して
    next: queued
",
    ] {
        require!(harness.put_named("wf", text));

        match expect_parse_error(harness.store().load(&name("wf"))) {
            WorkflowParseError::YamlSyntax { message, location } => {
                assert!(!message.is_empty());
                assert!(location.is_some(), "テキスト上の位置を伴う");
            }
            other => panic!("YamlSyntax として拒否される: {other:?}"),
        }
    }
    CaseOutcome::Ran
}

pub fn tc_port_workflow_store_018_トップレベルの許容外キーは拒否される(
    harness: &impl WorkflowStoreHarness,
) -> CaseOutcome {
    require!(harness.put_named(
        "wf",
        "initail: queued
statuses:
  queued:
    prompt: 実装して
    next: queued
"
    ));

    let error = expect_parse_error(harness.store().load(&name("wf")));

    assert!(
        matches!(&error, WorkflowParseError::UnknownKey { key, .. } if key == "initail"),
        "UnknownKey として拒否される: {error:?}"
    );
    CaseOutcome::Ran
}

pub fn tc_port_workflow_store_019_ステータス内のスキーマ外キーは拒否される(
    harness: &impl WorkflowStoreHarness,
) -> CaseOutcome {
    require!(harness.put_named(
        "wf",
        "initial: queued
statuses:
  queued:
    prmopt: 実装して
    next: queued
"
    ));

    assert_eq!(
        expect_parse_error(harness.store().load(&name("wf"))),
        WorkflowParseError::UnknownKey {
            location: "statuses.queued".to_owned(),
            key: "prmopt".to_owned()
        }
    );
    CaseOutcome::Ran
}

pub fn tc_port_workflow_store_020_動作種別に無関係なキーは拒否される(
    harness: &impl WorkflowStoreHarness,
) -> CaseOutcome {
    for (key, text) in [
        (
            "judge",
            "initial: waiting
statuses:
  waiting:
    run: wait
    judge: check
",
        ),
        (
            "next",
            "initial: done
statuses:
  done:
    run: cleanup
    next: done
",
        ),
    ] {
        require!(harness.put_named("wf", text));

        let error = expect_parse_error(harness.store().load(&name("wf")));
        assert!(
            matches!(&error, WorkflowParseError::ForbiddenKey { key: found, .. } if found == key),
            "ForbiddenKey({key}) として拒否される: {error:?}"
        );
    }
    CaseOutcome::Ran
}

pub fn tc_port_workflow_store_021_initialが無ければ拒否される(
    harness: &impl WorkflowStoreHarness,
) -> CaseOutcome {
    require!(harness.put_named(
        "wf",
        "statuses:
  queued:
    prompt: 実装して
    next: queued
"
    ));

    assert_eq!(
        expect_parse_error(harness.store().load(&name("wf"))),
        WorkflowParseError::MissingInitial
    );
    CaseOutcome::Ran
}

pub fn tc_port_workflow_store_022_initialの参照先が無ければ拒否される(
    harness: &impl WorkflowStoreHarness,
) -> CaseOutcome {
    require!(harness.put_named(
        "wf",
        "initial: missing
statuses:
  queued:
    prompt: 実装して
    next: queued
"
    ));

    assert_eq!(
        expect_parse_error(harness.store().load(&name("wf"))),
        WorkflowParseError::InitialNotFound {
            initial: "missing".to_owned()
        }
    );
    CaseOutcome::Ran
}

pub fn tc_port_workflow_store_023_statusesが空または欠落なら拒否される(
    harness: &impl WorkflowStoreHarness,
) -> CaseOutcome {
    for text in ["initial: queued\nstatuses: {}\n", "initial: queued\n"] {
        require!(harness.put_named("wf", text));

        assert_eq!(
            expect_parse_error(harness.store().load(&name("wf"))),
            WorkflowParseError::EmptyStatuses
        );
    }
    CaseOutcome::Ran
}

pub fn tc_port_workflow_store_024_動作宣言の無いステータスは拒否される(
    harness: &impl WorkflowStoreHarness,
) -> CaseOutcome {
    require!(harness.put_named(
        "wf",
        "initial: queued
statuses:
  queued:
    agent: claude
    next: queued
"
    ));

    assert_eq!(
        expect_parse_error(harness.store().load(&name("wf"))),
        WorkflowParseError::NoAction {
            status: "queued".to_owned()
        }
    );
    CaseOutcome::Ran
}

pub fn tc_port_workflow_store_025_動作宣言が複数あるステータスは拒否される(
    harness: &impl WorkflowStoreHarness,
) -> CaseOutcome {
    require!(harness.put_named(
        "wf",
        "initial: queued
statuses:
  queued:
    prompt: 実装して
    skill: review
    next: queued
"
    ));

    assert_eq!(
        expect_parse_error(harness.store().load(&name("wf"))),
        WorkflowParseError::MultipleActions {
            status: "queued".to_owned(),
            keys: vec!["prompt".to_owned(), "skill".to_owned()]
        }
    );
    CaseOutcome::Ran
}

pub fn tc_port_workflow_store_026_run値がwaitでもcleanupでもなければ拒否される(
    harness: &impl WorkflowStoreHarness,
) -> CaseOutcome {
    require!(harness.put_named(
        "wf",
        "initial: queued
statuses:
  queued:
    run: sleep
"
    ));

    assert_eq!(
        expect_parse_error(harness.store().load(&name("wf"))),
        WorkflowParseError::UnknownRunValue {
            status: "queued".to_owned(),
            value: "sleep".to_owned()
        }
    );
    CaseOutcome::Ran
}

pub fn tc_port_workflow_store_027_エージェント実行にnextが無ければ拒否される(
    harness: &impl WorkflowStoreHarness,
) -> CaseOutcome {
    require!(harness.put_named(
        "wf",
        "initial: queued
statuses:
  queued:
    prompt: 実装して
"
    ));

    assert_eq!(
        expect_parse_error(harness.store().load(&name("wf"))),
        WorkflowParseError::MissingNext {
            status: "queued".to_owned()
        }
    );
    CaseOutcome::Ran
}

pub fn tc_port_workflow_store_028_nextの参照先が無ければ拒否される(
    harness: &impl WorkflowStoreHarness,
) -> CaseOutcome {
    require!(harness.put_named(
        "wf",
        "initial: queued
statuses:
  queued:
    prompt: 実装して
    next: nowhere
"
    ));

    assert_eq!(
        expect_parse_error(harness.store().load(&name("wf"))),
        WorkflowParseError::NextNotFound {
            status: "queued".to_owned(),
            next: "nowhere".to_owned()
        }
    );
    CaseOutcome::Ran
}

pub fn tc_port_workflow_store_029_値の生成エラーは論理パスつきで拒否される(
    harness: &impl WorkflowStoreHarness,
) -> CaseOutcome {
    for (location, text) in [
        (
            "statuses.queued.prompt",
            "initial: queued
statuses:
  queued:
    prompt: ''
    next: queued
",
        ),
        (
            "statuses.queued.timeout",
            "initial: queued
statuses:
  queued:
    prompt: 実装して
    timeout: 0s
    next: queued
",
        ),
        (
            "statuses.queued.judge",
            "initial: queued
statuses:
  queued:
    prompt: 実装して
    judge: ''
    next: queued
",
        ),
        (
            "statuses. queued",
            "initial: queued
statuses:
  ' queued':
    prompt: 実装して
    next: queued
",
        ),
    ] {
        require!(harness.put_named("wf", text));

        let error = expect_parse_error(harness.store().load(&name("wf")));
        assert!(
            matches!(&error, WorkflowParseError::InvalidValue { location: found, .. } if found == location),
            "InvalidValue({location}) として拒否される: {error:?}"
        );
    }
    CaseOutcome::Ran
}

pub fn tc_port_workflow_store_030_読み取れない定義は入出力エラーになる(
    harness: &impl WorkflowStoreHarness,
) -> CaseOutcome {
    require!(harness.put_named("wf", VALID));
    let _restore = require!(harness.make_unreadable("wf"));

    match harness.store().load(&name("wf")) {
        Ok(loaded) => panic!(
            "読み取れない定義は読み込めない: {}",
            loaded.resolved_from().display()
        ),
        Err(WorkflowLoadError::Io { message }) => assert!(!message.is_empty()),
        Err(WorkflowLoadError::NotFound { attempted }) => {
            panic!("存在するので NotFound ではない: {}", attempted.display())
        }
        Err(WorkflowLoadError::Parse(error)) => {
            panic!("読めないことはパースエラーではない: {error:?}")
        }
    }
    CaseOutcome::Ran
}

pub fn tc_port_workflow_store_031_loadは呼び出し時点の内容を返す(
    harness: &impl WorkflowStoreHarness,
) -> CaseOutcome {
    require!(harness.put_named("wf", VALID));
    let before = expect_ok(harness.store().load(&name("wf")));

    require!(harness.put_named(
        "wf",
        "workflow: rewritten
initial: reviewing
statuses:
  reviewing:
    prompt: 見直して
    next: reviewing
"
    ));
    let after = expect_ok(harness.store().load(&name("wf")));

    assert_eq!(
        before.parsed().declared_name().map(WorkflowName::as_str),
        Some("implement")
    );
    assert_eq!(
        after.parsed().declared_name().map(WorkflowName::as_str),
        Some("rewritten")
    );
    assert_eq!(after.parsed().definition().initial(), &status("reviewing"));
    CaseOutcome::Ran
}

fn name(value: &str) -> WorkflowRef {
    WorkflowRef::Name(
        WorkflowName::parse(value.to_owned()).expect("ワークフロー名として受理される"),
    )
}

fn status(value: &str) -> StatusName {
    StatusName::parse(value.to_owned()).expect("ステータス名として受理される")
}

fn prompt(value: &str) -> Prompt {
    Prompt::parse(value.to_owned()).expect("プロンプトとして受理される")
}

fn expect_status<'definition>(
    definition: &'definition WorkflowDefinition,
    name: &str,
) -> &'definition StatusDefinition {
    match definition.status(&status(name)) {
        Some(status) => status,
        None => panic!("定義したステータス {name} が読み込まれる"),
    }
}

fn expect_ok(result: Result<LoadedWorkflow, WorkflowLoadError>) -> LoadedWorkflow {
    match result {
        Ok(loaded) => loaded,
        Err(WorkflowLoadError::NotFound { attempted }) => {
            panic!("配置したのに NotFound: {}", attempted.display())
        }
        Err(WorkflowLoadError::Parse(error)) => {
            panic!("受理されるべき定義が拒否された: {error:?}")
        }
        Err(WorkflowLoadError::Io { message }) => panic!("読み込みに失敗した: {message}"),
    }
}

fn expect_not_found(result: Result<LoadedWorkflow, WorkflowLoadError>) -> PathBuf {
    match result {
        Ok(loaded) => panic!(
            "解決先が無いので読み込めない: {}",
            loaded.resolved_from().display()
        ),
        Err(WorkflowLoadError::NotFound { attempted }) => attempted,
        Err(WorkflowLoadError::Parse(error)) => panic!("不在はパースエラーではない: {error:?}"),
        Err(WorkflowLoadError::Io { message }) => panic!("不在は Io ではない: {message}"),
    }
}

fn expect_parse_error(result: Result<LoadedWorkflow, WorkflowLoadError>) -> WorkflowParseError {
    match result {
        Ok(loaded) => panic!(
            "拒否されるべき定義が受理された: {}",
            loaded.resolved_from().display()
        ),
        Err(WorkflowLoadError::Parse(error)) => error,
        Err(WorkflowLoadError::NotFound { attempted }) => {
            panic!("配置したのに NotFound: {}", attempted.display())
        }
        Err(WorkflowLoadError::Io { message }) => panic!("読めるはずなのに Io: {message}"),
    }
}

/// WorkflowStore の適合スイートをアダプターに適用する。
///
/// `$setup` はケースごとに評価され、ハーネスは共有されない。`$allowed_skips` は
/// この環境でスキップを許容するケース(TC ID)の集合で、集合の外のスキップはその
/// ケースの失敗になる。
#[macro_export]
macro_rules! workflow_store_conformance {
    ($setup:expr, $allowed_skips:expr) => {
        use $crate::workflow_store as __pulsen_conformance_workflow_store;

        $crate::conformance_cases!(
            __pulsen_conformance_workflow_store,
            $setup,
            __PULSEN_CONFORMANCE_WORKFLOW_STORE_SKIPS = $allowed_skips,
            [
                tc_port_workflow_store_001_名前は既定の拡張子で解決される,
                tc_port_workflow_store_002_名前は別の拡張子へフォールバックしない,
                tc_port_workflow_store_003_名前の解決先が無ければ試みた絶対パスつきで不在になる,
                tc_port_workflow_store_004_絶対パス指定はそのパスから読み込まれる,
                tc_port_workflow_store_005_相対パス指定は基準ディレクトリから解決される,
                tc_port_workflow_store_006_指定パスにファイルが無ければ不在になる,
                tc_port_workflow_store_007_workflowキーは宣言名として読み込まれる,
                tc_port_workflow_store_008_トップレベルのagentとmodelはワークフローデフォルトになる,
                tc_port_workflow_store_009_workflowキーが無ければ宣言名を持たない,
                tc_port_workflow_store_010_エージェント実行の全キーが対応する型に落ちる,
                tc_port_workflow_store_011_timeout_noneは無制限になる,
                tc_port_workflow_store_012_retriesの0は正当な値として受理される,
                tc_port_workflow_store_013_自己参照と循環は受理される,
                tc_port_workflow_store_014_到達不能ステータスは受理される,
                tc_port_workflow_store_015_judgeの波括弧は文字どおり保持される,
                tc_port_workflow_store_016_未定義のエージェント名を参照する定義も受理される,
                tc_port_workflow_store_017_構文エラーと重複キーは位置つきで拒否される,
                tc_port_workflow_store_018_トップレベルの許容外キーは拒否される,
                tc_port_workflow_store_019_ステータス内のスキーマ外キーは拒否される,
                tc_port_workflow_store_020_動作種別に無関係なキーは拒否される,
                tc_port_workflow_store_021_initialが無ければ拒否される,
                tc_port_workflow_store_022_initialの参照先が無ければ拒否される,
                tc_port_workflow_store_023_statusesが空または欠落なら拒否される,
                tc_port_workflow_store_024_動作宣言の無いステータスは拒否される,
                tc_port_workflow_store_025_動作宣言が複数あるステータスは拒否される,
                tc_port_workflow_store_026_run値がwaitでもcleanupでもなければ拒否される,
                tc_port_workflow_store_027_エージェント実行にnextが無ければ拒否される,
                tc_port_workflow_store_028_nextの参照先が無ければ拒否される,
                tc_port_workflow_store_029_値の生成エラーは論理パスつきで拒否される,
                tc_port_workflow_store_030_読み取れない定義は入出力エラーになる,
                tc_port_workflow_store_031_loadは呼び出し時点の内容を返す,
            ]
        );
    };
}
