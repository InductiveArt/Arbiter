use std::iter;

use task_runner::declaration::fan_out::FanOut;
use task_runner::declaration::plan::InvalidPlan;
use task_runner::declaration::plan::Plan;

use crate::domain::law::LawFindings;
use crate::domain::law::LawName;
use crate::domain::subprocess::Subprocess;
use crate::domain::subprocess::SubprocessExit;
use crate::gate::listed;

// The task leaving the ecosystems, which a plan begins with unless they are
// seeded.
pub const ECOSYSTEMS: &str = "load_ecosystems";

// The tasks of a gate over a folder once its ecosystems are known, in the
// order a run records them.
pub const GATE: [&str; 12] = [
    "find_units",
    "read_graph",
    "list_laws",
    "find_law_instances",
    "read_dials",
    "check_generated",
    "digest_unit_cones",
    "plan_unit_checks",
    "list_check_subprocesses",
    "run_subprocess",
    "read_check_findings",
    "account_ledger",
];

// The tasks of a fixing run's repairs once the ecosystems are known.
pub const REPAIRS: [&str; 2] = ["find_units", "repair_units"];

// A plan of the tasks named, each task run once per item fanned out over its
// list when the plan holds it: each law apart, and each program apart.
pub fn planned(name: &str, tasks: &[&str]) -> Result<Plan, InvalidPlan> {
    let fan_outs: Vec<FanOut> = [
        FanOut::of::<LawName, LawFindings>("find_law_instances", |law| law.name.clone()),
        FanOut::of::<Subprocess, SubprocessExit>("run_subprocess", |subprocess| {
            subprocess.name.clone()
        }),
    ]
    .into_iter()
    .filter(|fan_out| tasks.contains(&fan_out.task.as_str()))
    .collect();
    Plan::fanned(name, &listed(), tasks, &fan_outs)
}

// The gate as `arbiter gate` runs it, its ecosystems loaded first.
pub fn gate() -> Result<Plan, InvalidPlan> {
    planned("gate", &loaded(&GATE))
}

// A fixing run's repairs as `arbiter gate --fix` runs them first.
pub fn repairs() -> Result<Plan, InvalidPlan> {
    planned("repairs", &loaded(&REPAIRS))
}

fn loaded<'tasks>(tasks: &[&'tasks str]) -> Vec<&'tasks str> {
    iter::once(ECOSYSTEMS)
        .chain(tasks.iter().copied())
        .collect()
}
