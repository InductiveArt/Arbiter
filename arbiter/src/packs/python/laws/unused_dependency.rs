use std::collections::BTreeSet;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;

use crate::core::language::pinned;
use crate::core::record;
use crate::core::repository::Repo;
use crate::domain::finding::Finding;
use crate::packs::python::distributions::distribution_map;
use crate::packs::python::distributions::normalized;
use crate::packs::python::distributions::Declared;
use crate::packs::python::language::handles;
use crate::packs::python::language::ADAPTER;
use crate::packs::python::reading::Reading;

// The mirror of an undeclared distribution: one a manifest declares and no
// file of the unit reaches is pulled and trusted for nothing. A record entry
// quieting one keeps it; an entry quieting nothing is stale.
pub fn instances(
    readings: &[Reading],
    root: &Path,
    repo: &Repo,
    declared: &HashMap<usize, Declared>,
) -> Vec<Finding> {
    let provided = pinned(ADAPTER);
    let known = distribution_map();
    let mut reached: HashMap<usize, HashSet<String>> = HashMap::new();
    for reading in readings {
        let modules = reading
            .imports
            .iter()
            .map(|import| &import.module)
            .chain(reading.plains.iter().map(|plain| &plain.module));
        for module in modules {
            let top = module.split('.').next().unwrap_or(module);
            let distribution =
                normalized(&known.get(top).cloned().unwrap_or_else(|| top.to_string()));
            reached
                .entry(repo.unit_of(&reading.file))
                .or_default()
                .insert(distribution);
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
    for (unit, entry) in declared {
        let home = repo.units[*unit].path.clone();
        if repo.is_test(*unit, &home) || !coded.contains(unit) {
            continue;
        }
        let name = repo.unit_name(*unit);
        let seen = reached.get(unit);
        let within = |name: &str| seen.is_some_and(|set| set.contains(name));
        for distribution in entry.dev.iter().collect::<BTreeSet<_>>() {
            if provided.contains(distribution) || within(distribution) {
                continue;
            }
            // A stub distribution serves the type checker of the library it types,
            // and a plugin of the pinned runner loads by entry point, not import.
            let base = distribution
                .strip_prefix("types-")
                .or_else(|| distribution.strip_suffix("-stubs"));
            if base.is_some_and(within) {
                continue;
            }
            if distribution.starts_with("pytest-") && provided.contains("pytest") {
                continue;
            }
            if let Some(at) = record::quiets(&subjects, "python", &name, distribution) {
                live.insert(at);
                continue;
            }
            findings.push(Finding::integrity(
                &entry.manifest,
                0,
                "unused-dependency",
                format!("{distribution} is declared, and nothing in the unit reaches it"),
            ));
        }
    }
    findings.extend(record::stale(&subjects, "python", &live));
    findings
}
