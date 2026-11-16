use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

use crate::core::repository::Repo;
use crate::packs::proto::language::MANIFEST;

// Each unit's buf.yaml with the deps it declares.
pub fn declared_deps(root: &Path, repo: &Repo) -> HashMap<usize, (PathBuf, Vec<String>)> {
    let mut declared = HashMap::new();
    for manifest in &repo.manifests {
        if manifest.file_name().is_none_or(|name| name != MANIFEST) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(root.join(manifest)) else {
            continue;
        };
        declared.insert(repo.unit_of(manifest), (manifest.clone(), buf_deps(&text)));
    }
    declared
}

// The deps a buf.yaml names, one per "- " line under the deps key.
fn buf_deps(text: &str) -> Vec<String> {
    let mut deps = Vec::new();
    let mut inside = false;
    for line in text.lines() {
        if !line.starts_with([' ', '\t']) {
            inside = line.trim_end() == "deps:";
            continue;
        }
        if inside {
            if let Some(rest) = line.trim_start().strip_prefix("- ") {
                deps.push(rest.trim().trim_matches('"').to_string());
            }
        }
    }
    deps
}
