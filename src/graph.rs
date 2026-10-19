use std::fs;
use std::path::PathBuf;

use crate::pack;
use crate::pack::{Def, Edge};
use crate::units;
use crate::units::Repo;

pub struct Finding {
    pub file: PathBuf,
    pub line: usize,
    pub name: String,
    pub reason: &'static str,
}

pub fn graph(asked: &str) -> Option<(Repo, Vec<Def>, Vec<Edge>)> {
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
        // Tests and a root are leaves of the graph, offering nothing for others to use.
        let surface = !repo.is_test(unit, file) && !repo.units[unit].root_files.contains(file);
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
        edges.extend(pack.resolve(index, &defs, &repo));
    }
    Some((repo, defs, edges))
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
            file: def.file.clone(),
            line: def.line,
            name: def.name.clone(),
            reason: if users.is_empty() {
                "no user anywhere"
            } else {
                "no user but tests"
            },
        });
    }
    findings.sort_by(|a, b| (&a.file, a.line).cmp(&(&b.file, b.line)));
    findings
}
