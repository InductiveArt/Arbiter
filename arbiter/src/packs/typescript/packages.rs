use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;

use crate::core::pack::Repo;
use crate::packs::typescript::language::MANIFEST;

// One unit's declarations, the tables that ship kept apart from the one that
// only develops: a test import may lean on either, shipped code on the first alone.
#[derive(Default)]
pub struct Declared {
    pub manifest: PathBuf,
    pub own: Option<String>,
    pub shipped: HashSet<String>,
    pub dev: HashSet<String>,
    // A peer is the consumer's to provide: a contract, never an own use.
    pub peers: HashSet<String>,
    // Every word of every script: a package run as a command is reached.
    pub scripts: HashSet<String>,
}

pub fn declared_packages(root: &Path, repo: &Repo) -> HashMap<usize, Declared> {
    let mut declared: HashMap<usize, Declared> = HashMap::new();
    for manifest in &repo.manifests {
        if manifest.file_name().is_none_or(|name| name != MANIFEST) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(root.join(manifest)) else {
            continue;
        };
        let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&text) else {
            continue;
        };
        let entry = declared.entry(repo.unit_of(manifest)).or_default();
        entry.manifest = manifest.clone();
        if let Some(name) = parsed.get("name").and_then(serde_json::Value::as_str) {
            entry.own = Some(name.to_string());
        }
        for table in ["dependencies", "peerDependencies", "optionalDependencies"] {
            let Some(names) = parsed.get(table).and_then(serde_json::Value::as_object) else {
                continue;
            };
            for name in names.keys() {
                entry.shipped.insert(name.clone());
                entry.dev.insert(name.clone());
                if table == "peerDependencies" {
                    entry.peers.insert(name.clone());
                }
            }
        }
        if let Some(scripts) = parsed.get("scripts").and_then(serde_json::Value::as_object) {
            for script in scripts.values().filter_map(serde_json::Value::as_str) {
                for word in script.split_whitespace() {
                    entry.scripts.insert(word.to_string());
                }
            }
        }
        if let Some(names) = parsed
            .get("devDependencies")
            .and_then(serde_json::Value::as_object)
        {
            for name in names.keys() {
                entry.dev.insert(name.clone());
            }
        }
    }
    declared
}

// The package a module specifier names: the first segment, two under a scope.
pub fn packaged(from: &str) -> String {
    if from.starts_with('@') {
        from.splitn(3, '/').take(2).collect::<Vec<&str>>().join("/")
    } else {
        from.split('/').next().unwrap_or(from).to_string()
    }
}

// A toolchain's config file runs at build time and ships nothing.
pub fn developmental(file: &Path) -> bool {
    file.file_stem()
        .map(|stem| stem.to_string_lossy().to_string())
        .unwrap_or_default()
        .ends_with(".config")
}

// Files the walk never checks whose imports still reach: scripts in .mjs,
// .cjs and .js, and code under dot folders such as a storybook's.
pub fn companions(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![(root.to_path_buf(), false)];
    while let Some((folder, hidden)) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&folder) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            let name = entry.file_name().to_string_lossy().to_string();
            if kind.is_dir() {
                if name == ".git"
                    || name == ".arbiter"
                    || name == "node_modules"
                    || name == "dist"
                    || name == "third_party"
                    || name.starts_with("bazel-")
                {
                    continue;
                }
                stack.push((entry.path(), hidden || name.starts_with('.')));
            } else if kind.is_file() {
                let scripted =
                    name.ends_with(".mjs") || name.ends_with(".cjs") || name.ends_with(".js");
                let shaded = hidden
                    && !name.ends_with(".d.ts")
                    && (name.ends_with(".ts") || name.ends_with(".tsx"));
                if scripted || shaded {
                    if let Ok(relative) = entry.path().strip_prefix(root) {
                        found.push(relative.to_path_buf());
                    }
                }
            }
        }
    }
    found
}
