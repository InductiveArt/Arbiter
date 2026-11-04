use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::core::graph::Graph;
use crate::core::pack::Finding;

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

// One claim's identity across runs: where it lives at unit grain and what it
// says, so a moved test file keeps its claim and a reworded one trades it.
pub fn identity(unit: &Path, proof: &Proof) -> String {
    let shown = if unit.as_os_str().is_empty() {
        "."
    } else {
        &unit.to_string_lossy()
    };
    format!("{shown} | {} | {}", proof.path.join(" / "), proof.sentence)
}

// Every claim's identity, duplicates kept: two tests speaking the same
// sentence are two proofs, and losing one still counts.
pub fn spoken(found: &Graph) -> Vec<String> {
    let (claimed, _) = proofs(found);
    let mut all: Vec<String> = claimed
        .iter()
        .flat_map(|(unit, proofs)| proofs.iter().map(|proof| identity(unit, proof)))
        .collect();
    all.sort();
    all
}

const REFERENCE: &str = ".arbiter/reference.json";

pub fn recorded_claims(root: &Path) -> Option<Vec<String>> {
    let text = fs::read_to_string(root.join(REFERENCE)).ok()?;
    let parsed: serde_json::Value = serde_json::from_str(&text).ok()?;
    let entries = parsed.get("claims")?.as_array()?;
    Some(
        entries
            .iter()
            .filter_map(|entry| entry.as_str().map(str::to_string))
            .collect(),
    )
}

// Each accepted claim the tree no longer speaks: a behavior that left without
// the user's signature. No recorded claims is no reference yet, not a pass.
pub fn vanished(found: &Graph) -> Vec<Finding> {
    let Some(recorded) = recorded_claims(&found.root) else {
        return Vec::new();
    };
    let mut remaining: BTreeMap<String, usize> = BTreeMap::new();
    for claim in spoken(found) {
        *remaining.entry(claim).or_insert(0) += 1;
    }
    let mut findings = Vec::new();
    for claim in recorded {
        if let Some(count) = remaining.get_mut(&claim) {
            if *count > 0 {
                *count -= 1;
                continue;
            }
        }
        let unit = claim.split(" | ").next().unwrap_or(".").to_string();
        findings.push(Finding {
            check: "proof",
            file: PathBuf::from(unit),
            line: 0,
            name: "vanished-claim".to_string(),
            reason: format!(
                "the claim '{claim}' left; a proven behavior leaves only with the user's signature"
            ),
        });
    }
    findings
}
