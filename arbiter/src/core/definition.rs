use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;

// A name a file defines, as reading it gives the name: its line, and whether
// the file itself names it away from that line.
#[derive(Serialize, Deserialize)]
pub struct Definition {
    pub name: String,
    pub line: usize,
    pub self_used: bool,
}

// A definition placed in the graph: its file, its unit, and the pack that read it.
#[derive(Clone, Serialize, Deserialize)]
pub struct Def {
    pub name: String,
    pub file: PathBuf,
    pub line: usize,
    pub unit: usize,
    pub pack: usize,
    // Whether the defining file itself names the symbol away from its definition line.
    pub self_used: bool,
}
