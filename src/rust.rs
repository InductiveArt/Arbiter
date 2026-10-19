use std::path::{Path, PathBuf};

use tree_sitter::{Language, Tree};

use crate::pack::{Def, Definition, Edge, Finding, Pack, Suppression};
use crate::prose;
use crate::trees;
use crate::units::Repo;

const DEFINITIONS: &str = include_str!("../packs/rust/definitions.scm");
const USES: &str = include_str!("../packs/rust/uses.scm");
const MENTIONS: &str = include_str!("../packs/rust/mentions.scm");
const SUPPRESSIONS: &str = include_str!("../packs/rust/suppressions.scm");
const PROSE: &str = include_str!("../packs/rust/prose.scm");

#[derive(Default)]
pub struct Rust {
    uses: Vec<(PathBuf, String, String, usize)>,
    suppressions: Vec<Suppression>,
    aliases: Vec<(PathBuf, String, usize)>,
    overreaches: Vec<(PathBuf, String, usize)>,
}

impl Rust {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Pack for Rust {
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
        Some(include_str!("../packs/rust/tools.toml"))
    }

    fn wiring(&self, folder: &Path) -> Option<Vec<String>> {
        let Ok(manifest) = std::fs::read_to_string(folder.join("Cargo.toml")) else {
            return Some(vec![
                "no Cargo.toml; the unit declares no environment".to_string()
            ]);
        };
        Some(manifest_findings(&manifest))
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
        for (module, name, line) in uses(&tree, source) {
            self.uses.push((file.to_path_buf(), module, name, line));
        }
        for captures in trees::matches(&language(), SUPPRESSIONS, &tree, source) {
            for capture in captures {
                let first = capture.text.lines().next().unwrap_or("").trim().to_string();
                let quiets = first.contains("allow(") || first.contains("expect(");
                let recorded = match capture.name.as_str() {
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
        if !surface {
            return Vec::new();
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
        let (comments, quoted) = prose_parts(&tree, source);
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

    fn integrity(&self, _repo: &Repo) -> Vec<Finding> {
        let mut findings: Vec<Finding> = self
            .aliases
            .iter()
            .map(|(file, alias, line)| Finding {
                check: "integrity",
                file: file.clone(),
                line: *line,
                name: "no-hidden-names".to_string(),
                reason: format!("imported as {alias}; a symbol keeps its own name"),
            })
            .collect();
        findings.extend(self.overreaches.iter().map(|(file, token, line)| Finding {
            check: "integrity",
            file: file.clone(),
            line: *line,
            name: "no-comment-overreach".to_string(),
            reason: format!(
                "a comment names {token}, which nothing in the file defines, uses or quotes"
            ),
        }));
        findings
    }

    fn resolve(&self, mine: usize, defs: &[Def], repo: &Repo) -> Vec<Edge> {
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

fn prose_parts(tree: &Tree, source: &str) -> (Vec<(String, usize)>, String) {
    let mut comments = Vec::new();
    let mut quoted = String::new();
    for captures in trees::matches(&language(), PROSE, tree, source) {
        for capture in captures {
            match capture.name.as_str() {
                "comment" => comments.push((capture.text, capture.line)),
                "quoted" => {
                    quoted.push_str(&capture.text);
                    quoted.push('\n');
                }
                _ => {}
            }
        }
    }
    (comments, quoted)
}

fn definitions(tree: &Tree, source: &str) -> Vec<(String, usize)> {
    trees::matches(&language(), DEFINITIONS, tree, source)
        .into_iter()
        .flatten()
        .filter(|capture| capture.name == "def")
        .map(|capture| (capture.text, capture.line))
        .collect()
}

fn mentions(tree: &Tree, source: &str) -> Vec<(String, usize)> {
    trees::matches(&language(), MENTIONS, tree, source)
        .into_iter()
        .flatten()
        .map(|capture| (capture.text, capture.line))
        .collect()
}

fn uses(tree: &Tree, source: &str) -> Vec<(String, String, usize)> {
    trees::matches(&language(), USES, tree, source)
        .into_iter()
        .filter_map(|captures| {
            let module = captures.iter().find(|capture| capture.name == "module")?;
            let name = captures.iter().find(|capture| capture.name == "name")?;
            Some((module.text.clone(), name.text.clone(), name.line))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{is_module_file, manifest_findings};

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
