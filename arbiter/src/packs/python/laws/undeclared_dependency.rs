use std::collections::BTreeSet;
use std::collections::HashMap;
use std::collections::HashSet;

use crate::core::language::pinned;
use crate::core::repository::Repo;
use crate::domain::finding::Finding;
use crate::packs::python::distributions::distribution_map;
use crate::packs::python::distributions::normalized;
use crate::packs::python::distributions::Declared;
use crate::packs::python::distributions::STDLIB;
use crate::packs::python::language::generated;
use crate::packs::python::language::ADAPTER;
use crate::packs::python::modules::module_file;
use crate::packs::python::reading::Reading;

// A module imported from no distribution the unit declares, named once per
// unit at the first file importing it: shipped code leans on the shipped
// lists alone, a test on the dependency groups too.
pub fn instances(
    readings: &[Reading],
    repo: &Repo,
    declared: &HashMap<usize, Declared>,
) -> Vec<Finding> {
    let provided = pinned(ADAPTER);
    let stdlib: HashSet<&str> = STDLIB.lines().collect();
    let known = distribution_map();
    let mut named = BTreeSet::new();
    let mut findings = Vec::new();
    let spoken = readings
        .iter()
        .flat_map(|reading| {
            reading
                .imports
                .iter()
                .map(move |import| (&reading.file, &import.module, import.line))
        })
        .chain(readings.iter().flat_map(|reading| {
            reading
                .plains
                .iter()
                .map(move |plain| (&reading.file, &plain.module, plain.line))
        }));
    for (file, module, line) in spoken {
        let unit = repo.unit_of(file);
        let home = repo.units[unit].path.clone();
        if repo.is_test(unit, &home) {
            continue;
        }
        // Generated code is scanned and holds no finding: its imports reach, and
        // the contract answers for what they name.
        let name = file
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        if generated(&name) {
            continue;
        }
        let top = module.split('.').next().unwrap_or(module);
        if stdlib.contains(top) || module_file(module, &repo.files).is_some() {
            continue;
        }
        let distribution = normalized(&known.get(top).cloned().unwrap_or_else(|| top.to_string()));
        if provided.contains(&distribution) {
            continue;
        }
        let manifest = declared.get(&unit);
        let shipped = manifest.is_some_and(|entry| entry.shipped.contains(&distribution));
        let developed = manifest.is_some_and(|entry| entry.dev.contains(&distribution));
        if shipped || (developed && repo.is_test(unit, file)) {
            continue;
        }
        if named.insert((unit, top.to_string())) {
            let reason = if developed {
                format!("{top} is imported by shipped code and declared only in a dependency group")
            } else {
                format!("{top} is imported, and the unit's manifest does not declare it")
            };
            findings.push(Finding::integrity(
                file,
                line,
                "undeclared-dependency",
                reason,
            ));
        }
    }
    findings
}
