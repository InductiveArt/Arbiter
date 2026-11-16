use crate::contract::RunRecord;
use crate::contract::TaskAttempt;
use crate::value::task_cache_key::TaskCacheKey;

// What the scheduler asks for, in the order decided, each about one
// invocation by its number in the run.
#[derive(Debug, PartialEq)]
pub enum Decision {
    // An attempt is wanted; with a key, the cache may name its output already.
    Attempt {
        invocation: usize,
        attempt: u32,
        key: Option<TaskCacheKey>,
    },
    // The run was cancelled: whatever is out for this invocation is told to stop.
    Stop {
        invocation: usize,
    },
    // A record landed: an attempt's, a replay's, or one whose outcome followed
    // from its producers, from its items or from a cancel.
    Recorded {
        invocation: usize,
        attempt: Box<TaskAttempt>,
    },
    // The invocation's outcome is final.
    Settled {
        invocation: usize,
        outcome: RunRecord,
    },
}
