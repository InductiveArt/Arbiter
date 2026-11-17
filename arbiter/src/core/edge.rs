use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;

// One use of a definition: the definition by its place in the graph, and the
// file, line and unit using it.
#[derive(Clone, Serialize, Deserialize)]
pub struct Edge {
    pub def: usize,
    pub file: PathBuf,
    pub line: usize,
    pub unit: usize,
}
