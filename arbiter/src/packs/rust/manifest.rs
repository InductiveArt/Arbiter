use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;

use crate::core::repository::Repo;
use crate::packs::rust::language::MANIFEST;

// Each unit's manifest with its dependency-table keys, dashed, which is how
// crates are published, and those of them it takes by path. Cargo already
// refuses a dev-dependency outside tests, so the tables stay one set; the
// crate's own name stays apart.
pub struct Crates {
    pub manifest: PathBuf,
    pub own: Option<String>,
    pub names: HashSet<String>,
    pub paths: HashSet<String>,
}

pub fn declared_crates(root: &Path, repo: &Repo) -> HashMap<usize, Crates> {
    let mut declared: HashMap<usize, Crates> = HashMap::new();
    for manifest in &repo.manifests {
        if manifest.file_name().is_none_or(|name| name != MANIFEST) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(root.join(manifest)) else {
            continue;
        };
        let Ok(parsed) = text.parse::<toml::Value>() else {
            continue;
        };
        let entry = declared
            .entry(repo.unit_of(manifest))
            .or_insert_with(|| Crates {
                manifest: manifest.clone(),
                own: None,
                names: HashSet::new(),
                paths: HashSet::new(),
            });
        if let Some(own) = table_at(&parsed, "package").and_then(|package| package.get("name")) {
            if let Some(own) = own.as_str() {
                entry.own = Some(own.replace('_', "-"));
            }
        }
        for path in [
            "dependencies",
            "dev-dependencies",
            "build-dependencies",
            "workspace.dependencies",
        ] {
            let Some(table) = table_at(&parsed, path) else {
                continue;
            };
            for (name, spec) in table {
                entry.names.insert(name.replace('_', "-"));
                if spec.get("path").is_some() {
                    entry.paths.insert(name.replace('_', "-"));
                }
            }
        }
    }
    declared
}

pub fn table_at<'manifest>(
    parsed: &'manifest toml::Value,
    path: &str,
) -> Option<&'manifest toml::Table> {
    let mut value = parsed;
    for part in path.split('.') {
        value = value.get(part)?;
    }
    value.as_table()
}
