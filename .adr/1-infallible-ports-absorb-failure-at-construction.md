# ADR: 無謬なポートの実装が持つ失敗は、構築時か値への写像で吸収する

## ステータス

承認済み

## コンテキスト

spec のポートのうち `TaskIdGenerator::generate(&self) -> TaskId` と `Clock::now(&self) -> Timestamp` はエラーを持たない。一方、実装に使う手段は失敗しうる。

- `getrandom` は `Result` を返す(エントロピー源が使えない場合)
- `SystemTime::now().duration_since(UNIX_EPOCH)` は `Result` を返す(時計が epoch より前の場合)
- `Timestamp` の表現可能範囲は 0001-01-01〜9999-12-31 に閉じている(`.adr/1-no-serde-in-domain-timestamp-conversion-in-domain.md`)

素直に書くと呼び出しごとに `unwrap` が入る。CLAUDE.md は「パニックは不変条件違反にのみ使う」と定めており、どちらも不変条件違反ではなく実行環境の異常なので `unwrap` は規約違反になる。ポートにエラーを足すのは spec からの逸脱であり、無謬であること自体はドメイン側の設計(時刻・ID の取得で分岐を作らない)として意図されたものなので変えない。

## 決定

失敗を呼び出し時から追い出す。

- `DefaultTaskIdGenerator::new(clock) -> Result<Self, IdGeneratorInitError>`: 構築時に一度だけ `getrandom` からシードを取り、以降は内部 PRNG(SplitMix64 相当)で進める。時刻成分の取得元となる `Clock` も構築時に受け取る(`.adr/1-task-id-format.md`)。失敗は合成ルートが実行環境エラーとして扱う。`generate` は無謬になる
- `SystemClock::now`: `duration_since(UNIX_EPOCH)` の `Err` を epoch 前の符号付き秒に写す(`Err(e)` は `e.duration()` で epoch からの差を持つ)。パニックしない
- 表現可能範囲外の壁時計は範囲の端に飽和させる。ここは「起こったら他の何もかもが壊れている」領域であり、値を返して処理を続けるほうがパニックで tick を落とすより縮退設計に沿う

無謬な生成は「必ず作れる値」を1つ必要とする。ドメインの値は検証つきの生成口しか持たないため、その口を次の2通りで用意する。

- **ドメインの総関数**: `Timestamp::saturating_from_unix_secs(i64) -> Timestamp`。範囲の知識をドメインに置いたまま(`.adr/1-no-serde-in-domain-timestamp-conversion-in-domain.md`)、アダプターが飽和を再実装することも既定値を捏造することもなくなる
- **構築時に検証した値**: `DefaultTaskIdGenerator` は `new` で組み立て規則が `TaskId::parse` を通ることを一度検証し、その値を `generate` の既定値として持つ。組み立て規則(時刻成分は `[0-9t]`、ランダム成分は base36 の8桁)は制約を常に満たすため既定値には落ちないが、`parse` の `Result` をパニックにも `unwrap` にもせずに畳める。仮に返っても、重複IDは `TaskRepository::create` の `Conflict` が拾う

ここで禁じるのはパニックする `unwrap` / `expect` であり、既定値を与える総関数(`Result::unwrap_or` 等)は使ってよい。

## 影響

- パニックしうる `unwrap` が実装から消え、ポートの無謬性が spec のまま保たれる。エントロピー取得が1回に減る
- 後続スライスが無謬なポート(`Clock` を持つアダプター等)を足すときも、「値の口をどこに置くか」の選択肢がこの2通りに定まる
- トレードオフ: 内部 PRNG を自前で持つ。ドメインに総関数の生成口が1つ増える。飽和と既定値は観測不能な領域なので適合テストでは検証しない
