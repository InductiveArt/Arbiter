use std::collections::BTreeSet;

use crate::core::pack::Repo;
use crate::domain::finding::Finding;
use crate::packs::python::language::generated;
use crate::packs::python::reading::Reading;

// A generated import that names no contract is a resolver truth told at the
// import site, never a dead contract somewhere else.
pub fn instances(readings: &[Reading], repo: &Repo) -> Vec<Finding> {
    let mut phantom = BTreeSet::new();
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
        let leaf = file
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        if generated(&leaf) {
            continue;
        }
        let Some(contract) = module.rsplit('.').next().and_then(|last| {
            last.strip_suffix("_pb2")
                .or_else(|| last.strip_suffix("_pb2_grpc"))
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
        if phantom.insert((unit, module.clone())) {
            findings.push(Finding::integrity(
                file,
                line,
                "phantom-import",
                format!(
                    "{module} names no contract; a generated import answers for \
                     the contract it was generated from"
                ),
            ));
        }
    }
    findings
}
