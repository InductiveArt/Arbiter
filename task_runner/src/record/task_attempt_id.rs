use std::fmt;

use crate::contract::RunId;
use crate::contract::TaskAttemptId;
use crate::contract::TaskInvocationId;

// One try at one invocation: the invocation, and which try. Attempt 0 is an
// invocation decided without a try.
impl TaskAttemptId {
    // An absent message reads as protobuf reads it: every field at its default.
    pub(crate) fn unset() -> &'static TaskAttemptId {
        static UNSET: TaskAttemptId = TaskAttemptId {
            invocation: None,
            attempt: 0,
        };
        &UNSET
    }

    pub fn new(run: RunId, path: impl Into<String>, attempt: u32) -> TaskAttemptId {
        TaskAttemptId {
            invocation: Some(TaskInvocationId {
                run: Some(run),
                path: path.into(),
            }),
            attempt,
        }
    }

    pub fn invocation(&self) -> &TaskInvocationId {
        self.invocation
            .as_ref()
            .unwrap_or(TaskInvocationId::unset())
    }

    pub fn path(&self) -> &str {
        &self.invocation().path
    }
}

impl fmt::Display for TaskAttemptId {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{}#{}", self.invocation(), self.attempt)
    }
}

#[cfg(test)]
mod tests {
    use crate::contract::RunId;
    use crate::contract::TaskAttemptId;

    #[test]
    fn a_task_attempt_id_reads_as_run_path_and_attempt() {
        let run = RunId {
            millis: 1700000000000,
            process: 41,
            seq: 3,
        };
        let attempt_id = TaskAttemptId::new(run, "gate/fix", 2);
        assert_eq!(attempt_id.to_string(), "1700000000000-41-3/gate/fix#2");
        assert_eq!(attempt_id.path(), "gate/fix");
    }
}
