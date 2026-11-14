#[cfg(feature = "exhibit")]
use crate::wip::exhibit::listed;
use crate::wip::task::Task;

// The tasks this binary runs by name. A worker is handed a name and finds the
// body here, so a task not listed cannot run.
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
