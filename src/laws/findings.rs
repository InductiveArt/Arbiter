use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

use crate::core::graph::strongly_connected;
use crate::core::graph::Graph;
use crate::core::pack::Def;
use crate::core::pack::Edge;
use crate::core::pack::Finding;
use crate::core::pack::Pack;
use crate::core::pack::Repo;

use crate::core::units::fits;
use crate::core::units::DECLARATIONS;

const LICENSES: &str = ".arbiter/licenses.toml";

// A public symbol no file but its own uses is named, inviting deletion.
pub fn winnow(
    defs: &[Def],
    edges: &[Edge],
    repo: &Repo,
    commissioned: &std::collections::HashSet<PathBuf>,
) -> (Vec<Finding>, Vec<String>) {
    let mut findings = Vec::new();
    let mut nascent = Vec::new();
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
        // Tests of a case the brief still plans are building toward a consumer:
        // the symbol is nascent, and the commission is the user's signature.
        if !users.is_empty() && users.iter().all(|edge| commissioned.contains(&edge.file)) {
            nascent.push(format!(
                "nascent: {} at {}:{} is reached only by tests of a case still planned",
                def.name,
                def.file.to_string_lossy(),
                def.line
            ));
            continue;
        }
        let (name, reason) = if users.is_empty() {
            ("dead-symbol", format!("{} has no user anywhere", def.name))
        } else {
            (
                "tests-only-symbol",
                format!("{} has no user but tests", def.name),
            )
        };
        findings.push(Finding {
            check: "winnow",
            file: def.file.clone(),
            line: def.line,
            name: name.to_string(),
            reason,
        });
    }
    findings.sort_by(|a, b| (&a.file, a.line).cmp(&(&b.file, b.line)));
    nascent.sort();
    (findings, nascent)
}

pub fn integrity(found: &Graph) -> Vec<Finding> {
    let mut findings = cycles(&found.defs, &found.edges, &found.repo);
    for pack in &found.packs {
        findings.extend(pack.integrity(&found.root, &found.repo));
    }
    findings.extend(split_version(&found.root, &found.repo, &found.packs));
    findings.extend(boundaries(found));
    findings.extend(folder_boundaries(found));
    findings.extend(licenses(found));
    findings.extend(crate::records::plans::findings(found));
    let (subjects, guards) = crate::core::record::read(&found.root);
    findings.extend(guards);
    let (cycles_found, cycles_live) = sibling_cycles(found, &subjects);
    findings.extend(cycles_found);
    findings.extend(crate::core::record::stale_cycles(&subjects, &cycles_live));
    // A subject no pack judges would never go stale from inside a pack.
    for subject in &subjects {
        let crate::core::record::Subject::Dependency {
            language,
            package,
            line,
            ..
        } = subject
        else {
            continue;
        };
        if !found.packs.iter().any(|pack| pack.label() == language) {
            findings.push(Finding {
                check: "integrity",
                file: PathBuf::from(crate::core::record::SUPPRESSIONS),
                line: *line,
                name: "stale-suppression".to_string(),
                reason: format!("{package} is suppressed for {language}, which no pack judges"),
            });
        }
    }
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
            name: "unparsable-user-record".to_string(),
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
            if permitted(&license, &allowed) {
                continue;
            }
            if !seen.insert((package.clone(), license.clone())) {
                continue;
            }
            findings.push(Finding {
                check: "licenses",
                file: place,
                line: 0,
                name: "unallowed-license".to_string(),
                reason: format!(
                    "{package} is licensed {license}, which the allowlist does not name"
                ),
            });
        }
    }
    findings
}

// An SPDX expression passes when one OR branch does and every AND side does;
// a common spelling folds to its SPDX name before the match.
fn permitted(license: &str, allowed: &[&str]) -> bool {
    let text = license.trim();
    if text.contains(" OR ") {
        return text.split(" OR ").any(|part| permitted(part, allowed));
    }
    if text.contains(" AND ") {
        return text.split(" AND ").all(|part| permitted(part, allowed));
    }
    let name = folded(text);
    allowed.iter().any(|allow| folded(allow) == name)
}

fn folded(name: &str) -> String {
    let trimmed = name
        .trim()
        .trim_matches(|part: char| part == '(' || part == ')')
        .trim();
    let lower = trimmed.to_ascii_lowercase();
    match lower.as_str() {
        "mit license" | "the mit license" => "mit".to_string(),
        "apache 2.0"
        | "apache license 2.0"
        | "apache license, version 2.0"
        | "apache software license" => "apache-2.0".to_string(),
        "3-clause bsd license" | "bsd 3-clause" | "new bsd license" | "bsd license" => {
            "bsd-3-clause".to_string()
        }
        "2-clause bsd license" | "bsd 2-clause" | "simplified bsd license" => {
            "bsd-2-clause".to_string()
        }
        "isc license" => "isc".to_string(),
        _ => lower,
    }
}

