use std::fmt;

use crate::contract::RunId;
use crate::contract::TaskInvocationId;

// One occurrence of a task in one run: the run, and the task's path from the
// root of the run, which ends with the task's name.
impl TaskInvocationId {
    // An absent message reads as protobuf reads it: every field at its default.
    pub(crate) fn unset() -> &'static TaskInvocationId {
        static UNSET: TaskInvocationId = TaskInvocationId {
            run: None,
            path: String::new(),
        };
        &UNSET
    }

    pub fn run(&self) -> RunId {
        self.run.unwrap_or_default()
    }
}

impl fmt::Display for TaskInvocationId {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{}/{}", self.run(), self.path)
    }
}
