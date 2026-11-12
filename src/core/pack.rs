use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;

pub struct Unit {
    pub path: PathBuf,
    pub test_files: HashSet<PathBuf>,
    pub root_files: HashSet<PathBuf>,
}

pub struct Repo {
    pub units: Vec<Unit>,
    pub files: Vec<PathBuf>,
    pub manifests: Vec<PathBuf>,
    pub lockfiles: Vec<PathBuf>,
    // Folders holding a PUBLISHED marker: that unit's surface serves users
    // the repository cannot see.
    pub published: HashSet<PathBuf>,
    // Folders holding their own .arbiter: other repositories, judged from
    // their own root. The scan names each one and never crosses its border.
    pub foreign: Vec<PathBuf>,
}

impl Repo {
    // The BUILD file's word when it gives one; the naming convention where no target claims the file.
    pub fn is_test(&self, unit: usize, file: &Path) -> bool {
        if self.units[unit].test_files.contains(file) {
            return true;
        }
        let stem = file
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_string())
            .unwrap_or_default();
        stem.starts_with("test_")
            || stem.ends_with("_test")
            || stem.ends_with(".test")
            || stem == "conftest"
            || file
                .components()
                .any(|part| part.as_os_str() == "tests" || part.as_os_str() == "__tests__")
    }

    pub fn is_published(&self, unit: usize) -> bool {
        self.published.contains(&self.units[unit].path)
    }

    pub fn unit_of(&self, file: &Path) -> usize {
        self.units
            .iter()
            .enumerate()
            .filter(|(_, unit)| file.starts_with(&unit.path))
            .max_by_key(|(_, unit)| unit.path.components().count())
            .map(|(index, _)| index)
            .unwrap_or(0)
    }

    // The name a declaration uses for the unit: its path, or "." at the root.
    pub fn unit_name(&self, unit: usize) -> String {
        let path = &self.units[unit].path;
        if path.as_os_str().is_empty() {
            ".".to_string()
        } else {
            path.to_string_lossy().to_string()
        }
    }
}

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

#[derive(Clone)]
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

// One executable claim about behavior: the sentence a test names itself with,
// under the suite path that frames it.
pub struct Claim {
    pub path: Vec<String>,
    pub sentence: String,
    pub line: usize,
}

// One tool-run judgment of a unit's generated code against its contracts.
pub struct Drift {
    pub check: String,
    pub problems: Vec<String>,
    pub seconds: f64,
}

// One dial finding, placed at the unit's config file.
pub fn dial(home: &Path, config: &str, name: &str, reason: String) -> Finding {
    Finding {
        check: "ecosystem-dials",
        file: home.join(config),
        line: 0,
        name: name.to_string(),
        reason,
    }
}

// A unit's service stood up: the command that starts it and the environment
// the language's layout needs.
pub struct Launch {
    pub command: Vec<String>,
    pub env: Vec<(String, String)>,
}

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

    // The way a unit's service stands up; None when the pack cannot start one.
    fn launch(&self, root: &Path, unit: &Path) -> Option<Launch> {
        let _ = (root, unit);
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

    // Holds the unit's declared environment to the pack's bar; None where no
    // bar exists yet. Settings may add, never lower: each finding names one
    // dial unset or lowered, placed at the config file home names; a note
    // tells machine trouble the tools will also hit.
    fn dials(&self, home: &Path, folder: &Path) -> Option<(Vec<Finding>, Vec<String>)> {
        let _ = (home, folder);
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
    fn shapes(&self, source: &str) -> Vec<crate::core::trees::Shape> {
        let _ = source;
        Vec::new()
    }

    // The behaviors one file's tests claim, each a sentence under its suite
    // path; a pack whose language has no test form answers nothing.
    fn claims(&self, source: &str) -> Vec<Claim> {
        let _ = source;
        Vec::new()
    }

    // Files whose symbols live by being named from outside the repository.
    fn roots(&self, root: &Path, repo: &Repo) -> Vec<PathBuf>;

    // Reads one file, keeping its uses; definitions come back only from a surface file.
    fn scan(&mut self, file: &Path, source: &str, surface: bool) -> Vec<Definition>;

    fn resolve(&self, mine: usize, defs: &[Def], root: &Path, repo: &Repo) -> Vec<Edge>;
}

// The packages the adapter itself pins and runs: a unit never declares the
// pack's toolchain, and a unit-written version would not even be the one run.
pub fn pinned(adapter: &str) -> std::collections::HashSet<String> {
    let mut provided = std::collections::HashSet::new();
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
