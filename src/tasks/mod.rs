pub mod graph;
pub mod plans;
pub mod rhyme;
pub mod tangles;

use crate::core::task::Task;

// Every task, concatenated in the order the modules above are listed.
pub fn all() -> Vec<Task> {
    let mut tasks = Vec::new();
    tasks.extend(graph::tasks());
    tasks.extend(plans::tasks());
    tasks.extend(rhyme::tasks());
    tasks.extend(tangles::tasks());
    tasks
}
