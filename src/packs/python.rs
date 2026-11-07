use std::collections::HashMap;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

use tree_sitter::Language;
use tree_sitter::Node;
use tree_sitter::Tree;

use crate::core::pack::pinned;
use crate::core::pack::Claim;
use crate::core::pack::Def;
use crate::core::pack::Definition;
use crate::core::pack::Edge;
use crate::core::pack::Finding;
use crate::core::pack::Pack;
use crate::core::pack::Suppression;
use crate::core::prose;
use crate::core::trees;
use crate::core::units::Repo;

const DEFINITIONS: &str = include_str!("../../packs/python/definitions.scm");
const USES: &str = include_str!("../../packs/python/uses.scm");
const MENTIONS: &str = include_str!("../../packs/python/mentions.scm");
const BAR: &str = include_str!("../../packs/python/bar.toml");
const SUPPRESSIONS: &str = include_str!("../../packs/python/suppressions.scm");
const PROSE: &str = include_str!("../../packs/python/prose.scm");
const STDLIB: &str = include_str!("../../packs/python/stdlib.txt");
const DISTRIBUTIONS: &str = include_str!("../../packs/python/distributions.toml");

const CHECKERS: [&str; 2] = ["basedpyright", "pyright"];
const MODES: [&str; 3] = ["strict", "recommended", "all"];

#[derive(Default)]
pub struct Python {
    imports: Vec<(PathBuf, String, String, usize)>,
    init_imports: Vec<(PathBuf, String, usize)>,
    plains: Vec<(PathBuf, String, usize)>,
    chains: Vec<(PathBuf, String, usize)>,
    computed: Vec<(PathBuf, usize)>,
    suppressions: Vec<Suppression>,
    aliases: Vec<(PathBuf, String, usize)>,
    overreaches: Vec<(PathBuf, String, usize)>,
    tautologies: Vec<(PathBuf, usize, String)>,
    protocols: Vec<String>,
    classes: Vec<(PathBuf, usize, String, Vec<String>)>,
    // One annotated parameter: function name, position, keyword, type names.
    askings: Vec<(String, usize, Option<String>, Vec<String>)>,
    // A constructor standing at an annotated site: direct types for an
    // assignment, or the callee and argument place for a call.
    assigns: Vec<(PathBuf, usize, Vec<String>, String)>,
    callargs: Vec<(PathBuf, usize, String, usize, Option<String>, String)>,
}

impl Python {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Pack for Python {
    fn rules(&self) -> &'static [(&'static str, &'static str)] {
        &[
            ("resolve", "held"),
            (
                "unpinned-image",
                "never: the language names no container images",
            ),
            ("reexport", "held"),
            (
                "self-entry",
                "never: an __init__ stays empty, so a unit has no barrel of its own to enter",
            ),
            (
                "interior-import",
                "never: a module is imported by its defining path; python declares no routes",
            ),
            ("hidden-name", "held"),
            ("crowded-import", "held"),
            ("comment-overreach", "held"),
            ("dynamic-import", "held"),
            ("unnamed-door", "held"),
            ("undeclared-dependency", "held"),
            ("unused-dependency", "held"),
            ("suppressions", "held"),
            ("ecosystem-dials", "held"),
            ("pinned-tools", "held"),
            ("no-warnings", "held"),
            ("tautology", "held"),
            (
                "stray-test",
                "never: a test is a def, and a def's name is always literal",
            ),
            ("container", "held"),
            ("generated", "held"),
            ("phantom-import", "held"),
            (
                "drift",
                "never: the proto pack judges any unit's buf.gen.yaml, whatever its language",
            ),
        ]
    }

