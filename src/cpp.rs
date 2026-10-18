use std::path::{Path, PathBuf};

use tree_sitter::{Language, Tree};

use crate::trees;
use crate::units::{Lang, Repo};
use crate::{Def, Edge};

const DEFINITIONS: &str = include_str!("../packs/cpp/definitions.scm");
const USES: &str = include_str!("../packs/cpp/uses.scm");

pub struct Source {
    pub file: PathBuf,
    pub includes: Vec<String>,
    pub calls: Vec<String>,
}

pub fn language() -> Language {
    tree_sitter_cpp::LANGUAGE.into()
}

pub fn definitions(tree: &Tree, source: &str) -> Vec<(String, usize)> {
    trees::matches(&language(), DEFINITIONS, tree, source)
        .into_iter()
        .flatten()
        .filter(|capture| capture.name == "def")
        .map(|capture| (capture.text, capture.line))
        .collect()
}

pub fn read(file: &Path, tree: &Tree, source: &str) -> Source {
    let mut includes = Vec::new();
    let mut calls = Vec::new();
    for captures in trees::matches(&language(), USES, tree, source) {
        for capture in captures {
            match capture.name.as_str() {
                "include" => includes.push(capture.text.trim_matches('"').to_string()),
                "use" => calls.push(capture.text),
                _ => {}
            }
        }
    }
    Source {
        file: file.to_path_buf(),
        includes,
        calls,
    }
}

pub fn resolve(sources: &[Source], defs: &[Def], repo: &Repo) -> Vec<Edge> {
    let mut edges = Vec::new();
    for source in sources {
        for (index, def) in defs.iter().enumerate() {
            if def.lang != Lang::Cpp {
                continue;
            }
            let Some(header) = def.file.file_name().map(|name| name.to_string_lossy()) else {
                continue;
            };
            let included = source
                .includes
                .iter()
                .any(|name| *name == header || name.ends_with(&format!("/{header}")));
            let called = source.calls.iter().any(|call| *call == def.name);
            if included && called {
                edges.push(Edge {
                    def: index,
                    file: source.file.clone(),
                    unit: repo.unit_of(&source.file),
                });
            }
        }
    }
    edges
}