// A third party resolved at two versions across the repository's lockfiles
// is named with every version and its place.
fn split_version(root: &Path, repo: &Repo, packs: &[Box<dyn Pack>]) -> Vec<Finding> {
    let mut findings = Vec::new();
    for pack in packs {
        let locks = pack.lockfiles();
        if locks.is_empty() {
            continue;
        }
        let mut versions: std::collections::BTreeMap<
            String,
            std::collections::BTreeMap<String, PathBuf>,
        > = std::collections::BTreeMap::new();
        for path in &repo.lockfiles {
            let Some(name) = path.file_name().map(|name| name.to_string_lossy()) else {
                continue;
            };
            if !locks.contains(&name.as_ref()) {
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
            for (locked, version) in pack.locked(&name, &text) {
                versions
                    .entry(locked)
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
                name: "split-version".to_string(),
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
            name: "unit-cycle".to_string(),
            reason: format!("units import each other: {}", names.join(", ")),
        });
    }
    findings
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
            "unparsable-user-record",
            format!("{DECLARATIONS} does not parse; the declared boundaries cannot be read"),
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
    for (user, owned, file, line) in &witness {
        let (invited, declared) = sanctioned(units, user, owned);
        if !invited {
            let patterns = listed(units, owned, "visibility").unwrap_or_default();
            findings.push(Finding::integrity(
                file,
                *line,
                "uninvited-unit-use",
                format!(
                    "{user} uses {owned}, which only {} may",
                    patterns.join(", ")
                ),
            ));
        }
        if !declared {
            findings.push(Finding::integrity(
                file,
                *line,
                "undeclared-unit-use",
                format!("{user} uses {owned} and does not declare it"),
            ));
        }
    }
    for (name, _) in units {
        if let Some(patterns) = listed(units, name, "visibility") {
            // A starred grant spans units that may not exist yet; only a named one can be stale.
            for pattern in patterns {
                if pattern.contains('*') {
                    continue;
                }
                let used = between
                    .iter()
                    .any(|(user, owned)| owned == name && *user == pattern);
                if !used {
                    findings.push(Finding::integrity(
                        Path::new(DECLARATIONS),
                        0,
                        "stale-unit-grant",
                        format!("{name} grants {pattern}, which never uses it"),
                    ));
                }
            }
        }
        if let Some(declared) = listed(units, name, "deps") {
            for dependency in declared {
                let used = between.contains(&(name.clone(), dependency.clone()));
                if !used {
                    findings.push(Finding::integrity(
                        Path::new(DECLARATIONS),
                        0,
                        "stale-unit-declaration",
                        format!("{name} declares {dependency} and never uses it"),
                    ));
                }
            }
        }
    }
    findings
}

// One field of one unit's declaration, as the written list; None where the
// unit or the field stays undeclared, which binds nothing.
fn listed(units: &toml::value::Table, name: &str, field: &str) -> Option<Vec<String>> {
    let list = units.get(name)?.get(field)?.as_array()?;
    Some(
        list.iter()
            .filter_map(toml::Value::as_str)
            .map(str::to_string)
            .collect(),
    )
}

// Whether one resolved edge stands inside the declarations, both ways: the
// owner's visibility invites the user, and the user's deps name the owner.
// An undeclared side binds nothing, so the edge passes it.
fn sanctioned(units: &toml::value::Table, user: &str, owner: &str) -> (bool, bool) {
    let invited = listed(units, owner, "visibility")
        .is_none_or(|patterns| patterns.iter().any(|pattern| fits(pattern, user)));
    let declared =
        listed(units, user, "deps").is_none_or(|declared| declared.contains(&owner.to_string()));
    (invited, declared)
}

// One declared inner node: the unit-relative path it governs and its two
// consents. A missing list binds nothing, as everywhere.
struct InnerNode {
    unit: usize,
    path: String,
    visibility: Option<Vec<String>>,
    deps: Option<Vec<String>>,
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

// The folder grain: inner nodes of a unit hold the same two consents, judged
// over every declared node an edge's end sits under - conjunctively, so a
// child only tightens what the unit allows. A node governs its border, never
// its interior. A relative pattern reads in the unit's frame; `//` reaches
// from the repository root.
fn folder_boundaries(found: &Graph) -> Vec<Finding> {
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

// Every folder's sibling graph: the edges between its immediate children,
// found by landing each same-unit edge at the deepest folder holding both
// ends. An edge wholly inside one child is that child's interior.
fn sibling_graphs(
    found: &Graph,
) -> std::collections::BTreeMap<(usize, String), std::collections::BTreeMap<String, BTreeSet<String>>>
{
    let mut graphs: std::collections::BTreeMap<
        (usize, String),
        std::collections::BTreeMap<String, BTreeSet<String>>,
    > = Default::default();
    for edge in &found.edges {
        let unit = edge.unit;
        if unit != found.defs[edge.def].unit {
            continue;
        }
        let home = found.repo.units[unit].path.clone();
        if found.repo.is_test(unit, &home) {
            continue;
        }
        let s = edge.file.strip_prefix(&home).unwrap_or(&edge.file);
        let t = found.defs[edge.def].file.clone();
        let t = t.strip_prefix(&home).unwrap_or(&t).to_path_buf();
        if s == t {
            continue;
        }
        let source: Vec<String> = s
            .components()
            .map(|part| part.as_os_str().to_string_lossy().to_string())
            .collect();
        let target: Vec<String> = t
            .components()
            .map(|part| part.as_os_str().to_string_lossy().to_string())
            .collect();
        let mut shared = 0;
        while shared < source.len() && shared < target.len() && source[shared] == target[shared] {
            shared += 1;
        }
        if shared >= source.len() || shared >= target.len() {
            continue;
        }
        let folder = source[..shared].join("/");
        let from = source[..=shared].join("/");
        let to = target[..=shared].join("/");
        graphs
            .entry((unit, folder))
            .or_default()
            .entry(from)
            .or_default()
            .insert(to);
    }
    graphs
}

// One family of the containment tree, told in repo coordinates: the unit it
// belongs to, its place, and the edges between its children.
pub struct Family {
    pub unit: String,
    pub place: String,
    pub edges: Vec<(String, String)>,
}

// Every family the containment tree holds - the drawable form of the sibling
// facts, suppressed or not.
pub fn families(found: &Graph) -> Vec<Family> {
    let mut told = Vec::new();
    for ((unit, folder), children) in sibling_graphs(found) {
        let home = &found.repo.units[unit].path;
        let place = if folder.is_empty() {
            if home.as_os_str().is_empty() {
                ".".to_string()
            } else {
                home.to_string_lossy().to_string()
            }
        } else {
            home.join(&folder).to_string_lossy().to_string()
        };
        let mut edges = Vec::new();
        for (from, reached) in &children {
            for to in reached {
                edges.push((from.clone(), to.clone()));
            }
        }
        told.push(Family {
            unit: found.repo.unit_name(unit),
            place,
            edges,
        });
    }
    told
}

// The sibling law: within every folder, the children form an order. A cycle
// is named with its whole member set; the user's record may quiet one exact
// set, and a quieting entry is proven alive or named stale.
fn sibling_cycles(
    found: &Graph,
    subjects: &[crate::core::record::Subject],
) -> (Vec<Finding>, std::collections::HashSet<usize>) {
    let mut findings = Vec::new();
    let mut live = std::collections::HashSet::new();
    for ((unit, folder), children) in sibling_graphs(found) {
        let names: Vec<&String> = children.keys().collect();
        let index: std::collections::BTreeMap<&str, usize> = names
            .iter()
            .enumerate()
            .map(|(at, name)| (name.as_str(), at))
            .collect();
        let mut adjacency = vec![BTreeSet::new(); names.len()];
        for (from, reached) in &children {
            for to in reached {
                if let (Some(&a), Some(&b)) = (index.get(from.as_str()), index.get(to.as_str())) {
                    adjacency[a].insert(b);
                }
            }
        }
        for group in strongly_connected(&adjacency) {
            if group.len() < 2 {
                continue;
            }
            let members: BTreeSet<String> = group.iter().map(|&at| names[at].clone()).collect();
            let spoken = found.repo.unit_name(unit);
            if let Some(at) = crate::core::record::quiets_cycle(subjects, &spoken, &members) {
                live.insert(at);
                continue;
            }
            let home = &found.repo.units[unit].path;
            let place = if folder.is_empty() {
                if home.as_os_str().is_empty() {
                    ".".to_string()
                } else {
                    home.to_string_lossy().to_string()
                }
            } else {
                home.join(&folder).to_string_lossy().to_string()
            };
            let listed: Vec<&str> = members.iter().map(String::as_str).collect();
            findings.push(Finding::integrity(
                Path::new(&place),
                0,
                "sibling-cycle",
                format!(
                    "the children of {place} form a cycle: {}",
                    listed.join(" <-> ")
                ),
            ));
        }
    }
    (findings, live)
}

// The tangles read: every sibling cycle the facts hold, suppressed or not,
// largest first - a measure, never findings.
pub fn tangles(found: &Graph) -> Vec<(String, Vec<String>)> {
    let mut groups = Vec::new();
    for ((unit, folder), children) in sibling_graphs(found) {
        let names: Vec<&String> = children.keys().collect();
        let index: std::collections::BTreeMap<&str, usize> = names
            .iter()
            .enumerate()
            .map(|(at, name)| (name.as_str(), at))
            .collect();
        let mut adjacency = vec![BTreeSet::new(); names.len()];
        for (from, reached) in &children {
            for to in reached {
                if let (Some(&a), Some(&b)) = (index.get(from.as_str()), index.get(to.as_str())) {
                    adjacency[a].insert(b);
                }
            }
        }
        for group in strongly_connected(&adjacency) {
            if group.len() < 2 {
                continue;
            }
            let home = &found.repo.units[unit].path;
            let place = if folder.is_empty() {
                if home.as_os_str().is_empty() {
                    ".".to_string()
                } else {
                    home.to_string_lossy().to_string()
                }
            } else {
                home.join(&folder).to_string_lossy().to_string()
            };
            let mut members: Vec<String> = group.iter().map(|&at| names[at].clone()).collect();
            members.sort();
            groups.push((place, members));
        }
    }
    groups.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then(a.0.cmp(&b.0)));
    groups
}

// Every resolved cross-unit edge of the code units, with whether it breaks
// the declarations: the drawable truth, declared against real.
pub fn drawn(found: &Graph) -> Vec<(String, String, bool)> {
    let empty = toml::value::Table::new();
    let units = fs::read_to_string(found.root.join(DECLARATIONS))
        .ok()
        .and_then(|text| text.parse::<toml::Value>().ok())
        .and_then(|parsed| parsed.get("units").and_then(toml::Value::as_table).cloned())
        .unwrap_or(empty);
    let mut pairs: BTreeSet<(String, String)> = BTreeSet::new();
    for edge in &found.edges {
        let owner = found.defs[edge.def].unit;
        if owner == edge.unit {
            continue;
        }
        let test = |unit: usize| {
            let home = found.repo.units[unit].path.clone();
            found.repo.is_test(unit, &home)
        };
        if test(owner) || test(edge.unit) {
            continue;
        }
        pairs.insert((shown(found, edge.unit), shown(found, owner)));
    }
    pairs
        .into_iter()
        .map(|(user, owner)| {
            let (invited, declared) = sanctioned(&units, &user, &owner);
            let breaks = !(invited && declared);
            (user, owner, breaks)
        })
        .collect()
}

fn shown(found: &Graph, unit: usize) -> String {
    found.repo.unit_name(unit)
}

#[cfg(test)]
mod tests {
    use super::permitted;
    use crate::core::units::fits;

    #[test]
    fn one_allowed_branch_of_an_or_passes() {
        let allowed = ["Apache-2.0", "MIT"];
        assert!(permitted("Apache-2.0 OR BSD-2-Clause", &allowed));
        assert!(permitted("(MIT OR GPL-3.0-only)", &allowed));
        assert!(!permitted("GPL-3.0-only OR AGPL-3.0-only", &allowed));
        assert!(!permitted("Apache-2.0 AND GPL-3.0-only", &allowed));
        assert!(permitted("Apache-2.0 AND MIT", &allowed));
    }

    #[test]
    fn a_common_spelling_folds_to_its_spdx_name() {
        let allowed = ["BSD-3-Clause", "Apache-2.0"];
        assert!(permitted("3-Clause BSD License", &allowed));
        assert!(permitted("Apache Software License", &allowed));
        assert!(permitted("apache-2.0", &allowed));
        assert!(!permitted("WTFPL", &allowed));
    }

    #[test]
    fn a_star_covers_the_folder_and_everything_below() {
        assert!(fits("apps/*", "apps/api"));
        assert!(fits("apps/*", "apps/web/admin"));
        assert!(!fits("apps/*", "apps-extra"));
        assert!(fits("libs/core", "libs/core"));
        assert!(!fits("libs/core", "libs/core_py"));
    }
}
