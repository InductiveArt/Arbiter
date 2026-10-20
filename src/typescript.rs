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
        &[(
            "biome.jsonc",
            include_str!("../packs/typescript/biome.jsonc"),
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
        let present: std::collections::HashSet<&PathBuf> = repo.files.iter().collect();
        let routes = manifest_routes(root, repo, &present);
        // A route target is the unit's declared surface, so a re-export there is the
        // declaration itself; anywhere else it hides a name.
        let mut findings: Vec<Finding> = self
            .reexports
            .iter()
            .filter(|(file, _, _)| !routes.surfaces.contains(file))
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
            if from.starts_with('.') || from.starts_with('#') || from.starts_with("node:") {
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
            let mut target = None;
            if from.starts_with('#') {
                if let Some(path) = routes.aliases.get(&unit).and_then(|map| map.get(from)) {
                    let folder = repo.units[unit].path.clone();
                    target = existing(&normalized(&folder.join(path)), &present);
                }
            } else if from.starts_with('.') {
                let base = file.parent().unwrap_or(Path::new(""));
                target = existing(&normalized(&base.join(from)), &present);
            } else if !routes.declared.contains(from.as_str())
                && !routes.names.contains_key(from.as_str())
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
            // A file no unit claims has no manifest, so no routes exist to demand.
            if owner == unit || repo.units[owner].path.as_os_str().is_empty() {
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

    fn generated(&self, name: &str) -> bool {
        name.ends_with("_pb.ts") || name.ends_with(".pb.ts")
    }

    fn resolve(&self, mine: usize, defs: &[Def], root: &Path, repo: &Repo) -> Vec<Edge> {
        let present: std::collections::HashSet<&PathBuf> = repo.files.iter().collect();
        // Each re-export, resolved to the file it reaches, so a name can pass through.
        let mut barrels: std::collections::HashMap<PathBuf, Vec<PathBuf>> =
            std::collections::HashMap::new();
        for (file, from, _) in &self.reexports {
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

struct Routes {
    names: std::collections::HashMap<String, PathBuf>,
    // Wildcard routes kept as written, the star standing for one span of a specifier.
    patterns: Vec<String>,
    aliases: std::collections::HashMap<usize, std::collections::HashMap<String, String>>,
    packages: std::collections::HashSet<String>,
    // Every route name a manifest writes, kept even when no repository file answers
    // it, since a route pointing at built output is declared all the same.
    declared: std::collections::HashSet<String>,
    // Every file a route or alias names: the declared surface of its unit.
    surfaces: std::collections::HashSet<PathBuf>,
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
            for (alias, target) in imports {
                let Some(path) = target.as_str() else {
                    continue;
                };
                if let Some(entry) = existing(&normalized(&folder.join(path)), present) {
                    routes.surfaces.insert(entry);
                }
                routes
                    .aliases
                    .entry(unit)
                    .or_default()
                    .insert(alias.clone(), path.to_string());
            }
        }
    }
    routes
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
        format!("{shown}/index.ts"),
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
            if def.pack == mine && def.name == name && def.file == file {
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

    use super::{normalized, option_findings, routed};

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
}
