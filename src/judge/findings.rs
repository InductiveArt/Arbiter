use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::core::graph::Graph;
use crate::core::pack::{Def, Edge, Finding, Pack};
use crate::core::units::Repo;

const DECLARATIONS: &str = ".arbiter/units.toml";
const LICENSES: &str = ".arbiter/licenses.toml";

// A public symbol no file but its own uses is named, inviting deletion.
pub fn winnow(defs: &[Def], edges: &[Edge], repo: &Repo) -> Vec<Finding> {
    let mut findings = Vec::new();
    for (index, def) in defs.iter().enumerate() {
        if def.self_used {
            continue;
        }
        // A PUBLISHED unit's surface serves users the repository cannot see.
        if repo.is_published(def.unit) {
            continue;
        }
        let users: Vec<&Edge> = edges
            .iter()
            .filter(|edge| edge.def == index && edge.file != def.file)
            .collect();
        // A test keeps nothing alive, wherever it lives.
        if users
            .iter()
            .any(|edge| !repo.is_test(edge.unit, &edge.file))
        {
            continue;
        }
        findings.push(Finding {
            check: "winnow",
            file: def.file.clone(),
            line: def.line,
            name: def.name.clone(),
            reason: if users.is_empty() {
                "no user anywhere".to_string()
            } else {
                "no user but tests".to_string()
            },
        });
    }
    findings.sort_by(|a, b| (&a.file, a.line).cmp(&(&b.file, b.line)));
    findings
}

pub fn integrity(found: &Graph) -> Vec<Finding> {
    let mut findings = cycles(&found.defs, &found.edges, &found.repo);
    for pack in &found.packs {
        findings.extend(pack.integrity(&found.root, &found.repo));
    }
    findings.extend(one_version(&found.root, &found.repo, &found.packs));
    findings.extend(boundaries(found));
    findings.extend(licenses(found));
    findings.sort_by(|a, b| (&a.file, a.line).cmp(&(&b.file, b.line)));
    findings
}

// A written allowlist turns the licenses the ecosystems materialized into
// judgments; without the file, no license policy exists to hold anything to.
fn licenses(found: &Graph) -> Vec<Finding> {
    let Ok(text) = fs::read_to_string(found.root.join(LICENSES)) else {
        return Vec::new();
    };
    let Ok(parsed) = text.parse::<toml::Value>() else {
        return vec![Finding {
            check: "licenses",
            file: PathBuf::from(LICENSES),
            line: 0,
            name: "allowlist".to_string(),
            reason: format!("{LICENSES} does not parse"),
        }];
    };
    // No allow key is no policy yet, the seeded file's state; an empty list
    // written out allows nothing.
    let Some(allowed) = parsed
        .get("allow")
        .and_then(toml::Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(toml::Value::as_str)
                .collect::<Vec<&str>>()
        })
    else {
        return Vec::new();
    };
    let mut findings = Vec::new();
    let mut seen = BTreeSet::new();
    for pack in &found.packs {
        for (place, package, license) in pack.licenses(&found.root, &found.repo) {
            if allowed.contains(&license.as_str()) {
                continue;
            }
            if !seen.insert((package.clone(), license.clone())) {
                continue;
            }
            findings.push(Finding {
                check: "licenses",
                file: place,
                line: 0,
                name: package.clone(),
                reason: format!(
                    "{package} is licensed {license}, which the allowlist does not name"
                ),
            });
        }
    }
    findings
}

