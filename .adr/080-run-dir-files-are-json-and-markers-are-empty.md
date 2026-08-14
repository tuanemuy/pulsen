# 080: run ディレクトリの各ファイルは JSON で書き、マーカーは空ファイルにする

## ステータス

承認済み

## コンテキスト

pid / starttime / exit の内容表現は spec が定めていない（ファイル名と意味だけが決まっている）。要件は3つ — (1) `Corrupt` を「不在」と区別できること、(2) 人間が直接辿れること（requirements §9）、(3) アトミック置換で書きかけが観測されないこと。

## 決定

タスクファイルと同じく `serde_json` の DTO で書く（ADR-023 / ADR-025 の作法をそのまま適用する）。

| ファイル | 内容 |
|---|---|
| `pid` | `{"pid": 4242, "kill_ident": "-4242"}` |
| `starttime` | `{"ident": "<不透明値>", "wall": "2026-08-12T09:15:30Z"}` |
| `exit` | `{"code": 0}` |
| `invalidated` | 空ファイル（存在のみが意味を持つ） |

- 復号は「JSON として読めない」または「値制約（`KillIdent` の非空・`ProcessStartTime` の非空・`Timestamp` の RFC3339）を満たさない」を `RunFileError::Corrupt { path, message }`、ファイル/ディレクトリの不在を `Ok(None)`、機構の失敗を `RunFileError::Io { message }` に写像する
- 書き込みは `util::atomic::write_atomic` を呼ぶだけにする（CLAUDE.md「個別に再実装しない」）。`write_invalidation_marker` はディレクトリを `ensure_dir` してから空バイト列を書く
- `write_atomic` が `ensure_dir` を内蔵しているため、`write_starttime` / `write_pid_file` / `write_exit` も書き込み先のディレクトリを作る。これを実装の副産物にせず**ポートの契約として明記する** — `prepare_attempt` が失敗した後でも spawn は行われる設計（状態を変えずに報告のみ）なので、ラッパーが自力でディレクトリを作って書けることが自己修復の前提になっている。契約に無いままだと、後続スライスが「write 系はディレクトリ不在で失敗する」と誤読し、適合テストの前提も定まらない

## 検討した代替案

- **素のテキスト（`4242` の1行など）** — `pid` は2値を持つので区切り規則を発明することになり、`Corrupt` の判定条件が手書きになる。JSON なら「パースできない = Corrupt」が構文で決まり、タスクファイルの復号と同じ形になる

## 影響

- 破損判定が一様になり、run ファイルの破損ケースがフック1つ（「解釈不能な内容を直接置く」）で書ける。後続スライスの `show` が exit を読むときも同じ DTO を使える
- トレードオフ: `cat exit` が `0` ではなく `{"code":0}` になる。マニュアルテストの確認手順が JSON を読む形になる
