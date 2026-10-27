use std::path::{Path, PathBuf};

use crate::core::units::Repo;

pub struct Def {
    pub name: String,
    pub file: PathBuf,
    pub line: usize,
    pub unit: usize,
    pub pack: usize,
    // Whether the defining file itself names the symbol away from its definition line.
    pub self_used: bool,
}

pub struct Edge {
    pub def: usize,
    pub file: PathBuf,
    pub line: usize,
    pub unit: usize,
}

pub struct Finding {
    pub check: &'static str,
    pub file: PathBuf,
    // Zero where the finding has no single line, as a cycle between units has none.
    pub line: usize,
    pub name: String,
    pub reason: String,
}

impl Finding {
    pub fn integrity(file: &Path, line: usize, name: &str, reason: String) -> Self {
        Finding {
            check: "integrity",
            file: file.to_path_buf(),
            line,
            name: name.to_string(),
            reason,
        }
    }
}

pub struct Suppression {
    pub file: PathBuf,
    pub line: usize,
    // The directive as written, which with the file is the entry's identity:
    // a line number moves with every edit above it, so it never identifies.
    pub text: String,
    // Whether it quiets more than one named rule, which is refused, never recorded.
    pub blanket: bool,
}

pub struct Definition {
    pub name: String,
    pub line: usize,
    pub self_used: bool,
}

// One tool-run judgment of a unit's generated code against its contracts.
pub struct Drift {
    pub check: String,
    pub problems: Vec<String>,
    pub seconds: f64,
}

// The judgments every pack answers for, each with its one-line law. A pack
// answers "held" or "never: <why>"; a rule left unanswered prints as open.
pub const RULES: &[(&str, &str)] = &[
    ("resolve", "an import reaches the file its module path names"),
    (
        "dynamic-import",
        "a run-time import of a literal module keeps its whole surface; a computed one is a finding",
    ),
    (
        "no-reexport",
        "a module offers only its own names; a surface names each symbol it offers",
    ),
    ("self-entry", "a unit's own code imports from the defining file"),
    (
        "interior-import",
        "a unit is entered through the routes its manifest declares",
    ),
    ("no-hidden-names", "a symbol keeps its own name"),
    (
        "one-import-per-line",
        "an import line names one symbol and the module it comes from",
    ),
    (
        "no-comment-overreach",
        "a comment names only what its file defines, uses or quotes",
    ),
    (
        "undeclared-dependency",
        "every third party an import names is declared by a manifest",
    ),
    (
        "suppressions",
        "every suppression is counted against the reference; a blanket form is a finding",
    ),
    ("wiring", "a unit's declared environment meets the pack's bar"),
    ("tools", "lint, types and tests run pinned by the pack"),
    (
        "no-warnings",
        "a tool's warning is a finding, never a soft state",
    ),
    (
        "container",
        "a members-only manifest bounds a unit without being judged",
    ),
    ("generated", "generated code is scanned, never judged"),
    (
        "drift",
        "generated code matches its contract, and a contract break is a finding",
    ),
];

// A language. It says what defines a symbol and what uses one; the core knows no language.
pub trait Pack {
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

    // A manifest declaring members and no code of its own bounds a unit without being one to judge.
    fn container(&self, manifest: &str) -> bool {
        let _ = manifest;
        false
    }

    // This pack's answer to each named rule: "held", or "never: <why>". A rule
    // left unanswered prints as open, never as a pass.
    fn rules(&self) -> &'static [(&'static str, &'static str)] {
        &[]
    }

    // Repairs the pack's own findings in place before the tools run; each note
    // names one file rewritten. Only a fixing run calls it.
    fn mend(&self, root: &Path, repo: &Repo) -> Vec<String> {
        let _ = (root, repo);
        Vec::new()
    }

    // Holds the unit's declared environment to the pack's bar; None where no bar exists yet.
    // Settings may add, never lower: each finding names one option sitting below the bar.
    fn wiring(&self, folder: &Path) -> Option<Vec<String>> {
        let _ = folder;
        None
    }

    // What the pack saw while scanning that breaks a structural rule of its language.
    fn integrity(&self, root: &Path, repo: &Repo) -> Vec<Finding> {
        let _ = (root, repo);
        Vec::new()
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

    // The lockfile recording what the ecosystem resolved, for holding versions to one.
    fn lockfile(&self) -> Option<&'static str> {
        None
    }

    // The (name, version) pairs the pack's lockfile records; TOML [[package]]
    // entries are the common shape, so they are the default reading.
    fn locked(&self, text: &str) -> Vec<(String, String)> {
        toml_locked(text)
    }

    // Every suppression the pack's language can write, seen while scanning.
    fn suppressions(&self) -> &[Suppression] {
        &[]
    }

    // Folders holding only the ecosystem's build output, which are never read.
    fn outputs(&self) -> &'static [&'static str] {
        &[]
    }

    // Whether a file of this name is generated: scanned for its uses, never judged,
    // since its contract's surface is the file it was generated from.
    fn generated(&self, name: &str) -> bool {
        let _ = name;
        false
    }

    // Judges the unit's generated code against its contracts with the unit's own
    // tools; a fixing run repairs in place first. A unit the pack has nothing to
    // judge returns nothing.
    fn drift(
        &self,
        root: &Path,
        folder: &Path,
        fixing: bool,
        notes: &mut Vec<String>,
    ) -> Vec<Drift> {
        let _ = (root, folder, fixing, notes);
        Vec::new()
    }

    // Writes the pack's accepted reference, as `arbiter accept` asks; each line
    // reports one record written or refused.
    fn record(&self, root: &Path, repo: &Repo) -> Vec<String> {
        let _ = (root, repo);
        Vec::new()
    }

    // Files whose symbols live by being named from outside the repository.
    fn roots(&self, root: &Path, repo: &Repo) -> Vec<PathBuf>;

    // Reads one file, keeping its uses; definitions come back only from a surface file.
    fn scan(&mut self, file: &Path, source: &str, surface: bool) -> Vec<Definition>;

    fn resolve(&self, mine: usize, defs: &[Def], root: &Path, repo: &Repo) -> Vec<Edge>;
}

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
