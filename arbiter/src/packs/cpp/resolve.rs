use crate::core::pack::Def;
use crate::core::pack::Edge;
use crate::core::pack::Repo;
use crate::packs::cpp::reading::Reading;

// A call reaches a definition in a header the file includes.
pub fn edges(readings: &[Reading], mine: usize, defs: &[Def], repo: &Repo) -> Vec<Edge> {
    let mut edges = Vec::new();
    for reading in readings {
        for (index, def) in defs.iter().enumerate() {
            if def.pack != mine {
                continue;
            }
            let Some(header) = def.file.file_name().map(|name| name.to_string_lossy()) else {
                continue;
            };
            let included = reading
                .includes
                .iter()
                .any(|name| *name == header || name.ends_with(&format!("/{header}")));
            let called = reading.calls.iter().find(|call| call.name == def.name);
            if let (true, Some(call)) = (included, called) {
                edges.push(Edge {
                    def: index,
                    file: reading.file.clone(),
                    line: call.line,
                    unit: repo.unit_of(&reading.file),
                });
            }
        }
    }
    edges
}
