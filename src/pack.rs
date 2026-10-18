use std::path::{Path, PathBuf};

use crate::cpp::Cpp;
use crate::python::Python;
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
    fn handles(&self, name: &str) -> bool;

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
        Box::new(TypeScript::new()),
    ]
}
