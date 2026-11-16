use std::path::Path;
use std::path::PathBuf;

use tree_sitter::Language;
use tree_sitter::Node;
use tree_sitter::Tree;

use crate::core::pack::dial;
use crate::core::pack::marked_generated;
use crate::core::pack::Def;
use crate::core::pack::Definition;
use crate::core::pack::Edge;
use crate::core::pack::Pack;
use crate::core::pack::Repo;
use crate::core::pack::Suppression;
use crate::core::prose;
use crate::core::record;
use crate::core::trees;
use crate::domain::finding::line_at;
use crate::domain::finding::Finding;

const DEFINITIONS: &str = include_str!("../../packs/rust/definitions.scm");
const USES: &str = include_str!("../../packs/rust/uses.scm");
const MENTIONS: &str = include_str!("../../packs/rust/mentions.scm");
const SUPPRESSIONS: &str = include_str!("../../packs/rust/suppressions.scm");
const PROSE: &str = include_str!("../../packs/rust/prose.scm");
const REEXPORTS: &str = include_str!("../../packs/rust/reexports.scm");

#[derive(Default)]
pub struct Rust {
    uses: Vec<(PathBuf, String, String, usize)>,
    // The first segment of each use declaration: where an external crate enters.
    entries: Vec<(PathBuf, String, usize)>,
    // Each name a use declaration binds, with the module it binds it from.
    imports: Vec<(PathBuf, String, String)>,
    // Each module a file declares, with a body or in a file of its own.
    modules: Vec<(PathBuf, String)>,
    // Each file an `include!` takes in, with the file taking it.
    includes: Vec<(PathBuf, PathBuf)>,
    // The files a generator wrote.
    generated: std::collections::HashSet<PathBuf>,
    suppressions: Vec<Suppression>,
    aliases: Vec<(PathBuf, String, usize)>,
    reexports: Vec<(PathBuf, String, usize)>,
    overreaches: Vec<(PathBuf, String, usize)>,
    tautologies: Vec<(PathBuf, usize, String)>,
    crowded: Vec<(PathBuf, usize, usize)>,
}

const BUILTIN_CRATES: [&str; 5] = ["std", "core", "alloc", "proc_macro", "test"];

impl Rust {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Pack for Rust {
    fn rules(&self) -> &'static [(&'static str, &'static str)] {
        &[
            (
                "unpinned-image",
                "never: the language names no container images",
            ),
            ("reexport", "held"),
            (
                "self-entry",
                "never: a crate reaches its own items as crate::, never through its published name",
            ),
            (
                "interior-import",
                "never: pub already walls a crate's interior",
            ),
            ("hidden-name", "held"),
            (
                "undeclared-protocol",
                "never: a trait is nominal; nothing implements invisibly",
            ),
            ("crowded-import", "held"),
            ("comment-overreach", "held"),
            ("undeclared-dependency", "held"),
            ("unused-dependency", "held"),
            ("stale-suppression", "held"),
            ("unset-dial", "held"),
            ("lowered-dial", "held"),
            ("off-channel-dependency", "held"),
            ("tautology", "held"),
            (
                "stray-test",
                "never: a test is a fn, and a fn's name is always literal",
            ),
            (
                "stale-generated",
                "never: the proto pack checks every unit's buf.gen.yaml, whatever its language",
            ),
            (
                "inter-language-break",
                "never: the proto pack checks every unit's buf.gen.yaml, whatever its language",
            ),
        ]
    }

