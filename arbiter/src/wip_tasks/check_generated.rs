use task_runner::context::task_context::TaskContext;
use task_runner::declaration::pull::In;
use task_runner::declaration::task::Task;
use task_runner::record::task_failure::TaskFailure;

use crate::core::repository::is_test;
use crate::domain::check_outcome::CheckOutcome;
use crate::domain::folder::Folder;
use crate::domain::found_units::FoundUnits;
use crate::domain::gate_mode::GateMode;
use crate::domain::generated_checks::GeneratedChecks;

pub fn check_generated_task() -> Task {
    Task::new(
        "check_generated",
        |context: &mut TaskContext,
         (folder, found, mode): (In<Folder>, In<FoundUnits>, In<GateMode>)| {
            check_generated(context, &folder, &found, *mode)
        },
    )
    .line(
        "each unit's generated code compared with what its contracts generate, and its \
         contracts with the image the user signed, by every language that compares any; a \
         fixing run regenerates first, and a unit on a test path is a fixture and is never \
         compared",
    )
}

fn check_generated(
    context: &mut TaskContext,
    folder: &Folder,
    found: &FoundUnits,
    mode: GateMode,
) -> Result<GeneratedChecks, TaskFailure> {
    let languages = crate::packs::all();
    let mut checks = GeneratedChecks {
        outcomes: Vec::new(),
        notes: Vec::new(),
    };
    for unit in &found.units {
        if is_test(&unit.path) {
            continue;
        }
        context.checkpoint()?;
        let home = folder.path.join(&unit.path);
        for language in &languages {
            let drifts = language.drift(
                &folder.path,
                &home,
                mode.fixing,
                mode.cold,
                &mut checks.notes,
            );
            for drift in drifts {
                checks.outcomes.push(CheckOutcome {
                    check: drift.check,
                    unit: unit.path.clone(),
                    language: language.label().to_string(),
                    passed: drift.problems.is_empty(),
                    seconds: drift.seconds,
                    output: drift.problems.join("\n"),
                    warm: false,
                });
            }
        }
    }
    let failed = checks.outcomes.iter().filter(|outcome| !outcome.passed);
    context.measure("checks", checks.outcomes.len() as f64);
    context.measure("failed", failed.count() as f64);
    Ok(checks)
}
