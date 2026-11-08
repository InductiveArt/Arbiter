use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use crate::core::pack::Def;
use crate::core::pack::Edge;
use crate::core::pack::Pack;
use crate::core::pack::Repo;
use crate::core::units;

pub struct Graph {
    pub root: PathBuf,
    pub repo: Repo,
    pub defs: Vec<Def>,
    pub edges: Vec<Edge>,
    pub packs: Vec<Box<dyn Pack>>,
    pub scan_seconds: f64,
    pub resolve_seconds: f64,
}

// The engine receives the packs: which packs exist is composition knowledge,
// and the base never knows the registry of its implementors.
pub fn graph(asked: &str, mut packs: Vec<Box<dyn Pack>>) -> Option<Graph> {
    let clock = std::time::Instant::now();
    let root = PathBuf::from(asked).canonicalize().ok()?;
    let mut repo = units::scan(&root, &packs);
    let scan_seconds = clock.elapsed().as_secs_f64();
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
        scan_seconds,
        resolve_seconds: clock.elapsed().as_secs_f64() - scan_seconds,
    })
}

// Tarjan's strongly connected components over an indexed adjacency: the one
// graph primitive every order law stands on.
struct Tarjan<'graph> {
    adjacency: &'graph [BTreeSet<usize>],
    index: Vec<Option<usize>>,
    low: Vec<usize>,
    on_stack: Vec<bool>,
    stack: Vec<usize>,
    next: usize,
    groups: Vec<Vec<usize>>,
}

pub fn strongly_connected(adjacency: &[BTreeSet<usize>]) -> Vec<Vec<usize>> {
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
