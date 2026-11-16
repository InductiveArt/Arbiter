use std::path::PathBuf;

// One use of a definition: the definition by its place in the graph, and the
// file, line and unit using it.
pub struct Edge {
    pub def: usize,
    pub file: PathBuf,
    pub line: usize,
    pub unit: usize,
}
