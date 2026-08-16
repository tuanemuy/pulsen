# ADR — Issue #9: spec 追従: Issue #1 / #2 / #3 の実装で判明した記述の食い違い

## ADR-001: 台帳の新規行は各グループの最大番号 + 1 で表の末尾に置き、テストケースの表にも行を挿入しない

### Status
Proposed

### Context

`_shared/references/spec-inventory.md` は台帳の ID について2つのことを同時に定めている。

- 「連番は spec 内の**出現順**」
- 「ID は一度振ったら**変えない**（要素が消えたら行を削除し、欠番はそのまま残す）」

本 Issue は `spec/testcases/ports/run-store.md` に適合ケースを1件足す（B6）。`spec/testcases/` の TC ID は表内の出現順で振られているため、表の途中に行を挿入すると以降の `TC-port-run-store-*` がすべてずれ、過去の検証記録・Issue のチェックリストとの対応が壊れる。domain / usecase / frontend の台帳にも新規行を7つ足すため、同じ衝突が起こる。

### Decision

**ID の安定性を出現順より優先する。**

- 台帳の新規行は、各グループの最大番号 + 1 を割り当てて表の末尾に追記する（`DOM-execution-072`〜`075` 等）。
- それに対応する spec 本体側の追加行（`spec/testcases/ports/run-store.md` の適合ケース）も、表の**末尾**に置く。
- 既存行の書き換えは要点欄のみで、ID は動かさない。

### Consequences

- 良い点: 既存 ID がひとつも動かないため、過去の完全性ゲートの結果・Issue のチェックリスト・`.thread/*/` の記録がそのまま生きる。
- トレードオフ: 台帳の連番が spec 内の出現順と一致しなくなる。台帳は ID で引くものであり出現順で読むものではないため、実用上の損失は小さい。
- 以降の spec 追従でも同じ規則を使う。テストケースの表に行を足すときは末尾に置く。

---

## ADR-002: 実装を直す判断になった2件（A2 / C5）は本 Issue の中で spec と実装を同時に変える

### Status
Proposed

### Context

25件のうち2件は「spec を書き換えると実装も変えなければ整合しない」性質を持つ。

- **A2**: Issue 本文は `WorkflowLoadError` のポート表を `Parse { error, resolved_from }` へ改めるとしている。現在の実装はポート表どおり `Parse(WorkflowParseError)` である。
- **C5**: `NotifyOutcome::Failed { detail }` を3変種の分類にすべきかという判断。設計としては分類化が正しい（`.adr/2-transition-error-holds-classification-only.md` の「表示専用のエラーは分類だけを持つ」が効く。対称に見える `JudgeConclusion::JudgeFailure { detail }` は帳簿に残るため `.adr/2-persisted-explanations-come-from-domain-describe.md` が効く側で、性質が違う）。

選択肢は3つあった。

1. 本 Issue で spec と実装を同時に変える
2. 本 Issue では spec だけを変え、実装は別 Issue にする
3. 本 Issue では現行の契約を spec に明記し、契約変更そのものを別 Issue にする

**なぜ切り出し（2・3）ではなく本 Issue で完結させるか。**

- **Issue の完了条件が実装変更を想定している。** 「6件それぞれについて、spec を言い換えるか『現状の spec が正しく実装側を直す』と判断するかを決め、**決めた側を反映する**。実装を直す判断になったものは、対応する `.adr/` エントリも更新する」。切り出しは、Issue が用意した2択の片方（実装を直す）を反映しないまま閉じることになる。
- **spec だけ先に変えると新しい乖離を作る（2 を採らない理由）。** 本 Issue の目的は spec と実装の一致の回復なので、乖離を1件残して閉じることは目的そのものに反する。
- **現行の形を spec の正本に書き込むと「書いてすぐ消す規約」が残る（3 を採らない理由）。** A2 について、`.adr/1-workflow-error-file-path-goes-into-free-form-messages.md` が回避策を採った唯一の理由は「ポート表で12種が確定しており、フィールドを増やすと1:1一致が壊れる」である。ポート表を直す Issue においてこの理由は成立せず、根拠の消えた回避策を契約節へ昇格させることになる。C5 についても、計画自身が「分類化が正しい」と判断した形と違うものを正本に書くことになる。
- **根拠 ADR が反映先として本 Issue を名指ししている。** `.adr/1-schema-error-location-is-logical.md` の影響節は「ポート表に解決先を持たせる改訂は spec 側の追従として提起する」、`.adr/3-notification-procedure-layering.md` の代替案節は「分類化は spec 追従の提起に回す」と書く。ここでさらに別 Issue へ回すと、同じ判断が3回目の Issue へ持ち越される。
- **変更コストが小さい。** A2 は5ファイル（コード4ファイル8箇所 + `crates/pulsen-conformance/HOOKS.md` の1セル）、C5 は5ファイル（`execution/mod.rs` の再エクスポートを含む。結合テストは `{ .. }` で受けているため変更を要さない）。重複する `cli/render.rs` を除いて合計9ファイル。25件の spec 追従を危険にさらす規模ではない。

