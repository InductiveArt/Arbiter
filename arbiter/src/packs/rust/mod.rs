mod language;
mod laws;
mod layout;
mod manifest;
mod mend;
mod module_files;
mod read;
mod reading;
mod resolve;
mod use_tree;

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
use crate::packs::rust::reading::Reading;

// The Rust pack as the pack contract asks for it: what the language is, and
// what it read from each file, one reading per file.
#[derive(Default)]
pub struct Rust {
    readings: Vec<Reading>,
}

impl Rust {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Pack for Rust {
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

    fn container(&self, manifest: &str) -> bool {
        language::container(manifest)
    }

    fn dials(&self, home: &Path, folder: &Path) -> Option<(Vec<Finding>, Vec<String>)> {
        Some((laws::dials(home, folder), Vec::new()))
    }

    fn mend(&self, root: &Path, repo: &Repo) -> Vec<String> {
        mend::mend(root, repo)
    }

    fn shapes(&self, source: &str) -> Vec<trees::Shape> {
        trees::shapes(&language::grammar(), source)
    }

    fn roots(&self, _root: &Path, repo: &Repo) -> Vec<PathBuf> {
        language::roots(repo)
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

    fn axioms(&self) -> &'static [&'static str] {
        language::AXIOMS
    }

    fn lockfiles(&self) -> &'static [&'static str] {
        language::LOCKFILES
    }

    fn epoch(&self, version: &str) -> String {
        language::epoch(version)
    }

    fn integrity(&self, root: &Path, repo: &Repo) -> Vec<Finding> {
        laws::instances(&self.readings, root, repo)
    }

    fn resolve(&self, mine: usize, defs: &[Def], root: &Path, repo: &Repo) -> Vec<Edge> {
        resolve::edges(&self.readings, mine, defs, root, repo)
    }
}
