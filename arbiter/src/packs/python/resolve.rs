use std::collections::HashSet;

use crate::core::pack::Def;
use crate::core::pack::Edge;
use crate::core::pack::Repo;
use crate::packs::python::modules::is_module_file;
use crate::packs::python::reading::Reading;

// Each import read to the definitions it reaches: the definition its module
// file holds, or, for a generated module, the definition in the contract it
// was generated from; then each dotted chain off a module imported whole.
pub fn edges(readings: &[Reading], mine: usize, defs: &[Def], repo: &Repo) -> Vec<Edge> {
    let mut edges = Vec::new();
    for reading in readings {
        let unit = repo.unit_of(&reading.file);
        for import in &reading.imports {
            // A generated module answers for the contract it was generated from, so a
            // name taken from one reaches the matching definition in the proto
            // file by its last segment, wherever the generated code sits.
            if let Some(contract) = import
                .module
                .rsplit('.')
                .next()
                .and_then(|last| last.strip_suffix("_pb2"))
            {
                let ending = format!("/{contract}.proto");
                let flat = format!("{contract}.proto");
                for (index, def) in defs.iter().enumerate() {
                    let shown = def.file.to_string_lossy();
                    if def.name == import.name && (shown.ends_with(&ending) || shown == flat) {
                        edges.push(Edge {
                            def: index,
                            file: reading.file.clone(),
                            line: import.line,
                            unit,
                        });
                    }
                }
            }
            for (index, def) in defs.iter().enumerate() {
                if def.pack != mine || def.name != import.name {
                    continue;
                }
                if is_module_file(&import.module, &def.file) {
                    edges.push(Edge {
                        def: index,
                        file: reading.file.clone(),
                        line: import.line,
                        unit,
                    });
                }
            }
        }
    }
    // A whole-module import reaches a name through a dotted chain: the longest
    // leading piece naming a module in the repository owns the segment after it.
    for reading in readings {
        let roots: HashSet<&str> = reading
            .plains
            .iter()
            .map(|plain| plain.module.split('.').next().unwrap_or(&plain.module))
            .collect();
        if roots.is_empty() {
            continue;
        }
        let unit = repo.unit_of(&reading.file);
        for chain in &reading.chains {
            let parts: Vec<&str> = chain.chain.split('.').map(str::trim).collect();
            if !roots.contains(parts[0]) {
                continue;
            }
            for split in (1..parts.len()).rev() {
                let prefix = parts[..split].join(".");
                let mut found = false;
                for (index, def) in defs.iter().enumerate() {
                    if def.pack != mine || def.name != parts[split] {
                        continue;
                    }
                    if is_module_file(&prefix, &def.file) {
                        edges.push(Edge {
                            def: index,
                            file: reading.file.clone(),
                            line: chain.line,
                            unit,
                        });
                        found = true;
                    }
                }
                if found {
                    break;
                }
            }
        }
    }
    edges
}
