use std::ffi::OsStr;
use std::path::{Component, Path, PathBuf};

use tree_sitter::{Language, Tree};

use crate::pack::{Def, Definition, Edge, Finding, Pack, Suppression};
use crate::prose;
use crate::trees;
use crate::units::Repo;

const DEFINITIONS: &str = include_str!("../packs/typescript/definitions.scm");
const USES: &str = include_str!("../packs/typescript/uses.scm");
const MENTIONS: &str = include_str!("../packs/typescript/mentions.scm");
const BAR: &str = include_str!("../packs/typescript/bar.toml");
const REEXPORTS: &str = include_str!("../packs/typescript/reexports.scm");
const SUPPRESSIONS: &str = include_str!("../packs/typescript/suppressions.scm");
const PROSE: &str = include_str!("../packs/typescript/prose.scm");
const BUILTINS: &str = include_str!("../packs/typescript/builtins.txt");

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
    reexports: Vec<(PathBuf, String, usize)>,
    suppressions: Vec<Suppression>,
    aliases: Vec<(PathBuf, String, usize)>,
    overreaches: Vec<(PathBuf, String, usize)>,
}

impl TypeScript {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Pack for TypeScript {
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
        Some(include_str!("../packs/typescript/tools.toml"))
    }

    fn configs(&self) -> &'static [(&'static str, &'static str)] {
        &[("biome.json", include_str!("../packs/typescript/biome.json"))]
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
        Some(option_findings(&options))
    }

    fn roots(&self, _root: &Path, _repo: &Repo) -> Vec<PathBuf> {
        Vec::new()
    }

    fn scan(&mut self, file: &Path, source: &str, surface: bool) -> Vec<Definition> {
        let Some(tree) = trees::parse(&language(), source) else {
            return Vec::new();
        };
        for (from, name, line) in imports(&tree, source) {
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
                self.reexports
                    .push((file.to_path_buf(), capture.text, capture.line));
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
        let mut findings: Vec<Finding> = self
            .reexports
            .iter()
            .map(|(file, from, line)| Finding {
                check: "integrity",
                file: file.clone(),
                line: *line,
                name: "no-reexport".to_string(),
                reason: format!("re-exports from \"{from}\"; a module offers only its own names"),
            })
            .collect();
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
        let declared = declared_packages(root, repo);
        let builtins: std::collections::HashSet<&str> = BUILTINS.lines().collect();
        let mut named = std::collections::BTreeSet::new();
        for (file, from, _, line) in &self.imports {
            let unit = repo.unit_of(file);
            let home = repo.units[unit].path.clone();
            if repo.is_test(unit, &home) {
                continue;
            }
            if from.starts_with('.') || from.starts_with("node:") {
                continue;
            }
            let package = if from.starts_with('@') {
                from.splitn(3, '/').take(2).collect::<Vec<&str>>().join("/")
            } else {
                from.split('/').next().unwrap_or(from).to_string()
            };
            if builtins.contains(package.as_str()) || declared.contains(&package) {
                continue;
            }
            if named.insert(package.clone()) {
                findings.push(Finding {
                    check: "integrity",
                    file: file.clone(),
                    line: *line,
                    name: "undeclared-dependency".to_string(),
                    reason: format!("{package} is imported, and no manifest declares it"),
                });
            }
        }
        findings
    }

    fn outputs(&self) -> &'static [&'static str] {
        &["node_modules", "dist"]
    }

    fn resolve(&self, mine: usize, defs: &[Def], repo: &Repo) -> Vec<Edge> {
        let mut edges = Vec::new();
        for (file, from, name, line) in &self.imports {
            // A bare specifier names a package, which is not this repository's code.
            if !from.starts_with('.') {
                continue;
            }
            let base = file.parent().unwrap_or(Path::new(""));
            let joined = normalized(&base.join(from));
            let shown = joined.to_string_lossy();
            let candidates = [
                format!("{shown}.ts"),
                format!("{shown}/index.ts"),
                shown.to_string(),
            ];
            for (index, def) in defs.iter().enumerate() {
                if def.pack != mine || def.name != *name {
                    continue;
                }
                if candidates
                    .iter()
                    .any(|candidate| def.file.to_string_lossy() == *candidate)
                {
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

fn language() -> Language {
    tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()
}

fn definitions(tree: &Tree, source: &str) -> Vec<(String, usize)> {
    trees::texts(&language(), DEFINITIONS, tree, source, "def")
}

fn mentions(tree: &Tree, source: &str) -> Vec<(String, usize)> {
    trees::texts(&language(), MENTIONS, tree, source, "mention")
}

fn imports(tree: &Tree, source: &str) -> Vec<(String, String, usize)> {
    trees::matches(&language(), USES, tree, source)
        .into_iter()
        .filter_map(|captures| {
            let from = captures.iter().find(|capture| capture.name == "source")?;
            let name = captures.iter().find(|capture| capture.name == "name")?;
            Some((from.text.clone(), name.text.clone(), name.line))
        })
        .collect()
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

fn declared_packages(root: &Path, repo: &Repo) -> std::collections::HashSet<String> {
    let mut declared = std::collections::HashSet::new();
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
        for table in [
            "dependencies",
            "devDependencies",
            "peerDependencies",
            "optionalDependencies",
        ] {
            let Some(entries) = parsed.get(table).and_then(serde_json::Value::as_object) else {
                continue;
            };
            for name in entries.keys() {
                declared.insert(name.clone());
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

    use super::{normalized, option_findings};

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
}
