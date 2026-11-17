use crate::contract::TaskInvocation;
use crate::contract::TaskInvocationId;

// One occurrence of a task in a run: which task, every attempt at it in
// order, and, for a task run once per item of a list, the invocation of each
// item. It holds one attempt at least and ends as its last attempt did; a task
// run per item holds only attempt 0, which ends as its items ended together.
impl TaskInvocation {
    pub fn id(&self) -> &TaskInvocationId {
        self.id.as_ref().unwrap_or(TaskInvocationId::unset())
    }
}
