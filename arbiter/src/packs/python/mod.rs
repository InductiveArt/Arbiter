mod distributions;
mod language;
mod laws;
mod licenses;
mod modules;
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
use crate::core::pack::Suppression;
use crate::core::stance::Stances;
use crate::core::trees;
use crate::domain::finding::Finding;
use crate::packs::python::reading::Reading;

// The Python pack as the pack contract asks for it: what the language is,
// and what it read from each file, one reading per file.
#[derive(Default)]
pub struct Python {
    readings: Vec<Reading>,
}

impl Python {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Pack for Python {
    fn stances(&self) -> &'static Stances {
        &language::STANCES
    }

    fn label(&self) -> &'static str {
        language::LABEL
    }

    fn handles(&self, name: &str) -> bool {
        language::handles(name)
    }

    fn manifest(&self) -> Option<&'static str> {
        Some(language::MANIFEST)
    }

    fn adapter(&self) -> Option<&'static str> {
        Some(language::ADAPTER)
    }

    fn configs(&self) -> &'static [(&'static str, &'static str)] {
        language::CONFIGS
    }

    fn container(&self, manifest: &str) -> bool {
        language::container(manifest)
    }

    fn dials(&self, home: &Path, folder: &Path) -> Option<(Vec<Finding>, Vec<String>)> {
        Some((laws::dials(home, folder), Vec::new()))
    }

    fn shapes(&self, source: &str) -> Vec<trees::Shape> {
        trees::shapes(&language::grammar(), source)
    }

    fn roots(&self, root: &Path, repo: &Repo) -> Vec<PathBuf> {
        modules::roots(root, repo)
    }

    fn scan(&mut self, file: &Path, source: &str, surface: bool) -> Vec<Definition> {
        let Some((reading, definitions)) = read::read(file, source, surface) else {
            return Vec::new();
        };
        self.readings.push(reading);
        definitions
    }

    fn suppressions(&self) -> Vec<Suppression> {
        self.readings
            .iter()
            .flat_map(|reading| reading.suppressions.iter().cloned())
            .collect()
    }

    fn outputs(&self) -> &'static [&'static str] {
        language::OUTPUTS
    }

    fn generated(&self, name: &str) -> bool {
        language::generated(name)
    }

    fn axioms(&self) -> &'static [&'static str] {
        language::AXIOMS
    }

    fn licenses(&self, root: &Path, repo: &Repo) -> Vec<(PathBuf, String, String)> {
        licenses::licenses(root, repo)
    }

    fn lockfiles(&self) -> &'static [&'static str] {
        language::LOCKFILES
    }

    fn integrity(&self, root: &Path, repo: &Repo) -> Vec<Finding> {
        laws::instances(&self.readings, root, repo)
    }

    fn resolve(&self, mine: usize, defs: &[Def], _root: &Path, repo: &Repo) -> Vec<Edge> {
        resolve::edges(&self.readings, mine, defs, repo)
    }
}
