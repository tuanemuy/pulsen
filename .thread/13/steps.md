# 実装手順 — Issue #13

## 設計

### 触らない層

ドメイン(`crates/pulsen-domain/`)・ポート・アダプター(`crates/pulsen/src/adapter/`)・ユースケース(`crates/pulsen/src/application/`)・CLI に変更は無い。`ExclusiveLock` ポートの契約も `FileExclusiveLock` の実装も変わらない。適合スイート本体(`crates/pulsen-conformance/src/`)も変えない — `ExclusiveLockHarness` のフックが `Option` を返す契約は、別実装(in-memory 等)が同じスイートを適用できることの前提であり、能力の区別は**適用側**が持つべき知識だからである。同クレートで触るのは `HOOKS.md`(ドキュメント)だけ。

変更はテストのフィクスチャ層(`crates/pulsen/tests/`)と、その表(`crates/pulsen-conformance/HOOKS.md`)、CI の why コメント、そして最後に `.adr/` への昇格に閉じる。`.adr/071` の停止規則では「その場で扱う」側に丸ごと収まる。

### この層での「内側」— 能力の型と判定

内側から順に4段。

1. **能力の型** — 「別プロセスにロックを保持させられるか」の答えを、4つの区別を持つ enum で表す。`Available` は起動できる実行ファイルのパスを持ち、`SignalTimedOut` は「起動はできるが合図が期限内に返らない」、`ProgramMissing` は「実行ファイルが無い」、`ProgramUnusable` は「実行ファイルはあるが起動できない」でその理由を持つ。この4つが**唯一の判断の源**になる。後ろ2つを分けるのは、原因も回避方法も違うものを1つの案内に潰さないため(adr.md ADR-007)。
2. **probe** — 実際に1回保持させてみて能力を決める。`OnceLock` で1度だけ評価する。手本は `probe_permission_restrictions`(`crates/pulsen-conformance/src/lib.rs:236-237`)と `git::tmpdir_outside_repository`(`crates/pulsen/tests/common/git.rs:90`)。
3. **フィクスチャ関数** — `spawn_holder` / `hold` は能力を見てから動く。`SignalTimedOut` なら起動を試みずに `None`(＝宣言済みスキップ)、`ProgramMissing` / `ProgramUnusable` なら原因に応じた案内を添えたパニック、`Available` なら起動して合図を待ち、そこでのタイムアウトは異常としてパニック。
4. **宣言** — `conformance_lock.rs` と `common/mod.rs` の `allowed_skips()` が、同じ能力を `match` で読む。許容集合に足すのは `SignalTimedOut` のときだけ。

この順序により、`SkipBudget`(`LazyLock`)がいつ評価されても、また5件が並列に走っても、宣言と実態が一致する。

### 起動結果の型(probe の材料)

probe とフィクスチャ関数の双方が「起動を試みた結果」を必要とし、そこには `Child` が乗る。能力の型は `Child` を持てない(`'static` に置くため)ので、起動結果は別の内部 enum にする。公開するのは能力の型だけ。

起動結果は4つに分ける — 合図を読めた / 合図を読み取れなかった / 期限内に何も返らなかった / 起動できなかった。3つ目と4つ目を潰さないのは能力の型と同じ理由で、2つ目を独立させるのは、読み取りの失敗が「ロックを取得できなかった」という別の主張の失敗として現れるのを避けるため(adr.md ADR-007)。

## 実装ステップ

### 1. 能力を表す型と起動結果の型を置く

