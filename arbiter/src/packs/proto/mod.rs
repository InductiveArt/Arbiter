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

use crate::core::definition::Def;
use crate::core::definition::Definition;
use crate::core::drift::Drift;
use crate::core::edge::Edge;
use crate::core::language::Language;
use crate::core::reader::Reader;
use crate::core::repository::Repo;
use crate::core::stance::Stances;
use crate::core::suppression::Suppression;
use crate::domain::finding::Finding;
use crate::packs::proto::reading::Reading;

// The contract language as the pack contracts ask for it: what it is, how
// it reads one contract, what it makes of its readings, and the code
// generated from the contracts held to them.
pub struct Proto;

impl Language for Proto {
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

    fn roots(&self, _root: &Path, _repo: &Repo) -> Vec<PathBuf> {
        Vec::new()
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

impl Reader for Proto {
    type Reading = Reading;

    fn read(&self, file: &Path, source: &str, surface: bool) -> Option<(Reading, Vec<Definition>)> {
        read::read(file, source, surface)
    }

    fn resolve(
        &self,
        readings: &[Reading],
        mine: usize,
        defs: &[Def],
        _root: &Path,
        repo: &Repo,
    ) -> Vec<Edge> {
        resolve::edges(readings, mine, defs, repo)
    }

    fn laws(&self, readings: &[Reading], root: &Path, repo: &Repo) -> Vec<Finding> {
        laws::instances(readings, root, repo)
    }

    fn suppressions(&self, readings: &[Reading]) -> Vec<Suppression> {
        readings
            .iter()
            .flat_map(|reading| reading.suppressions.iter().cloned())
            .collect()
    }
}
