use std::path::{Path, PathBuf};

use tree_sitter::{Language, Tree};

use crate::pack::{Def, Definition, Edge, Pack};
use crate::trees;
use crate::units::Repo;

const DEFINITIONS: &str = include_str!("../packs/rust/definitions.scm");
const USES: &str = include_str!("../packs/rust/uses.scm");
const MENTIONS: &str = include_str!("../packs/rust/mentions.scm");

pub struct Rust {
    uses: Vec<(PathBuf, String, String)>,
}

impl Rust {
    pub fn new() -> Self {
        Self { uses: Vec::new() }
    }
}

impl Pack for Rust {
    fn handles(&self, name: &str) -> bool {
        name.ends_with(".rs")
    }

    // Cargo's convention: a crate's binary entry is src/main.rs beside its manifest.
    fn roots(&self, _root: &Path, repo: &Repo) -> Vec<PathBuf> {
        let mut found = Vec::new();
        for manifest in &repo.manifests {
            if manifest.file_name().is_none_or(|name| name != "Cargo.toml") {
                continue;
            }
            let folder = manifest.parent().unwrap_or(Path::new(""));
            let entry = folder.join("src").join("main.rs");
            if repo.files.contains(&entry) {
                found.push(entry);
            }
        }
        found
    }

    fn scan(&mut self, file: &Path, source: &str, surface: bool) -> Vec<Definition> {
        let Some(tree) = trees::parse(&language(), source) else {
            return Vec::new();
        };
        for (module, name) in uses(&tree, source) {
            self.uses.push((file.to_path_buf(), module, name));
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
        for (file, module, name) in &self.uses {
            let Some(segment) = module.rsplit("::").next() else {
                continue;
            };
            if segment == "self" || segment == "super" {
                continue;
            }
            for (index, def) in defs.iter().enumerate() {
                if def.pack != mine || def.name != *name {
                    continue;
                }
                if is_module_file(segment, &def.file) {
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

fn is_module_file(segment: &str, file: &Path) -> bool {
    let shown = file.to_string_lossy();
    // The crate's own root module answers for the crate keyword.
    let endings = if segment == "crate" {
        ["src/main.rs".to_string(), "src/lib.rs".to_string()]
    } else {
        [format!("{segment}.rs"), format!("{segment}/mod.rs")]
    };
    endings
        .iter()
        .any(|ending| shown == *ending || shown.ends_with(&format!("/{ending}")))
}

fn language() -> Language {
    tree_sitter_rust::LANGUAGE.into()
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

fn uses(tree: &Tree, source: &str) -> Vec<(String, String)> {
    trees::matches(&language(), USES, tree, source)
        .into_iter()
        .filter_map(|captures| {
            let module = captures.iter().find(|capture| capture.name == "module")?;
            let name = captures.iter().find(|capture| capture.name == "name")?;
            Some((module.text.clone(), name.text.clone()))
        })
        .collect()
}
