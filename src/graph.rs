use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::pack;
use crate::pack::{Def, Edge, Finding, Pack};
use crate::units;
use crate::units::Repo;

pub struct Graph {
    pub root: PathBuf,
    pub repo: Repo,
    pub defs: Vec<Def>,
    pub edges: Vec<Edge>,
    pub packs: Vec<Box<dyn Pack>>,
}

pub fn graph(asked: &str) -> Option<Graph> {
    let root = PathBuf::from(asked).canonicalize().ok()?;
    let mut packs = pack::all();
    let mut repo = units::scan(&root, &packs);
    let rooted: Vec<PathBuf> = packs
        .iter()
        .flat_map(|pack| pack.roots(&root, &repo))
        .collect();
    for file in rooted {
        let unit = repo.unit_of(&file);
        repo.units[unit].root_files.insert(file);
    }
    let mut defs: Vec<Def> = Vec::new();
    for file in &repo.files {
        let Ok(source) = fs::read_to_string(root.join(file)) else {
            continue;
        };
        let name = file
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        let Some(index) = packs.iter().position(|pack| pack.handles(&name)) else {
            continue;
        };
        let unit = repo.unit_of(file);
        // Tests, roots and generated files are leaves: nothing of theirs is judged.
        let surface = !repo.is_test(unit, file)
            && !repo.units[unit].root_files.contains(file)
            && !packs[index].generated(&name);
        for definition in packs[index].scan(file, &source, surface) {
            defs.push(Def {
                name: definition.name,
                file: file.clone(),
                line: definition.line,
                unit,
                pack: index,
                self_used: definition.self_used,
            });
        }
    }
    let mut edges = Vec::new();
    for (index, pack) in packs.iter().enumerate() {
        edges.extend(pack.resolve(index, &defs, &root, &repo));
    }
    Some(Graph {
        root,
        repo,
        defs,
        edges,
        packs,
    })
}

pub fn integrity(found: &Graph) -> Vec<Finding> {
    let mut findings = cycles(&found.defs, &found.edges, &found.repo);
    for pack in &found.packs {
        findings.extend(pack.integrity(&found.root, &found.repo));
    }
    findings.extend(one_version(&found.root, &found.repo, &found.packs));
    findings.extend(crate::declared::boundaries(found));
    findings.sort_by(|a, b| (&a.file, a.line).cmp(&(&b.file, b.line)));
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

pub fn judge(defs: &[Def], edges: &[Edge], repo: &Repo) -> Vec<Finding> {
    let mut findings = Vec::new();
    for (index, def) in defs.iter().enumerate() {
        if def.self_used {
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
            check: "economy",
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
