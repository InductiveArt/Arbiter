use std::fs;
use std::path::Path;
use std::path::PathBuf;

use crate::core::repository::Repo;
use crate::packs::python::language::MANIFEST;

// Every module a manifest names as an entry point is a root.
pub fn roots(root: &Path, repo: &Repo) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for manifest in &repo.manifests {
        if manifest.file_name().is_none_or(|name| name != MANIFEST) {
            continue;
        }
        let Ok(source) = fs::read_to_string(root.join(manifest)) else {
            continue;
        };
        for module in manifest_entries(&source) {
            found.extend(module_file(&module, &repo.files));
        }
    }
    found
}

pub fn is_module_file(module: &str, file: &Path) -> bool {
    let path = module.replace('.', "/");
    let endings = [format!("{path}.py"), format!("{path}/__init__.py")];
    let shown = file.to_string_lossy();
    endings
        .iter()
        .any(|ending| shown == *ending || shown.ends_with(&format!("/{ending}")))
}

// An entry point is called by name from outside the repository, so its module is a root.
fn manifest_entries(source: &str) -> Vec<String> {
    let Ok(parsed) = source.parse::<toml::Value>() else {
        return Vec::new();
    };
    let Some(project) = parsed.get("project") else {
        return Vec::new();
    };
    let mut tables: Vec<&toml::Value> = Vec::new();
    for name in ["scripts", "gui-scripts"] {
        tables.extend(project.get(name));
    }
    if let Some(groups) = project.get("entry-points").and_then(toml::Value::as_table) {
        tables.extend(groups.values());
    }
    let mut modules = Vec::new();
    for table in tables {
        let Some(table) = table.as_table() else {
            continue;
        };
        for value in table.values() {
            if let Some(reference) = value.as_str() {
                let module = reference.split(':').next().unwrap_or(reference).trim();
                modules.push(module.to_string());
            }
        }
    }
    modules
}

pub fn module_file(module: &str, files: &[PathBuf]) -> Option<PathBuf> {
    files
        .iter()
        .find(|file| is_module_file(module, file))
        .cloned()
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::is_module_file;

    #[test]
    fn a_module_path_reaches_its_file_or_its_package() {
        assert!(is_module_file(
            "used.calc",
            Path::new("libs/used_py/src/used/calc.py")
        ));
        assert!(is_module_file("used", Path::new("src/used/__init__.py")));
    }

    #[test]
    fn a_longer_name_sharing_a_suffix_is_another_module() {
        assert!(!is_module_file(
            "used.calc",
            Path::new("src/used/recalc.py")
        ));
        assert!(!is_module_file("calc", Path::new("src/used/calc.pyi")));
    }
}