- **対象ファイル:** `crates/pulsen/tests/common/lock.rs`
- **変更内容:** 下のコード片が定めるのは型と分岐の骨子で、ドキュメンテーションコメントの最終形は `crates/pulsen/tests/common/lock.rs` を見る。文面は probe が何を測り何を測っていないかが確定してから決まり、ステップ10 の `.adr/` 昇格まで動く。
  - 公開 enum を足す。

    ```rust
    /// 別プロセスにロックを保持させられるか。
    pub enum HolderCapability {
        /// 保持プロセスを起動でき、合図の期限の超過を観測しなかった。
        Available(HolderProgram),
        /// 起動はできるが、合図が期限内に返らない。
        SignalTimedOut,
        /// 実行ファイルが無い(example がビルドされていない)。
        ProgramMissing,
        /// 実行ファイルはあるが起動できない。値は起動が失敗した理由。
        ProgramUnusable(io::Error),
    }

    /// probe が解決した保持プロセスの実行ファイル。パスは `lock.rs` の外へ出さない。
    pub struct HolderProgram(PathBuf);

    impl HolderProgram {
        fn path(&self) -> &Path {
            &self.0
        }
    }
    ```

    `Available` のペイロードを裸の `PathBuf` にしない。パスが公開されると、別のテストバイナリが能力の型から実行ファイルを取り出して `spawn_holder` の判断(不在・起動失敗・probe 成立後のタイムアウト)を迂回できてしまい、`holder_program()` を private にして得た保証(AC-2)が変種のペイロードで抜ける。

    起動が失敗した理由を `String` に畳まない。`io::Error` は `Send + Sync + 'static` なのでそのまま `OnceLock` に置け、文字列化すると `ErrorKind` が失われて「どの種類の失敗をどちら側に置くか」を後から書く材料が消える。パニック文言は `{error}` で `to_string()` と同じ内容になる。

  - 起動結果の内部 enum を足す。

    ```rust
    /// 保持プロセスの起動を試みた結果。
    enum Started {
        /// 期限内に子が応答した(合図を書いたか、書かずに終了したか)。
        /// `locked` は合図が `LOCKED` だったか。
        Signaled { holder: Child, locked: bool },
        /// 合図を読み取れなかった。値はその理由。
        SignalUnreadable { holder: Child, error: io::Error },
        /// 合図も終了も期限内に返らなかった(保持プロセスは終了させてある)。
        SignalTimedOut,
        /// 起動できなかった。
        SpawnFailed(io::Error),
    }
    ```

    最後の変種を `NotStarted` にしない。enum 名 `Started` で終わる変種名は `clippy::enum_variant_names`(既定 warn)に掛かり、CI の `-D warnings` で赤になる(AC-11)。`SpawnFailed` なら起動の失敗であることが名前で読め、`HolderCapability::ProgramUnusable` との対応も付く。

  - 実行ファイル不在を案内するメッセージを定数にする。案内の文面と、それをスキップにしない理由を1箇所に置くため。

    ```rust
    /// 実行ファイルが無いときの案内。環境の能力ではなくビルド構成の誤りなので、
    /// スキップにはせず失敗させる(HOOKS.md / ADR-068)。
    const PROGRAM_MISSING: &str = "ロック保持フィクスチャ(examples/lock_holder)の実行ファイルが無い。\
        単一のテストターゲットを指定した実行では example がビルドされないため、\
        `cargo test --workspace` のように example をビルドする形で実行する";
    ```

  - 保持プロセスを畳む共有ヘルパーを置く。

    ```rust
    /// 保持プロセスを畳む。正常終了は待たずに `kill` してから終了を回収する。
    fn kill_and_wait(mut holder: Child) {
        let _ = holder.kill();
        let _ = holder.wait();
    }
    ```

    `kill()` と `wait()` の結果はいずれも捨てる。既に終了している子への `kill()` はエラーになりうるが、目的は「残さないこと」だけで、どちらの結果もこの先の判断に使わない。

    呼び出し元は3つの経路にまたがる。**すべてをこのヘルパーに寄せる**こと。

    - `start_holder`(ステップ2) — `stdout` の取得に失敗した経路と、合図が期限内に返らなかった経路。
    - `probe_holder`(ステップ2) — 判定が済んだあとの後始末。
    - `spawn_holder` の `SignalUnreadable` 腕と `hold` の `!locked` 腕(ステップ3) — パニックする直前の後始末。

    ADR-005 が「stdin を閉じて正常終了を待つ `release` を使うのは、正常に保持できたプロセスを畳むときだけ」という基準を1本に通した結果として置かれる関数なので、置き場と呼び出し元を先に決めておく。基準が掛かるのは probe と `lock.rs` の失敗経路の2つで、適合ハーネスの `try_acquire_from_other_process`(`locked` の値によらず `release` を呼ぶ)は射程の外に置く。関数として立てずに各所へ `kill()` + `wait()` を書くと、片方の経路だけ `release` のまま残る。
  - `SIGNAL_DEADLINE`(14-16行)のドキュメンテーションコメントを、期限超過の意味が2つに分かれたことに合わせて改める。

    ```rust
    /// 合図を待つ期限。probe ではこの期限を超えたことが「この環境は合図を期限内に返せない」
    /// という能力の判定になり、probe が成立したあとに超えた場合は、同じ手順が一度は期限内に
    /// 返っている以上その超過を能力の宣言としては読めないので失敗になる。いずれの場合も
    /// 待ち続けないのは、フィクスチャのハングがテストの失敗より診断が難しいため(ADR-060)。
    ```

  - `use std::io;` を足す。
- **理由:** 原因の違う失敗を型で区別する。`ProgramMissing` と `SignalTimedOut` を同じ `None` に潰していたことが本Issueの原因(`research.md` の乖離2)で、同じ潰し方を「起動できない」「合図を読めない」に対して繰り返さない。

### 2. probe を置き、`holder_program` を内部に閉じる

