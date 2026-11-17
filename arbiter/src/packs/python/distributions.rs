use std::collections::HashMap;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

use crate::core::repository::Repo;
use crate::packs::python::language::MANIFEST;

// The import names of the standard library, one per line.
pub const STDLIB: &str = include_str!("../../../packs/python/stdlib.txt");

const DISTRIBUTIONS: &str = include_str!("../../../packs/python/distributions.toml");

// Distribution names as PEP 503 reads them: lowercased, runs of separators as one dash.
pub fn normalized(name: &str) -> String {
    name.to_lowercase()
        .replace(['_', '.'], "-")
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<&str>>()
        .join("-")
}

// One unit's declarations, the lists that ship kept apart from the dependency
// groups: a test import may lean on either, shipped code on the first alone.
#[derive(Default)]
pub struct Declared {
    pub manifest: PathBuf,
    pub shipped: HashSet<String>,
    pub dev: HashSet<String>,
}

pub fn declared_distributions(root: &Path, repo: &Repo) -> HashMap<usize, Declared> {
    let mut declared: HashMap<usize, Declared> = HashMap::new();
    for manifest in &repo.manifests {
        if manifest.file_name().is_none_or(|name| name != MANIFEST) {
            continue;
        }
        let Ok(text) = fs::read_to_string(root.join(manifest)) else {
            continue;
        };
        let Ok(parsed) = text.parse::<toml::Value>() else {
            continue;
        };
        let entry = declared.entry(repo.unit_of(manifest)).or_default();
        entry.manifest = manifest.clone();
        let mut ships: Vec<&toml::Value> = Vec::new();
        ships.extend(
            parsed
                .get("project")
                .and_then(|project| project.get("dependencies")),
        );
        if let Some(table) = parsed
            .get("project")
            .and_then(|project| project.get("optional-dependencies"))
            .and_then(toml::Value::as_table)
        {
            ships.extend(table.values());
        }
        for list in ships {
            for name in requirement_names(list) {
                entry.shipped.insert(name.clone());
                entry.dev.insert(name);
            }
        }
        if let Some(table) = parsed
            .get("dependency-groups")
            .and_then(toml::Value::as_table)
        {
            for list in table.values() {
                for name in requirement_names(list) {
                    entry.dev.insert(name);
                }
            }
        }
    }
    declared
}

fn requirement_names(list: &toml::Value) -> Vec<String> {
    let Some(entries) = list.as_array() else {
        return Vec::new();
    };
    entries
        .iter()
        .filter_map(toml::Value::as_str)
        .map(|requirement| {
            let name: String = requirement
                .chars()
                .take_while(|part| part.is_ascii_alphanumeric() || matches!(part, '-' | '_' | '.'))
                .collect();
            normalized(&name)
        })
        .collect()
}

pub fn distribution_map() -> HashMap<String, String> {
    let Ok(parsed) = DISTRIBUTIONS.parse::<toml::Value>() else {
        return HashMap::new();
    };
    let Some(imports) = parsed.get("imports").and_then(toml::Value::as_table) else {
        return HashMap::new();
    };
    imports
        .iter()
        .filter_map(|(import, distribution)| {
            distribution
                .as_str()
                .map(|name| (import.clone(), name.to_string()))
        })
        .collect()
}

// PyPI treats hyphens, underscores and case as one name; so does this reading.
pub fn normalized_name(name: &str) -> String {
    name.to_lowercase().replace('-', "_")
}
