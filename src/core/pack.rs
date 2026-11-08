use std::path::Path;
use std::path::PathBuf;

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

// The judgments every pack answers for, each with its one-line law. A pack
// answers "held" or "never: <why>"; a rule left unanswered prints as open.
pub const RULES: &[(&str, &str)] = &[
    (
        "reexport",
        "a module offers only its own names; a surface names each symbol it offers and defines none",
    ),
    ("self-entry", "a unit's own code imports from the defining file"),
    (
        "interior-import",
        "a unit is entered through the routes its manifest declares",
    ),
    ("hidden-name", "a symbol keeps its own name"),
    (
        "crowded-import",
        "an import line names one symbol and the module it comes from",
    ),
    (
        "comment-overreach",
        "a comment names only what its file defines, uses or quotes",
    ),
    (
        "undeclared-dependency",
        "every third party an import names is declared by the importing unit's manifest; a development table carries only what tests import",
    ),
    (
        "unused-dependency",
        "a third party a manifest declares and nothing reaches is named for deletion",
    ),
    (
        "stale-tool",
        "a tool the user names in units.toml is reached by invocation imports cannot see; one its manifest no longer declares is named, so the list cannot rot",
    ),
    (
        "undeclared-protocol",
        "a class standing where a protocol is asked names it among its bases",
    ),
    (
        "unset-dial",
        "a dial the bar requires is present in the unit's config",
    ),
    (
        "lowered-dial",
        "a dial sits at or above the pack's floor; a setting may add, never lower",
    ),
    (
        "off-channel-dependency",
        "a dependency comes from the locked registry or this repository; git, url and absolute paths slip the lock",
    ),
    (
        "untested-branch",
        "a unit's tests reach every branch of its source",
    ),
    (
        "phantom-import",
        "a generated import resolves to the contract it was generated from; one naming no contract is a finding",
    ),
    (
        "stale-generated",
        "generated code matches what its contracts generate; missing, differing and orphaned output is named",
    ),
    (
        "inter-language-break",
        "an inter-language contract breaks its signed image only with the user's signature",
    ),
    (
        "unpinned-image",
        "a container image is named by digest, never by a tag alone",
    ),
    (
        "stray-test",
        "every test names itself with a literal sentence; one built at run time cannot be heard",
    ),
    (
        "tautology",
        "an assertion compares the system to an expectation; a value equal to itself asserts nothing",
    ),
];

// The judgments the core makes through the one resolved graph, the same law for
// every language; a pack only teaches the core to read its ecosystem.
pub const CORE: &[(&str, &str)] = &[
    (
        "dead-symbol",
        "a public symbol no user reaches is named for deletion",
    ),
    (
        "tests-only-symbol",
        "a test keeps nothing alive: a symbol only tests reach is scaffolding or dead",
    ),
    (
        "cycle",
        "units form an order; no two import each other, however long the loop",
    ),
    (
        "split-version",
        "a third party resolves to one version across the repository's lockfiles; a copy nested inside another third party is that party's own",
    ),
    (
        "uninvited-use",
        "a unit is used only by the units its declared visibility grants",
    ),
    (
        "stale-grant",
        "a visibility grant names a unit that uses it; one never used is deleted",
    ),
    (
        "undeclared-unit",
        "a unit declaring deps names every unit it uses",
    ),
    (
        "stale-declaration",
        "a declared unit dep is used; one never used is deleted",
    ),
    (
        "unallowed-license",
        "a written allowlist holds every third party's license; no list is no policy yet",
    ),
    (
        "blanket-suppression",
        "a suppression names the one rule it quiets; a blanket form is refused, never recorded",
    ),
    (
        "vanished-claim",
        "a claim once signed leaves only with the user's signature",
    ),
    ("unwired-claim", "every claim answers a need in the brief"),
    (
        "unanswered-need",
        "every need binds claims, or says why not: planned or manual",
    ),
    (
        "stale-binding",
        "a binding names a claim the tree speaks; a reworded claim is rebound",
    ),
    (
        "empty-case",
        "a case declares at least one need; a case with no needs proves nothing",
    ),
    (
        "unparsable-user-record",
        "a record the user wrote parses, or the break itself is the finding; a store is rebuilt, never judged",
    ),
];

// The matrix's groups: each rule sits in exactly one, and a group's name
// answers arbiter rules with every law it holds. A group is a shelf the
// matrix prints by; it is never part of a rule's name.
pub const GROUPS: &[(&str, &[&str])] = &[
    ("symbols", &["dead-symbol", "tests-only-symbol"]),
    (
        "imports",
        &[
            "reexport",
            "hidden-name",
            "crowded-import",
            "self-entry",
            "interior-import",
            "phantom-import",
        ],
    ),
    ("protocols", &["undeclared-protocol"]),
    (
        "units",
        &[
            "cycle",
            "uninvited-use",
            "stale-grant",
            "undeclared-unit",
            "stale-declaration",
        ],
    ),
    (
        "third-parties",
        &[
            "undeclared-dependency",
            "unused-dependency",
            "stale-tool",
            "off-channel-dependency",
            "split-version",
            "unallowed-license",
        ],
    ),
    (
        "tooling",
        &["unset-dial", "lowered-dial", "untested-branch"],
    ),
    ("suppressions", &["blanket-suppression"]),
    (
        "claims",
        &[
            "tautology",
            "stray-test",
            "vanished-claim",
            "unwired-claim",
            "unanswered-need",
            "stale-binding",
            "empty-case",
            "unparsable-user-record",
        ],
    ),
    ("generated", &["stale-generated", "inter-language-break"]),
    ("comments", &["comment-overreach"]),
    ("images", &["unpinned-image"]),
];

// The laws that fire through the gate's outcomes instead of tool-free
// findings: config, tool and contract stages. Each drills; none is found.
pub const GATED: &[&str] = &[
    "unset-dial",
    "lowered-dial",
    "off-channel-dependency",
    "stale-generated",
    "inter-language-break",
    "untested-branch",
];

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

// Whether arbiter rules answers the name: every finding carries one it does.
pub fn drillable(name: &str) -> bool {
    RULES.iter().chain(CORE).any(|(rule, _)| *rule == name)
}

// Whether tool-free findings can carry the name: a law outside the gated set.
pub fn findable(name: &str) -> bool {
    drillable(name) && !GATED.contains(&name)
}

// The law a name states, wherever its row lives.
pub fn law(name: &str) -> Option<&'static str> {
    RULES
        .iter()
        .chain(CORE)
        .find(|(rule, _)| *rule == name)
        .map(|(_, law)| *law)
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
