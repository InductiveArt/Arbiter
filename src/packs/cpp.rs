use std::path::Path;
use std::path::PathBuf;

use tree_sitter::Language;
use tree_sitter::Tree;

use crate::core::pack::Def;
use crate::core::pack::Definition;
use crate::core::pack::Edge;
use crate::core::pack::Pack;
use crate::core::pack::Repo;
use crate::core::trees;

const DEFINITIONS: &str = include_str!("../../packs/cpp/definitions.scm");
const USES: &str = include_str!("../../packs/cpp/uses.scm");

struct Source {
    file: PathBuf,
    includes: Vec<String>,
    calls: Vec<(String, usize)>,
}

#[derive(Default)]
pub struct Cpp {
    sources: Vec<Source>,
}

impl Cpp {
    pub fn new() -> Self {
        Self {
            sources: Vec::new(),
        }
    }
}

impl Pack for Cpp {
    fn rules(&self) -> &'static [(&'static str, &'static str)] {
        &[
            (
                "unpinned-image",
                "never: the language names no container images",
            ),
            ("comment-overreach", "held"),
            (
                "crowded-import",
                "never: an include names one header by syntax",
            ),
            (
                "drift",
                "never: the proto pack judges any unit's buf.gen.yaml, whatever its language",
            ),
        ]
    }

    fn label(&self) -> &'static str {
        "cpp"
    }

    fn handles(&self, name: &str) -> bool {
        let Some(extension) = name.rsplit('.').next() else {
            return false;
        };
        matches!(extension, "cc" | "cpp" | "cxx" | "h" | "hh" | "hpp")
    }

    fn shapes(&self, source: &str) -> Vec<trees::Shape> {
        trees::shapes(&language(), source)
    }

    fn roots(&self, _root: &Path, _repo: &Repo) -> Vec<PathBuf> {
        Vec::new()
    }

    fn scan(&mut self, file: &Path, source: &str, surface: bool) -> Vec<Definition> {
        let Some(tree) = trees::parse(&language(), source) else {
            return Vec::new();
        };
        self.sources.push(read(file, &tree, source));
        if !surface || !is_header(file) {
            return Vec::new();
        }
        definitions(&tree, source)
            .into_iter()
            .map(|(name, line)| Definition {
                name,
                line,
                self_used: false,
            })
            .collect()
    }

    fn resolve(&self, mine: usize, defs: &[Def], _root: &Path, repo: &Repo) -> Vec<Edge> {
        let mut edges = Vec::new();
        for source in &self.sources {
            for (index, def) in defs.iter().enumerate() {
                if def.pack != mine {
                    continue;
                }
                let Some(header) = def.file.file_name().map(|name| name.to_string_lossy()) else {
                    continue;
                };
                let included = source
                    .includes
                    .iter()
                    .any(|name| *name == header || name.ends_with(&format!("/{header}")));
                let called = source.calls.iter().find(|(call, _)| *call == def.name);
                if let (true, Some((_, line))) = (included, called) {
                    edges.push(Edge {
                        def: index,
                        file: source.file.clone(),
                        line: *line,
                        unit: repo.unit_of(&source.file),
                    });
                }
            }
        }
        edges
    }
}

fn language() -> Language {
    tree_sitter_cpp::LANGUAGE.into()
}

fn is_header(file: &Path) -> bool {
    matches!(
        file.extension().and_then(|extension| extension.to_str()),
        Some("h" | "hh" | "hpp")
    )
}

fn definitions(tree: &Tree, source: &str) -> Vec<(String, usize)> {
    trees::texts(&language(), DEFINITIONS, tree, source, "def")
}

fn read(file: &Path, tree: &Tree, source: &str) -> Source {
    let mut includes = Vec::new();
    let mut calls = Vec::new();
    for captures in trees::matches(&language(), USES, tree, source) {
        for capture in captures {
            match capture.name.as_str() {
                "include" => includes.push(capture.text.trim_matches('"').to_string()),
                "use" => calls.push((capture.text, capture.line)),
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