    fn label(&self) -> &'static str {
        "rust"
    }

    fn handles(&self, name: &str) -> bool {
        name.ends_with(".rs")
    }

    fn manifest(&self) -> Option<&'static str> {
        Some("Cargo.toml")
    }

    fn adapter(&self) -> Option<&'static str> {
        Some(include_str!("../../packs/rust/ecosystem.toml"))
    }

    // A Cargo.toml declaring a workspace and no package bounds its members.
    fn container(&self, manifest: &str) -> bool {
        let Ok(parsed) = manifest.parse::<toml::Value>() else {
            return false;
        };
        parsed.get("workspace").is_some() && parsed.get("package").is_none()
    }

    fn dials(&self, home: &Path, folder: &Path) -> Option<(Vec<Finding>, Vec<String>)> {
        let Ok(manifest) = std::fs::read_to_string(folder.join("Cargo.toml")) else {
            return Some((
                vec![dial(
                    home,
                    "Cargo.toml",
                    "unset-dial",
                    "no Cargo.toml; the unit declares no environment".to_string(),
                )],
                Vec::new(),
            ));
        };
        Some((manifest_findings(&manifest, home), Vec::new()))
    }

    fn mend(&self, root: &Path, repo: &Repo) -> Vec<String> {
        let mut notes = Vec::new();
        for file in &repo.files {
            let name = file
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_default();
            if !self.handles(&name) || self.generated(&name) {
                continue;
            }
            // A unit on a test path is a fixture: data for some test, never repaired.
            let unit = repo.unit_of(file);
            let home = repo.units[unit].path.clone();
            if repo.is_test(unit, &home) {
                continue;
            }
            let path = root.join(file);
            let Ok(source) = std::fs::read_to_string(&path) else {
                continue;
            };
            if marked_generated(&source) {
                continue;
            }
            let Some(tree) = trees::parse(&language(), &source) else {
                continue;
            };
            if let Some(mended) = split_uses(tree.root_node(), &source) {
                if std::fs::write(&path, mended).is_ok() {
                    notes.push(format!("{} split to one use per line", file.display()));
                }
            }
        }
        notes
    }

    fn shapes(&self, source: &str) -> Vec<trees::Shape> {
        trees::shapes(&language(), source)
    }

    // Cargo's convention: a crate's binary entry is src/main.rs beside its manifest.
    fn roots(&self, _root: &Path, repo: &Repo) -> Vec<PathBuf> {
        let mut found = Vec::new();
        for manifest in &repo.manifests {
            if manifest.file_name().is_none_or(|name| name != "Cargo.toml") {
                continue;
            }
            let folder = manifest.parent().unwrap_or(Path::new(""));
            let entry = folder.join("src").join("main.rs");
            if repo.files.contains(&entry) {
                found.push(entry);
            }
        }
        found
    }

    fn scan(&mut self, file: &Path, source: &str, surface: bool) -> Vec<Definition> {
        let Some(tree) = trees::parse(&language(), source) else {
            return Vec::new();
        };
        for (module, name, line, from_use) in uses(&tree, source) {
            if from_use {
                let first = module.split("::").next().unwrap_or(&module).to_string();
                self.entries.push((file.to_path_buf(), first, line));
                self.imports
                    .push((file.to_path_buf(), name.clone(), module.clone()));
            }
            self.uses.push((file.to_path_buf(), module, name, line));
        }
        // A glob import binds every name its module offers, so each name the
        // file mentions is a use of that name in that module.
        let globbed = globs(tree.root_node(), source);
        if !globbed.is_empty() {
            let mut seen = std::collections::HashSet::new();
            for (name, line) in mentions(&tree, source) {
                if seen.insert(name.clone()) {
                    for module in &globbed {
                        self.uses
                            .push((file.to_path_buf(), module.clone(), name.clone(), line));
                    }
                }
            }
        }
        for name in declared_modules(tree.root_node(), source) {
            self.modules.push((file.to_path_buf(), name));
        }
        let folder = file.parent().unwrap_or(Path::new(""));
        for included in includes(tree.root_node(), source) {
            self.includes
                .push((file.to_path_buf(), folder.join(included)));
        }
        // What a generator wrote is its generator's: read for what it uses,
        // no finding lands in it, its suppressions never counted.
        if marked_generated(source) {
            self.generated.insert(file.to_path_buf());
            return Vec::new();
        }
        for captures in trees::matches(&language(), SUPPRESSIONS, &tree, source) {
            for capture in captures {
                let first = capture.text.lines().next().unwrap_or("").trim().to_string();
                let quiets = first.contains("allow(") || first.contains("expect(");
                let recorded = match capture.name.as_str() {
                    "outer" if first.contains("#[ignore") => Some(("#[ignore]".to_string(), false)),
                    "outer" if quiets => Some((first, false)),
                    "inner" if quiets => Some((first, true)),
                    "unsafe" => Some(("unsafe".to_string(), false)),
                    "modifiers" if capture.text.contains("unsafe") => {
                        Some(("unsafe fn".to_string(), false))
                    }
                    _ => None,
                };
                if let Some((text, blanket)) = recorded {
                    self.suppressions.push(Suppression {
                        file: file.to_path_buf(),
                        line: capture.line,
                        text,
                        blanket,
                    });
                }
            }
        }
        for (text, line) in tautologies(tree.root_node(), source) {
            self.tautologies.push((file.to_path_buf(), line, text));
        }
        if !surface {
            return Vec::new();
        }
        for captures in trees::matches(&language(), REEXPORTS, &tree, source) {
            for capture in captures {
                self.reexports
                    .push((file.to_path_buf(), capture.text, capture.line));
            }
        }
        for captures in trees::matches(&language(), USES, &tree, source) {
            for capture in captures {
                if capture.name != "alias" {
                    continue;
                }
                let alias = capture
                    .text
                    .rsplit(" as ")
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string();
                // Importing a trait as _ brings its methods without any name to hide.
                if alias != "_" {
                    self.aliases.push((file.to_path_buf(), alias, capture.line));
                }
            }
        }
        for (line, names) in crowded_uses(tree.root_node()) {
            self.crowded.push((file.to_path_buf(), line, names));
        }
        let mentions = mentions(&tree, source);
        let idents: std::collections::HashSet<String> =
            mentions.iter().map(|(name, _)| name.clone()).collect();
        let (comments, quoted) = trees::prose_parts(&language(), PROSE, &tree, source);
        for (token, line) in prose::overreaches(&comments, &idents, &quoted) {
            self.overreaches.push((file.to_path_buf(), token, line));
        }
        definitions(&tree, source)
            .into_iter()
            .map(|(name, line)| Definition {
                self_used: mentions
                    .iter()
                    .any(|(mentioned, at)| *mentioned == name && *at != line),
                name,
                line,
            })
            .collect()
    }

    fn suppressions(&self) -> &[Suppression] {
        &self.suppressions
    }

    fn outputs(&self) -> &'static [&'static str] {
        &["target"]
    }

    fn axioms(&self) -> &'static [&'static str] {
        &["cargo"]
    }

    fn lockfiles(&self) -> &'static [&'static str] {
        &["Cargo.lock"]
    }

    // Cargo's compatibility: a version's parts up to its first nonzero one
    // name its epoch, so 2.0.119 is 2 and 0.14.6 is 0.14.
    fn epoch(&self, version: &str) -> String {
        let release = version.split(['-', '+']).next().unwrap_or(version);
        let parts: Vec<&str> = release.split('.').collect();
        let kept = parts
            .iter()
            .position(|part| *part != "0")
            .map_or(parts.len(), |at| at + 1);
        parts[..kept].join(".")
    }

    fn integrity(&self, root: &Path, repo: &Repo) -> Vec<Finding> {
        let mut findings: Vec<Finding> = self
            .reexports
            .iter()
            .map(|(file, argument, line)| Finding {
                check: "integrity".to_string(),
                file: file.clone(),
                line: line_at(*line),
                rule: Some("reexport".to_string()),
                reason: format!("re-exports {argument}; a module offers only its own names"),
            })
            .collect();
        // A fixture's tautologies are some test's data, never findings.
        let fixture = |file: &PathBuf| {
            let unit = repo.unit_of(file);
            repo.is_test(unit, &repo.units[unit].path.clone())
        };
        findings.extend(
            self.tautologies
                .iter()
                .filter(|(file, _, _)| !fixture(file))
                .map(|(file, line, text)| Finding {
                    check: "integrity".to_string(),
                    file: file.clone(),
                    line: line_at(*line),
                    rule: Some("tautology".to_string()),
                    reason: format!("{text}; an assertion compares the system to an expectation"),
                }),
        );
        findings.extend(self.crowded.iter().map(|(file, line, names)| Finding {
            check: "integrity".to_string(),
            file: file.clone(),
            line: line_at(*line),
            rule: Some("crowded-import".to_string()),
            reason: format!(
                "names {names} symbols in one import; \
                 an import line names one symbol and the module it comes from"
            ),
        }));
        findings.extend(self.aliases.iter().map(|(file, alias, line)| Finding {
            check: "integrity".to_string(),
            file: file.clone(),
            line: line_at(*line),
            rule: Some("hidden-name".to_string()),
            reason: format!("imported as {alias}; a symbol keeps its own name"),
        }));
        findings.extend(self.overreaches.iter().map(|(file, token, line)| Finding {
            check: "integrity".to_string(),
            file: file.clone(),
            line: line_at(*line),
            rule: Some("comment-overreach".to_string()),
            reason: format!(
                "a comment names {token}, which nothing in the file defines, uses or quotes"
            ),
        }));
        let declared = declared_crates(root, repo);
        // A module the file declares, or a name it imports, is no crate.
        let local: std::collections::HashSet<(&Path, &str)> = self
            .modules
            .iter()
            .map(|(file, name)| (file.as_path(), name.as_str()))
            .chain(
                self.imports
                    .iter()
                    .filter(|(_, name, from)| binds(name, from))
                    .map(|(file, name, _)| (file.as_path(), name.as_str())),
            )
            .collect();
        let mut named = std::collections::BTreeSet::new();
        for (file, first, line) in &self.entries {
            let unit = repo.unit_of(file);
            let home = repo.units[unit].path.clone();
            if repo.is_test(unit, &home) {
                continue;
            }
            if first == "crate"
                || first == "self"
                || first == "super"
                || BUILTIN_CRATES.contains(&first.as_str())
            {
                continue;
            }
            if local.contains(&(file.as_path(), first.as_str())) {
                continue;
            }
            let dashed = first.replace('_', "-");
            // A crate's own name is reached from its binaries as the library half.
            if declared.get(&unit).is_some_and(|entry| {
                entry.names.contains(&dashed) || entry.own.as_deref() == Some(dashed.as_str())
            }) {
                continue;
            }
            if named.insert((unit, first.clone())) {
                findings.push(Finding {
                    check: "integrity".to_string(),
                    file: file.clone(),
                    line: line_at(*line),
                    rule: Some("undeclared-dependency".to_string()),
                    reason: format!(
                        "{first} is imported, and the unit's manifest does not declare it"
                    ),
                });
            }
        }
        // The mirror: a crate a manifest declares and no path of the unit
        // reaches is pulled and trusted for nothing. A use statement and a
        // qualified path both reach.
        let mut reached: std::collections::HashMap<usize, std::collections::HashSet<String>> =
            std::collections::HashMap::new();
        for (file, module, _, _) in &self.uses {
            let first = module.split("::").next().unwrap_or(module);
            reached
                .entry(repo.unit_of(file))
                .or_default()
                .insert(first.replace('_', "-"));
        }
        for (file, first, _) in &self.entries {
            reached
                .entry(repo.unit_of(file))
                .or_default()
                .insert(first.replace('_', "-"));
        }
        let mut coded: std::collections::HashSet<usize> = std::collections::HashSet::new();
        for file in &repo.files {
            let name = file
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_default();
            if self.handles(&name) {
                coded.insert(repo.unit_of(file));
            }
        }
        let (subjects, _) = record::read(root);
        let mut live: std::collections::HashSet<usize> = Default::default();
        for (unit, entry) in &declared {
            let home = repo.units[*unit].path.clone();
            if repo.is_test(*unit, &home) || !coded.contains(unit) {
                continue;
            }
            let unit_name = repo.unit_name(*unit);
            let seen = reached.get(unit);
            for name in entry
                .names
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
            {
                if Some(name.as_str()) == entry.own.as_deref() {
                    continue;
                }
                if seen.is_some_and(|set| set.contains(name)) {
                    continue;
                }
                if let Some(at) = record::quiets(&subjects, "rust", &unit_name, name) {
                    live.insert(at);
                    continue;
                }
                findings.push(Finding {
                    check: "integrity".to_string(),
                    file: entry.manifest.clone(),
                    line: None,
                    rule: Some("unused-dependency".to_string()),
                    reason: format!("{name} is declared, and nothing in the unit reaches it"),
                });
            }
        }
        findings.extend(record::stale(&subjects, "rust", &live));
        findings
    }

    fn resolve(&self, mine: usize, defs: &[Def], root: &Path, repo: &Repo) -> Vec<Edge> {
        // Each crate of the repository by the name its paths use, at its unit.
        // The crates each unit reaches inside this repository by name: its
        // own, and each its manifest takes by path. A crate taken from a
        // registry is a release, compiled from no source here.
        let declared = declared_crates(root, repo);
        let homes: std::collections::HashMap<&str, &PathBuf> = declared
            .iter()
            .filter_map(|(unit, entry)| Some((entry.own.as_deref()?, &repo.units[*unit].path)))
            .collect();
        let reached: std::collections::HashMap<usize, std::collections::HashMap<String, PathBuf>> =
            declared
                .iter()
                .map(|(unit, entry)| {
                    let named = entry.own.iter().chain(&entry.paths).filter_map(|name| {
                        Some((name.clone(), homes.get(name.as_str())?.to_path_buf()))
                    });
                    (*unit, named.collect())
                })
                .collect();
        let unreached = std::collections::HashMap::new();
        let mut defined: std::collections::HashMap<(&Path, &str), Vec<usize>> =
            std::collections::HashMap::new();
        for (index, def) in defs.iter().enumerate() {
            if def.pack == mine {
                defined
                    .entry((def.file.as_path(), def.name.as_str()))
                    .or_default()
                    .push(index);
            }
        }
        // A module taking in what a generator wrote answers for the contract it
        // was generated from, as a generated module does in every language:
        // the generated file is named for the contract's package, and the
        // package's definitions sit in the folder its name spells.
        let contracts: std::collections::HashMap<&Path, PathBuf> = self
            .includes
            .iter()
            .filter(|(taking, taken)| {
                self.generated.contains(taken) && !self.generated.contains(taking)
            })
            .filter_map(|(taking, taken)| {
                let named = taken.file_name()?.to_string_lossy();
                let package = named.strip_suffix(".rs")?;
                Some((taking.as_path(), package.split('.').collect()))
            })
            .collect();
        let mut contracted: std::collections::HashMap<&str, Vec<usize>> =
            std::collections::HashMap::new();
        for (index, def) in defs.iter().enumerate() {
            if def
                .file
                .extension()
                .is_some_and(|extension| extension == "proto")
            {
                contracted.entry(def.name.as_str()).or_default().push(index);
            }
        }
        let mut imported: std::collections::HashMap<&Path, Vec<(&str, &str)>> =
            std::collections::HashMap::new();
        for (file, name, from) in &self.imports {
            imported
                .entry(file.as_path())
                .or_default()
                .push((name.as_str(), from.as_str()));
        }
        let mut edges = Vec::new();
        for (file, module, name, line) in &self.uses {
            let unit = repo.unit_of(file);
            let imports = imported.get(file.as_path()).map_or(&[][..], Vec::as_slice);
            let crates = reached.get(&unit).unwrap_or(&unreached);
            for candidate in module_files(file, module, &repo.units[unit].path, imports, crates) {
                let own = defined
                    .get(&(candidate.as_path(), name.as_str()))
                    .into_iter()
                    .flatten()
                    .copied();
                let contract = contracts
                    .get(candidate.as_path())
                    .into_iter()
                    .flat_map(|package| {
                        contracted
                            .get(name.as_str())
                            .into_iter()
                            .flatten()
                            .copied()
                            .filter(move |def| {
                                defs[*def]
                                    .file
                                    .parent()
                                    .is_some_and(|folder| folder.ends_with(package))
                            })
                    });
                for def in own.chain(contract) {
                    edges.push(Edge {
                        def,
                        file: file.clone(),
                        line: *line,
                        unit,
                    });
                }
            }
        }
        edges
    }
}

