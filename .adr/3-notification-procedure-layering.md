# ADR: 通知は定義非依存の3値だけに依存し、成否の解釈はドメイン・保存の選択は呼び出し側に置く

## ステータス

承認済み

## コンテキスト

共通手続き notify は `TASK_ID` / `WORKFLOW` / `TASK_STATUS` を env に組み、`Exited(0)` のときだけ `mark_notified(now)` → `save` する。呼び出し元は複数ある — 上限超過3経路(`Tick::commit` の `Freeze::Frozen` の枝に集約済み)、`Branch::Notify` アーム、`TaskRecord::SnapshotUnreadable` かつ未通知 stopped の再通知。

最後の1つだけ対象が `DegradedTask` で、保存は `save_degraded` になる(`spec/domains/task.md#degradedtaskスナップショット破損タスク`、PAGE-tick-007、TC-exec-tick-158)。ところが既存の集約点 `Tick::commit` は `&Task` 専用で、`DegradedTask` を通せない。通せないまま放置すると、「スナップショット破損タスクを凍結 → notify_cmd 失敗」の後に再通知が永遠に行われず、requirements §8 の at-least-once が破れる。

成否の解釈にも同じ性質がある。`CommandCompletion` の4通りから「`Exited(0)` だけが `notified_at` を書く根拠になり、非0終了・timeout・起動不能はいずれも書かずに終える」という規則は、stopped を書いたすべての経路が共有しなければ経路ごとに at-least-once が破れる。この解釈をユースケースに置くかぎり、共通化できるのは env の組み立てだけで、解釈は呼び出し側ごとに書き直しになる。

ドメインには対称の関数が既にある。`JudgementService::interpret_judge_completion` は同じ `CommandCompletion` を判定の結論へ解釈している。

## 前提

- requirements §8 の at-least-once は、stopped を書くすべての経路が同じ規則を共有することで成立する
- 通知に渡す3値(`TaskId` / `WorkflowName` / `StatusName`)はいずれもスナップショットに依存しないフィールドである
- 縮退タスク(`DegradedTask`)と `Task` の共通の遷移は、現時点では `mark_notified` の1つだけである

## 決定

**通知の実行と `mark_notified` の保存を分ける。** 通知の実行は `TaskId` / `WorkflowName` / `StatusName` の3値だけを受け取る関数にする。この3値はどちらの型からも同じ形で取れる — `DegradedTask` はスナップショットを持たないが、3つはいずれもスナップショット非依存のフィールドである。これが「通知は定義非依存」の実体になる。保存は呼び出し側が `save` / `save_degraded` を選ぶ。

`Task` を扱う経路は引き続き `Tick::commit` を通し(`Freeze` の受け渡しは `.adr/2-freeze-is-passed-by-the-caller-of-the-transition.md` のまま)、`DegradedTask` の再通知だけが `save_degraded` の経路を持つ。

**成否の解釈はドメインに置く。** `NotificationService::interpret_notify_completion(&CommandCompletion) -> NotifyOutcome`(`Delivered` / `Failed { detail }`)をドメインに置き、ユースケースの `Delivery` は `NotConfigured` / `Attempted(NotifyOutcome)` に縮小する。ユースケースに残るのは「そもそも通知を実行する構成か」という配線の分岐だけになり、通知が成功したと言える条件はドメインの1関数にしか無くなる。

**共通化のためにトレイトで抽象化はしない。** 2つの型に共通の遷移は `mark_notified` の1つだけで、抽象を先に置くと後続スライスが足す `abort` / `retry` の差異(`retry` は `DegradedTask` では警告付きで受理される)を吸収しきれない。

## 検討した代替案

- **`Task` / `DegradedTask` をトレイトで抽象化して1本の経路にする** — 共通の遷移が1つしか無い時点の抽象は、後続が足す遷移の差異を吸収できない
- **`Tick::commit` を `DegradedTask` も通せるように広げる** — `commit` の役割(保存 + 凍結の集計)が2つの型の分岐を抱えることになり、`.adr/2-freeze-is-passed-by-the-caller-of-the-transition.md` の判断が濁る
- **成否の解釈をユースケースに残す** — 通知の実行だけを共通化しても規則は共有されず、経路が増えるたびに解釈が書き直される
- **`NotifyOutcome::Failed` を分類(非0終了 / timeout / 起動不能)にする** — `.adr/2-transition-error-holds-classification-only.md` の「表示専用のエラーは分類だけを持つ」に厳密に沿うが、隣の `JudgeConclusion::JudgeFailure { detail }`(こちらは帳簿に残るため `.adr/2-persisted-explanations-come-from-domain-describe.md` で文言をドメインに置く側)と非対称になる。判定と通知で形を揃えるほうを採り、分類化は spec 追従の提起に回す

## 影響

- 通知の env 構成と成否の解釈が1箇所ずつに閉じ、保存の違いだけが呼び出し側に残る。スナップショットが破損したタスクでも at-least-once が維持される
- 通知と判定で `CommandCompletion` の解釈位置が揃う。ポートの結末を解釈する場所がドメインに一本化される
- `Tick::commit` の役割(保存 + 凍結の集計)が変わらないので、`.adr/2-freeze-is-passed-by-the-caller-of-the-transition.md` の判断がそのまま生きる
- 後続スライスが足す `AbortTask` ユースケースは、同じ関数を呼ぶだけで通知の規則を共有できる(そちらも `Task` / `DegradedTask` の両方を扱う)
- トレードオフ: 通知アームが `Task` 用と `DegradedTask` 用の2本になる。分岐は「どちらの保存を呼ぶか」だけで、通知の判断は共有される
- トレードオフ: `NotifyOutcome::Failed { detail }` は帳簿に永続化されない完成文言をドメインが持つ
