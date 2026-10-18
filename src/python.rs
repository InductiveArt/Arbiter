use std::path::PathBuf;

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
        let path = module.replace('.', "/");
        let endings = [format!("{path}.py"), format!("{path}/__init__.py")];
        for (index, def) in defs.iter().enumerate() {
            if def.lang != Lang::Python || def.name != *name {
                continue;
            }
            let shown = def.file.to_string_lossy();
            let defined_there = endings
                .iter()
                .any(|ending| shown == *ending || shown.ends_with(&format!("/{ending}")));
            if defined_there {
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
