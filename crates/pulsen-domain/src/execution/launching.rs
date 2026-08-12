//! launching タスクの分類。

use crate::definition::DurationSpec;
use crate::task::{ProcessIdent, StartTimeRecord, Timestamp};

use super::value::PidFileContent;

/// launching タスクの分類結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchingDecision {
    /// pid ファイルが現れた — running へ取り込む。
    ConfirmRunning(ProcessIdent),
    /// pid なし・猶予内 — 何もしない。
    KeepWaiting,
    /// pid なし・猶予超過 — 無効化マーカーの書き込みと再確認へ進む。
    SuspectSpawnFailure,
}

/// 無効化マーカー書き込み後の再確認の結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchingRecheck {
    /// 再確認で pid が現れていた — running へ取り込む。
    ConfirmRunning(ProcessIdent),
    /// なお pid が無い — spawn 失敗が確定した。
    SpawnFailed,
}

/// ラッパーの書き込み順序(starttime → pid)の保証が破れた観測。
///
/// 破れの種別だけを持つ。どのタスク・どの run ディレクトリかの文脈は報告側が付与し、
/// 利用者に見せる言葉は `cli::render` が決める。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InconsistentRunFiles {
    /// pid ファイルがあるのに starttime ファイルがない。
    MissingStartTime,
}

/// launching タスクの分類(純粋)。
pub struct LaunchingClassifier;

impl LaunchingClassifier {
    /// launching 記録から pid ファイルの出現を待つ猶予時間(組み込み)。
    pub const GRACE_PERIOD: DurationSpec = DurationSpec::from_secs_unchecked(30);

    /// 観測した run ファイルから launching タスクを分類する。
    ///
    /// 経過は巻き戻りを 0 に飽和させて測り、猶予の超過は経過が `GRACE_PERIOD` を
    /// **上回った**ときにのみ成立する。
    pub fn classify(
        recorded_at: &Timestamp,
        now: &Timestamp,
        pid: Option<PidFileContent>,
        starttime: Option<StartTimeRecord>,
    ) -> Result<LaunchingDecision, InconsistentRunFiles> {
        match (pid, starttime) {
            (Some(pid), Some(starttime)) => {
                Ok(LaunchingDecision::ConfirmRunning(ident(pid, starttime)))
            }
            (Some(_), None) => Err(InconsistentRunFiles::MissingStartTime),
            (None, Some(_)) | (None, None) => {
                if now.elapsed_since(recorded_at) > Self::GRACE_PERIOD.seconds() {
                    Ok(LaunchingDecision::SuspectSpawnFailure)
                } else {
                    Ok(LaunchingDecision::KeepWaiting)
                }
            }
        }
    }

    /// 無効化マーカーを書いた後の再確認として分類する。
    ///
    /// pid が無ければ starttime の有無を問わず spawn 失敗が確定する — starttime のみの
    /// 状態は書き込み順序の正常な中間状態だが、マーカーを書いた後のラッパーは pid を
    /// 書かずに終える。
    pub fn classify_recheck(
        pid: Option<PidFileContent>,
        starttime: Option<StartTimeRecord>,
    ) -> Result<LaunchingRecheck, InconsistentRunFiles> {
        match (pid, starttime) {
            (Some(pid), Some(starttime)) => {
                Ok(LaunchingRecheck::ConfirmRunning(ident(pid, starttime)))
            }
            (Some(_), None) => Err(InconsistentRunFiles::MissingStartTime),
            (None, Some(_)) | (None, None) => Ok(LaunchingRecheck::SpawnFailed),
        }
    }
}

