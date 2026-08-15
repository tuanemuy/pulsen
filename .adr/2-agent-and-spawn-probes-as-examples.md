# ADR: 適合テストのエージェントとデタッチ性の検証は `examples/` のプログラムで供給する

## ステータス

承認済み

## コンテキスト

`run_agent` の適合ケース群は「exit code を制御できる」「引数どおりに出力する」「作業ディレクトリを検査する」「一定時間実行し続ける」「exit code を持たない終了をする」テスト用コマンドを要求する。デタッチ性のケースは「**呼び出し側プロセスを終了させ**、別プロセスから run ディレクトリを観測する」ことを要求し、in-process のテストでは表現できない。

シェル（`sh -c`）に頼るとクロスプラットフォームで破綻し、「シェルを介さない直接起動」を検証するケースと矛盾する。

## 決定

`crates/pulsen/examples/` に2つのプログラムを置く（`lock_holder.rs` の先例と同じ位置づけ — 利用者に見えるサブコマンドを増やさないため。`.adr/1-lock-holder-example-fixture.md`）。

- `agent_probe.rs`: 第1引数のモードで振る舞いを変えるテスト用エージェント。`exit <n>` / `print <stdout文字列> <stderr文字列>` / `check-cwd <期待パス>` / `echo-args <トークン...>` / `sleep <ミリ秒>` / `abort`。シグナル死は `std::process::abort()`（SIGABRT → 128+6）で作る — `unsafe` なしでシグナルによる終了を再現できる唯一の手段
- `spawn_probe.rs`: argv で受け取ったバイナリパス・run_dir・workspace・エージェントコマンドから `SystemProcessController` を組み立て、`spawn_wrapper` を呼んで**即座に終了する**。デタッチ性のケースはこれを起動して `wait` し、その後 run ディレクトリに starttime / pid / exit が現れることを確認する

ハーネスは `tests/common/lock.rs::holder_program()` と同じ規則（`<テストバイナリのディレクトリ>/examples/<name><EXE_SUFFIX>`）でパスを解決し、見つからなければフックが `None` を返してスキップする。

「実行権限がない実体」は POSIX でのみ作れるため、`permission_restrictions_effective()` と同じ「実際に効いたことを確認してから `Some` を返す」規則のフックにする（`.adr/1-port-conformance-suite-and-harness-hooks.md`）。

シグナル死のケースの期待は spec でも「**非0の符号化値**（POSIX 慣例では 128+シグナル番号）」であり、値そのものではない。適合スイート側は**非0の主張に留め**、`128+6` の具体値は `adapter/process.rs` の POSIX 側ユニットテストに置く。適合契約は契約の語彙で書き、具体値はそれを満たす実装の性質としてアダプター側で固定する — 適合スイートのケース関数にプラットフォーム分岐を増やさない（`pulsen-conformance` の `#[cfg]` は能力プローブに限られており、ケースの主張は分岐していない）。

## 検討した代替案

- **シェル（`sh -c`）でテストコマンドを組む** — クロスプラットフォームで破綻し、「シェルを介さない直接起動」の契約と矛盾する
- **サブコマンドとしてプローブを足す** — 利用者に見えるインターフェースが増える（`.adr/1-lock-holder-example-fixture.md` と同じ理由で採らない）

## 影響

- `run_agent` の全ケースと `spawn_wrapper` のデタッチ性が、シェルにもプラットフォーム固有コマンドにも依存せず検証できる。適合スイートはプラットフォーム非依存のまま保たれる
- トレードオフ: `examples/` にプログラムが2つ増え、適合テストが `cargo build --examples` に依存する（`lock_holder` と同じ依存なので新しい制約ではない）。`abort` によるシグナル死と実行権限のケースは Windows でスキップになる
