use std::collections::BTreeSet;
use std::fs;

use crate::core::graph::strongly_connected;
use crate::core::graph::Graph;
use crate::core::units::fits;
use crate::core::units::DECLARATIONS;

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

pub fn shown(found: &Graph, unit: usize) -> String {
    found.repo.unit_name(unit)
}

// Every folder's sibling graph: the edges between its immediate children,
// found by landing each same-unit edge at the deepest folder holding both
// ends. An edge wholly inside one child is that child's interior.
pub fn sibling_graphs(
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

// One field of one unit's declaration, as the written list; None where the
// unit or the field stays undeclared, which binds nothing.
pub fn listed(units: &toml::value::Table, name: &str, field: &str) -> Option<Vec<String>> {
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
pub fn sanctioned(units: &toml::value::Table, user: &str, owner: &str) -> (bool, bool) {
    let invited = listed(units, owner, "visibility")
        .is_none_or(|patterns| patterns.iter().any(|pattern| fits(pattern, user)));
    let declared =
        listed(units, user, "deps").is_none_or(|declared| declared.contains(&owner.to_string()));
    (invited, declared)
}
