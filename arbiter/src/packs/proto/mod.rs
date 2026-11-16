mod buf;
mod contracts;
mod generation;
mod language;
mod laws;
mod manifest;
mod read;
mod reading;
mod resolve;

use std::path::Path;
use std::path::PathBuf;

use crate::core::pack::Def;
use crate::core::pack::Definition;
use crate::core::pack::Drift;
use crate::core::pack::Edge;
use crate::core::pack::Pack;
use crate::core::pack::Repo;
use crate::core::pack::Suppression;
use crate::core::stance::Stances;
use crate::domain::finding::Finding;
use crate::packs::proto::reading::Reading;

// The proto pack as the pack contract asks for it: what the contract
// language is, what it read from each contract, and the code generated from
// the contracts held to them.
#[derive(Default)]
pub struct Proto {
    readings: Vec<Reading>,
}

impl Proto {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Pack for Proto {
    fn stances(&self) -> &'static Stances {
        &language::STANCES
    }

    fn label(&self) -> &'static str {
        language::LABEL
    }

    fn handles(&self, name: &str) -> bool {
        language::handles(name)
    }

    fn axioms(&self) -> &'static [&'static str] {
        language::AXIOMS
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

    fn suppressions(&self) -> Vec<Suppression> {
        self.readings
            .iter()
            .flat_map(|reading| reading.suppressions.iter().cloned())
            .collect()
    }

    fn integrity(&self, root: &Path, repo: &Repo) -> Vec<Finding> {
        laws::instances(&self.readings, root, repo)
    }

    fn drift(
        &self,
        root: &Path,
        folder: &Path,
        fixing: bool,
        cold: bool,
        notes: &mut Vec<String>,
    ) -> Vec<Drift> {
        let mut found: Vec<Drift> = generation::drift(root, folder, fixing, cold)
            .into_iter()
            .collect();
        found.extend(contracts::drift(root, folder, notes));
        found
    }

    fn record(&self, root: &Path, repo: &Repo) -> Vec<String> {
        contracts::record(root, repo)
    }
}