- **対象ファイル:** `crates/pulsen/tests/common/lock.rs`
- **変更内容:**
  - `holder_program()` を `pub` から private にする。ドキュメンテーションコメントの「不在は『前提を作れない環境』として扱う」を「不在はフィクスチャがビルドされていないことを意味し、環境の能力ではない」に改める。
  - 起動処理を `Started` を返す関数に切り出す。現行 `spawn_holder`(33-56行)の本体を移し、`?` と `.ok()?` で潰していた分岐を `Started` の4値に振り分ける。

    ```rust
    /// 保持プロセスを起動し、合図が期限内に返るかまでを見る。
    fn start_holder(program: &Path, lock_path: &Path) -> Started {
        // spawn が Err → Started::SpawnFailed(error)
        // stdout の取得に失敗 → kill + wait して Started::SpawnFailed(io::Error::other(..))
        // recv_timeout が Err(Timeout) → kill + wait して Started::SignalTimedOut
        // recv_timeout が Err(Disconnected) → Started::SignalUnreadable { holder, error }
        // 読み取りエラー(現行の Ok(None)) → Started::SignalUnreadable { holder, error }
        // 合図を受け取った → Started::Signaled { holder, locked: signal.trim() == LOCKED }
    }
    ```

    読み取りスレッドは `io::Result<String>` をそのまま送り、`recv_timeout` の失敗は `Timeout` と `Disconnected` に分けて `_` で潰さない。`Disconnected`(読み取りスレッドが結果を返さずに消えた)は期限を1ミリ秒も測っていないので、許容集合側に落ちる `SignalTimedOut` に倒さない。

    読み取りエラーを `Signaled { locked: false }` に寄せない。寄せると「合図を読めなかった」が `hold` では `!locked` の失敗(取得の合図が返らなかった)として、`try_acquire_from_other_process` 経由では TC-004 の「ドロップで解放され、別プロセスが取得できる」の失敗として現れ、どちらも実際の原因を述べない。
  - probe と公開の入口を足す。

    ```rust
    /// 実際に1回保持させてみて能力を決める(1度だけ評価して使い回す)。
    ///
    /// フィクスチャが本番で踏む手順そのもの(起動して合図を待つ)で判定するため、
    /// 判定と実際のスキップが食い違わない(ADR-055 の probe_permission_restrictions
    /// と同じ性質)。
    pub fn holder_capability() -> &'static HolderCapability {
        static CAPABILITY: OnceLock<HolderCapability> = OnceLock::new();
        CAPABILITY.get_or_init(probe_holder)
    }

    fn probe_holder() -> HolderCapability {
        let Some(program) = holder_program() else {
            return HolderCapability::ProgramMissing;
        };
        let dir = tempfile::tempdir().expect("一時ディレクトリを作れる");
        match start_holder(&program, &dir.path().join("lock")) {
            Started::Signaled { holder, locked: _ }
            | Started::SignalUnreadable { holder, error: _ } => {
                kill_and_wait(holder);
                HolderCapability::Available(HolderProgram(program))
            }
            Started::SignalTimedOut => HolderCapability::SignalTimedOut,
            Started::SpawnFailed(error) => HolderCapability::ProgramUnusable(error),
        }
    }
    ```

    後始末に `release`(stdin を閉じて正常終了を待つ)を使わない。probe が測っているのは「合図が期限内に返るか」だけで、正常終了できるかは測っていない。`release` の `wait()` には期限が無く、しかも probe は `OnceLock::get_or_init` の中なので、子プロセスが終了しない環境では `holder_capability()` を呼ぶ全スレッドがそこで止まる。`kill()` + `wait()` にすれば、後始末が測定対象から独立する(ADR-060 が期限を置いた理由と同じ)。`start_holder` のタイムアウト経路と共通の小さなヘルパー(`kill_and_wait`)にまとめる。

    読み取りエラー(`SignalUnreadable`)を `Available` に数えるのは、probe が測るのが「合図の期限を超えたか」だけで、読み取れたかどうかを能力の判定に入れないため(adr.md ADR-005)。読み取りの異常はケース側で失敗として扱う。
  - `use std::sync::OnceLock;` を足す。probe が使う一時ディレクトリは本番のロックとぶつからないよう `tempfile::tempdir()` で作り、判定が終わるまで保持する。
- **理由:** 宣言を「実行ファイルの有無」から「環境の能力」へ移す(`.adr/055` / `.adr/071`)。`holder_program()` を private にすることで、古い述語を使う呼び出し側が残らないことをコンパイラが保証する(AC-2)。

### 3. `spawn_holder` / `hold` を能力の上に載せる

- **対象ファイル:** `crates/pulsen/tests/common/lock.rs`
- **変更内容:**
  - `spawn_holder` を書き換える。シグネチャ `pub fn spawn_holder(lock_path: &Path) -> Option<(Child, bool)>` は変えず、**`None` の意味を「この環境は合図を期限内に返せない」1つに絞る**。

    ```rust
    /// 保持プロセスを起動し、ロックを取得できたかを添えて返す。
    ///
    /// `None` はこの環境が合図を期限内に返せないこと(宣言済みスキップ)だけを意味する。
    /// 実行ファイルの不在・起動の失敗・合図の読み取り失敗と、probe が成立したあとの
    /// タイムアウトは、いずれも環境の能力ではないのでスキップに逃がさず失敗させる。
    pub fn spawn_holder(lock_path: &Path) -> Option<(Child, bool)> {
        let program = match holder_capability() {
            HolderCapability::Available(program) => program,
            HolderCapability::SignalTimedOut => return None,
            HolderCapability::ProgramMissing => panic!("{PROGRAM_MISSING}"),
            HolderCapability::ProgramUnusable(error) => panic!(
                "ロック保持フィクスチャ(examples/lock_holder)を起動できなかった\
                 (probe の起動時に観測した理由): {error}"
            ),
        };
        match start_holder(program.path(), lock_path) {
            Started::Signaled { holder, locked } => Some((holder, locked)),
            Started::SignalUnreadable { holder, error } => {
                kill_and_wait(holder);
                panic!("保持プロセスの合図を読み取れなかった: {error}")
            }
            Started::SignalTimedOut => panic!(
                "保持プロセスの合図が {SIGNAL_DEADLINE:?} 以内に返らなかった。\
                 probe は同じ手順で成立している(この環境で繰り返し起きるなら SIGNAL_DEADLINE を見直す)"
            ),
            Started::SpawnFailed(error) => panic!(
                "ロック保持フィクスチャ(examples/lock_holder)を起動できなかった: {error}"
            ),
        }
    }
    ```

    `SignalTimedOut` のときに起動を試みないのは、5件それぞれで期限ぶん待たせないため。`ProgramUnusable` / `SpawnFailed` の案内に元のエラーを載せるのは、Windows Defender による隔離・権限不足・実行形式の不一致のどれなのかが、これ以外に出る場所が無いため(AC-4)。
  - `hold` を書き換え、`!locked` を `None` から外す。

    ```rust
    /// ロックを保持している別プロセスを用意する。
    ///
    /// `None` の意味は `spawn_holder` と同じ。誰も保持していないパスで取得の合図が返らない
    /// のは環境の能力ではないので、スキップにはしない。
    pub fn hold(lock_path: &Path) -> Option<Child> {
        let (holder, locked) = spawn_holder(lock_path)?;
        if !locked {
            kill_and_wait(holder);
            panic!("保持プロセスが、誰も保持していないロックを取得した合図を返さなかった");
        }
        Some(holder)
    }
    ```

    後始末は `release` ではなく `kill_and_wait` にする。ここは保持プロセスが想定どおりに振る舞っていないと判断した直後で、これからパニックする相手の正常終了を期限なしに待つ形は、ADR-005 が probe について外した依存をこの経路に残すことになる。同じ関数の2つの失敗経路(`SignalUnreadable` と `!locked`)で後始末の基準を1本にする。

    文言は観測に忠実にする。`spawn_holder` は `stderr(Stdio::null())` で子の標準エラーを捨てており、`lock_holder` はロック競合も機構の異常もそこにしか書かない。したがって `!locked` から言えるのは「取得の合図が返らなかった」ことまでで、原因を「取得できなかった」と断定する材料をフィクスチャは持たない(理由と、stderr を拾う案を採らない理由は adr.md ADR-007)。合図を読み取れなかった場合はここに届かない(`spawn_holder` が先に落とす)。
