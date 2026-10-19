use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use tree_sitter::{Language, Tree};

use crate::pack::{Def, Definition, Edge, Finding, Pack, Suppression};
use crate::prose;
use crate::trees;
use crate::units::Repo;

const DEFINITIONS: &str = include_str!("../packs/python/definitions.scm");
const USES: &str = include_str!("../packs/python/uses.scm");
const MENTIONS: &str = include_str!("../packs/python/mentions.scm");
const BAR: &str = include_str!("../packs/python/bar.toml");
const SUPPRESSIONS: &str = include_str!("../packs/python/suppressions.scm");
const PROSE: &str = include_str!("../packs/python/prose.scm");
const STDLIB: &str = include_str!("../packs/python/stdlib.txt");
const DISTRIBUTIONS: &str = include_str!("../packs/python/distributions.toml");

const CHECKERS: [&str; 2] = ["basedpyright", "pyright"];
const MODES: [&str; 3] = ["strict", "recommended", "all"];

#[derive(Default)]
pub struct Python {
    imports: Vec<(PathBuf, String, String, usize)>,
    init_imports: Vec<(PathBuf, String, usize)>,
    plains: Vec<(PathBuf, String, usize)>,
    chains: Vec<(PathBuf, String, usize)>,
    suppressions: Vec<Suppression>,
    aliases: Vec<(PathBuf, String, usize)>,
    overreaches: Vec<(PathBuf, String, usize)>,
}

impl Python {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Pack for Python {
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
        Some(include_str!("../packs/python/tools.toml"))
    }

    fn configs(&self) -> &'static [(&'static str, &'static str)] {
        &[("ruff.toml", include_str!("../packs/python/ruff.toml"))]
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

    fn wiring(&self, folder: &Path) -> Option<Vec<String>> {
        let Ok(manifest) = fs::read_to_string(folder.join("pyproject.toml")) else {
            return Some(vec![
                "no pyproject.toml; the unit declares no environment".to_string()
            ]);
        };
        Some(manifest_findings(&manifest))
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
        &["__pycache__"]
    }

    fn lockfile(&self) -> Option<&'static str> {
        Some("uv.lock")
    }

    fn integrity(&self, root: &Path, repo: &Repo) -> Vec<Finding> {
        let mut findings: Vec<Finding> = self
            .init_imports
            .iter()
            .map(|(file, name, line)| Finding {
                check: "integrity",
                file: file.clone(),
                line: *line,
                name: "no-reexport".to_string(),
                reason: format!(
                    "__init__.py imports {name}; a symbol is imported from the module defining it"
                ),
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
        let declared = declared_distributions(root, repo);
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
            let top = module.split('.').next().unwrap_or(module);
            if stdlib.contains(top) || module_file(module, &repo.files).is_some() {
                continue;
            }
            let distribution = known.get(top).cloned().unwrap_or_else(|| top.to_string());
            if declared.contains(&normalized(&distribution)) {
                continue;
            }
            if named.insert(top.to_string()) {
                findings.push(Finding {
                    check: "integrity",
                    file: file.clone(),
                    line,
                    name: "undeclared-dependency".to_string(),
                    reason: format!("{top} is imported, and no manifest declares it"),
                });
            }
        }
        findings
    }

    fn resolve(&self, mine: usize, defs: &[Def], repo: &Repo) -> Vec<Edge> {
        let mut edges = Vec::new();
        for (file, module, name, line) in &self.imports {
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
    trees::matches(&language(), DEFINITIONS, tree, source)
        .into_iter()
        .flatten()
        .filter(|capture| capture.name == "def" && !capture.text.starts_with('_'))
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

// Distribution names as PEP 503 reads them: lowercased, runs of separators as one dash.
fn normalized(name: &str) -> String {
    name.to_lowercase()
        .replace(['_', '.'], "-")
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<&str>>()
        .join("-")
}

fn declared_distributions(root: &Path, repo: &Repo) -> HashSet<String> {
    let mut declared = HashSet::new();
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
        let mut lists: Vec<&toml::Value> = Vec::new();
        lists.extend(
            parsed
                .get("project")
                .and_then(|project| project.get("dependencies")),
        );
        for groups in ["optional-dependencies", "dependency-groups"] {
            let table = match groups {
                "optional-dependencies" => parsed
                    .get("project")
                    .and_then(|project| project.get(groups)),
                _ => parsed.get(groups),
            };
            if let Some(table) = table.and_then(toml::Value::as_table) {
                lists.extend(table.values());
            }
        }
        for list in lists {
            let Some(entries) = list.as_array() else {
                continue;
            };
            for entry in entries {
                if let Some(requirement) = entry.as_str() {
                    let name: String = requirement
                        .chars()
                        .take_while(|part| {
                            part.is_ascii_alphanumeric() || matches!(part, '-' | '_' | '.')
                        })
                        .collect();
                    declared.insert(normalized(&name));
                }
            }
        }
    }
    declared
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
    trees::matches(&language(), USES, tree, source)
        .into_iter()
        .flatten()
        .filter(|capture| capture.name == wanted)
        .map(|capture| (capture.text, capture.line))
        .collect()
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

fn manifest_findings(manifest: &str) -> Vec<String> {
    let Ok(parsed) = manifest.parse::<toml::Value>() else {
        return vec!["pyproject.toml does not parse".to_string()];
    };
    let mut findings = refused_findings(&parsed);
    findings.extend(checker_findings(&parsed));
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
            }
        }
    }
    findings
}

fn is_error(value: &toml::Value) -> bool {
    value.as_str() == Some("error") || value.as_bool() == Some(true)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{is_module_file, manifest_findings};

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
    fn an_environment_setting_passes_the_wiring() {
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
