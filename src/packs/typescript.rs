use std::ffi::OsStr;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

use tree_sitter::Language;
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

const DEFINITIONS: &str = include_str!("../../packs/typescript/definitions.scm");
const USES: &str = include_str!("../../packs/typescript/uses.scm");
const MENTIONS: &str = include_str!("../../packs/typescript/mentions.scm");
const BAR: &str = include_str!("../../packs/typescript/bar.toml");
const REEXPORTS: &str = include_str!("../../packs/typescript/reexports.scm");
const SUPPRESSIONS: &str = include_str!("../../packs/typescript/suppressions.scm");
const PROSE: &str = include_str!("../../packs/typescript/prose.scm");
const BUILTINS: &str = include_str!("../../packs/typescript/builtins.txt");

const RESOLVE: [&str; 8] = [
    "npx",
    "--yes",
    "--package",
    "typescript@7.0.2",
    "tsc",
    "--showConfig",
    "-p",
    ".",
];

#[derive(Default)]
pub struct TypeScript {
    imports: Vec<(PathBuf, String, String, usize)>,
    // The source, line and whether the re-export is a star forwarding every name.
    reexports: Vec<(PathBuf, String, usize, bool)>,
    suppressions: Vec<Suppression>,
    aliases: Vec<(PathBuf, String, usize)>,
    overreaches: Vec<(PathBuf, String, usize)>,
    computed: Vec<(PathBuf, usize)>,
    crowded: Vec<(PathBuf, usize, usize)>,
    // The lines holding a type-only import, which erases at build time.
    typed: std::collections::HashSet<(PathBuf, usize)>,
    tautologies: Vec<(PathBuf, usize, String)>,
    // Test calls whose sentence no literal provides: claims the tree cannot speak.
    strays: Vec<(PathBuf, usize)>,
    interfaces: Vec<String>,
    classes: Vec<(PathBuf, usize, String, Vec<String>)>,
    // One annotated parameter: function name, position, and its type names.
    askings: Vec<(String, usize, Vec<String>)>,
    // A construction standing at an annotated site: direct types for a
    // declarator, or the callee and argument place for a call.
    assigns: Vec<(PathBuf, usize, Vec<String>, String)>,
    callargs: Vec<(PathBuf, usize, String, usize, String)>,
}

impl TypeScript {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Pack for TypeScript {
    fn rules(&self) -> &'static [(&'static str, &'static str)] {
        &[
            ("resolve", "held"),
            (
                "unpinned-image",
                "never: the language names no container images",
            ),
            ("dynamic-import", "held"),
            ("reexport", "held"),
            ("self-entry", "held"),
            ("interior-import", "held"),
            ("hidden-name", "held"),
            ("crowded-import", "held"),
            ("comment-overreach", "held"),
            ("undeclared-dependency", "held"),
            ("unused-dependency", "held"),
            ("suppressions", "held"),
            ("wiring", "held"),
            ("tools", "held"),
            ("no-warnings", "held"),
            ("tautology", "held"),
            ("stray-test", "held"),
            ("unnamed-door", "held"),
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
        "typescript"
    }

    fn handles(&self, name: &str) -> bool {
        !name.ends_with(".d.ts") && (name.ends_with(".ts") || name.ends_with(".tsx"))
    }

    fn manifest(&self) -> Option<&'static str> {
        Some("package.json")
    }

