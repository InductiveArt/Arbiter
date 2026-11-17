use std::collections::BTreeSet;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;

use crate::core::language::pinned;
use crate::core::record;
use crate::core::repository::is_test;
use crate::core::repository::Repo;
use crate::core::trees;
use crate::domain::finding::Finding;
use crate::packs::typescript::language::grammar;
use crate::packs::typescript::language::handles;
use crate::packs::typescript::language::ADAPTER;
use crate::packs::typescript::packages::companions;
use crate::packs::typescript::packages::packaged;
use crate::packs::typescript::packages::Declared;
use crate::packs::typescript::read::imports::imports;
use crate::packs::typescript::reading::Reading;

// The mirror of an undeclared package: one a manifest declares and no file of
// the unit reaches is pulled and trusted for nothing. A record entry quieting
// one keeps it; an entry quieting nothing is stale.
pub fn instances(
    readings: &[Reading],
    root: &Path,
    repo: &Repo,
    declared: &HashMap<usize, Declared>,
) -> Vec<Finding> {
    let provided = pinned(ADAPTER);
    let mut reached: HashMap<usize, HashSet<String>> = HashMap::new();
    for reading in readings {
        let unit = repo.unit_of(&reading.file);
        for import in &reading.imports {
            let from = &import.from;
            if from.starts_with("node:") {
                reached.entry(unit).or_default().insert("node".to_string());
                continue;
            }
            if from.starts_with('.') || from.starts_with('#') {
                continue;
            }
            reached.entry(unit).or_default().insert(packaged(from));
        }
    }
    for file in companions(root) {
        let Ok(source) = std::fs::read_to_string(root.join(&file)) else {
            continue;
        };
        let Some(tree) = trees::parse(&grammar(), &source) else {
            continue;
        };
        let unit = repo.unit_of(&file);
        for (from, _, _, _) in imports(&tree, &source) {
            if from.starts_with('.') || from.starts_with('#') || from.starts_with("node:") {
                continue;
            }
            reached.entry(unit).or_default().insert(packaged(&from));
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
    let owns: HashSet<String> = declared
        .values()
        .filter_map(|entry| entry.own.clone())
        .collect();
    let (subjects, _) = record::read(root);
    let mut live: HashSet<usize> = HashSet::new();
    let mut findings = Vec::new();
    for (unit, entry) in declared {
        let home = repo.units[*unit].path.clone();
        if is_test(&home) || !coded.contains(unit) {
            continue;
        }
        let name = repo.unit_name(*unit);
        let seen = reached.get(unit);
        let within = |package: &str| seen.is_some_and(|set| set.contains(package));
        for package in entry.dev.iter().collect::<BTreeSet<_>>() {
            if provided.contains(package)
                || owns.contains(package)
                || within(package)
                || entry.peers.contains(package)
                || entry.scripts.contains(package)
            {
                continue;
            }
            if let Some(base) = package.strip_prefix("@types/") {
                if base == "node"
                    || within(base)
                    || entry.peers.contains(base)
                    || within(&format!("@{}", base.replace("__", "/")))
                {
                    continue;
                }
            }
            if let Some(at) = record::quiets(&subjects, "typescript", &name, package) {
                live.insert(at);
                continue;
            }
            findings.push(Finding::integrity(
                &entry.manifest,
                0,
                "unused-dependency",
                format!("{package} is declared, and nothing in the unit reaches it"),
            ));
        }
    }
    findings.extend(record::stale(&subjects, "typescript", &live));
    findings
}
