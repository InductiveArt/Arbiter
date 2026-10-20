use std::path::{Path, PathBuf};

use tree_sitter::{Language, Tree};

use crate::pack::{Def, Definition, Edge, Pack, Suppression};
use crate::trees;
use crate::units::Repo;

const DEFINITIONS: &str = include_str!("../packs/proto/definitions.scm");
const USES: &str = include_str!("../packs/proto/uses.scm");
const MENTIONS: &str = include_str!("../packs/proto/mentions.scm");

#[derive(Default)]
pub struct Proto {
    typerefs: Vec<(PathBuf, String, usize)>,
    imports: Vec<(PathBuf, String)>,
}

impl Proto {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Pack for Proto {
    fn label(&self) -> &'static str {
        "proto"
    }

    fn handles(&self, name: &str) -> bool {
        name.ends_with(".proto")
    }

    fn manifest(&self) -> Option<&'static str> {
        Some("buf.yaml")
    }

    fn adapter(&self) -> Option<&'static str> {
        Some(include_str!("../packs/proto/tools.toml"))
    }

    fn roots(&self, _root: &Path, _repo: &Repo) -> Vec<PathBuf> {
        Vec::new()
    }

    fn scan(&mut self, file: &Path, source: &str, surface: bool) -> Vec<Definition> {
        let Some(tree) = trees::parse(&language(), source) else {
            return Vec::new();
        };
        for (name, line) in trees::texts(&language(), USES, &tree, source, "typeref") {
            self.typerefs.push((file.to_path_buf(), name, line));
        }
        for (path, _) in trees::texts(&language(), USES, &tree, source, "import") {
            self.imports
                .push((file.to_path_buf(), path.trim_matches('"').to_string()));
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

    // A type reference reaches a definition in its own file, or in a file it imports.
    fn resolve(&self, mine: usize, defs: &[Def], repo: &Repo) -> Vec<Edge> {
        let mut edges = Vec::new();
        for (file, reference, line) in &self.typerefs {
            let name = reference.rsplit('.').next().unwrap_or(reference);
            for (index, def) in defs.iter().enumerate() {
                if def.pack != mine || def.name != name || def.file == *file {
                    continue;
                }
                let imported = self
                    .imports
                    .iter()
                    .any(|(from, path)| from == file && def.file.to_string_lossy().ends_with(path));
                if imported {
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

    fn suppressions(&self) -> &[Suppression] {
        &[]
    }
}

fn language() -> Language {
    tree_sitter_proto::LANGUAGE.into()
}

fn definitions(tree: &Tree, source: &str) -> Vec<(String, usize)> {
    trees::texts(&language(), DEFINITIONS, tree, source, "def")
}

fn mentions(tree: &Tree, source: &str) -> Vec<(String, usize)> {
    trees::texts(&language(), MENTIONS, tree, source, "mention")
}
