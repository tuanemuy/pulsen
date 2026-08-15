# Inventory — adapter

生成元: spec/domains/(ポート実装。最終同期: 2026-08-15)

| ID | 要素 | 定義場所 | 実装されるべき振る舞いの要点 |
|----|------|---------|------------------------------|
| ADP-config-001 | ConfigStore.load(実装) | spec/domains/definition.md#configstore | 読み取り専用・ロック不要。YAML構文検証・未知キー検出(ADR-013)・型/期間形式の検証を読み込み時に行い`NotFound`/`Invalid`/`Io`を区別する。`Invalid`の`location`は2層で、構文エラー・重複キーは行・列、スキーマ違反(未知キー・型不一致)は問題のキーのパス(論理位置。`agents.claude.cmd`等)を指す。テンプレート内容(プレースホルダ)は検証しない(参照時検証)。キャッシュせず呼び出し時点のファイル内容を返す |
| ADP-workflowstore-001 | WorkflowStore.load(実装) | spec/domains/definition.md#workflowstore | 名前解決は`Name(n)`→`<home>/workflows/<n>.yaml`固定(`.yml`へのフォールバックなし)、`Path(p)`はプロセスのカレントディレクトリから相対解決。YAMLテキスト→`RawWorkflowDoc`変換時に構文エラー(`YamlSyntax`)・スキーマ外キー(`UnknownKey`)を検出し、`WorkflowAssembler::assemble`に委譲する。`resolved_from`に実際に読み込んだ絶対パスを返す。パースの失敗は`Parse{error,resolved_from}`として解決先を伴って返し、`WorkflowParseError`側の自由形式メッセージにはパスを前置しない(前置が残るのは`Io{message}`のみ)。読み取り専用・キャッシュしない |
| ADP-taskrepo-001 | TaskRepository.create(実装) | spec/domains/task.md#taskrepository | ID衝突をアダプターが担保し`Conflict`を返す(呼び出し側の事前確認に依存しない)。存在判定はデコード可否によらない(破損ファイルも存在扱い)。必要なディレクトリ(`state/tasks/`等)を自動作成する。書き込みはアトミック置換で行い部分的な内容を観測させない |
| ADP-taskrepo-002 | TaskRepository.save(実装) | spec/domains/task.md#taskrepository | 現役に存在しない場合`NotFound`。アトミック置換により書きかけの内容を観測させない。直後の`find`が更新後の内容を返す(read-your-writes) |
| ADP-taskrepo-003 | TaskRepository.save_degraded(実装) | spec/domains/task.md#taskrepository | スナップショットフィールドをファイル内の元の内容のまま書き戻す(往復可能性・修復材料の温存)。タスク側フィールドのみ更新する。アトミック置換 |
| ADP-taskrepo-004 | TaskRepository.find(実装) | spec/domains/task.md#taskrepository | 解決順はtasks→archive。ディレクトリ不在は空(`NotFound`)として扱う。デコード時に(a)タスク側フィールドの構文・値制約違反を`Corrupt`、(b)スナップショットの構文・構造不変条件・`task_status∈snapshot.statuses`の破れを`SnapshotUnreadable`として区別する。走査対象を読めない場合は`Io`(`NotFound`/`Corrupt`に写像しない) |
| ADP-taskrepo-005 | TaskRepository.list_active(実装) | spec/domains/task.md#taskrepository | 全件走査(ページングなし)。個別タスクの破損(`Corrupt`/`SnapshotUnreadable`)を走査全体の失敗にしない。命名形式(`<task-id>.json`)外のエントリは列挙しない。ディレクトリ読み取り不能は`Io`として返す(空リストに写像しない) |
| ADP-taskrepo-006 | TaskRepository.list_archived(実装) | spec/domains/task.md#taskrepository | list_activeと同じ区分・規則をアーカイブ側(`state/archive/`)に適用する |
| ADP-taskrepo-007 | TaskRepository.archive(実装) | spec/domains/task.md#taskrepository | 移動は原子的に行い、現役・アーカイブ双方に完全体がある/両方にないという中間状態を反復読み取りに観測させない。成功後は即座に現役側から消えアーカイブ側に現れる(read-your-writes)。移動先ディレクトリを自動作成する。失敗時は現役側に完全な内容のまま残す(部分移動を残さない) |
| ADP-taskid-001 | TaskIdGenerator.generate(実装) | spec/domains/task.md#taskidgenerator | `TaskId`の文字集合制約(`[a-z0-9-]`・1〜64文字・先頭英数字)を常に満たす形式で発行する。時刻成分のみに依存せず同一時刻内の連続発行・複数インスタンス間でも実用上重複しない(厳密な一意性は`TaskRepository.create`の`Conflict`がバックストップ) |
| ADP-clock-001 | Clock.now(実装) | spec/domains/task.md#clock | OS依存なくシステム時計から秒精度UTCの`Timestamp`を取得する。サブ秒成分を持たせない。単調性は保証しない(壁時計。巻き戻りは呼び出し側の責務で吸収する契約であり、アダプターは巻き戻りを補正しない) |
| ADP-runstore-001 | RunStore.prepare_attempt(実装) | spec/domains/execution.md#runstore | `RunDirPath::derive`と一致するパスに親を含めてディレクトリを作成する。冪等(既存の書き込み済みファイルに影響しない) |
| ADP-runstore-002 | RunStore.read_pid_file(実装) | spec/domains/execution.md#runstore | ファイル不在・ディレクトリ自体の不在はいずれも`Ok(None)`として扱う(区別しない)。内容が解釈不能な場合は`Corrupt`、機構失敗は`Io`として不在と区別する |
| ADP-runstore-003 | RunStore.read_starttime(実装) | spec/domains/execution.md#runstore | read_pid_fileと同じ不在/Corrupt/Ioの区別規則を`StartTimeRecord`に適用する |
| ADP-runstore-004 | RunStore.read_exit(実装) | spec/domains/execution.md#runstore | read_pid_fileと同じ不在/Corrupt/Ioの区別規則を`ExitCode`に適用する |
| ADP-runstore-005 | RunStore.attempt_exists(実装) | spec/domains/execution.md#runstore | attemptディレクトリ自体の存在確認をread系の`Ok(None)`とは独立に提供し、「空ディレクトリ」と「ディレクトリごと不在」を区別できるようにする |
| ADP-runstore-006 | RunStore.write_invalidation_marker(実装) | spec/domains/execution.md#runstore | ディレクトリ不在なら作成したうえでマーカーを書く。冪等 |
| ADP-runstore-007 | RunStore.marker_exists(実装) | spec/domains/execution.md#runstore | マーカーファイルの存在有無をそのまま返す |
| ADP-runstore-008 | RunStore.write_starttime(実装) | spec/domains/execution.md#runstore | アトミック置換で書き込み、並行読み取りに書きかけ・新旧混合の内容を観測させない。書き込み先のディレクトリは必要に応じて作る。書き込み失敗時も部分的な内容を残さない(不在または従前の完全な値のみを観測可能に保つ) |
| ADP-runstore-009 | RunStore.write_pid_file(実装) | spec/domains/execution.md#runstore | write_starttimeと同じアトミック置換・非観測性・書き込み先のディレクトリを必要に応じて作る契約を`PidFileContent`に適用する |
| ADP-runstore-010 | RunStore.write_exit(実装) | spec/domains/execution.md#runstore | write_starttimeと同じアトミック置換・非観測性・書き込み先のディレクトリを必要に応じて作る契約を`ExitCode`に適用する |
| ADP-runstore-011 | RunStore.list_runs(実装) | spec/domains/execution.md#runstore | `state/runs/`不在時は空の`RunListing`を返す。`attempt-<n>`形式外のエントリは列挙対象外とする。`last_activity`はディレクトリ内ファイルの最終更新時刻の最大値、ファイルが1つもなければディレクトリ自体の最終更新時刻とする。`TaskId`にパースできないディレクトリ名は生文字列のまま列挙する |
| ADP-runstore-012 | RunStore.delete_attempt(実装) | spec/domains/execution.md#runstore | 指定attemptのみを削除し他のattemptに影響しない。削除失敗は`Io`を値として返す(パニックしない。カウンタ・stoppedを発生させない呼び出し側判断の前提) |
| ADP-runstore-013 | RunStore.remove_task_dir_if_empty(実装) | spec/domains/execution.md#runstore | attemptが1つ以上、または`attempt-<n>`形式外のエントリが残る場合は削除せず`Ok`を返す(非空はエラーではない。残存エントリに触れない)。空になった場合のみタスクディレクトリを削除する |
| ADP-process-001 | ProcessController.spawn_wrapper(実装) | spec/domains/execution.md#processcontroller | プラットフォーム(OS)依存操作をアダプター層に隔離する。自身のバイナリをラッパーモードで新しいプロセスグループ相当の単位としてデタッチ起動する。起動後の成否は関知せず観測はrunディレクトリ経由とする。同期エラー時はrunディレクトリ・プロセスに副作用を残さない |
| ADP-process-002 | ProcessController.starttime_of(実装) | spec/domains/execution.md#processcontroller | 記録時と同一の取得手段で起動時刻を取得する(照合の前提)。プロセス不在は`Ok(None)`、取得機構自体の失敗は`Err(Io)`として区別する(機構失敗を死亡に写像しない) |
| ADP-process-003 | ProcessController.kill(実装) | spec/domains/execution.md#processcontroller | プロセスグループ相当の実行単位を一括終了する。プロセス内保持ハンドルに依存せず`KillIdent`のみで実行できる(ツール再起動後のkillに対応)。失敗は`Err(KillError::Failed)`を値として返す(パニックしない) |
| ADP-process-004 | ProcessController.try_kill_remnants(実装) | spec/domains/execution.md#processcontroller | 誤殺なく同定できる場合に限りベストエフォートで終了する。同定不能なら`NotIdentifiable`を返しいかなるプロセスも終了させない |
| ADP-process-005 | ProcessController.own_identity(実装) | spec/domains/execution.md#processcontroller | ラッパーモードの自プロセスのpid・kill同定子・`StartTimeRecord`をOS依存の取得手段で取得する。取得失敗は`Err(Io)`(パニックせず不正な同定情報で`Ok`を装わない) |
| ADP-process-006 | ProcessController.run_agent(実装) | spec/domains/execution.md#processcontroller | シェルを介さず直接起動する(引数のリテラル一致・展開なし)。cwdは常にworktree。標準出力・標準エラーを指定パスへリダイレクトする。起動不能(127/126)・シグナル死(128+n)を含め常に`ExitCode`を返し失敗しない。リダイレクト先を開けない場合はエージェントを起動せず126を返す |
| ADP-worktree-001 | WorktreeManager.validate_repo(実装) | spec/domains/execution.md#worktreemanager | git操作(リポジトリ存在・種別確認)をアダプター層に隔離する。`NotFound`/`NotARepository`/`Failed`を区別する |
| ADP-worktree-002 | WorktreeManager.head_branch(実装) | spec/domains/execution.md#worktreemanager | HEADのブランチ名取得。detached HEADは`DetachedHead`、コミットのない空リポジトリは`EmptyRepository`として区別する |
| ADP-worktree-003 | WorktreeManager.branch_exists(実装) | spec/domains/execution.md#worktreemanager | 指定ブランチの存在確認をそのまま返す |
| ADP-worktree-004 | WorktreeManager.create(実装) | spec/domains/execution.md#worktreemanager | `git worktree add`相当。`ws.path`の親(worktree_root)が不在なら作成する。`ws.path`が既に`ws.branch`のworktreeとして存在する場合は成功(冪等・自タスク残骸への復旧)。`ws.branch`のみ存在する場合は既存ブランチに張り直す(先端を変更しない)。それ以外の予期しない状態は自動修復せず`Failed`を返す |
| ADP-worktree-005 | WorktreeManager.remove(実装) | spec/domains/execution.md#worktreemanager | worktreeの内容の状態(未コミット変更・未追跡ファイル・`index.lock`等)によらず削除する(`git worktree remove --force`相当)。既に不在なら`AlreadyAbsent`として成功。ブランチには一切触れない |
| ADP-commandrunner-001 | CommandRunner.run(実装) | spec/domains/execution.md#commandrunner | シェルを介さず直接起動する(プレースホルダ展開・シェル解釈なし)。呼び出しプロセスの環境を継承し`env`で追加・上書きする。`timeout`超過時はプロセスを終了させ`TimedOut`を返す。コマンド不在・起動不能は`FailedToStart`。シグナル死等は`run_agent`と同じ符号化規則で`Exited`として返す。標準出力・標準エラーは捕捉せず呼び出しプロセスへ流す。同期実行 |
| ADP-lock-001 | ExclusiveLock.try_acquire(実装) | spec/domains/execution.md#exclusivelock | OS依存のプロセス間排他ロック(アドバイザリロック相当)をアダプター層に隔離する。グローバルホームごとに単一のロック。ブロックせず取得不能なら即座に`Ok(None)`を返す。`LockGuard`のドロップで解放し、保持プロセスの異常終了でもOSにより解放される。ロック機構自体の異常は`Err(LockError::Failed)`として区別する |
