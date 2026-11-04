use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use crate::core::graph::Graph;

pub struct Proof {
    pub file: PathBuf,
    pub path: Vec<String>,
    pub sentence: String,
    pub line: usize,
}

// Every behavior the tests claim, by unit, with the units claiming nothing:
// what the repository proves about itself, and where it proves nothing.
pub fn proofs(found: &Graph) -> (BTreeMap<PathBuf, Vec<Proof>>, Vec<PathBuf>) {
    let mut claimed: BTreeMap<PathBuf, Vec<Proof>> = BTreeMap::new();
    let mut coded: std::collections::BTreeSet<PathBuf> = std::collections::BTreeSet::new();
    for file in &found.repo.files {
        let name = file
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        let Some(index) = found.packs.iter().position(|pack| pack.handles(&name)) else {
            continue;
        };
        if found.packs[index].generated(&name) {
            continue;
        }
        // A unit on a test path is a fixture: data for some test, not code to judge.
        let unit = found.repo.unit_of(file);
        let home = found.repo.units[unit].path.clone();
        if found.repo.is_test(unit, &home) {
            continue;
        }
        coded.insert(home.clone());
        let Ok(source) = fs::read_to_string(found.root.join(file)) else {
            continue;
        };
        for claim in found.packs[index].claims(&source) {
            claimed.entry(home.clone()).or_default().push(Proof {
                file: file.clone(),
                path: claim.path,
                sentence: claim.sentence,
                line: claim.line,
            });
        }
    }
    for proofs in claimed.values_mut() {
        proofs.sort_by(|a, b| (&a.file, a.line).cmp(&(&b.file, b.line)));
    }
    let unproven = coded
        .into_iter()
        .filter(|unit| !claimed.contains_key(unit))
        .collect();
    (claimed, unproven)
}
