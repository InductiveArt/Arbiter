use std::path::Path;
use std::path::PathBuf;

use crate::core::repository::is_test;
use crate::core::repository::Repo;
use crate::packs::typescript::language::MANIFEST;

// Each third party's license as its package installed it under the unit's
// node_modules, by the manifest that pulled it.
pub fn licenses(root: &Path, repo: &Repo) -> Vec<(PathBuf, String, String)> {
    let mut found = Vec::new();
    for manifest in &repo.manifests {
        if manifest.file_name().is_none_or(|name| name != MANIFEST) {
            continue;
        }
        let unit = repo.unit_of(manifest);
        let home = repo.units[unit].path.clone();
        if is_test(&home) {
            continue;
        }
        let folder = manifest.parent().unwrap_or(Path::new(""));
        for dir in package_dirs(&root.join(folder).join("node_modules")) {
            let Ok(text) = std::fs::read_to_string(dir.join("package.json")) else {
                continue;
            };
            let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&text) else {
                continue;
            };
            let (name, license) = manifest_license(&parsed);
            if let Some(name) = name {
                found.push((manifest.clone(), name, license));
            }
        }
    }
    found.sort();
    found.dedup();
    found
}

// The installed packages, one folder each; a scope folder holds its own children.
fn package_dirs(modules: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(modules) else {
        return Vec::new();
    };
    let mut dirs = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        if name.starts_with('@') {
            if let Ok(scoped) = std::fs::read_dir(entry.path()) {
                dirs.extend(scoped.flatten().map(|child| child.path()));
            }
        } else {
            dirs.push(entry.path());
        }
    }
    dirs
}

// A manifest's license: the SPDX string, or the older object form's type field.
fn manifest_license(parsed: &serde_json::Value) -> (Option<String>, String) {
    let name = parsed
        .get("name")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    let license = parsed
        .get("license")
        .and_then(|value| {
            value.as_str().map(str::to_string).or_else(|| {
                value
                    .get("type")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_string)
            })
        })
        .unwrap_or_else(|| "unstated".to_string());
    (name, license)
}

#[cfg(test)]
mod tests {
    use super::manifest_license;

    #[test]
    fn a_license_is_the_string_or_the_object_form_or_unstated() {
        let spdx = serde_json::json!({"name": "a", "license": "MIT"});
        let object = serde_json::json!({"name": "b", "license": {"type": "ISC"}});
        let silent = serde_json::json!({"name": "c"});
        assert_eq!(
            manifest_license(&spdx),
            (Some("a".to_string()), "MIT".to_string())
        );
        assert_eq!(
            manifest_license(&object),
            (Some("b".to_string()), "ISC".to_string())
        );
        assert_eq!(
            manifest_license(&silent),
            (Some("c".to_string()), "unstated".to_string())
        );
    }
}
