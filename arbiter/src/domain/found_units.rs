use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;

use crate::domain::unit::Unit;

// What finding the units leaves: every unit under the folder, and each folder
// below it holding a repository of its own, named and never read.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FoundUnits {
    pub units: Vec<Unit>,
    pub foreign: Vec<PathBuf>,
}