- **理由:** AC-4 / AC-5 / AC-6。`None` が複数の意味を持つ限り、呼び出し側は区別できない。区別を能力の型と `Started` に集約し、境界の `Option` は「宣言済みスキップかどうか」だけを運ぶ。失敗を値ではなくパニックで表すのは、これがテストのフィクスチャで、`expect` と同じ位置づけだから(理由と代替案は adr.md ADR-004)。

### 4. 適合スイート適用側の宣言とハーネスを能力に寄せる

- **対象ファイル:** `crates/pulsen/tests/conformance_lock.rs`
- **変更内容:**
  - `hold_from_other_process` を `common::lock` の `hold` に委ねる(62-69行)。呼び出しは下記の `use` で取り込んだ短縮形で書く。`!locked` の判断が `hold` の中に1箇所だけある形になる。

    ```rust
    fn hold_from_other_process(&self) -> Option<Self::Holder> {
        hold(&self.lock_path())
    }
    ```

  - `try_acquire_from_other_process`(82-86行)は `spawn_holder` のまま据え置き、**変更が要らないことを確認して終える**(AC-2 がこのフックを名指ししているのは、probe の結果だけを見る状態を求めているためで、それは `spawn_holder` 経由の間接参照で既に成立する)。このフックは「別プロセスが取得できたか」を `bool` で観測する経路で、`locked == false` が正当な観測結果になりうる(TC-004 がそれを判定する)。`hold` と違ってここで `!locked` を失敗に倒すと、ケースの判定をフィクスチャが先取りしてしまう。
  - `allowed_skips()`(111-117行)を能力の `match` にする。ワイルドカードは使わない。

    ```rust
    /// この環境でスキップを許容するケース。
    ///
    /// 許容するのは保持プロセスの合図が期限内に返らない環境だけ。実行ファイルの不在や
    /// 起動の失敗は環境の能力ではないので、緑にせずケースの失敗にする
    /// (HOOKS.md / ADR-068)。同じ判定を CLI 側の受け入れテスト
    /// (TC-task-register-task-017)も使うため、両者で扱いが揃う(ADR-055)。
    fn allowed_skips() -> Vec<&'static str> {
        match holder_capability() {
            HolderCapability::SignalTimedOut => LOCK_HOLDER_CASES.to_vec(),
            HolderCapability::Available(_)
            | HolderCapability::ProgramMissing
            | HolderCapability::ProgramUnusable(_) => Vec::new(),
        }
    }
    ```

  - `LOCK_HOLDER_CASES` の doc コメント(97行)を「別プロセスにロックを保持させられない環境」から「保持プロセスの合図が期限内に返らない環境」に改める。
  - `use` を `common::lock::{HolderCapability, hold, holder_capability, release, spawn_holder}` に整理し、このファイルに `common::lock::` のフルパス呼び出しを残さない。由来が1点であることを型で示したファイルで、綴りだけが違って見える形を作らないため。
- **理由:** AC-2 / AC-3 / AC-6。適合スイート4件の許容集合がタイムアウト側だけを受け入れるようになる。

### 5. 受け入れテスト側の宣言を同じ能力に寄せる

- **対象ファイル:** `crates/pulsen/tests/common/mod.rs`
- **変更内容:**
  - `allowed_skips()`(41-53行)のロック分岐を書き換える。

    ```rust
    match lock::holder_capability() {
        lock::HolderCapability::SignalTimedOut => allowed.extend(LOCK_HOLDER_CASES),
        lock::HolderCapability::Available(_)
        | lock::HolderCapability::ProgramMissing
        | lock::HolderCapability::ProgramUnusable(_) => {}
    }
    ```

  - `LOCK_HOLDER_CASES` の doc コメント(31行)を、`conformance_lock.rs` と同じ言葉に揃える。
- **理由:** AC-2 / AC-3。`tc_task_register_task_017` は同じフィクスチャを使うので、適合スイート側と扱いが割れてはいけない。`cli_add_error.rs` の呼び出し側(123-140行)は `hold()` の `None` をスキップとして扱う形のままで変更不要 — `None` の意味が絞られたことで、この分岐がタイムアウトだけを指すようになる。

