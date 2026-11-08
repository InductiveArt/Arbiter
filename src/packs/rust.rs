use std::path::Path;
use std::path::PathBuf;

use tree_sitter::Language;
use tree_sitter::Node;
use tree_sitter::Tree;

use crate::core::pack::dial;
use crate::core::pack::Claim;
use crate::core::pack::Def;
use crate::core::pack::Definition;
use crate::core::pack::Edge;
use crate::core::pack::Finding;
use crate::core::pack::Pack;
use crate::core::pack::Suppression;
use crate::core::prose;
use crate::core::record;
use crate::core::trees;
use crate::core::units::Repo;

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
                "never: the proto pack judges any unit's buf.gen.yaml, whatever its language",
            ),
            (
                "inter-language-break",
                "never: the proto pack judges any unit's buf.gen.yaml, whatever its language",
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
        Some(include_str!("../../packs/rust/tools.toml"))
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

    fn claims(&self, source: &str) -> Vec<Claim> {
        let Some(tree) = trees::parse(&language(), source) else {
            return Vec::new();
        };
        let mut found = Vec::new();
        claimed(tree.root_node(), source, &mut Vec::new(), &mut found);
        found
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
            }
            self.uses.push((file.to_path_buf(), module, name, line));
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

    fn integrity(&self, root: &Path, repo: &Repo) -> Vec<Finding> {
        let mut findings: Vec<Finding> = self
            .reexports
            .iter()
            .map(|(file, argument, line)| Finding {
                check: "integrity",
                file: file.clone(),
                line: *line,
                name: "reexport".to_string(),
                reason: format!("re-exports {argument}; a module offers only its own names"),
            })
            .collect();
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
        findings.extend(self.crowded.iter().map(|(file, line, names)| Finding {
            check: "integrity",
            file: file.clone(),
            line: *line,
            name: "crowded-import".to_string(),
            reason: format!(
                "names {names} symbols in one import; \
                 an import line names one symbol and the module it comes from"
            ),
        }));
        findings.extend(self.aliases.iter().map(|(file, alias, line)| Finding {
            check: "integrity",
            file: file.clone(),
            line: *line,
            name: "hidden-name".to_string(),
            reason: format!("imported as {alias}; a symbol keeps its own name"),
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
        let declared = declared_crates(root, repo);
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
            if repo
                .files
                .iter()
                .any(|candidate| is_module_file(first, candidate))
            {
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
                    check: "integrity",
                    file: file.clone(),
                    line: *line,
                    name: "undeclared-dependency".to_string(),
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
                    check: "integrity",
                    file: entry.manifest.clone(),
                    line: 0,
                    name: "unused-dependency".to_string(),
                    reason: format!("{name} is declared, and nothing in the unit reaches it"),
                });
            }
        }
        findings.extend(record::stale(&subjects, "rust", &live));
        findings
    }

    fn resolve(&self, mine: usize, defs: &[Def], _root: &Path, repo: &Repo) -> Vec<Edge> {
        let mut edges = Vec::new();
        for (file, module, name, line) in &self.uses {
            let Some(segment) = module.rsplit("::").next() else {
                continue;
            };
            if segment == "self" || segment == "super" {
                continue;
            }
            for (index, def) in defs.iter().enumerate() {
                if def.pack != mine || def.name != *name {
                    continue;
                }
                if is_module_file(segment, &def.file) {
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

// Each unit's manifest with its dependency-table keys, dashed, which is how
// crates are published. Cargo already refuses a dev-dependency outside tests,
// so the tables stay one set; the crate's own name stays apart.
struct Crates {
    manifest: PathBuf,
    own: Option<String>,
    names: std::collections::HashSet<String>,
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
            for name in table.keys() {
                entry.names.insert(name.replace('_', "-"));
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

fn is_module_file(segment: &str, file: &Path) -> bool {
    let shown = file.to_string_lossy();
    // The crate's own root module answers for the crate keyword.
    let endings = if segment == "crate" {
        ["src/main.rs".to_string(), "src/lib.rs".to_string()]
    } else {
        [format!("{segment}.rs"), format!("{segment}/mod.rs")]
    };
    endings
        .iter()
        .any(|ending| shown == *ending || shown.ends_with(&format!("/{ending}")))
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
    trees::matches(&language(), USES, tree, source)
        .into_iter()
        .filter_map(|captures| {
            let module = captures
                .iter()
                .find(|capture| capture.name == "module" || capture.name == "declared_module")?;
            let name = captures.iter().find(|capture| capture.name == "name")?;
            Some((
                module.text.clone(),
                name.text.clone(),
                name.line,
                module.name == "declared_module",
            ))
        })
        .collect()
}

// Rust's grammar of behavior: a #[test] function claims its name as a
// sentence, framed by its modules; the conventional tests module frames nothing.
fn claimed(node: tree_sitter::Node, source: &str, path: &mut Vec<String>, found: &mut Vec<Claim>) {
    if node.kind() == "mod_item" {
        if let Some(name) = node.child_by_field_name("name") {
            let name = name.utf8_text(source.as_bytes()).unwrap_or("").to_string();
            let framed = name != "tests";
            if framed {
                path.push(name);
            }
            let mut walker = node.walk();
            for child in node.children(&mut walker) {
                claimed(child, source, path, found);
            }
            if framed {
                path.pop();
            }
            return;
        }
    }
    if node.kind() == "function_item" && attributed(node, source) {
        // An ignored test never runs, so it proves nothing.
        if ignored(node, source) {
            return;
        }
        if let Some(name) = node.child_by_field_name("name") {
            found.push(Claim {
                path: path.clone(),
                sentence: name
                    .utf8_text(source.as_bytes())
                    .unwrap_or("")
                    .replace('_', " "),
                line: node.start_position().row + 1,
            });
        }
    }
    let mut walker = node.walk();
    for child in node.children(&mut walker) {
        claimed(child, source, path, found);
    }
}

fn attributed(function: tree_sitter::Node, source: &str) -> bool {
    let mut before = function.prev_named_sibling();
    while let Some(node) = before {
        if node.kind() != "attribute_item" {
            return false;
        }
        if node
            .utf8_text(source.as_bytes())
            .unwrap_or("")
            .contains("test")
        {
            return true;
        }
        before = node.prev_named_sibling();
    }
    false
}

fn ignored(function: tree_sitter::Node, source: &str) -> bool {
    let mut before = function.prev_named_sibling();
    while let Some(node) = before {
        if node.kind() != "attribute_item" {
            return false;
        }
        if node
            .utf8_text(source.as_bytes())
            .unwrap_or("")
            .contains("#[ignore")
        {
            return true;
        }
        before = node.prev_named_sibling();
    }
    false
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

    use super::is_module_file;
    use super::manifest_findings;

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

    #[test]
    fn the_crate_keyword_reaches_either_crate_root() {
        assert!(is_module_file("crate", Path::new("src/main.rs")));
        assert!(is_module_file("crate", Path::new("src/lib.rs")));
        assert!(!is_module_file("crate", Path::new("src/pack.rs")));
    }

    #[test]
    fn a_segment_reaches_its_module_file_and_no_lookalike() {
        assert!(is_module_file("pack", Path::new("src/pack.rs")));
        assert!(is_module_file("pack", Path::new("src/pack/mod.rs")));
        assert!(!is_module_file("pack", Path::new("src/backpack.rs")));
    }
}
