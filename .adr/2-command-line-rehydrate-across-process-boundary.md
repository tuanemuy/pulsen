# ADR: `CommandLine` にプロセス境界からの再構築経路を足す

## ステータス

承認済み

## コンテキスト

`WrapperLaunchSpec.agent_cmd` は `CommandLine`（`DOM-definition-023`:「`CommandTemplate::expand` の結果としてのみ生成される1トークン以上のトークン列」）である。tick はこれを argv に直列化し、ラッパーは argv から復元して `run_agent` に渡す。しかし `CommandLine` には公開コンストラクタが無く、`definition` モジュールの外では作れない。

## 決定

`CommandLine::rehydrate(tokens: Vec<String>) -> Result<Self, CommandError>`（0トークンは `Empty`）を足す。doc に「プロセス境界（ラッパーの起動引数）からの再構築専用の経路であり、検証済みのトークン列がそのまま往復することだけを保証する」と why を書く。`Task::rehydrate` / `AttemptRef::rehydrate` と同じ命名・同じ位置づけにする（`.adr/1-rehydrate-takes-field-bundle.md`）。

## 検討した代替案

- **`run_agent` の引数を `PlainCommand` に変える** — spec が `run_agent(cmd: &CommandLine, ...)` と型を明示している
- **ラッパー側でテンプレート展開をやり直す** — ラッパーが config とスナップショットを読むことになり、「config は読まない」に反する（`.adr/2-wrapper-restores-state-root-from-run-dir.md`）

## 影響

- 直列化 → 復元の往復がドメインの型で閉じ、ラッパー側に文字列のままのコマンドが漏れない。直列化の破れが `Err` として素直に書ける
- トレードオフ: 「`expand` の結果としてのみ生成される」という台帳の記述と食い違う。生成経路が2つ（展開・再構築）になることの spec 追従が必要になる
