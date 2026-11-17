use std::path::Path;

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::core::definition::Def;
use crate::core::definition::Definition;
use crate::core::edge::Edge;
use crate::core::language::Language;
use crate::core::repository::Repo;
use crate::core::suppression::Suppression;
use crate::domain::finding::Finding;

// How a language reads one file into a reading of its own, and what it makes
// of its readings: the edges their uses resolve to, the laws they break, and
// the suppressions they hold.
pub trait Reader: Language {
    // Written out whole, so readings can travel to another process.
    type Reading: Serialize + DeserializeOwned;

    // One file read whole: its reading, and the definitions it offers when it
    // is a surface. A file the language cannot read leaves nothing.
    fn read(
        &self,
        file: &Path,
        source: &str,
        surface: bool,
    ) -> Option<(Self::Reading, Vec<Definition>)>;

    // Each use the readings hold, resolved to the definitions it reaches.
    fn resolve(
        &self,
        readings: &[Self::Reading],
        mine: usize,
        defs: &[Def],
        root: &Path,
        repo: &Repo,
    ) -> Vec<Edge>;

    // What the readings show that breaks a law of the language.
    fn laws(&self, readings: &[Self::Reading], root: &Path, repo: &Repo) -> Vec<Finding> {
        let _ = (readings, root, repo);
        Vec::new()
    }

    // Every suppression the readings hold.
    fn suppressions(&self, readings: &[Self::Reading]) -> Vec<Suppression> {
        let _ = readings;
        Vec::new()
    }
}
