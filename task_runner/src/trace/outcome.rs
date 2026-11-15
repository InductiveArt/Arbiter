use serde::Deserialize;
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Outcome {
    Ok,
    Failed(String),
    // Ran past the time its task allowed.
    TimedOut,
    // Stopped from outside before it could finish.
    Cancelled,
    // Never ran: a task it stands on did not pass, named by that task's path.
    Skipped(String),
    // Never ran: the branch it reads was not taken, named by the task that
    // decided it. By design, so it counts against nothing.
    Unneeded(String),
}

impl Outcome {
    pub fn clean(&self) -> bool {
        matches!(self, Outcome::Ok | Outcome::Unneeded(_))
    }

    // An attempt that ended without passing, as opposed to one never made.
    pub fn failed(&self) -> bool {
        matches!(
            self,
            Outcome::Failed(_) | Outcome::TimedOut | Outcome::Cancelled
        )
    }
}

#[derive(Debug)]
pub struct Failure {
    pub reason: String,
}

impl From<String> for Failure {
    fn from(reason: String) -> Self {
        Failure { reason }
    }
}

impl From<&str> for Failure {
    fn from(reason: &str) -> Self {
        Failure {
            reason: reason.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Outcome;

    #[test]
    fn an_outcome_is_clean_when_it_passed_or_was_not_needed() {
        assert!(Outcome::Ok.clean());
        assert!(Outcome::Unneeded("gate/check".to_string()).clean());
        assert!(!Outcome::Failed("broke".to_string()).clean());
        assert!(!Outcome::TimedOut.clean());
        assert!(!Outcome::Cancelled.clean());
        assert!(!Outcome::Skipped("gate/check".to_string()).clean());
    }

    #[test]
    fn an_outcome_failed_when_an_attempt_ended_without_passing() {
        assert!(Outcome::Failed("broke".to_string()).failed());
        assert!(Outcome::TimedOut.failed());
        assert!(Outcome::Cancelled.failed());
        assert!(!Outcome::Ok.failed());
        assert!(!Outcome::Skipped("gate/check".to_string()).failed());
        assert!(!Outcome::Unneeded("gate/check".to_string()).failed());
    }
}
