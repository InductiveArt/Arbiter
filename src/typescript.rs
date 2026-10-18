use std::ffi::OsStr;
use std::path::{Component, Path, PathBuf};

use tree_sitter::{Language, Tree};

use crate::trees;
use crate::units::{Lang, Repo};
use crate::{Def, Edge};

const DEFINITIONS: &str = include_str!("../packs/typescript/definitions.scm");
const USES: &str = include_str!("../packs/typescript/uses.scm");
const MENTIONS: &str = include_str!("../packs/typescript/mentions.scm");

pub fn language() -> Language {
    tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()
}

pub fn definitions(tree: &Tree, source: &str) -> Vec<(String, usize)> {
    trees::matches(&language(), DEFINITIONS, tree, source)
        .into_iter()
        .flatten()
        .filter(|capture| capture.name == "def")
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
            let from = captures.iter().find(|capture| capture.name == "source")?;
            let name = captures.iter().find(|capture| capture.name == "name")?;
            Some((from.text.clone(), name.text.clone()))
        })
        .collect()
}

pub fn resolve(imports: &[(PathBuf, String, String)], defs: &[Def], repo: &Repo) -> Vec<Edge> {
    let mut edges = Vec::new();
    for (file, from, name) in imports {
        // A bare specifier names a package, which is not this repository's code.
        if !from.starts_with('.') {
            continue;
        }
        let base = file.parent().unwrap_or(Path::new(""));
        let joined = normalized(&base.join(from));
        let shown = joined.to_string_lossy();
        let candidates = [
            format!("{shown}.ts"),
            format!("{shown}/index.ts"),
            shown.to_string(),
        ];
        for (index, def) in defs.iter().enumerate() {
            if def.lang != Lang::TypeScript || def.name != *name {
                continue;
            }
            if candidates
                .iter()
                .any(|candidate| def.file.to_string_lossy() == *candidate)
            {
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

fn normalized(path: &Path) -> PathBuf {
    let mut parts: Vec<&OsStr> = Vec::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                parts.pop();
            }
            Component::Normal(part) => parts.push(part),
            _ => {}
        }
    }
    parts.iter().collect()
}
