use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

use crate::core::definition::Def;
use crate::core::edge::Edge;
use crate::core::graph::Graph;
use crate::core::pack::Pack;
use crate::core::repository::Repo;

// The graph as it travels between processes: the repository, every
// definition and every edge, and each language's readings in the order the
// languages stand in, since a definition names its language by that place.
#[derive(Clone, Serialize, Deserialize)]
pub struct GraphData {
    pub root: PathBuf,
    pub repo: Repo,
    pub defs: Vec<Def>,
    pub edges: Vec<Edge>,
    pub readings: Vec<LanguageReadings>,
}

// One language's readings, written out, under the language's name.
#[derive(Clone, Serialize, Deserialize)]
pub struct LanguageReadings {
    pub language: String,
    pub readings: Value,
}

impl GraphData {
    pub fn of(graph: &Graph) -> Result<GraphData, String> {
        let mut readings = Vec::with_capacity(graph.packs.len());
        for pack in &graph.packs {
            readings.push(LanguageReadings {
                language: pack.label().to_string(),
                readings: pack.written()?,
            });
        }
        Ok(GraphData {
            root: graph.root.clone(),
            repo: graph.repo.clone(),
            defs: graph.defs.clone(),
            edges: graph.edges.clone(),
            readings,
        })
    }

    // The graph again, its languages taking back their readings. The
    // languages must stand where they stood when the graph was read.
    pub fn into_graph(self, mut packs: Vec<Box<dyn Pack>>) -> Result<Graph, String> {
        if packs.len() != self.readings.len() {
            return Err(format!(
                "the graph was read with {} languages and {} stand now",
                self.readings.len(),
                packs.len()
            ));
        }
        for (pack, written) in packs.iter_mut().zip(self.readings) {
            if pack.label() != written.language {
                return Err(format!(
                    "the graph was read with {} where {} stands now",
                    written.language,
                    pack.label()
                ));
            }
            pack.restore(written.readings)?;
        }
        Ok(Graph {
            root: self.root,
            repo: self.repo,
            defs: self.defs,
            edges: self.edges,
            packs,
            scan_seconds: 0.0,
            resolve_seconds: 0.0,
        })
    }
}
