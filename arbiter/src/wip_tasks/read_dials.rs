use std::path::Path;

use task_runner::context::task_context::TaskContext;
use task_runner::declaration::pull::In;
use task_runner::declaration::task::Task;
use task_runner::record::task_failure::TaskFailure;

use crate::core::repository::is_test;
use crate::domain::dials::Dials;
use crate::domain::folder::Folder;
use crate::domain::found_units::FoundUnits;

pub fn read_dials_task() -> Task {
    Task::new(
        "read_dials",
        |context: &mut TaskContext, (folder, found): (In<Folder>, In<FoundUnits>)| {
            read_dials(context, &folder, &found)
        },
    )
    .line(
        "each unit's dials as its own config sets them, held to the bar, for every language \
         with a marker in the unit except a marker that only lists members; a unit on a test \
         path is a fixture and has none",
    )
}

fn read_dials(
    context: &mut TaskContext,
    folder: &Folder,
    found: &FoundUnits,
) -> Result<Dials, TaskFailure> {
    let languages = crate::packs::all();
    let mut dials = Dials {
        findings: Vec::new(),
        notes: Vec::new(),
    };
    for unit in &found.units {
        if is_test(&unit.path) {
            continue;
        }
        context.checkpoint()?;
        let home = folder.path.join(&unit.path);
        for language in &languages {
            let Some(marker) = unit
                .markers
                .iter()
                .find(|marker| marker.ecosystem == language.label())
            else {
                continue;
            };
            if marker.members_only {
                continue;
            }
            if let Some((findings, notes)) = language.dials(&unit.path, &home) {
                dials.findings.extend(findings);
                let shown = shown(&unit.path);
                dials
                    .notes
                    .extend(notes.into_iter().map(|note| format!("{shown}: {note}")));
            }
        }
    }
    context.measure("findings", dials.findings.len() as f64);
    Ok(dials)
}

// A unit as a person reads its place: its path, or a dot for the folder itself.
pub fn shown(unit: &Path) -> String {
    if unit.as_os_str().is_empty() {
        ".".to_string()
    } else {
        unit.to_string_lossy().into_owned()
    }
}
