use std::collections::HashMap;
use std::path::PathBuf;

use crate::core::repository::is_test;
use crate::core::repository::Repo;
use crate::domain::finding::Finding;
use crate::packs::typescript::reading::Reading;
use crate::packs::typescript::routes::Routes;

// A surface a manifest's route names offers only names defined elsewhere,
// and a re-export anywhere else hides the names it forwards.
pub fn instances(
    readings: &[Reading],
    repo: &Repo,
    routes: &Routes,
    defined: &HashMap<PathBuf, Vec<(String, usize)>>,
) -> Vec<Finding> {
    let mut findings: Vec<Finding> = Vec::new();
    for file in &routes.written {
        let unit = repo.unit_of(file);
        let home = repo.units[unit].path.clone();
        if is_test(&home) {
            continue;
        }
        let Some(owned) = defined.get(file) else {
            continue;
        };
        let Some((name, line)) = owned.iter().min_by_key(|(_, line)| *line) else {
            continue;
        };
        let reason = if owned.len() == 1 {
            format!("defines {name}; a surface offers only names defined elsewhere")
        } else {
            format!(
                "defines {name} and {} more names; a surface offers only names defined elsewhere",
                owned.len() - 1
            )
        };
        findings.push(Finding::integrity(file, *line, "reexport", reason));
    }
    // A route target is the unit's declared surface, so a named re-export there is
    // the declaration itself; anywhere else it hides a name, and a star hides
    // every name even on the surface.
    for reading in readings {
        for reexport in &reading.reexports {
            let surface = routes.surfaces.contains(&reading.file);
            if surface && !reexport.star {
                continue;
            }
            let from = &reexport.from;
            let reason = if surface {
                format!(
                    "re-exports every name from \"{from}\"; a surface names each symbol it offers"
                )
            } else {
                format!("re-exports from \"{from}\"; a module offers only its own names")
            };
            findings.push(Finding::integrity(
                &reading.file,
                reexport.line,
                "reexport",
                reason,
            ));
        }
    }
    findings
}
