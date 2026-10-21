use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::core::graph::Graph;
use crate::core::pack::{Finding, Suppression};

const REFERENCE: &str = ".arbiter/reference.json";

// Named suppressions of the repository's own units; a fixture's belong to some test.
pub fn counted(found: &Graph) -> Vec<&Suppression> {
    kept(found)
        .into_iter()
        .filter(|suppression| !suppression.blanket)
        .collect()
}

pub fn blankets(found: &Graph) -> Vec<&Suppression> {
    kept(found)
        .into_iter()
        .filter(|suppression| suppression.blanket)
        .collect()
}

fn kept(found: &Graph) -> Vec<&Suppression> {
    let mut all: Vec<&Suppression> = Vec::new();
    for pack in &found.packs {
        for suppression in pack.suppressions() {
            let unit = found.repo.unit_of(&suppression.file);
            let home = found.repo.units[unit].path.clone();
            if found.repo.is_test(unit, &home) {
                continue;
            }
            all.push(suppression);
        }
    }
    all.sort_by(|a, b| (&a.file, a.line).cmp(&(&b.file, b.line)));
    all
}

pub fn blanket_findings(found: &Graph) -> Vec<Finding> {
    blankets(found)
        .into_iter()
        .map(|suppression| Finding {
            check: "ledger",
            file: suppression.file.clone(),
            line: suppression.line,
            name: "blanket-suppression".to_string(),
            reason: format!("{} quiets more than one named rule", suppression.text),
        })
        .collect()
}

pub fn reference(root: &Path) -> Option<Vec<(String, String)>> {
    let text = fs::read_to_string(root.join(REFERENCE)).ok()?;
    let parsed: serde_json::Value = serde_json::from_str(&text).ok()?;
    let entries = parsed.get("suppressions")?.as_array()?;
    Some(
        entries
            .iter()
            .filter_map(|entry| {
                let file = entry.get("file")?.as_str()?.to_string();
                let text = entry.get("text")?.as_str()?.to_string();
                Some((file, text))
            })
            .collect(),
    )
}

pub fn record(root: &Path, current: &[&Suppression]) -> io::Result<PathBuf> {
    let entries: Vec<serde_json::Value> = current
        .iter()
        .map(|suppression| {
            serde_json::json!({
                "file": suppression.file.to_string_lossy(),
                "text": suppression.text,
            })
        })
        .collect();
    let written = serde_json::to_string_pretty(&serde_json::json!({ "suppressions": entries }))
        .map_err(io::Error::other)?;
    let path = root.join(REFERENCE);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, written + "\n")?;
    Ok(path)
}

// The entries beyond what the reference holds; each known pair absorbs one occurrence.
pub fn added<'found>(
    current: &[&'found Suppression],
    reference: &[(String, String)],
) -> Vec<&'found Suppression> {
    let mut remaining: HashMap<(String, String), usize> = HashMap::new();
    for (file, text) in reference {
        *remaining.entry((file.clone(), text.clone())).or_default() += 1;
    }
    let mut extra = Vec::new();
    for suppression in current {
        let key = (
            suppression.file.to_string_lossy().to_string(),
            suppression.text.clone(),
        );
        if let Some(count) = remaining.get_mut(&key) {
            if *count > 0 {
                *count -= 1;
                continue;
            }
        }
        extra.push(*suppression);
    }
    extra
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::added;
    use crate::core::pack::Suppression;

    fn one(line: usize) -> Suppression {
        Suppression {
            file: PathBuf::from("a.rs"),
            line,
            text: "#[allow(clippy::x)]".to_string(),
            blanket: false,
        }
    }

    #[test]
    fn the_reference_absorbs_known_entries_and_names_the_extra() {
        let first = one(3);
        let second = one(9);
        let current = vec![&first, &second];
        let reference = vec![("a.rs".to_string(), "#[allow(clippy::x)]".to_string())];
        let extra = added(&current, &reference);
        assert_eq!(extra.len(), 1);
        assert_eq!(extra[0].line, 9);
    }
}
