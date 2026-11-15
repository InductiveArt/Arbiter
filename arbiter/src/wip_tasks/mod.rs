pub mod find_units;
pub mod load_ecosystems;
pub mod run_ecosystem_tools;
pub mod run_subprocess;

use task_runner::task::Task;

use crate::wip_tasks::find_units::find_units_task;
use crate::wip_tasks::load_ecosystems::load_ecosystems_task;
use crate::wip_tasks::run_ecosystem_tools::run_ecosystem_tools_task;
use crate::wip_tasks::run_subprocess::run_subprocess_task;

// Every task the gate is made of, listed one by one so a search for its name
// lands on its file.
pub fn listed() -> Vec<Task> {
    vec![
        load_ecosystems_task(),
        find_units_task(),
        run_ecosystem_tools_task(),
        run_subprocess_task(),
    ]
}
