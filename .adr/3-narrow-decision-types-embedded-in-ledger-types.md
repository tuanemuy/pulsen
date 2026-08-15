# ADR: 規則が値を絞る返り値は専用の狭い型にし、台帳が要求する広い型へは `From` で埋め込む

## ステータス

承認済み

## コンテキスト

ドメインの分類関数のいくつかは、規則によって返しうる値が型の変種より少ない。規則の担保が doc コメントにしか無いと、呼び出し側の網羅 `match` に到達不能なアーム(`unreachable!` を含む)が残り、型は規則について何も述べない。

同じ形が2箇所にあった。

- **running の分類。** 分類は2段で行う。1段目(exit の有無)は観測を行うユースケース側にあり、`RunningClassifier::classify_alive` が受け持つのは2段目(生存)だけである — exit があれば実行は終了しており、生存観測の一過性の失敗で判定を遅延させない。ところが `classify_alive` の返り値は4値の `RunningDecision` のままで、ユースケース側に `RunningDecision::Judge(_) => unreachable!(...)` が現れていた
- **デフォルト判定。** 見送り(`Skipped`)は判定コマンドの exit 20 だけが生む(`.adr/2026-08-11-skipped-judgement-outcome.md`)。`JudgementService::default_judgement` はこの規則により2値しか返さないが、返り値は3値の `JudgeOutcome` のままで、ユースケース側の `Settled::by_default` に到達不能な `JudgeOutcome::Skipped` アームが生きていた

広い型を狭い型へ**置き換える**ことはできない。`spec/inventory/domain.md` の PASS 要件が、`DOM-execution-008` は `Judge(ExitCode)` を含む4値の `RunningDecision` を、`DOM-execution-004` は `Skipped` を含む3値の `JudgeOutcome` を要求している。落とすと台帳の行が満たせなくなる。

## 前提

- 台帳(`spec/inventory/domain.md`)の PASS 要件は、型の変種の数まで含めて満たすべき対象である
- 規則によって返しうる値が絞られている関数が、今後も現れる

## 決定

**規則が値を絞る返り値には、その値だけを持つ専用の型を足し、返り値をそれに絞る。広い型は残し、`From` で埋め込む。**

- `AliveDecision`(`KeepRunning` / `KillOnTimeout` / `DiedWithoutExit`)を足し、`classify_alive` の返り値をこれに絞る。ユースケースは1段目を `RunningDecision::Judge(exit)` として値にし、2段目を `.into()` で合流させてから1つの網羅 `match` で分岐する。分類を値にしてから分岐する形は `.adr/2-tick-branch-decision-as-value.md` と同じ
- `DefaultJudgement`(`Completed` / `Failed`)を足し、`default_judgement` の返り値をこれに絞る

対応は `From` の1箇所に閉じ、狭い型の全変種がそのまま写ることをユニットテストで主張する。

## 検討した代替案

- **doc コメントと `unreachable!` で担保する(従来の形)** — 規則が守られていることを型が何も述べず、呼び出し側にパニック経路が残る
- **広い型を狭い型へ置き換える** — 台帳の PASS 要件が広い型の全変種を要求しているため、置き換えると要件の行を満たせなくなる
- **広い型のまま返し、呼び出し側が事前検査で絞る** — 呼び出し側が増えるたびに同じ検査を書くことになり、書き忘れを型が捕まえない

## 影響

- 「生存の観測からは判定が導かれない」「デフォルト判定は見送りを導かない」の担保がコメントから型へ戻り、到達不能アームと `unreachable!` が消える
- ドメインのユニットテストが狭い型の変種だけを主張でき、作れない値の存在を型が述べる
- 規則で値が絞られる分類が今後現れたときも、同じ形(狭い型を足し `From` で埋め込む)で扱える
- トレードオフ: 分類の型が2つに増える。対応は `From` の1箇所に閉じる
