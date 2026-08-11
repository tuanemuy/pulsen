//! ConfigStore の適合ケース(`spec/testcases/ports/config-store.md` の24行)。
//!
//! 1行 = 1ケース関数 = 1 `#[test]`。フィクスチャは `ConfigStoreHarness` のフックだけで
//! 組み立て、永続化技術には依存しない。
//!
//! `Invalid` に対して固定するのは「拒否されること」と、原因のキーがメッセージから辿れる
//! ことまでにする。文言そのものはアダプターが決めてよい。

use pulsen_domain::definition::{
    AgentName, ConfigLoadError, ConfigStore, DurationSpec, GlobalConfig, PlainCommand,
    RawAgentDefinition, SourceLocation,
};

use crate::{CaseOutcome, ConfigStoreHarness, require};

pub fn tc_port_config_store_001_全キーが反映される(
    harness: &impl ConfigStoreHarness,
) -> CaseOutcome {
    require!(harness.put_config(
        "agents:
  claude:
    cmd: claude -p {input}
    skill_input: /{skill}
notify_cmd: notify-send pulsen
judge_attempt_limit: 5
judge_timeout: 2m
spawn_fail_limit: 1
run_retention: 7d
"
    ));

    let config = expect_ok(harness.store().load());

    let agent = expect_agent(&config, "claude");
    assert_eq!(agent.cmd().tokens(), ["claude", "-p", "{input}"]);
    assert_eq!(agent.skill_input(), Some("/{skill}"));
    assert_eq!(
        config.notify_cmd().map(PlainCommand::tokens),
        Some(["notify-send".to_owned(), "pulsen".to_owned()].as_slice())
    );
    assert_eq!(config.judge_attempt_limit(), 5);
    assert_eq!(config.judge_timeout().seconds(), 120);
    assert_eq!(config.spawn_fail_limit(), 1);
    assert_eq!(
        config.run_retention().map(|spec| spec.seconds()),
        Some(604_800)
    );
    CaseOutcome::Ran
}

pub fn tc_port_config_store_002_空マッピングは全デフォルトになる(
    harness: &impl ConfigStoreHarness,
) -> CaseOutcome {
    require!(harness.put_config("{}\n"));

    assert_all_defaults(&expect_ok(harness.store().load()));
    CaseOutcome::Ran
}

pub fn tc_port_config_store_003_空ドキュメントは全デフォルトになる(
    harness: &impl ConfigStoreHarness,
) -> CaseOutcome {
    for text in ["", "null\n"] {
        require!(harness.put_config(text));

        assert_all_defaults(&expect_ok(harness.store().load()));
    }
    CaseOutcome::Ran
}

pub fn tc_port_config_store_004_記述したキーだけが既定値を上書きする(
    harness: &impl ConfigStoreHarness,
) -> CaseOutcome {
    require!(harness.put_config("spawn_fail_limit: 5\n"));

    let config = expect_ok(harness.store().load());

    assert_eq!(config.spawn_fail_limit(), 5);
    assert!(config.agents().is_empty());
    assert_eq!(config.notify_cmd(), None);
    assert_eq!(config.judge_attempt_limit(), 3);
    assert_eq!(config.judge_timeout().seconds(), 60);
    assert_eq!(config.run_retention(), None);
    CaseOutcome::Ran
}

pub fn tc_port_config_store_005_単位の異なる等価な期間は同じ秒数になる(
    harness: &impl ConfigStoreHarness,
) -> CaseOutcome {
    require!(harness.put_config("judge_timeout: 1m\n"));

    let config = expect_ok(harness.store().load());

    assert_eq!(config.judge_timeout().seconds(), 60);
    assert_eq!(
        Some(config.judge_timeout()),
        DurationSpec::parse("60s").ok()
    );
    CaseOutcome::Ran
}

pub fn tc_port_config_store_006_文字列形式のcmdは空白で分割される(
    harness: &impl ConfigStoreHarness,
) -> CaseOutcome {
    require!(harness.put_config(
        "agents:
  shell:
    cmd: 'sh  -c \"echo hi\"'
"
    ));

    let config = expect_ok(harness.store().load());

    assert_eq!(
        expect_agent(&config, "shell").cmd().tokens(),
        ["sh", "-c", "\"echo", "hi\""]
    );
    CaseOutcome::Ran
}

