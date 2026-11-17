use std::collections::BTreeSet;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;

use crate::core::record;
use crate::core::repository::is_test;
use crate::core::repository::Repo;
use crate::domain::finding::Finding;
use crate::packs::rust::language::handles;
use crate::packs::rust::manifest::Crates;
use crate::packs::rust::reading::Reading;

// The mirror of an undeclared crate: a crate a manifest declares and no path
// of the unit reaches is pulled and trusted for nothing. A use statement and
// a qualified path both reach. A record entry quieting one keeps it; an entry
// quieting nothing is stale.
pub fn instances(
    readings: &[Reading],
    root: &Path,
    repo: &Repo,
    declared: &HashMap<usize, Crates>,
) -> Vec<Finding> {
    let mut reached: HashMap<usize, HashSet<String>> = HashMap::new();
    for reading in readings {
        let crates = reached.entry(repo.unit_of(&reading.file)).or_default();
        for used in &reading.uses {
            let first = used.module.split("::").next().unwrap_or(&used.module);
            crates.insert(first.replace('_', "-"));
        }
        for entry in &reading.entries {
            crates.insert(entry.first.replace('_', "-"));
        }
    }
    let mut coded: HashSet<usize> = HashSet::new();
    for file in &repo.files {
        let name = file
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        if handles(&name) {
            coded.insert(repo.unit_of(file));
        }
    }
    let (subjects, _) = record::read(root);
    let mut live: HashSet<usize> = HashSet::new();
    let mut findings = Vec::new();
    for (unit, crates) in declared {
        let home = repo.units[*unit].path.clone();
        if is_test(&home) || !coded.contains(unit) {
            continue;
        }
        let unit_name = repo.unit_name(*unit);
        let seen = reached.get(unit);
        for name in crates.names.iter().collect::<BTreeSet<_>>() {
            if Some(name.as_str()) == crates.own.as_deref() {
                continue;
            }
            if seen.is_some_and(|set| set.contains(name)) {
                continue;
            }
            if let Some(at) = record::quiets(&subjects, "rust", &unit_name, name) {
                live.insert(at);
                continue;
            }
            findings.push(Finding::integrity(
                &crates.manifest,
                0,
                "unused-dependency",
                format!("{name} is declared, and nothing in the unit reaches it"),
            ));
        }
    }
    findings.extend(record::stale(&subjects, "rust", &live));
    findings
}
