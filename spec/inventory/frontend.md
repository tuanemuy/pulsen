# Inventory — frontend

生成元: spec/pages/(CLI コマンド。最終同期: 2026-08-16)

| ID | 要素 | 定義場所 | 実装されるべき振る舞いの要点 |
|----|------|---------|------------------------------|
| PAGE-common-001 | グローバルホームの解決 | spec/pages/index.md#共通事項 | `--home` フラグ > 環境変数 `PULSEN_HOME` > 既定 `~/.pulsen/` の優先順位で、全コマンドが同一規則でホームディレクトリを解決していれば PASS。 |
| PAGE-common-002 | ロック取得規則 | spec/pages/index.md#共通事項 | ls / show はロックを取得せず、add / tick / abort / retry / set-status は tick と同一の排他ロックを取得し、ロック待ちをせず取得失敗時は「別の操作が実行中」と報告して終了すれば PASS。 |
| PAGE-common-003 | config.yaml の起動時読み込みと参照時検証 | spec/pages/index.md#共通事項 | 全コマンド(wrapper を除く)が起動時に config.yaml を読み込み、キーはすべて任意で `agents` は add 登録時、`notify_cmd` 未定義は stopped 確定処理時というように参照時に個別検証されれば PASS。 |
| PAGE-common-004 | タスクIDの tasks→archive 解決順序 | spec/pages/index.md#共通事項 | タスクIDの解決が `state/tasks/` → `state/archive/` の順で探索され、tasks ヒットは現役、archive のみヒットはアーカイブ済み、双方なしはタスク不在として区別されれば PASS。 |
| PAGE-common-005 | exit code 規約 | spec/pages/index.md#共通事項 | 成功時は 0、入力・状態起因のエラー時は非0を返し、tick のロックスキップのみ例外的に 0 を返せば PASS。 |
| PAGE-common-006 | 縮退状態の生成規則 | spec/pages/index.md#縮退状態の共通規則 | 各コマンドが自身の動作に必要なリソースのみ検証し、読めないリソースへは書き込まず、複数タスク対象コマンドは問題タスクをスキップして報告し全体を失敗させず、状態変更不能時は部分変更を残さず終了する、という4規則が縮退状態表の全マスに一貫して適用されていれば PASS。 |
| PAGE-add-001 | add コマンド | spec/pages/index.md#add | `pulsen add --workflow <name\|path> --repo <path> [--base <branch>]` の構文を解析し、タスクを pending で登録するだけで実行はしない(次回tickを待たず開始したい場合は `add && tick` と合成する)動作であれば PASS。 |
| PAGE-add-002 | `--workflow` オプションの解釈 | spec/pages/index.md#add | 値がパス区切り文字を含むか `.yaml`/`.yml` で終わればファイルパスとして、それ以外は名前として `workflows/<name>.yaml` を解決し、記録・表示するワークフロー名は名前指定ならその名前、パス指定ならYAMLの `workflow:` キー(なければファイル名から拡張子を除いたもの)になれば PASS。 |
| PAGE-add-003 | `--repo` オプション | spec/pages/index.md#add | 対象リポジトリのパスとして受理され、後続の存在検証に渡されれば PASS。 |
| PAGE-add-004 | `--base` オプションの解釈 | spec/pages/index.md#add | 省略時はリポジトリの HEAD が指すブランチを使用し、detached HEAD やコミットのない空リポジトリで特定できない場合は `--base` の明示指定を案内してエラーとし、タスクを作らなければ PASS。 |
| PAGE-add-005 | ワークフロー定義の解決・パース・検証 | spec/pages/index.md#add | initial の指定と参照先の存在、各ステータスの next の参照先存在、エージェント実行ステータスの next 必須、動作宣言が prompt/skill/run のいずれか1つであること(run の値は cleanup/wait)、参照エージェント名の妥当性、skill 指定時の skill_input、テンプレートのプレースホルダと供給値の整合をすべて検証すれば PASS。 |
| PAGE-add-006 | リポジトリパス・ベースブランチの存在検証 | spec/pages/index.md#add | 指定されたリポジトリパスとベースブランチの存在を検証し、存在しなければ登録を拒否すれば PASS。 |
| PAGE-add-007 | 検証済み定義のスナップショット保存とタスクファイルの作成 | spec/pages/index.md#add | 検証済みのワークフロー定義をスナップショットとして保存し、実行状態 pending のタスクファイルを作成すれば PASS。 |
| PAGE-add-008 | タスクIDの発行と表示 | spec/pages/index.md#add | 登録時にタスクIDを発行し、その値を利用者に表示すれば PASS。 |
| PAGE-add-009 | 成功時の終了 | spec/pages/index.md#add | 検証・登録が成功した場合にタスクIDを表示して 0 で終了すれば PASS。 |
| PAGE-add-010 | 検証エラー時の拒否 | spec/pages/index.md#add | ワークフロー不在・YAML不正・エージェント未定義・skill_input 欠落・リポジトリ/ブランチ不在の各原因を表示して非0で終了し、タスクを作らず、ワークフロー名解決失敗時は解決を試みたパス(`workflows/<name>.yaml` の絶対パス)を、ワークフロー定義のパースエラー時は解決先の絶対パス(名前指定では利用者が直接書いていないため)を、エージェント未定義時は config.yaml に定義済みのエージェント名一覧を添えれば PASS。 |
| PAGE-tick-001 | tick コマンド | spec/pages/index.md#tick | `pulsen tick` の構文で1回のtickパスを実行して終了し、定期実行は外部スケジューラーに委ねる責務であれば PASS。 |
| PAGE-tick-002 | 全タスクファイルの走査と状態応じた処理 | spec/pages/index.md#tick | 全タスクファイルを走査し、実行状態・タスクステータスに応じて起動/launching分類/観測・判定/遷移/クリーンアップ/通知を行えば PASS。 |
| PAGE-tick-003 | run_retention 設定時のgc | spec/pages/index.md#tick | `run_retention` が設定されている場合にのみ保持期間を超えたattemptのrunディレクトリをgcし(保護規則・失敗時の扱いは requirements §9.2 準拠)、未設定なら行わなければ PASS。 |
| PAGE-tick-004 | 処理結果のサマリー表示 | spec/pages/index.md#tick | ID を並べる見出しがサマリーの10フィールド(launched/confirmed_running/judged/transitioned/skipped_back/frozen/notified/archived/gc_deleted/gc_errors)と1対1で対応し、空の見出しを出さなければ PASS(日本語の文言は表示層の裁量)。 |
| PAGE-tick-005 | 成功時の終了 | spec/pages/index.md#tick | 実行したアクションのサマリーを表示して 0 で終了し、記録すべきことが1つも起きなかった tick は「処理対象のタスクはありませんでした」を表示して 0 で終了すれば PASS。 |
| PAGE-ls-001 | ls コマンド | spec/pages/index.md#ls | `pulsen ls [--status <task-status>] [--state <exec-state>] [--all]` の構文を解析し、タスク一覧を表示する責務であれば PASS。 |
| PAGE-ls-002 | `--status` オプションによる絞り込み | spec/pages/index.md#ls | タスクステータス(ユーザー定義)で絞り込み、値の検証は行わず未知の値は「該当なし」として空の一覧(exit code 0)になれば PASS。 |
| PAGE-ls-003 | `--state` オプションによる絞り込み | spec/pages/index.md#ls | 実行状態(pending/launching/running/completed/failed/stopped)で絞り込めれば PASS。 |
| PAGE-ls-004 | `--all` オプションによる対象集合の拡張 | spec/pages/index.md#ls | `state/archive/` のアーカイブ済みタスクも対象集合に含めれば PASS。 |
| PAGE-ls-005 | オプション併用時の合成規則 | spec/pages/index.md#ls | `--status` と `--state` は AND で絞り込み、`--all` は絞り込みではなく対象集合の拡張(現役のみ→現役+アーカイブ)として、拡張後の集合に絞り込みが適用されれば PASS。 |
| PAGE-ls-006 | 一覧の表示項目 | spec/pages/index.md#ls | タスクID・ワークフロー名・対象(リポジトリ)・ブランチ・タスクステータス・実行状態・attempt_count・更新日時を表示し、ブランチは `--all` のアーカイブ済み行でも表示すれば PASS。 |
| PAGE-ls-007 | タスクステータスと実行状態の常時併記 | spec/pages/index.md#ls | 同じタスクステータスでも実行状態で意味が異なるため、両方を常に表示すれば PASS。 |
| PAGE-ls-008 | 破損タスクの検出・報告 | spec/pages/index.md#ls | パース不能なタスクファイル、およびスナップショットが読み取り不能なタスクを検出し、ファイルパスと読めない旨を一覧に含めて報告すれば PASS。 |
| PAGE-ls-009 | 成功(タスクあり)時の表示 | spec/pages/index.md#ls | 該当タスクがある場合に一覧を表示して 0 で終了し、`--all` ではアーカイブ済み行にその旨の印を付ければ PASS。 |
| PAGE-ls-010 | 成功(該当なし)時の表示 | spec/pages/index.md#ls | 該当タスクがない場合に空である旨を表示して 0 で終了すれば PASS。 |
| PAGE-ls-011 | `--state` 不正値指定時の拒否 | spec/pages/index.md#ls | `--state` に固定6値以外の値が指定された場合、有効な値の一覧を添えて非0で終了すれば PASS。 |
| PAGE-show-001 | show コマンド | spec/pages/index.md#show | `pulsen show <task-id>` の構文でタスクの詳細を表示する責務であれば PASS。 |
| PAGE-show-002 | タスクファイルの全属性表示 | spec/pages/index.md#show | ワークフロー名、対象(リポジトリ・ベースブランチ)、現在のタスクステータス、実行状態、workspace_path、branch、attempt_count/judge_attempt_count/spawn_fail_count(適用上限併記)、現在attemptのrunディレクトリパス・PID・kill同定子・starttime、直近の失敗要因、notified_at、更新日時を表示し、未実行タスクは attempt 関連を「なし」、workspace 未確定は「未作成」、launching で同定情報未取り込みは該当項目を「未取得」と表示すれば PASS。 |
| PAGE-show-003 | スナップショット定義済みステータス一覧とスナップショット保存先パスの表示 | spec/pages/index.md#show | スナップショットされたワークフロー定義の定義済みステータス一覧(set-status の遷移先確認用)とスナップショットの保存先パスを表示すれば PASS。 |
| PAGE-show-004 | 最新attemptの実行メタデータ参照の表示 | spec/pages/index.md#show | 最新attemptのrunディレクトリの `stdout.log`/`stderr.log`/`exit` のパスと exit の値を表示すれば PASS。 |
| PAGE-show-005 | stopped タスクの凍結要因表示 | spec/pages/index.md#show | stopped タスクでは凍結要因(直前実行の終了情報・最終出力への参照、またはツール操作の失敗要因)を表示すれば PASS。 |
| PAGE-show-006 | 成功時の終了 | spec/pages/index.md#show | 詳細を表示して 0 で終了すれば PASS。 |
| PAGE-abort-001 | abort コマンド | spec/pages/index.md#abort | `pulsen abort <task-id>` の構文でタスクを停止して stopped を記録し、stopped・アーカイブ済み以外のすべての実行状態に適用できれば PASS。 |
| PAGE-abort-002 | kill対象の有無による分岐 | spec/pages/index.md#abort | 分岐が実行状態のラベルではなく kill 対象の有無で決まり、同定情報が得られれば starttime 照合付きでkillしてstoppedを記録、得られなければ記録のみ行い、照合不一致(PID再利用)やプロセス既死亡時は誤殺を避けてkillせず記録のみ行えば PASS。 |
| PAGE-abort-003 | 同定情報の取得元 | spec/pages/index.md#abort | タスクファイルに取り込み済みの同定子(running等)、またはlaunchingならrunディレクトリのpidファイルから取得し、pidファイルがない場合は無効化マーカーを書いてから再確認し、なお存在しなければ記録のみ、存在すれば照合付きkillを行えば PASS。 |
| PAGE-abort-004 | kill対象不在状態でのプロセス操作なしstopped記録 | spec/pages/index.md#abort | pending/failed/completed のようにkill対象が存在しない状態では、プロセス操作なしでstoppedを記録すれば PASS。 |
| PAGE-abort-005 | ロック取得とCLI操作によるstopped確定 | spec/pages/index.md#abort | tickと同一のロックを取得して実行し、次のtickを待たずCLI操作自体でstoppedを確定させ、凍結要因として「人間によるabort」を記録すれば PASS。 |
| PAGE-abort-006 | stopped記録後の通知手順 | spec/pages/index.md#abort | requirements §8 と同一の手順(notify_cmd実行→notified_at追記)で通知し、notify_cmd実行に失敗した場合はnotified_atを残さず次のtickが再通知(at-least-once)すれば PASS。 |
| PAGE-abort-007 | 成功時の終了 | spec/pages/index.md#abort | stoppedを記録した旨(kill有無を含む)を表示して0で終了し、notify_cmd失敗時もstopped記録が完了していれば0とし通知失敗と次tick再通知の旨を警告表示すれば PASS。 |
| PAGE-abort-008 | すでにstoppedの場合の冪等な成功 | spec/pages/index.md#abort | 対象が既にstoppedならその旨を表示して0で終了し、何も変更しなければ PASS。 |
| PAGE-abort-009 | kill操作失敗時の拒否 | spec/pages/index.md#abort | 照合一致後のシグナル送出やジョブ終了自体のエラー時は状態を変更せず原因を表示して非0で終了し、stoppedを記録しなければ PASS。 |
| PAGE-retry-001 | retry コマンド | spec/pages/index.md#retry | `pulsen retry <task-id>` の構文で凍結(stopped)したタスクを再試行する責務であれば PASS。 |
| PAGE-retry-002 | カウンタリセットとpending化 | spec/pages/index.md#retry | attempt_count・judge_attempt_count・spawn_fail_count をリセットし実行状態をpendingに戻し、現在のタスクステータスから再実行、worktreeは前回の状態を引き継げば PASS。 |
| PAGE-retry-003 | 成功時の終了 | spec/pages/index.md#retry | pendingに戻した旨を表示して0で終了すれば PASS。 |
| PAGE-retry-004 | stopped以外へのretry拒否 | spec/pages/index.md#retry | stopped以外の状態へのretryを拒否して非0で終了し、failedは「放置すれば自動リトライされる」、pendingは「既に実行待ち」、completedは「判定済み、次のtickが次ステータスへ遷移させる」、launching/runningは「実行中、止めたい場合は先にabort」と案内すれば PASS。 |
| PAGE-set-status-001 | set-status コマンド | spec/pages/index.md#set-status | `pulsen set-status <task-id> <status>` の構文でタスクステータスを手動遷移させる責務であれば PASS。 |
| PAGE-set-status-002 | 遷移先のスナップショット定義存在検証 | spec/pages/index.md#set-status | 遷移先がスナップショットされた定義に存在することを検証し、遷移経路自体には制約を課さなければ PASS。 |
| PAGE-set-status-003 | 受理時のカウンタリセットとpending化 | spec/pages/index.md#set-status | 受理時に遷移先の動作種別によらず一律にattempt_count・judge_attempt_count・spawn_fail_countをリセットし実行状態をpendingにし(stoppedなら凍結が解ける)、次のtickの動作を遷移先の定義に委ねれば PASS。 |
| PAGE-set-status-004 | 成功時の終了 | spec/pages/index.md#set-status | 遷移した旨を表示して0で終了すれば PASS。 |
| PAGE-set-status-005 | launching/runningタスクへの拒否 | spec/pages/index.md#set-status | launching/runningのタスクに対しては拒否して非0で終了し、「先にabortせよ」と案内すれば PASS。 |
| PAGE-set-status-006 | 遷移先不在時の拒否 | spec/pages/index.md#set-status | 遷移先がスナップショット定義に存在しない場合、定義済みステータスの一覧を添えて非0で終了すれば PASS。 |
| PAGE-wrapper-001 | wrapper コマンド(内部) | spec/pages/index.md#wrapper内部コマンド | ツール自身のバイナリをラッパーモードで再実行する内部サブコマンドとして、tickのデタッチ起動でのみ使われ、利用者向けインターフェースやヘルプ一覧に現れなければ PASS。 |
| PAGE-wrapper-002 | 同定情報の書き込み | spec/pages/index.md#wrapper内部コマンド | starttime→pidの順で自身の同定情報をrunディレクトリへ書き込めば PASS。 |
| PAGE-wrapper-003 | 無効化マーカー確認による起動抑止 | spec/pages/index.md#wrapper内部コマンド | 無効化マーカーを確認し、存在すればエージェントを起動せず終了すれば PASS。 |
| PAGE-wrapper-004 | エージェント起動とログ・exitファイルの書き込み | spec/pages/index.md#wrapper内部コマンド | エージェントを起動し、stdout/stderrをログへリダイレクトし、終了後にexitファイルを書き込めば PASS。 |
| PAGE-wrapper-005 | 結果の観測点 | spec/pages/index.md#wrapper内部コマンド | 利用者が直接観測せず、結果がすべてrunディレクトリのファイル(pid/starttime/exit/ログ)として現れれば PASS。 |
| PAGE-common-007 | 出力形式 | spec/pages/index.md#共通事項 | 全コマンドの出力が人間可読なテキストであり、JSON等の機械可読形式を本フェーズでは提供しない |
| PAGE-common-008 | config.yaml 不在・パース不能時の全コマンド共通の拒否 | spec/pages/index.md#縮退状態の共通規則 | add/tick/ls/show/abort/retry/set-status のすべてが非0終了。不在時は「グローバルホームが未初期化」である旨・解決後のホームパス・作成が必要であることを、パース不能時は構文エラー・重複キーなら行・列を、スキーマ違反ならキーのパス(論理位置)を表示し、状態は変更しない(※1) |
| PAGE-common-009 | state/ 配下ディレクトリの自動作成 | spec/pages/index.md#縮退状態の共通規則 | `state/`(tasks / runs / archive)が不在でも状態を書き込むコマンドが必要に応じて自動作成する。ls は空一覧で 0、単一タスク対象コマンドは「タスク不在」として非0(※3) |
| PAGE-common-010 | ロック競合時のコマンド別の結末 | spec/pages/index.md#縮退状態の共通規則 | tick のみ 0 でスキップ、add は登録前競合のためタスクを作らず非0、abort/retry/set-status は非0、ls/show はロックを取得しないため影響なし。いずれも部分的な変更を残さない(※2) |
| PAGE-common-011 | 設定・ワークフロー定義の作成コマンドを提供しない | spec/pages/index.md#シナリオとの対応 | config.yaml / workflows/*.yaml の作成・編集用コマンドを設けず、検証は add の登録時検証が担う |
| PAGE-tick-006 | パース不能タスクファイルのスキップと報告 | spec/pages/index.md#縮退状態の共通規則 | 上書き・stopped 化を行わず当該タスクだけをスキップして報告し、処理全体を失敗させず 0 で終了する |
| PAGE-tick-007 | スナップショット破損タスクへの再通知のみの実行 | spec/pages/index.md#縮退状態の共通規則 | stopped かつ notified_at なしの場合の再通知だけを行い(at-least-once の維持)、起動・spawn確認・観測・判定・遷移・終端処理はすべてスキップして報告する(※5) |
| PAGE-tick-008 | アーカイブ済みタスクの走査対象外 | spec/pages/index.md#縮退状態の共通規則 | `state/archive/` のタスクを tick の走査対象に含めない |
| PAGE-tick-009 | worktree 不在時の扱い | spec/pages/index.md#縮退状態の共通規則 | 進行中の worktree 消失はエージェント実行の失敗として既存経路に落とし、クリーンアップでは削除済み扱いで続行する(※9) |
| PAGE-show-007 | タスク不在・パース不能時の拒否 | spec/pages/index.md#縮退状態の共通規則 | tasks / archive のいずれにも無い場合、およびタスクファイルがパース不能な場合に、書き込みを行わず原因を表示して非0 |
| PAGE-show-008 | アーカイブ済みタスクの表示 | spec/pages/index.md#縮退状態の共通規則 | アーカイブから読んで表示し、アーカイブ済みであること・worktree は削除済みであることを明示して 0(※4) |
| PAGE-show-009 | スナップショット破損時の注記付き表示 | spec/pages/index.md#縮退状態の共通規則 | タスクファイル由来の項目は表示し、スナップショット由来の項目に読めない旨を注記して 0(※6) |
| PAGE-show-010 | runディレクトリ・中身の不在の注記 | spec/pages/index.md#縮退状態の共通規則 | runディレクトリやその中身(pid / starttime / exit / ログ)が不在でも、不在である旨を注記して 0 |
| PAGE-show-011 | workspace_path の存在検証を行わない | spec/pages/index.md#縮退状態の共通規則 | worktree が手動削除されていても workspace_path を表示するのみで存在検証を行わない(※9) |
| PAGE-abort-010 | タスク不在・アーカイブ済み・パース不能時の拒否 | spec/pages/index.md#縮退状態の共通規則 | いずれの場合も書き込みを一切行わず原因を表示して非0 |
| PAGE-abort-011 | launching で runディレクトリ自体が不在の場合の処理 | spec/pages/index.md#縮退状態の共通規則 | ディレクトリを作成して無効化マーカーを書き、pidファイルを再確認したうえで stopped の記録のみを行う(遅延起動ラッパーの排除プロトコルを維持)。running 以降は同定子をタスクファイルから取るため runディレクトリに依存しない(※8b) |
| PAGE-abort-012 | スナップショット破損時も通常どおり動作 | spec/pages/index.md#縮退状態の共通規則 | abort は kill と stopped 記録のみでスナップショットに依存しないため、破損時も通常どおり動作する(※7) |
| PAGE-retry-005 | タスク不在・アーカイブ済み・パース不能時の拒否 | spec/pages/index.md#縮退状態の共通規則 | いずれの場合も書き込みを行わず非0 |
| PAGE-retry-006 | スナップショット破損時の受理と警告 | spec/pages/index.md#縮退状態の共通規則 | スナップショットに依存しないため受理して 0。ただし pending に戻しても tick に拾われないため、スナップショットの修復が必要である旨を警告表示する(※7) |
| PAGE-set-status-007 | タスク不在・アーカイブ済み・パース不能時の拒否 | spec/pages/index.md#縮退状態の共通規則 | いずれの場合も書き込みを行わず非0 |
| PAGE-set-status-008 | スナップショット不在・パース不能時の拒否 | spec/pages/index.md#縮退状態の共通規則 | 遷移先の検証にスナップショットが必要なため、状態を変更せず非0で拒否する(※7) |
| PAGE-tick-010 | サマリーの報告(errors)の見出しの規約 | spec/pages/index.md#tick | 報告の見出しが「失敗を記録 / 起動の結果が未確定 / スキップ / 後始末が残っている」の4つに固定され、見出しの軸が「タスクファイルに何を残したか」ではなく「報告が何を残したか＝運用者が次に取る行動」であれば PASS |
| PAGE-wrapper-006 | 終了コードの規約 | spec/pages/index.md#wrapper内部コマンド | 終了コードがラッパー自身の責務の達否を表し、エージェントを実行した場合と、エージェントを起動せずに終えた場合(無効化マーカーがあった場合と、マーカーの確認自体に失敗して安全側に倒した場合の両方)は 0、同定情報一式を残せずに終えた場合と起動引数が不正な場合は非0であり、エージェントの終了コードを伝播しなければ PASS |