    fn adapter(&self) -> Option<&'static str> {
        Some(include_str!("../../packs/typescript/tools.toml"))
    }

    // Declaring members and no entry of its own: a root with code declares one.
    fn container(&self, manifest: &str) -> bool {
        let Ok(parsed) = serde_json::from_str::<serde_json::Value>(manifest) else {
            return false;
        };
        parsed.get("workspaces").is_some()
            && parsed.get("main").is_none()
            && parsed.get("exports").is_none()
            && parsed.get("bin").is_none()
    }

    fn configs(&self) -> &'static [(&'static str, &'static str)] {
        &[(
            "biome.jsonc",
            include_str!("../../packs/typescript/biome.jsonc"),
        )]
    }

    fn wiring(&self, folder: &Path) -> Option<Vec<String>> {
        let finished = std::process::Command::new(RESOLVE[0])
            .args(&RESOLVE[1..])
            .current_dir(folder)
            .output();
        let Ok(output) = finished else {
            return Some(vec![format!("{} did not start", RESOLVE[0])]);
        };
        if !output.status.success() {
            return Some(vec![
                "no tsconfig.json resolves here; the unit declares no environment".to_string(),
            ]);
        }
        let Ok(resolved) = serde_json::from_slice::<serde_json::Value>(&output.stdout) else {
            return Some(vec![
                "tsc --showConfig answered something unreadable".to_string()
            ]);
        };
        let options = resolved
            .get("compilerOptions")
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        let mut findings = option_findings(&options);
        if let Ok(manifest) = std::fs::read_to_string(folder.join("package.json")) {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&manifest) {
                findings.extend(dependency_findings(&parsed));
            }
        }
        Some(findings)
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

    fn roots(&self, _root: &Path, _repo: &Repo) -> Vec<PathBuf> {
        Vec::new()
    }

    fn scan(&mut self, file: &Path, source: &str, surface: bool) -> Vec<Definition> {
        let Some(tree) = trees::parse(&language(), source) else {
            return Vec::new();
        };
        for (from, name, line, typed) in imports(&tree, source) {
            if typed {
                self.typed.insert((file.to_path_buf(), line));
            }
            self.imports.push((file.to_path_buf(), from, name, line));
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
        if testish(file) {
            for line in strays(tree.root_node(), source) {
                self.strays.push((file.to_path_buf(), line));
            }
        }
        doors(tree.root_node(), source, file, self);
        if !surface {
            return Vec::new();
        }
        for captures in trees::matches(&language(), REEXPORTS, &tree, source) {
            for capture in captures {
                let star = capture.name == "star";
                self.reexports
                    .push((file.to_path_buf(), capture.text, capture.line, star));
            }
        }
        for captures in trees::matches(&language(), USES, &tree, source) {
            for capture in captures {
                if capture.name == "aliased" {
                    let alias = capture
                        .text
                        .rsplit(" as ")
                        .next()
                        .unwrap_or("")
                        .trim()
                        .to_string();
                    self.aliases.push((file.to_path_buf(), alias, capture.line));
                }
                if capture.name == "computed" {
                    self.computed.push((file.to_path_buf(), capture.line));
                }
            }
        }
        for shape in import_shapes(&tree, source) {
            let names = usize::from(shape.default.is_some())
                + usize::from(shape.namespace.is_some())
                + shape.named.len();
            if names > 1 {
                self.crowded.push((file.to_path_buf(), shape.row, names));
            }
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

    fn integrity(&self, root: &Path, repo: &Repo) -> Vec<Finding> {
        let present: std::collections::HashSet<&PathBuf> = repo.files.iter().collect();
        let routes = manifest_routes(root, repo, &present);
        // A surface holding definitions is at once the offering file and the
        // defining file, and every judgment telling the two roles apart misreads it.
        let defined = surface_definitions(root, &routes.surfaces);
        let mut findings: Vec<Finding> = Vec::new();
        for file in &routes.written {
            let unit = repo.unit_of(file);
            let home = repo.units[unit].path.clone();
            if repo.is_test(unit, &home) {
                continue;
            }
            let Some(owned) = defined.get(file) else {
                continue;
            };
            let Some((name, line)) = owned.iter().min_by_key(|(_, line)| *line) else {
                continue;
            };
            let reason = if owned.len() == 1 {
                format!("defines {name}; a surface offers only names defined elsewhere")
            } else {
                format!(
                    "defines {name} and {} more names; a surface offers only names defined elsewhere",
                    owned.len() - 1
                )
            };
            findings.push(Finding::integrity(file, *line, "reexport", reason));
        }
        // A route target is the unit's declared surface, so a named re-export there is
        // the declaration itself; anywhere else it hides a name, and a star hides
        // every name even on the surface.
        for (file, from, line, star) in &self.reexports {
            let surface = routes.surfaces.contains(file);
            if surface && !star {
                continue;
            }
            let reason = if surface {
                format!(
                    "re-exports every name from \"{from}\"; a surface names each symbol it offers"
                )
            } else {
                format!("re-exports from \"{from}\"; a module offers only its own names")
            };
            findings.push(Finding::integrity(file, *line, "reexport", reason));
        }
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
        findings.extend(self.strays.iter().filter(|(file, _)| !fixture(file)).map(
            |(file, line)| Finding {
                check: "integrity",
                file: file.clone(),
                line: *line,
                name: "stray-test".to_string(),
                reason:
                    "a test the tree cannot name; a claim speaks a literal sentence".to_string(),
            },
        ));
        findings.extend(
            self.tautologies
                .iter()
                .filter(|(file, _, _)| !fixture(file))
                .map(|(file, line, text)| Finding {
                    check: "integrity",
                    file: file.clone(),
                    line: *line,
                    name: "tautology".to_string(),
                    reason: format!(
                "asserts {text} equals itself; an assertion compares the system to an expectation"
            ),
                }),
        );
        findings.extend(self.crowded.iter().map(|(file, line, names)| Finding {
            check: "integrity",
            file: file.clone(),
            line: *line,
            name: "crowded-import".to_string(),
            reason: format!(
                "names {names} symbols in one import; an import line names one symbol and the module it comes from"
            ),
        }));
        findings.extend(self.overreaches.iter().map(|(file, token, line)| Finding {
            check: "integrity",
            file: file.clone(),
            line: *line,
            name: "comment-overreach".to_string(),
            reason: format!(
                "a comment names {token}, which nothing in the file defines, uses or quotes"
            ),
        }));
        let declared = declared_packages(root, repo);
        let provided = pinned(self.adapter().unwrap_or(""));
        let builtins: std::collections::HashSet<&str> = BUILTINS.lines().collect();
        let mut named = std::collections::BTreeSet::new();
        for (file, from, _, line) in &self.imports {
            let unit = repo.unit_of(file);
            let home = repo.units[unit].path.clone();
            if repo.is_test(unit, &home) {
                continue;
            }
            // Generated code is scanned, never judged: its imports only reach.
            let leaf = file
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_default();
            if self.generated(&leaf) {
                continue;
            }
            if from.starts_with('.') || from.starts_with('#') || from.starts_with("node:") {
                continue;
            }
            let package = packaged(from);
            if builtins.contains(package.as_str()) || provided.contains(&package) {
                continue;
            }
            let manifest = declared.get(&unit);
            // A unit importing its own name is self-entry's case, never a missing dependency.
            if manifest.and_then(|entry| entry.own.as_deref()) == Some(package.as_str()) {
                continue;
            }
            let shipped = manifest.is_some_and(|entry| entry.shipped.contains(&package));
            let developed = manifest.is_some_and(|entry| entry.dev.contains(&package));
            // Development code may rest on devDependencies: tests, the toolchain's
            // config files, and a type-only import, which erases at build time.
            let development = repo.is_test(unit, file)
                || developmental(file)
                || self.typed.contains(&(file.clone(), *line));
            if shipped || (developed && development) {
                continue;
            }
            if named.insert((unit, package.clone())) {
                let reason = if developed {
                    format!(
                        "{package} is imported by shipped code and declared only under devDependencies"
                    )
                } else {
                    format!("{package} is imported, and the unit's manifest does not declare it")
                };
                findings.push(Finding {
                    check: "integrity",
                    file: file.clone(),
                    line: *line,
                    name: "undeclared-dependency".to_string(),
                    reason,
                });
            }
        }
        // A generated import answers for a contract; one resolving to none is
        // named where it stands, never left to read as dead code.
        let mut phantom = std::collections::BTreeSet::new();
        for (file, from, _, line) in &self.imports {
            let unit = repo.unit_of(file);
            let home = repo.units[unit].path.clone();
            if repo.is_test(unit, &home) {
                continue;
            }
            let leaf = file
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_default();
            if self.generated(&leaf) || !from.starts_with('.') {
                continue;
            }
            let Some(contract) = from.rsplit('/').next().and_then(|last| {
                last.strip_suffix("_pb")
                    .or_else(|| last.strip_suffix(".pb"))
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
            if phantom.insert((unit, from.clone())) {
                findings.push(Finding {
                    check: "integrity",
                    file: file.clone(),
                    line: *line,
                    name: "phantom-import".to_string(),
                    reason: format!(
                        "{from} names no contract; a generated import answers for \
                         the contract it was generated from"
                    ),
                });
            }
        }
        // The mirror: a package a manifest declares and no file of the unit
        // reaches is pulled and trusted for nothing.
        let mut reached: std::collections::HashMap<usize, std::collections::HashSet<String>> =
            std::collections::HashMap::new();
        for (file, from, _, _) in &self.imports {
            let unit = repo.unit_of(file);
            if from.starts_with("node:") {
                reached.entry(unit).or_default().insert("node".to_string());
                continue;
            }
            if from.starts_with('.') || from.starts_with('#') {
                continue;
            }
            reached.entry(unit).or_default().insert(packaged(from));
        }
        for file in companions(root) {
            let Ok(source) = std::fs::read_to_string(root.join(&file)) else {
                continue;
            };
            let Some(tree) = trees::parse(&language(), &source) else {
                continue;
            };
            let unit = repo.unit_of(&file);
            for (from, _, _, _) in imports(&tree, &source) {
                if from.starts_with('.') || from.starts_with('#') || from.starts_with("node:") {
                    continue;
                }
                reached.entry(unit).or_default().insert(packaged(&from));
            }
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
        let owns: std::collections::HashSet<String> = declared
            .values()
            .filter_map(|entry| entry.own.clone())
            .collect();
        for (unit, entry) in &declared {
            let home = repo.units[*unit].path.clone();
            if repo.is_test(*unit, &home) || !coded.contains(unit) {
                continue;
            }
            let seen = reached.get(unit);
            let within = |package: &str| seen.is_some_and(|set| set.contains(package));
            for package in entry.dev.iter().collect::<std::collections::BTreeSet<_>>() {
                if provided.contains(package)
                    || owns.contains(package)
                    || within(package)
                    || entry.peers.contains(package)
                    || entry.scripts.contains(package)
                {
                    continue;
                }
                if let Some(base) = package.strip_prefix("@types/") {
                    if base == "node"
                        || within(base)
                        || entry.peers.contains(base)
                        || within(&format!("@{}", base.replace("__", "/")))
                    {
                        continue;
                    }
                }
                findings.push(Finding {
                    check: "integrity",
                    file: entry.manifest.clone(),
                    line: 0,
                    name: "unused-dependency".to_string(),
                    reason: format!("{package} is declared, and nothing in the unit reaches it"),
                });
            }
        }
        // A class standing where an interface is asked names the interface:
        // a shape satisfied silently is an edge the graph cannot see.
        let doors: std::collections::HashSet<&String> = self.interfaces.iter().collect();
        let places: std::collections::HashMap<&str, (&PathBuf, usize, &Vec<String>)> = self
            .classes
            .iter()
            .map(|(file, line, name, bases)| (name.as_str(), (file, *line, bases)))
            .collect();
        let reaches = |class: &str, asked: &str| -> bool {
            let mut stack = vec![class];
            let mut walked = std::collections::HashSet::new();
            while let Some(name) = stack.pop() {
                if !walked.insert(name.to_string()) {
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
        let mut judge = |types: &Vec<String>,
                         construction: &String,
                         findings: &mut Vec<Finding>| {
            for asked in types {
                if !doors.contains(asked) || asked == construction {
                    continue;
                }
                let Some((place_file, place_line, _)) = places.get(construction.as_str()) else {
                    continue;
                };
                if reaches(construction, asked) {
                    continue;
                }
                if unnamed.insert((construction.clone(), asked.clone())) {
                    findings.push(Finding {
                        check: "integrity",
                        file: (*place_file).clone(),
                        line: *place_line,
                        name: "unnamed-door".to_string(),
                        reason: format!(
                            "{construction} stands where {asked} is asked and names no \
                                 such base; a protocol satisfied by shape is still named"
                        ),
                    });
                }
            }
        };
        for (file, _, types, construction) in &self.assigns {
            if fixture(file) || repo.is_test(repo.unit_of(file), file) {
                continue;
            }
            judge(types, construction, &mut findings);
        }
        for (file, _, callee, place, construction) in &self.callargs {
            if fixture(file) || repo.is_test(repo.unit_of(file), file) {
                continue;
            }
            for (name, at, types) in &self.askings {
                if name == callee && at == place {
                    judge(types, construction, &mut findings);
                }
            }
        }
        // An import crossing into another unit anywhere but a declared route bypasses
        // the boundary the manifest draws.
        let mut crossed = std::collections::BTreeSet::new();
        for (file, from, _, line) in &self.imports {
            if file
                .file_name()
                .is_some_and(|name| self.generated(&name.to_string_lossy()))
            {
                continue;
            }
            // One import statement carries many names and is judged once.
            if !crossed.insert((file.clone(), *line, from.clone())) {
                continue;
            }
            let unit = repo.unit_of(file);
            // A unit on a test path is a fixture: data for some test, not code to judge.
            let home = repo.units[unit].path.clone();
            if repo.is_test(unit, &home) {
                continue;
            }
            let mut target = None;
            if from.starts_with('#') {
                if let Some(path) = routes.aliases.get(&unit).and_then(|map| map.get(from)) {
                    let folder = repo.units[unit].path.clone();
                    target = existing(&normalized(&folder.join(path)), &present);
                }
            } else if from.starts_with('.') {
                let base = file.parent().unwrap_or(Path::new(""));
                target = existing(&normalized(&base.join(from)), &present);
            } else if let Some(entry) = routes.names.get(from.as_str()) {
                // A unit naming itself takes its own surface the long way round.
                if repo.unit_of(entry) == unit {
                    findings.push(Finding::integrity(
                        file,
                        *line,
                        "self-entry",
                        format!(
                            "enters its own unit at {}; a unit's own code imports from the defining file",
                            entry.display()
                        ),
                    ));
                }
            } else if !routes.declared.contains(from.as_str())
                && !routes.patterns.iter().any(|pattern| routed(from, pattern))
            {
                if let Some(package) = routes
                    .packages
                    .iter()
                    .find(|package| from.starts_with(&format!("{package}/")))
                {
                    findings.push(Finding::integrity(
                        file,
                        *line,
                        "interior-import",
                        format!("goes into {package} past its routes; a unit is entered through the routes its manifest declares"),
                    ));
                }
            }
            let Some(target) = target else {
                continue;
            };
            let owner = repo.unit_of(&target);
            if owner == unit {
                // A surface holding definitions is itself the defining file, so
                // reaching it by path is what the rule prescribes, not a detour.
                if routes.surfaces.contains(&target)
                    && target != *file
                    && !defined.contains_key(&target)
                {
                    findings.push(Finding::integrity(
                        file,
                        *line,
                        "self-entry",
                        format!(
                            "enters its own unit at {}; a unit's own code imports from the defining file",
                            target.display()
                        ),
                    ));
                }
                continue;
            }
            // A file no unit claims has no manifest, so no routes exist to demand.
            if repo.units[owner].path.as_os_str().is_empty() {
                continue;
            }
            findings.push(Finding::integrity(
                file,
                *line,
                "interior-import",
                format!(
                    "reaches {} inside {}; a unit is entered through the routes its manifest declares",
                    target.display(),
                    repo.units[owner].path.display()
                ),
            ));
        }
        findings
    }

    fn outputs(&self) -> &'static [&'static str] {
        &["node_modules", "dist"]
    }

    fn axioms(&self) -> &'static [&'static str] {
        &["node", "npx"]
    }

    fn licenses(&self, root: &Path, repo: &Repo) -> Vec<(PathBuf, String, String)> {
        let mut found = Vec::new();
        for manifest in &repo.manifests {
            if manifest
                .file_name()
                .is_none_or(|name| name != "package.json")
            {
                continue;
            }
            let unit = repo.unit_of(manifest);
            let home = repo.units[unit].path.clone();
            if repo.is_test(unit, &home) {
                continue;
            }
            let folder = manifest.parent().unwrap_or(Path::new(""));
            for dir in package_dirs(&root.join(folder).join("node_modules")) {
                let Ok(text) = std::fs::read_to_string(dir.join("package.json")) else {
                    continue;
                };
                let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&text) else {
                    continue;
                };
                let (name, license) = manifest_license(&parsed);
                if let Some(name) = name {
                    found.push((manifest.clone(), name, license));
                }
            }
        }
        found.sort();
        found.dedup();
        found
    }

    fn lockfiles(&self) -> &'static [&'static str] {
        &["pnpm-lock.yaml", "package-lock.json"]
    }

    fn locked(&self, name: &str, text: &str) -> Vec<(String, String)> {
        if name == "package-lock.json" {
            npm_locked(text)
        } else {
            pnpm_locked(text)
        }
    }

    fn generated(&self, name: &str) -> bool {
        name.ends_with("_pb.ts") || name.ends_with(".pb.ts")
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
            let Some(tree) = trees::parse(&language(), &source) else {
                continue;
            };
            let shapes = import_shapes(&tree, &source);
            if let Some(mended) = split_imports(&source, &shapes) {
                if std::fs::write(&path, mended).is_ok() {
                    notes.push(format!("{} split to one import per line", file.display()));
                }
            }
        }
        notes
    }

    fn resolve(&self, mine: usize, defs: &[Def], root: &Path, repo: &Repo) -> Vec<Edge> {
        let present: std::collections::HashSet<&PathBuf> = repo.files.iter().collect();
        // Each re-export, resolved to the file it reaches, so a name can pass through.
        let mut barrels: std::collections::HashMap<PathBuf, Vec<PathBuf>> =
            std::collections::HashMap::new();
        for (file, from, _, _) in &self.reexports {
            let base = file.parent().unwrap_or(Path::new(""));
            if let Some(target) = existing(&normalized(&base.join(from)), &present) {
                barrels.entry(file.clone()).or_default().push(target);
            }
        }
        let routes = manifest_routes(root, repo, &present);
        let mut edges = Vec::new();
        for (file, from, name, line) in &self.imports {
            // A generated module answers for the contract it was generated from, so a
            // name taken from one reaches the matching definition in the proto file.
            if let Some(contract) = from.rsplit('/').next().and_then(|last| {
                last.strip_suffix("_pb")
                    .or_else(|| last.strip_suffix(".pb"))
            }) {
                let ending = format!("/{contract}.proto");
                for (index, def) in defs.iter().enumerate() {
                    if def.name == *name && def.file.to_string_lossy().ends_with(&ending) {
                        edges.push(Edge {
                            def: index,
                            file: file.clone(),
                            line: *line,
                            unit: repo.unit_of(file),
                        });
                    }
                }
            }
            let mut entries: Vec<PathBuf> = Vec::new();
            if from.starts_with('#') {
                // A subpath alias reaches where the unit's own manifest says.
                let unit = repo.unit_of(file);
                if let Some(path) = routes.aliases.get(&unit).and_then(|map| map.get(from)) {
                    let folder = repo.units[unit].path.clone();
                    entries.extend(existing(&normalized(&folder.join(path)), &present));
                }
            } else if from.starts_with('.') {
                let base = file.parent().unwrap_or(Path::new(""));
                entries.extend(existing(&normalized(&base.join(from)), &present));
            } else if let Some(entry) = routes.names.get(from) {
                // A workspace package, entered where its manifest's exports say.
                entries.push(entry.clone());
            }
            for entry in entries {
                for index in named_through(&entry, name, defs, &barrels, mine) {
                    edges.push(Edge {
                        def: index,
                        file: file.clone(),
                        line: *line,
                        unit: repo.unit_of(file),
                    });
                }
            }
        }
        edges
    }
}

// One import statement read to its parts; the byte span allows rewriting.
struct ImportShape {
    start: usize,
    end: usize,
    row: usize,
    type_only: bool,
    module: String,
    default: Option<String>,
    namespace: Option<String>,
    named: Vec<String>,
}

fn import_shapes(tree: &Tree, source: &str) -> Vec<ImportShape> {
    let mut shapes = Vec::new();
    let root = tree.root_node();
    let mut cursor = root.walk();
    for node in root.named_children(&mut cursor) {
        if node.kind() != "import_statement" {
            continue;
        }
        let mut shape = ImportShape {
            start: node.start_byte(),
            end: node.end_byte(),
            row: node.start_position().row + 1,
            type_only: source[node.start_byte()..node.end_byte()].starts_with("import type "),
            module: String::new(),
            default: None,
            namespace: None,
            named: Vec::new(),
        };
        let mut children = node.walk();
        for child in node.named_children(&mut children) {
            match child.kind() {
                "import_clause" => {
                    let mut parts = child.walk();
                    for part in child.named_children(&mut parts) {
                        let written = source[part.start_byte()..part.end_byte()].to_string();
                        match part.kind() {
                            "identifier" => shape.default = Some(written),
                            "namespace_import" => shape.namespace = Some(written),
                            "named_imports" => {
                                let mut specs = part.walk();
                                for spec in part.named_children(&mut specs) {
                                    if spec.kind() == "import_specifier" {
                                        shape.named.push(
                                            source[spec.start_byte()..spec.end_byte()].to_string(),
                                        );
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
                "string" => {
                    let mut parts = child.walk();
                    for part in child.named_children(&mut parts) {
                        if part.kind() == "string_fragment" {
                            shape.module = source[part.start_byte()..part.end_byte()].to_string();
                        }
                    }
                }
                _ => {}
            }
        }
        shapes.push(shape);
    }
    shapes
}

// Splits each crowded statement where it stands, never reordering: the order
// of side effects is the file's own business.
fn split_imports(source: &str, shapes: &[ImportShape]) -> Option<String> {
    let mut text = source.to_string();
    let mut changed = false;
    for shape in shapes.iter().rev() {
        let names = usize::from(shape.default.is_some())
            + usize::from(shape.namespace.is_some())
            + shape.named.len();
        if names <= 1 {
            continue;
        }
        let head = if shape.type_only {
            "import type"
        } else {
            "import"
        };
        let mut lines = Vec::new();
        if let Some(default) = &shape.default {
            lines.push(format!("{head} {default} from \"{}\";", shape.module));
        }
        if let Some(namespace) = &shape.namespace {
            lines.push(format!("{head} {namespace} from \"{}\";", shape.module));
        }
        for written in &shape.named {
            let (form, spec) = match written.strip_prefix("type ") {
                Some(rest) if !shape.type_only => ("import type", rest),
                _ => (head, written.as_str()),
            };
            lines.push(format!("{form} {{ {spec} }} from \"{}\";", shape.module));
        }
        text.replace_range(shape.start..shape.end, &lines.join("\n"));
        changed = true;
    }
    changed.then_some(text)
}

fn language() -> Language {
    tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()
}

fn definitions(tree: &Tree, source: &str) -> Vec<(String, usize)> {
    trees::texts(&language(), DEFINITIONS, tree, source, "def")
}

fn mentions(tree: &Tree, source: &str) -> Vec<(String, usize)> {
    trees::texts(&language(), MENTIONS, tree, source, "mention")
}

fn imports(tree: &Tree, source: &str) -> Vec<(String, String, usize, bool)> {
    trees::matches(&language(), USES, tree, source)
        .into_iter()
        .filter_map(|captures| {
            // A dynamic literal import and a namespace import take names only
            // run time resolves: the star stands for every name the module offers.
            if let Some(dynamic) = captures.iter().find(|capture| capture.name == "dynamic") {
                return Some((dynamic.text.clone(), "*".to_string(), dynamic.line, false));
            }
            let typed = captures.iter().any(|capture| capture.name == "typemark");
            if captures.iter().any(|capture| capture.name == "namespace") {
                let from = captures.iter().find(|capture| capture.name == "source")?;
                return Some((from.text.clone(), "*".to_string(), from.line, typed));
            }
            // A default import binds no exported name, and "default" is a word
            // no export can carry, so it resolves to nothing by construction.
            if captures.iter().any(|capture| capture.name == "default") {
                let from = captures.iter().find(|capture| capture.name == "source")?;
                return Some((from.text.clone(), "default".to_string(), from.line, typed));
            }
            // A bare import binds nothing at all; the empty name matches no definition.
            if let Some(bare) = captures.iter().find(|capture| capture.name == "bare") {
                return Some((bare.text.clone(), String::new(), bare.line, false));
            }
            let from = captures.iter().find(|capture| capture.name == "source")?;
            let name = captures.iter().find(|capture| capture.name == "name")?;
            Some((from.text.clone(), name.text.clone(), name.line, typed))
        })
        .collect()
}

const DEPENDENCY_SECTIONS: [&str; 4] = [
    "dependencies",
    "devDependencies",
    "peerDependencies",
    "optionalDependencies",
];
const OUTSIDE_CHANNELS: [&str; 7] = [
    "git+", "git:", "github:", "http:", "https:", "file:", "link:",
];

// The registry and the workspace are the only channels: a git, URL or file
// source slips past the lockfile's hashes and past the split-version rule.
fn dependency_findings(parsed: &serde_json::Value) -> Vec<String> {
    let mut findings = Vec::new();
    for section in DEPENDENCY_SECTIONS {
        let Some(table) = parsed.get(section).and_then(serde_json::Value::as_object) else {
            continue;
        };
        for (name, version) in table {
            let spec = version.as_str().unwrap_or("");
            if OUTSIDE_CHANNELS
                .iter()
                .any(|channel| spec.starts_with(channel))
            {
                findings.push(format!(
                    "{section}.{name} is \"{spec}\"; \
                     a dependency comes from the locked registry or the workspace"
                ));
            }
        }
    }
    findings
}

// The installed packages, one folder each; a scope folder holds its own children.
fn package_dirs(modules: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(modules) else {
        return Vec::new();
    };
    let mut dirs = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        if name.starts_with('@') {
            if let Ok(scoped) = std::fs::read_dir(entry.path()) {
                dirs.extend(scoped.flatten().map(|child| child.path()));
            }
        } else {
            dirs.push(entry.path());
        }
    }
    dirs
}

// A manifest's license: the SPDX string, or the older object form's type field.
fn manifest_license(parsed: &serde_json::Value) -> (Option<String>, String) {
    let name = parsed
        .get("name")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    let license = parsed
        .get("license")
        .and_then(|value| {
            value.as_str().map(str::to_string).or_else(|| {
                value
                    .get("type")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_string)
            })
        })
        .unwrap_or_else(|| "unstated".to_string());
    (name, license)
}

fn option_findings(options: &serde_json::Value) -> Vec<String> {
    let bar = BAR.parse::<toml::Value>().expect("a pack's bar parses");
    let Some(required) = bar.get("compilerOptions").and_then(toml::Value::as_table) else {
        return Vec::new();
    };
    let mut findings = Vec::new();
    for (name, needed) in required {
        let actual = options.get(name);
        if actual.is_some_and(|value| meets(value, needed)) {
            continue;
        }
        let found = match actual {
            None => "unset".to_string(),
            Some(value) => value.to_string(),
        };
        findings.push(format!(
            "compilerOptions.{name} is {found}; the bar says {needed}"
        ));
    }
    findings
}

// One unit's declarations, the tables that ship kept apart from the one that
// only develops: a test import may lean on either, shipped code on the first alone.
#[derive(Default)]
struct Declared {
    manifest: PathBuf,
    own: Option<String>,
    shipped: std::collections::HashSet<String>,
    dev: std::collections::HashSet<String>,
    // A peer is the consumer's to provide: a contract, never an own use.
    peers: std::collections::HashSet<String>,
    // Every word of every script: a package run as a command is reached.
    scripts: std::collections::HashSet<String>,
}

// The package a module specifier names: the first segment, two under a scope.
fn packaged(from: &str) -> String {
    if from.starts_with('@') {
        from.splitn(3, '/').take(2).collect::<Vec<&str>>().join("/")
    } else {
        from.split('/').next().unwrap_or(from).to_string()
    }
}

// Files the walk never judges whose imports still reach: scripts in .mjs,
// .cjs and .js, and code under dot folders such as a storybook's.
fn companions(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![(root.to_path_buf(), false)];
    while let Some((folder, hidden)) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&folder) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            let name = entry.file_name().to_string_lossy().to_string();
            if kind.is_dir() {
                if name == ".git"
                    || name == ".arbiter"
                    || name == "node_modules"
                    || name == "dist"
                    || name == "third_party"
                    || name.starts_with("bazel-")
                {
                    continue;
                }
                stack.push((entry.path(), hidden || name.starts_with('.')));
            } else if kind.is_file() {
                let scripted =
                    name.ends_with(".mjs") || name.ends_with(".cjs") || name.ends_with(".js");
                let shaded = hidden
                    && !name.ends_with(".d.ts")
                    && (name.ends_with(".ts") || name.ends_with(".tsx"));
                if scripted || shaded {
                    if let Ok(relative) = entry.path().strip_prefix(root) {
                        found.push(relative.to_path_buf());
                    }
                }
            }
        }
    }
    found
}

fn declared_packages(root: &Path, repo: &Repo) -> std::collections::HashMap<usize, Declared> {
    let mut declared: std::collections::HashMap<usize, Declared> = std::collections::HashMap::new();
    for manifest in &repo.manifests {
        if manifest
            .file_name()
            .is_none_or(|name| name != "package.json")
        {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(root.join(manifest)) else {
            continue;
        };
        let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&text) else {
            continue;
        };
        let entry = declared.entry(repo.unit_of(manifest)).or_default();
        entry.manifest = manifest.clone();
        if let Some(name) = parsed.get("name").and_then(serde_json::Value::as_str) {
            entry.own = Some(name.to_string());
        }
        for table in ["dependencies", "peerDependencies", "optionalDependencies"] {
            let Some(names) = parsed.get(table).and_then(serde_json::Value::as_object) else {
                continue;
            };
            for name in names.keys() {
                entry.shipped.insert(name.clone());
                entry.dev.insert(name.clone());
                if table == "peerDependencies" {
                    entry.peers.insert(name.clone());
                }
            }
        }
        if let Some(scripts) = parsed.get("scripts").and_then(serde_json::Value::as_object) {
            for script in scripts.values().filter_map(serde_json::Value::as_str) {
                for word in script.split_whitespace() {
                    entry.scripts.insert(word.to_string());
                }
            }
        }
        if let Some(names) = parsed
            .get("devDependencies")
            .and_then(serde_json::Value::as_object)
        {
            for name in names.keys() {
                entry.dev.insert(name.clone());
            }
        }
    }
    declared
}

// Whether a comment suppresses, and whether it quiets more than one named rule.
fn suppression_of(text: &str) -> Option<bool> {
    if text.contains("biome-ignore") || text.contains("@ts-expect-error") {
        return Some(false);
    }
    if text.contains("@ts-ignore") || text.contains("@ts-nocheck") {
        return Some(true);
    }
    None
}

fn meets(actual: &serde_json::Value, needed: &toml::Value) -> bool {
    match (actual, needed) {
        (serde_json::Value::Bool(found), toml::Value::Boolean(asked)) => found == asked,
        (serde_json::Value::String(found), toml::Value::String(asked)) => found == asked,
        _ => false,
    }
}

struct Routes {
    names: std::collections::HashMap<String, PathBuf>,
    // Wildcard routes kept as written, the star standing for one span of a specifier.
    patterns: Vec<String>,
    aliases: std::collections::HashMap<usize, std::collections::HashMap<String, String>>,
    packages: std::collections::HashSet<String>,
    // Every route name a manifest writes, kept even when no repository file answers
    // it, since a route pointing at built output is declared all the same.
    declared: std::collections::HashSet<String>,
    // Every file an exports route names: the declared surface of its unit. An
    // imports alias is inside routing and blesses nothing.
    surfaces: std::collections::HashSet<PathBuf>,
    // The surfaces a manifest wrote a route for, as opposed to the imputed
    // src/index fallback: only a written route is the user's declaration to judge.
    written: std::collections::HashSet<PathBuf>,
}

// Where each workspace package enters and where each unit's subpath aliases lead,
// read from the package.json manifests.
fn manifest_routes(
    root: &Path,
    repo: &Repo,
    present: &std::collections::HashSet<&PathBuf>,
) -> Routes {
    let mut routes = Routes {
        names: std::collections::HashMap::new(),
        patterns: Vec::new(),
        aliases: std::collections::HashMap::new(),
        packages: std::collections::HashSet::new(),
        declared: std::collections::HashSet::new(),
        surfaces: std::collections::HashSet::new(),
        written: std::collections::HashSet::new(),
    };
    for manifest in &repo.manifests {
        if manifest
            .file_name()
            .is_none_or(|name| name != "package.json")
        {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(root.join(manifest)) else {
            continue;
        };
        let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&text) else {
            continue;
        };
        let folder = manifest.parent().unwrap_or(Path::new("")).to_path_buf();
        if let Some(name) = parsed.get("name").and_then(serde_json::Value::as_str) {
            routes.packages.insert(name.to_string());
            for (route, path) in exported(name, &parsed) {
                if route.contains('*') {
                    routes.patterns.push(route);
                    continue;
                }
                routes.declared.insert(route.clone());
                // A route whose conditions all point at built output falls back to the
                // src file the output is built from, the way the bare name falls back
                // to src/index.
                let fallback = match route
                    .strip_prefix(name)
                    .and_then(|rest| rest.strip_prefix('/'))
                {
                    Some(sub) => format!("src/{sub}"),
                    None => "src/index".to_string(),
                };
                let entry = existing(&normalized(&folder.join(&path)), present)
                    .or_else(|| existing(&normalized(&folder.join(fallback)), present));
                if let Some(entry) = entry {
                    routes.surfaces.insert(entry.clone());
                    routes.written.insert(entry.clone());
                    routes.names.insert(route, entry);
                }
            }
            if !routes.names.contains_key(name) {
                if let Some(entry) = existing(&normalized(&folder.join("src/index")), present) {
                    routes.surfaces.insert(entry.clone());
                    routes.names.insert(name.to_string(), entry);
                }
            }
        }
        if let Some(imports) = parsed.get("imports").and_then(serde_json::Value::as_object) {
            let unit = repo.unit_of(manifest);
            let map = routes.aliases.entry(unit).or_default();
            for (alias, target) in imports {
                if let Some(path) = target.as_str() {
                    map.insert(alias.clone(), path.to_string());
                }
            }
        }
    }
    routes
}

// pnpm writes YAML: under `packages:`, each key is name@version, quoted when
// scoped, any peer qualifiers in parentheses; older locks prefix a slash.
fn pnpm_locked(text: &str) -> Vec<(String, String)> {
    let mut found = Vec::new();
    let mut inside = false;
    for line in text.lines() {
        if !line.starts_with(' ') && !line.trim().is_empty() {
            inside = line.trim_end() == "packages:";
            continue;
        }
        if !inside || !line.starts_with("  ") || line.starts_with("   ") {
            continue;
        }
        let Some(key) = line.trim().strip_suffix(':') else {
            continue;
        };
        let key = key.trim_matches(['\'', '"']).trim_start_matches('/');
        let key = key.split('(').next().unwrap_or(key);
        let Some(at) = key.rfind('@') else {
            continue;
        };
        if at == 0 {
            continue;
        }
        found.push((key[..at].to_string(), key[at + 1..].to_string()));
    }
    found
}

// npm writes JSON: under "packages", each key is an install path whose last
// node_modules/ segment names the package; a workspace link carries no version.
// A copy nested inside another package's node_modules is that party's own and
// unreachable from any unit, so only single-segment paths count as resolved.
fn npm_locked(text: &str) -> Vec<(String, String)> {
    let Ok(parsed) = serde_json::from_str::<serde_json::Value>(text) else {
        return Vec::new();
    };
    let Some(packages) = parsed
        .get("packages")
        .and_then(serde_json::Value::as_object)
    else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for (key, entry) in packages {
        if key.matches("node_modules/").count() != 1 {
            continue;
        }
        let Some(position) = key.rfind("node_modules/") else {
            continue;
        };
        if entry
            .get("link")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
        {
            continue;
        }
        let Some(version) = entry.get("version").and_then(serde_json::Value::as_str) else {
            continue;
        };
        let name = &key[position + "node_modules/".len()..];
        found.push((name.to_string(), version.to_string()));
    }
    found
}

// A toolchain's config file runs at build time and ships nothing.
fn developmental(file: &Path) -> bool {
    file.file_stem()
        .map(|stem| stem.to_string_lossy().to_string())
        .unwrap_or_default()
        .ends_with(".config")
}

// What each surface file defines of its own, a named re-export counting as
// nothing: an empty answer marks the surface pure.
fn surface_definitions(
    root: &Path,
    surfaces: &std::collections::HashSet<PathBuf>,
) -> std::collections::HashMap<PathBuf, Vec<(String, usize)>> {
    let mut found = std::collections::HashMap::new();
    for file in surfaces {
        let Ok(source) = std::fs::read_to_string(root.join(file)) else {
            continue;
        };
        let Some(tree) = trees::parse(&language(), &source) else {
            continue;
        };
        let owned = definitions(&tree, &source);
        if !owned.is_empty() {
            found.insert(file.clone(), owned);
        }
    }
    found
}

// Every route the manifest exports with the file it enters, the source condition
// first, since that is the file in the repository.
fn exported(name: &str, parsed: &serde_json::Value) -> Vec<(String, String)> {
    let mut found = Vec::new();
    if let Some(exports) = parsed.get("exports") {
        if let Some(map) = exports.as_object() {
            if map.keys().any(|key| key.starts_with('.')) {
                for (key, value) in map {
                    let Some(path) = condition_source(value) else {
                        continue;
                    };
                    let route = if key == "." {
                        name.to_string()
                    } else {
                        format!("{name}/{}", key.trim_start_matches("./"))
                    };
                    found.push((route, path));
                }
                return found;
            }
        }
        if let Some(path) = condition_source(exports) {
            found.push((name.to_string(), path));
            return found;
        }
    }
    if let Some(main) = parsed.get("main").and_then(serde_json::Value::as_str) {
        found.push((name.to_string(), main.to_string()));
    }
    found
}

fn condition_source(value: &serde_json::Value) -> Option<String> {
    if let Some(path) = value.as_str() {
        return Some(path.to_string());
    }
    for condition in ["source", "import", "default"] {
        if let Some(path) = value.get(condition).and_then(serde_json::Value::as_str) {
            return Some(path.to_string());
        }
    }
    None
}

// Whether a specifier walks a wildcard route, the star standing for one span.
fn routed(from: &str, pattern: &str) -> bool {
    match pattern.split_once('*') {
        Some((head, tail)) => {
            from.len() >= head.len() + tail.len() && from.starts_with(head) && from.ends_with(tail)
        }
        None => from == pattern,
    }
}

fn existing(path: &Path, present: &std::collections::HashSet<&PathBuf>) -> Option<PathBuf> {
    let shown = path.to_string_lossy();
    for candidate in [
        shown.to_string(),
        format!("{shown}.ts"),
        format!("{shown}.tsx"),
        format!("{shown}/index.ts"),
        format!("{shown}/index.tsx"),
    ] {
        let candidate = PathBuf::from(candidate);
        if present.contains(&candidate) {
            return Some(candidate);
        }
    }
    None
}

// The definitions a name reaches from an entry, passing through every barrel.
fn named_through(
    entry: &Path,
    name: &str,
    defs: &[Def],
    barrels: &std::collections::HashMap<PathBuf, Vec<PathBuf>>,
    mine: usize,
) -> Vec<usize> {
    let mut queue = vec![entry.to_path_buf()];
    let mut seen = std::collections::HashSet::new();
    let mut found = Vec::new();
    while let Some(file) = queue.pop() {
        if !seen.insert(file.clone()) {
            continue;
        }
        for (index, def) in defs.iter().enumerate() {
            if def.pack == mine && (name == "*" || def.name == name) && def.file == file {
                found.push(index);
            }
        }
        if let Some(next) = barrels.get(&file) {
            queue.extend(next.iter().cloned());
        }
    }
    found
}

fn normalized(path: &Path) -> PathBuf {
    let mut parts: Vec<&OsStr> = Vec::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                parts.pop();
            }
            Component::Normal(part) => parts.push(part),
            _ => {}
        }
    }
    parts.iter().collect()
}

// The vitest grammar of behavior: describe frames a path, it and test claim a
// sentence; a describe body is walked inside its own frame.
fn claimed(node: tree_sitter::Node, source: &str, path: &mut Vec<String>, found: &mut Vec<Claim>) {
    if node.kind() == "call_expression" {
        let (name, mark) = test_callee(node, source);
        if matches!(name, "describe" | "it" | "test" | "bench") {
            // A skipped or todo claim proves nothing, so it never enters the tree.
            if matches!(mark, "skip" | "todo") {
                return;
            }
            if let Some(sentence) = first_sentence(node, source) {
                if name == "describe" {
                    path.push(sentence);
                    let mut walker = node.walk();
                    for child in node.children(&mut walker) {
                        claimed(child, source, path, found);
                    }
                    path.pop();
                } else {
                    found.push(Claim {
                        path: path.clone(),
                        sentence,
                        line: node.start_position().row + 1,
                    });
                }
                return;
            }
        }
    }
    let mut walker = node.walk();
    for child in node.children(&mut walker) {
        claimed(child, source, path, found);
    }
}

fn first_sentence(call: tree_sitter::Node, source: &str) -> Option<String> {
    let arguments = call.child_by_field_name("arguments")?;
    let mut walker = arguments.walk();
    for argument in arguments.named_children(&mut walker) {
        if argument.kind() == "string" || argument.kind() == "template_string" {
            let text = argument.utf8_text(source.as_bytes()).ok()?;
            return Some(text.trim_matches(['"', '\'', '`']).to_string());
        }
        // A naming helper still names by a literal: the sentence rides one
        // call deep, as test(its("...")) does.
        if argument.kind() == "call_expression" {
            if let Some(inner) = argument.child_by_field_name("arguments") {
                let mut deeper = inner.walk();
                for part in inner.named_children(&mut deeper) {
                    if part.kind() == "string" || part.kind() == "template_string" {
                        let text = part.utf8_text(source.as_bytes()).ok()?;
                        return Some(text.trim_matches(['"', '\'', '`']).to_string());
                    }
                }
            }
        }
    }
    None
}

// A skipped, focused or postponed test is a suppression the ledger counts:
// skip and todo silence the claim, only silences every sibling.
fn marked(node: tree_sitter::Node, source: &str) -> Vec<(String, usize)> {
    let mut found = Vec::new();
    let mut stack = vec![node];
    while let Some(node) = stack.pop() {
        if node.kind() == "member_expression" {
            let object = node
                .child_by_field_name("object")
                .filter(|object| object.kind() == "identifier")
                .and_then(|object| object.utf8_text(source.as_bytes()).ok())
                .unwrap_or("");
            let property = node
                .child_by_field_name("property")
                .and_then(|property| property.utf8_text(source.as_bytes()).ok())
                .unwrap_or("");
            if matches!(object, "describe" | "it" | "test" | "bench")
                && matches!(property, "skip" | "todo" | "only")
            {
                found.push((
                    format!("{object}.{property}"),
                    node.start_position().row + 1,
                ));
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

// expect(a).toBe(a) with both sides one literal or one name: always true,
// asserting nothing. NaN is the lone exception and no test leans on it.
fn tautologies(node: tree_sitter::Node, source: &str) -> Vec<(String, usize)> {
    let mut found = Vec::new();
    let mut stack = vec![node];
    while let Some(node) = stack.pop() {
        if node.kind() == "call_expression" {
            if let Some((left, right)) = asserted_pair(node, source) {
                if left == right {
                    found.push((left, node.start_position().row + 1));
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

fn asserted_pair(call: tree_sitter::Node, source: &str) -> Option<(String, String)> {
    let callee = call.child_by_field_name("function")?;
    if callee.kind() != "member_expression" {
        return None;
    }
    let property = callee
        .child_by_field_name("property")?
        .utf8_text(source.as_bytes())
        .ok()?;
    if !matches!(property, "toBe" | "toEqual" | "toStrictEqual") {
        return None;
    }
    let object = callee.child_by_field_name("object")?;
    if object.kind() != "call_expression"
        || object
            .child_by_field_name("function")?
            .utf8_text(source.as_bytes())
            .ok()?
            != "expect"
    {
        return None;
    }
    let expected = simple_argument(object, source)?;
    let compared = simple_argument(call, source)?;
    Some((expected, compared))
}

// The sole argument when it is a bare literal or name; anything built is not judged.
fn simple_argument(call: tree_sitter::Node, source: &str) -> Option<String> {
    let arguments = call.child_by_field_name("arguments")?;
    let mut walker = arguments.walk();
    let named: Vec<tree_sitter::Node> = arguments.named_children(&mut walker).collect();
    let [argument] = named[..] else {
        return None;
    };
    if !matches!(
        argument.kind(),
        "identifier" | "number" | "string" | "true" | "false"
    ) {
        return None;
    }
    Some(argument.utf8_text(source.as_bytes()).ok()?.to_string())
}

// The test vocabulary a call speaks, through a member or an each table: the
// caller's name and the property marking it, if any.
fn test_callee<'tree>(
    node: tree_sitter::Node<'tree>,
    source: &'tree str,
) -> (&'tree str, &'tree str) {
    let Some(callee) = node.child_by_field_name("function") else {
        return ("", "");
    };
    match callee.kind() {
        "identifier" => (callee.utf8_text(source.as_bytes()).unwrap_or(""), ""),
        "member_expression" => member_parts(callee, source),
        // it.each(table)("sentence", run): the sentence rides the outer call.
        "call_expression" => match callee.child_by_field_name("function") {
            Some(inner) if inner.kind() == "member_expression" => {
                let (name, mark) = member_parts(inner, source);
                if mark == "each" {
                    (name, "each")
                } else {
                    ("", "")
                }
            }
            _ => ("", ""),
        },
        _ => ("", ""),
    }
}

fn member_parts<'tree>(
    member: tree_sitter::Node<'tree>,
    source: &'tree str,
) -> (&'tree str, &'tree str) {
    let name = member
        .child_by_field_name("object")
        .filter(|object| object.kind() == "identifier")
        .and_then(|object| object.utf8_text(source.as_bytes()).ok())
        .unwrap_or("");
    let mark = member
        .child_by_field_name("property")
        .and_then(|property| property.utf8_text(source.as_bytes()).ok())
        .unwrap_or("");
    (name, mark)
}

// Test files by their names alone, the way the repository tells them.
fn testish(file: &Path) -> bool {
    let stem = file
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_string())
        .unwrap_or_default();
    stem.starts_with("test_")
        || stem.ends_with("_test")
        || stem.ends_with(".test")
        || file
            .components()
            .any(|part| part.as_os_str() == "tests" || part.as_os_str() == "__tests__")
}

// A test call whose sentence no literal provides is a stray: it runs, yet the
// tree cannot speak for it, so no need can bind it.
fn strays(node: tree_sitter::Node, source: &str) -> Vec<usize> {
    let mut found = Vec::new();
    let mut stack = vec![node];
    while let Some(node) = stack.pop() {
        if node.kind() == "call_expression" {
            // bench stays out: a fluent builder of that name is not a test form.
            let (name, mark) = test_callee(node, source);
            if matches!(name, "describe" | "it" | "test")
                && !matches!(mark, "skip" | "todo")
                && first_sentence(node, source).is_none()
            {
                found.push(node.start_position().row + 1);
            }
        }
        let mut walker = node.walk();
        for child in node.children(&mut walker) {
            stack.push(child);
        }
    }
    found.sort_unstable();
    found
}

// Collects the door shapes: interfaces, each class's named heritage,
// annotated parameters, and the constructions standing at annotated sites.
fn doors(node: tree_sitter::Node, source: &str, file: &Path, pack: &mut TypeScript) {
    let text = |at: tree_sitter::Node| source[at.byte_range()].to_string();
    match node.kind() {
        "interface_declaration" => {
            if let Some(name) = node.child_by_field_name("name") {
                pack.interfaces.push(text(name));
            }
        }
        "class_declaration" => {
            if let Some(name) = node.child_by_field_name("name") {
                let mut bases = Vec::new();
                let mut stack = vec![node];
                while let Some(at) = stack.pop() {
                    match at.kind() {
                        "extends_clause" | "implements_clause" => {
                            for index in 0..at.named_child_count() {
                                if let Some(base) = at.named_child(index) {
                                    let whole = text(base);
                                    let first = whole.split('<').next().unwrap_or(&whole);
                                    bases.push(
                                        first.rsplit('.').next().unwrap_or(first).to_string(),
                                    );
                                }
                            }
                        }
                        "class_body" => {}
                        _ => {
                            for index in 0..at.named_child_count() {
                                if let Some(child) = at.named_child(index) {
                                    if child.kind() != "class_body" {
                                        stack.push(child);
                                    }
                                }
                            }
                        }
                    }
                }
                pack.classes.push((
                    file.to_path_buf(),
                    node.start_position().row + 1,
                    text(name),
                    bases,
                ));
            }
        }
        "function_declaration" => {
            if let (Some(name), Some(parameters)) = (
                node.child_by_field_name("name"),
                node.child_by_field_name("parameters"),
            ) {
                let mut place = 0;
                for index in 0..parameters.named_child_count() {
                    let Some(parameter) = parameters.named_child(index) else {
                        continue;
                    };
                    if let Some(annotation) = parameter.child_by_field_name("type") {
                        let names = annotation_names(annotation, source);
                        if !names.is_empty() {
                            pack.askings.push((text(name), place, names));
                        }
                    }
                    place += 1;
                }
            }
        }
        "variable_declarator" => {
            if let (Some(annotation), Some(value)) = (
                node.child_by_field_name("type"),
                node.child_by_field_name("value"),
            ) {
                if let Some(construction) = constructed_ts(value, source) {
                    pack.assigns.push((
                        file.to_path_buf(),
                        node.start_position().row + 1,
                        annotation_names(annotation, source),
                        construction,
                    ));
                }
            }
        }
        "call_expression" => {
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
                        if let Some(construction) = constructed_ts(argument, source) {
                            pack.callargs.push((
                                file.to_path_buf(),
                                argument.start_position().row + 1,
                                callee.clone(),
                                place,
                                construction,
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

// A direct construction: a new expression over a bare name.
fn constructed_ts(node: tree_sitter::Node, source: &str) -> Option<String> {
    if node.kind() != "new_expression" {
        return None;
    }
    let constructor = node.child_by_field_name("constructor")?;
    if constructor.kind() != "identifier" {
        return None;
    }
    Some(source[constructor.byte_range()].to_string())
}

// Every type identifier an annotation names, so a union or an array element
// still speaks each of its doors.
fn annotation_names(node: tree_sitter::Node, source: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut stack = vec![node];
    while let Some(at) = stack.pop() {
        if at.kind() == "type_identifier" {
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
    use std::path::PathBuf;

    use super::dependency_findings;
    use super::manifest_license;
    use super::normalized;
    use super::option_findings;
    use super::routed;

    #[test]
    fn a_license_is_the_string_or_the_object_form_or_unstated() {
        let spdx = serde_json::json!({"name": "a", "license": "MIT"});
        let object = serde_json::json!({"name": "b", "license": {"type": "ISC"}});
        let silent = serde_json::json!({"name": "c"});
        assert_eq!(
            manifest_license(&spdx),
            (Some("a".to_string()), "MIT".to_string())
        );
        assert_eq!(
            manifest_license(&object),
            (Some("b".to_string()), "ISC".to_string())
        );
        assert_eq!(
            manifest_license(&silent),
            (Some("c".to_string()), "unstated".to_string())
        );
    }

    #[test]
    fn a_dependency_outside_the_registry_or_workspace_is_named() {
        let manifest = serde_json::json!({
            "dependencies": {
                "lodash": "^4.17.21",
                "ui": "workspace:*",
                "forked": "github:x/forked",
                "grabbed": "https://example.com/pkg.tgz"
            },
            "devDependencies": { "linked": "link:../elsewhere" }
        });
        let findings = dependency_findings(&manifest);
        assert!(findings
            .iter()
            .any(|finding| finding.starts_with("dependencies.forked is \"github:x/forked\";")));
        assert!(findings
            .iter()
            .any(|finding| finding.contains("dependencies.grabbed")));
        assert!(findings
            .iter()
            .any(|finding| finding.contains("devDependencies.linked")));
        assert_eq!(findings.len(), 3);
    }

    #[test]
    fn the_bar_names_each_unset_option_and_leaves_a_met_one() {
        let options = serde_json::json!({ "strict": true });
        let findings = option_findings(&options);
        assert!(findings
            .iter()
            .any(|finding| finding.contains("noUnusedLocals is unset")));
        assert!(!findings.iter().any(|finding| finding.contains("strict is")));
    }

    #[test]
    fn a_lowered_option_is_named_with_its_value() {
        let options = serde_json::json!({ "allowUnreachableCode": true });
        assert!(option_findings(&options)
            .iter()
            .any(|finding| finding
                == "compilerOptions.allowUnreachableCode is true; the bar says false"));
    }

    #[test]
    fn a_pnpm_lock_yields_each_package_with_peers_stripped() {
        let lock = "lockfileVersion: '9.0'\n\nimporters:\n  .:\n    dependencies:\n      \
                    lodash:\n        specifier: ^4.0.0\n        version: 4.17.21\n\n\
                    packages:\n\n  '@scope/kit@2.0.1':\n    resolution: {integrity: sha512-a}\n\n  \
                    lodash@4.17.21:\n    resolution: {integrity: sha512-b}\n\n  \
                    react-dom@18.2.0(react@18.2.0):\n    resolution: {integrity: sha512-c}\n";
        assert_eq!(
            super::pnpm_locked(lock),
            [
                ("@scope/kit".to_string(), "2.0.1".to_string()),
                ("lodash".to_string(), "4.17.21".to_string()),
                ("react-dom".to_string(), "18.2.0".to_string()),
            ]
        );
    }

    #[test]
    fn a_wildcard_route_admits_only_what_it_spans() {
        assert!(routed("@ui/icons/arrow", "@ui/icons/*"));
        assert!(!routed("@ui/parts/arrow", "@ui/icons/*"));
        assert!(!routed("@ui/icons", "@ui/icons/*"));
        assert!(routed("@ui/test", "@ui/test"));
    }

    #[test]
    fn dots_fold_away_and_parents_climb() {
        assert_eq!(
            normalized(Path::new("web/src/./sib")),
            PathBuf::from("web/src/sib")
        );
        assert_eq!(
            normalized(Path::new("web/src/../lib/x")),
            PathBuf::from("web/lib/x")
        );
    }
    #[test]
    fn an_npm_lock_keeps_what_units_reach_and_skips_links_and_nested_copies() {
        let lock = r#"{
          "packages": {
            "": { "name": "web", "version": "0.0.0" },
            "node_modules/@demo/ui": { "resolved": "../ui", "link": true },
            "node_modules/lodash": { "version": "4.17.21" },
            "apps/x/node_modules/lodash": { "version": "4.18.0" },
            "node_modules/a/node_modules/lodash": { "version": "4.17.20" }
          }
        }"#;
        assert_eq!(
            super::npm_locked(lock),
            [
                ("lodash".to_string(), "4.18.0".to_string()),
                ("lodash".to_string(), "4.17.21".to_string()),
            ]
        );
    }
}