    fn label(&self) -> &'static str {
        "python"
    }

    fn handles(&self, name: &str) -> bool {
        name.ends_with(".py")
    }

    fn manifest(&self) -> Option<&'static str> {
        Some("pyproject.toml")
    }

    fn adapter(&self) -> Option<&'static str> {
        Some(include_str!("../../packs/python/tools.toml"))
    }

    fn configs(&self) -> &'static [(&'static str, &'static str)] {
        &[("ruff.toml", include_str!("../../packs/python/ruff.toml"))]
    }

    fn container(&self, manifest: &str) -> bool {
        let Ok(parsed) = manifest.parse::<toml::Value>() else {
            return false;
        };
        let workspace = parsed
            .get("tool")
            .and_then(|tool| tool.get("uv"))
            .and_then(|uv| uv.get("workspace"));
        parsed.get("project").is_none() && workspace.is_some()
    }

    fn dials(&self, folder: &Path) -> Option<Vec<String>> {
        let Ok(manifest) = fs::read_to_string(folder.join("pyproject.toml")) else {
            return Some(vec![
                "no pyproject.toml; the unit declares no environment".to_string()
            ]);
        };
        Some(manifest_findings(&manifest))
    }

    fn claims(&self, source: &str) -> Vec<Claim> {
        let Some(tree) = trees::parse(&language(), source) else {
            return Vec::new();
        };
        let mut found = Vec::new();
        claimed(tree.root_node(), source, &mut Vec::new(), &mut found);
        found
    }

    fn shapes(&self, source: &str) -> Vec<trees::Shape> {
        trees::shapes(&language(), source)
    }

    fn roots(&self, root: &Path, repo: &Repo) -> Vec<PathBuf> {
        let mut found = Vec::new();
        for manifest in &repo.manifests {
            if manifest
                .file_name()
                .is_none_or(|name| name != "pyproject.toml")
            {
                continue;
            }
            let Ok(source) = fs::read_to_string(root.join(manifest)) else {
                continue;
            };
            for module in manifest_entries(&source) {
                found.extend(module_file(&module, &repo.files));
            }
        }
        found
    }

    fn scan(&mut self, file: &Path, source: &str, surface: bool) -> Vec<Definition> {
        let Some(tree) = trees::parse(&language(), source) else {
            return Vec::new();
        };
        let in_init = file.file_name().is_some_and(|name| name == "__init__.py");
        for (module, name, line) in imports(&tree, source) {
            if surface && in_init {
                self.init_imports
                    .push((file.to_path_buf(), name.clone(), line));
            }
            self.imports.push((file.to_path_buf(), module, name, line));
        }
        for (name, line) in captured(&tree, source, "plain") {
            self.plains.push((file.to_path_buf(), name, line));
        }
        for (module, line) in loaded(&tree, source) {
            match module {
                Some(module) => self.plains.push((file.to_path_buf(), module, line)),
                None if surface => self.computed.push((file.to_path_buf(), line)),
                None => {}
            }
        }
        for (chain, line) in captured(&tree, source, "chain") {
            self.chains.push((file.to_path_buf(), chain, line));
        }
        for captures in trees::matches(&language(), SUPPRESSIONS, &tree, source) {
            for capture in captures {
                if let Some(blanket) = suppression_of(&capture.text) {
                    self.suppressions.push(Suppression {
                        file: file.to_path_buf(),
                        line: capture.line,
                        text: capture.text.trim().to_string(),
                        blanket,
                    });
                }
            }
        }
        for (text, line) in marked(tree.root_node(), source) {
            self.suppressions.push(Suppression {
                file: file.to_path_buf(),
                line,
                text,
                blanket: false,
            });
        }
        for (text, line) in tautologies(tree.root_node(), source) {
            self.tautologies.push((file.to_path_buf(), line, text));
        }
        doors(tree.root_node(), source, file, self);
        if !surface {
            return Vec::new();
        }
        for (text, line) in captured(&tree, source, "alias") {
            let alias = text.rsplit(" as ").next().unwrap_or("").trim().to_string();
            if !allowed_aliases().contains(&alias) {
                self.aliases.push((file.to_path_buf(), alias, line));
            }
        }
        let mentions = mentions(&tree, source);
        let idents: HashSet<String> = mentions.iter().map(|(name, _)| name.clone()).collect();
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
        &["__pycache__"]
    }

    fn generated(&self, name: &str) -> bool {
        name.ends_with("_pb2.py") || name.ends_with("_pb2_grpc.py") || name.ends_with("_pb2.pyi")
    }

    fn axioms(&self) -> &'static [&'static str] {
        &["uv"]
    }

    fn licenses(&self, root: &Path, repo: &Repo) -> Vec<(PathBuf, String, String)> {
        let mut found = Vec::new();
        for manifest in &repo.manifests {
            if manifest
                .file_name()
                .is_none_or(|name| name != "pyproject.toml")
            {
                continue;
            }
            let unit = repo.unit_of(manifest);
            let home = repo.units[unit].path.clone();
            if repo.is_test(unit, &home) {
                continue;
            }
            let folder = manifest.parent().unwrap_or(Path::new(""));
            // The unit's own package sits in its venv too, and is nobody's third party.
            let own = std::fs::read_to_string(root.join(manifest))
                .ok()
                .and_then(|text| text.parse::<toml::Value>().ok())
                .and_then(|parsed| {
                    parsed
                        .get("project")?
                        .get("name")
                        .and_then(toml::Value::as_str)
                        .map(normalized_name)
                });
            let lib = root.join(folder).join(".venv").join("lib");
            let Ok(pythons) = std::fs::read_dir(&lib) else {
                continue;
            };
            for python in pythons.flatten() {
                let site = python.path().join("site-packages");
                let Ok(entries) = std::fs::read_dir(&site) else {
                    continue;
                };
                for entry in entries.flatten() {
                    if !entry.file_name().to_string_lossy().ends_with(".dist-info") {
                        continue;
                    }
                    let Ok(text) = std::fs::read_to_string(entry.path().join("METADATA")) else {
                        continue;
                    };
                    if let Some((name, license)) = metadata_license(&text) {
                        if own.as_deref() == Some(normalized_name(&name).as_str()) {
                            continue;
                        }
                        found.push((manifest.clone(), name, license));
                    }
                }
            }
        }
        found.sort();
        found.dedup();
        found
    }

    fn lockfiles(&self) -> &'static [&'static str] {
        &["uv.lock"]
    }

    fn integrity(&self, root: &Path, repo: &Repo) -> Vec<Finding> {
        let mut findings: Vec<Finding> = self
            .init_imports
            .iter()
            .map(|(file, name, line)| Finding {
                check: "integrity",
                file: file.clone(),
                line: *line,
                name: "reexport".to_string(),
                reason: format!(
                    "__init__.py imports {name}; a symbol is imported from the module defining it"
                ),
            })
            .collect();
        findings.extend(self.aliases.iter().map(|(file, alias, line)| Finding {
            check: "integrity",
            file: file.clone(),
            line: *line,
            name: "hidden-name".to_string(),
            reason: format!("imported as {alias}; a symbol keeps its own name"),
        }));
        findings.extend(self.computed.iter().map(|(file, line)| {
            Finding {
                check: "integrity",
                file: file.clone(),
                line: *line,
                name: "hidden-name".to_string(),
                reason: "imports a module through a computed value; a module is named by a literal"
                    .to_string(),
            }
        }));
        // A fixture's tautologies are some test's data, never judged.
        let fixture = |file: &PathBuf| {
            let unit = repo.unit_of(file);
            repo.is_test(unit, &repo.units[unit].path.clone())
        };
        findings.extend(
            self.tautologies
                .iter()
                .filter(|(file, _, _)| !fixture(file))
                .map(|(file, line, text)| Finding {
                    check: "integrity",
                    file: file.clone(),
                    line: *line,
                    name: "tautology".to_string(),
                    reason: format!("{text}; an assertion compares the system to an expectation"),
                }),
        );
        findings.extend(self.overreaches.iter().map(|(file, token, line)| Finding {
            check: "integrity",
            file: file.clone(),
            line: *line,
            name: "comment-overreach".to_string(),
            reason: format!(
                "a comment names {token}, which nothing in the file defines, uses or quotes"
            ),
        }));
        let declared = declared_distributions(root, repo);
        let provided = pinned(self.adapter().unwrap_or(""));
        let stdlib: HashSet<&str> = STDLIB.lines().collect();
        let known = distribution_map();
        let mut named = std::collections::BTreeSet::new();
        let modules = self
            .imports
            .iter()
            .map(|(file, module, _, line)| (file, module, *line))
            .chain(
                self.plains
                    .iter()
                    .map(|(file, name, line)| (file, name, *line)),
            );
        for (file, module, line) in modules {
            let unit = repo.unit_of(file);
            let home = repo.units[unit].path.clone();
            if repo.is_test(unit, &home) {
                continue;
            }
            // Generated code is scanned, never judged: its imports reach, and
            // the contract answers for what they name.
            let name = file
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_default();
            if self.generated(&name) {
                continue;
            }
            let top = module.split('.').next().unwrap_or(module);
            if stdlib.contains(top) || module_file(module, &repo.files).is_some() {
                continue;
            }
            let distribution =
                normalized(&known.get(top).cloned().unwrap_or_else(|| top.to_string()));
            if provided.contains(&distribution) {
                continue;
            }
            let manifest = declared.get(&unit);
            let shipped = manifest.is_some_and(|entry| entry.shipped.contains(&distribution));
            let developed = manifest.is_some_and(|entry| entry.dev.contains(&distribution));
            if shipped || (developed && repo.is_test(unit, file)) {
                continue;
            }
            if named.insert((unit, top.to_string())) {
                let reason = if developed {
                    format!(
                        "{top} is imported by shipped code and declared only in a dependency group"
                    )
                } else {
                    format!("{top} is imported, and the unit's manifest does not declare it")
                };
                findings.push(Finding {
                    check: "integrity",
                    file: file.clone(),
                    line,
                    name: "undeclared-dependency".to_string(),
                    reason,
                });
            }
        }
        // A generated import that names no contract is a resolver truth told
        // at the import site, never a dead contract somewhere else.
        let mut phantom = std::collections::BTreeSet::new();
        let spoken = self
            .imports
            .iter()
            .map(|(file, module, _, line)| (file, module, *line))
            .chain(
                self.plains
                    .iter()
                    .map(|(file, name, line)| (file, name, *line)),
            );
        for (file, module, line) in spoken {
            let unit = repo.unit_of(file);
            let home = repo.units[unit].path.clone();
            if repo.is_test(unit, &home) {
                continue;
            }
            let leaf = file
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_default();
            if self.generated(&leaf) {
                continue;
            }
            let Some(contract) = module.rsplit('.').next().and_then(|last| {
                last.strip_suffix("_pb2")
                    .or_else(|| last.strip_suffix("_pb2_grpc"))
            }) else {
                continue;
            };
            let ending = format!("/{contract}.proto");
            let flat = format!("{contract}.proto");
            let matched = repo.files.iter().any(|candidate| {
                let shown = candidate.to_string_lossy();
                shown.ends_with(&ending) || shown == flat
            });
            if matched {
                continue;
            }
            if phantom.insert((unit, module.clone())) {
                findings.push(Finding {
                    check: "integrity",
                    file: file.clone(),
                    line,
                    name: "phantom-import".to_string(),
                    reason: format!(
                        "{module} names no contract; a generated import answers for \
                         the contract it was generated from"
                    ),
                });
            }
        }
        // The mirror: a distribution a manifest declares and no file of the
        // unit reaches is pulled and trusted for nothing.
        let mut reached: HashMap<usize, HashSet<String>> = HashMap::new();
        let spoken = self
            .imports
            .iter()
            .map(|(file, module, _, _)| (file, module))
            .chain(self.plains.iter().map(|(file, name, _)| (file, name)));
        for (file, module) in spoken {
            let top = module.split('.').next().unwrap_or(module);
            let distribution =
                normalized(&known.get(top).cloned().unwrap_or_else(|| top.to_string()));
            reached
                .entry(repo.unit_of(file))
                .or_default()
                .insert(distribution);
        }
        let mut coded: HashSet<usize> = HashSet::new();
        for file in &repo.files {
            let name = file
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_default();
            if self.handles(&name) {
                coded.insert(repo.unit_of(file));
            }
        }
        for (unit, entry) in &declared {
            let home = repo.units[*unit].path.clone();
            if repo.is_test(*unit, &home) || !coded.contains(unit) {
                continue;
            }
            let seen = reached.get(unit);
            let within = |name: &str| seen.is_some_and(|set| set.contains(name));
            for distribution in entry.dev.iter().collect::<std::collections::BTreeSet<_>>() {
                if provided.contains(distribution) || within(distribution) {
                    continue;
                }
                // A stub distribution serves the type checker of the library it types,
                // and a plugin of the pinned runner loads by entry point, not import.
                let base = distribution
                    .strip_prefix("types-")
                    .or_else(|| distribution.strip_suffix("-stubs"));
                if base.is_some_and(within) {
                    continue;
                }
                if distribution.starts_with("pytest-") && provided.contains("pytest") {
                    continue;
                }
                findings.push(Finding {
                    check: "integrity",
                    file: entry.manifest.clone(),
                    line: 0,
                    name: "unused-dependency".to_string(),
                    reason: format!(
                        "{distribution} is declared, and nothing in the unit reaches it"
                    ),
                });
            }
        }
        // A class standing where a protocol is asked names the protocol:
        // a shape satisfied silently is an edge the graph cannot see.
        let doors: HashSet<&String> = self.protocols.iter().collect();
        let places: HashMap<&str, (&PathBuf, usize, &Vec<String>)> = self
            .classes
            .iter()
            .map(|(file, line, name, bases)| (name.as_str(), (file, *line, bases)))
            .collect();
        let reaches = |class: &str, asked: &str| -> bool {
            let mut stack = vec![class];
            let mut seen = HashSet::new();
            while let Some(name) = stack.pop() {
                if !seen.insert(name.to_string()) {
                    continue;
                }
                let Some((_, _, bases)) = places.get(name) else {
                    continue;
                };
                for base in bases.iter() {
                    if base == asked {
                        return true;
                    }
                    stack.push(base);
                }
            }
            false
        };
        let mut unnamed = std::collections::BTreeSet::new();
        let fixture = |file: &PathBuf| {
            let unit = repo.unit_of(file);
            repo.is_test(unit, &repo.units[unit].path.clone()) || repo.is_test(unit, file)
        };
        let mut judge = |types: &Vec<String>, constructor: &String, findings: &mut Vec<Finding>| {
            for asked in types {
                if !doors.contains(asked) || asked == constructor {
                    continue;
                }
                let Some((place_file, place_line, _)) = places.get(constructor.as_str()) else {
                    continue;
                };
                if reaches(constructor, asked) {
                    continue;
                }
                if unnamed.insert((constructor.clone(), asked.clone())) {
                    findings.push(Finding {
                        check: "integrity",
                        file: (*place_file).clone(),
                        line: *place_line,
                        name: "unnamed-door".to_string(),
                        reason: format!(
                            "{constructor} stands where {asked} is asked and names no such \
                             base; a protocol satisfied by shape is still named"
                        ),
                    });
                }
            }
        };
        for (file, _, types, constructor) in &self.assigns {
            if fixture(file) {
                continue;
            }
            judge(types, constructor, &mut findings);
        }
        for (file, _, callee, place, keyword, constructor) in &self.callargs {
            if fixture(file) {
                continue;
            }
            for (name, at, key, types) in &self.askings {
                if name != callee {
                    continue;
                }
                let fits = match keyword {
                    Some(keyword) => key.as_ref() == Some(keyword),
                    None => at == place,
                };
                if fits {
                    judge(types, constructor, &mut findings);
                }
            }
        }
        findings
    }

    fn resolve(&self, mine: usize, defs: &[Def], _root: &Path, repo: &Repo) -> Vec<Edge> {
        let mut edges = Vec::new();
        for (file, module, name, line) in &self.imports {
            // A generated module answers for the contract it was generated from, so a
            // name taken from one reaches the matching definition in the proto
            // file by its last segment, wherever the generated code sits.
            if let Some(contract) = module
                .rsplit('.')
                .next()
                .and_then(|last| last.strip_suffix("_pb2"))
            {
                let ending = format!("/{contract}.proto");
                let flat = format!("{contract}.proto");
                for (index, def) in defs.iter().enumerate() {
                    let shown = def.file.to_string_lossy();
                    if def.name == *name && (shown.ends_with(&ending) || shown == flat) {
                        edges.push(Edge {
                            def: index,
                            file: file.clone(),
                            line: *line,
                            unit: repo.unit_of(file),
                        });
                    }
                }
            }
            for (index, def) in defs.iter().enumerate() {
                if def.pack != mine || def.name != *name {
                    continue;
                }
                if is_module_file(module, &def.file) {
                    edges.push(Edge {
                        def: index,
                        file: file.clone(),
                        line: *line,
                        unit: repo.unit_of(file),
                    });
                }
            }
        }
        // A whole-module import reaches a name through a dotted chain: the longest
        // leading piece naming a module in the repository owns the segment after it.
        let mut imported: HashMap<&PathBuf, HashSet<&str>> = HashMap::new();
        for (file, name, _) in &self.plains {
            let first = name.split('.').next().unwrap_or(name);
            imported.entry(file).or_default().insert(first);
        }
        for (file, chain, line) in &self.chains {
            let parts: Vec<&str> = chain.split('.').map(str::trim).collect();
            let Some(roots) = imported.get(file) else {
                continue;
            };
            if !roots.contains(parts[0]) {
                continue;
            }
            for split in (1..parts.len()).rev() {
                let prefix = parts[..split].join(".");
                let mut found = false;
                for (index, def) in defs.iter().enumerate() {
                    if def.pack != mine || def.name != parts[split] {
                        continue;
                    }
                    if is_module_file(&prefix, &def.file) {
                        edges.push(Edge {
                            def: index,
                            file: file.clone(),
                            line: *line,
                            unit: repo.unit_of(file),
                        });
                        found = true;
                    }
                }
                if found {
                    break;
                }
            }
        }
        edges
    }
}

