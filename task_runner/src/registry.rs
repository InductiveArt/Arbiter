#[cfg(feature = "exhibit")]
use crate::exhibit::tasks::listed;
use crate::task::Task;

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
