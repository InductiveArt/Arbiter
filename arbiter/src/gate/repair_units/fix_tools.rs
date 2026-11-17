use std::time::Instant;

use task_runner::context::task_context::TaskContext;
use task_runner::record::task_failure::TaskFailure;

use crate::domain::check_outcome::CheckOutcome;
use crate::domain::ecosystem::Ecosystem;
use crate::domain::folder::Folder;
use crate::domain::tool_run::ToolRun;
use crate::domain::unit::shown;
use crate::domain::unit::Unit;
use crate::gate::run_subprocess::run_subprocess_in;

// Each tool that can repair what it finds, run in every unit of its ecosystem
// one at a time: unit by unit, each unit's ecosystems in the order given and
// their tools in the order the record writes them. A unit whose marker only
// lists members is repaired through its members. A repair that does not exit
// with 0, or cannot start, is one outcome that did not pass; each repair's
// configs and all it printed are kept in a folder of its own.
pub fn fix_tools(
    context: &mut TaskContext,
    folder: &Folder,
    units: &[&Unit],
    ecosystems: &[Ecosystem],
) -> Result<Vec<CheckOutcome>, TaskFailure> {
    let mut failed = Vec::new();
    let mut ran: u32 = 0;
    for unit in units {
        for ecosystem in ecosystems {
            let held = unit
                .markers
                .iter()
                .any(|marker| marker.ecosystem == ecosystem.name && !marker.members_only);
            if !held {
                continue;
            }
            for tool in &ecosystem.tools {
                let Some(words) = &tool.fix else {
                    continue;
                };
                let Some(run) = ToolRun::of(words, ecosystem) else {
                    return Err(TaskFailure::from(format!(
                        "the repair of the tool {} names no program",
                        tool.name
                    )));
                };
                context.checkpoint()?;
                ran += 1;
                let name = format!("{} in {}", tool.name, shown(&unit.path));
                context.note(format!("repairing: {name}"));
                let kept = context.dir().join("repairs").join(ran.to_string());
                let started = Instant::now();
                let subprocess = run.subprocess(name, &folder.path.join(&unit.path), None);
                let told = match run_subprocess_in(context, &subprocess, &kept) {
                    Ok(exit) if exit.exited_with_zero() => None,
                    Ok(exit) => Some(format!("{}{}", exit.stdout, exit.stderr)),
                    Err(failure) => Some(failure.reason),
                };
                if let Some(output) = told {
                    failed.push(CheckOutcome {
                        check: tool.name.clone(),
                        unit: unit.path.clone(),
                        language: ecosystem.name.clone(),
                        passed: false,
                        seconds: started.elapsed().as_secs_f64(),
                        output,
                        warm: false,
                    });
                }
            }
        }
    }
    context.measure("repairs", f64::from(ran));
    Ok(failed)
}
