use std::fmt;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use serde::Deserialize;
use serde::Serialize;

// One run of a plan, told apart from every other: the clock when it began and
// a count within this process, so two runs in one millisecond still differ.
// Runs order by the clock, then the count.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct RunId {
    pub millis: u64,
    pub seq: u32,
}

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
            seq: RUNS.fetch_add(1, Ordering::Relaxed),
        }
    }

    // The id back from the text it is shown as; nothing for any other text.
    pub fn parse(text: &str) -> Option<RunId> {
        let (millis, seq) = text.split_once('-')?;
        Some(RunId {
            millis: millis.parse().ok()?,
            seq: seq.parse().ok()?,
        })
    }
}

impl fmt::Display for RunId {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{}-{}", self.millis, self.seq)
    }
}

// One attempt at one task in one run: the run, the task's path, and which try.
// Attempt 0 is a task that never ran.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct JobId {
    pub run: RunId,
    pub path: String,
    pub attempt: u32,
}

impl JobId {
    pub fn new(run: RunId, path: impl Into<String>, attempt: u32) -> JobId {
        JobId {
            run,
            path: path.into(),
            attempt,
        }
    }
}

impl fmt::Display for JobId {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{}/{}#{}", self.run, self.path, self.attempt)
    }
}

// A token one side sets and the other reads. Whoever asks to be woken is
// called once, under the same lock that sets the flag, so no asker is missed.
#[derive(Clone, Default)]
pub struct Cancel {
    inner: Arc<Flag>,
}

#[derive(Default)]
struct Flag {
    set: AtomicBool,
    wakers: Mutex<Vec<Box<dyn Fn() + Send + Sync>>>,
}

impl Cancel {
    pub fn new() -> Cancel {
        Cancel::default()
    }

    pub fn cancelled(&self) -> bool {
        self.inner.set.load(Ordering::SeqCst)
    }

    pub fn cancel(&self) {
        let drained: Vec<Box<dyn Fn() + Send + Sync>> = {
            let mut wakers = self
                .inner
                .wakers
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            self.inner.set.store(true, Ordering::SeqCst);
            wakers.drain(..).collect()
        };
        for waker in drained {
            waker();
        }
    }

    pub fn on_cancel(&self, waker: impl Fn() + Send + Sync + 'static) {
        let mut wakers = self
            .inner
            .wakers
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if self.cancelled() {
            drop(wakers);
            waker();
        } else {
            wakers.push(Box::new(waker));
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;
    use std::sync::Arc;

    use super::Cancel;
    use super::JobId;
    use super::RunId;

    #[test]
    fn two_run_ids_taken_in_a_row_differ() {
        let one = RunId::next();
        let two = RunId::next();
        assert_ne!(one, two);
        assert!(two > one);
    }

    #[test]
    fn a_run_id_reads_back_from_its_text_and_other_text_reads_as_nothing() {
        let run = RunId::next();
        assert_eq!(RunId::parse(&run.to_string()), Some(run));
        assert_eq!(RunId::parse("store"), None);
        assert_eq!(RunId::parse("17-x"), None);
    }

    #[test]
    fn a_job_id_reads_as_run_path_and_attempt() {
        let run = RunId {
            millis: 1700000000000,
            seq: 3,
        };
        let job = JobId::new(run, "gate/fix", 2);
        assert_eq!(job.to_string(), "1700000000000-3/gate/fix#2");
    }

    #[test]
    fn a_cancel_token_wakes_each_asker_once_and_a_late_asker_at_once() {
        let cancel = Cancel::new();
        let woken = Arc::new(AtomicUsize::new(0));
        let count = Arc::clone(&woken);
        cancel.on_cancel(move || {
            count.fetch_add(1, Ordering::SeqCst);
        });
        assert!(!cancel.cancelled());
        cancel.cancel();
        cancel.cancel();
        assert!(cancel.cancelled());
        assert_eq!(woken.load(Ordering::SeqCst), 1);
        let late = Arc::clone(&woken);
        cancel.on_cancel(move || {
            late.fetch_add(10, Ordering::SeqCst);
        });
        assert_eq!(woken.load(Ordering::SeqCst), 11);
    }
}
