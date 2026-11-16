use crate::core::pack::Def;
use crate::core::pack::Edge;
use crate::core::pack::Repo;
use crate::packs::proto::reading::Reading;

// A type reference reaches a definition in its own file, or in a file it imports.
pub fn edges(readings: &[Reading], mine: usize, defs: &[Def], repo: &Repo) -> Vec<Edge> {
    let mut edges = Vec::new();
    for reading in readings {
        for typeref in &reading.typerefs {
            let reference = &typeref.name;
            let name = reference.rsplit('.').next().unwrap_or(reference);
            for (index, def) in defs.iter().enumerate() {
                if def.pack != mine || def.name != name || def.file == reading.file {
                    continue;
                }
                let imported = reading
                    .imports
                    .iter()
                    .any(|path| def.file.to_string_lossy().ends_with(path));
                if imported {
                    edges.push(Edge {
                        def: index,
                        file: reading.file.clone(),
                        line: typeref.line,
                        unit: repo.unit_of(&reading.file),
                    });
                }
            }
        }
    }
    edges
}
