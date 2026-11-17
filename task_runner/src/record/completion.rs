use serde::Deserialize;
use serde::Serialize;

use crate::contract::RunRecord;
use crate::contract::TaskAttemptId;

// How the producer of one type finished: its last attempt and how it ended.
// What a read fired by failing or finishing is handed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Completion {
    pub attempt_id: TaskAttemptId,
    pub outcome: RunRecord,
}