// Each unit's manifest with its dependency-table keys, dashed, which is how
// crates are published, and those of them it takes by path. Cargo already
// refuses a dev-dependency outside tests, so the tables stay one set; the
// crate's own name stays apart.
struct Crates {
    manifest: PathBuf,
    own: Option<String>,
    names: std::collections::HashSet<String>,
    paths: std::collections::HashSet<String>,
}

fn declared_crates(root: &Path, repo: &Repo) -> std::collections::HashMap<usize, Crates> {
    let mut declared: std::collections::HashMap<usize, Crates> = std::collections::HashMap::new();
    for manifest in &repo.manifests {
        if manifest.file_name().is_none_or(|name| name != "Cargo.toml") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(root.join(manifest)) else {
            continue;
        };
        let Ok(parsed) = text.parse::<toml::Value>() else {
            continue;
        };
        let entry = declared
            .entry(repo.unit_of(manifest))
            .or_insert_with(|| Crates {
                manifest: manifest.clone(),
                own: None,
                names: std::collections::HashSet::new(),
                paths: std::collections::HashSet::new(),
            });
        if let Some(own) = table_at(&parsed, "package").and_then(|package| package.get("name")) {
            if let Some(own) = own.as_str() {
                entry.own = Some(own.replace('_', "-"));
            }
        }
        for path in [
            "dependencies",
            "dev-dependencies",
            "build-dependencies",
            "workspace.dependencies",
        ] {
            let Some(table) = table_at(&parsed, path) else {
                continue;
            };
            for (name, spec) in table {
                entry.names.insert(name.replace('_', "-"));
                if spec.get("path").is_some() {
                    entry.paths.insert(name.replace('_', "-"));
                }
            }
        }
    }
    declared
}

