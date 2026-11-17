use std::collections::BTreeSet;

use crate::core::repository::is_test;
use crate::core::repository::Repo;
use crate::domain::finding::Finding;
use crate::packs::typescript::language::generated;
use crate::packs::typescript::reading::Reading;

// A generated import answers for a contract; one resolving to none is named
// where it stands, never left to read as dead code.
pub fn instances(readings: &[Reading], repo: &Repo) -> Vec<Finding> {
    let mut phantom = BTreeSet::new();
    let mut findings = Vec::new();
    for (_, file, from, line) in readings.iter().flat_map(|reading| {
        reading
            .imports
            .iter()
            .map(move |import| (reading, &reading.file, &import.from, import.line))
    }) {
        let unit = repo.unit_of(file);
        let home = repo.units[unit].path.clone();
        if is_test(&home) {
            continue;
        }
        let leaf = file
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        if generated(&leaf) || !from.starts_with('.') {
            continue;
        }
        let Some(contract) = from.rsplit('/').next().and_then(|last| {
            last.strip_suffix("_pb")
                .or_else(|| last.strip_suffix(".pb"))
        }) else {
            continue;
        };
        let ending = format!("/{contract}.proto");
        let flat = format!("{contract}.proto");
        let matched = repo.files.iter().any(|candidate| {
            let shown = candidate.to_string_lossy();
            shown.ends_with(&ending) || shown == flat
        });
        if matched {
            continue;
        }
        if phantom.insert((unit, from.clone())) {
            findings.push(Finding::integrity(
                file,
                line,
                "phantom-import",
                format!(
                    "{from} names no contract; a generated import answers for \
                     the contract it was generated from"
                ),
            ));
        }
    }
    findings
}
