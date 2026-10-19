use std::ffi::OsStr;
use std::path::{Component, Path, PathBuf};

use tree_sitter::{Language, Tree};

use crate::pack::{Def, Definition, Edge, Pack};
use crate::trees;
use crate::units::Repo;

const DEFINITIONS: &str = include_str!("../packs/typescript/definitions.scm");
const USES: &str = include_str!("../packs/typescript/uses.scm");
const MENTIONS: &str = include_str!("../packs/typescript/mentions.scm");

#[derive(Default)]
pub struct TypeScript {
    imports: Vec<(PathBuf, String, String)>,
}

impl TypeScript {
    pub fn new() -> Self {
        Self {
            imports: Vec::new(),
        }
    }
}

impl Pack for TypeScript {
    fn handles(&self, name: &str) -> bool {
        !name.ends_with(".d.ts") && (name.ends_with(".ts") || name.ends_with(".tsx"))
    }

    fn roots(&self, _root: &Path, _repo: &Repo) -> Vec<PathBuf> {
        Vec::new()
    }

    fn scan(&mut self, file: &Path, source: &str, surface: bool) -> Vec<Definition> {
        let Some(tree) = trees::parse(&language(), source) else {
            return Vec::new();
        };
        for (from, name) in imports(&tree, source) {
            self.imports.push((file.to_path_buf(), from, name));
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
        for (file, from, name) in &self.imports {
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
                if def.pack != mine || def.name != *name {
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
}

fn language() -> Language {
    tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()
}

fn definitions(tree: &Tree, source: &str) -> Vec<(String, usize)> {
    trees::matches(&language(), DEFINITIONS, tree, source)
        .into_iter()
        .flatten()
        .filter(|capture| capture.name == "def")
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

fn imports(tree: &Tree, source: &str) -> Vec<(String, String)> {
    trees::matches(&language(), USES, tree, source)
        .into_iter()
        .filter_map(|captures| {
            let from = captures.iter().find(|capture| capture.name == "source")?;
            let name = captures.iter().find(|capture| capture.name == "name")?;
            Some((from.text.clone(), name.text.clone()))
        })
        .collect()
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

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::normalized;

    #[test]
    fn dots_fold_away_and_parents_climb() {
        assert_eq!(
            normalized(Path::new("web/src/./sib")),
            PathBuf::from("web/src/sib")
        );
        assert_eq!(
            normalized(Path::new("web/src/../lib/x")),
            PathBuf::from("web/lib/x")
        );
    }
}
