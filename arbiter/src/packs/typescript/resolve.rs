use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;

use crate::core::definition::Def;
use crate::core::edge::Edge;
use crate::core::repository::Repo;
use crate::packs::typescript::paths::existing;
use crate::packs::typescript::paths::normalized;
use crate::packs::typescript::reading::Reading;
use crate::packs::typescript::routes::manifest_routes;

// Each import read to the definitions it reaches: through a relative path, a
// subpath alias or a workspace package's route, passing through every
// barrel; a generated module answers for the contract it was generated from.
pub fn edges(
    readings: &[Reading],
    mine: usize,
    defs: &[Def],
    root: &Path,
    repo: &Repo,
) -> Vec<Edge> {
    let present: HashSet<&PathBuf> = repo.files.iter().collect();
    // Each re-export, resolved to the file it reaches, so a name can pass through.
    let mut barrels: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();
    for reading in readings {
        for reexport in &reading.reexports {
            let base = reading.file.parent().unwrap_or(Path::new(""));
            if let Some(target) = existing(&normalized(&base.join(&reexport.from)), &present) {
                barrels
                    .entry(reading.file.clone())
                    .or_default()
                    .push(target);
            }
        }
    }
    let routes = manifest_routes(root, repo, &present);
    let mut edges = Vec::new();
    for (file, from, name, line) in readings.iter().flat_map(|reading| {
        reading
            .imports
            .iter()
            .map(move |import| (&reading.file, &import.from, &import.name, &import.line))
    }) {
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

// The definitions a name reaches from an entry, passing through every barrel.
fn named_through(
    entry: &Path,
    name: &str,
    defs: &[Def],
    barrels: &HashMap<PathBuf, Vec<PathBuf>>,
    mine: usize,
) -> Vec<usize> {
    let mut queue = vec![entry.to_path_buf()];
    let mut seen = HashSet::new();
    let mut found = Vec::new();
    while let Some(file) = queue.pop() {
        if !seen.insert(file.clone()) {
            continue;
        }
        for (index, def) in defs.iter().enumerate() {
            if def.pack == mine && (name == "*" || def.name == name) && def.file == file {
                found.push(index);
            }
        }
        if let Some(next) = barrels.get(&file) {
            queue.extend(next.iter().cloned());
        }
    }
    found
}
