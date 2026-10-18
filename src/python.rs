use std::path::{Path, PathBuf};

use tree_sitter::{Language, Tree};

use crate::trees;
use crate::units::{Lang, Repo};
use crate::{Def, Edge};

const DEFINITIONS: &str = include_str!("../packs/python/definitions.scm");
const USES: &str = include_str!("../packs/python/uses.scm");
const MENTIONS: &str = include_str!("../packs/python/mentions.scm");

pub fn language() -> Language {
    tree_sitter_python::LANGUAGE.into()
}

pub fn definitions(tree: &Tree, source: &str) -> Vec<(String, usize)> {
    trees::matches(&language(), DEFINITIONS, tree, source)
        .into_iter()
        .flatten()
        .filter(|capture| capture.name == "def" && !capture.text.starts_with('_'))
        .map(|capture| (capture.text, capture.line))
        .collect()
}

pub fn mentions(tree: &Tree, source: &str) -> Vec<(String, usize)> {
    trees::matches(&language(), MENTIONS, tree, source)
        .into_iter()
        .flatten()
        .map(|capture| (capture.text, capture.line))
        .collect()
}

pub fn imports(tree: &Tree, source: &str) -> Vec<(String, String)> {
    trees::matches(&language(), USES, tree, source)
        .into_iter()
        .filter_map(|captures| {
            let module = captures.iter().find(|capture| capture.name == "module")?;
            let name = captures.iter().find(|capture| capture.name == "name")?;
            Some((module.text.clone(), name.text.clone()))
        })
        .collect()
}

pub fn resolve(imports: &[(PathBuf, String, String)], defs: &[Def], repo: &Repo) -> Vec<Edge> {
    let mut edges = Vec::new();
    for (file, module, name) in imports {
        for (index, def) in defs.iter().enumerate() {
            if def.lang != Lang::Python || def.name != *name {
                continue;
            }
            if is_module_file(module, &def.file) {
                edges.push(Edge {
                    def: index,
                    file: file.clone(),
                    unit: repo.unit_of(file),
                });
            }
        }
    }
    edges
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
pub fn manifest_entries(source: &str) -> Vec<String> {
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

pub fn module_file(module: &str, files: &[(PathBuf, Lang)]) -> Option<PathBuf> {
    files
        .iter()
        .find(|(file, lang)| *lang == Lang::Python && is_module_file(module, file))
        .map(|(file, _)| file.clone())
}
