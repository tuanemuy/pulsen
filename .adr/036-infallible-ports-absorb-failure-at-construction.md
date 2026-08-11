# 036: 無謬なポートの実装が持つ失敗は、構築時か値への写像で吸収する

## ステータス

提案中

## コンテキスト

spec のポートのうち `TaskIdGenerator::generate(&self) -> TaskId` と `Clock::now(&self) -> Timestamp` はエラーを持たない。一方、実装に使う手段は失敗しうる。

- `getrandom` は `Result` を返す(エントロピー源が使えない場合)
- `SystemTime::now().duration_since(UNIX_EPOCH)` は `Result` を返す(時計が epoch より前の場合)
- `Timestamp` の表現可能範囲は 0001-01-01〜9999-12-31 に閉じている(ADR-020)

素直に書くと呼び出しごとに `unwrap` が入る。CLAUDE.md は「パニックは不変条件違反にのみ使う」と定めており、どちらも不変条件違反ではなく実行環境の異常なので `unwrap` は規約違反になる。ポートにエラーを足すのは spec からの逸脱であり、無謬であること自体はドメイン側の設計(時刻・ID の取得で分岐を作らない)として意図されたものなので変えない。

## 決定

失敗を呼び出し時から追い出す。

- `DefaultTaskIdGenerator::new(clock) -> Result<Self, IdGeneratorInitError>`: 構築時に一度だけ `getrandom` からシードを取り、以降は内部 PRNG(SplitMix64 相当)で進める。時刻成分の取得元となる `Clock` も構築時に受け取る(ADR-026)。失敗は合成ルートが実行環境エラーとして扱う。`generate` は無謬になる
- `SystemClock::now`: `duration_since(UNIX_EPOCH)` の `Err` を epoch 前の符号付き秒に写す(`Err(e)` は `e.duration()` で epoch からの差を持つ)。パニックしない
- 表現可能範囲外の壁時計は範囲の端に飽和させる。ここは「起こったら他の何もかもが壊れている」領域であり、値を返して処理を続けるほうがパニックで tick を落とすより縮退設計に沿う

## 影響

- `unwrap` が実装から消え、ポートの無謬性が spec のまま保たれる。エントロピー取得が1回に減る
- トレードオフ: 内部 PRNG を自前で持つ。飽和は観測不能な領域なので適合テストでは検証しない
