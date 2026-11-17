use std::fmt;

use crate::contract::run_record::Ended;
use crate::contract::Cancelled;
use crate::contract::Passed;
use crate::contract::RunRecord;
use crate::contract::TimedOut;

// How an attempt, a task or a whole run ended. A record whose ending is unset
// reads as neither clean nor a failure: nothing is known of it yet.
impl RunRecord {
    // An absent message reads as protobuf reads it: every field at its default.
    pub(crate) fn unset() -> &'static RunRecord {
        static UNSET: RunRecord = RunRecord { ended: None };
        &UNSET
    }

    pub fn passed() -> RunRecord {
        RunRecord::ending(Ended::Passed(Passed {}))
    }

    pub fn failed(reason: impl Into<String>) -> RunRecord {
        RunRecord::ending(Ended::Failed(reason.into()))
    }

    // Ran past the time its task allowed.
    pub fn timed_out() -> RunRecord {
        RunRecord::ending(Ended::TimedOut(TimedOut {}))
    }

    // Stopped from outside before it could finish.
    pub fn cancelled() -> RunRecord {
        RunRecord::ending(Ended::Cancelled(Cancelled {}))
    }

    // Never ran: a task it stands on did not pass, named by that task's path.
    pub fn skipped(path: impl Into<String>) -> RunRecord {
        RunRecord::ending(Ended::Skipped(path.into()))
    }

    // Never ran: the branch it reads was not taken, named by the task that
    // decided it. By design, so it counts against nothing.
    pub fn unneeded(path: impl Into<String>) -> RunRecord {
        RunRecord::ending(Ended::Unneeded(path.into()))
    }

    fn ending(ended: Ended) -> RunRecord {
        RunRecord { ended: Some(ended) }
    }

    pub fn clean(&self) -> bool {
        matches!(self.ended, Some(Ended::Passed(_) | Ended::Unneeded(_)))
    }

    // An attempt that ended without passing, as opposed to one never made.
    pub fn is_failure(&self) -> bool {
        matches!(
            self.ended,
            Some(Ended::Failed(_) | Ended::TimedOut(_) | Ended::Cancelled(_))
        )
    }
}

impl fmt::Display for RunRecord {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.ended {
            Some(Ended::Passed(_)) => write!(out, "passed"),
            Some(Ended::Failed(reason)) => write!(out, "failed: {reason}"),
            Some(Ended::TimedOut(_)) => write!(out, "timed out"),
            Some(Ended::Cancelled(_)) => write!(out, "cancelled"),
            Some(Ended::Skipped(path)) => write!(out, "skipped by {path}"),
            Some(Ended::Unneeded(path)) => write!(out, "unneeded by {path}"),
            None => write!(out, "unknown"),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::contract::RunRecord;

    #[test]
    fn a_run_record_is_clean_when_it_passed_or_was_not_needed() {
        assert!(RunRecord::passed().clean());
        assert!(RunRecord::unneeded("gate/check").clean());
        assert!(!RunRecord::failed("broke").clean());
        assert!(!RunRecord::timed_out().clean());
        assert!(!RunRecord::cancelled().clean());
        assert!(!RunRecord::skipped("gate/check").clean());
        assert!(!RunRecord::default().clean());
    }

    #[test]
    fn a_run_record_failed_when_an_attempt_ended_without_passing() {
        assert!(RunRecord::failed("broke").is_failure());
        assert!(RunRecord::timed_out().is_failure());
        assert!(RunRecord::cancelled().is_failure());
        assert!(!RunRecord::passed().is_failure());
        assert!(!RunRecord::skipped("gate/check").is_failure());
        assert!(!RunRecord::unneeded("gate/check").is_failure());
        assert!(!RunRecord::default().is_failure());
    }
}