### 6. HOOKS.md の該当行を新しい述語に改める

- **対象ファイル:** `crates/pulsen-conformance/HOOKS.md`
- **変更内容:**
  - 「環境で走らなくなりうる行」の表(43行目)を書き換える。
    - 前提を作れない環境: 「別プロセスにロックを保持させる実行ファイルが無い(単一テストターゲットを指定した実行では example がビルドされない)」→ 「保持プロセスの合図が期限内に返らない」。**原因(初回起動のスキャン・高負荷など)の推定を添えない。** 期限を超えたことしか観測していないので、正本に事実の形で書けるのはそこまでである。**適用側の定数名(`SIGNAL_DEADLINE`)も書かない。** 判定列に掛ける非結合の縛り(下記)は、同じ行の他の列にも同じだけ効く — 期限の名前を正本に置くと、`crates/pulsen/tests/` の定数を改名するだけで表が古くなる。閾値そのものは適用側の関心で、この列が伝えるべきは「合図が期限内に返らない」という前提の壊れ方だけ。
    - 判定: 「ハーネスが `hold_from_other_process` / `try_acquire_from_other_process` を提供するか(この適用先では、保持プロセスを1回起動して合図が期限内に返るかで決まる)」。**適用側の関数名(`holder_capability()`)を主語にしない。** HOOKS.md は「適用範囲」節で ExclusiveLock を含む5ポートのスイートが別実装にも適用できると宣言しており、フック由来の行の判定列はフック水準で書かれている。ここに `crates/pulsen/tests/` にしか無い関数名を置くと、正本の表がこの適用先に結合する(先例の `permission_restrictions_effective` はスイート側にあるので事情が違う)。ADR-003 が「スイートは適用先を知らないままでいられる」を良い点に挙げているのと同じ立場に、表も揃える。
    - 3 OS の列は `実行` のまま(ステップ9で実測と突き合わせる)。
  - **ExclusiveLock 節の前書き(205行)を同じ述語に改める。** 現状は「TC-002〜005 は別プロセスに保持させる**実行ファイルを要するため**、環境で走らなくなりうる(「環境で走らなくなりうる行」を参照)」で、これは本Issueが切ろうとしている因果(実行ファイルの不在 → 前提を作れない環境 → スキップ)そのもの。しかも参照先として表を名指ししているので、ここを残すと、更新した表を古い理解で読ませる導線が正本に残る。「TC-002〜005 は別プロセスにロックを保持させるフィクスチャを要し、その保持プロセスの合図が期限内に返らない環境では走らなくなりうる(「環境で走らなくなりうる行」を参照)。実行ファイルが無い場合はスキップではなくケースの失敗になる」の趣旨に改める。
    HOOKS.md 全体を読み直して確認した限り、旧い述語を前提にした記述はこの2箇所(43行の表・205行の前書き)だけで、他のポート節・「適用範囲」節・`ExclusiveLockHarness` のフック一覧には無い(47行・59行の古さは PR #11 に関するもので、述語とは別件 — plan.md スコープ)。
  - 同じ表の直後か該当行の近くに、**実行ファイルの不在・起動の失敗はこの表の「前提を作れない環境」ではなくケースの失敗である**ことを1文で明記する。表は「スキップを許容する条件」の一覧なので、失敗に倒す条件がここに無いことが読み取れないと、次に読む人が元の述語に戻す。
  - 「3ランナーでの実測」で、区分 B の5件が3 OS で走ったとする記述(「`--workspace` で example がビルドされ、ロックを保持する別プロセスが3 OS で機能した」)は**そのままにする。** 実測の節に置けるのは採取した観測だけで、この run が採ったのは `SKIP` 行の集合(＝走ったかどうか)である。合図が期限内に返ったかは測っていないので、走った理由に「合図が期限内に返った」ことを重ねると、観測していない事実を実測の節に足すことになる。実測の値そのものも書き換えない。同節の「測定したのは `e524981`…」(47行目)と「その更新は #11 の責務とする」(59行目)の古さは本Issueでは直さない(plan.md スコープ)。
- **理由:** AC-8。HOOKS.md は「行 × フック」の正本(`.adr/027`)で、宣言の述語が変われば表も変わる。`.adr/071` も吸収の反映先として HOOKS.md を名指ししている。

### 7. CI の why コメントを事実に合わせる

- **対象ファイル:** `.github/workflows/ci.yml`(137-142行)
- **変更内容:** 「単一のテストターゲットを指定した実行に切り替えると example がビルドされず、ロック系の4件＋1件が『宣言済みスキップ』に化ける」を、「…ロック系の4件＋1件が**失敗する**(実行ファイルの不在は環境の能力ではないので、`SkipBudget` の許容集合に入れない)」に改める。あわせて成立条件を1文添える。文案:

  > 単一のテストターゲットを指定した実行は example をビルドしないだけで、過去にビルドされた成果物を消しはしない。したがって失敗するのは、`target/` にその成果物が残っていない場合である。

  この条件が書かれていないと、手元で再現できない読み手が記述を疑う。`--workspace` のままにする理由は変わらないので残す。

  **書き加える文に、単一ターゲット指定のコマンドの綴りを入れない。** `.thread/10` の ADR-009(Status: Accepted)が「使わないと決めた道具については理由を残し、綴りは残さない。why コメントでは日本語の言い換えを使う」と決めており、この件については「単一のテストターゲットを指定した実行に切り替えると example がビルドされず…」という言い換えを名指しで採用している。理由は `.thread/10/testing.md:113-118` — 「使っていないこと」を `grep -n -- "--test " .github/workflows/ci.yml` が0件であることで確認する形にしているので、綴りが1語でもコメントに混ざると確認手段が壊れ、以後は目視で除外する運用になって、実際に使い始めたときの検出力が落ちる。現在の ci.yml は該当の綴りが**0件**で、137-142行の既存コメントも意図的に言い換えになっている。上の文案が「単一のテストターゲットを指定した実行」で通しているのはこのため。書き換えたあとに同じ grep を回し、0件のままであることを確かめる。
- **理由:** AC-13。コメントが述べている帰結が本Issueの変更で変わる。`.adr/068` は「静かな緑を作る条件が、やらない理由つきでワークフローに記録される」ことを影響に挙げており、記録が古びると同じ役割を果たさない。この6行はまさに本Issueが語る帰結(スキップ→失敗)を述べる場所なので、書き手が綴りへ手を伸ばしやすく、`.thread/10` ADR-009 の制約を踏み抜きやすい。なお ADR-009 は `.adr/` へ未昇格の作業ログなので、本Issueで昇格まで背負わない(スコープ外のまま、綴りを避けるだけでよい)。

### 8. 経路ごとに検証する

- **対象ファイル:** なし(検証)
- **変更内容:**
  1. **`Available` 経路。** `cargo fmt --all` / `cargo clippy --workspace --all-targets --locked -- -D warnings`(CI と同じ形) / `cargo test --workspace --locked -- --nocapture` が緑で、ロック系5件の `SKIP` 行が出ないこと。clippy を CI と同じ形で回すのは、新しく置く enum の名前が既定の warn 級 lint に掛かると `-D warnings` で赤になるため(AC-11)。
  2. **`SignalTimedOut` 経路。** `SIGNAL_DEADLINE` を一時的に `Duration::from_nanos(1)` にして `cargo test -p pulsen -- --nocapture` を回し、`tc_port_exclusive_lock_002/003/004/005` と `tc_task_register_task_017` の5件が `SKIP` 行として出たうえで**緑**になること。`-p pulsen` は example もビルドするので probe は起動まで進み、期限だけが原因で `SignalTimedOut` に倒れる(この経路と 4 の違いは、example がビルドされるかどうかにある)。確認後に元へ戻す。
  3. **probe 成立後のタイムアウト経路。** probe を通したうえで本番だけ期限を超えさせる必要があるので、`SIGNAL_DEADLINE` の値ではなく `start_holder` の待ち方を一時的に差し替える。`start_holder` の先頭に呼び出し回数のカウンタを置き、`recv_timeout` に渡す期限を「1回目は `SIGNAL_DEADLINE`、2回目以降は `Duration::from_nanos(1)`」にする。probe が1回目、本番のケースが2回目以降になるので、能力は `Available` のまま5件が `Started::SignalTimedOut` を踏む。

     ```rust
     // 一時的な差し込み(確認後に必ず戻す)
     static CALLS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
     let deadline = if CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed) == 0 {
         SIGNAL_DEADLINE
     } else {
         Duration::from_nanos(1)
     };
     ```

     `cargo test -p pulsen -- --nocapture` を回し、5件が**失敗**してメッセージに「probe は同じ手順で成立している」が出ることを確認する。`examples/lock_holder.rs` に `sleep` を入れる形は取らない(スコープ外)。確認後に元へ戻す。
  4. **`ProgramMissing` 経路。** `--test` 指定は example をビルドしないが、**既にビルドされた成果物を消しはしない**。1 を回した直後は `target/debug/examples/lock_holder{,.exe}` が必ず存在し、`holder_program()` は `Some` を返して probe が `Available` に倒れるため、削除せずに `--test` を指定しても5件は普通に通ってしまう。そこで成果物を明示的に消してから回す。

     ```
     rm -f target/debug/examples/lock_holder target/debug/examples/lock_holder.exe
     cargo test -p pulsen --test conformance_lock -- --nocapture
     cargo test -p pulsen --test cli_add_error -- --nocapture
     ```

     該当ケースが**失敗**し、メッセージが実行ファイルの不在と `cargo test --workspace` での実行を案内すること。(`CARGO_TARGET_DIR` を別のディレクトリに向けて `--test` 指定で1回だけ回す形でも同じ状態を作れる。依存の再ビルドを避けるなら削除のほうが速い。)

     `cli_add_error` では、**ロックを使わないケースのスキップが probe を起こす**。`common/mod.rs` の `allowed_skips()` は `SKIPS`(`LazyLock`)の初期化時に一度だけ評価され、その中で `holder_capability()` を呼ぶので、最初に `common::skipped` を通るのが権限系(`tc_task_register_task_016 / 021`)や git 系(`036`)であれば、そこで probe が走る。この確認では成果物を消してあるので probe は `ProgramMissing` を即座に返し、待ちは入らない。一方、`tc_task_register_task_017` を含まない絞り込みで回した場合も probe は走るので、合図が期限内に返らない環境では `SIGNAL_DEADLINE` ぶん待つ — 待ちが入っても異常ではない。
  5. **`ProgramUnusable` 経路。** unix では、ビルド済みの実行ファイルから実行ビットを落とせば「実行ファイルはあるが起動できない」状態を確定的に作れる。4 の直後に成果物を作り直してから回す。

     ```
     cargo test --workspace --locked --no-fail-fast -- --nocapture   # 成果物を作り直す
     chmod 000 target/debug/examples/lock_holder
     cargo test -p pulsen --test conformance_lock -- --nocapture
     chmod 755 target/debug/examples/lock_holder
     ```

     4件が**失敗**し、メッセージが `ロック保持フィクスチャ(examples/lock_holder)を起動できなかった(probe の起動時に観測した理由): Permission denied (os error 13)` になること。`spawn` が返した `io::Error` の内容がそのまま載り、「起動できませんでした」のような固定文言に置き換わっていないことを見る(AC-4 の後半)。4件が `SKIP` 行としては現れないことも併せて見る。

     Windows には同じ手段が無い(実行ビットの概念が無く、Defender の隔離も再現できない)ので、そちら側は `Started::SpawnFailed(error)` から `HolderCapability::ProgramUnusable(error)` を経てメッセージに `{error}` が載ることをコードレビューで確認する。
  6. **後始末。** 2〜5 のあとに 1 を再度通して緑に戻ること(4 で消した成果物は `--workspace` が作り直す)、5 の実行ビットが戻っていること(`ls -l target/debug/examples/lock_holder`)、保持プロセス(`lock_holder`)が残っていないこと、`git diff` に一時的な差し替えが残っていないこと(AC-7)。
- **理由:** AC-4 / AC-5 / AC-7 / AC-9 / AC-11。分岐はいずれも環境を作らないと通らないため、5経路を実地で1回ずつ踏む。3 OS すべてで再現できないことは、どこでも検証できないことを意味しない — `ProgramUnusable` は unix で踏み、Windows 固有の事情(Defender の隔離・実行形式の不一致)だけをコードレビューに残す。

### 9. CI の実測と予測を突き合わせる

- **対象ファイル:** `crates/pulsen-conformance/HOOKS.md`(必要なら)
- **変更内容:**
  1. PR を上げる前に、3 OS それぞれで期待する `SKIP` 集合を書き出す。**3 OS の列(＝前回の実測)を写すのではなく、「環境で走らなくなりうる行」の判定列の述語が CI の環境で成立するかを1行ずつ見て導く。** 述語ごとの根拠と件数:
     - **`permission_restrictions_effective`** — CI は非 root(unix ジョブは `id -u` を直接アサート)・コンテナ指定なしなので、unix では権限制限が効いて**成立**(実行)。Windows は POSIX の権限操作が効かないので**不成立**(スキップ)。該当は適合8行(config-store-023 / workflow-store-030 / task-repository-005・011・012・019・035・041)と、同じ述語を使う CLI 側の2件(TC-task-register-task-016 / 021)の**計10件**。
     - **ハーネスが `rewind` を提供するか(TC-port-clock-005)** — `SystemClockHarness` は実時計を巻き戻せないので提供しない。環境ではなくアダプターの性質による恒久スキップで、3 OS とも**1件**。
     - **ハーネスが `hold_from_other_process` / `try_acquire_from_other_process` を提供するか(本Issueで述語が変わる5件)** — CI は `--workspace` なので example がビルドされ、probe は起動して合図を待つところまで進む。3ランナーとも保持プロセスが機能してきた実績があるので `Available` に倒れると見込む。したがって適合4件と TC-task-register-task-017 は3 OS とも**出ない**(0件)。ここが本Issueで唯一述語が変わった行なので、外れた場合に真っ先に疑う先でもある。
     - **残るフック提供系**(`observe_wall_clock` / `unusable_lock` / `failing_manager` / `non_repo_dir`)と `git::tmpdir_outside_repository` — ハーネスが提供する / 一時ディレクトリの置き場が git リポジトリ配下にならない見込みなので、いずれも実行(0件)。
     - **合計の見込み:** unix(ubuntu / macOS)は `tc_port_clock_005` の**1件**、Windows はそれに権限系10件を加えた**11件**。
     - **数えないもの:** `pulsen-conformance` の lib ユニットテストが `SkipBudget` 自身の検証で出す架空の3行(`SKIP tc_port_clock_004_…` / `tc_port_clock_0051_…` / `tc_port_clock_005_…`)は**全 OS の `test.log` に出る**が、走らなかった適合ケースではないので集合に入れない(HOOKS.md も集計から外している)。ジョブサマリー側は ci.yml がその区間を落とすので現れない。`crates/pulsen/src/adapter/task_repository.rs` の `#[cfg(all(test, unix))]` の3件も、Windows では `SKIP` として現れない(コンパイルされない)ので差分にならない。
  2. CI の `test.log` / ジョブサマリーと突き合わせる。`test.log` を目で見る場合は、架空の3行を除いてから数える。
  3. 一致すれば HOOKS.md の3 OS 列は変更なし。ずれた場合は、**予測が誤っていた理由を先に特定してから**列を更新し、出典の run を「3ランナーでの実測」に書き足す。
  4. **実測(run 31683976608 / コミット `b344401`)。** 7ジョブ(fmt 1 + test 3 OS + msrv 3 OS)がすべて success。ジョブサマリーの `SKIP` 行は、ubuntu / macOS が `tc_port_clock_005` の**1件**、Windows がそれに権限系10件(`tc_port_config_store_023` / `tc_port_workflow_store_030` / `tc_port_task_repository_005・011・012・019・035・041` / `tc_task_register_task_016・021`)を加えた**11件**。ロック系5件(`tc_port_exclusive_lock_002` / `003` / `004` / `005` と `tc_task_register_task_017`)は3 OS とも**0件**。架空の3行は手順1 の宣言どおり数えていない。
  5. **突き合わせ。** 手順1 の予測(unix 1件 / Windows 11件 / ロック系5件は3 OS とも0件)と実測が一致した。述語が変わった唯一の行が3 OS とも `Available` に倒れており、宣言と実態が割れていない。手順3 のとおり HOOKS.md の3 OS 列は変更なし。
- **理由:** AC-10。`.adr/068` の「観測値を期待値へ書き写す順序は取らない」をそのまま踏む。この変更は許容集合の述語そのものを動かすので、突き合わせを飛ばすと宣言が正しいかを誰も確かめていない状態になる。

### 10. 設計判断を `.adr/` へ昇格する

実装・レビュー・3 OS の検証がすべて終わったあとの最後のステップ。決定の形が確定してから正本に載せる(`.adr/035` が「実装で形が変わりうる決定を承認済みにするのは確定したステップの完了時」としたのと同じ理由)。

- **対象ファイル:** `.adr/073-holder-capability-skip-vs-fail.md`(新規) / `.thread/13/adr.md` / `crates/pulsen/tests/common/lock.rs` / `crates/pulsen/tests/conformance_lock.rs` / `crates/pulsen-conformance/HOOKS.md`
- **変更内容:**
  1. `.thread/13/adr.md` の各エントリを記録基準(寿命テスト＝この理由はマージ後にこの Issue を見ていない人にも意味を持つか / 波及テスト＝覆すと複数のモジュール・レイヤーに波及するか)にかける。判定は次を出発点にする。
     - **ADR-001(合図タイムアウトは能力の probe として表す)・ADR-002(実行ファイル不在は失敗にする / 能力側と失敗側を分ける基準)・ADR-005(probe の判定基準)・ADR-006(probe 成立後のタイムアウトは失敗)** — 両テストを満たす。Issue コメントが求めているのは「合図タイムアウト＝環境の能力の probe、実行ファイル不在＝失敗、という区別の理由」を残す**1本**なので、4本を1本に畳んで起票する。ADR-005 / ADR-006 は「probe が何を測り、測っていないものをどう扱うか」という同じ決定の内訳であり、別立てにすると正本で理由が分散する。
       畳んだ「決定」には、ADR-002 が置いた**能力側と失敗側を分ける基準**を必ず含める。基準が無いと、正本は4つの区別のうち `ProgramUnusable`(実行ファイルはあるが起動できない)をどちら側に置くかを決められず、次にこの経路を触る人がスキップ側へ倒しても反証を持たない(AC-12)。
     - **ADR-003(probe の置き場所)** — 置き場所の基準(スイート側が判定できる能力か、適用側の具体的フィクスチャに依存する能力か)は後続の適用先にも効くので、上の1本の「決定」の中に基準として畳む。
     - **ADR-004(`Option` のまま `None` の意味を絞りパニックで失敗させる)・ADR-007(起動失敗と合図の読み取り失敗を独立した区別として持つ)** — 波及テストを満たさない(`crates/pulsen/tests/common/lock.rs` に閉じ、理由はコードのすぐ横の doc コメントで伝わる)。作業ログ限りとする。
  2. 昇格分を `.adr/073-holder-capability-skip-vs-fail.md` として起票する。既存の最大採番は **072**(`ls .adr/` で確認済み)なので連番の起点は 073。書式は `.adr/038` に従い、見出しを `## ステータス` / `## コンテキスト` / `## 決定` / `## 検討した代替案` / `## 影響`、ステータス語を **承認済み** に変換する。
     `## 影響` に、**`.adr/068` が「決定」で挙げた帰結が本Issueで改まる**ことを1文入れる。068 は「単一テストターゲット指定では example がビルドされず、ロック保持のフィクスチャが消えてロック系のケースが『宣言済みスキップ』に化ける」と書いているが、実行ファイルの不在を失敗側へ倒したことで、その帰結は「4件＋1件が失敗する」に改まる。`.adr/068` 自体は書き換えない(判断が下された時点の記録 — plan.md スコープ)ので、068 だけを読んだ人が現行の述語へ辿れる導線を、後から来た 073 の側に置く。`.adr/` が相互参照する運用(`.adr/005` / `037` / `071`)と揃う扱いであり、AC-12 の「区別の理由が正本に残っている」の範囲に収まる。
  3. `.thread/13/adr.md` の各エントリの Status 行を更新する。昇格したものは `→ .adr/073-... に昇格`、しなかったものは作業ログ限りである旨と理由を書き、どちらか判別できる状態にする(`.adr/038`)。
  4. ADR 参照に新番号を足す。対象は `lock.rs` の `PROGRAM_MISSING` の doc コメント(現状 `HOOKS.md / ADR-068`)、`conformance_lock.rs` の `allowed_skips()` の doc コメント、`HOOKS.md` で失敗側を明記している段落の典拠。068(許容集合に入れない方針)の参照は残したまま、区別の理由の所在として 073 を併記する。HOOKS.md を落とすと、正本の表の脇だけが「起動できない場合」を扱っていない 068 を単独の典拠に据えたまま残る。
- **理由:** AC-12。Issue コメントの「あわせて更新する箇所」3点のうち残る1点。`.adr/035` が「実装中に生じた新しい決定は同じ規則で連番を続けて起票する」と定めており、`.thread/13/adr.md` は作業ログであって正本ではない。昇格しないと、Issue が合意した成果物が1つ欠けたまま完了と判定されうる。