// The bar denies every warning, so a lint turned to allow in the manifest lowers it.
fn manifest_findings(manifest: &str, home: &Path) -> Vec<Finding> {
    let Ok(parsed) = manifest.parse::<toml::Value>() else {
        return vec![dial(
            home,
            "Cargo.toml",
            "unset-dial",
            "Cargo.toml does not parse; every dial is unreadable".to_string(),
        )];
    };
    let mut findings = Vec::new();
    for base in ["lints", "workspace.lints"] {
        for group in ["rust", "clippy"] {
            let path = format!("{base}.{group}");
            let Some(table) = table_at(&parsed, &path) else {
                continue;
            };
            for (name, level) in table {
                if allows(level) {
                    findings.push(dial(
                        home,
                        "Cargo.toml",
                        "lowered-dial",
                        format!("{path}.{name} is allow; the bar denies every warning"),
                    ));
                }
            }
        }
    }
    // The registry and the repository are the only channels: a git source slips
    // past the lockfile's hashes and past the split-version rule.
    for base in [
        "dependencies",
        "dev-dependencies",
        "build-dependencies",
        "workspace.dependencies",
    ] {
        let Some(table) = table_at(&parsed, base) else {
            continue;
        };
        for (name, spec) in table {
            if spec.get("git").is_some() {
                findings.push(dial(
                    home,
                    "Cargo.toml",
                    "off-channel-dependency",
                    format!(
                        "{base}.{name} comes from git; \
                         a dependency comes from the locked registry or this repository"
                    ),
                ));
            } else if spec
                .get("path")
                .and_then(toml::Value::as_str)
                .is_some_and(|path| Path::new(path).is_absolute())
            {
                findings.push(dial(
                    home,
                    "Cargo.toml",
                    "off-channel-dependency",
                    format!(
                        "{base}.{name} is an absolute path; \
                         a path dependency stays inside the repository"
                    ),
                ));
            }
        }
    }
    findings
}

