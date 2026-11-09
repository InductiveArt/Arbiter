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
        "stale-suppression",
        "a recorded suppression quiets one finding this run, or is named: one that suppresses nothing is stale",
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
        "unit-cycle",
        "units form an order; no two import each other, however long the loop",
    ),
    (
        "split-version",
        "a third party resolves to one version across the repository's lockfiles; a copy nested inside another third party is that party's own",
    ),
    (
        "uninvited-unit-use",
        "a unit is used only by the units its declared visibility grants",
    ),
    (
        "stale-unit-grant",
        "a visibility grant names a unit that uses it; one never used is deleted",
    ),
    (
        "undeclared-unit-use",
        "a unit declaring deps names every unit it uses",
    ),
    (
        "stale-unit-declaration",
        "a declared unit dep is used; one never used is deleted",
    ),
    (
        "uninvited-folder-use",
        "a declared folder is entered only by what its visibility grants",
    ),
    (
        "stale-folder-grant",
        "a folder's grant names a user it has; one never used is deleted",
    ),
    (
        "undeclared-folder-use",
        "a declared folder's deps name everything it uses, inside its unit or out",
    ),
    (
        "stale-folder-declaration",
        "a declared folder dep is used; one never used is deleted",
    ),
    (
        "sibling-cycle",
        "every folder's children form an order; two siblings reaching each other are one concept split, or a representation unfinished",
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
        "misplaced-case",
        "a placed case names a unit this repository builds",
    ),
    (
        "orphan-case",
        "a case serving a need serves one another case states",
    ),
    (
        "unmandated-trial",
        "a trial proves needs its case states; one proving nothing does not sit",
    ),
    (
        "stale-mandate",
        "every sentence a trial proves is a need of its case",
    ),
    (
        "off-plan-step",
        "a flow walks only edges the plan declares: a door served, a channel dialed, a volume mounted",
    ),
    (
        "astray-fork",
        "a flow forks at a service its steps visit",
    ),
    (
        "phantom-service",
        "every name a plan record dials - a need, a channel, a placement - is a declared service",
    ),
    (
        "boot-cycle",
        "a plan's needs form an order; a cycle in bring-up is a plan that cannot boot",
    ),
    (
        "undeclared-volume",
        "a volume a service mounts is declared by the record",
    ),
    (
        "unbacked-volume",
        "a declared volume names its backup policy; state is never implicit",
    ),
    (
        "unplanned-service",
        "once a plan is written, every service is placed by one",
    ),
    (
        "overcommitted-machine",
        "a plan's placements fit the machine's declared capacity",
    ),
    (
        "uninvited-channel",
        "a channel reaches only services whose accepts grant the caller; an absent grant binds nothing",
    ),
    (
        "stale-channel-grant",
        "an accepts entry names a caller that dials it; one never dialed is deleted",
    ),
    (
        "unbuilt-artifact",
        "a service naming its unit names one this repository builds",
    ),
    (
        "unparsable-user-record",
        "a record the user wrote parses, or the break itself is the finding; a store is rebuilt, never judged",
    ),
];

// Where each law detains its power: the facts alone, or a record the user
// wrote - a record-powered law waits until its record exists.
pub enum Power {
    Facts,
    Record(&'static str),
}

const RECORDED: &[(&str, &str)] = &[
    ("uninvited-unit-use", ".arbiter/units.toml"),
    ("stale-unit-grant", ".arbiter/units.toml"),
    ("undeclared-unit-use", ".arbiter/units.toml"),
    ("stale-unit-declaration", ".arbiter/units.toml"),
    ("uninvited-folder-use", ".arbiter/units.toml"),
    ("stale-folder-grant", ".arbiter/units.toml"),
    ("undeclared-folder-use", ".arbiter/units.toml"),
    ("stale-folder-declaration", ".arbiter/units.toml"),
    ("unallowed-license", ".arbiter/licenses.toml"),
    ("blanket-suppression", ".arbiter/suppressions.toml"),
    ("stale-suppression", ".arbiter/suppressions.toml"),
    ("unwired-claim", ".arbiter/brief.toml"),
    ("unanswered-need", ".arbiter/brief.toml"),
    ("stale-binding", ".arbiter/brief.toml"),
    ("empty-case", ".arbiter/brief.toml"),
    ("misplaced-case", ".arbiter/brief.toml"),
    ("orphan-case", ".arbiter/brief.toml"),
    ("unmandated-trial", ".arbiter/brief.toml"),
    ("stale-mandate", ".arbiter/brief.toml"),
    ("off-plan-step", ".arbiter/brief.toml"),
    ("astray-fork", ".arbiter/brief.toml"),
    ("vanished-claim", ".arbiter/reference.json"),
    ("phantom-service", ".arbiter/plans.toml"),
    ("boot-cycle", ".arbiter/plans.toml"),
    ("undeclared-volume", ".arbiter/plans.toml"),
    ("unbacked-volume", ".arbiter/plans.toml"),
    ("unplanned-service", ".arbiter/plans.toml"),
    ("overcommitted-machine", ".arbiter/plans.toml"),
    ("uninvited-channel", ".arbiter/plans.toml"),
    ("stale-channel-grant", ".arbiter/plans.toml"),
    ("unbuilt-artifact", ".arbiter/plans.toml"),
];

pub fn power(rule: &str) -> Power {
    RECORDED
        .iter()
        .find(|(name, _)| *name == rule)
        .map_or(Power::Facts, |(_, file)| Power::Record(file))
}

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
        "plans",
        &[
            "phantom-service",
            "boot-cycle",
            "undeclared-volume",
            "unbacked-volume",
            "unplanned-service",
            "overcommitted-machine",
            "uninvited-channel",
            "stale-channel-grant",
            "unbuilt-artifact",
        ],
    ),
    (
        "folders",
        &[
            "uninvited-folder-use",
            "stale-folder-grant",
            "undeclared-folder-use",
            "stale-folder-declaration",
            "sibling-cycle",
        ],
    ),
    (
        "units",
        &[
            "unit-cycle",
            "uninvited-unit-use",
            "stale-unit-grant",
            "undeclared-unit-use",
            "stale-unit-declaration",
        ],
    ),
    (
        "third-parties",
        &[
            "undeclared-dependency",
            "unused-dependency",
            "off-channel-dependency",
            "split-version",
            "unallowed-license",
        ],
    ),
    (
        "tooling",
        &["unset-dial", "lowered-dial", "untested-branch"],
    ),
    (
        "suppressions",
        &["blanket-suppression", "stale-suppression"],
    ),
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
            "misplaced-case",
            "orphan-case",
            "unmandated-trial",
            "stale-mandate",
            "off-plan-step",
            "astray-fork",
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
