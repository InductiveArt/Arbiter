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
}

// The engine receives the packs: which packs exist is composition knowledge,
// and the base never knows the registry of its implementors.
pub fn graph(asked: &str, mut packs: Vec<Box<dyn Pack>>) -> Option<Graph> {
    let root = PathBuf::from(asked).canonicalize().ok()?;
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
