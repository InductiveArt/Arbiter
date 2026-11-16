mod language;
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
use crate::core::trees;
use crate::packs::cpp::reading::Reading;

// The C++ language as the pack contracts ask for it: what it is, how it
// reads one file, and what it makes of its readings.
pub struct Cpp;

impl Language for Cpp {
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
}

impl Reader for Cpp {
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
}
