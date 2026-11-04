use std::path::{Path, PathBuf};

use tree_sitter::{Language, Tree};

use crate::core::pack::{Claim, Def, Definition, Edge, Finding, Pack, Suppression};
use crate::core::prose;
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
            ("resolve", "held"),
            (
                "pinned-image",
                "never: the language names no container images",
            ),
            (
                "dynamic-import",
                "never: a crate links its dependencies; nothing imports at run time",
            ),
            ("no-reexport", "held"),
            (
                "self-entry",
                "never: a crate reaches its own items as crate::, never through its published name",
            ),
            (
                "interior-import",
                "never: pub already walls a crate's interior",
            ),
            ("no-hidden-names", "held"),
            ("no-comment-overreach", "held"),
            ("undeclared-dependency", "held"),
            ("suppressions", "held"),
            ("wiring", "held"),
            ("tools", "held"),
            ("no-warnings", "held"),
            ("tautology", "held"),
            (
                "drift",
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

    fn wiring(&self, folder: &Path) -> Option<Vec<String>> {
        let Ok(manifest) = std::fs::read_to_string(folder.join("Cargo.toml")) else {
            return Some(vec![
                "no Cargo.toml; the unit declares no environment".to_string()
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
                name: "no-reexport".to_string(),
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
        findings.extend(self.aliases.iter().map(|(file, alias, line)| Finding {
            check: "integrity",
            file: file.clone(),
            line: *line,
            name: "no-hidden-names".to_string(),
            reason: format!("imported as {alias}; a symbol keeps its own name"),
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
            if declared
                .get(&unit)
                .is_some_and(|names| names.contains(&first.replace('_', "-")))
            {
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

// Each unit's dependency-table keys, dashed, which is how crates are published.
// Cargo already refuses a dev-dependency outside tests, so the tables stay one set.
fn declared_crates(
    root: &Path,
    repo: &Repo,
) -> std::collections::HashMap<usize, std::collections::HashSet<String>> {
    let mut declared: std::collections::HashMap<usize, std::collections::HashSet<String>> =
        std::collections::HashMap::new();
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
        let entry = declared.entry(repo.unit_of(manifest)).or_default();
        // A crate's own name: its library half is imported under it from its binaries.
        if let Some(own) = table_at(&parsed, "package").and_then(|package| package.get("name")) {
            if let Some(own) = own.as_str() {
                entry.insert(own.replace('_', "-"));
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
                entry.insert(name.replace('_', "-"));
            }
        }
    }
    declared
}

// The bar denies every warning, so a lint turned to allow in the manifest lowers it.
fn manifest_findings(manifest: &str) -> Vec<String> {
    let Ok(parsed) = manifest.parse::<toml::Value>() else {
        return vec!["Cargo.toml does not parse".to_string()];
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
                    findings.push(format!(
                        "{path}.{name} is allow; the bar denies every warning"
                    ));
                }
            }
        }
    }
    // The registry and the repository are the only channels: a git source slips
    // past the lockfile's hashes and past the one-version rule.
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
                findings.push(format!(
                    "{base}.{name} comes from git; \
                     a dependency comes from the locked registry or this repository"
                ));
            } else if spec
                .get("path")
                .and_then(toml::Value::as_str)
                .is_some_and(|path| Path::new(path).is_absolute())
            {
                findings.push(format!(
                    "{base}.{name} is an absolute path; \
                     a path dependency stays inside the repository"
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

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{is_module_file, manifest_findings};

    #[test]
    fn a_git_or_absolute_path_dependency_is_named() {
        let manifest = "[dependencies]\nserde = \"1\"\n\
                        forked = { git = \"https://github.com/x/forked\" }\n\
                        patched = { path = \"/abs/patched\" }\nlocal = { path = \"../local\" }\n";
        let findings = manifest_findings(manifest);
        assert!(findings
            .iter()
            .any(|finding| finding.starts_with("dependencies.forked comes from git;")));
        assert!(findings
            .iter()
            .any(|finding| finding.starts_with("dependencies.patched is an absolute path;")));
        assert_eq!(findings.len(), 2);
    }

    #[test]
    fn a_lint_turned_to_allow_is_named_in_either_form() {
        let manifest = "[lints.clippy]\ncmp_owned = \"allow\"\n\
                        [workspace.lints.rust]\ndead_code = { level = \"allow\", priority = 1 }\n";
        let findings = manifest_findings(manifest);
        assert!(findings
            .iter()
            .any(|finding| finding.starts_with("lints.clippy.cmp_owned is allow;")));
        assert!(findings
            .iter()
            .any(|finding| finding.starts_with("workspace.lints.rust.dead_code is allow;")));
    }

    #[test]
    fn a_lint_raised_to_deny_passes_the_wiring() {
        let manifest = "[lints.clippy]\nunwrap_used = \"deny\"\n";
        assert!(manifest_findings(manifest).is_empty());
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