### Decision

**1 を採る。** A2 / C5 は spec 本体・台帳・`crates/`・`.adr/` を同一 PR で変える。残り23件は spec の言い換えに閉じ、実装変更をこの2件の参照箇所の外へ波及させない。

- A2: `spec/domains/definition.md#workflowstore` のエラー一覧を `Parse { error: WorkflowParseError, resolved_from: PathBuf }` に改め、`spec/testcases/ports/workflow-store.md` の適合ケース13行と `spec/usecases/task.md` のエラーケース表を追従させ、`WorkflowLoadError::Parse` を参照する4ファイルと `crates/pulsen-conformance/HOOKS.md`（`TC-port-workflow-store-017` の組み立て手段）を直す。`.adr/1-workflow-error-file-path-goes-into-free-form-messages.md` と `.adr/1-schema-error-location-is-logical.md` を更新する（後者は決定理由と影響の一部が A2 で偽になるため）。
- C5: `NotifyOutcome::Failed { detail }` を `Failed { cause: NotifyFailureCause }` に改める（形は ADR-004）。`.adr/3-notification-procedure-layering.md` を更新する。

「レビューの観点も受け入れ基準も2種類混ざる」という懸念は、A2 / C5 に限れば受け入れ基準を実装側の検証まで含む形で1行ずつ書けば足りる（AC-A2 / AC-C5 / AC-Z3 / AC-Z5）。追跡が緩む度合いより、乖離を残して閉じる代償のほうが大きい。

### Consequences

- 良い点: 本 Issue の完了時点で25件すべてについて spec と実装が一致し、下流の完全性ゲートが正しい基準で走る。追跡先が失われることもない。
- 良い点: spec・実装・`.adr/` が1つの PR で動くので、どの時点でも spec ≡ 実装が保たれる。
- トレードオフ: 本 Issue の差分が `crates/` に及ぶため、レビューは「spec の言い換え23件」と「実装変更2件」の2種類を見ることになる。範囲を機械的に確認できるよう、受け入れ基準に `git diff --name-only crates/` の対象9ファイルを列挙する（AC-Z3）。
- トレードオフ: `crates/pulsen/src/cli/render.rs` を A2 と C5 の両方が触る。片方だけ直して他方の `match` が壊れたまま残らないよう、最後に `cargo test` を通す（AC-Z5）。

---

## ADR-003: tick の `errors` の分類表はユースケース層の spec に置き、ドメインには置かない

### Status
Proposed

### Context

`errors` の分類（`TickIssue` 21変種、`RunFailureCause`、`RemnantsLeft`）を spec のどこに書くかが決まっていない。分類の中にはドメインのエラー型（`TransitionError` / `RunFileError` / `InconsistentRunFiles` / `SaveError`）を内側に持つものがあるため、ドメインの spec に置く選択肢もあった。

実装では `TickIssue` / `RunFailureCause` / `RemnantsLeft` はいずれも `crates/pulsen/src/application/tick/mod.rs`（ユースケース層）にある。ドメインのエラー型は**埋め込まれる側**であって、分類そのものはドメインの語彙ではない。

### Decision

**分類表は `spec/usecases/execution.md` の「出力DTO(サマリー)」に置く。** ドメインのエラー型（`TransitionError` 等）は `spec/domains/` が定義し、分類はそれを内側に持つ形で参照する。表示の見出しへの振り分けは `spec/pages/index.md` に置き、3層（ドメインのエラー型 → ユースケースの分類 → 表示の見出し）が別々の spec ファイルに1回ずつ現れるようにする。

### Consequences

- 良い点: `errors` の分類が増えても変更先が1ファイルに閉じる。ドメインが tick の報告の都合を知らずに済み、依存方向（外→内）が spec 上でも保たれる。
- 良い点: 「文言は CLI 層が組み立てる」（`.adr/2-tick-errors-are-structured-values.md`）が spec の構造としても表れる。
- トレードオフ: 分類・見出しの対応を読むには usecases と pages の2ファイルを見る必要がある。対応は pages 側に一覧として置く。

---

## ADR-004: 通知の失敗の原因は `Failed` の内側の分類として持ち、`NotifyOutcome` を平坦化しない

