mod language;
mod laws;
mod read;
mod reading;

use std::path::Path;
use std::path::PathBuf;

use crate::core::pack::Def;
use crate::core::pack::Definition;
use crate::core::pack::Edge;
use crate::core::pack::Pack;
use crate::core::pack::Repo;
use crate::core::stance::Stances;
use crate::domain::finding::Finding;
use crate::packs::infra::reading::Reading;

// Deployment manifests as checked text: the pack defines no symbols and runs
// no tools; it holds the written state to the laws text alone can prove.
#[derive(Default)]
pub struct Infra {
    readings: Vec<Reading>,
}

impl Infra {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Pack for Infra {
    fn stances(&self) -> &'static Stances {
        &language::STANCES
    }

    fn label(&self) -> &'static str {
        language::LABEL
    }

    fn handles(&self, name: &str) -> bool {
        language::handles(name)
    }

    fn roots(&self, _root: &Path, _repo: &Repo) -> Vec<PathBuf> {
        Vec::new()
    }

    fn scan(&mut self, file: &Path, source: &str, surface: bool) -> Vec<Definition> {
        self.readings.extend(read::read(file, source, surface));
        Vec::new()
    }

    fn integrity(&self, _root: &Path, _repo: &Repo) -> Vec<Finding> {
        laws::instances(&self.readings)
    }

    fn resolve(&self, _mine: usize, _defs: &[Def], _root: &Path, _repo: &Repo) -> Vec<Edge> {
        Vec::new()
    }
}
