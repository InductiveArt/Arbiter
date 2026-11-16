use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;

use crate::core::drift::Drift;
use crate::core::repository::Repo;
use crate::core::stance::Stances;
use crate::core::trees::Shape;
use crate::domain::finding::Finding;

// What a language is and does, with no state: the files it handles, the
// manifest and lockfiles of its ecosystem, its stance on each rule, and the
// work it does on a repository without reading it into the graph.
pub trait Language {
    fn label(&self) -> &'static str;

    fn handles(&self, name: &str) -> bool;

    // The manifest file marking a folder as a unit of this language.
    fn manifest(&self) -> Option<&'static str> {
        None
    }

    // The adapter: the tools that hold a unit of this language to the bar, as TOML.
    fn adapter(&self) -> Option<&'static str> {
        None
    }

    // Configuration files the adapter's tools read, carried by the pack so the bar is
    // the pack's, never a tool's default. A command reaches them where {pack} says.
    fn configs(&self) -> &'static [(&'static str, &'static str)] {
        &[]
    }

    // This pack's stance on every rule a pack holds or declines.
    fn stances(&self) -> &'static Stances;

    // Repairs the pack's own findings in place before the tools run; each note
    // names one file rewritten. Only a fixing run calls it.
    fn mend(&self, root: &Path, repo: &Repo) -> Vec<String> {
        let _ = (root, repo);
        Vec::new()
    }

    // Holds the unit's declared environment to the pack's bar; None where no
    // bar exists yet. Settings may add, never lower: each finding names one
    // dial unset or lowered, placed at the config file home names; a note
    // tells machine trouble the tools will also hit.
    fn dials(&self, home: &Path, folder: &Path) -> Option<(Vec<Finding>, Vec<String>)> {
        let _ = (home, folder);
        None
    }

    // The machine's own executables the pack's checks stand on; a missing one is
    // a broken machine, never a finding.
    fn axioms(&self) -> &'static [&'static str] {
        &[]
    }

    // Each third party's license, read from what the ecosystem materialized, as
    // (place, package, license); a pack with nothing readable answers nothing.
    fn licenses(&self, root: &Path, repo: &Repo) -> Vec<(PathBuf, String, String)> {
        let _ = (root, repo);
        Vec::new()
    }

    // The lockfiles recording what the ecosystem resolved, for holding versions
    // to one; an ecosystem with several lock formats names each.
    fn lockfiles(&self) -> &'static [&'static str] {
        &[]
    }

    // The (name, version) pairs one of the pack's lockfiles records; TOML
    // [[package]] entries are the common shape, so they are the default reading.
    fn locked(&self, name: &str, text: &str) -> Vec<(String, String)> {
        let _ = name;
        toml_locked(text)
    }

    // The epoch a locked version belongs to: versions of one epoch are one
    // third party and converge, and two epochs are two third parties the
    // ecosystem builds side by side. One epoch holds every version unless the
    // ecosystem says otherwise.
    fn epoch(&self, version: &str) -> String {
        let _ = version;
        String::new()
    }

    // Folders holding only the ecosystem's build output, which are never read.
    fn outputs(&self) -> &'static [&'static str] {
        &[]
    }

    // Whether a file of this name is generated: scanned for its uses, holding no finding,
    // since its contract's surface is the file it was generated from.
    fn generated(&self, name: &str) -> bool {
        let _ = name;
        false
    }

    // Compares the unit's generated code with its contracts using the unit's own
    // tools; a fixing run repairs in place first. A unit the pack has nothing to
    // compare returns nothing.
    fn drift(
        &self,
        root: &Path,
        folder: &Path,
        fixing: bool,
        cold: bool,
        notes: &mut Vec<String>,
    ) -> Vec<Drift> {
        let _ = (root, folder, fixing, cold, notes);
        Vec::new()
    }

    // Writes the pack's signed reference, as `arbiter ledger sign` asks; each line
    // reports one record written or refused.
    fn record(&self, root: &Path, repo: &Repo) -> Vec<String> {
        let _ = (root, repo);
        Vec::new()
    }

    // The function-grain shapes of one file, feeding the rhyme measure; a pack
    // with no function grammar answers nothing.
    fn shapes(&self, source: &str) -> Vec<Shape> {
        let _ = source;
        Vec::new()
    }

    // Files whose symbols live by being named from outside the repository.
    fn roots(&self, root: &Path, repo: &Repo) -> Vec<PathBuf>;
}

// A source a generator wrote, marked as rustfmt reads the mark: @generated
// in one of its first five lines.
pub fn marked_generated(source: &str) -> bool {
    source
        .lines()
        .take(5)
        .any(|line| line.contains("@generated"))
}

// The packages the adapter itself pins and runs: a unit never declares the
// pack's toolchain, and a unit-written version would not even be the one run.
pub fn pinned(adapter: &str) -> HashSet<String> {
    let mut provided = HashSet::new();
    let tokens = adapter.split(|c: char| c.is_whitespace() || matches!(c, '"' | ',' | '[' | ']'));
    for raw in tokens {
        let token = raw.trim_start_matches("--package=");
        let (name, version) = match token.split_once("==") {
            Some(parts) => parts,
            None => match token.rfind('@') {
                Some(at) if at > 0 => (&token[..at], &token[at + 1..]),
                _ => continue,
            },
        };
        if !name.is_empty() && version.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            provided.insert(name.to_string());
        }
    }
    provided
}

// The (name, version) pairs of a lockfile's TOML [[package]] entries.
pub fn toml_locked(text: &str) -> Vec<(String, String)> {
    let Ok(parsed) = text.parse::<toml::Value>() else {
        return Vec::new();
    };
    let Some(packages) = parsed.get("package").and_then(toml::Value::as_array) else {
        return Vec::new();
    };
    packages
        .iter()
        .filter_map(|entry| {
            let name = entry.get("name")?.as_str()?;
            let version = entry.get("version")?.as_str()?;
            Some((name.to_string(), version.to_string()))
        })
        .collect()
}