fn table_at<'manifest>(
    parsed: &'manifest toml::Value,
    path: &str,
) -> Option<&'manifest toml::Table> {
    let mut value = parsed;
    for part in path.split('.') {
        value = value.get(part)?;
    }
    value.as_table()
}

fn allows(level: &toml::Value) -> bool {
    level.as_str() == Some("allow")
        || level.get("level").and_then(toml::Value::as_str) == Some("allow")
}

// The files that can define what a path's module names, read from its first
// segment as the compiler reads it: a name the file imports stands for the
// path it imports, `crate` is the file's own crate, a crate of the
// repository is that crate's library, `self` and `super` are the file's
// module and the ones above it, and any other name is a module under the
// file's own. A path naming `self` or `super` alone stays inside its file.
fn module_files(
    file: &Path,
    module: &str,
    unit: &Path,
    imports: &[(&str, &str)],
    crates: &std::collections::HashMap<String, PathBuf>,
) -> Vec<PathBuf> {
    if module == "self" || module == "super" {
        return Vec::new();
    }
    let segments = expanded(module, imports);
    let Some((first, rest)) = segments.split_first() else {
        return Vec::new();
    };
    let layout = CrateLayout::of(unit, file);
    let below = |kept: usize, tail: &[String]| -> Vec<String> {
        layout.own[..kept].iter().chain(tail).cloned().collect()
    };
    let paths = match first.as_str() {
        "crate" => vec![rest.to_vec()],
        "self" => vec![below(layout.own.len(), rest)],
        "super" => {
            let ups = segments
                .iter()
                .take_while(|segment| *segment == "super")
                .count();
            // Above the file's module, or above an inline module inside it.
            [ups, ups - 1]
                .iter()
                .filter_map(|up| layout.own.len().checked_sub(*up))
                .map(|kept| below(kept, &segments[ups..]))
                .collect()
        }
        name => match crates.get(&name.replace('_', "-")) {
            Some(home) => return CrateLayout::library(home).files(rest),
            None => vec![below(layout.own.len(), &segments)],
        },
    };
    paths.iter().flat_map(|path| layout.files(path)).collect()
}

// A module's segments with the first read through the file's imports.
fn expanded(module: &str, imports: &[(&str, &str)]) -> Vec<String> {
    let mut segments: Vec<String> = module.split("::").map(str::to_string).collect();
    if let Some((name, from)) = imports
        .iter()
        .find(|(name, from)| *name == segments[0] && binds(name, from))
    {
        let mut whole: Vec<String> = from.split("::").map(str::to_string).collect();
        whole.push(name.to_string());
        whole.extend(segments.drain(1..));
        segments = whole;
    }
    segments
}

// Whether a name a use declaration binds stands for the path it binds: not
// when that path starts with the name itself, as `use log::log;` does, where
// the first segment is the crate and never the binding.
fn binds(name: &str, from: &str) -> bool {
    from.split("::").next() != Some(name)
}

// The folders Cargo makes a crate of each file in.
const TARGET_FOLDERS: [&str; 3] = ["tests", "examples", "benches"];

// Where a crate finds its modules, as Cargo lays a package out: the library
// and the main binary under src, every other binary, test, example and bench
// a crate of its own; and the file it was read for, as a module path from
// that crate's root.
struct CrateLayout {
    dir: PathBuf,
    // The files that can be the crate's root module.
    roots: Vec<PathBuf>,
    own: Vec<String>,
}

impl CrateLayout {
    fn of(unit: &Path, file: &Path) -> CrateLayout {
        let inside: Vec<String> = file
            .strip_prefix(unit)
            .unwrap_or(file)
            .iter()
            .map(|part| part.to_string_lossy().into_owned())
            .collect();
        let parts: Vec<&str> = inside.iter().map(String::as_str).collect();
        match parts.as_slice() {
            ["src", "bin", _] => CrateLayout {
                dir: unit.join("src").join("bin"),
                roots: vec![file.to_path_buf()],
                own: Vec::new(),
            },
            ["src", "bin", binary, rest @ ..] => {
                let dir = unit.join("src").join("bin").join(binary);
                CrateLayout {
                    roots: vec![dir.join("main.rs")],
                    own: module_path(rest),
                    dir,
                }
            }
            ["src", rest @ ..] => {
                let dir = unit.join("src");
                CrateLayout {
                    roots: vec![dir.join("lib.rs"), dir.join("main.rs")],
                    own: module_path(rest),
                    dir,
                }
            }
            [folder, _] if TARGET_FOLDERS.contains(folder) => CrateLayout {
                dir: unit.join(folder),
                roots: vec![file.to_path_buf()],
                own: Vec::new(),
            },
            // A module the folder's crates share, as tests/common/mod.rs is.
            [folder, rest @ ..] if TARGET_FOLDERS.contains(folder) => CrateLayout {
                dir: unit.join(folder),
                roots: Vec::new(),
                own: module_path(rest),
            },
            _ => CrateLayout {
                dir: file.parent().map(Path::to_path_buf).unwrap_or_default(),
                roots: vec![file.to_path_buf()],
                own: Vec::new(),
            },
        }
    }

    // A crate of the repository as another reaches it by name: its library.
    fn library(unit: &Path) -> CrateLayout {
        let dir = unit.join("src");
        CrateLayout {
            roots: vec![dir.join("lib.rs")],
            dir,
            own: Vec::new(),
        }
    }

