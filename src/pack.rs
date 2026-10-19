use std::path::{Path, PathBuf};

use crate::cpp::Cpp;
use crate::python::Python;
use crate::rust::Rust;
use crate::typescript::TypeScript;
use crate::units::Repo;

pub struct Def {
    pub name: String,
    pub file: PathBuf,
    pub line: usize,
    pub unit: usize,
    pub pack: usize,
    // Whether the defining file itself names the symbol away from its definition line.
    pub self_used: bool,
}

pub struct Edge {
    pub def: usize,
    pub file: PathBuf,
    pub unit: usize,
}

pub struct Definition {
    pub name: String,
    pub line: usize,
    pub self_used: bool,
}

// A language. It says what defines a symbol and what uses one; the core knows no language.
pub trait Pack {
    fn label(&self) -> &'static str;

    fn handles(&self, name: &str) -> bool;

    // The manifest file marking a folder as a unit of this language.
    fn manifest(&self) -> Option<&'static str> {
        None
    }

    // The adapter: the tools that hold a unit of this language to the bar, as TOML.
    fn adapter(&self) -> Option<&'static str> {
        None
    }

    // A manifest declaring members and no code of its own bounds a unit without being one to judge.
    fn container(&self, manifest: &str) -> bool {
        let _ = manifest;
        false
    }

    // Holds the unit's declared environment to the pack's bar; None where no bar exists yet.
    // Settings may add, never lower: each finding names one option sitting below the bar.
    fn wiring(&self, folder: &Path) -> Option<Vec<String>> {
        let _ = folder;
        None
    }

    // Files whose symbols live by being named from outside the repository.
    fn roots(&self, root: &Path, repo: &Repo) -> Vec<PathBuf>;

    // Reads one file, keeping its uses; definitions come back only from a surface file.
    fn scan(&mut self, file: &Path, source: &str, surface: bool) -> Vec<Definition>;

    fn resolve(&self, mine: usize, defs: &[Def], repo: &Repo) -> Vec<Edge>;
}

pub fn all() -> Vec<Box<dyn Pack>> {
    vec![
        Box::new(Cpp::new()),
        Box::new(Python::new()),
        Box::new(Rust::new()),
        Box::new(TypeScript::new()),
    ]
}