/// 揃った観測から同定情報一式を組み立てる。
fn ident(pid: PidFileContent, starttime: StartTimeRecord) -> ProcessIdent {
    ProcessIdent::new(pid.pid(), pid.kill_ident().clone(), starttime)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task::{KillIdent, Pid, ProcessStartTime};

    fn at(text: &str) -> Timestamp {
        Timestamp::parse_rfc3339(text).expect("受理される")
    }

    fn recorded_at() -> Timestamp {
        at("2026-08-11T09:15:30Z")
    }

    fn after(seconds: i64) -> Timestamp {
        Timestamp::from_unix_secs(recorded_at().unix_secs() + seconds).expect("受理される")
    }

    fn pid_file() -> PidFileContent {
        PidFileContent::new(
            Pid::new(4242),
            KillIdent::parse("-4242".to_owned()).expect("受理される"),
        )
    }

    fn starttime() -> StartTimeRecord {
        StartTimeRecord::new(
            ProcessStartTime::parse("871234".to_owned()).expect("受理される"),
            at("2026-08-11T09:15:31Z"),
        )
    }

    #[test]
    fn 猶予時間は30秒である() {
        assert_eq!(LaunchingClassifier::GRACE_PERIOD.seconds(), 30);
    }

    #[test]
    fn pidとstarttimeが揃っていれば同定情報つきで起動確認になる() {
        let decision = LaunchingClassifier::classify(
            &recorded_at(),
            &after(1),
            Some(pid_file()),
            Some(starttime()),
        )
        .expect("矛盾しない観測");

        assert_eq!(
            decision,
            LaunchingDecision::ConfirmRunning(ProcessIdent::new(
                Pid::new(4242),
                KillIdent::parse("-4242".to_owned()).expect("受理される"),
                starttime(),
            ))
        );
    }

    #[test]
    fn pidが無く猶予内なら待ち続ける() {
        for elapsed in [0, 29, 30] {
            let decision =
                LaunchingClassifier::classify(&recorded_at(), &after(elapsed), None, None)
                    .expect("矛盾しない観測");
            assert_eq!(decision, LaunchingDecision::KeepWaiting, "{elapsed}秒");
        }
    }

    #[test]
    fn pidが無く猶予を超えていればspawn失敗を疑う() {
        let decision = LaunchingClassifier::classify(&recorded_at(), &after(31), None, None)
            .expect("矛盾しない観測");

        assert_eq!(decision, LaunchingDecision::SuspectSpawnFailure);
    }

    #[test]
    fn starttimeだけが現れている状態も猶予の判断に合流する() {
        assert_eq!(
            LaunchingClassifier::classify(&recorded_at(), &after(30), None, Some(starttime())),
            Ok(LaunchingDecision::KeepWaiting)
        );
        assert_eq!(
            LaunchingClassifier::classify(&recorded_at(), &after(31), None, Some(starttime())),
            Ok(LaunchingDecision::SuspectSpawnFailure)
        );
    }

    #[test]
    fn 時計が巻き戻っていても猶予を超えたとみなさない() {
        let decision = LaunchingClassifier::classify(&recorded_at(), &after(-3600), None, None)
            .expect("矛盾しない観測");

        assert_eq!(decision, LaunchingDecision::KeepWaiting);
    }

    #[test]
    fn pidだけが現れている観測は書き込み順序の破れとして報告される() {
        let error =
            LaunchingClassifier::classify(&recorded_at(), &after(1), Some(pid_file()), None)
                .expect_err("矛盾した観測");

        assert_eq!(error, InconsistentRunFiles::MissingStartTime);
    }

    #[test]
    fn 再確認でpidとstarttimeが揃っていれば起動確認になる() {
        let recheck = LaunchingClassifier::classify_recheck(Some(pid_file()), Some(starttime()))
            .expect("矛盾しない観測");

        assert_eq!(
            recheck,
            LaunchingRecheck::ConfirmRunning(ProcessIdent::new(
                Pid::new(4242),
                KillIdent::parse("-4242".to_owned()).expect("受理される"),
                starttime(),
            ))
        );
    }

    #[test]
    fn 再確認でpidが無ければstarttimeの有無によらずspawn失敗が確定する() {
        assert_eq!(
            LaunchingClassifier::classify_recheck(None, None),
            Ok(LaunchingRecheck::SpawnFailed)
        );
        assert_eq!(
            LaunchingClassifier::classify_recheck(None, Some(starttime())),
            Ok(LaunchingRecheck::SpawnFailed)
        );
    }

    #[test]
    fn 再確認でpidだけが現れている観測は書き込み順序の破れとして報告される() {
        let error = LaunchingClassifier::classify_recheck(Some(pid_file()), None)
            .expect_err("矛盾した観測");

        assert_eq!(error, InconsistentRunFiles::MissingStartTime);
    }
}
