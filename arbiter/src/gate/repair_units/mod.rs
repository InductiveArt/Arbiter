mod fix_tools;
mod mend_files;
mod regenerate_code;

use task_runner::context::task_context::TaskContext;
use task_runner::declaration::pull::In;
use task_runner::declaration::task::Task;
use task_runner::record::task_failure::TaskFailure;

use crate::core::repository::is_test;
use crate::domain::ecosystem::Ecosystem;
use crate::domain::folder::Folder;
use crate::domain::found_units::FoundUnits;
use crate::domain::gate_mode::GateMode;
use crate::domain::repairs::Repairs;
use crate::domain::unit::Unit;
use crate::gate::repair_units::fix_tools::fix_tools;
use crate::gate::repair_units::mend_files::mend_files;
use crate::gate::repair_units::regenerate_code::regenerate_code;

// What repairing reads: the listed folder, the units found in it, the
// ecosystems with their tools, and whether a past run's store is read.
type RepairReads = (In<Folder>, In<FoundUnits>, In<Vec<Ecosystem>>, In<GateMode>);

pub fn repair_units_task() -> Task {
    Task::new(
        "repair_units",
        |context: &mut TaskContext, (folder, found, ecosystems, mode): RepairReads| {
            repair_units(context, &folder, &found, &ecosystems, *mode)
        },
    )
    .line(
        "every repair arbiter can make, one after another since each rewrites the folder the \
         next reads: the languages mend their own findings, the generated code is written \
         again from its contracts, then each tool that can repair what it finds does, unit by \
         unit; a fixture is never repaired, and a repair that does not succeed is told",
    )
}

// The units a repair may rewrite: every unit but a fixture, which is data for
// some test.
fn repaired(found: &FoundUnits) -> Vec<&Unit> {
    found
        .units
        .iter()
        .filter(|unit| !is_test(&unit.path))
        .collect()
}

fn repair_units(
    context: &mut TaskContext,
    folder: &Folder,
    found: &FoundUnits,
    ecosystems: &[Ecosystem],
    mode: GateMode,
) -> Result<Repairs, TaskFailure> {
    let notes = context.substep("mend_files", |substep| mend_files(substep, folder, found))?;
    let units = repaired(found);
    let mut failed = context.substep("regenerate_code", |substep| {
        regenerate_code(substep, folder, &units, mode)
    })?;
    failed.extend(context.substep("fix_tools", |substep| {
        fix_tools(substep, folder, &units, ecosystems)
    })?);
    context.measure("failed", failed.len() as f64);
    Ok(Repairs { notes, failed })
}