### Status
Proposed

### Context

C5 を本 Issue で反映する（ADR-002）と決めた結果、`NotifyOutcome::Failed { detail: String }` を分類に改める形を決める必要がある。原因は3つ — 非0終了・timeout・起動不能。形の候補は2つあった。

1. 平坦化する: `NotifyOutcome = Delivered | ExitedNonZero { exit } | TimedOut | FailedToStart { message }`（4変種）
2. 内側に持つ: `NotifyOutcome = Delivered | Failed { cause: NotifyFailureCause }`、`NotifyFailureCause = ExitedNonZero { exit } | TimedOut | FailedToStart { message }`

`NotifyOutcome` の2分岐は単なる分類ではなく、requirements §8 の at-least-once そのものである — 「`Delivered` だけが `notified_at` を書く根拠になる」という規則を、stopped を書くすべての経路（tick の各上限超過・DegradedTask の再通知・後続の abort）が共有する。

### Decision

**2 を採る。**

- `Failed` の内側に `NotifyFailureCause` を置く。呼び出し側は `Delivered` / `Failed` の2分岐のまま「書くか書かないか」を決められ、原因を見るのは表示層だけになる。
- `TimedOut` はフィールドを持たない。通知の timeout は設定値ではなく組み込み定数 `NotificationService::NOTIFY_TIMEOUT` の1つに定まるため、秒数は表示側が定数を読む（値を運ぶと同じ数が2箇所に現れる）。
- `TickIssue::NotifyFailed` も `message: String` ではなく `cause: NotifyFailureCause` を持ち、文言の組み立ては `cli::render` に閉じる。

1 を採らないのは、平坦化すると「`notified_at` を書かない」という判断を呼び出し側が3変種の列挙で表すことになり、at-least-once の根拠が型から読めなくなるため。経路が増えるたびに列挙が増え、1つ漏らすと at-least-once がその経路だけで破れる。

### Consequences

- 良い点: `.adr/2-transition-error-holds-classification-only.md` の「表示専用のエラーは分類だけを持つ」が満たされ、完成文言がドメインから消える。
- 良い点: 既に同じ形の前例がある（`TickIssue::RunFailed { cause: RunFailureCause }`、`.adr/3-run-failure-cause-and-remnants-as-classifications.md`）。分類を内側に持つ規則が2箇所で揃う。
- 良い点: 呼び出し側（`tick/notify.rs` の2アーム）の分岐の形が変わらないため、C5 の変更が `match` の書き換えではなくフィールドの付け替えに収まる。
- トレードオフ: 型が1つ増える。`NotifyOutcome` だけを読んでも原因の3値は見えず、`NotifyFailureCause` まで辿る必要がある。

---

## ADR-005: 解決先パスは構造化フィールドでのみ示し、自由形式メッセージへの前置は構造化フィールドを持たない変種にだけ残す

→ `.adr/9-resolved-path-only-in-structured-fields.md` に昇格

### Status
Proposed

### Context

A2 で `WorkflowLoadError::Parse` が `resolved_from: PathBuf` を持つようになると、「どの変種が解決先パスを自由形式のメッセージに前置するか」を決め直す必要がある。実コードの現状は次のとおり。

`WorkflowLoadError`（`crates/pulsen-domain/src/definition/port.rs`）の3変種:

| 変種 | 解決先を持つ形 | 現在の前置 |
|---|---|---|
| `NotFound { attempted: PathBuf }` | 構造化フィールド | なし（`render.rs:529` が `attempted` から出す） |
| `Parse(WorkflowParseError)` → A2 で `Parse { error, resolved_from }` | A2 後は構造化フィールド | `YamlSyntax` のみ `at()` で前置（`adapter/workflow_store.rs:74`） |
| `Io { message }` | **持たない** | `at()` で前置（`adapter/workflow_store.rs:99`） |

`WorkflowParseError`（`crates/pulsen-domain/src/definition/assembler.rs:25-94`）の12変種（`YamlSyntax` / `UnknownKey` / `ForbiddenKey` / `MissingInitial` / `InitialNotFound` / `EmptyStatuses` / `NoAction` / `MultipleActions` / `UnknownRunValue` / `MissingNext` / `NextNotFound` / `InvalidValue`）は、**すべて `Parse` の内側にある**。つまり `Parse { resolved_from }` の1フィールドで12変種すべての解決先が示せる。

A2 後、`cli/render.rs` の `Parse` アームは `resolved_from` を無条件に案内へ出す。このとき `YamlSyntax { message }` の前置を残すと、構文エラー時に「解決を試みたパス: /x/wf.yaml」と「YAML 構文エラー: /x/wf.yaml: …」が同じ案内に並ぶ。

