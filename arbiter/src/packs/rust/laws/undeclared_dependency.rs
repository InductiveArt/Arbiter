use std::collections::BTreeSet;
use std::collections::HashMap;
use std::collections::HashSet;

use crate::core::repository::is_test;
use crate::core::repository::Repo;
use crate::domain::finding::Finding;
use crate::packs::rust::language::BUILTIN_CRATES;
use crate::packs::rust::manifest::Crates;
use crate::packs::rust::module_files::binds;
use crate::packs::rust::reading::Reading;

// A crate a use declaration enters through and the unit's own manifest does
// not declare, named once per unit.
pub fn instances(
    readings: &[Reading],
    repo: &Repo,
    declared: &HashMap<usize, Crates>,
) -> Vec<Finding> {
    let mut findings = Vec::new();
    let mut named = BTreeSet::new();
    for reading in readings {
        let unit = repo.unit_of(&reading.file);
        let home = repo.units[unit].path.clone();
        if is_test(&home) {
            continue;
        }
        // A module the file declares, or a name it imports, is no crate.
        let local: HashSet<&str> = reading
            .modules
            .iter()
            .map(String::as_str)
            .chain(
                reading
                    .imports
                    .iter()
                    .filter(|import| binds(&import.name, &import.from))
                    .map(|import| import.name.as_str()),
            )
            .collect();
        for entry in &reading.entries {
            let first = entry.first.as_str();
            if first == "crate"
                || first == "self"
                || first == "super"
                || BUILTIN_CRATES.contains(&first)
            {
                continue;
            }
            if local.contains(first) {
                continue;
            }
            let dashed = first.replace('_', "-");
            // A crate's own name is reached from its binaries as the library half.
            if declared.get(&unit).is_some_and(|crates| {
                crates.names.contains(&dashed) || crates.own.as_deref() == Some(dashed.as_str())
            }) {
                continue;
            }
            if named.insert((unit, first.to_string())) {
                findings.push(Finding::integrity(
                    &reading.file,
                    entry.line,
                    "undeclared-dependency",
                    format!("{first} is imported, and the unit's manifest does not declare it"),
                ));
            }
        }
    }
    findings
}
