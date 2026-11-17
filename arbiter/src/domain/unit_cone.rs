use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;

// A unit's input cone as one digest: the unit by its path from the listed
// folder, and the digest of every file its tools may read.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UnitCone {
    pub unit: PathBuf,
    pub digest: String,
}
