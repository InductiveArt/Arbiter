mod language;
mod read;
mod reading;
mod resolve;

use std::path::Path;
use std::path::PathBuf;

use crate::core::pack::Def;
use crate::core::pack::Definition;
use crate::core::pack::Edge;
use crate::core::pack::Pack;
use crate::core::pack::Repo;
use crate::core::stance::Stances;
use crate::core::trees;
use crate::packs::cpp::reading::Reading;

// The C++ pack as the pack contract asks for it: what the language is, and
// what it read from each file, one reading per file.
#[derive(Default)]
pub struct Cpp {
    readings: Vec<Reading>,
}

impl Cpp {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Pack for Cpp {
    fn stances(&self) -> &'static Stances {
        &language::STANCES
    }

    fn label(&self) -> &'static str {
        language::LABEL
    }

    fn handles(&self, name: &str) -> bool {
        language::handles(name)
    }

    fn shapes(&self, source: &str) -> Vec<trees::Shape> {
        trees::shapes(&language::grammar(), source)
    }

    fn roots(&self, _root: &Path, _repo: &Repo) -> Vec<PathBuf> {
        Vec::new()
    }

    fn scan(&mut self, file: &Path, source: &str, surface: bool) -> Vec<Definition> {
        let Some((reading, definitions)) = read::read(file, source, surface) else {
            return Vec::new();
        };
        self.readings.push(reading);
        definitions
    }

    fn resolve(&self, mine: usize, defs: &[Def], _root: &Path, repo: &Repo) -> Vec<Edge> {
        resolve::edges(&self.readings, mine, defs, repo)
    }
}
