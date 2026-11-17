use std::collections::BTreeSet;
use std::collections::HashMap;
use std::path::PathBuf;

use crate::core::repository::is_test;
use crate::core::repository::Repo;
use crate::domain::finding::Finding;

// An external import in a unit whose manifest declares no dependency at all,
// named once per unit and path.
pub fn instances(
    repo: &Repo,
    declared: &HashMap<usize, (PathBuf, Vec<String>)>,
    externals: &HashMap<usize, Vec<(PathBuf, String)>>,
) -> Vec<Finding> {
    let mut findings = Vec::new();
    let mut named = BTreeSet::new();
    for (unit, paths) in externals {
        let home = repo.units[*unit].path.clone();
        if is_test(&home) {
            continue;
        }
        let empty = declared
            .get(unit)
            .map(|(_, deps)| deps.is_empty())
            .unwrap_or(true);
        if !empty {
            continue;
        }
        for (file, path) in paths {
            if named.insert((*unit, path.clone())) {
                findings.push(Finding::integrity(
                    file,
                    0,
                    "undeclared-dependency",
                    format!("{path} is imported, and the unit's manifest does not declare it"),
                ));
            }
        }
    }
    findings
}
