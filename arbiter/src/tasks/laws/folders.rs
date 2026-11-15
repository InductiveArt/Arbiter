use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

use crate::core::graph::Graph;
use crate::core::pack::Finding;
use crate::core::rule;
use crate::core::rule::Law;
use crate::core::rule::Rule;
use crate::core::units::DECLARATIONS;

pub const RULES: &[Rule] = &[
    rule::record(
        "uninvited-folder-use",
        "folders",
        ".arbiter/units.toml",
        "a declared folder is entered only by what its visibility grants",
    ),
    rule::record(
        "stale-folder-grant",
        "folders",
        ".arbiter/units.toml",
        "a folder's grant names a user it has; one never used is deleted",
    ),
    rule::record(
        "undeclared-folder-use",
        "folders",
        ".arbiter/units.toml",
        "a declared folder's deps name everything it uses, inside its unit or out",
    ),
    rule::record(
        "stale-folder-declaration",
        "folders",
        ".arbiter/units.toml",
        "a declared folder dep is used; one never used is deleted",
    ),
];

pub fn folders_law() -> Law {
    Law {
        task: "folders",
        rules: RULES,
        run: judge,
    }
}

// The folder grain: inner nodes of a unit hold the same two consents, judged
// over every declared node an edge's end sits under - conjunctively, so a
// child only tightens what the unit allows. A node governs its border, never
// its interior. A relative pattern reads in the unit's frame; `//` reaches
// from the repository root.
fn judge(found: &Graph) -> Vec<Finding> {
    let Ok(text) = fs::read_to_string(found.root.join(DECLARATIONS)) else {
        return Vec::new();
    };
    // A file that does not parse is already named where the units are read.
    let Ok(parsed) = text.parse::<toml::Value>() else {
        return Vec::new();
    };
    let Some(units) = parsed.get("units").and_then(toml::Value::as_table) else {
        return Vec::new();
    };
    let mut nodes: Vec<InnerNode> = Vec::new();
    for (name, declaration) in units {
        let Some(inner) = declaration.get("inner").and_then(toml::Value::as_table) else {
            continue;
        };
        let Some(unit) = (0..found.repo.units.len()).find(|&at| found.repo.unit_name(at) == *name)
        else {
            continue;
        };
        for (path, node) in inner {
            let read = |field: &str| -> Option<Vec<String>> {
                Some(
                    node.get(field)?
                        .as_array()?
                        .iter()
                        .filter_map(toml::Value::as_str)
                        .map(str::to_string)
                        .collect(),
                )
            };
            nodes.push(InnerNode {
                unit,
                path: path.clone(),
                visibility: read("visibility"),
                deps: read("deps"),
            });
        }
    }
    if nodes.is_empty() {
        return Vec::new();
    }
    let rel = |unit: usize, file: &Path| -> String {
        let home = &found.repo.units[unit].path;
        file.strip_prefix(home)
            .unwrap_or(file)
            .to_string_lossy()
            .to_string()
    };
    // A relative pattern consents only inside its own unit; a `//` pattern
    // consents to the other end wherever it lives.
    let consents = |pattern: &str, same: bool, other_rel: &str, other_repo: &str| -> bool {
        match pattern.strip_prefix("//") {
            Some(absolute) => covers(absolute, other_repo),
            None => same && covers(pattern, other_rel),
        }
    };
    let mut findings = Vec::new();
    let mut seen: BTreeSet<(PathBuf, usize, String)> = BTreeSet::new();
    let mut live_grants: BTreeSet<(usize, usize)> = BTreeSet::new();
    let mut live_deps: BTreeSet<(usize, usize)> = BTreeSet::new();
    for edge in &found.edges {
        let src = edge.unit;
        let dst = found.defs[edge.def].unit;
        let same = src == dst;
        let s_rel = rel(src, &edge.file);
        let t_rel = rel(dst, &found.defs[edge.def].file);
        let s_repo = edge.file.to_string_lossy().to_string();
        let t_repo = found.defs[edge.def].file.to_string_lossy().to_string();
        for (at, node) in nodes.iter().enumerate() {
            // An edge wholly inside a node is its interior, not its border.
            if node.unit == src
                && covers(&node.path, &s_rel)
                && !(same && covers(&node.path, &t_rel))
            {
                if let Some(deps) = &node.deps {
                    let mut consented = false;
                    for (entry, pattern) in deps.iter().enumerate() {
                        if consents(pattern, same, &t_rel, &t_repo) {
                            live_deps.insert((at, entry));
                            consented = true;
                        }
                    }
                    if !consented {
                        let shown = if same {
                            t_rel.clone()
                        } else {
                            format!("//{t_repo}")
                        };
                        let reason =
                            format!("{s_rel} uses {shown}, which {} does not declare", node.path);
                        if seen.insert((edge.file.clone(), edge.line, reason.clone())) {
                            findings.push(Finding::integrity(
                                &edge.file,
                                edge.line,
                                "undeclared-folder-use",
                                reason,
                            ));
                        }
                    }
                }
            }
            if node.unit == dst
                && covers(&node.path, &t_rel)
                && !(same && covers(&node.path, &s_rel))
            {
                if let Some(visibility) = &node.visibility {
                    let mut invited = false;
                    for (entry, pattern) in visibility.iter().enumerate() {
                        if consents(pattern, same, &s_rel, &s_repo) {
                            live_grants.insert((at, entry));
                            invited = true;
                        }
                    }
                    if !invited {
                        let shown = if same {
                            s_rel.clone()
                        } else {
                            format!("//{s_repo}")
                        };
                        let reason = format!(
                            "{shown} uses {t_rel}, which only {} may",
                            visibility.join(", ")
                        );
                        if seen.insert((edge.file.clone(), edge.line, reason.clone())) {
                            findings.push(Finding::integrity(
                                &edge.file,
                                edge.line,
                                "uninvited-folder-use",
                                reason,
                            ));
                        }
                    }
                }
            }
        }
    }
    for (at, node) in nodes.iter().enumerate() {
        let place = format!("{} in {}", node.path, found.repo.unit_name(node.unit));
        if let Some(visibility) = &node.visibility {
            for (entry, pattern) in visibility.iter().enumerate() {
                if pattern.contains('*') || live_grants.contains(&(at, entry)) {
                    continue;
                }
                findings.push(Finding::integrity(
                    Path::new(DECLARATIONS),
                    0,
                    "stale-folder-grant",
                    format!("{place} grants {pattern}, which never uses it"),
                ));
            }
        }
        if let Some(deps) = &node.deps {
            for (entry, pattern) in deps.iter().enumerate() {
                if pattern.contains('*') || live_deps.contains(&(at, entry)) {
                    continue;
                }
                findings.push(Finding::integrity(
                    Path::new(DECLARATIONS),
                    0,
                    "stale-folder-declaration",
                    format!("{place} declares {pattern} and never uses it"),
                ));
            }
        }
    }
    findings
}

// A folder pattern covers itself and everything below; `*` covers all. The
// folder grain is prefix-native: a bare path is its own subtree.
fn covers(pattern: &str, path: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    let base = pattern.strip_suffix("/*").unwrap_or(pattern);
    path == base || path.starts_with(&format!("{base}/"))
}

// One declared inner node: the unit-relative path it governs and its two
// consents. A missing list binds nothing, as everywhere.
struct InnerNode {
    unit: usize,
    path: String,
    visibility: Option<Vec<String>>,
    deps: Option<Vec<String>>,
}
