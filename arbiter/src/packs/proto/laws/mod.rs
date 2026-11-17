mod undeclared_dependency;
mod unused_dependency;

use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

use crate::core::repository::Repo;
use crate::domain::finding::Finding;
use crate::packs::proto::manifest::declared_deps;
use crate::packs::proto::reading::Reading;

// Every law the proto pack holds over its readings, in the order they are told.
pub fn instances(readings: &[Reading], root: &Path, repo: &Repo) -> Vec<Finding> {
    let declared = declared_deps(root, repo);
    let externals = externals(readings, repo);
    let mut findings = undeclared_dependency::instances(repo, &declared, &externals);
    findings.extend(unused_dependency::instances(
        root, repo, &declared, &externals,
    ));
    findings
}

// An import reaching no local file and no well-known contract leans on a
// declared dependency; which one, only the registry could say.
fn externals(readings: &[Reading], repo: &Repo) -> HashMap<usize, Vec<(PathBuf, String)>> {
    let mut externals: HashMap<usize, Vec<(PathBuf, String)>> = HashMap::new();
    for reading in readings {
        for path in &reading.imports {
            if path.starts_with("google/protobuf/") {
                continue;
            }
            let local = repo.files.iter().any(|candidate| {
                let shown = candidate.to_string_lossy();
                shown == *path || shown.ends_with(&format!("/{path}"))
            });
            if local {
                continue;
            }
            externals
                .entry(repo.unit_of(&reading.file))
                .or_default()
                .push((reading.file.clone(), path.clone()));
        }
    }
    externals
}
