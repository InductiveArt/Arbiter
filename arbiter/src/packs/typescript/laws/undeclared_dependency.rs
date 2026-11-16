use std::collections::BTreeSet;
use std::collections::HashMap;
use std::collections::HashSet;

use crate::core::pack::pinned;
use crate::core::pack::Repo;
use crate::domain::finding::Finding;
use crate::packs::typescript::language::generated;
use crate::packs::typescript::language::ADAPTER;
use crate::packs::typescript::packages::developmental;
use crate::packs::typescript::packages::packaged;
use crate::packs::typescript::packages::Declared;
use crate::packs::typescript::reading::Reading;

const BUILTINS: &str = include_str!("../../../../packs/typescript/builtins.txt");

// A package imported that the unit's manifest does not declare, named once
// per unit at the first file importing it: shipped code leans on the shipped
// tables alone, development code on devDependencies too.
pub fn instances(
    readings: &[Reading],
    repo: &Repo,
    declared: &HashMap<usize, Declared>,
) -> Vec<Finding> {
    let provided = pinned(ADAPTER);
    let builtins: HashSet<&str> = BUILTINS.lines().collect();
    let mut named = BTreeSet::new();
    let mut findings = Vec::new();
    for (reading, file, from, line) in readings.iter().flat_map(|reading| {
        reading
            .imports
            .iter()
            .map(move |import| (reading, &reading.file, &import.from, import.line))
    }) {
        let unit = repo.unit_of(file);
        let home = repo.units[unit].path.clone();
        if repo.is_test(unit, &home) {
            continue;
        }
        // Generated code is scanned and holds no finding: its imports only reach.
        let leaf = file
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        if generated(&leaf) {
            continue;
        }
        if from.starts_with('.') || from.starts_with('#') || from.starts_with("node:") {
            continue;
        }
        let package = packaged(from);
        if builtins.contains(package.as_str()) || provided.contains(&package) {
            continue;
        }
        let manifest = declared.get(&unit);
        // A unit importing its own name is self-entry's case, never a missing dependency.
        if manifest.and_then(|entry| entry.own.as_deref()) == Some(package.as_str()) {
            continue;
        }
        let shipped = manifest.is_some_and(|entry| entry.shipped.contains(&package));
        let developed = manifest.is_some_and(|entry| entry.dev.contains(&package));
        // Development code may rest on devDependencies: tests, the toolchain's
        // config files, and a type-only import, which erases at build time.
        let development =
            repo.is_test(unit, file) || developmental(file) || reading.typed.contains(&line);
        if shipped || (developed && development) {
            continue;
        }
        if named.insert((unit, package.clone())) {
            let reason = if developed {
                format!(
                    "{package} is imported by shipped code and declared only under devDependencies"
                )
            } else {
                format!("{package} is imported, and the unit's manifest does not declare it")
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