pub fn tc_port_config_store_007_配列形式のcmdは要素がそのままトークンになる(
    harness: &impl ConfigStoreHarness,
) -> CaseOutcome {
    require!(harness.put_config(
        "agents:
  shell:
    cmd: [\"sh\", \"-c\", \"echo hi\", \"\"]
"
    ));

    let config = expect_ok(harness.store().load());

    assert_eq!(
        expect_agent(&config, "shell").cmd().tokens(),
        ["sh", "-c", "echo hi", ""]
    );
    CaseOutcome::Ran
}

pub fn tc_port_config_store_008_notify_cmdは文字列形式でも配列形式でもトークン列になる(
    harness: &impl ConfigStoreHarness,
) -> CaseOutcome {
    require!(harness.put_config("notify_cmd: notify-send pulsen\n"));
    let from_text = expect_ok(harness.store().load());

    require!(harness.put_config("notify_cmd: [\"notify-send\", \"pulsen\"]\n"));
    let from_tokens = expect_ok(harness.store().load());

    let expected = ["notify-send".to_owned(), "pulsen".to_owned()];
    assert_eq!(
        from_text.notify_cmd().map(PlainCommand::tokens),
        Some(expected.as_slice())
    );
    assert_eq!(
        from_tokens.notify_cmd().map(PlainCommand::tokens),
        Some(expected.as_slice())
    );
    CaseOutcome::Ran
}

pub fn tc_port_config_store_009_cmdの未知プレースホルダはloadで検証されない(
    harness: &impl ConfigStoreHarness,
) -> CaseOutcome {
    require!(harness.put_config(
        "agents:
  broken:
    cmd: claude {foo}
"
    ));

    let config = expect_ok(harness.store().load());
    let agent = expect_agent(&config, "broken");

    assert_eq!(agent.cmd().tokens(), ["claude", "{foo}"]);
    assert!(
        agent.parse().is_err(),
        "テンプレートの内容は参照時に検証される"
    );
    CaseOutcome::Ran
}

pub fn tc_port_config_store_010_cmdの波括弧不正はloadで検証されない(
    harness: &impl ConfigStoreHarness,
) -> CaseOutcome {
    for cmd in ["claude {input", "claude {}"] {
        require!(harness.put_config(&format!(
            "agents:
  broken:
    cmd: '{cmd}'
"
        )));

        let config = expect_ok(harness.store().load());
        assert!(expect_agent(&config, "broken").parse().is_err());
    }
    CaseOutcome::Ran
}

pub fn tc_port_config_store_011_skill_inputの他のプレースホルダはloadで検証されない(
    harness: &impl ConfigStoreHarness,
) -> CaseOutcome {
    require!(harness.put_config(
        "agents:
  broken:
    cmd: claude {input}
    skill_input: '{input}'
"
    ));

    let config = expect_ok(harness.store().load());
    let agent = expect_agent(&config, "broken");

    assert_eq!(agent.skill_input(), Some("{input}"));
    assert!(agent.parse().is_err());
    CaseOutcome::Ran
}

pub fn tc_port_config_store_012_notify_cmdの波括弧は文字どおり保持される(
    harness: &impl ConfigStoreHarness,
) -> CaseOutcome {
    require!(harness.put_config("notify_cmd: notify-send {task}\n"));

    let config = expect_ok(harness.store().load());

    assert_eq!(
        config.notify_cmd().map(PlainCommand::tokens),
        Some(["notify-send".to_owned(), "{task}".to_owned()].as_slice())
    );
    CaseOutcome::Ran
}

pub fn tc_port_config_store_013_設定が無ければ解決後のホームパスつきで不在になる(
    harness: &impl ConfigStoreHarness,
) -> CaseOutcome {
    let home = require!(harness.home_path());
    require!(harness.remove_config());

    match harness.store().load() {
        Ok(_) => panic!("config.yaml が無ければ読み込みは失敗する"),
        Err(ConfigLoadError::NotFound { home: reported }) => assert_eq!(reported, home),
        Err(ConfigLoadError::Invalid { message, .. }) => {
            panic!("不在は Invalid ではない: {message}")
        }
        Err(ConfigLoadError::Io { message }) => panic!("不在は Io ではない: {message}"),
    }
    CaseOutcome::Ran
}

pub fn tc_port_config_store_014_yaml構文エラーは位置つきで拒否される(
    harness: &impl ConfigStoreHarness,
) -> CaseOutcome {
    require!(harness.put_config("agents: [\n"));

    let (message, location) = expect_invalid(harness.store().load());

    assert!(!message.is_empty());
    match location {
        Some(SourceLocation { line, column }) => {
            assert!(line >= 1 && column >= 1, "1 始まりの位置を返す");
        }
        None => panic!("構文エラーはテキスト上の位置を伴う"),
    }
    CaseOutcome::Ran
}

pub fn tc_port_config_store_015_スキーマに無いトップレベルキーは拒否される(
    harness: &impl ConfigStoreHarness,
) -> CaseOutcome {
    require!(harness.put_config("run_retension: 7d\n"));

    let (message, _) = expect_invalid(harness.store().load());

    assert!(
        message.contains("run_retension"),
        "どのキーが未知かが分かる: {message}"
    );
    CaseOutcome::Ran
}

pub fn tc_port_config_store_016_組み込み定数に相当するキーは拒否される(
    harness: &impl ConfigStoreHarness,
) -> CaseOutcome {
    for text in ["retries: 2\n", "timeout: 1h\n"] {
        require!(harness.put_config(text));

        expect_invalid(harness.store().load());
    }
    CaseOutcome::Ran
}

pub fn tc_port_config_store_017_agentsエントリ内の未知キーは拒否される(
    harness: &impl ConfigStoreHarness,
) -> CaseOutcome {
    require!(harness.put_config(
        "agents:
  claude:
    cmd: claude {input}
    skil_input: /{skill}
"
    ));

    let (message, _) = expect_invalid(harness.store().load());

    assert!(message.contains("skil_input"), "{message}");
    CaseOutcome::Ran
}

pub fn tc_port_config_store_018_agentsエントリにcmdが無ければ拒否される(
    harness: &impl ConfigStoreHarness,
) -> CaseOutcome {
    require!(harness.put_config(
        "agents:
  claude:
    skill_input: /{skill}
"
    ));

    let (message, _) = expect_invalid(harness.store().load());

    assert!(message.contains("cmd"), "{message}");
    CaseOutcome::Ran
}

pub fn tc_port_config_store_019_型不一致は拒否される(
    harness: &impl ConfigStoreHarness,
) -> CaseOutcome {
    for text in [
        "judge_attempt_limit: たくさん\n",
        "agents:
  claude:
    cmd: 12
",
    ] {
        require!(harness.put_config(text));

        expect_invalid(harness.store().load());
    }
    CaseOutcome::Ran
}

pub fn tc_port_config_store_020_上限がゼロなら拒否される(
    harness: &impl ConfigStoreHarness,
) -> CaseOutcome {
    for text in ["judge_attempt_limit: 0\n", "spawn_fail_limit: 0\n"] {
        require!(harness.put_config(text));

        expect_invalid(harness.store().load());
    }
    CaseOutcome::Ran
}

pub fn tc_port_config_store_021_期間形式の不正は拒否される(
    harness: &impl ConfigStoreHarness,
) -> CaseOutcome {
    for text in [
        "judge_timeout: 0s\n",
        "judge_timeout: 5x\n",
        "judge_timeout: '60'\n",
        "judge_timeout: ' 5s'\n",
        "run_retention: 0s\n",
    ] {
        require!(harness.put_config(text));

        expect_invalid(harness.store().load());
    }
    CaseOutcome::Ran
}

pub fn tc_port_config_store_022_トークン0個のコマンドは拒否される(
    harness: &impl ConfigStoreHarness,
) -> CaseOutcome {
    for text in [
        "agents:
  shell:
    cmd: ''
",
        "agents:
  shell:
    cmd: []
",
        "notify_cmd: ''\n",
    ] {
        require!(harness.put_config(text));

        expect_invalid(harness.store().load());
    }
    CaseOutcome::Ran
}

pub fn tc_port_config_store_023_読み取れない設定は入出力エラーになる(
    harness: &impl ConfigStoreHarness,
) -> CaseOutcome {
    require!(harness.put_config("spawn_fail_limit: 5\n"));
    let _restore = require!(harness.make_unreadable());

    match harness.store().load() {
        Ok(_) => panic!("読み取れない config.yaml は読み込めない"),
        Err(ConfigLoadError::Io { message }) => assert!(!message.is_empty()),
        Err(ConfigLoadError::NotFound { home }) => {
            panic!("存在するので NotFound ではない: {}", home.display())
        }
        Err(ConfigLoadError::Invalid { message, .. }) => {
            panic!("読めないことは Invalid ではない: {message}")
        }
    }
    CaseOutcome::Ran
}

pub fn tc_port_config_store_024_loadは呼び出し時点の内容を返す(
    harness: &impl ConfigStoreHarness,
) -> CaseOutcome {
    require!(harness.put_config("spawn_fail_limit: 5\n"));
    let before = expect_ok(harness.store().load());

    require!(harness.put_config("spawn_fail_limit: 9\n"));
    let after = expect_ok(harness.store().load());

    assert_eq!(before.spawn_fail_limit(), 5);
    assert_eq!(after.spawn_fail_limit(), 9);
    CaseOutcome::Ran
}

fn expect_ok(result: Result<GlobalConfig, ConfigLoadError>) -> GlobalConfig {
    match result {
        Ok(config) => config,
        Err(ConfigLoadError::NotFound { home }) => {
            panic!("配置したのに NotFound: {}", home.display())
        }
        Err(ConfigLoadError::Invalid { message, location }) => {
            panic!("受理されるべき設定が拒否された: {message} {location:?}")
        }
        Err(ConfigLoadError::Io { message }) => panic!("読み込みに失敗した: {message}"),
    }
}

fn expect_invalid(
    result: Result<GlobalConfig, ConfigLoadError>,
) -> (String, Option<SourceLocation>) {
    match result {
        Ok(_) => panic!("構造エラーとして拒否されるべき設定が受理された"),
        Err(ConfigLoadError::Invalid { message, location }) => (message, location),
        Err(ConfigLoadError::NotFound { home }) => {
            panic!("配置したのに NotFound: {}", home.display())
        }
        Err(ConfigLoadError::Io { message }) => panic!("読めるはずなのに Io: {message}"),
    }
}

fn expect_agent<'config>(config: &'config GlobalConfig, name: &str) -> &'config RawAgentDefinition {
    let name = AgentName::parse(name.to_owned()).expect("エージェント名として受理される");
    match config.agent(&name) {
        Some(definition) => definition,
        None => panic!("定義したエージェントが読み込まれる"),
    }
}

