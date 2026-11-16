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
use crate::core::trees::Shape;
use crate::domain::finding::Finding;

// A language with the readings it has made, as the graph drives it: the one
// face the core holds every language by; the core knows no language.
pub trait Pack: Language {
    // Reads one file, keeping its reading; definitions come back only from a surface file.
    fn scan(&mut self, file: &Path, source: &str, surface: bool) -> Vec<Definition>;

    // Each use read so far, resolved to the definitions it reaches.
    fn resolve(&self, mine: usize, defs: &[Def], root: &Path, repo: &Repo) -> Vec<Edge>;

    // What the readings show that breaks a law of the language.
    fn integrity(&self, root: &Path, repo: &Repo) -> Vec<Finding>;

    // Every suppression the readings hold.
    fn suppressions(&self) -> Vec<Suppression>;
}

// A reader and the readings it has made so far, one per file read.
pub struct Packed<R: Reader> {
    reader: R,
    readings: Vec<R::Reading>,
}

impl<R: Reader> Packed<R> {
    pub fn new(reader: R) -> Self {
        Packed {
            reader,
            readings: Vec::new(),
        }
    }
}

impl<R: Reader> Language for Packed<R> {
    fn label(&self) -> &'static str {
        self.reader.label()
    }

    fn handles(&self, name: &str) -> bool {
        self.reader.handles(name)
    }

    fn manifest(&self) -> Option<&'static str> {
        self.reader.manifest()
    }

    fn adapter(&self) -> Option<&'static str> {
        self.reader.adapter()
    }

    fn configs(&self) -> &'static [(&'static str, &'static str)] {
        self.reader.configs()
    }

    fn stances(&self) -> &'static Stances {
        self.reader.stances()
    }

    fn mend(&self, root: &Path, repo: &Repo) -> Vec<String> {
        self.reader.mend(root, repo)
    }

    fn dials(&self, home: &Path, folder: &Path) -> Option<(Vec<Finding>, Vec<String>)> {
        self.reader.dials(home, folder)
    }

    fn axioms(&self) -> &'static [&'static str] {
        self.reader.axioms()
    }

    fn licenses(&self, root: &Path, repo: &Repo) -> Vec<(PathBuf, String, String)> {
        self.reader.licenses(root, repo)
    }

    fn lockfiles(&self) -> &'static [&'static str] {
        self.reader.lockfiles()
    }

    fn locked(&self, name: &str, text: &str) -> Vec<(String, String)> {
        self.reader.locked(name, text)
    }

    fn epoch(&self, version: &str) -> String {
        self.reader.epoch(version)
    }

    fn outputs(&self) -> &'static [&'static str] {
        self.reader.outputs()
    }

    fn generated(&self, name: &str) -> bool {
        self.reader.generated(name)
    }

    fn drift(
        &self,
        root: &Path,
        folder: &Path,
        fixing: bool,
        cold: bool,
        notes: &mut Vec<String>,
    ) -> Vec<Drift> {
        self.reader.drift(root, folder, fixing, cold, notes)
    }

    fn record(&self, root: &Path, repo: &Repo) -> Vec<String> {
        self.reader.record(root, repo)
    }

    fn shapes(&self, source: &str) -> Vec<Shape> {
        self.reader.shapes(source)
    }

    fn roots(&self, root: &Path, repo: &Repo) -> Vec<PathBuf> {
        self.reader.roots(root, repo)
    }
}

impl<R: Reader> Pack for Packed<R> {
    fn scan(&mut self, file: &Path, source: &str, surface: bool) -> Vec<Definition> {
        let Some((reading, definitions)) = self.reader.read(file, source, surface) else {
            return Vec::new();
        };
        self.readings.push(reading);
        definitions
    }

    fn resolve(&self, mine: usize, defs: &[Def], root: &Path, repo: &Repo) -> Vec<Edge> {
        self.reader.resolve(&self.readings, mine, defs, root, repo)
    }

    fn integrity(&self, root: &Path, repo: &Repo) -> Vec<Finding> {
        self.reader.laws(&self.readings, root, repo)
    }

    fn suppressions(&self) -> Vec<Suppression> {
        self.reader.suppressions(&self.readings)
    }
}