### Decision

**構造化フィールドで解決先を示せるようになった経路は、自由形式のメッセージへの前置をやめる。前置を残すのは構造化フィールドを持たない変種だけ。**

- `WorkflowParseError::YamlSyntax { message }` の前置を外す（`adapter/workflow_store.rs:74` の `at(&resolved, &error.message)` をやめ、`message` は原因のみにする）。
- `WorkflowLoadError::Io { message }` の前置は残す。解決先を載せられる構造上の置き場が無く、CLI 側も解決先を知らないため。
- `at()` の doc（`adapter/workflow_store.rs:105-106`）から `WorkflowParseError::YamlSyntax` の記述を外し、利用者を `read_error` の1経路にする。

「変種ごとに前置するかを個別に決める」のではなく「構造化フィールドで示せるなら前置しない」という規則にする。`WorkflowParseError` に変種が増えても判断が要らず、二重表示が構造として起こらない。

### Consequences

- 良い点: 同じパスが1つの案内に2回現れない。解決先の出所が `render.rs` の1箇所に定まり、表示の形を変えるときの変更先も1箇所になる。
- 良い点: `.adr/1-schema-error-location-is-logical.md` が「スキーマ違反にパスが出ない」トレードオフとして残していた記述と、実装が同じ方向で解消される（前置の有無で変種ごとに挙動が割れない）。
- トレードオフ: `at()` の利用者が1つに減り、関数として残す価値が薄くなる。`read_error` の中へ畳む選択もあるが、25件に由来しない整理なので本 Issue では触らない。
- 検証: 前置を外しても既存テストは通る。受け入れテスト `crates/pulsen/tests/cli_add_error.rs:197 / 207` は `["YAML 構文エラー", "位置:", "行"]` の3語しか見ておらず、適合スイート `crates/pulsen-conformance/src/workflow_store.rs:435-437` も `message` の非空と `location` の存在しか主張していない。

---

## ADR-006: 置き換わった `.adr/` エントリは決定節を決定時点のまま保ち、置き換えは Status と影響節が述べる

→ `.adr/9-superseded-adr-keeps-its-original-decision.md` に昇格

### Status
Proposed

### Context

`.adr/` のエントリは「その時点で何を材料に何を決めたか」の記録である。後の変更で決定が置き換わったとき、決定節を新しい規則へ書き換えると、記録としての中身（当時の前提・退けた代替案・負ったトレードオフ）が失われ、同じ文書が「当時の記録」と「現行の規則」の両方を名乗ることになる。前提を述べるコンテキスト節と決定節が反転して読めるのもこのときである。

A2 で `.adr/1-workflow-error-file-path-goes-into-free-form-messages.md` の決定（解決先パスは自由形式のメッセージにだけ前置する）が置き換わり、この扱いを決める必要がある。

### Decision

**決定が置き換わったエントリは、決定節と代替案節を決定時点の文面のまま保つ。** 代替案節の各行は当時検討して退けた記録なので、後から採ったものも含めて消さない。

- Status に `置き換え済み(Issue #N)` と、置き換え後の形を書く
- 影響節に「どの代替案を採ったか」「退けた理由がなぜ成立しなくなったか」「当時のトレードオフがどうなったか」と、置き換え後に生きている規則の所在（spec の契約）を書く
- タイトルに `(置き換え済み)` を添える。`.adr/` は一覧とタイトルで引かれるため、開く前に性格が分かる必要がある。ファイル名は既存の参照を壊すので変えない

**決定本体が生きていて細部だけが古くなったエントリは、Status を `承認済み` のままにして該当箇所を現行へ改訂する。** 置き換え（決定そのものが別の判断に取って代わられた）と改訂（決定は生きたまま形が細かくなった）を Status で分け、決定節を書き換えてよいのは後者だけにする。

### Consequences

- 良い点: 当時の判断材料と、それが何に置き換わったかの両方が1つの文書から読める。決定節を書き換えると前者が失われ、ADR を残す理由自体が消える
- 良い点: 現行の規則の正本は spec の契約に一本化される。同じ規則が ADR と spec の2箇所で別々に育つことがない
- トレードオフ: 現行の規則は決定節から直接は読めず、Status・影響節を経由して spec の契約へ辿ることになる
- 適用: `.adr/1-workflow-error-file-path-goes-into-free-form-messages.md` は置き換え、`.adr/1-schema-error-location-is-logical.md`（`location` は論理位置）と `.adr/3-notification-procedure-layering.md`（通知の3値分離と成否の解釈の位置）は決定本体が生きているため改訂。この2種の扱いの違いは Status から読める

---
