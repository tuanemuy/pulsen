# Inventory — domain

生成元: spec/domains/(最終同期: 2026-08-16)

| ID | 要素 | 定義場所 | 実装されるべき振る舞いの要点 |
|----|------|---------|------------------------------|
| DOM-definition-001 | AgentName 値オブジェクト | spec/domains/definition.md#名前系文字列-newtype | 非空・前後空白なしの文字列newtypeとして`parse`経由でのみ生成し、違反は`NameError`を返す |
| DOM-definition-002 | ModelName 値オブジェクト | spec/domains/definition.md#名前系文字列-newtype | AgentNameと同じ非空・前後空白なし制約を`parse`で検証する |
| DOM-definition-003 | SkillName 値オブジェクト | spec/domains/definition.md#名前系文字列-newtype | 同上の非空・前後空白なし制約を`parse`で検証する |
| DOM-definition-004 | Prompt 値オブジェクト | spec/domains/definition.md#名前系文字列-newtype | 非空制約のみを`parse`で検証する(前後空白は許容) |
| DOM-definition-005 | StatusName 値オブジェクト | spec/domains/definition.md#名前系文字列-newtype | CLI引数・YAMLキーとして使われるため非空・前後空白なしを`parse`で検証する |
| DOM-definition-006 | WorkflowName 値オブジェクト | spec/domains/definition.md#名前系文字列-newtype | 非空・前後空白なしを`parse`で検証する |
| DOM-definition-007 | InputText 値オブジェクト | spec/domains/definition.md#名前系文字列-newtype | 制約なしの文字列newtypeとして`{input}`へ渡る展開結果を保持する。制約がないため`parse`ではなく総関数`new(s)->Self`で生成する(`Err`になる経路のない`Result`を呼び出し側に持ち込まない) |
| DOM-definition-008 | NameError エラー型 | spec/domains/definition.md#名前系文字列-newtype | `Empty`と`SurroundingWhitespace`を名前系VO共通の`parse`失敗として区別する |
| DOM-definition-009 | DurationSpec 値オブジェクト | spec/domains/definition.md#durationspec | `<正の10進整数><s/m/h/d>`のみ受理し秒数に正規化、`1m`と`60s`が等価になる |
| DOM-definition-010 | DurationError エラー型 | spec/domains/definition.md#durationspec | `InvalidFormat{given}`と`Zero`(`0s`等)を区別する |
| DOM-definition-011 | TimeoutSpec 値オブジェクト | spec/domains/definition.md#timeoutspec | 期間文字列を`Limited(DurationSpec)`に、`none`を`Unlimited`に変換し、それ以外はエラーにする直和型 |
| DOM-definition-012 | RawCommand 値オブジェクト | spec/domains/definition.md#rawcommand構造解釈済みトークン列 | 文字列形式は空白分割(連続空白は1区切り)、配列形式は要素そのまま(空文字列トークン許容)をトークン列にし、0トークンは`CommandError::Empty`を返す |
| DOM-definition-013 | CommandError エラー型 | spec/domains/definition.md#rawcommand構造解釈済みトークン列 | トークン0個(空文字列・空配列)を`Empty`として返す |
| DOM-definition-014 | PlainCommand 値オブジェクト | spec/domains/definition.md#plaincommand | RawCommandと同じ生成規則でjudge/notify_cmd用トークン列を保持し、`{...}`を含んでもプレースホルダ検査をしない |
| DOM-definition-015 | AgentInput 値オブジェクト | spec/domains/definition.md#agentinput | `Prompt(Prompt)`/`Skill(SkillName)`の排他的直和型でステータスの入力を表現する |
| DOM-definition-016 | Placeholder 値オブジェクト | spec/domains/definition.md#placeholder | `input`/`model`/`workspace`/`skill`のみを`parse`で受理し、それ以外は`UnknownPlaceholder`を返す |
| DOM-definition-017 | CommandTemplate 値オブジェクト | spec/domains/definition.md#commandtemplate | `{`から次の`}`までをプレースホルダ名として`allowed`集合で検証し、未知名は`UnknownPlaceholder`、閉じ`}`欠落・空`{}`は`MalformedBrace`として`parse`で拒否する |
| DOM-definition-018 | TemplateError エラー型 | spec/domains/definition.md#commandtemplate | `UnknownPlaceholder{token,name}`と`MalformedBrace{token}`をCommandTemplate/SkillInputTemplateの`parse`失敗として区別する |
| DOM-definition-019 | CommandTemplate.placeholders ドメイン関数 | spec/domains/definition.md#commandtemplate | テンプレートが参照するプレースホルダ集合を`BTreeSet<Placeholder>`として返す |
| DOM-definition-020 | CommandTemplate.expand ドメイン関数 | spec/domains/definition.md#commandtemplate | トークン単位で`Hole`を`PlaceholderValues`の値へ1パスで置換し、値がなければ`ExpansionError::MissingValue`を返し置換後の値は再走査しない |
| DOM-definition-021 | PlaceholderValues 値オブジェクト | spec/domains/definition.md#commandtemplate | `input`/`model`/`workspace`(素の`PathBuf`)を保持し`skill`値は持たない構造体としてexpandへ渡される |
| DOM-definition-022 | ExpansionError エラー型 | spec/domains/definition.md#commandtemplate | 参照プレースホルダに値がない場合`MissingValue(placeholder)`を返す |
| DOM-definition-023 | CommandLine 値オブジェクト | spec/domains/definition.md#commandline | 1トークン以上のトークン列(先頭がプログラム)。生成は`CommandTemplate::expand`の結果とプロセス境界からの`rehydrate`の2経路で、どちらも同じ不変条件を通る |
| DOM-definition-024 | RawAgentDefinition 値オブジェクト | spec/domains/definition.md#rawagentdefinition-と-agentdefinition | `cmd`(RawCommand)必須・`skill_input`(生文字列テンプレート)任意として読み込み時は構造のみ保持し内容検証を行わない |
| DOM-definition-025 | RawAgentDefinition.parse ドメイン関数 | spec/domains/definition.md#rawagentdefinition-と-agentdefinition | 参照時検証の境界として`cmd`を許容プレースホルダ`{input,model,workspace}`で、`skill_input`を`{skill}`のみで検証し`AgentDefinition`を返す |
| DOM-definition-026 | AgentDefinition 値オブジェクト | spec/domains/definition.md#rawagentdefinition-と-agentdefinition | 検証済み`CommandTemplate`と任意の`SkillInputTemplate`を保持する |
| DOM-definition-027 | AgentDefError エラー型 | spec/domains/definition.md#rawagentdefinition-と-agentdefinition | `InvalidCmd`/`InvalidSkillInput`/`MissingSkillInput`の3種を区別する |
| DOM-definition-028 | AgentDefinition.render_input ドメイン関数 | spec/domains/definition.md#rawagentdefinition-と-agentdefinition | `Prompt`はそのまま`InputText`へ、`Skill`は`skill_input`で変換し未定義なら`MissingSkillInput`を返す |
| DOM-definition-029 | AgentDefinition.build_command_line ドメイン関数 | spec/domains/definition.md#rawagentdefinition-と-agentdefinition | `PlaceholderValues`を組み立てて`cmd.expand`を呼び、テンプレートが参照しない値は無視して展開する |
| DOM-definition-030 | SkillInputTemplate 値オブジェクト | spec/domains/definition.md#skillinputtemplate | CommandTemplateと同じ波括弧規則で`{skill}`のみを許容プレースホルダとして`parse`する |
| DOM-definition-031 | SkillInputTemplate.render ドメイン関数 | spec/domains/definition.md#skillinputtemplate | `SkillName`を`InputText`へ展開する |
| DOM-definition-032 | GlobalConfig 値オブジェクト | spec/domains/definition.md#globalconfig | 全キー任意・空ドキュメントは全デフォルトで`Ok`、未知キーは構造エラーとし、`agents`/`notify_cmd`等の既定値を保持する |
| DOM-definition-033 | WorkflowDefinition 値オブジェクト | spec/domains/definition.md#workflowdefinition | `initial∈statuses`と全`AgentRun.next∈statuses`を不変条件として保持する(ワークフロー名は保持しない) |
| DOM-definition-034 | WorkflowDefinition.status ドメイン関数 | spec/domains/definition.md#workflowdefinition | 指定`StatusName`の`StatusDefinition`を`statuses`から引く |
| DOM-definition-035 | WorkflowDefinition.effective_agent ドメイン関数 | spec/domains/definition.md#workflowdefinition | ステータス上書き値を`default_agent`より優先して実効エージェントを解決する |
| DOM-definition-036 | WorkflowDefinition.effective_model ドメイン関数 | spec/domains/definition.md#workflowdefinition | ステータス上書き値を`default_model`より優先して実効モデルを解決する |
| DOM-definition-037 | WorkflowDefinition.effective_timeout ドメイン関数 | spec/domains/definition.md#workflowdefinition | ステータス指定値を組み込みデフォルト`Limited(1h)`より優先して実効timeoutを解決する |
| DOM-definition-038 | WorkflowDefinition.effective_retry_limit ドメイン関数 | spec/domains/definition.md#workflowdefinition | AgentRunは`retries`>組み込みデフォルト2、Cleanupは常に2、Waitは適用対象外として実効リトライ上限を解決する |
| DOM-definition-039 | StatusDefinition 値オブジェクト | spec/domains/definition.md#statusdefinition直和型 | `AgentRun{input,agent,model,timeout,retries,judge,next}`/`Wait`/`Cleanup`の直和型で、動作種別ごとに許可キーが排他になる |
| DOM-definition-040 | WorkflowParseError エラー型 | spec/domains/definition.md#workflowparseerror登録時パースエラーの列挙 | `YamlSyntax`/`UnknownKey`(アダプター生成)と`ForbiddenKey`/`MissingInitial`/`InitialNotFound`/`EmptyStatuses`/`NoAction`/`MultipleActions`/`UnknownRunValue`/`MissingNext`/`NextNotFound`/`InvalidValue`(WorkflowAssembler生成)の12種を区別する |
| DOM-definition-041 | WorkflowRef 値オブジェクト | spec/domains/definition.md#workflowref | `/`区切り文字を含むか`.yaml`/`.yml`で終わる値を`Path`、それ以外を`Name`とファイル存在に依存せず決定的に`parse`する直和型 |
| DOM-definition-042 | WorkflowRef.display_name ドメイン関数 | spec/domains/definition.md#workflowref | `Name(n)`は`workflow:`キーを使わず`n`を、`Path(p)`は`declared`優先・なければファイル名から拡張子除去した値を表示名として決定する |
| DOM-definition-043 | WorkflowSnapshot 値オブジェクト | spec/domains/definition.md#workflowsnapshot | 登録時検証済み`WorkflowDefinition`を包む不変newtypeで、登録後のconfig.yaml編集との再突き合わせは行わない |
| DOM-definition-044 | WorkflowSnapshot.rehydrate ドメイン関数 | spec/domains/definition.md#workflowsnapshot | TaskRepositoryアダプターのデコード時専用の再構築経路として、構造検証済みの`WorkflowDefinition`からconfigとの再突き合わせなしに`WorkflowSnapshot`を組み立てる |
| DOM-definition-045 | WorkflowAssembler ドメインサービス | spec/domains/definition.md#workflowassembler | `assemble(RawWorkflowDoc)->Result<ParsedWorkflow,WorkflowParseError>`として動作宣言の排他性・`ForbiddenKey`・initial/next参照整合・値の生成エラーを純粋関数で全件検証する |
| DOM-definition-046 | RawWorkflowDoc 値オブジェクト | spec/domains/definition.md#workflowassembler | `declared_name`/`default_agent`/`default_model`/`initial`/`statuses`(未パース文字列群)を保持し、YAML構文解析後のドメイン側入力DTOとしてassembleへ渡される |
| DOM-definition-047 | RawStatusDoc 値オブジェクト | spec/domains/definition.md#workflowassembler | `prompt`/`skill`/`run`/`agent`/`model`/`timeout`/`retries`/`judge`/`next`の未パースフィールド全てを保持し`ForbiddenKey`検出を可能にする |
| DOM-definition-048 | ParsedWorkflow 値オブジェクト | spec/domains/definition.md#workflowassembler | `declared_name`と`definition`を保持し、表示名への採否は関知しない出力型 |
| DOM-definition-049 | RegistrationValidator ドメインサービス | spec/domains/definition.md#registrationvalidator | `validate(def,config)->Result<WorkflowSnapshot,Vec<RegistrationError>>`として全AgentRunステータスの実効エージェント解決・config.agents存在・テンプレート検証・skill_input要否・model要否を最初の1件で打ち切らず全件検証する。`status`を持たない`UnknownAgent`/`InvalidAgentDefinition`はエージェント単位の誤りであり同値の重複を1件にまとめ、`status`を持つ3種は各ステータス分を積む |
| DOM-definition-050 | RegistrationError エラー型 | spec/domains/definition.md#registrationvalidator | `MissingAgent`/`UnknownAgent`/`InvalidAgentDefinition`/`MissingSkillInput`/`MissingModel`の5種を区別する |
| DOM-definition-051 | ConfigStore.load ポートメソッド | spec/domains/definition.md#configstore | 読み取り専用・ロック不要・キャッシュなしで構造検証済み`GlobalConfig`を返し、`NotFound`/`Invalid`/`Io`を区別する契約 |
| DOM-definition-052 | WorkflowStore.load ポートメソッド | spec/domains/definition.md#workflowstore | `Name`は`<home>/workflows/<n>.yaml`固定解決(`.yml`フォールバックなし)、`Path`はカレントディレクトリから解決し`LoadedWorkflow{parsed,resolved_from}`を返す契約。失敗時も解決先の案内は構造化フィールド(`NotFound{attempted}`/`Parse{resolved_from}`)で示し、自由形式メッセージへの前置は`Io{message}`にのみ残す |
| DOM-task-001 | TaskId 値オブジェクト | spec/domains/task.md#taskid | 1〜64文字・`[a-z0-9-]`・先頭英数字の制約を`parse`で検証し、ファイル名・gitブランチ名として常に安全な集合に制限する |
| DOM-task-002 | TaskIdError エラー型 | spec/domains/task.md#taskid | `Empty`/`TooLong`/`InvalidChar{char,position}`/`InvalidLeadingChar`の4種を区別する |
| DOM-task-003 | RepoPath 値オブジェクト | spec/domains/task.md#repopath-worktreepath | 絶対パスであることを検証する(相対パスは受理しない) |
| DOM-task-004 | WorktreePath 値オブジェクト | spec/domains/task.md#repopath-worktreepath | RepoPathと同じ絶対パス制約を独立した型として検証する |
| DOM-task-005 | BranchName 値オブジェクト | spec/domains/task.md#branchname | 非空・空白/制御文字なし・先頭`-`不可・`..`不可・`/`始端終端不可・`.lock`終端不可の実用サブセットを検証する |
| DOM-task-006 | Workspace 値オブジェクト | spec/domains/task.md#workspace | `path`(WorktreePath)と`branch`(BranchName)の組として両フィールド一致で等価性を持つ |
| DOM-task-007 | Timestamp 値オブジェクト | spec/domains/task.md#timestamp | UTC秒精度の日時をClockまたはRFC3339パースで生成し全順序比較を持つ |
| DOM-task-008 | Timestamp.elapsed_since ドメイン関数 | spec/domains/task.md#timestamp | 2時刻間の経過秒数(DurationSpec相当)を返す |
| DOM-task-009 | AttemptNumber 値オブジェクト | spec/domains/task.md#attemptnumber | 1以上のu32として検証する |
| DOM-task-010 | AttemptNumber.next ドメイン関数 | spec/domains/task.md#attemptnumber | +1した新しい番号を返し、launching記録のたびに単調増加し過去番号を再利用しない |
| DOM-task-011 | StateRoot 値オブジェクト | spec/domains/task.md#stateroot-worktreeroot | グローバルホーム配下`state/`を指す絶対パスnewtype |
| DOM-task-012 | WorktreeRoot 値オブジェクト | spec/domains/task.md#stateroot-worktreeroot | `worktrees/`を指す絶対パスnewtype |
| DOM-task-013 | RunDirPath 値オブジェクト | spec/domains/task.md#rundirpath | `derive(state_root,id,n)`で`<state_root>/runs/<task-id>/attempt-<n>`を決定的に導出し、導出結果をタスクファイルにも記録する。逆写像`state_root()`は`derive`との一致を条件にのみ`StateRoot`を復元する |
| DOM-task-014 | TaskFilePath.active ドメイン関数 | spec/domains/task.md#taskfilepath | `<state_root>/tasks/<task-id>.json`を決定的に導出する |
| DOM-task-015 | TaskFilePath.archived ドメイン関数 | spec/domains/task.md#taskfilepath | `<state_root>/archive/<task-id>.json`を決定的に導出する |
| DOM-task-016 | Pid 値オブジェクト | spec/domains/task.md#プロセス同定系 | u32のプロセスID newtype |
| DOM-task-017 | KillIdent 値オブジェクト | spec/domains/task.md#プロセス同定系 | 非空のプラットフォーム定義kill同定子(文字列一致で等価性判定) |
| DOM-task-018 | ProcessStartTime 値オブジェクト | spec/domains/task.md#プロセス同定系 | 非空の不透明な起動時刻表現で等価比較のみに用い時間演算しない |
| DOM-task-019 | StartTimeRecord 値オブジェクト | spec/domains/task.md#プロセス同定系 | `ident`(照合用)と`wall`(壁時計)の両方を保持しPID再利用照合とtimeout起点の両要求を満たす |
| DOM-task-020 | ProcessIdent 値オブジェクト | spec/domains/task.md#プロセス同定系 | `pid`/`kill_ident`/`starttime`一式をrunning遷移時に取り込み、以降のkillをこの値のみで実行可能にする |
| DOM-task-021 | AttemptRef 値オブジェクト | spec/domains/task.md#attemptref | `number`/`run_dir`/`process`を保持し、launching記録でのみ丸ごと置換され他のどの遷移でもクリアされない |
| DOM-task-022 | RetryCounters 値オブジェクト | spec/domains/task.md#retrycounters | `attempt_count`/`judge_attempt_count`/`spawn_fail_count`(初期値0)を連続失敗数として保持する |
| DOM-task-023 | FailureNote 値オブジェクト | spec/domains/task.md#failurenote | `kind`/`message`(非空)/`at`を保持し、成功時にクリアせず直近の失敗のみ上書きする |
| DOM-task-024 | FailureKind 値オブジェクト | spec/domains/task.md#failurenote | `WorktreeCreate`/`WorktreeRemove`/`ArchiveMove`/`SpawnFail`/`JudgeFail`の5種の直和型。うちツール操作の3種は`ToolFailureKind`として取り出され、記録時に`FailureKind`へ写される |
| DOM-task-025 | StopReason 値オブジェクト | spec/domains/task.md#stopreason | `RetryLimitExceeded`/`JudgeLimitExceeded`/`SpawnFailLimitExceeded`/`Aborted`のstoppedへ至る4経路と1対1対応する |
| DOM-task-026 | ExecutionStateKind 値オブジェクト | spec/domains/task.md#executionstatekind | `ls --state`の入力として小文字6値のみを`parse`で受理する判別子 |
| DOM-task-027 | StateKindError エラー型 | spec/domains/task.md#executionstatekind | `Unknown{given,valid}`として有効値一覧を案内に含めて返す |
| DOM-task-028 | Task エンティティ(集約ルート) | spec/domains/task.md#task集約ルート | `id`/`workflow_name`/`target`/`snapshot`/`task_status`/`execution`/`workspace`/`current_attempt`/`counters`/`last_failure`/`updated_at`を保持し不変条件1〜8を常に満たす |
| DOM-task-029 | ExecutionState 値オブジェクト | spec/domains/task.md#executionstate直和型 | `Pending`/`Launching{recorded_at}`/`Running`/`Completed`/`Failed`/`Stopped{reason,notified_at}`の6状態を型で表現し`Stopped`以外は`notified_at`を持てない |
| DOM-task-030 | Task.register ドメイン関数 | spec/domains/task.md#振る舞い遷移関数 | `task_status=snapshot.initial`・`Pending`・workspace/attempt/failure=None・カウンタ全0でタスクを新規生成する |
| DOM-task-031 | Task.rehydrate ドメイン関数 | spec/domains/task.md#振る舞い遷移関数 | 永続化からの唯一の再構築経路として不変条件1(`task_status∈snapshot.statuses`)を検証し、破れは`RehydrateError`を返す |
| DOM-task-032 | RehydrateError エラー型 | spec/domains/task.md#振る舞い遷移関数 | `StatusNotInSnapshot`をアダプターが`SnapshotUnreadable`へ写像する根拠として返す |
| DOM-task-033 | Task.confirm_workspace ドメイン関数 | spec/domains/task.md#振る舞い遷移関数 | `workspace=None`前提でworkspaceを確定し、既にSomeなら再確定せずエラーを返す |
| DOM-task-034 | Task.record_launching ドメイン関数 | spec/domains/task.md#振る舞い遷移関数 | `Pending/Failed`・AgentRun・workspace確定済み前提で次番号を採番し`RunDirPath::derive`で導出したrun_dirを`current_attempt`に設定して`Launching{recorded_at:now}`にする |
| DOM-task-035 | Task.confirm_running ドメイン関数 | spec/domains/task.md#振る舞い遷移関数 | `Launching`前提で`Running`にし`current_attempt.process`を設定・`spawn_fail_count=0`にリセットする |
| DOM-task-036 | Task.record_spawn_failure ドメイン関数 | spec/domains/task.md#振る舞い遷移関数 | `Launching`前提の非同期猶予超過経路で`spawn_fail_count+=1`し超過なら`Stopped{SpawnFailLimitExceeded}`、でなければ`Pending`に戻す |
| DOM-task-037 | Task.record_spawn_failure_in_place ドメイン関数 | spec/domains/task.md#振る舞い遷移関数 | `Pending/Failed`前提のテンプレート展開失敗の同期経路で状態を変えずattempt採番もせず`spawn_fail_count+=1`する |
| DOM-task-038 | Task.complete_run ドメイン関数 | spec/domains/task.md#振る舞い遷移関数 | `Running`前提で判定completedを`Completed`に反映しattempt_count/judge_attempt_countを0にリセットする |
| DOM-task-039 | Task.skip_run ドメイン関数 | spec/domains/task.md#振る舞い遷移関数 | `Running`前提で判定skippedをタスクステータス不変のまま`Pending`に戻しカウンタを0にリセットする |
| DOM-task-040 | Task.fail_run ドメイン関数 | spec/domains/task.md#振る舞い遷移関数 | `Running`前提で`attempt_count+=1`・`judge_attempt_count=0`し超過なら`Stopped{RetryLimitExceeded}`、でなければ`Failed`にする |
| DOM-task-041 | Task.record_judge_failure ドメイン関数 | spec/domains/task.md#振る舞い遷移関数 | `Running`前提で`judge_attempt_count+=1`・`last_failure=JudgeFail`とし超過なら`Stopped{JudgeLimitExceeded}`、でなければ`Running`を維持する |
| DOM-task-042 | Task.record_tool_failure ドメイン関数 | spec/domains/task.md#振る舞い遷移関数 | `Pending/Failed`前提でworktree作成/削除・アーカイブ移動失敗を`attempt_count+=1`で記録し超過なら`Stopped{RetryLimitExceeded}`、でなければ`Failed`にする。`kind`は`ToolFailureKind`だけを受け取り、`SpawnFail`/`JudgeFail`を型で排除する |
| DOM-task-043 | Task.advance ドメイン関数 | spec/domains/task.md#振る舞い遷移関数 | `Completed`かつAgentRun前提で`task_status`をnextへ進め`Pending`に戻す |
| DOM-task-044 | Task.abort ドメイン関数 | spec/domains/task.md#振る舞い遷移関数 | `Stopped`以外前提で`Stopped{Aborted,notified_at:None}`にする(kill成否確認は呼び出し側の責務) |
| DOM-task-045 | Task.mark_notified ドメイン関数 | spec/domains/task.md#振る舞い遷移関数 | `Stopped{notified_at:None}`前提で`notified_at=Some(now)`にする |
| DOM-task-046 | Task.retry ドメイン関数 | spec/domains/task.md#振る舞い遷移関数 | `Stopped`前提で`Pending`・カウンタ全0にし、それ以外の状態は`RetryError::NotStopped`を返す |
| DOM-task-047 | Task.set_status ドメイン関数 | spec/domains/task.md#振る舞い遷移関数 | `Launching/Running`以外前提でstatusがsnapshotに存在することを検証し`task_status`変更・`Pending`・カウンタ全0にする(`Launching/Running`は`SetStatusError::Active`) |
| DOM-task-048 | Task.execution_kind ドメイン関数 | spec/domains/task.md#問い合わせ読み取り | 実行状態の判別子`ExecutionStateKind`を返す |
| DOM-task-049 | Task.current_status_def ドメイン関数 | spec/domains/task.md#問い合わせ読み取り | snapshotから現ステータスの定義を引く全域関数(不変条件1により常に存在) |
| DOM-task-050 | Task.next_attempt_number ドメイン関数 | spec/domains/task.md#問い合わせ読み取り | `current_attempt`の次番号(Noneなら1)を返す |
| DOM-task-051 | Task.is_agent_run/is_wait/is_cleanup ドメイン関数 | spec/domains/task.md#問い合わせ読み取り | 現ステータスの動作種別をbool判定として返す3メソッド |
| DOM-task-052 | Task.applicable_retry_limit ドメイン関数 | spec/domains/task.md#問い合わせ読み取り | AgentRunは`effective_retry_limit`、Cleanupは2、Waitは`None`を返す |
| DOM-task-053 | TransitionError エラー型 | spec/domains/task.md#エラー型 | `InvalidState{expected:&[ExecutionStateKind],actual}`/`WorkspaceAlreadySet`/`WorkspaceNotSet`/`NotAgentRunStatus`/`MissingCurrentAttempt`/`AlreadyNotified`の6種を区別する。永続化されず表示にしか使われないため分類だけを持ち完成文言を持たない |
| DOM-task-054 | RetryError エラー型 | spec/domains/task.md#エラー型 | `NotStopped{actual}`としてretryの案内文言分岐に使う |
| DOM-task-055 | SetStatusError エラー型 | spec/domains/task.md#エラー型 | `Active{actual}`/`UnknownStatus{given,defined}`の2種を区別する |
| DOM-task-056 | DegradedTask エンティティ | spec/domains/task.md#degradedtaskスナップショット破損タスク | Taskからsnapshotを除いた全フィールド+`snapshot_error`を持ち、スナップショット依存操作(set_status/advance/record_launching等)を型上呼べない |
| DOM-task-057 | DegradedTask.abort ドメイン関数 | spec/domains/task.md#degradedtaskスナップショット破損タスク | Taskと同じ規則で`Stopped`以外から`Stopped{Aborted}`にする |
| DOM-task-058 | DegradedTask.retry ドメイン関数 | spec/domains/task.md#degradedtaskスナップショット破損タスク | Taskと同じ規則で受理されるがtickには拾われず、CLIが修復要警告を出す前提を満たす |
| DOM-task-059 | DegradedTask.mark_notified ドメイン関数 | spec/domains/task.md#degradedtaskスナップショット破損タスク | Taskと同じ規則で`notified_at`を設定し、スナップショット破損でも再通知をスキップしないat-least-once保証を担う |
| DOM-task-060 | DegradedTask 読み取り各種 ドメイン関数 | spec/domains/task.md#degradedtaskスナップショット破損タスク | `execution_kind`等スナップショット非依存の各フィールド参照をshow/lsの注記付き表示に提供する |
| DOM-task-061 | WorkspacePlanner ドメインサービス | spec/domains/task.md#workspaceplanner | `derive(worktree_root,id)`でpath=`<worktree_root>/<task-id>`・branch=`pulsen/<task-id>`を決定的に導出する |
| DOM-task-062 | TaskRepository.create ポートメソッド | spec/domains/task.md#taskrepository | 同IDが現役・アーカイブいずれかに存在すれば`Conflict`を返しID衝突をポートが担保する |
| DOM-task-063 | CreateError エラー型 | spec/domains/task.md#taskrepository | `Conflict`/`Io(message)`の2種を区別する |
| DOM-task-064 | TaskRepository.save ポートメソッド | spec/domains/task.md#taskrepository | 現役に存在しない場合`NotFound`を返し部分的な書き込みを観測させないアトミック性を保証する |
| DOM-task-065 | SaveError エラー型 | spec/domains/task.md#taskrepository | `NotFound`/`Io`の2種をsave/save_degraded共通で区別する |
| DOM-task-066 | TaskRepository.save_degraded ポートメソッド | spec/domains/task.md#taskrepository | DegradedTaskを保存しスナップショットフィールドを元の内容のまま書き戻す(修復材料を温存) |
| DOM-task-067 | TaskRepository.find ポートメソッド | spec/domains/task.md#taskrepository | tasks→archiveの順で解決し`TaskLookup`(Active/Archived/NotFound/Corrupt)を返す |
| DOM-task-068 | ReadError エラー型 | spec/domains/task.md#taskrepository | findのIo系読み取り失敗を`Io`として返す |
| DOM-task-069 | TaskRepository.list_active ポートメソッド | spec/domains/task.md#taskrepository | `state/tasks/`を全件走査し個別破損をエラーにせず`TaskEntry`の配列として返す |
| DOM-task-070 | TaskRepository.list_archived ポートメソッド | spec/domains/task.md#taskrepository | `state/archive/`に同じ走査規則を適用する |
| DOM-task-071 | TaskRepository.archive ポートメソッド | spec/domains/task.md#taskrepository | 現役からアーカイブへ移動し、直後に現役側から消えアーカイブ側に現れるread-your-writesを保証する |
| DOM-task-072 | ArchiveError エラー型 | spec/domains/task.md#taskrepository | `NotFound`/`Io`の2種を区別する |
| DOM-task-073 | TaskLookup 値オブジェクト | spec/domains/task.md#taskrepository | `Active(TaskRecord)`/`Archived(TaskRecord)`/`NotFound`/`Corrupt{path,message}`の4種でfindの結果を表現する |
| DOM-task-074 | TaskRecord 値オブジェクト | spec/domains/task.md#taskrepository | `Intact(Task)`/`SnapshotUnreadable(DegradedTask)`の2種でスナップショットのみ破損したタスクをファイル全体破損と区別する |
| DOM-task-075 | TaskEntry 値オブジェクト | spec/domains/task.md#taskrepository | `Record(TaskRecord)`/`Corrupt{path,message}`の2種でlist_active/list_archivedの走査結果1件を表現する |
| DOM-task-076 | TaskIdGenerator.generate ポートメソッド | spec/domains/task.md#taskidgenerator | 呼び出しごとに実用上衝突しないTaskIdを発行する(厳密一意性はcreateのConflictがバックストップ) |
| DOM-task-077 | Clock.now ポートメソッド | spec/domains/task.md#clock | 現在時刻を`Timestamp`として返す(単調性は要求せず、巻き戻りは経過0として扱われる契約) |
| DOM-execution-001 | ExitCode 値オブジェクト | spec/domains/execution.md#exitcode | i32の終了結果符号化値(正常終了はexit code、シグナル等は128+シグナル番号、起動不能は127/126)を保持する |
| DOM-execution-002 | ExitCode.is_success ドメイン関数 | spec/domains/execution.md#exitcode | 値が0かどうかを判定する |
| DOM-execution-003 | PidFileContent 値オブジェクト | spec/domains/execution.md#pidfilecontent | `pid`/`kill_ident`を保持し、starttime→pidの書き込み順序によりtickが同定情報一式の完了とみなす出現シグナルになる |
| DOM-execution-004 | JudgeOutcome 値オブジェクト | spec/domains/execution.md#judgeoutcome-judgeconclusion | `Completed`/`Failed`/`Skipped`の3値直和型。3値のまま残るのは判定コマンドの exit 20(`interpret_judge_completion`)が`Skipped`を生むため。`From<DefaultJudgement> for JudgeOutcome`は2値が3値に含まれることを型で示す変換 |
| DOM-execution-005 | JudgeConclusion 値オブジェクト | spec/domains/execution.md#judgeoutcome-judgeconclusion | `Outcome(JudgeOutcome)`/`JudgeFailure{detail}`の2種で判定自体の壊れを区別する |
| DOM-execution-006 | LaunchingDecision 値オブジェクト | spec/domains/execution.md#分類の決定直和型 | `ConfirmRunning(ProcessIdent)`/`KeepWaiting`/`SuspectSpawnFailure`の3値でlaunching分類結果を表現する |
| DOM-execution-007 | LaunchingRecheck 値オブジェクト | spec/domains/execution.md#分類の決定直和型 | `ConfirmRunning(ProcessIdent)`/`SpawnFailed`の2値でマーカー書き込み後の再確認結果を表現する |
| DOM-execution-008 | RunningDecision 値オブジェクト | spec/domains/execution.md#分類の決定直和型 | `Judge(ExitCode)`/`KeepRunning`/`KillOnTimeout`/`DiedWithoutExit`の4値でrunning分類結果を表現する。生存分類の3値`AliveDecision`は`From`で合流する |
| DOM-execution-009 | Aliveness 値オブジェクト | spec/domains/execution.md#分類の決定直和型 | `Alive`/`Dead`の2値(Deadは取得不能とPID再利用不一致の両方を含む) |
| DOM-execution-010 | CommandCompletion 値オブジェクト | spec/domains/execution.md#commandcompletion | `Exited(ExitCode)`/`TimedOut`/`FailedToStart{message}`の3値でCommandRunnerの全結末を値として表現する |
| DOM-execution-011 | GcPlan 値オブジェクト | spec/domains/execution.md#gcplan | `deletions: Vec<(String,AttemptNumber)>`としてTaskIdへパースできない孤児も生文字列で削除対象に含める |
| DOM-execution-012 | IdentityCheck.check ドメイン関数 | spec/domains/execution.md#identitycheck | observed(起動時刻)がNoneまたはrecordedと不一致なら`Dead`、一致のみ`Alive`を返しkill実行可否の判定にも使う |
| DOM-execution-013 | LaunchingClassifier ドメインサービス | spec/domains/execution.md#launchingclassifier | `GRACE_PERIOD=30秒`の組み込み定数を持ちlaunching状態タスクの分類を担う |
| DOM-execution-014 | LaunchingClassifier.classify ドメイン関数 | spec/domains/execution.md#launchingclassifier | pid/starttime双方Someなら`ConfirmRunning`、pid None・猶予内なら`KeepWaiting`、猶予超過なら`SuspectSpawnFailure`、pid Some・starttime Noneなら`InconsistentRunFiles`エラーを返す |
| DOM-execution-015 | LaunchingClassifier.classify_recheck ドメイン関数 | spec/domains/execution.md#launchingclassifier | 再確認でpid/starttime双方Someなら`ConfirmRunning`、pid Noneなら`SpawnFailed`、pid Some・starttime Noneなら`InconsistentRunFiles`エラーを返す |
| DOM-execution-016 | InconsistentRunFiles エラー型 | spec/domains/execution.md#launchingclassifier | ラッパーの書き込み順序保証が破れたケースを破れの種別だけを持つ列挙(現在の変種は`MissingStartTime`の1つ)で表現し、当該tickはスキップして次tickで再観測させる。文言は表示側が組み立てる |
| DOM-execution-017 | RunningClassifier.classify_alive ドメイン関数 | spec/domains/execution.md#runningclassifier | 2段規則の2段目(生存)だけを受け持ち、`Alive`かつtimeout未超過は`KeepRunning`、超過は`KillOnTimeout`、`Dead`は`DiedWithoutExit`を返す(timeoutはstarted_wall起点)。返り値型`AliveDecision`が「`Judge`を返さない」を担保し、1段目(exitの有無)はユースケース側にある |
| DOM-execution-018 | JudgementService ドメインサービス | spec/domains/execution.md#judgementservice | 終了した実行の判定(exit code解釈・判定コマンド結果解釈・判定env構成)を担う |
| DOM-execution-019 | JudgementService.default_judgement ドメイン関数 | spec/domains/execution.md#judgementservice | 0=`Completed`、非0=`Failed`。返り値型`DefaultJudgement`が「`Skipped`を返さない」を担保する |
| DOM-execution-020 | JudgementService.interpret_judge_completion ドメイン関数 | spec/domains/execution.md#judgementservice | `Exited(0)`=Completed、`Exited(10)`=Failed、`Exited(20)`=Skipped、それ以外/TimedOut/FailedToStartは`JudgeFailure`を返す |
| DOM-execution-021 | JudgementService.judge_env ドメイン関数 | spec/domains/execution.md#judgementservice | `TASK_ID`/`WORKSPACE`/`EXIT_CODE`(10進文字列)/`RUN_DIR`の4変数を構成する |
| DOM-execution-022 | NotificationService.notify_env ドメイン関数 | spec/domains/execution.md#notificationservice | `TASK_ID`/`WORKFLOW`/`TASK_STATUS`の3変数を構成し、notify_cmd未定義時は通知せず`notified_at`も書かない契約を担う。結末の成否の解釈はこの関数ではなく`interpret_notify_completion`が担う |
| DOM-execution-023 | GcPolicy.plan ドメイン関数 | spec/domains/execution.md#gcpolicy | 保護されておらず`now-last_activity>retention`のattemptを削除対象とし、protection未登録のdir_nameは`Unprotected`として扱う |
| DOM-execution-024 | TaskProtection 値オブジェクト | spec/domains/execution.md#gcpolicy | `ActiveCurrent(Option<AttemptNumber>)`/`AllProtected`/`Unprotected`の3値でgc保護規則を表現する |
| DOM-execution-025 | RunListing 値オブジェクト | spec/domains/execution.md#gcpolicy | gc対象のrunディレクトリ一覧を`RunDirListing`の配列として保持する入力型 |
| DOM-execution-026 | RunDirListing 値オブジェクト | spec/domains/execution.md#gcpolicy | `dir_name`と`attempts: Vec<AttemptInfo>`を保持する1ランディレクトリ分のエントリ |
| DOM-execution-027 | AttemptInfo 値オブジェクト | spec/domains/execution.md#gcpolicy | `{number,last_activity}`としてattempt単位のgc判定材料を保持する |
| DOM-execution-028 | RunDirPath.pid_file ドメイン関数 | spec/domains/execution.md#rundirpath-のファイル配置語彙 | `<run_dir>/pid`パスを導出する |
| DOM-execution-029 | RunDirPath.starttime_file ドメイン関数 | spec/domains/execution.md#rundirpath-のファイル配置語彙 | `<run_dir>/starttime`パスを導出する |
| DOM-execution-030 | RunDirPath.exit_file ドメイン関数 | spec/domains/execution.md#rundirpath-のファイル配置語彙 | `<run_dir>/exit`パスを導出する |
| DOM-execution-031 | RunDirPath.stdout_log ドメイン関数 | spec/domains/execution.md#rundirpath-のファイル配置語彙 | `<run_dir>/stdout.log`パスを導出する |
| DOM-execution-032 | RunDirPath.stderr_log ドメイン関数 | spec/domains/execution.md#rundirpath-のファイル配置語彙 | `<run_dir>/stderr.log`パスを導出する |
| DOM-execution-033 | RunDirPath.marker_file ドメイン関数 | spec/domains/execution.md#rundirpath-のファイル配置語彙 | `<run_dir>/invalidated`パス(存在のみが意味を持つ空ファイル)を導出する |
| DOM-execution-034 | RunStore.prepare_attempt ポートメソッド | spec/domains/execution.md#runstore | attemptディレクトリを親含め冪等に作成し`RunDirPath`を返す |
| DOM-execution-035 | RunStore.read_pid_file ポートメソッド | spec/domains/execution.md#runstore | 不在は`Ok(None)`、内容不正は`Corrupt`として区別して`PidFileContent`を読む |
| DOM-execution-036 | RunStore.read_starttime ポートメソッド | spec/domains/execution.md#runstore | read_pid_fileと同じ不在/Corrupt区別規則を`StartTimeRecord`に適用する |
| DOM-execution-037 | RunStore.read_exit ポートメソッド | spec/domains/execution.md#runstore | read_pid_fileと同じ不在/Corrupt区別規則を`ExitCode`に適用する |
| DOM-execution-038 | RunStore.attempt_exists ポートメソッド | spec/domains/execution.md#runstore | attemptディレクトリ自体の存在を読み系のOk(None)とは独立に返し「空ディレクトリ」と「ディレクトリごと不在」を区別可能にする |
| DOM-execution-039 | RunStore.write_invalidation_marker ポートメソッド | spec/domains/execution.md#runstore | ディレクトリ不在なら作成したうえで冪等にマーカーを書く |
| DOM-execution-040 | RunStore.marker_exists ポートメソッド | spec/domains/execution.md#runstore | マーカーファイルの存在有無を返す(ラッパーが使用) |
| DOM-execution-041 | RunStore.write_starttime ポートメソッド | spec/domains/execution.md#runstore | アトミック置換で`StartTimeRecord`を書き、書きかけを観測させない。書き込み先のディレクトリは必要に応じて作る |
| DOM-execution-042 | RunStore.write_pid_file ポートメソッド | spec/domains/execution.md#runstore | アトミック置換で`PidFileContent`を書き、書きかけを観測させない。書き込み先のディレクトリは必要に応じて作る |
| DOM-execution-043 | RunStore.write_exit ポートメソッド | spec/domains/execution.md#runstore | アトミック置換で`ExitCode`を書き、書きかけを観測させない。書き込み先のディレクトリは必要に応じて作る |
| DOM-execution-044 | RunStore.list_runs ポートメソッド | spec/domains/execution.md#runstore | `state/runs/`不在時は空`RunListing`を返し、`attempt-<n>`形式外のエントリは列挙対象外とする |
| DOM-execution-045 | RunStore.delete_attempt ポートメソッド | spec/domains/execution.md#runstore | 指定attemptを削除し、失敗は呼び出し側がスキップ・報告する値として返す |
| DOM-execution-046 | RunStore.remove_task_dir_if_empty ポートメソッド | spec/domains/execution.md#runstore | attemptや形式外エントリが残る非空ディレクトリは削除せず`Ok`を返す(エラーにしない) |
| DOM-execution-047 | RunFileError エラー型 | spec/domains/execution.md#runstore | `Corrupt{path,message}`/`Io{message}`の2種で内容不正を不在と区別する |
| DOM-execution-048 | ProcessController.spawn_wrapper ポートメソッド | spec/domains/execution.md#processcontroller | 自バイナリをラッパーモードで新しいプロセスグループ相当の単位としてデタッチ起動し、同期エラー時は状態を変更しない |
| DOM-execution-049 | ProcessController.starttime_of ポートメソッド | spec/domains/execution.md#processcontroller | プロセス不在は`Ok(None)`、取得機構自体の失敗は`Err(Io)`として区別し状態変更に使わない |
| DOM-execution-050 | ProcessController.kill ポートメソッド | spec/domains/execution.md#processcontroller | `IdentityCheck`がAliveの前提でプロセスグループ相当を一括終了し、失敗時は呼び出し側が状態を変更しない |
| DOM-execution-051 | ProcessController.try_kill_remnants ポートメソッド | spec/domains/execution.md#processcontroller | `Killed`/`NotIdentifiable`/`Failed{message}`を返すベストエフォート終了で、誤殺なく同定できる場合のみ実行する |
| DOM-execution-052 | ProcessController.own_identity ポートメソッド | spec/domains/execution.md#processcontroller | ラッパー自身のpid・kill同定子・StartTimeRecordを取得する |
| DOM-execution-053 | ProcessController.run_agent ポートメソッド | spec/domains/execution.md#processcontroller | cwdを常にworktreeとしエージェントを同期実行し、起動不能(127/126)・シグナル死(128+n)を含め常にExitCodeを返す(失敗しない) |
| DOM-execution-054 | WrapperLaunchSpec 値オブジェクト | spec/domains/execution.md#processcontroller | `run_dir`/`agent_cmd`/`workspace`をラッパーモード起動引数として保持する |
| DOM-execution-055 | WrapperIdentity 値オブジェクト | spec/domains/execution.md#processcontroller | `pid`/`kill_ident`/`starttime`をown_identityの結果として保持する |
| DOM-execution-056 | RemnantOutcome 値オブジェクト | spec/domains/execution.md#processcontroller | `Killed`/`NotIdentifiable`/`Failed{message}`の3値でtry_kill_remnantsの結果を表現する |
| DOM-execution-057 | SpawnError エラー型 | spec/domains/execution.md#processcontroller | `Failed{message}`としてOSレベル起動失敗を表現し状態は変更しない |
| DOM-execution-058 | KillError エラー型 | spec/domains/execution.md#processcontroller | `Failed{message}`としてシグナル送出・ジョブ終了自体のエラーを表現する |
| DOM-execution-059 | WorktreeManager.validate_repo ポートメソッド | spec/domains/execution.md#worktreemanager | `NotFound`/`NotARepository`を区別してリポジトリ存在・種別を検証する |
| DOM-execution-060 | WorktreeManager.head_branch ポートメソッド | spec/domains/execution.md#worktreemanager | HEADのブランチ名を取得し`DetachedHead`/`EmptyRepository`を区別する |
| DOM-execution-061 | WorktreeManager.branch_exists ポートメソッド | spec/domains/execution.md#worktreemanager | 指定ブランチの存在有無を返す |
| DOM-execution-062 | WorktreeManager.create ポートメソッド | spec/domains/execution.md#worktreemanager | baseから新ブランチとworktreeを作成し、自タスク残骸(既存worktree/既存ブランチのみ)に対しては冪等に成功する |
| DOM-execution-063 | WorktreeManager.remove ポートメソッド | spec/domains/execution.md#worktreemanager | worktree内容の状態によらず強制削除し、既に不在なら`AlreadyAbsent`として成功、ブランチには触れない |
| DOM-execution-064 | RemoveOutcome 値オブジェクト | spec/domains/execution.md#worktreemanager | `Removed`/`AlreadyAbsent`の2値で削除結果を表現する |
| DOM-execution-065 | TargetError エラー型 | spec/domains/execution.md#worktreemanager | `NotFound`/`NotARepository`/`DetachedHead`/`EmptyRepository`/`Failed{message}`の5種を区別する |
| DOM-execution-066 | WorktreeError エラー型 | spec/domains/execution.md#worktreemanager | `Failed{message}`としてgit操作失敗を表現する(分類に使わない不透明message) |
| DOM-execution-067 | CommandRunner.run ポートメソッド | spec/domains/execution.md#commandrunner | シェル非経由・プレースホルダ展開なしで判定/通知コマンドを同期実行し、timeout超過は`TimedOut`、起動不能は`FailedToStart`を返す |
| DOM-execution-068 | ExclusiveLock.try_acquire ポートメソッド | spec/domains/execution.md#exclusivelock | グローバルホーム単位の単一ロックをブロックせず取得し、取得不能は`Ok(None)`(エラーではない) |
| DOM-execution-069 | LockError エラー型 | spec/domains/execution.md#exclusivelock | `Failed{message}`としてロック機構自体の異常を表現する |
| DOM-execution-070 | LockGuard 値オブジェクト | spec/domains/execution.md#exclusivelock | ドロップで解放されるRAIIガードとして、保持プロセスの異常終了時もOSにより解放される |
| DOM-definition-053 | LoadedWorkflow 値オブジェクト | spec/domains/definition.md#workflowstore | `parsed: ParsedWorkflow` と `resolved_from`(実際に読み込んだ絶対パス)を保持し、add 成功時の「解決したワークフロー名と解決先」表示の供給元になる |
| DOM-definition-054 | ConfigLoadError エラー型 | spec/domains/definition.md#configstore | `NotFound{home}` / `Invalid{message,location}` / `Io{message}` の3種を区別する |
| DOM-definition-055 | WorkflowLoadError エラー型 | spec/domains/definition.md#workflowstore | `NotFound{attempted}`(解決を試みた絶対パス) / `Parse{error:WorkflowParseError,resolved_from:PathBuf}`(解決先を構造として持つ) / `Io{message}` の3種を区別する |
| DOM-definition-056 | RawCommandDoc 値オブジェクト | spec/domains/definition.md#workflowassembler | `RawStatusDoc.judge` が保持する未パースのコマンド入力(文字列形式・トークン配列形式)を表し、`RawCommand` 生成規則の入力になる |
| DOM-task-078 | Target 値オブジェクト | spec/domains/task.md#task集約ルート | `repo: RepoPath` と `base_branch: BranchName` の組を不変に保持する |
| DOM-task-079 | ExecutionState.kind ドメイン関数 | spec/domains/task.md#executionstatekind | 実行状態の直和型からデータなしの判別子 `ExecutionStateKind` を導出する(`Task.execution_kind` の委譲先) |
| DOM-execution-071 | NotificationService ドメインサービス | spec/domains/execution.md#notificationservice | stopped確定通知の環境変数の構成と、通知の結末の成否の解釈を責務とする。組み込み `NOTIFY_TIMEOUT=60秒`(ADR-018)を notify_cmd に必ず適用し、成否の解釈は`interpret_notify_completion`に一本化する |
| DOM-definition-057 | CommandLine.rehydrate ドメイン関数 | spec/domains/definition.md#commandline | プロセス境界を越えた argv から `CommandLine` を復元する。トークン0個は `CommandError::Empty` を返し、テンプレートを持たない側でも1トークン以上の不変条件を通す |
| DOM-task-080 | ToolFailureKind 値オブジェクト | spec/domains/task.md#failurenote | `WorktreeCreate`/`WorktreeRemove`/`ArchiveMove` の3値直和型。`record_tool_failure` の引数を絞り、記録時に `FailureKind` へ写す |
| DOM-task-081 | RunDirPath.state_root ドメイン関数 | spec/domains/task.md#rundirpath | パスから `attempt-<n>` と task-id を読み、`derive` で組み直した結果が自身と一致する場合にのみ `Some(StateRoot)` を返す(config もホームも読まないラッパーが `RunStore` を組むために使う) |
| DOM-execution-072 | AliveDecision 値オブジェクト | spec/domains/execution.md#分類の決定直和型 | `KeepRunning`/`KillOnTimeout`/`DiedWithoutExit` の3値。`RunningDecision` から `Judge` を除いた型で、`From<AliveDecision> for RunningDecision` により合流する |
| DOM-execution-073 | DefaultJudgement 値オブジェクト | spec/domains/execution.md#judgeoutcome-judgeconclusion | `Completed`/`Failed` の2値。`Skipped` は判定コマンドの exit 20 だけが生むため、デフォルト判定の返り値型から除かれる。この経路の結末は2値のまま写す |
| DOM-execution-074 | NotifyOutcome 値オブジェクト | spec/domains/execution.md#notificationservice | `Delivered`/`Failed{cause:NotifyFailureCause}` の2分岐。`Delivered` だけが `notified_at` を書く根拠になる(at-least-once)ため、`Failed` を平坦化しない |
| DOM-execution-075 | NotificationService.interpret_notify_completion ドメイン関数 | spec/domains/execution.md#notificationservice | `Exited(0)` を `Delivered`、非0終了 / `TimedOut` / `FailedToStart` を `Failed{cause}` に解釈する。原因は分類として持ち完成文言は持たない(文言は CLI 層が組み立てる) |
| DOM-execution-076 | NotifyFailureCause 値オブジェクト | spec/domains/execution.md#notificationservice | `ExitedNonZero{exit}`/`TimedOut`/`FailedToStart{message}` の3値。`TimedOut` は組み込み定数 `NOTIFY_TIMEOUT` の1つに定まるためフィールドを持たない |
