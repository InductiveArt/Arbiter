pub mod check_generated;
pub mod find_law_instances;
pub mod find_units;
pub mod list_check_subprocesses;
pub mod list_laws;
pub mod load_ecosystems;
pub mod plan_unit_checks;
pub mod read_check_findings;
pub mod read_dials;
pub mod read_graph;
pub mod run_subprocess;

use task_runner::declaration::task::Task;

use crate::wip_tasks::check_generated::check_generated_task;
use crate::wip_tasks::find_law_instances::find_law_instances_task;
use crate::wip_tasks::find_units::find_units_task;
use crate::wip_tasks::list_check_subprocesses::list_check_subprocesses_task;
use crate::wip_tasks::list_laws::list_laws_task;
use crate::wip_tasks::load_ecosystems::load_ecosystems_task;
use crate::wip_tasks::plan_unit_checks::plan_unit_checks_task;
use crate::wip_tasks::read_check_findings::read_check_findings_task;
use crate::wip_tasks::read_dials::read_dials_task;
use crate::wip_tasks::read_graph::read_graph_task;
use crate::wip_tasks::run_subprocess::run_subprocess_task;

// Every task the gate is made of, listed one by one so a search for its name
// lands on its file.
pub fn listed() -> Vec<Task> {
    vec![
        load_ecosystems_task(),
        find_units_task(),
        read_graph_task(),
        list_laws_task(),
        find_law_instances_task(),
        read_dials_task(),
        check_generated_task(),
        plan_unit_checks_task(),
        list_check_subprocesses_task(),
        run_subprocess_task(),
        read_check_findings_task(),
    ]
}
