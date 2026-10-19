use std::fs;
use std::path::{Path, PathBuf};

use tree_sitter::{Language, Tree};

use crate::pack::{Def, Definition, Edge, Pack};
use crate::trees;
use crate::units::Repo;

const DEFINITIONS: &str = include_str!("../packs/python/definitions.scm");
const USES: &str = include_str!("../packs/python/uses.scm");
const MENTIONS: &str = include_str!("../packs/python/mentions.scm");

#[derive(Default)]
pub struct Python {
    imports: Vec<(PathBuf, String, String, usize)>,
}

impl Python {
    pub fn new() -> Self {
        Self {
            imports: Vec::new(),
        }
    }
}

impl Pack for Python {
    fn label(&self) -> &'static str {
        "python"
    }

    fn handles(&self, name: &str) -> bool {
        name.ends_with(".py")
    }

    fn manifest(&self) -> Option<&'static str> {
        Some("pyproject.toml")
    }

    fn adapter(&self) -> Option<&'static str> {
        Some(include_str!("../packs/python/tools.toml"))
    }

    fn container(&self, manifest: &str) -> bool {
        let Ok(parsed) = manifest.parse::<toml::Value>() else {
            return false;
        };
        let workspace = parsed
            .get("tool")
            .and_then(|tool| tool.get("uv"))
            .and_then(|uv| uv.get("workspace"));
        parsed.get("project").is_none() && workspace.is_some()
    }

    fn roots(&self, root: &Path, repo: &Repo) -> Vec<PathBuf> {
        let mut found = Vec::new();
        for manifest in &repo.manifests {
            if manifest
                .file_name()
                .is_none_or(|name| name != "pyproject.toml")
            {
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

    fn scan(&mut self, file: &Path, source: &str, surface: bool) -> Vec<Definition> {
        let Some(tree) = trees::parse(&language(), source) else {
            return Vec::new();
        };
        for (module, name, line) in imports(&tree, source) {
            self.imports.push((file.to_path_buf(), module, name, line));
        }
        if !surface {
            return Vec::new();
        }
        let mentions = mentions(&tree, source);
        definitions(&tree, source)
            .into_iter()
            .map(|(name, line)| Definition {
                self_used: mentions
                    .iter()
                    .any(|(mentioned, at)| *mentioned == name && *at != line),
                name,
                line,
            })
            .collect()
    }

    fn resolve(&self, mine: usize, defs: &[Def], repo: &Repo) -> Vec<Edge> {
        let mut edges = Vec::new();
        for (file, module, name, line) in &self.imports {
            for (index, def) in defs.iter().enumerate() {
                if def.pack != mine || def.name != *name {
                    continue;
                }
                if is_module_file(module, &def.file) {
                    edges.push(Edge {
                        def: index,
                        file: file.clone(),
                        line: *line,
                        unit: repo.unit_of(file),
                    });
                }
            }
        }
        edges
    }
}

fn language() -> Language {
    tree_sitter_python::LANGUAGE.into()
}

fn definitions(tree: &Tree, source: &str) -> Vec<(String, usize)> {
    trees::matches(&language(), DEFINITIONS, tree, source)
        .into_iter()
        .flatten()
        .filter(|capture| capture.name == "def" && !capture.text.starts_with('_'))
        .map(|capture| (capture.text, capture.line))
        .collect()
}

fn mentions(tree: &Tree, source: &str) -> Vec<(String, usize)> {
    trees::matches(&language(), MENTIONS, tree, source)
        .into_iter()
        .flatten()
        .map(|capture| (capture.text, capture.line))
        .collect()
}

fn imports(tree: &Tree, source: &str) -> Vec<(String, String, usize)> {
    trees::matches(&language(), USES, tree, source)
        .into_iter()
        .filter_map(|captures| {
            let module = captures.iter().find(|capture| capture.name == "module")?;
            let name = captures.iter().find(|capture| capture.name == "name")?;
            Some((module.text.clone(), name.text.clone(), name.line))
        })
        .collect()
}

fn is_module_file(module: &str, file: &Path) -> bool {
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

fn module_file(module: &str, files: &[PathBuf]) -> Option<PathBuf> {
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
