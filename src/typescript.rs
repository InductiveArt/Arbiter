use std::ffi::OsStr;
use std::path::{Component, Path, PathBuf};

use tree_sitter::{Language, Tree};

use crate::pack::{Def, Definition, Edge, Pack};
use crate::trees;
use crate::units::Repo;

const DEFINITIONS: &str = include_str!("../packs/typescript/definitions.scm");
const USES: &str = include_str!("../packs/typescript/uses.scm");
const MENTIONS: &str = include_str!("../packs/typescript/mentions.scm");
const BAR: &str = include_str!("../packs/typescript/bar.toml");

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
}

impl TypeScript {
    pub fn new() -> Self {
        Self {
            imports: Vec::new(),
        }
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
        if !surface {
            return Vec::new();
        }
        let mentions = mentions(&tree, source);
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
