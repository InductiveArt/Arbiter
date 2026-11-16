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

use crate::core::definition::Def;
use crate::core::definition::Definition;
use crate::core::edge::Edge;
use crate::core::language::Language;
use crate::core::reader::Reader;
use crate::core::repository::Repo;
use crate::core::stance::Stances;
use crate::core::suppression::Suppression;
use crate::core::trees;
use crate::domain::finding::Finding;
use crate::packs::rust::reading::Reading;

// The Rust language as the pack contracts ask for it: what it is, how it
// reads one file, and what it makes of its readings.
pub struct Rust;

impl Language for Rust {
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
}

impl Reader for Rust {
    type Reading = Reading;

    fn read(&self, file: &Path, source: &str, surface: bool) -> Option<(Reading, Vec<Definition>)> {
        read::read(file, source, surface)
    }

    fn resolve(
        &self,
        readings: &[Reading],
        mine: usize,
        defs: &[Def],
        root: &Path,
        repo: &Repo,
    ) -> Vec<Edge> {
        resolve::edges(readings, mine, defs, root, repo)
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