fn language() -> Language {
    tree_sitter_python::LANGUAGE.into()
}

fn definitions(tree: &Tree, source: &str) -> Vec<(String, usize)> {
    trees::texts(&language(), DEFINITIONS, tree, source, "def")
        .into_iter()
        .filter(|(name, _)| !name.starts_with('_'))
        .collect()
}

fn mentions(tree: &Tree, source: &str) -> Vec<(String, usize)> {
    trees::texts(&language(), MENTIONS, tree, source, "mention")
}

// Distribution names as PEP 503 reads them: lowercased, runs of separators as one dash.
fn normalized(name: &str) -> String {
    name.to_lowercase()
        .replace(['_', '.'], "-")
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<&str>>()
        .join("-")
}

// One unit's declarations, the lists that ship kept apart from the dependency
// groups: a test import may lean on either, shipped code on the first alone.
#[derive(Default)]
struct Declared {
    manifest: PathBuf,
    shipped: HashSet<String>,
    dev: HashSet<String>,
}

fn declared_distributions(root: &Path, repo: &Repo) -> HashMap<usize, Declared> {
    let mut declared: HashMap<usize, Declared> = HashMap::new();
    for manifest in &repo.manifests {
        if manifest
            .file_name()
            .is_none_or(|name| name != "pyproject.toml")
        {
            continue;
        }
        let Ok(text) = fs::read_to_string(root.join(manifest)) else {
            continue;
        };
        let Ok(parsed) = text.parse::<toml::Value>() else {
            continue;
        };
        let entry = declared.entry(repo.unit_of(manifest)).or_default();
        entry.manifest = manifest.clone();
        let mut ships: Vec<&toml::Value> = Vec::new();
        ships.extend(
            parsed
                .get("project")
                .and_then(|project| project.get("dependencies")),
        );
        if let Some(table) = parsed
            .get("project")
            .and_then(|project| project.get("optional-dependencies"))
            .and_then(toml::Value::as_table)
        {
            ships.extend(table.values());
        }
        for list in ships {
            for name in requirement_names(list) {
                entry.shipped.insert(name.clone());
                entry.dev.insert(name);
            }
        }
        if let Some(table) = parsed
            .get("dependency-groups")
            .and_then(toml::Value::as_table)
        {
            for list in table.values() {
                for name in requirement_names(list) {
                    entry.dev.insert(name);
                }
            }
        }
    }
    declared
}

