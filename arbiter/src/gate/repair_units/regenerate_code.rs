use task_runner::context::task_context::TaskContext;
use task_runner::record::task_failure::TaskFailure;

use crate::domain::check_outcome::CheckOutcome;
use crate::domain::folder::Folder;
use crate::domain::gate_mode::GateMode;
use crate::domain::unit::Unit;

// Each unit's generated code written again from its contracts by every
// language generating any; a generation still differing after is one outcome
// that did not pass.
pub fn regenerate_code(
    context: &mut TaskContext,
    folder: &Folder,
    units: &[&Unit],
    mode: GateMode,
) -> Result<Vec<CheckOutcome>, TaskFailure> {
    let languages = crate::packs::all();
    let mut failed = Vec::new();
    let mut regenerated = 0;
    for unit in units {
        let home = folder.path.join(&unit.path);
        for language in &languages {
            context.checkpoint()?;
            let Some(drift) = language.regenerate(&folder.path, &home, mode.cold) else {
                continue;
            };
            regenerated += 1;
            if !drift.problems.is_empty() {
                failed.push(CheckOutcome {
                    check: drift.check,
                    unit: unit.path.clone(),
                    language: language.label().to_string(),
                    passed: false,
                    seconds: drift.seconds,
                    output: drift.problems.join("\n"),
                    warm: false,
                });
            }
        }
    }
    context.measure("regenerated", f64::from(regenerated));
    Ok(failed)
}
