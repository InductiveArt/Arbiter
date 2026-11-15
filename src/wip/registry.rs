#[cfg(feature = "exhibit")]
use crate::wip::exhibit::tasks::listed;
use crate::wip::task::Task;

// The exhibit's tasks where the build carries them, nothing otherwise: what
// the runner plans and runs with no task from outside it.
pub fn known() -> Vec<Task> {
    #[cfg(feature = "exhibit")]
    {
        listed()
    }
    #[cfg(not(feature = "exhibit"))]
    {
        Vec::new()
    }
}
