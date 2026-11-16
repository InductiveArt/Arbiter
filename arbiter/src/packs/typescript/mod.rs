mod import_shape;
mod language;
mod laws;
mod licenses;
mod lockfiles;
mod mend;
mod packages;
mod paths;
mod read;
mod reading;
mod resolve;
mod routes;
mod surfaces;

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
use crate::packs::typescript::reading::Reading;

// The TypeScript pack as the pack contract asks for it: what the language
// is, and what it read from each file, one reading per file.
#[derive(Default)]
pub struct TypeScript {
    readings: Vec<Reading>,
}

impl TypeScript {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Pack for TypeScript {
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

    fn configs(&self) -> &'static [(&'static str, &'static str)] {
        language::CONFIGS
    }

    fn dials(&self, home: &Path, folder: &Path) -> Option<(Vec<Finding>, Vec<String>)> {
        Some(laws::dials(home, folder))
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

    fn suppressions(&self) -> Vec<Suppression> {
        self.readings
            .iter()
            .flat_map(|reading| reading.suppressions.iter().cloned())
            .collect()
    }

    fn integrity(&self, root: &Path, repo: &Repo) -> Vec<Finding> {
        laws::instances(&self.readings, root, repo)
    }

    fn outputs(&self) -> &'static [&'static str] {
        language::OUTPUTS
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

    fn locked(&self, name: &str, text: &str) -> Vec<(String, String)> {
        lockfiles::locked(name, text)
    }

    fn generated(&self, name: &str) -> bool {
        language::generated(name)
    }

    fn mend(&self, root: &Path, repo: &Repo) -> Vec<String> {
        mend::mend(root, repo)
    }

    fn resolve(&self, mine: usize, defs: &[Def], root: &Path, repo: &Repo) -> Vec<Edge> {
        resolve::edges(&self.readings, mine, defs, root, repo)
    }
}
