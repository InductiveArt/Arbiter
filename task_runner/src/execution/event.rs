use crate::contract::RunRecord;
use crate::contract::RunReport;
use crate::contract::TaskAttempt;
use crate::contract::TaskAttemptId;

// What a run tells as it goes: an attempt out on its slot, a record landing,
// whether an attempt's, a replay's or one that never ran, a task's final
// outcome, and the whole report once the run ended.
pub enum Event<'a> {
    Started {
        attempt_id: &'a TaskAttemptId,
        slot: usize,
    },
    Ended {
        attempt: &'a TaskAttempt,
    },
    Settled {
        path: &'a str,
        outcome: &'a RunRecord,
    },
    Finished {
        report: &'a RunReport,
    },
}
