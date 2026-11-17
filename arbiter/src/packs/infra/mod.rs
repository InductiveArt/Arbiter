mod language;
mod laws;
mod read;
mod reading;

use std::path::Path;
use std::path::PathBuf;

use crate::core::definition::Def;
use crate::core::definition::Definition;
use crate::core::edge::Edge;
use crate::core::language::Language;
use crate::core::reader::Reader;
use crate::core::repository::Repo;
use crate::core::stance::Stances;
use crate::domain::finding::Finding;
use crate::packs::infra::reading::Reading;

// Deployment manifests as checked text: the pack defines no symbols and runs
// no tools; it holds the written state to the laws text alone can prove.
pub struct Infra;

impl Language for Infra {
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
}

impl Reader for Infra {
    type Reading = Reading;

    fn read(&self, file: &Path, source: &str, surface: bool) -> Option<(Reading, Vec<Definition>)> {
        read::read(file, source, surface).map(|reading| (reading, Vec::new()))
    }

    fn resolve(
        &self,
        _readings: &[Reading],
        _mine: usize,
        _defs: &[Def],
        _root: &Path,
        _repo: &Repo,
    ) -> Vec<Edge> {
        Vec::new()
    }

    fn laws(&self, readings: &[Reading], _root: &Path, _repo: &Repo) -> Vec<Finding> {
        laws::instances(readings)
    }
}
