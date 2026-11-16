use serde::Deserialize;
use serde::Serialize;

use crate::contract::TaskAttemptId;
use crate::protocol::task_order_input::TaskOrderInput;

// What the run writes for the process performing an attempt: which attempt,
// which task by name, and each read in the task's order.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct TaskOrder {
    pub attempt_id: TaskAttemptId,
    pub task: String,
    pub inputs: Vec<TaskOrderInput>,
}
