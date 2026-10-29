use std::ffi::OsStr;
use std::path::{Component, Path, PathBuf};

use tree_sitter::{Language, Tree};

use crate::core::pack::{pinned, Def, Definition, Edge, Finding, Pack, Suppression};
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
                "pinned-image",
                "never: the language names no container images",
            ),
            ("dynamic-import", "held"),
            ("no-reexport", "held"),
            ("self-entry", "held"),
            ("interior-import", "held"),
            ("no-hidden-names", "held"),
            ("one-import-per-line", "held"),
            ("no-comment-overreach", "held"),
            ("undeclared-dependency", "held"),
            ("suppressions", "held"),
            ("wiring", "held"),
            ("tools", "held"),
            ("no-warnings", "held"),
            ("container", "held"),
            ("generated", "held"),
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
            findings.push(Finding::integrity(file, *line, "no-reexport", reason));
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
            findings.push(Finding::integrity(file, *line, "no-reexport", reason));
        }
        findings.extend(self.aliases.iter().map(|(file, alias, line)| Finding {
            check: "integrity",
            file: file.clone(),
            line: *line,
            name: "no-hidden-names".to_string(),
            reason: format!("imported as {alias}; a symbol keeps its own name"),
        }));
        findings.extend(self.computed.iter().map(|(file, line)| {
            Finding {
                check: "integrity",
                file: file.clone(),
                line: *line,
                name: "no-hidden-names".to_string(),
                reason: "imports a module through a computed value; a module is named by a literal"
                    .to_string(),
            }
        }));
        findings.extend(self.crowded.iter().map(|(file, line, names)| Finding {
            check: "integrity",
            file: file.clone(),
            line: *line,
            name: "one-import-per-line".to_string(),
            reason: format!(
                "names {names} symbols in one import; an import line names one symbol and the module it comes from"
            ),
        }));
        findings.extend(self.overreaches.iter().map(|(file, token, line)| Finding {
            check: "integrity",
            file: file.clone(),
            line: *line,
            name: "no-comment-overreach".to_string(),
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
            if from.starts_with('.') || from.starts_with('#') || from.starts_with("node:") {
                continue;
            }
            let package = if from.starts_with('@') {
                from.splitn(3, '/').take(2).collect::<Vec<&str>>().join("/")
            } else {
                from.split('/').next().unwrap_or(from).to_string()
            };
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
// source slips past the lockfile's hashes and past the one-version rule.
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
    own: Option<String>,
    shipped: std::collections::HashSet<String>,
    dev: std::collections::HashSet<String>,
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

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{dependency_findings, manifest_license, normalized, option_findings, routed};

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