fn assert_all_defaults(config: &GlobalConfig) {
    assert!(config.agents().is_empty());
    assert_eq!(config.notify_cmd(), None);
    assert_eq!(config.judge_attempt_limit(), 3);
    assert_eq!(config.judge_timeout().seconds(), 60);
    assert_eq!(config.spawn_fail_limit(), 3);
    assert_eq!(config.run_retention(), None);
}

/// ConfigStore の適合スイートをアダプターに適用する。
///
/// `$setup` はケースごとに評価され、ハーネスは共有されない。
#[macro_export]
macro_rules! config_store_conformance {
    ($setup:expr) => {
        use $crate::config_store as __pulsen_conformance_config_store;

        $crate::conformance_cases!(
            __pulsen_conformance_config_store,
            $setup,
            [
                tc_port_config_store_001_全キーが反映される,
                tc_port_config_store_002_空マッピングは全デフォルトになる,
                tc_port_config_store_003_空ドキュメントは全デフォルトになる,
                tc_port_config_store_004_記述したキーだけが既定値を上書きする,
                tc_port_config_store_005_単位の異なる等価な期間は同じ秒数になる,
                tc_port_config_store_006_文字列形式のcmdは空白で分割される,
                tc_port_config_store_007_配列形式のcmdは要素がそのままトークンになる,
                tc_port_config_store_008_notify_cmdは文字列形式でも配列形式でもトークン列になる,
                tc_port_config_store_009_cmdの未知プレースホルダはloadで検証されない,
                tc_port_config_store_010_cmdの波括弧不正はloadで検証されない,
                tc_port_config_store_011_skill_inputの他のプレースホルダはloadで検証されない,
                tc_port_config_store_012_notify_cmdの波括弧は文字どおり保持される,
                tc_port_config_store_013_設定が無ければ解決後のホームパスつきで不在になる,
                tc_port_config_store_014_yaml構文エラーは位置つきで拒否される,
                tc_port_config_store_015_スキーマに無いトップレベルキーは拒否される,
                tc_port_config_store_016_組み込み定数に相当するキーは拒否される,
                tc_port_config_store_017_agentsエントリ内の未知キーは拒否される,
                tc_port_config_store_018_agentsエントリにcmdが無ければ拒否される,
                tc_port_config_store_019_型不一致は拒否される,
                tc_port_config_store_020_上限がゼロなら拒否される,
                tc_port_config_store_021_期間形式の不正は拒否される,
                tc_port_config_store_022_トークン0個のコマンドは拒否される,
                tc_port_config_store_023_読み取れない設定は入出力エラーになる,
                tc_port_config_store_024_loadは呼び出し時点の内容を返す,
            ]
        );
    };
}