// Bazel's one-version rule as a judgment: a third party resolved at two versions
// across the repository's lockfiles is named with every version and its place.
fn one_version(root: &Path, repo: &Repo, packs: &[Box<dyn Pack>]) -> Vec<Finding> {
    let mut findings = Vec::new();
    for pack in packs {
        let Some(lock) = pack.lockfile() else {
            continue;
        };
        let mut versions: std::collections::BTreeMap<
            String,
            std::collections::BTreeMap<String, PathBuf>,
        > = std::collections::BTreeMap::new();
        for path in &repo.lockfiles {
            if path.file_name().is_none_or(|name| name != lock) {
                continue;
            }
            let unit = repo.unit_of(path);
            let home = repo.units[unit].path.clone();
            if repo.is_test(unit, &home) {
                continue;
            }
            let Ok(text) = fs::read_to_string(root.join(path)) else {
                continue;
            };
            for (name, version) in pack.locked(&text) {
                versions
                    .entry(name)
                    .or_default()
                    .entry(version)
                    .or_insert_with(|| path.clone());
            }
        }
        for (name, seen) in versions {
            if seen.len() < 2 {
                continue;
            }
            let places: Vec<String> = seen
                .iter()
                .map(|(version, place)| format!("{version} in {}", place.to_string_lossy()))
                .collect();
            let Some(anchor) = seen.values().min() else {
                continue;
            };
            findings.push(Finding {
                check: "integrity",
                file: anchor.clone(),
                line: 0,
                name: "one-version".to_string(),
                reason: format!("{name} is locked at {}", places.join(" and ")),
            });
        }
    }
    findings
}

fn cycles(defs: &[Def], edges: &[Edge], repo: &Repo) -> Vec<Finding> {
    let mut adjacency = vec![BTreeSet::new(); repo.units.len()];
    for edge in edges {
        let owner = defs[edge.def].unit;
        if owner != edge.unit {
            adjacency[edge.unit].insert(owner);
        }
    }
    let mut findings = Vec::new();
    for group in strongly_connected(&adjacency) {
        if group.len() < 2 {
            continue;
        }
        let mut names: Vec<String> = group
            .iter()
            .map(|&unit| repo.units[unit].path.to_string_lossy().to_string())
            .collect();
        names.sort();
        findings.push(Finding {
            check: "integrity",
            file: PathBuf::from(&names[0]),
            line: 0,
            name: "cycle".to_string(),
            reason: format!("units import each other: {}", names.join(", ")),
        });
    }
    findings
}

struct Tarjan<'graph> {
    adjacency: &'graph [BTreeSet<usize>],
    index: Vec<Option<usize>>,
    low: Vec<usize>,
    on_stack: Vec<bool>,
    stack: Vec<usize>,
    next: usize,
    groups: Vec<Vec<usize>>,
}

fn strongly_connected(adjacency: &[BTreeSet<usize>]) -> Vec<Vec<usize>> {
    let count = adjacency.len();
    let mut state = Tarjan {
        adjacency,
        index: vec![None; count],
        low: vec![0; count],
        on_stack: vec![false; count],
        stack: Vec::new(),
        next: 0,
        groups: Vec::new(),
    };
    for at in 0..count {
        if state.index[at].is_none() {
            visit(&mut state, at);
        }
    }
    state.groups
}

fn visit(state: &mut Tarjan, at: usize) {
    state.index[at] = Some(state.next);
    state.low[at] = state.next;
    state.next += 1;
    state.stack.push(at);
    state.on_stack[at] = true;
    for &to in state.adjacency[at].iter() {
        if state.index[to].is_none() {
            visit(state, to);
            state.low[at] = state.low[at].min(state.low[to]);
        } else if state.on_stack[to] {
            if let Some(entered) = state.index[to] {
                state.low[at] = state.low[at].min(entered);
            }
        }
    }
    if Some(state.low[at]) != state.index[at] {
        return;
    }
    let mut group = Vec::new();
    while let Some(popped) = state.stack.pop() {
        state.on_stack[popped] = false;
        group.push(popped);
        if popped == at {
            break;
        }
    }
    group.sort_unstable();
    state.groups.push(group);
}

