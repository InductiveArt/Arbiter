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
use crate::packs::python::reading::Reading;

// The Python language as the pack contracts ask for it: what it is, how it
// reads one file, and what it makes of its readings.
pub struct Python;

impl Language for Python {
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

    fn dials(&self, home: &Path, folder: &Path) -> Option<(Vec<Finding>, Vec<String>)> {
        Some((laws::dials(home, folder), Vec::new()))
    }

    fn shapes(&self, source: &str) -> Vec<trees::Shape> {
        trees::shapes(&language::grammar(), source)
    }

    fn roots(&self, root: &Path, repo: &Repo) -> Vec<PathBuf> {
        modules::roots(root, repo)
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
}

impl Reader for Python {
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