fn requirement_names(list: &toml::Value) -> Vec<String> {
    let Some(entries) = list.as_array() else {
        return Vec::new();
    };
    entries
        .iter()
        .filter_map(toml::Value::as_str)
        .map(|requirement| {
            let name: String = requirement
                .chars()
                .take_while(|part| part.is_ascii_alphanumeric() || matches!(part, '-' | '_' | '.'))
                .collect();
            normalized(&name)
        })
        .collect()
}

fn distribution_map() -> HashMap<String, String> {
    let Ok(parsed) = DISTRIBUTIONS.parse::<toml::Value>() else {
        return HashMap::new();
    };
    let Some(imports) = parsed.get("imports").and_then(toml::Value::as_table) else {
        return HashMap::new();
    };
    imports
        .iter()
        .filter_map(|(import, distribution)| {
            distribution
                .as_str()
                .map(|name| (import.clone(), name.to_string()))
        })
        .collect()
}

fn allowed_aliases() -> Vec<String> {
    let Ok(bar) = BAR.parse::<toml::Value>() else {
        return Vec::new();
    };
    bar.get("aliases")
        .and_then(|aliases| aliases.get("allowed"))
        .and_then(toml::Value::as_array)
        .map(|allowed| {
            allowed
                .iter()
                .filter_map(|name| name.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

// Whether a comment suppresses, and whether it quiets more than one named rule.
fn suppression_of(text: &str) -> Option<bool> {
    if text.contains("noqa") {
        return Some(!text.contains("noqa:"));
    }
    if text.contains("pyright: ignore") {
        return Some(!text.contains("pyright: ignore["));
    }
    if text.contains("type: ignore") {
        return Some(true);
    }
    None
}

fn captured(tree: &Tree, source: &str, wanted: &str) -> Vec<(String, usize)> {
    trees::texts(&language(), USES, tree, source, wanted)
}

fn imports(tree: &Tree, source: &str) -> Vec<(String, String, usize)> {
    trees::matches(&language(), USES, tree, source)
        .into_iter()
        .filter_map(|captures| {
            let module = captures.iter().find(|capture| capture.name == "module")?;
            let name = captures.iter().find(|capture| capture.name == "name")?;
            Some((module.text.clone(), name.text.clone(), name.line))
        })
        .collect()
}

// Run-time imports: Some(module) when the call names a literal, None when
// the value only run time resolves.
fn loaded(tree: &Tree, source: &str) -> Vec<(Option<String>, usize)> {
    trees::matches(&language(), USES, tree, source)
        .into_iter()
        .filter_map(|captures| {
            let loader = captures.iter().find(|capture| capture.name == "loader")?;
            let importer = matches!(
                loader.text.as_str(),
                "__import__" | "import_module" | "importlib.import_module"
            );
            if !importer {
                return None;
            }
            if let Some(dynamic) = captures.iter().find(|capture| capture.name == "dynamic") {
                return Some((literal_module(&dynamic.text), dynamic.line));
            }
            captures
                .iter()
                .find(|capture| capture.name == "computed")
                .map(|computed| (None, computed.line))
        })
        .collect()
}

// A plain or raw literal names its module; an f-string resolves at run time.
fn literal_module(text: &str) -> Option<String> {
    let trimmed = text.trim();
    let at = trimmed.find(['"', '\''])?;
    let (prefix, quoted) = trimmed.split_at(at);
    if prefix.to_ascii_lowercase().contains('f') {
        return None;
    }
    let inner = quoted.trim_matches(['"', '\'']);
    (!inner.is_empty()).then(|| inner.to_string())
}

fn is_module_file(module: &str, file: &Path) -> bool {
    let path = module.replace('.', "/");
    let endings = [format!("{path}.py"), format!("{path}/__init__.py")];
    let shown = file.to_string_lossy();
    endings
        .iter()
        .any(|ending| shown == *ending || shown.ends_with(&format!("/{ending}")))
}

// An entry point is called by name from outside the repository, so its module is a root.
fn manifest_entries(source: &str) -> Vec<String> {
    let Ok(parsed) = source.parse::<toml::Value>() else {
        return Vec::new();
    };
    let Some(project) = parsed.get("project") else {
        return Vec::new();
    };
    let mut tables: Vec<&toml::Value> = Vec::new();
    for name in ["scripts", "gui-scripts"] {
        tables.extend(project.get(name));
    }
    if let Some(groups) = project.get("entry-points").and_then(toml::Value::as_table) {
        tables.extend(groups.values());
    }
    let mut modules = Vec::new();
    for table in tables {
        let Some(table) = table.as_table() else {
            continue;
        };
        for value in table.values() {
            if let Some(reference) = value.as_str() {
                let module = reference.split(':').next().unwrap_or(reference).trim();
                modules.push(module.to_string());
            }
        }
    }
    modules
}

fn module_file(module: &str, files: &[PathBuf]) -> Option<PathBuf> {
    files
        .iter()
        .find(|file| is_module_file(module, file))
        .cloned()
}

// PyPI treats hyphens, underscores and case as one name; so does this reading.
fn normalized_name(name: &str) -> String {
    name.to_lowercase().replace('-', "_")
}

// The package and its license from wheel metadata: the SPDX expression where one
// is stated, else the License field, else what the classifier names.
fn metadata_license(text: &str) -> Option<(String, String)> {
    let mut name = None;
    let mut expression = None;
    let mut field = None;
    let mut classifier = None;
    for line in text.lines() {
        // The headers end at the first blank line; the description follows.
        if line.is_empty() {
            break;
        }
        if let Some(value) = line.strip_prefix("Name: ") {
            name = Some(value.trim().to_string());
        } else if let Some(value) = line.strip_prefix("License-Expression: ") {
            expression = Some(value.trim().to_string());
        } else if let Some(value) = line.strip_prefix("License: ") {
            field = Some(value.trim().to_string());
        } else if let Some(value) = line.strip_prefix("Classifier: License :: OSI Approved :: ") {
            classifier = Some(value.trim().trim_end_matches(" License").to_string());
        } else if let Some(value) = line.strip_prefix("Classifier: License :: ") {
            classifier.get_or_insert(value.trim().to_string());
        }
    }
    let license = expression
        .or_else(|| field.filter(|value| !value.is_empty() && value != "UNKNOWN"))
        .or(classifier)
        .unwrap_or_else(|| "unstated".to_string());
    Some((name?, license))
}

fn manifest_findings(manifest: &str) -> Vec<String> {
    let Ok(parsed) = manifest.parse::<toml::Value>() else {
        return vec!["pyproject.toml does not parse".to_string()];
    };
    let mut findings = refused_findings(&parsed);
    findings.extend(checker_findings(&parsed));
    findings.extend(dependency_findings(&parsed));
    findings
}

// The registry and the repository are the only channels: a git or URL source
// slips past the lockfile's hashes and past the split-version rule.
fn dependency_findings(parsed: &toml::Value) -> Vec<String> {
    let mut findings = Vec::new();
    let mut lists: Vec<(String, Vec<toml::Value>)> = Vec::new();
    if let Some(list) = lookup(parsed, "project.dependencies").and_then(toml::Value::as_array) {
        lists.push(("project.dependencies".to_string(), list.clone()));
    }
    for path in ["project.optional-dependencies", "dependency-groups"] {
        let Some(groups) = lookup(parsed, path).and_then(toml::Value::as_table) else {
            continue;
        };
        for (group, list) in groups {
            let entries = list.as_array().cloned().unwrap_or_default();
            lists.push((format!("{path}.{group}"), entries));
        }
    }
    for (place, entries) in lists {
        for entry in entries {
            let requirement = entry.as_str().unwrap_or("");
            if requirement.contains("://") || requirement.contains("git+") {
                let name = requirement.split_whitespace().next().unwrap_or(requirement);
                findings.push(format!(
                    "{place} pulls {name} past the registry; \
                     a dependency comes from the locked registry or this repository"
                ));
            }
        }
    }
    let Some(sources) = lookup(parsed, "tool.uv.sources").and_then(toml::Value::as_table) else {
        return findings;
    };
    for (name, source) in sources {
        if source.get("git").is_some() || source.get("url").is_some() {
            findings.push(format!(
                "tool.uv.sources.{name} points past the registry; \
                 a dependency comes from the locked registry or this repository"
            ));
        } else if source
            .get("path")
            .and_then(toml::Value::as_str)
            .is_some_and(|path| Path::new(path).is_absolute())
        {
            findings.push(format!(
                "tool.uv.sources.{name} is an absolute path; \
                 a path source stays inside the repository"
            ));
        }
    }
    findings
}

fn refused_findings(parsed: &toml::Value) -> Vec<String> {
    let bar = BAR.parse::<toml::Value>().expect("a pack's bar parses");
    let Some(refused) = bar.get("refused").and_then(toml::Value::as_table) else {
        return Vec::new();
    };
    let mut findings = Vec::new();
    for (path, reason) in refused {
        if lookup(parsed, path).is_some() {
            findings.push(format!("{path} is set; {}", reason.as_str().unwrap_or("")));
        }
    }
    findings
}

fn lookup<'manifest>(parsed: &'manifest toml::Value, path: &str) -> Option<&'manifest toml::Value> {
    let mut value = parsed;
    for part in path.split('.') {
        value = value.get(part)?;
    }
    Some(value)
}

fn checker_findings(parsed: &toml::Value) -> Vec<String> {
    let mut findings = Vec::new();
    for checker in CHECKERS {
        let table = parsed
            .get("tool")
            .and_then(|tool| tool.get(checker))
            .and_then(toml::Value::as_table);
        let Some(table) = table else {
            continue;
        };
        for (name, value) in table {
            if name == "typeCheckingMode" {
                let mode = value.as_str().unwrap_or("");
                if !MODES.contains(&mode) {
                    findings.push(format!(
                        "tool.{checker}.typeCheckingMode is {mode}; the bar starts at strict"
                    ));
                }
            } else if name.starts_with("report") && !is_error(value) {
                findings.push(format!(
                    "tool.{checker}.{name} is lowered; a rule reports an error or stays unset"
                ));
            } else if name == "exclude" {
                for entry in value.as_array().cloned().unwrap_or_default() {
                    let glob = entry.as_str().unwrap_or("");
                    if !glob.contains("_pb2") {
                        findings.push(format!(
                            "tool.{checker}.exclude hides {glob}; only generated code leaves the checker's sight"
                        ));
                    }
                }
            }
        }
    }
    findings
}

fn is_error(value: &toml::Value) -> bool {
    value.as_str() == Some("error") || value.as_bool() == Some(true)
}

// pytest's grammar of behavior: a test_ function claims its name as a
// sentence, framed by the classes that hold it.
fn claimed(node: tree_sitter::Node, source: &str, path: &mut Vec<String>, found: &mut Vec<Claim>) {
    if node.kind() == "class_definition" {
        if let Some(name) = node.child_by_field_name("name") {
            path.push(name.utf8_text(source.as_bytes()).unwrap_or("").to_string());
            let mut walker = node.walk();
            for child in node.children(&mut walker) {
                claimed(child, source, path, found);
            }
            path.pop();
            return;
        }
    }
    if node.kind() == "function_definition" {
        if let Some(name) = node.child_by_field_name("name") {
            let name = name.utf8_text(source.as_bytes()).unwrap_or("");
            if let Some(rest) = name.strip_prefix("test_") {
                // A skipped or expected-to-fail test proves nothing.
                if silenced(node, source) {
                    return;
                }
                found.push(Claim {
                    path: path.clone(),
                    sentence: rest.replace('_', " "),
                    line: node.start_position().row + 1,
                });
            }
        }
    }
    let mut walker = node.walk();
    for child in node.children(&mut walker) {
        claimed(child, source, path, found);
    }
}

// A pytest skip, skipif or xfail is a suppression the ledger counts.
fn marked(node: tree_sitter::Node, source: &str) -> Vec<(String, usize)> {
    let mut found = Vec::new();
    let mut stack = vec![node];
    while let Some(node) = stack.pop() {
        if node.kind() == "decorator" {
            let text = node.utf8_text(source.as_bytes()).unwrap_or("");
            if ["mark.skipif", "mark.skip", "mark.xfail"]
                .iter()
                .any(|form| text.contains(form))
            {
                let first = text.lines().next().unwrap_or("").trim().to_string();
                found.push((first, node.start_position().row + 1));
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

// Whether a test's decorators silence it: skip and skipif never run it, and a
// strict xfail proves only that it fails.
fn silenced(function: tree_sitter::Node, source: &str) -> bool {
    let Some(parent) = function.parent() else {
        return false;
    };
    if parent.kind() != "decorated_definition" {
        return false;
    }
    let mut walker = parent.walk();
    let silenced = parent.children(&mut walker).any(|child| {
        child.kind() == "decorator"
            && ["mark.skipif", "mark.skip", "mark.xfail"]
                .iter()
                .any(|form| {
                    child
                        .utf8_text(source.as_bytes())
                        .unwrap_or("")
                        .contains(form)
                })
    });
    silenced
}

// assert True and assert on a value compared to itself: always true, asserting
// nothing.
fn tautologies(node: tree_sitter::Node, source: &str) -> Vec<(String, usize)> {
    let mut found = Vec::new();
    let mut stack = vec![node];
    while let Some(node) = stack.pop() {
        if node.kind() == "assert_statement" {
            if let Some(condition) = node.named_child(0) {
                let text = condition.utf8_text(source.as_bytes()).unwrap_or("");
                if condition.kind() == "true" || condition.kind() == "integer" {
                    found.push((format!("asserts {text}"), node.start_position().row + 1));
                }
                if condition.kind() == "comparison_operator" {
                    let mut walker = condition.walk();
                    let sides: Vec<String> = condition
                        .named_children(&mut walker)
                        .filter(|&side| {
                            matches!(
                                side.kind(),
                                "identifier" | "integer" | "string" | "true" | "false"
                            )
                        })
                        .map(|side| side.utf8_text(source.as_bytes()).unwrap_or("").to_string())
                        .collect();
                    if sides.len() == 2 && sides[0] == sides[1] && text.contains("==") {
                        found.push((
                            format!("asserts {} equals itself", sides[0]),
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

// Collects the door shapes: protocol classes, each class's bases, annotated
// parameters, and the constructors standing at annotated sites.
fn doors(node: Node, source: &str, file: &Path, pack: &mut Python) {
    let text = |at: Node| source[at.byte_range()].to_string();
    match node.kind() {
        "class_definition" => {
            let Some(name) = node.child_by_field_name("name") else {
                return;
            };
            let mut bases = Vec::new();
            if let Some(supers) = node.child_by_field_name("superclasses") {
                for index in 0..supers.named_child_count() {
                    if let Some(base) = supers.named_child(index) {
                        let whole = text(base);
                        let last = whole.rsplit('.').next().unwrap_or(&whole);
                        bases.push(last.split('[').next().unwrap_or(last).to_string());
                    }
                }
            }
            if bases.iter().any(|base| base == "Protocol") {
                pack.protocols.push(text(name));
            }
            pack.classes.push((
                file.to_path_buf(),
                node.start_position().row + 1,
                text(name),
                bases,
            ));
        }
        "function_definition" => {
            let Some(name) = node.child_by_field_name("name") else {
                return;
            };
            if let Some(parameters) = node.child_by_field_name("parameters") {
                let mut place = 0;
                for index in 0..parameters.named_child_count() {
                    let Some(parameter) = parameters.named_child(index) else {
                        continue;
                    };
                    let kind = parameter.kind();
                    if kind == "typed_parameter" || kind == "typed_default_parameter" {
                        let keyword = parameter
                            .named_child(0)
                            .filter(|child| child.kind() == "identifier")
                            .map(text);
                        if let Some(annotation) = parameter.child_by_field_name("type") {
                            let names = type_names(annotation, source);
                            if !names.is_empty() {
                                pack.askings.push((text(name), place, keyword, names));
                            }
                        }
                    }
                    place += 1;
                }
            }
        }
        "assignment" => {
            if let (Some(annotation), Some(right)) = (
                node.child_by_field_name("type"),
                node.child_by_field_name("right"),
            ) {
                if let Some(constructor) = constructed(right, source) {
                    pack.assigns.push((
                        file.to_path_buf(),
                        node.start_position().row + 1,
                        type_names(annotation, source),
                        constructor,
                    ));
                }
            }
        }
        "call" => {
            if let (Some(function), Some(arguments)) = (
                node.child_by_field_name("function"),
                node.child_by_field_name("arguments"),
            ) {
                if function.kind() == "identifier" {
                    let callee = text(function);
                    let mut place = 0;
                    for index in 0..arguments.named_child_count() {
                        let Some(argument) = arguments.named_child(index) else {
                            continue;
                        };
                        if argument.kind() == "keyword_argument" {
                            if let (Some(key), Some(value)) = (
                                argument.child_by_field_name("name"),
                                argument.child_by_field_name("value"),
                            ) {
                                if let Some(constructor) = constructed(value, source) {
                                    pack.callargs.push((
                                        file.to_path_buf(),
                                        argument.start_position().row + 1,
                                        callee.clone(),
                                        usize::MAX,
                                        Some(text(key)),
                                        constructor,
                                    ));
                                }
                            }
                            continue;
                        }
                        if let Some(constructor) = constructed(argument, source) {
                            pack.callargs.push((
                                file.to_path_buf(),
                                argument.start_position().row + 1,
                                callee.clone(),
                                place,
                                None,
                                constructor,
                            ));
                        }
                        place += 1;
                    }
                }
            }
        }
        _ => {}
    }
    for index in 0..node.named_child_count() {
        if let Some(child) = node.named_child(index) {
            doors(child, source, file, pack);
        }
    }
}

// A direct constructor: a call whose function is a bare capitalized name.
fn constructed(node: Node, source: &str) -> Option<String> {
    if node.kind() != "call" {
        return None;
    }
    let function = node.child_by_field_name("function")?;
    if function.kind() != "identifier" {
        return None;
    }
    let name = source[function.byte_range()].to_string();
    name.chars()
        .next()
        .is_some_and(char::is_uppercase)
        .then_some(name)
}

// Every identifier a type annotation names, so a union or an Optional still
// speaks each of its doors.
fn type_names(node: Node, source: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut stack = vec![node];
    while let Some(at) = stack.pop() {
        if at.kind() == "identifier" {
            names.push(source[at.byte_range()].to_string());
        }
        for index in 0..at.named_child_count() {
            if let Some(child) = at.named_child(index) {
                stack.push(child);
            }
        }
    }
    names
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::is_module_file;
    use super::manifest_findings;
    use super::metadata_license;

    #[test]
    fn an_exclude_may_name_generated_code_and_nothing_else() {
        let manifest =
            "[tool.basedpyright]\nexclude = [\"**/*_pb2.py\", \"**/*_pb2.pyi\", \"src/core\"]\n";
        let findings = manifest_findings(manifest);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].starts_with("tool.basedpyright.exclude hides src/core;"));
    }

    #[test]
    fn a_license_comes_from_the_expression_the_field_or_the_classifier() {
        let modern = "Name: alpha\nLicense-Expression: MIT OR Apache-2.0\n\nBody";
        let fielded = "Name: beta\nLicense: BSD-3-Clause\n\nBody";
        let classified =
            "Name: gamma\nLicense: UNKNOWN\nClassifier: License :: OSI Approved :: MIT License\n";
        assert_eq!(
            metadata_license(modern),
            Some(("alpha".to_string(), "MIT OR Apache-2.0".to_string()))
        );
        assert_eq!(
            metadata_license(fielded),
            Some(("beta".to_string(), "BSD-3-Clause".to_string()))
        );
        assert_eq!(
            metadata_license(classified),
            Some(("gamma".to_string(), "MIT".to_string()))
        );
    }

    #[test]
    fn a_dependency_pulled_past_the_registry_is_named() {
        let manifest = "[project]\nname = \"a\"\nversion = \"0\"\n\
                        dependencies = [\"requests>=2\", \"left @ git+https://github.com/x/left\"]\n\
                        [dependency-groups]\ndev = [\"tool @ https://example.com/tool.whl\"]\n\
                        [tool.uv.sources]\nforked = { git = \"https://github.com/x/forked\" }\n\
                        patched = { path = \"/abs/patched\" }\nlocal = { path = \"../local\" }\n";
        let findings = manifest_findings(manifest);
        assert!(findings.iter().any(|finding| {
            finding.starts_with("project.dependencies pulls left past the registry;")
        }));
        assert!(findings.iter().any(|finding| {
            finding.starts_with("dependency-groups.dev pulls tool past the registry;")
        }));
        assert!(
            findings
                .iter()
                .any(|finding| finding
                    .starts_with("tool.uv.sources.forked points past the registry;"))
        );
        assert!(findings
            .iter()
            .any(|finding| finding.starts_with("tool.uv.sources.patched is an absolute path;")));
        assert_eq!(findings.len(), 4);
    }

    #[test]
    fn a_setting_lowering_the_bar_is_named_with_its_reason() {
        let manifest = "[tool.ruff.lint]\nignore = [\"E501\"]\n";
        let findings = manifest_findings(manifest);
        assert!(findings
            .iter()
            .any(|finding| finding.starts_with("tool.ruff.lint.ignore is set;")));
    }

    #[test]
    fn a_lowered_checker_rule_and_mode_are_named() {
        let manifest =
            "[tool.basedpyright]\ntypeCheckingMode = \"basic\"\nreportAny = \"warning\"\n";
        let findings = manifest_findings(manifest);
        assert!(findings
            .iter()
            .any(|finding| finding.contains("typeCheckingMode is basic")));
        assert!(findings
            .iter()
            .any(|finding| finding.contains("reportAny is lowered")));
    }

    #[test]
    fn an_environment_setting_passes_the_dials() {
        let manifest = "[project]\nname = \"a\"\nversion = \"0\"\n\
                        [tool.ruff]\nextend = \"shared.toml\"\n\
                        [tool.basedpyright]\nextends = \"shared.json\"\n";
        assert!(manifest_findings(manifest).is_empty());
    }

    #[test]
    fn a_module_path_reaches_its_file_or_its_package() {
        assert!(is_module_file(
            "used.calc",
            Path::new("libs/used_py/src/used/calc.py")
        ));
        assert!(is_module_file("used", Path::new("src/used/__init__.py")));
    }

    #[test]
    fn a_longer_name_sharing_a_suffix_is_another_module() {
        assert!(!is_module_file(
            "used.calc",
            Path::new("src/used/recalc.py")
        ));
        assert!(!is_module_file("calc", Path::new("src/used/calc.pyi")));
    }
}