    // The files a module path can live in: the crate's root for no path,
    // otherwise the file its last segment names or that folder's mod.rs.
    fn files(&self, path: &[String]) -> Vec<PathBuf> {
        if path.is_empty() {
            return self.roots.clone();
        }
        let folder = path
            .iter()
            .fold(self.dir.clone(), |at, segment| at.join(segment));
        vec![folder.with_extension("rs"), folder.join("mod.rs")]
    }
}

// A file's module path from the folder its crate finds modules in: a mod.rs
// names its folder's module, and a crate's root file names none.
fn module_path(inside: &[&str]) -> Vec<String> {
    match inside {
        [] | ["lib.rs"] | ["main.rs"] => Vec::new(),
        [folders @ .., last] => {
            let mut path: Vec<String> = folders.iter().map(|folder| folder.to_string()).collect();
            if *last != "mod.rs" {
                path.push(last.trim_end_matches(".rs").to_string());
            }
            path
        }
    }
}

fn language() -> Language {
    tree_sitter_rust::LANGUAGE.into()
}

fn definitions(tree: &Tree, source: &str) -> Vec<(String, usize)> {
    trees::texts(&language(), DEFINITIONS, tree, source, "def")
}

fn mentions(tree: &Tree, source: &str) -> Vec<(String, usize)> {
    trees::texts(&language(), MENTIONS, tree, source, "mention")
}

fn uses(tree: &Tree, source: &str) -> Vec<(String, String, usize, bool)> {
    let mut found: Vec<(String, String, usize, bool)> =
        trees::matches(&language(), USES, tree, source)
            .into_iter()
            .filter_map(|captures| {
                let module = captures.iter().find(|capture| {
                    capture.name == "module" || capture.name == "declared_module"
                })?;
                let name = captures.iter().find(|capture| capture.name == "name")?;
                Some((
                    from_root(&module.text),
                    name.text.clone(),
                    name.line,
                    module.name == "declared_module",
                ))
            })
            .collect();
    found.extend(
        macro_paths(tree.root_node(), source)
            .into_iter()
            .map(|(module, name, line)| (module, name, line, false)),
    );
    found
}

// The file each `include!` of a literal path names, as written.
fn includes(node: Node, source: &str) -> Vec<String> {
    if node.kind() == "macro_invocation" {
        let named = node
            .child_by_field_name("macro")
            .is_some_and(|name| &source[name.byte_range()] == "include");
        let literal = (0..node.named_child_count())
            .filter_map(|index| node.named_child(index))
            .find(|child| child.kind() == "token_tree")
            .and_then(|tokens| tokens.named_child(0))
            .filter(|first| first.kind() == "string_literal");
        return match (named, literal) {
            (true, Some(literal)) => {
                vec![source[literal.byte_range()].trim_matches('"').to_string()]
            }
            _ => Vec::new(),
        };
    }
    (0..node.named_child_count())
        .filter_map(|index| node.named_child(index))
        .flat_map(|child| includes(child, source))
        .collect()
}

// A path from the root namespace, as `::name::item` is, names the crate
// after the colons.
fn from_root(path: &str) -> String {
    path.strip_prefix("::").unwrap_or(path).to_string()
}

// Every path written among a macro's tokens, where the parser builds no path
// node: each level of it, as a path node gives them.
fn macro_paths(node: Node, source: &str) -> Vec<(String, String, usize)> {
    if node.kind() == "token_tree" {
        let mut leaves = Vec::new();
        leaves_of(node, &mut leaves);
        return paths_among(&leaves, source);
    }
    let mut found = Vec::new();
    for index in 0..node.child_count() {
        if let Some(child) = node.child(index) {
            found.extend(macro_paths(child, source));
        }
    }
    found
}

fn leaves_of<'tree>(node: Node<'tree>, leaves: &mut Vec<Node<'tree>>) {
    if node.child_count() == 0 {
        leaves.push(node);
        return;
    }
    for index in 0..node.child_count() {
        if let Some(child) = node.child(index) {
            leaves_of(child, leaves);
        }
    }
}

// Each path in a run of tokens: segments joined by `::`, `$crate` read as
// the crate defining the macro.
fn paths_among(leaves: &[Node], source: &str) -> Vec<(String, String, usize)> {
    let mut found = Vec::new();
    let mut path: Vec<String> = Vec::new();
    let mut joined = false;
    for leaf in leaves {
        let text = &source[leaf.byte_range()];
        let segment = match leaf.kind() {
            "identifier" | "self" | "super" | "crate" => Some(text),
            "metavariable" if text == "$crate" => Some("crate"),
            _ => None,
        };
        match segment {
            Some(segment) => {
                if joined {
                    found.push((
                        path.join("::"),
                        segment.to_string(),
                        leaf.start_position().row + 1,
                    ));
                } else {
                    path.clear();
                }
                path.push(segment.to_string());
                joined = false;
            }
            None if leaf.kind() == "::" && !path.is_empty() && !joined => joined = true,
            None => {
                path.clear();
                joined = false;
            }
        }
    }
    found
}

// The module each glob import in the file takes every name from, with the
// path of any use list holding it.
fn globs(node: Node, source: &str) -> Vec<String> {
    if node.kind() == "use_wildcard" {
        let mut path: Vec<String> = node
            .named_child(0)
            .map(|own| source[own.byte_range()].to_string())
            .into_iter()
            .collect();
        let mut above = node.parent();
        while let Some(list) = above {
            if list.kind() == "scoped_use_list" {
                if let Some(prefix) = list.child_by_field_name("path") {
                    path.insert(0, source[prefix.byte_range()].to_string());
                }
            }
            above = list.parent();
        }
        if path.is_empty() {
            return Vec::new();
        }
        return vec![from_root(&path.join("::"))];
    }
    let mut found = Vec::new();
    for index in 0..node.named_child_count() {
        if let Some(child) = node.named_child(index) {
            found.extend(globs(child, source));
        }
    }
    found
}

