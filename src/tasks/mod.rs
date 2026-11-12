pub mod deps;
pub mod fallout;
pub mod impact;
pub mod laws;
pub mod plan;
pub mod rhymes;
pub mod tangles;

use crate::core::task::Task;

// Every task, listed one by one so a search for its name lands on its file.
pub fn all() -> Vec<Task> {
    vec![
        deps::task(),
        fallout::task(),
        plan::task(),
        impact::task(),
        rhymes::task(),
        tangles::task(),
    ]
}
