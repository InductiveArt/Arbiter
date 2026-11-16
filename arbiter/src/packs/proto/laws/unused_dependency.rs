use std::collections::BTreeSet;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;

use crate::core::pack::Repo;
use crate::core::record;
use crate::domain::finding::Finding;

// Each dependency a manifest declares in a unit no external import leans on.
// A record entry quieting one keeps it; an entry quieting nothing is stale.
pub fn instances(
    root: &Path,
    repo: &Repo,
    declared: &HashMap<usize, (PathBuf, Vec<String>)>,
    externals: &HashMap<usize, Vec<(PathBuf, String)>>,
) -> Vec<Finding> {
    let mut findings = Vec::new();
    let (subjects, _) = record::read(root);
    let mut live: HashSet<usize> = HashSet::new();
    for (unit, (manifest, deps)) in declared {
        let home = repo.units[*unit].path.clone();
        if repo.is_test(*unit, &home) || deps.is_empty() {
            continue;
        }
        if externals.get(unit).is_some_and(|paths| !paths.is_empty()) {
            continue;
        }
        let name = repo.unit_name(*unit);
        for dep in deps.iter().collect::<BTreeSet<_>>() {
            if let Some(at) = record::quiets(&subjects, "proto", &name, dep) {
                live.insert(at);
                continue;
            }
            findings.push(Finding::integrity(
                manifest,
                0,
                "unused-dependency",
                format!("{dep} is declared, and nothing in the unit reaches it"),
            ));
        }
    }
    findings.extend(record::stale(&subjects, "proto", &live));
    findings
}