fn declared_modules(node: Node, source: &str) -> Vec<String> {
    let mut found = Vec::new();
    if node.kind() == "mod_item" {
        if let Some(name) = node.child_by_field_name("name") {
            found.push(source[name.byte_range()].to_string());
        }
    }
    for index in 0..node.named_child_count() {
        if let Some(child) = node.named_child(index) {
            found.extend(declared_modules(child, source));
        }
    }
    found
}

// assert!(true) and assert_eq! on a value paired with itself: always true,
// asserting nothing.
fn tautologies(node: tree_sitter::Node, source: &str) -> Vec<(String, usize)> {
    let mut found = Vec::new();
    let mut stack = vec![node];
    while let Some(node) = stack.pop() {
        if node.kind() == "macro_invocation" {
            let name = node
                .child_by_field_name("macro")
                .and_then(|name| name.utf8_text(source.as_bytes()).ok())
                .unwrap_or("");
            let tokens = node
                .named_child(node.named_child_count().saturating_sub(1))
                .filter(|last| last.kind() == "token_tree")
                .and_then(|last| last.utf8_text(source.as_bytes()).ok())
                .unwrap_or("");
            let inner = tokens.trim_start_matches('(').trim_end_matches(')').trim();
            if name == "assert" && inner == "true" {
                found.push(("asserts true".to_string(), node.start_position().row + 1));
            }
            if name == "assert_eq" {
                if let Some((left, right)) = paired(inner) {
                    if left == right && !left.is_empty() {
                        found.push((
                            format!("asserts {left} equals itself"),
                            node.start_position().row + 1,
                        ));
                    }
                }
            }
        }
        let mut walker = node.walk();
        for child in node.children(&mut walker) {
            stack.push(child);
        }
    }
    found.sort_by_key(|(_, line)| *line);
    found
}

// The two macro arguments split at the one comma outside any nesting.
fn paired(tokens: &str) -> Option<(String, String)> {
    let mut depth = 0;
    let mut split = None;
    for (at, part) in tokens.char_indices() {
        match part {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => {
                if split.is_some() {
                    return None;
                }
                split = Some(at);
            }
            _ => {}
        }
    }
    let at = split?;
    Some((
        tokens[..at].trim().to_string(),
        tokens[at + 1..].trim().to_string(),
    ))
}

// Counts the leaf names each use binds: more than one on a line hides a
// symbol from a whole-word search pairing it with its module.
fn crowded_uses(node: Node) -> Vec<(usize, usize)> {
    let mut found = Vec::new();
    if node.kind() == "use_declaration" {
        let mut names = 0;
        for index in 0..node.named_child_count() {
            if let Some(child) = node.named_child(index) {
                if child.kind() != "visibility_modifier" {
                    names += use_leaves(child);
                }
            }
        }
        if names > 1 {
            found.push((node.start_position().row + 1, names));
        }
        return found;
    }
    for index in 0..node.named_child_count() {
        if let Some(child) = node.named_child(index) {
            found.extend(crowded_uses(child));
        }
    }
    found
}

fn use_leaves(node: Node) -> usize {
    match node.kind() {
        "use_list" => {
            let mut sum = 0;
            for index in 0..node.named_child_count() {
                if let Some(child) = node.named_child(index) {
                    sum += use_leaves(child);
                }
            }
            sum
        }
        "scoped_use_list" => node
            .child_by_field_name("list")
            .map(use_leaves)
            .unwrap_or(0),
        _ => 1,
    }
}

// Splits each crowded use where it stands. An attributed use is left for the
// hand: the attribute would govern only the first line a split writes.
fn split_uses(root: Node, source: &str) -> Option<String> {
    let mut spots: Vec<(usize, usize, String)> = Vec::new();
    collect_splits(root, source, &mut spots);
    if spots.is_empty() {
        return None;
    }
    let mut text = source.to_string();
    for (start, end, replacement) in spots.into_iter().rev() {
        text.replace_range(start..end, &replacement);
    }
    Some(text)
}

fn collect_splits(node: Node, source: &str, spots: &mut Vec<(usize, usize, String)>) {
    if node.kind() == "use_declaration" {
        let mut names = 0;
        let mut visibility = String::new();
        let mut argument = None;
        for index in 0..node.named_child_count() {
            if let Some(child) = node.named_child(index) {
                if child.kind() == "visibility_modifier" {
                    visibility = format!("{} ", &source[child.byte_range()]);
                } else {
                    names += use_leaves(child);
                    argument = Some(child);
                }
            }
        }
        let Some(argument) = argument else {
            return;
        };
        if names <= 1
            || node
                .prev_named_sibling()
                .is_some_and(|previous| previous.kind() == "attribute_item")
        {
            return;
        }
        let start = node.start_byte();
        let line_start = source[..start].rfind('\n').map(|at| at + 1).unwrap_or(0);
        let indent = &source[line_start..start];
        let lines: Vec<String> = flattened("", argument, source)
            .iter()
            .map(|path| format!("{visibility}use {path};"))
            .collect();
        spots.push((start, node.end_byte(), lines.join(&format!("\n{indent}"))));
        return;
    }
    for index in 0..node.named_child_count() {
        if let Some(child) = node.named_child(index) {
            collect_splits(child, source, spots);
        }
    }
}

