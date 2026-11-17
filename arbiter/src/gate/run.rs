use std::path::Path;
use std::path::PathBuf;

use task_runner::contract::RunReport;
use task_runner::declaration::plan::InvalidPlan;
use task_runner::declaration::plan::Plan;
use task_runner::execution::runner::Runner;
use task_runner::value::run_store::RunStore;

use crate::domain::folder::Folder;
use crate::domain::gate_mode::GateMode;
use crate::domain::repairs::Repairs;
use crate::gate::plans;
use crate::gate::store;
use crate::report::gate_run::GateRun;
use crate::report::of_plan;

// One gate over a folder: when fixing, its repairs first, then the gate's
// plan over what they left, and the run the report is written from. The
// runner given performs every attempt; its runs and cache are kept in the
// folder's store. A plan that cannot run, or a task that did not pass, leaves
// no run.
pub fn run(asked: &str, fixing: bool, cold: bool, runner: Runner) -> Result<GateRun, String> {
    let folder = PathBuf::from(asked)
        .canonicalize()
        .map_err(|_| format!("no folder at {asked}"))?;
    let store = store::opened(&folder).map_err(|error| {
        format!(
            "the store in {} could not be made: {error}",
            folder.display()
        )
    })?;
    let runner = if cold { runner.cold() } else { runner };
    let mode = GateMode { cold };
    let repaired = if fixing {
        let (record, values) = ran(&runner, plans::repairs(), &folder, mode, &store)?;
        let repairs = values.get::<Repairs>().ok_or_else(|| unfinished(&record))?;
        Some((record, repairs))
    } else {
        None
    };
    let (record, values) = ran(&runner, plans::gate(), &folder, mode, &store)?;
    let gated = of_plan::gate_run(&values, &record)?;
    Ok(match repaired {
        None => gated,
        Some((record, repairs)) => {
            let mut gated = gated.repaired(repairs);
            let mut steps = of_plan::steps(&record);
            steps.append(&mut gated.steps);
            gated.steps = steps;
            gated
        }
    })
}

// The plan run over the folder in the mode, refused as a whole when it cannot
// run, and as unfinished when a task did not pass.
fn ran(
    runner: &Runner,
    plan: Result<Plan, InvalidPlan>,
    folder: &Path,
    mode: GateMode,
    store: &Path,
) -> Result<(RunReport, RunStore), String> {
    let plan = plan.map_err(|invalid| format!("the plan does not hold: {invalid:?}"))?;
    let mut seeds = RunStore::new();
    seeds
        .put(&Folder {
            path: folder.to_path_buf(),
        })
        .and_then(|()| seeds.put(&mode))
        .map_err(|error| format!("the seeds do not encode: {error}"))?;
    let (record, values) = runner.run(&plan, seeds, store).map_err(|refusal| {
        format!(
            "the {} plan was refused: {}",
            plan.name,
            refusal.to_string().trim_end()
        )
    })?;
    if record.outcome().is_failure() {
        return Err(unfinished(&record));
    }
    Ok((record, values))
}

// Why a run left no result, and where its record is.
fn unfinished(record: &RunReport) -> String {
    format!(
        "the run did not finish: {}; its record is in {}",
        record.outcome(),
        record.dir().display()
    )
}
