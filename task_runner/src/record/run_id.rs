use std::cmp::Ordering;
use std::fmt;
use std::process;
use std::sync::atomic;
use std::sync::atomic::AtomicU32;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use crate::contract::RunId;

// One run of a plan, told apart from every other: the clock when it began,
// the process running it and a count within that process, so two runs begun
// in one millisecond differ in one process or in two. Runs order by the
// clock first.
static RUNS: AtomicU32 = AtomicU32::new(0);

impl RunId {
    pub fn next() -> RunId {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()
            .and_then(|since| u64::try_from(since.as_millis()).ok())
            .unwrap_or(0);
        RunId {
            millis,
            process: process::id(),
            seq: RUNS.fetch_add(1, atomic::Ordering::Relaxed),
        }
    }

    // The id back from the text it is shown as; nothing for any other text.
    pub fn parse(text: &str) -> Option<RunId> {
        let mut parts = text.split('-');
        let run = RunId {
            millis: parts.next()?.parse().ok()?,
            process: parts.next()?.parse().ok()?,
            seq: parts.next()?.parse().ok()?,
        };
        parts.next().is_none().then_some(run)
    }
}

impl fmt::Display for RunId {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{}-{}-{}", self.millis, self.process, self.seq)
    }
}

impl PartialOrd for RunId {
    fn partial_cmp(&self, other: &RunId) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for RunId {
    fn cmp(&self, other: &RunId) -> Ordering {
        (self.millis, self.process, self.seq).cmp(&(other.millis, other.process, other.seq))
    }
}

#[cfg(test)]
mod tests {
    use crate::contract::RunId;

    #[test]
    fn two_run_ids_taken_in_a_row_differ() {
        let one = RunId::next();
        let two = RunId::next();
        assert_ne!(one, two);
        assert!(two > one);
    }

    #[test]
    fn two_processes_starting_a_run_in_the_same_millisecond_take_different_ids() {
        let mine = RunId::next();
        assert_eq!(mine.process, std::process::id());
        let theirs = RunId {
            process: mine.process.wrapping_add(1),
            ..mine
        };
        assert_ne!(mine, theirs);
        assert_ne!(mine.to_string(), theirs.to_string());
        assert_eq!(RunId::parse(&theirs.to_string()), Some(theirs));
    }

    #[test]
    fn a_run_id_reads_back_from_its_text_and_other_text_reads_as_nothing() {
        let run = RunId::next();
        assert_eq!(RunId::parse(&run.to_string()), Some(run));
        assert_eq!(RunId::parse("store"), None);
        assert_eq!(RunId::parse("17-x"), None);
        assert_eq!(
            RunId::parse("17-3"),
            None,
            "a count with no process is no run"
        );
        assert_eq!(RunId::parse("17-3-1-0"), None);
    }
}
