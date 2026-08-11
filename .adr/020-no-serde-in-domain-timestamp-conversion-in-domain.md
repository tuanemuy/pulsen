# 020: ドメイン型に serde を実装せず、`Timestamp` の RFC3339 変換はドメインに持たせる

## ステータス

提案中

## コンテキスト

タスクファイル(JSON)とグローバル設定・ワークフロー定義(YAML)の読み書きが必要になる。ドメイン型に `#[derive(Serialize, Deserialize)]` を付ければ実装は短くなるが、`pulsen-domain` が serde に依存する(ADR-019 に反する)。加えて spec は「デコード(YAML/JSON → ドメイン型)はアダプター境界の責務」であり、復号の失敗を `Corrupt` / `SnapshotUnreadable` に**区別して**写像することを求めている。

当初は `Timestamp` ↔ RFC3339 の変換も `pulsen::util::rfc3339` に置く方針だったが、前提が2点崩れた。

- spec/domains/task.md の `Timestamp` は「生成: `Clock` ポート、または RFC3339 文字列のパース」と定めており、変換をドメインの外に出すのは spec からの逸脱になる
- TC-port-clock-002 は「`now` の返値を RFC3339 に直列化して再パースし、元の `Timestamp` と等価」を要求するが、`pulsen-domain` のみに依存する適合スイート(ADR-019)からは `pulsen::util` を呼べず、このケースが書けない

## 決定

- ドメイン型は serde を持たない。アダプター側に永続化 DTO(serde derive 付き)を定義し、DTO ↔ ドメインの変換で必ず `parse` / `rehydrate` を通す
- `Timestamp` はドメインで Unix 秒(UTC・秒精度)を保持し、`parse_rfc3339(&str) -> Result<Self, TimestampError>` と `to_rfc3339(&self) -> String` を**ドメインに持たせる**。暦計算は proleptic Gregorian の days-from-civil / civil-from-days を自前で書く(外部クレート不要)。受理形式は `YYYY-MM-DDTHH:MM:SSZ` のみに限定する(spec の直列化表現がこれ1つであり、オフセット付き表記やサブ秒を受理すると往復可能性が壊れる)
- **日数と秒への分解は `div_euclid` / `rem_euclid` を使う**。Rust の `/` / `%` はゼロ方向に丸めるため、epoch より前(負の Unix 秒)で1日ずれる(実測: `-1 / 86400 = 0`・`-1 % 86400 = -1` に対し `div_euclid = -1`・`rem_euclid = 86399`)
- **`Clock` 実装が `Timestamp` を作る唯一の口を `Timestamp::from_unix_secs(secs: i64) -> Result<Self, TimestampError>` としてドメインに置く**。この口がないと (a) ドメインに場当たりの生成関数を足す、(b) アダプターで暦計算を再実装する のどちらかに流れる。`parse_rfc3339` と合わせ、生成経路はこの2つだけになる
- **`Timestamp` の表現可能範囲を 0001-01-01T00:00:00Z 〜 9999-12-31T23:59:59Z に閉じる**。`format!("{:04}", y)` は5桁年で単に桁が増え、20文字固定の `parse_rfc3339` が受け付けないため、範囲を閉じないと `to_rfc3339 ∘ parse_rfc3339 = id` が全域で成り立たない(実測確認)。範囲外の壁時計を `Clock` がどう扱うかは ADR-036
- この結果、`time` クレートは不要になる(ADR-023)。`util::rfc3339` モジュールは設けない
- 直列化形式(JSON のキー名・列挙値の綴り)はアダプターの関心事であり、spec の型名に機械的に一致させる義務を負わない

## 影響

- 「不正な状態を型で表現不能にする」が直列化経路でも保たれる(デシリアライズが不変条件を迂回しない)。`Corrupt` / `SnapshotUnreadable` の判定を DTO 層で自然に書ける。適合スイートが Clock TC-002 を1テストとして書ける
- トレードオフ: DTO とドメイン型の二重定義になり、フィールド追加のたびに2箇所を触る。暦計算をドメインに持つ(純粋関数なのでユニットテストで網羅できる)