// The declared unit graph, held against the resolved one. A unit declaring nothing is
// unchecked: the file is adopted one unit at a time, the way a baseline is.
fn boundaries(found: &Graph) -> Vec<Finding> {
    let Ok(text) = fs::read_to_string(found.root.join(DECLARATIONS)) else {
        return Vec::new();
    };
    let Ok(parsed) = text.parse::<toml::Value>() else {
        return vec![Finding::integrity(
            Path::new(DECLARATIONS),
            0,
            "boundaries",
            format!("{DECLARATIONS} does not parse"),
        )];
    };
    let Some(units) = parsed.get("units").and_then(toml::Value::as_table) else {
        return Vec::new();
    };
    let mut findings = Vec::new();
    // Each resolved edge between two different units, as (user, owner, first user file).
    let mut between: BTreeSet<(String, String)> = BTreeSet::new();
    let mut witness: Vec<(String, String, PathBuf, usize)> = Vec::new();
    for edge in &found.edges {
        let owner = found.defs[edge.def].unit;
        if owner == edge.unit {
            continue;
        }
        let user = shown(found, edge.unit);
        let owned = shown(found, owner);
        if between.insert((user.clone(), owned.clone())) {
            witness.push((user, owned, edge.file.clone(), edge.line));
        }
    }
    for (name, declaration) in units {
        if let Some(allowed) = declaration
            .get("visibility")
            .and_then(toml::Value::as_array)
        {
            let patterns: Vec<&str> = allowed.iter().filter_map(toml::Value::as_str).collect();
            for (user, owned, file, line) in &witness {
                if owned == name && !patterns.iter().any(|pattern| fits(pattern, user)) {
                    findings.push(Finding::integrity(
                        file,
                        *line,
                        "visibility",
                        format!(
                            "{user} uses {owned}, which only {} may",
                            patterns.join(", ")
                        ),
                    ));
                }
            }
            // A starred grant spans units that may not exist yet; only a named one can be stale.
            for pattern in &patterns {
                if pattern.contains('*') {
                    continue;
                }
                let used = between
                    .iter()
                    .any(|(user, owned)| owned == name && user == pattern);
                if !used {
                    findings.push(Finding::integrity(
                        Path::new(DECLARATIONS),
                        0,
                        "visibility",
                        format!("{name} grants {pattern}, which never uses it"),
                    ));
                }
            }
        }
        if let Some(deps) = declaration.get("deps").and_then(toml::Value::as_array) {
            let declared: Vec<&str> = deps.iter().filter_map(toml::Value::as_str).collect();
            for (user, owned, file, line) in &witness {
                if user == name && !declared.contains(&owned.as_str()) {
                    findings.push(Finding::integrity(
                        file,
                        *line,
                        "deps",
                        format!("{user} uses {owned} and does not declare it"),
                    ));
                }
            }
            for dependency in &declared {
                let used = between.contains(&(name.clone(), (*dependency).to_string()));
                if !used {
                    findings.push(Finding::integrity(
                        Path::new(DECLARATIONS),
                        0,
                        "deps",
                        format!("{name} declares {dependency} and never uses it"),
                    ));
                }
            }
        }
    }
    findings
}

fn shown(found: &Graph, unit: usize) -> String {
    let path = &found.repo.units[unit].path;
    if path.as_os_str().is_empty() {
        ".".to_string()
    } else {
        path.to_string_lossy().to_string()
    }
}

// An exact unit path, or a folder and star covering every unit below it.
fn fits(pattern: &str, unit: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    match pattern.strip_suffix("/*") {
        Some(base) => unit == base || unit.starts_with(&format!("{base}/")),
        None => unit == pattern,
    }
}

#[cfg(test)]
mod tests {
    use super::fits;

    #[test]
    fn a_star_covers_the_folder_and_everything_below() {
        assert!(fits("apps/*", "apps/api"));
        assert!(fits("apps/*", "apps/web/admin"));
        assert!(!fits("apps/*", "apps-extra"));
        assert!(fits("libs/core", "libs/core"));
        assert!(!fits("libs/core", "libs/core_py"));
    }
}