fn flattened(prefix: &str, node: Node, source: &str) -> Vec<String> {
    let glue = |tail: &str| {
        if prefix.is_empty() {
            tail.to_string()
        } else {
            format!("{prefix}::{tail}")
        }
    };
    match node.kind() {
        "use_list" => {
            let mut all = Vec::new();
            for index in 0..node.named_child_count() {
                if let Some(child) = node.named_child(index) {
                    all.extend(flattened(prefix, child, source));
                }
            }
            all
        }
        "scoped_use_list" => {
            let deeper = node
                .child_by_field_name("path")
                .map(|path| glue(&source[path.byte_range()]))
                .unwrap_or_else(|| prefix.to_string());
            node.child_by_field_name("list")
                .map(|list| flattened(&deeper, list, source))
                .unwrap_or_default()
        }
        "use_as_clause" => {
            let path = node
                .child_by_field_name("path")
                .map(|path| glue(&source[path.byte_range()]))
                .unwrap_or_default();
            let alias = node
                .child_by_field_name("alias")
                .map(|alias| source[alias.byte_range()].to_string())
                .unwrap_or_default();
            vec![format!("{path} as {alias}")]
        }
        "self" => vec![prefix.to_string()],
        _ => vec![glue(&source[node.byte_range()])],
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::path::PathBuf;

    use super::manifest_findings;
    use super::module_files;
    use super::CrateLayout;

    #[test]
    fn a_git_or_absolute_path_dependency_is_named() {
        let manifest = "[dependencies]\nserde = \"1\"\n\
                        forked = { git = \"https://github.com/x/forked\" }\n\
                        patched = { path = \"/abs/patched\" }\nlocal = { path = \"../local\" }\n";
        let findings = manifest_findings(manifest, Path::new("u"));
        assert!(findings.iter().any(|finding| finding
            .reason
            .starts_with("dependencies.forked comes from git;")));
        assert!(findings.iter().any(|finding| finding
            .reason
            .starts_with("dependencies.patched is an absolute path;")));
        assert_eq!(findings.len(), 2);
    }

    #[test]
    fn a_lint_turned_to_allow_is_named_in_either_form() {
        let manifest = "[lints.clippy]\ncmp_owned = \"allow\"\n\
                        [workspace.lints.rust]\ndead_code = { level = \"allow\", priority = 1 }\n";
        let findings = manifest_findings(manifest, Path::new("u"));
        assert!(findings.iter().any(|finding| finding
            .reason
            .starts_with("lints.clippy.cmp_owned is allow;")));
        assert!(findings.iter().any(|finding| finding
            .reason
            .starts_with("workspace.lints.rust.dead_code is allow;")));
    }

    #[test]
    fn a_lint_raised_to_deny_passes_the_dials() {
        let manifest = "[lints.clippy]\nunwrap_used = \"deny\"\n";
        assert!(manifest_findings(manifest, Path::new("u")).is_empty());
    }

    // Each file by its crate's module folder and its own module path.
    fn read(file: &str) -> (String, String) {
        let layout = CrateLayout::of(Path::new("app"), Path::new(file));
        (
            layout.dir.to_string_lossy().into_owned(),
            layout.own.join("::"),
        )
    }

    fn shown(files: Vec<PathBuf>) -> Vec<String> {
        files
            .iter()
            .map(|file| file.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn a_file_s_crate_and_module_path_follow_cargo_s_layout() {
        for (file, dir, own) in [
            ("app/src/lib.rs", "app/src", ""),
            ("app/src/core/task.rs", "app/src", "core::task"),
            ("app/src/core/mod.rs", "app/src", "core"),
            ("app/src/bin/tool.rs", "app/src/bin", ""),
            ("app/src/bin/tool/main.rs", "app/src/bin/tool", ""),
            ("app/src/bin/tool/args.rs", "app/src/bin/tool", "args"),
            ("app/tests/runs.rs", "app/tests", ""),
            ("app/tests/common/mod.rs", "app/tests", "common"),
            ("app/build.rs", "app", ""),
        ] {
            assert_eq!(read(file), (dir.to_string(), own.to_string()), "{file}");
        }
        let library = CrateLayout::library(Path::new("app"));
        assert_eq!(shown(library.files(&[])), ["app/src/lib.rs"]);
        assert_eq!(
            shown(library.files(&["core".to_string(), "task".to_string()])),
            ["app/src/core/task.rs", "app/src/core/task/mod.rs"]
        );
    }

    #[test]
    fn a_path_is_read_from_its_first_segment_as_the_compiler_reads_it() {
        let crates =
            std::collections::HashMap::from([("runner".to_string(), PathBuf::from("runner"))]);
        let imports = [("trees", "crate::core")];
        let files = |module: &str| {
            shown(module_files(
                Path::new("app/src/tasks/tools.rs"),
                module,
                Path::new("app"),
                &imports,
                &crates,
            ))
        };
        assert_eq!(
            files("crate::core::task"),
            ["app/src/core/task.rs", "app/src/core/task/mod.rs"]
        );
        assert_eq!(
            files("runner::declaration::task"),
            [
                "runner/src/declaration/task.rs",
                "runner/src/declaration/task/mod.rs"
            ]
        );
        assert_eq!(
            files("trees"),
            ["app/src/core/trees.rs", "app/src/core/trees/mod.rs"],
            "an imported name stands for the path it was imported from"
        );
        assert_eq!(
            files("self::args"),
            [
                "app/src/tasks/tools/args.rs",
                "app/src/tasks/tools/args/mod.rs"
            ]
        );
        assert_eq!(
            files("super::run"),
            [
                "app/src/tasks/run.rs",
                "app/src/tasks/run/mod.rs",
                "app/src/tasks/tools/run.rs",
                "app/src/tasks/tools/run/mod.rs"
            ],
            "super is above the file, or above an inline module inside it"
        );
        assert_eq!(
            files("super::super::gate"),
            [
                "app/src/gate.rs",
                "app/src/gate/mod.rs",
                "app/src/tasks/gate.rs",
                "app/src/tasks/gate/mod.rs"
            ]
        );
        assert_eq!(
            files("args"),
            [
                "app/src/tasks/tools/args.rs",
                "app/src/tasks/tools/args/mod.rs"
            ],
            "any other name is a module under the file's own"
        );
        assert!(files("super").is_empty(), "super alone stays in its file");
    }
}
