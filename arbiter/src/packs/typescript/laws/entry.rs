use std::collections::BTreeSet;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;

use crate::core::repository::is_test;
use crate::core::repository::Repo;
use crate::domain::finding::Finding;
use crate::packs::typescript::language::generated;
use crate::packs::typescript::paths::existing;
use crate::packs::typescript::paths::normalized;
use crate::packs::typescript::reading::Reading;
use crate::packs::typescript::routes::routed;
use crate::packs::typescript::routes::Routes;

// How a unit is entered, from both sides: its own code enters it at the file
// defining a name, never through its surface, and another unit enters it
// only through the routes its manifest declares.
pub fn instances(
    readings: &[Reading],
    repo: &Repo,
    routes: &Routes,
    defined: &HashMap<PathBuf, Vec<(String, usize)>>,
    present: &HashSet<&PathBuf>,
) -> Vec<Finding> {
    // An import crossing into another unit anywhere but a declared route bypasses
    // the boundary the manifest draws.
    let mut findings = Vec::new();
    let mut crossed = BTreeSet::new();
    for (file, from, line) in readings.iter().flat_map(|reading| {
        reading
            .imports
            .iter()
            .map(move |import| (&reading.file, &import.from, &import.line))
    }) {
        if file
            .file_name()
            .is_some_and(|name| generated(&name.to_string_lossy()))
        {
            continue;
        }
        // One import statement carries many names and is read once.
        if !crossed.insert((file.clone(), *line, from.clone())) {
            continue;
        }
        let unit = repo.unit_of(file);
        // A unit on a test path is a fixture: data for some test, not code to check.
        let home = repo.units[unit].path.clone();
        if is_test(&home) {
            continue;
        }
        let mut target = None;
        if from.starts_with('#') {
            if let Some(path) = routes.aliases.get(&unit).and_then(|map| map.get(from)) {
                let folder = repo.units[unit].path.clone();
                target = existing(&normalized(&folder.join(path)), present);
            }
        } else if from.starts_with('.') {
            let base = file.parent().unwrap_or(Path::new(""));
            target = existing(&normalized(&base.join(from)), present);
        } else if let Some(entry) = routes.names.get(from.as_str()) {
            // A unit naming itself takes its own surface the long way round.
            if repo.unit_of(entry) == unit {
                findings.push(Finding::integrity(
                    file,
                    *line,
                    "self-entry",
                    format!(
                        "enters its own unit at {}; a unit's own code imports from the defining file",
                        entry.display()
                    ),
                ));
            }
        } else if !routes.declared.contains(from.as_str())
            && !routes.patterns.iter().any(|pattern| routed(from, pattern))
        {
            if let Some(package) = routes
                .packages
                .iter()
                .find(|package| from.starts_with(&format!("{package}/")))
            {
                findings.push(Finding::integrity(
                    file,
                    *line,
                    "interior-import",
                    format!("goes into {package} past its routes; a unit is entered through the routes its manifest declares"),
                ));
            }
        }
        let Some(target) = target else {
            continue;
        };
        let owner = repo.unit_of(&target);
        if owner == unit {
            // A surface holding definitions is itself the defining file, so
            // reaching it by path is what the rule prescribes, not a detour.
            if routes.surfaces.contains(&target)
                && target != *file
                && !defined.contains_key(&target)
            {
                findings.push(Finding::integrity(
                    file,
                    *line,
                    "self-entry",
                    format!(
                        "enters its own unit at {}; a unit's own code imports from the defining file",
                        target.display()
                    ),
                ));
            }
            continue;
        }
        // A file no unit claims has no manifest, so no routes exist to demand.
        if repo.units[owner].path.as_os_str().is_empty() {
            continue;
        }
        findings.push(Finding::integrity(
            file,
            *line,
            "interior-import",
            format!(
                "reaches {} inside {}; a unit is entered through the routes its manifest declares",
                target.display(),
                repo.units[owner].path.display()
            ),
        ));
    }
    findings
}
